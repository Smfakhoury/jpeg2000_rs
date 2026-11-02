# Reduced selected decode applies tile-local nonlinear transforms on unreduced tile coordinates

`DEV-355FE751F9` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § B.1

**Required behavior:** Express selected component nonlinear-transform regions as nonempty half-open intersections of tile-component bounds and output extents, translated to output-local sample coordinates.

**Observed deviation:** Reduced selected decode applies tile-local nonlinear transforms on unreduced tile coordinates

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-355FE751F9 --output rfc-results/DEV-355FE751F9
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/selected-contract-suite decode::tests::selected_contract_geometry_and_native_samples --exact --nocapture
```

### Test input and recorded failure

Safe Rust slices and default options; actual encoded SIZ/COD/QCD/SOT/SOD/NLT bytes, not patched layout or reconstruction state. Image [1,32)x[1,16), eight 8x8 tiles, one unsigned 8-bit unit-sampled component, one decomposition level, empty packet payloads reconstructing zero coefficients. Tile-local gamma slopes [1,2,4,8,2,4,8,1] are positive supported parameters with equal source/target precision, exponent=1, threshold=1, a=1,b=0. All tile boundaries are multiples of eight, so discard=1 ownership is unambiguous. Region [3,29)x[3,15) lies inside the image. Tiny resource requirements are below default policy and ordinary caller scale. Successful unwrap and preceding controls establish successful public-entry decoding, while the independent slope oracle exposes wrong tile transform assignment.

**Recorded failure** (matched by the replay runner):

```text
native tile-specific nonlinear transform mismatch: ["d=1, region=None, native=(4,1), tile=1, expected=64, actual=128", "d=1, region=Some(ImageRegion { x: 3, y: 3, width: 26, height: 12 }), native=(4,2), tile=1, expected=64, actual=128"]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/decode_selected_contract_tests.rs:L4-L4`](files/src/decode_selected_contract_tests.rs#L4)

```text
for value in [32_u32, 16, 1, 1, 8, 8, 0, 0] {
```

[`src/decode_selected_contract_tests.rs:L53-L59`](files/src/decode_selected_contract_tests.rs#L53-L59)

```text
fn selected_contract_patterned() -> Vec<u8> {
    let mut bytes = selected_contract_grid(1, &[[7, 1, 1]]);
    for (tile, slope) in [1_u8, 2, 4, 8, 2, 4, 8, 1].iter().enumerate() {
        selected_contract_tile_header(&mut bytes, tile as u16, selected_contract_gamma(*slope));
    }
    bytes
}
```

[`src/decode_selected_contract_tests.rs:L204-L208`](files/src/decode_selected_contract_tests.rs#L204-L208)

```text
for discard in 0..=1 {
        for region in [None, Some(crate::ImageRegion::new(3, 3, 26, 12))] {
            let selection =
                crate::options::selection_from_parts(TileSelection::All, region, discard);
            let image = decode_codestream_selected(&patterned, &options, &selection).unwrap();
```

[`src/decode_selected_contract_tests.rs:L232-L234`](files/src/decode_selected_contract_tests.rs#L232-L234)

```text
let tile = (py * scale / 8) * 4 + px * scale / 8;
                    let expected = 128 / u64::from(slopes[tile as usize]);
                    let actual = samples[((py - y) * (ex - x) + px - x) as usize];
```

[`src/decode_selected_contract_tests.rs:L247-L250`](files/src/decode_selected_contract_tests.rs#L247-L250)

```text
assert!(
        mismatches.is_empty(),
        "native tile-specific nonlinear transform mismatch: {mismatches:?}"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/decode_selected_contract_tests.rs`](files/src/decode_selected_contract_tests.rs)
- [`selected_contract_setup.py`](files/selected_contract_setup.py)

**Build commands** (in order):

```sh
python3 selected_contract_setup.py
```

```sh
cargo test --locked --lib --no-run
```

```sh
python3 -c 'import glob,os,shutil,subprocess; p=[p for p in glob.glob('"'"'target/debug/deps/jpeg2000_rs-*'"'"') if os.path.isfile(p) and os.access(p,os.X_OK)]; assert len(p)==1,p; names=subprocess.check_output([p[0],'"'"'--list'"'"'],text=True); assert names.count('"'"'decode::tests::selected_contract_'"'"')==5,names; shutil.copy2(p[0],'"'"'target/selected-contract-suite'"'"')'
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed assertion is the final mismatches.is_empty() check in selected_contract_geometry_and_native_samples, not a setup error or generic exit status: host replay reports d=1 native=(4,1), tile=1, expected=64, actual=128, and the equivalent cropped failure at (4,2). The clause has infer origin, not rfc_map. Its promise that selected native-resolution samples receive their nonlinear processing is sound for this API: README explicitly promises native wavelet resolution, decode_typed_layout deliberately routes selected reconstruction through typed_layout, and typed_layout deliberately applies each tile's nonlinear transform. This is not a request for a new standards guarantee or a revision to match a shared model.

Entry realism was checked: the failing fixture has image endpoints 32x16, origin (1,1), one unsigned 8-bit 1x1-sampled component, eight 8x8 tiles, one decomposition level, one layer, 16 empty packets and at most 32 full-resolution codeblocks. Its full component has 465 samples (3720 sample bytes), with 105 samples at discard=1; the cropped reduced output has 78. The byte stream is under 1 KiB. These are far below HARDENED/default limits (width/height 1,000,000, pixels 100,000,000, components 64, tiles 16,384, resolutions 33, codeblocks/precincts 16,777,216, decoded bytes 2,147,483,648), without excessive stack depth or fabricated private state. Real JP2/JPH selected and JPIP callers forward the same byte-slice/options/selection interface; this direct public unit-contract test need not simulate their containers. All-tile selection, discard=1 within the available level, and the optional interior rectangle [3,3,29,15] are admissible. Test-only setup merely includes tests and adds an unused-in-this-case bounded precinct setter; production decoder behavior is unchanged.

Compiler declaration and CFG confirm parse_layout succeeds into bb5 then bb7 calls decode_typed_layout with Some(selection). Source then follows decode_layout_selected: validate execution and selection, decode all selected tile parts with Some(discard_levels), assemble reduced components, then decoded.typed and collect_nonlinear_regions. The latter computes output extents from the reduced TypedImage but intersects them directly with original SIZ tile_component_bounds, without a discard parameter or coordinate reduction. For d=1, tile 1's correct native extent is [4,8)x[1,4), but its unreduced bounds are [8,16)x[1,8). Native (4,1) consequently receives tile 0's slope-1 transform rather than tile 1's slope-2 transform. The same happens at (4,2) in the crop. Zero coefficients type to 128; source gamma logic's linear branch divides by slope, so tile 1 must produce 64, not 128. Aligned tile boundaries remove ownership ambiguity; d=0 controls pass. This specifically violates the clause's native-reconstruction nonlinear sample guarantee, even though dimensions remain correct. The supplied host replay is authoritative; the earlier report_deviation not_reproduced record is not relied on. This is an agent-assessed reproduced public-entry deviation, not a formal reachability or general standards-conformance proof.

[Standards quote, production lines, and provenance](../../evidence.md#dev-355fe751f9)

</details>
