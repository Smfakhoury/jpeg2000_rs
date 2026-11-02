# Automatic JPIP header normalization rejects legal SOD-omitted tile-header bins

`DEV-E2DF8B9FAC` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.3.3

**Required behavior:** Provide automatic precinct reconstruction with automatic normalization of the cached headers, as distinct from the explicitly caller-reframed plan path.

**Observed deviation:** Automatic JPIP header normalization rejects legal SOD-omitted tile-header bins

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-E2DF8B9FAC --output rfc-results/DEV-E2DF8B9FAC
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/automatic_headers_contract jpip::tests::automatic_headers_optional_sod_contract --exact --nocapture
```

### Test input and recorded failure

Safe-Rust unit test of the actual owning automatic_headers function, with test-only private access and no production behavior changes. Stream 37 has a legal SOC/SIZ/COD/QCD/COM main header, 1 tile, 1 component, and a completed class-2 in-class-0 empty tile bin. contract_insert uses normal fragment insertion with final_byte=true, then asserts is_complete and byte equality. Empty raw JPP tile-header bins and omitted SOD are explicitly permitted by pinned T.808 A.3.3. The sweep remains far below resource limits, uses one tile-part/tile and valid non-packed Part-1 fixtures, and requires no precinct payload to derive headers. Paired SOD-present entries pass the same exact canonical-header assertions.

**Recorded failure** (matched by the replay runner):

```text
A.3.3 class-2 raw JPP bins: stream=37, tiles=1, components=1, marker_bytes=0, SOD=false: rejected legal bin: MalformedInput { offset: 0, kind: MissingRequiredStructure }
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_automatic_headers_contract_tests.rs:L31-L33`](files/src/jpip_automatic_headers_contract_tests.rs#L31-L33)

```text
    cache
        .insert(fragment(target, split as u64, &bytes[split..], true))
        .unwrap();
```

[`src/jpip_automatic_headers_contract_tests.rs:L39-L40`](files/src/jpip_automatic_headers_contract_tests.rs#L39-L40)

```text
    assert!(cache.is_complete(target));
    assert_eq!(cache.databin_view(target).unwrap().as_bytes(), bytes);
```

[`src/jpip_automatic_headers_contract_tests.rs:L203-L216`](files/src/jpip_automatic_headers_contract_tests.rs#L203-L216)

```text
                for with_sod in [true, false] {
                    let mut raw = markers.clone();
                    if with_sod {
                        raw.extend_from_slice(&[0xff, 0x93]);
                    }
                    let mut cache = DatabinCache::new(ResourceLimits::default());
                    contract_insert(&mut cache, DatabinId::new(6, stream, 0).unwrap(), &main);
                    for tile in 0..tiles {
                        contract_insert(
                            &mut cache,
                            DatabinId::new(2, stream, u64::from(tile)).unwrap(),
                            &raw,
                        );
                    }
```

[`src/jpip_automatic_headers_contract_tests.rs:L223-L223`](files/src/jpip_automatic_headers_contract_tests.rs#L223)

```text
                    match cache.automatic_headers(stream) {
```

[`src/jpip_automatic_headers_contract_tests.rs:L242-L244`](files/src/jpip_automatic_headers_contract_tests.rs#L242-L244)

```text
                        Err(error) => {
                            failures.push(format!("{context}: rejected legal bin: {error:?}"))
                        }
```

[`src/jpip_automatic_headers_contract_tests.rs:L260-L264`](files/src/jpip_automatic_headers_contract_tests.rs#L260-L264)

```text
    assert!(
        failures.is_empty(),
        "automatic header normalization failures:\n{}",
        failures.join("\n")
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_automatic_headers_contract_tests.rs`](files/src/jpip_automatic_headers_contract_tests.rs)
- [`validation/build_automatic_headers_contract.py`](files/validation/build_automatic_headers_contract.py)

**Build commands** (in order):

```sh
python3 validation/build_automatic_headers_contract.py
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first host-observed failing check is the final failures.is_empty() assertion in automatic_headers_optional_sod_contract; its first collected failure is stream=37, tiles=1, components=1, marker_bytes=0, SOD=false returning MissingRequiredStructure at offset 0. This is a valid direct unit-contract entry to automatic_headers, not a fixture bypassing the owning normalization caller. Realism check: each case has 1/2/7 tiles, 1/3 components, image dimensions at most 14x3 (42 pixels), one tile-part/tile and no recursive nesting. The largest cache in this failing sweep contains 78 main-header bytes plus seven 9-byte tile bins, 141 bytes total, at most eight bins and 16 insertions. Default limits are HARDENED (16,384 tiles, 64 components, 100,000,000 pixels, 2,147,483,648 input bytes, 1,048,576 fragments), so these are ordinary small inputs, not a capacity/stack stress case. The real derive_precinct_map caller obtains its headers by this same automatic_headers call; its existing regression also builds a cache from completed main and tile bins before mapping. Precinct packet data is not an entry requirement for deriving planning headers. The replay builds the unchanged implementation with only an additive test include, constructs main/class-6 and tile/class-2 bins through insert and JpipFragment::new, explicitly completes even empty bins, and checks completion and exact retrieved bytes. The independent full-codestream fixture passes parse_layout before the target call. The applicable authority is the rfc_map parent's pinned T.808 (12/2022), ISO/IEC 15444-9:2023, Annex A.3.3, independently read through read_rfc_section using origin item 644e28e929d66e2e5c7a925f: tile-header bins exclude SOT, 'Inclusion of SOD markers is optional', and a server must send a tile-header bin even when empty. These fixtures instantiate exactly that raw JPP representation, not the explicitly caller-reframed plan interface. SUPPORTED.md says automatic reconstruction performs its own normalization. Thus the clause is sound; requiring an input SOD would contradict this normative representation allowance, not repair the test. Trace: automatic_headers retrieves completed main and all required tile bins, computes SIZ cardinality, normalizes main, and passes each raw tile vector unchanged to normalize_automatic_tile_headers. The non-SOT parser branch returns one transition containing the original bytes even for empty input. The normalizer sets found_sod=false, consumes zero or all valid marker segments, then rejects on !found_sod before its output SOD synthesis. The target MIR confirms the normalizer call at bb51 and unchanged error propagation through bb53/bb54/bb56. MissingRequiredStructure is therefore caused by an unnecessary framing requirement, not missing/incomplete bins, a fixture assertion, or a parser/build failure. All 18 omitted-SOD cases fail at offsets 0/7/6, while the paired SOD-present calls satisfy the exact output oracle. Retained implementation-descriptive error clauses documenting SOD rejection have no caller requires; they do not override the standards-derived consumer contract or make this entry inadmissible. The aggregate assertion reports the actual Result failures adequately; this diagnosis uses that successful host replay, not the separately noted failed standalone deviation replay. Consequence: legal completed JPP header data is rejected, preventing automatic planning/reconstruction instead of synthesizing canonical SOT/SOD framing.

[Standards quote, production lines, and provenance](../../evidence.md#dev-e2df8b9fac)

</details>
