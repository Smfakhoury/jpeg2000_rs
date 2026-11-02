# Ordinary COC accepts zero precinct exponents above resolution zero

`DEV-64FD2DDB2B` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § A.6.2

**Required behavior:** Decode the COC component index using the Csiz-dependent width, reject nonexistent components and malformed ordinary COC field/length encodings, and return the selected component with the parsed component style.

**Observed deviation:** Ordinary COC accepts zero precinct exponents above resolution zero

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-64FD2DDB2B --output rfc-results/DEV-64FD2DDB2B
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 coc_clause_harness.py run admission
```

### Test input and recorded failure

The harness uses ResourceLimits::HARDENED, the existing SIZ fixture (64x32, one component, Rsiz=0), one valid COD with one layer/NL=1/4x4 reversible code blocks and implicit precincts, compatible QCD, one main-header COC for component zero with Scoc=1 and NL=1, no CAP/Part 2 selectors or overrides/duplicates, and a properly framed first tile part. COC payload [0,1,1,0,0,0,1,0x11,0xf0] has exactly two precinct bytes. All entry requirements and limits are respected; only the second precinct's zero width exponent is intentionally malformed, which this parser is required to reject.

**Recorded failure** (matched by the replay runner):

```text
main-header parse_layout accepted illegal high-resolution precinct: [(1, 1), (0, 15)]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/codestream/tests/coc_clause_tests.rs:L320-L324`](files/src/codestream/tests/coc_clause_tests.rs#L320-L324)

```text
let cod = segment(COD, &[0, 0, 0, 1, 0, 1, 0, 0, 0, 1]);
    let qcd = segment(QCD, &[0, 0, 0, 0, 0]);
    let coc = segment(COC, &[0, 1, 1, 0, 0, 0, 1, 0x11, 0xf0]);
    let bytes = codestream_with_headers(1, &[cod, qcd, coc], &[], &[0, 0]);
    let limits = ResourceLimits::HARDENED;
```

[`src/codestream/tests/coc_clause_tests.rs:L341-L341`](files/src/codestream/tests/coc_clause_tests.rs#L341)

```text
match parse_layout(&bytes, &limits) {
```

[`src/codestream/tests/coc_clause_tests.rs:L346-L347`](files/src/codestream/tests/coc_clause_tests.rs#L346-L347)

```text
Ok(layout) => failures.push(format!(
            "main-header parse_layout accepted illegal high-resolution precinct: {:?}",
```

[`src/codestream/tests/coc_clause_tests.rs:L357-L362`](files/src/codestream/tests/coc_clause_tests.rs#L357-L362)

```text
assert!(
        failures.is_empty(),
        "{} COC contract mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/codestream/tests/coc_clause_tests.rs`](files/src/codestream/tests/coc_clause_tests.rs)
- [`coc_clause_harness.py`](files/coc_clause_harness.py)

**Build commands** (in order):

```sh
python3 coc_clause_harness.py build
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The failed check is the admission test's final `failures.is_empty()` assertion, which aggregates mismatches rather than stopping at each comparison. The retained output begins partway through that diagnostic, so its first historical mismatch is unavailable; however, it explicitly records both erroneous Ok results for malformed precinct fields and the within-policy main-header acceptance quoted in observation. This is not an arbitrary exit-101 attribution. Clause af4bc1414e81593a62244981 requires rejecting malformed ordinary COC fields. Its origin is rfc_map, not infer. I read the parent evidence and the assigned T.800 (06/2019) A.6.1 and A.6.2 through read_rfc_section using origin item 8ff9f4815b9913140344f4a4. A.6.2 assigns SPcoc the Table A.15 parameters; A.15 orders precinct entries from NLLL through successive resolutions; Table A.21 states separately for PPx and PPy: `This value may only equal zero at the resolution level corresponding to the NLLL band.` With NL=1, the second entry 0xf0 therefore illegally specifies PPx=0 at resolution 1. This is ordinary Part 1, Rsiz=0, no CAP and no Part 2 decomposition reference, so the parent finding's unresolved directional/Part 2 applicability does not apply. This conclusion is scoped to the assigned 2019 edition; repository newer-edition references remain unreconciled and no broader conformance claim is made. The implementation and stored parse_cod_payload contract preserve zeros, but that correlated model/implementation agreement does not invalidate this standards-derived clause.

Realism checked before deeper tracing: the direct sweep executes 1,782 valid cases and over 20,000 malformed cases sequentially, with depth at most 32 (plus an intentional depth-33 rejection), at most 33 explicit precinct bytes, and framed inputs at most 51 bytes for its ordinary payloads. Its Csiz values above HARDENED.maximum_components=64 do not allocate component arrays and are accepted by the unit helper's unrestricted u16-count contract; they are not evidence of full-decoder capability. Crucially, attribution does not depend on these cases: the supplemental real caller has one component, a 64x32 image (2,048 pixels), one tile/first tile part, one layer, two resolutions, 4x4 code blocks, a nine-byte COC payload, and a 99-byte complete framed stream. These are well below HARDENED input, geometry, component, metadata and resolution limits. The entry may contain a malformed COC because rejecting such fields is the parser's obligation; all framing, selector, defaults and caller-state requirements are met. The two tile-data bytes are opaque to parse_layout; no full entropy-decoding claim is made.

Route: the harness's unmodified existing codestream_with_headers helper emits SOC/SIZ/COD/QCD/COC/SOT/SOD/data/EOC with correct lengths and Psot. parse_layout parses SIZ, validates/retains the single COC component selector, obtains the valid COD and QCD, then its component loop calls parse_coc with validated components=1 and the effective COD default. Compiler declaration and MIR CFG confirm parse_coc calls prepare_coc_payload then parse_cod_payload, propagating errors or returning the style. Preparation copies the ordinary SPcoc bytes into the synthesized COD payload. parse_cod_payload accepts its exact length and bounded dimensions/depth, then copies the precinct nibbles without checking that both are nonzero for resolution>0. It returns Ok, parse_coc returns that style, and the owning caller installs it. No Part 2 structure exists to reinterpret the fields. The host actually observes parse_layout returning the illegal style [(1,1),(0,15)], contradicting the clause's rejection guarantee. The precise project error kind/offset is not specified by the standard, but this failure is acceptance, not merely a disputed diagnostic. Production code was not changed and no new replay is needed to ground the already host-observed failing check.

[Standards quote, production lines, and provenance](../../evidence.md#dev-64fd2ddb2b)

</details>
