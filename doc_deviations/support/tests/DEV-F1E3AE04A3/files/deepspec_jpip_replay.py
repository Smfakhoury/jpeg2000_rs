"""Replay additive native JPIP unit tests, building the libtest binary once."""

import json
from pathlib import Path
import shutil
import subprocess
import sys


CASES = (
    "jpip::tests::deepspec_standard_header_forms",
    "jpip::tests::deepspec_bounded_data_and_eor_framing",
    "jpip::tests::deepspec_parser_local_inheritance_lifecycle",
)
BINARY = Path("target/deepspec-jpip-suite")


def build():
    subprocess.run(
        ["patch", "--batch", "--forward", "--fuzz=0", "-p1",
         "-i", "deepspec_jpip_parse.patch"],
        check=True,
    )
    source = Path("src/jpip.rs").read_text()
    for case in CASES:
        assert source.count("fn " + case.rsplit("::", 1)[1] + "()") == 1, case
    for guard in (
        "first inherited {wire:02x?}",
        "whole supplied input, not only declared body, must respect limit",
        "context changed after prefix={prefix}",
        "let explicit = [0x72, 0x01, 0x01, 0x00, 0x03, 0x07, 1, 2, 3];",
        "let inherited = [0x31, 0x03, 0x01, 0, 9];",
        "let class_only = [0x50, 0x08, 0x00, 0x00];",
        "let input = [0, 2, 3, b'e', b'o', b'r'];",
    ):
        assert guard in source, f"Replay source guard missing: {guard}"
    result = subprocess.run(
        ["cargo", "test", "--locked", "--lib", "--no-run", "--message-format=json"],
        text=True,
        stdout=subprocess.PIPE,
        check=True,
    )
    executables = []
    for line in result.stdout.splitlines():
        event = json.loads(line)
        if (event.get("reason") == "compiler-artifact"
                and event.get("profile", {}).get("test")
                and event.get("executable")):
            executables.append(event["executable"])
    assert len(executables) == 1, executables
    BINARY.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(executables[0], BINARY)
    listing = subprocess.run(
        [str(BINARY), "--list"], text=True, stdout=subprocess.PIPE, check=True
    ).stdout
    for case in CASES:
        assert f"{case}: test" in listing, f"Unregistered case: {case}"
    print("All three clause cases are present in replay source and compiled binary.")


def run(case):
    assert case in CASES, case
    print(f"Route: JpipMessageParser::new -> JpipMessageParser::parse; case={case}",
          flush=True)
    result = subprocess.run(
        [str(BINARY), case, "--exact", "--nocapture"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    print(result.stdout, end="")
    assert "running 1 test" in result.stdout, "Exact clause case did not execute"
    return result.returncode


if __name__ == "__main__":
    if sys.argv[1:] == ["build"]:
        build()
    elif len(sys.argv) == 3 and sys.argv[1] == "run":
        sys.exit(run(sys.argv[2]))
    else:
        raise SystemExit("usage: deepspec_jpip_replay.py build | run CASE")
