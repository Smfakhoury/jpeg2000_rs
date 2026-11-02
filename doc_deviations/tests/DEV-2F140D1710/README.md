# Native selected-color context excludes the CRG-registered predecessor sample

`DEV-2F140D1710` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § A.9.1

**Required behavior:** Selected-color context preparation shall honor component registration and subsampling when retaining the full-image sampling phase before cropping; registration-bearing metadata from a CRG marker must retain its defined physical offset meaning.

**Observed deviation:** Native selected-color context excludes the CRG-registered predecessor sample

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-2F140D1710 --output rfc-results/DEV-2F140D1710
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 -c 'import os, subprocess; from pathlib import Path; binaries = [p for p in Path('"'"'target/deepspec-crg/debug/deps'"'"').glob('"'"'jpeg2000_rs-*'"'"') if p.is_file() and os.access(p, os.X_OK)]; assert len(binaries) == 1, binaries; name = '"'"'decode::tests::selected_color_context_uses_crg_component_separation'"'"'; assert name + '"'"': test'"'"' in subprocess.check_output([str(binaries[0]), '"'"'--list'"'"'], text=True).splitlines(); os.execv(str(binaries[0]), [str(binaries[0]), '"'"'--exact'"'"', name, '"'"'--nocapture'"'"'])'
```

### Test input and recorded failure

Safe native-resolution direct helper invocation with parser-derived ImageSize, positive sampling, origin zero, a nonempty in-image ROI and a validator-produced tile list [0]. The 1024x1024 single-tile fixtures with at most three components are well within HARDENED geometry/resource limits. The CRG appears exactly once in the main header with one u16 pair per component and length 2+4*N, satisfying T.800 (06/2019) A.9.1 framing. No manual unit conversion or metadata mutation occurs. At separation 4 and registration 32768, the physical offset is 2 and the project's floor/predecessor policy requires sample 1 at coordinate 9; the context starting at 5 contains only coded sample indices 2..3.

**Recorded failure** (matched by the replay runner):

```text
assertion `left == right` failed: parse_layout(CRG)->validate_selection->color_selection_from_validated: components=[[4, 4, 32768, 32768]] ROI=[9, 9, 10, 10] context=ImageRegion { x: 5, y: 5, width: 5, height: 5 }; component=0 axis=0 coordinate=9: physical sample 1 missing from coded range 2..3
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/decode_crg_tests.rs:L125-L126`](files/src/decode_crg_tests.rs#L125-L126)

```text
        let layout = parse_layout(&bytes, &ResourceLimits::HARDENED)
            .expect("standards-framed main-header CRG fixture must parse");
```

[`src/decode_crg_tests.rs:L152-L158`](files/src/decode_crg_tests.rs#L152-L158)

```text
        let validated = validate_selection(size, &requested)
            .expect("nonempty in-image ROI on the sole tile is valid");
        assert_eq!(validated.region, region);
        assert_eq!(validated.tiles, vec![0]);
        assert_eq!(validated.discard_levels, 0);
        let (expanded, target) = color_selection_from_validated(size, validated)
            .expect("native-resolution context preparation must succeed");
```

[`src/decode_crg_tests.rs:L64-L66`](files/src/decode_crg_tests.rs#L64-L66)

```text
    let mut cases = (9..=14)
        .map(|x| (vec![[4, 4, 32_768, 32_768]], [x, x, x + 1, x + 1]))
        .collect::<Vec<_>>();
```

[`src/decode_crg_tests.rs:L51-L52`](files/src/decode_crg_tests.rs#L51-L52)

```text
    let offset = separation * u64::from(registration);
    let position = u64::from(coordinate) * 65_536;
```

[`src/decode_crg_tests.rs:L56-L56`](files/src/decode_crg_tests.rs#L56)

```text
    position.saturating_sub(offset) / (separation * 65_536)
```

[`src/decode_crg_tests.rs:L175-L176`](files/src/decode_crg_tests.rs#L175-L176)

```text
                let first_available = start.div_ceil(separation);
                let end_available = u64::from(region[axis + 2]).div_ceil(separation);
```

[`src/decode_crg_tests.rs:L181-L181`](files/src/decode_crg_tests.rs#L181)

```text
                    if required < first_available || required >= end_available {
```

[`src/decode_crg_tests.rs:L198-L198`](files/src/decode_crg_tests.rs#L198)

```text
    assert_eq!(failures, 0, "{}", examples.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/decode_crg_tests.rs`](files/src/decode_crg_tests.rs)
- [`validation/crg-context-tests.patch`](files/validation/crg-context-tests.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i validation/crg-context-tests.patch
```

```sh
cargo test --locked --lib --no-run --target-dir target/deepspec-crg
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing assertion is the final assert_eq!(failures, 0), with host-observed left=3874 and right=0; earlier parsing, metadata, ROI validation, target and returned-geometry checks succeeded. This is a valid direct unit-contract test, not a bypassed caller fixture: it obtains ImageSize from parse_layout(CRG), constructs an ordinary DecodeSelection, calls the actual validate_selection, then invokes color_selection_from_validated exactly as color_selection_for_size does. The test-only patch only includes the harness in decode::tests. Resource realism was checked first: each fixture is a 1024x1024 image (1,048,576 pixels), one tile, 1 or 3 eight-bit components, one resolution/layer, positive SIZ sampling <=255, discard=0, and a single main-header CRG of admissible length. HARDENED allows dimensions 1,000,000, 100,000,000 pixels, 64 components and 16,384 tiles. The 5437 cases are processed sequentially with tiny metadata/packet fixtures; even a decoded three-component image would be only a few MB, far below the 2 GiB decoded-byte limit. No oversized buffer, stack recursion or fabricated ValidatedSelection is involved. Empty packets are not used to claim decoded-pixel behavior: this helper consumes parsed geometry, not decoded sample data.

The clause has rfc_map origin. I read its parent finding and re-read T.800 (06/2019) A.9.1 through the parent mapping item. That section explicitly defines offsets in fractions of component separation, applies CRG to rendering rather than coded-grid decoding, and permits these marker values. With zero origin, separation=4 and registration=32768, physical sample i is at 4*i+2; the project's predecessor/floor sampling policy therefore requires sample floor((9-2)/4)=1 for coordinate 9. This policy is evidenced by nominal_component_at calling registered_floor_div; the standard is not being asserted to mandate a particular interpolation filter. The declared CRG physical-offset meaning and documented selected-color context promise make this a sound native-resolution clause, not a spec to rewrite to match the correlated raw-unit models/tests.

Concrete route: parse_layout copies raw CRG=32768 into both registration fields; validate_selection returns region [9,9,10,10], tiles [0], discard 0. MIR bb0 takes the d<32 branch, bb4 gives scale=1, bb24 sets target to that region, bb26 enters the component loop, bb51/bb60 subtract raw registration from 65536*9, and bb61/bb70 take nonzero remainder paths that set horizontal=vertical=4. After the loop, bb87/bb88 and bb91/bb92 produce x0=y0=5 and the successful return is region (5,5,5,5). The ordinary coded SIZ sample interval is [ceil(5/4),ceil(10/4))=[2,3), excluding physically required sample 1 at coded coordinate 4. Thus the helper has already discarded necessary registered predecessor context before any color conversion. Raw registration subtraction also has the parent finding's units discrepancy (32768 means two reference-grid units here, not half a unit); merely changing that subtraction would not by itself establish sufficient context, because one-separation lookbehind from a nonaligned ROI can still exclude the predecessor. The actual failed check is missing physical context, not merely a different formula or arbitrary nonzero exit.

This records the reproduced helper-contract deviation only, not an end-to-end wrong-pixel reproduction or formal public reachability proof. The cited edition is T.800 (06/2019), whose exact A.9.1 wording was checked; reconciliation with the newer editions named in SUPPORTED.md remains outside the evidence and is not claimed. Existing review labels and agreement of the Verus/native reference with raw-unit arithmetic do not negate this witnessed, contract-valid unit failure.

[Standards quote, production lines, and provenance](../../evidence.md#dev-2f140d1710)

</details>
