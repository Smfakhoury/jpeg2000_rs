# JPIP single-codestream placeholders reject a permitted present-but-ignored NCS field

`DEV-B80F74BB23` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.3.6.3

**Required behavior:** Extract incremental codestream identifiers and stream counts from received metadata placeholders for the semantic assembler.

**Observed deviation:** JPIP single-codestream placeholders reject a permitted present-but-ignored NCS field

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-B80F74BB23 --output rfc-results/DEV-B80F74BB23
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/placeholder-contract-tests jpip::tests::placeholder_contract_stream_identifiers_and_counts --exact --nocapture
```

### Test input and recorded failure

First rejected payload: flags=4, OrigID=7, ordinary OrigBH jp2c length 256, intermediate EquivID=9 and ordinary EquivBH jp2c length 256, CSID=0, present NCS=0. Total 48 payload bytes, outer ordinary phld length 56. Complete required fields, valid slices, supported low flags, no unknown extensions, no stream-ID overflow. T.808 (12/2022) Table A.3 requires effective NCS=1 for 01xx even with a present zero-valued NCS. Referenced lengths do not allocate content, and no stream-array expansion or cache-state fabrication occurs.

**Recorded failure** (matched by the replay runner):

```text
736 legal placeholders rejected; first failures:
flags=0x4, CSID=0x0, NCS=Some(0), wide=false: MalformedInput { offset: 0, kind: InvalidBoxContents }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_placeholder_contract_tests.rs:L115-L123`](files/src/jpip_placeholder_contract_tests.rs#L115-L123)

```text
                    let payload = contract_payload(
                        flags,
                        7,
                        &header,
                        Some((9, &header)),
                        Some(stream),
                        count,
                    );
                    match parse_jpip_placeholder(&payload) {
```

[`src/jpip_placeholder_contract_tests.rs:L50-L53`](files/src/jpip_placeholder_contract_tests.rs#L50-L53)

```text
    let mut reader = crate::jp2::BoxReader::new(&framed, &ResourceLimits::default()).unwrap();
    let received = reader.next_box_ref().unwrap().unwrap();
    assert_eq!(received.payload, payload);
    assert!(reader.next_box_ref().is_none());
```

[`src/jpip_placeholder_contract_tests.rs:L113-L114`](files/src/jpip_placeholder_contract_tests.rs#L113-L114)

```text
                    let expected = if flags & 8 == 0 { 1 } else { count.unwrap() };
                    assert!(stream.checked_add(u64::from(expected) - 1).is_some());
```

[`src/jpip_placeholder_contract_tests.rs:L173-L176`](files/src/jpip_placeholder_contract_tests.rs#L173-L176)

```text
    assert!(
        failures.is_empty(),
        "{} legal placeholders rejected; first failures:\n{}",
        failures.len(),
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_placeholder_contract_tests.rs`](files/src/jpip_placeholder_contract_tests.rs)
- [`validation/build_placeholder_contract_tests.py`](files/validation/build_placeholder_contract_tests.py)

**Build commands** (in order):

```sh
python3 validation/build_placeholder_contract_tests.py
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is failures.is_empty() at src/jpip_placeholder_contract_tests.rs:173, reporting 736 legal placeholders rejected. Its first concrete case is flags=4, CSID=0, NCS=Some(0), wide=false, returning InvalidBoxContents. This is a valid direct unit-contract entry, not an incomplete DatabinCache fixture. The input is a complete 48-byte payload (56 bytes including ordinary phld framing): Flags/OrigID/8-byte OrigBH/EquivID/8-byte EquivBH/CSID/4-byte NCS. Both referenced headers declare length 256 and type jp2c; their content is not required inside a placeholder. The test validates framing and payload extraction using BoxReader before calling the parser. Zero is permitted in this unused NCS field: effective count is one. The output also independently reports rejection with NCS=Some(1), so the deviation does not depend on zero or an extreme count.
Resource realism: the main matrix is 2 header widths x 23 identifiers x 8 flag values x 5 count representations = 1840 independent parser calls. Each payload is at most 64 bytes (72 framed), with no nesting, recursive reconstruction, large referenced-content allocation, or stream enumeration. NCS and CSID are scalar wire fields; the single-stream failures always request one effective stream, including when encoded NCS is UINT32_MAX. These are far below HARDENED input/metadata limits (2147483648/16777216 bytes), depth 32, and codestream capacity 64. Multi-stream scalar parse tests do not invoke downstream reconstruction or claim it must accept billions of streams. Real caller expand_metadata_bin likewise passes box_ref.payload directly after BoxReader identifies phld. The owning parser has no cache-state or resource-limit parameter; this test respects its slice and read-helper requirements.
Authority is the rfc_map parent, not correlated implementation models. I read its pinned T.808 (12/2022) A.3.6.3, SHA-256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc. Table A.3 explicitly requires 01xx to treat NCS as 1 regardless of its actual value when present; surrounding prose permits termination after CSID but does not require it, and requires intermediate fields even if unused. The harness includes these intermediate fields. Thus this is a concrete refutation of the mapped extraction clause under its originating standards interpretation, not a clause requiring revision to match the parser. REFERENCES.md explicitly leaves the newer 2026 edition unaudited; this finding is bounded to the cited 2022 text and does not establish a newer-edition conformance failure.
Source and MIR identify the exact route: after complete headers, flags & 4 reads CSID and advances position to 44. Since flags & 8 is zero, MIR bb106 -> bb109 sets count=1 without consuming optional NCS. bb124 observes 44 != 48, bb125/bb126/bb127 bypass the provisional-layout diagnostic (48 != original_end+24, which is 44), and bb130/bb131 return InvalidBoxContents. The required result was successful extraction of CSID=0 and stream_count=1. The 736 failures equal both widths x 23 IDs x four 01xx flags x four present NCS values. The compatibility diagnostic remains an error, not an accepting path. No production modification, arbitrary nonzero exit, or unrelated unused-high-bit failure is used as evidence; the recorded successful build and concrete parser error establish this specific deviation.

[Standards quote, production lines, and provenance](../../evidence.md#dev-b80f74bb23)

</details>
