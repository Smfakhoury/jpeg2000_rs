# Inverse LUT NLT normalizes reconstructed samples by 2^precision instead of 2^precision−1

`DEV-61C38E5752` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t801-202106.txt § K

**Required behavior:** Produce typed decoder output by applying admitted inverse nonlinear transformations after reconstructed samples and inverse component processing, preserving the selected reconstruction's geometry.

**Observed deviation:** Inverse LUT NLT normalizes reconstructed samples by 2^precision instead of 2^precision−1

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-61C38E5752 --output rfc-results/DEV-61C38E5752
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/typed-layout-contract decode::typed_layout_contract_tests::typed_layout_inverse_nlt_after_reconstruction_preserves_geometry --exact --nocapture
```

### Test input and recorded failure

A bounded valid direct unit entry, not an entropy-decoding claim: 3 equal four-bit source components, equal subsampling, one tile, matching selected reconstruction bounds and sample counts, ResourceLimits::HARDENED. Component 0's type-2 NLT has legal precision/sign, two points [255,0], PTval8, Dmin0 and Dmax=u32::MAX (normalized 0 and1); the NLT capability is signaled and main-header metadata is inherited into the only tile. The quoted route uses fraction_bits16 with exact integer-centered reconstruction and inverse_rct=false. Source rank0..15 corresponds to signed -8..7, entirely inside the source range. Existing assembly and typed conversion perform their native checks. Test access is a cfg(test) overlay that includes decode.rs unchanged.

**Recorded failure** (matched by the replay runner):

```text
typed_layout source_signed=true target_signed=false region=[3, 1, 35, 9] subsampling=1 fraction_bits=16 inverse_rct=false active=true component=0: actual=[255, 239, 223, 207, 191, 175, 159, 143, 128, 112, 96, 80, 64, 48, 32, 16] expected=[255, 238, 221, 204, 187, 170, 153, 136, 119, 102, 85, 68, 51, 34, 17, 0]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/decode_contract_overlay.rs:L2-L2`](files/src/decode_contract_overlay.rs#L2)

```text
include!("decode.rs");
```

[`src/typed_layout_contract_tests.rs:L153-L154`](files/src/typed_layout_contract_tests.rs#L153-L154)

```text
    let mut decoded = assemble_selected_image(size, &tiles, region, 0, &ResourceLimits::HARDENED)
        .expect("valid reconstructed selected-image fixture");
```

[`src/typed_layout_contract_tests.rs:L296-L296`](files/src/typed_layout_contract_tests.rs#L296)

```text
                for (fraction_bits, inverse_rct) in [(0, false), (16, false), (0, true)] {
```

[`src/typed_layout_contract_tests.rs:L324-L326`](files/src/typed_layout_contract_tests.rs#L324-L326)

```text
                        let output = typed_layout(&layout, decoded, &limits);
                        let output = output.expect(&route);
                        assert_geometry(&output, &geometry, region);
```

[`src/typed_layout_contract_tests.rs:L310-L310`](files/src/typed_layout_contract_tests.rs#L310)

```text
                                *value = 255 - 17 * rank - if target_signed { 128 } else { 0 };
```

[`src/typed_layout_contract_tests.rs:L342-L347`](files/src/typed_layout_contract_tests.rs#L342-L347)

```text
    assert!(
        discrepancies.is_empty(),
        "standards-derived NLT mismatch ({} scenarios):\n{}",
        discrepancies.len(),
        discrepancies.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/decode_contract_overlay.rs`](files/src/decode_contract_overlay.rs)
- [`src/typed_layout_contract_tests.rs`](files/src/typed_layout_contract_tests.rs)
- [`src/lib.rs`](files/src/lib.rs)

**Build commands** (in order):

```sh
python3 -c 'import json, shutil, subprocess, sys
p = subprocess.run(['"'"'cargo'"'"', '"'"'test'"'"', '"'"'--lib'"'"', '"'"'--no-run'"'"', '"'"'--message-format=json'"'"'], stdout=subprocess.PIPE, text=True)
if p.returncode:
    print(p.stdout)
    sys.exit(p.returncode)
artifacts = [json.loads(line) for line in p.stdout.splitlines() if line.startswith('"'"'{'"'"')]
[executable] = [a['"'"'executable'"'"'] for a in artifacts if a.get('"'"'reason'"'"') == '"'"'compiler-artifact'"'"' and a.get('"'"'executable'"'"') and a['"'"'target'"'"']['"'"'name'"'"'] == '"'"'jpeg2000_rs'"'"' and a['"'"'profile'"'"']['"'"'test'"'"']]
shutil.copy2(executable, '"'"'target/typed-layout-contract'"'"')'
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The failed check is the reconstruction test's final assert!(discrepancies.is_empty()), not fixture construction, resource exhaustion, or a geometry assertion. The retained host output contains concrete component-0 actual/expected sample mismatches. For example, signed four-bit reconstructed samples -8..7 converted to unsigned eight-bit output through the two-point descending LUT produce [255,239,...,16], instead of [255,238,...,0]. Geometry checks precede discrepancy collection and did not fail. Entry realism was checked first: the largest selection is 32x8, with three components and at most 768 total reconstructed i64 samples (6144 bytes of raw sample payload); a single tile, shallow fixed loops, and two LUT points are far below HARDENED limits (64 components, 100000000 pixels, 2147483648 decoded bytes). No implicit stack/pool stress or implausible count is involved. The normal caller decode_typed_layout also passes a full or selected reconstructed DecodedImage into typed_layout. This is a valid direct unit test of that stage, not a claim that arbitrary samples were decoded from the fixture's empty packets: parse_layout supplies the admitted layout, assemble_selected_image constructs matching selected component geometry, and the optional real inverse reversible component transform runs before typed_layout. The failing fraction_bits=16, inverse_rct=false route needs no component-transform assumptions: centered integer samples are multiplied by 2^16 and typed conversion restores valid signed four-bit values. The test-only overlay includes the unchanged decode.rs and merely registers a child module.

Authority is the rfc_map origin, not agreement with a Verus model or existing helper-based oracle. Independently read the pinned T.801 (06/2021), SHA256 985af26033db03edf53ce28b360107fb33b78d1a6af09b6d92ea72e89e277eb1, A.3.10 and K.2.2. A.3.10 admits this component-specific main-header type-2 NLT, two points, PTval=8, BDnlt=8, and Dmin=0/Dmax=u32::MAX representing exactly 0/1. K-7 normalizes unsigned samples by 2^by−1 and signed samples by (sample+2^(by−1))/(2^by−1). K-8 and K-9 with table [255,0], bt=bz=8 imply the exact integer oracle 255−17*rank, with an additional −128 only for signed output. No rounding convention or newer-edition claim is needed. The origin mapping cautions that it did not itself establish every numerical rule; the present independent K.2.2 reading establishes this specific admitted transformation. Thus the clause's requirement to apply the admitted inverse transformation is sound here; preserving geometry alone does not satisfy it.

Compiler declaration/CFG confirms typed_layout calls decoded.typed, then collect_nonlinear_regions, then apply_nonlinear_regions on nonempty regions (bb0 → bb4 → bb8 → bb11/bb12 → bb17). The sole inherited NLT covers component 0 of the selected tile. Per-sample processing calls inverse_nlt_sample, which dispatches type 2 to inverse_nlt_lut_sample. That function sets source_scale=2^source_info.precision; nlt_lut_coordinates multiplies the Dmin/Dmax parameters by that scale. Here it therefore interpolates at rank/16 rather than rank/15. At rank15 it rounds 255*(1−15/16) to16, instead of reaching the last LUT point0; the signed-output analogue is −112 instead of−128. This is the specific target deviation reproduced by the host, assessed on a valid unit entry, not a formal public-stream reachability proof. The host tail truncates the panic heading and earlier scenarios, but retains complete failing scenario lines and the final failed-test result, sufficient to ground this attribution. No production edits or new replay artifacts were needed.

[Standards quote, production lines, and provenance](../../evidence.md#dev-61c38e5752)

</details>
