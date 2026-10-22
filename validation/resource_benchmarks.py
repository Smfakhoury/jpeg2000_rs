"""Representative process measurements, separate from decoded-storage limits."""

from __future__ import annotations

import copy
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
FILE_BYTES = 32 << 20
OUTPUT_BYTES = 16 << 20
BASELINE_PIXEL_SIDE = 65_535
BASELINE_PIXEL_CLAIM = {"resource": "Pixels", "limit": 100_000_000, "actual": BASELINE_PIXEL_SIDE ** 2}
R1_CASES = (
    ("large-gray", 1536, 1, 8, 128, 1, False),
    ("components8", 128, 8, 16, 128, 1, False),
    ("precision24", 256, 1, 24, 256, 1, False),
    ("tiles256", 512, 1, 8, 32, 1, False),
    ("layers16", 128, 3, 16, 128, 16, False),
    ("ht-large", 768, 3, 16, 128, 1, True),
    ("jpx-components", 256, 3, 16, 256, 1, False),
    ("jpx-compose", 128, 3, 16, 128, 1, False),
    ("jpip-fragments", 256, 3, 16, 256, 1, False),
    ("jpip-ht-fragments", 256, 3, 16, 256, 1, True),
)
R1_SELECTIONS = ("large-gray", "tiles256", "ht-large")
R1_REJECTIONS = {
    "declared-width": ("Width", 1_000_000, 1_000_001),
    "declared-height": ("Height", 1_000_000, 1_000_001),
    "declared-components": ("Components", 64, 65),
    "declared-tiles": ("Tiles", 16_384, 16_641),
    "declared-layers": ("Layers", 256, 257),
    "declared-jpx-codestreams": ("Codestreams", 64, 65),
    "declared-jpip-offset": ("InputBytes", 2_147_483_648, 2_147_483_649),
}
ACCOUNTING_PATHS = (
    dict(id="selected-raw", family="selected", operation="select", format="raw", size=32,
         components=1, precision=8, tile_size=16, decomposition_levels=2, output_protocol="selected-image", output_components=1),
    dict(id="selected-color", family="selected-color", operation="select-color", format="jp2", size=32,
         components=3, precision=16, tile_size=16, decomposition_levels=2, output_protocol="image", output_components=3),
    dict(id="jpx-compose", family="composition", operation="render", format="jpx", size=16,
         components=3, precision=16, tile_size=16, decomposition_levels=2, output_protocol="image",
         composited_layers=2, output_components=4),
    dict(id="jpip-reconstruct", family="jpip", operation="reconstruct", format="jpip", size=16,
         components=3, precision=16, tile_size=16, decomposition_levels=2, output_protocol="reconstructed-image", output_components=3),
    dict(id="jpip-progressive", family="jpip", operation="progressive", format="auto", size=16,
         components=1, precision=8, tile_size=16, decomposition_levels=0, output_protocol="progressive",
         expected_updates=7, output_components=1),
)
ACCOUNTING_CEILINGS = {"DecodedBytes": 1 << 20, "WorkUnits": 100_000}
ACCOUNTING_MATRIX_SHA256 = "cc25a51fec71019cd8cfe8a2418b441d0d27214c2a46328ed89b16e3fc79d1f4"
REJECTION_THRESHOLDS = {"maximum_ms": 1000.0, "maximum_peak_rss_bytes": 128 << 20}
R1_GAPS = [
    "Finite Linux representatives, not a universal production-default resource bound or R1 closure.",
    "Observed admission frontiers cover only the declared small paths, not global cumulative "
    "work across reset scopes, allocation totals, arbitrary selections, HT accounting or broader progressive profiles.",
    "Default-maximum allocation, sustained stress, other platforms and exhaustive profile combinations remain unmeasured.",
]


def _accounting_declaration() -> dict[str, Any]:
    declaration = {"id": "r1-effective-frontiers-v2", "baseline_cases": 45,
                   "paths": copy.deepcopy(list(ACCOUNTING_PATHS)), "ceilings": dict(ACCOUNTING_CEILINGS),
                   "maximum_search_probes": MAXIMUM_SEARCH_PROBES}
    if framework.sha256_bytes(framework.canonical_bytes(declaration)) != ACCOUNTING_MATRIX_SHA256:
        raise ValueError("accounting matrix differs from its frozen premeasurement declaration")
    return declaration


def _declaration() -> dict[str, Any]:
    return {
        "id": "r1-safe-production-v1",
        "baseline_cases": 22,
        "workloads": [
            {"id": name, "width": size, "height": size, "components": channels,
             "precision": precision, "tile_size": tile, "layers": layers,
             "high_throughput": ht,
             "modes": ["full", "tile0", "native2"] if name in R1_SELECTIONS else ["full"]}
            for name, size, channels, precision, tile, layers, ht in R1_CASES
        ],
        "rejections": [{"id": name, "resource": resource, "limit": limit, "actual": actual}
                       for name, (resource, limit, actual) in R1_REJECTIONS.items()],
    }


def _limit_process() -> None:
    if framework.resource is None:
        raise RuntimeError("POSIX process limits are unavailable")
    framework.resource.setrlimit(framework.resource.RLIMIT_AS, (ADDRESS_SPACE_BYTES, ADDRESS_SPACE_BYTES))
    framework.resource.setrlimit(framework.resource.RLIMIT_FSIZE, (FILE_BYTES, FILE_BYTES))


def _peak_rss(path: Path) -> int:
    lines = path.read_text().splitlines()
    if not lines:
        raise ValueError("GNU Time did not report peak RSS")
    value = int(lines[-1].split("\t")[0]) * 1024
    if value <= 0:
        raise ValueError("GNU Time reported an unavailable peak RSS")
    return value


def _process_metrics(path: Path) -> dict[str, Any]:
    lines = path.read_text().splitlines()
    if not lines or len(lines[-1].split("\t")) != 3:
        raise ValueError("GNU Time omitted RSS/user/system CPU measurements")
    _, user, system = lines[-1].split("\t")
    values = [float(user) * 1000, float(system) * 1000]
    if any(not math.isfinite(value) or value < 0 for value in values):
        raise ValueError("GNU Time reported invalid CPU measurements")
    return {"peak_rss_bytes": _peak_rss(path), "user_cpu_ms": values[0],
            "system_cpu_ms": values[1], "total_cpu_ms": sum(values)}


def _run_reference(command: list[str], timeout: float) -> None:
    process, _, limited = framework._run_process_bounded(command, timeout, OUTPUT_BYTES, _limit_process)
    if limited or process.returncode != 0:
        raise ValueError(f"resource reference failed: {command[0]}: "
                         f"{(process.stdout + process.stderr)[-2048:]!r}, output_limit={limited}")


def _qualify(original: dict[str, Any], reference: dict[str, Any], payload: Path,
             sources: list[Path], references: list[Path]) -> dict[str, Any]:
    if numerical.image_error_metrics(original, reference)["maximum_error"] != 0:
        raise ValueError("independent resource fixture failed its lossless original/reference qualification")
    return {
        "original_output_sha256": framework.normalized_hash(original),
        "reference_output_sha256": framework.normalized_hash(reference),
        "payload_sha256": framework.sha256_file(payload),
        "source_files": [{"name": path.name, "sha256": framework.sha256_file(path)} for path in sources],
        "reference_files": [{"name": path.name, "sha256": framework.sha256_file(path)} for path in references],
    }


def _parse_typed_error(output: bytes) -> tuple[dict[str, str], dict[str, Any] | None]:
    diagnostic = output.decode("utf-8").removesuffix("\n").removesuffix("\r")
    match = re.fullmatch(
        r"ERR\tLimitExceeded\tresource limit exceeded for ([A-Za-z][A-Za-z0-9]*): "
        r"limit ([0-9]+), actual ([0-9]+)", diagnostic,
    )
    if match is not None:
        claim = {"resource": match[1], "limit": int(match[2]), "actual": int(match[3])}
        if claim["actual"] <= claim["limit"]:
            raise ValueError("typed limit rejection does not report an excessive actual claim")
        return {"category": "LimitExceeded", "reason": match[1], "diagnostic": diagnostic}, claim
    match = re.fullmatch(r"ERR\tMalformedInput\tmalformed input at byte [0-9]+: ([A-Za-z][A-Za-z0-9]*)",
                         diagnostic)
    if match is not None:
        return {"category": "MalformedInput", "reason": match[1], "diagnostic": diagnostic}, None
    raise ValueError("resource observation omitted an exact supported typed error diagnostic")


def _parse_progressive_output(output: bytes, expected: dict[str, Any], successful: bool,
                              expected_updates: int) -> tuple[bytes, list[dict[str, Any]]]:
    lines, updates, position, previous = output.splitlines(), [], 0, 0
    image_output = None
    while position < len(lines):
        line = lines[position]
        if line.startswith(b"ERR\t"):
            if successful or position != len(lines) - 1:
                raise ValueError("progressive resource output contains an unexpected diagnostic")
            return line + b"\n", updates
        fields = line.split(b"\t")
        if len(fields) != 6 or fields[0] != b"UPDATE":
            raise ValueError("progressive resource output omitted a typed update")
        status = fields[1].decode("ascii")
        old, available, present, missing = (int(value) for value in fields[2:])
        if (old != previous or available not in (0, 1) or available < old or missing < 0
                or present not in (0, 1) or bool(present) != (available > old)
                or status != ("Complete" if available == 1 else "WaitingForData")
                or (available == 1 and missing != 0) or len(updates) >= expected_updates):
            raise ValueError("progressive resource update violates the declared single-layer protocol")
        position += 1
        output_hash = None
        if present:
            if position + 2 > len(lines) or lines[position] != b"IMAGE":
                raise ValueError("progressive image update omitted its image")
            header = [int(value) for value in lines[position + 1].split()]
            if len(header) != 3 or not 1 <= header[2] <= 64 or position + 2 + header[2] > len(lines):
                raise ValueError("progressive image update has invalid component framing")
            image_output = b"\n".join(lines[position:position + 2 + header[2]]) + b"\n"
            image = framework.parse_subject_output(image_output, b"", 0)
            if numerical.image_error_metrics(expected, image)["maximum_error"] != 0:
                raise ValueError("progressive resource image differs from its independent expectation")
            output_hash = framework.normalized_hash(image)
            position += 2 + header[2]
        updates.append({"status": status, "previous_layers": old, "available_layers": available,
                        "image_present": bool(present), "missing_ranges": missing,
                        "output_sha256": output_hash})
        previous = available
    if not successful or image_output is None or previous != 1 or len(updates) != expected_updates:
        raise ValueError("progressive resource observation omitted completion or its final typed error")
    return image_output, updates


def _measure(command: list[str], time_tool: str, metrics_file: Path,
             expected: dict[str, Any] | None, category: str | None, reason: str | None,
             selected: bool, timeout: float, *, allow_success: bool = False,
             configured_value: int | None = None,
             reconstructed_sha256: str | None = None,
             progressive_updates_expected: int | None = None) -> dict[str, Any]:
    if (category is None or allow_success) and expected is None:
        raise ValueError("successful resource observations require an independent expected image")
    process, elapsed, limited = framework._run_process_bounded(
        [time_tool, "-f", "%M\t%U\t%S", "-o", str(metrics_file), *command],
        timeout, OUTPUT_BYTES, _limit_process,
    )
    if limited:
        raise ValueError("resource observation exceeded the bounded output capture")
    output = process.stdout
    updates = None
    if progressive_updates_expected is not None:
        if expected is None:
            raise ValueError("progressive prefixes require an independent expected image, including rejections")
        output, updates = _parse_progressive_output(output, expected, process.returncode == 0,
                                                     progressive_updates_expected)
    reconstructed = None
    if reconstructed_sha256 is not None and process.returncode == 0:
        prefix, separator, output = output.partition(b"\n")
        if not separator or not prefix.startswith(b"BYTES\t"):
            raise ValueError("JPIP resource observation omitted reconstructed bytes")
        reconstructed = framework.sha256_bytes(bytes.fromhex(prefix[6:].decode("ascii")))
        if reconstructed != reconstructed_sha256:
            raise ValueError("JPIP resource observation changed reconstructed codestream bytes")
    if selected and process.returncode == 0:
        frame, separator, output = output.partition(b"\n")
        if not separator or frame.split(b"\t") != [b"FRAME", b"0", b"0"]:
            raise ValueError("selected resource observation has an unexpected origin")
    actual = framework.parse_subject_output(output, process.stderr, process.returncode)
    claim = None
    observed_error = None
    if category is None or (allow_success and process.returncode == 0):
        if process.returncode != 0 or numerical.image_error_metrics(expected, actual)["maximum_error"] != 0:
            raise ValueError("resource case did not reproduce independent image samples")
        output_hash = framework.normalized_hash(actual)
    else:
        if process.returncode != 1 or actual.get("accepted") or actual.get("execution_failed"):
            raise ValueError(f"resource case did not produce the expected typed {category}/{reason} rejection")
        observed_error, claim = _parse_typed_error(output)
        if observed_error["category"] != category or observed_error["reason"] != reason:
            raise ValueError(f"resource case did not produce the expected typed {category}/{reason} rejection")
        output_hash = None
        if configured_value is not None:
            if claim is None or claim["limit"] != configured_value:
                raise ValueError("typed rejection does not identify the configured limit and an excessive claim")
    result: dict[str, Any] = {"elapsed_ms": elapsed, **_process_metrics(metrics_file),
                              "returncode": process.returncode, "output_sha256": output_hash}
    if reconstructed is not None:
        result["reconstructed_sha256"] = reconstructed
    if claim is not None:
        result["limit_claim"] = claim
    if observed_error is not None:
        result["observed_error"] = observed_error
    if updates is not None:
        result["progressive_updates"] = updates
        result["protocol_sha256"] = framework.sha256_bytes(process.stdout)
    return result


def _check_thresholds(measured: dict[str, Any], thresholds: dict[str, Any]) -> None:
    if (not math.isfinite(measured["elapsed_ms"]) or measured["elapsed_ms"] < 0
            or type(measured["peak_rss_bytes"]) is not int or measured["peak_rss_bytes"] <= 0):
        raise ValueError("resource observation requires finite wall time and positive integer RSS")
    if measured["elapsed_ms"] > thresholds["maximum_ms"]:
        raise ValueError("measured process wall time exceeds the declared engineering threshold")
    if measured["peak_rss_bytes"] > thresholds["maximum_peak_rss_bytes"]:
        raise ValueError("measured peak process RSS exceeds the declared engineering threshold")


def _find_limit_boundary(command: list[str], resource: str, upper: int, expected: dict[str, Any],
                         time_tool: str, metrics_file: Path, timeout: float,
                         thresholds: dict[str, Any], observations: list[dict[str, Any]], *,
                         selected: bool = False, reconstructed_sha256: str | None = None,
                         progressive_updates_expected: int | None = None,
                         rejection_thresholds: dict[str, Any] | None = None) -> int:
    if type(upper) is not int or upper <= 0:
        raise ValueError("resource search requires a positive finite integer ceiling")

    def probe(value: int) -> bool:
        if len(observations) >= MAXIMUM_SEARCH_PROBES:
            raise ValueError("resource search exceeded its finite probe budget")
        measured = _measure(
            [*command, str(value)], time_tool, metrics_file, expected,
            "LimitExceeded", resource, selected, timeout,
            allow_success=True, configured_value=value, reconstructed_sha256=reconstructed_sha256,
            progressive_updates_expected=progressive_updates_expected,
        )
        observations.append({"limit_value": value, **measured})
        _check_thresholds(measured, thresholds)
        if measured["returncode"] == 1 and rejection_thresholds is not None:
            _check_thresholds(measured, rejection_thresholds)
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


def _production_fixture(spec: tuple, work: Path, tools: dict[str, Any], timeout: float,
                        *, invert: bool = False) -> tuple[Path, dict[str, Any], dict[str, Any]]:
    name, size, channels, precision, tile, layers, ht = spec
    label = name + ("-foreground" if invert else "")
    case = numerical.NumericalCase(label, width=size, height=size, channels=channels, precision=precision)
    encoded = work / f"{label}.j2k"
    extension = "raw" if channels == 8 else numerical.source_extension(case)
    source = work / f"{label}.{extension}"
    if channels == 8:
        planes = [[((x * 31 + y * 17 + channel * 71 + x * y * 13) % 256) * 257
                   for y in range(size) for x in range(size)] for channel in range(channels)]
        source.write_bytes(b"".join(struct.pack(">H", value) for plane in planes for value in plane))
        original = {"accepted": True, "image": {"width": size, "height": size, "components": [
            {"width": size, "height": size, "precision": precision, "signed": False,
             "sample_kind": "unsigned", "samples": plane} for plane in planes]}}
    else:
        source.write_bytes(numerical.source_image(case))
        original = (framework.parse_pgx if extension == "pgx" else framework.parse_pnm)(source)
        if invert:
            peak = (1 << precision) - 1
            original = copy.deepcopy(original)
            for component in original["image"]["components"]:
                component["samples"] = [peak - value for value in component["samples"]]
            values = (component["samples"][index]
                      for index in range(size * size) for component in original["image"]["components"])
            source.write_bytes(f"P6\n{size} {size}\n{peak}\n".encode()
                               + b"".join(value.to_bytes(2, "big") for value in values))
    encoder, decoder = (tools["ht_encoder" if ht else "encoder"]["path"],
                        tools["ht_decoder" if ht else "decoder"]["path"])
    options = (["-num_decomps", "2", "-reversible", "true", "-tile_size", f"{{{tile},{tile}}}"]
               if ht else ["-n", "3", "-t", f"{tile},{tile}"])
    if layers > 1:
        options += ["-r", ",".join([*(str(value) for value in range(layers, 1, -1)), "1"])]
    if channels == 8:
        options += ["-F", f"{size},{size},{channels},{precision},u", "-mct", "0"]
    _run_reference([encoder, "-i", str(source), "-o", str(encoded), *options], timeout)
    data = encoded.read_bytes()
    numerical.validate_required_headers(case, data)
    headers = dict(numerical._main_header_segments(data))
    siz, cod = headers.get(0xff51, b""), headers.get(0xff52, b"")
    if (len(siz) != 36 + 3 * channels or len(cod) < 10
            or struct.unpack_from(">IIII", siz, 2) != (size, size, 0, 0)
            or struct.unpack_from(">IIII", siz, 18) != (tile, tile, 0, 0)
            or siz[34:] != struct.pack(">H", channels) + bytes((precision - 1, 1, 1)) * channels
            or int.from_bytes(cod[2:4], "big") != layers or cod[9] != 1
            or (channels == 8 and cod[4] != 0)):
        raise ValueError("resource encoder omitted declared production geometry/component/precision/tile/layer settings")
    if ht:
        capability = headers.get(0xff50, b"")
        if not cod[8] & 0x40 or not int.from_bytes(capability[:4], "big") & 0x20000:
            raise ValueError("resource encoder omitted declared HT signaling")
    reference = work / f"{label}-reference.{'pgx' if channels == 8 else extension}"
    _run_reference([decoder, "-i", str(encoded), "-o", str(reference)], timeout)
    if channels == 8 or extension == "pgx":
        references = [reference.with_stem(reference.stem + f"_{index}") for index in range(channels)]
        images = [framework.parse_pgx(path) for path in references]
        expected = copy.deepcopy(images[0])
        expected["image"]["components"] = [image["image"]["components"][0] for image in images]
    else:
        references = [reference]
        expected = framework.parse_pnm(reference)
    qualification = _qualify(original, expected, encoded, [source], references)
    return encoded, expected, qualification


def _jpx(data: list[bytes], size: int) -> bytes:
    box = framework.box
    header = box(b"ihdr", struct.pack(">IIH4B", size, size, 3, 15, 7, 0, 0))
    header += box(b"colr", bytes((1, 0, 0, 0, 0, 0, 16)))
    return (box(b"jP  ", b"\x0d\x0a\x87\x0a") + box(b"ftyp", b"jpxb\0\0\0\0jpxb")
            + box(b"jp2h", header)
            + b"".join(box(b"jpch", header) for _ in data)
            + b"".join(box(b"jplh", box(b"creg", struct.pack(">HHH4B", 1, 1, index, 1, 1, 0, 0)))
                       for index in range(len(data)))
            + box(b"comp", box(b"copt", struct.pack(">IIB", size, size, 0)))
            + b"".join(box(b"jp2c", raw) for raw in data))


def _vbas(value: int) -> bytes:
    if type(value) is not int or value < 0:
        raise ValueError("resource JPIP values must be nonnegative integers")
    groups = [value & 127]
    while value >> 7:
        value >>= 7
        groups.append(value & 127)
    return bytes(group | (128 if index else 0)
                 for index, group in reversed(list(enumerate(groups))))


def _jpip_message(class_id: int, offset: int, body: bytes, final: bool = False) -> bytes:
    return bytes((0x70 if final else 0x60,)) + _vbas(class_id) + b"\0" + _vbas(offset) + _vbas(len(body)) + body


def _jpip(data: bytes) -> bytes:
    if not data.startswith(b"\xff\x4f") or not data.endswith(b"\xff\xd9"):
        raise ValueError("resource JPIP requires a bounded SOC/EOC codestream")
    position, tile = 2, None
    while position + 4 <= len(data):
        marker, length = struct.unpack_from(">HH", data, position)
        if marker == 0xff93:
            if tile is None or tile + int.from_bytes(data[tile + 6:tile + 10], "big") != len(data) - 2:
                raise ValueError("resource JPIP requires exactly one bounded tile-part")
            bins = ((6, data[:tile]), (2, data[tile:position + 2]), (4, data[position + 2:]))
            fragments = []
            for class_id, body in bins:
                middle = len(body) // 2
                fragments += [_jpip_message(class_id, middle, body[middle:], True),
                              _jpip_message(class_id, 0, body[:middle]),
                              _jpip_message(class_id, 0, body[:middle])]
            return b"".join(fragments) + b"\0\x01\0"
        if length < 2 or position + 2 + length > len(data):
            raise ValueError("resource JPIP has a truncated marker")
        if marker == 0xff90:
            if tile is not None:
                raise ValueError("resource JPIP has multiple tile-parts")
            tile = position
        position += 2 + length
    raise ValueError("resource JPIP has no SOD")


def _single_packet_response(data: bytes) -> bytes:
    position, tile, lengths, pending = 2, None, [], 0
    while position + 4 <= len(data):
        marker, length = struct.unpack_from(">HH", data, position)
        if marker == 0xff93:
            if (tile is None or not data.endswith(b"\xff\xd9")
                    or tile + int.from_bytes(data[tile + 6:tile + 10], "big") != len(data) - 2
                    or lengths != [len(data) - position - 4] or pending):
                raise ValueError("progressive accounting fixture requires one bounded tile-part and one PLT packet")
            return b"".join(_jpip_message(kind, 0, body, True) for kind, body in (
                (6, data[:tile]), (2, data[tile:position + 2]), (0, data[position + 2:-2])))
        if length < 2 or position + 2 + length > len(data):
            raise ValueError("progressive accounting fixture has a truncated marker")
        if marker == 0xff90:
            if tile is not None:
                raise ValueError("progressive accounting fixture has multiple tile-parts")
            tile = position
        elif marker == 0xff58:
            if data[position + 4] != 0 or lengths:
                raise ValueError("progressive accounting fixture requires a single PLT segment")
            for value in data[position + 5:position + 2 + length]:
                pending = pending * 128 + (value & 127)
                if not value & 128:
                    lengths.append(pending)
                    pending = 0
        position += 2 + length
    raise ValueError("progressive accounting fixture omitted SOD")


def _opaque_rgba(expected: dict[str, Any]) -> dict[str, Any]:
    result = copy.deepcopy(expected)
    image = result["image"]
    if len(image["components"]) != 3 or any(c["precision"] != 16 for c in image["components"]):
        raise ValueError("accounting color oracle requires independently qualified RGB16")
    alpha = copy.deepcopy(image["components"][0])
    alpha["samples"] = [65535] * (alpha["width"] * alpha["height"])
    image["components"].append(alpha)
    return result


def _accounting_fixture(path: dict[str, Any], work: Path, tools: dict[str, Any],
                        timeout: float) -> tuple[Path, dict[str, Any], list[dict[str, Any]]]:
    label, size = "account-" + path["id"], path["size"]
    if path["id"] == "jpip-progressive":
        case = numerical.NumericalCase(label, width=size, height=size, decomposition_levels=0)
        source, encoded, reference = (work / f"{label}.{suffix}" for suffix in ("pgm", "j2k", "reference.pgm"))
        source.write_bytes(numerical.source_image(case))
        _run_reference([tools["encoder"]["path"], "-i", str(source), "-o", str(encoded),
                        "-n", "1", "-t", f"{size},{size}", "-PLT"], timeout)
        data = encoded.read_bytes()
        numerical.validate_required_headers(case, data)
        headers = dict(numerical._main_header_segments(data))
        siz, coding = headers[0xff51], headers[0xff52]
        if (struct.unpack_from(">IIII", siz, 2) != (size, size, 0, 0)
                or struct.unpack_from(">IIII", siz, 18) != (size, size, 0, 0)
                or siz[34:] != b"\0\1\x07\1\1" or coding[2:4] != b"\0\1" or coding[9] != 1):
            raise ValueError("progressive accounting fixture omitted its declared grayscale geometry/lossless coding")
        response = _single_packet_response(data)
        _run_reference([tools["decoder"]["path"], "-i", str(encoded), "-o", str(reference)], timeout)
        expected = framework.parse_pnm(reference)
        qualification = _qualify(framework.parse_pnm(source), expected, encoded, [source], [reference])
        container = work / f"{label}.jpip"
        container.write_bytes(response)
        return container, expected, [qualification]
    spec = (label, size, path["components"], path["precision"], path["tile_size"], 1, False)
    encoded, expected, qualification = _production_fixture(spec, work, tools, timeout)
    qualifications = [qualification]
    if path["id"] == "selected-raw":
        expected = framework.crop_image(expected, (0, 0, size, size), (0, 0, 16, 16))
    elif path["id"] == "selected-color":
        container = work / f"{label}.jp2"
        container.write_bytes(framework.jp2_file(encoded.read_bytes(), width=size, height=size,
                                               components=3, precision=16))
        encoded = container
        expected = framework.crop_image(expected, (0, 0, size, size), (0, 0, 16, 16))
    elif path["id"] == "jpx-compose":
        foreground, expected, second = _production_fixture(spec, work, tools, timeout, invert=True)
        qualifications.append(second)
        container = work / f"{label}.jpx"
        container.write_bytes(_jpx([encoded.read_bytes(), foreground.read_bytes()], size))
        encoded, expected = container, _opaque_rgba(expected)
    elif path["id"] == "jpip-reconstruct":
        container = work / f"{label}.jpip"
        container.write_bytes(_jpip(encoded.read_bytes()))
        encoded = container
    else:
        raise ValueError("undeclared accounting fixture")
    return encoded, expected, qualifications


def _rejection_inputs() -> dict[str, tuple[bytes, str, str]]:
    def raw(width=1, height=1, components=1, tile=1):
        siz = struct.pack(">HIIIIIIIIH", 0, width, height, 0, 0, tile, tile, 0, 0, components)
        return b"\xff\x4f" + framework.segment(0xff51, siz + bytes((7, 1, 1)) * components) + framework.raw_codestream()[45:]
    layered = bytearray(framework.raw_codestream())
    struct.pack_into(">H", layered, layered.index(b"\xff\x52") + 6, 257)
    jpx = framework.jpx_file(framework.raw_codestream())
    jpx += framework.box(b"jp2c", framework.raw_codestream()) * 64
    return {
        "declared-width": (raw(width=1_000_001, tile=1_000_001), "decode", "raw"),
        "declared-height": (raw(height=1_000_001, tile=1_000_001), "decode", "raw"),
        "declared-components": (raw(components=65), "decode", "raw"),
        "declared-tiles": (raw(width=129, height=129), "decode", "raw"),
        "declared-layers": (bytes(layered), "decode", "raw"),
        "declared-jpx-codestreams": (jpx, "decode", "jpx"),
        "declared-jpip-offset": (_jpip_message(6, 2_147_483_648, b"x", True), "ingest", "jpip"),
    }


def _expected_case_ids() -> set[str]:
    ids = {f"{label}-{mode}" for label in ("gray-1024", "rgb-512", "rgb16-512", "ht-rgb16-512")
           for mode in ("full", "tile0", "native2")}
    ids |= {f"{label}-{resource}-{side}" for label in ("gray24-small", "rgb16-layered")
            for resource in ("decoded-bytes", "work-units") for side in ("at", "below")}
    ids |= {"declared-pixel-limit", "truncated-main-header"}
    ids |= {f"{spec[0]}-{mode}" for spec in R1_CASES
            for mode in (("full", "tile0", "native2") if spec[0] in R1_SELECTIONS else ("full",))}
    return ids | set(R1_REJECTIONS)


def _expected_accounting_ids() -> set[str]:
    return {f"account-{path['id']}-{selector}-{side}" for path in ACCOUNTING_PATHS
            for selector in ("decoded-bytes", "work-units") for side in ("at", "below")}


def _validate_evidence(result: dict[str, Any]) -> None:
    if result["declaration"] != _declaration():
        raise ValueError("resource evidence changed the predeclared workload matrix")
    records = result["records"]
    ids = [record["id"] for record in records]
    if len(set(ids)) != len(ids):
        raise ValueError("resource evidence has duplicate case IDs")
    if result["status"] == "passed" and set(ids) != _expected_case_ids():
        raise ValueError("resource evidence omitted a predeclared case")
    if (result["accounting_declaration"] != _accounting_declaration()
            or result["accounting_declaration_sha256"] != ACCOUNTING_MATRIX_SHA256):
        raise ValueError("resource evidence changed the frozen accounting matrix")
    accounting_ids = [record["id"] for record in result["accounting_records"]]
    if len(set(accounting_ids)) != len(accounting_ids) or set(ids) & set(accounting_ids):
        raise ValueError("resource evidence has duplicate accounting IDs")
    if result["status"] == "passed" and set(accounting_ids) != _expected_accounting_ids():
        raise ValueError("resource evidence omitted a declared accounting frontier")
    records = [*records, *result["accounting_records"]]
    for record in records:
        accounting_path = record["accounting_path"]
        if accounting_path is not None:
            path = next((path for path in ACCOUNTING_PATHS if path["id"] == accounting_path), None)
            if path is None or record["output_protocol"] != path["output_protocol"]:
                raise ValueError("resource evidence mislabels its actual accounting path/protocol")
            command, configured = record["command"], record["configured_limit"]
            if not isinstance(configured, dict) or configured.get("resource") not in ACCOUNTING_CEILINGS:
                raise ValueError("resource evidence omitted its declared accounting resource")
            extent = path["tile_size"] if path["operation"].startswith("select") else path["size"]
            if (record["output_width"] != extent or record["output_height"] != extent
                    or record["output_components"] != path["output_components"]
                    or record["typed_sample_bytes"] != extent * extent * path["output_components"] * 8
                    or record["progressive_updates_expected"] != path.get("expected_updates")
                    or (record["search_ceiling"] is not None
                        and record["search_ceiling"] != ACCOUNTING_CEILINGS[configured["resource"]])):
                raise ValueError("resource evidence changed declared output cardinality or accounting ceiling")
            selector = {"DecodedBytes": "decoded-bytes", "WorkUnits": "work-units"}[configured["resource"]]
            side = "at" if record["search_ceiling"] is not None else "below"
            if (record["id"] != f"account-{accounting_path}-{selector}-{side}"
                    or record["expected_category"] != (None if side == "at" else "LimitExceeded")
                    or (side == "below" and record["expected_reason"] != configured["resource"])
                    or not 0 <= configured["value"] <= ACCOUNTING_CEILINGS[configured["resource"]]):
                raise ValueError("resource evidence changed accounting path/resource/frontier identity")
            if record["status"] == "passed" and (command[:3] != [result["tools"]["subject"]["path"], path["operation"] + "-limited", path["format"]]
                    or command[-2:] != [selector, str(configured["value"])]
                    or len(command) != (9 if path["operation"].startswith("select") else 6)
                    or (path["operation"].startswith("select") and command[4:7] != ["0", "0", "all"])):
                raise ValueError("resource evidence command differs from its declared limited API path")
        if record["id"] == "declared-pixel-limit":
            if (record["expected_limit_claim"] != BASELINE_PIXEL_CLAIM
                    or record["configured_limit"] is not None
                    or record["expected_category"] != "LimitExceeded" or record["expected_reason"] != "Pixels"):
                raise ValueError("resource evidence changed the baseline production-default Pixels declaration")
        expected_mode = "single-tightened-defaults" if record["configured_limit"] is not None else "production-default"
        if record["limit_mode"] != expected_mode:
            raise ValueError("resource evidence mislabels production-default versus tightened accounting")
        for qualification in record["qualifications"]:
            if qualification["original_output_sha256"] != qualification["reference_output_sha256"]:
                raise ValueError("resource evidence contains an unqualified original/reference disagreement")
        for measured in [*record["runs"], *record["search_runs"]]:
            if record["output_protocol"] == "progressive":
                updates = measured.get("progressive_updates")
                if not isinstance(updates, list) or "protocol_sha256" not in measured:
                    raise ValueError("resource evidence omitted observed progressive updates")
                previous, images = 0, 0
                for update in updates:
                    available = update["available_layers"]
                    if (update["previous_layers"] != previous or available not in (0, 1)
                            or available < previous or update["image_present"] != (available > previous)
                            or update["status"] != ("Complete" if available == 1 else "WaitingForData")
                            or update["missing_ranges"] < 0 or (available == 1 and update["missing_ranges"] != 0)):
                        raise ValueError("resource evidence changed its progressive update sequence")
                    if update["image_present"]:
                        images += 1
                        if update["output_sha256"] != record["expected_output_sha256"]:
                            raise ValueError("resource evidence progressive image differs from its independent expectation")
                    elif update["output_sha256"] is not None:
                        raise ValueError("resource evidence invented an unchanged progressive image")
                    previous = available
                if len(updates) > 7 or (measured["returncode"] == 0 and (len(updates) != 7 or images != 1 or previous != 1)):
                    raise ValueError("resource evidence omitted declared progressive completion")
            elif "progressive_updates" in measured or "protocol_sha256" in measured:
                raise ValueError("resource evidence attached progression to another protocol")
            if not math.isclose(measured["total_cpu_ms"],
                                measured["user_cpu_ms"] + measured["system_cpu_ms"]):
                raise ValueError("resource evidence CPU sum differs")
            if measured["returncode"] == 0:
                if "observed_error" in measured or "limit_claim" in measured:
                    raise ValueError("successful resource observation contains typed error evidence")
                if measured["output_sha256"] != record["expected_output_sha256"]:
                    raise ValueError("resource evidence output hash differs from its independent expectation")
                if record["expected_reconstructed_sha256"] is not None:
                    if measured.get("reconstructed_sha256") != record["expected_reconstructed_sha256"]:
                        raise ValueError("resource evidence omitted exact JPIP reconstruction")
            else:
                observed_error = measured.get("observed_error")
                if not isinstance(observed_error, dict) or not isinstance(observed_error.get("diagnostic"), str):
                    raise ValueError("resource evidence omitted its observed typed error")
                parsed_error, claim = _parse_typed_error(observed_error["diagnostic"].encode("utf-8"))
                if observed_error != parsed_error or measured.get("limit_claim") != claim:
                    raise ValueError("resource evidence typed category/reason/claim differs from its actual diagnostic")
                configured = record["configured_limit"]
                if "limit_value" in measured and configured is not None:
                    category, reason, limit = "LimitExceeded", configured["resource"], measured["limit_value"]
                else:
                    category, reason = record["expected_category"], record["expected_reason"]
                    limit = configured["value"] if configured is not None else None
                if observed_error["category"] != category or observed_error["reason"] != reason:
                    raise ValueError("resource evidence observed typed category/reason differs from its expectation")
                if limit is not None and (claim is None or claim["limit"] != limit):
                    raise ValueError("resource evidence observed claim differs from its actual configured limit")
            if record["status"] == "passed":
                _check_thresholds(measured, record["thresholds"])
                if accounting_path is not None and measured["returncode"] == 1:
                    _check_thresholds(measured, REJECTION_THRESHOLDS)
        if record["status"] == "passed" and record["search_ceiling"] is not None:
            value = record["configured_limit"]["value"]
            outcomes = {(run["limit_value"], run["returncode"]) for run in record["search_runs"]}
            if not {(0, 1), (record["search_ceiling"], 0), (value, 0), (value - 1, 1)} <= outcomes:
                raise ValueError("resource evidence omitted a bracket endpoint or adjacent budget observation")
        if record["status"] == "passed" and record["expected_limit_claim"] is not None:
            if any(run.get("limit_claim") != record["expected_limit_claim"] for run in record["runs"]):
                raise ValueError("resource evidence default-limit claim differs")
    for report_key, run_key in (("native_total_ms", "elapsed_ms"),
                                ("native_user_cpu_ms", "user_cpu_ms"),
                                ("native_system_cpu_ms", "system_cpu_ms"),
                                ("native_total_cpu_ms", "total_cpu_ms")):
        total = sum(run[run_key] for record in records for run in [*record["runs"], *record["search_runs"]])
        if not math.isclose(result[report_key], total):
            raise ValueError("resource evidence aggregate differs from all final/search observations")
    if result["status"] == "passed" and result["native_total_ms"] > result["maximum_native_total_ms"]:
        raise ValueError("resource evidence exceeds the unchanged aggregate wall budget")


def run_resources(executable: Path | None = None, output_dir: Path | None = None,
                  encoder: Path | None = None, decoder: Path | None = None,
                  time_tool: Path | None = None, timeout_seconds: float = 10.0, *,
                  ht_encoder: Path | None = None, ht_decoder: Path | None = None) -> dict[str, Any]:
    if not math.isfinite(timeout_seconds) or timeout_seconds <= 0:
        raise ValueError("resource timeout must be positive and finite")
    destination = framework.report_path(output_dir, "resource-results.json")
    result: dict[str, Any] = {
        "$schema": framework.report_schema(output_dir, "resource-result.schema.json"),
        "schema_version": 5, "scope": "Finite Linux production-default engineering CPU/wall/RSS representatives and effective path admission frontiers, not global cumulative counters, universal resource guarantees or R1 closure.",
        "provenance": framework.source_binding(), "status": "passed", "blockers": [], "errors": [],
        "tools": {}, "address_space_limit_bytes": ADDRESS_SPACE_BYTES,
        "file_size_limit_bytes": FILE_BYTES, "output_limit_bytes": OUTPUT_BYTES,
        "declaration": _declaration(), "remaining_r1_gaps": list(R1_GAPS),
        "accounting_declaration": _accounting_declaration(),
        "accounting_declaration_sha256": ACCOUNTING_MATRIX_SHA256, "accounting_records": [],
        "qualification_failures": [],
        "repetitions": REPETITIONS, "records": [], "native_total_ms": 0.0,
        "native_user_cpu_ms": 0.0, "native_system_cpu_ms": 0.0, "native_total_cpu_ms": 0.0,
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
                         configured_limit=None, search=False, qualifications=(), reconstructed_sha256=None,
                         expected_limit_claim=None, accounting_path=None, output_protocol=None,
                         progressive_updates_expected=None):
                if category is None and not qualifications:
                    raise ValueError("positive resource case requires original/reference qualification")
                record = {
                    "id": name, "input_bytes": input_path.stat().st_size,
                    "accounting_path": accounting_path, "command": list(command),
                    "output_protocol": output_protocol or ("selected-image" if selected else
                                                           "reconstructed-image" if reconstructed_sha256 else "image"),
                    "progressive_updates_expected": progressive_updates_expected,
                    "input_sha256": framework.sha256_file(input_path),
                    "expected_category": category, "expected_reason": reason,
                    "expected_output_sha256": framework.normalized_hash(expected) if expected is not None else None,
                    "qualifications": list(qualifications),
                    "expected_reconstructed_sha256": reconstructed_sha256,
                    "expected_limit_claim": expected_limit_claim,
                    "limit_mode": "production-default" if configured_limit is None else "single-tightened-defaults",
                    "mode": "rejection" if category else "selection" if selected else "full",
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
                            selected=record["output_protocol"] == "selected-image",
                            reconstructed_sha256=reconstructed_sha256,
                            progressive_updates_expected=progressive_updates_expected,
                            rejection_thresholds=REJECTION_THRESHOLDS if accounting_path is not None else None,
                        )
                    value = record["configured_limit"]["value"] if configured_limit is not None else None
                    measured_command = [*command, str(value)] if configured_limit is not None else command
                    record["command"] = list(measured_command)
                    for repeat in range(REPETITIONS):
                        measured = _measure(measured_command, tools["time"]["path"], work / f"{name}-{repeat}.time",
                                            expected, category, reason, record["output_protocol"] == "selected-image", timeout_seconds,
                                            configured_value=(expected_limit_claim["limit"]
                                                              if expected_limit_claim is not None else value),
                                            reconstructed_sha256=reconstructed_sha256,
                                            progressive_updates_expected=progressive_updates_expected)
                        record["runs"].append(measured)
                        if expected_limit_claim is not None and measured.get("limit_claim") != expected_limit_claim:
                            raise ValueError("default declaration rejection did not report its exact declared limit/actual")
                        _check_thresholds(measured, record["thresholds"])
                    record["median_ms"] = statistics.median(run["elapsed_ms"] for run in record["runs"])
                    record["peak_rss_bytes"] = max(run["peak_rss_bytes"] for run in record["runs"])
                    if record["typed_sample_bytes"]:
                        record["amplification"] = {
                            "typed_sample_bytes_per_input_byte": record["typed_sample_bytes"] / record["input_bytes"],
                            "peak_rss_per_typed_sample_byte": record["peak_rss_bytes"] / record["typed_sample_bytes"],
                        }
                    record["status"] = "passed"
                except (OSError, ValueError, subprocess.SubprocessError) as error:
                    record["error"] = str(error)
                    failure = framework.report_path(output_dir, f"resource-failures/{name}/{record['input_sha256']}/{input_path.name}")
                    failure.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(input_path, failure)
                    record["artifacts"].append(failure.relative_to(destination.parent).as_posix())
                result["accounting_records" if accounting_path is not None else "records"].append(record)
                return record

            def check_selection_ratio(records, label, encoded):
                if all(record["status"] == "passed" for record in records):
                    records[1]["selection_full_rss_ratio"] = records[1]["peak_rss_bytes"] / records[0]["peak_rss_bytes"]
                    if records[1]["peak_rss_bytes"] > result["maximum_selection_rss_ratio"] * records[0]["peak_rss_bytes"]:
                        records[1]["status"] = "failed"
                        records[1]["error"] = "tile selection did not reduce peak RSS by the required 25 percent"
                        failure = framework.report_path(output_dir, f"resource-failures/{label}-rss-ratio/{encoded.name}")
                        failure.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copyfile(encoded, failure)
                        records[1]["artifacts"].append(failure.relative_to(destination.parent).as_posix())

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
                _run_reference([encode, "-i", str(source), "-o", str(encoded), *encoding_options], timeout_seconds)
                if high_throughput:
                    _validate_ht_resource_headers(encoded.read_bytes(), size)
                full_reference = work / f"{label}-full-reference.{suffix}"
                _run_reference([decode, "-i", str(encoded), "-o", str(full_reference)], timeout_seconds)
                full_expected = framework.parse_pnm(full_reference)
                qualification = _qualify(framework.parse_pnm(source), full_expected, encoded,
                                         [source], [full_reference])
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
                        _run_reference([decode, "-i", str(encoded), "-o", str(reference),
                                                *reference_options], timeout_seconds)
                        expected = framework.parse_pnm(reference)
                    command = [str(executable), "select" if selection else "decode", "raw", str(encoded),
                               *(selection or [])]
                    records.append(run_case(f"{label}-{mode}", command, encoded, expected, selected=selection is not None,
                                            qualifications=[qualification]))
                check_selection_ratio(records, label, encoded)
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
                _run_reference([tools["encoder"]["path"], "-i", str(source), "-o", str(encoded),
                                        "-n", "3", *case.options], timeout_seconds)
                if case.channels == 3:
                    data = encoded.read_bytes()
                    required_cod = framework.segment(0xff52, bytes((0, 0, 0, 3, 1, 2, 4, 4, 0, 1)))
                    if (len(data) < 32 or data[:4] != b"\xff\x4f\xff\x51"
                            or struct.unpack_from(">IIII", data, 16) != (3, 5, 16, 16)
                            or required_cod not in data.split(b"\xff\x90", 1)[0]):
                        raise ValueError("independent resource fixture omitted the required tiling/origin/three-layer headers")
                _run_reference([tools["decoder"]["path"], "-i", str(encoded), "-o", str(reference)],
                                       timeout_seconds)
                parse_image = framework.parse_pgx if extension == "pgx" else framework.parse_pnm
                if extension == "pgx":
                    reference = reference.with_stem(reference.stem + "_0")
                expected = parse_image(reference)
                qualification = _qualify(parse_image(source), expected, encoded, [source], [reference])
                for selector, resource, ceiling in (
                    ("decoded-bytes", "DecodedBytes", 16 << 20),
                    ("work-units", "WorkUnits", 10_000_000),
                ):
                    command = [str(executable), "decode-limited", "raw", str(encoded), selector]
                    record = run_case(
                        f"{case.name}-{selector}-at", command, encoded, expected,
                        configured_limit={"resource": resource, "value": ceiling}, search=True,
                        qualifications=[qualification],
                    )
                    if record["status"] == "passed":
                        run_case(
                            f"{case.name}-{selector}-below", command, encoded, None,
                            category="LimitExceeded", reason=resource,
                            configured_limit={"resource": resource, "value": record["configured_limit"]["value"] - 1},
                        )
            for spec in R1_CASES:
                label, size, channels, precision, tile, layers, ht = spec
                try:
                    encoded, full_expected, qualification = _production_fixture(spec, work, tools, timeout_seconds)
                    qualifications = [qualification]
                    if label == "jpx-compose":
                        foreground, full_expected, second = _production_fixture(spec, work, tools, timeout_seconds,
                                                                               invert=True)
                        qualifications.append(second)
                        container = work / f"{label}.jpx"
                        container.write_bytes(_jpx([encoded.read_bytes(), foreground.read_bytes()], size))
                        encoded = container
                        full_expected = copy.deepcopy(full_expected)
                        alpha = copy.deepcopy(full_expected["image"]["components"][0])
                        alpha["samples"] = [65535] * (size * size)
                        full_expected["image"]["components"].append(alpha)
                        operation, format_name = "render", "jpx"
                    elif label == "jpx-components":
                        container = work / f"{label}.jpx"
                        container.write_bytes(_jpx([encoded.read_bytes()], size))
                        encoded = container
                        operation, format_name = "decode", "jpx"
                    elif label.startswith("jpip-"):
                        container = work / f"{label}.jpip"
                        container.write_bytes(_jpip(encoded.read_bytes()))
                        encoded = container
                        operation, format_name = "reconstruct", "jpip"
                    else:
                        operation, format_name = "decode", "raw"
                except (OSError, ValueError, subprocess.SubprocessError) as error:
                    failure = destination.parent / "resource-failures" / f"{label}-qualification"
                    failure.mkdir(parents=True, exist_ok=True)
                    artifacts = []
                    for path in sorted(work.glob(f"{label}*")):
                        if path.is_file():
                            shutil.copyfile(path, failure / path.name)
                            artifacts.append((failure / path.name).relative_to(destination.parent).as_posix())
                    result["qualification_failures"].append({"id": label, "error": str(error), "artifacts": artifacts})
                    continue
                records = []
                for mode in (("full", "tile0", "native2") if label in R1_SELECTIONS else ("full",)):
                    expected = full_expected
                    selection = None
                    if mode == "tile0":
                        expected = framework.crop_image(full_expected, (0, 0, size, size), (0, 0, tile, tile))
                        selection = ["0", "0", "all"]
                    elif mode == "native2":
                        reference = work / f"{label}-native2.{'ppm' if channels == 3 else 'pgm'}"
                        _run_reference([tools["ht_decoder" if ht else "decoder"]["path"],
                                        "-i", str(encoded), "-o", str(reference),
                                        "-skip_res" if ht else "-r", "2"], timeout_seconds)
                        expected = framework.parse_pnm(reference)
                        selection = ["2", "all", "all"]
                    records.append(run_case(
                        f"{label}-{mode}", [str(executable), "select" if selection else operation,
                                            format_name, str(encoded), *(selection or [])],
                        encoded, expected, selected=selection is not None, qualifications=qualifications,
                        reconstructed_sha256=qualification["payload_sha256"] if label.startswith("jpip-") else None,
                    ))
                if label in R1_SELECTIONS:
                    check_selection_ratio(records, label, encoded)
            for label, (data, operation, format_name) in _rejection_inputs().items():
                path = work / label
                path.write_bytes(data)
                resource, limit, actual = R1_REJECTIONS[label]
                run_case(label, [str(executable), operation, format_name, str(path)], path, None,
                         category="LimitExceeded", reason=resource,
                         expected_limit_claim={"resource": resource, "limit": limit, "actual": actual})
            limit_input = work / "declared-pixel-limit.j2k"
            data = bytearray(framework.raw_codestream())
            struct.pack_into(">II", data, 8, BASELINE_PIXEL_SIDE, BASELINE_PIXEL_SIDE)
            struct.pack_into(">II", data, 24, BASELINE_PIXEL_SIDE, BASELINE_PIXEL_SIDE)
            limit_input.write_bytes(data)
            run_case("declared-pixel-limit", [str(executable), "decode", "raw", str(limit_input)],
                     limit_input, None, category="LimitExceeded", reason="Pixels",
                     expected_limit_claim=dict(BASELINE_PIXEL_CLAIM))
            truncated = work / "truncated-main-header.j2k"
            truncated.write_bytes(framework.raw_codestream()[:12])
            run_case("truncated-main-header", [str(executable), "decode", "raw", str(truncated)],
                     truncated, None, category="MalformedInput", reason="Truncated")
            for path in ACCOUNTING_PATHS:
                try:
                    input_path, expected, qualifications = _accounting_fixture(path, work, tools, timeout_seconds)
                    extent = path["tile_size"] if path["operation"].startswith("select") else path["size"]
                    if (expected["image"]["width"] != extent or expected["image"]["height"] != extent
                            or len(expected["image"]["components"]) != path["output_components"]):
                        raise ValueError("independent accounting expectation disagrees with the frozen output cardinality")
                except (OSError, ValueError, subprocess.SubprocessError) as error:
                    failure = destination.parent / "resource-failures" / f"account-{path['id']}-qualification"
                    failure.mkdir(parents=True, exist_ok=True)
                    artifacts = []
                    for file in sorted(work.glob(f"account-{path['id']}*")):
                        if file.is_file():
                            shutil.copyfile(file, failure / file.name)
                            artifacts.append((failure / file.name).relative_to(destination.parent).as_posix())
                    result["qualification_failures"].append(
                        {"id": f"account-{path['id']}", "error": str(error), "artifacts": artifacts})
                    continue
                selection = ["0", "0", "all"] if path["operation"].startswith("select") else []
                reconstructed = qualifications[0]["payload_sha256"] if path["id"] == "jpip-reconstruct" else None
                for resource, ceiling in ACCOUNTING_CEILINGS.items():
                    selector = "decoded-bytes" if resource == "DecodedBytes" else "work-units"
                    command = [str(executable), path["operation"] + "-limited", path["format"],
                               str(input_path), *selection, selector]
                    options = dict(selected=bool(selection), qualifications=qualifications,
                                   reconstructed_sha256=reconstructed, accounting_path=path["id"],
                                   output_protocol=path["output_protocol"],
                                   progressive_updates_expected=path.get("expected_updates"))
                    at = run_case(f"account-{path['id']}-{selector}-at", command, input_path, expected,
                                  configured_limit={"resource": resource, "value": ceiling}, search=True, **options)
                    if at["status"] == "passed":
                        run_case(f"account-{path['id']}-{selector}-below", command, input_path, expected,
                                 category="LimitExceeded", reason=resource,
                                 configured_limit={"resource": resource, "value": at["configured_limit"]["value"] - 1},
                                 **options)
        all_records = [*result["records"], *result["accounting_records"]]
        result["native_total_ms"] = sum(run["elapsed_ms"] for record in all_records
                                      for run in [*record["runs"], *record["search_runs"]])
        for report_key, run_key in (("native_user_cpu_ms", "user_cpu_ms"),
                                    ("native_system_cpu_ms", "system_cpu_ms"),
                                    ("native_total_cpu_ms", "total_cpu_ms")):
            result[report_key] = sum(run[run_key] for record in all_records
                                     for run in [*record["runs"], *record["search_runs"]])
        result["errors"] = [f"{record['id']}: {record['error']}" for record in all_records
                            if record["status"] != "passed"]
        result["errors"] += [f"{failure['id']}: unqualified reference: {failure['error']}"
                             for failure in result["qualification_failures"]]
        missing = _expected_accounting_ids() - {record["id"] for record in result["accounting_records"]}
        if missing:
            result["errors"].append(f"accounting stage incomplete: missing {sorted(missing)}")
        if result["native_total_ms"] > result["maximum_native_total_ms"]:
            result["errors"].append("aggregate measured native wall time exceeds the declared engineering threshold")
        if result["errors"]:
            result["status"] = "failed"
    _validate_evidence(result)
    framework.validate_schema(framework.VALIDATION / "resource-result.schema.json", result)
    framework.write_json(destination, result)
    return result
