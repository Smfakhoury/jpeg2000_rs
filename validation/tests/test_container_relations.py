import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
from contextlib import contextmanager
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import framework
import numerical


def image(width, height, planes, precision=16):
    return {"accepted": True, "image": {
        "width": width, "height": height,
        "components": [{"width": width, "height": height, "precision": precision,
                        "signed": False, "sample_kind": "unsigned", "samples": samples}
                       for samples in planes],
    }}


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


def jpx_file(codestreams, case, instructions=None, chroma=False):
    box = framework.box
    header = box(b"ihdr", struct.pack(">IIH4B", case.height, case.width, case.channels,
                                     case.precision - 1, 7, 0, 0))
    header += box(b"colr", bytes([1, 0, 0, 0, 0, 0, 17 if case.channels == 1 else 16]))
    result = box(b"jP  ", b"\x0d\x0a\x87\x0a") + box(b"ftyp", b"jpxb\0\0\0\0jpxb")
    result += box(b"jp2h", header)
    result += b"".join(box(b"jpch", header) for _ in codestreams)
    for index in range(len(codestreams)):
        layer = box(b"creg", struct.pack(">HHH4B", 1, 1, index, 1, 1, 0, 0))
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
            files = {path.name: framework.sha256_file(path) for path in directory.iterdir() if path.is_file()}
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
