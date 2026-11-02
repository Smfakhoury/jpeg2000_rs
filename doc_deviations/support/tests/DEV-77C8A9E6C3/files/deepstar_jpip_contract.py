import json
from pathlib import Path
import shutil
import subprocess


source = Path("src/jpip.rs")
original = source.read_text()
anchor = "#[cfg(test)]\nmod tests {\n    use super::*;\n"
registration = '\n    include!("jpip_class_contract_tests.rs");\n'
assert original.count(anchor) == 1, "existing JPIP unit-test module not found uniquely"
assert registration not in original, "contract test already registered"
updated = original.replace(anchor, anchor + registration, 1)
assert updated.replace(registration, "", 1) == original
source.write_text(updated)
assert registration in source.read_text(), "test registration did not persist"
test_source = Path("src/jpip_class_contract_tests.rs").read_text()
assert "fn class_identifiers_follow_t808_table_a2()" in test_source
assert "DatabinClass::from_identifier(identifier)" in test_source
result = subprocess.run(
    ["cargo", "test", "--lib", "--locked", "--no-run", "--message-format=json"],
    text=True,
    stdout=subprocess.PIPE,
    check=True,
)
executables = []
for line in result.stdout.splitlines():
    message = json.loads(line)
    if (
        message.get("reason") == "compiler-artifact"
        and message.get("profile", {}).get("test")
        and message.get("executable")
        and "lib" in message["target"]["kind"]
    ):
        executables.append(message["executable"])
assert len(executables) == 1, f"expected one library test binary: {executables}"
binary = Path("target/deepstar-jpip-contract")
shutil.copy2(executables[0], binary)
name = "jpip::tests::class_identifiers_follow_t808_table_a2"
listing = subprocess.check_output([str(binary), "--list"], text=True)
assert f"{name}: test" in listing, "compiled binary omitted the new test"
assert "jpip::tests::parses_all_three_standard_header_forms_and_auxiliary: test" in listing
