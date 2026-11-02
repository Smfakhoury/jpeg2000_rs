// Compile the unchanged decoder with private unit-test access, preserving its existing tests.
include!("decode.rs");

#[path = "typed_layout_contract_tests.rs"]
mod typed_layout_contract_tests;
