import importlib.util
import io
import json
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "packaged_source", Path(__file__).resolve().parents[1] / "check-packaged-source.py",
)
packaged_source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packaged_source)


class PackagedSourceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="jpeg2000-package-binding-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.git("init", "--initial-branch=main")
        self.git("config", "user.name", "Package binding test")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "core.autocrlf", "false")
        self.files = {
            ".gitignore": b"target/\n",
            "Cargo.toml": b'[package]\nname="jpeg2000_rs"\nversion="0.1.0"\n',
            "src/lib.rs": b"pub fn fixture() {}\n",
            "specs/mod.rs": b"// model fixture\n",
            "proofs/mod.rs": b"// proof fixture\n",
        }
        for name, data in self.files.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        self.git("add", "--all")
        self.git("commit", "--message", "Fixture")
        self.commit = self.git("rev-parse", "HEAD")
        self.package = self.root / "target/package/jpeg2000_rs-0.1.0"
        self.archive = self.package.parent / (self.package.name + ".crate")
        self.package.mkdir(parents=True)
        self.members = {name: data for name, data in self.files.items() if name != ".gitignore"}
        self.members["Cargo.toml.orig"] = self.files["Cargo.toml"]
        self.members[".cargo_vcs_info.json"] = json.dumps({
            "git": {"sha1": self.commit}, "path_in_vcs": "",
        }).encode()
        self.write_archive()

    def git(self, *arguments):
        return subprocess.check_output(
            ["git", "-C", str(self.root), *arguments], stderr=subprocess.PIPE, text=True,
        ).strip()

    def write_archive(self, extra=None):
        with tarfile.open(self.archive, "w:gz") as archive:
            for name, data in self.members.items():
                path = self.package / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
                member = tarfile.TarInfo(self.package.name + "/" + name)
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))
            if extra is not None:
                archive.addfile(extra)

    def snapshot(self):
        return packaged_source.snapshot(self.root, self.package, self.archive)

    def test_exact_package_binds_to_clean_source(self):
        result = self.snapshot()
        self.assertEqual(result["source_commit"], self.commit)
        self.assertEqual(len(result["rust_source_hashes"]), 3)
        packaged_source.verify_snapshot(self.root, self.package, self.archive, result)

    def test_package_path_preserves_the_full_semantic_version(self):
        metadata = {
            "packages": [{"name": "jpeg2000_rs", "version": "0.1.0",
                          "manifest_path": str(self.root / "Cargo.toml")}],
            "target_directory": str(self.root / "target"),
        }
        with patch.object(packaged_source.subprocess, "check_output",
                          return_value=json.dumps(metadata).encode()):
            package, archive = packaged_source.package_paths(self.root)
        self.assertEqual(package, self.package)
        self.assertEqual(archive.name, "jpeg2000_rs-0.1.0.crate")

    def test_changed_unpacked_source_is_rejected(self):
        (self.package / "src/lib.rs").write_text("pub fn changed() {}\n")
        with self.assertRaisesRegex(ValueError, "differs from its archive"):
            self.snapshot()

    def test_missing_model_in_archive_is_rejected(self):
        del self.members["specs/mod.rs"]
        (self.package / "specs/mod.rs").unlink()
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "Rust sources differ"):
            self.snapshot()

    def test_extra_rust_source_is_rejected(self):
        self.members["src/extra.rs"] = b"pub fn extra() {}\n"
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "Rust sources differ"):
            self.snapshot()

    def test_ignored_uncommitted_rust_is_not_an_authoritative_source(self):
        (self.root / ".git/info/exclude").write_text("src/extra.rs\n")
        (self.root / "src/extra.rs").write_text("pub fn extra() {}\n")
        self.members["src/extra.rs"] = b"pub fn extra() {}\n"
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "Rust sources differ"):
            self.snapshot()

    def test_assume_unchanged_does_not_hide_rust_mutation(self):
        self.git("update-index", "--assume-unchanged", "src/lib.rs")
        (self.root / "src/lib.rs").write_text("pub fn changed() {}\n")
        with self.assertRaisesRegex(ValueError, "committed source"):
            self.snapshot()

    def test_wrong_original_manifest_is_rejected(self):
        self.members["Cargo.toml.orig"] = b"changed manifest\n"
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "original manifest differs"):
            self.snapshot()

    def test_wrong_vcs_identity_is_rejected(self):
        self.members[".cargo_vcs_info.json"] = json.dumps({
            "git": {"sha1": "0" * 40}, "path_in_vcs": "",
        }).encode()
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "VCS identity"):
            self.snapshot()

    def test_dirty_source_is_rejected(self):
        (self.root / "src/lib.rs").write_text("pub fn changed() {}\n")
        with self.assertRaisesRegex(ValueError, "must be clean"):
            self.snapshot()

    def test_path_traversal_is_rejected(self):
        extra = tarfile.TarInfo(self.package.name + "/../escape")
        self.write_archive(extra)
        with self.assertRaisesRegex(ValueError, "unexpected path"):
            self.snapshot()

    def test_symbolic_link_member_is_rejected(self):
        extra = tarfile.TarInfo(self.package.name + "/linked")
        extra.type = tarfile.SYMTYPE
        extra.linkname = "/outside"
        self.write_archive(extra)
        with self.assertRaisesRegex(ValueError, "regular files"):
            self.snapshot()

    def test_inputs_cannot_change_between_prepare_and_verify(self):
        before = self.snapshot()
        self.members["README.md"] = b"changed packaged documentation\n"
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "changed during replay"):
            packaged_source.verify_snapshot(self.root, self.package, self.archive, before)


if __name__ == "__main__":
    unittest.main()
