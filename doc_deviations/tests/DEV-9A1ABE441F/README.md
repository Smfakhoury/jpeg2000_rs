# Bounded standard JPIP VBAS fields with redundant zero groups are rejected as malformed

`DEV-9A1ABE441F` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.2.1

**Required behavior:** Decode bounded standard binary JPIP header fields by preserving their payload value and field boundary, with checked rejection of unsupported numeric extents.

**Observed deviation:** Bounded standard JPIP VBAS fields with redundant zero groups are rejected as malformed

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-9A1ABE441F --output rfc-results/DEV-9A1ABE441F
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --lib --locked jpip::tests::bounded_standard_vbas_contract -- --exact --nocapture
```

### Test input and recorded failure

This is a real eight-byte slice with fresh JpipHeaderContext and default ResourceLimits, not fabricated parser internals. Independent form bb=11 explicitly supplies Class and CSn; Class payload [0x80,0x08] denotes metadata class 8, valid for both JPP/JPT; CSn=0, Bin-ID=0, offset=0, length=1, c=0, and even Class means no Aux. Body [0xab] exists and 0x5a is the following unconsumed sentinel. Both padded fields are two bytes, well within MAX_VBAS_BYTES=10, and all values and extents fit u64/usize and default input limit 2147483648. The direct-unit cases likewise use valid Rust references, actual small slices and the two caller-used masks; redundant-zero fields are at most ten bytes and preserve mathematical payload. There is no callee precondition requiring a nonzero first group: the canonical predicate is total over u8/bool, and its false return is the implementation decision being tested.

**Recorded failure** (matched by the replay runner):

```text
parse_jpip_message -> read_vbas, independent metadata header [60, 80, 08, 00, 00, 01, ab, 5a]: expected value-preserving Data, got Err(MalformedInput { offset: 1, kind: InvalidJpipVbas })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`validation/jpip_vbas_contract.rs:L130-L134`](files/validation/jpip_vbas_contract.rs#L130-L134)

```text
    for (class, length) in [
        (&[8][..], &[1][..]),
        (&[0x80, 8][..], &[1][..]),
        (&[8][..], &[0x80, 1][..]),
    ] {
```

[`validation/jpip_vbas_contract.rs:L135-L140`](files/validation/jpip_vbas_contract.rs#L135-L140)

```text
        let mut input = vec![0x60];
        input.extend_from_slice(class);
        input.extend_from_slice(&[0, 0]);
        input.extend_from_slice(length);
        let header_length = input.len();
        input.extend_from_slice(&[0xab, 0x5a]);
```

[`validation/jpip_vbas_contract.rs:L142-L143`](files/validation/jpip_vbas_contract.rs#L142-L143)

```text
        let mut context = JpipHeaderContext::new();
        match parse_jpip_message(&input, &mut context, &ResourceLimits::default()) {
```

[`validation/jpip_vbas_contract.rs:L157-L160`](files/validation/jpip_vbas_contract.rs#L157-L160)

```text
            other => failures.push(format!(
                "parse_jpip_message -> read_vbas, independent metadata header \
                 {input:02x?}: expected value-preserving Data, got {other:?}"
            )),
```

[`validation/jpip_vbas_contract.rs:L163-L168`](files/validation/jpip_vbas_contract.rs#L163-L168)

```text
    assert!(
        failures.is_empty(),
        "{} standard VBAS checks failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`validation/jpip_vbas_contract.rs`](files/validation/jpip_vbas_contract.rs)
- [`validation/install-jpip-vbas-contract.py`](files/validation/install-jpip-vbas-contract.py)

**Build commands** (in order):

```sh
python3 validation/install-jpip-vbas-contract.py
```

```sh
cargo test --lib --locked --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

Attribute this exact standards-derived clause 6b2c861fc7792bec596e0b48 to code_deviation, not the separate canonical-only inferred clauses. Reality check first: the suite has 703 canonical values, each tested sequentially with caller-used masks; the longest successful-field fixture is 3 prefix + 10 field + 3 suffix = 16 bytes. The eleven-byte negative field plus prefix is 14 bytes and is expected to reject, not succeed. Public positive witnesses are seven/eight bytes with a one-byte body. No recursion, enormous object, stack/pool pressure, unsupported numeric extent or impossible cursor is involved. Counts do not become parser input size. Default input capacity is 2 GiB; the public witness has offset+length=1. Real parse_jpip_message callers use the same None mask for Class and Msg-Length, so these are not merely disconnected private-function states.

The host build succeeded and host replay exited 101 with concrete value/boundary mismatches and both public-message errors. The failed assertion in the harness is the final failures.is_empty(), accumulating checks actual != Ok(expected) || position != prefix + field.len() and unexpected public-message results. The retained tail does not expose the earliest accumulated mismatch; I do not invent that first case. It does contain complete individual failures, including the observation quote and a direct unit example: read_vbas mask=None, field=[80, 81, 80, 00], start=0: expected Ok(16384) at 4, got Err(MalformedInput { offset: 0, kind: InvalidJpipVbas }) at 1. These directly contradict preservation of a bounded standard field's payload/boundary, rather than merely showing an arbitrary nonzero exit. Canonical and negative-control assertions preceding the final collection do not explain away these errors.

Authority: clause_origin is rfc_map dc9db665e258a9562dc090af, parent finding 28580cd70f92601009d53cc22676c850fe46dda58f07d6e8943318325d231890. I independently read original indexed T.808 (12/2022), SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc, A.2.1 anchor 811b789e3885dd767654c636fd2b2d671f29562ca6e2c162f494bf96cddafcff and A.2.2 anchor 02738b07ac711a51f6242015308bf1724b620044dca76fbb4b9f473ddb9b7bc1 through read_rfc_section using the parent mapping item. A.2.1 defines continuation-bit segmentation and explicitly defines Class, CSn, Msg-Offset and Msg-Length as nonnegative integers from seven-bit groups concatenated in big-endian order. [80,08] therefore denotes 8, [80,01] denotes 1; neither changes the terminal byte or following field. The inspected section imposes no shortest-form restriction. Table A.1 permits independent bb=11; A.2.2 assigns Class 8 to metadata in both stream types and Aux iff odd. This is receiving-side supported binary-header syntax, not excluded network/request/cache signaling or arbitrary metadata interpretation. The local profile advertises standard independent headers and checked bounded VBAS, without documenting a shortest-form exclusion. A corpus search for 'leading zero' returned no T.808 section; this search alone is not proof of global absence, but agrees with the full relevant syntax definition and supplied parent evidence.

Compiler evidence confirms the exact cause: describe_declaration reports a total Rust signature on slice/mutable cursor/optional mask. MIR CFG bb12 commits the first cursor increment, bb14 selects the None path, bb19 calls canonical(0,true), bb21 takes false to bb22/bb23 returning InvalidJpipVbas before loop bb24. The helper has no requires and correctly returns false per its own implementation/model. The defect is imposing that extra acceptance restriction in read_vbas, not violating the helper's contract. The checked arithmetic helper is not reached on this failure. Thus the unit entry is admissible and the complete-message witness also traverses the actual owning parser. Existing ghost canonical rules/native rejection tests describe the same restriction; they are not independent normative shortest-form authority and cannot justify spec_error.

Qualification: this is an agent-assessed reproduced divergence from the mapped, locally acquired 2022 syntax and the stated bounded-header contract, not whole-edition certification or formal reachability proof. REFERENCES.md explicitly says selected 2026 T.808 is only a catalog record without acquired text/edition reconciliation. PDF-converted figures were not independently reconciled; I do not establish that the 2026 edition is identical. Preserve that cross-edition/diagram uncertainty for review without rewriting this rfc_map clause to fit the implementation. All rejected redundant-zero encodings and both public witnesses share one root cause, including the host-noted F11 overlap; do not count them as separate defects. External diagnosis-guidance.json access was permission-denied; embedded host review_notes and original replay artifacts were read. No production/model/spec/test changes or additional execution were made.

[Standards quote, production lines, and provenance](../../evidence.md#dev-9a1abe441f)

</details>
