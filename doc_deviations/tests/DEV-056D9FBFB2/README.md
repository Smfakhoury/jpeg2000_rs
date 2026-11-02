# JP2 validation accepts duplicate File Type boxes as opaque metadata

`DEV-056D9FBFB2` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § I.5.2

**Required behavior:** JP2 container validation shall require the File Type box immediately after the signature and validate that the file contains exactly one File Type box.

**Observed deviation:** JP2 validation accepts duplicate File Type boxes as opaque metadata

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-056D9FBFB2 --output rfc-results/DEV-056D9FBFB2
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 validation/jp2_spec_runner.py spec_file_type_immediate_and_unique
```

### Test input and recorded failure

A safe direct call to the actual parse implementation with a fully owned 187-byte buffer and valid default ResourceLimits. The baseline 167-byte JP2 is independently exercised through decode_jp2, yielding one 1x1 component with sample 128, and through parse successfully. Only an intact 20-byte ftyp is duplicated; all lengths, the JP2 header, and the codestream remain unchanged. The constructed input is deliberately nonconforming in the exact cardinality being validated, but the parser API has no precondition that untrusted input already conform. No internal state is fabricated and no resource limit is approached.

**Recorded failure** (matched by the replay runner):

```text
second ftyp at box 2; malformed rejection, got Ok(Jp2File
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2/spec_tests.rs:L17-L17`](files/src/jp2/spec_tests.rs#L17)

```text
    assert_eq!(parts[1], box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
```

[`src/jp2/spec_tests.rs:L27-L27`](files/src/jp2/spec_tests.rs#L27)

```text
    let image = crate::decode::decode_jp2(bytes, &crate::DecodeOptions::default()).unwrap();
```

[`src/jp2/spec_tests.rs:L116-L124`](files/src/jp2/spec_tests.rs#L116-L124)

```text
    for position in 2..=4 {
        let mut duplicate = parts.clone();
        duplicate.insert(position, parts[1].clone());
        spec_reject(
            &mut failures,
            &duplicate.concat(),
            format!("second ftyp at box {position}"),
        );
    }
```

[`src/jp2/spec_tests.rs:L46-L53`](files/src/jp2/spec_tests.rs#L46-L53)

```text
fn spec_reject(failures: &mut Vec<String>, bytes: &[u8], context: impl Into<String>) {
    let result = parse(bytes, &ResourceLimits::default());
    spec_record(
        failures,
        matches!(result, Err(DecodeError::MalformedInput { .. })),
        format!("{}; malformed rejection, got {result:?}", context.into()),
    );
}
```

[`src/jp2/spec_tests.rs:L55-L61`](files/src/jp2/spec_tests.rs#L55-L61)

```text
fn spec_finish(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "clause violations:\n{}",
        failures.join("\n")
    );
}
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

The first failed check is the malformed-input rejection for 'second ftyp at box 2'; the final failures.is_empty() assertion aggregates that failure and equivalent duplicates at positions 3 and 4. Host replay reports Ok(Jp2File), not a setup failure or merely a nonzero exit. The entry is realistic: the 167-byte gray.jp2 fixture becomes 187 bytes after inserting one 20-byte ftyp, with five top-level boxes, two JP2-header children, one 1x1 8-bit component, one codestream, one compatible brand per ftyp, and metadata inspection depth 1. ResourceLimits::default() is HARDENED (2,147,483,648 input bytes, 16,777,216 metadata bytes, depth 32, 64 brands, 64 codestreams, and 1,000,000,000 work units); no large allocation, recursion, fixed pool or platform limit is approached. Real decoder interfaces also pass immutable input slices and ResourceLimits directly to jp2::parse. This is a valid direct unit-contract test: parse accepts untrusted byte slices for validation, not only already-conforming JP2 files. Deliberately duplicating a framed box is the validation target, not an inadmissible fabricated internal state. The unmodified fixture is decoded and its sample 128 verified before mutation, and the unmodified parse is checked for success. The test-only patch only includes the harness inside the existing test module. The compiler declaration confirms the slice/limits API; the MIR CFG confirms FILE_TYPE is checked for the second box only, later dispatch falls through to inspect_metadata_box (bb229-bb233), and the function ultimately returns Ok (bb282). Source inspection shows ftyp is not handled by any later dedicated branch and inspect_metadata_box immediately accepts non-association boxes. Thus the duplicate is counted as metadata, never checked for uniqueness, and a successful Jp2File is returned. The rfc_map origin was assessed rather than overridden by implementation intent: its parent finding cites T.800 (06/2019), I.5.2, and reading that section confirms both immediate placement and 'All files shall contain one and only one File Type box.' The fixture's jp2 brand and jp2 compatibility make this JP2 rule applicable. Together with the published JP2 'Container validation' scope and this clause's explicit validation requirement, the observed acceptance violates precisely the uniqueness guarantee. This does not claim the standard requires every reader to reject every malformed input, nor establish conformance to a later edition; the origin's unreconciled-edition qualification remains. There is no standards-specific contrary evidence supporting spec_error here. The malformed-error variant oracle does not affect this attribution because the actual result is Ok, which fails any rejection oracle.

[Standards quote, production lines, and provenance](../../evidence.md#dev-056d9fbfb2)

</details>
