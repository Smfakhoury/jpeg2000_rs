"""Register an additive include in the existing JPIP unit-test module."""
from pathlib import Path
import subprocess

source = Path("src/jpip.rs")
tests = Path("src/jpip_transition_contract_tests.rs")
anchor = "mod tests {\n    use super::*;\n"
registration = '    include!("jpip_transition_contract_tests.rs");\n'
text = source.read_text()
assert text.count(anchor) == 1, "JPIP test registration anchor changed"
assert tests.is_file(), "submitted test source is missing"
for name in (
    "contract_missing_completion_suffix",
    "contract_boundary_availability",
    "contract_exact_parse_and_errors",
    "contract_owned_results_and_cache_frame",
    "contract_history_and_idempotence",
    "contract_overlapping_immutable_queries",
):
    assert f"fn {name}()" in tests.read_text(), f"missing clause case: {name}"
if registration not in text:
    source.write_text(text.replace(anchor, anchor + registration, 1))
assert source.read_text().count(registration) == 1
assert "fn tile_part_transition_status_distinguishes_sufficient_packet_boundaries()" in source.read_text()
subprocess.run(["cargo", "test", "--locked", "--lib", "--no-run"], check=True)
