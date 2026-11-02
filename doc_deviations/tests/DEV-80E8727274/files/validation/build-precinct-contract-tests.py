"""Register test-only code in the existing JPIP libtest module and build once.

The two source registrations below are the complete test-access overlay: an
include inside cfg(test) and a local dev-dependency for allocation accounting.
No production function, visibility, feature, or unsafe-code policy is changed.
Cargo updates only the test dependency's local lock entry. The executable is
copied to a stable name so clause runs invoke libtest without rebuilding.
"""

import json
from pathlib import Path
import shutil
import subprocess


root = Path(__file__).resolve().parents[1]
source = root / "src/jpip.rs"
text = source.read_text()
anchor = "mod tests {\n    use super::*;\n"
registration = '    include!("jpip_precinct_validation_tests.rs");\n'
assert text.count(anchor) == 1, "existing JPIP test-module anchor changed"
if registration not in text:
    source.write_text(text.replace(anchor, anchor + "\n" + registration, 1))

manifest = root / "Cargo.toml"
text = manifest.read_text()
dependency = 'precinct_contract_alloc = { path = "validation/precinct-contract-alloc" }'
if dependency not in text:
    assert "[dev-dependencies]" not in text, "unexpected dev-dependency section"
    manifest.write_text(text + "\n[dev-dependencies]\n" + dependency + "\n")

result = subprocess.run(
    ["cargo", "test", "--lib", "--no-run", "--message-format=json"],
    cwd=root,
    text=True,
    stdout=subprocess.PIPE,
    check=True,
)
executables = []
for line in result.stdout.splitlines():
    message = json.loads(line)
    if (
        message.get("reason") == "compiler-artifact"
        and message.get("executable")
        and message["target"]["name"] == "jpeg2000_rs"
        and message["profile"]["test"]
    ):
        executables.append(message["executable"])
assert len(executables) == 1, f"expected one libtest executable: {executables}"
destination = root / "target/precinct-contract-tests"
shutil.copy2(executables[0], destination)
listed = subprocess.run(
    [str(destination), "--list"],
    text=True,
    stdout=subprocess.PIPE,
    check=True,
).stdout
for case in [
    "precinct_contract_structural_eligibility",
    "precinct_contract_error_propagation_and_frame",
    "precinct_contract_ownership_and_cleanup",
]:
    assert f"jpip::tests::{case}: test" in listed, f"unregistered test: {case}"
print(f"Built {destination}")
