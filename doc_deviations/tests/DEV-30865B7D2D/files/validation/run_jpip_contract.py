"""Register additive native tests, build once, then run an exact clause case."""

import json
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
BINARY_RECORD = ROOT / ".jpip-contract-binary"
NAMES = {
    "eor": "jpip::tests::contract_bounded_message_and_eor",
    "forms": "jpip::tests::contract_standard_header_forms",
}


def build():
    source = ROOT / "src/jpip.rs"
    text = source.read_text()
    anchor = "#[cfg(test)]\nmod tests {\n    use super::*;"
    registration = '\n    include!("jpip_contract_tests.rs");'
    assert text.count(anchor) == 1, "original test module anchor changed"
    if registration not in text:
        source.write_text(text.replace(anchor, anchor + registration, 1))
    replay_source = source.read_text()
    assert anchor + registration in replay_source, "test registration missing"
    assert replay_source.replace(registration, "", 1) == text.replace(
        registration, "", 1
    ), "registration changed unrelated source"
    for original in [
        "parses_all_three_standard_header_forms_and_auxiliary",
        "parses_end_of_response",
        "rejects_malformed_prefixes_and_vbas",
    ]:
        assert f"fn {original}()" in replay_source
    added = (ROOT / "src/jpip_contract_tests.rs").read_text()
    for name in NAMES.values():
        assert f"fn {name.rsplit('::', 1)[1]}()" in added
    result = subprocess.run(
        ["cargo", "test", "--locked", "--lib", "--no-run", "--message-format=json"],
        cwd=ROOT, text=True, stdout=subprocess.PIPE, check=True,
    )
    binaries = []
    for line in result.stdout.splitlines():
        event = json.loads(line)
        if event.get("reason") == "compiler-message":
            print(event["message"].get("rendered", ""), file=sys.stderr)
        if (event.get("reason") == "compiler-artifact"
                and event.get("profile", {}).get("test")
                and event.get("executable")):
            binaries.append(event["executable"])
    assert len(binaries) == 1, binaries
    BINARY_RECORD.write_text(binaries[0])


def run(case):
    binary = BINARY_RECORD.read_text()
    name = NAMES[case]
    listing = subprocess.run(
        [binary, "--list"], cwd=ROOT, text=True,
        stdout=subprocess.PIPE, check=True,
    ).stdout
    assert f"{name}: test" in listing, f"exact case not compiled: {name}"
    return subprocess.run(
        [binary, name, "--exact", "--nocapture"], cwd=ROOT
    ).returncode


if __name__ == "__main__":
    if sys.argv[1:] == ["build"]:
        build()
    else:
        assert len(sys.argv) == 3 and sys.argv[1] == "run"
        sys.exit(run(sys.argv[2]))
