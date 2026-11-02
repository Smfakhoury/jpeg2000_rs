# JPIP classifier assigns standard data-bin semantics to reserved class identifiers 3 and 7

`DEV-77C8A9E6C3` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.2.2

**Required behavior:** The project's bounded JPIP profile classifies accepted message class identifiers into the semantic data-bin class used by parsed identifiers and cache lookup.

**Observed deviation:** JPIP classifier assigns standard data-bin semantics to reserved class identifiers 3 and 7

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-77C8A9E6C3 --output rfc-results/DEV-77C8A9E6C3
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/deepstar-jpip-contract jpip::tests::class_identifiers_follow_t808_table_a2 --exact --nocapture
```

### Test input and recorded failure

No owning-caller fixture is bypassed: this is a direct unit_contract test of a pure scalar function with an unrestricted u64 entry domain. Identifiers 3/7 satisfy that entry and the callee's precondition-free contract, despite being reserved by the pinned standard; rejection of unsupported identifiers is part of classification. No parsed headers, resource-limited buffers, cache objects, stream-type combinations, or negotiated extension states are manufactured. There are 2231 bounded sequential comparisons, constant call depth, and only two diagnostic strings on this replay. The build helper inserted only test registration and compiled the unchanged production implementation. Applicability assumes no separately negotiated reserved-class extension, as in the originating standards finding.

**Recorded failure** (matched by the replay runner):

```text
semantic classification mismatches:
DatabinClass::from_identifier(3): expected None, got Some(TileHeader)
DatabinClass::from_identifier(7): expected None, got Some(MainHeader)
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_class_contract_tests.rs:L16-L24`](files/src/jpip_class_contract_tests.rs#L16-L24)

```text
    let mut check = |identifier, expected| {
        let actual = DatabinClass::from_identifier(identifier);
        checks += 1;
        if actual != expected {
            failures.push(format!(
                "DatabinClass::from_identifier({identifier}): expected {expected:?}, got {actual:?}"
            ));
        }
    };
```

[`src/jpip_class_contract_tests.rs:L31-L35`](files/src/jpip_class_contract_tests.rs#L31-L35)

```text
    for identifier in 0..=1024 {
        if ![0, 1, 2, 4, 5, 6, 8].contains(&identifier) {
            check(identifier, None);
        }
    }
```

[`src/jpip_class_contract_tests.rs:L59-L63`](files/src/jpip_class_contract_tests.rs#L59-L63)

```text
    assert!(
        failures.is_empty(),
        "semantic classification mismatches:\n{}",
        failures.join("\n")
    );
```

[`deepstar_jpip_contract.py:L9-L14`](files/deepstar_jpip_contract.py#L9-L14)

```text
anchor = "#[cfg(test)]\nmod tests {\n    use super::*;\n"
registration = '\n    include!("jpip_class_contract_tests.rs");\n'
assert original.count(anchor) == 1, "existing JPIP unit-test module not found uniquely"
assert registration not in original, "contract test already registered"
updated = original.replace(anchor, anchor + registration, 1)
assert updated.replace(registration, "", 1) == original
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_class_contract_tests.rs`](files/src/jpip_class_contract_tests.rs)
- [`deepstar_jpip_contract.py`](files/deepstar_jpip_contract.py)

**Build commands** (in order):

```sh
python3 deepstar_jpip_contract.py
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is failures.is_empty() at src/jpip_class_contract_tests.rs:59, with the first mismatch 'DatabinClass::from_identifier(3): expected None, got Some(TileHeader)' and a second mismatch for 7/MainHeader. This violates the standards-derived semantic-classification clause 482717998449fb68d677ca32, interpreted with its rfc_map parent finding, not the separate implementation-inferred interval clause 23bb46fbb1035a00030a243f. I read clause_origin and independently retrieved the pinned T.808 (12/2022), A.2.2, version 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc: Table A.2 defines only 0,1,2,4,5,6,8; all other values are reserved. Odd parity supplies auxiliary-field syntax for parsing even unrecognized messages; it does not define extended tile-header or extended main-header semantics. This is classification, not a claim that the standard mandates this particular Option API or unconditional rejection rather than skipping. Under the originating finding's no-separately-negotiated-extension assumption, None faithfully expresses the absence of a supported standard semantic class. The module advertises standard header forms and classes 0 through 8 but provides no negotiated-extension premise for inventing meanings for 3/7. Its range predicate, native oracle and correlated inferred contracts do not override the cited standard. This conclusion is limited to that edition and assumption, not every future extension.

The entry is realistic and admissible before considering downstream results: the unit accepts any by-value u64 with no requires; there is no buffer, pool, recursion, per-call allocation, indexing, or stack-depth requirement. The entire harness performs 2231 sequential calls (7 defined values, 1018 other values through 1024, 180 power-of-two-neighbor values, 2 maxima, 1024 deterministic samples); it allocates only diagnostic strings for the two observed mismatches. Large integer values are scalar rejection probes, not requested buffer sizes. Identifiers 3/7 are tiny one-byte VBAS values within the representation actually supplied by public DatabinId::new and by parse_jpip_message's explicit-class read_vbas path. The unit test does not claim to replay that parser or fabricate any parser/cache state, and independent pure calls do not mix JPP/JPT stream fixtures.

Compiler declaration and CFG establish the exact failing route: direct private unit call -> bb0 jpip_class_identifier_valid(3 or 7), whose <=8 implementation returns true without violating its contract -> bb1 true -> bb2 identifier switch -> bb8 for 3 or bb6 for 7 -> bb10 returning Some(TileHeader/MainHeader). The replay's build succeeded and registered this test using only a cfg(test) include; no production executable statement was changed. Thus neither a build/fixture/precondition error nor a standards-specific clause misreading explains the failure. The consequence is actual semantic recognition of reserved identifiers; the public constructor stores this erroneous class as DatabinId.class, and the wire parser uses that constructor. Only the direct unit classification failure is claimed as host-reproduced, not an end-to-end wire/cache failure or a formal reachability proof.

[Standards quote, production lines, and provenance](../../evidence.md#dev-77c8a9e6c3)

</details>
