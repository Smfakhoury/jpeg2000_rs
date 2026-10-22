"""Generated color/opacity expectations independent of decoder implementation.

Fractions express sample normalization, the declared sYCC matrix and source-over
directly. No subject output, conversion code or composition code is imported.
"""

from __future__ import annotations

from fractions import Fraction
from pathlib import Path
import struct
from typing import Any

import framework
import numerical
import selection

COLOR_PROFILES = (
    ("q2-srgb8-native", 8, 16, (1, 3), ((1, 1), (2, 2), (2, 2)), 0),
    ("q2-srgb16-native", 16, 16, (1, 3), ((1, 1), (2, 1), (2, 1)), 0),
    ("q2-sycc8-native", 8, 18, (2, 4), ((1, 1), (2, 2), (2, 2)), 514),
    ("q2-sycc16-native", 16, 18, (2, 4), ((1, 1), (2, 1), (2, 1)), 12),
)


def image(width: int, height: int, planes: list[list[int]], precision: int = 16) -> dict[str, Any]:
    result = {"accepted": True, "image": {
        "width": width, "height": height,
        "components": [{"width": width, "height": height, "precision": precision,
                        "signed": False, "sample_kind": "unsigned", "samples": plane}
                       for plane in planes],
    }}
    numerical.image_error_metrics(result, result)
    return result


def normalize(value: int, precision: int, signed: bool = False) -> int:
    if type(precision) is not int or not 1 <= precision <= 32 or type(signed) is not bool:
        raise ValueError("invalid sample representation")
    lower = -(1 << (precision - 1)) if signed else 0
    if type(value) is not int or not lower <= value < lower + (1 << precision):
        raise ValueError("sample outside its declared range")
    return int(Fraction(value - lower, (1 << precision) - 1) * 65535)


def generated_planes(precision: int, sampling: tuple[tuple[int, int], ...]) -> list[dict[str, Any]]:
    peak = (1 << precision) - 1
    return [{"width": 32 // sx, "height": 32 // sy, "precision": precision,
             "signed": False, "sample_kind": "unsigned",
             "samples": [(0 if x == 0 and y == 0 else peak if x == 1 and y == 0 else
                          (x * 31 + y * 17 + channel * 71 + x * y * 13) % 256 * peak // 255)
                         for y in range(32 // sy) for x in range(32 // sx)]}
            for channel, (sx, sy) in enumerate(sampling)]


def gray_alpha_planes(precision: int, premultiplied: bool) -> list[dict[str, Any]]:
    peak = (1 << precision) - 1
    levels = (0, 1, 2, peak // 4, peak // 2, peak // 2 + 1, peak - 1, peak)
    planes = generated_planes(precision, ((1, 1), (1, 1)))
    alpha = [levels[(x + 3 * y) % len(levels)] for y in range(32) for x in range(32)]
    planes[0]["samples"] = alpha
    if premultiplied:
        planes[1]["samples"] = [value * a // peak for value, a in zip(planes[1]["samples"], alpha)]
    return planes


def gray_alpha_header(precision: int, premultiplied: bool) -> bytes:
    return (framework.box(b"ihdr", struct.pack(">IIH4B", 32, 32, 2, precision - 1, 7, 0, 0)) +
            framework.box(b"colr", struct.pack(">BBBI", 1, 0, 0, 17)) +
            framework.box(b"cdef", struct.pack(">7H", 2, 0, 2 if premultiplied else 1, 0, 1, 0, 1)))


def mixed_codestream(legacy: bytes, ht: bytes) -> bytes:
    """Author a two-tile fixture; CAP declares legacy/HT and differing headers."""
    for data, style in ((legacy, 0), (ht, 0x40)):
        numerical.validate_image_headers(numerical.NumericalCase("mixed"), data)
        segments = numerical._main_header_segments(data)
        siz = next(payload for marker, payload in segments if marker == 0xff51)
        coding = next(payload for marker, payload in segments if marker == 0xff52)
        if (siz[2:34] != struct.pack(">8I", 32, 32, 0, 0, 32, 32, 0, 0)
                or coding[8] != style or not data.endswith(b"\xff\xd9")):
            raise ValueError("mixed fixture requires canonical 32x32 legacy/HT single tiles")
    legacy_start, ht_start = legacy.index(b"\xff\x90"), ht.index(b"\xff\x90")
    main = bytearray(ht[:ht_start])
    main[8:12] = struct.pack(">I", 64)
    cap = main.index(b"\xff\x50")
    main[cap + 8:cap + 10] = struct.pack(">H", 0x8802)
    legacy_tile = bytearray(legacy[legacy_start:-2])
    ht_tile = bytearray(ht[ht_start:-2])
    for tile in (legacy_tile, ht_tile):
        if (tile[:6] != b"\xff\x90\x00\x0a\x00\x00" or tile[10:12] != b"\x00\x01"
                or int.from_bytes(tile[6:10], "big") != len(tile) or tile[12:14] != b"\xff\x93"):
            raise ValueError("mixed fixture requires one bounded tile part without extra tile headers")
    extra = b"".join(framework.segment(marker, payload)
                     for marker, payload in numerical._main_header_segments(legacy)
                     if marker in (0xff52, 0xff5c))
    legacy_tile[6:10] = struct.pack(">I", len(legacy_tile) + len(extra))
    legacy_tile = bytes(legacy_tile[:12]) + extra + bytes(legacy_tile[12:])
    ht_tile[4:6] = b"\x00\x01"
    return bytes(main) + legacy_tile + bytes(ht_tile) + b"\xff\xd9"


def convert(reference: dict[str, Any], frame: tuple[int, ...],
            sampling: tuple[tuple[int, int], ...], color_space: int,
            gray_alpha: bool = False) -> dict[str, Any]:
    numerical.image_error_metrics(reference, reference)
    components = reference["image"]["components"]
    count = 2 if gray_alpha else 3
    if len(components) != count or len(sampling) != count or color_space not in (16, 17, 18):
        raise ValueError("unsupported independent color profile")
    if gray_alpha != (color_space == 17):
        raise ValueError("grayscale expectations require an alpha/gray pair")
    x0, y0, x1, y1 = frame
    planes = []
    for component, (sx, sy) in zip(components, sampling):
        if sx < 1 or sy < 1:
            raise ValueError("invalid component sampling")
        cx0, cy0 = (x0 + sx - 1) // sx, (y0 + sy - 1) // sy
        width, height = component["width"], component["height"]
        samples = component["samples"]
        if width * height != len(samples):
            raise ValueError("incomplete independent component")
        planes.append([
            samples[max(0, min(height - 1, y // sy - cy0)) * width
                    + max(0, min(width - 1, x // sx - cx0))]
            for y in range(y0, y1) for x in range(x0, x1)
        ])
    if gray_alpha:
        alpha, gray = [normalize(value, components[0]["precision"], components[0]["signed"])
                       for value in planes[0]], [
                           normalize(value, components[1]["precision"], components[1]["signed"])
                           for value in planes[1]]
        return image(x1 - x0, y1 - y0, [gray, alpha])
    if color_space == 16:
        return image(x1 - x0, y1 - y0, [
            [normalize(value, component["precision"], component["signed"]) for value in plane]
            for component, plane in zip(components, planes)
        ])
    if any(component["signed"] or component["precision"] != components[0]["precision"]
           for component in components):
        raise ValueError("sYCC expectation requires uniform unsigned components")
    precision = components[0]["precision"]
    peak, center = (1 << precision) - 1, 1 << (precision - 1)
    output: list[list[int]] = [[], [], []]
    for y, cb, cr in zip(*planes):
        values = (
            y + Fraction(1402, 1000) * (cr - center),
            y - Fraction(344136, 1000000) * (cb - center) - Fraction(714136, 1000000) * (cr - center),
            y + Fraction(1772, 1000) * (cb - center),
        )
        for plane, value in zip(output, values):
            plane.append(int(max(Fraction(0), min(Fraction(peak), value)) * 65535 / peak))
    return image(x1 - x0, y1 - y0, output)


def source_over(width: int, height: int, placements: list[tuple]) -> dict[str, Any]:
    if not placements:
        raise ValueError("source-over requires at least one placement")
    channels = len(placements[0][0]["image"]["components"])
    if channels not in (2, 4):
        raise ValueError("source-over requires gray/alpha or RGBA")
    output = [[0] * (width * height) for _ in range(channels)]
    for observation, premultiplied, target, source in placements:
        components = observation["image"]["components"]
        if len(components) != channels or any(component["precision"] != 16 or component["signed"]
                                               for component in components):
            raise ValueError("source-over requires matching normalized unsigned channels")
        numerical.image_error_metrics(observation, observation)
        tx, ty, tw, th = target
        sx, sy, sw, sh = source
        iw, ih = observation["image"]["width"], observation["image"]["height"]
        if (min(tx, ty, sx, sy) < 0 or min(tw, th, sw, sh) < 1
                or sx + sw > iw or sy + sh > ih):
            raise ValueError("invalid source-over placement")
        for y in range(min(th, height - ty)):
            for x in range(min(tw, width - tx)):
                index = (sy + y * sh // th) * iw + sx + x * sw // tw
                pixel = (ty + y) * width + tx + x
                alpha_sample = components[-1]["samples"][index]
                a, b = Fraction(alpha_sample, 65535), Fraction(output[-1][pixel], 65535)
                alpha = a + (1 - a) * b
                for channel in range(channels - 1):
                    sample = components[channel]["samples"][index]
                    if premultiplied and sample > alpha_sample:
                        raise ValueError("premultiplied sample exceeds alpha")
                    color = Fraction(sample, 65535)
                    numerator = (color if premultiplied else color * a)
                    numerator += Fraction(output[channel][pixel], 65535) * b * (1 - a)
                    output[channel][pixel] = int(numerator / alpha * 65535 + Fraction(1, 2)) if alpha else 0
                output[-1][pixel] = int(alpha * 65535 + Fraction(1, 2))
    return image(width, height, output)


def planar_reference(directory: Path, planes: list[dict[str, Any]],
                     sampling: tuple[tuple[int, int], ...], origin: tuple[int, int],
                     encoder: str, decoder: str, commands: list[list[str]],
                     decompositions: int = 2) -> tuple[Path, dict[int, dict[str, Any]]]:
    precision = planes[0]["precision"]
    if precision not in (8, 16) or any(plane["signed"] or plane["precision"] != precision for plane in planes):
        raise ValueError("planar reference requires uniform unsigned 8/16-bit words")
    if len(planes) != len(sampling):
        raise ValueError("planar sampling count mismatch")
    source = {"accepted": True, "image": {"width": 32, "height": 32, "components": planes}}
    numerical.image_error_metrics(source, source)
    framework.write_json(directory / "original.json", source)
    raw = directory / "original.raw"
    raw.write_bytes(b"".join(value.to_bytes(precision // 8, "big")
                             for plane in planes for value in plane["samples"]))
    encoded = directory / "encoded.j2k"
    command = [encoder, "-i", str(raw), "-o", str(encoded),
               "-F", f"32,32,{len(planes)},{precision},u@" +
               ":".join(f"{sx}x{sy}" for sx, sy in sampling), "-n", str(decompositions + 1),
               "-mct", "0", "-d", ",".join(map(str, origin)), "-t", "16,16"]
    commands.append(command)
    numerical._run_checked(command, 10.0)
    segments = numerical._main_header_segments(encoded.read_bytes())
    siz = next(payload for marker, payload in segments if marker == 0xff51)
    coding = next(payload for marker, payload in segments if marker == 0xff52)
    geometry = struct.pack(">8I", origin[0] + 32, origin[1] + 32, *origin, 16, 16, 0, 0)
    if (siz[2:34] != geometry or siz[34:36] != struct.pack(">H", len(planes))
            or siz[36:] != b"".join(bytes((precision - 1, sx, sy)) for sx, sy in sampling)
            or coding[4:6] != bytes((0, decompositions)) or coding[9] != 1):
        raise ValueError("independent planar SIZ/COD differs from declared profile")
    references = {}
    for discard in range(decompositions + 1):
        components = []
        for channel in range(len(planes)):
            path = directory / f"reference-{discard}-{channel}.pgx"
            command = [decoder, "-i", str(encoded), "-o", str(path), "-c", str(channel), "-r", str(discard)]
            commands.append(command)
            numerical._run_checked(command, 10.0)
            components.extend(framework.parse_pgx(path.with_stem(path.stem + "_0"))["image"]["components"])
        frame = selection.native_rectangle((*origin, origin[0] + 32, origin[1] + 32), discard)
        references[discard] = {"accepted": True, "image": {
            "width": frame[2] - frame[0], "height": frame[3] - frame[1], "components": components,
        }}
    if numerical.image_error_metrics(source, references[0])["maximum_error"] != 0:
        raise ValueError("independent original/reference planar mismatch")
    return encoded, references
