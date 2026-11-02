"""Runner checks; these are separate from the expected-failing Rust regressions."""
import os
import json
from pathlib import Path
import sys
import tempfile
import unittest

from reproduce import artifact_path, classify_failure, execute


class RunnerTests(unittest.TestCase):
    def test_packaged_markers_accept_actual_serial_native_output(self):
        root = Path(__file__).resolve().parent
        manifest = json.loads((root / "manifest.json").read_text())
        for suite in manifest["suites"]:
            for case in suite["tests"]:
                with self.subTest(case=case["id"]):
                    log = (root / "verification-initial-marker-review" / suite["id"]
                           / f"{case['id']}.log").read_text()
                    run = {"exit_code": 101, "timed_out": False}
                    self.assertEqual(classify_failure(run, log, case)["status"], "reproduced")
                    for marker in case["markers"]:
                        self.assertEqual(
                            classify_failure(run, log.replace(marker, ""), case)["status"],
                            "unexpected_result",
                        )

    def test_only_exact_failure_with_every_marker_is_reproduced(self):
        case = {"expected_exit": 101, "markers": ["wrong sample", "expected 64"]}
        for code, timeout, text, expected in (
            (101, False, "wrong sample: expected 64", "reproduced"),
            (0, False, "wrong sample: expected 64", "unexpected_result"),
            (1, False, "wrong sample: expected 64", "unexpected_result"),
            (101, True, "wrong sample: expected 64", "unexpected_result"),
            (101, False, "wrong sample: expected 128", "unexpected_result"),
            (101, False, "unrelated panic", "unexpected_result"),
            (0, False, "running 0 tests", "unexpected_result"),
        ):
            with self.subTest(code=code, timeout=timeout, text=text):
                run = {"exit_code": code, "timed_out": timeout}
                self.assertEqual(classify_failure(run, text, case)["status"], expected)

    def test_empty_markers_and_zero_expected_exit_cannot_reproduce(self):
        for code, expected_exit, markers in (
            (0, 0, ["diagnostic"]), (101, 101, []), (101, 101, [""]),
        ):
            with self.subTest(markers=markers, expected_exit=expected_exit):
                case = {"expected_exit": expected_exit, "markers": markers}
                run = {"exit_code": code, "timed_out": False}
                self.assertEqual(
                    classify_failure(run, "diagnostic", case)["status"], "unexpected_result"
                )

    def test_artifact_paths_stay_relative(self):
        self.assertEqual(artifact_path("src/test.rs"), Path("src/test.rs"))
        for path in ("../outside", "/absolute", "src/../../outside", "", "."):
            with self.subTest(path=path), self.assertRaises(ValueError):
                artifact_path(path)

    def test_output_and_nonzero_status_are_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = execute(
                [sys.executable, "-c", "print('specific failure'); raise SystemExit(101)"],
                root, os.environ.copy(), root / "run.log", 10,
            )
            self.assertEqual(result["exit_code"], 101)
            self.assertFalse(result["timed_out"])
            self.assertEqual((root / "run.log").read_text(), "specific failure\n")

    def test_timeout_is_not_a_native_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = execute(
                [sys.executable, "-c", "import time; time.sleep(30)"],
                root, os.environ.copy(), root / "run.log", 0.1,
            )
            self.assertTrue(result["timed_out"])
            self.assertLess(result["exit_code"], 0)

    def test_missing_program_raises_instead_of_reproducing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(FileNotFoundError):
                execute([str(root / "missing")], root, os.environ.copy(), root / "run.log", 10)


if __name__ == "__main__":
    unittest.main()
