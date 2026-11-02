# JP2 validation accepts additional top-level JPEG 2000 signature boxes

`DEV-2D75F3E8F6` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § I.5.1

**Required behavior:** JP2 container validation shall recognize the fixed JPEG 2000 signature as the first box and validate the file rule that exactly one such signature exists.

**Observed deviation:** JP2 validation accepts additional top-level JPEG 2000 signature boxes

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-2D75F3E8F6 --output rfc-results/DEV-2D75F3E8F6
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 validation/jp2_spec_runner.py spec_signature_first_fixed_and_unique
```

### Test input and recorded failure

A direct public parser unit test supplies owned, fully framed bytes and a standard immutable ResourceLimits::default(), with no forged internal state or bypassed owning caller. The complete original 167-byte fixture is verified/decoded successfully and its canonical 12-byte signature alone is duplicated, yielding 179 bytes, five top-level boxes and a 1x1 single-component image. Resource requirements are tiny relative to HARDENED limits. Signature uniqueness is the intended rejected property, not an entry requirement for this validation API; its assessed unit-contract requires list is empty.

**Recorded failure** (matched by the replay runner):

```text
second signature at box 2; malformed rejection, got Ok(Jp2File
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2/spec_tests.rs:L5-L5`](files/src/jp2/spec_tests.rs#L5)

```text
    let bytes = include_bytes!("../../validation/image-consumer-probe/tests/fixtures/gray.jp2");
```

[`src/jp2/spec_tests.rs:L68-L68`](files/src/jp2/spec_tests.rs#L68)

```text
    assert!(parse(&parts.concat(), &ResourceLimits::default()).is_ok());
```

[`src/jp2/spec_tests.rs:L87-L95`](files/src/jp2/spec_tests.rs#L87-L95)

```text
    for position in 2..=4 {
        let mut duplicate = parts.clone();
        duplicate.insert(position, parts[0].clone());
        spec_reject(
            &mut failures,
            &duplicate.concat(),
            format!("second signature at box {position}"),
        );
    }
```

[`src/jp2/spec_tests.rs:L47-L52`](files/src/jp2/spec_tests.rs#L47-L52)

```text
    let result = parse(bytes, &ResourceLimits::default());
    spec_record(
        failures,
        matches!(result, Err(DecodeError::MalformedInput { .. })),
        format!("{}; malformed rejection, got {result:?}", context.into()),
    );
```

[`src/jp2/spec_tests.rs:L56-L60`](files/src/jp2/spec_tests.rs#L56-L60)

```text
    assert!(
        failures.is_empty(),
        "clause violations:\n{}",
        failures.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jp2/spec_tests.rs`](files/src/jp2/spec_tests.rs)
- [`jp2-spec-registration.patch`](files/jp2-spec-registration.patch)
- [`validation/jp2_spec_runner.py`](files/validation/jp2_spec_runner.py)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i jp2-spec-registration.patch
```

```sh
python3 validation/jp2_spec_runner.py build
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is spec_finish's failures.is_empty() at src/jp2/spec_tests.rs:56; its first recorded failed check is 'second signature at box 2; malformed rejection, got Ok(Jp2File ...'. This specifically refutes the clause's signature-uniqueness requirement, not merely an exit-code expectation. The later box-3 and box-4 checks reproduce the same omission. Entry realism: the repository's complete gray.jp2 fixture is 167 bytes; inserting its canonical 12-byte signature produces a 179-byte input with five top-level boxes, one 1x1 8-bit grayscale component, one codestream, two header children, and no association recursion. Top-level non-codestream framing totals 89 bytes. These are far below default HARDENED limits (2,147,483,648 input bytes, 16,777,216 metadata bytes, depth 32, 64 codestreams/components, 100,000,000 pixels, 1,000,000,000 work units). This is the same byte-slice/resource-limit entry shape used by real decode callers, using their normal tiny grayscale fixture, not an artificial private-state fixture. The unit-contract entry has no requires, and rejecting structurally invalid container bytes is the operation under test: a duplicate signature violates the target validation predicate, not a caller precondition. Harness setup also verifies canonical ftyp/header/framing, decodes the original fixture to its expected 1x1 sample, and successfully parses the original before adding the duplicate. Registration only includes tests within the existing cfg(test) module; production behavior is unchanged. Compiler describe_declaration confirms the &[u8], &ResourceLimits -> Result<Jp2File, DecodeError> signature. The control-flow graph and source show first-signature validation followed by a loop testing jp2h/jpch/jplh/comp/jp2c but not SIGNATURE; the duplicate falls into inspect_metadata_box, whose non-ASSOCIATION leaf path returns Ok((1, depth)). Parsing then reaches Ok(Jp2File), counting the duplicate as one metadata box. Standard authority was checked, not inferred from agreement with project tests/models: clause_origin is rfc_map, and read_rfc_section on its originating mapping confirms T.800 (2019-06) I.5.1 explicitly requires first position and 'one and only one' signature. This file-validity requirement applies to the advertised JP2 container-validation endpoint and the ordinary jp2-compatible fixture. It is not a claim that every permissive reader must reject every nonconforming file to meet reader conformance. The available standard edition is 2019, while the project lists a 2024 target; no edition reconciliation or whole-reader conformance claim is made. That traceability limitation is retained, but does not contradict the explicit cited rule or justify weakening this clause to match the implementation. The observed deviation is specifically acceptance contrary to that clause's validation guarantee. No resource/setup error or unsupported error category masks it: the host records successful parse results for the offending inputs.

[Standards quote, production lines, and provenance](../../evidence.md#dev-2d75f3e8f6)

</details>
