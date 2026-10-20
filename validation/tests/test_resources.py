import copy
import math
import shutil
import subprocess
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
        self.metrics.write_text("4\n")
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

    def test_missing_tools_are_blocked_not_successful_or_built(self):
        with mock.patch.object(shutil, "which", return_value=None), mock.patch.object(framework, "build_subject") as build:
            report = benchmarks.run_resources(output_dir=self.work)
            build.assert_not_called()
            self.assertEqual(report["status"], "blocked")
            self.assertEqual(report["records"], [])
            self.assertTrue(report["blockers"])
            with self.assertRaises(FileNotFoundError):
                benchmarks.run_resources(output_dir=self.work, encoder=Path("missing"))
        framework.validate_evidence_schemas(self.work)

    def test_invalid_timeouts_do_not_generate_reports(self):
        for value in (0, -1, math.inf, math.nan):
            with self.subTest(timeout=value), self.assertRaises(ValueError):
                benchmarks.run_resources(output_dir=self.work, timeout_seconds=value)
        self.assertFalse((self.work / "resource-results.json").exists())

    def test_success_schema_requires_three_measured_runs_and_output_hashes(self):
        with mock.patch.object(shutil, "which", return_value=None):
            report = benchmarks.run_resources(output_dir=self.work)
        report.update(status="passed", blockers=[], native_total_ms=24.0)
        tool = {"path": sys.executable, "sha256": framework.sha256_file(Path(sys.executable))}
        report["tools"] = {name: tool for name in ("encoder", "decoder", "time", "subject")}
        run = {"elapsed_ms": 1.0, "peak_rss_bytes": 4096, "returncode": 0,
               "output_sha256": framework.normalized_hash(self.expected)}
        record = {
            "id": "case", "input_bytes": 1, "input_sha256": "0" * 64,
            "expected_category": None, "expected_reason": None,
            "expected_output_sha256": run["output_sha256"],
            "typed_sample_bytes": 16, "output_width": 2, "output_height": 1, "output_components": 1,
            "thresholds": {"maximum_ms": 5000.0, "maximum_peak_rss_bytes": 1 << 20},
            "runs": [copy.deepcopy(run) for _ in range(3)], "status": "passed",
            "error": None, "artifacts": [], "median_ms": 1.0, "peak_rss_bytes": 4096,
        }
        report["records"] = [dict(copy.deepcopy(record), id=f"case-{index}") for index in range(8)]
        schema = framework.VALIDATION / "resource-result.schema.json"
        framework.validate_schema(schema, report)
        for mutation in ("rss", "count", "hash", "exit"):
            invalid = copy.deepcopy(report)
            current = invalid["records"][0]
            if mutation == "rss":
                del current["runs"][0]["peak_rss_bytes"]
            elif mutation == "count":
                current["runs"].pop()
            elif mutation == "hash":
                current["runs"][0]["output_sha256"] = None
            else:
                current["runs"][0]["returncode"] = 2
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                framework.validate_schema(schema, invalid)
