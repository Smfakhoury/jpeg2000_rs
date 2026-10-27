import argparse
import os
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def held_path(path: str) -> bool:
    if path.endswith(".rs") or path.startswith(("src/", ".cargo/", ".github/")):
        return True
    if path.startswith(("specs/", "proofs/", "validation/")):
        return not path.endswith(".md")
    return path.rsplit("/", 1)[-1] in {
        "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "rust-toolchain",
        ".gitattributes", ".gitmodules",
    }


def commit_sha(value: str) -> str:
    if re.fullmatch(r"[0-9a-f]{40}", value) is None:
        raise ValueError(f"expected a full SHA-1 commit ID, got {value!r}")
    return value


def git(repository: Path, *arguments: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(repository), *arguments],
        check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    ).stdout


def held_changes(repository: Path, base: str, head: str) -> list[str]:
    commit_sha(base)
    commit_sha(head)
    ancestors = git(repository, "merge-base", "--all", base, head).splitlines()
    if len(ancestors) != 1:
        raise ValueError("review requires exactly one merge base")
    changes = git(
        repository, "diff", "--no-ext-diff", "--no-textconv", "--no-renames",
        "--name-only", "-z", ancestors[0].decode("ascii"), head, "--",
    )
    return [os.fsdecode(path) for path in changes.split(b"\0") if path and held_path(os.fsdecode(path))]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Hold proof/model changes pending separately approved review controls.",
    )
    parser.add_argument("base", help="full target-branch commit ID")
    parser.add_argument("head", help="full proposed commit ID")
    parser.add_argument("--repository", type=Path, default=ROOT)
    arguments = parser.parse_args()
    try:
        changes = held_changes(arguments.repository, arguments.base, arguments.head)
    except ValueError as error:
        print(f"Model-review hold could not assess the change: {error}", file=sys.stderr)
        return 2
    except subprocess.CalledProcessError as error:
        print(
            "Model-review hold could not read the required Git history:\n"
            + error.stderr.decode("utf-8", errors="replace"),
            file=sys.stderr,
        )
        return 2
    if changes:
        print("HELD: changes affect the frozen engineering proof/model surface:")
        for path in changes:
            print(f"  {path!r}")
        print(
            "Resolve the document-use and independent-review controls before amending "
            "this surface. No author-controlled approval flag overrides this hold. "
            "See PROOFS.md, #149 and #150."
        )
        return 1
    print("PASS: no changes to the held engineering proof/model surface.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
