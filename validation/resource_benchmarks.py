"""Representative process measurements, separate from decoded-storage limits."""

from __future__ import annotations

import math
import shutil
import statistics
import struct
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

import framework
import numerical

ADDRESS_SPACE_BYTES = 1 << 30
REPETITIONS = 3


def _limit_process() -> None:
    if framework.resource is None:
        raise RuntimeError("POSIX process limits are unavailable")
    framework.resource.setrlimit(framework.resource.RLIMIT_AS, (ADDRESS_SPACE_BYTES, ADDRESS_SPACE_BYTES))
    framework.resource.setrlimit(framework.resource.RLIMIT_FSIZE, (32 << 20, 32 << 20))


def _peak_rss(path: Path) -> int:
    lines = path.read_text().splitlines()
    if not lines:
        raise ValueError("GNU Time did not report peak RSS")
    value = int(lines[-1]) * 1024
    if value <= 0:
        raise ValueError("GNU Time reported an unavailable peak RSS")
    return value


def _measure(command, time_tool, metrics_file, expected, category, reason, selected, timeout):
    process, elapsed, limited = framework._run_process_bounded(
        [time_tool, "-f", "%M", "-o", str(metrics_file), *command],
        timeout, 16 << 20, _limit_process,
    )
    if limited:
        raise ValueError("resource observation exceeded the bounded output capture")
    output = process.stdout
    if selected:
        frame, separator, output = output.partition(b"\n")
        if not separator or frame.split(b"\t") != [b"FRAME", b"0", b"0"]:
            raise ValueError("selected resource observation has an unexpected origin")
    actual = framework.parse_subject_output(output, process.stderr, process.returncode)
    if category is None:
        if process.returncode != 0 or numerical.image_error_metrics(expected, actual)["maximum_error"] != 0:
            raise ValueError("resource case did not reproduce independent image samples")
        output_hash = framework.normalized_hash(actual)
    else:
        if (process.returncode != 1 or actual.get("accepted") or actual.get("execution_failed")
                or actual.get("category") != category or reason not in output.decode(errors="replace")):
            raise ValueError(f"resource case did not produce the expected typed {category}/{reason} rejection")
        output_hash = None
    return {"elapsed_ms": elapsed, "peak_rss_bytes": _peak_rss(metrics_file),
            "returncode": process.returncode, "output_sha256": output_hash}


def run_resources(executable: Path | None = None, output_dir: Path | None = None,
                  encoder: Path | None = None, decoder: Path | None = None,
                  time_tool: Path | None = None, timeout_seconds: float = 10.0) -> dict[str, Any]:
    if not math.isfinite(timeout_seconds) or timeout_seconds <= 0:
        raise ValueError("resource timeout must be positive and finite")
    destination = framework.report_path(output_dir, "resource-results.json")
    result: dict[str, Any] = {
        "$schema": framework.report_schema(output_dir, "resource-result.schema.json"),
        "schema_version": 1, "scope": "Representative Linux engineering wall-time/RSS bounds, not universal resource guarantees.",
        "provenance": framework.source_binding(), "status": "passed", "blockers": [], "errors": [],
        "tools": {}, "address_space_limit_bytes": ADDRESS_SPACE_BYTES,
        "repetitions": REPETITIONS, "records": [], "native_total_ms": 0.0,
        "maximum_native_total_ms": 30000.0,
        "maximum_selection_rss_ratio": 0.75,
        "subject_build": "provided" if executable is not None else "cargo-release",
    }
    if sys.platform != "linux" or framework.resource is None:
        result["blockers"].append("Linux GNU Time and POSIX address-space enforcement are required")
    for role, explicit, default in (("encoder", encoder, "opj_compress"),
                                    ("decoder", decoder, "opj_decompress"), ("time", time_tool, "time")):
        resolved = shutil.which(str(explicit or default))
        if resolved is None:
            if explicit is not None:
                raise FileNotFoundError(f"configured resource {role} is unavailable: {explicit}")
            result["blockers"].append(f"resource tool unavailable: {default}")
        else:
            result["tools"][role] = {"path": resolved, "sha256": framework.sha256_file(Path(resolved))}
    if result["blockers"]:
        result["status"] = "blocked"
    else:
        tools = result["tools"]
        version = subprocess.run([tools["time"]["path"], "--version"], capture_output=True, check=True)
        if b"GNU" not in version.stdout:
            raise ValueError("resource measurements require GNU Time")
        executable = executable or framework.build_subject(release=True)
        tools["subject"] = {"path": str(executable), "sha256": framework.sha256_file(executable)}
        destination.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="resources-", dir=destination.parent) as directory:
            work = Path(directory)

            def run_case(name, command, input_path, expected, *, category=None, reason=None, selected=False):
                record = {
                    "id": name, "input_bytes": input_path.stat().st_size,
                    "input_sha256": framework.sha256_file(input_path),
                    "expected_category": category, "expected_reason": reason,
                    "expected_output_sha256": framework.normalized_hash(expected) if expected is not None else None,
                    "typed_sample_bytes": sum(len(c["samples"]) * 8 for c in expected["image"]["components"])
                    if expected is not None else 0,
                    "output_width": expected["image"]["width"] if expected is not None else 0,
                    "output_height": expected["image"]["height"] if expected is not None else 0,
                    "output_components": len(expected["image"]["components"]) if expected is not None else 0,
                    "thresholds": {"maximum_ms": 1000.0 if category else 2500.0 if selected else 5000.0,
                                   "maximum_peak_rss_bytes": (128 if category else 256 if selected else 512) << 20},
                    "runs": [], "status": "failed", "error": None, "artifacts": [],
                }
                try:
                    for repeat in range(REPETITIONS):
                        measured = _measure(command, tools["time"]["path"], work / f"{name}-{repeat}.time",
                                            expected, category, reason, selected, timeout_seconds)
                        record["runs"].append(measured)
                        if measured["elapsed_ms"] > record["thresholds"]["maximum_ms"]:
                            raise ValueError("measured process wall time exceeds the declared engineering threshold")
                        if measured["peak_rss_bytes"] > record["thresholds"]["maximum_peak_rss_bytes"]:
                            raise ValueError("measured peak process RSS exceeds the declared engineering threshold")
                    record["median_ms"] = statistics.median(run["elapsed_ms"] for run in record["runs"])
                    record["peak_rss_bytes"] = max(run["peak_rss_bytes"] for run in record["runs"])
                    record["status"] = "passed"
                except (OSError, ValueError, subprocess.SubprocessError) as error:
                    record["error"] = str(error)
                    failure = framework.report_path(output_dir, f"resource-failures/{name}/{record['input_sha256']}/{input_path.name}")
                    failure.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(input_path, failure)
                    record["artifacts"].append(failure.relative_to(destination.parent).as_posix())
                result["records"].append(record)
                return record

            for label, size, channels in (("gray-1024", 1024, 1), ("rgb-512", 512, 3)):
                case = numerical.NumericalCase(label, width=size, height=size, channels=channels)
                suffix = "ppm" if channels == 3 else "pgm"
                source, encoded = work / f"{label}.{suffix}", work / f"{label}.j2k"
                source.write_bytes(numerical.source_image(case))
                numerical._run_checked([tools["encoder"]["path"], "-i", str(source), "-o", str(encoded),
                                        "-n", "3", "-t", "128,128"], timeout_seconds)
                records = []
                for mode, reference_options, selection in (
                    ("full", [], None), ("tile0", ["-d", "0,0,128,128"], ["0", "0", "all"]),
                    ("native2", ["-r", "2"], ["2", "all", "all"]),
                ):
                    reference = work / f"{label}-{mode}.{suffix}"
                    numerical._run_checked([tools["decoder"]["path"], "-i", str(encoded), "-o", str(reference),
                                            *reference_options], timeout_seconds)
                    expected = framework.parse_pnm(reference)
                    if mode == "full" and framework.normalized_hash(expected) != framework.normalized_hash(framework.parse_pnm(source)):
                        raise ValueError("independent resource fixture failed its lossless self-roundtrip")
                    command = [str(executable), "select" if selection else "decode", "raw", str(encoded),
                               *(selection or [])]
                    records.append(run_case(f"{label}-{mode}", command, encoded, expected, selected=selection is not None))
                if all(record["status"] == "passed" for record in records):
                    if records[1]["peak_rss_bytes"] > result["maximum_selection_rss_ratio"] * records[0]["peak_rss_bytes"]:
                        records[1]["status"] = "failed"
                        records[1]["error"] = "tile selection did not reduce peak RSS by the required 25 percent"
                        failure = framework.report_path(output_dir, f"resource-failures/{label}-rss-ratio/{encoded.name}")
                        failure.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copyfile(encoded, failure)
                        records[1]["artifacts"].append(failure.relative_to(destination.parent).as_posix())
            limit_input = work / "declared-pixel-limit.j2k"
            data = bytearray(framework.raw_codestream())
            struct.pack_into(">II", data, 8, 65535, 65535)
            struct.pack_into(">II", data, 24, 65535, 65535)
            limit_input.write_bytes(data)
            run_case("declared-pixel-limit", [str(executable), "decode", "raw", str(limit_input)],
                     limit_input, None, category="LimitExceeded", reason="Pixels")
            truncated = work / "truncated-main-header.j2k"
            truncated.write_bytes(framework.raw_codestream()[:12])
            run_case("truncated-main-header", [str(executable), "decode", "raw", str(truncated)],
                     truncated, None, category="MalformedInput", reason="Truncated")
        result["native_total_ms"] = sum(run["elapsed_ms"] for record in result["records"] for run in record["runs"])
        result["errors"] = [f"{record['id']}: {record['error']}" for record in result["records"]
                            if record["status"] != "passed"]
        if result["native_total_ms"] > result["maximum_native_total_ms"]:
            result["errors"].append("aggregate measured native wall time exceeds the declared engineering threshold")
        if result["errors"]:
            result["status"] = "failed"
    framework.validate_schema(framework.VALIDATION / "resource-result.schema.json", result)
    framework.write_json(destination, result)
    return result
