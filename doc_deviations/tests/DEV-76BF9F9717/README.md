# JPIP geometric mapping rejects admissible clipped precincts and invents empty-resolution precincts

`DEV-76BF9F9717` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.3.2.1

**Required behavior:** Derive precinct coordinates and JPIP in-class identifiers from SIZ/COD/COC geometry, with resolution-prefix numbering and checked tile/component interleaving.

**Observed deviation:** JPIP geometric mapping rejects admissible clipped precincts and invents empty-resolution precincts

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-76BF9F9717 --output rfc-results/DEV-76BF9F9717
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --locked --lib jpip::tests::derive_precinct_map_matches_standard_geometry_contract -- --exact --nocapture
```

### Test input and recorded failure

This is a direct public unit-contract entry with an API-reachable cache, not an internal planner fixture. Fixture13's seeded geometry is 56x47, origin/tile-origin (3,2), tiles 24x24, six tiles and three legal sample separations, one layer and D<=2. Completed bins contain SOC/SIZ/main COD/QCD and properly scoped first-tile-part COD/COC/QCD/QCC overrides and SOD. Coding-style precedence and explicit precinct exponents satisfy T.800 A.6.1/B.6; headers and resource requirements are far below default limits. Missing precinct payload bins are intentional for geometric derivation and consistent with native usage and the target's synthetic header-only template. The zero-count execution violating the packet constructor's predicate is created by the owning production planner, not supplied by the test.

**Recorded failure** (matched by the replay runner):

```text
fixture=13 progression=0: expected 810 mappings, got MalformedInput { offset: 0, kind: InvalidPacketHeader }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_precinct_map_contract.rs:L161-L165`](files/src/jpip_precinct_map_contract.rs#L161-L165)

```text
        let mut cache = DatabinCache::new(limits);
        for (id, bytes) in &bins {
            cache.insert(fragment(*id, 0, bytes, true)).unwrap();
            assert!(cache.is_complete(*id));
        }
```

[`src/jpip_precinct_map_contract.rs:L241-L241`](files/src/jpip_precinct_map_contract.rs#L241)

```text
    let mapping = cache.derive_precinct_map(stream)?;
```

[`src/jpip_precinct_map_contract.rs:L275-L280`](files/src/jpip_precinct_map_contract.rs#L275-L280)

```text
    let mut seed = 0x4a50_4950_u32;
    for index in 0..32 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let width = 25 + seed % 42;
        let height = 25 + (seed >> 8) % 42;
        let origin = [seed % 4, (seed >> 4) % 4];
```

[`src/jpip_precinct_map_contract.rs:L330-L342`](files/src/jpip_precinct_map_contract.rs#L330-L342)

```text
            let (cache, bins) = geometry.fixture(stream, progression, ResourceLimits::default());
            let observed = match map_contract_observe(&cache, stream) {
                Ok(observed) => observed,
                Err(error) => {
                    let failure = format!(
                        "fixture={index} progression={progression}: expected {} mappings, got {error:?}",
                        expected.len()
                    );
                    eprintln!("{failure}");
                    failures.push(failure);
                    continue;
                }
            };
```

[`src/jpip_precinct_map_contract.rs:L207-L212`](files/src/jpip_precinct_map_contract.rs#L207-L212)

```text
                        // T.800 B-16 explicitly gives zero for empty resolution bounds.
                        spans[axis] = if hi == lo {
                            0
                        } else {
                            hi.div_ceil(width) - lo / width
                        };
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_precinct_map_contract.rs`](files/src/jpip_precinct_map_contract.rs)
- [`precinct-map-test-registration.patch`](files/precinct-map-test-registration.patch)

**Build commands** (in order):

```sh
patch --batch --forward -p1 -i precinct-map-test-registration.patch
```

```sh
python3 -c 'from pathlib import Path; s=Path('"'"'src/jpip.rs'"'"').read_text(); t=Path('"'"'src/jpip_precinct_map_contract.rs'"'"').read_text(); assert '"'"'include!("jpip_precinct_map_contract.rs");'"'"' in s; assert '"'"'fn derive_precinct_map_matches_standard_geometry_contract()'"'"' in t; assert '"'"'failures.is_empty()'"'"' in t; assert '"'"'requested'"'"' not in t'
```

```sh
cargo test --locked --lib --no-run
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failed check is the accumulated geometry-contract assertion at src/jpip_precinct_map_contract.rs:382, whose first recorded discrepancy is fixture=13 progression=0: expected 810 mappings, got MalformedInput { offset: 0, kind: InvalidPacketHeader }. This is a geometry failure of the actual public derive_precinct_map entry, not a build failure, sanitizer failure, or a fixture that directly injects invalid decoder state.

Realism was checked first. Fixture13 is a 56x47 image at origin (3,2), 24x24 tiles anchored at (3,2), six tiles, three components with sampling (1,1),(2,1),(1,3), one quality layer, and decomposition levels at most two. The independent geometry count is 810. Across the suite, the largest image is only 513x257, there are at most three components, four resolutions, and fewer than 32 tiles per cache. The fixtures execute sequentially, with bounded shallow loops and small completed header bins. ResourceLimits::default is HARDENED: 100,000,000 pixels, 64 components, 16,384 tiles, 33 resolutions, 16,777,216 precincts/code blocks, 67,108,864 packets, and 2,147,483,648 input bytes. These inputs are nowhere near those limits or implicit stack/pool capacities. Existing native JPIP callers likewise insert completed class-6/class-2 header bins and derive a map before receiving precinct bytes. The separately tested packet-limit rejection is expected and did not trigger the failing aggregate.

Compiler declaration and MIR CFG establish the target route: automatic_headers -> build_automatic_template -> parse_layout -> increasing tiles -> automatic_tile_part -> full plan_progression_tile_at_with_component_overrides, propagating its error through the ? branch before returning a map. The harness constructs legal SIZ/COD/COC/QCD/QCC markers, inserts bins with public constructors/insert, checks their completion, and then calls the public subject. It does not construct private grids or decoder executions. The synthetic-template code explicitly rewrites Psot to the header-only length; absent packet data is therefore not a missing fixture precondition.

Tracing fixture13's first tile gives the specific bad operation. Tile 0 component 0 uses its tile COD override: D=2, explicit precinct bytes [0x32,0x23,0x32]. At resolution 2, precinct 0, the component bounds are [3,2,27,26], the precinct's subband-domain rectangle is [0,0,2,4], the LH subband bounds are [2,1,14,13], and effective block size is [2,4]. derive_planned_subband_lattice sets start=(2,1), end=(2,4). Its only emptiness check compares start to the entire subband's upper bound BEFORE clipping to the precinct. That check passes, and it returns Some with blocks=[0,1] instead of omitting this empty subband intersection. plan_subband_execution copies those counts into PacketSubbandExecution. packet_precinct_from_subbands correctly rejects code_blocks_across==0 with InvalidPacketHeader, which propagates to derive_precinct_map and the observed failure. The invalid downstream execution is generated by production planning from admissible headers; the test does not violate that callee's contract.

Authority is the rfc_map origin caf66b63dc43823a29f23597, not agreement with a native reference or Verus model. I read its parent finding, applicability and document manifest, then retrieved T.808 (12/2022) A.3.2.1 and T.800 (06/2019) B.6/A.6.1. T.808 requires contiguous precinct sequence numbering by resolution and Equation A-1 tile/component interleaving. T.800 B.6 expressly permits precincts to have no contributing subband coefficients, and requires their packets when the geometric precinct count is nonzero. A.6.1 confirms tile COC > tile COD > main COC > main COD, matching the harness oracle. Thus this valid ordinary 2D JPP input cannot be dismissed because its first precinct has an empty intersection with one subband. No newer-edition conformance is claimed.

There is also a separately grounded exact-clause violation in the same replay: fixture45 progression=0 stream=62 returns four mappings rather than one. Its one-pixel image occupies [1,2)x[1,2), D=3; resolutions 0,1,2 have equal lower/upper sample bounds. T.800 B-16 explicitly gives zero precincts on equality, whereas derive_component_resolution_geometry_for subtracts floor(lower/precinct_size) from ceil(upper/precinct_size) without the equality case, producing phantom precincts and shifting resolution-prefix identifiers. This independently demonstrates wrong coordinates/IDs on a normally successful admissible call. The diagnostics aggregate failures rather than logging the internal bad execution; the numeric route above is source-assessed, not a claim that the output itself contains an internal trace or a formal reachability proof. The host-observed failing checks and exact replayed public-entry harness are the reproduction evidence. No production changes are proposed or made.

[Standards quote, production lines, and provenance](../../evidence.md#dev-76bf9f9717)

</details>
