import argparse
import hashlib
import json
import os
import subprocess
import sys
import tarfile
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parents[1]
BINDING = ROOT / "target/validation/packaged-source-inputs.json"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def git(root: Path, *arguments: str) -> str:
    return subprocess.check_output(["git", "-C", str(root), *arguments], text=True).strip()


def package_paths(root: Path) -> tuple[Path, Path]:
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
        cwd=root, text=True,
    ))
    packages = [package for package in metadata["packages"]
                if Path(package["manifest_path"]).resolve() == (root / "Cargo.toml").resolve()]
    if len(packages) != 1 or packages[0]["name"] != "jpeg2000-decoder":
        raise ValueError("expected exactly the production jpeg2000-decoder package")
    target = Path(metadata["target_directory"])
    if target.resolve() != (root / "target").resolve():
        raise ValueError("packaged verification requires the repository-local target directory")
    package = target / "package" / f"{packages[0]['name']}-{packages[0]['version']}"
    return package, package.parent / (package.name + ".crate")


def snapshot(root: Path, package: Path, archive: Path) -> dict:
    commit = git(root, "rev-parse", "HEAD")
    if git(root, "status", "--porcelain", "--untracked-files=normal"):
        raise ValueError("repository source must be clean before packaged verification")
    vcs = json.loads((package / ".cargo_vcs_info.json").read_text())
    if vcs["git"]["sha1"] != commit or vcs["git"].get("dirty", False) is not False:
        raise ValueError("packaged VCS identity does not describe the clean source candidate")
    top = Path(git(root, "rev-parse", "--show-toplevel"))
    relative = root.resolve().relative_to(top.resolve()).as_posix()
    if vcs["path_in_vcs"] != ("" if relative == "." else relative):
        raise ValueError("packaged VCS subdirectory differs from the source crate")
    hashes = {}
    with tarfile.open(archive, "r:gz") as packed:
        for member in packed.getmembers():
            path = PurePosixPath(member.name)
            if path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] != package.name:
                raise ValueError("unexpected path in packaged archive")
            if member.isdir():
                continue
            if not member.isfile() or len(path.parts) < 2:
                raise ValueError("packaged archive must contain regular files")
            name = PurePosixPath(*path.parts[1:]).as_posix()
            if name in hashes:
                raise ValueError("duplicate packaged file")
            extracted = package / name
            stream = packed.extractfile(member)
            if stream is None or extracted.is_symlink() or not extracted.is_file():
                raise ValueError(f"missing regular packaged file: {name}")
            data = stream.read()
            if extracted.read_bytes() != data:
                raise ValueError(f"unpacked file differs from its archive: {name}")
            hashes[name] = sha256(data)
    prefix = "" if relative == "." else relative + "/"
    original_manifest = subprocess.check_output(
        ["git", "-C", str(root), "show", f"HEAD:{prefix}Cargo.toml"],
    )
    if (root / "Cargo.toml").read_bytes() != original_manifest:
        raise ValueError("working manifest differs from the committed source")
    if (package / "Cargo.toml.orig").read_bytes() != original_manifest:
        raise ValueError("Cargo's original manifest differs from the source candidate")
    expected_rust = {}
    actual_rust = {}
    tracked = subprocess.check_output(
        ["git", "-C", str(root), "ls-files", "-z", "--", "src", "specs", "proofs"],
    )
    for raw in tracked.split(b"\0"):
        name = os.fsdecode(raw)
        if not name.endswith(".rs"):
            continue
        path = root / name
        committed = subprocess.check_output(
            ["git", "-C", str(root), "show", f"HEAD:{prefix}{name}"],
        )
        if path.is_symlink() or path.read_bytes() != committed:
            raise ValueError(f"Rust file differs from the committed source: {name}")
        expected_rust[name] = sha256(committed)
    for directory in ("src", "specs", "proofs"):
        for path in (package / directory).rglob("*.rs"):
            if path.is_symlink():
                raise ValueError("packaged Rust files must not be symbolic links")
            actual_rust[path.relative_to(package).as_posix()] = sha256(path.read_bytes())
    if not expected_rust or actual_rust != expected_rust:
        raise ValueError("packaged runtime/model/proof Rust sources differ from the source candidate")
    return {
        "scope": "Exact packaged verification inputs, not a proof or release approval.",
        "source_commit": commit,
        "path_in_vcs": vcs["path_in_vcs"],
        "archive_sha256": sha256(archive.read_bytes()),
        "archive_files": hashes,
        "rust_source_hashes": actual_rust,
        "cargo_manifest_normalization": {
            "original_sha256": sha256((root / "Cargo.toml").read_bytes()),
            "packaged_sha256": sha256((package / "Cargo.toml").read_bytes()),
            "authority": "Cargo package output; Cargo.toml.orig equals the committed source manifest.",
        },
    }


def verify_snapshot(root: Path, package: Path, archive: Path, before: dict) -> None:
    if snapshot(root, package, archive) != before:
        raise ValueError("packaged verification inputs changed during replay")


def main() -> int:
    parser = argparse.ArgumentParser(description="Bind archive proof replay to the exact package and source.")
    parser.add_argument("action", choices=("prepare", "verify"))
    arguments = parser.parse_args()
    try:
        package, archive = package_paths(ROOT)
        if arguments.action == "prepare":
            binding = snapshot(ROOT, package, archive)
            BINDING.parent.mkdir(parents=True, exist_ok=True)
            BINDING.write_text(json.dumps(binding, indent=2, sort_keys=True) + "\n")
            print(package)
        else:
            verify_snapshot(ROOT, package, archive, json.loads(BINDING.read_text()))
            print("Packaged verification inputs remained identical to the clean source and archive.")
    except (OSError, ValueError, KeyError, tarfile.TarError, subprocess.SubprocessError) as error:
        print(f"packaged source error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
