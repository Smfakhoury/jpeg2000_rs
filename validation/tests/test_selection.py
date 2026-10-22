import copy
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import framework
import numerical
import selection


def native_rectangle(rectangle, discard):
    scale = 1 << discard
    return tuple((value + scale - 1) // scale for value in rectangle)


crop_image = framework.crop_image


class SelectionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.subject = framework.build_subject()

    def setUp(self):
        root = framework.RESULTS_DIR / "test-work"
        root.mkdir(parents=True, exist_ok=True)
        self.directory = tempfile.TemporaryDirectory(dir=root)
        self.addCleanup(self.directory.cleanup)
        self.work = Path(self.directory.name)

    def selected(self, encoded, format_name, discard, tile, region):
        process, _ = numerical._run_checked([
            str(self.subject), "select", format_name, str(encoded), str(discard),
            "all" if tile is None else str(tile),
            "all" if region is None else ",".join(map(str, region)),
        ], 10.0)
        frame, output = process.stdout.split(b"\n", 1)
        fields = frame.split(b"\t")
        self.assertEqual(fields[0], b"FRAME")
        self.assertEqual(len(fields), 3)
        origin = (int(fields[1]), int(fields[2]))
        return framework.parse_subject_output(output, process.stderr, process.returncode), origin

    def check_case(self, case, high_throughput, origin, tiled, sampling=(1, 1)):
        encoder = os.environ.get("OPENJPH_ENCODER", "ojph_compress") if high_throughput else "opj_compress"
        decoder = os.environ.get("OPENJPH_DECODER", "ojph_expand") if high_throughput else "opj_decompress"
        encoder = shutil.which(encoder)
        decoder = shutil.which(decoder)
        if encoder is None or decoder is None:
            raise FileNotFoundError("selection reference encoder/decoder is unavailable")
        tools = {
            role: {"path": str(path), "sha256": framework.sha256_file(Path(path))}
            for role, path in (("encoder", encoder), ("decoder", decoder), ("subject", self.subject))
        }
        directory = self.work / case.name
        directory.mkdir()
        extension = "ppm" if case.channels == 3 else "pgm"
        source = directory / f"source.{extension}"
        raw = directory / "encoded.j2k"
        source.write_bytes(numerical.source_image(case))
        frame = (*origin, origin[0] + (case.width - 1) * sampling[0] + 1,
                 origin[1] + (case.height - 1) * sampling[1] + 1)
        region = (origin[0] + 1, origin[1] + 1, frame[2] - frame[0] - 2, frame[3] - frame[1] - 2)
        if high_throughput:
            options = ["-num_decomps", str(case.decomposition_levels),
                       "-reversible", str(case.lossless).lower(),
                       "-image_offset", "{" + ",".join(map(str, origin)) + "}",
                       "-tile_offset", "{0,0}"]
            if tiled:
                options += ["-tile_size", "{16,16}"]
        else:
            options = ["-n", str(case.decomposition_levels + 1), "-d", ",".join(map(str, origin))]
            options += ["-s", ",".join(map(str, sampling))]
            if tiled:
                options += ["-t", "16,16"]
            if not case.lossless:
                options += ["-I"]
        context = {"stage": "encoding"}
        try:
            numerical._run_checked([encoder, "-i", str(source), "-o", str(raw), *options, *case.options], 10.0)
            numerical.validate_required_headers(case, raw.read_bytes())
            encoded = raw
            if case.format != "raw":
                encoded = directory / f"encoded.{case.format}"
                encoded.write_bytes(framework.jp2_file(
                    raw.read_bytes(), {"jp2": b"jp2 ", "jph": b"jph ", "jpx": b"jpx "}[case.format],
                    width=frame[2] - frame[0], height=frame[3] - frame[1],
                    components=case.channels, precision=case.precision,
                ))
            context = {"stage": "reference-full"}
            full_reference = directory / f"reference-0.{extension}"
            numerical._run_checked([decoder, "-i", str(raw), "-o", str(full_reference)], 10.0)
            original_reference = framework.parse_pnm(full_reference)
            if case.lossless:
                self.assertEqual(
                    framework.normalized_hash(original_reference),
                    framework.normalized_hash(framework.parse_pnm(source)),
                    "independent lossless encoder/decoder changed original samples",
                )
            context = {"stage": "ordinary-full"}
            process, _ = numerical._run_checked([str(self.subject), "decode", case.format, str(encoded)], 10.0)
            ordinary_full = framework.parse_subject_output(process.stdout, process.stderr, process.returncode)
            if tiled:
                for tiles, invalid_region, reason in (
                    (1, region, "RegionOutsideSelectedTiles"),
                    ("0,2", None, "NonRectangularTileSet"),
                ):
                    with self.assertRaisesRegex(ValueError, rf"ERR\tInvalidSelection\t.*{reason}"):
                        self.selected(encoded, case.format, 0, tiles, invalid_region)
            for discard in range(case.decomposition_levels + 1):
                context = {"stage": "native-full", "discard_levels": discard}
                if discard == 0:
                    expected = copy.deepcopy(original_reference)
                else:
                    reference = directory / f"reference-{discard}.{extension}"
                    reduction = ["-skip_res" if high_throughput else "-r", str(discard)]
                    numerical._run_checked([decoder, "-i", str(raw), "-o", str(reference), *reduction], 10.0)
                    expected = framework.parse_pnm(reference)
                native_frame = native_rectangle(frame, discard)
                expected["image"]["width"] = native_frame[2] - native_frame[0]
                expected["image"]["height"] = native_frame[3] - native_frame[1]
                full, full_origin = self.selected(encoded, case.format, discard, None, None)
                self.assertEqual(full_origin, native_frame[:2])
                metrics = numerical.image_error_metrics(expected, full)
                self.assertLessEqual(metrics["maximum_error"], case.maximum_error)
                self.assertLessEqual(metrics["worst_component_rmse"], case.maximum_rmse)
                if discard == 0:
                    self.assertEqual(numerical.image_error_metrics(ordinary_full, full)["maximum_error"], 0)
                if discard == 1 and origin == (0, 0):
                    component = ordinary_full["image"]["components"][0]
                    decimated = [
                        component["samples"][y * component["width"] + x]
                        for y in range(0, component["height"], 2)
                        for x in range(0, component["width"], 2)
                    ]
                    self.assertNotEqual(full["image"]["components"][0]["samples"], decimated)
                requests = [(None, region, (region[0], region[1],
                                           region[0] + region[2], region[1] + region[3]))]
                if tiled:
                    bounds = (max(frame[0], 16), frame[1], min(frame[2], 32), min(frame[3], 16))
                    tile_region = (bounds[0] + 1, bounds[1] + 1,
                                   bounds[2] - bounds[0] - 2, bounds[3] - bounds[1] - 2)
                    requests += [
                        (1, None, bounds),
                        (1, tile_region, (bounds[0] + 1, bounds[1] + 1, bounds[2] - 1, bounds[3] - 1)),
                        ("0,1", None, (frame[0], frame[1], min(frame[2], 32), min(frame[3], 16))),
                    ]
                for tile, selected_region, rectangle in requests:
                    context = {"stage": "selection", "discard_levels": discard,
                               "tiles": tile, "region": selected_region}
                    rectangle = native_rectangle(rectangle, discard)
                    selected, selected_origin = self.selected(encoded, case.format, discard, tile, selected_region)
                    self.assertEqual(selected_origin, rectangle[:2])
                    cropped_full = crop_image(full, native_frame, rectangle, sampling)
                    self.assertEqual(numerical.image_error_metrics(cropped_full, selected)["maximum_error"], 0)
                    reference_crop = crop_image(expected, native_frame, rectangle, sampling)
                    metrics = numerical.image_error_metrics(reference_crop, selected)
                    self.assertLessEqual(metrics["maximum_error"], case.maximum_error)
                    self.assertLessEqual(metrics["worst_component_rmse"], case.maximum_rmse)
        except (AssertionError, OSError, ValueError, subprocess.SubprocessError) as error:
            destination = framework.RESULTS_DIR / "selection-failures" / case.name
            shutil.copytree(directory, destination, dirs_exist_ok=True)
            framework.write_json(destination / "reproduction.json", {
                "case": case.name, "format": case.format, "origin": origin, "sampling": sampling,
                "encoder_options": options, "request": context, "error": str(error), "tools": tools,
                "provenance": framework.source_binding(),
                "test_sha256": framework.sha256_file(Path(__file__)),
                "files": {path.name: framework.sha256_file(path) for path in directory.iterdir() if path.is_file()},
            })
            raise

    @unittest.skipUnless(shutil.which("opj_compress") and shutil.which("opj_decompress"),
                         "independent OpenJPEG tools are not installed")
    def test_legacy_regions_tiles_and_native_resolutions(self):
        for name, format_name, precision, channels, lossless in (
            ("legacy-gray", "raw", 8, 1, True),
            ("legacy-gray16", "raw", 16, 1, True),
            ("legacy-rgb-jp2", "jp2", 8, 3, True),
            ("legacy-rgb-jpx", "jpx", 8, 3, True),
            ("legacy-irreversible", "raw", 8, 1, False),
        ):
            with self.subTest(case=name):
                self.check_case(numerical.NumericalCase(
                    name, width=47, height=35, format=format_name, precision=precision,
                    channels=channels, lossless=lossless,
                    maximum_error=0 if lossless else 1, maximum_rmse=0 if lossless else 0.25,
                ), False, (3, 5), True)

    @unittest.skipUnless(shutil.which(os.environ.get("OPENJPH_ENCODER", "ojph_compress"))
                         and shutil.which(os.environ.get("OPENJPH_DECODER", "ojph_expand")),
                         "independent OpenJPH tools are not installed")
    def test_ht_regions_tiles_and_native_resolutions(self):
        for name, format_name, precision, channels, lossless, origin, tiled in (
            ("ht-gray", "raw", 8, 1, True, (0, 0), True),
            ("ht-origin-jph", "jph", 8, 1, True, (3, 5), False),
            ("ht-rgb16-jph", "jph", 16, 3, True, (0, 0), True),
            ("ht-irreversible", "raw", 8, 1, False, (0, 0), True),
        ):
            with self.subTest(case=name):
                self.check_case(numerical.NumericalCase(
                    name, width=47, height=35, format=format_name, precision=precision,
                    channels=channels, lossless=lossless,
                    maximum_error=0 if lossless else 1, maximum_rmse=0 if lossless else 0.25,
                ), True, origin, tiled)

    @unittest.skipUnless(shutil.which(os.environ.get("OPENJPH_ENCODER", "ojph_compress"))
                         and shutil.which(os.environ.get("OPENJPH_DECODER", "ojph_expand")),
                         "independent OpenJPH tools are not installed")
    def test_ht_rgb16_transform_modes_at_nonzero_tiled_origins(self):
        for transform in ("true", "false"):
            with self.subTest(transform=transform):
                self.check_case(numerical.NumericalCase(
                    f"ht-rgb16-origin-{transform}", width=47, height=35, format="jph",
                    precision=16, channels=3, options=("-colour_trans", transform),
                ), True, (3, 5), True)

    @unittest.skipUnless(shutil.which("opj_compress") and shutil.which("opj_decompress"),
                         "independent OpenJPEG tools are not installed")
    def test_subsampled_regions_tiles_and_native_resolutions(self):
        for channels, format_name in ((1, "raw"), (3, "jp2")):
            with self.subTest(channels=channels, format=format_name):
                self.check_case(numerical.NumericalCase(
                    f"subsampled-{channels}", width=47, height=35,
                    format=format_name, channels=channels,
                ), False, (4, 6), True, sampling=(2, 3))

    @unittest.skipUnless(shutil.which("opj_compress") and shutil.which("opj_decompress"),
                         "independent OpenJPEG tools are not installed")
    def test_component_maxshift_regions_tiles_and_native_resolutions(self):
        for format_name, precision, channels, component, shift in (
            ("raw", 8, 1, 0, 3), ("jp2", 16, 3, 1, 1), ("jpx", 8, 3, 2, 5),
        ):
            with self.subTest(format=format_name, precision=precision, component=component):
                self.check_case(numerical.NumericalCase(
                    f"maxshift-{format_name}", width=47, height=35, format=format_name,
                    precision=precision, channels=channels,
                    options=("-ROI", f"c={component},U={shift}"),
                ), False, (3, 5), True)

    @unittest.skipUnless(shutil.which("opj_compress") and shutil.which("opj_decompress"),
                         "independent OpenJPEG tools are not installed")
    def test_q2_prequalified_maxshift_native_grid_expectations(self):
        with self.subTest(scope="independent native planes before selections"):
            directory = framework.RESULTS_DIR / "q2-selection"
            directory.mkdir(parents=True, exist_ok=True)
            cases = [case for case in numerical.CASES if case.name.startswith("q2-")]
            declarations = [{"id": case.name, "options": case.options,
                             "maximum_error": case.maximum_error, "maximum_rmse": case.maximum_rmse}
                            for case in cases]
            framework.write_json(directory / "matrix.json", declarations)
            matrix_hash = framework.sha256_file(directory / "matrix.json")
            encoder, decoder = shutil.which("opj_compress"), shutil.which("opj_decompress")
            commands, prepared = [], []
            for case in cases:
                inputs = directory / case.name
                inputs.mkdir(exist_ok=True)
                encoded, frame, references = selection.prepare_reference(case, inputs, encoder, decoder, commands)
                prepared.append((case, encoded, frame, references))
            framework.write_json(directory / "references.json", {
                "matrix_sha256": matrix_hash, "provenance": framework.source_binding(), "commands": commands,
                "reference_source_recipes": numerical.REFERENCE_SOURCE_RECIPES,
                "tools": {role: {"path": path, "sha256": framework.sha256_file(Path(path))}
                          for role, path in (("encoder", encoder), ("decoder", decoder))},
                "references": {case.name: references for case, _, _, references in prepared},
                "files": {path.relative_to(directory).as_posix(): framework.sha256_file(path)
                          for path in directory.rglob("*")
                          if path.is_file() and path.name not in ("references.json", "results.json")},
            })
            reference_hash = framework.sha256_file(directory / "references.json")
            results = []
            for case, encoded, frame, references in prepared:
                for discard, reference in references.items():
                    native = selection.native_rectangle(frame, discard)
                    for request in selection.requests(frame, discard):
                        with self.subTest(case=case.name, discard=discard, request=request):
                            expected = selection.expected_crop(reference, native, request["bounds"])
                            actual, origin = selection.observe(self.subject, "select", "raw", encoded, discard, request)
                            self.assertEqual(origin, tuple(request["bounds"][:2]))
                            metrics = numerical.image_error_metrics(expected, actual)
                            self.assertLessEqual(metrics["maximum_error"], case.maximum_error)
                            self.assertLessEqual(metrics["worst_component_rmse"], case.maximum_rmse)
                            results.append({"case": case.name, "discard": discard, "request": request, "metrics": metrics})
            framework.write_json(directory / "results.json", {
                "reference_sha256": reference_hash, "provenance": framework.source_binding(),
                "subject_sha256": framework.sha256_file(self.subject), "records": results,
                "summary": {"expected": 45, "observed": len(results),
                            "status": "passed" if len(results) == 45 else "failed"},
            })


class SelectionGeometryTests(unittest.TestCase):
    def test_q2_expected_crop_is_independent_and_native_bounds_are_absolute(self):
        original = {"accepted": True, "image": {"width": 3, "height": 2, "components": [{
            "width": 3, "height": 2, "precision": 8, "signed": False,
            "sample_kind": "unsigned", "samples": [1, 2, 3, 4, 5, 6],
        }]}}
        cropped = selection.expected_crop(original, (2, 3, 5, 5), (3, 3, 5, 5))
        self.assertEqual(cropped["image"]["components"][0]["samples"], [2, 3, 5, 6])
        self.assertEqual(selection.native_rectangle((3, 5, 50, 40), 2), (1, 2, 13, 10))
        cropped["image"]["components"][0]["samples"][0] = 99
        self.assertEqual(original["image"]["components"][0]["samples"][1], 2)
        for invalid in (-1, 33, True):
            with self.assertRaises(ValueError):
                selection.native_rectangle((0, 0, 32, 32), invalid)
    def test_failed_reference_retains_bound_reproduction_files(self):
        root = framework.RESULTS_DIR / "test-work"
        root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=root) as directory:
            work = Path(directory)
            helper = SelectionTests("test_legacy_regions_tiles_and_native_resolutions")
            helper.subject = Path(sys.executable)
            helper.work = work / "inputs"
            helper.work.mkdir()
            with mock.patch.object(framework, "RESULTS_DIR", work / "reports"), \
                    mock.patch.object(shutil, "which", return_value=sys.executable), \
                    mock.patch.object(numerical, "_run_checked", side_effect=ValueError("reference failure")):
                with self.assertRaisesRegex(ValueError, "reference failure"):
                    helper.check_case(numerical.NumericalCase("retention-probe"), False, (0, 0), False)
            retained = work / "reports" / "selection-failures" / "retention-probe"
            record = json.loads((retained / "reproduction.json").read_text())
            self.assertEqual(record["request"], {"stage": "encoding"})
            self.assertEqual(record["error"], "reference failure")
            self.assertEqual(record["files"]["source.pgm"], framework.sha256_file(retained / "source.pgm"))
            self.assertEqual(record["tools"]["subject"]["sha256"], framework.sha256_file(Path(sys.executable)))
            self.assertEqual(record["test_sha256"], framework.sha256_file(Path(__file__)))

    def test_subsampled_crop_uses_absolute_grid_coordinates(self):
        observation = {
            "accepted": True,
            "image": {
                "width": 11, "height": 10,
                "components": [{
                    "width": 6, "height": 4, "precision": 8, "signed": False,
                    "sample_kind": "unsigned", "samples": list(range(24)),
                }],
            },
        }
        cropped = crop_image(observation, (4, 6, 15, 16), (7, 8, 13, 14), (2, 3))
        self.assertEqual((cropped["image"]["width"], cropped["image"]["height"]), (6, 6))
        component = cropped["image"]["components"][0]
        self.assertEqual((component["width"], component["height"]), (3, 2))
        self.assertEqual(component["samples"], [8, 9, 10, 14, 15, 16])
        self.assertEqual(len(observation["image"]["components"][0]["samples"]), 24)
        self.assertEqual(native_rectangle((3, 5, 50, 40), 1), (2, 3, 25, 20))
