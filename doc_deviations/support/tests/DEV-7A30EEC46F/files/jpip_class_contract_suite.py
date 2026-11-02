"""Replay support for two cases added to the existing JPIP libtest module."""

import json
import os
from pathlib import Path
import subprocess
import sys

CASES = (
    "jpip::tests::standard_assigned_classes_exclude_reserved_signaling",
    "jpip::tests::standard_precinct_forms_share_semantics_and_packet_admission",
)
EXECUTABLE = Path("target/deepspec-class/executable.txt")


def build():
    source = Path("src/jpip.rs")
    anchor = "#[cfg(test)]\nmod tests {\n    use super::*;\n"
    registration = '    include!("jpip_class_contract_cases.rs");\n'
    original = source.read_text()
    assert original.count(anchor) == 1, "existing JPIP test module missing/ambiguous"
    assert registration not in original, "unexpected preinstalled test overlay"
    edited = original.replace(anchor, anchor + registration, 1)
    assert edited.replace(registration, "", 1) == original
    source.write_text(edited)
    assert registration in source.read_text(), "test registration did not persist"
    cases_source = Path("src/jpip_class_contract_cases.rs").read_text()
    for case in CASES:
        assert f"fn {case.rsplit('::', 1)[1]}()" in cases_source
    command = [
        "cargo", "test", "--locked", "--lib", "--no-run",
        "--target-dir", "target/deepspec-class", "--message-format=json",
    ]
    result = subprocess.run(command, text=True, stdout=subprocess.PIPE)
    if result.returncode:
        print(result.stdout)
        raise SystemExit(result.returncode)
    executables = []
    for line in result.stdout.splitlines():
        event = json.loads(line)
        if (event.get("reason") == "compiler-artifact"
                and event.get("profile", {}).get("test")
                and event.get("executable")):
            executables.append(event["executable"])
    assert len(executables) == 1, executables
    binary = executables[0]
    listing = subprocess.check_output([binary, "--list"], text=True)
    for case in CASES:
        assert f"{case}: test" in listing, f"case not compiled: {case}"
    EXECUTABLE.write_text(binary)


if __name__ == "__main__":
    if sys.argv[1:] == ["build"]:
        build()
    else:
        assert len(sys.argv) == 2 and sys.argv[1] in CASES, sys.argv
        binary = EXECUTABLE.read_text()
        os.execv(binary, [binary, "--exact", sys.argv[1], "--nocapture"])
