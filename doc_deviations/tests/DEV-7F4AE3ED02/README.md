# JPX layer-reference validation accepts mixed registration-box presence

`DEV-7F4AE3ED02` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t801-202106.txt § M.11.7.7

**Required behavior:** A complete JPX layer-registration structural validation must account for file-wide all-or-none registration-box presence as well as whether each referenced codestream exists.

**Observed deviation:** JPX layer-reference validation accepts mixed registration-box presence

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-7F4AE3ED02 --output rfc-results/DEV-7F4AE3ED02
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --lib --locked jp2::tests::layer_reference_contract::registration_presence_is_file_wide_and_references_exist -- --exact --nocapture
```

### Test input and recorded failure

A direct unit-contract test is available with no requires predicates. Both layers are produced by the production parser from scoped headers, have local grayscale colour groups, default/unit registration grids, and, where creg exists, one entry with XS=YS=XR=YR=1 and XO=YO=0. The supplied count is a valid unit input; both explicit and implicit references exist in that sequence. No actual codestream objects are required by this function's slice/count API. Mixed presence is intentionally invalid under the downstream structural predicate, but is not forbidden at this validator's entry. Two layers, two codestreams, 80 fixture bytes and depth 3 are comfortably within configured limits. No complete-file parsing or codestream-decoding reproduction is claimed.

**Recorded failure** (matched by the replay runner):

```text
M.11.7.7 all-or-none presence: accepted 494 mixed patterns; first=Some((2, 1))
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jp2_layer_reference_tests.rs:L35-L35`](files/src/jp2_layer_reference_tests.rs#L35)

```text
let layer = parse_compositing_layer(framed, &limits).unwrap();
```

[`src/jp2_layer_reference_tests.rs:L141-L149`](files/src/jp2_layer_reference_tests.rs#L141-L149)

```text
let references: Vec<Vec<u16>> = (0..length)
                .map(|index| {
                    if mask & (1 << index) == 0 {
                        Vec::new()
                    } else {
                        vec![u16::try_from(length - 1 - index).unwrap()]
                    }
                })
                .collect();
```

[`src/jp2_layer_reference_tests.rs:L152-L152`](files/src/jp2_layer_reference_tests.rs#L152)

```text
let actual = validate_layer_references(&layers, length);
```

[`src/jp2_layer_reference_tests.rs:L155-L155`](files/src/jp2_layer_reference_tests.rs#L155)

```text
} else if actual.is_ok() {
```

[`src/jp2_layer_reference_tests.rs:L161-L161`](files/src/jp2_layer_reference_tests.rs#L161)

```text
accepted_mixed.push((length, mask));
```

[`src/jp2_layer_reference_tests.rs:L169-L174`](files/src/jp2_layer_reference_tests.rs#L169-L174)

```text
assert!(
        accepted_mixed.is_empty(),
        "M.11.7.7 all-or-none presence: accepted {} mixed patterns; first={:?}",
        accepted_mixed.len(),
        accepted_mixed.first()
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jp2_layer_reference_tests.rs`](files/src/jp2_layer_reference_tests.rs)
- [`layer_reference_tests.patch`](files/layer_reference_tests.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i layer_reference_tests.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jp2.rs'"'"').read_text(); t=Path('"'"'src/jp2_layer_reference_tests.rs'"'"').read_text(); assert '"'"'mod layer_reference_contract {\n        include!("jp2_layer_reference_tests.rs");\n    }'"'"' in s; assert '"'"'fn no_registration_uses_ordinal_bounds()'"'"' in t; assert '"'"'fn registration_presence_is_file_wide_and_references_exist()'"'"' in t; assert '"'"'accepted_mixed.is_empty()'"'"' in t'
```

```sh
cargo test --lib --locked --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is the final accepted_mixed.is_empty() check, not a fixture/parser failure: host replay reports 494 accepted mixed patterns and first=Some((2, 1)). The earlier original regression, all-registered controls, out-of-range-reference checks, and coherent all/none controls completed before this assertion. The smallest failing unit entry contains two parser-produced layers and codestream_count=2: layer 0 has one explicit reference to codestream 1; layer 1 has no creg and uses implicit ordinal 1. This is an admissible input to the validator, whose assessed unit contract has no requires predicates; mixed presence is the property to validate, not an entry precondition. The harness does not fabricate private layer fields or bypass layer parsing: it builds framed jplh/cgrp/colr/creg bytes and invokes BoxReader and parse_compositing_layer before the target. Resource realism is not a concern: the failing family uses only 2–8 layers/codestreams, one registration per registered layer and nesting depth 3; the smallest case is 80 bytes. Even the preceding control families use at most 256 layers, 64 codestreams and two registrations per layer, within the configured 256-layer, 64-codestream, depth-32 and metadata/input budgets. Real parse callers pass the same parser-produced layer vector and codestream_boxes.len(). MIR confirms that each layer is considered independently: nonempty registrations only take the bounded explicit-index route; empty registrations only take the bounded ordinal route; both join the layer increment and ultimately return Ok. Thus the smallest mixed case reaches Ok despite violating the exact all-or-none structural condition. The origin is rfc_map, not infer. I read its parent finding and the original T.801 (06/2021), ISO/IEC 15444-2:2021 M.11.7.7 section through read_rfc_section using the parent mapping item: it explicitly requires every Compositing Layer Header box to have creg if any has one. This applies to these ordinary layer headers and does not depend on rendering or multiple registrations per layer. Empty vectors faithfully denote absence because the local creg parser requires at least one entry. No prior cross-layer check exists in the inspected parse path, which publishes Jp2File immediately after this validator. This diagnosis is limited to the standards-derived structural-validation clause and the reproduced valid unit entry; it does not assert that every reader must reject every nonconforming file, that this replay executed a complete mixed JPX file, or that the unexamined project-selected newer standard edition is established. The review-marked edition qualification remains: the cited and checked authority here is the 2021 edition, not a claim of full current-edition conformance.

[Standards quote, production lines, and provenance](../../evidence.md#dev-7f4ae3ed02)

</details>
