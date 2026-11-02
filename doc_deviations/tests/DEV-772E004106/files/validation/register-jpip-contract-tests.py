from pathlib import Path

source = Path("src/jpip.rs")
before = source.read_text()
anchor = "mod tests {\n    use super::*;\n"
registration = '    include!("jpip_ingest_contract_tests.rs");\n'
assert before.count(anchor) == 1, "test-module anchor changed"
assert registration not in before, "registration unexpectedly already present"
suite = Path("src/jpip_ingest_contract_tests.rs").read_text()
for case in (
    "contract_ingest_standard_headers_and_response_local_defaults",
    "contract_ingest_eor_reason_extent_and_no_cache_body",
    "contract_ingest_payload_offsets_and_chunking",
):
    assert f"fn {case}()" in suite, f"missing replay test {case}"
after = before.replace(anchor, anchor + "\n" + registration, 1)
source.write_text(after)
assert source.read_text().replace("\n" + registration, "", 1) == before
assert source.read_text().count(registration) == 1
print("Verified additive JPIP contract-test registration and all three cases")
