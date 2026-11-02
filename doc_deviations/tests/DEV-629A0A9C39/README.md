# JP2 header rejects reserved-method later colr boxes that the reader must ignore

`DEV-629A0A9C39` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § I.5.3.3

**Required behavior:** In JP2-reader mode, after selecting the first Colour Specification box, ignore later Colour Specification boxes rather than requiring their method-specific semantic parsing to succeed.

**Observed deviation:** JP2 header rejects reserved-method later colr boxes that the reader must ignore

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-629A0A9C39 --output rfc-results/DEV-629A0A9C39
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/jp2-header-spec-tests --exact jp2::tests::spec_header_ignores_later_colour_semantics --nocapture
```

### Test input and recorded failure

The tested entry is the explicitly available direct unit contract, with initialized safe Rust byte vectors and ResourceLimits::default(), not fabricated parser internals. ihdr is 13x17, three components, unsigned depth8, compression7, known colourspace/no-IP flags. Its first colr is METH1, PREC0, APPROX0, EnumCS16 sRGB, matching three unsigned channels. All later colr boxes are contiguous and correctly framed; the reserved box has the three common fields and only permitted opaque trailing bytes. Four boxes consume at most 94 bytes and four reader work units with no recursive nesting, comfortably within all applicable default limits. The selected base=40 case is within the input limit and has no offset overflow. The pinned 2019 standard explicitly requires readers to ignore these reserved methods, so these bytes are admissible for the reader-ignore obligation even though a JP2 writer is instructed to emit METH1 or METH2. Earlier successful baseline parsing establishes the entry's unaffected metadata is accepted.

**Recorded failure** (matched by the replay runner):

```text
method=255 extra=0 base=40: expected ignored colr, got MalformedInput { offset: 77, kind: InvalidColorSpecification }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2_header_spec_tests.rs:L112-L115`](files/src/jp2_header_spec_tests.rs#L112-L115)

```text
        let mut baseline = spec_ihdr(13, 17, 3, 7);
        baseline.extend(spec_colr());
        let expected = parse_jp2_header(&baseline, &limits, base).unwrap();
        assert_eq!(expected.color, ColorSpecification::enumerated(16));
```

[`src/jp2_header_spec_tests.rs:L111-L111`](files/src/jp2_header_spec_tests.rs#L111)

```text
    for base in [0, 40, 1 << 32] {
```

[`src/jp2_header_spec_tests.rs:L116-L118`](files/src/jp2_header_spec_tests.rs#L116-L118)

```text
        for method in [0, 4, 5, 127, 254, 255] {
            for extra in [0, 1, 7, 31] {
                let mut ignored = vec![method, 0, 0];
```

[`src/jp2_header_spec_tests.rs:L127-L129`](files/src/jp2_header_spec_tests.rs#L127-L129)

```text
                let mut bytes = baseline.clone();
                bytes.extend(encoded);
                bytes.extend(spec_colr());
```

[`src/jp2_header_spec_tests.rs:L133-L133`](files/src/jp2_header_spec_tests.rs#L133)

```text
                match parse_jp2_header(&bytes, &limits, base) {
```

[`src/jp2_header_spec_tests.rs:L140-L142`](files/src/jp2_header_spec_tests.rs#L140-L142)

```text
                    Err(error) => failures.push(format!(
                        "method={method} extra={extra} base={base}: expected ignored colr, got {error:?}"
                    )),
```

[`src/jp2_header_spec_tests.rs:L162-L166`](files/src/jp2_header_spec_tests.rs#L162-L166)

```text
    assert!(
        failures.is_empty(),
        "JP2 reader ignore predicate violated:\n{}",
        failures.join("\n")
    );
```

[`src/jp2_header_spec_tests.rs:L10-L12`](files/src/jp2_header_spec_tests.rs#L10-L12)

```text
fn spec_colr() -> Vec<u8> {
    box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16])
}
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jp2_header_spec_tests.rs`](files/src/jp2_header_spec_tests.rs)
- [`validation/build-jp2-header-spec-tests.py`](files/validation/build-jp2-header-spec-tests.py)

**Build commands** (in order):

```sh
python3 validation/build-jp2-header-spec-tests.py
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The failed check is the final assert!(failures.is_empty()) in spec_header_ignores_later_colour_semantics, not a fixture panic, build failure, or unexplained nonzero exit. The host output identifies concrete parse_jp2_header calls returning InvalidColorSpecification instead of ignoring a later colr. In particular method=255, extra=0, base=40 returns offset=77, exactly base+37, the second colr's header offset after the 22-byte ihdr and 15-byte first colr. The baseline parse and selected-sRGB assertion precede the failing variants and succeed; the negative checks also complete before the aggregated assertion.

Entry realism was checked: each invocation has a 13x17 image (221 pixels), three unsigned 8-bit components, compression=7, valid zero flags, and four flat, contiguous, correctly length-encoded boxes. Payload size is 63..94 bytes, versus default limits of 2,147,483,648 input bytes, 16,777,216 metadata bytes, 1,000,000 width/height, 100,000,000 pixels, 64 components, depth 32, and 1,000,000,000 work units. There is no recursive fixture, image-sized allocation, private-state fabrication, or aggregate resource stress. The replay has 72 sequential variants, not a single combined oversized input. Base=40 is a realistic JP2 header payload offset after signature, file-type, and superbox framing; the conclusion does not depend on base=2^32, which is diagnostic rebasing rather than allocated input. The actual container parser passes box_ref.payload, limits, and payload_offset directly to this function. This is an allowed direct unit-contract test with no declared requires, not a claimed full-file execution. Safe Rust slices and reader-produced BoxRefs satisfy the relevant callee premises; unsupported colour methods are documented rejected inputs of the semantic parser, not caller safety preconditions.

The clause origin is rfc_map. Reading its pinned T.800 (06/2019), I.5.3.3 confirms both 'A conforming JP2 reader shall ignore all Colour Specification boxes after the first' and Table I.9's explicit requirement to ignore the entire box for METH other than 1 or 2, permitting optional fields after APPROX. The fixture uses exactly this reserved-method case, with PREC=APPROX=0 and an already accepted first sRGB colr; it does not rely on malformed ICC data or truncated recognized-method data. The file-writer rule restricting METH to 1 or 2 does not negate the explicit reader obligation for other values. This diagnoses the cited 2019 reader rule, not conformance to the unreconciled project-selected 2024 edition or JPX selection rules.

Compiler declaration and MIR CFG confirm the route: after framing the later colr, bb72/bb73 dispatch to bb74 parse_color_specification; bb75/bb76 branch on its Result; error bb78 propagates through FromResidual before the color.is_none() guard in bb77/bb80 can be reached. The semantic parser's unsupported-method branch returns InvalidColorSpecification. Thus the code rejects a well-framed later reserved-method box that this JP2-reader clause requires ignoring. The output tail truncates the beginning of the aggregate report, but retains complete failing case descriptions, including the base=40 case used here; it is sufficient to identify the operation and failed predicate. No production modification or new replay is needed to interpret this host-observed failure.

[Standards quote, production lines, and provenance](../../evidence.md#dev-629a0a9c39)

</details>
