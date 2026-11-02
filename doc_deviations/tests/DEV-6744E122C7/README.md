# Stream-equivalent metadata payload is reconstructed with the original box identity

`DEV-6744E122C7` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.3.6.3

**Required behavior:** Reconstruct the supported metadata placeholder representation into correctly identified container boxes.

**Observed deviation:** Stream-equivalent metadata payload is reconstructed with the original box identity

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-6744E122C7 --output rfc-results/DEV-6744E122C7
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 validation/run_metadata_contract_suite.py stream_equivalent_uses_equivalent_box_identity
```

### Test input and recorded failure

A direct helper unit entry over public-API-established cache state, with a legal compact phld, complete bin 1, complete contents-only bin 2 on the failing phase, CSn 0 throughout, flags=2, ignored unavailable OrigID 99, EquivID 2, original xref header, equivalent colr header, consistent depth/budget and noncyclic ancestor identity [0]. Only 67 bytes maximum are cached and maximum reached depth is 2. Bin 1 represents superbox contents, not a complete file; the unit assumes the standard-permitted JPX xref-to-colr context, without claiming public assembler acceptance or full JPX conformance. The helper has no requirement to retrieve or validate the ancestor bin or unavailable original contents.

**Recorded failure** (matched by the replay runner):

```text
xref->colr original-XLBox=false, equivalent-XLBox=false: got [0, 0, 0, 15, 120, 114, 101, 102, 1, 0, 0, 0, 0, 0, 17], expected [0, 0, 0, 15, 99, 111, 108, 114, 1, 0, 0, 0, 0, 0, 17]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_metadata_contract_tests.rs:L267-L273`](files/src/jpip_metadata_contract_tests.rs#L267-L273)

```text
let original = header(*b"xref", 32, original_extended);
            let equivalent = header(*b"colr", color.len(), equivalent_extended);
            let mut fields = 2_u32.to_be_bytes().to_vec();
            fields.extend_from_slice(&99_u64.to_be_bytes());
            fields.extend_from_slice(&original);
            fields.extend_from_slice(&2_u64.to_be_bytes());
            fields.extend_from_slice(&equivalent);
```

[`src/jpip_metadata_contract_tests.rs:L282-L285`](files/src/jpip_metadata_contract_tests.rs#L282-L285)

```text
cache
                .insert(fragment(id(8, 1), 0, &placeholder, true))
                .unwrap();
            let mut active = vec![0];
```

[`src/jpip_metadata_contract_tests.rs:L309-L326`](files/src/jpip_metadata_contract_tests.rs#L309-L326)

```text
cache.insert(fragment(id(8, 2), 0, &color, true)).unwrap();
            missing.clear();
            needs.clear();
            assert!(
                cache
                    .expand_metadata_bin(
                        0,
                        1,
                        true,
                        1,
                        u32::MAX - 1,
                        &mut active,
                        &mut output,
                        &mut missing,
                        &mut needs,
                    )
                    .unwrap()
            );
```

[`src/jpip_metadata_contract_tests.rs:L330-L335`](files/src/jpip_metadata_contract_tests.rs#L330-L335)

```text
let expected = boxed(*b"colr", &color, equivalent_extended);
            if output != expected {
                failures.push(format!(
                    "xref->colr original-XLBox={original_extended}, equivalent-XLBox={equivalent_extended}: got {output:?}, expected {expected:?}"
                ));
            }
```

[`src/jpip_metadata_contract_tests.rs:L338-L342`](files/src/jpip_metadata_contract_tests.rs#L338-L342)

```text
assert!(
        failures.is_empty(),
        "wrong stream-equivalent box header:\n{}",
        failures.join("\n")
    );
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

The first host-observed failed assertion is the final failures.is_empty() check, reported as 'wrong stream-equivalent box header'. All preceding admission, missing-payload, completion, active-stack and diagnostic assertions passed. The first compact/compact case emits TBox xref (120,114,101,102) instead of colr (99,111,108,114), violating the exact correctly-identified-box clause; the other three cases also expose use of the original rather than equivalent header width.

Realism was checked before tracing: each of the four sequential cases uses one 44–60-byte placeholder and one 7-byte payload, at most 67 cached bytes, two bins, and recursion from depth 1 to 2 with no cycle. Output is at most 23 bytes. These are far below default maximum_input_bytes=2147483648, maximum_metadata_bytes=16777216 and maximum_box_depth=32. The u32::MAX-1 remaining_depth argument is the ordinary caller's decreasing arithmetic budget, not requested nesting or allocation. The tested context matches recursive box-contents expansion, not an oversized platform stress case.

This is an admissible direct unit-contract test, not a claimed public-assembler path. Public DatabinCache insertion and JpipFragment construction establish complete metadata bins at CSn 0; there is no private-cache corruption. A nonroot bin can contain a superbox's child sequence, and its sole placeholder is top-level within that bin. Entry active=[0], bin=1, boxes=true, depth=1 and remaining_depth=u32::MAX-1 are consistent recursion-context values; ancestor bytes are not a requirement of this helper, which only checks ancestor identities for cycles. A complete JPX file is not this helper's input. The test assumes a JPX host context permitting xref-to-colr substitution and does not establish full-file conformance, cross-reference resolution or pixel equivalence. Flags=2 disables original access, so absent OrigID=99 and unavailable original contents are permitted, not an unmet precondition.

Authority is the rfc_map origin, not the implementation-derived original-shell hypothesis. I read the pinned T.808 (12/2022), ISO/IEC 15444-9:2023, SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc, sections A.3.6.2 and A.3.6.3 through the originating mapping item. A.3.6.2 explicitly permits a Cross-reference box's stream equivalent to be a Colourspace Specification box, with a distinct equivalent box header and contents bin. A.3.6.3 defines EquivBH as that stream-equivalent header; Table A.3 says flags bit 0 clear means ignore OrigID, and bit 1 set supplies equivalent contents. Both compact and extended EquivBH are allowed, while the phld itself correctly uses compact framing. This supports the test oracle for the cited edition. It does not assert conformance to a newer catalog edition.

Compiler declaration and MIR CFG confirm the route: parse the placeholder; bypass flags&1; select flags&2, equivalent_bin=2 and equivalent_type=colr; verify equivalent_header exists but never use it; recursively copy bin 2 opaquely because colr is not a superbox; on successful replacement call append_reconstructed_box with original_header. That helper copies the supplied header and only rewrites its length. Thus valid colr contents are labeled xref, and original header width is retained. The host replay's exact byte assertion demonstrates this operation, rather than relying on a nonzero exit or the runner's printed route. Existing implementation-derived contracts explicitly qualify their original-shell policy as an unestablished equivalence hypothesis; they cannot override this standards evidence. No production changes or additional replay are needed to attribute this already observed mismatch.

[Standards quote, production lines, and provenance](../../evidence.md#dev-6744e122c7)

</details>
