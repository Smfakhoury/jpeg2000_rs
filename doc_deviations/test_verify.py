import copy
import json
from pathlib import Path
import shutil
import tempfile
import unittest

from verify import FINDINGS, ROOT, report_matches, selected_cases, transform_expectation, verify_integrity


class VerificationTests(unittest.TestCase):
    def test_modified_archive_is_rejected_before_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "jpeg2000-review-package-20261029.tar.gz").write_bytes(b"modified")
            with self.assertRaisesRegex(ValueError, "Archive checksum mismatch"):
                verify_integrity(root)

    def test_modified_unpacked_test_is_rejected(self):
        name = "jpeg2000-review-package-20261029"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            shutil.copy2(ROOT / (name + ".tar.gz"), root)
            shutil.copytree(ROOT / name, root / name)
            test = (root / name / "cases/2bbb4b7f6d9814081dc6d341/artifacts/"
                    "validation/nonlinear_lookup_contract.rs")
            test.write_text(test.read_text() + "\n// changed\n")
            with self.assertRaisesRegex(ValueError, "Unpacked evidence differs"):
                verify_integrity(root)

    def test_complete_portable_inventory(self):
        manifests, count = verify_integrity(ROOT)
        self.assertEqual(len(manifests), 4)
        self.assertGreater(count, 300)
        findings = {f["id"] for m in manifests.values() for f in m["findings"]}
        self.assertEqual(findings | {"T1", "T2"}, FINDINGS)

    def test_recorded_reports_with_current_metadata_match_exact_cases(self):
        for name in ("jpeg2000-review-supplement-20261030",
                     "jpeg2000-final-jpip-witnesses-20261030", "jpeg2000-u-extension-witness-20261030"):
            with self.subTest(package=name):
                manifest = json.loads((ROOT / name / "manifest.json").read_text())
                report = json.loads((ROOT / name / "verification/report.json").read_text())
                selected = {f["id"] for f in manifest["findings"]}
                self.assertTrue(report_matches(report, selected_cases(manifest, selected), selected))

    def test_legacy_report_without_toolchain_is_not_accepted_as_fresh(self):
        root = ROOT / "jpeg2000-review-package-20261029"
        manifest = json.loads((root / "manifest.json").read_text())
        report = json.loads((root / "verification/report.json").read_text())
        selected = {f["id"] for f in manifest["findings"]}
        self.assertNotIn("toolchain", report)
        self.assertFalse(report_matches(report, selected_cases(manifest, selected), selected))

    def test_missing_duplicate_unexpected_and_passing_cases_are_not_reproductions(self):
        name = "jpeg2000-final-jpip-witnesses-20261030"
        manifest = json.loads((ROOT / name / "manifest.json").read_text())
        original = json.loads((ROOT / name / "verification/report.json").read_text())
        selected = {"F11", "F31"}
        expected = selected_cases(manifest, selected)
        mutations = [
            lambda r: r["suites"].pop(),
            lambda r: r["suites"].append(copy.deepcopy(r["suites"][0])),
            lambda r: r["suites"][0]["tests"][0].update(case="unexpected"),
            lambda r: r["suites"][0]["tests"][0].update(exit_code=0),
            lambda r: r["suites"][0]["tests"][0].update(timed_out=True),
            lambda r: r["suites"][0]["tests"][0].update(missing_expected_markers=["diagnostic"]),
            lambda r: r["suites"][0]["build"][0].update(exit_code=1),
            lambda r: r.update(revision="wrong"),
            lambda r: r.update(toolchain=None),
            lambda r: r.update(selected_findings=["F31"]),
            lambda r: r.update(executed_cases=0),
        ]
        for mutation in mutations:
            report = copy.deepcopy(original)
            mutation(report)
            self.assertFalse(report_matches(report, expected, selected))
        self.assertFalse(report_matches({}, expected, selected))
        self.assertFalse(report_matches(original, set(), selected))

    def test_shared_geometry_case_is_not_counted_twice(self):
        manifest = json.loads((ROOT / "jpeg2000-review-package-20261029/manifest.json").read_text())
        self.assertEqual(selected_cases(manifest, {"F14"}), selected_cases(manifest, {"F15"}))
        self.assertEqual(len(selected_cases(manifest, {"F14", "F15"})), 1)

    def test_transform_selection_requires_exact_values_and_counts(self):
        for selected, counts in [({"T1"}, "1 passed; 1 failed"), ({"T2"}, "0 passed; 1 failed"),
                                 ({"T1", "T2"}, "1 passed; 2 failed")]:
            selector, markers = transform_expectation(selected)
            self.assertTrue(selector.startswith("wave86_"))
            self.assertIn(counts, markers[0])
            self.assertTrue(any("left:" in marker for marker in markers))
            self.assertTrue(any("right:" in marker for marker in markers))
        with self.assertRaises(ValueError):
            transform_expectation(set())


if __name__ == "__main__":
    unittest.main()
