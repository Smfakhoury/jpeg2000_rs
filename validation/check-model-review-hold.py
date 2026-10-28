import argparse
import os
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

# Only records adopted on the trusted default branch authorize a structural change.
# Each path binds both the original and proposed regular-file Git blob IDs.
REVIEWED_TEST_EXTRACTIONS: tuple[dict[str, tuple[str | None, str]], ...] = (
    {
        "src/codestream.rs": (
            "d8c8066b191da8382ce10c2ca3eb6587d07a9b44",
            "09f0b4369eabb7c05098d2aaa22041bfeadbe698",
        ),
        "src/codestream/tests.rs": (
            None,
            "9317d9d81f43fb8d8d382e2a5733d8583789afc2",
        ),
        "validation/expected-package-files.txt": (
            "ef00ad4587c92a7c52365e5d6a1a0dec18f12005",
            "026b291e83d7b551c092b5919886bc19b8b7b90e",
        ),
        "validation/normative-coverage.json": (
            "8053dd2c9ce66efdb4f39f75aa0e45539ccc7e9c",
            "05237df10a29fd9e430180a0f7250da810db567f",
        ),
        "validation/tests/test_framework.py": (
            "28edc2ee48064bbb2693c44656705ab8934086fb",
            "43da2a6f2e292bef0b9d411f772b59993f39403d",
        ),
    },
)

REVIEWED_CONSUMER_PROBES: tuple[dict[str, tuple[str | None, str]], ...] = (
    {
        ".github/workflows/ci.yml": (
            "c4c373dc1604314d7588de438c33ed7182de7052",
            "eaad74889b89bab5ca025282d4c53f9503d9e18c",
        ),
        "validation/image-consumer-probe/.gitignore": (
            None,
            "b83d22266ac8aa2f8df2edef68082c789727841d",
        ),
        "validation/image-consumer-probe/Cargo.lock": (
            None,
            "491fa432326017b6eb284354ebc9fbf595c5f56f",
        ),
        "validation/image-consumer-probe/Cargo.toml": (
            None,
            "1735ac1955818412f4b4f266d080f0c418d137aa",
        ),
        "validation/image-consumer-probe/src/decoder.rs": (
            None,
            "e8c36bb5ac7b5b12612fbc85163f44768e6eaa7c",
        ),
        "validation/image-consumer-probe/src/lib.rs": (
            None,
            "40c4eff6e40c9b647ac4ec1e12a295cb956d8060",
        ),
        "validation/image-consumer-probe/tests/adapter.rs": (
            None,
            "1eeb94542c3e000b2bc6631198437389eb8c2cea",
        ),
        "validation/image-consumer-probe/tests/fixtures/gray.jp2": (
            None,
            "efc957bcd86ffef8f416081a790b1f64d6408bfb",
        ),
        "validation/image-consumer-probe/tests/fixtures/rgb.jp2": (
            None,
            "b36f70e6d01316b056e30aa72aa05344f1181b55",
        ),
    },
)


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


def reviewed_structural_change(
    repository: Path, base: str, head: str, changes: list[str],
) -> bool:
    for record in REVIEWED_TEST_EXTRACTIONS + REVIEWED_CONSUMER_PROBES:
        if set(record) != set(changes):
            continue
        matches = True
        for path, (before, after) in record.items():
            for commit, blob in ((base, before), (head, after)):
                expected = b"" if blob is None else (
                    f"100644 blob {blob}\t{path}\0".encode("utf-8")
                )
                if git(repository, "ls-tree", "-z", commit, "--", path) != expected:
                    matches = False
                    break
            if not matches:
                break
        if matches:
            return True
    return False


def held_changes(repository: Path, base: str, head: str) -> list[str]:
    commit_sha(base)
    commit_sha(head)
    ancestors = git(repository, "merge-base", "--all", base, head).splitlines()
    if len(ancestors) != 1:
        raise ValueError("review requires exactly one merge base")
    ancestor = ancestors[0].decode("ascii")
    changes = git(
        repository, "diff", "--no-ext-diff", "--no-textconv", "--no-renames",
        "--name-only", "-z", ancestor, head, "--",
    )
    held = [os.fsdecode(path) for path in changes.split(b"\0") if path and held_path(os.fsdecode(path))]
    return [] if reviewed_structural_change(repository, ancestor, head, held) else held


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
    print("PASS: no unapproved changes to the held engineering proof/model surface.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
