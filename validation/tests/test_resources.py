import copy
import math
import shutil
import subprocess
import struct
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import framework
import resource_benchmarks as benchmarks


class ResourceTests(unittest.TestCase):
    def setUp(self):
        root = framework.RESULTS_DIR / "test-work"
        root.mkdir(parents=True, exist_ok=True)
        self.directory = tempfile.TemporaryDirectory(dir=root)
        self.addCleanup(self.directory.cleanup)
        self.work = Path(self.directory.name)
        self.metrics = self.work / "time.txt"
        self.metrics.write_text("4\t0.01\t0.02\n")
        self.output = b"IMAGE\n2 1 1\n2\t1\t8\t0\tunsigned\t0,255\n"
        self.expected = framework.parse_subject_output(self.output, b"", 0)

    def test_peak_rss_is_measured_in_bytes_and_unavailable_values_fail(self):
        self.assertEqual(benchmarks._peak_rss(self.metrics), 4096)
        for invalid in ("", "0\n", "-1\n", "not-a-measurement\n"):
            self.metrics.write_text(invalid)
            with self.subTest(value=invalid), self.assertRaises(ValueError):
                benchmarks._peak_rss(self.metrics)

    def test_measurement_requires_exact_images_and_typed_rejections(self):
        process = subprocess.CompletedProcess([], 0, self.output, b"")
        with mock.patch.object(framework, "_run_process_bounded", return_value=(process, 1.0, False)):
            measured = benchmarks._measure(["subject"], "time", self.metrics, self.expected,
                                           None, None, False, 10.0)
        self.assertEqual(measured["output_sha256"], framework.normalized_hash(self.expected))
        self.assertEqual(measured["peak_rss_bytes"], 4096)
        self.assertEqual(measured["user_cpu_ms"], 10.0)
        self.assertEqual(measured["system_cpu_ms"], 20.0)
        self.assertEqual(measured["total_cpu_ms"], 30.0)
        for code, output, limited in (
            (0, self.output.replace(b"0,255", b"0,254"), False),
            (2, b"ERR\tLimitExceeded\tPixels\n", False),
            (1, b"ERR\tLimitExceeded\tWidth\n", False),
            (1, b"not an observation", False),
            (0, self.output, True),
        ):
            process = subprocess.CompletedProcess([], code, output, b"")
            with mock.patch.object(framework, "_run_process_bounded", return_value=(process, 1.0, limited)):
                with self.subTest(code=code, output=output, limited=limited), self.assertRaises(ValueError):
                    benchmarks._measure(["subject"], "time", self.metrics, self.expected,
                                        None if code == 0 else "LimitExceeded",
                                        None if code == 0 else "Pixels", False, 10.0)
        process = subprocess.CompletedProcess([], 1, b"ERR\tLimitExceeded\tPixels\n", b"")
        with mock.patch.object(framework, "_run_process_bounded", return_value=(process, 1.0, False)):
            measured = benchmarks._measure(["subject"], "time", self.metrics, None,
                                           "LimitExceeded", "Pixels", False, 10.0)
        self.assertIsNone(measured["output_sha256"])

    def test_cpu_metrics_are_required_finite_nonnegative_and_not_work_units(self):
        for text in ("", "4\n", "4\t0.01\n", "4\tNaN\t0\n", "4\t0\tinf\n", "4\t-1\t0\n"):
            self.metrics.write_text(text)
            with self.subTest(text=text), self.assertRaises(ValueError):
                benchmarks._process_metrics(self.metrics)

    def test_process_caps_are_unchanged_and_invalid_measurements_cannot_pass(self):
        thresholds = {"maximum_ms": 5000.0, "maximum_peak_rss_bytes": 512 << 20}
        benchmarks._check_thresholds({"elapsed_ms": 5000.0, "peak_rss_bytes": 512 << 20}, thresholds)
        for elapsed, rss in ((5000.001, 512 << 20), (5000.0, (512 << 20) + 1),
                             (math.nan, 4096), (math.inf, 4096), (-1, 4096), (1, 0), (1, 1.5)):
            with self.subTest(elapsed=elapsed, rss=rss), self.assertRaises(ValueError):
                benchmarks._check_thresholds({"elapsed_ms": elapsed, "peak_rss_bytes": rss}, thresholds)
        if framework.resource is not None:
            with mock.patch.object(framework.resource, "setrlimit") as limit:
                benchmarks._limit_process()
            self.assertEqual(limit.call_args_list, [
                mock.call(framework.resource.RLIMIT_AS, (1 << 30, 1 << 30)),
                mock.call(framework.resource.RLIMIT_FSIZE, (32 << 20, 32 << 20)),
            ])
        else:
            with self.assertRaises(RuntimeError):
                benchmarks._limit_process()

    def test_jpip_requires_exact_reconstructed_bytes_as_well_as_exact_samples(self):
        digest = framework.sha256_bytes(b"payload")
        for prefix, valid in ((b"BYTES\t" + b"payload".hex().encode(), True),
                              (b"BYTES\t00", False), (b"META\tpayload", False),
                              (b"BYTES\tnot-hex", False)):
            process = subprocess.CompletedProcess([], 0, prefix + b"\n" + self.output, b"")
            with mock.patch.object(framework, "_run_process_bounded", return_value=(process, 1.0, False)):
                if valid:
                    measured = benchmarks._measure(["subject"], "time", self.metrics, self.expected,
                                                   None, None, False, 10.0, reconstructed_sha256=digest)
                    self.assertEqual(measured["reconstructed_sha256"], digest)
                else:
                    with self.subTest(prefix=prefix), self.assertRaises(ValueError):
                        benchmarks._measure(["subject"], "time", self.metrics, self.expected,
                                            None, None, False, 10.0, reconstructed_sha256=digest)

    def test_predeclared_matrix_is_finite_and_keeps_all_baseline_cases(self):
        declaration = benchmarks._declaration()
        self.assertEqual(declaration["baseline_cases"], 22)
        self.assertEqual(len(declaration["workloads"]), 10)
        self.assertEqual(sum(len(case["modes"]) for case in declaration["workloads"]), 16)
        self.assertEqual(len(declaration["rejections"]), 7)
        self.assertEqual(len(benchmarks._expected_case_ids()), 45)
        self.assertLessEqual(max(case["width"] for case in declaration["workloads"]), 1536)
        fixtures = benchmarks._rejection_inputs()
        self.assertEqual(set(fixtures), set(benchmarks.R1_REJECTIONS))
        self.assertLess(max(len(data) for data, _, _ in fixtures.values()), 8192)
        for resource, limit, actual in benchmarks.R1_REJECTIONS.values():
            self.assertGreater(actual, limit, resource)

    def test_references_are_bounded_and_disagreements_never_qualify(self):
        process = subprocess.CompletedProcess([], 0, b"", b"")
        with mock.patch.object(framework, "_run_process_bounded", return_value=(process, 1.0, False)) as run:
            benchmarks._run_reference(["reference"], 10.0)
        self.assertEqual(run.call_args.args[2], 16 << 20)
        self.assertIs(run.call_args.args[3], benchmarks._limit_process)
        payload = self.work / "fixture"
        payload.write_bytes(b"qualified")
        qualification = benchmarks._qualify(self.expected, self.expected, payload, [payload], [payload])
        self.assertEqual(qualification["original_output_sha256"], qualification["reference_output_sha256"])
        altered = copy.deepcopy(self.expected)
        altered["image"]["components"][0]["samples"][0] = 1
        with self.assertRaisesRegex(ValueError, "original/reference"):
            benchmarks._qualify(self.expected, altered, payload, [payload], [payload])
        for code, limited in ((1, False), (0, True)):
            process = subprocess.CompletedProcess([], code, b"", b"")
            with mock.patch.object(framework, "_run_process_bounded", return_value=(process, 1.0, limited)):
                with self.subTest(code=code, limited=limited), self.assertRaises(ValueError):
                    benchmarks._run_reference(["reference"], 10.0)

    def test_production_headers_must_match_the_declared_workload_before_qualification(self):
        spec = ("header-check", 32, 1, 8, 32, 1, False)
        siz = struct.pack(">HIIIIIIIIH", 0, 32, 32, 0, 0, 32, 32, 0, 0, 1) + bytes((7, 1, 1))
        base = bytearray(b"\xff\x4f" + framework.segment(0xff51, siz) + framework.raw_codestream()[45:])
        cod = base.index(b"\xff\x52") + 4
        base[cod + 5] = 2
        tools = {"encoder": {"path": "encoder"}, "decoder": {"path": "decoder"}}

        def checked(command, timeout):
            output = Path(command[command.index("-o") + 1])
            if command[0] == "encoder":
                output.write_bytes(data)
            else:
                shutil.copyfile(self.work / "header-check.pgm", output)

        data = bytes(base)
        with mock.patch.object(benchmarks, "_run_reference", side_effect=checked):
            _, _, qualification = benchmarks._production_fixture(spec, self.work, tools, 10.0)
        self.assertEqual(qualification["original_output_sha256"], qualification["reference_output_sha256"])
        for position, value in ((42, 15), (cod + 5, 1), (cod + 9, 0), (cod + 3, 2), (27, 16)):
            mutated = bytearray(base)
            mutated[position] = value
            data = bytes(mutated)
            with mock.patch.object(benchmarks, "_run_reference", side_effect=checked) as reference:
                with self.subTest(position=position), self.assertRaises(ValueError):
                    benchmarks._production_fixture(spec, self.work, tools, 10.0)
                self.assertEqual(reference.call_count, 1)

    def test_missing_tools_are_blocked_not_successful_or_built(self):
        with mock.patch.object(shutil, "which", return_value=None), \
                mock.patch.dict(benchmarks.os.environ, {"OPENJPH_ENCODER": "", "OPENJPH_DECODER": ""}), \
                mock.patch.object(framework, "build_subject") as build:
            report = benchmarks.run_resources(output_dir=self.work)
            build.assert_not_called()
            self.assertEqual(report["status"], "blocked")
            self.assertEqual(report["records"], [])
            self.assertTrue(report["blockers"])
            with self.assertRaises(FileNotFoundError):
                benchmarks.run_resources(output_dir=self.work, encoder=Path("missing"))
        framework.validate_evidence_schemas(self.work)

    def test_ht_tools_are_required_and_explicit_missing_paths_are_errors(self):
        def which(command):
            return None if str(command).endswith("ojph_expand") else sys.executable
        with mock.patch.object(shutil, "which", side_effect=which), \
                mock.patch.dict(benchmarks.os.environ, {"OPENJPH_ENCODER": "", "OPENJPH_DECODER": ""}), \
                mock.patch.object(framework, "build_subject") as build:
            report = benchmarks.run_resources(output_dir=self.work)
            self.assertEqual(report["status"], "blocked")
            self.assertTrue(any("ht_decoder" in blocker for blocker in report["blockers"]))
            build.assert_not_called()
        with mock.patch.object(shutil, "which", return_value=None):
            with self.assertRaises(FileNotFoundError):
                benchmarks.run_resources(output_dir=self.work, ht_encoder=Path("missing-ht-encoder"))

    def test_ht_resource_geometry_and_coding_signals_cannot_be_omitted(self):
        def fixture(tile=128, depth=15, style=0x40, capability=True):
            siz = struct.pack(">HIIIIIIIIH", 0x4000, 512, 512, 0, 0, tile, tile, 0, 0, 3)
            data = b"\xff\x4f" + framework.segment(0xff51, siz + bytes((depth, 1, 1)) * 3)
            if capability:
                data += framework.segment(0xff50, bytes((0, 2, 0, 0, 0, 0)))
            return data + framework.segment(0xff52, bytes((0, 0, 0, 1, 1, 2, 4, 4, style, 1))) + b"\xff\x90"
        benchmarks._validate_ht_resource_headers(fixture(), 512)
        for invalid in (fixture(tile=64), fixture(depth=7), fixture(style=0), fixture(capability=False)):
            with self.subTest(data=invalid), self.assertRaises(ValueError):
                benchmarks._validate_ht_resource_headers(invalid, 512)

    def test_shared_reference_crop_is_exact_and_rejects_invalid_geometry(self):
        cropped = framework.crop_image(self.expected, (0, 0, 2, 1), (0, 0, 1, 1))
        self.assertEqual(cropped["image"]["components"][0]["samples"], [0])
        self.assertEqual(self.expected["image"]["components"][0]["samples"], [0, 255])
        for rectangle, sampling in (((0, 0, 3, 1), (1, 1)), ((0, 0, 0, 1), (1, 1)),
                                    ((0, 0, 1, 1), (0, 1))):
            with self.subTest(rectangle=rectangle, sampling=sampling), self.assertRaises(ValueError):
                framework.crop_image(self.expected, (0, 0, 2, 1), rectangle, sampling)
        with self.assertRaises(ValueError):
            framework.crop_image(self.expected, (0, 0, 3, 1), (0, 0, 1, 1))

    def test_configured_rejections_require_exact_resource_limit_and_excessive_claim(self):
        output = b"ERR\tLimitExceeded\tresource limit exceeded for WorkUnits: limit 8, actual 9\n"
        process = subprocess.CompletedProcess([], 1, output, b"")
        with mock.patch.object(framework, "_run_process_bounded", return_value=(process, 1.0, False)):
            measured = benchmarks._measure(["subject"], "time", self.metrics, self.expected,
                                           "LimitExceeded", "WorkUnits", False, 10.0,
                                           allow_success=True, configured_value=8)
        self.assertEqual(measured["limit_claim"], {"resource": "WorkUnits", "limit": 8, "actual": 9})
        for invalid in (
            output.replace(b"limit 8", b"limit 7"),
            output.replace(b"actual 9", b"actual 8"),
            output.replace(b"WorkUnits", b"DecodedBytes"),
            output.replace(b"LimitExceeded", b"MalformedInput"),
            output.replace(b"actual 9", b"actual unknown"),
        ):
            process = subprocess.CompletedProcess([], 1, invalid, b"")
            with mock.patch.object(framework, "_run_process_bounded", return_value=(process, 1.0, False)):
                with self.subTest(output=invalid), self.assertRaises(ValueError):
                    benchmarks._measure(["subject"], "time", self.metrics, self.expected,
                                        "LimitExceeded", "WorkUnits", False, 10.0,
                                        allow_success=True, configured_value=8)
        process = subprocess.CompletedProcess([], 0, self.output, b"")
        with mock.patch.object(framework, "_run_process_bounded", return_value=(process, 1.0, False)):
            measured = benchmarks._measure(["subject"], "time", self.metrics, self.expected,
                                           "LimitExceeded", "WorkUnits", False, 10.0,
                                           allow_success=True, configured_value=9)
        self.assertEqual(measured["output_sha256"], framework.normalized_hash(self.expected))
        self.assertNotIn("limit_claim", measured)
        process = subprocess.CompletedProcess([], 0, self.output.replace(b"0,255", b"0,254"), b"")
        with mock.patch.object(framework, "_run_process_bounded", return_value=(process, 1.0, False)):
            with self.assertRaises(ValueError):
                benchmarks._measure(["subject"], "time", self.metrics, self.expected,
                                    "LimitExceeded", "WorkUnits", False, 10.0,
                                    allow_success=True, configured_value=9)

    def test_limit_search_measures_both_endpoints_and_finds_adjacent_outcomes(self):
        observations = []
        def measured(command, *args, **kwargs):
            value = int(command[-1])
            self.assertTrue(kwargs["allow_success"])
            self.assertEqual(kwargs["configured_value"], value)
            return {"elapsed_ms": 1.0, "peak_rss_bytes": 4096,
                    "returncode": 0 if value >= 9 else 1,
                    "output_sha256": framework.normalized_hash(self.expected) if value >= 9 else None}
        with mock.patch.object(benchmarks, "_measure", side_effect=measured):
            accepted = benchmarks._find_limit_boundary(
                ["subject", "decode-limited"], "WorkUnits", 64, self.expected, "time",
                self.metrics, 10.0, {"maximum_ms": 100.0, "maximum_peak_rss_bytes": 8192}, observations,
            )
        self.assertEqual(accepted, 9)
        self.assertEqual([run["limit_value"] for run in observations[:2]], [0, 64])
        self.assertTrue(any(run["limit_value"] == 8 and run["returncode"] == 1 for run in observations))
        self.assertTrue(any(run["limit_value"] == 9 and run["returncode"] == 0 for run in observations))
        self.assertLessEqual(len(observations), benchmarks.MAXIMUM_SEARCH_PROBES)

    def test_limit_search_fails_for_unbracketed_inputs_and_probe_time_rss_or_count_excess(self):
        thresholds = {"maximum_ms": 100.0, "maximum_peak_rss_bytes": 8192}
        def search(upper=64):
            return benchmarks._find_limit_boundary(
                ["subject"], "WorkUnits", upper, self.expected, "time", self.metrics,
                10.0, thresholds, [],
            )
        for upper in (0, -1, True, 1.5):
            with self.subTest(upper=upper), self.assertRaises(ValueError):
                search(upper)
        for returncode, elapsed, rss in ((0, 1.0, 4096), (1, 1.0, 4096),
                                          (1, 101.0, 4096), (1, 1.0, 8193)):
            measured = {"elapsed_ms": elapsed, "peak_rss_bytes": rss, "returncode": returncode,
                        "output_sha256": None}
            with mock.patch.object(benchmarks, "_measure", return_value=measured):
                with self.subTest(measured=measured), self.assertRaises(ValueError):
                    search()
        def many_probes(command, *args, **kwargs):
            return {"elapsed_ms": 1.0, "peak_rss_bytes": 4096,
                    "returncode": 1 if int(command[-1]) == 0 else 0, "output_sha256": None}
        with mock.patch.object(benchmarks, "_measure", side_effect=many_probes):
            with self.assertRaisesRegex(ValueError, "finite probe budget"):
                search(1 << 40)

    def test_invalid_timeouts_do_not_generate_reports(self):
        for value in (0, -1, math.inf, math.nan):
            with self.subTest(timeout=value), self.assertRaises(ValueError):
                benchmarks.run_resources(output_dir=self.work, timeout_seconds=value)
        self.assertFalse((self.work / "resource-results.json").exists())

    def test_success_schema_requires_three_measured_runs_and_output_hashes(self):
        with mock.patch.object(shutil, "which", return_value=None), \
                mock.patch.dict(benchmarks.os.environ, {"OPENJPH_ENCODER": "", "OPENJPH_DECODER": ""}):
            report = benchmarks.run_resources(output_dir=self.work)
        report.update(status="passed", blockers=[], native_total_ms=151.0)
        tool = {"path": sys.executable, "sha256": framework.sha256_file(Path(sys.executable))}
        report["tools"] = {name: tool for name in ("encoder", "decoder", "time", "subject", "ht_encoder", "ht_decoder")}
        run = {"elapsed_ms": 1.0, "peak_rss_bytes": 4096, "returncode": 0,
               "user_cpu_ms": 0.0, "system_cpu_ms": 0.0, "total_cpu_ms": 0.0,
               "output_sha256": framework.normalized_hash(self.expected)}
        record = {
            "id": "case", "input_bytes": 1, "input_sha256": "0" * 64,
            "expected_category": None, "expected_reason": None,
            "expected_output_sha256": run["output_sha256"],
            "qualifications": [{
                "original_output_sha256": run["output_sha256"], "reference_output_sha256": run["output_sha256"],
                "payload_sha256": "0" * 64,
                "source_files": [{"name": "source", "sha256": "0" * 64}],
                "reference_files": [{"name": "reference", "sha256": "0" * 64}],
            }],
            "expected_reconstructed_sha256": None, "expected_limit_claim": None,
            "limit_mode": "production-default", "mode": "full",
            "typed_sample_bytes": 16, "output_width": 2, "output_height": 1, "output_components": 1,
            "thresholds": {"maximum_ms": 5000.0, "maximum_peak_rss_bytes": 512 << 20},
            "runs": [copy.deepcopy(run) for _ in range(3)], "status": "passed",
            "error": None, "artifacts": [], "median_ms": 1.0, "peak_rss_bytes": 4096,
            "configured_limit": None, "search_ceiling": None, "search_runs": [],
        }
        report["records"] = [dict(copy.deepcopy(record), id=name)
                             for name in sorted(benchmarks._expected_case_ids())]
        for index in range(4):
            current = report["records"][index]
            resource = "DecodedBytes" if index < 2 else "WorkUnits"
            current["configured_limit"] = {"resource": resource, "value": 9}
            current["limit_mode"] = "single-tightened-defaults"
            current["search_ceiling"] = 64
            rejected = {"elapsed_ms": 1.0, "peak_rss_bytes": 4096, "returncode": 1,
                        "user_cpu_ms": 0.0, "system_cpu_ms": 0.0, "total_cpu_ms": 0.0,
                        "output_sha256": None, "limit_value": 0,
                        "limit_claim": {"resource": resource, "limit": 0, "actual": 9}}
            current["search_runs"] = [rejected, {"limit_value": 64, **run},
                                      {**rejected, "limit_value": 8,
                                       "limit_claim": {"resource": resource, "limit": 8, "actual": 9}},
                                      {"limit_value": 9, **run}]
            below = report["records"][index + 4]
            below.update(configured_limit={"resource": resource, "value": 8},
                         expected_category="LimitExceeded", expected_reason=resource,
                         expected_output_sha256=None, typed_sample_bytes=0,
                         output_width=0, output_height=0, output_components=0,
                         mode="rejection", qualifications=[],
                         limit_mode="single-tightened-defaults",
                         thresholds={"maximum_ms": 1000.0, "maximum_peak_rss_bytes": 128 << 20})
            below["runs"] = [{**run, "returncode": 1, "output_sha256": None,
                              "limit_claim": {"resource": resource, "limit": 8, "actual": 9}}
                             for _ in range(3)]
        schema = framework.VALIDATION / "resource-result.schema.json"
        framework.validate_schema(schema, report)
        benchmarks._validate_evidence(report)
        for mutation in ("rss", "count", "hash", "exit", "claim", "search", "probe-count", "probe-claim",
                         "cpu", "caps", "qualification", "matrix"):
            invalid = copy.deepcopy(report)
            current = invalid["records"][0]
            if mutation == "rss":
                del current["runs"][0]["peak_rss_bytes"]
            elif mutation == "count":
                current["runs"].pop()
            elif mutation == "hash":
                current["runs"][0]["output_sha256"] = None
            elif mutation == "exit":
                current["runs"][0]["returncode"] = 2
            elif mutation == "claim":
                del invalid["records"][4]["runs"][0]["limit_claim"]
            elif mutation == "search":
                current["search_runs"].clear()
            elif mutation == "probe-count":
                current["search_runs"] = [current["search_runs"][0]] * 33
            elif mutation == "probe-claim":
                del current["search_runs"][0]["limit_claim"]
            elif mutation == "cpu":
                del current["runs"][0]["user_cpu_ms"]
            elif mutation == "caps":
                invalid["maximum_native_total_ms"] = 31000.0
            elif mutation == "qualification":
                current["qualifications"] = []
            else:
                invalid["declaration"]["workloads"].pop()
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                framework.validate_schema(schema, invalid)
        for mutation in ("duplicate", "omission", "matrix", "prequalification", "cpu-sum", "aggregate",
                         "output", "adjacent", "limit-mode", "reconstruction", "default-claim"):
            invalid = copy.deepcopy(report)
            current = invalid["records"][0]
            if mutation == "duplicate":
                invalid["records"][1]["id"] = current["id"]
            elif mutation == "omission":
                invalid["records"].pop()
            elif mutation == "matrix":
                invalid["declaration"]["workloads"][0]["width"] = 1535
            elif mutation == "prequalification":
                current["qualifications"][0]["reference_output_sha256"] = "f" * 64
            elif mutation == "cpu-sum":
                current["runs"][0]["user_cpu_ms"] = 1.0
            elif mutation == "aggregate":
                invalid["native_total_ms"] += 1
            elif mutation == "output":
                current["runs"][0]["output_sha256"] = "f" * 64
            elif mutation == "adjacent":
                current["search_runs"] = current["search_runs"][:2]
            elif mutation == "limit-mode":
                current["limit_mode"] = "production-default"
            elif mutation == "reconstruction":
                current["expected_reconstructed_sha256"] = "f" * 64
            else:
                invalid["records"][4]["expected_limit_claim"] = {"resource": "WorkUnits", "limit": 8, "actual": 10}
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                benchmarks._validate_evidence(invalid)
