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
                extension = numerical.source_extension(case)
                path = self.work / f"source.{extension}"
                path.write_bytes(data)
                parse_image = framework.parse_pgx if extension == "pgx" else framework.parse_pnm
                parsed = parse_image(path)
                self.assertEqual(len(parsed["image"]["components"]), case.channels)
                for component in parsed["image"]["components"]:
                    self.assertEqual(component["precision"], case.precision)
                    self.assertEqual(len(component["samples"]), case.width * case.height)
                    self.assertEqual(component["signed"], case.signed)
                    lower = -(1 << (case.precision - 1)) if case.signed else 0
                    self.assertEqual(min(component["samples"]), lower)
                    self.assertEqual(max(component["samples"]), lower + (1 << case.precision) - 1)

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

    def test_pgx_preserves_endianness_signedness_and_padded_sample_widths(self):
        for precision, signed in ((1, True), (8, False), (9, True), (16, True),
                                  (17, True), (24, False), (32, False)):
            lower = -(1 << (precision - 1)) if signed else 0
            values = [lower, lower + (1 << precision) - 1, 0]
            sample_bytes = 1 if precision <= 8 else 2 if precision <= 16 else 4
            for marker, byteorder in (("ML", "big"), ("LM", "little")):
                with self.subTest(precision=precision, signed=signed, byteorder=byteorder):
                    payload = b"".join(value.to_bytes(sample_bytes, byteorder, signed=signed)
                                       for value in values)
                    path = self.work / "samples.pgx"
                    path.write_bytes(f"PG {marker} {'-' if signed else '+'} {precision} 3 1\n".encode() + payload)
                    self.assertEqual(framework.parse_pgx(path), image(values, precision, signed))
        path.write_bytes(b"PG ML + 8 4 1\r\n" + bytes((10, 13, 32, 255)))
        self.assertEqual(framework.parse_pgx(path), image([10, 13, 32, 255]))

    def test_pgx_rejects_malformed_headers_payloads_and_out_of_range_samples(self):
        for invalid in (
            b"PG ML + 8 1 1",
            b"P5 ML + 8 1 1\n\x00",
            b"PG XX + 8 1 1\n\x00",
            b"PG ML ? 8 1 1\n\x00",
            b"PG ML + 0 1 1\n\x00",
            b"PG ML + 33 1 1\n\x00",
            b"PG ML + 8 0 1\n",
            b"PG ML + 8 1 -1\n",
            b"PG ML + bad 1 1\n\x00",
            b"PG ML + 8 1 1 extra\n\x00",
            b"PG ML + 8 2 1\n\x00",
            b"PG ML + 8 1 1\n\x00\x00",
            b"PG ML + 17 1 1\n\x00\x00\x00",
            b"PG ML + 12 1 1\n\xff\xff",
            b"PG ML - 9 1 1\n\xfe\x00",
        ):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                path = self.work / "invalid.pgx"
                path.write_bytes(invalid)
                framework.parse_pgx(path)

    def test_fixed_and_sparse_pgx_sources_preserve_declared_ranges(self):
        values = (-(1 << 23), -1, 0, (1 << 23) - 1)
        case = numerical.NumericalCase("fixed-signed", precision=24, signed=True,
                                       width=4, height=1, pattern="fixed", samples=values)
        path = self.work / "source.pgx"
        path.write_bytes(numerical.source_image(case))
        self.assertEqual(framework.parse_pgx(path), image(list(values), 24, True))
        sparse = numerical.NumericalCase("sparse-signed", precision=12, signed=True, pattern="sparse")
        path.write_bytes(numerical.source_image(sparse))
        samples = framework.parse_pgx(path)["image"]["components"][0]["samples"]
        self.assertEqual(set(samples), {-(1 << 11), 0, (1 << 11) - 1})
        for invalid in (
            numerical.NumericalCase("invalid", precision=0),
            numerical.NumericalCase("invalid", precision=33),
            numerical.NumericalCase("invalid", signed=1),
            numerical.NumericalCase("invalid", signed=True, channels=3),
            numerical.NumericalCase("invalid", precision=24, channels=3),
            numerical.NumericalCase("invalid", signed=True, width=1, height=1,
                                    pattern="fixed", samples=(128,)),
        ):
            with self.subTest(case=invalid), self.assertRaises(ValueError):
                numerical.source_image(invalid)

    def test_binary32_fixture_words_preserve_endianness_and_negative_zero(self):
        values = [0x3f800000, -(1 << 31), -1082130432]
        for scale, byteorder in ((b"-1.0", "<"), (b"1.0", ">")):
            path = self.work / "source.pfm"
            path.write_bytes(b"Pf\n3 1\n" + scale + b"\n" + struct.pack(byteorder + "3i", *values))
            self.assertEqual(framework.parse_pfm_bit_patterns(path), image(values, 32, True))
        case = next(case for case in numerical.HT_CASES if case.sample_representation != "integer")
        path.write_bytes(numerical.source_image(case))
        parsed = framework.parse_pfm_bit_patterns(path)["image"]["components"][0]
        self.assertEqual(parsed["samples"], list(case.samples))
        self.assertIn(-(1 << 31), parsed["samples"])
        self.assertIn(0, parsed["samples"])

    def test_binary32_fixture_headers_and_payloads_are_strict(self):
        path = self.work / "invalid.pfm"
        for data in (b"Pf\n1 1\n-1.0", b"P5\n1 1\n-1.0\n\0\0\0\0",
                     b"Pf\n0 1\n-1.0\n", b"Pf\n1 1\n2.0\n\0\0\0\0",
                     b"Pf\n1 1\nnan\n\0\0\0\0", b"Pf\n1 1\n-1.0\n\0\0\0",
                     b"Pf\n1 1\n-1.0\n\0\0\0\0\0"):
            with self.subTest(data=data), self.assertRaises(ValueError):
                path.write_bytes(data)
                framework.parse_pfm_bit_patterns(path)

    def test_required_roi_and_nlt_headers_cannot_be_silently_omitted(self):
        roi = next(case for case in numerical.CASES if case.name == "roi-gray-shift3")
        segment = framework.segment(0xff5e, b"\x00\x00\x03")
        self.assertEqual(numerical.validate_required_headers(roi, b"\xff\x4f" + segment + b"\xff\x90"),
                         ["component-maxshift"])
        nlt = next(case for case in numerical.HT_CASES if case.sample_representation != "integer")
        segment = framework.segment(0xff76, b"\xff\xff\x9f\x03")
        self.assertEqual(numerical.validate_required_headers(nlt, b"\xff\x4f" + segment + b"\xff\x90"),
                         ["binary-complement-nlt"])
        for case in (roi, nlt):
            with self.subTest(case=case.name), self.assertRaises(ValueError):
                numerical.validate_required_headers(case, framework.raw_codestream())

    def test_signed_fixture_changes_only_the_matching_siz_sign_bit(self):
        unsigned = framework.raw_codestream()
        signed = numerical.signed_codestream(unsigned, 8)
        self.assertEqual([index for index, (left, right) in enumerate(zip(unsigned, signed))
                          if left != right], [42])
        self.assertEqual(signed[42], 0x87)
        self.assertEqual(signed[43:], unsigned[43:])
        for invalid in (b"", unsigned[:44], signed,
                        unsigned[:40] + b"\x00\x02" + unsigned[42:],
                        unsigned[:4] + b"\x00\x28" + unsigned[6:]):
            with self.subTest(data=invalid), self.assertRaises(ValueError):
                numerical.signed_codestream(invalid, 8)
        with self.assertRaises(ValueError):
            numerical.signed_codestream(unsigned, 9)

    def test_signed_jp2_fixture_metadata_matches_the_signed_codestream(self):
        signed = numerical.signed_codestream(framework.raw_codestream(), 8)
        container = framework.jp2_file(signed, signed=True)
        offset = container.index(b"ihdr") + 4
        self.assertEqual(struct.unpack(">IIH4B", container[offset:offset + 14]),
                         (1, 1, 1, 0x87, 7, 0, 0))
        self.assertTrue(container.endswith(signed))
        with self.assertRaises(ValueError):
            framework.jp2_file(signed, signed=1)

    def test_pgx_reference_mismatch_and_corruption_fail_before_subject_execution(self):
        case = numerical.NumericalCase("signed-reference", width=1, height=1, precision=17, signed=True)
        for payload in ((123).to_bytes(4, "big", signed=True), b"\x00"):
            calls = []

            def reference_process(command, timeout_seconds):
                calls.append(command)
                if "-o" not in command:
                    self.fail("subject must not run after an invalid PGX reference")
                output = Path(command[command.index("-o") + 1])
                if "reference" in output.stem and output.suffix == ".pgx":
                    output.with_stem(output.stem + "_0").write_bytes(b"PG ML - 17 1 1\n" + payload)
                else:
                    fixture = bytearray(framework.raw_codestream())
                    fixture[42] = 16
                    output.write_bytes(fixture)
                return subprocess.CompletedProcess(command, 0, b"", b""), 0.0

            with mock.patch.object(numerical.shutil, "which", return_value=sys.executable), \
                    mock.patch.object(numerical, "CASES", (case,)), \
                    mock.patch.object(numerical, "_run_checked", side_effect=reference_process):
                result = numerical.run_numerical(Path(sys.executable), self.work)
            self.assertEqual(len(calls), 2)
            self.assertEqual(result["status"], "failed")
            record = result["records"][0]
            self.assertTrue(record["error"])
            self.assertTrue(any(name.endswith("-reference_0.pgx") for name in record["artifacts"]))
            self.assertEqual((record["precision"], record["signed"], record["source_format"]), (17, True, "pgx"))

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

    def test_unavailable_posix_enforcement_is_explicit_in_reports(self):
        with mock.patch.object(framework, "resource", None), \
                mock.patch.object(numerical.shutil, "which", return_value=None), \
                mock.patch.dict(numerical.os.environ, {"OPENJPEG_ENCODER": "", "OPENJPEG_DECODER": ""}):
            result = numerical.run_numerical(output_dir=self.work)
        self.assertTrue(result["execution_platform"])
        self.assertEqual(result["process_limits"], {
            "stage_timeout_seconds": 10.0, "maximum_output_bytes": 1 << 20,
            "requested_address_space_bytes": None, "requested_file_size_bytes": None,
        })
        framework.validate_schema(framework.VALIDATION / "numerical-result.schema.json", result)

    def test_macos_does_not_request_the_unsupported_address_space_limit(self):
        limits = mock.Mock()
        with mock.patch.object(framework, "resource", limits), \
                mock.patch.object(numerical.sys, "platform", "darwin"):
            numerical._limit_native_process()
        limits.setrlimit.assert_called_once_with(
            limits.RLIMIT_FSIZE, (numerical.NATIVE_FILE_BYTES, numerical.NATIVE_FILE_BYTES))

    def test_ht_sources_and_container_metadata_cover_real_component_layouts(self):
        self.assertEqual({case.format for case in numerical.HT_CASES}, {"raw", "jph"})
        self.assertEqual({case.precision for case in numerical.HT_CASES}, {8, 16, 32})
        self.assertEqual({case.channels for case in numerical.HT_CASES}, {1, 3})
        self.assertEqual({case.pattern for case in numerical.HT_CASES}, {"varied", "sparse", "fixed"})
        for case in numerical.HT_CASES:
            data = numerical.source_image(case)
            self.assertEqual(data, numerical.source_image(case))
            path = self.work / "ht-source.pnm"
            path.write_bytes(data)
            parse_image = (framework.parse_pfm_bit_patterns if numerical.source_extension(case) == "pfm"
                           else framework.parse_pnm)
            image = parse_image(path)["image"]
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
        binary32 = [record for record in result["records"] if record["source_format"] == "pfm"]
        self.assertEqual(len(binary32), 3)
        for record in binary32:
            self.assertEqual(record["verified_features"], ["binary-complement-nlt"])
            self.assertEqual(record["sample_representation"], "ieee754-binary32-bits")
            self.assertFalse(record["signed_header_authored"])
            self.assertEqual(record["encoder_input_sha256"], record["input_sha256"])
            self.assertEqual(record["source_metrics"]["maximum_error"], 0)
            self.assertEqual(record["reference_sha256"], record["decoded_sha256"])
        invalid = copy.deepcopy(result)
        next(record for record in invalid["records"] if record["source_format"] == "pfm")["verified_features"] = []
        with self.assertRaises(ValueError):
            framework.validate_schema(framework.VALIDATION / "numerical-result.schema.json", invalid)
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
        for case, record in zip(numerical.CASES, result["records"]):
            self.assertEqual(record["precision"], case.precision)
            self.assertEqual(record["signed"], case.signed)
            self.assertEqual(record["source_format"], numerical.source_extension(case))
            self.assertEqual(record["signed_header_authored"], case.signed)
            if case.signed:
                self.assertNotEqual(record["encoder_input_sha256"], record["input_sha256"])
        pgx = [record for record in result["records"] if record["source_format"] == "pgx"]
        self.assertEqual(len(pgx), 11)
        self.assertEqual({record["precision"] for record in pgx}, {8, 9, 12, 16, 17, 20, 24})
        for record in pgx:
            self.assertEqual(record["source_metrics"]["maximum_error"], 0)
            self.assertEqual(record["metrics"]["maximum_error"], 0)
            self.assertEqual(record["reference_sha256"], record["decoded_sha256"])
        for field in ("precision", "signed"):
            invalid = copy.deepcopy(result)
            del next(record for record in invalid["records"] if record["source_format"] == "pgx")[field]
            with self.subTest(field=field), self.assertRaises(ValueError):
                framework.validate_schema(framework.VALIDATION / "numerical-result.schema.json", invalid)
        roi = [record for record in result["records"] if record["id"].startswith("roi-")]
        self.assertEqual(len(roi), 9)
        self.assertTrue(all(record["verified_features"] == ["component-maxshift"] for record in roi))
        framework.validate_evidence_schemas(self.work)
