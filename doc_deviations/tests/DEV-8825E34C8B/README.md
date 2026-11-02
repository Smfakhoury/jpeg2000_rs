# Inherited main-header component NLT overrides first-tile-header default

`DEV-8825E34C8B` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t801-202106.txt § A.3.10

**Required behavior:** Select a retained component-specific NLT before a retained default, as documented by the current local model; standards-compliant use must additionally preserve tile-header precedence over main-header selectors.

**Observed deviation:** Inherited main-header component NLT overrides first-tile-header default

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-8825E34C8B --output rfc-results/DEV-8825E34C8B
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
target/nonlinear-lookup-contract --exact codestream::tests::nonlinear_lookup_standard_header_precedence --nocapture
```

### Test input and recorded failure

Parser-created owned receiver; valid component 0 of three; first and only tile-part; unique per-header selectors; bounded native 64x32 fixture; Rsiz=0x8200; legal Tnlt=2 LUT payloads and marker lengths. No production modification, manual merged-registry construction, skipped owning parser, or violation of subject requires. This establishes admissible unit entry and selection failure, not full decoder execution.

**Recorded failure** (matched by the replay runner):

```text
parse_layout -> first tile extensions -> nonlinear_transform_for: Rsiz=0x8200, main=[(Some(0), 9), (Some(2), 9)], tile=[(Some(2), 11), (None, 10)], component=0, expected=Some(NonlinearPointTransform { component: None, precision: 10, signed: false, transform_type: 2, parameters: [0, 1, 0, 0, 0, 0, 255, 255, 255, 255, 8, 0, 255], d_min: Some(0), d_max: Some(4294967295), point_bits: Some(8), points: [0, 255], gamma_parameters: None }), actual=Some(NonlinearPointTransform { component: Some(0), precision: 9, signed: false, transform_type: 2, parameters: [0, 1, 0, 0, 0, 0, 255, 255, 255, 255, 8, 0, 255], d_min: Some(0), d_max: Some(4294967295), point_bits: Some(8), points: [0, 255], gamma_parameters: None })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`validation/nonlinear_lookup_contract.rs:L79-L79`](files/validation/nonlinear_lookup_contract.rs#L79)

```text
            bytes[6..8].copy_from_slice(&0x8200_u16.to_be_bytes());
```

[`validation/nonlinear_lookup_contract.rs:L85-L85`](files/validation/nonlinear_lookup_contract.rs#L85)

```text
            let layout = parse_layout(&bytes, &limits).expect("bounded, capability-signalled NLT header fixture must parse");
```

[`validation/nonlinear_lookup_contract.rs:L91-L91`](files/validation/nonlinear_lookup_contract.rs#L91)

```text
            let owner = &layout.tile_parts[0].part2_extensions;
```

[`validation/nonlinear_lookup_contract.rs:L94-L97`](files/validation/nonlinear_lookup_contract.rs#L94-L97)

```text
                let expected_entry = tile.iter().find(|(s, _)| *s == Some(component))
                    .or_else(|| tile.iter().find(|(s, _)| s.is_none()))
                    .or_else(|| main.iter().find(|(s, _)| *s == Some(component)))
                    .or_else(|| main.iter().find(|(s, _)| s.is_none()));
```

[`validation/nonlinear_lookup_contract.rs:L99-L99`](files/validation/nonlinear_lookup_contract.rs#L99)

```text
                let actual = owner.nonlinear_transform_for(component);
```

[`validation/nonlinear_lookup_contract.rs:L109-L109`](files/validation/nonlinear_lookup_contract.rs#L109)

```text
    assert!(failures.is_empty(), "NLT header precedence violations: {failures:#?}");
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`validation/nonlinear_lookup_contract.rs`](files/validation/nonlinear_lookup_contract.rs)
- [`validation/build_nonlinear_lookup_contract.py`](files/validation/build_nonlinear_lookup_contract.py)

**Build commands** (in order):

```sh
python3 validation/build_nonlinear_lookup_contract.py
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The failed check is the standards case's final assert!(failures.is_empty()), after parsing and owner-preservation checks and the 192-observation count. Its recorded mismatches show a main-header component-specific precision-9 descriptor returned instead of the tile-default precision-10 descriptor. This violates the standards-precedence half of clause 32aa5ed6bcd40eed56feecfe, not its accurately documented local specific-before-default rule. I read clause_origin (rfc_map) and re-read T.801 (06/2021) A.3.10 and A.2.1 using origin a8d4c31de8ce4de63071ab43: A.3.10 explicitly says 'Tile-part NLT > Tile-part NLT default > Main NLT > Main NLT default', and A.2.1 confirms Rsiz=0x8200 signals Part 2 and NLT without CAP. The fixture meets the applicable conditions: main and first tile-part headers only, unique selectors per header, component indices 0 and 2 within three components, optional 65535 default, legal unsigned depths 8–11, Tnlt=2 with two LUT values and Lnlt=19, Dmin=0/Dmax=4294967295 and PTval=8. Before tracing the implementation I checked entry scale: each of the 64 independently parsed inputs has one 64x32 image (2048 pixels, 6144 component samples), three components, one tile/first tile-part, one layer/resolution, three empty packet headers, at most six 21-byte NLT segments, and at most 216 input bytes. These are far below default limits (2 GiB input, 16 MiB metadata, 100 million pixels, 64 components, 16384 tiles/parts); no deep recursion or fabricated pool/stack state is involved. Real collect_nonlinear_regions also takes the tile's parser-produced extensions and queries the same component indices. This is a valid direct unit-contract call with no extra subject requires, not a claim that the harness executes collect_nonlinear_regions or reconstructs pixels. The owning parser is actually run: first tile extensions inherit main extensions; parse_nlt replaces only matching Option<u16> selectors, so a tile default leaves inherited main specifics intact. describe_declaration and the MIR CFG confirm the lookup's first scan returns an inherited specific through bb4 -> bb5 -> bb6 -> bb19, never reaching the default scan. Consequence: the selected descriptor/precision comes from the wrong header. The output tail truncates the panic heading and earliest mismatch, so I do not identify the first failing combination or claim a mismatch count; it nevertheless retains complete expected/actual route records and the failing native test result. Local-model agreement does not invalidate this standards-derived requirement. Edition scope remains the acquired June 2021 T.801 text, not uninspected newer editions; no numerical LUT or end-to-end image deviation is asserted.

[Standards quote, production lines, and provenance](../../evidence.md#dev-8825e34c8b)

</details>
