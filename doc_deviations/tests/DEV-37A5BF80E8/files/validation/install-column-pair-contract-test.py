"""Install a guarded, test-only include into the existing native test module."""

from pathlib import Path

source = Path("src/transform.rs")
include = '    include!("transform_column_pair_contract_tests.rs");\n'
anchor = "pub(crate) mod tests {\n    use super::*;\n"
text = source.read_text()
assert text.count(anchor) == 1, "existing transform test registration changed"
assert text.count("fn inverse_vertical_stage_matches_column_synthesis()") == 1
assert text.count(
    "fn complete_standard_planes_match_independent_reference_for_all_small_geometries()"
) == 1
assert include not in text, "test registration must start from a pristine checkout"
source.write_text(text.replace(anchor, anchor + include, 1))
replayed = source.read_text()
assert replayed.count(include) == 1, "replay skipped the test registration"
assert replayed.replace(anchor + include, anchor, 1) == text
harness = Path("src/transform_column_pair_contract_tests.rs").read_text()
assert "fn part1_column_pair_contract()" in harness
assert "vec![h[0] / 2]" in harness, "standard singleton oracle is missing"
