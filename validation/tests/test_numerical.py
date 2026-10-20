import copy
import json
import math
import os
import shutil
import subprocess
import sys
import struct
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import framework
import numerical


def image(samples, precision=8, signed=False):
    return {
        "accepted": True,
        "image": {
            "width": len(samples), "height": 1,
            "components": [{
                "width": len(samples), "height": 1, "precision": precision,
                "signed": signed, "sample_kind": "signed" if signed else "unsigned",
                "samples": samples,
            }],
        },
    }


class NumericalTests(unittest.TestCase):
    def setUp(self):
        root = framework.RESULTS_DIR / "test-work"
        root.mkdir(parents=True, exist_ok=True)
        self.directory = tempfile.TemporaryDirectory(dir=root)
        self.addCleanup(self.directory.cleanup)
        self.work = Path(self.directory.name)

    def test_metrics_measure_error_and_exact_output_without_nonfinite_json(self):
        metrics = numerical.image_error_metrics(image([0, 10]), image([2, 6]))
        self.assertEqual(metrics["maximum_error"], 4)
        self.assertAlmostEqual(metrics["rmse"], math.sqrt(10))
        self.assertAlmostEqual(metrics["components"][0]["mse"], 10)
        self.assertAlmostEqual(metrics["components"][0]["psnr_db"], 10 * math.log10(255**2 / 10))
        exact = numerical.image_error_metrics(image([0, 65535], 16), image([0, 65535], 16))
        self.assertEqual(exact["maximum_error"], 0)
        self.assertIsNone(exact["components"][0]["psnr_db"])
        json.dumps(exact, allow_nan=False)
        signed = numerical.image_error_metrics(image([-128, 127], signed=True),
                                               image([-127, 126], signed=True))
        self.assertEqual(signed["maximum_error"], 1)

    def test_metrics_reject_missing_samples_metadata_and_out_of_range_values(self):
        expected = image([0, 255])
        for invalid in (
            {"accepted": False},
            image([0]),
            image([0, 256]),
            image([0, -1]),
            image([0, 1.5]),
            image([0, True]),
        ):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                numerical.image_error_metrics(expected, invalid)
        incomplete = copy.deepcopy(expected)
        incomplete["image"]["components"][0]["samples"].pop()
        with self.assertRaisesRegex(ValueError, "sample count"):
            numerical.image_error_metrics(expected, incomplete)
        missing_component = copy.deepcopy(expected)
        missing_component["image"]["components"].clear()
        with self.assertRaisesRegex(ValueError, "metadata"):
            numerical.image_error_metrics(expected, missing_component)

    def test_generated_sources_are_reproducible_nonzero_and_cover_sample_ranges(self):
        for case in numerical.CASES:
            with self.subTest(case=case.name):
                data = numerical.source_image(case)
                self.assertEqual(data, numerical.source_image(case))
                path = self.work / "source.pnm"
                path.write_bytes(data)
                parsed = framework.parse_pnm(path)
                self.assertEqual(len(parsed["image"]["components"]), case.channels)
                for component in parsed["image"]["components"]:
                    self.assertEqual(component["precision"], case.precision)
                    self.assertEqual(len(component["samples"]), case.width * case.height)
                    self.assertEqual(min(component["samples"]), 0)
                    self.assertEqual(max(component["samples"]), (1 << case.precision) - 1)

    def test_missing_tools_are_blocked_and_configured_missing_tool_is_an_error(self):
        with mock.patch.object(numerical.shutil, "which", return_value=None), mock.patch.dict(
            numerical.os.environ, {"OPENJPEG_ENCODER": "", "OPENJPEG_DECODER": ""}
        ):
            result = numerical.run_numerical(output_dir=self.work)
            self.assertEqual(result["status"], "blocked")
            self.assertEqual(result["summary"]["passed"], 0)
            self.assertEqual(len(result["blockers"]), 2)
            with self.assertRaises(FileNotFoundError):
                numerical.run_numerical(output_dir=self.work, encoder=Path("missing-encoder"))
        framework.validate_evidence_schemas(self.work)

    def test_native_failures_and_timeouts_are_recorded_not_counted_as_success(self):
        for failure in (ValueError("native failure"), subprocess.TimeoutExpired("encoder", 0.01)):
            with mock.patch.object(numerical.shutil, "which", return_value=sys.executable), \
                    mock.patch.object(numerical, "_run_checked", side_effect=failure):
                result = numerical.run_numerical(Path(sys.executable), self.work)
            self.assertEqual(result["status"], "failed")
            self.assertEqual(result["summary"]["failed"], len(numerical.CASES))
            self.assertTrue(all(record["error"] for record in result["records"]))
            self.assertTrue(all(record["artifacts"] for record in result["records"]))
            for record in result["records"]:
                for artifact in record["artifacts"]:
                    self.assertTrue((self.work / artifact).is_file())
            framework.validate_evidence_schemas(self.work)

    def test_native_process_output_cap_and_exit_failure_are_explicit(self):
        for status, limited in ((1, False), (0, True)):
            process = subprocess.CompletedProcess(["encoder"], status, b"", b"failure")
            with mock.patch.object(framework, "_run_process_bounded", return_value=(process, 0.0, limited)):
                with self.assertRaisesRegex(ValueError, "failed"):
                    numerical._run_checked(["encoder"], 1.0)

    def test_empty_or_unmeasured_success_report_fails_schema(self):
        with mock.patch.object(numerical.shutil, "which", return_value=None), mock.patch.dict(
            numerical.os.environ, {"OPENJPEG_ENCODER": "", "OPENJPEG_DECODER": ""}
        ):
            result = numerical.run_numerical(output_dir=self.work)
        result["status"] = "passed"
        with self.assertRaises(ValueError):
            framework.validate_schema(framework.VALIDATION / "numerical-result.schema.json", result)

    def test_invalid_timeouts_fail_before_generating_a_report(self):
        for timeout in (0, -1, math.inf, math.nan):
            with self.subTest(timeout=timeout), self.assertRaises(ValueError):
                numerical.run_numerical(output_dir=self.work, timeout_seconds=timeout)
        self.assertFalse((self.work / "numerical-results.json").exists())

    def test_ht_sources_and_container_metadata_cover_real_component_layouts(self):
        self.assertEqual({case.format for case in numerical.HT_CASES}, {"raw", "jph"})
        self.assertEqual({case.precision for case in numerical.HT_CASES}, {8, 16})
        self.assertEqual({case.channels for case in numerical.HT_CASES}, {1, 3})
        self.assertEqual({case.pattern for case in numerical.HT_CASES}, {"varied", "sparse", "fixed"})
        for case in numerical.HT_CASES:
            data = numerical.source_image(case)
            self.assertEqual(data, numerical.source_image(case))
            path = self.work / "ht-source.pnm"
            path.write_bytes(data)
            image = framework.parse_pnm(path)["image"]
            self.assertEqual((image["width"], image["height"]), (case.width, case.height))
            self.assertEqual(len(image["components"]), case.channels)
        container = framework.jp2_file(b"generated fixture", b"jph ", width=7, height=5,
                                       components=3, precision=16)
        offset = container.index(b"ihdr") + 4
        self.assertEqual(struct.unpack(">IIH4B", container[offset:offset + 14]),
                         (5, 7, 3, 15, 7, 0, 0))
        with self.assertRaises(ValueError):
            framework.jp2_file(b"", width=0)
        with self.assertRaises(ValueError):
            numerical.source_image(numerical.NumericalCase("invalid", pattern="unknown"))
        with self.assertRaises(ValueError):
            numerical.source_image(numerical.NumericalCase("invalid", pattern="fixed"))
        with self.assertRaises(ValueError):
            numerical.source_image(numerical.NumericalCase("invalid", pattern="fixed", samples=(256,), width=1, height=1))

    def test_missing_ht_reference_is_blocked_without_rewriting_legacy_results(self):
        with mock.patch.object(numerical.shutil, "which", return_value=None), mock.patch.dict(
            os.environ, {"OPENJPH_ENCODER": "", "OPENJPH_DECODER": ""}
        ):
            result = numerical.run_numerical(output_dir=self.work, high_throughput=True)
        self.assertEqual(result["status"], "blocked")
        self.assertTrue((self.work / "htj2k-results.json").is_file())
        self.assertFalse((self.work / "numerical-results.json").exists())
        framework.validate_evidence_schemas(self.work)

    def test_reference_sample_mismatch_fails_before_subject_execution(self):
        calls = []
        def reference_process(command, timeout_seconds):
            calls.append(command)
            if "-o" not in command:
                self.fail("subject must not run after an invalid lossless reference roundtrip")
            output = Path(command[command.index("-o") + 1])
            output.write_bytes(b"P5\n1 1\n255\n\x80" if "reference" in output.stem else b"synthetic encoded fixture")
            return subprocess.CompletedProcess(command, 0, b"", b""), 0.0
        case = numerical.NumericalCase("reference-mismatch", width=1, height=1, decomposition_levels=1)
        with mock.patch.object(numerical.shutil, "which", return_value=sys.executable), \
                mock.patch.object(numerical, "HT_CASES", (case,)), \
                mock.patch.object(numerical, "_run_checked", side_effect=reference_process):
            result = numerical.run_numerical(Path(sys.executable), self.work, high_throughput=True)
        self.assertEqual(len(calls), 2)
        self.assertEqual(result["status"], "failed")
        self.assertIn("changed original samples", result["records"][0]["error"])
        self.assertTrue(result["records"][0]["artifacts"])

    @unittest.skipUnless(shutil.which(os.environ.get("OPENJPH_ENCODER", "ojph_compress"))
                         and shutil.which(os.environ.get("OPENJPH_DECODER", "ojph_expand")),
                         "independent OpenJPH tools are not installed")
    def test_independent_ht_lossless_lossy_and_jph_roundtrips(self):
        result = numerical.run_numerical(framework.build_subject(), self.work, high_throughput=True)
        self.assertEqual(result["status"], "passed", result["records"])
        self.assertEqual(result["summary"]["passed"], len(numerical.HT_CASES))
        self.assertTrue(all(record["metrics"]["maximum_error"] == 0
                            for record in result["records"] if record["lossless"]))
        for case, record in zip(numerical.HT_CASES, result["records"]):
            self.assertEqual(record["encoder_invoked"], case.codestream is None)
            if case.codestream is not None:
                self.assertIsNone(record["timings"]["encoder_ms"])
                self.assertEqual(record["encoder_options"], [])
            else:
                self.assertIsInstance(record["timings"]["encoder_ms"], float)
        framework.validate_evidence_schemas(self.work)
        for invoked, timing, options in (
            (False, 0.0, []),
            (False, None, ["unexpected-option"]),
            (True, None, []),
        ):
            invalid = copy.deepcopy(result)
            invalid["records"][0]["encoder_invoked"] = invoked
            invalid["records"][0]["encoder_options"] = options
            invalid["records"][0]["timings"]["encoder_ms"] = timing
            with self.subTest(invoked=invoked, timing=timing, options=options), self.assertRaises(ValueError):
                framework.validate_schema(framework.VALIDATION / "numerical-result.schema.json", invalid)

    @unittest.skipUnless(shutil.which("opj_compress") and shutil.which("opj_decompress"),
                         "independent OpenJPEG tools are not installed")
    def test_independent_lossless_and_lossy_roundtrips(self):
        result = numerical.run_numerical(framework.build_subject(), self.work)
        self.assertEqual(result["status"], "passed", result["records"])
        self.assertEqual(result["summary"]["passed"], len(numerical.CASES))
        self.assertTrue(all(record["codestream_sha256"] for record in result["records"]))
        self.assertTrue(all(record["metrics"]["maximum_error"] == 0
                            for record in result["records"] if record["lossless"]))
        self.assertTrue(all(record["metrics"] is not None
                            for record in result["records"] if not record["lossless"]))
        framework.validate_evidence_schemas(self.work)
