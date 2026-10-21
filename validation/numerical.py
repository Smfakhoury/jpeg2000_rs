"""Independent, generated-image regression checks; not official conformance."""

from __future__ import annotations

import math
import os
import platform
import shutil
import struct
import subprocess
import tempfile
import time
from dataclasses import dataclass, replace
from collections.abc import Iterable
from pathlib import Path
from typing import Any

import framework

NATIVE_ADDRESS_BYTES = 512 << 20
NATIVE_FILE_BYTES = 2 << 20
NATIVE_OUTPUT_BYTES = 1 << 20


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
    decomposition_levels: int = 2
    pattern: str = "varied"
    samples: tuple[int, ...] | None = None
    codestream: bytes | None = None
    signed: bool = False


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
    NumericalCase("signed8-lossless", signed=True),
    NumericalCase("signed9-odd-grid", signed=True, precision=9, width=33, height=29),
    NumericalCase("signed12-tiles-origin", signed=True, precision=12,
                  options=("-t", "16,16", "-d", "3,3", "-p", "RPCL", "-M", "8")),
    NumericalCase("signed16-jp2", signed=True, precision=16, format="jp2"),
    NumericalCase("signed17-layers-bypass", signed=True, precision=17,
                  options=("-r", "8,3,1", "-M", "1")),
    NumericalCase("signed24-lossless", signed=True, precision=24, width=33, height=29),
    NumericalCase("gray12-lossless", precision=12),
    NumericalCase("gray20-lossless", precision=20),
    NumericalCase("gray24-jp2", precision=24, format="jp2"),
)

def _ht_test_codestream(width: int, packet: bytes, exponent: int = 7) -> bytes:
    siz = struct.pack(">HIIIIIIIIH", 0x4000, width, 1, 0, 0, width, 1, 0, 0, 1) + bytes((7, 1, 1))
    data = b"\xff\x4f" + framework.segment(0xff51, siz)
    data += framework.segment(0xff50, bytes((0, 2, 0, 0, 0, 0)))
    data += framework.segment(0xff52, bytes((0, 2, 0, 1, 0, 0, 4, 4, 0x40, 1)))
    data += framework.segment(0xff5c, bytes((0x20, exponent << 3)))
    data += framework.segment(0xff90, struct.pack(">HI2B", 0, 14 + len(packet), 0, 1))
    return data + b"\xff\x93" + packet + b"\xff\xd9"


def _ht_refinement_cases() -> tuple[NumericalCase, ...]:
    cases = []
    for negative in (False, True):
        cleanup = bytes((0x00, 0x63, 0x00)) if negative else bytes((0xfe, 0x00, 0x63, 0x00))
        for bit in (0, 1):
            fields = "11" + "0" * 5 + "1" + "1100" + "0"
            fields += f"{len(cleanup):03b}" + "0001"
            fields += "0" * (-len(fields) % 8)
            header = int(fields, 2).to_bytes(len(fields) // 8, "big")
            value = 128 + (-1 if negative else 1) * (2 + bit)
            cases.append(NumericalCase(
                f"ht-mrp-{'negative' if negative else 'positive'}-{bit}",
                width=1, height=1, decomposition_levels=0, format="jph", pattern="fixed",
                samples=(value,), codestream=_ht_test_codestream(1, header + cleanup + bytes((bit,))),
            ))
        fields = "11" + "0" * 5 + "1" + "10" + "0" + f"{len(cleanup):03b}" + "001"
        fields += "0" * (-len(fields) % 8)
        header = int(fields, 2).to_bytes(len(fields) // 8, "big")
        values = (125, 127) if negative else (131, 129)
        cases.append(NumericalCase(
            f"ht-spp-{'negative' if negative else 'positive'}",
            width=2, height=1, decomposition_levels=0, format="jph", pattern="fixed",
            samples=values, codestream=_ht_test_codestream(2, header + cleanup + bytes((3 if negative else 1,))),
        ))
    for bit in (0, 1):
        fields = "11" + "0" * 5 + "1" + "1100" + "0" + "100" + "0010"
        fields += "0" * (-len(fields) % 8)
        header = int(fields, 2).to_bytes(len(fields) // 8, "big")
        cases.append(NumericalCase(
            f"ht-spp-mrp-{bit}", width=2, height=1, decomposition_levels=0,
            format="jph", pattern="fixed", samples=(130 + bit, 129),
            codestream=_ht_test_codestream(2, header + bytes((0xfe, 0x00, 0x63, 0x00, 1, bit))),
        ))
    cases.append(NumericalCase(
        "ht-full-negative-range", width=1, height=1, decomposition_levels=0,
        format="jph", pattern="fixed", samples=(0,),
        codestream=_ht_test_codestream(1, bytes((0xc0, 0x48, 0x01, 0x07, 0x74, 0x00)), 8),
    ))
    return tuple(cases)


HT_CASES = (
    NumericalCase("ht-gray-lossless"),
    NumericalCase("ht-rgb-lossless", channels=3),
    NumericalCase("ht-gray16-lossless", precision=16),
    NumericalCase("ht-odd-grid", width=31, height=29),
    NumericalCase("ht-sparse-gray", pattern="sparse"),
    NumericalCase("ht-sparse-rgb", channels=3, pattern="sparse"),
    NumericalCase("ht-sparse-gray16", precision=16, pattern="sparse"),
    NumericalCase("ht-tiles-origin", width=37, height=29,
                  options=("-tile_size", "{16,16}", "-image_offset", "{3,5}", "-prog_order", "CPRL")),
    *(NumericalCase(f"ht-progression-{order}", options=("-prog_order", order))
      for order in ("LRCP", "RLCP", "PCRL", "CPRL")),
    NumericalCase("ht-single-low", width=1, height=1, decomposition_levels=1),
    NumericalCase("ht-jph-gray", format="jph"),
    NumericalCase("ht-jph-rgb", channels=3, format="jph"),
    NumericalCase("ht-lossy-gray", lossless=False, maximum_error=1, maximum_rmse=0.25),
    NumericalCase("ht-lossy-rgb", channels=3, lossless=False, maximum_error=2, maximum_rmse=0.5),
    NumericalCase("ht-lossy-gray16", precision=16, lossless=False, maximum_error=8, maximum_rmse=2.0),
    NumericalCase("ht-representative-gray", width=256, height=256),
    NumericalCase("ht-representative-rgb", channels=3, width=256, height=256),
    NumericalCase("ht-representative-gray16", precision=16, width=256, height=256),
    *_ht_refinement_cases(),
)


def source_extension(case: NumericalCase) -> str:
    if case.signed or case.precision not in (8, 16):
        if case.channels != 1:
            raise ValueError("generated PGX profile requires one component")
        return "pgx"
    return "ppm" if case.channels == 3 else "pgm"


def source_image(case: NumericalCase) -> bytes:
    if type(case.precision) is not int or not 1 <= case.precision <= 32 or type(case.signed) is not bool:
        raise ValueError("generated source requires precision 1..32 and boolean signedness")
    if case.pattern not in ("varied", "sparse", "fixed"):
        raise ValueError("unknown generated image pattern")
    extension = source_extension(case)
    peak = (1 << case.precision) - 1
    lower = -(1 << (case.precision - 1)) if case.signed else 0
    values: Iterable[int]
    if case.pattern == "fixed":
        if (case.samples is None or len(case.samples) != case.width * case.height * case.channels
                or any(type(value) is not int or not lower <= value <= lower + peak for value in case.samples)):
            raise ValueError("fixed generated samples must match geometry and precision")
        values = case.samples
    else:
        if case.samples is not None:
            raise ValueError("explicit generated samples require the fixed pattern")
        values = (
        lower + (0 if x == 0 and y == 0 else peak if x == 1 and y == 0 else
         (peak if (x + 3 * y + 17 * channel) % 97 == 0 else (peak + 1) // 2)
         if case.pattern == "sparse" else
         ((x * 31 + y * 17 + channel * 71 + x * y * 13) % 256) * peak // 255)
        for y in range(case.height) for x in range(case.width) for channel in range(case.channels)
        )
    sample_bytes = 1 if case.precision <= 8 else 2 if case.precision <= 16 else 4
    payload = b"".join(value.to_bytes(sample_bytes, "big", signed=case.signed) for value in values)
    if extension == "pgx":
        sign = "-" if case.signed else "+"
        return f"PG ML {sign} {case.precision} {case.width} {case.height}\n".encode() + payload
    return f"P{6 if case.channels == 3 else 5}\n{case.width} {case.height}\n{peak}\n".encode() + payload


def signed_codestream(data: bytes, precision: int) -> bytes:
    if (len(data) < 45 or data[:4] != b"\xff\x4f\xff\x51"
            or int.from_bytes(data[4:6], "big") != 41
            or int.from_bytes(data[40:42], "big") != 1
            or data[42] != precision - 1):
        raise ValueError("signed fixture requires a matching unsigned single-component SIZ header")
    # The offset unsigned input has the same centered coefficients as the signed source.
    return data[:42] + bytes((data[42] | 0x80,)) + data[43:]


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
        framework.resource.setrlimit(framework.resource.RLIMIT_AS, (NATIVE_ADDRESS_BYTES, NATIVE_ADDRESS_BYTES))
        framework.resource.setrlimit(framework.resource.RLIMIT_FSIZE, (NATIVE_FILE_BYTES, NATIVE_FILE_BYTES))


def _run_checked(command: list[str], timeout_seconds: float) -> tuple[subprocess.CompletedProcess[bytes], float]:
    process, elapsed_ms, limited = framework._run_process_bounded(
        command, timeout_seconds, NATIVE_OUTPUT_BYTES,
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
    high_throughput: bool = False,
) -> dict[str, Any]:
    if not math.isfinite(timeout_seconds) or timeout_seconds <= 0:
        raise ValueError("numerical timeout must be positive and finite")
    report_name = "htj2k-results.json" if high_throughput else "numerical-results.json"
    reference_name = "OpenJPH" if high_throughput else "OpenJPEG"
    cases = HT_CASES if high_throughput else CASES
    destination = framework.report_path(output_dir, report_name)
    tools = {}
    missing = []
    specifications = (
        ("encoder", encoder, "OPENJPH_ENCODER", "ojph_compress"),
        ("decoder", decoder, "OPENJPH_DECODER", "ojph_expand"),
    ) if high_throughput else (
        ("encoder", encoder, "OPENJPEG_ENCODER", "opj_compress"),
        ("decoder", decoder, "OPENJPEG_DECODER", "opj_decompress"),
    )
    for role, explicit, environment, default in specifications:
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
        "scope": f"{reference_name} generated-image engineering regression thresholds, not official numerical conformance.",
        "execution_platform": platform.platform(),
        "process_limits": {
            "stage_timeout_seconds": timeout_seconds,
            "maximum_output_bytes": NATIVE_OUTPUT_BYTES,
            "requested_address_space_bytes": NATIVE_ADDRESS_BYTES if framework.resource is not None else None,
            "requested_file_size_bytes": NATIVE_FILE_BYTES if framework.resource is not None else None,
        },
        "provenance": framework.source_binding(),
        "tools": tools,
        "status": "blocked" if missing else "passed",
        "blockers": [f"required {reference_name} tool unavailable: {name}" for name in missing],
        "summary": {"cases": 0, "passed": 0, "failed": 0},
        "records": [],
    }
    if not missing:
        executable = executable or framework.build_subject()
        tools["subject"] = {"path": str(executable), "sha256": framework.sha256_file(executable)}
        destination.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="numerical-", dir=destination.parent) as directory:
            work = Path(directory)
            for case in cases:
                start = time.monotonic()
                encoder_options = (
                    ["-num_decomps", str(case.decomposition_levels), "-reversible", str(case.lossless).lower()]
                    if high_throughput else ["-n", str(case.decomposition_levels + 1)]
                )
                record: dict[str, Any] = {
                    "id": case.name, "lossless": case.lossless,
                    "width": case.width, "height": case.height,
                    "format": case.format,
                    "precision": case.precision, "signed": case.signed,
                    "source_format": source_extension(case),
                    "signed_header_authored": case.signed,
                    "pattern": case.pattern,
                    "encoder_options": [*encoder_options, *case.options] if case.codestream is None else [],
                    "encoder_invoked": case.codestream is None,
                    "thresholds": {"maximum_error": case.maximum_error, "maximum_rmse": case.maximum_rmse,
                                   "maximum_case_ms": timeout_seconds * 3000},
                    "status": "failed", "error": None, "metrics": None, "source_metrics": None,
                    "artifacts": [],
                }
                extension = source_extension(case)
                parse_image = framework.parse_pgx if extension == "pgx" else framework.parse_pnm
                source = work / f"{case.name}.{extension}"
                encoded_extension = case.format if case.format != "raw" else "j2c" if high_throughput else "j2k"
                encoded = work / f"{case.name}.{encoded_extension}"
                codestream = (encoded.with_suffix(".j2c")
                              if case.format == "jph" or (case.signed and case.format == "jp2") else encoded)
                reference_output = work / f"{case.name}-reference.{extension}"
                reference = (reference_output.with_stem(reference_output.stem + "_0")
                             if extension == "pgx" else reference_output)
                source.write_bytes(source_image(case))
                record["input_sha256"] = framework.sha256_file(source)
                encoder_source = source
                if case.signed:
                    shifted_samples = (tuple(value + (1 << (case.precision - 1)) for value in case.samples)
                                       if case.samples is not None else None)
                    shifted_case = replace(case, signed=False, samples=shifted_samples)
                    encoder_source = source.with_name(source.stem + "-unsigned." + source_extension(shifted_case))
                    encoder_source.write_bytes(source_image(shifted_case))
                record["encoder_input_sha256"] = (framework.sha256_file(encoder_source)
                                                  if case.codestream is None else None)
                try:
                    encoder_ms: float | None = None
                    if case.codestream is None:
                        _, encoder_ms = _run_checked([tools["encoder"]["path"], "-i", str(encoder_source), "-o", str(codestream),
                                                      *record["encoder_options"]], timeout_seconds)
                    else:
                        codestream.write_bytes(case.codestream)
                    if case.signed:
                        codestream.write_bytes(signed_codestream(codestream.read_bytes(), case.precision))
                        if case.format == "jp2":
                            encoded.write_bytes(framework.jp2_file(
                                codestream.read_bytes(), width=case.width, height=case.height,
                                components=case.channels, precision=case.precision, signed=True,
                            ))
                    if case.format == "jph":
                        encoded.write_bytes(framework.jp2_file(
                            codestream.read_bytes(), b"jph ", width=case.width, height=case.height,
                            components=case.channels, precision=case.precision,
                        ))
                    record["codestream_sha256"] = framework.sha256_file(encoded)
                    _, reference_ms = _run_checked([tools["decoder"]["path"], "-i", str(encoded), "-o", str(reference_output)], timeout_seconds)
                    expected = parse_image(reference)
                    original = parse_image(source)
                    if case.lossless and framework.normalized_hash(original) != framework.normalized_hash(expected):
                        raise ValueError("independent lossless encoder/decoder changed original samples")
                    process, subject_ms = _run_checked([str(executable), "decode", case.format, str(encoded)], timeout_seconds)
                    record["timings"] = {"encoder_ms": encoder_ms, "reference_ms": reference_ms, "subject_ms": subject_ms}
                    actual = framework.parse_subject_output(process.stdout, process.stderr, process.returncode)
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
                    for path in dict.fromkeys((source, encoder_source, codestream, encoded, reference)):
                        if path.is_file():
                            saved = framework.report_path(
                                output_dir, f"numerical-failures/{case.name}/{path.name}",
                            )
                            saved.parent.mkdir(parents=True, exist_ok=True)
                            shutil.copyfile(path, saved)
                            record["artifacts"].append(saved.relative_to(destination.parent).as_posix())
                result["records"].append(record)
        passed = sum(record["status"] == "passed" for record in result["records"])
        result["summary"] = {"cases": len(cases), "passed": passed, "failed": len(cases) - passed}
        if passed != len(cases):
            result["status"] = "failed"
    framework.validate_schema(framework.VALIDATION / "numerical-result.schema.json", result)
    framework.write_json(destination, result)
    return result
