import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


VALIDATION = Path(__file__).resolve().parents[1]
CONTROLLER = VALIDATION / "check-model-review-hold.py"
spec = importlib.util.spec_from_file_location("model_review_hold", CONTROLLER)
model_review_hold = importlib.util.module_from_spec(spec)
spec.loader.exec_module(model_review_hold)


class ModelReviewHoldTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="jpeg2000-model-hold-")
        self.addCleanup(directory.cleanup)
        self.repository = Path(directory.name)
        self.git("init", "--initial-branch=main")
        self.git("config", "user.name", "Model hold test")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "core.autocrlf", "false")
        self.write("src/lib.rs", "pub fn example() {}\n")
        self.write("specs/README.md", "# Guide\n")
        self.base = self.commit()

    def git(self, *arguments):
        return subprocess.run(
            ["git", "-C", str(self.repository), *arguments],
            check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        ).stdout.decode("utf-8").strip()

    def write(self, name, content):
        path = self.repository / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def commit(self):
        self.git("add", "--all")
        self.git("commit", "--message", "Fixture")
        return self.git("rev-parse", "HEAD")

    def run_controller(self, base, head):
        return subprocess.run(
            [sys.executable, str(CONTROLLER), "--repository", str(self.repository), "--", base, head],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )

    def test_surface_includes_targets_dependencies_and_controller(self):
        for path in (
            "src/lib.rs", "src/table.bin", "specs/new.rs", "specs/table.json",
            "proofs/shared/view.rs", "examples/decoder.rs", "other/model.rs",
            ".github/workflows/model-review-hold.yml", ".github/new-workflow.yml",
            ".cargo/config.toml", "Cargo.toml", "Cargo.lock", "fuzz/Cargo.toml",
            "rust-toolchain.toml", "rust-toolchain", "validation/framework.py",
            ".gitattributes", ".gitmodules",
            "validation/check-model-review-hold.py", "validation/normative-coverage.json",
            "validation/tests/test_model_review_hold.py",
        ):
            with self.subTest(path=path):
                self.assertTrue(model_review_hold.held_path(path))
        for path in ("README.md", "PROOFS.md", "specs/README.md", "specs/AUDIT_REFERENCE.md",
                     "validation/VALIDATION.md", "proofs/README.md"):
            with self.subTest(path=path):
                self.assertFalse(model_review_hold.held_path(path))

    def test_documentation_change_passes(self):
        self.write("specs/README.md", "# Improved guide\n")
        self.write("specs/AUDIT_REFERENCE.md", "# Existing detail\n")
        result = self.run_controller(self.base, self.commit())
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("PASS: no unapproved changes", result.stdout)

    def test_source_change_is_held(self):
        self.write("src/lib.rs", "pub fn changed() {}\n")
        result = self.run_controller(self.base, self.commit())
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("'src/lib.rs'", result.stdout)

    def test_new_target_is_held(self):
        self.write("specs/new.rs", "pub spec fn weakened() -> bool { true }\n")
        self.assertEqual(
            model_review_hold.held_changes(self.repository, self.base, self.commit()),
            ["specs/new.rs"],
        )

    def test_source_deletion_is_held(self):
        (self.repository / "src/lib.rs").unlink()
        self.assertEqual(
            model_review_hold.held_changes(self.repository, self.base, self.commit()),
            ["src/lib.rs"],
        )

    def test_rename_to_documentation_does_not_hide_source_removal(self):
        self.git("mv", "src/lib.rs", "source.md")
        self.assertEqual(
            model_review_hold.held_changes(self.repository, self.base, self.commit()),
            ["src/lib.rs"],
        )

    def test_candidate_controller_cannot_approve_its_own_model_change(self):
        self.write("validation/check-model-review-hold.py", "raise SystemExit(0)\n")
        self.write("src/lib.rs", "pub fn changed() {}\n")
        result = self.run_controller(self.base, self.commit())
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("'src/lib.rs'", result.stdout)
        self.assertIn("'validation/check-model-review-hold.py'", result.stdout)

    def test_workflow_only_change_is_held(self):
        self.write(".github/workflows/model-review-hold.yml", "jobs: {}\n")
        result = self.run_controller(self.base, self.commit())
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("'.github/workflows/model-review-hold.yml'", result.stdout)

    def test_unrelated_base_progress_is_not_a_proposed_change(self):
        self.write("specs/README.md", "# Improved guide\n")
        head = self.commit()
        self.git("checkout", "-b", "target-progress", self.base)
        self.write("src/lib.rs", "pub fn unrelated_base_change() {}\n")
        base = self.commit()
        result = self.run_controller(base, head)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_uncommitted_files_do_not_replace_committed_evidence(self):
        self.write("src/lib.rs", "pub fn changed() {}\n")
        head = self.commit()
        self.write("src/lib.rs", "pub fn example() {}\n")
        result = self.run_controller(self.base, head)
        self.assertEqual(result.returncode, 1, result.stderr)

    def test_empty_change_passes(self):
        result = self.run_controller(self.base, self.base)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_incomplete_or_option_shaped_commit_ids_are_errors(self):
        for value in (self.base[:8], "--no-index", "0" * 39, "z" * 40):
            with self.subTest(value=value):
                result = self.run_controller(value, self.base)
                self.assertEqual(result.returncode, 2)
                self.assertIn("expected a full SHA-1 commit ID", result.stderr)

    def test_missing_history_is_an_error_not_an_allowed_change(self):
        result = self.run_controller(self.base, "0" * 40)
        self.assertEqual(result.returncode, 2)
        self.assertIn("could not read", result.stderr)

    def test_unrelated_histories_are_an_error_not_an_allowed_change(self):
        self.git("checkout", "--orphan", "unrelated")
        self.write("specs/README.md", "# Unrelated root commit\n")
        head = self.commit()
        result = self.run_controller(self.base, head)
        self.assertEqual(result.returncode, 2)
        self.assertIn("could not read", result.stderr)

    def test_multiple_merge_bases_are_an_error(self):
        tree = self.git("rev-parse", self.base + "^{tree}")
        left = self.git("commit-tree", tree, "-p", self.base, "-m", "Left")
        right = self.git("commit-tree", tree, "-p", self.base, "-m", "Right")
        base = self.git("commit-tree", tree, "-p", left, "-p", right, "-m", "Left merge")
        head = self.git("commit-tree", tree, "-p", right, "-p", left, "-m", "Right merge")
        result = self.run_controller(base, head)
        self.assertEqual(result.returncode, 2)
        self.assertIn("exactly one merge base", result.stderr)

    def test_git_paths_are_nul_delimited_not_line_delimited(self):
        name = "specs/name\npart.rs"
        with patch.object(model_review_hold, "git", side_effect=[
            self.base.encode("ascii") + b"\n",
            name.encode("utf-8") + b"\0specs/README.md\0",
        ]):
            self.assertEqual(
                model_review_hold.held_changes(self.repository, self.base, self.base),
                [name],
            )

    def reviewed_extraction_fixture(self):
        originals = {
            "src/codestream.rs": "fn value() -> u8 { 1 }\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn example() { assert_eq!(super::value(), 1); }\n}\n",
            "validation/normative-coverage.json": '{"test_file":"src/codestream.rs"}\n',
            "validation/expected-package-files.txt": "src/codestream.rs\n",
        }
        proposed = {
            "src/codestream.rs": "fn value() -> u8 { 1 }\n#[cfg(test)]\nmod tests;\n",
            "src/codestream/tests.rs": "#[test]\nfn example() { assert_eq!(super::value(), 1); }\n",
            "validation/normative-coverage.json": '{"test_file":"src/codestream/tests.rs"}\n',
            "validation/expected-package-files.txt": "src/codestream.rs\nsrc/codestream/tests.rs\n",
        }
        for path, content in originals.items():
            self.write(path, content)
        base = self.commit()
        for path, content in proposed.items():
            self.write(path, content)
        head = self.commit()
        record = {
            path: (
                self.git("rev-parse", f"{base}:{path}") if path in originals else None,
                self.git("rev-parse", f"{head}:{path}"),
            )
            for path in proposed
        }
        return base, head, record

    def test_exact_reviewed_test_extraction_passes(self):
        base, head, record = self.reviewed_extraction_fixture()
        with patch.object(model_review_hold, "REVIEWED_TEST_EXTRACTIONS", (record,)):
            self.assertEqual(model_review_hold.held_changes(self.repository, base, head), [])

    def test_unreviewed_test_extraction_is_held(self):
        base, head, record = self.reviewed_extraction_fixture()
        with patch.object(model_review_hold, "REVIEWED_TEST_EXTRACTIONS", ()):
            self.assertEqual(
                set(model_review_hold.held_changes(self.repository, base, head)),
                set(record),
            )

    def test_incomplete_reviewed_move_is_held(self):
        base, _, record = self.reviewed_extraction_fixture()
        self.git("checkout", "-b", "incomplete", base)
        self.write("src/codestream.rs", "fn value() -> u8 { 1 }\n#[cfg(test)]\nmod tests;\n")
        self.write("src/codestream/tests.rs", "#[test]\nfn example() {}\n")
        with patch.object(model_review_hold, "REVIEWED_TEST_EXTRACTIONS", (record,)):
            self.assertTrue(model_review_hold.held_changes(self.repository, base, self.commit()))

    def test_changed_reviewed_test_body_is_held(self):
        base, _, record = self.reviewed_extraction_fixture()
        self.write("src/codestream/tests.rs", "#[test]\nfn example() {}\n")
        with patch.object(model_review_hold, "REVIEWED_TEST_EXTRACTIONS", (record,)):
            self.assertIn(
                "src/codestream/tests.rs",
                model_review_hold.held_changes(self.repository, base, self.commit()),
            )

    def test_changed_runtime_in_reviewed_move_is_held(self):
        base, _, record = self.reviewed_extraction_fixture()
        self.write("src/codestream.rs", "fn value() -> u8 { 0 }\n#[cfg(test)]\nmod tests;\n")
        with patch.object(model_review_hold, "REVIEWED_TEST_EXTRACTIONS", (record,)):
            self.assertIn(
                "src/codestream.rs",
                model_review_hold.held_changes(self.repository, base, self.commit()),
            )

    def test_reviewed_move_does_not_authorize_another_model(self):
        base, _, record = self.reviewed_extraction_fixture()
        self.write("specs/new.rs", "pub spec fn weakened() -> bool { true }\n")
        with patch.object(model_review_hold, "REVIEWED_TEST_EXTRACTIONS", (record,)):
            self.assertIn(
                "specs/new.rs",
                model_review_hold.held_changes(self.repository, base, self.commit()),
            )

    def test_reviewed_move_cannot_edit_its_controller(self):
        base, _, record = self.reviewed_extraction_fixture()
        self.write("validation/check-model-review-hold.py", "raise SystemExit(0)\n")
        with patch.object(model_review_hold, "REVIEWED_TEST_EXTRACTIONS", (record,)):
            self.assertIn(
                "validation/check-model-review-hold.py",
                model_review_hold.held_changes(self.repository, base, self.commit()),
            )

    def test_reviewed_move_checks_regular_file_modes(self):
        base, _, record = self.reviewed_extraction_fixture()
        self.git("update-index", "--chmod=+x", "src/codestream/tests.rs")
        self.git("commit", "--message", "Change mode")
        with patch.object(model_review_hold, "REVIEWED_TEST_EXTRACTIONS", (record,)):
            self.assertIn(
                "src/codestream/tests.rs",
                model_review_hold.held_changes(self.repository, base, self.git("rev-parse", "HEAD")),
            )

    def test_reviewed_move_is_bound_to_original_source(self):
        base, head, record = self.reviewed_extraction_fixture()
        self.git("checkout", "-b", "different-original", base)
        self.write("src/codestream.rs", "fn value() -> u8 { 2 }\n")
        other_base = self.commit()
        for path in record:
            self.write(path, self.git("show", f"{head}:{path}") + "\n")
        other_head = self.commit()
        with patch.object(model_review_hold, "REVIEWED_TEST_EXTRACTIONS", (record,)):
            self.assertTrue(model_review_hold.held_changes(self.repository, other_base, other_head))

    def test_reviewed_move_requires_the_new_test_file(self):
        base, _, record = self.reviewed_extraction_fixture()
        (self.repository / "src/codestream/tests.rs").unlink()
        with patch.object(model_review_hold, "REVIEWED_TEST_EXTRACTIONS", (record,)):
            self.assertTrue(model_review_hold.held_changes(self.repository, base, self.commit()))

    def reviewed_consumer_fixture(self):
        workflow = ".github/workflows/ci.yml"
        self.write(workflow, "jobs: original\n")
        base = self.commit()
        proposed = {
            workflow: "jobs: checked-package-consumer\n",
            "validation/image-consumer-probe/.gitignore": "/target/\n",
            "validation/image-consumer-probe/Cargo.toml": "[package]\nname = 'probe'\n",
            "validation/image-consumer-probe/Cargo.lock": "version = 4\n",
            "validation/image-consumer-probe/src/lib.rs": "mod decoder;\n",
            "validation/image-consumer-probe/src/decoder.rs": "pub struct Decoder;\n",
            "validation/image-consumer-probe/tests/adapter.rs": "#[test]\nfn example() {}\n",
            "validation/image-consumer-probe/tests/fixtures/gray.jp2": "gray fixture\n",
            "validation/image-consumer-probe/tests/fixtures/rgb.jp2": "rgb fixture\n",
        }
        for path, content in proposed.items():
            self.write(path, content)
        head = self.commit()
        record = {
            path: (
                self.git("rev-parse", f"{base}:{path}") if path == workflow else None,
                self.git("rev-parse", f"{head}:{path}"),
            )
            for path in proposed
        }
        return base, head, record

    def test_exact_reviewed_consumer_with_guide_change_passes(self):
        base, _, record = self.reviewed_consumer_fixture()
        self.write("RELEASING.md", "# Consumer scope\n")
        with patch.object(model_review_hold, "REVIEWED_CONSUMER_PROBES", (record,)):
            self.assertEqual(model_review_hold.held_changes(self.repository, base, self.commit()), [])

    def test_unreviewed_consumer_is_held(self):
        base, head, record = self.reviewed_consumer_fixture()
        with patch.object(model_review_hold, "REVIEWED_CONSUMER_PROBES", ()):
            self.assertEqual(
                set(model_review_hold.held_changes(self.repository, base, head)),
                set(record),
            )

    def test_reviewed_consumer_cannot_change_its_source(self):
        base, _, record = self.reviewed_consumer_fixture()
        path = "validation/image-consumer-probe/src/decoder.rs"
        self.write(path, "pub struct Changed;\n")
        with patch.object(model_review_hold, "REVIEWED_CONSUMER_PROBES", (record,)):
            self.assertIn(path, model_review_hold.held_changes(self.repository, base, self.commit()))

    def test_reviewed_consumer_cannot_change_its_workflow(self):
        base, _, record = self.reviewed_consumer_fixture()
        self.write(".github/workflows/ci.yml", "jobs: changed\n")
        with patch.object(model_review_hold, "REVIEWED_CONSUMER_PROBES", (record,)):
            self.assertIn(
                ".github/workflows/ci.yml",
                model_review_hold.held_changes(self.repository, base, self.commit()),
            )

    def test_reviewed_consumer_requires_all_files(self):
        base, _, record = self.reviewed_consumer_fixture()
        (self.repository / "validation/image-consumer-probe/Cargo.lock").unlink()
        with patch.object(model_review_hold, "REVIEWED_CONSUMER_PROBES", (record,)):
            self.assertTrue(model_review_hold.held_changes(self.repository, base, self.commit()))

    def test_reviewed_consumer_does_not_authorize_another_model(self):
        base, _, record = self.reviewed_consumer_fixture()
        self.write("specs/new.rs", "pub spec fn weakened() -> bool { true }\n")
        with patch.object(model_review_hold, "REVIEWED_CONSUMER_PROBES", (record,)):
            self.assertIn(
                "specs/new.rs",
                model_review_hold.held_changes(self.repository, base, self.commit()),
            )

    def test_reviewed_consumer_cannot_edit_its_controller(self):
        base, _, record = self.reviewed_consumer_fixture()
        self.write("validation/check-model-review-hold.py", "raise SystemExit(0)\n")
        with patch.object(model_review_hold, "REVIEWED_CONSUMER_PROBES", (record,)):
            self.assertIn(
                "validation/check-model-review-hold.py",
                model_review_hold.held_changes(self.repository, base, self.commit()),
            )

    def test_reviewed_consumer_checks_regular_file_modes(self):
        base, _, record = self.reviewed_consumer_fixture()
        path = "validation/image-consumer-probe/src/decoder.rs"
        self.git("update-index", "--chmod=+x", path)
        self.git("commit", "--message", "Change mode")
        with patch.object(model_review_hold, "REVIEWED_CONSUMER_PROBES", (record,)):
            self.assertIn(
                path,
                model_review_hold.held_changes(self.repository, base, self.git("rev-parse", "HEAD")),
            )

    def test_reviewed_consumer_is_bound_to_original_workflow(self):
        base, head, record = self.reviewed_consumer_fixture()
        self.git("checkout", "-b", "different-original", base)
        self.write(".github/workflows/ci.yml", "jobs: different-original\n")
        other_base = self.commit()
        for path in record:
            self.write(path, self.git("show", f"{head}:{path}") + "\n")
        with patch.object(model_review_hold, "REVIEWED_CONSUMER_PROBES", (record,)):
            self.assertTrue(
                model_review_hold.held_changes(self.repository, other_base, self.commit()),
            )

    def test_separate_reviewed_records_do_not_authorize_their_combination(self):
        base, _, extraction = self.reviewed_extraction_fixture()
        _, head, consumer = self.reviewed_consumer_fixture()
        with patch.object(model_review_hold, "REVIEWED_TEST_EXTRACTIONS", (extraction,)), \
                patch.object(model_review_hold, "REVIEWED_CONSUMER_PROBES", (consumer,)):
            self.assertTrue(model_review_hold.held_changes(self.repository, base, head))


if __name__ == "__main__":
    unittest.main()
