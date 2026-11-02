# Part 1 vertical 9/7 synthesis omits odd-origin singleton halving

`DEV-37A5BF80E8` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § F.3.5

**Required behavior:** The Part 1 vertical-column adapter extracts matching low/high columns at x and returns their phase-sensitive one-dimensional inverse synthesis for placement in that output column.

**Observed deviation:** Part 1 vertical 9/7 synthesis omits odd-origin singleton halving

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-37A5BF80E8 --output rfc-results/DEV-37A5BF80E8
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --lib transform::tests::part1_column_pair_contract -- --exact --nocapture
```

### Test input and recorded failure

For extent [1,2), the phase formula produces low_height=0 and high_height=1; both bands have width=1, x=0, exact row-major slice lengths zero and one, respectively. False low_pass_first correctly selects the high-band singleton. The one signed Q16 coefficient is even and modest, and all dimensions, capacities, byte limits and arithmetic are representable. Interleave's required low count is floor(1/2)=0 and high count is one. The unit test needs no fabricated private state, and the public caller accepts the same fixture. Test-only registration does not change production behavior.

**Recorded failure** (matched by the replay runner):

```text
inverse_column_pair width=1 height=1 origin=1 x=0 irreversible=true low=[] high=[-15859712]: expected=[-7929856], actual=Ok([-15859712])
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/transform_column_pair_contract_tests.rs:L15-L17`](files/src/transform_column_pair_contract_tests.rs#L15-L17)

```text
let origin = u32::from(!low_first);
                    let low_height = (origin + height).div_ceil(2) - origin.div_ceil(2);
                    let high_height = height - low_height;
```

[`src/transform_column_pair_contract_tests.rs:L58-L59`](files/src/transform_column_pair_contract_tests.rs#L58-L59)

```text
assert_eq!(low.samples.len(), (width * low_height) as usize);
                    assert_eq!(high.samples.len(), (width * high_height) as usize);
```

[`src/transform_column_pair_contract_tests.rs:L65-L74`](files/src/transform_column_pair_contract_tests.rs#L65-L74)

```text
let expected = if height == 1 {
                            // F.3.6: X(i0)=Y(i0) for even i0, Y(i0)/2 for odd i0.
                            Ok(if low_first {
                                vec![l[0]]
                            } else {
                                vec![h[0] / 2]
                            })
                        } else {
                            reference_sequence(l, h, low_first, irreversible)
                        };
```

[`src/transform_column_pair_contract_tests.rs:L80-L82`](files/src/transform_column_pair_contract_tests.rs#L80-L82)

```text
let actual =
                            inverse_column_pair(low, high, x, low_first, irreversible, None);
                        if actual.as_ref() != Ok(&expected) {
```

[`src/transform_column_pair_contract_tests.rs:L112-L118`](files/src/transform_column_pair_contract_tests.rs#L112-L118)

```text
inverse_vertical_9_7(
                                low,
                                high,
                                height,
                                low_first,
                                &ResourceLimits::default(),
                            )
```

[`src/transform_column_pair_contract_tests.rs:L157-L157`](files/src/transform_column_pair_contract_tests.rs#L157)

```text
assert!(failures.is_empty(), "{}", failures.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/transform_column_pair_contract_tests.rs`](files/src/transform_column_pair_contract_tests.rs)
- [`validation/install-column-pair-contract-test.py`](files/validation/install-column-pair-contract-test.py)

**Build commands** (in order):

```sh
python3 validation/install-column-pair-contract-test.py
```

```sh
cargo test --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed numerical check accumulated by the replay is width=1,height=1,origin=1,x=0,irreversible=true: expected [-7929856], actual Ok([-15859712]); the final failures.is_empty() assertion reports it. This is a valid unit-contract entry, not malformed fixture setup. Realism was checked: generated dimensions are at most 7x65 (455 i64 samples, 3640 bytes per full plane); 1196 selected columns are processed sequentially with no recursion or large fixed stack buffers. All generated coefficient bands together contain 19656 samples across the sequential geometry/filter/phase fixtures, not a simultaneous allocation. Default HARDENED limits allow 1000000 per dimension, 100000000 pixels and 2147483648 decoded bytes. The failing entry needs only one i64 coefficient. Widths match, x=0 is in bounds, rectangular backing lengths are exact, and origin=1 with extent [1,2) gives low height zero and high height one. Empty low storage is therefore required, not an inadmissible missing band. Q16 high=-15859712 is even, so division by two is exact and does not depend on rounding or overflow conventions. No callee entry requirement is violated.
The clause originates in rfc_map 0c974871043c1354a71e5867. Independently re-read its pinned T.800 (06/2019), SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e: F.3.5 requires per-column 1D_SR; F.3.6 states for length one X(i0)=Y(i0) for even i0 and Y(i0)/2 for odd i0. That singleton rule precedes and is not restricted by the subsequent length>=2 filter-specific procedures, so it applies to the selected Part 1 irreversible filter as well. None excludes Part 2 kernels. This establishes the oracle for this cited edition without claiming reconciliation with newer editions. The stored inverse_9_7 contracts and Q16 model explicitly describe unchanged singletons, but their stated authority is the project model, not independently established standard conformance; their agreement with implementation cannot override this standards-derived clause.
Compiler declaration/CFG evidence shows inverse_column_pair successfully collects [] and [-15859712], takes kernel=None then irreversible=true (bb8 -> bb14 -> bb15 -> bb16 -> bb17), and calls inverse_9_7. Its CFG takes interleave success then length<=1 (bb4 -> bb6 -> bb7) and returns the unchanged vector, without phase-sensitive singleton division. The harness directly invokes this valid unit entry and independently exercises the owning public route inverse_vertical_9_7 -> inverse_vertical -> inverse_columns -> inverse_column_pair -> inverse_9_7. Public geometry/byte checks accept the entry, and store_column writes the same incorrect value into the 1x1 output. The host output explicitly reports that public result too. Consequence: the odd-origin singleton column is twice its required reconstructed value. All 17 reported mismatches are this boundary (13 selected columns and four public planes); no claim of whole-decoder/codestream reachability or other lifting-rule correctness is needed. Production source is unchanged.

[Standards quote, production lines, and provenance](../../evidence.md#dev-37a5bf80e8)

</details>
