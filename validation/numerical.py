"""Independent, generated-image regression checks; not official conformance."""

from __future__ import annotations

import math
import os
import shutil
import subprocess
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import framework


@dataclass(frozen=True)
class NumericalCase:
    name: str
    channels: int = 1
    precision: int = 8
    format: str = "raw"
    options: tuple[str, ...] = ()
    lossless: bool = True
    maximum_error: int = 0
    maximum_rmse: float = 0.0
    width: int = 32
    height: int = 32


CASES = (
    NumericalCase("gray-lossless"),
    NumericalCase("gray-tiles-origin", options=("-t", "16,16", "-d", "3,5", "-p", "RPCL")),
    NumericalCase("gray16-lossless", precision=16, options=("-p", "CPRL", "-SOP", "-EPH", "-PLT")),
    NumericalCase("rgb-lossless", channels=3, options=("-p", "PCRL")),
    NumericalCase("gray-layers", options=("-r", "8,3,1", "-p", "RLCP")),
    NumericalCase("rgb-jp2", channels=3, format="jp2"),
    *(NumericalCase(f"coding-style-{style}", options=("-M", str(style)))
      for style in (1, 2, 4, 8, 16, 32, 38, 63)),
    *(NumericalCase(f"layered-style-{style}", options=("-r", "8,3,1", "-M", str(style)))
      for style in (1, 16, 63)),
    NumericalCase("gray-irreversible", options=("-I", "-r", "1"),
                  lossless=False, maximum_error=1, maximum_rmse=0.25),
    NumericalCase("rgb-irreversible", channels=3, options=("-I", "-r", "1"),
                  lossless=False, maximum_error=2, maximum_rmse=0.5),
    NumericalCase("gray16-irreversible", precision=16, options=("-I", "-r", "1"),
                  lossless=False, maximum_error=8, maximum_rmse=2.0),
    NumericalCase("representative-gray", width=256, height=256),
    NumericalCase("representative-rgb", channels=3, width=256, height=256),
    NumericalCase("representative-gray16", precision=16, width=256, height=256),
)


def source_image(case: NumericalCase) -> bytes:
    peak = (1 << case.precision) - 1
    payload = b"".join(
        (0 if x == 0 and y == 0 else peak if x == 1 and y == 0 else
         ((x * 31 + y * 17 + channel * 71 + x * y * 13) % 256) * peak // 255)
        .to_bytes(case.precision // 8, "big")
        for y in range(case.height) for x in range(case.width) for channel in range(case.channels)
    )
    return f"P{6 if case.channels == 3 else 5}\n{case.width} {case.height}\n{peak}\n".encode() + payload


def image_error_metrics(reference: dict[str, Any], actual: dict[str, Any]) -> dict[str, Any]:
    if not reference.get("accepted") or not actual.get("accepted"):
        raise ValueError("numerical comparison requires two accepted images")
    expected_image, actual_image = reference["image"], actual["image"]
    if framework._image_metrics(reference) != framework._image_metrics(actual):
        raise ValueError("numerical comparison geometry or component metadata differs")
    components = []
    total_squared_error = 0
    total_samples = 0
    for expected, observed in zip(expected_image["components"], actual_image["components"]):
        count = expected["width"] * expected["height"]
        if count <= 0 or len(expected["samples"]) != count or len(observed["samples"]) != count:
            raise ValueError("numerical comparison sample count differs from geometry")
        peak = (1 << expected["precision"]) - 1
        lower = -(1 << (expected["precision"] - 1)) if expected["signed"] else 0
        upper = lower + peak
        if any(type(value) is not int or not lower <= value <= upper
               for value in expected["samples"] + observed["samples"]):
            raise ValueError("numerical comparison sample exceeds declared precision")
        errors = [left - right for left, right in zip(expected["samples"], observed["samples"])]
        squared_error = sum(error * error for error in errors)
        mse = squared_error / count
        components.append({
            "samples": count,
            "maximum_error": max(abs(error) for error in errors),
            "mse": mse,
            "rmse": math.sqrt(mse),
            "psnr_db": 10 * math.log10(peak * peak / mse) if mse else None,
        })
        total_squared_error += squared_error
        total_samples += count
    if not components:
        raise ValueError("numerical comparison has no components")
    return {
        "maximum_error": max(component["maximum_error"] for component in components),
        "rmse": math.sqrt(total_squared_error / total_samples),
        "worst_component_rmse": max(component["rmse"] for component in components),
        "components": components,
    }


def _limit_native_process() -> None:
    if framework.resource is not None:
        framework.resource.setrlimit(framework.resource.RLIMIT_AS, (512 << 20, 512 << 20))
        framework.resource.setrlimit(framework.resource.RLIMIT_FSIZE, (2 << 20, 2 << 20))


def _run_checked(command: list[str], timeout_seconds: float) -> tuple[subprocess.CompletedProcess[bytes], float]:
    process, elapsed_ms, limited = framework._run_process_bounded(
        command, timeout_seconds, 1 << 20,
        _limit_native_process if framework.resource is not None else None,
    )
    if limited or process.returncode != 0:
        message = (process.stdout + process.stderr).decode("utf-8", "replace")[-4096:]
        raise ValueError(f"{Path(command[0]).name} failed (exit {process.returncode}, output_limit={limited}): {message}")
    return process, elapsed_ms


def run_numerical(
    executable: Path | None = None,
    output_dir: Path | None = None,
    encoder: Path | None = None,
    decoder: Path | None = None,
    timeout_seconds: float = 10.0,
) -> dict[str, Any]:
    if not math.isfinite(timeout_seconds) or timeout_seconds <= 0:
        raise ValueError("numerical timeout must be positive and finite")
    destination = framework.report_path(output_dir, "numerical-results.json")
    tools = {}
    missing = []
    for role, explicit, environment, default in (
        ("encoder", encoder, "OPENJPEG_ENCODER", "opj_compress"),
        ("decoder", decoder, "OPENJPEG_DECODER", "opj_decompress"),
    ):
        configured = explicit or os.environ.get(environment)
        resolved = shutil.which(str(configured or default))
        if resolved is None:
            if configured:
                raise FileNotFoundError(f"configured numerical {role} is unavailable: {configured}")
            missing.append(default)
        else:
            tools[role] = {"path": resolved, "sha256": framework.sha256_file(Path(resolved))}
    result: dict[str, Any] = {
        "$schema": framework.report_schema(output_dir, "numerical-result.schema.json"),
        "schema_version": 1,
        "scope": "Generated-image engineering regression thresholds, not official numerical conformance.",
        "provenance": framework.source_binding(),
        "tools": tools,
        "status": "blocked" if missing else "passed",
        "blockers": [f"required OpenJPEG tool unavailable: {name}" for name in missing],
        "summary": {"cases": 0, "passed": 0, "failed": 0},
        "records": [],
    }
    if not missing:
        executable = executable or framework.build_subject()
        tools["subject"] = {"path": str(executable), "sha256": framework.sha256_file(executable)}
        destination.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="numerical-", dir=destination.parent) as directory:
            work = Path(directory)
            for case in CASES:
                start = time.monotonic()
                record: dict[str, Any] = {
                    "id": case.name, "lossless": case.lossless,
                    "width": case.width, "height": case.height,
                    "encoder_options": ["-n", "3", *case.options],
                    "thresholds": {"maximum_error": case.maximum_error, "maximum_rmse": case.maximum_rmse,
                                   "maximum_case_ms": timeout_seconds * 3000},
                    "status": "failed", "error": None, "metrics": None, "source_metrics": None,
                    "artifacts": [],
                }
                source = work / f"{case.name}.pnm"
                encoded = work / f"{case.name}.{'jp2' if case.format == 'jp2' else 'j2k'}"
                reference = work / f"{case.name}-reference.pnm"
                source.write_bytes(source_image(case))
                record["input_sha256"] = framework.sha256_file(source)
                try:
                    _, encoder_ms = _run_checked([tools["encoder"]["path"], "-i", str(source), "-o", str(encoded),
                                                  *record["encoder_options"]], timeout_seconds)
                    record["codestream_sha256"] = framework.sha256_file(encoded)
                    _, reference_ms = _run_checked([tools["decoder"]["path"], "-i", str(encoded), "-o", str(reference)], timeout_seconds)
                    expected = framework.parse_pnm(reference)
                    process, subject_ms = _run_checked([str(executable), "decode", case.format, str(encoded)], timeout_seconds)
                    record["timings"] = {"encoder_ms": encoder_ms, "reference_ms": reference_ms, "subject_ms": subject_ms}
                    actual = framework.parse_subject_output(process.stdout, process.stderr, process.returncode)
                    original = framework.parse_pnm(source)
                    if case.lossless and framework.normalized_hash(original) != framework.normalized_hash(expected):
                        raise ValueError("independent lossless encoder/decoder changed original samples")
                    metrics = image_error_metrics(expected, actual)
                    record["metrics"] = metrics
                    record["source_metrics"] = image_error_metrics(original, actual)
                    record["reference_sha256"] = framework.normalized_hash(expected)
                    record["decoded_sha256"] = framework.normalized_hash(actual)
                    if (metrics["maximum_error"] > case.maximum_error
                            or metrics["worst_component_rmse"] > case.maximum_rmse):
                        raise ValueError("decoded samples exceed declared engineering error thresholds")
                    record["status"] = "passed"
                except (OSError, ValueError, subprocess.SubprocessError) as error:
                    record["error"] = str(error)
                record["elapsed_ms"] = (time.monotonic() - start) * 1000
                if record["status"] == "passed" and record["elapsed_ms"] > record["thresholds"]["maximum_case_ms"]:
                    record["status"] = "failed"
                    record["error"] = "case execution exceeded declared aggregate time budget"
                if record["status"] == "failed":
                    for path in (source, encoded, reference):
                        if path.is_file():
                            saved = framework.report_path(
                                output_dir, f"numerical-failures/{case.name}/{path.name}",
                            )
                            saved.parent.mkdir(parents=True, exist_ok=True)
                            shutil.copyfile(path, saved)
                            record["artifacts"].append(saved.relative_to(destination.parent).as_posix())
                result["records"].append(record)
        passed = sum(record["status"] == "passed" for record in result["records"])
        result["summary"] = {"cases": len(CASES), "passed": passed, "failed": len(CASES) - passed}
        if passed != len(CASES):
            result["status"] = "failed"
    framework.validate_schema(framework.VALIDATION / "numerical-result.schema.json", result)
    framework.write_json(destination, result)
    return result
