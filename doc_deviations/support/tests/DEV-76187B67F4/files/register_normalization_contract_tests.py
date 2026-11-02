from pathlib import Path

source = Path("src/jpip.rs")
tests = Path("src/jpip_normalization_contract_tests.rs")
text = source.read_text()
anchor = "#[cfg(test)]\nmod tests {\n    use super::*;\n"
registration = '    include!("jpip_normalization_contract_tests.rs");\n'
assert text.count(anchor) == 1, "unexpected jpip test module layout"
assert registration not in text, "registration already present"
for name in (
    "automatic_normalization_accepts_optional_wire_sod_contract",
    "automatic_normalization_sot_fields_contract",
):
    assert f"fn {name}()" in tests.read_text(), f"missing assertion case {name}"
source.write_text(text.replace(anchor, anchor + registration, 1))
assert registration in source.read_text(), "test registration did not persist"
