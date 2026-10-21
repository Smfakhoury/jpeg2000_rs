"""Representative process measurements, separate from decoded-storage limits."""

from __future__ import annotations

import math
import os
import re
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
MAXIMUM_SEARCH_PROBES = 32


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


def _measure(command: list[str], time_tool: str, metrics_file: Path,
             expected: dict[str, Any] | None, category: str | None, reason: str | None,
             selected: bool, timeout: float, *, allow_success: bool = False,
             configured_value: int | None = None) -> dict[str, Any]:
    if (category is None or allow_success) and expected is None:
        raise ValueError("successful resource observations require an independent expected image")
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
    claim = None
    if category is None or (allow_success and process.returncode == 0):
        if process.returncode != 0 or numerical.image_error_metrics(expected, actual)["maximum_error"] != 0:
            raise ValueError("resource case did not reproduce independent image samples")
        output_hash = framework.normalized_hash(actual)
    else:
        if (process.returncode != 1 or actual.get("accepted") or actual.get("execution_failed")
                or actual.get("category") != category or reason not in output.decode(errors="replace")):
            raise ValueError(f"resource case did not produce the expected typed {category}/{reason} rejection")
        output_hash = None
        if configured_value is not None:
            if category != "LimitExceeded" or reason is None:
                raise ValueError("configured resource observations require an explicit limit resource")
            match = re.fullmatch(
                rf"ERR\tLimitExceeded\tresource limit exceeded for {re.escape(reason)}: "
                rf"limit {configured_value}, actual ([0-9]+)", output.decode().strip(),
            )
            if match is None or int(match[1]) <= configured_value:
                raise ValueError("typed rejection does not identify the configured limit and an excessive claim")
            claim = {"resource": reason, "limit": configured_value, "actual": int(match[1])}
    result: dict[str, Any] = {"elapsed_ms": elapsed, "peak_rss_bytes": _peak_rss(metrics_file),
                              "returncode": process.returncode, "output_sha256": output_hash}
    if claim is not None:
        result["limit_claim"] = claim
    return result


def _check_thresholds(measured: dict[str, Any], thresholds: dict[str, Any]) -> None:
    if measured["elapsed_ms"] > thresholds["maximum_ms"]:
        raise ValueError("measured process wall time exceeds the declared engineering threshold")
    if measured["peak_rss_bytes"] > thresholds["maximum_peak_rss_bytes"]:
        raise ValueError("measured peak process RSS exceeds the declared engineering threshold")


def _find_limit_boundary(command: list[str], resource: str, upper: int, expected: dict[str, Any],
                         time_tool: str, metrics_file: Path, timeout: float,
                         thresholds: dict[str, Any], observations: list[dict[str, Any]]) -> int:
    if type(upper) is not int or upper <= 0:
        raise ValueError("resource search requires a positive finite integer ceiling")

    def probe(value: int) -> bool:
        if len(observations) >= MAXIMUM_SEARCH_PROBES:
            raise ValueError("resource search exceeded its finite probe budget")
        measured = _measure(
            [*command, str(value)], time_tool, metrics_file, expected,
            "LimitExceeded", resource, False, timeout,
            allow_success=True, configured_value=value,
        )
        observations.append({"limit_value": value, **measured})
        _check_thresholds(measured, thresholds)
        return measured["returncode"] == 0

    if probe(0):
        raise ValueError("nonempty resource fixture unexpectedly succeeded with a zero budget")
    if not probe(upper):
        raise ValueError("resource fixture exceeds the declared search ceiling")
    lower = 0
    while upper - lower > 1:
        middle = (lower + upper) // 2
        if probe(middle):
            upper = middle
        else:
            lower = middle
    return upper


def _validate_ht_resource_headers(data: bytes, size: int) -> None:
    numerical.validate_required_headers(numerical.NumericalCase("ht-resource", channels=3, precision=16), data)
    headers = dict(numerical._main_header_segments(data))
    coding, siz, capability = headers.get(0xff52), headers.get(0xff51), headers.get(0xff50)
    if coding is None or siz is None or capability is None:
        raise ValueError("independent HT resource fixture omitted required header markers")
    if (not coding[8] & 0x40 or len(siz) < 45
            or struct.unpack_from(">IIII", siz, 2) != (size, size, 0, 0)
            or struct.unpack_from(">II", siz, 18) != (128, 128)
            or struct.unpack_from(">II", siz, 26) != (0, 0)
            or siz[34:] != b"\x00\x03" + bytes((15, 1, 1)) * 3
            or len(capability) < 4 or not int.from_bytes(capability[:4], "big") & 0x20000):
        raise ValueError("independent HT resource fixture omitted required RGB16 geometry/HT signaling")


def run_resources(executable: Path | None = None, output_dir: Path | None = None,
                  encoder: Path | None = None, decoder: Path | None = None,
                  time_tool: Path | None = None, timeout_seconds: float = 10.0, *,
                  ht_encoder: Path | None = None, ht_decoder: Path | None = None) -> dict[str, Any]:
    if not math.isfinite(timeout_seconds) or timeout_seconds <= 0:
        raise ValueError("resource timeout must be positive and finite")
    destination = framework.report_path(output_dir, "resource-results.json")
    result: dict[str, Any] = {
        "$schema": framework.report_schema(output_dir, "resource-result.schema.json"),
        "schema_version": 2, "scope": "Representative Linux engineering wall-time/RSS and configured-limit boundaries, not universal resource guarantees.",
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
                                    ("decoder", decoder, "opj_decompress"), ("time", time_tool, "time"),
                                    ("ht_encoder", ht_encoder, os.environ.get("OPENJPH_ENCODER") or "ojph_compress"),
                                    ("ht_decoder", ht_decoder, os.environ.get("OPENJPH_DECODER") or "ojph_expand")):
        resolved = shutil.which(str(explicit or default))
        if resolved is None:
            if explicit is not None or (role.startswith("ht_") and os.environ.get("OPENJPH_" + role[3:].upper())):
                raise FileNotFoundError(f"configured resource {role} is unavailable: {explicit or default}")
            result["blockers"].append(f"resource {role} unavailable: {default}")
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

            def run_case(name, command, input_path, expected, *, category=None, reason=None, selected=False,
                         configured_limit=None, search=False):
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
                    "configured_limit": dict(configured_limit) if configured_limit is not None else None,
                    "search_ceiling": configured_limit["value"] if search else None,
                    "search_runs": [],
                }
                try:
                    if search:
                        record["configured_limit"]["value"] = _find_limit_boundary(
                            command, configured_limit["resource"], configured_limit["value"],
                            expected, tools["time"]["path"], work / f"{name}-search.time",
                            timeout_seconds, record["thresholds"], record["search_runs"],
                        )
                    value = record["configured_limit"]["value"] if configured_limit is not None else None
                    measured_command = [*command, str(value)] if configured_limit is not None else command
                    for repeat in range(REPETITIONS):
                        measured = _measure(measured_command, tools["time"]["path"], work / f"{name}-{repeat}.time",
                                            expected, category, reason, selected, timeout_seconds,
                                            configured_value=value)
                        record["runs"].append(measured)
                        _check_thresholds(measured, record["thresholds"])
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

            for label, size, channels, precision, high_throughput in (
                ("gray-1024", 1024, 1, 8, False), ("rgb-512", 512, 3, 8, False),
                ("rgb16-512", 512, 3, 16, False), ("ht-rgb16-512", 512, 3, 16, True),
            ):
                case = numerical.NumericalCase(label, width=size, height=size, channels=channels, precision=precision)
                suffix = "ppm" if channels == 3 else "pgm"
                source, encoded = work / f"{label}.{suffix}", work / f"{label}.j2k"
                source.write_bytes(numerical.source_image(case))
                encode = tools["ht_encoder" if high_throughput else "encoder"]["path"]
                decode = tools["ht_decoder" if high_throughput else "decoder"]["path"]
                encoding_options = (["-num_decomps", "2", "-reversible", "true", "-tile_size", "{128,128}"]
                                    if high_throughput else ["-n", "3", "-t", "128,128"])
                numerical._run_checked([encode, "-i", str(source), "-o", str(encoded), *encoding_options], timeout_seconds)
                if high_throughput:
                    _validate_ht_resource_headers(encoded.read_bytes(), size)
                full_reference = work / f"{label}-full-reference.{suffix}"
                numerical._run_checked([decode, "-i", str(encoded), "-o", str(full_reference)], timeout_seconds)
                full_expected = framework.parse_pnm(full_reference)
                if framework.normalized_hash(full_expected) != framework.normalized_hash(framework.parse_pnm(source)):
                    raise ValueError("independent resource fixture failed its lossless self-roundtrip")
                records = []
                for mode, reference_options, selection in (
                    ("full", [], None), ("tile0", ["-d", "0,0,128,128"], ["0", "0", "all"]),
                    ("native2", ["-skip_res" if high_throughput else "-r", "2"], ["2", "all", "all"]),
                ):
                    if mode == "full":
                        expected = full_expected
                    elif high_throughput and mode == "tile0":
                        expected = framework.crop_image(full_expected, (0, 0, size, size), (0, 0, 128, 128))
                    else:
                        reference = work / f"{label}-{mode}.{suffix}"
                        numerical._run_checked([decode, "-i", str(encoded), "-o", str(reference),
                                                *reference_options], timeout_seconds)
                        expected = framework.parse_pnm(reference)
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
            for case in (
                numerical.NumericalCase("gray24-small", precision=24),
                numerical.NumericalCase("rgb16-layered", channels=3, precision=16,
                                       options=("-r", "8,3,1", "-t", "16,16", "-d", "3,5")),
            ):
                extension = numerical.source_extension(case)
                source = work / f"{case.name}.{extension}"
                encoded = work / f"{case.name}.j2k"
                reference = work / f"{case.name}-reference.{extension}"
                source.write_bytes(numerical.source_image(case))
                numerical._run_checked([tools["encoder"]["path"], "-i", str(source), "-o", str(encoded),
                                        "-n", "3", *case.options], timeout_seconds)
                if case.channels == 3:
                    data = encoded.read_bytes()
                    required_cod = framework.segment(0xff52, bytes((0, 0, 0, 3, 1, 2, 4, 4, 0, 1)))
                    if (len(data) < 32 or data[:4] != b"\xff\x4f\xff\x51"
                            or struct.unpack_from(">IIII", data, 16) != (3, 5, 16, 16)
                            or required_cod not in data.split(b"\xff\x90", 1)[0]):
                        raise ValueError("independent resource fixture omitted the required tiling/origin/three-layer headers")
                numerical._run_checked([tools["decoder"]["path"], "-i", str(encoded), "-o", str(reference)],
                                       timeout_seconds)
                parse_image = framework.parse_pgx if extension == "pgx" else framework.parse_pnm
                if extension == "pgx":
                    reference = reference.with_stem(reference.stem + "_0")
                expected = parse_image(reference)
                if framework.normalized_hash(expected) != framework.normalized_hash(parse_image(source)):
                    raise ValueError("independent resource-boundary fixture failed its lossless self-roundtrip")
                for selector, resource, ceiling in (
                    ("decoded-bytes", "DecodedBytes", 16 << 20),
                    ("work-units", "WorkUnits", 10_000_000),
                ):
                    command = [str(executable), "decode-limited", "raw", str(encoded), selector]
                    record = run_case(
                        f"{case.name}-{selector}-at", command, encoded, expected,
                        configured_limit={"resource": resource, "value": ceiling}, search=True,
                    )
                    if record["status"] == "passed":
                        run_case(
                            f"{case.name}-{selector}-below", command, encoded, None,
                            category="LimitExceeded", reason=resource,
                            configured_limit={"resource": resource, "value": record["configured_limit"]["value"] - 1},
                        )
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
        result["native_total_ms"] = sum(run["elapsed_ms"] for record in result["records"]
                                      for run in [*record["runs"], *record["search_runs"]])
        result["errors"] = [f"{record['id']}: {record['error']}" for record in result["records"]
                            if record["status"] != "passed"]
        if result["native_total_ms"] > result["maximum_native_total_ms"]:
            result["errors"].append("aggregate measured native wall time exceeds the declared engineering threshold")
        if result["errors"]:
            result["status"] = "failed"
    framework.validate_schema(framework.VALIDATION / "resource-result.schema.json", result)
    framework.write_json(destination, result)
    return result
