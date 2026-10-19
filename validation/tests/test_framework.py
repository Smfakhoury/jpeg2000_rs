import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

VALIDATION = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(VALIDATION))

import framework
from jsonschema import Draft202012Validator, ValidationError


class FrameworkTests(unittest.TestCase):
    def test_generation_is_deterministic_and_complete(self):
        first = framework.build_manifest(framework.generated_cases())
        second = framework.build_manifest(framework.generated_cases())
        self.assertEqual(first, second)
        formats = {entry["format"] for entry in first["entries"]}
        kinds = {entry["kind"] for entry in first["entries"]}
        self.assertEqual(formats, {"raw", "jp2", "jph", "jpx", "jpip"})
        self.assertEqual(
            kinds, {"positive", "malformed-prefix", "negative", "metamorphic"}
        )

    def test_committed_generated_corpus_has_valid_provenance(self):
        framework.generate_corpus(check=True)
        result = framework.verify_manifest()
        self.assertEqual(result["status"], "passed")

    def test_subject_output_normalization_is_stable(self):
        output = b"IMAGE\n1 1 1\n1\t1\t8\t0\tunsigned\t128\n"
        parsed = framework.parse_subject_output(output, b"", 0)
        self.assertTrue(parsed["accepted"])
        self.assertEqual(
            framework.normalized_hash(parsed), framework.normalized_hash(parsed)
        )

    def test_error_normalization_omits_unstable_detail(self):
        first = framework.parse_subject_output(
            b"ERR\tMalformedInput\tat byte 1\n", b"", 1
        )
        second = framework.parse_subject_output(
            b"ERR\tMalformedInput\tat byte 9\n", b"", 1
        )
        self.assertEqual(first, second)

    def test_binary_pnm_normalization(self):
        with tempfile.TemporaryDirectory(dir=framework.VALIDATION) as directory:
            path = Path(directory) / "output.pnm"
            path.write_bytes(b"P6\n1 1\n255\n\x01\x02\x03")
            parsed = framework.parse_pnm(path)
        self.assertEqual(
            [component["samples"] for component in parsed["image"]["components"]],
            [[1], [2], [3]],
        )

    def test_missing_external_oracles_are_explicit_in_config(self):
        config = framework.load_oracles()
        self.assertEqual(
            {entry["name"] for entry in config}, {"openjpeg", "grok", "jasper"}
        )
        self.assertTrue(all(entry["environment"] for entry in config))

    def test_release_schema_requires_evidence_bindings(self):
        schema = json.loads((framework.ROOT / "release-attestation.schema.json").read_text())
        conditional = schema["allOf"][0]["then"]["required"]
        self.assertEqual(set(conditional), {"bindings", "gates"})
        required_bindings = set(schema["properties"]["bindings"]["required"])
        self.assertTrue(
            {
                "source",
                "toolchains",
                "proof",
                "corpus",
                "validation",
                "budgets",
                "adjudication",
                "architecture",
                "approvals",
            }.issubset(required_bindings)
        )
        required_gates = set(schema["properties"]["gates"]["required"])
        self.assertIn("proof_boundary", required_gates)

    def test_normative_coverage_requires_concrete_contract_evidence(self):
        coverage = json.loads(
            (framework.ROOT / "normative-coverage.json").read_text()
        )
        self.assertEqual(
            framework.validate_normative_coverage(coverage)["status"], "passed"
        )
        coverage["requirements"][0]["contract_refs"][0]["symbol"] = "unchecked_shadow"
        with self.assertRaises(ValueError):
            framework.validate_normative_coverage(coverage)
        coverage = json.loads(
            (framework.ROOT / "normative-coverage.json").read_text()
        )
        coverage["unlinked_selected_part2_requirement_groups"] = [
            "orphan selected semantics"
        ]
        coverage["summary"]["unlinked_selected_part2_requirement_groups"] = 1
        with self.assertRaises(ValueError):
            framework.validate_normative_coverage(coverage)
        coverage = json.loads(
            (framework.ROOT / "normative-coverage.json").read_text()
        )
        coverage["unlinked_part1_requirement_groups"] = ["orphan semantics"]
        coverage["summary"]["unlinked_part1_requirement_groups"] = 1
        with self.assertRaises(ValueError):
            framework.validate_normative_coverage(coverage)

    def test_source_tree_revision_is_deterministic_and_separates_production(self):
        first = framework.build_source_tree_manifest()
        second = framework.build_source_tree_manifest()
        self.assertEqual(first, second)
        self.assertRegex(first["source_revision"], r"^sha256-tree-v1:[a-f0-9]{64}$")
        self.assertRegex(
            first["production_source_revision"],
            r"^sha256-tree-v1:[a-f0-9]{64}$",
        )
        self.assertTrue(
            all(
                entry["path"].startswith("src/")
                for entry in first["production_files"]
            )
        )

    def test_unsupported_boundary_audit_has_no_selected_profile_gap(self):
        audit = json.loads(
            (framework.ROOT / "unsupported-boundary-audit.json").read_text()
        )
        summary = audit["classification_summary"]
        self.assertEqual(summary["class_c_selected_profile_unsupported_paths_remaining"], 0)
        self.assertEqual(summary["contradictions"], 0)
        approved = {
            entry["approved_boundary"]
            for entry in audit["remaining_production_unsupported_constructions"]
        }
        self.assertEqual(
            approved,
            {"icc_execution", "unpinned_color_registry_values", "unselected_parts"},
        )
        matrix = json.loads((framework.ROOT / "support-matrix.json").read_text())
        closure = matrix["unsupported"]["selected_profile_closure_audit"]
        self.assertEqual(closure["class_c_remaining"], 0)
        self.assertEqual(closure["contradictions"], 0)

    def test_architecture_review_is_current_and_covers_phase_m(self):
        review = framework.build_architecture_review()
        self.assertEqual(review["status"], "passed")
        self.assertEqual(
            review["source_revision"],
            framework.build_source_tree_manifest()["source_revision"],
        )
        checklist = {entry["id"]: entry for entry in review["checklist"]}
        self.assertTrue(
            {
                "selected-parts-functional-refinement",
                "unsupported-boundary-audit",
                "complete-proof-dispositions",
                "lifetime-analysis-proof-boundary",
            }.issubset(checklist)
        )
        finding = checklist["selected-parts-functional-refinement"]["finding"]
        self.assertIn("Parts 1, 2, 9, and 15", finding)
        self.assertIn("54", finding)
        self.assertIn("160", finding)

    def test_disagreement_identifier_is_stable(self):
        disagreement = {
            "case_id": "case",
            "oracle": "oracle",
            "kind": "acceptance",
            "subject_sha256": "a" * 64,
            "oracle_sha256": "b" * 64,
            "status": "unadjudicated",
        }
        first = framework.disagreement_id(disagreement)
        disagreement["status"] = "adjudicated"
        self.assertEqual(first, framework.disagreement_id(disagreement))
        self.assertRegex(first, r"^oracle-disagreement-v1:[a-f0-9]{64}$")

    def test_release_schema_rejects_passed_status_with_blocked_gates(self):
        schema = json.loads(
            (framework.ROOT / "release-attestation.schema.json").read_text()
        )
        attestation = json.loads(
            (framework.ROOT / "release-attestation.json").read_text()
        )
        attestation["status"] = "passed"
        attestation["result"]["publish_allowed"] = True
        with self.assertRaises(ValidationError):
            Draft202012Validator(schema).validate(attestation)

    def test_official_vectors_execute_only_from_hash_verified_external_root(self):
        with tempfile.TemporaryDirectory(dir=framework.VALIDATION) as directory:
            root = Path(directory)
            archive = root / "suite.bin"
            vector = root / "vector.j2k"
            harness = root / "harness.py"
            archive.write_bytes(b"official-suite-archive-placeholder")
            vector.write_bytes(b"official-vector-placeholder")
            harness.write_text(
                "#!/usr/bin/env python3\n"
                "print('ERR\\tMalformedInput\\texpected rejection')\n"
                "raise SystemExit(1)\n"
            )
            harness.chmod(0o700)
            manifest = {
                "$schema": str(
                    framework.VALIDATION / "official-vector-manifest.schema.json"
                ),
                "schema_version": 1,
                "profile_id": framework.PROFILE_ID,
                "suite": {
                    "source_url": framework._official_source_url(),
                    "acquired_at": "2026-10-18T00:00:00Z",
                    "access_status": "authorized-internal",
                    "license": {
                        "status": "approved-internal-use",
                        "reviewed_at": "2026-10-18T00:00:00Z",
                        "redistribution_permitted": False,
                        "basis": "test fixture",
                    },
                    "archive": {
                        "path": archive.name,
                        "bytes": archive.stat().st_size,
                        "sha256": framework.sha256_file(archive),
                    },
                    "selection": {
                        "complete": True,
                        "applicable_vector_count": 1,
                        "basis": "test fixture",
                    },
                },
                "entries": [
                    {
                        "id": "private-fixture-id",
                        "identifier_public": False,
                        "path": vector.name,
                        "sha256": framework.sha256_file(vector),
                        "bytes": vector.stat().st_size,
                        "format": "raw",
                        "operation": "decode",
                        "applicable": True,
                        "expected": {
                            "accepted": False,
                            "category": "MalformedInput",
                        },
                    }
                ],
            }
            manifest_path = root / "official-vector-manifest.json"
            manifest_path.write_text(json.dumps(manifest))
            result_path = root / "result.json"
            with mock.patch.object(
                framework, "OFFICIAL_RESULT_PATH", result_path
            ):
                result = framework.run_official_vectors(
                    executable=harness,
                    vector_root=root,
                    manifest_path=manifest_path,
                )
            self.assertEqual(result["status"], "passed")
            self.assertEqual(result["summary"]["passed"], 1)
            self.assertNotIn("public_id", result["records"][0])
            self.assertFalse(any(root.as_posix() in str(value) for value in result.values()))


if __name__ == "__main__":
    unittest.main()
