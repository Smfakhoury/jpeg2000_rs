"""Register additive tests in jpip's existing libtest module and build once."""

import json
from pathlib import Path
import shutil
import subprocess


source = Path("src/jpip.rs")
anchor = "#[cfg(test)]\nmod tests {\n    use super::*;\n"
registration = '    include!("jpip_placeholder_contract_tests.rs");\n'
text = source.read_text()
assert text.count(anchor) == 1, "jpip test-module anchor changed"
if registration not in text:
    source.write_text(text.replace(anchor, anchor + registration, 1))
assert source.read_text().count(registration) == 1
tests = (
    "placeholder_contract_stream_identifiers_and_counts",
    "placeholder_contract_original_header_ownership",
    "placeholder_contract_received_semantics_ignore_unused_bits",
)
overlay = Path("src/jpip_placeholder_contract_tests.rs").read_text()
for name in tests:
    assert f"fn {name}()" in overlay, f"missing submitted assertion case: {name}"
result = subprocess.run(
    ["cargo", "test", "--locked", "--lib", "--no-run", "--message-format=json"],
    text=True,
    stdout=subprocess.PIPE,
    check=True,
)
executables = []
for line in result.stdout.splitlines():
    message = json.loads(line)
    if message.get("reason") == "compiler-artifact" and message.get("executable"):
        executables.append(message["executable"])
assert len(executables) == 1, executables
binary = Path("target/placeholder-contract-tests")
shutil.copy2(executables[0], binary)
listing = subprocess.check_output([str(binary), "--list"], text=True)
for name in (*tests, "placeholder_parser_accepts_standard_bit_flags_and_types_extensions"):
    assert f"jpip::tests::{name}: test" in listing, f"unregistered test: {name}"
print(f"Built {binary}; all three contract cases and original regression registered.")
