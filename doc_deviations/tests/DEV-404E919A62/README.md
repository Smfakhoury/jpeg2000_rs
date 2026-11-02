# Arbitrary-filter lifting selects the wrong source neighbour for odd-origin axes

`DEV-404E919A62` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § F.3.2

**Required behavior:** Reconstruct one two-dimensional wavelet level with the supplied arbitrary kernel, using its reversibility-derived precision mode, the level's horizontal and vertical phases and the declared output geometry.

**Observed deviation:** Arbitrary-filter lifting selects the wrong source neighbour for odd-origin axes

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-404E919A62 --output rfc-results/DEV-404E919A62
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --locked --lib transform::tests::arbitrary_level_contract::arbitrary_level_matches_standard_equations_and_rejects_bad_geometry_or_budget -- --exact --nocapture
```

### Test input and recorded failure

Three ordinary borrowed samples, mathematically correct parity-partitioned dimensions and exact sample lengths, W=1,H=3, and decoded-byte limit 24 satisfy admission. The semantic REV/ARB/CON kernel has minit=0, signed 8-bit coefficients [-1] then [2], zero offsets/exponents/residues, and no scaling. It is built with existing explicit test-only constructors; unused descriptor metadata is not interpreted by this numerical unit API. Both singleton high-pass horizontal inputs are even, so halving is exact. All subsequent arithmetic is small exact reversible integer arithmetic. The independent standard oracle yields vertical [-90,128,-40] -> [-90,208,-40] -> [118,208,168].

**Recorded failure** (matched by the replay runner):

```text
1x3, phases=false/false, REV=true, minit=false: interleaved input=[-180, 256, -80]; expected [118, 208, 168]; got ReconstructedPlane { width: 1, height: 3, fraction_bits: 0, samples: [218, 308, 268] }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/transform/tests/arbitrary_level_contract.rs:L212-L212`](files/src/transform/tests/arbitrary_level_contract.rs#L212)

```text
                            let result = inverse_level_arbitrary(level, &kernel, &limits);
```

[`src/transform/tests/arbitrary_level_contract.rs:L208-L211`](files/src/transform/tests/arbitrary_level_contract.rs#L208-L211)

```text
                            let bytes = (width * height * 8) as u64;
                            let limits = ResourceLimits::default()
                                .with_maximum_decoded_bytes(bytes)
                                .unwrap();
```

[`src/transform/tests/arbitrary_level_contract.rs:L166-L169`](files/src/transform/tests/arbitrary_level_contract.rs#L166-L169)

```text
                                    let high_x = (x % 2 == 0) != first_x;
                                    let high_y = (y % 2 == 0) != first_y;
                                    bands[usize::from(high_x) + 2 * usize::from(high_y)]
                                        .push(data[y * width + x]);
```

[`src/transform/tests/arbitrary_level_contract.rs:L74-L75`](files/src/transform/tests/arbitrary_level_contract.rs#L74-L75)

```text
            let n = (coordinate - parity) / 2;
            let source = 2 * n + 1 - parity;
```

[`src/transform/tests/arbitrary_level_contract.rs:L309-L312`](files/src/transform/tests/arbitrary_level_contract.rs#L309-L312)

```text
    assert!(
        failures.is_empty(),
        "{} standard-equation mismatches; first examples:\n{}",
        failures.len(),
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/transform/tests/arbitrary_level_contract.rs`](files/src/transform/tests/arbitrary_level_contract.rs)
- [`arbitrary-level-registration.patch`](files/arbitrary-level-registration.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i arbitrary-level-registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/transform.rs'"'"').read_text(); t=Path('"'"'src/transform/tests/arbitrary_level_contract.rs'"'"').read_text(); assert '"'"'    mod arbitrary_level_contract;'"'"' in s; assert '"'"'assert_eq!(contexts, 560);'"'"' in t; assert '"'"'failures.is_empty()'"'"' in t; assert '"'"'let result = inverse_level_arbitrary(level, &kernel, &limits);'"'"' in t; print('"'"'Replay source guards: additive registration, 560-case assertion, output oracle and subject call present'"'"')'
```

```sh
cargo test --locked --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

Case 8b69097aab2e6e4d6823ff4c faithfully exposes a deviation from clause d348494f1956deec3f860649: reconstruction with the supplied arbitrary kernel must respect the horizontal and vertical phases, including the source coordinates of its lifting filter. The first failed assertion is failures.is_empty() at harness line 309, after all 560 contexts and geometry/storage/budget/immutability checks completed. It aggregates 393 numerical mismatches, not a setup failure or an arbitrary nonzero exit. The first printed example is an irreversible one-unit discrepancy; I do not rely on exact irreversible precision or classify every reported mismatch. The same failed assertion includes the exact reversible 1x3 witness quoted in observation, which has no rounding-tolerance ambiguity.

Entry realism was checked first: the sweep uses at most 17*7=119 input samples (952 bytes), two lifting steps with one coefficient each, at most seven row syntheses and seventeen column syntheses, and sequential cases with small heap temporaries and no recursion. Each admitted call sets its decoded-byte limit to exactly 8*W*H; the selected witness uses only three samples and 24 bytes. These are far below the default 2 GiB decoded-byte limit and ordinary layout/stack capacities. decode.rs constructs the same WaveletLevel from four working bands and geometry phases before calling inverse_level_arbitrary; there is no minimum-size or even-origin requirement. This is a direct unit_contract invocation, not a fixture skipping an owning caller whose prerequisites it needs.

Authority is rfc_map item 470d13ab8f2fa16772f8b95e, not an inference to revise to match the implementation. I read its parent finding and pinned manifest, T.800 (06/2019) F.3.2 and T.801 (06/2021) A.3.5, H.2.1, H.2.2, H.2.3, H.2.4.1-3 and H.2.5.1. F.3.2 supplies the four-band horizontal-before-vertical ordering. H.2.1 defines singleton halving at an odd origin and reverse step order. H-3 uses absolute target coordinate 2n+m_s and source coordinate 2n+1-m_s+2(k+offs); H.2.4.3 extends each parity subsequence independently. A.3.5 permits the witness's REV/ARB/CON signed-8-bit coefficients, two steps, zero offsets/exponents/residues and minit=0. The parent finding limits its demonstrated mapping to axis order rather than certifying numerical equivalence; that limitation is not permission to reconstruct the wrong arbitrary filter at an odd phase. Neither the executable model nor retained native tests can override H-3.

For W=1,H=3, phases=false/false, reversible=true,minit=false, the bands are LL=(0,1,[]), HL=(1,1,[256]), LH=(0,2,[]), HH=(1,2,[-180,-80]). validate_level accepts exactly these dimensions and sample counts. Existing explicit cfg(test) constructors provide valid decoded numerical kernel fields: step 0 alpha=-1, step 1 alpha=2, all offsets/exponents/residues zero, no scaling. Negative coefficients use the same sign-extended representation as read_kernel_coefficient. for_test's descriptor sentinel is not a valid serialized ATK header, but neither descriptor nor marker metadata participates in this decoded-kernel API or its raw numerical view; the numerical fields can be signalled by legal descriptor 0x10ff. No codestream-parsing claim is needed for this unit entry.

The compiler declaration and MIR CFG show inverse_level_arbitrary forwards !kernel.reversible() and Some(kernel) to inverse_level. That helper validates/admit-budgets, reconstructs the low and high rows, then columns. The width-one odd horizontal phase halves the input into vertical interleaved [-90,128,-40]. inverse_column_pair calls inverse_arbitrary with low_pass_first=false. apply_arbitrary_step correctly maps absolute even targets to local odd indices, but lifting_source_sum discards the origin: it sets target_parity=target&1 and computes delta=1-2*target_parity+2*(coefficient+offset). For the first reconstruction step, local target 1 is absolute coordinate 2. H-3 requires source absolute 3/local 2 (value -40); the implementation instead selects local 0 (value -90). Thus the middle value becomes 128-2*(-90)=308 rather than 128-2*(-40)=208. The next step produces [218,308,268] instead of [118,208,168], exactly matching host output. When the origin is odd, the local/absolute parity reversal changes the required source displacement by two. This is a phase-sensitive arbitrary-filter synthesis error, not malformed input, budget exhaustion, a wrong reversible oracle, or an overstrong specification. No production changes or additional replay artifacts were needed.

[Standards quote, production lines, and provenance](../../evidence.md#dev-404e919a62)

</details>
