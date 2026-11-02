# Tile-header parser rejects consistent combined PLM and PLT metadata permitted by T.800 A.7.3

`DEV-C36C516466` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t800-201906.txt § A.7.3

**Required behavior:** Provide the documented PLM/PLT packet-length support and associated tile-part metadata checks; the implementation's exclusion of combined sources requires reconciliation with that feature intent and T.800 A.7.3.

**Observed deviation:** Tile-header parser rejects consistent combined PLM and PLT metadata permitted by T.800 A.7.3

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-C36C516466 --output rfc-results/DEV-C36C516466
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
python3 validation/read-tile-part-contract.py run plm_plt
```

### Test input and recorded failure

One component/tile at 1x1, reversible Part1 style, zero decompositions, compatible one-step QCD and initialized component tables; first tile-part with exact Psot and trailing EOC. Counts are 1,2,127,128,255, within default 256-layer admission and the cited edition's 8-bit Nplm bound. PLM has Zplm=0 and Nplm=count followed by count complete positive one-byte length codewords; the borrowed selected descriptor has exact count/size. PLT has Zplt=0 and identical codewords. Payload consists of count zero bytes, each an empty inline packet, so declared lengths sum to payload length. No packed headers, transforms, extension state, malformed private parser fields, oversized allocations or injected downstream failure. This is valid direct-unit evidence, explicitly not a claim that the separately discrepant native parse_plm accepts standard 8-bit framing.

**Recorded failure** (matched by the replay runner):

```text
consistent PLM/PLT must work separately AND together: ["count=1 sources=3: MalformedInput { offset: 84, kind: InvalidMarkerOrder }", "count=2 sources=3: MalformedInput { offset: 85, kind: InvalidMarkerOrder }", "count=127 sources=3: MalformedInput { offset: 210, kind: InvalidMarkerOrder }", "count=128 sources=3: MalformedInput { offset: 211, kind: InvalidMarkerOrder }", "count=255 sources=3: MalformedInput { offset: 338, kind: InvalidMarkerOrder }"]
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/codestream/read_tile_part_contract.rs:L52-L60`](files/src/codestream/read_tile_part_contract.rs#L52-L60)

```text
        assert_eq!(plm.payload[0], 0);
        assert_eq!(usize::from(plm.payload[1]), plm.payload.len() - 2);
        let encoded = &plm.payload[2..];
        assert!(encoded.iter().all(|&b| b == 1));
        main_lengths = PacketLengths {
            source: PacketLengthSource::Plm(encoded),
            packet_count: encoded.len() as u64,
            encoded_bytes: encoded.len() as u64,
        };
```

[`src/codestream/read_tile_part_contract.rs:L62-L79`](files/src/codestream/read_tile_part_contract.rs#L62-L79)

```text
    let sot = parser.read_sot(1).unwrap();
    let result = parser.read_tile_part(
        sot,
        (style, quant),
        ComponentCodingStyles::new(size.components, style).unwrap(),
        ComponentQuantizations::new(size.components, quant).unwrap(),
        RoiShifts::none(),
        ProgressionChanges::none(),
        MctParameters {
            arrays: vec![],
            records: vec![],
            order: vec![],
        },
        Part2Extensions::default(),
        size.components,
        main_lengths,
        PackedPacketHeaders::none(),
    );
```

[`src/codestream/read_tile_part_contract.rs:L324-L339`](files/src/codestream/read_tile_part_contract.rs#L324-L339)

```text
    for count in [1_u16, 2, 127, 128, 255] {
        for sources in 1..=3 {
            let lengths = vec![1; usize::from(count)];
            let mut main = vec![tile_contract_cod(count, 0, 2, 0), segment(QCD, &[0, 0])];
            if sources & 1 != 0 {
                let mut payload = vec![0, count as u8]; // Zplm, 8-bit Nplm
                payload.extend(vec![1; usize::from(count)]);
                main.push(segment(PLM, &payload));
            }
            let tile = if sources & 2 != 0 {
                vec![plt(0, &lengths)]
            } else {
                vec![]
            };
            let bytes =
                tile_contract_stream(1, 0, &main, &tile, &vec![0; usize::from(count)], false);
```

[`src/codestream/read_tile_part_contract.rs:L350-L353`](files/src/codestream/read_tile_part_contract.rs#L350-L353)

```text
    assert!(
        failures.is_empty(),
        "consistent PLM/PLT must work separately AND together: {failures:?}"
    );
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/codestream/read_tile_part_contract.rs`](files/src/codestream/read_tile_part_contract.rs)
- [`validation/read-tile-part-contract.py`](files/validation/read-tile-part-contract.py)

**Build commands** (in order):

```sh
python3 validation/read-tile-part-contract.py build
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The first failing assertion is the final failures.is_empty() check at src/codestream/read_tile_part_contract.rs:350, reporting 'consistent PLM/PLT must work separately AND together'. The first collected mismatch is count=1 sources=3: MalformedInput { offset: 84, kind: InvalidMarkerOrder }; all five combined-source cases fail, whereas the same replay reports no failures for either source separately. This is a specific rejection returned by the subject, not a setup panic or merely exit 101.

Realism checked first: each stream has one 1x1, 8-bit, unit-sampled component, one tile/first tile-part, zero decompositions, 1/2/127/128/255 layers and one one-byte empty inline packet per layer. At most 255 packet-length bytes per source and 255 payload bytes are requested. The largest combined stream is 857 bytes (92+3*255); its PLM payload is 257 bytes and its PLT payload 256, far below marker-length and platform integer limits. Default limits are HARDENED: 256 layers, 64 components, 67,108,864 packets, 268,435,456 packet-header bytes, 2,147,483,648 input bytes and 1,000,000,000 work units. No nesting, giant stack allocation or exhaustion is induced. Real parse_layout selects a PLM group for the current tile-part and passes it alongside inherited tables to this exact helper; the unit uses those same argument shapes and smaller ordinary values.

Authority is the rfc_map origin, not an inferred implementation-only clause. I read its parent finding and manifest, then retrieved pinned T.800 (06/2019), SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e, A.7.2 and A.7.3 using the originating mapping item. Both explicitly say PLM and PLT are optional and can be used together or separately. A.7.2 specifies 8-bit Nplm with at most 255 encoded bytes, which these fixtures obey; A.7.3 permits PLT in this tile-part header before its described packets. Both descriptions encode identical positive one-byte lengths and totals, with no PPM/PPT/SOP/EPH ambiguity. SUPPORTED.md advertises PLM/PLT support. Thus the coexistence oracle faithfully exercises this clause's originating standard finding, rather than confusing PLM/PLT with the separate PPM/PPT exclusion. Existing implementation-local contracts that describe this rejection do not override the standards-derived requirement. This finding is scoped to the pinned 2019 edition; the origin notes that newer advertised reference editions remain unreconciled, and this is not a newer-edition or whole-decoder conformance verdict.

The admissible route is a direct unit_contract entry (work-item unit assessment available, requires=[]), not a claim that the public parser accepts this standard-framed PLM. The replay's test-only include exposes the native test module without changing production behavior. tile_contract_unit uses new_parser, consumes SOC/SIZ/COD/QCD and SOT with real operations, constructs consistent inherited style/quantization tables, and supplies empty ROI/progression/MCT/Part2/packed-header state. It reads a genuine standard PLM segment, checks Zplm=0, Nplm=encoded.len(), and every encoded byte=1, then constructs PacketLengths::Plm borrowing just the actual Iplm bytes with exact packet_count and encoded_bytes. This is the selected-group value consumed by read_tile_part and its iterator, not an impossible descriptor or arbitrary parser-field mutation. The helper has no requirement that this value be produced by parse_plm, nor any minimum PLM-related resource-counter precondition. Native parse_plm's separate four-byte Nplm convention is deliberately bypassed at this unit boundary; consequently this replay does not establish an end-to-end standard-PLM caller route or diagnose that separate framing behavior.

describe_declaration confirms the descriptor is the PacketLengths argument _11. control_flow_graph shows the precise failure: PLT comparison bb132 -> is_present(_11) bb133 -> true bb134 -> segment offset bb135 -> InvalidMarkerOrder bb136 -> Err bb137, then cleanup. Source read_tile_part rejects immediately whenever main_packet_lengths.is_present(), before checking or decoding the PLT payload. For count=1, the selected PLM segment ends at byte72, SOT advances to84, and the valid PLT at84 triggers this unconditional exclusion. Consistent, legally coexisting packet-length metadata is therefore rejected and no tile payload is returned. The clause is sound on this admissible unit entry; the production exclusion is the deviation. No production files were changed.

[Standards quote, production lines, and provenance](../../evidence.md#dev-c36c516466)

</details>
