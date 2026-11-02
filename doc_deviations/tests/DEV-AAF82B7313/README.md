# Metadata-bin lookup incorrectly partitions one logical target by message codestream index

`DEV-AAF82B7313` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.3.6.1

**Required behavior:** Reassemble and resolve the metadata bins referenced by JPIP placeholders for semantic container reconstruction.

**Observed deviation:** Metadata-bin lookup incorrectly partitions one logical target by message codestream index

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-AAF82B7313 --output rfc-results/DEV-AAF82B7313
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 validation/run_metadata_contract_suite.py metadata_identity_ignores_message_csn
```

### Test input and recorded failure

One logical target per freshly initialized default-limit cache; four metadata IDs 0..3; valid JP2 root signature/brand, original-content placeholders and independently validated reconstruction; contents-only non-root bins; complementary out-of-order fragments with consistent final lengths; at most 342 transported bytes, eight insertions and depth 2. No production behavior is modified and no cache internals are forged. Root entry and fresh vectors match assemble_semantic_container exactly. T.808 A.3.6.1 requires ignoring metadata CSn, so selecting CSn 7 and mixing CSn 0/7 are admissible, rather than a requirement to supply absent codestream data.

**Recorded failure** (matched by the replay runner):

```text
iteration=11 mode=0 selected-CSn=7: complete=false, output=0/282, missing=[JpipMissingRange { databin: DatabinId { class_identifier: 8, class: Metadata, codestream: 7, in_class: 0 }, offset: 0, length: None }], needs=[JpipSemanticNeed { kind: MetadataBin, identifier: 0 }]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_metadata_contract_tests.rs:L164-L169`](files/src/jpip_metadata_contract_tests.rs#L164-L169)

```text
let (first, last) = match mode {
                    0 => (0, 0),
                    1 => (7, 7),
                    _ => (0, 7),
                };
                insert_split(&mut cache, *bin, bytes, first, last, split);
```

[`src/jpip_metadata_contract_tests.rs:L173-L175`](files/src/jpip_metadata_contract_tests.rs#L173-L175)

```text
for selected in [0, 7, u64::MAX] {
                let (complete, output, missing, needs) = expand(&cache, selected);
                if !complete || output != f.expected || !missing.is_empty() || !needs.is_empty() {
```

[`src/jpip_metadata_contract_tests.rs:L100-L110`](files/src/jpip_metadata_contract_tests.rs#L100-L110)

```text
.expand_metadata_bin(
            csn,
            0,
            true,
            0,
            u32::MAX,
            &mut active,
            &mut output,
            &mut missing,
            &mut needs,
        )
```

[`src/jpip_metadata_contract_tests.rs:L184-L188`](files/src/jpip_metadata_contract_tests.rs#L184-L188)

```text
assert!(
        failures.is_empty(),
        "metadata identity/expansion violated:\n{}",
        failures.join("\n")
    );
```

[`src/jpip_metadata_contract_tests.rs:L84-L84`](files/src/jpip_metadata_contract_tests.rs#L84)

```text
crate::jp2::parse_and_inspect(&expected, &ResourceLimits::default()).unwrap();
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_metadata_contract_tests.rs`](files/src/jpip_metadata_contract_tests.rs)
- [`validation/run_metadata_contract_suite.py`](files/validation/run_metadata_contract_suite.py)

**Build commands** (in order):

```sh
python3 validation/run_metadata_contract_suite.py build
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing assertion is the case's final assert!(failures.is_empty(), "metadata identity/expansion violated:\n{}", ...), not a build, setup, sanitizer, or resource failure. The retained host output specifically reports iteration=11 mode=0 selected-CSn=7 returning complete=false, zero output instead of 282 bytes, and a spurious missing root-bin request. Mode 0 inserted all metadata under CSn 0; selecting 7 must not hide that metadata. The test also exercises fragmented data spanning CSn 0 and 7, but the same-CSn mode-0 witness already suffices.

Realism/admission was checked before tracing: each fresh cache receives four logical metadata bins (at most eight implementation bins when CSn partitions fragments), at most eight fragments, at most 342 total transported payload bytes (160 root + 93 nested contents + 7 colour contents + 82 codestream bytes), and at most depth 2. Expected reconstructed output is at most 282 bytes. These are far below default/HARDENED limits: 2,147,483,648 input bytes, 16,777,216 metadata bytes, 1,048,576 fragments, and depth 32. remaining_depth=u32::MAX is the actual caller's recursion fuel, not requested nesting or allocation. CSn values are identifiers, not allocation counts; the decisive witness uses ordinary value 7, not u64::MAX. All insertions use public checked JpipFragment::new and DatabinCache::insert, with in-bounds complementary offsets, consistent final lengths, no conflicting overlaps, one logical target, and no fabricated private cache fields. The expected JP2 is independently constructed and parse_and_inspect succeeds before expansion. The direct unit entry uses exactly the root state initialized by assemble_semantic_container: bin=0, boxes=true, depth=0, remaining_depth=u32::MAX and fresh vectors. This is a valid direct unit test, not an owning-caller bypass with invented state.

Authority is the rfc_map parent, not the implementation-correlated identity contracts: its pinned T.808 (12/2022; ISO/IEC 15444-9:2023) A.3.6.1 was read through read_rfc_section using origin item c3e124e770eb767b4dca0e91. It explicitly says 'The codestream index shall be ignored in any message which has the metadata-bin class identifier.' This applies to both JPP/JPT metadata from one logical target and root bin 0; the fixture meets that scope. Thus the clause's metadata reassembly/resolution promise is sound under its parent finding, and codestream-sensitive implementation/model agreement cannot justify changing the oracle. This conclusion is scoped to the pinned edition, not unverified 2026 changes.

describe_declaration and the MIR CFG establish the route: root admission and the empty active-stack scan pass; bb19 constructs DatabinId(8, selected CSn, 0), bb23 calls is_complete, and bb25->bb27->bb32 appends missing/need information and returns Ok(false). Source lookup runs status/find_databin_index/same_as/jpip_databin_identity, whose unconditional codestream equality excludes the complete CSn-0 root when selected CSn is 7. insert uses this same lookup, so mixed-CSn fragments likewise fail to coalesce. No reached callee precondition requires a particular selected CSn or forbids this cache state. The consequence is false incompleteness and failure to resolve already available placeholder metadata. The output tail truncates the panic heading and earliest individual mismatch, but retains this precise failing-check diagnostic and failed test result; attribution uses those specifics, not exit code 101 alone.

[Standards quote, production lines, and provenance](../../evidence.md#dev-aaf82b7313)

</details>
