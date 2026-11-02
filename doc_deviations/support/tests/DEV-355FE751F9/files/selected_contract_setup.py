from pathlib import Path


def insert(path, before, after):
    source = Path(path).read_text()
    assert source.count(before) == 1, path
    assert after not in source, path
    Path(path).write_text(source.replace(before, after, 1))


insert(
    "src/decode.rs",
    "#[cfg(test)]\nmod tests {\n    use super::*;\n",
    "#[cfg(test)]\nmod tests {\n    use super::*;\n"
    '    include!("decode_selected_contract_tests.rs");\n',
)
insert(
    "src/limits.rs",
    "impl ResourceLimits {\n",
    "impl ResourceLimits {\n"
    "    #[cfg(test)]\n"
    "    pub(crate) fn with_maximum_precincts_for_selected_contract_test(mut self, value: u64) -> Self {\n"
    "        assert!(value <= Self::HARDENED.maximum_precincts);\n"
    "        self.maximum_precincts = value;\n"
    "        self\n"
    "    }\n\n",
)
