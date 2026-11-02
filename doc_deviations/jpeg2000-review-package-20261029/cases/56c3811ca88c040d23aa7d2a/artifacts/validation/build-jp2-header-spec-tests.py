"""Register additive native tests and build one deterministic replay executable."""
import json
from pathlib import Path
import shutil
import subprocess

source = Path("src/jp2.rs")
text = source.read_text()
anchor = "mod tests {\n    use super::*;\n"
registration = '    include!("jp2_header_spec_tests.rs");\n'
assert text.count(anchor) == 1, "native test registration anchor changed"
assert registration not in text, "expected pristine replay source"
assert "fn requires_bpcc_for_variable_component_depths()" in text
source.write_text(text.replace(anchor, anchor + registration, 1))
assert registration in source.read_text()
suite = Path("src/jp2_header_spec_tests.rs").read_text()
names = [
    "spec_header_first_child_is_ihdr",
    "spec_header_ignores_later_colour_semantics",
    "spec_header_bpcc_presence_and_multiplicity",
]
for name in names:
    assert f"fn {name}()" in suite, f"missing replay assertion case {name}"
result = subprocess.run(
    ["cargo", "test", "--lib", "--no-run", "--message-format=json"],
    text=True, stdout=subprocess.PIPE, check=True,
)
artifacts = [
    message["executable"]
    for line in result.stdout.splitlines()
    if (message := json.loads(line)).get("reason") == "compiler-artifact"
    and message.get("profile", {}).get("test")
    and message.get("executable")
]
assert len(artifacts) == 1, artifacts
output = Path("target/jp2-header-spec-tests")
shutil.copy2(artifacts[0], output)
listing = subprocess.check_output([str(output), "--list"], text=True)
for name in names:
    assert f"jp2::tests::{name}: test" in listing, f"test not registered: {name}"
