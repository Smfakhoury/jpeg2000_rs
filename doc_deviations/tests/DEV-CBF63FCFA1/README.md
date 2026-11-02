# JPX parser admits nonzero codestream-header deficits and exposes unassociated codestreams

`DEV-CBF63FCFA1` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t801-202106.txt § M.11.6

**Required behavior:** For admitted JPX files with explicit codestream headers, expose each codestream together with its ordinal header association; nonzero header/codestream cardinality mismatches should not be admitted as a valid association set.

**Observed deviation:** JPX parser admits nonzero codestream-header deficits and exposes unassociated codestreams

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-CBF63FCFA1 --output rfc-results/DEV-CBF63FCFA1
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --locked --lib jp2::tests::retains_and_inspects_explicit_jpx_codestreams_and_headers -- --exact --nocapture
```

### Test input and recorded failure

Public byte-parser entry accepts a borrowed byte slice and valid ResourceLimits; it need not assume cardinality-valid input because rejection of that malformed structure is precisely the tested obligation. Fixture uses the retained 167-byte grayscale JP2's actual image/color and codestream bytes, JPX signature/type and rreq framing, one explicit top-level header for two streams, and one default compositing layer. Payload lifetimes are valid and all getter indices are in range on the actual parse-produced receiver. Smallest failing input is 357 bytes; all cases stay below 10KB, at most 64 streams/headers, with one layer and shallow nesting. Positive zero-header and equal-header controls pass. The deliberate header deficit is not private-state fabrication or a caller-contract violation.

**Recorded failure** (matched by the replay runner):

```text
N=2, H=1: admitted 2 streams, missing headers at [1]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`deepspec_codestream.patch:L14-L14`](files/deepspec_codestream.patch#L14)

```text
let gray = include_bytes!("../validation/image-consumer-probe/tests/fixtures/gray.jp2");
```

[`deepspec_codestream.patch:L117-L117`](files/deepspec_codestream.patch#L117)

```text
(2, 1), (3, 2), (8, 7), (64, 63), (64, 1),
```

[`deepspec_codestream.patch:L121-L121`](files/deepspec_codestream.patch#L121)

```text
match parse(&bytes, &limits) {
```

[`deepspec_codestream.patch:L126-L126`](files/deepspec_codestream.patch#L126)

```text
.filter(|&index| file.codestream(index).unwrap().codestream_header().is_none())
```

[`deepspec_codestream.patch:L136-L136`](files/deepspec_codestream.patch#L136)

```text
admitted_mismatches.is_empty(),
```

[`deepspec_codestream.patch:L137-L137`](files/deepspec_codestream.patch#L137)

```text
"T.801 M.11.6 forbids nonzero cardinality mismatches:\n{}"
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`deepspec_codestream.patch`](files/deepspec_codestream.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i deepspec_codestream.patch
```

```sh
python3 -c 'import hashlib; from pathlib import Path; assert hashlib.sha256(Path('"'"'src/jp2.rs'"'"').read_bytes()).hexdigest() == '"'"'85817f58973870ac765c4ba110650fffcd4b43a5e20c9a5c04a92bb15dfb5750'"'"', '"'"'replay source must contain the entire additive assertion extension'"'"''
```

```sh
cargo test --locked --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is the final admitted_mismatches.is_empty() check, not fixture setup, decoding, or a resource failure. Host output specifically reports 'N=2, H=1: admitted 2 streams, missing headers at [1]'; all H=N and H=0 controls and the four H>N rejection cases completed before that assertion. Entry realism: the smallest failing case is only 357 bytes with two 1x1, one-component codestreams and one explicit header. The largest constructed case is 9,811 bytes, with at most 64 streams, 64 headers, one layer and shallow box nesting, within configured limits of 64 codestreams, 256 layers, 32 box depth, 16,777,216 metadata bytes and 2,147,483,648 input bytes. There is no large allocation or stack-exhaustion explanation. Native decode_jpx_codestream also passes caller-supplied bytes to this same public parser with options.limits(); the test does not invent an internal receiver or bypass its owning producer.

Authority is the rfc_map origin, not agreement with existing code or models. Reading its pinned T.801 (06/2021), SHA-256 985af26033db03edf53ce28b360107fb33b78d1a6af09b6d92ea72e89e277eb1, M.11.6 confirms both ordinal association and the explicit requirement that if any Codestream Header boxes exist, codestream and available-header counts shall be equal. The zero-header default exception does not apply to H=1,N=2. This fixture uses only top-level jpch and contiguous jp2c boxes, so fragment/multiple-codestream counting and compositing-layer-extension repetition do not affect applicability. SUPPORTED.md independently promises rejection of malformed structures. The project selects a later standard edition; this diagnosis establishes the specific mapped 2021 obligation, not exhaustive conformance to the later edition, whose text was not independently checked here.

Route: the replay additively extends the native test, builds JPX bytes from the real grayscale fixture, and invokes public parse(&bytes, &limits). Malformed cardinality is the intentional downstream rejection predicate under test, not a precondition of this byte-parser entry. Signature, JPX file type, rreq, inherited grayscale jp2h, explicit jpch, one default layer and actual codestream payloads are constructed; no private Jp2File state is fabricated. Parser MIR and source show each stream starts with codestream_header: None, bb250 rejects only H>N, and bb255 associates only indices below H. With 0<H<N it then validates layer references and bb282 returns Ok(Jp2File). The replay subsequently calls file.codestream(index) on every in-range index and observes the missing association. Getter MIR bb0-bb4 performs slice get and ok_or, correctly exposing the stored element without repairing the producer's invariant. Thus the violation belongs to parse admission and its resulting exposed association set, not getter bounds handling. It directly violates this clause's nonzero-cardinality-rejection guarantee. No full rendering, fallback execution, or formal reachability proof is claimed. This is neither test_error nor a reason to weaken the standards-derived clause.

[Standards quote, production lines, and provenance](../../evidence.md#dev-cbf63fcfa1)

</details>
