# Ordinary JP2 rejects a resource-admitted second contiguous codestream instead of allowing first-codestream display

`DEV-457ACAB2C3` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § I.5.4

**Required behavior:** In support of ordinary JP2 display, the container reader shall permit later well-framed Contiguous Codestream boxes after the JP2 Header and allow display of the first codestream while ignoring subsequent codestreams, subject to resource admission.

**Observed deviation:** Ordinary JP2 rejects a resource-admitted second contiguous codestream instead of allowing first-codestream display

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-457ACAB2C3 --output rfc-results/DEV-457ACAB2C3
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 validation/jp2_spec_runner.py spec_later_jp2_codestreams_preserve_first_display
```

### Test input and recorded failure

Public parse accepts immutable bytes and a validated ResourceLimits without requiring an owning caller or private object invariants. The test uses ResourceLimits::default().with_maximum_codestreams(8).unwrap(), a 283-byte ordinary JP2 with two complete separately decoded repository codestreams, canonical signature/ftyp/header, valid lengths and legal post-header placement. Its first gray codestream matches the JP2 image header. The second complete RGB stream is intentionally distinct so first-stream display preservation is meaningful; T.800 I.5.4 says subsequent streams are ignored for display. Two streams are within eight, and all byte, depth, metadata, geometry, and work demands are small. No fixture setup assertion failed in host replay.

**Recorded failure** (matched by the replay runner):

```text
jp2::parse: ordinary JP2 accepts 2 complete well-framed codestreams within limit 8; Err(MalformedInput { offset: 176, kind: InvalidBoxContents }): false
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2/spec_tests.rs:L308-L310`](files/src/jp2/spec_tests.rs#L308-L310)

```text
    let limits = ResourceLimits::default()
        .with_maximum_codestreams(8)
        .unwrap();
```

[`src/jp2/spec_tests.rs:L312-L323`](files/src/jp2/spec_tests.rs#L312-L323)

```text
    for count in [1, 2, 3, 7, 8] {
        let mut input = parts[..3].to_vec();
        for index in 0..count {
            input.push(if index == 0 {
                parts[3].clone()
            } else {
                rgb[77..].to_vec()
            });
            input.push(box32(*b"zzzz", &[0xff]));
        }
        let bytes = input.concat();
        let result = parse(&bytes, &limits);
```

[`src/jp2/spec_tests.rs:L326-L328`](files/src/jp2/spec_tests.rs#L326-L328)

```text
            result.as_ref().is_ok_and(|file| {
                file.codestream_count() >= 1 && file.codestreams()[0].data() == &parts[3][8..]
            }),
```

[`src/jp2/spec_tests.rs:L16-L17`](files/src/jp2/spec_tests.rs#L16-L17)

```text
    assert_eq!(parts[0], b"\0\0\0\x0cjP  \r\n\x87\n");
    assert_eq!(parts[1], box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
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

The first failed predicate is 'ordinary JP2 accepts 2 complete well-framed codestreams within limit 8', reporting InvalidBoxContents at offset 176. This is a valid direct unit-contract entry to public parse(&[u8], &ResourceLimits), not a fabricated internal state or a fixture bypassing an owning caller. Resource realism: the two-stream input is 283 bytes: 77 bytes of signature/ftyp/jp2h, a 90-byte gray jp2c, a 9-byte opaque box, a 98-byte RGB jp2c, and another 9-byte opaque box. The largest admitted sweep input is 925 bytes with eight streams and eight shallow opaque boxes; the nine-stream negative control is 887 bytes. These are far below the 2 GiB input, 16 MiB metadata, depth 32, and one-billion work-unit ceilings. The configured stream limit is explicitly eight. Each fixture is a complete repository 1x1 codestream; the replay first successfully decodes the gray and RGB originals and checks their samples. The first stream and JP2 header match; subsequent standalone complete RGB streams need not determine the first image's component count. The signature, ordinary jp2 brand/compatibility, header placement and box extents are asserted by the replayed harness. No large allocation, excessive nesting, private-state manipulation, or hidden pool/stack limit explains this small input. The real decode_jp2 caller passes the same input and limits through parse_and_inspect, so this also blocks its ordinary JP2 display route.

Authority is the rfc_map origin, not agreement with existing implementation/oracles. I read its parent finding and retrieved T.800 (06/2019) I.5.4 using read_rfc_section under origin item 230760b6124854275489a5da. It expressly requires readers displaying an image to ignore codestreams after the first and permits contiguous codestream boxes anywhere except before jp2h. These fixtures meet that placement and complete-codestream requirement. SUPPORTED.md advertises JP2 typed output; its explicit no-implicit-first-selection exclusion is for JPX, not ordinary JP2. This supports the clause rather than a spec_error. Qualification: the supplied document is the 2019 edition, whereas project references select 2024; no edition reconciliation or whole-reader conformance claim is made here.

Compiler declaration and MIR CFG confirm the specific route: signature and ftyp validation; JP2 header parsed; first jp2c pushed; opaque box inspected; second jp2c at 176 computes actual=2. MIR bb217 takes the within-limit branch to bb222; actual>1 leads through bb223/bb224 with is_jpx_compatible=false, then bb225-bb227 constructs and returns InvalidBoxContents before bb228 can push that stream. This exact source branch violates the clause's admitted later-box acceptance and consequently prevents first-stream display. The displayed-output failure is downstream propagation of this parser error, not independent evidence that the later decode_jp2 count guard executed. The ninth-stream assertion is masked by the same earlier rejection and does not establish a separate resource-admission defect. The aggregate panic/exit alone is not the evidence; the first explicitly printed failed predicate and its matching source route are.

[Standards quote, production lines, and provenance](../../evidence.md#dev-457acab2c3)

</details>
