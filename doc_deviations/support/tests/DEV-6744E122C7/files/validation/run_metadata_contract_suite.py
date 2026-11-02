"""Build the native JPIP tests once, then run one exact clause case."""

import json
from pathlib import Path
import subprocess
import sys


CASES = {
    "metadata_identity_ignores_message_csn",
    "original_contents_preserve_nested_box_order",
    "stream_equivalent_uses_equivalent_box_identity",
}
REGISTRATION = (
    '    mod metadata_contract_tests {\n'
    '        use super::*;\n'
    '        include!("jpip_metadata_contract_tests.rs");\n'
    '    }\n\n'
)
SOURCE = Path("src/jpip.rs")
EXECUTABLE = Path("target/metadata-contract-executable")

if sys.argv[1:] == ["build"]:
    original = SOURCE.read_text()
    anchor = "mod tests {\n    use super::*;\n\n"
    assert original.count(anchor) == 1, "unexpected native test-module layout"
    if REGISTRATION not in original:
        updated = original.replace(anchor, anchor + REGISTRATION, 1)
        assert updated.replace(REGISTRATION, "", 1) == original
        SOURCE.write_text(updated)
    replay_source = SOURCE.read_text()
    assert replay_source.count(REGISTRATION) == 1, "registration did not reach replay source"
    tests = Path("src/jpip_metadata_contract_tests.rs").read_text()
    for case in CASES:
        assert f"fn {case}()" in tests, f"missing replay case: {case}"
    result = subprocess.run(
        ["cargo", "test", "--lib", "--no-run", "--message-format=json"],
        text=True,
        stdout=subprocess.PIPE,
        check=True,
    )
    binaries = []
    for line in result.stdout.splitlines():
        message = json.loads(line)
        if message.get("reason") == "compiler-artifact" and message.get("executable"):
            if message["target"]["name"] == "jpeg2000_rs" and message["profile"]["test"]:
                binaries.append(message["executable"])
    assert len(binaries) == 1, f"expected one native library test executable: {binaries}"
    EXECUTABLE.write_text(binaries[0])
elif len(sys.argv) == 2 and sys.argv[1] in CASES:
    binary = EXECUTABLE.read_text()
    case = "jpip::tests::metadata_contract_tests::" + sys.argv[1]
    listing = subprocess.run(
        [binary, "--list", "--exact", case], text=True, stdout=subprocess.PIPE, check=True
    ).stdout
    assert f"{case}: test" in listing, f"exact case not registered: {listing}"
    print(f"route=DatabinCache::expand_metadata_bin case={case}", flush=True)
    sys.exit(subprocess.run([binary, "--exact", case, "--nocapture"]).returncode)
else:
    raise SystemExit("expected build or an exact metadata contract case")
