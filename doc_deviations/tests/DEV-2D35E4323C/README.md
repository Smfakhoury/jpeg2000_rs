# Reserved T.808 (12/2022) classes 3 and 7 receive standard TileHeader and MainHeader semantics

`DEV-2D35E4323C` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.2.2

**Required behavior:** When constructing semantic data-bin identities for standard T.808 (12/2022) messages, the defined class values are 0,1,2,4,5,6,8. Reserved values 3 and 7 shall not be assigned ordinary TileHeader or MainHeader semantics as if defined by that edition, absent a documented separately supported extension. This does not prescribe whether a standalone raw-header utility must reject or preserve an unrecognized value.

**Observed deviation:** Reserved T.808 (12/2022) classes 3 and 7 receive standard TileHeader and MainHeader semantics

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-2D35E4323C --output rfc-results/DEV-2D35E4323C
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/databin-contract-tests jpip::tests::standard_databin_class_assignment --exact --nocapture
```

### Test input and recorded failure

A direct public unit-contract entry accepting three by-value u64 arguments and returning Result, with no safety or caller preconditions. No fabricated private state, payload, wire frame, cache, negotiated extension, or decoder setup is involved. Reserved identifiers are admissible inputs to this fallible validation boundary. Only small finite vectors and constant stack depth are required; even the first stream=0, bin=0 cases produce the failure. Large numeric identifiers do not request corresponding resources. Scope is standard T.808 (12/2022) semantic assignment, not mandatory raw-header rejection or 2026 conformance.

**Recorded failure** (matched by the replay runner):

```text
264 reserved-class assignments received ordinary standard semantics; first examples: [(3, 0, 0, TileHeader), (7, 0, 0, MainHeader), (3, 0, 1, TileHeader), (7, 0, 1, MainHeader), (3, 0, 15, TileHeader), (7, 0, 15, MainHeader), (3, 0, 16, TileHeader), (7, 0, 16, MainHeader), (3, 0, 127, TileHeader), (7, 0, 127, MainHeader), (3, 0, 128, TileHeader), (7, 0, 128, MainHeader)]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_databin_contract_tests.rs:L1-L3`](files/src/jpip_databin_contract_tests.rs#L1-L3)

```text
// T.808 (12/2022), A.2.2/Table A.2 and A.3.6.1. These tests construct
// identities directly: there is no wire framing, AUX interpretation, or
// negotiated extension. Each cache represents exactly one logical target.
```

[`src/jpip_databin_contract_tests.rs:L61-L76`](files/src/jpip_databin_contract_tests.rs#L61-L76)

```text
        for raw in [3, 7] {
            // Rejection OR an explicit unknown representation is acceptable.
            // Do not impose a raw-header rejection/error-code requirement.
            if let Ok(identity) = DatabinId::new(raw, stream, bin) {
                if matches!(
                    identity.class(),
                    DatabinClass::Precinct
                        | DatabinClass::TileHeader
                        | DatabinClass::TileData
                        | DatabinClass::MainHeader
                        | DatabinClass::Metadata
                ) {
                    forbidden.push((raw, stream, bin, identity.class()));
                }
            }
        }
```

[`src/jpip_databin_contract_tests.rs:L78-L83`](files/src/jpip_databin_contract_tests.rs#L78-L83)

```text
    assert!(
        forbidden.is_empty(),
        "{} reserved-class assignments received ordinary standard semantics; first examples: {:?}",
        forbidden.len(),
        &forbidden[..forbidden.len().min(12)]
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_databin_contract_tests.rs`](files/src/jpip_databin_contract_tests.rs)
- [`validation/databin-contract-tests.patch`](files/validation/databin-contract-tests.patch)
- [`validation/databin-contract-runner.py`](files/validation/databin-contract-runner.py)

**Build commands** (in order):

```sh
bash -eu -c 'patch --batch --forward -p1 < validation/databin-contract-tests.patch; grep -Fq '"'"'include!("jpip_databin_contract_tests.rs");'"'"' src/jpip.rs; cargo test --lib --no-run --locked; python3 validation/databin-contract-runner.py'
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is forbidden.is_empty() in standard_databin_class_assignment, not a build, setup, resource, or unrelated cache failure. Host replay reports 264 reserved-class assignments, including (3,0,0,TileHeader) and (7,0,0,MainHeader). The test invokes the real public DatabinId::new and reads class(); it does not fabricate a DatabinId or bypass an owning caller. Its 132 coordinates produce 1,188 scalar constructor calls (seven defined and two reserved classes per coordinate), with 264 small failure records, constant call depth, and only small Vec allocations. u64::MAX coordinates are opaque identifiers, not requested buffer sizes or allocation counts. There is no constructor ResourceLimits argument, indexing by these identifiers, or payload allocation. The all-zero coordinate already exposes both deviations, so the finding does not depend on large values, nonzero main-header bin identifiers, or caller capacities.

The constructor's three arguments are unrestricted u64 values; the retained constructor/classifier contracts have no entry requirements, and from_identifier is explicitly total over u64. Reserved class values are legitimate inputs to this fallible class-validation boundary, not a claim that the corresponding messages are standards-conforming. Real parse_jpip_message callers pass read_vbas class/codestream/bin values directly into this same constructor without prior class validation; internal reconstruction callers also use ordinary small identities. The direct unit route is sufficient and does not claim replay of the parser. Compiler declaration and MIR CFG establish new -> from_identifier (bb0), Some -> ok_or_else -> Try::Continue (bb1-bb3), storage of that semantic class in DatabinId and Ok return (bb5 -> bb7). Source maps 3 to TileHeader and 7 to MainHeader after accepting every identifier <=8. class() exposes the stored ordinary class. Thus the exact failed check observes the forbidden semantic assignment.

Authority is the rfc_map parent, not the implementation-derived mapping contract. Independently read the parent-pinned T.808 (12/2022) A.2.2, SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc: Table A.2 defines only 0,1,2,4,5,6,8, and all other class identifiers are reserved. Oddness determines presence of AUX for parsing/skipping; it does not allocate extended tile-header or main-header semantics to 3 or 7. The clause and harness are limited to that edition with no separately supported extension. The module advertises standard message forms; SUPPORTED.md says reserved fields are not supported extensions. The oracle allows rejection or an unknown representation and does not strengthen the conditional recommendation to skip unknown messages into mandatory rejection. Consequently this is neither a harness error nor a clause overreach. The correlated native/Verus classification of 0..=8 cannot override the cited standard. The repository's 2026 catalog reference is not acquired normative text or an edition-difference review; this diagnosis makes no assertion about the unacquired 2026 edition or full JPIP conformance.

[Standards quote, production lines, and provenance](../../evidence.md#dev-2d35e4323c)

</details>
