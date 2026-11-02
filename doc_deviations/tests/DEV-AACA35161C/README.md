# JPX container validation accepts a nonempty codestream-header sequence shorter than the codestream sequence

`DEV-AACA35161C` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t801-202106.txt § M.11.6

**Required behavior:** For supported top-level JPX codestream representations, header i shall describe codestream i, and container validation shall check that a nonempty header sequence covers every codestream; if there are no headers, JP2 defaults apply to all streams.

**Observed deviation:** JPX container validation accepts a nonempty codestream-header sequence shorter than the codestream sequence

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-AACA35161C --output rfc-results/DEV-AACA35161C
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 validation/jp2_spec_runner.py spec_jpx_header_bijection_and_defaults
```

### Test input and recorded failure

This is a contract-valid direct unit call with ordinary immutable borrowed bytes and a valid ResourceLimits value. The prefix uses the fixed signature, jpx brand with jp2/jpx compatibility, rreq, and the original matching 1x1 grayscale JP2 header. Each jpch has a valid matching ihdr plus label and inherits default colour; each jp2c is copied from the repository's complete, independently decoded gray.jp2 fixture. The observed input is 1148 bytes, H=7,C=8, with both individual counts <=9 and negligible metadata/depth/work. The deliberate header shortfall is precisely the downstream file rule being validated, not an inadmissible parser entry. No omitted owning caller, forged private state, oversized configuration, incomplete codestream or production modification is needed.

**Recorded failure** (matched by the replay runner):

```text
codestream_header: None }], compositing_layers: [], composition: None, metadata_boxes: 1, metadata_depth: 1, box_count: 19 })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2/spec_tests.rs:L382-L384`](files/src/jp2/spec_tests.rs#L382-L384)

```text
    let limits = ResourceLimits::default()
        .with_maximum_codestreams(9)
        .unwrap();
```

[`src/jp2/spec_tests.rs:L386-L386`](files/src/jp2/spec_tests.rs#L386)

```text
    for count in [1_usize, 2, 3, 8] {
```

[`src/jp2/spec_tests.rs:L404-L404`](files/src/jp2/spec_tests.rs#L404)

```text
        for header_count in [0, count, count + 1, count.saturating_sub(1)] {
```

[`src/jp2/spec_tests.rs:L411-L421`](files/src/jp2/spec_tests.rs#L411-L421)

```text
            input.extend((0..count).map(|_| parts[3].clone()));
            let bytes = input.concat();
            let result = parse(&bytes, &limits);
            if header_count != 0 && header_count != count {
                spec_record(
                    &mut failures,
                    matches!(result, Err(DecodeError::MalformedInput { .. })),
                    format!(
                        "JPX streams={count}, headers={header_count}: reject unequal nonempty sequence; {result:?}"
                    ),
                );
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

The failed assertion is spec_finish's failures.is_empty(), which aggregates the exact clause check matches!(result, Err(DecodeError::MalformedInput { .. })) whenever header_count != 0 && header_count != count. This is not a crash, setup failure, or incorrect expectation about ordinal association. The retained host output contains the successful Jp2File suffix with an unassociated last stream and box_count: 19, identifying the replayed C=8,H=7 shortfall fixture (4 prefix boxes + 7 jpch + 8 jp2c). Earlier in test order the same check is exercised at C=2,H=1; the output tail does not retain that earlier diagnostic, so the grounded observation here is the retained C=8,H=7 result, not a claim to have seen the earlier line.

Entry realism was checked before deeper tracing: the suite uses at most 8 complete 1x1, one-component codestreams and 9 small jpch boxes, with maximum_codestreams explicitly set to 9. The largest constructed input is 1242 bytes; the observed C=8,H=7 input is 1148 bytes. These are far below HARDENED/default input 2147483648 bytes, metadata 16777216 bytes, depth 32, width/height 1000000, component 64 and work 1000000000 limits. There is no deep recursion, fixture pool, unsafe state fabrication or extraordinary allocation. Real decode_jpx_codestream* and parse_and_inspect callers pass borrowed file bytes and configured limits directly to parse; the assessed direct unit contract has no additional requires. The intentionally inconsistent header count is a file-validation predicate, not a precondition on this public byte parser.

Authority is the rfc_map origin, not agreement with existing implementation-derived specs. I read the assigned T.801 (06/2021), ISO/IEC 15444-2:2021 M.11.6 with read_rfc_section using origin item 230760b6124854275489a5da and confirmed its manifest SHA/version 985af26033db03edf53ce28b360107fb33b78d1a6af09b6d92ea72e89e277eb1. It expressly states: 'If Codestream Header boxes appear anywhere in the file, the number of codestreams found in the file shall be the same as the number of available codestream headers.' It separately specifies ordinal pairing and the no-header defaults case. These fixtures contain only supported top-level jpch and jp2c representations: no fragments, multiple-codestream boxes, cross-references or compositing-layer extensions can supply missing headers. The clause requires this check in the selected container-validation role; this diagnosis does not infer a general obligation for every tolerant JPEG 2000 reader to reject every nonconforming file.

describe_declaration establishes parse(&[u8], &ResourceLimits) -> Result<Jp2File, DecodeError>. Source and control_flow_graph trace the route: accept signature, JPX-compatible ftyp, opaque rreq, jp2h, then H valid jpch inheriting matching defaults, followed by C complete jp2c. At EOF MIR bb248-bb250 computes only H>C; for 7<8 it takes bb253. bb254-bb275 associates exactly H headers; bb277-bb282 validates the empty layer list and returns Ok, leaving stream 7's association None. That violates this clause's nonempty coverage check, although it agrees with weaker implementation-inferred clauses. The replay's test-only include patch changes no production behavior and the runner built and executed the named Rust test successfully. Diagnostics are unnecessarily verbose and truncate the earliest accumulated failures, but the retained successful result, exact harness construction and specific failed predicate still identify this counterexample; the nonzero exit alone is not the evidence.

[Standards quote, production lines, and provenance](../../evidence.md#dev-aaca35161c)

</details>
