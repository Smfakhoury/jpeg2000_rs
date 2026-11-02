"""Register the additive VBAS contract test inside the existing unit-test module."""

from pathlib import Path

source = Path("src/jpip.rs")
text = source.read_text()
anchor = "mod tests {\n    use super::*;\n"
registration = '    include!("../validation/jpip_vbas_contract.rs");\n'
assert text.count(anchor) == 1, "existing JPIP test module must be preserved"
assert registration not in text, "unexpected pre-existing contract registration"
test = Path("validation/jpip_vbas_contract.rs").read_text()
assert "fn bounded_standard_vbas_contract()" in test
source.write_text(text.replace(anchor, anchor + registration, 1))
assert source.read_text().count(registration) == 1
