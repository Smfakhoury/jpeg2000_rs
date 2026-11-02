"""Build the native packet tests once and guard replay's additive registration.

The four cases call the private subject from its existing Rust test module.
SOP/EPH fixtures use Part-1 COD in the main header, a single SOT/SOD tile-part,
and optional Zppt=0 PPT before SOD (never PPM plus PPT). SOP remains in tile
data; EPH remains with the header. T.800 (2019) A.6.1, A.7.5, A.8.1 and A.8.2
provide the marker oracle. In particular, enabled SOP permits omission.

The selection sweep is an internal unit-contract fixture, not a T.803
compliance-class claim. It also executes the existing full-progression HT
regression unchanged. Direct counter/cursor fixtures have no declared requires.
All buffers are small and counters remain below the default packet limit.
No sanitizer or coverage target is configured in this checkout.
"""

import json
from pathlib import Path
import shutil
import subprocess
import sys


CASES = (
    "progression_contract_sop_modulo_65536",
    "progression_contract_eph_cursors_and_preflight",
    "progression_contract_selection_boundary_and_structure",
    "progression_contract_sop_policy_and_optional_marker",
)

source = Path("src/packet.rs")
registration = '    include!("packet_progression_contract_tests.rs");\n'
text = source.read_text()
if registration not in text:
    subprocess.run(
        ["patch", "-p1", "--batch", "--forward", "-i",
         "validation/deepspec-progression-registration.patch"],
        check=True,
    )
text = source.read_text()
assert text.count(registration) == 1, "test registration missing or duplicated"
assert "mod tests {\n    use super::*;\n\n" + registration in text

test_source = Path("src/packet_progression_contract_tests.rs").read_text()
for case in CASES:
    assert test_source.count("fn " + case + "()") == 1, case
assert "pub(crate) fn exact_packet_precinct_snapshot" in text
assert "fn full_selected_progression_derives_literal_headers_and_skips_only_higher_resolution_coefficients" in text
assert "fn precinct_payload_literal_inline_and_packed_streams_account_for_markers_and_real_coefficients" in text

build = subprocess.run(
    ["cargo", "test", "--lib", "--no-run", "--message-format=json"],
    text=True, capture_output=True,
)
sys.stderr.write(build.stderr)
artifacts = []
for line in build.stdout.splitlines():
    message = json.loads(line)
    if message.get("reason") == "compiler-message":
        sys.stderr.write(message["message"].get("rendered", ""))
    if (message.get("reason") == "compiler-artifact"
            and message.get("profile", {}).get("test")
            and message.get("executable")):
        artifacts.append(message["executable"])
if build.returncode:
    sys.exit(build.returncode)
assert len(artifacts) == 1, artifacts
binary = Path("target/deepspec-progression-suite")
binary.parent.mkdir(exist_ok=True)
shutil.copy2(artifacts[0], binary)
listing = subprocess.check_output([str(binary), "--list"], text=True)
for case in CASES:
    assert f"packet::tests::{case}: test\n" in listing, case
for original in (
    "full_selected_progression_derives_literal_headers_and_skips_only_higher_resolution_coefficients",
    "precinct_payload_literal_inline_and_packed_streams_account_for_markers_and_real_coefficients",
):
    assert f"packet::tests::{original}: test\n" in listing, original
print("Built one native suite; all four clause cases and reused regressions are registered.")
