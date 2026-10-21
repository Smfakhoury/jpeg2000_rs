import copy
import importlib.util
import json
import shutil
import subprocess
import sys
import unittest
import uuid
from pathlib import Path

VALIDATION = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(VALIDATION))

import framework

spec = importlib.util.spec_from_file_location("proof_coverage", VALIDATION / "generate-proof-coverage.py")
proof_coverage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proof_coverage)


class ProofCoverageTests(unittest.TestCase):
    def setUp(self):
        self.work = framework.RESULTS_DIR / "test-work" / uuid.uuid4().hex
        (self.work / "src").mkdir(parents=True)
        self.addCleanup(shutil.rmtree, self.work)
        (self.work / "src/lib.rs").write_text("pub fn fixture() {}\n")
        self.result = {
            "verification-results": {
                "success": True, "errors": 0, "verified": 1,
                "is-verifying-entire-crate": True,
                "encountered-error": False, "encountered-vir-error": False,
            },
            "verus": {
                "version": "fixture", "commit": proof_coverage.EXPECTED_VERUS_COMMIT,
                "profile": "release", "toolchain": "fixture",
            },
            "func-details": {"jpeg2000_rs::fixture": {"failed_proof_notes": [], "obligation_proof_notes": []}},
        }

    def test_valid_observed_project_run_has_honest_boundary(self):
        report = proof_coverage.generate_coverage(self.result, self.work)
        self.assertEqual(report["results"]["verified_project_functions"], 1)
        self.assertIn("does not verify lifetimes", report["known_proof_boundary"]["lifetime_analysis"])
        self.assertIn("No exhaustive", report["scope"])
        framework.validate_schema(VALIDATION / "proof-coverage.schema.json", report)
        self.assertEqual(report["verification_entrypoint"], "cargo verus verify")
        self.assertEqual(report["crate_name"], "jpeg2000_rs")
        self.assertEqual(report["known_proof_boundary"]["termination"], [])

    def test_empty_and_malformed_results_fail_closed(self):
        for result in (None, [], {}, {"verification-results": []}, {**self.result, "func-details": {}}):
            with self.subTest(result=result), self.assertRaises(ValueError):
                proof_coverage.generate_coverage(result, self.work)
        for key, value in (
            ("success", False), ("success", "true"), ("errors", 1), ("errors", False),
            ("verified", 0), ("verified", "1"), ("is-verifying-entire-crate", False),
            ("encountered-error", True), ("encountered-vir-error", True),
        ):
            result = copy.deepcopy(self.result)
            result["verification-results"][key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                proof_coverage.generate_coverage(result, self.work)

    def test_partial_wrong_commit_missing_project_or_trust_evidence_rejected(self):
        mutations = [
            lambda r: r["verus"].update(commit="0" * 40),
            lambda r: r.update({"func-details": {"vstd::fixture": {"failed_proof_notes": []}}}),
            lambda r: r.update({"func-details": {"jpeg2000_rs::fixture": {}}}),
            lambda r: r.update({"func-details": {"jpeg2000_rs::fixture": {"failed_proof_notes": ["failed"]}}}),
            lambda r: r.update({"func-details": {"jpeg2000_rs::fixture": {"failed_proof_notes": [], "obligation_proof_notes": [], "trusted": True}}}),
            lambda r: r.update({"func-details": {"jpeg2000_rs::fixture": {"failed_proof_notes": []}}}),
            lambda r: r.update({"func-details": {"jpeg2000_rs::fixture": {"failed_proof_notes": [], "obligation_proof_notes": ["unexpected trust"]}}}),
            lambda r: r.update({"func-details": {"jpeg2000_rs::fixture": None}}),
        ]
        for mutate in mutations:
            result = copy.deepcopy(self.result)
            mutate(result)
            with self.subTest(mutation=mutate), self.assertRaises(ValueError):
                proof_coverage.generate_coverage(result, self.work)

    def test_unmapped_termination_bypass_and_trusted_escapes_rejected(self):
        for escape in (
            "#[verifier::exec_allows_no_decreases_clause]",
            "#[verifier :: exec_allows_no_decreases_clause]",
            "#[verifier::external_body]",
            "#[verifier :: external]",
            "#[verifier::verify(false)]",
            "assume (true);", "assume!(true);", "admit ();",
            "assume_specification[fixture]();", "axiom fn fixture();",
        ):
            (self.work / "src/lib.rs").write_text(escape + "\npub fn fixture() {}\n")
            with self.subTest(escape=escape), self.assertRaises(ValueError):
                proof_coverage.generate_coverage(self.result, self.work)

    def test_missing_source_root_rejected(self):
        (self.work / "src/lib.rs").unlink()
        with self.assertRaisesRegex(ValueError, "source root"):
            proof_coverage.generate_coverage(self.result, self.work)

    def test_cargo_success_without_root_json_is_not_verification(self):
        for text in (
            "",
            "Finished dev profile\n",
            "WARNING: no crates opted into verification\nFinished dev profile\n",
            "verification results:: 2060 verified, 0 errors\n",
            "{}",
            '{"verification-results":',
            json.dumps(self.result) + "\n" + json.dumps(self.result),
        ):
            with self.subTest(text=text[:90]), self.assertRaises(ValueError):
                proof_coverage.load_cargo_verus_result(text)

    def test_cargo_output_extracts_one_root_result_after_dependency_summary(self):
        text = "verification results:: 2060 verified, 0 errors\n" + json.dumps(self.result, indent=2)
        self.assertEqual(proof_coverage.load_cargo_verus_result(text), self.result)

    def test_dependency_legacy_or_other_crate_cannot_be_root_evidence(self):
        for namespace in ("vstd", "lib", "other_project"):
            result = copy.deepcopy(self.result)
            result["func-details"] = {
                namespace + "::fixture": {"failed_proof_notes": [], "obligation_proof_notes": []},
            }
            with self.subTest(namespace=namespace), self.assertRaisesRegex(ValueError, "no jpeg2000_rs"):
                proof_coverage.generate_coverage(result, self.work)

    def test_internal_specs_are_scanned_and_hashed(self):
        internal = self.work / "proofs/internal_specs.rs"
        internal.parent.mkdir()
        internal.write_text("pub fn invariant_fixture() {}\n")
        report = proof_coverage.generate_coverage(self.result, self.work)
        self.assertEqual(report["source_hashes"]["proofs/internal_specs.rs"], framework.sha256_file(internal))
        internal.write_text("#[verifier::exec_allows_no_decreases_clause]\npub fn invariant_fixture() {}\n")
        with self.assertRaisesRegex(ValueError, "forbidden proof escape"):
            proof_coverage.generate_coverage(self.result, self.work)

    def test_project_opts_in_without_default_decoder_dependencies(self):
        completed = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"],
            cwd=framework.ROOT, capture_output=True, text=True, check=True,
        )
        package = next(
            p for p in json.loads(completed.stdout)["packages"]
            if p["name"] == proof_coverage.EXPECTED_CRATE
        )
        self.assertIs(package["metadata"]["verus"]["verify"], True)
        self.assertEqual(package["features"]["default"], [])
        self.assertEqual(package["features"]["verus"], ["dep:vstd"])
        dependency = next(d for d in package["dependencies"] if d["name"] == "vstd")
        self.assertTrue(dependency["optional"])
        self.assertEqual(dependency["req"], "=0.0.0-2026-10-04-0306")
        tree = subprocess.run(
            ["cargo", "tree", "--locked", "--edges", "normal", "--prefix", "none"],
            cwd=framework.ROOT, capture_output=True, text=True, check=True,
        )
        self.assertEqual(len(tree.stdout.strip().splitlines()), 1)

    def test_cli_redirects_output_and_invalid_input_does_not_write_report(self):
        raw = self.work / "cargo-verus-output.log"
        raw.write_text("verification results:: 2060 verified, 0 errors\n" + json.dumps(self.result))
        output = self.work / "reports"
        command = [
            sys.executable, str(VALIDATION / "generate-proof-coverage.py"),
            str(raw), "--root", str(self.work), "--output-dir", str(output),
        ]
        completed = subprocess.run(command, cwd=framework.ROOT, capture_output=True, text=True)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        report = json.loads((output / "proof-coverage.json").read_text())
        normalized = output / "verus-result.json"
        self.assertEqual(json.loads(normalized.read_text()), self.result)
        self.assertEqual(report["verus_result_sha256"], framework.sha256_file(normalized))
        self.assertEqual(report["cargo_verus_output_sha256"], framework.sha256_file(raw))
        self.assertTrue((output / report["$schema"]).is_file())
        (output / "proof-coverage.json").unlink()
        normalized.unlink()
        raw.write_text("{}")
        completed = subprocess.run(command, cwd=framework.ROOT, capture_output=True, text=True)
        self.assertEqual(completed.returncode, 2)
        self.assertFalse((output / "proof-coverage.json").exists())
        self.assertFalse(normalized.exists())


if __name__ == "__main__":
    unittest.main()
