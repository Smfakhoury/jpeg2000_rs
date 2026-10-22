"""Independent native-grid expectations for bounded generated selection profiles."""

from __future__ import annotations

import copy
from dataclasses import replace
from pathlib import Path
from typing import Any

import framework
import numerical


def native_rectangle(rectangle: tuple[int, ...], discard: int) -> tuple[int, ...]:
    if type(discard) is not int or not 0 <= discard <= 32:
        raise ValueError("invalid native discard level")
    scale = 1 << discard
    return tuple((value + scale - 1) // scale for value in rectangle)


def requests(frame: tuple[int, ...], discard: int) -> list[dict[str, Any]]:
    x0, y0, x1, y1 = frame
    tile = (16, y0, 32, 16)
    region = (x0 + 1, y0 + 1, x1 - x0 - 2, y1 - y0 - 2)
    inside = (17, y0 + 1, 14, 14 - y0)
    return [
        {"tiles": "all", "region": None, "bounds": native_rectangle(frame, discard)},
        {"tiles": "all", "region": region,
         "bounds": native_rectangle((region[0], region[1], x1 - 1, y1 - 1), discard)},
        {"tiles": "1", "region": None, "bounds": native_rectangle(tile, discard)},
        {"tiles": "1", "region": inside,
         "bounds": native_rectangle((17, y0 + 1, 31, 15), discard)},
        {"tiles": "0,1", "region": None,
         "bounds": native_rectangle((x0, y0, 32, 16), discard)},
    ]


def expected_crop(reference: dict[str, Any], frame: tuple[int, ...],
                  bounds: tuple[int, ...]) -> dict[str, Any]:
    if frame == bounds:
        return copy.deepcopy(reference)
    return framework.crop_image(reference, frame, bounds)


def observe(subject: Path, operation: str, format_name: str, encoded: Path,
            discard: int, request: dict[str, Any]) -> tuple[dict[str, Any], tuple[int, int] | None]:
    region = request["region"]
    process, _ = numerical._run_checked([
        str(subject), operation, format_name, str(encoded), str(discard),
        request["tiles"], "all" if region is None else ",".join(map(str, region)),
    ], 10.0)
    output = process.stdout
    origin = None
    if operation == "select":
        header, output = output.split(b"\n", 1)
        fields = header.split(b"\t")
        if len(fields) != 3 or fields[0] != b"FRAME":
            raise ValueError("selected output omitted its native frame")
        origin = (int(fields[1]), int(fields[2]))
    return framework.parse_subject_output(output, process.stderr, process.returncode), origin


def prepare_reference(case: numerical.NumericalCase, directory: Path, encoder: str,
                      decoder: str, commands: list[list[str]]) -> tuple[Path, tuple[int, ...], dict[int, dict[str, Any]]]:
    extension = numerical.source_extension(case)
    source = directory / f"original.{extension}"
    source.write_bytes(numerical.source_image(case))
    parse = framework.parse_pgx if extension == "pgx" else framework.parse_pnm
    encoder_source = source
    if case.signed:
        encoder_source = directory / "unsigned.pgx"
        encoder_source.write_bytes(numerical.source_image(replace(case, signed=False)))
    encoded = directory / "encoded.j2k"
    command = [encoder, "-i", str(encoder_source), "-o", str(encoded),
               "-n", str(case.decomposition_levels + 1), *case.options]
    commands.append(command)
    numerical._run_checked(command, 10.0)
    if case.signed:
        encoded.write_bytes(numerical.signed_codestream(encoded.read_bytes(), case.precision))
    numerical.validate_image_headers(case, encoded.read_bytes())
    references = {}
    for discard in range(case.decomposition_levels + 1):
        path = directory / f"reference-{discard}.{extension}"
        command = [decoder, "-i", str(encoded), "-o", str(path), "-r", str(discard)]
        commands.append(command)
        numerical._run_checked(command, 10.0)
        references[discard] = parse(path.with_stem(path.stem + "_0") if extension == "pgx" else path)
    metrics = numerical.image_error_metrics(parse(source), references[0])
    if metrics["maximum_error"] > case.maximum_error or metrics["worst_component_rmse"] > case.maximum_rmse:
        raise ValueError("independent original/reference selection budget exceeded")
    siz = next(payload for marker, payload in numerical._main_header_segments(encoded.read_bytes()) if marker == 0xff51)
    frame = tuple(int.from_bytes(siz[offset:offset + 4], "big") for offset in (10, 14, 2, 6))
    return encoded, frame, references
