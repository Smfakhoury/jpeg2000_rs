import copy
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
from contextlib import contextmanager
from fractions import Fraction
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import framework
import numerical
import container_relations
import selection


def image(width, height, planes, precision=16):
    return {"accepted": True, "image": {
        "width": width, "height": height,
        "components": [{"width": width, "height": height, "precision": precision,
                        "signed": False, "sample_kind": "unsigned", "samples": samples}
                       for samples in planes],
    }}


def declared_legacy_jph(data):
    if len(data) < 45 or data[:4] != b"\xff\x4f\xff\x51" or data[6:8] != b"\x00\x00":
        raise ValueError("fixture requires a legacy codestream starting with SIZ")
    end = 4 + int.from_bytes(data[4:6], "big")
    if end > len(data) or end < 45 or end != 42 + 3 * int.from_bytes(data[40:42], "big"):
        raise ValueError("fixture has truncated SIZ")
    result = bytearray(data)
    result[6:8] = b"\x40\x00"
    return bytes(result[:end]) + framework.segment(0xff50, struct.pack(">IH", 0x00020000, 0x801f)) + bytes(result[end:])


def crop_converted(observation, origin, region, discard=0):
    scale = 1 << discard
    x, y, width, height = region
    left = (x + scale - 1) // scale - (origin[0] + scale - 1) // scale
    top = (y + scale - 1) // scale - (origin[1] + scale - 1) // scale
    right = (x + width + scale - 1) // scale - (origin[0] + scale - 1) // scale
    bottom = (y + height + scale - 1) // scale - (origin[1] + scale - 1) // scale
    source = observation["image"]
    if not 0 <= left < right <= source["width"] or not 0 <= top < bottom <= source["height"]:
        raise ValueError("converted fixture crop is empty or outside its frame")
    result = copy.deepcopy(observation)
    result["image"]["width"], result["image"]["height"] = right - left, bottom - top
    for component in result["image"]["components"]:
        values, stride = component["samples"], component["width"]
        component["samples"] = [values[row * stride + column]
                               for row in range(top, bottom) for column in range(left, right)]
        component["width"], component["height"] = right - left, bottom - top
    return result


def vbas(value):
    if value < 0:
        raise ValueError("VBAS fixture values must be nonnegative")
    groups = [value & 127]
    while value >> 7:
        value >>= 7
        groups.append(value & 127)
    return bytes(group | (128 if index else 0) for index, group in reversed(list(enumerate(groups))))


def message(class_id, offset, body, final=False, inherited=False):
    prefix = bytes([(0x20 if inherited else 0x60) | (0x10 if final else 0)])
    return prefix + (b"" if inherited else vbas(class_id) + b"\0") + vbas(offset) + vbas(len(body)) + body


def split_single_tile(data):
    if not data.startswith(b"\xff\x4f") or not data.endswith(b"\xff\xd9"):
        raise ValueError("fixture must be an SOC/EOC-delimited codestream")
    position = 2
    tile = None
    lengths = []
    pending = 0
    inline_header = bytearray()
    while position + 4 <= len(data):
        marker, size = struct.unpack_from(">HH", data, position)
        if marker == 0xff93:
            if tile is None:
                raise ValueError("fixture has no tile header")
            end = tile + int.from_bytes(data[tile + 6:tile + 10], "big")
            if end != len(data) - 2:
                raise ValueError("fixture must contain exactly one bounded tile-part")
            inline_header += b"\xff\x93"
            inline_header[6:10] = struct.pack(">I", len(inline_header) + end - position - 2)
            return data[:tile], data[tile:position + 2], data[position + 2:end], lengths, bytes(inline_header)
        if size < 2 or position + 2 + size > len(data):
            raise ValueError("fixture has a truncated marker segment")
        if marker == 0xff90:
            if tile is not None:
                raise ValueError("fixture has multiple tile-parts")
            tile = position
        if marker == 0xff58:
            for value in data[position + 5:position + 2 + size]:
                pending = pending * 128 + (value & 127)
                if not value & 128:
                    lengths.append(pending)
                    pending = 0
        elif tile is not None:
            inline_header += data[position:position + 2 + size]
        position += 2 + size
    raise ValueError("fixture has no SOD")


def rgba_header(case, premultiplied, order=(0, 1, 2, 3)):
    if sorted(order) != [0, 1, 2, 3]:
        raise ValueError("RGBA fixture requires a permutation of its four channels")
    header = framework.box(b"ihdr", struct.pack(">IIH4B", case.height, case.width, 4,
                                               case.precision - 1, 7, 0, 0))
    header += framework.box(b"colr", bytes((1, 0, 0, 0, 0, 0, 16)))
    definitions = struct.pack(">H", 4)
    for physical, logical in enumerate(order):
        definitions += struct.pack(">HHH", physical,
                                   (2 if premultiplied else 1) if logical == 3 else 0,
                                   0 if logical == 3 else logical + 1)
    return header + framework.box(b"cdef", definitions)


def rgba_container(raw, case, premultiplied, format_name="jp2", order=(0, 1, 2, 3)):
    header = rgba_header(case, premultiplied, order)
    if format_name == "jpx":
        return jpx_file([raw], case, headers=[header])
    if format_name not in ("jp2", "jph"):
        raise ValueError("unsupported RGBA fixture container")
    brand = b"jp2 " if format_name == "jp2" else b"jph "
    codestream = raw if format_name == "jp2" else declared_legacy_jph(raw)
    return (framework.box(b"jP  ", b"\x0d\x0a\x87\x0a") +
            framework.box(b"ftyp", brand + b"\0\0\0\0" + brand) +
            framework.box(b"jp2h", header) + framework.box(b"jp2c", codestream))


def jpx_file(codestreams, case, instructions=None, chroma=False, headers=None, opacities=None):
    box = framework.box
    header = box(b"ihdr", struct.pack(">IIH4B", case.height, case.width, case.channels,
                                     case.precision - 1, 7, 0, 0))
    header += box(b"colr", bytes([1, 0, 0, 0, 0, 0, 17 if case.channels == 1 else 16]))
    headers = [header] * len(codestreams) if headers is None else headers
    if not headers or len(headers) != len(codestreams):
        raise ValueError("JPX fixture headers must match its codestreams")
    if opacities is not None and (chroma or len(opacities) != len(codestreams)
                                  or any(value not in (0, 1) for value in opacities)):
        raise ValueError("JPX fixture opacity declarations must match its layers")
    result = box(b"jP  ", b"\x0d\x0a\x87\x0a") + box(b"ftyp", b"jpxb\0\0\0\0jpxb")
    result += box(b"jp2h", headers[0])
    result += b"".join(box(b"jpch", current) for current in headers)
    for index in range(len(codestreams)):
        layer = box(b"creg", struct.pack(">HHH4B", 1, 1, index, 1, 1, 0, 0))
        if opacities is not None:
            layer += box(b"opct", bytes((opacities[index],)))
        if chroma and index == 1:
            layer += box(b"opct", bytes([2, case.channels]) + bytes(case.channels * (case.precision // 8)))
        result += box(b"jplh", layer)
    composition = box(b"copt", struct.pack(">IIB", case.height + 3, case.width + 4, 0))
    if instructions is not None:
        payload = struct.pack(">HHI", 0x27, 0, 1)
        payload += b"".join(struct.pack(">10I", *record) for record in instructions)
        composition += box(b"inst", payload)
    result += box(b"comp", composition)
    return result + b"".join(box(b"jp2c", data) for data in codestreams)


def converted(source, chroma=False):
    data = source["image"]
    peak = (1 << data["components"][0]["precision"]) - 1
    planes = [[sample * 65535 // peak for sample in component["samples"]] for component in data["components"]]
    alpha = [0 if chroma and all(plane[index] == 0 for plane in planes) else 65535
             for index in range(data["width"] * data["height"])]
    return image(data["width"], data["height"], planes + [alpha])


def canvas(width, height, placements):
    channels = len(placements[0][0]["image"]["components"])
    planes = [[0] * (width * height) for _ in range(channels)]
    for observation, target, source in placements:
        data = observation["image"]
        tx, ty, tw, th = target
        sx, sy, sw, sh = source
        for y in range(th):
            for x in range(tw):
                if tx + x >= width or ty + y >= height:
                    continue
                src = (sy + y * sh // th) * data["width"] + sx + x * sw // tw
                dst = (ty + y) * width + tx + x
                alpha = data["components"][-1]["samples"][src]
                if alpha not in (0, 65535):
                    raise ValueError("this fixture compositor requires binary alpha")
                if alpha:
                    for output, component in zip(planes, data["components"]):
                        output[dst] = component["samples"][src]
    return image(width, height, planes)


def fractional_canvas(width, height, placements):
    output = [[0] * (width * height) for _ in range(4)]
    for observation, premultiplied, target, source in placements:
        components = observation["image"]["components"]
        if len(components) != 4 or any(component["precision"] != 16 for component in components):
            raise ValueError("fractional fixture compositor requires 16-bit RGBA")
        tx, ty, tw, th = target
        sx, sy, sw, sh = source
        for y in range(th):
            for x in range(tw):
                if tx + x >= width or ty + y >= height:
                    continue
                index = (sy + y * sh // th) * observation["image"]["width"] + sx + x * sw // tw
                pixel = (ty + y) * width + tx + x
                alpha_sample = components[3]["samples"][index]
                source_alpha = Fraction(alpha_sample, 65535)
                destination_alpha = Fraction(output[3][pixel], 65535)
                alpha = source_alpha + (1 - source_alpha) * destination_alpha
                for channel in range(3):
                    sample = components[channel]["samples"][index]
                    if premultiplied and sample > alpha_sample:
                        raise ValueError("premultiplied fixture sample exceeds alpha")
                    value = Fraction(sample, 65535)
                    source_color = value if premultiplied else value * source_alpha
                    destination_color = Fraction(output[channel][pixel], 65535)
                    numerator = source_color + destination_color * destination_alpha * (1 - source_alpha)
                    color = numerator / alpha if alpha else Fraction(0)
                    output[channel][pixel] = int(color * 65535 + Fraction(1, 2))
                output[3][pixel] = int(alpha * 65535 + Fraction(1, 2))
    return image(width, height, output)


class ContainerRelationsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.subject = framework.build_subject()

    def setUp(self):
        root = framework.RESULTS_DIR / "test-work"
        root.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=root)
        self.addCleanup(self.temporary.cleanup)
        self.work = Path(self.temporary.name)

    @contextmanager
    def fixture(self, name):
        directory = self.work / name
        directory.mkdir()
        try:
            yield directory
        except (AssertionError, OSError, ValueError, subprocess.SubprocessError) as error:
            files = {path.relative_to(directory).as_posix(): framework.sha256_file(path)
                     for path in directory.rglob("*") if path.is_file()}
            test_hash = framework.sha256_file(Path(__file__))
            subject_hash = framework.sha256_file(self.subject)
            failure = framework.sha256_bytes(framework.canonical_bytes({
                "files": files, "test": test_hash, "subject": subject_hash,
            }))[:16]
            destination = framework.RESULTS_DIR / "container-failures" / name / failure
            shutil.copytree(directory, destination, dirs_exist_ok=True)
            framework.write_json(destination / "reproduction.json", {
                "case": name, "error": str(error), "provenance": framework.source_binding(),
                "test_sha256": test_hash, "subject_sha256": subject_hash, "files": files,
            })
            raise

    def encode(self, directory, case, high_throughput=False, options=()):
        source = directory / f"{case.name}.{'ppm' if case.channels == 3 else 'pgm'}"
        encoded = directory / f"{case.name}.j2k"
        reference = source.with_name(f"{case.name}-reference{source.suffix}")
        source.write_bytes(numerical.source_image(case))
        encoder = os.environ.get("OPENJPH_ENCODER", "ojph_compress") if high_throughput else "opj_compress"
        decoder = os.environ.get("OPENJPH_DECODER", "ojph_expand") if high_throughput else "opj_decompress"
        encoder, decoder = shutil.which(encoder), shutil.which(decoder)
        if encoder is None or decoder is None:
            raise FileNotFoundError("container reference encoder/decoder is unavailable")
        parameters = (["-num_decomps", str(case.decomposition_levels), "-reversible", "true"]
                      if high_throughput else ["-n", str(case.decomposition_levels + 1)])
        framework.write_json(directory / f"{case.name}-tools.json", {
            "tools": {role: {"path": path, "sha256": framework.sha256_file(Path(path))}
                      for role, path in (("encoder", encoder), ("decoder", decoder))},
            "encoder_options": [*parameters, *options],
        })
        numerical._run_checked([encoder, "-i", str(source), "-o", str(encoded), *parameters, *options], 10.0)
        numerical._run_checked([decoder, "-i", str(encoded), "-o", str(reference)], 10.0)
        expected = framework.parse_pnm(reference)
        self.assertEqual(framework.normalized_hash(expected), framework.normalized_hash(framework.parse_pnm(source)))
        return encoded.read_bytes(), expected

    def observe(self, operation, format_name, path):
        process, _ = numerical._run_checked([str(self.subject), operation, format_name, str(path)], 10.0)
        return framework.parse_subject_output(process.stdout, process.stderr, process.returncode)

    def assert_image(self, expected, actual):
        self.assertEqual(numerical.image_error_metrics(expected, actual)["maximum_error"], 0)

    @unittest.skipUnless(shutil.which("opj_compress") and shutil.which("opj_decompress"),
                         "independent OpenJPEG tools are not installed")
    def test_q2_independent_color_opacity_and_composition_profiles(self):
        directory = framework.RESULTS_DIR / "q2-container"
        directory.mkdir(parents=True, exist_ok=True)
        opacity_profiles = [(f"q2-gray-alpha{precision}-{'premultiplied' if premultiplied else 'straight'}",
                             precision, premultiplied) for precision in (8, 16) for premultiplied in (False, True)]
        framework.write_json(directory / "matrix.json", {
            "color": container_relations.COLOR_PROFILES, "opacity": opacity_profiles,
            "opacity_maximum_error": 0, "opacity_maximum_rmse": 0,
            "color_rmse_ceiling": "same as maximum_error; normalized 16-bit units",
            "containers": ["jp2", "jph-declared-legacy", "jpx-indexed"],
            "discards": [0, 1, 2], "premultiplied_discards": [0],
        })
        matrix_hash = framework.sha256_file(directory / "matrix.json")
        encoder, decoder = shutil.which("opj_compress"), shutil.which("opj_decompress")
        commands, prepared = [], []

        def append(name, path, expected, budget, operation, format_name, discard=None, request=None):
            prepared.append({"id": name, "path": str(path), "expected": expected,
                             "maximum_error": budget, "maximum_rmse": budget, "operation": operation,
                             "format": format_name, "discard": discard, "request": request})

        def selected_expectations(name, path, references, frame, sampling, space, budget, format_name, alpha=False):
            for discard, reference in references.items():
                native = selection.native_rectangle(frame, discard)
                expected = container_relations.convert(reference, native, sampling, space, alpha)
                if discard == 0:
                    append(name, path, expected, budget, "convert", format_name)
                for request in selection.requests(frame, discard):
                    append(name, path, selection.expected_crop(expected, native, request["bounds"]),
                           budget, "select-color", format_name, discard, request)

        for name, precision, space, origin, sampling, budget in container_relations.COLOR_PROFILES:
            inputs = directory / name
            inputs.mkdir(exist_ok=True)
            raw, references = container_relations.planar_reference(
                inputs, container_relations.generated_planes(precision, sampling), sampling, origin,
                encoder, decoder, commands)
            frame = (*origin, origin[0] + 32, origin[1] + 32)
            for format_name, brand in (("jp2", b"jp2 "), ("jph", b"jph "), ("jpx", b"jpx ")):
                path = inputs / f"image.{format_name}"
                path.write_bytes(framework.jp2_file(
                    declared_legacy_jph(raw.read_bytes()) if format_name == "jph" else raw.read_bytes(),
                    brand, width=32, height=32, components=3, precision=precision, color_space=space))
                selected_expectations(name, path, references, frame, sampling, space, budget, format_name)
        for name, precision, premultiplied in opacity_profiles:
            inputs = directory / name
            inputs.mkdir(exist_ok=True)
            sampling, frame = ((1, 1), (1, 1)), (3, 5, 35, 37)
            raw, references = container_relations.planar_reference(
                inputs, container_relations.gray_alpha_planes(precision, premultiplied),
                sampling, (3, 5), encoder, decoder, commands, 0 if premultiplied else 2)
            header = container_relations.gray_alpha_header(precision, premultiplied)
            case = numerical.NumericalCase(name, channels=2, precision=precision)
            for format_name, brand in (("jp2", b"jp2 "), ("jph", b"jph "), ("jpx", b"jpx ")):
                path = inputs / f"image.{format_name}"
                if format_name == "jpx":
                    data = jpx_file([raw.read_bytes()], case, headers=[header])
                else:
                    codestream = declared_legacy_jph(raw.read_bytes()) if format_name == "jph" else raw.read_bytes()
                    data = (framework.box(b"jP  ", b"\x0d\x0a\x87\x0a") +
                            framework.box(b"ftyp", brand + b"\0" * 4 + brand) +
                            framework.box(b"jp2h", header) + framework.box(b"jp2c", codestream))
                path.write_bytes(data)
                selected_expectations(name, path, references, frame, sampling, 17, 0, format_name, True)
            full = (0, 0, 32, 32)
            target, source = (3, 2, 24, 20), (1, 3, 12, 10)
            records = [(0, 0, 32, 32, 0x80000000, 0, *full), (*target, 100, 0, *source)]
            path = inputs / "composition.jpx"
            path.write_bytes(jpx_file([raw.read_bytes(), raw.read_bytes()], case, records, headers=[header, header]))
            layer = container_relations.convert(references[0], frame, sampling, 17, True)
            expected = container_relations.source_over(36, 35, [
                (layer, premultiplied, full, full), (layer, premultiplied, target, source),
            ])
            append(name, path, expected, 0, "render", "jpx")
            append(name, path, expected, 0, "render-frame", "0")
        reference_path = directory / "references.json"
        framework.write_json(reference_path, {
            "matrix_sha256": matrix_hash, "provenance": framework.source_binding(), "commands": commands,
            "reference_source_recipes": numerical.REFERENCE_SOURCE_RECIPES,
            "tools": {role: {"path": path, "sha256": framework.sha256_file(Path(path))}
                      for role, path in (("encoder", encoder), ("decoder", decoder))},
            "records": prepared,
            "files": {path.relative_to(directory).as_posix(): framework.sha256_file(path)
                      for path in directory.rglob("*") if path.is_file() and path.name not in ("references.json", "results.json")},
        })
        reference_hash = framework.sha256_file(reference_path)
        results = []
        for record in prepared:
            with self.subTest(case=record["id"], operation=record["operation"],
                              format=record["format"], discard=record["discard"], request=record["request"]):
                if record["discard"] is None:
                    actual = self.observe(record["operation"], record["format"], Path(record["path"]))
                else:
                    actual, _ = selection.observe(self.subject, record["operation"], record["format"],
                                                  Path(record["path"]), record["discard"], record["request"])
                metrics = numerical.image_error_metrics(record["expected"], actual)
                self.assertLessEqual(metrics["maximum_error"], record["maximum_error"])
                self.assertLessEqual(metrics["worst_component_rmse"], record["maximum_rmse"])
                results.append({key: value for key, value in record.items() if key != "expected"} | {"metrics": metrics})
        framework.write_json(directory / "results.json", {
            "reference_sha256": reference_hash, "provenance": framework.source_binding(),
            "subject_sha256": framework.sha256_file(self.subject), "records": results,
            "summary": {"expected": len(prepared), "observed": len(results),
                        "status": "passed" if len(results) == len(prepared) else "failed"},
        })

    @unittest.skipUnless(shutil.which("opj_compress") and shutil.which("opj_decompress")
                         and shutil.which(os.environ.get("OPENJPH_ENCODER", "ojph_compress"))
                         and shutil.which(os.environ.get("OPENJPH_DECODER", "ojph_expand")),
                         "independent OpenJPEG/OpenJPH tools are not installed")
    def test_q2_nonzero_mixed_tiles_require_whole_stream_reference(self):
        directory = framework.RESULTS_DIR / "q2-mixed"
        directory.mkdir(parents=True, exist_ok=True)
        framework.write_json(directory / "matrix.json", {
            "formats": ["raw", "jph"], "geometry": [64, 32], "tile": [32, 32],
            "discards": [0, 1, 2], "maximum_error": 0, "maximum_rmse": 0,
            "reference": "Unmodified OpenJPEG whole-stream output, not stitched subject/constituent outputs",
        })
        matrix_hash = framework.sha256_file(directory / "matrix.json")
        case = numerical.NumericalCase("mixed")
        legacy, original = self.encode(directory, case)
        ht, _ = self.encode(directory, numerical.NumericalCase("mixed-ht"), high_throughput=True)
        raw = container_relations.mixed_codestream(legacy, ht)
        path = directory / "mixed.j2k"
        path.write_bytes(raw)
        expected = copy.deepcopy(original)
        expected["image"]["width"] = 64
        component = expected["image"]["components"][0]
        values = component["samples"]
        component["samples"] = [value for row in range(32) for value in values[row * 32:(row + 1) * 32] * 2]
        component["width"] = 64
        decoder = shutil.which("opj_decompress")
        references, commands = {}, []
        for discard in range(3):
            output = directory / f"reference-{discard}.pgm"
            command = [decoder, "-i", str(path), "-o", str(output), "-r", str(discard)]
            commands.append(command)
            numerical._run_checked(command, 10.0)
            references[discard] = framework.parse_pnm(output)
        self.assert_image(expected, references[0])
        framework.write_json(directory / "references.json", {
            "matrix_sha256": matrix_hash, "commands": commands, "references": references,
            "reference_source_recipes": numerical.REFERENCE_SOURCE_RECIPES,
            "codestream_sha256": framework.sha256_file(path), "provenance": framework.source_binding(),
            "decoder": {"path": decoder, "sha256": framework.sha256_file(Path(decoder))},
        })
        reference_hash = framework.sha256_file(directory / "references.json")
        results = []
        for format_name in ("raw", "jph"):
            encoded = path
            if format_name == "jph":
                encoded = directory / "mixed.jph"
                encoded.write_bytes(framework.jp2_file(raw, b"jph ", width=64, height=32))
            self.assert_image(expected, self.observe("decode", format_name, encoded))
            for discard, reference in references.items():
                frame = selection.native_rectangle((0, 0, 64, 32), discard)
                for tiles, region, rectangle in (
                    ("all", None, (0, 0, 64, 32)), ("all", (1, 1, 62, 30), (1, 1, 63, 31)),
                    ("0", None, (0, 0, 32, 32)), ("1", None, (32, 0, 64, 32)),
                    ("1", (33, 1, 30, 30), (33, 1, 63, 31)),
                ):
                    bounds = selection.native_rectangle(rectangle, discard)
                    request = {"tiles": tiles, "region": region}
                    with self.subTest(format=format_name, discard=discard, request=request):
                        actual, origin = selection.observe(self.subject, "select", format_name, encoded, discard, request)
                        self.assertEqual(origin, bounds[:2])
                        expected_native = selection.expected_crop(reference, frame, bounds)
                        self.assert_image(expected_native, actual)
                        results.append({"format": format_name, "discard": discard, "request": request,
                                        "expected_sha256": framework.normalized_hash(expected_native),
                                        "actual_sha256": framework.normalized_hash(actual)})
        framework.write_json(directory / "results.json", {
            "reference_sha256": reference_hash, "provenance": framework.source_binding(),
            "subject_sha256": framework.sha256_file(self.subject), "records": results,
            "summary": {"expected": 30, "observed": len(results),
                        "status": "passed" if len(results) == 30 else "failed"},
        })

    def encode_nonuniform(self, directory, sampling, precision, origin, tiled=False):
        width, height = 32, 32
        peak = (1 << precision) - 1
        planes = []
        for channel, (sx, sy) in enumerate(sampling):
            cw, ch = width // sx, height // sy
            values = [(0 if x == 0 and y == 0 else peak if x == 1 and y == 0 else
                       ((x * 31 + y * 17 + channel * 71 + x * y * 13) % 256) * peak // 255)
                      for y in range(ch) for x in range(cw)]
            planes.append({"width": cw, "height": ch, "precision": precision,
                           "signed": False, "sample_kind": "unsigned", "samples": values})
        return self.encode_planar(directory, planes, sampling, origin, tiled)

    def encode_planar(self, directory, planes, sampling, origin=(0, 0), tiled=False):
        width, height = 32, 32
        if not planes or len(planes) != len(sampling):
            raise ValueError("planar fixture components must match their sampling descriptors")
        precision = planes[0]["precision"]
        if precision not in (8, 16) or any(
                plane["precision"] != precision or plane["signed"] for plane in planes):
            raise ValueError("planar fixtures require uniform unsigned 8/16-bit components")
        source = {"accepted": True, "image": {"width": width, "height": height, "components": planes}}
        raw = directory / "source.raw"
        raw.write_bytes(b"".join(value.to_bytes(precision // 8, "big")
                                 for plane in planes for value in plane["samples"]))
        encoder = shutil.which(os.environ.get("OPENJPEG_ENCODER", "opj_compress"))
        decoder = shutil.which(os.environ.get("OPENJPEG_DECODER", "opj_decompress"))
        if encoder is None or decoder is None:
            raise FileNotFoundError("nonuniform reference tools are unavailable")
        options = ["-F", f"{width},{height},{len(planes)},{precision},u@" +
                   ":".join(f"{sx}x{sy}" for sx, sy in sampling),
                   "-n", "3", "-mct", "0", "-d", ",".join(map(str, origin))]
        if tiled:
            options += ["-t", "15,15"]
        encoded = directory / "encoded.j2k"
        framework.write_json(directory / "tools.json", {
            "tools": {role: {"path": path, "sha256": framework.sha256_file(Path(path))}
                      for role, path in (("encoder", encoder), ("decoder", decoder))},
            "encoder_options": options, "sampling": sampling,
            "source_sha256": framework.normalized_hash(source),
        })
        numerical._run_checked([encoder, "-i", str(raw), "-o", str(encoded), *options], 10.0)
        for channel, expected in enumerate(planes):
            output = directory / f"reference-{channel}.pgx"
            numerical._run_checked([decoder, "-i", str(encoded), "-o", str(output),
                                    "-c", str(channel)], 10.0)
            actual = framework.parse_pgx(output.with_stem(output.stem + "_0"))
            self.assertEqual(actual["image"]["components"], [expected],
                             "independent nonuniform roundtrip changed source samples/metadata")
        self.assert_image(source, self.observe("decode", "raw", encoded))
        for discard in (1, 2):
            references = []
            for channel in range(len(planes)):
                output = directory / f"native-{discard}-{channel}.pgx"
                numerical._run_checked([decoder, "-i", str(encoded), "-o", str(output),
                                        "-c", str(channel), "-r", str(discard)], 10.0)
                references += framework.parse_pgx(output.with_stem(output.stem + "_0"))["image"]["components"]
            scale = 1 << discard
            native = {"accepted": True, "image": {
                "width": (origin[0] + width + scale - 1) // scale - (origin[0] + scale - 1) // scale,
                "height": (origin[1] + height + scale - 1) // scale - (origin[1] + scale - 1) // scale,
                "components": references,
            }}
            process, _ = numerical._run_checked([
                str(self.subject), "select", "raw", str(encoded), str(discard), "all", "all",
            ], 10.0)
            _, pixels = process.stdout.split(b"\n", 1)
            self.assert_image(native, framework.parse_subject_output(pixels, process.stderr, process.returncode))
        return encoded.read_bytes(), source, decoder

    def selected_color(self, path, format_name, discard, tiles, region):
        process, _ = numerical._run_checked([
            str(self.subject), "select-color", format_name, str(path), str(discard),
            str(tiles), "all" if region is None else ",".join(map(str, region)),
        ], 10.0)
        return framework.parse_subject_output(process.stdout, process.stderr, process.returncode)

    def encode_alpha(self, directory, precision, premultiplied, alpha_step, seed, order):
        directory.mkdir()
        peak = (1 << precision) - 1
        alpha_width = 32 // alpha_step
        levels = (0, 1, 2, peak // 4, peak // 2, peak // 2 + 1, peak - 1, peak)
        alpha = [levels[(x + 3 * y + seed) % len(levels)]
                 for y in range(alpha_width) for x in range(alpha_width)]
        full_alpha = [alpha[(y // alpha_step) * alpha_width + x // alpha_step]
                      for y in range(32) for x in range(32)]
        colors = [[((x * 31 + y * 17 + channel * 71 + x * y * 13 + seed * 29) % 256) * peak // 255
                   for y in range(32) for x in range(32)] for channel in range(3)]
        if premultiplied:
            colors = [[value * opacity // peak for value, opacity in zip(plane, full_alpha)]
                      for plane in colors]
        planes = [{"width": 32, "height": 32, "precision": precision, "signed": False,
                   "sample_kind": "unsigned", "samples": plane} for plane in colors]
        planes.append({"width": alpha_width, "height": alpha_width, "precision": precision,
                       "signed": False, "sample_kind": "unsigned", "samples": alpha})
        sampling = ((1, 1), (1, 1), (1, 1), (alpha_step, alpha_step))
        raw, _, _ = self.encode_planar(directory, [planes[index] for index in order],
                                        [sampling[index] for index in order])
        expected = image(32, 32, [[value * 65535 // peak for value in plane]
                                 for plane in colors + [full_alpha]])
        return raw, expected

    @unittest.skipUnless(shutil.which("opj_compress") and shutil.which("opj_decompress"),
                         "independent OpenJPEG tools are not installed")
    def test_fractional_and_premultiplied_alpha_color_layers_and_frames(self):
        full = (0, 0, 32, 32)
        for precision in (8, 16):
            for premultiplied in (False, True):
                for alpha_step in (1, 2):
                    name = f"alpha-{precision}-{premultiplied}-{alpha_step}"
                    with self.subTest(case=name), self.fixture(name) as directory:
                        case = numerical.NumericalCase("rgba", channels=4, precision=precision)
                        order = (3, 2, 0, 1)
                        background, back_pixels = self.encode_alpha(
                            directory / "background", precision, not premultiplied, 1, 0, (0, 1, 2, 3))
                        foreground, front_pixels = self.encode_alpha(
                            directory / "foreground", precision, premultiplied, alpha_step, 1, order)
                        headers = [rgba_header(case, not premultiplied),
                                   rgba_header(case, premultiplied, order)]
                        for format_name in ("jp2", "jph", "jpx"):
                            path = directory / f"foreground.{format_name}"
                            path.write_bytes(rgba_container(foreground, case, premultiplied, format_name, order))
                            self.assert_image(front_pixels, self.observe("convert", format_name, path))
                            for region in ((1, 1, 9, 11), (1, 1, 1, 1)):
                                self.assert_image(crop_converted(front_pixels, (0, 0), region),
                                                  self.selected_color(path, format_name, 0, "all", region))
                        path = directory / "static.jpx"
                        path.write_bytes(jpx_file([background, foreground], case, headers=headers))
                        for index, pixels in enumerate((back_pixels, front_pixels)):
                            self.assert_image(pixels, self.observe("render-layer", str(index), path))
                        placements = [(back_pixels, not premultiplied, full, full),
                                      (front_pixels, premultiplied, full, full)]
                        self.assert_image(fractional_canvas(36, 35, placements),
                                          self.observe("render", "jpx", path))
                        target, source = (3, 2, 24, 20), (1, 3, 12, 10)
                        records = [(0, 0, 32, 32, 0x80000000, 0, *full),
                                   (*target, 100, 0, *source)]
                        path = directory / "placed.jpx"
                        path.write_bytes(jpx_file([background, foreground], case, records, headers=headers))
                        expected = fractional_canvas(36, 35, [
                            (back_pixels, not premultiplied, full, full),
                            (front_pixels, premultiplied, target, source),
                        ])
                        self.assert_image(expected, self.observe("render-frame", "0", path))
                        self.assert_image(expected, self.observe("render", "jpx", path))
                        for persistent in (False, True):
                            records = [(0, 0, 32, 32, 100 | (0x80000000 if persistent else 0), 0, *full),
                                       (*target, 100, 0, *source)]
                            path = directory / f"frames-{persistent}.jpx"
                            path.write_bytes(jpx_file([background, foreground], case, records, headers=headers))
                            placements = [(back_pixels, not premultiplied, full, full)] if persistent else []
                            placements.append((front_pixels, premultiplied, target, source))
                            self.assert_image(fractional_canvas(36, 35, placements),
                                              self.observe("render-frame", "1", path))
                            with self.assertRaisesRegex(ValueError, "FrameIndexOutOfRange"):
                                self.observe("render-frame", "2", path)
                        if not premultiplied:
                            path = directory / "invalid-premultiplied.jpx"
                            path.write_bytes(jpx_file([foreground], case, headers=[rgba_header(case, True, order)]))
                            with self.assertRaisesRegex(ValueError, "InvalidBoxContents"):
                                self.observe("render", "jpx", path)
                        path = directory / "opacity-override.jpx"
                        path.write_bytes(jpx_file(
                            [foreground], case, headers=[rgba_header(case, not premultiplied, order)],
                            opacities=[1 if premultiplied else 0],
                        ))
                        self.assert_image(
                            fractional_canvas(36, 35, [(front_pixels, premultiplied, full, full)]),
                            self.observe("render", "jpx", path))

    @unittest.skipUnless(shutil.which("opj_compress") and shutil.which("opj_decompress"),
                         "independent OpenJPEG tools are not installed")
    def test_equivalent_straight_and_premultiplied_encodings_render_identically(self):
        for precision in (8, 16):
            with self.subTest(precision=precision), self.fixture(f"alpha-equivalence-{precision}") as directory:
                peak = (1 << precision) - 1
                alpha = [(0, 1, 2, peak // 2, peak - 1, peak)[(x + 3 * y) % 6]
                         for y in range(16) for x in range(16)]
                full_alpha = [alpha[(y // 2) * 16 + x // 2] for y in range(32) for x in range(32)]
                colors = [[peak if (x + y + channel) % 2 else 0
                           for y in range(32) for x in range(32)] for channel in range(3)]
                expected_pixels = image(32, 32, [[value * 65535 // peak for value in plane]
                                                for plane in colors + [full_alpha]])
                case = numerical.NumericalCase("equivalent", channels=4, precision=precision)
                full = (0, 0, 32, 32)
                target, crop = (28, 30, 24, 20), (3, 1, 12, 10)
                observed = {}
                for premultiplied in (False, True):
                    place = directory / str(premultiplied)
                    place.mkdir()
                    encoded_colors = ([[value * opacity // peak for value, opacity in zip(plane, full_alpha)]
                                       for plane in colors] if premultiplied else colors)
                    planes = [{"width": 32, "height": 32, "precision": precision, "signed": False,
                               "sample_kind": "unsigned", "samples": plane} for plane in encoded_colors]
                    planes.append({"width": 16, "height": 16, "precision": precision, "signed": False,
                                   "sample_kind": "unsigned", "samples": alpha})
                    raw, _, _ = self.encode_planar(place, planes, ((1, 1), (1, 1), (1, 1), (2, 2)))
                    path = directory / f"static-{premultiplied}.jpx"
                    header = rgba_header(case, premultiplied)
                    path.write_bytes(jpx_file([raw], case, headers=[header]))
                    observed[premultiplied] = self.observe("render", "jpx", path)
                    self.assert_image(
                        fractional_canvas(36, 35, [(expected_pixels, False, full, full)]),
                        observed[premultiplied])
                    path = directory / f"placed-{premultiplied}.jpx"
                    records = [(*target, 100, 0, *crop)]
                    path.write_bytes(jpx_file([raw], case, records, headers=[header]))
                    self.assert_image(
                        fractional_canvas(36, 35, [(expected_pixels, False, target, crop)]),
                        self.observe("render-frame", "0", path))
                self.assert_image(observed[False], observed[True])

    @unittest.skipUnless(shutil.which("opj_compress") and shutil.which("opj_decompress"),
                         "independent OpenJPEG tools are not installed")
    def test_nonuniform_components_full_color_and_selection_context(self):
        for sampling in (((1, 1), (2, 2), (2, 2)), ((1, 1), (2, 1), (2, 1))):
            for precision, origin, tiled in ((8, (0, 0), False), (16, (2, 4), True),
                                             (8, (1, 3), True)):
                name = f"nonuniform-{sampling[1][1]}-{precision}-{origin[0]}"
                with self.subTest(case=name), self.fixture(name) as directory:
                    raw, source, decoder = self.encode_nonuniform(directory, sampling, precision, origin, tiled)
                    peak = (1 << precision) - 1
                    expected = image(32, 32, [], precision=16)
                    for channel, (plane, (sx, sy)) in enumerate(zip(source["image"]["components"], sampling)):
                        cx0, cy0 = (origin[0] + sx - 1) // sx, (origin[1] + sy - 1) // sy
                        pixels = [
                            plane["samples"][
                                max(0, min(plane["height"] - 1, (origin[1] + y) // sy - cy0)) * plane["width"]
                                + max(0, min(plane["width"] - 1, (origin[0] + x) // sx - cx0))
                            ] * 65535 // peak for y in range(32) for x in range(32)
                        ]
                        expected["image"]["components"].append({
                            "width": 32, "height": 32, "precision": 16, "signed": False,
                            "sample_kind": "unsigned", "samples": pixels,
                        })
                        if origin[0] % 2 == 0 and origin[1] % 2 == 0:
                            output = directory / f"upsampled-{channel}.pgx"
                            numerical._run_checked([decoder, "-i", str(directory / "encoded.j2k"),
                                                    "-o", str(output), "-c", str(channel), "-upsample"], 10.0)
                            reference = framework.parse_pgx(output.with_stem(output.stem + "_0"))
                            self.assertEqual([value * 65535 // peak
                                              for value in reference["image"]["components"][0]["samples"]], pixels)
                    for color in (16, 18):
                        full_by_discard = {}
                        for format_name, brand in (("jp2", b"jp2 "), ("jph", b"jph "), ("jpx", b"jpx ")):
                            path = directory / f"image-{color}.{format_name}"
                            path.write_bytes(framework.jp2_file(
                                declared_legacy_jph(raw) if format_name == "jph" else raw,
                                brand, width=32, height=32, components=3,
                                precision=precision, color_space=color,
                            ))
                            full = self.observe("convert", format_name, path)
                            if color == 16:
                                self.assert_image(expected, full)
                            if format_name == "jp2" and color == 18 and origin[0] % 2 == 0 and origin[1] % 2 == 0:
                                reference = directory / "reference-rgb.ppm"
                                numerical._run_checked([decoder, "-i", str(path), "-o", str(reference)], 10.0)
                                normalized = copy.deepcopy(full)
                                for component in normalized["image"]["components"]:
                                    component["precision"] = precision
                                    component["samples"] = [(value * peak + 32767) // 65535
                                                            for value in component["samples"]]
                                metrics = numerical.image_error_metrics(framework.parse_pnm(reference), normalized)
                                self.assertLessEqual(metrics["maximum_error"], 2 if precision == 8 else 12)
                            for discard in (0, 1, 2):
                                native = self.selected_color(path, format_name, discard, "all", None)
                                if discard == 0:
                                    self.assert_image(full, native)
                                if discard in full_by_discard:
                                    self.assert_image(full_by_discard[discard], native)
                                else:
                                    full_by_discard[discard] = native
                                for offset in (1, 2, 7):
                                    region = (origin[0] + offset, origin[1] + offset, 9, 11)
                                    self.assert_image(crop_converted(native, origin, region, discard), self.selected_color(
                                        path, format_name, discard, "all", region))
                            region = (origin[0] + 1, origin[1] + 1, 1, 1)
                            self.assert_image(crop_converted(full, origin, region),
                                              self.selected_color(path, format_name, 0, "all", region))
                            for reason, discard, tiles, region in (
                                ("EmptyRegion", 0, "all", (origin[0], origin[1], 0, 1)),
                                ("RegionOutOfBounds", 0, "all", (origin[0] + 32, origin[1], 1, 1)),
                                ("ReductionTooLarge", 255, "all", None),
                            ):
                                with self.assertRaisesRegex(ValueError, reason):
                                    self.selected_color(path, format_name, discard, tiles, region)
                            if tiled:
                                for tiles, region in (("4", (15, 15, 15, 15)),
                                                       ("3,4", (origin[0], 15, 30 - origin[0], 15))):
                                    self.assert_image(crop_converted(full, origin, region),
                                                      self.selected_color(path, format_name, 0, tiles, None))
                                with self.assertRaisesRegex(ValueError, "RegionOutsideSelectedTiles"):
                                    self.selected_color(path, format_name, 0, "4",
                                                        (origin[0], origin[1], 5, 5))
                                for tiles, reason in (("0,2", "NonRectangularTileSet"),
                                                       ("4,4", "DuplicateTile")):
                                    with self.assertRaisesRegex(ValueError, reason):
                                        self.selected_color(path, format_name, 0, tiles, None)

    @unittest.skipUnless(shutil.which("opj_compress") and shutil.which("opj_decompress"),
                         "independent OpenJPEG tools are not installed")
    def test_registered_nonuniform_color_regions_preserve_full_image_phase(self):
        with self.fixture("registered-nonuniform") as directory:
            raw, _, _ = self.encode_nonuniform(directory, ((1, 1), (2, 2), (2, 2)), 8, (0, 0))
            end = 4 + int.from_bytes(raw[4:6], "big")
            registration = framework.segment(0xff63, struct.pack(">6H", 0, 0, 32768, 32768, 32768, 32768))
            raw = raw[:end] + registration + raw[end:]
            for color in (16, 18):
                for format_name, brand in (("jp2", b"jp2 "), ("jph", b"jph "), ("jpx", b"jpx ")):
                    path = directory / f"image-{color}.{format_name}"
                    path.write_bytes(framework.jp2_file(
                        declared_legacy_jph(raw) if format_name == "jph" else raw, brand,
                        width=32, height=32, components=3, color_space=color,
                    ))
                    for discard in (0, 1):
                        full = self.selected_color(path, format_name, discard, "all", None)
                        for offset in (1, 2, 3, 4):
                            region = (offset, offset, 9, 11)
                            self.assert_image(crop_converted(full, (0, 0), region, discard),
                                              self.selected_color(path, format_name, discard, "all", region))

    def check_reconstruction(self, directory, raw, original):
        main, tile, body, _, _ = split_single_tile(raw)
        response = bytearray()
        for class_id, data in ((6, main), (2, tile), (4, body + b"\xff\xd9")):
            middle = len(data) // 2
            response += message(class_id, middle, data[middle:], True)
            response += message(class_id, 0, data[:middle], inherited=True)
            response += message(class_id, 0, data[:middle], inherited=True)
        response += b"\0\x01\0"
        path = directory / "fragments.jpip"
        path.write_bytes(response)
        process, _ = numerical._run_checked([str(self.subject), "reconstruct", "jpip", str(path)], 10.0)
        prefix, output = process.stdout.split(b"\n", 1)
        self.assertEqual(bytes.fromhex(prefix.decode().split("\t")[1]), raw)
        self.assert_image(original, framework.parse_subject_output(output, process.stderr, process.returncode))

    @unittest.skipUnless(shutil.which("opj_compress") and shutil.which("opj_decompress"),
                         "independent OpenJPEG tools are not installed")
    def test_nonzero_jpx_layers_composition_crops_and_persistent_frames(self):
        for channels, precision in ((1, 8), (3, 16)):
            with self.subTest(channels=channels, precision=precision), self.fixture(f"jpx-{channels}-{precision}") as directory:
                case = numerical.NumericalCase("first", width=17, height=13, channels=channels, precision=precision)
                first, original = self.encode(directory, case)
                peak = (1 << precision) - 1
                samples = tuple(peak - component["samples"][pixel]
                                for pixel in range(case.width * case.height)
                                for component in original["image"]["components"])
                second_case = numerical.NumericalCase("second", width=17, height=13, channels=channels,
                                                      precision=precision, pattern="fixed", samples=samples)
                second, other = self.encode(directory, second_case)
                layers = [converted(original), converted(other, chroma=True)]
                full = (0, 0, case.width, case.height)
                width, height = case.width + 4, case.height + 3
                path = directory / "static.jpx"
                path.write_bytes(jpx_file([first, second], case, chroma=True))
                for index, expected in enumerate(layers):
                    self.assert_image(expected, self.observe("render-layer", str(index), path))
                self.assert_image(canvas(width, height, [(layer, full, full) for layer in layers]),
                                  self.observe("render", "jpx", path))
                records = [
                    (0, 0, case.width, case.height, 0x80000000, 0, *full),
                    (3, 2, case.width, case.height, 100, 0, *full),
                ]
                path = directory / "placed.jpx"
                path.write_bytes(jpx_file([first, second], case, records, chroma=True))
                expected = canvas(width, height, [(layers[0], full, full),
                                                  (layers[1], (3, 2, case.width, case.height), full)])
                self.assert_image(expected, self.observe("render-frame", "0", path))
                self.assert_image(expected, self.observe("render", "jpx", path))
                crop = (1, 1, case.width - 2, case.height - 2)
                for persistent in (False, True):
                    records = [
                        (0, 0, case.width, case.height, 100 | (0x80000000 if persistent else 0), 0, *full),
                        (3, 2, crop[2], crop[3], 100, 0, *crop),
                    ]
                    path = directory / f"frames-{persistent}.jpx"
                    path.write_bytes(jpx_file([first, second], case, records, chroma=True))
                    first_frame = canvas(width, height, [(layers[0], full, full)])
                    placements = [(layers[0], full, full)] if persistent else []
                    placements.append((layers[1], (3, 2, crop[2], crop[3]), crop))
                    self.assert_image(first_frame, self.observe("render-frame", "0", path))
                    self.assert_image(canvas(width, height, placements), self.observe("render-frame", "1", path))
                    self.assert_image(first_frame, self.observe("render", "jpx", path))
                    with self.assertRaisesRegex(ValueError, r"ERR\tFrameIndexOutOfRange\t"):
                        self.observe("render-frame", "2", path)

    @unittest.skipUnless(shutil.which("opj_compress") and shutil.which("opj_decompress"),
                         "independent OpenJPEG tools are not installed")
    def test_nonzero_jpip_fragment_reconstruction_and_quality_prefixes(self):
        for precision in (8, 16):
            with self.subTest(precision=precision), self.fixture(f"jpip-{precision}") as directory:
                case = numerical.NumericalCase("quality", width=64, height=64, precision=precision, decomposition_levels=0)
                raw, original = self.encode(directory, case, options=("-r", "4,1", "-PLT"))
                main, tile, body, lengths, inline_tile = split_single_tile(raw)
                self.assertEqual(len(lengths), 2)
                self.assertEqual(sum(lengths), len(body))
                self.assertGreater(min(lengths), 2)
                self.check_reconstruction(directory, raw, original)


                first_packet, second_packet = body[:lengths[0]], body[lengths[0]:]
                a, b, c = len(main) // 2, len(first_packet) // 2, len(second_packet) // 2
                arrivals = [
                    message(6, a, main[a:], True), message(2, 0, tile, True),
                    message(6, 0, main[:a]), message(0, lengths[0] + c, second_packet[c:], True),
                    message(0, 0, first_packet[:b]), message(0, b, first_packet[b:]),
                    message(0, 0, first_packet), message(0, lengths[0], second_packet[:c]),
                    message(0, 0, first_packet),
                ]
                path = directory / "progressive.jpip"
                path.write_bytes(b"".join(arrivals))
                expected = {}
                encoded = directory / "quality.j2k"
                for layer in (1, 2):
                    reference = directory / f"prefix-{layer}.pgm"
                    numerical._run_checked(["opj_decompress", "-i", str(encoded), "-o", str(reference), "-l", str(layer)], 10.0)
                    expected[layer] = framework.parse_pnm(reference)
                self.assertNotEqual(framework.normalized_hash(expected[1]), framework.normalized_hash(expected[2]))
                self.assertGreater(len(set(expected[1]["image"]["components"][0]["samples"])), 1)
                for mode in ("auto", ",".join(map(str, lengths))):
                    if mode != "auto":
                        with self.assertRaisesRegex(ValueError, r"ERR\tMalformedInput\t"):
                            numerical._run_checked([str(self.subject), "progressive", mode, str(path)], 10.0)
                        manual_arrivals = list(arrivals)
                        manual_arrivals[1] = message(2, 0, inline_tile, True)
                        path = directory / "manual-progressive.jpip"
                        path.write_bytes(b"".join(manual_arrivals))
                    process, _ = numerical._run_checked([str(self.subject), "progressive", mode, str(path)], 10.0)
                    updates = process.stdout.decode().split("UPDATE\t")[1:]
                    self.assertEqual(len(updates), 1 + 2 * len(arrivals))
                    previous = 0
                    images = []
                    for update in updates:
                        header, _, observation = update.partition("\n")
                        status, old, available, present, missing = header.split("\t")
                        self.assertEqual(int(old), previous)
                        available = int(available)
                        self.assertGreaterEqual(available, previous)
                        if present == "1":
                            self.assertGreater(available, previous)
                            actual = framework.parse_subject_output(observation.encode(), b"", 0)
                            self.assert_image(expected[available], actual)
                            images.append(available)
                        else:
                            self.assertFalse(observation.strip())
                        self.assertEqual(status, "Complete" if available == 2 else
                                         "QualityAdvanced" if available > previous else "WaitingForData")
                        if available == 2:
                            self.assertEqual(int(missing), 0)
                        previous = available
                    self.assertEqual(images, [1, 2])

    @unittest.skipUnless(shutil.which(os.environ.get("OPENJPH_ENCODER", "ojph_compress"))
                         and shutil.which(os.environ.get("OPENJPH_DECODER", "ojph_expand")),
                         "independent OpenJPH tools are not installed")
    def test_nonzero_ht_jpip_fragment_reconstruction(self):
        for channels, precision in ((1, 8), (3, 16)):
            with self.subTest(channels=channels, precision=precision), self.fixture(f"ht-jpip-{channels}-{precision}") as directory:
                case = numerical.NumericalCase("ht", width=23, height=19, channels=channels, precision=precision)
                raw, original = self.encode(directory, case, high_throughput=True)
                self.check_reconstruction(directory, raw, original)


class ContainerFixtureTests(unittest.TestCase):
    def test_q2_planar_prequalification_rejects_reference_mismatch_and_wrong_headers(self):
        root = framework.RESULTS_DIR / "test-work"
        root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=root) as work:
            directory = Path(work)
            planes = container_relations.generated_planes(8, ((1, 1),))
            for wrong_header in (False, True):
                def fake_run(command, timeout):
                    output = Path(command[command.index("-o") + 1])
                    if command[0] == "encoder":
                        siz = struct.pack(">H8IH", 0, 32, 32, 0, 0, 16, 16, 0, 0, 1)
                        siz += bytes((7, 2 if wrong_header else 1, 1))
                        cod = bytes((0, 0, 0, 1, 0, 2, 4, 4, 0, 1))
                        output.write_bytes(b"\xff\x4f" + framework.segment(0xff51, siz) +
                                           framework.segment(0xff52, cod) + b"\xff\x90")
                    else:
                        discard = int(command[command.index("-r") + 1])
                        size = 32 >> discard
                        output.with_stem(output.stem + "_0").write_bytes(
                            f"PG ML + 8 {size} {size}\n".encode() + bytes(size * size))
                    return subprocess.CompletedProcess(command, 0, b"", b""), 0.0
                with self.subTest(wrong_header=wrong_header), mock.patch.object(numerical, "_run_checked", fake_run):
                    with self.assertRaisesRegex(ValueError, "SIZ/COD|original/reference"):
                        container_relations.planar_reference(
                            directory, planes, ((1, 1),), (0, 0), "encoder", "decoder", [])

    def test_q2_rational_color_normalization_and_gray_alpha_source_over(self):
        for precision in (1, 8, 12, 16, 31):
            peak = (1 << precision) - 1
            self.assertEqual(container_relations.normalize(0, precision), 0)
            self.assertEqual(container_relations.normalize(peak, precision), 65535)
            self.assertEqual(container_relations.normalize(-(1 << (precision - 1)), precision, True), 0)
        for arguments in ((-1, 8), (256, 8), (True, 8), (0, 0), (0, 8, 1)):
            with self.assertRaises(ValueError):
                container_relations.normalize(*arguments)
        back = image(1, 1, [[10000], [32768]])
        front = image(1, 1, [[50000], [32768]])
        full = (0, 0, 1, 1)
        actual = container_relations.source_over(1, 1, [
            (back, False, full, full), (front, False, full, full),
        ])
        self.assertEqual([component["samples"] for component in actual["image"]["components"]],
                         [[36667], [49152]])
        invalid = image(1, 1, [[65535], [1]])
        with self.assertRaisesRegex(ValueError, "exceeds alpha"):
            container_relations.source_over(1, 1, [(invalid, True, full, full)])
        with self.assertRaises(ValueError):
            container_relations.source_over(1, 1, [])
        with self.assertRaises(ValueError):
            container_relations.source_over(1, 1, [(back, False, full, (1, 0, 1, 1))])
        planes = [{"width": 1, "height": 1, "precision": 8, "signed": False,
                   "sample_kind": "unsigned", "samples": [value]} for value in (128, 128, 128)]
        neutral = {"accepted": True, "image": {"width": 1, "height": 1, "components": planes}}
        expected = container_relations.convert(neutral, (0, 0, 1, 1), ((1, 1),) * 3, 18)
        self.assertEqual([component["samples"] for component in expected["image"]["components"]], [[32896]] * 3)
        for space, sampling in ((99, ((1, 1),) * 3), (17, ((1, 1),) * 3), (16, ((1, 1),))):
            with self.assertRaises(ValueError):
                container_relations.convert(neutral, (0, 0, 1, 1), sampling, space)
        incomplete = copy.deepcopy(neutral)
        incomplete["image"]["components"][0]["samples"].clear()
        with self.assertRaises(ValueError):
            container_relations.convert(incomplete, (0, 0, 1, 1), ((1, 1),) * 3, 16)
    def test_fractional_oracle_matches_known_straight_and_premultiplied_results(self):
        full = (0, 0, 1, 1)
        black = image(1, 1, [[0], [0], [0], [65535]])
        half_white = image(1, 1, [[65535], [65535], [65535], [32768]])
        self.assertEqual(fractional_canvas(1, 1, [(black, False, full, full),
                                                 (half_white, False, full, full)]),
                         image(1, 1, [[32768], [32768], [32768], [65535]]))
        premultiplied = image(1, 1, [[32768], [32768], [32768], [32768]])
        self.assertEqual(fractional_canvas(1, 1, [(premultiplied, True, full, full)]), half_white)
        transparent = image(1, 1, [[123], [456], [789], [0]])
        self.assertEqual(fractional_canvas(1, 1, [(transparent, False, full, full)]),
                         image(1, 1, [[0], [0], [0], [0]]))
        with self.assertRaisesRegex(ValueError, "exceeds alpha"):
            fractional_canvas(1, 1, [(transparent, True, full, full)])

    def test_fractional_oracle_equivalent_representations_match_at_alpha_boundaries(self):
        full = (0, 0, 1, 1)
        background = image(1, 1, [[1234], [4567], [8901], [32123]])
        for alpha in (0, 1, 2, 32767, 32768, 65534, 65535):
            straight = image(1, 1, [[0], [65535], [65535], [alpha]])
            premultiplied = image(1, 1, [[0], [alpha], [alpha], [alpha]])
            self.assertEqual(
                fractional_canvas(1, 1, [(background, False, full, full), (straight, False, full, full)]),
                fractional_canvas(1, 1, [(background, False, full, full), (premultiplied, True, full, full)]))

    def test_alpha_fixture_metadata_rejects_invalid_orders_and_layer_counts(self):
        case = numerical.NumericalCase("invalid", channels=4)
        with self.assertRaises(ValueError):
            rgba_header(case, False, (0, 1, 2, 2))
        with self.assertRaises(ValueError):
            jpx_file([b""], case, headers=[])
        with self.assertRaises(ValueError):
            jpx_file([b""], case, opacities=[])
        with self.assertRaises(ValueError):
            jpx_file([b""], case, opacities=[2])

    def test_declared_legacy_wrapper_preserves_coding_bytes_and_validates_siz(self):
        raw = framework.raw_codestream()
        declared = declared_legacy_jph(raw)
        end = 4 + int.from_bytes(raw[4:6], "big")
        self.assertEqual(declared[6:8], b"\x40\x00")
        self.assertEqual(declared[end:end + 10],
                         framework.segment(0xff50, struct.pack(">IH", 0x00020000, 0x801f)))
        self.assertEqual(declared[end + 10:], raw[end:])
        for invalid in (b"", raw[:44], declared, raw[:4] + b"\xff\xff" + raw[6:]):
            with self.subTest(data=invalid), self.assertRaises(ValueError):
                declared_legacy_jph(invalid)

    def test_converted_crop_uses_native_bounds_without_resampling(self):
        original = image(4, 4, [list(range(16))])
        self.assertEqual(crop_converted(original, (0, 0), (1, 1, 2, 2)),
                         image(2, 2, [[5, 6, 9, 10]]))
        reduced = image(2, 2, [[10, 20, 30, 40]])
        self.assertEqual(crop_converted(reduced, (0, 0), (1, 1, 2, 2), 1),
                         image(1, 1, [[40]]))
        for region in ((0, 0, 0, 1), (3, 3, 2, 2), (-1, 0, 1, 1)):
            with self.subTest(region=region), self.assertRaises(ValueError):
                crop_converted(original, (0, 0), region)

    def test_vbas_and_single_tile_fixture_boundaries(self):
        for value, expected in ((0, b"\0"), (127, b"\x7f"), (128, b"\x81\0"), (16384, b"\x81\x80\0")):
            self.assertEqual(vbas(value), expected)
        with self.assertRaises(ValueError):
            vbas(-1)
        raw = framework.raw_codestream()
        main, tile, body, lengths, inline = split_single_tile(raw)
        self.assertEqual(main + tile + body + b"\xff\xd9", raw)
        self.assertEqual(inline, tile)
        self.assertEqual(lengths, [])
        for invalid in (b"", raw[:-1], raw[:4] + b"\0\1" + raw[6:]):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                split_single_tile(invalid)

    def test_fixture_compositor_does_not_silently_accept_fractional_alpha(self):
        with self.assertRaisesRegex(ValueError, "binary alpha"):
            canvas(1, 1, [(image(1, 1, [[123], [32768]]), (0, 0, 1, 1), (0, 0, 1, 1))])

    def test_distinct_disagreements_retain_distinct_hash_bound_files(self):
        root = framework.RESULTS_DIR / "test-work"
        root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=root) as temporary:
            directory = Path(temporary)
            helper = ContainerRelationsTests("test_nonzero_ht_jpip_fragment_reconstruction")
            helper.subject = Path(sys.executable)
            with mock.patch.object(framework, "RESULTS_DIR", directory / "reports"):
                for version in (1, 2):
                    helper.work = directory / str(version)
                    helper.work.mkdir()
                    with self.assertRaisesRegex(ValueError, "relation failure"):
                        with helper.fixture("probe") as work:
                            (work / "input.bin").write_bytes(bytes([version]))
                            raise ValueError("relation failure")
            records = list((directory / "reports" / "container-failures" / "probe").glob("*/reproduction.json"))
            self.assertEqual(len(records), 2)
            for path in records:
                record = json.loads(path.read_text())
                self.assertEqual(record["error"], "relation failure")
                self.assertEqual(record["files"]["input.bin"], framework.sha256_file(path.parent / "input.bin"))
                self.assertEqual(record["subject_sha256"], framework.sha256_file(Path(sys.executable)))
