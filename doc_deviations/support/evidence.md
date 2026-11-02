# Test evidence

## DEV-0A9BF676EF

- **Mapping:** `644e28e929d66e2e5c7a925f` / `bd15e57ea85f5d17c5886b2269c4b780f08f9df3b98bbf01246ba631daf4dbdf`
- **Specification:** `f2c61ee4ddec8a88d2a0a9e7`
- **Deviation:** `4264ae18da850a4abdc4c97b`
- **Test item:** `0825d2151cc175fbae2c4912`
- **Case:** `46e0d3178e7a407a7247f7b8`
- **Exact standards quote:**

> Length, in bytes, from the beginning of the first byte of this SOT marker segment of the tile-part to
>                     the end of the data of that tile-part.

- **Entry:** `src/jpip.rs:3088:jpip::DatabinCache::transcode_precinct_codestream`
- **Command:** `python3 validation/jpip-transcode-suite.py run explicit_profile_rejects_all_non_lrcp_and_attached_sources`
- **Setup:** Safe JpipPrecinctCodestreamPlan::new, JpipPrecinctPacket::new, DatabinId::new, JpipFragment::new and DatabinCache::insert establish an admissible public unit-contract entry. No private state is fabricated; private accesses only snapshot state. One layer and one one-byte packet, complete finalized headers, and three bins totaling 104 bytes are far below default limits. The reconstructed stream is 106 bytes, 1x1, one component and one tile-part. parse_layout and independent fixture-premise checks precede the target call and passed under host replay. The deliberately non-LRCP effective tile is the profile violation the routine must reject, not a violated entry requirement.
- **Route:** Host replay runs python3 validation/jpip-transcode-suite.py run explicit_profile_rejects_all_non_lrcp_and_attached_sources. The registered native unit test creates simple_plan(1), constructs complete cache bins via insert, and changes only tile[17] to progression 1..4 in the failing variants. It validates fixture parsing/profile premises, then call(&c,&p,1) invokes cache.transcode_precinct_codestream(plan,k). Available data reaches reconstruction and validate_precinct_codestream; that helper checks main but not effective tile progression and returns Ok. The public method returns Available/Some, which the harness accumulates and reports in its final failed assertion.
- **Observed assertion:** `profile contract violations:<br>effective tile progression 1: published output=true, status=Available<br>effective tile progression 2: published output=true, status=Available<br>effective tile progression 3: published output=true, status=Available<br>effective tile progression 4: published output=true, status=Available`

**Full diagnosis:**

The first host-observed failed assertion is failures.is_empty() at replayed src/jpip/transcode_contract_tests.rs:697, reporting effective tile progression 1..4: published output=true, status=Available. This specifically refutes clause f2c61ee4ddec8a88d2a0a9e7's effective-tile LRCP publication condition, not an arbitrary nonzero exit. clause_origin is infer; SUPPORTED.md independently restricts explicit plans to a single LRCP tile-part, supporting the clause rather than revision to match the missing check. The needs_review label concerns retained dependency-summary gaps and does not invalidate this independently traced profile guarantee.

Entry realism was checked before tracing implementation: each failing variant has one 1x1 image tile, one component, one resolution, one layer, one one-byte packet, three complete bins/fragments totaling 104 stored bytes and 106 reconstructed bytes including EOC. Defaults are HARDENED: 2,147,483,648 input bytes, 256 layers, 67,108,864 packets, 1,048,576 JPIP fragments and 16,384 tiles/tile-parts. There is no deep nesting, oversized allocation or stack demand. The 17 profile variants run sequentially; no capacity-exceeding fixture is involved. The real update_inner caller likewise passes an available layer prefix of its constructed plan; this direct public unit-contract call uses k=1=p.layers() and need not instantiate the progressive decoder.

The fixture constructs plan, IDs, ranges, fragments and cache through safe constructors and insert, not private representation writes. Header-byte edits are legal supplied data: the test deliberately changes the tile COD progression byte to each valid order 1..4 while preserving main LRCP, correct segment framing, one tile-part and the layer count. Before the target call it independently assembles the expected reconstruction, parses it with default limits, checks tile/layer counts and verifies the effective tile is outside the profile. Those premise assertions passed in the host replay. Non-LRCP input is not a forbidden entry precondition: this routine is responsible for rejecting complete out-of-profile reconstructions; the constructor cannot inspect header bytes. A one-layer, one-component, one-resolution, one-precinct empty packet is also consistent across the different progression orders. The test-only registration adds a child test module; the replay build succeeded and production implementation is unchanged.

Compiler declaration/CFG and source establish the route: packet_end_for_layers succeeds; combined availability is Available; header snapshots are patched only for layer counts; the selected byte is copied; Psot and total length are rebuilt; validate_precinct_codestream parses successfully. Its MIR bb13 tests only the MAIN progression, then bb14..bb24 test POC and attached sources, reaching bb27 Ok without inspecting tile coding_style.progression_order. The parser explicitly assigns tile COD to tile_coding_style and publishes it as TilePartPayload.coding_style. The parent MIR bb114 calls this validator and bb117 publishes Some(JpipByteView). Thus the specific missing effective-tile progression guard allows all four host-observed non-LRCP outputs. This is an agent-assessed reproduced public unit-contract deviation, not a formal caller-reachability proof; no downstream entropy-decoding failure or image corruption is claimed.

- **Production evidence:** [`SUPPORTED.md:L138-L140`](../../SUPPORTED.md#L138-L140)

```text
Explicit `JpipPrecinctCodestreamPlan` transcodes are restricted to a single
tile-part with LRCP progression, inline packet headers, and no POC, PLT, or PPT
in the reconstructed input.
```

- **Production evidence:** [`src/jpip.rs:L3745-L3760`](../../src/jpip.rs#L3745-L3760)

```text
    let layout = crate::codestream::parse_layout(bytes, limits)?;
    if layout.info.image_size.tile_count()? != 1
        || layout.tile_parts.len() != 1
        || layout.info.coding_style.layers != layers
        || !matches!(
            layout.info.coding_style.progression_order,
            crate::codestream::ProgressionOrder::Lrcp
        )
        || layout.info.progression_changes.is_present()
        || layout.tile_parts[0].progression_changes.is_present()
        || layout.tile_parts[0].packet_lengths.is_present()
        || layout.tile_parts[0].packed_packet_headers.is_present()
    {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipLayout));
    }
    Ok(())
```

- **Production evidence:** [`src/jpip.rs:L3158-L3163`](../../src/jpip.rs#L3158-L3163)

```text
        validate_precinct_codestream(&bytes, layers, &self.limits)?;
        Ok(JpipPrecinctTranscode {
            layers,
            availability,
            codestream: Some(JpipByteView { bytes }),
        })
```

- **Production evidence:** [`src/codestream.rs:L9910-L9911`](../../src/codestream.rs#L9910-L9911)

```text
                        tile_cod_seen = true;
                        tile_coding_style = self.parse_cod(segment)?;
```

- **Production evidence:** [`src/jpip.rs:L3115-L3116`](../../src/jpip.rs#L3115-L3116)

```text
        let tile_part_length = u32::try_from(tile_part_length)
            .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
```

- **Production evidence:** [`src/jpip.rs:L3127-L3130`](../../src/jpip.rs#L3127-L3130)

```text
        tile_header
            .get_mut(6..10)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?
            .copy_from_slice(&tile_part_bytes);
```

- **Production evidence:** [`src/jpip.rs:L3154-L3157`](../../src/jpip.rs#L3154-L3157)

```text
        bytes.extend_from_slice(&main_header);
        bytes.extend_from_slice(&tile_header);
        bytes.extend_from_slice(&packet_bytes);
        bytes.extend_from_slice(&EOC);
```

- **Production evidence:** [`src/jpip.rs:L3616-L3618`](../../src/jpip.rs#L3616-L3618)

```text
    if header.len() < 14
        || header.get(0..2) != Some(&[0xff, 0x90])
        || header.get(2..4) != Some(&[0, 10])
```

- **Harness evidence:** [`src/jpip/transcode_contract_tests.rs:L53`](tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L53): `    let result = cache.transcode_precinct_codestream(plan, k);`
- **Harness evidence:** [`src/jpip/transcode_contract_tests.rs:L622-L629`](tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L622-L629): `        let mut tile = t.clone();<br>        tile[17] = progression;<br>        variants.push((<br>            format!("effective tile progression {progression}"),<br>            h.clone(),<br>            tile,<br>            false,<br>        ));`
- **Harness evidence:** [`src/jpip/transcode_contract_tests.rs:L667-L668`](tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L667-L668): `        let layout = crate::codestream::parse_layout(&expected, &limits)<br>            .unwrap_or_else(\|e\| panic!("profile fixture {name} must parse: {e:?}"));`
- **Harness evidence:** [`src/jpip/transcode_contract_tests.rs:L684-L686`](tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L684-L686): `        assert_eq!(explicit, eligible, "independent fixture premises {name}");<br>        let c = complete_cache(&main, &tile, &[0], ResourceLimits::default(), false);<br>        match call(&c, &p, 1) {`
- **Harness evidence:** [`src/jpip/transcode_contract_tests.rs:L688-L693`](tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L688-L693): `            Err(e) if !eligible && e == malformed(0, MalformedKind::InvalidJpipLayout) => {}<br>            Ok(r) => failures.push(format!(<br>                "{name}: published output={}, status={:?}",<br>                r.codestream().is_some(),<br>                r.availability().status()<br>            )),`
- **Harness evidence:** [`src/jpip/transcode_contract_tests.rs:L697-L701`](tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L697-L701): `    assert!(<br>        failures.is_empty(),<br>        "profile contract violations:\n{}",<br>        failures.join("\n")<br>    );`
- **Test source:** [`src/jpip/transcode_contract_tests.rs:L53`](tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L53)<br>[`src/jpip/transcode_contract_tests.rs:L622-L629`](tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L622-L629)<br>[`src/jpip/transcode_contract_tests.rs:L667-L668`](tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L667-L668)<br>[`src/jpip/transcode_contract_tests.rs:L684-L686`](tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L684-L686)<br>[`src/jpip/transcode_contract_tests.rs:L688-L693`](tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L688-L693)<br>[`src/jpip/transcode_contract_tests.rs:L697-L701`](tests/DEV-0A9BF676EF/files/src/jpip/transcode_contract_tests.rs#L697-L701)
- **Test source:** [`validation/jpip-transcode-registration.patch`](tests/DEV-0A9BF676EF/files/validation/jpip-transcode-registration.patch)
- **Test source:** [`validation/jpip-transcode-suite.py`](tests/DEV-0A9BF676EF/files/validation/jpip-transcode-suite.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-926EF1A301

- **Mapping:** `b74f5a18aa024a0fe1bc915d` / `e6cd36cac4cb0b06a17235ea91be4d4b954ff15a7e292a24ffcafb335b86ac5b`
- **Specification:** `20f954947eed5f28a085ba7e`
- **Deviation:** `855a0fa63dd570a3b1b405b2`
- **Test item:** `32cc62dbf63c6da1321051c0`
- **Case:** `294dc3cab57bd4b0e4d6345f`
- **Exact standards quote:**

> Psot:     Length, in bytes, from the beginning of the first byte of this SOT marker segment of the tile-part to
>                     the end of the data of that tile-part.

- **Entry:** `src/jpip.rs:3874:jpip::parse_tile_part_transitions`
- **Command:** `python3 validation/tile_transition_suite.py run contract_transition_psot_lengths`
- **Setup:** A safe direct unit-contract invocation supplies a live immutable slice and matching expected_tile. SOT's length field is 10, so the complete SOT occupies 12 bytes; SOD occupies bytes 12..14. There are no intermediate segments or packet-data requirements. Psot=13 fits usize and is intentionally semantically malformed, not invalid fixture memory: validating this condition belongs to the target, whose entry has no precondition excluding malformed lengths. Part index 5 and total count 0 do not violate this helper's entry requirements. The single 14-byte header and tiny associated vectors are comfortably within resource limits; no production mutation or unsafe access is introduced.
- **Route:** The replay wrapper executes the exact native jpip::tests::contract_transition_psot_lengths case. After positive controls, its first malformed iteration has empty markers, H=14, length=13, tile=7, part=5, and empty prefix. contract_part produces the 14 bytes printed by the host; contract_error calls parse_tile_part_transitions(input, tile) and compares the returned Result to InvalidTilePart at offset 0. The parser accepts SOT and SOD, maps nonzero length underflow to None, pushes one transition and returns Ok, causing the equality failure.
- **Observed assertion:** `assertion `left == right` failed: route=parse_tile_part_transitions(tile=7); input=[ff, 90, 00, 0a, 00, 07, 00, 00, 00, 0d, 05, 00, ff, 93]<br>  left: Ok([JpipTilePartTransition { tile: 7, tile_part: 5, packet_count: None, data_length: None, functional: false, header: [255, 144, 0, 10, 0, 7, 0, 0, 0, 13, 5, 0, 255, 147] }])<br> right: Err(MalformedInput { offset: 0, kind: InvalidTilePart })`

**Full diagnosis:**

The first host-observed failure is the result equality in contract_error at src/jpip_transition_contract_tests.rs:53: a complete 14-byte SOT/SOD header declaring Psot=13 returns Ok with data_length=None instead of Err(MalformedInput { offset: 0, kind: InvalidTilePart }). The earlier input-preservation and sentinel assertions passed. This directly refutes clause 20f954947eed5f28a085ba7e's nonzero L<H rejection, not merely its inferred choice of error category: any successful transition here violates the documented distinction between a nonzero original length and the legal zero-Psot unknown-length form.

Resource/entry realism: the failing call borrows one 14-byte Vec-backed immutable slice, contains one 12-byte SOT and one 2-byte SOD, uses tile 7 and part 5 with an unspecified total-part count, and has no recursion, threads, pool demand, or packet-body allocation. This is far below the configured HARDENED input limit of 2,147,483,648 bytes and tile-part limit of 16,384. The selected Psot case's preceding positive inputs have headers no larger than 273 bytes (at most two retained headers in the zero-nonfinal checks); even u32::MAX is only a declared scalar length, never an allocation request. These are ordinary header-sized buffers like those supplied by DatabinCache::tile_part_transitions, which passes complete cached bytes directly to this parser without validating Psot. No entry contract requires valid declared lengths, sequential part numbers, or the presence of original packet bytes: this helper validates retained headers, and such a malformed declaration is precisely the rejection domain under test. The direct unit-contract call therefore does not skip an owning caller required to establish state. Rust shared borrows and all accessed segment/read bounds are valid.

The replay builds the native library tests and adds access only inside the existing cfg(test) module. Its other patch changes only a pre-existing test fixture's declared lengths; no production parser behavior is altered. The failure occurs after that existing positive fixture and the generated valid-length/zero-form checks complete.

Compiler declaration confirms the immutable slice/u16 entry and owned Result return. The CFG follows the SOT branch, checks segment size 12 and matching tile, reads Psot=13, recognizes SOD at offset 12 and advances to 14. The nonfinal-zero rejection is inapplicable. It calculates H=14 in bb144-148, converts the nonzero declaration in bb151-155, then checked_sub(13,14) produces None in bb155; Option::map in bb157 preserves None, bb158-164 pushes that transition, and bb188 -> bb46 -> bb165 returns Ok. Source documentation explicitly reserves None for legal zero Psot, so this is not a spec_error permitting lenient interpretation. The exact InvalidTilePart category remains inferred from neighboring structural validation, as the reviewed clause discloses, but that uncertainty cannot explain acceptance. The sentinel check is weak evidence of general framing and is not relied on; the actual host-observed Ok result, exact input, native harness call, and CFG ground this agent-assessed reproduced deviation.

- **Production evidence:** [`src/jpip.rs:L1148-L1150`](../../src/jpip.rs#L1148-L1150)

```text
    /// Original tile-part data bytes derived from a non-zero `Psot`.
    ///
    /// `None` denotes the legal final-tile-part form whose `Psot` is zero.
```

- **Production evidence:** [`src/jpip.rs:L4030-L4037`](../../src/jpip.rs#L4030-L4037)

```text
        let data_length = if tile_part_length == 0 {
            None
        } else {
            let declared = usize::try_from(tile_part_length).map_err(|_| {
                DecodeError::malformed(part_start as u64, MalformedKind::ArithmeticOverflow)
            })?;
            declared.checked_sub(header_length).map(|data| data as u64)
        };
```

- **Production evidence:** [`src/jpip.rs:L1929-L1930`](../../src/jpip.rs#L1929-L1930)

```text
        let bytes = self.databin_view(id)?.into_bytes();
        let transitions = parse_tile_part_transitions(&bytes, tile)?;
```

- **Production evidence:** [`src/limits.rs:L133`](../../src/limits.rs#L133)

```text
        maximum_input_bytes: 2_147_483_648,
```

- **Production evidence:** [`src/limits.rs:L143`](../../src/limits.rs#L143)

```text
        maximum_tile_parts: 16_384,
```

- **Production evidence:** [`src/jpip.rs:L4036`](../../src/jpip.rs#L4036)

```text
            declared.checked_sub(header_length).map(|data| data as u64)
```

- **Production evidence:** [`src/jpip.rs:L1148-L1150`](../../src/jpip.rs#L1148-L1150)

```text
    /// Original tile-part data bytes derived from a non-zero `Psot`.
    ///
    /// `None` denotes the legal final-tile-part form whose `Psot` is zero.
```

- **Production evidence:** [`src/jpip.rs:L1955-L1957`](../../src/jpip.rs#L1955-L1957)

```text
            if transition.packet_count().is_none() && transition.data_length().is_none() {
                preceding_packets_known = false;
            }
```

- **Harness evidence:** [`src/jpip_transition_contract_tests.rs:L3-L11`](tests/DEV-926EF1A301/files/src/jpip_transition_contract_tests.rs#L3-L11): `fn contract_part(tile: u16, part: u8, psot: u32, markers: &[u8]) -> Vec<u8> {<br>    let mut payload = tile.to_be_bytes().to_vec();<br>    payload.extend_from_slice(&psot.to_be_bytes());<br>    payload.extend_from_slice(&[part, 0]);<br>    let mut header = segment(0xff90, &payload);<br>    header.extend_from_slice(markers);<br>    header.extend_from_slice(&[0xff, 0x93]);<br>    header<br>}`
- **Harness evidence:** [`src/jpip_transition_contract_tests.rs:L234-L245`](tests/DEV-926EF1A301/files/src/jpip_transition_contract_tests.rs#L234-L245): `    for markers in [vec![], segment(0xff64, &[0, 0, 3])] {<br>        let h = 14 + markers.len();<br>        for length in [h as u32 - 1, 1] {<br>            let bad = contract_part(7, 5, length, &markers);<br>            for prefix in [vec![], contract_part(7, 0, 14, &[])] {<br>                let offset = prefix.len();<br>                contract_error(<br>                    &[prefix, bad.clone()].concat(),<br>                    7,<br>                    offset,<br>                    MalformedKind::InvalidTilePart,<br>                );`
- **Harness evidence:** [`src/jpip_transition_contract_tests.rs:L50-L60`](tests/DEV-926EF1A301/files/src/jpip_transition_contract_tests.rs#L50-L60): `    let result = parse_tile_part_transitions(input, tile);<br>    assert_eq!(input, before, "error path mutated input");<br>    assert_eq!(sentinel, vec![0x31; 97], "error path mutated caller state");<br>    assert_eq!(<br>        result,<br>        Err(DecodeError::MalformedInput {<br>            offset: offset as u64,<br>            kind<br>        }),<br>        "route=parse_tile_part_transitions(tile={tile}); input={input:02x?}"<br>    );`
- **Test source:** [`src/jpip_transition_contract_tests.rs:L3-L11`](tests/DEV-926EF1A301/files/src/jpip_transition_contract_tests.rs#L3-L11)<br>[`src/jpip_transition_contract_tests.rs:L234-L245`](tests/DEV-926EF1A301/files/src/jpip_transition_contract_tests.rs#L234-L245)<br>[`src/jpip_transition_contract_tests.rs:L50-L60`](tests/DEV-926EF1A301/files/src/jpip_transition_contract_tests.rs#L50-L60)
- **Test source:** [`validation/tile_transition_access.patch`](tests/DEV-926EF1A301/files/validation/tile_transition_access.patch)
- **Test source:** [`validation/tile_transition_suite.py`](tests/DEV-926EF1A301/files/validation/tile_transition_suite.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-64FD2DDB2B

- **Mapping:** `8ff9f4815b9913140344f4a4` / `ce49573084a2d38810f6888cc3b1d1365650e05499f96102bce036c6b2d54693`
- **Specification:** `af4bc1414e81593a62244981`
- **Deviation:** `68863f8ed83222b5c6ed28e3`
- **Test item:** `7b34b23231926ec82a67e1e9`
- **Case:** `13da61704101aa4758fec233`
- **Exact standards quote:**

> Ccoc:     The index of the component to which this marker segment relates. The components are indexed 0, 1,
>                     2, etc.

- **Entry:** `src/codestream.rs:8739:codestream::Parser::parse_coc`
- **Command:** `python3 coc_clause_harness.py run admission`
- **Setup:** The harness uses ResourceLimits::HARDENED, the existing SIZ fixture (64x32, one component, Rsiz=0), one valid COD with one layer/NL=1/4x4 reversible code blocks and implicit precincts, compatible QCD, one main-header COC for component zero with Scoc=1 and NL=1, no CAP/Part 2 selectors or overrides/duplicates, and a properly framed first tile part. COC payload [0,1,1,0,0,0,1,0x11,0xf0] has exactly two precinct bytes. All entry requirements and limits are respected; only the second precinct's zero width exponent is intentionally malformed, which this parser is required to reject.
- **Route:** Replayed admission test constructs a real single-component codestream with codestream_with_headers, then calls the public parse_layout(&bytes, &limits). Its main-header component loop invokes Parser::parse_coc -> prepare_coc_payload -> parse_cod_payload and installs the returned component style. The harness records the observed Ok(layout) in failures, and its final failures.is_empty() assertion fails. This follows the owning caller, not a fabricated Parser state or an entry-name claim.
- **Observed assertion:** `main-header parse_layout accepted illegal high-resolution precinct: [(1, 1), (0, 15)]`

**Full diagnosis:**

The failed check is the admission test's final `failures.is_empty()` assertion, which aggregates mismatches rather than stopping at each comparison. The retained output begins partway through that diagnostic, so its first historical mismatch is unavailable; however, it explicitly records both erroneous Ok results for malformed precinct fields and the within-policy main-header acceptance quoted in observation. This is not an arbitrary exit-101 attribution. Clause af4bc1414e81593a62244981 requires rejecting malformed ordinary COC fields. Its origin is rfc_map, not infer. I read the parent evidence and the assigned T.800 (06/2019) A.6.1 and A.6.2 through read_rfc_section using origin item 8ff9f4815b9913140344f4a4. A.6.2 assigns SPcoc the Table A.15 parameters; A.15 orders precinct entries from NLLL through successive resolutions; Table A.21 states separately for PPx and PPy: `This value may only equal zero at the resolution level corresponding to the NLLL band.` With NL=1, the second entry 0xf0 therefore illegally specifies PPx=0 at resolution 1. This is ordinary Part 1, Rsiz=0, no CAP and no Part 2 decomposition reference, so the parent finding's unresolved directional/Part 2 applicability does not apply. This conclusion is scoped to the assigned 2019 edition; repository newer-edition references remain unreconciled and no broader conformance claim is made. The implementation and stored parse_cod_payload contract preserve zeros, but that correlated model/implementation agreement does not invalidate this standards-derived clause.

Realism checked before deeper tracing: the direct sweep executes 1,782 valid cases and over 20,000 malformed cases sequentially, with depth at most 32 (plus an intentional depth-33 rejection), at most 33 explicit precinct bytes, and framed inputs at most 51 bytes for its ordinary payloads. Its Csiz values above HARDENED.maximum_components=64 do not allocate component arrays and are accepted by the unit helper's unrestricted u16-count contract; they are not evidence of full-decoder capability. Crucially, attribution does not depend on these cases: the supplemental real caller has one component, a 64x32 image (2,048 pixels), one tile/first tile part, one layer, two resolutions, 4x4 code blocks, a nine-byte COC payload, and a 99-byte complete framed stream. These are well below HARDENED input, geometry, component, metadata and resolution limits. The entry may contain a malformed COC because rejecting such fields is the parser's obligation; all framing, selector, defaults and caller-state requirements are met. The two tile-data bytes are opaque to parse_layout; no full entropy-decoding claim is made.

Route: the harness's unmodified existing codestream_with_headers helper emits SOC/SIZ/COD/QCD/COC/SOT/SOD/data/EOC with correct lengths and Psot. parse_layout parses SIZ, validates/retains the single COC component selector, obtains the valid COD and QCD, then its component loop calls parse_coc with validated components=1 and the effective COD default. Compiler declaration and MIR CFG confirm parse_coc calls prepare_coc_payload then parse_cod_payload, propagating errors or returning the style. Preparation copies the ordinary SPcoc bytes into the synthesized COD payload. parse_cod_payload accepts its exact length and bounded dimensions/depth, then copies the precinct nibbles without checking that both are nonzero for resolution>0. It returns Ok, parse_coc returns that style, and the owning caller installs it. No Part 2 structure exists to reinterpret the fields. The host actually observes parse_layout returning the illegal style [(1,1),(0,15)], contradicting the clause's rejection guarantee. The precise project error kind/offset is not specified by the standard, but this failure is acceptance, not merely a disputed diagnostic. Production code was not changed and no new replay is needed to ground the already host-observed failing check.

- **Production evidence:** [`src/codestream.rs:L8747-L8749`](../../src/codestream.rs#L8747-L8749)

```text
let prepared = self.prepare_coc_payload(segment, components, default)?;
        let mut style =
            self.parse_cod_payload(segment.offset, &prepared.payload[..prepared.length])?;
```

- **Production evidence:** [`src/codestream.rs:L8129`](../../src/codestream.rs#L8129)

```text
precinct_exponents[resolution] = (payload[index] & 0x0f, payload[index] >> 4);
```

- **Production evidence:** [`src/codestream.rs:L4421-L4424`](../../src/codestream.rs#L4421-L4424)

```text
if let Some(segment) = coc_segment {
                let (_, style) = parser.parse_coc(segment, image_size.components, coding_style)?;
                component_coding_styles.set(component, style)?;
            }
```

- **Production evidence:** [`src/codestream.rs:L8069-L8071`](../../src/codestream.rs#L8069-L8071)

```text
if payload.len() != expected_length {
            return Err(self.malformed_at(segment_offset, MalformedKind::InvalidMarkerLength));
        }
```

- **Production evidence:** [`src/codestream.rs:L8747-L8749`](../../src/codestream.rs#L8747-L8749)

```text
let prepared = self.prepare_coc_payload(segment, components, default)?;
        let mut style =
            self.parse_cod_payload(segment.offset, &prepared.payload[..prepared.length])?;
```

- **Production evidence:** [`src/codestream.rs:L8183`](../../src/codestream.rs#L8183)

```text
let component_bytes = if components < 257 { 1_usize } else { 2_usize };
```

- **Production evidence:** [`src/codestream.rs:L8187-L8191`](../../src/codestream.rs#L8187-L8191)

```text
let component = if component_bytes == 1 {
            u16::from(segment.payload[0])
        } else {
            read_u16([segment.payload[0], segment.payload[1]])
        };
```

- **Production evidence:** [`src/codestream.rs:L8192-L8194`](../../src/codestream.rs#L8192-L8194)

```text
if component >= components {
            return Err(self.malformed_at(segment.offset, invalid_kind));
        }
```

- **Production evidence:** [`src/codestream.rs:L8779-L8782`](../../src/codestream.rs#L8779-L8782)

```text
let scoc = segment.payload[component_bytes];
        if scoc & 0xfe != 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidCodingStyle));
        }
```

- **Production evidence:** [`src/codestream.rs:L8069-L8071`](../../src/codestream.rs#L8069-L8071)

```text
if payload.len() != expected_length {
            return Err(self.malformed_at(segment_offset, MalformedKind::InvalidMarkerLength));
        }
```

- **Harness evidence:** [`src/codestream/tests/coc_clause_tests.rs:L320-L324`](tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs#L320-L324): `let cod = segment(COD, &[0, 0, 0, 1, 0, 1, 0, 0, 0, 1]);<br>    let qcd = segment(QCD, &[0, 0, 0, 0, 0]);<br>    let coc = segment(COC, &[0, 1, 1, 0, 0, 0, 1, 0x11, 0xf0]);<br>    let bytes = codestream_with_headers(1, &[cod, qcd, coc], &[], &[0, 0]);<br>    let limits = ResourceLimits::HARDENED;`
- **Harness evidence:** [`src/codestream/tests/coc_clause_tests.rs:L341`](tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs#L341): `match parse_layout(&bytes, &limits) {`
- **Harness evidence:** [`src/codestream/tests/coc_clause_tests.rs:L346-L347`](tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs#L346-L347): `Ok(layout) => failures.push(format!(<br>            "main-header parse_layout accepted illegal high-resolution precinct: {:?}",`
- **Harness evidence:** [`src/codestream/tests/coc_clause_tests.rs:L357-L362`](tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs#L357-L362): `assert!(<br>        failures.is_empty(),<br>        "{} COC contract mismatches:\n{}",<br>        failures.len(),<br>        failures.join("\n")<br>    );`
- **Test source:** [`src/codestream/tests/coc_clause_tests.rs:L320-L324`](tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs#L320-L324)<br>[`src/codestream/tests/coc_clause_tests.rs:L341`](tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs#L341)<br>[`src/codestream/tests/coc_clause_tests.rs:L346-L347`](tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs#L346-L347)<br>[`src/codestream/tests/coc_clause_tests.rs:L357-L362`](tests/DEV-64FD2DDB2B/files/src/codestream/tests/coc_clause_tests.rs#L357-L362)
- **Test source:** [`coc_clause_harness.py`](tests/DEV-64FD2DDB2B/files/coc_clause_harness.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-C56978D32B

- **Mapping:** `c0445cf15c23a5bb5de922d7` / `045ad5b0acc8dba5018adbf9c0fd2a37e87ad5208566bf2d63e48b85ce08bdd4`
- **Specification:** `c8b504049e1d05d2b877050b`
- **Deviation:** `8bbaee85a2f30c8fd352e207`
- **Test item:** `c7779586ba7e5308becdb3ab`
- **Case:** `b062a512e7267bca1f14bad8`
- **Exact standards quote:**

> This value is expressed with an 8-bit number limiting the number of Iplm bytes to 255 and the number of packets in
>           a tile-part to 255 or less.

- **Entry:** `src/codestream.rs:9168:codestream::Parser::parse_plm`
- **Command:** `target/plm-standard-tests codestream::tests::standard_plm::standard_plm_octet_counts_and_wire_order --exact --nocapture`
- **Setup:** This is a permitted direct unit-contract test of metadata interpretation. The real constructor supplies valid parser references, lifetime, limits and zero counters; the real segment reader supplies a correctly bounded raw PLM segment at offset 0. Zplm and next_index both equal 0 and groups starts empty, matching the actual main-header owner's state. Case 1 has Lplm=5, a one-byte Nplm value 1 and one completed positive codeword 1, conforming to pinned T.800 (06/2019) A.7.2 Tables A.35/A.36. Its seven total bytes, one packet and one group are well within platform/configuration limits. No owning-caller normalization is skipped, and production code is unchanged by the test-only child-module patch.
- **Route:** The host-built native libtest runs codestream::tests::standard_plm::standard_plm_octet_counts_and_wire_order. Its case 1 expected vector is [[1]]. standard_group uses the existing encode_packet_lengths helper to produce [Nplm=1,Iplm=1]; standard_marker adds Zplm=0 and the real FF57/Lplm header. new_parser(&bytes,&limits) and parser.read_segment() succeed. The test directly calls parser.parse_plm(raw,&mut next,&mut groups); the three-byte payload takes MIR bb0 -> bb1 -> bb2 -> return Err(InvalidMarkerLength). The harness appends that exact diagnostic, continues other cases, and fails assert!(failures.is_empty()).
- **Observed assertion:** `case 1: valid A.35 octet framing rejected: MalformedInput { offset: 0, kind: InvalidMarkerLength }`

**Full diagnosis:**

The first failing check in this replayed test is its final assert!(failures.is_empty(), "{}", failures.join("\n")); the preceding framing/read_segment assertions succeeded. The host tail truncates the case-0 diagnostic, so the grounded concrete witness is case 1: a single packet of length 1, encoded as marker bytes FF 57 00 05 00 01 01 (payload [Zplm=0,Nplm=1,Iplm=1]). This is a valid direct unit-contract entry, not a fixture bypassing an owning caller. new_parser initializes the actual parser and counters, read_segment returns the actual PLM Segment, next_index is zero, and groups is empty, exactly as initialized by the main-header owner. No full packet body is required by this metadata helper. Real parse_layout dispatch passes the raw segment to parse_plm with no conversion of Nplm to four bytes.

Realism was assessed before tracing the implementation: the 38 independent unit fixtures have at most three groups, at most 450 encoded information bytes and at most 458 total marker bytes each; the fixed 255-packet fixture uses 255 encoded bytes, and generated groups contain at most 30 codewords each. Nplm never exceeds 255, there is no recursive nesting or packet-body allocation proportional to numeric lengths, and all counters start at zero. These inputs are far below default limits of 2 GiB input, 16384 tile-parts, 67108864 packets, 268435456 packet-header bytes and 1000000000 work units. The seven-byte case-1 witness is itself ordinary caller metadata, independent of any large numeric case.

Authority is the rfc_map origin, not the correlated implementation-derived contracts/model/native four-byte oracle. I read its parent finding and the pinned T.800 (06/2019) A.7.2 section (SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e). Its normative Table A.35 expressly assigns Nplmi 8 bits, 0..255, and Lplm 4..65535; Table A.36 gives completed MSB-first seven-bit packet length codewords. Thus case 1 meets the cited standard and the clause's ordinary PLM main-header applicability. This is not PPM's distinct 32-bit count syntax. No claim about unaudited later-edition deltas is needed for this cited-edition finding.

describe_declaration identifies the exact helper and parameter types. Its MIR bb0 tests payload length <5; bb1/bb2 construct and return InvalidMarkerLength before any index increment, validator, or group publication. Case 1 has three payload bytes and necessarily takes this route. Source also reads Nplm with read_u32 and advances position by four on larger payloads, revealing the same wrong-width framing root cause. Rejecting this valid standard group violates the clause's requirement to interpret the main-header packet-length groups. It is not test_error: the test neither requires an invalid entry nor mistakes the implementation's four-byte framing for a normative precondition. It is not spec_error: the assigned standard and advertised PLM support sustain the clause. The concrete observed consequence is rejection, not independently demonstrated reordering or a continuation-specific defect; successful ordering checks are blocked by the framing error. The truncated first diagnostic is a weak diagnostic only, since the case-1 line and replayed harness provide an unambiguous failing target call.

- **Production evidence:** [`src/codestream.rs:L9183-L9185`](../../src/codestream.rs#L9183-L9185)

```text
if segment.payload.len() < 5 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
```

- **Production evidence:** [`src/codestream.rs:L9212-L9217`](../../src/codestream.rs#L9212-L9217)

```text
let encoded_length = match usize::try_from(read_u32([
                segment.payload[position],
                segment.payload[position + 1],
                segment.payload[position + 2],
                segment.payload[position + 3],
            ]))
```

- **Production evidence:** [`src/codestream.rs:L4276-L4279`](../../src/codestream.rs#L4276-L4279)

```text
PLM => {
                    plm_seen = true;
                    parser.parse_plm(segment, &mut next_plm_index, &mut plm_groups)?;
                }
```

- **Production evidence:** [`SUPPORTED.md:L37`](../../SUPPORTED.md#L37)

```text
- PLM/PLT packet lengths, PPM/PPT packed headers, SOP/EPH, and TLM checks.
```

- **Production evidence:** [`src/codestream.rs:L9183-L9185`](../../src/codestream.rs#L9183-L9185)

```text
if segment.payload.len() < 5 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
```

- **Production evidence:** [`src/codestream.rs:L9212-L9217`](../../src/codestream.rs#L9212-L9217)

```text
let encoded_length = match usize::try_from(read_u32([
                segment.payload[position],
                segment.payload[position + 1],
                segment.payload[position + 2],
                segment.payload[position + 3],
            ]))
```

- **Production evidence:** [`src/codestream.rs:L9225`](../../src/codestream.rs#L9225)

```text
position += 4;
```

- **Production evidence:** [`specs/mod.rs:L12916-L12917`](../../specs/mod.rs#L12916-L12917)

```text
let length = part1_siz_word(segment.payload, position as int) as nat;
        let start = position + 4;
```

- **Harness evidence:** [`src/codestream/tests/standard_plm.rs:L6-L8`](tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L6-L8): `let encoded = encode_packet_lengths(lengths);<br>    let mut group = vec![u8::try_from(encoded.len()).expect("Nplm <= 255")];<br>    group.extend(encoded);`
- **Harness evidence:** [`src/codestream/tests/standard_plm.rs:L36-L38`](tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L36-L38): `let mut cases = vec![<br>        vec![vec![]],<br>        vec![vec![1]],`
- **Harness evidence:** [`src/codestream/tests/standard_plm.rs:L66-L72`](tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L66-L72): `let bytes = standard_marker(0, &body);<br>        let mut parser = new_parser(&bytes, &limits).unwrap();<br>        let raw = parser.read_segment().unwrap();<br>        assert_eq!(raw.marker, PLM);<br>        assert_eq!(raw.offset, 0);<br>        let mut next = 0;<br>        let mut groups = Vec::new();`
- **Harness evidence:** [`src/codestream/tests/standard_plm.rs:L74-L79`](tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L74-L79): `if let Err(error) = parser.parse_plm(raw, &mut next, &mut groups) {<br>            failures.push(format!(<br>                "case {case}: valid A.35 octet framing rejected: {error:?}"<br>            ));<br>            continue;<br>        }`
- **Harness evidence:** [`src/codestream/tests/standard_plm.rs:L97`](tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L97): `assert!(failures.is_empty(), "{}", failures.join("\n"));`
- **Test source:** [`src/codestream/tests/standard_plm.rs:L6-L8`](tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L6-L8)<br>[`src/codestream/tests/standard_plm.rs:L36-L38`](tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L36-L38)<br>[`src/codestream/tests/standard_plm.rs:L66-L72`](tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L66-L72)<br>[`src/codestream/tests/standard_plm.rs:L74-L79`](tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L74-L79)<br>[`src/codestream/tests/standard_plm.rs:L97`](tests/DEV-C56978D32B/files/src/codestream/tests/standard_plm.rs#L97)
- **Test source:** [`validation/plm-standard-tests.patch`](tests/DEV-C56978D32B/files/validation/plm-standard-tests.patch)
- **Exact mapping record:** [report.json](report.json)

## DEV-4F2CFF1E64

- **Mapping:** `7d135fbf69889a164456fada` / `d040afbcef6d3fe9b50fc5cbccac8f707395f59e92d962e1383cb4d99f60e43b`
- **Specification:** `d0a062abc9fd5b2e44b3b868`
- **Deviation:** `c2df0cdeb60b3d6d297077c7`
- **Test item:** `65dbccafb1e648edaf1e39cd`
- **Case:** `5674a6a14bea7bfdf0ff42ae`
- **Exact standards quote:**

> This value is expressed with an 8-bit number limiting the number of Iplm bytes to 255 and the number of packets in
>           a tile-part to 255 or less.

- **Entry:** `src/codestream.rs:3633:codestream::parse`
- **Command:** `cargo test --lib --locked codestream::tests::metadata_contract::metadata_structural_admission_includes_standard_plm -- --exact --nocapture`
- **Setup:** The harness uses ResourceLimits::default() and byte constructors, not fabricated Parser internals. Existing siz/codestream_with_headers helpers emit SOC, ordinary Part-1 SIZ with one unsigned 8-bit 64x32 component/tile, COD with nonzero LRCP layers and zero decomposition levels, QCD, a main-header PLM, one SOT with correct Psot/Isot/TPsot/TNsot, SOD, one padded empty packet byte per layer, and EOC. The harness independently checks critical header fields and Psot. No PPM/PPT/SOP/EPH changes packet-length semantics. standard_plm encodes Zplm=0, one 8-bit Nplmi equal to the number of packet-length bytes, and one terminating Iplm=1 per actual packet. Maximum 255 layers/packets and 597-byte input fit default limits. For the first failing case, one packet, Nplmi=1 and an 89-byte stream establish a legal, small direct parse entry.
- **Route:** The replayed exact-name libtest metadata_structural_admission_includes_standard_plm loops through layers_to_check(), invoking accepted(&mut failures, layers, true, false). accepted builds fixture(layers,true,false) and calls parse(&bytes,&limits). parse delegates to parse_layout; main-header PLM dispatch calls parse_plm. The first valid PLM payload [0,1,1] at byte 65 hits the minimum-five-byte guard. accepted collects the unexpected Err; the final assert!(failures.is_empty()) at harness line 243 reports the quoted failure.
- **Observed assertion:** `legal T.800 PLM=true, PLT=false, layers=1: expected metadata, got MalformedInput { offset: 65, kind: InvalidMarkerLength }`

**Full diagnosis:**

The first reported failing check is the accumulated acceptance check for layers=1, PLM=true, PLT=false: parse returns MalformedInput { offset: 65, kind: InvalidMarkerLength } rather than metadata. This is an admissible direct unit-contract entry, not a bypass of an owning caller. Entry realism was checked before tracing: the failing acceptance sweep uses 1..255 layers, one component, one 64x32 tile (2048 pixels), one tile-part, at most 255 one-byte packets, and codestreams of 89..597 bytes; it has no recursive nesting or large fixture arrays. Defaults allow 256 layers, 64 components, 16384 tiles/tile-parts, 67108864 packets, 100000000 pixels and 2147483648 input bytes. These are ordinary bounded codestream inputs, not an implicit-capacity stress case. The real jp2::parse_and_inspect caller similarly passes a codestream box's byte slice and caller limits to this public function.

The origin is rfc_map, not infer. I read its parent finding and manifest, then independently retrieved A.7.2 from the pinned T.800 (06/2019), SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e, using originating item 7d135fbf69889a164456fada. Table A.35 explicitly specifies Nplmi size 8 bits, values 0..255; A.7.2 permits PLM independently of PLT and defines inline packet lengths to include headers. Thus the fixture's Zplm=0, one-byte Nplmi=1, Iplm=1 is legal for its single one-byte empty packet. SUPPORTED.md expressly claims PLM support. Existing native four-byte PLM helpers and correlated Verus annotations do not override this edition's syntax. No claim is made about an unacquired later edition.

Compiler declaration and MIR CFG confirm parse calls parse_layout and propagates its Err through the residual branch. Source traces parse_layout -> new_parser -> Parser::parse_layout, consumes SOC/SIZ/COD/QCD, then dispatches the PLM at offset 65 to parse_plm. For layers=1 the payload is [0,1,1], length 3, so parse_plm's payload.len()<5 guard immediately returns InvalidMarkerLength. For layers>=3 it instead reads four bytes starting at Nplmi as a u32 and advances four bytes, also incompatible with the standard. The wrapper's error propagation works; the underlying admission error is erroneous, denying metadata for valid supported PLM. This directly violates the clause's supported-PLM admission guarantee, so neither the faithful oracle nor the clause should be revised to match the four-byte dialect. The host-observed assertion and replayed harness, not a nonzero exit alone, establish this route. Combined PLM/PLT exclusion and full pixel decoding are not attributed by this case.

- **Production evidence:** [`src/codestream.rs:L3633-L3635`](../../src/codestream.rs#L3633-L3635)

```text
pub fn parse(input: &[u8], limits: &ResourceLimits) -> Result<CodestreamInfo, DecodeError> {
    Ok(parse_layout(input, limits)?.info)
}
```

- **Production evidence:** [`src/codestream.rs:L4278`](../../src/codestream.rs#L4278)

```text
parser.parse_plm(segment, &mut next_plm_index, &mut plm_groups)?;
```

- **Production evidence:** [`src/codestream.rs:L9183-L9185`](../../src/codestream.rs#L9183-L9185)

```text
if segment.payload.len() < 5 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
```

- **Production evidence:** [`src/codestream.rs:L9212-L9217`](../../src/codestream.rs#L9212-L9217)

```text
let encoded_length = match usize::try_from(read_u32([
                segment.payload[position],
                segment.payload[position + 1],
                segment.payload[position + 2],
                segment.payload[position + 3],
            ])) {
```

- **Production evidence:** [`SUPPORTED.md:L37`](../../SUPPORTED.md#L37)

```text
- PLM/PLT packet lengths, PPM/PPT packed headers, SOP/EPH, and TLM checks.
```

- **Production evidence:** [`src/codestream.rs:L4278`](../../src/codestream.rs#L4278)

```text
parser.parse_plm(segment, &mut next_plm_index, &mut plm_groups)?;
```

- **Production evidence:** [`src/codestream.rs:L9183-L9185`](../../src/codestream.rs#L9183-L9185)

```text
if segment.payload.len() < 5 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
```

- **Production evidence:** [`src/codestream.rs:L9212-L9217`](../../src/codestream.rs#L9212-L9217)

```text
let encoded_length = match usize::try_from(read_u32([
                segment.payload[position],
                segment.payload[position + 1],
                segment.payload[position + 2],
                segment.payload[position + 3],
            ])) {
```

- **Production evidence:** [`src/codestream.rs:L9225`](../../src/codestream.rs#L9225)

```text
position += 4;
```

- **Harness evidence:** [`src/codestream/metadata_contract_tests.rs:L20-L22`](tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L20-L22): `let mut payload = vec![0, layers];<br>        payload.extend(vec![1; usize::from(layers)]);<br>        let result = segment(PLM, &payload);`
- **Harness evidence:** [`src/codestream/metadata_contract_tests.rs:L46`](tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L46): `let bytes = codestream_with_headers(1, &main, &tile, &vec![0; usize::from(layers)]);`
- **Harness evidence:** [`src/codestream/metadata_contract_tests.rs:L121-L122`](tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L121-L122): `let limits = ResourceLimits::default();<br>        let bytes = fixture(layers, plm, plt);`
- **Harness evidence:** [`src/codestream/metadata_contract_tests.rs:L124`](tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L124): `match parse(&bytes, &limits) {`
- **Harness evidence:** [`src/codestream/metadata_contract_tests.rs:L240-L243`](tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L240-L243): `for layers in layers_to_check() {<br>            accepted(&mut failures, layers, true, false);<br>        }<br>        assert!(failures.is_empty(), "{}", failures.join("\n"));`
- **Test source:** [`src/codestream/metadata_contract_tests.rs:L20-L22`](tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L20-L22)<br>[`src/codestream/metadata_contract_tests.rs:L46`](tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L46)<br>[`src/codestream/metadata_contract_tests.rs:L121-L122`](tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L121-L122)<br>[`src/codestream/metadata_contract_tests.rs:L124`](tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L124)<br>[`src/codestream/metadata_contract_tests.rs:L240-L243`](tests/DEV-4F2CFF1E64/files/src/codestream/metadata_contract_tests.rs#L240-L243)
- **Exact mapping record:** [report.json](report.json)

## DEV-B6570A1D68

- **Mapping:** `7d135fbf69889a164456fada` / `e9191fbf9b54e8e24315ef56a0306713418ba54f83a102458b8863cd69ecb5c0`
- **Specification:** `3657ef26f30ed53d4b5e2551`
- **Deviation:** `ccd9c97cc7525a9cdd102fd7`
- **Test item:** `65dbccafb1e648edaf1e39cd`
- **Case:** `2a02569cbc667013b0d0abec`
- **Exact standards quote:**

> Both the PLM and PLT marker segments are optional
> and can be used together or separately.

- **Entry:** `src/codestream.rs:3633:codestream::parse`
- **Command:** `cargo test --lib --locked codestream::tests::metadata_contract::metadata_plm_plt_admission_and_error_propagation -- --exact --nocapture`
- **Setup:** Safe Rust byte vectors and ResourceLimits::default(); no private parser state fabricated. Existing baseline helpers construct SOC/SIZ/COD/QCD, exactly one SOT/SOD/EOC tile-part with patched Psot and valid 64x32 one-component geometry. The added standard_plm uses 8-bit Nplmi per pinned T.800 A.7.2. At layers=1 the payload [0,1,1] describes the same one-byte packet as PLT [0,1], and inline tile data [0] encodes an empty, byte-aligned packet under B.9/B.10.3. All fixture and Psot assertions completed. The failing stream is 95 bytes and all positive sweep cases are at most 862 bytes and 255 layers/packets, below default resource limits. Test-only inclusion changes no production behavior.
- **Route:** The exact host-replayed libtest metadata_plm_plt_admission_and_error_propagation iterates layers_to_check(), beginning at 1, and invokes accepted(&mut failures, layers, true, true). accepted constructs fixture and directly calls parse(&bytes, &limits). parse calls parse_layout, whose main-header PLM arm invokes parse_plm on the segment at byte 65. The length-3 payload fails parse_plm's minimum-5 check; its InvalidMarkerLength is propagated back and collected as the first acceptance mismatch, then asserted by failures.is_empty().
- **Observed assertion:** `legal T.800 PLM=true, PLT=true, layers=1: expected metadata, got MalformedInput { offset: 65, kind: InvalidMarkerLength }`

**Full diagnosis:**

The first failing check is the aggregated acceptance assertion at metadata_contract_tests.rs:175, whose first recorded mismatch is layers=1, PLM=true, PLT=true: expected metadata but received MalformedInput { offset: 65, kind: InvalidMarkerLength }. This is a valid direct unit-contract entry to parse, not a bypass of a required owning caller. Real callers jp2::parse_and_inspect and decode::decode_jpx_file likewise pass a raw codestream byte slice and resource limits directly; neither imposes an alternate PLM encoding. Realism was checked before tracing: the first failing stream is only 95 bytes, 64x32 (2048 pixels), one unsigned 8-bit component, one tile/part/resolution/precinct/layer/packet, with one inline empty packet byte. The complete sweep has at most 862 input bytes, 255 layers/packets, one tile-part and no nesting; defaults allow 2147483648 input bytes, 16777216 metadata bytes, 256 layers, 67108864 packets, 16384 tile-parts, and 100000000 pixels. These do not approach an implicit platform, stack, pool or configured limit. The separately tightened packet-limit negative case is intentional and is not the observed failure.

Authority is the rfc_map origin, not agreement with existing native fixtures or Verus models. I read the pinned T.800 (06/2019), SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e, A.7.2 through the originating mapping item. Table A.35 specifies Nplmi as 8 bits, and the text explicitly permits PLM and PLT together or separately. Its note limits the PLM description to 255 bytes/packets per tile-part; this test respects that limit. B.9 and B.10.3 independently justify the byte-aligned empty packets. The first PLM is FF57 0005 00 01 01: Zplm=0, Nplmi=1, Iplm=1, matching the tile-header PLT and the single inline packet. No conflicting declarations, unsupported profile, malformed segment length or packet truncation is constructed.

Route: test -> accepted(...,1,true,true) -> fixture -> parse -> parse_layout -> new_parser -> Parser::parse_layout main-header PLM arm -> parse_plm. At offset 65 the valid PLM payload is [0,1,1], length 3. parse_plm incorrectly requires at least five payload bytes, immediately returning InvalidMarkerLength. For larger legal groups its four-byte read of Nplmi likewise treats an 8-bit count and packet bytes as a u32. The main-header '?' and parse wrapper propagate that rejection; MIR confirms the error branch. Consequently standard, supported packet-length metadata prevents successful metadata inspection, violating the admission part of the exact clause. Error propagation itself works; it is not the deviation. The pre-existing plm test helper uses the same four-byte dialect and cannot override the cited standard. The separate PLM/PLT mutual-exclusion branch identified by the parent finding is not reached in this replay and is not claimed reproduced here. The harness's route print alone is not reachability proof; the specific observed rejection, exact fixture bytes, main-header dispatch and callee guard establish this assessed route.

- **Production evidence:** [`src/codestream.rs:L9183-L9185`](../../src/codestream.rs#L9183-L9185)

```text
if segment.payload.len() < 5 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
```

- **Production evidence:** [`src/codestream.rs:L9212-L9217`](../../src/codestream.rs#L9212-L9217)

```text
let encoded_length = match usize::try_from(read_u32([
                segment.payload[position],
                segment.payload[position + 1],
                segment.payload[position + 2],
                segment.payload[position + 3],
            ]))
```

- **Production evidence:** [`src/codestream.rs:L4276-L4279`](../../src/codestream.rs#L4276-L4279)

```text
PLM => {
                    plm_seen = true;
                    parser.parse_plm(segment, &mut next_plm_index, &mut plm_groups)?;
                }
```

- **Production evidence:** [`src/codestream.rs:L3633-L3635`](../../src/codestream.rs#L3633-L3635)

```text
pub fn parse(input: &[u8], limits: &ResourceLimits) -> Result<CodestreamInfo, DecodeError> {
    Ok(parse_layout(input, limits)?.info)
}
```

- **Production evidence:** [`SUPPORTED.md:L37`](../../SUPPORTED.md#L37)

```text
- PLM/PLT packet lengths, PPM/PPT packed headers, SOP/EPH, and TLM checks.
```

- **Production evidence:** [`src/codestream.rs:L10002-L10008`](../../src/codestream.rs#L10002-L10008)

```text
} else if segment.marker == PLT {
                        if main_packet_lengths.is_present() {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerOrder,
                            ));
                        }
```

- **Harness evidence:** [`src/codestream/metadata_contract_tests.rs:L20-L22`](tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L20-L22): `let mut payload = vec![0, layers];<br>        payload.extend(vec![1; usize::from(layers)]);<br>        let result = segment(PLM, &payload);`
- **Harness evidence:** [`src/codestream/metadata_contract_tests.rs:L46`](tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L46): `let bytes = codestream_with_headers(1, &main, &tile, &vec![0; usize::from(layers)]);`
- **Harness evidence:** [`src/codestream/metadata_contract_tests.rs:L121-L122`](tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L121-L122): `let limits = ResourceLimits::default();<br>        let bytes = fixture(layers, plm, plt);`
- **Harness evidence:** [`src/codestream/metadata_contract_tests.rs:L124`](tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L124): `match parse(&bytes, &limits) {`
- **Harness evidence:** [`src/codestream/metadata_contract_tests.rs:L173`](tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L173): `accepted(&mut failures, layers, true, true);`
- **Harness evidence:** [`src/codestream/metadata_contract_tests.rs:L175`](tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L175): `assert!(failures.is_empty(), "{}", failures.join("\n"));`
- **Test source:** [`src/codestream/metadata_contract_tests.rs:L20-L22`](tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L20-L22)<br>[`src/codestream/metadata_contract_tests.rs:L46`](tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L46)<br>[`src/codestream/metadata_contract_tests.rs:L121-L122`](tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L121-L122)<br>[`src/codestream/metadata_contract_tests.rs:L124`](tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L124)<br>[`src/codestream/metadata_contract_tests.rs:L173`](tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L173)<br>[`src/codestream/metadata_contract_tests.rs:L175`](tests/DEV-B6570A1D68/files/src/codestream/metadata_contract_tests.rs#L175)
- **Exact mapping record:** [report.json](report.json)

## DEV-80E8727274

- **Mapping:** `b74f5a18aa024a0fe1bc915d` / `ef252b9643ef28923ab0b6e44ec27b567156d254bedf64a6e9f868439d9395a5`
- **Specification:** `5d199dfcfdfdb4afc8f756e9`
- **Deviation:** `770981616b006ca434fb132a`
- **Test item:** `8afbde10624bca348eb275a1`
- **Case:** `3cf1a88fbaecd2883adbd142`
- **Exact standards quote:**

> Both the PLM and PLT marker segments
> are optional and can be used together or separately.

- **Entry:** `src/jpip.rs:3740:jpip::validate_precinct_codestream`
- **Command:** `target/precinct-contract-tests --exact jpip::tests::precinct_contract_structural_eligibility --nocapture --test-threads=1`
- **Setup:** A safe direct unit-contract entry with real parsed bytes, not a synthetic CodestreamLayout: 96-byte 1x1 single-component, single-tile, single-part stream, one requested and declared layer, default ResourceLimits, valid tile COD override, no progression-change or attached packet-header/length sources. A zero-byte packet header represents an empty packet; full entropy decoding is not required by this structural clause. The owning caller need not execute for this unit contract, and only the direct-validator false acceptance is claimed.
- **Route:** The replayed exact libtest jpip::tests::precinct_contract_structural_eligibility constructs the progression matrix. For main=0 and tile=1, precinct_contract_stream builds actual SOC/SIZ/main COD/QCD/SOT/tile COD/SOD/one zero packet/EOC bytes, correcting Psot. Independent parse_layout assertions establish LRCP main order and RLCP effective tile order. The check closure reparses successfully and directly calls validate_precinct_codestream(bytes, 1, &default_limits); its unexpected Ok is accumulated and reported by the final failed assertion. The same route fails for tile=2,3,4.
- **Observed assertion:** `main progression=0, effective tile progression=1: expected Err(MalformedInput { offset: 0, kind: InvalidJpipLayout }), got Ok(())`

**Full diagnosis:**

The first failed eligibility comparison is main progression=0, effective tile progression=1: the host observed Ok(()) instead of InvalidJpipLayout at offset 0. The final assertion aggregates that comparison and the analogous tile orders 2, 3 and 4; fixture parsing and parsed-order assertions completed successfully before that assertion. This violates exactly the clause's effective-tile LRCP requirement. The clause is sound: SUPPORTED.md restricts explicit-plan reconstructed input to one tile-part with LRCP progression, not merely an LRCP main-header default. The parser intentionally accepts a tile COD override and exposes it as the tile part's effective coding_style.

Entry realism was checked: each failing fixture is only 96 bytes, a 1x1 image, one component, one tile, one tile part, one layer, zero decomposition levels, and a one-byte empty-packet payload, with ordinary owned Vec storage and ResourceLimits::default(). This is well below hardened limits (2,147,483,648 input bytes, 100,000,000 pixels, 64 components, 16,384 tiles/parts, 256 layers and 33 resolutions); no excessive stack, pool or nesting demand exists. The existing real-caller regression uses the same small codestream constructor. Separate successful layer-sweep cases use a test-only expanded layer limit up to 65,535 and about 65.6 KB of bytes; those are not the failing cases, and the progression matrix explicitly uses the unmodified default limits.

This is an admissible direct unit-contract test, not a bypassed caller-owned state fixture: the entry takes immutable bytes/layers/limits, has no additional preconditions, and itself parses the actual bytes. The harness independently parses those same bytes and verifies the main and effective tile progression orders, then directly invokes the original private validator through a cfg(test) include. The replay builder only registers test code and a test allocator dependency; it does not change production behavior. Real caller context is consistent: transcode_precinct_codestream assembles headers/packets/EOC and calls this function as its final gate before returning Some(codestream). The negative caller path itself was not replayed and is not claimed.

Compiler declaration and CFG confirm the route: parse_layout succeeds (bb0-bb4), tile_count is one (bb8), tile_parts.len() is one (bb11), requested/main layers match (bb12), main progression is LRCP (bb13), and no main/tile POC, attached lengths or packed headers are present (bb14-bb24). Execution reaches Ok(()) in bb27 without examining tile_parts[0].coding_style.progression_order. Source read_tile_part assigns the tile COD into tile_coding_style and returns it in TilePartPayload, so the unchecked override is real parsed state. Consequence established by replay is false structural eligibility for a documented non-LRCP input; no entropy-decoding failure or packet corruption is asserted.

- **Production evidence:** [`SUPPORTED.md:L138-L140`](../../SUPPORTED.md#L138-L140)

```text
Explicit `JpipPrecinctCodestreamPlan` transcodes are restricted to a single
tile-part with LRCP progression, inline packet headers, and no POC, PLT, or PPT
in the reconstructed input.
```

- **Production evidence:** [`src/jpip.rs:L3746-L3760`](../../src/jpip.rs#L3746-L3760)

```text
if layout.info.image_size.tile_count()? != 1
        || layout.tile_parts.len() != 1
        || layout.info.coding_style.layers != layers
        || !matches!(
            layout.info.coding_style.progression_order,
            crate::codestream::ProgressionOrder::Lrcp
        )
        || layout.info.progression_changes.is_present()
        || layout.tile_parts[0].progression_changes.is_present()
        || layout.tile_parts[0].packet_lengths.is_present()
        || layout.tile_parts[0].packed_packet_headers.is_present()
    {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipLayout));
    }
    Ok(())
```

- **Production evidence:** [`src/codestream.rs:L9910-L9911`](../../src/codestream.rs#L9910-L9911)

```text
tile_cod_seen = true;
                        tile_coding_style = self.parse_cod(segment)?;
```

- **Production evidence:** [`src/codestream.rs:L10223`](../../src/codestream.rs#L10223)

```text
coding_style: tile_coding_style,
```

- **Production evidence:** [`src/jpip.rs:L3158-L3163`](../../src/jpip.rs#L3158-L3163)

```text
validate_precinct_codestream(&bytes, layers, &self.limits)?;
        Ok(JpipPrecinctTranscode {
            layers,
            availability,
            codestream: Some(JpipByteView { bytes }),
        })
```

- **Production evidence:** [`src/jpip.rs:L3755-L3756`](../../src/jpip.rs#L3755-L3756)

```text
        || layout.tile_parts[0].packet_lengths.is_present()
        || layout.tile_parts[0].packed_packet_headers.is_present()
```

- **Harness evidence:** [`src/jpip_precinct_validation_tests.rs:L59`](tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L59): `let limits = ResourceLimits::default();`
- **Harness evidence:** [`src/jpip_precinct_validation_tests.rs:L113-L114`](tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L113-L114): `let override_cod = segment(0xff52, &[0, tile, 0, 1, 0, 0, 0, 0, 0, 1]);<br>            let bytes = precinct_contract_stream(1, main, &[], &[override_cod], &[0]);`
- **Harness evidence:** [`src/jpip_precinct_validation_tests.rs:L120-L123`](tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L120-L123): `assert_eq!(<br>                layout.tile_parts[0].coding_style.progression_order,<br>                orders[usize::from(tile)]<br>            );`
- **Harness evidence:** [`src/jpip_precinct_validation_tests.rs:L124-L130`](tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L124-L130): `check(<br>                &format!("main progression={main}, effective tile progression={tile}"),<br>                &bytes,<br>                1,<br>                &limits,<br>                main == 0 && tile == 0,<br>            );`
- **Harness evidence:** [`src/jpip_precinct_validation_tests.rs:L71`](tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L71): `let actual = validate_precinct_codestream(bytes, layers, limits);`
- **Harness evidence:** [`src/jpip_precinct_validation_tests.rs:L31-L33`](tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L31-L33): `let length = u32::try_from(bytes.len() - start).unwrap();<br>    bytes[start + 6..start + 10].copy_from_slice(&length.to_be_bytes());<br>    bytes.extend_from_slice(&EOC);`
- **Test source:** [`src/jpip_precinct_validation_tests.rs:L59`](tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L59)<br>[`src/jpip_precinct_validation_tests.rs:L113-L114`](tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L113-L114)<br>[`src/jpip_precinct_validation_tests.rs:L120-L123`](tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L120-L123)<br>[`src/jpip_precinct_validation_tests.rs:L124-L130`](tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L124-L130)<br>[`src/jpip_precinct_validation_tests.rs:L71`](tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L71)<br>[`src/jpip_precinct_validation_tests.rs:L31-L33`](tests/DEV-80E8727274/files/src/jpip_precinct_validation_tests.rs#L31-L33)
- **Test source:** [`validation/precinct-contract-alloc/Cargo.toml`](tests/DEV-80E8727274/files/validation/precinct-contract-alloc/Cargo.toml)
- **Test source:** [`validation/precinct-contract-alloc/lib.rs`](tests/DEV-80E8727274/files/validation/precinct-contract-alloc/lib.rs)
- **Test source:** [`validation/build-precinct-contract-tests.py`](tests/DEV-80E8727274/files/validation/build-precinct-contract-tests.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-C36C516466

- **Mapping:** `77c6eb0e23d1ea342a7cb7c8` / `dfce9e2ac2ffc71055fbbb793f3d2921196197449b435f46443751c47c0eb953`
- **Specification:** `d6220bf1a69f68a0f1f19555`
- **Deviation:** `d376ea8284e282622433a04e`
- **Test item:** `9a82786057472396f5b9ef42`
- **Case:** `db69a93bf86d084071e11986`
- **Exact standards quote:**

> Both the PLM and PLT marker segments
> are optional and can be used together or separately.

- **Entry:** `src/codestream.rs:9830:codestream::Parser::read_tile_part`
- **Command:** `python3 validation/read-tile-part-contract.py run plm_plt`
- **Setup:** One component/tile at 1x1, reversible Part1 style, zero decompositions, compatible one-step QCD and initialized component tables; first tile-part with exact Psot and trailing EOC. Counts are 1,2,127,128,255, within default 256-layer admission and the cited edition's 8-bit Nplm bound. PLM has Zplm=0 and Nplm=count followed by count complete positive one-byte length codewords; the borrowed selected descriptor has exact count/size. PLT has Zplt=0 and identical codewords. Payload consists of count zero bytes, each an empty inline packet, so declared lengths sum to payload length. No packed headers, transforms, extension state, malformed private parser fields, oversized allocations or injected downstream failure. This is valid direct-unit evidence, explicitly not a claim that the separately discrepant native parse_plm accepts standard 8-bit framing.
- **Route:** Host ran python3 validation/read-tile-part-contract.py run plm_plt, which executes the exact native codestream::tests::read_tile_part_contract_plm_plt case. For each count and source mask it builds a framed stream and invokes tile_contract_unit. That helper uses real prefix/SOT readers, supplies the validated selected PLM group at the direct read_tile_part unit boundary, and encounters the first tile-header PLT. With sources=3, read_tile_part's PLT branch tests main_packet_lengths.is_present() and returns InvalidMarkerOrder at the PLT offset before PLT decoding. The test accumulates that Err and its final assertion fails. For count=1 the offset is84, matching the host output and constructed stream.
- **Observed assertion:** `consistent PLM/PLT must work separately AND together: ["count=1 sources=3: MalformedInput { offset: 84, kind: InvalidMarkerOrder }", "count=2 sources=3: MalformedInput { offset: 85, kind: InvalidMarkerOrder }", "count=127 sources=3: MalformedInput { offset: 210, kind: InvalidMarkerOrder }", "count=128 sources=3: MalformedInput { offset: 211, kind: InvalidMarkerOrder }", "count=255 sources=3: MalformedInput { offset: 338, kind: InvalidMarkerOrder }"]`

**Full diagnosis:**

The first failing assertion is the final failures.is_empty() check at src/codestream/read_tile_part_contract.rs:350, reporting 'consistent PLM/PLT must work separately AND together'. The first collected mismatch is count=1 sources=3: MalformedInput { offset: 84, kind: InvalidMarkerOrder }; all five combined-source cases fail, whereas the same replay reports no failures for either source separately. This is a specific rejection returned by the subject, not a setup panic or merely exit 101.

Realism checked first: each stream has one 1x1, 8-bit, unit-sampled component, one tile/first tile-part, zero decompositions, 1/2/127/128/255 layers and one one-byte empty inline packet per layer. At most 255 packet-length bytes per source and 255 payload bytes are requested. The largest combined stream is 857 bytes (92+3*255); its PLM payload is 257 bytes and its PLT payload 256, far below marker-length and platform integer limits. Default limits are HARDENED: 256 layers, 64 components, 67,108,864 packets, 268,435,456 packet-header bytes, 2,147,483,648 input bytes and 1,000,000,000 work units. No nesting, giant stack allocation or exhaustion is induced. Real parse_layout selects a PLM group for the current tile-part and passes it alongside inherited tables to this exact helper; the unit uses those same argument shapes and smaller ordinary values.

Authority is the rfc_map origin, not an inferred implementation-only clause. I read its parent finding and manifest, then retrieved pinned T.800 (06/2019), SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e, A.7.2 and A.7.3 using the originating mapping item. Both explicitly say PLM and PLT are optional and can be used together or separately. A.7.2 specifies 8-bit Nplm with at most 255 encoded bytes, which these fixtures obey; A.7.3 permits PLT in this tile-part header before its described packets. Both descriptions encode identical positive one-byte lengths and totals, with no PPM/PPT/SOP/EPH ambiguity. SUPPORTED.md advertises PLM/PLT support. Thus the coexistence oracle faithfully exercises this clause's originating standard finding, rather than confusing PLM/PLT with the separate PPM/PPT exclusion. Existing implementation-local contracts that describe this rejection do not override the standards-derived requirement. This finding is scoped to the pinned 2019 edition; the origin notes that newer advertised reference editions remain unreconciled, and this is not a newer-edition or whole-decoder conformance verdict.

The admissible route is a direct unit_contract entry (work-item unit assessment available, requires=[]), not a claim that the public parser accepts this standard-framed PLM. The replay's test-only include exposes the native test module without changing production behavior. tile_contract_unit uses new_parser, consumes SOC/SIZ/COD/QCD and SOT with real operations, constructs consistent inherited style/quantization tables, and supplies empty ROI/progression/MCT/Part2/packed-header state. It reads a genuine standard PLM segment, checks Zplm=0, Nplm=encoded.len(), and every encoded byte=1, then constructs PacketLengths::Plm borrowing just the actual Iplm bytes with exact packet_count and encoded_bytes. This is the selected-group value consumed by read_tile_part and its iterator, not an impossible descriptor or arbitrary parser-field mutation. The helper has no requirement that this value be produced by parse_plm, nor any minimum PLM-related resource-counter precondition. Native parse_plm's separate four-byte Nplm convention is deliberately bypassed at this unit boundary; consequently this replay does not establish an end-to-end standard-PLM caller route or diagnose that separate framing behavior.

describe_declaration confirms the descriptor is the PacketLengths argument _11. control_flow_graph shows the precise failure: PLT comparison bb132 -> is_present(_11) bb133 -> true bb134 -> segment offset bb135 -> InvalidMarkerOrder bb136 -> Err bb137, then cleanup. Source read_tile_part rejects immediately whenever main_packet_lengths.is_present(), before checking or decoding the PLT payload. For count=1, the selected PLM segment ends at byte72, SOT advances to84, and the valid PLT at84 triggers this unconditional exclusion. Consistent, legally coexisting packet-length metadata is therefore rejected and no tile payload is returned. The clause is sound on this admissible unit entry; the production exclusion is the deviation. No production files were changed.

- **Production evidence:** [`src/codestream.rs:L10002-L10008`](../../src/codestream.rs#L10002-L10008)

```text
                    } else if segment.marker == PLT {
                        if main_packet_lengths.is_present() {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerOrder,
                            ));
                        }
```

- **Production evidence:** [`src/codestream.rs:L9241-L9245`](../../src/codestream.rs#L9241-L9245)

```text
            groups.push(PacketLengths {
                source: PacketLengthSource::Plm(encoded),
                packet_count,
                encoded_bytes: encoded.len() as u64,
            });
```

- **Production evidence:** [`src/codestream.rs:L4567`](../../src/codestream.rs#L4567)

```text
                    let main_packet_lengths = if plm_seen {
```

- **Production evidence:** [`SUPPORTED.md:L37`](../../SUPPORTED.md#L37)

```text
- PLM/PLT packet lengths, PPM/PPT packed headers, SOP/EPH, and TLM checks.
```

- **Production evidence:** [`src/codestream.rs:L10002-L10008`](../../src/codestream.rs#L10002-L10008)

```text
                    } else if segment.marker == PLT {
                        if main_packet_lengths.is_present() {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerOrder,
                            ));
                        }
```

- **Production evidence:** [`src/codestream.rs:L4567`](../../src/codestream.rs#L4567)

```text
                    let main_packet_lengths = if plm_seen {
```

- **Harness evidence:** [`src/codestream/read_tile_part_contract.rs:L52-L60`](tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs#L52-L60): `        assert_eq!(plm.payload[0], 0);<br>        assert_eq!(usize::from(plm.payload[1]), plm.payload.len() - 2);<br>        let encoded = &plm.payload[2..];<br>        assert!(encoded.iter().all(\|&b\| b == 1));<br>        main_lengths = PacketLengths {<br>            source: PacketLengthSource::Plm(encoded),<br>            packet_count: encoded.len() as u64,<br>            encoded_bytes: encoded.len() as u64,<br>        };`
- **Harness evidence:** [`src/codestream/read_tile_part_contract.rs:L62-L79`](tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs#L62-L79): `    let sot = parser.read_sot(1).unwrap();<br>    let result = parser.read_tile_part(<br>        sot,<br>        (style, quant),<br>        ComponentCodingStyles::new(size.components, style).unwrap(),<br>        ComponentQuantizations::new(size.components, quant).unwrap(),<br>        RoiShifts::none(),<br>        ProgressionChanges::none(),<br>        MctParameters {<br>            arrays: vec![],<br>            records: vec![],<br>            order: vec![],<br>        },<br>        Part2Extensions::default(),<br>        size.components,<br>        main_lengths,<br>        PackedPacketHeaders::none(),<br>    );`
- **Harness evidence:** [`src/codestream/read_tile_part_contract.rs:L324-L339`](tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs#L324-L339): `    for count in [1_u16, 2, 127, 128, 255] {<br>        for sources in 1..=3 {<br>            let lengths = vec![1; usize::from(count)];<br>            let mut main = vec![tile_contract_cod(count, 0, 2, 0), segment(QCD, &[0, 0])];<br>            if sources & 1 != 0 {<br>                let mut payload = vec![0, count as u8]; // Zplm, 8-bit Nplm<br>                payload.extend(vec![1; usize::from(count)]);<br>                main.push(segment(PLM, &payload));<br>            }<br>            let tile = if sources & 2 != 0 {<br>                vec![plt(0, &lengths)]<br>            } else {<br>                vec![]<br>            };<br>            let bytes =<br>                tile_contract_stream(1, 0, &main, &tile, &vec![0; usize::from(count)], false);`
- **Harness evidence:** [`src/codestream/read_tile_part_contract.rs:L350-L353`](tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs#L350-L353): `    assert!(<br>        failures.is_empty(),<br>        "consistent PLM/PLT must work separately AND together: {failures:?}"<br>    );`
- **Test source:** [`src/codestream/read_tile_part_contract.rs:L52-L60`](tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs#L52-L60)<br>[`src/codestream/read_tile_part_contract.rs:L62-L79`](tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs#L62-L79)<br>[`src/codestream/read_tile_part_contract.rs:L324-L339`](tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs#L324-L339)<br>[`src/codestream/read_tile_part_contract.rs:L350-L353`](tests/DEV-C36C516466/files/src/codestream/read_tile_part_contract.rs#L350-L353)
- **Test source:** [`validation/read-tile-part-contract.py`](tests/DEV-C36C516466/files/validation/read-tile-part-contract.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-709CD8127B

- **Mapping:** `16964ff92143d3388c78c5f3` / `4ce32a6a48db60a94d16d03e39d2b4d3b01e994f3b74f5cdb2cc0269790a847c`
- **Specification:** `9581f8bf93e6bf0e73ab4d0f`
- **Deviation:** `4fb578e5efe0b16eaaae3a78`
- **Test item:** `f71c07d698d8441f7f243d58`
- **Case:** `44c55ec8f76ec6b24e477888`
- **Exact standards quote:**

> If SOP marker segments are allowed (by signalling in the COD marker segment, see A.6.1), each packet in any given tile-
> part may or may not be appended with an SOP marker segment.

- **Entry:** `src/packet.rs:2965:packet::decode_progression_precinct_packet`
- **Command:** `target/deepspec-progression-suite --exact packet::tests::progression_contract_sop_policy_and_optional_marker --nocapture`
- **Setup:** A tiny valid first-packet unit entry: a parsed single-tile Part-1 stream, legal COD Scod=2, one empty header byte 0x00, no EPH or packed source, all initial cursors/counters unchanged at zero, coherent one-block literal precinct, valid component/style and constructor-derived grid/input/progress, default resource limits. A.8.1 in the assigned 2019 edition permits SOP omission on this packet. Absence of SOP is the legal input being exercised, not a violated entry requirement. No body decoding or unsafe state fabrication is involved.
- **Route:** Host build adds only an include in packet::tests, compiles the native library tests and verifies the exact test registration. Host executes packet::tests::progression_contract_sop_policy_and_optional_marker. The first mismatching iteration is packed=false, flags=2: contract_codestream builds COD-enabled SOP with data=[0] and no SOP; contract_layout successfully parses it; contract_progression_call prepares a production grid/input/progress and calls decode_progression_precinct_packet at coordinate (0,0,0,0), offset=packed_offset=packets=0, expected=None, selection=None. Target delegates to decode_precinct_packet, then payload decoding and its prefix helper, whose SOP branch rejects the one-byte tile suffix before parsing the empty-packet header. The aggregate assertion at line 522 reports that specific mismatch.
- **Observed assertion:** `T.800 A.8.1 permits absent SOP with Scod bit 1 set: packed=false, flags=2, actual=Err(MalformedInput { offset: 0, kind: Truncated }), expected=Ok((PacketBodyDecode { present: false, code_blocks: 1, contributions: 0, segments: 0, passes: 0, header_bytes: 1, body_bytes: 0, bytes_consumed: 1 }, 1, 0))`

**Full diagnosis:**

The first failing assertion is the final omissions.is_empty() check at src/packet_progression_contract_tests.rs:522. Its first collected mismatch is packed=false, flags=2: a complete one-byte empty-packet header produces Truncated instead of Ok with one tile byte consumed. This is a code deviation under the clause's rfc_map authority, not a bad oracle inferred from native/model agreement. I read clause_origin and independently retrieved its original T.800 (06/2019), ISO/IEC 15444-1:2019 A.8.1 passage, version 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e, through the originating rfc_map item. It explicitly says that when COD allows SOP, each packet may or may not have an SOP segment; Nsop still counts every packet. It also explicitly permits SOP placement before tile-part packet bodies with PPT/PPM. Thus the intended SOP policy is permission, not mandatory presence. The clause's generic model-routing wording is interpreted with that parent finding; the existing model/callee summaries that charge six SOP bytes whenever enabled share the implementation restriction and are not caller preconditions overriding the standard. SUPPORTED.md advertises SOP/EPH support without documenting this mandatory-per-packet restriction.

Entry realism was checked: the failing case has one 1x1 image/component/tile, one resolution/layer/precinct/code-block, a one-byte empty header, zero body bytes, and counters/cursors all zero. Serialized omission streams are below 100 bytes, with no recursive/deep structure, versus default/HARDENED capacities of 2 GiB input, 268435456 header bytes, 16777216 blocks, 67108864 packets and 1000000000 work units. No excessive allocation or stack demand is involved. The fixture serializes main-header COD and QCD, a correctly sized SOT/SOD tile-part and EOC, parses it using parse_layout, and prepares packed payloads, grid and progress using production constructors. The reused literal precinct has one state/context/1x1 decoder and one nonempty subband, matching component/resolution/precinct zero. The first failing case is inline, so packed-placement edge cases cannot account for it. Although its helper allows direct counter overrides for other tests, this failure writes only the zero values already produced by progression_tile_progress_new. It is an admissible direct unit-contract entry, not a forged caller history: execute_progression_packet also passes the first unseen coordinate, initial cursors and expected=None for absent PLM/PLT. No prerequisite requires six marker bytes merely because SOP is allowed.

Compiler declaration and MIR CFG establish the target route: set layer zero, fetch packed[0], take selection=None (decode=true), derive expected sequence zero, and tail-call decode_precinct_packet. Its shape/resource/offset/style checks succeed and it passes sop=true from the parsed COD flag into decode_precinct_packet_payload. That calls decode_precinct_packet_payload_prefix before header decoding. The prefix unconditionally attempts data.get(0..2) under parameters.sop; the legal inline data=[0] therefore returns Truncated at offset zero before reading its false presence bit. The replay's specific error, not merely exit 101, matches this branch. Legal SOP omission with EPH and/or packed headers is likewise rejected in the same replay. The consequence is rejection of legal SOP-enabled Part-1 packet input. Truncated is a misleading diagnostic for this complete unmarked packet, not separate evidence of malformed setup. This attribution is scoped to the replayed native unit route and cited edition; it is not a formal whole-decoder reachability/conformance proof.

- **Production evidence:** [`src/packet.rs:L3989-L3993`](../../src/packet.rs#L3989-L3993)

```text
    if parameters.sop {
        let marker = match data.get(0..2) {
            Some(marker) => marker,
            None => return Err(packet_reader_error(offset, MalformedKind::Truncated)),
        };
```

- **Production evidence:** [`src/packet.rs:L3963-L3965`](../../src/packet.rs#L3963-L3965)

```text
            sop: tile_part.coding_style.sop_markers,
            eph: tile_part.coding_style.eph_markers,
            packed: tile_part.packed_packet_headers.is_present(),
```

- **Production evidence:** [`src/packet.rs:L4057`](../../src/packet.rs#L4057)

```text
    let prefix = decode_precinct_packet_payload_prefix(data, packed_headers, parameters, limits)?;
```

- **Production evidence:** [`src/codestream.rs:L8160`](../../src/codestream.rs#L8160)

```text
            sop_markers: flags & 2 != 0,
```

- **Production evidence:** [`SUPPORTED.md:L37`](../../SUPPORTED.md#L37)

```text
- PLM/PLT packet lengths, PPM/PPT packed headers, SOP/EPH, and TLM checks.
```

- **Production evidence:** [`src/codestream.rs:L8160`](../../src/codestream.rs#L8160)

```text
sop_markers: flags & 2 != 0,
```

- **Production evidence:** [`src/packet.rs:L3963`](../../src/packet.rs#L3963)

```text
sop: tile_part.coding_style.sop_markers,
```

- **Production evidence:** [`src/packet.rs:L3989-L3993`](../../src/packet.rs#L3989-L3993)

```text
if parameters.sop {
        let marker = match data.get(0..2) {
            Some(marker) => marker,
            None => return Err(packet_reader_error(offset, MalformedKind::Truncated)),
        };
```

- **Production evidence:** [`src/packet.rs:L3998-L4000`](../../src/packet.rs#L3998-L4000)

```text
if !crate::refinement::sop_valid(marker[0], marker[1], marker_length[0], marker_length[1]) {
            return Err(invalid_packet_header());
        }
```

- **Harness evidence:** [`src/packet_progression_contract_tests.rs:L421-L430`](tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L421-L430): `    for packed in [false, true] {<br>        for flags in [0_u8, 2, 4, 6] {<br>            let eph = flags & 4 != 0;<br>            let mut header = vec![0];<br>            if eph {<br>                header.extend_from_slice(&[0xff, 0x92]);<br>            }<br>            let tile: &[u8] = if packed { &[] } else { &header };<br>            let stream = contract_codestream(flags, 1, tile, packed.then_some(header.as_slice()));<br>            let layout = contract_layout(&stream, flags, 1, packed.then_some(header.as_slice()));`
- **Harness evidence:** [`src/packet_progression_contract_tests.rs:L433-L442`](tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L433-L442): `            let actual = contract_progression_call(<br>                &layout.tile_parts,<br>                None,<br>                contract_coordinate(0),<br>                0,<br>                0,<br>                0,<br>                None,<br>                &mut precinct,<br>            );`
- **Harness evidence:** [`src/packet_progression_contract_tests.rs:L97`](tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L97): `    let input = prepare_progression_payloads(parts, parts[0].tile_index, &limits).unwrap();`
- **Harness evidence:** [`src/packet_progression_contract_tests.rs:L117`](tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L117): `    let mut progress = progression_tile_progress_new(&params).unwrap();`
- **Harness evidence:** [`src/packet_progression_contract_tests.rs:L127-L129`](tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L127-L129): `    decode_progression_precinct_packet(<br>        &params, &parts[0], &progress, coordinate, expected, precinct,<br>    )`
- **Harness evidence:** [`src/packet_progression_contract_tests.rs:L522`](tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L522): `    assert!(omissions.is_empty(), "{}", omissions.join("\n"));`
- **Test source:** [`src/packet_progression_contract_tests.rs:L421-L430`](tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L421-L430)<br>[`src/packet_progression_contract_tests.rs:L433-L442`](tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L433-L442)<br>[`src/packet_progression_contract_tests.rs:L97`](tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L97)<br>[`src/packet_progression_contract_tests.rs:L117`](tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L117)<br>[`src/packet_progression_contract_tests.rs:L127-L129`](tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L127-L129)<br>[`src/packet_progression_contract_tests.rs:L522`](tests/DEV-709CD8127B/files/src/packet_progression_contract_tests.rs#L522)
- **Test source:** [`validation/deepspec-progression-registration.patch`](tests/DEV-709CD8127B/files/validation/deepspec-progression-registration.patch)
- **Test source:** [`validation/build-progression-contract.py`](tests/DEV-709CD8127B/files/validation/build-progression-contract.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-EA51ACDA28

- **Mapping:** `898c9a095355b7c764a77b6d` / `4b4151e6eb9a3c51d3ef3e46e2c15afc002f3bec3e04fc4e19a0556c5bd69dd0`
- **Specification:** `62986c33c311dfc825987719`
- **Deviation:** `b9e9833ed3767a08c6ef0862`
- **Test item:** `f68e289bc92b358b513e6f20`
- **Case:** `23cf463d2324619732b2286f`
- **Exact standards quote:**

> Usage: Shall be used if and only if indicated in the proper COD marker segment (see A.6.1). It appears immediately after
> a packet header.

- **Entry:** `src/packet.rs:972:packet::decode_jpip_precinct_packet_inner`
- **Command:** `target/deepspec_jpip_contract/runner --exact packet::tests::jpip_contract_eph_policy_errors_and_prefix --nocapture`
- **Setup:** A bounded safe-Rust inspection-unit fixture: one fresh packet state, one context and decoder, one valid 1x1 LL subband with fresh inclusion/zero-bitplane trees. The helper sets context maximum_zero_bitplanes=8 and constructs a matching eight-bitplane regular decoder using the real constructor. Header bits encode first inclusion, ZBP=6<8, one pass, Lblock=8, length=255. Mandatory terminal stuffing is 00, followed by enabled EPH FF92 and all 255 declared opaque body bytes. Layer zero is valid, component zero has a valid style, packed headers and SOP are absent, all default limits are respected. Inspection does not execute Tier-1, so entropy validity of the body is not a premise.
- **Route:** Host runner --exact packet::tests::jpip_contract_eph_policy_errors_and_prefix --nocapture reaches its final terminal-stuffing case after the ordinary EPH sweeps. jpip_terminal_ff_packet(true) builds C0 BE FF 00 FF92 plus 255 body bytes; jpip_assert_inspect directly calls decode_jpip_precinct_packet_inner with SOP=false, EPH=true, sequence=0, layer=0. The adapter passes the inline stream to decode_precinct_packet with decode_tier1=false; after contribution headers the reader reports only three bytes, so EPH validation examines 00 FF and returns the observed error.
- **Observed assertion:** `assertion `left == right` failed: inline inspection result/error/prefix<br>  left: Err(MalformedInput { offset: 0, kind: InvalidPacketHeader })<br> right: Ok(261)`

**Full diagnosis:**

The first failing assertion is src/packet_jpip_contract_tests.rs:50, reached by the terminal-FF case at lines 205-208: the host observed Err(MalformedInput { offset: 0, kind: InvalidPacketHeader }) instead of Ok(261). This violates clause 62986c33c311dfc825987719's EPH validation/consumed-prefix guarantee on an admissible direct inspection-unit entry, not a build failure or Tier-1 decode failure. Capacity check: this case supplies 261 bytes (four header bytes, two EPH bytes, 255 body bytes), one code block, one subband, two one-node trees, one new pass, layer 0 of two, and eight maximum bitplanes. These are far below default limits (2,147,483,648 input bytes, 268,435,456 header bytes, 67,580 code-block bytes, 16,777,216 code blocks/tag-tree nodes, 874 passes, 256 layers, and 1,000,000,000 work units); there is no deep recursion or oversized fixture. The fresh state, single-leaf trees, matching vector lengths, coordinates and eight-bitplane decoder are consistent with the normal PacketPrecinctDecoder::new/new_subbands construction for one block. It is an inspection contract: opaque declared body bytes need not establish entropy-decoder success because the adapter explicitly passes decode_tier1=false. No owning-caller preparation is skipped that this direct unit contract requires.
Route: jpip_contract_eph_policy_errors_and_prefix -> jpip_terminal_ff_packet(true) -> jpip_assert_inspect -> decode_jpip_precinct_packet_inner. Its MIR bb1 installs layer zero; bb12 constructs inline data with absent packed headers and invokes decode_precinct_packet at offset zero, SOP disabled, EPH enabled, Tier-1 disabled. Contribution parsing consumes exactly 24 information bits C0 BE FF: presence and first inclusion, six zero bitplanes, one pass, Lblock incremented from 3 to 8, length 255. The fourth byte 00 is the mandatory final stuffed/padded byte. PacketBitReader only loads a following stuffed byte when another information bit is requested. align_to_byte merely clears bits_remaining, so after_headers obtains header_bytes=3, then validates packet_data[3..5]=00 FF as EPH instead of the actual FF92 at [4..6]. The invalid-marker branch returns InvalidPacketHeader, propagated unchanged by the inner adapter's MIR bb16. The exact failed check is marker validation at the incorrectly computed boundary, not a failure in fixture setup.
Authority: clause_origin is rfc_map, not infer. I read its parent finding and manifest, and independently read cited T.800 (06/2019) B.10.1 and A.8.2 through the document tools. B.10.1 expressly requires a stuffed zero even when FF would otherwise be the last header byte; A.8.2 places FF92 immediately after the complete packet header when COD signals EPH. The harness meets both. The parent finding explicitly excluded the final-FF boundary from its implemented assessment; that is a limitation of that assessment, not an input illegality or a caller requirement that the implementation already compute the boundary correctly. Accordingly this is not grounds to revise a sound standards rule to match the correlated native/model accounting. Edition reconciliation with selected T.800 (2024) remains outstanding as recorded in specs/README.md: this diagnosis is grounded in the cited 2019 edition and the stated inspection clause, not a certification of 2024 conformance or a fully decoded JPIP codestream.

- **Production evidence:** [`src/packet.rs:L164-L166`](../../src/packet.rs#L164-L166)

```text
pub fn align_to_byte(&mut self) {
        self.bits_remaining = 0;
    }
```

- **Production evidence:** [`src/packet.rs:L150-L152`](../../src/packet.rs#L150-L152)

```text
pub const fn bytes_consumed(&self) -> usize {
        self.byte_index
    }
```

- **Production evidence:** [`src/packet.rs:L4173-L4174`](../../src/packet.rs#L4173-L4174)

```text
reader.align_to_byte();
    let header_bytes = reader.bytes_consumed();
```

- **Production evidence:** [`src/packet.rs:L4183-L4184`](../../src/packet.rs#L4183-L4184)

```text
if parameters.eph {
        let marker_start = header_bytes;
```

- **Production evidence:** [`src/packet.rs:L4189-L4195`](../../src/packet.rs#L4189-L4195)

```text
let marker = match packet_data.get(marker_start..marker_end) {
            Some(marker) => marker,
            None => return Err(packet_reader_error(offset, MalformedKind::Truncated)),
        };
        if !crate::refinement::eph_valid(marker[0], marker[1]) {
            return Err(invalid_packet_header());
        }
```

- **Production evidence:** [`src/packet.rs:L999-L1009`](../../src/packet.rs#L999-L1009)

```text
let (decoded, consumed, packed) = decode_precinct_packet(
        &inline,
        0,
        &[],
        0,
        None,
        expected_sop_sequence,
        precinct,
        false,
        limits,
    )?;
```

- **Production evidence:** [`src/packet.rs:L3963-L3965`](../../src/packet.rs#L3963-L3965)

```text
sop: tile_part.coding_style.sop_markers,
            eph: tile_part.coding_style.eph_markers,
            packed: tile_part.packed_packet_headers.is_present(),
```

- **Production evidence:** [`src/packet.rs:L4183-L4184`](../../src/packet.rs#L4183-L4184)

```text
if parameters.eph {
        let marker_start = header_bytes;
```

- **Production evidence:** [`src/packet.rs:L4189-L4195`](../../src/packet.rs#L4189-L4195)

```text
let marker = match packet_data.get(marker_start..marker_end) {
            Some(marker) => marker,
            None => return Err(packet_reader_error(offset, MalformedKind::Truncated)),
        };
        if !crate::refinement::eph_valid(marker[0], marker[1]) {
            return Err(invalid_packet_header());
        }
```

- **Production evidence:** [`src/refinement.rs:L801-L803`](../../src/refinement.rs#L801-L803)

```text
pub(crate) const fn eph_valid(marker0: u8, marker1: u8) -> bool {
    marker0 == 0xff && marker1 == 0x92
}
```

- **Harness evidence:** [`src/packet_jpip_contract_tests.rs:L61-L65`](tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs#L61-L65): `let mut precinct = literal_packet_precinct();<br>    precinct.contexts[0].maximum_zero_bitplanes = 8;<br>    precinct.decoders[0] = Tier1CodeBlockDecoder::new(<br>        1, 1, 8, SubbandOrientation::Ll, Tier1CodingStyle::default(),<br>    ).unwrap();`
- **Harness evidence:** [`src/packet_jpip_contract_tests.rs:L75-L80`](tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs#L75-L80): `assert_eq!(bits.len(), 24);<br>    assert_eq!(pack_packet_bits(&bits), [0xc0, 0xbe, 0xff]);<br>    let header = [0xc0, 0xbe, 0xff, 0];<br>    assert_eq!(header[3] & 0x80, 0, "mandatory stuffed zero");<br>    let body = vec![0xff; 255];<br>    (precinct, jpip_framed(&header, &body, false, eph, 0))`
- **Harness evidence:** [`src/packet_jpip_contract_tests.rs:L205-L208`](tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs#L205-L208): `let (mut precinct, data) = jpip_terminal_ff_packet(true);<br>    assert_eq!(&data[4..6], &[0xff, 0x92], "EPH follows the complete stuffed header");<br>    jpip_assert_inspect(&jpip_template(false, true), &data, 0, 0, &mut precinct,<br>        &limits, Ok(4 + 2 + 255));`
- **Harness evidence:** [`src/packet_jpip_contract_tests.rs:L50-L54`](tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs#L50-L54): `assert_eq!(<br>        decode_jpip_precinct_packet_inner(template, bytes, sequence, layer, precinct, limits),<br>        expected,<br>        "inline inspection result/error/prefix"<br>    );`
- **Test source:** [`src/packet_jpip_contract_tests.rs:L61-L65`](tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs#L61-L65)<br>[`src/packet_jpip_contract_tests.rs:L75-L80`](tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs#L75-L80)<br>[`src/packet_jpip_contract_tests.rs:L205-L208`](tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs#L205-L208)<br>[`src/packet_jpip_contract_tests.rs:L50-L54`](tests/DEV-EA51ACDA28/files/src/packet_jpip_contract_tests.rs#L50-L54)
- **Test source:** [`jpip_contract_registration.patch`](tests/DEV-EA51ACDA28/files/jpip_contract_registration.patch)
- **Exact mapping record:** [report.json](report.json)

## DEV-2F140D1710

- **Mapping:** `792efa70c953586e8f952abd` / `bc44d40b4d810c5fbf49fc6d5fc0638ac726f7523b57d42714f71b6557f90e97`
- **Specification:** `cd4674a2d589a697678c3223`
- **Deviation:** `b75a5ade5d09f9ed11e823db`
- **Test item:** `166619668a5d7e448c6c7ed7`
- **Case:** `cc6701332d6429318af1632c`
- **Exact standards quote:**

> Value of the horizontal offset, in units of 1/65536 of the horizontal separation XRsiz i, for the ith
>                        component. Thus, values range from 0/65536 (sample occupies its reference grid point) to
>                        XRsizc(65535/65536) (just before the next sample's reference grid point).

- **Entry:** `src/decode.rs:4113:decode::color_selection_from_validated`
- **Command:** `python3 -c import os, subprocess; from pathlib import Path; binaries = [p for p in Path('target/deepspec-crg/debug/deps').glob('jpeg2000_rs-*') if p.is_file() and os.access(p, os.X_OK)]; assert len(binaries) == 1, binaries; name = 'decode::tests::selected_color_context_uses_crg_component_separation'; assert name + ': test' in subprocess.check_output([str(binaries[0]), '--list'], text=True).splitlines(); os.execv(str(binaries[0]), [str(binaries[0]), '--exact', name, '--nocapture'])`
- **Setup:** Safe native-resolution direct helper invocation with parser-derived ImageSize, positive sampling, origin zero, a nonempty in-image ROI and a validator-produced tile list [0]. The 1024x1024 single-tile fixtures with at most three components are well within HARDENED geometry/resource limits. The CRG appears exactly once in the main header with one u16 pair per component and length 2+4*N, satisfying T.800 (06/2019) A.9.1 framing. No manual unit conversion or metadata mutation occurs. At separation 4 and registration 32768, the physical offset is 2 and the project's floor/predecessor policy requires sample 1 at coordinate 9; the context starting at 5 contains only coded sample indices 2..3.
- **Route:** The replay applies an include!-only patch to decode::tests, builds the default-feature library test binary, and executes exactly decode::tests::selected_color_context_uses_crg_component_separation. The harness frames SIZ/COD/QCD/CRG/SOT/SOD/EOC bytes, calls parse_layout under HARDENED, confirms unchanged raw CRG fields, obtains a real ValidatedSelection through validate_selection, calls color_selection_from_validated, and checks whether the returned context's coded sample interval contains the standard-offset predecessor required for each ROI coordinate. The first recorded example is the one-component separation-4 half-registration ROI [9,9,10,10]; the aggregate final assertion fails after collecting examples.
- **Observed assertion:** `assertion `left == right` failed: parse_layout(CRG)->validate_selection->color_selection_from_validated: components=[[4, 4, 32768, 32768]] ROI=[9, 9, 10, 10] context=ImageRegion { x: 5, y: 5, width: 5, height: 5 }; component=0 axis=0 coordinate=9: physical sample 1 missing from coded range 2..3`

**Full diagnosis:**

The first failing assertion is the final assert_eq!(failures, 0), with host-observed left=3874 and right=0; earlier parsing, metadata, ROI validation, target and returned-geometry checks succeeded. This is a valid direct unit-contract test, not a bypassed caller fixture: it obtains ImageSize from parse_layout(CRG), constructs an ordinary DecodeSelection, calls the actual validate_selection, then invokes color_selection_from_validated exactly as color_selection_for_size does. The test-only patch only includes the harness in decode::tests. Resource realism was checked first: each fixture is a 1024x1024 image (1,048,576 pixels), one tile, 1 or 3 eight-bit components, one resolution/layer, positive SIZ sampling <=255, discard=0, and a single main-header CRG of admissible length. HARDENED allows dimensions 1,000,000, 100,000,000 pixels, 64 components and 16,384 tiles. The 5437 cases are processed sequentially with tiny metadata/packet fixtures; even a decoded three-component image would be only a few MB, far below the 2 GiB decoded-byte limit. No oversized buffer, stack recursion or fabricated ValidatedSelection is involved. Empty packets are not used to claim decoded-pixel behavior: this helper consumes parsed geometry, not decoded sample data.

The clause has rfc_map origin. I read its parent finding and re-read T.800 (06/2019) A.9.1 through the parent mapping item. That section explicitly defines offsets in fractions of component separation, applies CRG to rendering rather than coded-grid decoding, and permits these marker values. With zero origin, separation=4 and registration=32768, physical sample i is at 4*i+2; the project's predecessor/floor sampling policy therefore requires sample floor((9-2)/4)=1 for coordinate 9. This policy is evidenced by nominal_component_at calling registered_floor_div; the standard is not being asserted to mandate a particular interpolation filter. The declared CRG physical-offset meaning and documented selected-color context promise make this a sound native-resolution clause, not a spec to rewrite to match the correlated raw-unit models/tests.

Concrete route: parse_layout copies raw CRG=32768 into both registration fields; validate_selection returns region [9,9,10,10], tiles [0], discard 0. MIR bb0 takes the d<32 branch, bb4 gives scale=1, bb24 sets target to that region, bb26 enters the component loop, bb51/bb60 subtract raw registration from 65536*9, and bb61/bb70 take nonzero remainder paths that set horizontal=vertical=4. After the loop, bb87/bb88 and bb91/bb92 produce x0=y0=5 and the successful return is region (5,5,5,5). The ordinary coded SIZ sample interval is [ceil(5/4),ceil(10/4))=[2,3), excluding physically required sample 1 at coded coordinate 4. Thus the helper has already discarded necessary registered predecessor context before any color conversion. Raw registration subtraction also has the parent finding's units discrepancy (32768 means two reference-grid units here, not half a unit); merely changing that subtraction would not by itself establish sufficient context, because one-separation lookbehind from a nonaligned ROI can still exclude the predecessor. The actual failed check is missing physical context, not merely a different formula or arbitrary nonzero exit.

This records the reproduced helper-contract deviation only, not an end-to-end wrong-pixel reproduction or formal public reachability proof. The cited edition is T.800 (06/2019), whose exact A.9.1 wording was checked; reconciliation with the newer editions named in SUPPORTED.md remains outside the evidence and is not claimed. Existing review labels and agreement of the Verus/native reference with raw-unit arithmetic do not negate this witnessed, contract-valid unit failure.

- **Production evidence:** [`src/decode.rs:L4176-L4179`](../../src/decode.rs#L4176-L4179)

```text
        let x = u64::from(target[0])
            .checked_mul(65_536)
            .ok_or_else(arithmetic_overflow)?
            .saturating_sub(u64::from(info.x_registration));
```

- **Production evidence:** [`src/decode.rs:L4184-L4186`](../../src/decode.rs#L4184-L4186)

```text
        if x.checked_rem(x_step).ok_or_else(invalid_decode)? != 0 {
            horizontal = horizontal.max(u32::from(info.x_subsampling));
        }
```

- **Production evidence:** [`src/decode.rs:L4192-L4194`](../../src/decode.rs#L4192-L4194)

```text
    let x0 = validated.region[0]
        .saturating_sub(horizontal.saturating_mul(scale))
        .max(size.x_origin);
```

- **Production evidence:** [`src/codestream.rs:L4496-L4497`](../../src/codestream.rs#L4496-L4497)

```text
                info.x_registration = registration.horizontal;
                info.y_registration = registration.vertical;
```

- **Production evidence:** [`src/decode.rs:L4103-L4104`](../../src/decode.rs#L4103-L4104)

```text
    let validated = validate_selection(size, requested)?;
    color_selection_from_validated(size, validated)
```

- **Production evidence:** [`src/color.rs:L1695`](../../src/color.rs#L1695)

```text
    let absolute_sample_x = registered_floor_div(absolute_x, x_step, info.x_registration)?
```

- **Production evidence:** [`SUPPORTED.md:L20-L23`](../../SUPPORTED.md#L20-L23)

```text
Selected color conversion preserves the full-image sampling phase at the
requested native DWT resolution, including regions starting between component
samples. It may decode bounded left/top context from neighboring tiles before
cropping the converted output; resource limits include that context.
```

- **Production evidence:** [`src/codestream.rs:L5122-L5125`](../../src/codestream.rs#L5122-L5125)

```text
                    horizontal: read_u16([
                        self.segment_byte(segment, position, 0)?,
                        self.segment_byte(segment, position, 1)?,
                    ]),
```

- **Production evidence:** [`src/codestream.rs:L4496-L4497`](../../src/codestream.rs#L4496-L4497)

```text
                info.x_registration = registration.horizontal;
                info.y_registration = registration.vertical;
```

- **Production evidence:** [`src/decode.rs:L4170-L4172`](../../src/decode.rs#L4170-L4172)

```text
        let x_step = u64::from(info.x_subsampling)
            .checked_mul(65_536)
            .ok_or_else(arithmetic_overflow)?;
```

- **Production evidence:** [`src/decode.rs:L4176-L4179`](../../src/decode.rs#L4176-L4179)

```text
        let x = u64::from(target[0])
            .checked_mul(65_536)
            .ok_or_else(arithmetic_overflow)?
            .saturating_sub(u64::from(info.x_registration));
```

- **Production evidence:** [`src/color.rs:L1881-L1886`](../../src/color.rs#L1881-L1886)

```text
    let adjusted = numerator
        .checked_sub(u64::from(registration))
        .ok_or_else(arithmetic_overflow)?;
    let denominator = u64::from(divisor)
        .checked_mul(scale)
        .ok_or_else(arithmetic_overflow)?;
```

- **Harness evidence:** [`src/decode_crg_tests.rs:L125-L126`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L125-L126): `        let layout = parse_layout(&bytes, &ResourceLimits::HARDENED)<br>            .expect("standards-framed main-header CRG fixture must parse");`
- **Harness evidence:** [`src/decode_crg_tests.rs:L152-L158`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L152-L158): `        let validated = validate_selection(size, &requested)<br>            .expect("nonempty in-image ROI on the sole tile is valid");<br>        assert_eq!(validated.region, region);<br>        assert_eq!(validated.tiles, vec![0]);<br>        assert_eq!(validated.discard_levels, 0);<br>        let (expanded, target) = color_selection_from_validated(size, validated)<br>            .expect("native-resolution context preparation must succeed");`
- **Harness evidence:** [`src/decode_crg_tests.rs:L64-L66`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L64-L66): `    let mut cases = (9..=14)<br>        .map(\|x\| (vec![[4, 4, 32_768, 32_768]], [x, x, x + 1, x + 1]))<br>        .collect::<Vec<_>>();`
- **Harness evidence:** [`src/decode_crg_tests.rs:L51-L52`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L51-L52): `    let offset = separation * u64::from(registration);<br>    let position = u64::from(coordinate) * 65_536;`
- **Harness evidence:** [`src/decode_crg_tests.rs:L56`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L56): `    position.saturating_sub(offset) / (separation * 65_536)`
- **Harness evidence:** [`src/decode_crg_tests.rs:L175-L176`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L175-L176): `                let first_available = start.div_ceil(separation);<br>                let end_available = u64::from(region[axis + 2]).div_ceil(separation);`
- **Harness evidence:** [`src/decode_crg_tests.rs:L181`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L181): `                    if required < first_available \|\| required >= end_available {`
- **Harness evidence:** [`src/decode_crg_tests.rs:L198`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L198): `    assert_eq!(failures, 0, "{}", examples.join("\n"));`
- **Test source:** [`src/decode_crg_tests.rs:L125-L126`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L125-L126)<br>[`src/decode_crg_tests.rs:L152-L158`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L152-L158)<br>[`src/decode_crg_tests.rs:L64-L66`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L64-L66)<br>[`src/decode_crg_tests.rs:L51-L52`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L51-L52)<br>[`src/decode_crg_tests.rs:L56`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L56)<br>[`src/decode_crg_tests.rs:L175-L176`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L175-L176)<br>[`src/decode_crg_tests.rs:L181`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L181)<br>[`src/decode_crg_tests.rs:L198`](tests/DEV-2F140D1710/files/src/decode_crg_tests.rs#L198)
- **Test source:** [`validation/crg-context-tests.patch`](tests/DEV-2F140D1710/files/validation/crg-context-tests.patch)
- **Exact mapping record:** [report.json](report.json)

## DEV-355FE751F9

- **Mapping:** `de8b2b27d2f3b6d75bb83616` / `389c13b2543531f2986c7b3b1d14f8d77877e47376e48b3e4f429a01a6903164`
- **Specification:** `1db73a2c9da1c067c2a9d39a`
- **Deviation:** `cb061dee09165dab5415dd82`
- **Test item:** `12db5111ad29d4dc5a0ff9bb`
- **Case:** `817c898abc85ba6ef483648d`
- **Exact standards quote:**

> In the accompanying figures, boundaries and coordinate axes are shown. In each case, the samples or coefficients
> coincident with the left and upper boundaries are included in a given region, while samples or coefficients along the right
> and/or lower boundaries are not included in that region.

- **Entry:** `src/decode.rs:3588:decode::decode_codestream_selected`
- **Command:** `target/selected-contract-suite decode::tests::selected_contract_geometry_and_native_samples --exact --nocapture`
- **Setup:** Safe Rust slices and default options; actual encoded SIZ/COD/QCD/SOT/SOD/NLT bytes, not patched layout or reconstruction state. Image [1,32)x[1,16), eight 8x8 tiles, one unsigned 8-bit unit-sampled component, one decomposition level, empty packet payloads reconstructing zero coefficients. Tile-local gamma slopes [1,2,4,8,2,4,8,1] are positive supported parameters with equal source/target precision, exponent=1, threshold=1, a=1,b=0. All tile boundaries are multiples of eight, so discard=1 ownership is unambiguous. Region [3,29)x[3,15) lies inside the image. Tiny resource requirements are below default policy and ordinary caller scale. Successful unwrap and preceding controls establish successful public-entry decoding, while the independent slope oracle exposes wrong tile transform assignment.
- **Route:** Host ran target/selected-contract-suite decode::tests::selected_contract_geometry_and_native_samples --exact --nocapture. The harness constructs selected_contract_patterned() byte codestream, calls decode_codestream_selected(&patterned, &options, &selection).unwrap() for All tiles, discard 0 and 1, with/without an interior crop, and compares returned unsigned samples against the owning native tile's gamma slope. Entry parsing succeeds, selected tile reconstruction and assembly succeed, then typed_layout obtains nonlinear regions from unreduced SIZ bounds. The final assertion records two d=1 sample mismatches.
- **Observed assertion:** `native tile-specific nonlinear transform mismatch: ["d=1, region=None, native=(4,1), tile=1, expected=64, actual=128", "d=1, region=Some(ImageRegion { x: 3, y: 3, width: 26, height: 12 }), native=(4,2), tile=1, expected=64, actual=128"]`

**Full diagnosis:**

The first failed assertion is the final mismatches.is_empty() check in selected_contract_geometry_and_native_samples, not a setup error or generic exit status: host replay reports d=1 native=(4,1), tile=1, expected=64, actual=128, and the equivalent cropped failure at (4,2). The clause has infer origin, not rfc_map. Its promise that selected native-resolution samples receive their nonlinear processing is sound for this API: README explicitly promises native wavelet resolution, decode_typed_layout deliberately routes selected reconstruction through typed_layout, and typed_layout deliberately applies each tile's nonlinear transform. This is not a request for a new standards guarantee or a revision to match a shared model.

Entry realism was checked: the failing fixture has image endpoints 32x16, origin (1,1), one unsigned 8-bit 1x1-sampled component, eight 8x8 tiles, one decomposition level, one layer, 16 empty packets and at most 32 full-resolution codeblocks. Its full component has 465 samples (3720 sample bytes), with 105 samples at discard=1; the cropped reduced output has 78. The byte stream is under 1 KiB. These are far below HARDENED/default limits (width/height 1,000,000, pixels 100,000,000, components 64, tiles 16,384, resolutions 33, codeblocks/precincts 16,777,216, decoded bytes 2,147,483,648), without excessive stack depth or fabricated private state. Real JP2/JPH selected and JPIP callers forward the same byte-slice/options/selection interface; this direct public unit-contract test need not simulate their containers. All-tile selection, discard=1 within the available level, and the optional interior rectangle [3,3,29,15] are admissible. Test-only setup merely includes tests and adds an unused-in-this-case bounded precinct setter; production decoder behavior is unchanged.

Compiler declaration and CFG confirm parse_layout succeeds into bb5 then bb7 calls decode_typed_layout with Some(selection). Source then follows decode_layout_selected: validate execution and selection, decode all selected tile parts with Some(discard_levels), assemble reduced components, then decoded.typed and collect_nonlinear_regions. The latter computes output extents from the reduced TypedImage but intersects them directly with original SIZ tile_component_bounds, without a discard parameter or coordinate reduction. For d=1, tile 1's correct native extent is [4,8)x[1,4), but its unreduced bounds are [8,16)x[1,8). Native (4,1) consequently receives tile 0's slope-1 transform rather than tile 1's slope-2 transform. The same happens at (4,2) in the crop. Zero coefficients type to 128; source gamma logic's linear branch divides by slope, so tile 1 must produce 64, not 128. Aligned tile boundaries remove ownership ambiguity; d=0 controls pass. This specifically violates the clause's native-reconstruction nonlinear sample guarantee, even though dimensions remain correct. The supplied host replay is authoritative; the earlier report_deviation not_reproduced record is not relied on. This is an agent-assessed reproduced public-entry deviation, not a formal reachability or general standards-conformance proof.

- **Production evidence:** [`src/decode.rs:L3610-L3614`](../../src/decode.rs#L3610-L3614)

```text
let decoded = match requested {
        None => decode_layout(layout, limits)?,
        Some(requested) => decode_layout_selected(layout, requested, limits)?,
    };
    let image = typed_layout(layout, decoded, limits)?;
```

- **Production evidence:** [`src/decode.rs:L3705-L3709`](../../src/decode.rs#L3705-L3709)

```text
let mut image = decoded.typed(limits)?;
    let regions = collect_nonlinear_regions(layout, &image)?;
    if !regions.is_empty() {
        image.apply_nonlinear_regions(&regions, limits)?;
    }
```

- **Production evidence:** [`src/decode.rs:L3857-L3864`](../../src/decode.rs#L3857-L3864)

```text
let bounds = layout
                    .info
                    .image_size
                    .tile_component_bounds(tile_index, component)?;
                let x0 = bounds.x0.max(output_x0);
                let y0 = bounds.y0.max(output_y0);
                let x1 = bounds.x1.min(output_x1);
                let y1 = bounds.y1.min(output_y1);
```

- **Production evidence:** [`src/codestream.rs:L282-L285`](../../src/codestream.rs#L282-L285)

```text
let x0 = registered_ceil_div(tile.x0, x_subsampling, info.x_registration)?;
        let y0 = registered_ceil_div(tile.y0, y_subsampling, info.y_registration)?;
        let x1 = registered_ceil_div(tile.x1, x_subsampling, info.x_registration)?;
        let y1 = registered_ceil_div(tile.y1, y_subsampling, info.y_registration)?;
```

- **Production evidence:** [`src/image.rs:L1215-L1216`](../../src/image.rs#L1215-L1216)

```text
} else if normalized <= threshold {
        nlt_fixed_div_q32(normalized, slope)
```

- **Production evidence:** [`README.md:L81-L83`](../../README.md#L81-L83)

```text
Regions use absolute, half-open SIZ image-reference-grid coordinates. Discarding
resolution levels selects native wavelet resolution, not a resize of a fully
decoded image. A selection must fit the input's actual origin and geometry.
```

- **Production evidence:** [`src/decode.rs:L3737-L3744`](../../src/decode.rs#L3737-L3744)

```text
    let x1 = x0
        .checked_add(output.width())
        .ok_or_else(arithmetic_overflow)?;
    let y1 = y0
        .checked_add(output.height())
        .ok_or_else(arithmetic_overflow)?;
    let count = size.tile_count()?;
    Ok(((x0, y0, x1, y1), count))
```

- **Production evidence:** [`src/decode.rs:L3861-L3873`](../../src/decode.rs#L3861-L3873)

```text
                let x0 = bounds.x0.max(output_x0);
                let y0 = bounds.y0.max(output_y0);
                let x1 = bounds.x1.min(output_x1);
                let y1 = bounds.y1.min(output_y1);
                if x0 < x1 && y0 < y1 {
                    regions.push(NonlinearRegion {
                        component,
                        x0: x0.checked_sub(output_x0).ok_or_else(arithmetic_overflow)?,
                        y0: y0.checked_sub(output_y0).ok_or_else(arithmetic_overflow)?,
                        x1: x1.checked_sub(output_x0).ok_or_else(arithmetic_overflow)?,
                        y1: y1.checked_sub(output_y0).ok_or_else(arithmetic_overflow)?,
                        transform,
                    });
```

- **Production evidence:** [`src/image.rs:L881`](../../src/image.rs#L881)

```text
                    while x < region.x1 {
```

- **Harness evidence:** [`src/decode_selected_contract_tests.rs:L4`](tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L4): `for value in [32_u32, 16, 1, 1, 8, 8, 0, 0] {`
- **Harness evidence:** [`src/decode_selected_contract_tests.rs:L53-L59`](tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L53-L59): `fn selected_contract_patterned() -> Vec<u8> {<br>    let mut bytes = selected_contract_grid(1, &[[7, 1, 1]]);<br>    for (tile, slope) in [1_u8, 2, 4, 8, 2, 4, 8, 1].iter().enumerate() {<br>        selected_contract_tile_header(&mut bytes, tile as u16, selected_contract_gamma(*slope));<br>    }<br>    bytes<br>}`
- **Harness evidence:** [`src/decode_selected_contract_tests.rs:L204-L208`](tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L204-L208): `for discard in 0..=1 {<br>        for region in [None, Some(crate::ImageRegion::new(3, 3, 26, 12))] {<br>            let selection =<br>                crate::options::selection_from_parts(TileSelection::All, region, discard);<br>            let image = decode_codestream_selected(&patterned, &options, &selection).unwrap();`
- **Harness evidence:** [`src/decode_selected_contract_tests.rs:L232-L234`](tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L232-L234): `let tile = (py * scale / 8) * 4 + px * scale / 8;<br>                    let expected = 128 / u64::from(slopes[tile as usize]);<br>                    let actual = samples[((py - y) * (ex - x) + px - x) as usize];`
- **Harness evidence:** [`src/decode_selected_contract_tests.rs:L247-L250`](tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L247-L250): `assert!(<br>        mismatches.is_empty(),<br>        "native tile-specific nonlinear transform mismatch: {mismatches:?}"<br>    );`
- **Test source:** [`src/decode_selected_contract_tests.rs:L4`](tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L4)<br>[`src/decode_selected_contract_tests.rs:L53-L59`](tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L53-L59)<br>[`src/decode_selected_contract_tests.rs:L204-L208`](tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L204-L208)<br>[`src/decode_selected_contract_tests.rs:L232-L234`](tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L232-L234)<br>[`src/decode_selected_contract_tests.rs:L247-L250`](tests/DEV-355FE751F9/files/src/decode_selected_contract_tests.rs#L247-L250)
- **Test source:** [`selected_contract_setup.py`](tests/DEV-355FE751F9/files/selected_contract_setup.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-404E919A62

- **Mapping:** `470d13ab8f2fa16772f8b95e` / `0a75469548b586a45c0611d35904584708c4e87a3158bf8fda5e5cee73ea8e61`
- **Specification:** `d348494f1956deec3f860649`
- **Deviation:** `482b542f6def2f270392639d`
- **Test item:** `bf508298baa60bed6c89cac4`
- **Case:** `8b69097aab2e6e4d6823ff4c`
- **Exact standards quote:**

> The 2D_SR procedure then applies the HOR_SR procedure to all rows of a(u, v), and
> finally applies the VER_SR procedure to all columns of a(u, v) to produce the reconstructed sub-band

- **Entry:** `src/transform.rs:1274:transform::inverse_level_arbitrary`
- **Command:** `cargo test --locked --lib transform::tests::arbitrary_level_contract::arbitrary_level_matches_standard_equations_and_rejects_bad_geometry_or_budget -- --exact --nocapture`
- **Setup:** Three ordinary borrowed samples, mathematically correct parity-partitioned dimensions and exact sample lengths, W=1,H=3, and decoded-byte limit 24 satisfy admission. The semantic REV/ARB/CON kernel has minit=0, signed 8-bit coefficients [-1] then [2], zero offsets/exponents/residues, and no scaling. It is built with existing explicit test-only constructors; unused descriptor metadata is not interpreted by this numerical unit API. Both singleton high-pass horizontal inputs are even, so halving is exact. All subsequent arithmetic is small exact reversible integer arithmetic. The independent standard oracle yields vertical [-90,128,-40] -> [-90,208,-40] -> [118,208,168].
- **Route:** The replayed libtest iterates width=1,height=3,first_x=false,first_y=false,reversible=true,odd=false, partitions fixed-seed input [-180,256,-80] into valid bands, computes its independent absolute-coordinate H-3 oracle, and calls inverse_level_arbitrary directly. inverse_level performs validated/admitted singleton horizontal synthesis, then inverse_columns -> inverse_column_pair -> inverse_arbitrary -> apply_arbitrary_steps -> apply_arbitrary_step -> arbitrary_reversible_target -> lifting_source_sum. The odd-origin source-index error returns the wrong successful plane; the tuple comparison adds this context to failures and the final failures.is_empty() assertion fails.
- **Observed assertion:** `1x3, phases=false/false, REV=true, minit=false: interleaved input=[-180, 256, -80]; expected [118, 208, 168]; got ReconstructedPlane { width: 1, height: 3, fraction_bits: 0, samples: [218, 308, 268] }`

**Full diagnosis:**

Case 8b69097aab2e6e4d6823ff4c faithfully exposes a deviation from clause d348494f1956deec3f860649: reconstruction with the supplied arbitrary kernel must respect the horizontal and vertical phases, including the source coordinates of its lifting filter. The first failed assertion is failures.is_empty() at harness line 309, after all 560 contexts and geometry/storage/budget/immutability checks completed. It aggregates 393 numerical mismatches, not a setup failure or an arbitrary nonzero exit. The first printed example is an irreversible one-unit discrepancy; I do not rely on exact irreversible precision or classify every reported mismatch. The same failed assertion includes the exact reversible 1x3 witness quoted in observation, which has no rounding-tolerance ambiguity.

Entry realism was checked first: the sweep uses at most 17*7=119 input samples (952 bytes), two lifting steps with one coefficient each, at most seven row syntheses and seventeen column syntheses, and sequential cases with small heap temporaries and no recursion. Each admitted call sets its decoded-byte limit to exactly 8*W*H; the selected witness uses only three samples and 24 bytes. These are far below the default 2 GiB decoded-byte limit and ordinary layout/stack capacities. decode.rs constructs the same WaveletLevel from four working bands and geometry phases before calling inverse_level_arbitrary; there is no minimum-size or even-origin requirement. This is a direct unit_contract invocation, not a fixture skipping an owning caller whose prerequisites it needs.

Authority is rfc_map item 470d13ab8f2fa16772f8b95e, not an inference to revise to match the implementation. I read its parent finding and pinned manifest, T.800 (06/2019) F.3.2 and T.801 (06/2021) A.3.5, H.2.1, H.2.2, H.2.3, H.2.4.1-3 and H.2.5.1. F.3.2 supplies the four-band horizontal-before-vertical ordering. H.2.1 defines singleton halving at an odd origin and reverse step order. H-3 uses absolute target coordinate 2n+m_s and source coordinate 2n+1-m_s+2(k+offs); H.2.4.3 extends each parity subsequence independently. A.3.5 permits the witness's REV/ARB/CON signed-8-bit coefficients, two steps, zero offsets/exponents/residues and minit=0. The parent finding limits its demonstrated mapping to axis order rather than certifying numerical equivalence; that limitation is not permission to reconstruct the wrong arbitrary filter at an odd phase. Neither the executable model nor retained native tests can override H-3.

For W=1,H=3, phases=false/false, reversible=true,minit=false, the bands are LL=(0,1,[]), HL=(1,1,[256]), LH=(0,2,[]), HH=(1,2,[-180,-80]). validate_level accepts exactly these dimensions and sample counts. Existing explicit cfg(test) constructors provide valid decoded numerical kernel fields: step 0 alpha=-1, step 1 alpha=2, all offsets/exponents/residues zero, no scaling. Negative coefficients use the same sign-extended representation as read_kernel_coefficient. for_test's descriptor sentinel is not a valid serialized ATK header, but neither descriptor nor marker metadata participates in this decoded-kernel API or its raw numerical view; the numerical fields can be signalled by legal descriptor 0x10ff. No codestream-parsing claim is needed for this unit entry.

The compiler declaration and MIR CFG show inverse_level_arbitrary forwards !kernel.reversible() and Some(kernel) to inverse_level. That helper validates/admit-budgets, reconstructs the low and high rows, then columns. The width-one odd horizontal phase halves the input into vertical interleaved [-90,128,-40]. inverse_column_pair calls inverse_arbitrary with low_pass_first=false. apply_arbitrary_step correctly maps absolute even targets to local odd indices, but lifting_source_sum discards the origin: it sets target_parity=target&1 and computes delta=1-2*target_parity+2*(coefficient+offset). For the first reconstruction step, local target 1 is absolute coordinate 2. H-3 requires source absolute 3/local 2 (value -40); the implementation instead selects local 0 (value -90). Thus the middle value becomes 128-2*(-90)=308 rather than 128-2*(-40)=208. The next step produces [218,308,268] instead of [118,208,168], exactly matching host output. When the origin is odd, the local/absolute parity reversal changes the required source displacement by two. This is a phase-sensitive arbitrary-filter synthesis error, not malformed input, budget exhaustion, a wrong reversible oracle, or an overstrong specification. No production changes or additional replay artifacts were needed.

- **Production evidence:** [`src/transform.rs:L1279`](../../src/transform.rs#L1279)

```text
    inverse_level(level, limits, !kernel.reversible(), Some(kernel))
```

- **Production evidence:** [`src/transform.rs:L653-L657`](../../src/transform.rs#L653-L657)

```text
    let target_local_odd = if low_pass_first {
        update_odd
    } else {
        !update_odd
    };
```

- **Production evidence:** [`src/transform.rs:L881`](../../src/transform.rs#L881)

```text
        let target_parity = target & 1;
```

- **Production evidence:** [`src/transform.rs:L890-L902`](../../src/transform.rs#L890-L902)

```text
        let base = 1_isize
            .checked_sub(doubled_parity)
            .ok_or_else(arithmetic_overflow)?;
        let position = coefficient
            .checked_add(offset as isize)
            .ok_or_else(arithmetic_overflow)?;
        let distance = position.checked_mul(2).ok_or_else(arithmetic_overflow)?;
        let delta = base.checked_add(distance).ok_or_else(arithmetic_overflow)?;
        extended_sample(
            samples,
            target.checked_add(delta).ok_or_else(arithmetic_overflow)?,
            whole_sample_extension,
        )
```

- **Production evidence:** [`src/transform.rs:L1710-L1711`](../../src/transform.rs#L1710-L1711)

```text
    if let Some(kernel) = kernel {
        inverse_arbitrary(&low_column, &high_column, low_pass_first, kernel)
```

- **Production evidence:** [`src/decode.rs:L3368-L3369`](../../src/decode.rs#L3368-L3369)

```text
            if let Some(kernel) = kernel {
                inverse_level_arbitrary(level, kernel, limits)?
```

- **Production evidence:** [`src/transform.rs:L1279`](../../src/transform.rs#L1279)

```text
    inverse_level(level, limits, !kernel.reversible(), Some(kernel))
```

- **Production evidence:** [`src/transform.rs:L1743-L1750`](../../src/transform.rs#L1743-L1750)

```text
    let low_rows = inverse_rows(
        level.ll,
        level.hl,
        level.low_pass_first_x,
        irreversible,
        kernel,
        low_capacity,
    )?;
```

- **Production evidence:** [`src/transform.rs:L1752-L1759`](../../src/transform.rs#L1752-L1759)

```text
    let high_rows = inverse_rows(
        level.lh,
        level.hh,
        level.low_pass_first_x,
        irreversible,
        kernel,
        high_capacity,
    )?;
```

- **Production evidence:** [`src/transform.rs:L1760-L1776`](../../src/transform.rs#L1760-L1776)

```text
    let output = inverse_columns(
        WaveletBand {
            width: level.output_width,
            height: low_height,
            samples: &low_rows,
        },
        WaveletBand {
            width: level.output_width,
            height: high_height,
            samples: &high_rows,
        },
        level.output_height,
        level.low_pass_first_y,
        irreversible,
        kernel,
        output_len,
    )?;
```

- **Harness evidence:** [`src/transform/tests/arbitrary_level_contract.rs:L212`](tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L212): `                            let result = inverse_level_arbitrary(level, &kernel, &limits);`
- **Harness evidence:** [`src/transform/tests/arbitrary_level_contract.rs:L208-L211`](tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L208-L211): `                            let bytes = (width * height * 8) as u64;<br>                            let limits = ResourceLimits::default()<br>                                .with_maximum_decoded_bytes(bytes)<br>                                .unwrap();`
- **Harness evidence:** [`src/transform/tests/arbitrary_level_contract.rs:L166-L169`](tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L166-L169): `                                    let high_x = (x % 2 == 0) != first_x;<br>                                    let high_y = (y % 2 == 0) != first_y;<br>                                    bands[usize::from(high_x) + 2 * usize::from(high_y)]<br>                                        .push(data[y * width + x]);`
- **Harness evidence:** [`src/transform/tests/arbitrary_level_contract.rs:L74-L75`](tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L74-L75): `            let n = (coordinate - parity) / 2;<br>            let source = 2 * n + 1 - parity;`
- **Harness evidence:** [`src/transform/tests/arbitrary_level_contract.rs:L309-L312`](tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L309-L312): `    assert!(<br>        failures.is_empty(),<br>        "{} standard-equation mismatches; first examples:\n{}",<br>        failures.len(),`
- **Test source:** [`src/transform/tests/arbitrary_level_contract.rs:L212`](tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L212)<br>[`src/transform/tests/arbitrary_level_contract.rs:L208-L211`](tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L208-L211)<br>[`src/transform/tests/arbitrary_level_contract.rs:L166-L169`](tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L166-L169)<br>[`src/transform/tests/arbitrary_level_contract.rs:L74-L75`](tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L74-L75)<br>[`src/transform/tests/arbitrary_level_contract.rs:L309-L312`](tests/DEV-404E919A62/files/src/transform/tests/arbitrary_level_contract.rs#L309-L312)
- **Test source:** [`arbitrary-level-registration.patch`](tests/DEV-404E919A62/files/arbitrary-level-registration.patch)
- **Exact mapping record:** [report.json](report.json)

## DEV-37A5BF80E8

- **Mapping:** `0c974871043c1354a71e5867` / `fe3f99a20b65efc22c76c0b5682b2875abd51cb731d4433f9d8fe723169c9524`
- **Specification:** `addb5e4e5b1eb100ab900f62`
- **Deviation:** `1d56123f20c53670cd1ffde5`
- **Test item:** `a58f5a821f0df6756f7e94f2`
- **Case:** `1a17e3ea63a5e2d61153056f`
- **Exact standards quote:**

> As illustrated in Figure F.12, the VER_SR procedure applies the one-dimensional sub-band reconstruction (1D_SR
> procedure) to each column u of the input array a(u, v) and stores the result back in each column.

- **Entry:** `src/transform.rs:1700:transform::inverse_column_pair`
- **Command:** `cargo test --lib transform::tests::part1_column_pair_contract -- --exact --nocapture`
- **Setup:** For extent [1,2), the phase formula produces low_height=0 and high_height=1; both bands have width=1, x=0, exact row-major slice lengths zero and one, respectively. False low_pass_first correctly selects the high-band singleton. The one signed Q16 coefficient is even and modest, and all dimensions, capacities, byte limits and arithmetic are representable. Interleave's required low count is floor(1/2)=0 and high count is one. The unit test needs no fabricated private state, and the public caller accepts the same fixture. Test-only registration does not change production behavior.
- **Route:** The replayed cfg(test) include runs part1_column_pair_contract. Its width=1,height=1,low_first=false,irreversible=true fixture calls inverse_column_pair(low,high,0,false,true,None), compares the returned column against the independent F.3.6 singleton oracle, and collects the displayed mismatch. The same fixture calls public inverse_vertical_9_7 with default limits, reaching inverse_vertical -> inverse_columns -> inverse_column_pair -> inverse_9_7 and reports the same unhalved value in the output plane. The terminal failures.is_empty() assertion fails after all 1196 columns ran.
- **Observed assertion:** `inverse_column_pair width=1 height=1 origin=1 x=0 irreversible=true low=[] high=[-15859712]: expected=[-7929856], actual=Ok([-15859712])`

**Full diagnosis:**

The first failed numerical check accumulated by the replay is width=1,height=1,origin=1,x=0,irreversible=true: expected [-7929856], actual Ok([-15859712]); the final failures.is_empty() assertion reports it. This is a valid unit-contract entry, not malformed fixture setup. Realism was checked: generated dimensions are at most 7x65 (455 i64 samples, 3640 bytes per full plane); 1196 selected columns are processed sequentially with no recursion or large fixed stack buffers. All generated coefficient bands together contain 19656 samples across the sequential geometry/filter/phase fixtures, not a simultaneous allocation. Default HARDENED limits allow 1000000 per dimension, 100000000 pixels and 2147483648 decoded bytes. The failing entry needs only one i64 coefficient. Widths match, x=0 is in bounds, rectangular backing lengths are exact, and origin=1 with extent [1,2) gives low height zero and high height one. Empty low storage is therefore required, not an inadmissible missing band. Q16 high=-15859712 is even, so division by two is exact and does not depend on rounding or overflow conventions. No callee entry requirement is violated.
The clause originates in rfc_map 0c974871043c1354a71e5867. Independently re-read its pinned T.800 (06/2019), SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e: F.3.5 requires per-column 1D_SR; F.3.6 states for length one X(i0)=Y(i0) for even i0 and Y(i0)/2 for odd i0. That singleton rule precedes and is not restricted by the subsequent length>=2 filter-specific procedures, so it applies to the selected Part 1 irreversible filter as well. None excludes Part 2 kernels. This establishes the oracle for this cited edition without claiming reconciliation with newer editions. The stored inverse_9_7 contracts and Q16 model explicitly describe unchanged singletons, but their stated authority is the project model, not independently established standard conformance; their agreement with implementation cannot override this standards-derived clause.
Compiler declaration/CFG evidence shows inverse_column_pair successfully collects [] and [-15859712], takes kernel=None then irreversible=true (bb8 -> bb14 -> bb15 -> bb16 -> bb17), and calls inverse_9_7. Its CFG takes interleave success then length<=1 (bb4 -> bb6 -> bb7) and returns the unchanged vector, without phase-sensitive singleton division. The harness directly invokes this valid unit entry and independently exercises the owning public route inverse_vertical_9_7 -> inverse_vertical -> inverse_columns -> inverse_column_pair -> inverse_9_7. Public geometry/byte checks accept the entry, and store_column writes the same incorrect value into the 1x1 output. The host output explicitly reports that public result too. Consequence: the odd-origin singleton column is twice its required reconstructed value. All 17 reported mismatches are this boundary (13 selected columns and four public planes); no claim of whole-decoder/codestream reachability or other lifting-rule correctness is needed. Production source is unchanged.

- **Production evidence:** [`src/transform.rs:L461-L469`](../../src/transform.rs#L461-L469)

```text
pub fn inverse_9_7(
    low: &[i64],
    high: &[i64],
    low_pass_first: bool,
) -> Result<Vec<i64>, DecodeError> {
    let mut output = interleave(low, high, low_pass_first)?;
    if output.len() <= 1 {
        return Ok(output);
    }
```

- **Production evidence:** [`src/transform.rs:L1708-L1716`](../../src/transform.rs#L1708-L1716)

```text
let low_column = collect_column(low.samples, low.width, low.height, x)?;
    let high_column = collect_column(high.samples, high.width, high.height, x)?;
    if let Some(kernel) = kernel {
        inverse_arbitrary(&low_column, &high_column, low_pass_first, kernel)
    } else if irreversible {
        inverse_9_7(&low_column, &high_column, low_pass_first)
    } else {
        inverse_5_3(&low_column, &high_column, low_pass_first)
    }
```

- **Production evidence:** [`src/transform.rs:L1518`](../../src/transform.rs#L1518)

```text
inverse_vertical(low, high, output_height, low_pass_first, limits, true, None)
```

- **Production evidence:** [`src/transform.rs:L1653`](../../src/transform.rs#L1653)

```text
match store_column(&mut samples, low.width, output_height, x, &column) {
```

- **Production evidence:** [`src/transform.rs:L1708-L1716`](../../src/transform.rs#L1708-L1716)

```text
let low_column = collect_column(low.samples, low.width, low.height, x)?;
    let high_column = collect_column(high.samples, high.width, high.height, x)?;
    if let Some(kernel) = kernel {
        inverse_arbitrary(&low_column, &high_column, low_pass_first, kernel)
    } else if irreversible {
        inverse_9_7(&low_column, &high_column, low_pass_first)
    } else {
        inverse_5_3(&low_column, &high_column, low_pass_first)
    }
```

- **Production evidence:** [`src/transform.rs:L1653`](../../src/transform.rs#L1653)

```text
match store_column(&mut samples, low.width, output_height, x, &column) {
```

- **Production evidence:** [`src/transform.rs:L1990-L1994`](../../src/transform.rs#L1990-L1994)

```text
let row = transform_dimension(y)?
        .checked_mul(transform_dimension(width)?)
        .ok_or_else(arithmetic_overflow)?;
    row.checked_add(transform_dimension(x)?)
        .ok_or_else(arithmetic_overflow)
```

- **Harness evidence:** [`src/transform_column_pair_contract_tests.rs:L15-L17`](tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L15-L17): `let origin = u32::from(!low_first);<br>                    let low_height = (origin + height).div_ceil(2) - origin.div_ceil(2);<br>                    let high_height = height - low_height;`
- **Harness evidence:** [`src/transform_column_pair_contract_tests.rs:L58-L59`](tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L58-L59): `assert_eq!(low.samples.len(), (width * low_height) as usize);<br>                    assert_eq!(high.samples.len(), (width * high_height) as usize);`
- **Harness evidence:** [`src/transform_column_pair_contract_tests.rs:L65-L74`](tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L65-L74): `let expected = if height == 1 {<br>                            // F.3.6: X(i0)=Y(i0) for even i0, Y(i0)/2 for odd i0.<br>                            Ok(if low_first {<br>                                vec![l[0]]<br>                            } else {<br>                                vec![h[0] / 2]<br>                            })<br>                        } else {<br>                            reference_sequence(l, h, low_first, irreversible)<br>                        };`
- **Harness evidence:** [`src/transform_column_pair_contract_tests.rs:L80-L82`](tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L80-L82): `let actual =<br>                            inverse_column_pair(low, high, x, low_first, irreversible, None);<br>                        if actual.as_ref() != Ok(&expected) {`
- **Harness evidence:** [`src/transform_column_pair_contract_tests.rs:L112-L118`](tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L112-L118): `inverse_vertical_9_7(<br>                                low,<br>                                high,<br>                                height,<br>                                low_first,<br>                                &ResourceLimits::default(),<br>                            )`
- **Harness evidence:** [`src/transform_column_pair_contract_tests.rs:L157`](tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L157): `assert!(failures.is_empty(), "{}", failures.join("\n"));`
- **Test source:** [`src/transform_column_pair_contract_tests.rs:L15-L17`](tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L15-L17)<br>[`src/transform_column_pair_contract_tests.rs:L58-L59`](tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L58-L59)<br>[`src/transform_column_pair_contract_tests.rs:L65-L74`](tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L65-L74)<br>[`src/transform_column_pair_contract_tests.rs:L80-L82`](tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L80-L82)<br>[`src/transform_column_pair_contract_tests.rs:L112-L118`](tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L112-L118)<br>[`src/transform_column_pair_contract_tests.rs:L157`](tests/DEV-37A5BF80E8/files/src/transform_column_pair_contract_tests.rs#L157)
- **Test source:** [`validation/install-column-pair-contract-test.py`](tests/DEV-37A5BF80E8/files/validation/install-column-pair-contract-test.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-2D75F3E8F6

- **Mapping:** `230760b6124854275489a5da` / `0cff904f26ae145b48b676d2059a6cf09fb21630387412319825758c3ae005af`
- **Specification:** `8d6f7e8f32d0032e5626ca7b`
- **Deviation:** `e7d4bee5fa853e9cb6baabd5`
- **Test item:** `ea8d9c3e31baf68e430c41aa`
- **Case:** `68574d962e014b599022f579`
- **Exact standards quote:**

> The JPEG 2000 Signature box shall be the first box in the file, and all files shall contain one and only one
> JPEG 2000 Signature box.

- **Entry:** `src/jp2.rs:977:jp2::parse`
- **Command:** `python3 validation/jp2_spec_runner.py spec_signature_first_fixed_and_unique`
- **Setup:** A direct public parser unit test supplies owned, fully framed bytes and a standard immutable ResourceLimits::default(), with no forged internal state or bypassed owning caller. The complete original 167-byte fixture is verified/decoded successfully and its canonical 12-byte signature alone is duplicated, yielding 179 bytes, five top-level boxes and a 1x1 single-component image. Resource requirements are tiny relative to HARDENED limits. Signature uniqueness is the intended rejected property, not an entry requirement for this validation API; its assessed unit-contract requires list is empty.
- **Route:** Host runs python3 validation/jp2_spec_runner.py spec_signature_first_fixed_and_unique. The runner invokes the exactly registered jp2::tests::spec_signature_first_fixed_and_unique test. spec_parts reads and checks gray.jp2, the test confirms baseline parse succeeds, then duplicate.insert(2, parts[0].clone()) adds a second canonical signature after ftyp. spec_reject calls parse(&duplicate.concat(), &ResourceLimits::default()). parse validates the first signature and ftyp, dispatches the second signature through its metadata fallback and inspect_metadata_box's non-association leaf success, parses the original header/codestream, and returns Ok. spec_reject records the unmet malformed-input check, and spec_finish asserts failures.is_empty(), producing the quoted host-observed failure.
- **Observed assertion:** `second signature at box 2; malformed rejection, got Ok(Jp2File`

**Full diagnosis:**

The first failed assertion is spec_finish's failures.is_empty() at src/jp2/spec_tests.rs:56; its first recorded failed check is 'second signature at box 2; malformed rejection, got Ok(Jp2File ...'. This specifically refutes the clause's signature-uniqueness requirement, not merely an exit-code expectation. The later box-3 and box-4 checks reproduce the same omission. Entry realism: the repository's complete gray.jp2 fixture is 167 bytes; inserting its canonical 12-byte signature produces a 179-byte input with five top-level boxes, one 1x1 8-bit grayscale component, one codestream, two header children, and no association recursion. Top-level non-codestream framing totals 89 bytes. These are far below default HARDENED limits (2,147,483,648 input bytes, 16,777,216 metadata bytes, depth 32, 64 codestreams/components, 100,000,000 pixels, 1,000,000,000 work units). This is the same byte-slice/resource-limit entry shape used by real decode callers, using their normal tiny grayscale fixture, not an artificial private-state fixture. The unit-contract entry has no requires, and rejecting structurally invalid container bytes is the operation under test: a duplicate signature violates the target validation predicate, not a caller precondition. Harness setup also verifies canonical ftyp/header/framing, decodes the original fixture to its expected 1x1 sample, and successfully parses the original before adding the duplicate. Registration only includes tests within the existing cfg(test) module; production behavior is unchanged. Compiler describe_declaration confirms the &[u8], &ResourceLimits -> Result<Jp2File, DecodeError> signature. The control-flow graph and source show first-signature validation followed by a loop testing jp2h/jpch/jplh/comp/jp2c but not SIGNATURE; the duplicate falls into inspect_metadata_box, whose non-ASSOCIATION leaf path returns Ok((1, depth)). Parsing then reaches Ok(Jp2File), counting the duplicate as one metadata box. Standard authority was checked, not inferred from agreement with project tests/models: clause_origin is rfc_map, and read_rfc_section on its originating mapping confirms T.800 (2019-06) I.5.1 explicitly requires first position and 'one and only one' signature. This file-validity requirement applies to the advertised JP2 container-validation endpoint and the ordinary jp2-compatible fixture. It is not a claim that every permissive reader must reject every nonconforming file to meet reader conformance. The available standard edition is 2019, while the project lists a 2024 target; no edition reconciliation or whole-reader conformance claim is made. That traceability limitation is retained, but does not contradict the explicit cited rule or justify weakening this clause to match the implementation. The observed deviation is specifically acceptance contrary to that clause's validation guarantee. No resource/setup error or unsupported error category masks it: the host records successful parse results for the offending inputs.

- **Production evidence:** [`src/jp2.rs:L1177-L1179`](../../src/jp2.rs#L1177-L1179)

```text
        } else {
            let (count, depth) = inspect_metadata_box(box_ref, limits, 1, u32::MAX - 1)?;
            metadata_boxes = metadata_boxes
```

- **Production evidence:** [`src/jp2.rs:L1336-L1338`](../../src/jp2.rs#L1336-L1338)

```text
    if !box_ref.header.box_type().equals(BoxType::ASSOCIATION) {
        return Ok((1, depth));
    }
```

- **Production evidence:** [`src/jp2.rs:L1219-L1222`](../../src/jp2.rs#L1219-L1222)

```text
    Ok(Jp2File {
        file_type,
        header,
        codestreams: codestream_boxes,
```

- **Production evidence:** [`SUPPORTED.md:L12`](../../SUPPORTED.md#L12)

```text
| JP2 | Container validation, typed component output, palette/channel mapping, and explicit color conversion |
```

- **Production evidence:** [`src/jp2.rs:L980-L987`](../../src/jp2.rs#L980-L987)

```text
    if !signature.header.box_type().equals(BoxType::SIGNATURE)
        || signature.header.total_length() != 12
        || signature.payload.len() != SIGNATURE_MAGIC.len()
        || signature.payload[0] != SIGNATURE_MAGIC[0]
        || signature.payload[1] != SIGNATURE_MAGIC[1]
        || signature.payload[2] != SIGNATURE_MAGIC[2]
        || signature.payload[3] != SIGNATURE_MAGIC[3]
    {
```

- **Production evidence:** [`src/jp2.rs:L1178`](../../src/jp2.rs#L1178)

```text
            let (count, depth) = inspect_metadata_box(box_ref, limits, 1, u32::MAX - 1)?;
```

- **Production evidence:** [`src/jp2.rs:L1336-L1338`](../../src/jp2.rs#L1336-L1338)

```text
    if !box_ref.header.box_type().equals(BoxType::ASSOCIATION) {
        return Ok((1, depth));
    }
```

- **Harness evidence:** [`src/jp2/spec_tests.rs:L5`](tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L5): `    let bytes = include_bytes!("../../validation/image-consumer-probe/tests/fixtures/gray.jp2");`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L68`](tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L68): `    assert!(parse(&parts.concat(), &ResourceLimits::default()).is_ok());`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L87-L95`](tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L87-L95): `    for position in 2..=4 {<br>        let mut duplicate = parts.clone();<br>        duplicate.insert(position, parts[0].clone());<br>        spec_reject(<br>            &mut failures,<br>            &duplicate.concat(),<br>            format!("second signature at box {position}"),<br>        );<br>    }`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L47-L52`](tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L47-L52): `    let result = parse(bytes, &ResourceLimits::default());<br>    spec_record(<br>        failures,<br>        matches!(result, Err(DecodeError::MalformedInput { .. })),<br>        format!("{}; malformed rejection, got {result:?}", context.into()),<br>    );`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L56-L60`](tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L56-L60): `    assert!(<br>        failures.is_empty(),<br>        "clause violations:\n{}",<br>        failures.join("\n")<br>    );`
- **Test source:** [`src/jp2/spec_tests.rs:L5`](tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L5)<br>[`src/jp2/spec_tests.rs:L68`](tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L68)<br>[`src/jp2/spec_tests.rs:L87-L95`](tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L87-L95)<br>[`src/jp2/spec_tests.rs:L47-L52`](tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L47-L52)<br>[`src/jp2/spec_tests.rs:L56-L60`](tests/DEV-2D75F3E8F6/files/src/jp2/spec_tests.rs#L56-L60)
- **Test source:** [`jp2-spec-registration.patch`](tests/DEV-2D75F3E8F6/files/jp2-spec-registration.patch)
- **Test source:** [`validation/jp2_spec_runner.py`](tests/DEV-2D75F3E8F6/files/validation/jp2_spec_runner.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-056D9FBFB2

- **Mapping:** `230760b6124854275489a5da` / `52966b2fb279def700e144d9268f1928206629fa56ae2db2f5196e85b30c8b07`
- **Specification:** `af4791b760f272348295a870`
- **Deviation:** `8ebf8aea0e804b44b4414860`
- **Test item:** `ea8d9c3e31baf68e430c41aa`
- **Case:** `1732b85cde6699859e0f568a`
- **Exact standards quote:**

> All files shall contain one and only one File Type box.

- **Entry:** `src/jp2.rs:977:jp2::parse`
- **Command:** `python3 validation/jp2_spec_runner.py spec_file_type_immediate_and_unique`
- **Setup:** A safe direct call to the actual parse implementation with a fully owned 187-byte buffer and valid default ResourceLimits. The baseline 167-byte JP2 is independently exercised through decode_jp2, yielding one 1x1 component with sample 128, and through parse successfully. Only an intact 20-byte ftyp is duplicated; all lengths, the JP2 header, and the codestream remain unchanged. The constructed input is deliberately nonconforming in the exact cardinality being validated, but the parser API has no precondition that untrusted input already conform. No internal state is fabricated and no resource limit is approached.
- **Route:** The replay applies a test-only include patch, builds the existing library tests with Cargo, then validation/jp2_spec_runner.py runs exactly jp2::tests::spec_file_type_immediate_and_unique. spec_parts splits and verifies the baseline gray.jp2 fixture. The test clones its four boxes and inserts parts[1] (the complete ftyp) at position 2, producing signature/ftyp/ftyp/jp2h/jp2c. spec_reject calls parse on the concatenated bytes with default limits and records matches!(result, Err(DecodeError::MalformedInput { .. })) as false because parse returns Ok. spec_finish asserts failures.is_empty() and panics. The same replay also observes acceptance at positions 3 and 4.
- **Observed assertion:** `second ftyp at box 2; malformed rejection, got Ok(Jp2File`

**Full diagnosis:**

The first failed check is the malformed-input rejection for 'second ftyp at box 2'; the final failures.is_empty() assertion aggregates that failure and equivalent duplicates at positions 3 and 4. Host replay reports Ok(Jp2File), not a setup failure or merely a nonzero exit. The entry is realistic: the 167-byte gray.jp2 fixture becomes 187 bytes after inserting one 20-byte ftyp, with five top-level boxes, two JP2-header children, one 1x1 8-bit component, one codestream, one compatible brand per ftyp, and metadata inspection depth 1. ResourceLimits::default() is HARDENED (2,147,483,648 input bytes, 16,777,216 metadata bytes, depth 32, 64 brands, 64 codestreams, and 1,000,000,000 work units); no large allocation, recursion, fixed pool or platform limit is approached. Real decoder interfaces also pass immutable input slices and ResourceLimits directly to jp2::parse. This is a valid direct unit-contract test: parse accepts untrusted byte slices for validation, not only already-conforming JP2 files. Deliberately duplicating a framed box is the validation target, not an inadmissible fabricated internal state. The unmodified fixture is decoded and its sample 128 verified before mutation, and the unmodified parse is checked for success. The test-only patch only includes the harness inside the existing test module. The compiler declaration confirms the slice/limits API; the MIR CFG confirms FILE_TYPE is checked for the second box only, later dispatch falls through to inspect_metadata_box (bb229-bb233), and the function ultimately returns Ok (bb282). Source inspection shows ftyp is not handled by any later dedicated branch and inspect_metadata_box immediately accepts non-association boxes. Thus the duplicate is counted as metadata, never checked for uniqueness, and a successful Jp2File is returned. The rfc_map origin was assessed rather than overridden by implementation intent: its parent finding cites T.800 (06/2019), I.5.2, and reading that section confirms both immediate placement and 'All files shall contain one and only one File Type box.' The fixture's jp2 brand and jp2 compatibility make this JP2 rule applicable. Together with the published JP2 'Container validation' scope and this clause's explicit validation requirement, the observed acceptance violates precisely the uniqueness guarantee. This does not claim the standard requires every reader to reject every malformed input, nor establish conformance to a later edition; the origin's unreconciled-edition qualification remains. There is no standards-specific contrary evidence supporting spec_error here. The malformed-error variant oracle does not affect this attribution because the actual result is Ok, which fails any rejection oracle.

- **Production evidence:** [`src/jp2.rs:L994-L1001`](../../src/jp2.rs#L994-L1001)

```text
    let file_type_box = required_box(&mut reader, 1)?;
    if !file_type_box.header.box_type().equals(BoxType::FILE_TYPE) {
        return Err(DecodeError::malformed(
            file_type_box.header.offset(),
            MalformedKind::InvalidBoxOrder,
        ));
    }
    let file_type = parse_file_type(file_type_box, limits)?;
```

- **Production evidence:** [`src/jp2.rs:L1177-L1183`](../../src/jp2.rs#L1177-L1183)

```text
        } else {
            let (count, depth) = inspect_metadata_box(box_ref, limits, 1, u32::MAX - 1)?;
            metadata_boxes = metadata_boxes
                .checked_add(count)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            metadata_depth = metadata_depth.max(depth);
        }
```

- **Production evidence:** [`src/jp2.rs:L1336-L1338`](../../src/jp2.rs#L1336-L1338)

```text
    if !box_ref.header.box_type().equals(BoxType::ASSOCIATION) {
        return Ok((1, depth));
    }
```

- **Production evidence:** [`SUPPORTED.md:L12`](../../SUPPORTED.md#L12)

```text
| JP2 | Container validation, typed component output, palette/channel mapping, and explicit color conversion |
```

- **Production evidence:** [`src/jp2.rs:L994-L995`](../../src/jp2.rs#L994-L995)

```text
    let file_type_box = required_box(&mut reader, 1)?;
    if !file_type_box.header.box_type().equals(BoxType::FILE_TYPE) {
```

- **Production evidence:** [`src/jp2.rs:L1178`](../../src/jp2.rs#L1178)

```text
            let (count, depth) = inspect_metadata_box(box_ref, limits, 1, u32::MAX - 1)?;
```

- **Production evidence:** [`src/jp2.rs:L1336-L1338`](../../src/jp2.rs#L1336-L1338)

```text
    if !box_ref.header.box_type().equals(BoxType::ASSOCIATION) {
        return Ok((1, depth));
    }
```

- **Harness evidence:** [`src/jp2/spec_tests.rs:L17`](tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L17): `    assert_eq!(parts[1], box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L27`](tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L27): `    let image = crate::decode::decode_jp2(bytes, &crate::DecodeOptions::default()).unwrap();`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L116-L124`](tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L116-L124): `    for position in 2..=4 {<br>        let mut duplicate = parts.clone();<br>        duplicate.insert(position, parts[1].clone());<br>        spec_reject(<br>            &mut failures,<br>            &duplicate.concat(),<br>            format!("second ftyp at box {position}"),<br>        );<br>    }`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L46-L53`](tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L46-L53): `fn spec_reject(failures: &mut Vec<String>, bytes: &[u8], context: impl Into<String>) {<br>    let result = parse(bytes, &ResourceLimits::default());<br>    spec_record(<br>        failures,<br>        matches!(result, Err(DecodeError::MalformedInput { .. })),<br>        format!("{}; malformed rejection, got {result:?}", context.into()),<br>    );<br>}`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L55-L61`](tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L55-L61): `fn spec_finish(failures: Vec<String>) {<br>    assert!(<br>        failures.is_empty(),<br>        "clause violations:\n{}",<br>        failures.join("\n")<br>    );<br>}`
- **Test source:** [`src/jp2/spec_tests.rs:L17`](tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L17)<br>[`src/jp2/spec_tests.rs:L27`](tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L27)<br>[`src/jp2/spec_tests.rs:L116-L124`](tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L116-L124)<br>[`src/jp2/spec_tests.rs:L46-L53`](tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L46-L53)<br>[`src/jp2/spec_tests.rs:L55-L61`](tests/DEV-056D9FBFB2/files/src/jp2/spec_tests.rs#L55-L61)
- **Test source:** [`jp2-spec-registration.patch`](tests/DEV-056D9FBFB2/files/jp2-spec-registration.patch)
- **Test source:** [`validation/jp2_spec_runner.py`](tests/DEV-056D9FBFB2/files/validation/jp2_spec_runner.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-629A0A9C39

- **Mapping:** `6f3ae182625346bf926027f1` / `396e649e213d3f7cea40f171049d2fe64d83f1d9d6ba80bc573b38660f13d4ee`
- **Specification:** `523cb72fe91c187071e962e3`
- **Deviation:** `8857d0298e2942da6070ff7c`
- **Test item:** `56c3811ca88c040d23aa7d2a`
- **Case:** `349f62632c2048a190657687`
- **Exact standards quote:**

> A conforming JP2 reader shall ignore all Colour Specification boxes after the first.

- **Entry:** `src/jp2.rs:2139:jp2::parse_jp2_header`
- **Command:** `target/jp2-header-spec-tests --exact jp2::tests::spec_header_ignores_later_colour_semantics --nocapture`
- **Setup:** The tested entry is the explicitly available direct unit contract, with initialized safe Rust byte vectors and ResourceLimits::default(), not fabricated parser internals. ihdr is 13x17, three components, unsigned depth8, compression7, known colourspace/no-IP flags. Its first colr is METH1, PREC0, APPROX0, EnumCS16 sRGB, matching three unsigned channels. All later colr boxes are contiguous and correctly framed; the reserved box has the three common fields and only permitted opaque trailing bytes. Four boxes consume at most 94 bytes and four reader work units with no recursive nesting, comfortably within all applicable default limits. The selected base=40 case is within the input limit and has no offset overflow. The pinned 2019 standard explicitly requires readers to ignore these reserved methods, so these bytes are admissible for the reader-ignore obligation even though a JP2 writer is instructed to emit METH1 or METH2. Earlier successful baseline parsing establishes the entry's unaffected metadata is accepted.
- **Route:** The host builds the additive native tests using validation/build-jp2-header-spec-tests.py and invokes target/jp2-header-spec-tests --exact jp2::tests::spec_header_ignores_later_colour_semantics --nocapture. The test successfully parses ihdr+sRGB-colr as its baseline. It appends a length-framed reserved-method colr and another sRGB-colr, then directly invokes parse_jp2_header. For base=40, method=255, extra=0, the reader accepts ihdr at relative 0 and the first colr at relative 22, then reaches the reserved colr at relative 37. Unconditional semantic parsing returns InvalidColorSpecification at absolute 77; the test records this actual Err and the final failures.is_empty() assertion fails.
- **Observed assertion:** `method=255 extra=0 base=40: expected ignored colr, got MalformedInput { offset: 77, kind: InvalidColorSpecification }`

**Full diagnosis:**

The failed check is the final assert!(failures.is_empty()) in spec_header_ignores_later_colour_semantics, not a fixture panic, build failure, or unexplained nonzero exit. The host output identifies concrete parse_jp2_header calls returning InvalidColorSpecification instead of ignoring a later colr. In particular method=255, extra=0, base=40 returns offset=77, exactly base+37, the second colr's header offset after the 22-byte ihdr and 15-byte first colr. The baseline parse and selected-sRGB assertion precede the failing variants and succeed; the negative checks also complete before the aggregated assertion.

Entry realism was checked: each invocation has a 13x17 image (221 pixels), three unsigned 8-bit components, compression=7, valid zero flags, and four flat, contiguous, correctly length-encoded boxes. Payload size is 63..94 bytes, versus default limits of 2,147,483,648 input bytes, 16,777,216 metadata bytes, 1,000,000 width/height, 100,000,000 pixels, 64 components, depth 32, and 1,000,000,000 work units. There is no recursive fixture, image-sized allocation, private-state fabrication, or aggregate resource stress. The replay has 72 sequential variants, not a single combined oversized input. Base=40 is a realistic JP2 header payload offset after signature, file-type, and superbox framing; the conclusion does not depend on base=2^32, which is diagnostic rebasing rather than allocated input. The actual container parser passes box_ref.payload, limits, and payload_offset directly to this function. This is an allowed direct unit-contract test with no declared requires, not a claimed full-file execution. Safe Rust slices and reader-produced BoxRefs satisfy the relevant callee premises; unsupported colour methods are documented rejected inputs of the semantic parser, not caller safety preconditions.

The clause origin is rfc_map. Reading its pinned T.800 (06/2019), I.5.3.3 confirms both 'A conforming JP2 reader shall ignore all Colour Specification boxes after the first' and Table I.9's explicit requirement to ignore the entire box for METH other than 1 or 2, permitting optional fields after APPROX. The fixture uses exactly this reserved-method case, with PREC=APPROX=0 and an already accepted first sRGB colr; it does not rely on malformed ICC data or truncated recognized-method data. The file-writer rule restricting METH to 1 or 2 does not negate the explicit reader obligation for other values. This diagnoses the cited 2019 reader rule, not conformance to the unreconciled project-selected 2024 edition or JPX selection rules.

Compiler declaration and MIR CFG confirm the route: after framing the later colr, bb72/bb73 dispatch to bb74 parse_color_specification; bb75/bb76 branch on its Result; error bb78 propagates through FromResidual before the color.is_none() guard in bb77/bb80 can be reached. The semantic parser's unsupported-method branch returns InvalidColorSpecification. Thus the code rejects a well-framed later reserved-method box that this JP2-reader clause requires ignoring. The output tail truncates the beginning of the aggregate report, but retains complete failing case descriptions, including the base=40 case used here; it is sufficient to identify the operation and failed predicate. No production modification or new replay is needed to interpret this host-observed failure.

- **Production evidence:** [`src/jp2.rs:L2199-L2202`](../../src/jp2.rs#L2199-L2202)

```text
let parsed_color = parse_color_specification(box_ref, base_offset)?;
            if color.is_none() {
                color = Some(parsed_color);
            }
```

- **Production evidence:** [`src/jp2.rs:L2498-L2503`](../../src/jp2.rs#L2498-L2503)

```text
    } else {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidColorSpecification,
        ));
    };
```

- **Production evidence:** [`src/jp2.rs:L1052-L1056`](../../src/jp2.rs#L1052-L1056)

```text
            parsed_header = Some(parse_jp2_header(
                box_ref.payload,
                limits,
                box_ref.header.payload_offset()?,
            )?);
```

- **Production evidence:** [`src/jp2.rs:L2199-L2202`](../../src/jp2.rs#L2199-L2202)

```text
let parsed_color = parse_color_specification(box_ref, base_offset)?;
            if color.is_none() {
                color = Some(parsed_color);
            }
```

- **Production evidence:** [`src/jp2.rs:L2501`](../../src/jp2.rs#L2501)

```text
MalformedKind::InvalidColorSpecification,
```

- **Harness evidence:** [`src/jp2_header_spec_tests.rs:L112-L115`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L112-L115): `        let mut baseline = spec_ihdr(13, 17, 3, 7);<br>        baseline.extend(spec_colr());<br>        let expected = parse_jp2_header(&baseline, &limits, base).unwrap();<br>        assert_eq!(expected.color, ColorSpecification::enumerated(16));`
- **Harness evidence:** [`src/jp2_header_spec_tests.rs:L111`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L111): `    for base in [0, 40, 1 << 32] {`
- **Harness evidence:** [`src/jp2_header_spec_tests.rs:L116-L118`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L116-L118): `        for method in [0, 4, 5, 127, 254, 255] {<br>            for extra in [0, 1, 7, 31] {<br>                let mut ignored = vec![method, 0, 0];`
- **Harness evidence:** [`src/jp2_header_spec_tests.rs:L127-L129`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L127-L129): `                let mut bytes = baseline.clone();<br>                bytes.extend(encoded);<br>                bytes.extend(spec_colr());`
- **Harness evidence:** [`src/jp2_header_spec_tests.rs:L133`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L133): `                match parse_jp2_header(&bytes, &limits, base) {`
- **Harness evidence:** [`src/jp2_header_spec_tests.rs:L140-L142`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L140-L142): `                    Err(error) => failures.push(format!(<br>                        "method={method} extra={extra} base={base}: expected ignored colr, got {error:?}"<br>                    )),`
- **Harness evidence:** [`src/jp2_header_spec_tests.rs:L162-L166`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L162-L166): `    assert!(<br>        failures.is_empty(),<br>        "JP2 reader ignore predicate violated:\n{}",<br>        failures.join("\n")<br>    );`
- **Harness evidence:** [`src/jp2_header_spec_tests.rs:L10-L12`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L10-L12): `fn spec_colr() -> Vec<u8> {<br>    box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16])<br>}`
- **Test source:** [`src/jp2_header_spec_tests.rs:L112-L115`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L112-L115)<br>[`src/jp2_header_spec_tests.rs:L111`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L111)<br>[`src/jp2_header_spec_tests.rs:L116-L118`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L116-L118)<br>[`src/jp2_header_spec_tests.rs:L127-L129`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L127-L129)<br>[`src/jp2_header_spec_tests.rs:L133`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L133)<br>[`src/jp2_header_spec_tests.rs:L140-L142`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L140-L142)<br>[`src/jp2_header_spec_tests.rs:L162-L166`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L162-L166)<br>[`src/jp2_header_spec_tests.rs:L10-L12`](tests/DEV-629A0A9C39/files/src/jp2_header_spec_tests.rs#L10-L12)
- **Test source:** [`validation/build-jp2-header-spec-tests.py`](tests/DEV-629A0A9C39/files/validation/build-jp2-header-spec-tests.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-2032F100C3

- **Mapping:** `e033943567bf66a65e7591f2` / `8107c45b6c296c4f7248b587e90bef8d034d5a77aceb71c914581d1c52606376`
- **Specification:** `e8e89fffa5faef739c2121f3`
- **Deviation:** `b836483a38f343fb15d1b5e1`
- **Test item:** `b22776d76bcd039a5c524749`
- **Case:** `85c71c4469785571f2d66fe3`
- **Exact standards quote:**

> This value shall be in the range 1 to 1024 and is encoded as a 2-byte
>                     big-endian unsigned integer.

- **Entry:** `src/jp2.rs:2535:jp2::parse_palette`
- **Command:** `cargo test --lib --locked jp2::tests::palette_contract_t800_entry_count_domain_and_complete_table -- --exact --nocapture`
- **Setup:** The assessed parser unit entry has no requires. All inputs are initialized safe Rust values with valid shared borrows. Each negative fixture has NPC=1, a supported unsigned-eight-bit descriptor, exact normal 8-byte pclr framing and a complete NE-byte table, without padding or truncation. Header fields are produced by BoxReader rather than fabricated. At NE=1025 there are 1029 payload bytes and 1037 box bytes. The largest case is 65547 box bytes, under default input/metadata limits; a single box requires one framing work unit and no recursion. Only the tested NE validation predicate is false. No callee receives an invalid precision, width, offset, or header. The sole source patch registers test code under cfg(test), preserving production behavior.
- **Route:** Host cargo test runs jp2::tests::palette_contract_t800_entry_count_domain_and_complete_table. Its negative-count loop constructs a complete pclr byte vector via contract_palette_fixture and box32, then contract_parse_palette_box obtains a genuine BoxRef through BoxReader and directly invokes parse_palette(box_ref, 0). The parser returns Ok for NE=1025 and other above-limit counts; contract_assert_retained_palette confirms actual retained entries and samples, then the test records those counts and fails wrongly_admitted.is_empty(). This is a unit-contract route, not a claimed whole-file caller reproduction.
- **Observed assertion:** `T.800 (06/2019) I.5.3.4 requires NE in 1..1024; admitted [1025, 1026, 2048, 32768, 65535]`

**Full diagnosis:**

The first failed assertion is wrongly_admitted.is_empty(), with host output: "T.800 (06/2019) I.5.3.4 requires NE in 1..1024; admitted [1025, 1026, 2048, 32768, 65535]". All preceding legal-count retention checks passed. This specifically refutes the NE admission part of clause e8e89fffa5faef739c2121f3, not table preservation. The host also explicitly logged acceptance of NE=1025, so attribution does not rest on cargo's nonzero exit alone.

Entry realism: this failing case uses one normally framed pclr box at a time, no recursion, no fabricated private state, and default ResourceLimits. Legal cases range over NE=1..1024 with three columns (eight encoded bytes per row), at most 8,206 input bytes and 3,072 i64 values per table; the 1,024 iterations total 524,800 rows, processed sequentially. Negative cases use one unsigned-eight-bit column, with box size NE+12, at most 65,547 bytes and 65,535 i64 values (524,280 bytes per retained or expected vector). These sizes are well below default limits of 2,147,483,648 input bytes, 16,777,216 metadata bytes and 1,000,000,000 framing work units; there is one box and no nesting or stack-sized table. The minimal refutation, NE=1025, needs only 1,037 input bytes. Real header callers likewise obtain framed BoxRefs and pass them directly to parse_palette without prevalidating NE; no owning caller establishes an NE<=1024 entry requirement. The supplied assessed unit_contract entry has no requires, and all downstream contracts are respected: descriptor 7 means unsigned precision 8, each sample slice has length one, offsets are zero, and header fields originate in BoxReader. Out-of-domain NE is the field this validator must reject, not an unsafe or inadmissible parser entry.

The replay patch only adds an include inside the existing cfg(test) module. The route is the replayed unit test -> contract_palette_fixture -> box32 -> contract_parse_palette_box -> BoxReader::new/next_box_ref -> unchanged parse_palette. For NE=1025,NPC=1, payload length is exactly 3+1+1025=1029. Compiler declaration evidence identifies the actual parser and callees; MIR bb13 checks entries==0 and bb14 checks columns==0, then permits the nonzero case. The supported descriptor and exact-length checks succeed, the loops decode all entries, and bb166 returns Ok(Palette). There is no upper-bound check. The harness verifies that returned palette before collecting the forbidden count and failing the range assertion.

Authority: clause_origin is rfc_map, not infer. I read its parent finding and re-read pinned T.800 (06/2019) I.5.3.4 using the origin item e033943567bf66a65e7591f2; document SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e. Both NE prose and Table I.12 expressly give 1 to 1024. This is the ordinary JP2 pclr interpretation, with no JPX reinterpretation. SUPPORTED.md advertises JP2 container validation and rejects malformed structures as extensions. Thus this is a reproduced mismatch against the explicitly named local 2019 validation contract, not grounds to revise a standards-derived clause to match the implementation. The project names a newer 2024 target and disclaims certification; this finding does not establish a 2024 conformance failure or claim the standard mandates a particular DecodeError variant. Whole-file acceptance and other palette/header constraints are not inferred from this direct unit reproduction.

- **Production evidence:** [`src/jp2.rs:L2542-L2544`](../../src/jp2.rs#L2542-L2544)

```text
let entries = read_u16([box_ref.payload[0], box_ref.payload[1]]);
    let columns = box_ref.payload[2];
    if entries == 0 || columns == 0 {
```

- **Production evidence:** [`src/jp2.rs:L2602-L2604`](../../src/jp2.rs#L2602-L2604)

```text
let sample_bytes = usize::from(entries)
        .checked_mul(bytes_per_entry)
        .ok_or_else(|| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?;
```

- **Production evidence:** [`src/jp2.rs:L2676-L2680`](../../src/jp2.rs#L2676-L2680)

```text
Ok(Palette {
        entries,
        columns,
        values,
    })
```

- **Production evidence:** [`src/jp2.rs:L2210`](../../src/jp2.rs#L2210)

```text
palette = Some(parse_palette(box_ref, base_offset)?);
```

- **Production evidence:** [`SUPPORTED.md:L12`](../../SUPPORTED.md#L12)

```text
| JP2 | Container validation, typed component output, palette/channel mapping, and explicit color conversion |
```

- **Production evidence:** [`SUPPORTED.md:L164-L165`](../../SUPPORTED.md#L164-L165)

```text
Reserved fields, contradictory
signaling, and malformed structures are not accepted as supported extensions.
```

- **Production evidence:** [`src/jp2.rs:L2542-L2544`](../../src/jp2.rs#L2542-L2544)

```text
let entries = read_u16([box_ref.payload[0], box_ref.payload[1]]);
    let columns = box_ref.payload[2];
    if entries == 0 || columns == 0 {
```

- **Production evidence:** [`src/jp2.rs:L2602-L2604`](../../src/jp2.rs#L2602-L2604)

```text
let sample_bytes = usize::from(entries)
        .checked_mul(bytes_per_entry)
        .ok_or_else(|| DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow))?;
```

- **Production evidence:** [`src/jp2.rs:L2210`](../../src/jp2.rs#L2210)

```text
palette = Some(parse_palette(box_ref, base_offset)?);
```

- **Production evidence:** [`src/jp2.rs:L2270`](../../src/jp2.rs#L2270)

```text
if palette.is_some() && component_mappings.is_empty() {
```

- **Harness evidence:** [`src/jp2_palette_contract_tests.rs:L64-L65`](tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L64-L65): `let mut reader = BoxReader::new(bytes, &ResourceLimits::default()).unwrap();<br>    let box_ref = reader.next_box_ref().unwrap().unwrap();`
- **Harness evidence:** [`src/jp2_palette_contract_tests.rs:L76-L77`](tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L76-L77): `assert!(reader.next_box_ref().is_none());<br>    parse_palette(box_ref, 0)`
- **Harness evidence:** [`src/jp2_palette_contract_tests.rs:L179`](tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L179): `for entries in [0, 1025, 1026, 2048, 32768, u16::MAX] {`
- **Harness evidence:** [`src/jp2_palette_contract_tests.rs:L192-L196`](tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L192-L196): `Ok(palette) => {<br>                contract_assert_retained_palette(&palette, entries, &columns, &expected);<br>                wrongly_admitted.push(entries);<br>                eprintln!("predicate violated: admitted forbidden NE={entries}");<br>            }`
- **Harness evidence:** [`src/jp2_palette_contract_tests.rs:L199-L202`](tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L199-L202): `assert!(<br>        wrongly_admitted.is_empty(),<br>        "T.800 (06/2019) I.5.3.4 requires NE in 1..1024; admitted {wrongly_admitted:?}"<br>    );`
- **Test source:** [`src/jp2_palette_contract_tests.rs:L64-L65`](tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L64-L65)<br>[`src/jp2_palette_contract_tests.rs:L76-L77`](tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L76-L77)<br>[`src/jp2_palette_contract_tests.rs:L179`](tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L179)<br>[`src/jp2_palette_contract_tests.rs:L192-L196`](tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L192-L196)<br>[`src/jp2_palette_contract_tests.rs:L199-L202`](tests/DEV-2032F100C3/files/src/jp2_palette_contract_tests.rs#L199-L202)
- **Test source:** [`validation/palette-contract-tests.patch`](tests/DEV-2032F100C3/files/validation/palette-contract-tests.patch)
- **Exact mapping record:** [report.json](report.json)

## DEV-3DB90259D8

- **Mapping:** `e033943567bf66a65e7591f2` / `07c8ff48f73fd66a9ceae30d8b6d16232939939ddc5e204b33bf1749d3a7a30b`
- **Specification:** `8d7860edcf7d914425decbe8`
- **Deviation:** `4e6dad2eb5607e3961b89ce6`
- **Test item:** `d558f26e6e40b685f1a86c0c`
- **Case:** `93c8c78793c85a4f283ead83`
- **Exact standards quote:**

> The values in Table I.18 specify indices that have been assigned to represent specific "colours" and do not refer to specific
> channels (or components within the codestream or palette).

- **Entry:** `src/jp2.rs:2755:jp2::parse_channel_definition`
- **Command:** `target/deepspec-cdef/cdef-tests --exact jp2::tests::spec_cdef_separates_channel_and_colour_domains --nocapture`
- **Setup:** Admissible direct unit input: genuine framed cdef BoxRef; N=3 and exact 2+6*N payload; one physical output channel with every Cni=0; supported Color type; RGB associations 1,2,3 each represented exactly once; unsigned 1x1 component, sRGB colr, no cmap/palette/MCT, one cdef after ihdr. The pinned T.800 section expressly permits multiple descriptions for one channel and separates colour indices from physical channel indices. Sizes are far below default resource limits. There is no callee precondition requiring the association predicate to return true, and no owning-caller-only state is needed by this pure parser boundary. This is not a claim of end-to-end file reachability or complete-file conformance.
- **Route:** Host compiled the test-only src/jp2.rs overlay with cargo test --lib --locked --no-run and ran the exact spec_cdef_separates_channel_and_colour_domains test. Its first loop iteration selects channels=1, constructs RGB descriptions for channel 0, calls spec_parse_rgb_cdef, frames ihdr/colr/cdef, obtains the third BoxRef via BoxReader and directly calls parse_channel_definition(boxes[2], 1, 73). Parsing the second record reaches channel_definition_valid(0,0,2,1), returns false and produces InvalidBoxContents at 73+37=110. The harness accumulates this unexpected Err and fails failures.is_empty().
- **Observed assertion:** `channels=1: RGB colour indices 1,2,3 must be retained independently of channel count: Err(MalformedInput { offset: 110, kind: InvalidBoxContents })`

**Full diagnosis:**

The first host-observed failing check is the accumulated channels=1 exact-record expectation: the parser returned InvalidBoxContents at offset 110 instead of retaining channel 0's RGB associations 1, 2 and 3. This is a valid direct unit-contract entry, not an invalid fixture or a bypass of a required owning caller. Resource realism: the first witness has a 1x1 unsigned 8-bit image, one physical component, N=3, a 20-byte cdef payload and 65-byte header payload. The failing case's largest fixture has 64 components, N=67 and a 449-byte header payload, with only three flat boxes and heap-backed vectors. Default limits are HARDENED: 64 components, 16,777,216 metadata bytes, 2,147,483,648 input bytes and depth 32. No excessive allocation, stack nesting or platform limit explains this failure. The separate passing retention case's N=65535 is not needed for this diagnosis.

The rfc_map origin was assessed using its parent finding and the complete pinned T.800 (06/2019) I.5.3.6 section, read through the origin item; its returned version matches SHA256 436b1208baef6f60f4690b3f9e2beb62f4d6c55fc1615587d8163a707993a61e. It explicitly permits multiple descriptions of one channel, defines RGB associations 1/2/3 as colour rather than channel indices, and requires every colour to be represented. The witness has exactly (0,0,1), (0,0,2), (0,0,3): every physical channel and required RGB colour is described, with unique defined type/association pairs. No palette, opacity or multiple-component transform is involved. The explicit cdef is necessary for this shared-channel mapping. This finding is conditional on that supplied edition and does not certify newer editions or full-file decoding.

The replay patch adds only tests and constructs real BoxReader-produced BoxRefs after ihdr and sRGB colr, then invokes the original parse_channel_definition with the actual physical channel count. Real parse_jp2_header callers likewise pass image.components when cmap is absent; the unit test establishes that input without needing a complete codestream. Compiler declaration confirms the three-argument unit boundary. MIR bb59 calls channel_definition_valid and bb60 takes its false edge through bb62-bb68 to InvalidBoxContents. The first record (0,0,1) passes, but (0,0,2) fails solely because 2 > output_channels=1. The error offset is base 73 plus cdef offset 37 = 110, matching the observed output. The callee accepts every u16 tuple as an input; its false result is not a caller-precondition violation. Its inferred/native model merely describes the same implementation restriction and cannot override the standards-derived clause. Consequently valid metadata is rejected before association conversion/retention. The accumulated assertion has weaker per-record diagnostics, but its exact error and the traced branch identify this deviation; this is not attribution based on exit status alone.

- **Production evidence:** [`src/refinement.rs:L1055-L1057`](../../src/refinement.rs#L1055-L1057)

```text
channel < output_channels
        && matches!(channel_type, 0 | 1 | 2 | 0xffff)
        && (association <= output_channels || association == 0xffff)
```

- **Production evidence:** [`src/jp2.rs:L2798-L2808`](../../src/jp2.rs#L2798-L2808)

```text
if !crate::refinement::channel_definition_valid(
            channel,
            channel_type,
            association,
            output_channels,
        ) {
            return Err(DecodeError::malformed(
                add_offset(base_offset, box_ref.header.offset())?,
                MalformedKind::InvalidBoxContents,
            ));
        }
```

- **Production evidence:** [`src/jp2.rs:L2238-L2244`](../../src/jp2.rs#L2238-L2244)

```text
let output_channels = if component_mappings.is_empty() {
                image.components
            } else {
                u16::try_from(component_mappings.len()).map_err(|_| {
                    DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
                })?
            };
```

- **Production evidence:** [`src/refinement.rs:L1055-L1057`](../../src/refinement.rs#L1055-L1057)

```text
channel < output_channels
        && matches!(channel_type, 0 | 1 | 2 | 0xffff)
        && (association <= output_channels || association == 0xffff)
```

- **Production evidence:** [`src/jp2.rs:L2238-L2244`](../../src/jp2.rs#L2238-L2244)

```text
let output_channels = if component_mappings.is_empty() {
                image.components
            } else {
                u16::try_from(component_mappings.len()).map_err(|_| {
                    DecodeError::malformed(base_offset, MalformedKind::ArithmeticOverflow)
                })?
            };
```

- **Production evidence:** [`src/jp2.rs:L2798-L2803`](../../src/jp2.rs#L2798-L2803)

```text
if !crate::refinement::channel_definition_valid(
            channel,
            channel_type,
            association,
            output_channels,
        ) {
```

- **Production evidence:** [`src/color.rs:L1123`](../../src/color.rs#L1123)

```text
if colors[slot_index] != usize::MAX && colors[slot_index] != component_index {
```

- **Production evidence:** [`src/color.rs:L1137`](../../src/color.rs#L1137)

```text
*colors.get_mut(slot_index).ok_or_else(invalid_color_input)? = component_index;
```

- **Harness evidence:** [`cdef-spec-tests.patch:L156-L162`](tests/DEV-3DB90259D8/files/cdef-spec-tests.patch#L156-L162): `+        for channels in [1_u16, 2, 3, 4, 8, 64] {<br>+            // Sharing all RGB colours makes cdef necessary even with one channel.<br>+            let mut descriptions = vec![<br>+                (0, ChannelType::Color, ChannelAssociation::Color(1)),<br>+                (0, ChannelType::Color, ChannelAssociation::Color(2)),<br>+                (0, ChannelType::Color, ChannelAssociation::Color(3)),<br>+            ];`
- **Harness evidence:** [`cdef-spec-tests.patch:L41-L47`](tests/DEV-3DB90259D8/files/cdef-spec-tests.patch#L41-L47): `+        let mut header = box32(*b"ihdr", &ihdr);<br>+        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16]));<br>+        header.extend(box32(*b"cdef", &payload));<br>+        let boxes = BoxReader::new(&header, &ResourceLimits::default())<br>+            .unwrap()<br>+            .collect::<Result<Vec<_>, _>>()<br>+            .unwrap();`
- **Harness evidence:** [`cdef-spec-tests.patch:L60`](tests/DEV-3DB90259D8/files/cdef-spec-tests.patch#L60): `+        parse_channel_definition(boxes[2], channels, 73)`
- **Harness evidence:** [`cdef-spec-tests.patch:L225`](tests/DEV-3DB90259D8/files/cdef-spec-tests.patch#L225): `+        assert!(failures.is_empty(), "{}", failures.join("\n"));`
- **Test source:** [`cdef-spec-tests.patch:L156-L162`](tests/DEV-3DB90259D8/files/cdef-spec-tests.patch#L156-L162)<br>[`cdef-spec-tests.patch:L41-L47`](tests/DEV-3DB90259D8/files/cdef-spec-tests.patch#L41-L47)<br>[`cdef-spec-tests.patch:L60`](tests/DEV-3DB90259D8/files/cdef-spec-tests.patch#L60)<br>[`cdef-spec-tests.patch:L225`](tests/DEV-3DB90259D8/files/cdef-spec-tests.patch#L225)
- **Exact mapping record:** [report.json](report.json)

## DEV-457ACAB2C3

- **Mapping:** `230760b6124854275489a5da` / `0e16f2d0fb4270b98799f9c5dc3a4834edb70843867633257c626b3901385e34`
- **Specification:** `6137c299a688f80716db60bc`
- **Deviation:** `db8bb16a144e122cd7532716`
- **Test item:** `ea8d9c3e31baf68e430c41aa`
- **Case:** `e68962b8c90cb4b6836ef844`
- **Exact standards quote:**

> When
> displaying the image, a conforming reader shall ignore all codestreams after the first codestream found in the file.
> Contiguous Codestream boxes may be found anywhere in the file except before the JP2 Header box.

- **Entry:** `src/jp2.rs:977:jp2::parse`
- **Command:** `python3 validation/jp2_spec_runner.py spec_later_jp2_codestreams_preserve_first_display`
- **Setup:** Public parse accepts immutable bytes and a validated ResourceLimits without requiring an owning caller or private object invariants. The test uses ResourceLimits::default().with_maximum_codestreams(8).unwrap(), a 283-byte ordinary JP2 with two complete separately decoded repository codestreams, canonical signature/ftyp/header, valid lengths and legal post-header placement. Its first gray codestream matches the JP2 image header. The second complete RGB stream is intentionally distinct so first-stream display preservation is meaningful; T.800 I.5.4 says subsequent streams are ignored for display. Two streams are within eight, and all byte, depth, metadata, geometry, and work demands are small. No fixture setup assertion failed in host replay.
- **Route:** The replay runner builds the existing crate's lib-test binary with only a cfg(test) include registration, verifies the exact named Rust test is registered, and invokes jp2::tests::spec_later_jp2_codestreams_preserve_first_display. spec_parts checks and successfully decodes the repository gray JP2; the test also successfully decodes the RGB JP2. For count=2 it concatenates signature, ordinary JP2 ftyp, matching gray jp2h, gray jp2c, an opaque 9-byte box, RGB jp2c, and another opaque box. Its direct parse(&bytes, &limits) reaches the second jp2c at offset 176 and returns InvalidBoxContents. spec_record prints the failed acceptance predicate; spec_finish subsequently reports it in the aggregate panic. The decode_jp2 check reaches the same parser rejection via parse_and_inspect.
- **Observed assertion:** `jp2::parse: ordinary JP2 accepts 2 complete well-framed codestreams within limit 8; Err(MalformedInput { offset: 176, kind: InvalidBoxContents }): false`

**Full diagnosis:**

The first failed predicate is 'ordinary JP2 accepts 2 complete well-framed codestreams within limit 8', reporting InvalidBoxContents at offset 176. This is a valid direct unit-contract entry to public parse(&[u8], &ResourceLimits), not a fabricated internal state or a fixture bypassing an owning caller. Resource realism: the two-stream input is 283 bytes: 77 bytes of signature/ftyp/jp2h, a 90-byte gray jp2c, a 9-byte opaque box, a 98-byte RGB jp2c, and another 9-byte opaque box. The largest admitted sweep input is 925 bytes with eight streams and eight shallow opaque boxes; the nine-stream negative control is 887 bytes. These are far below the 2 GiB input, 16 MiB metadata, depth 32, and one-billion work-unit ceilings. The configured stream limit is explicitly eight. Each fixture is a complete repository 1x1 codestream; the replay first successfully decodes the gray and RGB originals and checks their samples. The first stream and JP2 header match; subsequent standalone complete RGB streams need not determine the first image's component count. The signature, ordinary jp2 brand/compatibility, header placement and box extents are asserted by the replayed harness. No large allocation, excessive nesting, private-state manipulation, or hidden pool/stack limit explains this small input. The real decode_jp2 caller passes the same input and limits through parse_and_inspect, so this also blocks its ordinary JP2 display route.

Authority is the rfc_map origin, not agreement with existing implementation/oracles. I read its parent finding and retrieved T.800 (06/2019) I.5.4 using read_rfc_section under origin item 230760b6124854275489a5da. It expressly requires readers displaying an image to ignore codestreams after the first and permits contiguous codestream boxes anywhere except before jp2h. These fixtures meet that placement and complete-codestream requirement. SUPPORTED.md advertises JP2 typed output; its explicit no-implicit-first-selection exclusion is for JPX, not ordinary JP2. This supports the clause rather than a spec_error. Qualification: the supplied document is the 2019 edition, whereas project references select 2024; no edition reconciliation or whole-reader conformance claim is made here.

Compiler declaration and MIR CFG confirm the specific route: signature and ftyp validation; JP2 header parsed; first jp2c pushed; opaque box inspected; second jp2c at 176 computes actual=2. MIR bb217 takes the within-limit branch to bb222; actual>1 leads through bb223/bb224 with is_jpx_compatible=false, then bb225-bb227 constructs and returns InvalidBoxContents before bb228 can push that stream. This exact source branch violates the clause's admitted later-box acceptance and consequently prevents first-stream display. The displayed-output failure is downstream propagation of this parser error, not independent evidence that the later decode_jp2 count guard executed. The ninth-stream assertion is masked by the same earlier rejection and does not establish a separate resource-admission defect. The aggregate panic/exit alone is not the evidence; the first explicitly printed failed predicate and its matching source route are.

- **Production evidence:** [`src/jp2.rs:L1166-L1171`](../../src/jp2.rs#L1166-L1171)

```text
            if actual > 1 && !file_type.is_jpx_compatible() {
                return Err(DecodeError::malformed(
                    box_ref.header.offset(),
                    MalformedKind::InvalidBoxContents,
                ));
            }
```

- **Production evidence:** [`src/jp2.rs:L1079-L1085`](../../src/jp2.rs#L1079-L1085)

```text
            if actual > u64::from(limits.maximum_codestreams()) {
                return Err(DecodeError::limit(
                    Resource::Codestreams,
                    u64::from(limits.maximum_codestreams()),
                    actual,
                ));
            }
```

- **Production evidence:** [`src/decode.rs:L3895-L3896`](../../src/decode.rs#L3895-L3896)

```text
pub fn decode_jp2(input: &[u8], options: &DecodeOptions) -> Result<TypedImage, DecodeError> {
    let inspected = jp2::parse_and_inspect(input, options.limits())?;
```

- **Production evidence:** [`SUPPORTED.md:L12`](../../SUPPORTED.md#L12)

```text
| JP2 | Container validation, typed component output, palette/channel mapping, and explicit color conversion |
```

- **Production evidence:** [`src/jp2.rs:L1166-L1171`](../../src/jp2.rs#L1166-L1171)

```text
            if actual > 1 && !file_type.is_jpx_compatible() {
                return Err(DecodeError::malformed(
                    box_ref.header.offset(),
                    MalformedKind::InvalidBoxContents,
                ));
            }
```

- **Production evidence:** [`src/decode.rs:L3895-L3896`](../../src/decode.rs#L3895-L3896)

```text
pub fn decode_jp2(input: &[u8], options: &DecodeOptions) -> Result<TypedImage, DecodeError> {
    let inspected = jp2::parse_and_inspect(input, options.limits())?;
```

- **Production evidence:** [`src/decode.rs:L3904-L3906`](../../src/decode.rs#L3904-L3906)

```text
    if inspected.file().codestream_count() != 1 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
```

- **Harness evidence:** [`src/jp2/spec_tests.rs:L308-L310`](tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs#L308-L310): `    let limits = ResourceLimits::default()<br>        .with_maximum_codestreams(8)<br>        .unwrap();`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L312-L323`](tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs#L312-L323): `    for count in [1, 2, 3, 7, 8] {<br>        let mut input = parts[..3].to_vec();<br>        for index in 0..count {<br>            input.push(if index == 0 {<br>                parts[3].clone()<br>            } else {<br>                rgb[77..].to_vec()<br>            });<br>            input.push(box32(*b"zzzz", &[0xff]));<br>        }<br>        let bytes = input.concat();<br>        let result = parse(&bytes, &limits);`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L326-L328`](tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs#L326-L328): `            result.as_ref().is_ok_and(\|file\| {<br>                file.codestream_count() >= 1 && file.codestreams()[0].data() == &parts[3][8..]<br>            }),`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L16-L17`](tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs#L16-L17): `    assert_eq!(parts[0], b"\0\0\0\x0cjP  \r\n\x87\n");<br>    assert_eq!(parts[1], box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));`
- **Test source:** [`src/jp2/spec_tests.rs:L308-L310`](tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs#L308-L310)<br>[`src/jp2/spec_tests.rs:L312-L323`](tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs#L312-L323)<br>[`src/jp2/spec_tests.rs:L326-L328`](tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs#L326-L328)<br>[`src/jp2/spec_tests.rs:L16-L17`](tests/DEV-457ACAB2C3/files/src/jp2/spec_tests.rs#L16-L17)
- **Test source:** [`jp2-spec-registration.patch`](tests/DEV-457ACAB2C3/files/jp2-spec-registration.patch)
- **Test source:** [`validation/jp2_spec_runner.py`](tests/DEV-457ACAB2C3/files/validation/jp2_spec_runner.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-8825E34C8B

- **Mapping:** `a8d4c31de8ce4de63071ab43` / `8078fc03b942058033407d74f52f5d5b370e86358354500eb055fa290a70d957`
- **Specification:** `32aa5ed6bcd40eed56feecfe`
- **Deviation:** `ef3353983851a3fec301a138`
- **Test item:** `2bbb4b7f6d9814081dc6d341`
- **Case:** `efad0ebad5f0e363d0ab2122`
- **Exact standards quote:**

> Tile-part NLT > Tile-part NLT default > Main NLT > Main NLT default

- **Entry:** `src/codestream.rs:1801:codestream::Part2Extensions::nonlinear_transform_for`
- **Command:** `target/nonlinear-lookup-contract --exact codestream::tests::nonlinear_lookup_standard_header_precedence --nocapture`
- **Setup:** Parser-created owned receiver; valid component 0 of three; first and only tile-part; unique per-header selectors; bounded native 64x32 fixture; Rsiz=0x8200; legal Tnlt=2 LUT payloads and marker lengths. No production modification, manual merged-registry construction, skipped owning parser, or violation of subject requires. This establishes admissible unit entry and selection failure, not full decoder execution.
- **Route:** Host build includes validation/nonlinear_lookup_contract.rs only inside native codestream::tests and restores tests.rs. Native test constructs capability-signalled byte streams, calls parse_layout, then directly invokes nonlinear_transform_for on layout.tile_parts[0].part2_extensions. In the quoted replayed combination, main Some(0)/Some(2) descriptors have precision 9, tile Some(2) replaces that selector with precision 11, and tile None is appended at precision 10. The inherited Some(0) survives and the first specific scan returns it for component 0 before considering tile None. The independent header-based oracle expects None/precision 10, collects the mismatch, and the final empty-failures assertion fails.
- **Observed assertion:** `parse_layout -> first tile extensions -> nonlinear_transform_for: Rsiz=0x8200, main=[(Some(0), 9), (Some(2), 9)], tile=[(Some(2), 11), (None, 10)], component=0, expected=Some(NonlinearPointTransform { component: None, precision: 10, signed: false, transform_type: 2, parameters: [0, 1, 0, 0, 0, 0, 255, 255, 255, 255, 8, 0, 255], d_min: Some(0), d_max: Some(4294967295), point_bits: Some(8), points: [0, 255], gamma_parameters: None }), actual=Some(NonlinearPointTransform { component: Some(0), precision: 9, signed: false, transform_type: 2, parameters: [0, 1, 0, 0, 0, 0, 255, 255, 255, 255, 8, 0, 255], d_min: Some(0), d_max: Some(4294967295), point_bits: Some(8), points: [0, 255], gamma_parameters: None })`

**Full diagnosis:**

The failed check is the standards case's final assert!(failures.is_empty()), after parsing and owner-preservation checks and the 192-observation count. Its recorded mismatches show a main-header component-specific precision-9 descriptor returned instead of the tile-default precision-10 descriptor. This violates the standards-precedence half of clause 32aa5ed6bcd40eed56feecfe, not its accurately documented local specific-before-default rule. I read clause_origin (rfc_map) and re-read T.801 (06/2021) A.3.10 and A.2.1 using origin a8d4c31de8ce4de63071ab43: A.3.10 explicitly says 'Tile-part NLT > Tile-part NLT default > Main NLT > Main NLT default', and A.2.1 confirms Rsiz=0x8200 signals Part 2 and NLT without CAP. The fixture meets the applicable conditions: main and first tile-part headers only, unique selectors per header, component indices 0 and 2 within three components, optional 65535 default, legal unsigned depths 8–11, Tnlt=2 with two LUT values and Lnlt=19, Dmin=0/Dmax=4294967295 and PTval=8. Before tracing the implementation I checked entry scale: each of the 64 independently parsed inputs has one 64x32 image (2048 pixels, 6144 component samples), three components, one tile/first tile-part, one layer/resolution, three empty packet headers, at most six 21-byte NLT segments, and at most 216 input bytes. These are far below default limits (2 GiB input, 16 MiB metadata, 100 million pixels, 64 components, 16384 tiles/parts); no deep recursion or fabricated pool/stack state is involved. Real collect_nonlinear_regions also takes the tile's parser-produced extensions and queries the same component indices. This is a valid direct unit-contract call with no extra subject requires, not a claim that the harness executes collect_nonlinear_regions or reconstructs pixels. The owning parser is actually run: first tile extensions inherit main extensions; parse_nlt replaces only matching Option<u16> selectors, so a tile default leaves inherited main specifics intact. describe_declaration and the MIR CFG confirm the lookup's first scan returns an inherited specific through bb4 -> bb5 -> bb6 -> bb19, never reaching the default scan. Consequence: the selected descriptor/precision comes from the wrong header. The output tail truncates the panic heading and earliest mismatch, so I do not identify the first failing combination or claim a mismatch count; it nevertheless retains complete expected/actual route records and the failing native test result. Local-model agreement does not invalidate this standards-derived requirement. Edition scope remains the acquired June 2021 T.801 text, not uninspected newer editions; no numerical LUT or end-to-end image deviation is asserted.

- **Production evidence:** [`src/codestream.rs:L4642-L4645`](../../src/codestream.rs#L4642-L4645)

```text
                    let inherited_part2_extensions = match tile_part2_extensions.get(tile_index) {
                        Some(Some(extensions)) => extensions.clone(),
                        _ => part2_extensions.clone(),
                    };
```

- **Production evidence:** [`src/codestream.rs:L5645-L5646`](../../src/codestream.rs#L5645-L5646)

```text
            if extensions.nonlinear_transforms[position].component == replacement.component {
                extensions.nonlinear_transforms[position] = replacement;
```

- **Production evidence:** [`src/codestream.rs:L5658`](../../src/codestream.rs#L5658)

```text
        extensions.nonlinear_transforms.push(replacement);
```

- **Production evidence:** [`src/codestream.rs:L1817-L1820`](../../src/codestream.rs#L1817-L1820)

```text
            let transform = &self.nonlinear_transforms[position];
            if transform.component == Some(component) {
                return Some(transform);
            }
```

- **Production evidence:** [`src/decode.rs:L3846-L3848`](../../src/decode.rs#L3846-L3848)

```text
                if tile_part.tile_index == tile_index {
                    extensions = &tile_part.part2_extensions;
                }
```

- **Production evidence:** [`src/decode.rs:L3856`](../../src/decode.rs#L3856)

```text
            if let Some(transform) = extensions.nonlinear_transform_for(component) {
```

- **Production evidence:** [`src/codestream.rs:L4642-L4645`](../../src/codestream.rs#L4642-L4645)

```text
                    let inherited_part2_extensions = match tile_part2_extensions.get(tile_index) {
                        Some(Some(extensions)) => extensions.clone(),
                        _ => part2_extensions.clone(),
                    };
```

- **Production evidence:** [`src/codestream.rs:L10086-L10087`](../../src/codestream.rs#L10086-L10087)

```text
                    } else if segment.marker == NLT {
                        self.parse_nlt(segment, components, &mut part2_extensions)?;
```

- **Production evidence:** [`src/codestream.rs:L5645-L5646`](../../src/codestream.rs#L5645-L5646)

```text
            if extensions.nonlinear_transforms[position].component == replacement.component {
                extensions.nonlinear_transforms[position] = replacement;
```

- **Production evidence:** [`src/codestream.rs:L5658`](../../src/codestream.rs#L5658)

```text
        extensions.nonlinear_transforms.push(replacement);
```

- **Production evidence:** [`src/codestream.rs:L1817-L1820`](../../src/codestream.rs#L1817-L1820)

```text
            let transform = &self.nonlinear_transforms[position];
            if transform.component == Some(component) {
                return Some(transform);
            }
```

- **Production evidence:** [`src/decode.rs:L3846-L3848`](../../src/decode.rs#L3846-L3848)

```text
                if tile_part.tile_index == tile_index {
                    extensions = &tile_part.part2_extensions;
                }
```

- **Production evidence:** [`src/decode.rs:L3856`](../../src/decode.rs#L3856)

```text
            if let Some(transform) = extensions.nonlinear_transform_for(component) {
```

- **Harness evidence:** [`validation/nonlinear_lookup_contract.rs:L79`](tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L79): `            bytes[6..8].copy_from_slice(&0x8200_u16.to_be_bytes());`
- **Harness evidence:** [`validation/nonlinear_lookup_contract.rs:L85`](tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L85): `            let layout = parse_layout(&bytes, &limits).expect("bounded, capability-signalled NLT header fixture must parse");`
- **Harness evidence:** [`validation/nonlinear_lookup_contract.rs:L91`](tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L91): `            let owner = &layout.tile_parts[0].part2_extensions;`
- **Harness evidence:** [`validation/nonlinear_lookup_contract.rs:L94-L97`](tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L94-L97): `                let expected_entry = tile.iter().find(\|(s, _)\| *s == Some(component))<br>                    .or_else(\|\| tile.iter().find(\|(s, _)\| s.is_none()))<br>                    .or_else(\|\| main.iter().find(\|(s, _)\| *s == Some(component)))<br>                    .or_else(\|\| main.iter().find(\|(s, _)\| s.is_none()));`
- **Harness evidence:** [`validation/nonlinear_lookup_contract.rs:L99`](tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L99): `                let actual = owner.nonlinear_transform_for(component);`
- **Harness evidence:** [`validation/nonlinear_lookup_contract.rs:L109`](tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L109): `    assert!(failures.is_empty(), "NLT header precedence violations: {failures:#?}");`
- **Test source:** [`validation/nonlinear_lookup_contract.rs:L79`](tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L79)<br>[`validation/nonlinear_lookup_contract.rs:L85`](tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L85)<br>[`validation/nonlinear_lookup_contract.rs:L91`](tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L91)<br>[`validation/nonlinear_lookup_contract.rs:L94-L97`](tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L94-L97)<br>[`validation/nonlinear_lookup_contract.rs:L99`](tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L99)<br>[`validation/nonlinear_lookup_contract.rs:L109`](tests/DEV-8825E34C8B/files/validation/nonlinear_lookup_contract.rs#L109)
- **Test source:** [`validation/build_nonlinear_lookup_contract.py`](tests/DEV-8825E34C8B/files/validation/build_nonlinear_lookup_contract.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-49C7BAFDE3

- **Mapping:** `b091a9c63532ebcacba21070` / `a72486651c3b6f2a46bb7fdec2e050397216c308f97f94dbb365476614475380`
- **Specification:** `7ca4daea027df926ca127cca`
- **Deviation:** `f750644bce87325996e9be31`
- **Test item:** `a352b8922cbf51067edebe9c`
- **Case:** `87e93cf11efec2e559bb6034`
- **Exact standards quote:**

> If the decorrelation
> transformation array index is not zero, then the referenced MCT marker segment contains (N + 1) ⋅ N elements.

- **Entry:** `src/codestream.rs:7676:codestream::Parser::resolve_mct_collection`
- **Command:** `bash -c for bin in target/deepspec-mct/debug/deps/jpeg2000_rs-*; do if test -x "$bin" && ! test -d "$bin"; then exec "$bin" --exact codestream::tests::resolve_mct_collection_clause_7ca4daea --nocapture; fi; done; exit 2`
- **Setup:** A valid direct unit entry, not a claimed complete codestream. Parser is initialized with empty input and HARDENED limits through new_parser; parse_mct accepts explicit complete unfragmented segments with Zmct=Ymct=0. For the quoted case there are six unique input components and six permuted unique output components, matching nonzero decorrelation index 7, no offsets selected, 42 Int16 integer coefficients, zero fractional precision, scale factors 2 and final scale 4. These satisfy the retrieved 2021 J.3.1.1.3 conditions and native caller/reference/lookup contracts. Six components and 42 elements are far below caps of 64 and 4096, respectively; all tested sizes remain within those caps. The resolver does not require the parser input to contain the separately supplied arrays, nor an N*N array as an entry premise.
- **Route:** Host build includes resolve_mct_contract.rs in the native codestream::tests module and runs --exact codestream::tests::resolve_mct_collection_clause_7ca4daea. After the existing graph-model sweep returns, the test constructs and parses Int16 MCT arrays, builds a reversible Decorrelation reference, and calls parser.resolve_mct_collection(&reference, &arrays). For N=6 with offset_index=0 and transform_index=7, lookup succeeds, expected_mct_elements returns 36 instead of the normative 42, and invalid_matrix_dimensions causes InvalidTransform. The positive actual-versus-expected check appends the quoted discrepancy, causing the final failures.is_empty() assertion to fail.
- **Observed assertion:** `direct resolve_mct_collection: J.3.1.1.3 N=6, integer coefficients=42, offsets=false: expected full collection, got Err(MalformedInput { offset: 0, kind: InvalidTransform })`

**Full diagnosis:**

This is a reproduced unit-contract code deviation for the standards-derived meaning of clause 7ca4daea027df926ca127cca, not an instruction to rewrite its oracle to match the Verus model. I read clause_origin (rfc_map b091a9c63532ebcacba21070, finding a72486651c3b6f2a46bb7fdec2e050397216c308f97f94dbb365476614475380) and retrieved its original T.801 (06/2021) J.3.1.1.3 section using read_rfc_section with the origin item. Outside the informative factorization note, it explicitly requires (N+1)*N coefficients for a nonzero reversible decorrelation array index, equal nonempty input/output cardinalities, integer coefficients and power-of-two scale factors. The positive harness meets these conditions. Agreement of the native model test and implementation on N*N cannot override this normative count. Qualification: project REFERENCES.md selects the 2023 edition and explicitly leaves its reconciliation with the acquired 2021 edition open. This diagnosis establishes the admission mismatch against the cited 2021 authority; it does not assert verified 2023 conformance differences or complete numerical transform correctness.

Realism was assessed before pursuing the branch: dimensions are only 1..8,16,31,63, bounded by HARDENED.maximum_components=64; the largest positive array is 4032 elements, below parse_mct's 4096-element cap. Its Int16 marker is 8074 bytes including marker/length/header, well below the 16-bit marker length limit. The largest wrong-length probe is 4033 elements, also below the cap. Allocations are small heap vectors, cases are sequential, and there is no recursive fixture. The real decode_mcc_collection caller accepts the same reversible decorrelation variant with equal unique component lists and transform index 7, offset index 0 or 8; resolve_mct_stage forwards those references and arrays without imposing an N*N entry precondition. The direct resolver is a valid unit entry: new_parser(&[], &limits) is valid, parse_mct explicitly receives complete legal segments independent of parser.input, arrays have unique matching indices/kinds, and fraction_bits=0. This is not a fixture requiring an uncalled owner to establish hidden state. The earlier reused broad native model sweep includes synthetic states, but it returns successfully; those states are not the failing positive setup.

The failing assertion is the final failures.is_empty() aggregate: positive checks append a discrepancy when actual != the full expected collection. The host tail truncates the earliest discrepancy, so I do not invent its N value; an exact retained positive discrepancy is N=6, offsets=false, 42 integer coefficients returning InvalidTransform. Compiler declaration/CFG and source establish the route: resolve_mct_collection accepts rows=columns=6, uses no offsets, finds/clones decorrelation array 7, then calls expected_mct_elements(6,6,Decorrelation,true). Its first Decorrelation branch ignores reversible and returns 36. MIR bb32 calls the helper, bb38-bb39 compute 42 != 36, and bb44 takes bb48-bb49 to InvalidTransform. The required complete array is therefore rejected. The output also records acceptance of N*N arrays, corroborating the admission boundary, but attribution rests on the valid positive case, not potentially invalid truncated coefficient fixtures. Fractional rejection at normative count may be masked by the dimension guard and is not an independently established precision deviation. No full codestream/profile reachability or numerical reconstruction result is claimed, and no production edits or new replay were needed.

- **Production evidence:** [`src/codestream.rs:L7771-L7776`](../../src/codestream.rs#L7771-L7776)

```text
if mct_array_kinds_equal(kind, MctArrayKind::Decorrelation) {
            match rows.checked_mul(columns) {
                Some(value) => Ok(value),
                None => Err(self.malformed_at(0, MalformedKind::ArithmeticOverflow)),
            }
        } else if reversible {
```

- **Production evidence:** [`src/codestream.rs:L7717-L7722`](../../src/codestream.rs#L7717-L7722)

```text
let expected_elements =
                    self.expected_mct_elements(rows, columns, kind, reversible)?;
                let invalid_matrix_dimensions = match &matrix {
                    Some(array) => array.values.len() != expected_elements,
                    None => false,
                };
```

- **Production evidence:** [`src/codestream.rs:L7727-L7732`](../../src/codestream.rs#L7727-L7732)

```text
if invalid_offset_dimensions
                    || invalid_matrix_dimensions
                    || (reversible && (floating_matrix || floating_offsets))
                {
                    return Err(self.malformed_at(0, MalformedKind::InvalidTransform));
                }
```

- **Production evidence:** [`src/codestream.rs:L7717-L7722`](../../src/codestream.rs#L7717-L7722)

```text
let expected_elements =
                    self.expected_mct_elements(rows, columns, kind, reversible)?;
                let invalid_matrix_dimensions = match &matrix {
                    Some(array) => array.values.len() != expected_elements,
                    None => false,
                };
```

- **Production evidence:** [`src/codestream.rs:L7771-L7776`](../../src/codestream.rs#L7771-L7776)

```text
if mct_array_kinds_equal(kind, MctArrayKind::Decorrelation) {
            match rows.checked_mul(columns) {
                Some(value) => Ok(value),
                None => Err(self.malformed_at(0, MalformedKind::ArithmeticOverflow)),
            }
        } else if reversible {
```

- **Harness evidence:** [`src/codestream/resolve_mct_contract.rs:L9-L10`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L9-L10): `let limits = ResourceLimits::HARDENED;<br>    let parser = new_parser(&[], &limits).unwrap();`
- **Harness evidence:** [`src/codestream/resolve_mct_contract.rs:L19-L20`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L19-L20): `let count = (n + 1) * n;<br>        assert!(count <= usize::from(limits.maximum_components()).pow(2));`
- **Harness evidence:** [`src/codestream/resolve_mct_contract.rs:L26-L27`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L26-L27): `for row in 0..n { coefficients[row * n + n - 1 - row] = 2; }<br>        coefficients[count - 1] = 4;`
- **Harness evidence:** [`src/codestream/resolve_mct_contract.rs:L39`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L39): `parser.parse_mct(Segment { marker: MCT, offset: 0, payload: &bytes[4..] }).unwrap()`
- **Harness evidence:** [`src/codestream/resolve_mct_contract.rs:L65`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L65): `kind: MctArrayKind::Decorrelation, reversible: true, transform_index: 7, offset_index: 0,`
- **Harness evidence:** [`src/codestream/resolve_mct_contract.rs:L76-L80`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L76-L80): `let actual = parser.resolve_mct_collection(&reference, &arrays);<br>            checks += 1;<br>            if actual != expected {<br>                failures.push(format!("direct resolve_mct_collection: J.3.1.1.3 N={n}, integer coefficients={count}, offsets={with_offsets}: expected full collection, got {actual:?}"));<br>            }`
- **Harness evidence:** [`src/codestream/resolve_mct_contract.rs:L113`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L113): `assert!(failures.is_empty(), "{} discriminating checks failed:\n{}", failures.len(), failures.join("\n"));`
- **Test source:** [`src/codestream/resolve_mct_contract.rs:L9-L10`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L9-L10)<br>[`src/codestream/resolve_mct_contract.rs:L19-L20`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L19-L20)<br>[`src/codestream/resolve_mct_contract.rs:L26-L27`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L26-L27)<br>[`src/codestream/resolve_mct_contract.rs:L39`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L39)<br>[`src/codestream/resolve_mct_contract.rs:L65`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L65)<br>[`src/codestream/resolve_mct_contract.rs:L76-L80`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L76-L80)<br>[`src/codestream/resolve_mct_contract.rs:L113`](tests/DEV-49C7BAFDE3/files/src/codestream/resolve_mct_contract.rs#L113)
- **Exact mapping record:** [report.json](report.json)

## DEV-61C38E5752

- **Mapping:** `de8b2b27d2f3b6d75bb83616` / `e799a251e3d5b349ed1009dbb9ff819e99a4d3a4fc51180aae6c9fbdb84f6300`
- **Specification:** `fb7222713c5ba9b4fdbf5e39`
- **Deviation:** `a9caca3ed72661d73b6269c1`
- **Test item:** `19e89c8564c4bcac4d2588b1`
- **Case:** `43212f9b85589508f443ca6f`
- **Exact standards quote:**

> This annex specifies three non-linear point transformations that are used after the decoding processes and inverse multiple
> component transformations to map reconstructed values back to their proper range.

- **Entry:** `src/decode.rs:3698:decode::typed_layout`
- **Command:** `target/typed-layout-contract decode::typed_layout_contract_tests::typed_layout_inverse_nlt_after_reconstruction_preserves_geometry --exact --nocapture`
- **Setup:** A bounded valid direct unit entry, not an entropy-decoding claim: 3 equal four-bit source components, equal subsampling, one tile, matching selected reconstruction bounds and sample counts, ResourceLimits::HARDENED. Component 0's type-2 NLT has legal precision/sign, two points [255,0], PTval8, Dmin0 and Dmax=u32::MAX (normalized 0 and1); the NLT capability is signaled and main-header metadata is inherited into the only tile. The quoted route uses fraction_bits16 with exact integer-centered reconstruction and inverse_rct=false. Source rank0..15 corresponds to signed -8..7, entirely inside the source range. Existing assembly and typed conversion perform their native checks. Test access is a cfg(test) overlay that includes decode.rs unchanged.
- **Route:** The replay runs decode::typed_layout_contract_tests::typed_layout_inverse_nlt_after_reconstruction_preserves_geometry. It parses signaled Part-2 headers, constructs a matching full/cropped reconstructed image through assemble_selected_image, optionally applies native inverse RCT, and directly invokes the unchanged typed_layout. On the quoted signed-source/unsigned-target, Q16, non-RCT case, typed conversion produces -8..7, region collection selects the inherited component-0 type-2 LUT, apply_nonlinear_regions calls inverse_nlt_sample → inverse_nlt_lut_sample → nlt_lut_position → nlt_lut_coordinates. The returned samples are compared to an independent K-7..K-9 oracle; discrepancies make the terminal assertion fail.
- **Observed assertion:** `typed_layout source_signed=true target_signed=false region=[3, 1, 35, 9] subsampling=1 fraction_bits=16 inverse_rct=false active=true component=0: actual=[255, 239, 223, 207, 191, 175, 159, 143, 128, 112, 96, 80, 64, 48, 32, 16] expected=[255, 238, 221, 204, 187, 170, 153, 136, 119, 102, 85, 68, 51, 34, 17, 0]`

**Full diagnosis:**

The failed check is the reconstruction test's final assert!(discrepancies.is_empty()), not fixture construction, resource exhaustion, or a geometry assertion. The retained host output contains concrete component-0 actual/expected sample mismatches. For example, signed four-bit reconstructed samples -8..7 converted to unsigned eight-bit output through the two-point descending LUT produce [255,239,...,16], instead of [255,238,...,0]. Geometry checks precede discrepancy collection and did not fail. Entry realism was checked first: the largest selection is 32x8, with three components and at most 768 total reconstructed i64 samples (6144 bytes of raw sample payload); a single tile, shallow fixed loops, and two LUT points are far below HARDENED limits (64 components, 100000000 pixels, 2147483648 decoded bytes). No implicit stack/pool stress or implausible count is involved. The normal caller decode_typed_layout also passes a full or selected reconstructed DecodedImage into typed_layout. This is a valid direct unit test of that stage, not a claim that arbitrary samples were decoded from the fixture's empty packets: parse_layout supplies the admitted layout, assemble_selected_image constructs matching selected component geometry, and the optional real inverse reversible component transform runs before typed_layout. The failing fraction_bits=16, inverse_rct=false route needs no component-transform assumptions: centered integer samples are multiplied by 2^16 and typed conversion restores valid signed four-bit values. The test-only overlay includes the unchanged decode.rs and merely registers a child module.

Authority is the rfc_map origin, not agreement with a Verus model or existing helper-based oracle. Independently read the pinned T.801 (06/2021), SHA256 985af26033db03edf53ce28b360107fb33b78d1a6af09b6d92ea72e89e277eb1, A.3.10 and K.2.2. A.3.10 admits this component-specific main-header type-2 NLT, two points, PTval=8, BDnlt=8, and Dmin=0/Dmax=u32::MAX representing exactly 0/1. K-7 normalizes unsigned samples by 2^by−1 and signed samples by (sample+2^(by−1))/(2^by−1). K-8 and K-9 with table [255,0], bt=bz=8 imply the exact integer oracle 255−17*rank, with an additional −128 only for signed output. No rounding convention or newer-edition claim is needed. The origin mapping cautions that it did not itself establish every numerical rule; the present independent K.2.2 reading establishes this specific admitted transformation. Thus the clause's requirement to apply the admitted inverse transformation is sound here; preserving geometry alone does not satisfy it.

Compiler declaration/CFG confirms typed_layout calls decoded.typed, then collect_nonlinear_regions, then apply_nonlinear_regions on nonempty regions (bb0 → bb4 → bb8 → bb11/bb12 → bb17). The sole inherited NLT covers component 0 of the selected tile. Per-sample processing calls inverse_nlt_sample, which dispatches type 2 to inverse_nlt_lut_sample. That function sets source_scale=2^source_info.precision; nlt_lut_coordinates multiplies the Dmin/Dmax parameters by that scale. Here it therefore interpolates at rank/16 rather than rank/15. At rank15 it rounds 255*(1−15/16) to16, instead of reaching the last LUT point0; the signed-output analogue is −112 instead of−128. This is the specific target deviation reproduced by the host, assessed on a valid unit entry, not a formal public-stream reachability proof. The host tail truncates the panic heading and earlier scenarios, but retains complete failing scenario lines and the final failed-test result, sufficient to ground this attribution. No production edits or new replay artifacts were needed.

- **Production evidence:** [`src/decode.rs:L3610-L3614`](../../src/decode.rs#L3610-L3614)

```text
    let decoded = match requested {
        None => decode_layout(layout, limits)?,
        Some(requested) => decode_layout_selected(layout, requested, limits)?,
    };
    let image = typed_layout(layout, decoded, limits)?;
```

- **Production evidence:** [`src/decode.rs:L3705-L3710`](../../src/decode.rs#L3705-L3710)

```text
    let mut image = decoded.typed(limits)?;
    let regions = collect_nonlinear_regions(layout, &image)?;
    if !regions.is_empty() {
        image.apply_nonlinear_regions(&regions, limits)?;
    }
    Ok(image)
```

- **Production evidence:** [`src/image.rs:L904-L905`](../../src/image.rs#L904-L905)

```text
                            decoded = inverse_nlt_sample(source, source_info, region.transform)?;
                            *slot = Some(decoded);
```

- **Production evidence:** [`src/image.rs:L1349`](../../src/image.rs#L1349)

```text
        2 | 4 => inverse_nlt_lut_sample(source, source_info, transform)?,
```

- **Production evidence:** [`src/image.rs:L1561`](../../src/image.rs#L1561)

```text
    let source_scale = nlt_power_of_two(source_info.precision)?;
```

- **Production evidence:** [`src/image.rs:L1646-L1651`](../../src/image.rs#L1646-L1651)

```text
    let minimum_scaled = i128::from(minimum)
        .checked_mul(source_scale)
        .ok_or_else(arithmetic_overflow)?;
    let maximum_scaled = i128::from(maximum)
        .checked_mul(source_scale)
        .ok_or_else(arithmetic_overflow)?;
```

- **Production evidence:** [`src/decode.rs:L3610-L3614`](../../src/decode.rs#L3610-L3614)

```text
    let decoded = match requested {
        None => decode_layout(layout, limits)?,
        Some(requested) => decode_layout_selected(layout, requested, limits)?,
    };
    let image = typed_layout(layout, decoded, limits)?;
```

- **Production evidence:** [`src/decode.rs:L3705-L3710`](../../src/decode.rs#L3705-L3710)

```text
    let mut image = decoded.typed(limits)?;
    let regions = collect_nonlinear_regions(layout, &image)?;
    if !regions.is_empty() {
        image.apply_nonlinear_regions(&regions, limits)?;
    }
    Ok(image)
```

- **Production evidence:** [`src/image.rs:L904-L905`](../../src/image.rs#L904-L905)

```text
                            decoded = inverse_nlt_sample(source, source_info, region.transform)?;
                            *slot = Some(decoded);
```

- **Harness evidence:** [`src/decode_contract_overlay.rs:L2`](tests/DEV-61C38E5752/files/src/decode_contract_overlay.rs#L2): `include!("decode.rs");`
- **Harness evidence:** [`src/typed_layout_contract_tests.rs:L153-L154`](tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L153-L154): `    let mut decoded = assemble_selected_image(size, &tiles, region, 0, &ResourceLimits::HARDENED)<br>        .expect("valid reconstructed selected-image fixture");`
- **Harness evidence:** [`src/typed_layout_contract_tests.rs:L296`](tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L296): `                for (fraction_bits, inverse_rct) in [(0, false), (16, false), (0, true)] {`
- **Harness evidence:** [`src/typed_layout_contract_tests.rs:L324-L326`](tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L324-L326): `                        let output = typed_layout(&layout, decoded, &limits);<br>                        let output = output.expect(&route);<br>                        assert_geometry(&output, &geometry, region);`
- **Harness evidence:** [`src/typed_layout_contract_tests.rs:L310`](tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L310): `                                *value = 255 - 17 * rank - if target_signed { 128 } else { 0 };`
- **Harness evidence:** [`src/typed_layout_contract_tests.rs:L342-L347`](tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L342-L347): `    assert!(<br>        discrepancies.is_empty(),<br>        "standards-derived NLT mismatch ({} scenarios):\n{}",<br>        discrepancies.len(),<br>        discrepancies.join("\n")<br>    );`
- **Test source:** [`src/decode_contract_overlay.rs:L2`](tests/DEV-61C38E5752/files/src/decode_contract_overlay.rs#L2)
- **Test source:** [`src/typed_layout_contract_tests.rs:L153-L154`](tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L153-L154)<br>[`src/typed_layout_contract_tests.rs:L296`](tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L296)<br>[`src/typed_layout_contract_tests.rs:L324-L326`](tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L324-L326)<br>[`src/typed_layout_contract_tests.rs:L310`](tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L310)<br>[`src/typed_layout_contract_tests.rs:L342-L347`](tests/DEV-61C38E5752/files/src/typed_layout_contract_tests.rs#L342-L347)
- **Test source:** [`src/lib.rs`](tests/DEV-61C38E5752/files/src/lib.rs)
- **Exact mapping record:** [report.json](report.json)

## DEV-AFC4DEDE80

- **Mapping:** `a44ef90a569ac2e2a08ae5a9` / `77be5e988855259692e4572cb75d7218b74c0a82cf1405711a66f508e112e25f`
- **Specification:** `fddd4285e23c96ff7162e33c`
- **Deviation:** `a5fb215b31e469aa5bcdd3f3`
- **Test item:** `9970988c331d1d0c2b443c1d`
- **Case:** `31f9c6f9ee4357015c058216`
- **Exact standards quote:**

> The Association box is a superbox,
> containing a sequence of two or more boxes.

- **Entry:** `src/jp2.rs:1323:jp2::inspect_metadata_box`
- **Command:** `cargo test --lib jp2::tests::association_requires_two_immediate_children -- --exact --nocapture`
- **Setup:** A direct unit-contract entry has no requires predicates. The BoxRef is reader-produced, not forged; LBox is 8 with empty Association payload. Default limits admit eight bytes and depth 1; the recursive budget is u32::MAX-1. Empty child cardinality is intentionally invalid input for the structural validator, precisely the condition whose rejection the clause requires, not a violated separate caller/callee premise. Production callers pass the same depth/budget for metadata roots. Larger controls remain below all capacities and use unrestricted label/XML/Association children.
- **Route:** The replay applies an additive include inside jp2::tests, compiles the library tests, and runs exactly jp2::tests::association_requires_two_immediate_children. The first fixture wraps no children in a box32 asoc frame, parses its root through BoxReader, verifies type/length/exhaustion, and calls inspect_metadata_box(root, &limits, 1, u32::MAX - 1). The inspector exhausts the empty child reader and returns Ok((1,1)); the harness records this mismatch and its final cardinality assertion panics.
- **Observed assertion:** `empty at depth 1: immediate=0, expected=None, observed=Ok((1, 1))`

**Full diagnosis:**

The first failing predicate is the final failures.is_empty() assertion, whose first recorded mismatch is the empty Association at depth 1: expected rejection, observed Ok((1, 1)). All earlier framing/admission assertions completed. This is a valid direct unit-contract negative-input test, not a bypass of a required owning caller: BoxReader constructs the BoxRef from an ordinary eight-byte LBox=8/TBox=asoc frame, and the inspector's purpose includes inspecting malformed metadata. Requiring the very cardinality under test as an entry premise would be circular. Real production calls in the top-level parser and parse_compositing_layer pass depth=1 and remaining_depth=u32::MAX-1, exactly the first failing case's arguments. Higher-depth cases represent recursive subtree entries and are not necessary for the attribution.

Entry realism was checked: 37 fixtures at three entry depths make 111 independent calls, not one accumulated budget. Largest frame is 1221 bytes, largest tree 65 nodes, actual recursive stack at most four inspector frames. Default/HARDENED capacities are input 2147483648 bytes, metadata 16777216 bytes, depth 32, work 1000000000. Depths 1, 2 and 29 with maximum descendant edge distance 3 fit the configured limit; remaining_depth is ample. There is no oversized allocation, excessive nesting, or inadmissible fabricated header. The first failing fixture requires only eight input bytes, zero payload bytes and one node.

The origin is rfc_map, not infer. I read the pinned T.801 (06/2021) M.11.11 section via read_rfc_section (hash 985af26033db03edf53ce28b360107fb33b78d1a6af09b6d92ea72e89e277eb1). It explicitly defines an Association superbox as containing a sequence of two or more boxes, permits recursive associations, and describes label-plus-entity associations. Thus immediate-child cardinality, not aggregate descendant count, is the applicable structural rule. Excluding arbitrary metadata semantics does not exclude this framing/cardinality check; SUPPORTED.md also disallows accepting malformed structures as supported extensions. File-level branding/Reader Requirements placement is not an input requirement of this direct subtree-inspection unit. This assessment is confined to the cited 2021 edition; reconciliation with the project's newer selected edition remains an inherited review qualification, not a claim of newer-edition conformance.

describe_declaration and control_flow_graph confirm the route: admitted depth -> Association branch -> checked depth increment -> successful BoxReader::new -> count=1 and maximum_depth=depth -> next_box_ref returns None for empty payload -> loop breaks -> unconditional Ok((count, maximum_depth)). No immediate-child minimum is checked. Singleton children similarly recurse successfully and only affect aggregate count/depth. Consequently structurally malformed JPX associations are reported as successful inspections. The host replay compiled and registered the additive test-only include, then failed this exact cardinality assertion; it was not a setup/build/sanitizer failure. The success-shaped expected=None log is less explicit than an error name, but the replayed match explicitly requires MalformedInput for these cases. No implementation/model/native-oracle agreement was used to override the standard.

- **Production evidence:** [`src/jp2.rs:L1342-L1344`](../../src/jp2.rs#L1342-L1344)

```text
let mut reader = BoxReader::new(box_ref.payload, limits)?;
    let mut count = 1_u32;
    let mut maximum_depth = depth;
```

- **Production evidence:** [`src/jp2.rs:L1026-L1028`](../../src/jp2.rs#L1026-L1028)

```text
let Some(next) = reader.next_box_ref() else {
            break;
        };
```

- **Production evidence:** [`src/jp2.rs:L1377`](../../src/jp2.rs#L1377)

```text
Ok((count, maximum_depth))
```

- **Production evidence:** [`src/jp2.rs:L1178`](../../src/jp2.rs#L1178)

```text
let (count, depth) = inspect_metadata_box(box_ref, limits, 1, u32::MAX - 1)?;
```

- **Production evidence:** [`src/jp2.rs:L1303`](../../src/jp2.rs#L1303)

```text
let (count, _) = inspect_metadata_box(child, limits, 1, u32::MAX - 1)?;
```

- **Production evidence:** [`SUPPORTED.md:L164-L165`](../../SUPPORTED.md#L164-L165)

```text
Unsupported capabilities remain typed errors. Reserved fields, contradictory
signaling, and malformed structures are not accepted as supported extensions.
```

- **Production evidence:** [`src/jp2.rs:L1336-L1338`](../../src/jp2.rs#L1336-L1338)

```text
if !box_ref.header.box_type().equals(BoxType::ASSOCIATION) {
        return Ok((1, depth));
    }
```

- **Production evidence:** [`src/jp2.rs:L1342-L1344`](../../src/jp2.rs#L1342-L1344)

```text
let mut reader = BoxReader::new(box_ref.payload, limits)?;
    let mut count = 1_u32;
    let mut maximum_depth = depth;
```

- **Production evidence:** [`src/jp2.rs:L1026-L1028`](../../src/jp2.rs#L1026-L1028)

```text
let Some(next) = reader.next_box_ref() else {
            break;
        };
```

- **Production evidence:** [`src/jp2.rs:L1377`](../../src/jp2.rs#L1377)

```text
Ok((count, maximum_depth))
```

- **Production evidence:** [`src/jp2.rs:L1303`](../../src/jp2.rs#L1303)

```text
let (count, _) = inspect_metadata_box(child, limits, 1, u32::MAX - 1)?;
```

- **Harness evidence:** [`src/jp2_metadata_cardinality_test.rs:L17`](tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L17): `("empty".to_owned(), vec![], None),`
- **Harness evidence:** [`src/jp2_metadata_cardinality_test.rs:L9`](tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L9): `let association = \|children: &[Vec<u8>]\| box32(*b"asoc", &children.concat());`
- **Harness evidence:** [`src/jp2_metadata_cardinality_test.rs:L75-L79`](tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L75-L79): `let mut reader = BoxReader::new(&bytes, &limits).unwrap();<br>            let root = reader.next_box_ref().unwrap().unwrap();<br>            assert_eq!(root.header.box_type(), BoxType::ASSOCIATION);<br>            assert_eq!(root.header.total_length(), bytes.len() as u64);<br>            assert!(reader.next_box_ref().is_none());`
- **Harness evidence:** [`src/jp2_metadata_cardinality_test.rs:L88`](tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L88): `let observed = inspect_metadata_box(root, &limits, depth, u32::MAX - depth);`
- **Harness evidence:** [`src/jp2_metadata_cardinality_test.rs:L98`](tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L98): `None => matches!(observed, Err(DecodeError::MalformedInput { .. })),`
- **Harness evidence:** [`src/jp2_metadata_cardinality_test.rs:L110-L114`](tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L110-L114): `assert!(<br>        failures.is_empty(),<br>        "M.11.11 immediate-child cardinality violations:\n{}",<br>        failures.join("\n")<br>    );`
- **Test source:** [`src/jp2_metadata_cardinality_test.rs:L17`](tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L17)<br>[`src/jp2_metadata_cardinality_test.rs:L9`](tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L9)<br>[`src/jp2_metadata_cardinality_test.rs:L75-L79`](tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L75-L79)<br>[`src/jp2_metadata_cardinality_test.rs:L88`](tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L88)<br>[`src/jp2_metadata_cardinality_test.rs:L98`](tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L98)<br>[`src/jp2_metadata_cardinality_test.rs:L110-L114`](tests/DEV-AFC4DEDE80/files/src/jp2_metadata_cardinality_test.rs#L110-L114)
- **Test source:** [`validation/metadata-cardinality-registration.patch`](tests/DEV-AFC4DEDE80/files/validation/metadata-cardinality-registration.patch)
- **Exact mapping record:** [report.json](report.json)

## DEV-AACA35161C

- **Mapping:** `230760b6124854275489a5da` / `273fdccf4bc2c3e34c85369b38d40d26937da93d705e62262b345a1ee655a7ea`
- **Specification:** `7e6b2fe8a69f4c8dd15c378e`
- **Deviation:** `a048c253f1a0de8dcc9926ae`
- **Test item:** `ea8d9c3e31baf68e430c41aa`
- **Case:** `cde914b61a0470c6f30bbbb3`
- **Exact standards quote:**

> Both codestreams and codestream headers are numbered separately, starting with 0. Codestream header i provides header
> information for codestream i.

- **Entry:** `src/jp2.rs:977:jp2::parse`
- **Command:** `python3 validation/jp2_spec_runner.py spec_jpx_header_bijection_and_defaults`
- **Setup:** This is a contract-valid direct unit call with ordinary immutable borrowed bytes and a valid ResourceLimits value. The prefix uses the fixed signature, jpx brand with jp2/jpx compatibility, rreq, and the original matching 1x1 grayscale JP2 header. Each jpch has a valid matching ihdr plus label and inherits default colour; each jp2c is copied from the repository's complete, independently decoded gray.jp2 fixture. The observed input is 1148 bytes, H=7,C=8, with both individual counts <=9 and negligible metadata/depth/work. The deliberate header shortfall is precisely the downstream file rule being validated, not an inadmissible parser entry. No omitted owning caller, forged private state, oversized configuration, incomplete codestream or production modification is needed.
- **Route:** The replay applies a test-only include patch, compiles the library test binary, and validation/jp2_spec_runner.py invokes jp2::tests::spec_jpx_header_bijection_and_defaults. The test constructs a JPX prefix and top-level headers followed by complete streams, then calls parse(&bytes, &limits). For the host-retained count=8,header_count=7 case parse returns a Jp2File with 19 top-level boxes and no header association on its eighth stream. The unequal-nonempty-sequence branch records a failure because the result is not MalformedInput; spec_finish then asserts that the accumulated failure list is empty.
- **Observed assertion:** `codestream_header: None }], compositing_layers: [], composition: None, metadata_boxes: 1, metadata_depth: 1, box_count: 19 })`

**Full diagnosis:**

The failed assertion is spec_finish's failures.is_empty(), which aggregates the exact clause check matches!(result, Err(DecodeError::MalformedInput { .. })) whenever header_count != 0 && header_count != count. This is not a crash, setup failure, or incorrect expectation about ordinal association. The retained host output contains the successful Jp2File suffix with an unassociated last stream and box_count: 19, identifying the replayed C=8,H=7 shortfall fixture (4 prefix boxes + 7 jpch + 8 jp2c). Earlier in test order the same check is exercised at C=2,H=1; the output tail does not retain that earlier diagnostic, so the grounded observation here is the retained C=8,H=7 result, not a claim to have seen the earlier line.

Entry realism was checked before deeper tracing: the suite uses at most 8 complete 1x1, one-component codestreams and 9 small jpch boxes, with maximum_codestreams explicitly set to 9. The largest constructed input is 1242 bytes; the observed C=8,H=7 input is 1148 bytes. These are far below HARDENED/default input 2147483648 bytes, metadata 16777216 bytes, depth 32, width/height 1000000, component 64 and work 1000000000 limits. There is no deep recursion, fixture pool, unsafe state fabrication or extraordinary allocation. Real decode_jpx_codestream* and parse_and_inspect callers pass borrowed file bytes and configured limits directly to parse; the assessed direct unit contract has no additional requires. The intentionally inconsistent header count is a file-validation predicate, not a precondition on this public byte parser.

Authority is the rfc_map origin, not agreement with existing implementation-derived specs. I read the assigned T.801 (06/2021), ISO/IEC 15444-2:2021 M.11.6 with read_rfc_section using origin item 230760b6124854275489a5da and confirmed its manifest SHA/version 985af26033db03edf53ce28b360107fb33b78d1a6af09b6d92ea72e89e277eb1. It expressly states: 'If Codestream Header boxes appear anywhere in the file, the number of codestreams found in the file shall be the same as the number of available codestream headers.' It separately specifies ordinal pairing and the no-header defaults case. These fixtures contain only supported top-level jpch and jp2c representations: no fragments, multiple-codestream boxes, cross-references or compositing-layer extensions can supply missing headers. The clause requires this check in the selected container-validation role; this diagnosis does not infer a general obligation for every tolerant JPEG 2000 reader to reject every nonconforming file.

describe_declaration establishes parse(&[u8], &ResourceLimits) -> Result<Jp2File, DecodeError>. Source and control_flow_graph trace the route: accept signature, JPX-compatible ftyp, opaque rreq, jp2h, then H valid jpch inheriting matching defaults, followed by C complete jp2c. At EOF MIR bb248-bb250 computes only H>C; for 7<8 it takes bb253. bb254-bb275 associates exactly H headers; bb277-bb282 validates the empty layer list and returns Ok, leaving stream 7's association None. That violates this clause's nonempty coverage check, although it agrees with weaker implementation-inferred clauses. The replay's test-only include patch changes no production behavior and the runner built and executed the named Rust test successfully. Diagnostics are unnecessarily verbose and truncate the earliest accumulated failures, but the retained successful result, exact harness construction and specific failed predicate still identify this counterexample; the nonzero exit alone is not the evidence.

- **Production evidence:** [`src/jp2.rs:L1194-L1196`](../../src/jp2.rs#L1194-L1196)

```text
    if codestream_headers.len() > codestream_boxes.len() {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
    }
```

- **Production evidence:** [`src/jp2.rs:L1205`](../../src/jp2.rs#L1205)

```text
    while header_index < codestream_headers.len() {
```

- **Production evidence:** [`src/jp2.rs:L1213`](../../src/jp2.rs#L1213)

```text
        codestream_box.codestream_header = Some(association);
```

- **Production evidence:** [`src/jp2.rs:L1218-L1219`](../../src/jp2.rs#L1218-L1219)

```text
    validate_layer_references(&compositing_layers, codestream_boxes.len())?;
    Ok(Jp2File {
```

- **Production evidence:** [`src/jp2.rs:L1194-L1196`](../../src/jp2.rs#L1194-L1196)

```text
    if codestream_headers.len() > codestream_boxes.len() {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
    }
```

- **Production evidence:** [`src/jp2.rs:L1205`](../../src/jp2.rs#L1205)

```text
    while header_index < codestream_headers.len() {
```

- **Production evidence:** [`src/jp2.rs:L1213`](../../src/jp2.rs#L1213)

```text
        codestream_box.codestream_header = Some(association);
```

- **Production evidence:** [`src/decode.rs:L4399`](../../src/decode.rs#L4399)

```text
        None => Some(file.header()),
```

- **Harness evidence:** [`src/jp2/spec_tests.rs:L382-L384`](tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L382-L384): `    let limits = ResourceLimits::default()<br>        .with_maximum_codestreams(9)<br>        .unwrap();`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L386`](tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L386): `    for count in [1_usize, 2, 3, 8] {`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L404`](tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L404): `        for header_count in [0, count, count + 1, count.saturating_sub(1)] {`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L411-L421`](tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L411-L421): `            input.extend((0..count).map(\|_\| parts[3].clone()));<br>            let bytes = input.concat();<br>            let result = parse(&bytes, &limits);<br>            if header_count != 0 && header_count != count {<br>                spec_record(<br>                    &mut failures,<br>                    matches!(result, Err(DecodeError::MalformedInput { .. })),<br>                    format!(<br>                        "JPX streams={count}, headers={header_count}: reject unequal nonempty sequence; {result:?}"<br>                    ),<br>                );`
- **Harness evidence:** [`src/jp2/spec_tests.rs:L55-L61`](tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L55-L61): `fn spec_finish(failures: Vec<String>) {<br>    assert!(<br>        failures.is_empty(),<br>        "clause violations:\n{}",<br>        failures.join("\n")<br>    );<br>}`
- **Test source:** [`src/jp2/spec_tests.rs:L382-L384`](tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L382-L384)<br>[`src/jp2/spec_tests.rs:L386`](tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L386)<br>[`src/jp2/spec_tests.rs:L404`](tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L404)<br>[`src/jp2/spec_tests.rs:L411-L421`](tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L411-L421)<br>[`src/jp2/spec_tests.rs:L55-L61`](tests/DEV-AACA35161C/files/src/jp2/spec_tests.rs#L55-L61)
- **Test source:** [`jp2-spec-registration.patch`](tests/DEV-AACA35161C/files/jp2-spec-registration.patch)
- **Test source:** [`validation/jp2_spec_runner.py`](tests/DEV-AACA35161C/files/validation/jp2_spec_runner.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-CBF63FCFA1

- **Mapping:** `6f26e2fe85b2cee05006d642` / `fec1f7322388266751a278bf14f2d63349948f502fb37874f2f1f0e10bf151df`
- **Specification:** `88410e65d017a25fe34e462f`
- **Deviation:** `b9a36ad9f01aa7bd3ec1763c`
- **Test item:** `111437184a2d565c3ed13b32`
- **Case:** `09f93dd85547f3089d8c40f4`
- **Exact standards quote:**

> If Codestream Header boxes appear anywhere in the file, the number of codestreams found in the file shall be the same
> as the number of available codestream headers.

- **Entry:** `src/jp2.rs:653:jp2::Jp2File::codestream`
- **Command:** `cargo test --locked --lib jp2::tests::retains_and_inspects_explicit_jpx_codestreams_and_headers -- --exact --nocapture`
- **Setup:** Public byte-parser entry accepts a borrowed byte slice and valid ResourceLimits; it need not assume cardinality-valid input because rejection of that malformed structure is precisely the tested obligation. Fixture uses the retained 167-byte grayscale JP2's actual image/color and codestream bytes, JPX signature/type and rreq framing, one explicit top-level header for two streams, and one default compositing layer. Payload lifetimes are valid and all getter indices are in range on the actual parse-produced receiver. Smallest failing input is 357 bytes; all cases stay below 10KB, at most 64 streams/headers, with one layer and shallow nesting. Positive zero-header and equal-header controls pass. The deliberate header deficit is not private-state fabrication or a caller-contract violation.
- **Route:** Host cargo test --locked --lib jp2::tests::retains_and_inspects_explicit_jpx_codestreams_and_headers -- --exact --nocapture runs the additive association_fixture cases. For (streams,headers)=(2,1), public parse returns Ok(file); the Ok arm invokes file.codestream(index).unwrap().codestream_header().is_none() over 0..streams, records missing index 1, and the final admitted_mismatches.is_empty() assertion fails with the quoted diagnostic.
- **Observed assertion:** `N=2, H=1: admitted 2 streams, missing headers at [1]`

**Full diagnosis:**

The first failed assertion is the final admitted_mismatches.is_empty() check, not fixture setup, decoding, or a resource failure. Host output specifically reports 'N=2, H=1: admitted 2 streams, missing headers at [1]'; all H=N and H=0 controls and the four H>N rejection cases completed before that assertion. Entry realism: the smallest failing case is only 357 bytes with two 1x1, one-component codestreams and one explicit header. The largest constructed case is 9,811 bytes, with at most 64 streams, 64 headers, one layer and shallow box nesting, within configured limits of 64 codestreams, 256 layers, 32 box depth, 16,777,216 metadata bytes and 2,147,483,648 input bytes. There is no large allocation or stack-exhaustion explanation. Native decode_jpx_codestream also passes caller-supplied bytes to this same public parser with options.limits(); the test does not invent an internal receiver or bypass its owning producer.

Authority is the rfc_map origin, not agreement with existing code or models. Reading its pinned T.801 (06/2021), SHA-256 985af26033db03edf53ce28b360107fb33b78d1a6af09b6d92ea72e89e277eb1, M.11.6 confirms both ordinal association and the explicit requirement that if any Codestream Header boxes exist, codestream and available-header counts shall be equal. The zero-header default exception does not apply to H=1,N=2. This fixture uses only top-level jpch and contiguous jp2c boxes, so fragment/multiple-codestream counting and compositing-layer-extension repetition do not affect applicability. SUPPORTED.md independently promises rejection of malformed structures. The project selects a later standard edition; this diagnosis establishes the specific mapped 2021 obligation, not exhaustive conformance to the later edition, whose text was not independently checked here.

Route: the replay additively extends the native test, builds JPX bytes from the real grayscale fixture, and invokes public parse(&bytes, &limits). Malformed cardinality is the intentional downstream rejection predicate under test, not a precondition of this byte-parser entry. Signature, JPX file type, rreq, inherited grayscale jp2h, explicit jpch, one default layer and actual codestream payloads are constructed; no private Jp2File state is fabricated. Parser MIR and source show each stream starts with codestream_header: None, bb250 rejects only H>N, and bb255 associates only indices below H. With 0<H<N it then validates layer references and bb282 returns Ok(Jp2File). The replay subsequently calls file.codestream(index) on every in-range index and observes the missing association. Getter MIR bb0-bb4 performs slice get and ok_or, correctly exposing the stored element without repairing the producer's invariant. Thus the violation belongs to parse admission and its resulting exposed association set, not getter bounds handling. It directly violates this clause's nonzero-cardinality-rejection guarantee. No full rendering, fallback execution, or formal reachability proof is claimed. This is neither test_error nor a reason to weaken the standards-derived clause.

- **Production evidence:** [`src/jp2.rs:L1194-L1196`](../../src/jp2.rs#L1194-L1196)

```text
if codestream_headers.len() > codestream_boxes.len() {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
    }
```

- **Production evidence:** [`src/jp2.rs:L1172-L1176`](../../src/jp2.rs#L1172-L1176)

```text
codestream_boxes.push(CodestreamBox {
                header: box_ref.header,
                data: box_ref.payload,
                codestream_header: None,
            });
```

- **Production evidence:** [`src/jp2.rs:L1205`](../../src/jp2.rs#L1205)

```text
while header_index < codestream_headers.len() {
```

- **Production evidence:** [`src/jp2.rs:L1213`](../../src/jp2.rs#L1213)

```text
codestream_box.codestream_header = Some(association);
```

- **Production evidence:** [`src/jp2.rs:L1218-L1219`](../../src/jp2.rs#L1218-L1219)

```text
validate_layer_references(&compositing_layers, codestream_boxes.len())?;
    Ok(Jp2File {
```

- **Production evidence:** [`SUPPORTED.md:L164-L165`](../../SUPPORTED.md#L164-L165)

```text
Unsupported capabilities remain typed errors. Reserved fields, contradictory
signaling, and malformed structures are not accepted as supported extensions.
```

- **Production evidence:** [`src/jp2.rs:L1194-L1196`](../../src/jp2.rs#L1194-L1196)

```text
if codestream_headers.len() > codestream_boxes.len() {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
    }
```

- **Production evidence:** [`src/jp2.rs:L1205`](../../src/jp2.rs#L1205)

```text
while header_index < codestream_headers.len() {
```

- **Production evidence:** [`src/jp2.rs:L1210-L1213`](../../src/jp2.rs#L1210-L1213)

```text
let codestream_box = codestream_boxes
            .get_mut(header_index)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidBoxContents))?;
        codestream_box.codestream_header = Some(association);
```

- **Production evidence:** [`src/jp2.rs:L575`](../../src/jp2.rs#L575)

```text
None => Ok(fallback),
```

- **Production evidence:** [`src/decode.rs:L4366-L4370`](../../src/decode.rs#L4366-L4370)

```text
let selected = file.codestream(codestream_index)?;
    let associated_header = Some(jp2::associated_header_for_codestream(
        selected,
        file.header(),
    )?);
```

- **Harness evidence:** [`deepspec_codestream.patch:L14`](tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L14): `let gray = include_bytes!("../validation/image-consumer-probe/tests/fixtures/gray.jp2");`
- **Harness evidence:** [`deepspec_codestream.patch:L117`](tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L117): `(2, 1), (3, 2), (8, 7), (64, 63), (64, 1),`
- **Harness evidence:** [`deepspec_codestream.patch:L121`](tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L121): `match parse(&bytes, &limits) {`
- **Harness evidence:** [`deepspec_codestream.patch:L126`](tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L126): `.filter(\|&index\| file.codestream(index).unwrap().codestream_header().is_none())`
- **Harness evidence:** [`deepspec_codestream.patch:L136`](tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L136): `admitted_mismatches.is_empty(),`
- **Harness evidence:** [`deepspec_codestream.patch:L137`](tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L137): `"T.801 M.11.6 forbids nonzero cardinality mismatches:\n{}"`
- **Test source:** [`deepspec_codestream.patch:L14`](tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L14)<br>[`deepspec_codestream.patch:L117`](tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L117)<br>[`deepspec_codestream.patch:L121`](tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L121)<br>[`deepspec_codestream.patch:L126`](tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L126)<br>[`deepspec_codestream.patch:L136`](tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L136)<br>[`deepspec_codestream.patch:L137`](tests/DEV-CBF63FCFA1/files/deepspec_codestream.patch#L137)
- **Exact mapping record:** [report.json](report.json)

## DEV-7F4AE3ED02

- **Mapping:** `6f3ae182625346bf926027f1` / `1aad6942d102d0f145fa98197210e4ceeec1964a9087bb0f4a29822c77d210f5`
- **Specification:** `ce77e09dc97f1a44c8c8d5e3`
- **Deviation:** `df903a5147260ea3adf42972`
- **Test item:** `bb705c76759f74c8112ad3a5`
- **Case:** `e62d0f8ba0be84c76f97c921`
- **Exact standards quote:**

> If any Compositing Layer Header box contains a Codestream Registration box, then every
> Compositing Layer Header box shall contain a Codestream Registration box.

- **Entry:** `src/jp2.rs:1919:jp2::validate_layer_references`
- **Command:** `cargo test --lib --locked jp2::tests::layer_reference_contract::registration_presence_is_file_wide_and_references_exist -- --exact --nocapture`
- **Setup:** A direct unit-contract test is available with no requires predicates. Both layers are produced by the production parser from scoped headers, have local grayscale colour groups, default/unit registration grids, and, where creg exists, one entry with XS=YS=XR=YR=1 and XO=YO=0. The supplied count is a valid unit input; both explicit and implicit references exist in that sequence. No actual codestream objects are required by this function's slice/count API. Mixed presence is intentionally invalid under the downstream structural predicate, but is not forbidden at this validator's entry. Two layers, two codestreams, 80 fixture bytes and depth 3 are comfortably within configured limits. No complete-file parsing or codestream-decoding reproduction is claimed.
- **Route:** Replayed cargo test jp2::tests::layer_reference_contract::registration_presence_is_file_wide_and_references_exist -> layer_headers constructs real box bytes -> parsed_layers invokes BoxReader::new/next_box_ref and parse_compositing_layer -> validate_layer_references(&layers, length). For length=2, mask=1, explicit index 1 in layer 0 is less than count 2, and absent-registration layer 1 has ordinal 1 less than 2. Both branches advance to Ok(()). actual.is_ok() records (2,1), and the final accepted_mixed.is_empty() assertion fails after enumerating the family.
- **Observed assertion:** `M.11.7.7 all-or-none presence: accepted 494 mixed patterns; first=Some((2, 1))`

**Full diagnosis:**

The first failed assertion is the final accepted_mixed.is_empty() check, not a fixture/parser failure: host replay reports 494 accepted mixed patterns and first=Some((2, 1)). The earlier original regression, all-registered controls, out-of-range-reference checks, and coherent all/none controls completed before this assertion. The smallest failing unit entry contains two parser-produced layers and codestream_count=2: layer 0 has one explicit reference to codestream 1; layer 1 has no creg and uses implicit ordinal 1. This is an admissible input to the validator, whose assessed unit contract has no requires predicates; mixed presence is the property to validate, not an entry precondition. The harness does not fabricate private layer fields or bypass layer parsing: it builds framed jplh/cgrp/colr/creg bytes and invokes BoxReader and parse_compositing_layer before the target. Resource realism is not a concern: the failing family uses only 2–8 layers/codestreams, one registration per registered layer and nesting depth 3; the smallest case is 80 bytes. Even the preceding control families use at most 256 layers, 64 codestreams and two registrations per layer, within the configured 256-layer, 64-codestream, depth-32 and metadata/input budgets. Real parse callers pass the same parser-produced layer vector and codestream_boxes.len(). MIR confirms that each layer is considered independently: nonempty registrations only take the bounded explicit-index route; empty registrations only take the bounded ordinal route; both join the layer increment and ultimately return Ok. Thus the smallest mixed case reaches Ok despite violating the exact all-or-none structural condition. The origin is rfc_map, not infer. I read its parent finding and the original T.801 (06/2021), ISO/IEC 15444-2:2021 M.11.7.7 section through read_rfc_section using the parent mapping item: it explicitly requires every Compositing Layer Header box to have creg if any has one. This applies to these ordinary layer headers and does not depend on rendering or multiple registrations per layer. Empty vectors faithfully denote absence because the local creg parser requires at least one entry. No prior cross-layer check exists in the inspected parse path, which publishes Jp2File immediately after this validator. This diagnosis is limited to the standards-derived structural-validation clause and the reproduced valid unit entry; it does not assert that every reader must reject every nonconforming file, that this replay executed a complete mixed JPX file, or that the unexamined project-selected newer standard edition is established. The review-marked edition qualification remains: the cited and checked authority here is the 2021 edition, not a claim of full current-edition conformance.

- **Production evidence:** [`src/jp2.rs:L1935-L1936`](../../src/jp2.rs#L1935-L1936)

```text
if layer.registrations.is_empty() {
            if layer_index >= codestream_count {
```

- **Production evidence:** [`src/jp2.rs:L1956`](../../src/jp2.rs#L1956)

```text
if usize::from(registration.codestream_index) >= codestream_count {
```

- **Production evidence:** [`src/jp2.rs:L1218-L1219`](../../src/jp2.rs#L1218-L1219)

```text
validate_layer_references(&compositing_layers, codestream_boxes.len())?;
    Ok(Jp2File {
```

- **Production evidence:** [`src/jp2.rs:L1386`](../../src/jp2.rs#L1386)

```text
if box_ref.payload.len() < 10 || (box_ref.payload.len() - 4) % 6 != 0 {
```

- **Production evidence:** [`src/jp2.rs:L1218-L1219`](../../src/jp2.rs#L1218-L1219)

```text
validate_layer_references(&compositing_layers, codestream_boxes.len())?;
    Ok(Jp2File {
```

- **Production evidence:** [`src/jp2.rs:L1263-L1268`](../../src/jp2.rs#L1263-L1268)

```text
if has_registration {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, child.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
```

- **Harness evidence:** [`src/jp2_layer_reference_tests.rs:L35`](tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L35): `let layer = parse_compositing_layer(framed, &limits).unwrap();`
- **Harness evidence:** [`src/jp2_layer_reference_tests.rs:L141-L149`](tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L141-L149): `let references: Vec<Vec<u16>> = (0..length)<br>                .map(\|index\| {<br>                    if mask & (1 << index) == 0 {<br>                        Vec::new()<br>                    } else {<br>                        vec![u16::try_from(length - 1 - index).unwrap()]<br>                    }<br>                })<br>                .collect();`
- **Harness evidence:** [`src/jp2_layer_reference_tests.rs:L152`](tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L152): `let actual = validate_layer_references(&layers, length);`
- **Harness evidence:** [`src/jp2_layer_reference_tests.rs:L155`](tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L155): `} else if actual.is_ok() {`
- **Harness evidence:** [`src/jp2_layer_reference_tests.rs:L161`](tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L161): `accepted_mixed.push((length, mask));`
- **Harness evidence:** [`src/jp2_layer_reference_tests.rs:L169-L174`](tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L169-L174): `assert!(<br>        accepted_mixed.is_empty(),<br>        "M.11.7.7 all-or-none presence: accepted {} mixed patterns; first={:?}",<br>        accepted_mixed.len(),<br>        accepted_mixed.first()<br>    );`
- **Test source:** [`src/jp2_layer_reference_tests.rs:L35`](tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L35)<br>[`src/jp2_layer_reference_tests.rs:L141-L149`](tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L141-L149)<br>[`src/jp2_layer_reference_tests.rs:L152`](tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L152)<br>[`src/jp2_layer_reference_tests.rs:L155`](tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L155)<br>[`src/jp2_layer_reference_tests.rs:L161`](tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L161)<br>[`src/jp2_layer_reference_tests.rs:L169-L174`](tests/DEV-7F4AE3ED02/files/src/jp2_layer_reference_tests.rs#L169-L174)
- **Test source:** [`layer_reference_tests.patch`](tests/DEV-7F4AE3ED02/files/layer_reference_tests.patch)
- **Exact mapping record:** [report.json](report.json)

## DEV-C635AC0588

- **Mapping:** `ae0ab2a24be5b5f3a4ca7374` / `f020bb18a62bcef46ad170bc3df0ba509edb79caf54896cba75ede7e34f2b8ef`
- **Specification:** `e11d683717746f5872e7302c`
- **Deviation:** `dc9e856b125a6dd08ee14a6e`
- **Test item:** `d18e7664ef908ecf323e5b74`
- **Case:** `e4ed0a3af2c9d0dbd08711cc`
- **Exact standards quote:**

> The PERSIST and LIFE parameters for the closing instruction of a frame definition are applied
>                to the virtual compositing layer as a whole. That is, upon close of frame composition mode, all of the
>                compositing layers involved in the frame composition sequence acquire the PERSIST and LIFE values of
>                the closing frame in the sequence.

- **Entry:** `src/jpx.rs:169:jpx::JpxFrameInfo::persistent`
- **Command:** `python3 -c import os, pathlib, subprocess; name='jpx::tests::timeline_expands_repetition_reuse_and_timing_deterministically'; bins=[p for p in pathlib.Path('target/debug/deps').glob('jpeg2000_rs-*') if p.is_file() and os.access(p,os.X_OK)]; assert len(bins)==1, bins; exe=str(bins[0]); listed=subprocess.check_output([exe,'--list','--exact',name],text=True); assert name+': test' in listed, listed; os.execv(exe,[exe,'--exact',name,'--nocapture'])`
- **Setup:** Valid direct normalized unit entry, not claimed public-byte reachability: four layers and four in-bounds disjoint 1x1 placements on a 4x1 canvas; sequential layer assignment, no reuse/crop, one unrepeated instruction set, tick=1, loop_count=1, two requested in-range frames. build_timeline owns creation of all inspected frame objects. Supplied layer keys cover every assignment, each grayscale layer has exactly one u16 sample and alpha=None denotes opacity. Fresh supplied banks are used per render. Default limits and the explicit two-frame builder bound suffice. LIFE=0/PERSIST=false is an expressly defined standard special case, not an invalid state. Neither setup nor output depends on altered production behavior.
- **Route:** Host runs the exact registered jpx::tests::timeline_expands_repetition_reuse_and_timing_deterministically test. Its additive assert_frame_closing_semantics call constructs normalized Composition instructions, calls production build_timeline(&skipped,4,2), observes the returned frame metadata, then calls closing_semantics_render for frames 0 and 1. That helper builds valid opaque one-pixel supplied layers and calls production render_normalized_frame -> select_frame_instructions -> collect_supplied_layers -> render_decoded_layers. The frame-0 selection includes the zero-LIFE nonpersistent instruction retained by append_timeline_frame; the exact tuple assertion observes its unwanted column-1 pixel and alpha.
- **Observed assertion:** `assertion `left == right` failed: T.801 M.5.3.2.1: PERSIST=false/LIFE=0 should perform no action; column 1 must remain transparent, not display the skipped thumbnail<br>  left: ([(0, 17, false), (17, 5, true)], [1000, 2000, 3000, 0], [65535, 65535, 65535, 0], [0, 0, 0, 4000], [0, 0, 0, 65535])<br> right: ([(0, 17, false), (17, 5, true)], [1000, 0, 3000, 0], [65535, 0, 65535, 0], [0, 0, 0, 4000], [0, 0, 0, 65535])`

**Full diagnosis:**

The first host-observed failed assertion is the final zero-LIFE nonpersistent pixel/alpha comparison at src/jpx_frame_semantics_tests.rs:215, not an earlier metadata assertion or a build failure. Frame 0 contains gray 2000 and alpha 65535 at column 1, where the independent oracle expects transparency. This refutes the standards-derived inspection/indexed-rendering clause e11d683717746f5872e7302c; it does not demonstrate an incorrect field load in persistent() or refute the narrower getter clause 2a438ae4e8c4d1f07241ce8b.

Entry realism was checked before tracing: the failing fixture has four instructions, four opaque 1x1 grayscale layers, a 4x1 canvas, two frames, one instruction set, repeat=0, tick=1, loop_count=1 and finite closing lives 17 and 5. There is no recursive fixture or duration-driven waiting. The preceding 136 sweep fixtures run sequentially, each with at most six layers/pixels and three frames; the maximum finite LIFE is stored numerically, not expanded into that many frames. Defaults are ResourceLimits::HARDENED (256 layers, 4096 frames, 100000000 pixels, 2147483648 decoded bytes, 1000000000 work units). Even the tighter build_timeline limit of two frames in the failing case permits the result, and its instruction bound is 4*2=8 for four instructions. Real inspection calls this same builder with effective_layer_count(file) and limits.maximum_frames(). These are small, representative normalized unit inputs, not an implicit-capacity violation.

Authority is the rfc_map origin ae0ab2a24be5b5f3a4ca7374, including its parent finding. I read the pinned T.801 (06/2021) M.5.3.2.1 text (SHA256 985af26033db03edf53ce28b360107fb33b78d1a6af09b6d92ea72e89e277eb1). It explicitly says that when PERSIST is false and LIFE is zero, no action should be performed, and explains skipping a thumbnail/print frame. Its adjacent frame-composition rule groups persistent zero-LIFE prefixes and applies the closing instruction's LIFE/PERSIST to the resulting virtual layer. The fixture faithfully tests this exception inside an active composition sequence. This is a SHOULD recommendation deviation, not an unconditional SHALL violation. SUPPORTED.md identifies this 06/2021 text as an implementation reference while leaving reconciliation with its selected 08/2023 consolidation open; this diagnosis is limited to the cited edition and does not claim later-edition or official conformance.

Route: the existing registered timeline regression calls the test-only helper; build_timeline creates valid JpxFrameInfo objects from the supplied Composition, assigning layers 0,1,2,3. apply_timeline_instruction calls append_timeline_frame for each assigned instruction without excluding LIFE=0/PERSIST=false. The initial (0,true) instruction starts frame 0. For the next (0,false) instruction, append's MIR bb5 -> bb6 -> bb17 observes the preceding duration is zero, and bb22 unconditionally appends that instruction to the active frame. The (17,false) instruction closes that same frame; (5,true) starts frame 1. render_normalized_frame selects frame 0; select_frame_instructions includes every instruction of the selected frame, so collect_supplied_layers retrieves the valid opaque layer 1 and rendering paints its column. The later selected frame correctly omits the nonpersistent prior group, matching the observed second image. The getter MIR is only a shared-reference bool copy and correctly reports the stored closing flag.

This is an admissible direct normalized unit-contract test: no JpxFrameInfo is fabricated or mutated, dimensions and samples match, all assigned layer indices have distinct supplied entries, all crops/reuse are absent and requested frames are in range. The patch changes only cfg(test) registration and preserves the original regression. It deliberately does not establish a public encoded-file route, parser flag admission, profile signaling or decoding; those are outside this unit boundary, not prerequisites of these normalized operations. The run's abbreviated satisfies list mentions persistent zero-LIFE prefixes but omits the additional nonpersistent exception; the actual replayed source and precise failure expose that case clearly. No production changes or new replay were needed to attribute this already host-observed failure.

- **Production evidence:** [`src/jpx.rs:L708-L714`](../../src/jpx.rs#L708-L714)

```text
    append_timeline_frame(
        &mut state.frames,
        &mut state.milliseconds,
        instruction_info(layer_index, instruction),
        instruction.life,
        maximum_frames,
    )?;
```

- **Production evidence:** [`src/jpx.rs:L830-L833`](../../src/jpx.rs#L830-L833)

```text
    let fresh = match frames.last() {
        Some(frame) => frame.duration_milliseconds != 0,
        None => true,
    };
```

- **Production evidence:** [`src/jpx.rs:L867`](../../src/jpx.rs#L867)

```text
    frame.instructions.push(instruction);
```

- **Production evidence:** [`src/jpx.rs:L1134`](../../src/jpx.rs#L1134)

```text
        if index == frame_index || frame.persistent {
```

- **Production evidence:** [`src/jpx.rs:L1460-L1462`](../../src/jpx.rs#L1460-L1462)

```text
    let instructions = select_frame_instructions(frames, loop_count, requested_frame)?;
    let placements = collect_supplied_layers(&instructions, supplied)?;
    render_decoded_layers(width, height, &placements, maximum_decoded_bytes, limits)
```

- **Production evidence:** [`SUPPORTED.md:L177-L178`](../../SUPPORTED.md#L177-L178)

```text
Part 2 implementation references include the separately acquired T.801
06/2021 text.
```

- **Production evidence:** [`src/jpx.rs:L830-L833`](../../src/jpx.rs#L830-L833)

```text
let fresh = match frames.last() {
        Some(frame) => frame.duration_milliseconds != 0,
        None => true,
    };
```

- **Production evidence:** [`src/jpx.rs:L863-L864`](../../src/jpx.rs#L863-L864)

```text
frame.duration_milliseconds = life;
    frame.persistent = instruction.persistent;
```

- **Production evidence:** [`src/jpx.rs:L867`](../../src/jpx.rs#L867)

```text
frame.instructions.push(instruction);
```

- **Production evidence:** [`src/jpx.rs:L1134`](../../src/jpx.rs#L1134)

```text
if index == frame_index || frame.persistent {
```

- **Production evidence:** [`src/jpx.rs:L1154-L1157`](../../src/jpx.rs#L1154-L1157)

```text
let instruction = *frame
                    .instructions
                    .get(instruction_index)
                    .ok_or_else(arithmetic_overflow)?;
```

- **Production evidence:** [`src/jpx.rs:L169-L171`](../../src/jpx.rs#L169-L171)

```text
pub const fn persistent(&self) -> bool {
        self.persistent
    }
```

- **Production evidence:** [`src/jpx.rs:L173-L175`](../../src/jpx.rs#L173-L175)

```text
pub fn instructions(&self) -> &[JpxInstructionInfo] {
        &self.instructions
    }
```

- **Harness evidence:** [`frame-semantics.patch:L16`](tests/DEV-C635AC0588/files/frame-semantics.patch#L16): `+        assert_frame_closing_semantics();`
- **Harness evidence:** [`src/jpx_frame_semantics_tests.rs:L188-L194`](tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L188-L194): `    let skipped = closing_semantics_composition(vec![<br>        closing_semantics_instruction(0, 0, true),<br>        closing_semantics_instruction(1, 0, false),<br>        closing_semantics_instruction(2, 17, false),<br>        closing_semantics_instruction(3, 5, true),<br>    ]);<br>    let frames = build_timeline(&skipped, 4, 2).unwrap();`
- **Harness evidence:** [`src/jpx_frame_semantics_tests.rs:L205-L206`](tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L205-L206): `    let first = closing_semantics_render(&frames, 4, 0).unwrap();<br>    let next = closing_semantics_render(&frames, 4, 1).unwrap();`
- **Harness evidence:** [`src/jpx_frame_semantics_tests.rs:L45-L51`](tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L45-L51): `                Some(Ok(OwnedLayer {<br>                    width: 1,<br>                    height: 1,<br>                    color_space: ConvertedColorSpace::Grayscale,<br>                    colors: vec![vec![u16::try_from((index + 1) * 1000).unwrap()]],<br>                    alpha: None,<br>                })),`
- **Harness evidence:** [`src/jpx_frame_semantics_tests.rs:L55-L64`](tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L55-L64): `    render_normalized_frame(<br>        width,<br>        1,<br>        frames,<br>        1,<br>        requested,<br>        bank,<br>        limits.maximum_decoded_bytes(),<br>        &limits,<br>    )`
- **Harness evidence:** [`src/jpx_frame_semantics_tests.rs:L218-L220`](tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L218-L220): `            vec![(0, 17, false), (17, 5, true)],<br>            vec![1000, 0, 3000, 0],<br>            vec![u16::MAX, 0, u16::MAX, 0],`
- **Test source:** [`frame-semantics.patch:L16`](tests/DEV-C635AC0588/files/frame-semantics.patch#L16)
- **Test source:** [`src/jpx_frame_semantics_tests.rs:L188-L194`](tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L188-L194)<br>[`src/jpx_frame_semantics_tests.rs:L205-L206`](tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L205-L206)<br>[`src/jpx_frame_semantics_tests.rs:L45-L51`](tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L45-L51)<br>[`src/jpx_frame_semantics_tests.rs:L55-L64`](tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L55-L64)<br>[`src/jpx_frame_semantics_tests.rs:L218-L220`](tests/DEV-C635AC0588/files/src/jpx_frame_semantics_tests.rs#L218-L220)
- **Exact mapping record:** [report.json](report.json)

## DEV-9A1ABE441F

- **Mapping:** `dc9db665e258a9562dc090af` / `28580cd70f92601009d53cc22676c850fe46dda58f07d6e8943318325d231890`
- **Specification:** `6b2c861fc7792bec596e0b48`
- **Deviation:** `1a09df4370e8bfc3bfe3d592`
- **Test item:** `55730ec94ef7342e41f0ebc5`
- **Case:** `a47255338004ddac419b357f`
- **Exact standards quote:**

> Each VBAS consists of a sequence of bytes, all but the last of which has a most significant
> bit (bit 7) of 1, as seen in Figure A.2. The least significant 7 bits of each byte in the VBAS are concatenated to form a bit
> stream which is used in different ways for different VBASs.

- **Entry:** `src/jpip.rs:361:jpip::read_vbas`
- **Command:** `cargo test --lib --locked jpip::tests::bounded_standard_vbas_contract -- --exact --nocapture`
- **Setup:** This is a real eight-byte slice with fresh JpipHeaderContext and default ResourceLimits, not fabricated parser internals. Independent form bb=11 explicitly supplies Class and CSn; Class payload [0x80,0x08] denotes metadata class 8, valid for both JPP/JPT; CSn=0, Bin-ID=0, offset=0, length=1, c=0, and even Class means no Aux. Body [0xab] exists and 0x5a is the following unconsumed sentinel. Both padded fields are two bytes, well within MAX_VBAS_BYTES=10, and all values and extents fit u64/usize and default input limit 2147483648. The direct-unit cases likewise use valid Rust references, actual small slices and the two caller-used masks; redundant-zero fields are at most ten bytes and preserve mathematical payload. There is no callee precondition requiring a nonzero first group: the canonical predicate is total over u8/bool, and its false return is the implementation decision being tested.
- **Route:** Host successfully builds the additive include inside the existing jpip::tests module and runs cargo test --lib --locked jpip::tests::bounded_standard_vbas_contract -- --exact --nocapture. Its complete-message loop calls unchanged parse_jpip_message with the padded Class case [60,80,08,00,00,01,ab,5a]. Input/resource/header-form checks pass; Bin-ID 0x60 decodes through Some(0x0f) as in-class 0 and selects explicit Class and CSn. The Class call enters read_vbas at position 1 with None. It reads 0x80, sets value=0 and position=2, calls jpip_vbas_first_is_canonical(0,true), and immediately returns InvalidJpipVbas at offset 1. The harness records the Err instead of expected Data, and its final failures.is_empty assertion fails. A separate padded Msg-Length witness reaches the same guard at offset 4.
- **Observed assertion:** `parse_jpip_message -> read_vbas, independent metadata header [60, 80, 08, 00, 00, 01, ab, 5a]: expected value-preserving Data, got Err(MalformedInput { offset: 1, kind: InvalidJpipVbas })`

**Full diagnosis:**

Attribute this exact standards-derived clause 6b2c861fc7792bec596e0b48 to code_deviation, not the separate canonical-only inferred clauses. Reality check first: the suite has 703 canonical values, each tested sequentially with caller-used masks; the longest successful-field fixture is 3 prefix + 10 field + 3 suffix = 16 bytes. The eleven-byte negative field plus prefix is 14 bytes and is expected to reject, not succeed. Public positive witnesses are seven/eight bytes with a one-byte body. No recursion, enormous object, stack/pool pressure, unsupported numeric extent or impossible cursor is involved. Counts do not become parser input size. Default input capacity is 2 GiB; the public witness has offset+length=1. Real parse_jpip_message callers use the same None mask for Class and Msg-Length, so these are not merely disconnected private-function states.

The host build succeeded and host replay exited 101 with concrete value/boundary mismatches and both public-message errors. The failed assertion in the harness is the final failures.is_empty(), accumulating checks actual != Ok(expected) || position != prefix + field.len() and unexpected public-message results. The retained tail does not expose the earliest accumulated mismatch; I do not invent that first case. It does contain complete individual failures, including the observation quote and a direct unit example: read_vbas mask=None, field=[80, 81, 80, 00], start=0: expected Ok(16384) at 4, got Err(MalformedInput { offset: 0, kind: InvalidJpipVbas }) at 1. These directly contradict preservation of a bounded standard field's payload/boundary, rather than merely showing an arbitrary nonzero exit. Canonical and negative-control assertions preceding the final collection do not explain away these errors.

Authority: clause_origin is rfc_map dc9db665e258a9562dc090af, parent finding 28580cd70f92601009d53cc22676c850fe46dda58f07d6e8943318325d231890. I independently read original indexed T.808 (12/2022), SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc, A.2.1 anchor 811b789e3885dd767654c636fd2b2d671f29562ca6e2c162f494bf96cddafcff and A.2.2 anchor 02738b07ac711a51f6242015308bf1724b620044dca76fbb4b9f473ddb9b7bc1 through read_rfc_section using the parent mapping item. A.2.1 defines continuation-bit segmentation and explicitly defines Class, CSn, Msg-Offset and Msg-Length as nonnegative integers from seven-bit groups concatenated in big-endian order. [80,08] therefore denotes 8, [80,01] denotes 1; neither changes the terminal byte or following field. The inspected section imposes no shortest-form restriction. Table A.1 permits independent bb=11; A.2.2 assigns Class 8 to metadata in both stream types and Aux iff odd. This is receiving-side supported binary-header syntax, not excluded network/request/cache signaling or arbitrary metadata interpretation. The local profile advertises standard independent headers and checked bounded VBAS, without documenting a shortest-form exclusion. A corpus search for 'leading zero' returned no T.808 section; this search alone is not proof of global absence, but agrees with the full relevant syntax definition and supplied parent evidence.

Compiler evidence confirms the exact cause: describe_declaration reports a total Rust signature on slice/mutable cursor/optional mask. MIR CFG bb12 commits the first cursor increment, bb14 selects the None path, bb19 calls canonical(0,true), bb21 takes false to bb22/bb23 returning InvalidJpipVbas before loop bb24. The helper has no requires and correctly returns false per its own implementation/model. The defect is imposing that extra acceptance restriction in read_vbas, not violating the helper's contract. The checked arithmetic helper is not reached on this failure. Thus the unit entry is admissible and the complete-message witness also traverses the actual owning parser. Existing ghost canonical rules/native rejection tests describe the same restriction; they are not independent normative shortest-form authority and cannot justify spec_error.

Qualification: this is an agent-assessed reproduced divergence from the mapped, locally acquired 2022 syntax and the stated bounded-header contract, not whole-edition certification or formal reachability proof. REFERENCES.md explicitly says selected 2026 T.808 is only a catalog record without acquired text/edition reconciliation. PDF-converted figures were not independently reconciled; I do not establish that the 2026 edition is identical. Preserve that cross-edition/diagram uncertainty for review without rewriting this rfc_map clause to fit the implementation. All rejected redundant-zero encodings and both public witnesses share one root cause, including the host-noted F11 overlap; do not count them as separate defects. External diagnosis-guidance.json access was permission-denied; embedded host review_notes and original replay artifacts were read. No production/model/spec/test changes or additional execution were made.

- **Production evidence:** [`src/jpip.rs:L4-L6`](../../src/jpip.rs#L4-L6)

```text
//! This proof-of-concept profile accepts the standard binary message-header
//! forms with inherited, explicit-class, or explicit-class-and-codestream
//! identifiers.
```

- **Production evidence:** [`src/jpip.rs:L17`](../../src/jpip.rs#L17)

```text
const MAX_VBAS_BYTES: usize = 10;
```

- **Production evidence:** [`src/jpip.rs:L377-L389`](../../src/jpip.rs#L377-L389)

```text
    if first_mask.is_none()
        && !crate::refinement::jpip_vbas_first_is_canonical(
            u8::try_from(value).map_err(|_| {
                DecodeError::malformed(start as u64, MalformedKind::InvalidJpipVbas)
            })?,
            byte & 0x80 != 0,
        )
    {
        return Err(DecodeError::malformed(
            start as u64,
            MalformedKind::InvalidJpipVbas,
        ));
    }
```

- **Production evidence:** [`src/refinement.rs:L2347-L2349`](../../src/refinement.rs#L2347-L2349)

```text
pub(crate) const fn jpip_vbas_first_is_canonical(payload: u8, continued: bool) -> bool {
    !continued || payload > 0
}
```

- **Production evidence:** [`src/jpip.rs:L262-L263`](../../src/jpip.rs#L262-L263)

```text
    let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
```

- **Production evidence:** [`REFERENCES.md:L39-L40`](../../REFERENCES.md#L39-L40)

```text
The T.808 (06/2026) reference above identifies a catalog record, not an acquired
normative text or completed edition-difference review.
```

- **Production evidence:** [`src/jpip.rs:L17`](../../src/jpip.rs#L17)

```text
const MAX_VBAS_BYTES: usize = 10;
```

- **Production evidence:** [`src/jpip.rs:L370-L371`](../../src/jpip.rs#L370-L371)

```text
let mask = first_mask.unwrap_or(0x7f);
    let mut value = u64::from(first & mask);
```

- **Production evidence:** [`src/jpip.rs:L377-L383`](../../src/jpip.rs#L377-L383)

```text
if first_mask.is_none()
        && !crate::refinement::jpip_vbas_first_is_canonical(
            u8::try_from(value).map_err(|_| {
                DecodeError::malformed(start as u64, MalformedKind::InvalidJpipVbas)
            })?,
            byte & 0x80 != 0,
        )
```

- **Production evidence:** [`src/refinement.rs:L2347-L2349`](../../src/refinement.rs#L2347-L2349)

```text
pub(crate) const fn jpip_vbas_first_is_canonical(payload: u8, continued: bool) -> bool {
    !continued || payload > 0
}
```

- **Production evidence:** [`src/jpip.rs:L407-L411`](../../src/jpip.rs#L407-L411)

```text
let group = byte & 0x7f;
        let stepped = value
            .checked_mul(128)
            .and_then(|shifted| shifted.checked_add(u64::from(group)))
            .ok_or_else(|| DecodeError::malformed(start as u64, MalformedKind::InvalidJpipVbas))?;
```

- **Production evidence:** [`src/jpip.rs:L260`](../../src/jpip.rs#L260)

```text
let in_class = read_vbas(input, &mut position, Some(0x0f))?;
```

- **Harness evidence:** [`validation/jpip_vbas_contract.rs:L130-L134`](tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L130-L134): `    for (class, length) in [<br>        (&[8][..], &[1][..]),<br>        (&[0x80, 8][..], &[1][..]),<br>        (&[8][..], &[0x80, 1][..]),<br>    ] {`
- **Harness evidence:** [`validation/jpip_vbas_contract.rs:L135-L140`](tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L135-L140): `        let mut input = vec![0x60];<br>        input.extend_from_slice(class);<br>        input.extend_from_slice(&[0, 0]);<br>        input.extend_from_slice(length);<br>        let header_length = input.len();<br>        input.extend_from_slice(&[0xab, 0x5a]);`
- **Harness evidence:** [`validation/jpip_vbas_contract.rs:L142-L143`](tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L142-L143): `        let mut context = JpipHeaderContext::new();<br>        match parse_jpip_message(&input, &mut context, &ResourceLimits::default()) {`
- **Harness evidence:** [`validation/jpip_vbas_contract.rs:L157-L160`](tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L157-L160): `            other => failures.push(format!(<br>                "parse_jpip_message -> read_vbas, independent metadata header \<br>                 {input:02x?}: expected value-preserving Data, got {other:?}"<br>            )),`
- **Harness evidence:** [`validation/jpip_vbas_contract.rs:L163-L168`](tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L163-L168): `    assert!(<br>        failures.is_empty(),<br>        "{} standard VBAS checks failed:\n{}",<br>        failures.len(),<br>        failures.join("\n")<br>    );`
- **Test source:** [`validation/jpip_vbas_contract.rs:L130-L134`](tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L130-L134)<br>[`validation/jpip_vbas_contract.rs:L135-L140`](tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L135-L140)<br>[`validation/jpip_vbas_contract.rs:L142-L143`](tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L142-L143)<br>[`validation/jpip_vbas_contract.rs:L157-L160`](tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L157-L160)<br>[`validation/jpip_vbas_contract.rs:L163-L168`](tests/DEV-9A1ABE441F/files/validation/jpip_vbas_contract.rs#L163-L168)
- **Test source:** [`validation/install-jpip-vbas-contract.py`](tests/DEV-9A1ABE441F/files/validation/install-jpip-vbas-contract.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-A47C95C101

- **Mapping:** `dc9db665e258a9562dc090af` / `31268b3beec44bbd2e61921d15f62ac2544307539a6a4197c9bf82f8098dd3d5`
- **Specification:** `f927e4d42200fb374bda61d6`
- **Deviation:** `2afa90f245a7e27a3a0ff8b8`
- **Test item:** `66aff522bc300198b9d75ec3`
- **Case:** `1efdfed94ecf6fa7e50d4390`
- **Exact standards quote:**

> If the Class VBAS is
> not present, the message class identifier is unchanged from that associated with the previous message. If the Class VBAS
> is not present and there is no previous message, the message class identifier is 0.

- **Entry:** `src/jpip.rs:3504:jpip::ingest_jpip_response`
- **Command:** `cargo test --locked --lib jpip::tests::contract_ingest_header_forms_response_local -- --exact --nocapture`
- **Setup:** Safe DatabinCache::new(ResourceLimits::default()) and ordinary Rust slices/borrows; no private state mutation. Prior accepted responses leave only distinct metadata bins (class 8/codestream 127/bin 0, three bytes; class 8/codestream 0/bin 0, one byte), two fragments and four stored bytes. The new response [0x30,0,1,0] is a complete Annex A form-1 final precinct-bin-0 message with initial class/codestream defaults 0, offset 0 and one-byte body. Its semantic bin is absent, so it cannot conflict with those metadata bins; all resource counts/extents are far below default limits. This API has no transport-packet precondition, and no H.2 unreliable transport is invoked. Response-local reset is respected, not bypassed.
- **Route:** Host cargo test --locked --lib jpip::tests::contract_ingest_header_forms_response_local -- --exact --nocapture executes the registered additive test. After successful earlier checks, line 265 invokes contract_ingest(&mut cache, &initial_dependent, 1). The helper calls the actual public ingest_jpip_response and asserts result.is_ok() at line 62. Ingestion constructs a fresh parser, parses the first suffix at position 0, and propagates the parser's missing-initial-class error before any fragment insertion. This is the actual first failed check for the four-byte response, not a later counter or payload claim.
- **Observed assertion:** `input=[30, 00, 01, 00], result=Err(MalformedInput { offset: 0, kind: InvalidJpipHeader })`

**Full diagnosis:**

The first host-observed failed assertion is contract_ingest's result.is_ok() at src/jpip_ingest_contract_tests.rs:62 for input [30, 00, 01, 00], with Err(MalformedInput { offset: 0, kind: InvalidJpipHeader }). It is not a build failure, fixture failure, failed later byte assertion, or inference from exit status. The additive patch only includes a test module inside the existing cfg(test) tests module; host compilation and registration guards succeeded and this named native test actually executed.

Entry realism: this public unit-contract entry has no additional entry requirements. The failing response is four bytes, contains one nonrecursive message with a one-byte body, and uses class 0, codestream 0, bin 0, offset 0, final length 1. ResourceLimits::default() is HARDENED: maximum_input_bytes=2,147,483,648, maximum_metadata_bytes=16,777,216, maximum_jpip_fragments=1,048,576. The safely constructed cache contains only two compatible metadata fragments totaling four bytes before this call; successful acceptance would produce three bins, three accepted fragments, and five stored bytes. The earlier 144 class/bin/stream combinations per encoding allocate fresh caches, each holding only three messages/three bytes, rather than accumulating a large state. Large numeric identifiers in earlier successful sweeps are identifiers, not allocation sizes; the failing identifiers are all zero. There is no excessive stack, nesting, pool, buffer, or count demand. No external caller path is claimed: this is a permitted direct public API unit test, comparable in scale and invocation to the existing ingests_response_with_inherited_context regression. No fabricated parser/header state or private cache mutation is used.

Authority is the rfc_map origin dc9db665e258a9562dc090af, finding 31268b3beec44bbd2e61921d15f62ac2544307539a6a4197c9bf82f8098dd3d5, not a correlated native or Verus oracle. Independently reread the pinned T.808 (12/2022), A.2.1 (manifest SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc): absent initial Class defaults to 0 and absent initial CSn defaults to 0. Table A.1 permits bb=01 with neither field. Thus 0x30 is a complete form-1 Bin-ID, final flag set, in-class identifier 0; the remaining bytes encode offset 0, length 1, body 0x00. Class 0 requires no auxiliary field, and this API retains opaque precinct payload rather than decoding a complete codestream. Also reread H.2: its explicit-first-header requirement is for packets on an unreliable data connection. This harness invokes bare ordered Annex A message ingestion, with no such packet transport; the module explicitly excludes transport implementation. H.2 therefore does not invalidate this entry. The claim is scoped to the acquired 12/2022 edition, not conformance to a newer project-selected edition or official test assets.

Route: the harness's contract_ingest directly calls ingest_jpip_response. The wrapper creates JpipMessageParser::new(cache.limits), whose context has previous_class=None and previous_codestream=None regardless of prior calls sharing that cache. Its first parse receives the full four-byte slice. parse_jpip_message accepts the form and reads the initial in-class field, then requires context.previous_class through ok_or_else instead of using the standard's initial 0. It returns InvalidJpipHeader before reading offset, length, or body; ingestion propagates that error before fragment construction/insertion. Compiler declaration facts and MIR confirm ingestion bb0 initialization, bb3/bb4 parsing, bb5/bb6 Result branching and bb9 error propagation; parser bb31-bb35 is the missing-class Option error route. Even after repairing that first branch, the subsequent codestream branch also lacks the initial form-1 default, so both defaults belong to the same parser root cause. The consequence is rejection of a standard permitted response, not improper inheritance from an earlier response. The implementation-descriptive lifecycle clause that predicts this rejection does not override this standards-derived clause or the module's documented standard-header support. This is neither a wrong test oracle nor a spec overreach on these established premises.

Deduplicate with published F11/the existing parse-header finding and candidate a344db6b768a6a42a25deab2; this wrapper observation does not establish a new root-cause family. Preserve the existing missing-canonical-test-file/list-validation warnings and review labels: they do not erase the supplied original overlay and observed native failure, but no clean-list validation, formal reachability proof, exhaustive conformance, or execution of assertions after line 265 is claimed. No files, expectations, production code, models, or prior records were modified and no additional replay was substituted.

- **Production evidence:** [`src/jpip.rs:L4-L6`](../../src/jpip.rs#L4-L6)

```text
//! This proof-of-concept profile accepts the standard binary message-header
//! forms with inherited, explicit-class, or explicit-class-and-codestream
//! identifiers.
```

- **Production evidence:** [`src/jpip.rs:L3508`](../../src/jpip.rs#L3508)

```text
let mut parser = JpipMessageParser::new(cache.limits);
```

- **Production evidence:** [`src/jpip.rs:L109-L114`](../../src/jpip.rs#L109-L114)

```text
pub const fn new() -> Self {
        Self {
            previous_class: None,
            previous_codestream: None,
        }
    }
```

- **Production evidence:** [`src/jpip.rs:L262-L268`](../../src/jpip.rs#L262-L268)

```text
let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
```

- **Production evidence:** [`src/jpip.rs:L271-L277`](../../src/jpip.rs#L271-L277)

```text
} else if let Some(previous) = context.previous_codestream {
        previous
    } else if form == 2 {
        0
    } else {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipHeader));
    };
```

- **Production evidence:** [`src/jpip.rs:L3519`](../../src/jpip.rs#L3519)

```text
let (message, consumed) = parser.parse(&input[position..])?;
```

- **Production evidence:** [`src/jpip.rs:L3508`](../../src/jpip.rs#L3508)

```text
let mut parser = JpipMessageParser::new(cache.limits);
```

- **Production evidence:** [`src/jpip.rs:L109-L114`](../../src/jpip.rs#L109-L114)

```text
pub const fn new() -> Self {
        Self {
            previous_class: None,
            previous_codestream: None,
        }
    }
```

- **Production evidence:** [`src/jpip.rs:L262-L268`](../../src/jpip.rs#L262-L268)

```text
let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
```

- **Production evidence:** [`src/jpip.rs:L271-L277`](../../src/jpip.rs#L271-L277)

```text
} else if let Some(previous) = context.previous_codestream {
        previous
    } else if form == 2 {
        0
    } else {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipHeader));
    };
```

- **Harness evidence:** [`src/jpip_ingest_contract_tests.rs:L61-L62`](tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs#L61-L62): `let result = ingest_jpip_response(input, cache);<br>    assert!(result.is_ok(), "input={input:02x?}, result={result:?}");`
- **Harness evidence:** [`src/jpip_ingest_contract_tests.rs:L97`](tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs#L97): `let mut cache = DatabinCache::new(ResourceLimits::default());`
- **Harness evidence:** [`src/jpip_ingest_contract_tests.rs:L255-L261`](tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs#L255-L261): `let previous = DatabinId::new(8, 127, 0).unwrap();<br>    contract_ingest(<br>        &mut cache,<br>        &contract_message(3, previous, 0, b"old", true),<br>        1,<br>    );<br>    contract_ingest(&mut cache, &[0x50, 8, 0, 1, b'n'], 1);`
- **Harness evidence:** [`src/jpip_ingest_contract_tests.rs:L263-L265`](tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs#L263-L265): `let initial_dependent = [0x30, 0, 1, 0]; // precinct 0, one empty packet, final<br>    eprintln!("A.2.1 initial defaults: wire={initial_dependent:02x?}, expected class=0 CSn=0");<br>    contract_ingest(&mut cache, &initial_dependent, 1);`
- **Test source:** [`src/jpip_ingest_contract_tests.rs:L61-L62`](tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs#L61-L62)<br>[`src/jpip_ingest_contract_tests.rs:L97`](tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs#L97)<br>[`src/jpip_ingest_contract_tests.rs:L255-L261`](tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs#L255-L261)<br>[`src/jpip_ingest_contract_tests.rs:L263-L265`](tests/DEV-A47C95C101/files/src/jpip_ingest_contract_tests.rs#L263-L265)
- **Test source:** [`validation/jpip-ingest-contract.patch`](tests/DEV-A47C95C101/files/validation/jpip-ingest-contract.patch)
- **Exact mapping record:** [report.json](report.json)

## DEV-34CC912F98

- **Mapping:** `0987d00a9b563ca14d7ccefd` / `1960c1dc8f5ef400a3a690544c6b8ca91c0bdb02f9dc34e580ee01710af202d1`
- **Specification:** `c4f741cfbb88dfa62062114b`
- **Deviation:** `6ef3377eebd7db346c4a310b`
- **Test item:** `5419dc777157dc2a298419c7`
- **Case:** `a17a60fedfd5f588b3c6f6a3`
- **Exact standards quote:**

> If the Class VBAS
> is not present and there is no previous message, the message class identifier is 0.

- **Entry:** `src/jpip.rs:109:jpip::JpipHeaderContext::new`
- **Command:** `cargo test --locked --lib jpip::tests::fresh_header_context_preserves_absence_and_independent_standard_history -- --exact`
- **Setup:** Public constructor only; no private-state writes, injected faults, or fabricated history. Default ResourceLimits; three input bytes and zero body/offset are within capacity. Form 01, bin 0, canonical single-byte zero Msg-Offset/Msg-Length, no final flag, and no Aux for initial even class 0 satisfy T.808 (12/2022) A.2.1/A.2.2 in a fresh ordered JPP sequence with codestream 0 available. A direct header-parser unit test need not run cache insertion or image decoding. Absence of previous class/codestream is the legitimate initial state for which the standard defines both defaults.
- **Route:** The replayed cargo test invokes fresh_header_context_preserves_absence_and_independent_standard_history. After earlier checks, it constructs first_inherited via public JpipHeaderContext::new(), then calls check(&mut first_inherited, &limits, 1, 0, 0). check's wire helper creates [0x20,0x00,0x00], logs those bytes, and calls parse_jpip_message. The parser rejects absent previous_class and ? propagates that error through check to the final expect, producing the exact observed panic.
- **Observed assertion:** `fresh inherited header must select standard initial identity (0,0): MalformedInput { offset: 0, kind: InvalidJpipHeader }`

**Full diagnosis:**

The first host-observed failing check is the final expect("fresh inherited header must select standard initial identity (0,0)"): parsing [0x20,0x00,0x00] returns MalformedInput { offset: 0, kind: InvalidJpipHeader }. Earlier freshness, independence, explicit-form and established-history checks completed. Entry realism: the generated sweep makes 1,030 sequential parsing calls, each with only 3–5 header bytes, zero payload, zero offset, bin 0, no nesting and a constant number of small value contexts; it does not retain an aggregate buffer/cache or consume a fragment pool. Codestream indices are 0–63, within the default 64-codestream capacity, and inputs are far below the 2,147,483,648-byte default input limit. The failing call uses only codestream 0. Real callers JpipMessageParser::new and ingest_jpip_response likewise start with JpipHeaderContext::new and feed ordinary message bytes into this parser. This is a valid direct unit entry, not private-state fabrication or bypass of a required owning caller.

Read clause_origin: this is an rfc_map-derived clause, not merely the independent inferred constructor None/None postcondition. Independently read the pinned T.808 (12/2022), SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc, A.2.1 and A.2.2 through the originating map. Table A.1 permits indicator 01 with neither Class nor CSn. A.2.1 explicitly assigns class 0 and codestream 0 when those fields are absent and no previous message exists. Offset and length are non-negative, so zero is admissible. A.2.2 identifies class 0 as a JPP precinct message and requires Aux only for odd classes. Assume an ordered fresh JPP message sequence with codestream 0 available; header parsing does not require the fixture to supply codestream marker data. The implementation documents support for the standard inherited form without excluding the initial message. Thus the failed check exercises the supported-standard-forms meaning of c4f741cfbb88dfa62062114b; it does not demand changing the valid None/None representation. This conclusion is confined to the cited 2022 edition, not reconciliation with newer editions.

Compiler evidence for new shows a single block returning None/None with no callees. Parser MIR and source then trace the actual failure: nonempty input passes the size check and is not EOR; form=(0x20>>5)&3=1 is valid; read_vbas with mask 0x0f consumes the first byte as in-class 0; jpip_header_has_explicit_class(1) is false (MIR bb23/bb25), so bb31 copies previous_class=None and calls ok_or_else; bb32/bb33 select the error and bb35 propagates it to return bb94. This is the precise rejection seen by the host, before Msg-Offset/Msg-Length parsing or context updates. The absent-CSn branch also lacks the required form-1 initial default, but that is a secondary source observation, not a separately reproduced failure. The constructor itself correctly distinguishes absent history from Some(0); its parser consumer incorrectly treats that absence as forbidden instead of applying the standard defaults. A correlated native regression rejecting a fresh inherited header cannot override the cited standard. No production changes or additional replay are needed to attribute this already host-reproduced failure.

- **Production evidence:** [`src/jpip.rs:L4-L6`](../../src/jpip.rs#L4-L6)

```text
//! This proof-of-concept profile accepts the standard binary message-header
//! forms with inherited, explicit-class, or explicit-class-and-codestream
//! identifiers.
```

- **Production evidence:** [`src/jpip.rs:L109-L114`](../../src/jpip.rs#L109-L114)

```text
    pub const fn new() -> Self {
        Self {
            previous_class: None,
            previous_codestream: None,
        }
    }
```

- **Production evidence:** [`src/jpip.rs:L262-L268`](../../src/jpip.rs#L262-L268)

```text
    let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
```

- **Production evidence:** [`src/jpip.rs:L269-L277`](../../src/jpip.rs#L269-L277)

```text
    let codestream = if crate::refinement::jpip_header_has_explicit_codestream(form) {
        read_vbas(input, &mut position, None)?
    } else if let Some(previous) = context.previous_codestream {
        previous
    } else if form == 2 {
        0
    } else {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipHeader));
    };
```

- **Production evidence:** [`src/refinement.rs:L2326-L2328`](../../src/refinement.rs#L2326-L2328)

```text
pub(crate) const fn jpip_header_has_explicit_class(form: u8) -> bool {
    form >= 2
}
```

- **Production evidence:** [`src/jpip.rs:L219-L224`](../../src/jpip.rs#L219-L224)

```text
    pub const fn new(limits: ResourceLimits) -> Self {
        Self {
            context: JpipHeaderContext::new(),
            limits,
        }
    }
```

- **Production evidence:** [`src/jpip.rs:L219-L224`](../../src/jpip.rs#L219-L224)

```text
pub const fn new(limits: ResourceLimits) -> Self {
        Self {
            context: JpipHeaderContext::new(),
            limits,
        }
    }
```

- **Production evidence:** [`src/jpip.rs:L262-L268`](../../src/jpip.rs#L262-L268)

```text
let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
```

- **Production evidence:** [`src/refinement.rs:L2326-L2328`](../../src/refinement.rs#L2326-L2328)

```text
pub(crate) const fn jpip_header_has_explicit_class(form: u8) -> bool {
    form >= 2
}
```

- **Harness evidence:** [`src/jpip_header_context_contract_tests.rs:L86-L90`](tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs#L86-L90): `    let mut first_inherited = JpipHeaderContext::new();<br>    // A.2.1 supplies class=0 and CSn=0 when there is no previous message.<br>    // No codestream body/marker validation is required for header parsing.<br>    check(&mut first_inherited, &limits, 1, 0, 0)<br>        .expect("fresh inherited header must select standard initial identity (0,0)");`
- **Harness evidence:** [`src/jpip_header_context_contract_tests.rs:L9-L17`](tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs#L9-L17): `        let mut bytes = vec![form << 5];<br>        if form >= 2 {<br>            bytes.push(class as u8);<br>        }<br>        if form == 3 {<br>            bytes.push(codestream as u8);<br>        }<br>        bytes.extend([0, 0]);<br>        bytes`
- **Harness evidence:** [`src/jpip_header_context_contract_tests.rs:L33`](tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs#L33): `        let (message, consumed) = parse_jpip_message(&bytes, context, limits)?;`
- **Harness evidence:** [`src/jpip_header_context_contract_tests.rs:L57-L58`](tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs#L57-L58): `    let limits = ResourceLimits::default();<br>    assert_eq!(limits.maximum_codestreams(), 64);`
- **Test source:** [`src/jpip_header_context_contract_tests.rs:L86-L90`](tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs#L86-L90)<br>[`src/jpip_header_context_contract_tests.rs:L9-L17`](tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs#L9-L17)<br>[`src/jpip_header_context_contract_tests.rs:L33`](tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs#L33)<br>[`src/jpip_header_context_contract_tests.rs:L57-L58`](tests/DEV-34CC912F98/files/src/jpip_header_context_contract_tests.rs#L57-L58)
- **Test source:** [`tests/jpip-context-registration.patch`](tests/DEV-34CC912F98/files/tests/jpip-context-registration.patch)
- **Exact mapping record:** [report.json](report.json)

## DEV-30865B7D2D

- **Mapping:** `644e28e929d66e2e5c7a925f` / `b727900984d6434bcd30d09b418bce2c91735a26e0ef469173c0f65b4d8f9c59`
- **Specification:** `a3f0825227645d309edff279`
- **Deviation:** `9368214fac938ede78f24ab7`
- **Test item:** `2d56080287e2a81af4e09946`
- **Case:** `07be402c570ea0a3c7248a7d`
- **Exact standards quote:**

> If the Class VBAS
> is not present and there is no previous message, the message class identifier is 0.

- **Entry:** `src/jpip.rs:235:jpip::parse_jpip_message`
- **Command:** `python3 validation/run_jpip_contract.py run forms`
- **Setup:** The context is constructed by JpipHeaderContext::new(), not fabricated or privately mutated; there is deliberately no previous message. ResourceLimits::default().with_maximum_input_bytes(65536).unwrap() is valid. The five-byte message has form 01, class/CSn omitted as A.2.1 permits with zero defaults, in-class identifier 0, offset 0, body length 2, complete body [0x12,0x34], last=false and no Aux for class 0. Its complete canonical one-byte fields and extent 2 satisfy the bounded parser and read_vbas contracts. All 20 observed variants remain 5-7 bytes, so no platform capacity issue or owning-caller requirement is skipped. This is direct header parsing, not an assertion that the body is a fully decodable JPEG2000 image.
- **Route:** validation/run_jpip_contract.py run forms invokes the compiled libtest case jpip::tests::contract_standard_header_forms with --exact --nocapture. Its final boundary loop encodes a first form-01 message and calls parse_jpip_message with a new public context. For bin=0,last=false the exact bytes are [0x20,0x00,0x02,0x12,0x34]. The parser accepts the size/form and Bin-ID VBAS, then errors on absent previous_class (MIR bb31 -> bb35). The test records the unexpected error and fails its final rejected.is_empty() assertion.
- **Observed assertion:** `A.2.1 first form-01 bytes=[20, 00, 02, 12, 34]: expected class=0, CSn=0; got Err(MalformedInput { offset: 0, kind: InvalidJpipHeader })`

**Full diagnosis:**

The first failed assertion is src/jpip_contract_tests.rs:383, assert!(rejected.is_empty(), ...), after all preceding header-form sweeps and negative checks succeeded. Host case 07be402c570ea0a3c7248a7d specifically reports the five-byte input [20, 00, 02, 12, 34] returning InvalidJpipHeader rather than a data message with class=0 and CSn=0. This violates clause a3f0825227645d309edff279's acceptance of standard binary header forms, not an assertion about image decoding or full-response conformance.

Entry realism: the minimal call supplies only five bytes, a two-byte body, offset zero, in-class identifier zero, and no recursion, cache allocation or nested structure. The 20 fresh-context variants use 5-7 bytes each, in-class identifiers at most 16384, and the same two-byte body; input size and extent 2 are far below the configured 65536-byte ceiling. The preceding 280 sequential fixtures use bodies at most 256 bytes and offsets below 32000 (boundary offsets at most 16384); these too stay below that ceiling. Deliberately malformed/overflowing fixtures assert rejection and passed, so they are not the failing admission. No implicit stack, buffer or pool exhaustion explains this failure. The actual JpipMessageParser::new creates the same fresh context and parse delegates directly to parse_jpip_message; ingest_jpip_response likewise creates a fresh parser before reading the first message. This is also an available direct public unit-contract entry with no additional requires, so no owning caller or image reconstruction state is being bypassed.

Authority: clause_origin is rfc_map. I independently read its pinned T.808 (12/2022), carrying ISO/IEC 15444-9:2023, A.2.1 using the originating mapping item. Table A.1 defines 01 as no Class or CSn; the prose explicitly defaults missing Class and missing CSn to zero when there is no previous message. The fixture is a general ordered Annex-A JPP header, not a K.1 HTTP-UDP chunk. The public bounded parser does not impose that transport-specific first-message requirement. Class zero is supported, no Aux is required, the VBAS encodings are canonical and complete, and opaque body inspection does not require decoding a precinct. Thus neither a harness precondition error nor revising the clause to match the implementation is justified. This diagnosis is bounded to the cited 12/2022 edition and documented parser promise; the parent finding does not establish comparison with the separately cataloged 06/2026 edition, and I make no newer-edition conformance claim.

Route: Python's forms command selects the compiled exact native test jpip::tests::contract_standard_header_forms. Its final loop constructs form=1 using contract_data_bytes, creates JpipHeaderContext::new(), and directly invokes parse_jpip_message. Compiler declaration/CFG and source agree: nonempty/size/EOR checks pass; 0x20 gives form=1, jpip_header_form_valid is true, and read_vbas at position zero with mask 0x0f succeeds with in_class=0. jpip_header_has_explicit_class(1) is false, so MIR bb25 -> bb31 reads previous_class=None; ok_or_else produces InvalidJpipHeader and bb35 returns the error, before any body parsing or context update. All callee input/reference requirements on this route are met. The codestream fallback also defaults only form 2 and would reject a fresh form 1 even if the earlier class rejection were repaired alone. Consequence: admissible initial dependent headers are rejected instead of receiving standard zero identifiers. Existing native/model expectations of initial rejection are correlated implementation facts, not authority overriding A.2.1. The authoritative failure is the explicit unexpected parse result and rejected-list assertion, not merely exit 101; the separate ancillary report_deviation build error is not used as reproduction evidence.

- **Production evidence:** [`src/jpip.rs:L4-L6`](../../src/jpip.rs#L4-L6)

```text
//! This proof-of-concept profile accepts the standard binary message-header
//! forms with inherited, explicit-class, or explicit-class-and-codestream
//! identifiers.
```

- **Production evidence:** [`src/jpip.rs:L111-L112`](../../src/jpip.rs#L111-L112)

```text
            previous_class: None,
            previous_codestream: None,
```

- **Production evidence:** [`src/jpip.rs:L262-L268`](../../src/jpip.rs#L262-L268)

```text
    let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
```

- **Production evidence:** [`src/jpip.rs:L273-L277`](../../src/jpip.rs#L273-L277)

```text
    } else if form == 2 {
        0
    } else {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipHeader));
    };
```

- **Production evidence:** [`src/refinement.rs:L2326-L2328`](../../src/refinement.rs#L2326-L2328)

```text
pub(crate) const fn jpip_header_has_explicit_class(form: u8) -> bool {
    form >= 2
}
```

- **Production evidence:** [`src/jpip.rs:L230-L232`](../../src/jpip.rs#L230-L232)

```text
    pub fn parse<'a>(&mut self, input: &'a [u8]) -> Result<(JpipMessage<'a>, usize), DecodeError> {
        parse_jpip_message(input, &mut self.context, &self.limits)
    }
```

- **Production evidence:** [`src/jpip.rs:L111-L112`](../../src/jpip.rs#L111-L112)

```text
            previous_class: None,
            previous_codestream: None,
```

- **Production evidence:** [`src/jpip.rs:L262-L268`](../../src/jpip.rs#L262-L268)

```text
    let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
```

- **Production evidence:** [`src/jpip.rs:L273-L277`](../../src/jpip.rs#L273-L277)

```text
    } else if form == 2 {
        0
    } else {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipHeader));
    };
```

- **Production evidence:** [`src/refinement.rs:L2326-L2328`](../../src/refinement.rs#L2326-L2328)

```text
pub(crate) const fn jpip_header_has_explicit_class(form: u8) -> bool {
    form >= 2
}
```

- **Harness evidence:** [`src/jpip_contract_tests.rs:L184-L186`](tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs#L184-L186): `    let limits = ResourceLimits::default()<br>        .with_maximum_input_bytes(65536)<br>        .unwrap();`
- **Harness evidence:** [`src/jpip_contract_tests.rs:L309-L317`](tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs#L309-L317): `    let data = ContractData {<br>        class: 0,<br>        stream: 0,<br>        bin: 0,<br>        offset: 0,<br>        last: false,<br>        auxiliary: None,<br>        body: Vec::new(),<br>    };`
- **Harness evidence:** [`src/jpip_contract_tests.rs:L354-L361`](tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs#L354-L361): `            let mut first = data.clone();<br>            first.bin = bin;<br>            first.last = last;<br>            first.body = vec![0x12, 0x34];<br>            let (bytes, header_length) = contract_data_bytes(&first, 1);<br>            assert!(bytes.len() as u64 <= limits.maximum_input_bytes());<br>            let mut fresh = JpipHeaderContext::new();<br>            match parse_jpip_message(&bytes, &mut fresh, &limits) {`
- **Harness evidence:** [`src/jpip_contract_tests.rs:L383`](tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs#L383): `    assert!(rejected.is_empty(), "A.2.1 first-header defaults rejected: {rejected:?}");`
- **Harness evidence:** [`validation/run_jpip_contract.py:L64-L66`](tests/DEV-30865B7D2D/files/validation/run_jpip_contract.py#L64-L66): `    return subprocess.run(<br>        [binary, name, "--exact", "--nocapture"], cwd=ROOT<br>    ).returncode`
- **Test source:** [`src/jpip_contract_tests.rs:L184-L186`](tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs#L184-L186)<br>[`src/jpip_contract_tests.rs:L309-L317`](tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs#L309-L317)<br>[`src/jpip_contract_tests.rs:L354-L361`](tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs#L354-L361)<br>[`src/jpip_contract_tests.rs:L383`](tests/DEV-30865B7D2D/files/src/jpip_contract_tests.rs#L383)
- **Test source:** [`validation/run_jpip_contract.py:L64-L66`](tests/DEV-30865B7D2D/files/validation/run_jpip_contract.py#L64-L66)
- **Exact mapping record:** [report.json](report.json)

## DEV-F1E3AE04A3

- **Mapping:** `c3e124e770eb767b4dca0e91` / `ffeed0a3342d0e813d9226d596e3080256b9aa9ddff9ae787c5ab31bc04a96f5`
- **Specification:** `43f15ffa053ac75972b21f66`
- **Deviation:** `ad8869565c7aded2454bb8f6`
- **Test item:** `8bd5786813557fc358538a36`
- **Case:** `b1b7a6f7872efc32ef4a1492`
- **Exact standards quote:**

> If the Class VBAS
> is not present and there is no previous message, the message class identifier is 0.

- **Entry:** `src/jpip.rs:230:jpip::JpipMessageParser::parse`
- **Command:** `python3 deepspec_jpip_replay.py run jpip::tests::deepspec_standard_header_forms`
- **Setup:** Public constructor establishes an ordinary valid fresh parser, without private-field mutation or skipped initialization. A.2.1 in the acquired T.808 (12/2022)/ISO 15444-9:2023 text defines absent initial Class and CSn as zero. Form 01 is permitted, in-class=0 is a supported precinct identifier, offset and body length are zero, no auxiliary is required for class zero, and all three wire bytes are present. Three bytes and zero extent are far below default InputBytes=2,147,483,648; no allocation, stack-depth or pool stress is involved. The module promises inherited standard headers and the real response consumer also begins with the same unprimed public constructor. Opaque body framing is the unit contract, not image decoding.
- **Route:** Host build applies deepspec_jpip_parse.patch as additive native unit tests and compiles cargo test --locked --lib. The exact deepspec_standard_header_forms case constructs SpecMessage(class=0,stream=0,bin=0,offset=0,empty body,last=false,auxiliary=None), serializes it using form 1 to [0x20,0,0], constructs JpipMessageParser::new(ResourceLimits::default()), and calls parser.parse(&wire). The wrapper directly calls parse_jpip_message; its omitted-class branch rejects the constructor's None context. The harness pushes the resulting error into failures and the aggregate failures.is_empty() assertion fails with the quoted diagnostic.
- **Observed assertion:** `A.2.1 zero defaults violated:<br>first inherited [20, 00, 00]: MalformedInput { offset: 0, kind: InvalidJpipHeader }`

**Full diagnosis:**

The first failed assertion is failures.is_empty() in deepspec_standard_header_forms, reporting 'A.2.1 zero defaults violated' and specifically rejecting [0x20,0x00,0x00] with InvalidJpipHeader. This violates clause 43f15ffa053ac75972b21f66, not an implementation-derived acceptance oracle. Its origin is rfc_map finding ffeed0a3342d0e813d9226d596e3080256b9aa9ddff9ae787c5ab31bc04a96f5. I read the actual cited A.2.1 passage through the parent mapping item: T.808 (12/2022), ISO/IEC 15444-9:2023, SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc. Table A.1 permits bb=01, omitting Class and CSn; the text explicitly makes each identifier zero when absent and no previous message exists. Msg-Offset and Msg-Length are nonnegative, so offset=length=0 is admissible. This applies to receiving binary headers, which module documentation explicitly supports; no transport, server, complete-image or cache-reassembly precondition is needed for this direct parser unit contract. The edition uncertainty in the mapping remains: this finding establishes the cited 2022/2023 rule, not independent conformance to the catalog's newer 2026 edition.

Entry realism: the failing family contains twelve sequential constructor-based calls, each with a 3- or 4-byte header, no body, zero offset/length, bin identifier at most 128, and no recursion, fragment pool or fabricated context. ResourceLimits::default() is HARDENED with maximum_input_bytes=2,147,483,648. Earlier successful generated cases use small wire buffers and offsets below 65,536, so no implicit resource limit is approached. The real ingest_jpip_response caller likewise constructs JpipMessageParser::new(cache.limits) and immediately passes the first response slice to parse without priming inheritance. This test does not skip an owning caller required by the unit contract.

The replay patch is additive unit-test code and its Python runner builds the normal cargo --locked --lib test binary, selects the exact registered case, and verifies that one test runs. describe_declaration confirms parse directly delegates to parse_jpip_message. The delegate CFG and source show the route: nonempty input passes the whole-slice ceiling; 0x20 is not EOR; form=(0x20>>5)&3=1 is valid; masked read_vbas returns in-class zero; has_explicit_class(1) is false; new() supplied previous_class=None; ok_or_else at src/jpip.rs:265-267 therefore returns InvalidJpipHeader before reading offset or length. parse propagates that error unchanged and the test records it before failing its explicit zero-default assertion. The codestream branch also lacks the initial form-1 default, although that branch is not reached in this observed failure. Consequence: a standard first inherited data header is rejected rather than parsed as class 0, codestream 0. Existing implementation contracts accurately describing this stricter behavior do not override the standards-derived documented contract. The authoritative output exposes both the failing check and rejected wire; attribution does not rely on the runner's claimed Route label or exit code alone.

- **Production evidence:** [`src/jpip.rs:L4-L6`](../../src/jpip.rs#L4-L6)

```text
//! This proof-of-concept profile accepts the standard binary message-header
//! forms with inherited, explicit-class, or explicit-class-and-codestream
//! identifiers.
```

- **Production evidence:** [`src/jpip.rs:L109-L114`](../../src/jpip.rs#L109-L114)

```text
pub const fn new() -> Self {
        Self {
            previous_class: None,
            previous_codestream: None,
        }
    }
```

- **Production evidence:** [`src/jpip.rs:L230-L232`](../../src/jpip.rs#L230-L232)

```text
pub fn parse<'a>(&mut self, input: &'a [u8]) -> Result<(JpipMessage<'a>, usize), DecodeError> {
        parse_jpip_message(input, &mut self.context, &self.limits)
    }
```

- **Production evidence:** [`src/jpip.rs:L262-L268`](../../src/jpip.rs#L262-L268)

```text
let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
```

- **Production evidence:** [`src/jpip.rs:L3508`](../../src/jpip.rs#L3508)

```text
let mut parser = JpipMessageParser::new(cache.limits);
```

- **Production evidence:** [`src/limits.rs:L133`](../../src/limits.rs#L133)

```text
maximum_input_bytes: 2_147_483_648,
```

- **Production evidence:** [`src/jpip.rs:L109-L114`](../../src/jpip.rs#L109-L114)

```text
pub const fn new() -> Self {
        Self {
            previous_class: None,
            previous_codestream: None,
        }
    }
```

- **Production evidence:** [`src/jpip.rs:L262-L268`](../../src/jpip.rs#L262-L268)

```text
let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
```

- **Production evidence:** [`src/jpip.rs:L273-L277`](../../src/jpip.rs#L273-L277)

```text
} else if form == 2 {
        0
    } else {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipHeader));
    };
```

- **Harness evidence:** [`deepspec_jpip_parse.patch:L117`](tests/DEV-F1E3AE04A3/files/deepspec_jpip_parse.patch#L117): `+        let limits = ResourceLimits::default();`
- **Harness evidence:** [`deepspec_jpip_parse.patch:L175-L193`](tests/DEV-F1E3AE04A3/files/deepspec_jpip_parse.patch#L175-L193): `+        for bin in [0, 1, 15, 16, 127, 128] {<br>+            for last in [false, true] {<br>+                let fixture = SpecMessage {<br>+                    class: 0,<br>+                    stream: 0,<br>+                    bin,<br>+                    offset: 0,<br>+                    body: Vec::new(),<br>+                    last,<br>+                    auxiliary: None,<br>+                };<br>+                let (wire, _) = fixture.wire(1);<br>+                let mut parser = JpipMessageParser::new(limits);<br>+                match parser.parse(&wire) {<br>+                    Ok(_) => fixture.assert_parse(&mut JpipMessageParser::new(limits), 1, false),<br>+                    Err(error) => failures.push(format!("first inherited {wire:02x?}: {error:?}")),<br>+                }<br>+            }<br>+        }`
- **Harness evidence:** [`deepspec_jpip_parse.patch:L194-L198`](tests/DEV-F1E3AE04A3/files/deepspec_jpip_parse.patch#L194-L198): `+        assert!(<br>+            failures.is_empty(),<br>+            "A.2.1 zero defaults violated:\n{}",<br>+            failures.join("\n")<br>+        );`
- **Harness evidence:** [`deepspec_jpip_replay.py:L38`](tests/DEV-F1E3AE04A3/files/deepspec_jpip_replay.py#L38): `["cargo", "test", "--locked", "--lib", "--no-run", "--message-format=json"]`
- **Test source:** [`deepspec_jpip_parse.patch:L117`](tests/DEV-F1E3AE04A3/files/deepspec_jpip_parse.patch#L117)<br>[`deepspec_jpip_parse.patch:L175-L193`](tests/DEV-F1E3AE04A3/files/deepspec_jpip_parse.patch#L175-L193)<br>[`deepspec_jpip_parse.patch:L194-L198`](tests/DEV-F1E3AE04A3/files/deepspec_jpip_parse.patch#L194-L198)
- **Test source:** [`deepspec_jpip_replay.py:L38`](tests/DEV-F1E3AE04A3/files/deepspec_jpip_replay.py#L38)
- **Exact mapping record:** [report.json](report.json)

## DEV-772E004106

- **Mapping:** `fba8839f9712aeba8ceb5246` / `cde71a42a0b62c19b49c59b430c36414cc4788933d0c7227239551ce9fc9f928`
- **Specification:** `45eff4865fe38a6f814ee21b`
- **Deviation:** `f26860378c67ef91480cc6ae`
- **Test item:** `901c8e46d43b2c36f29e9cc9`
- **Case:** `6586aa830a6ecaabb80bb964`
- **Exact standards quote:**

> If the Class VBAS is not present and there is no previous message, the message class identifier is 0.

- **Entry:** `src/jpip.rs:3504:jpip::ingest_jpip_response`
- **Command:** `cargo test --locked --lib jpip::tests::contract_ingest_standard_headers_and_response_local_defaults -- --exact --nocapture`
- **Setup:** The caller uses DatabinCache::new(ResourceLimits::default()), DatabinId::new(0,0,0), and safe ingestion only. For the first failing instance, one preexisting metadata databin holds 'm'; the addressed precinct databin is absent. 0x30 encodes permitted form 01, final=true, bin=0; offset=0, length=1 and payload='p' are complete, followed by EOR [0,2,0]. Both missing identifiers default to zero under the cited T.808 A.2.1. Seven input bytes, two eventual stored bytes, and two eventual accepted fragments are far within configured capacities. No transport-packet constraint is asserted: this is a complete ordered de-framed response, and this safe public entry owns parser creation.
- **Route:** Host replay ran cargo test --locked --lib jpip::tests::contract_ingest_standard_headers_and_response_local_defaults -- --exact --nocapture. The test creates a safe cache, successfully ingests a class-8 metadata response, then constructs a new form-1 response for class/codestream defaults zero and calls ingest_jpip_response directly. That invocation creates a fresh parser, calls parse on the entire seven-byte response, and returns the missing-previous-class InvalidJpipHeader error. The test collects that actual Err and the final failures.is_empty() assertion fails. Host output identifies the exact first wire as first form-1 header: primed=false, bin=0, bytes=[48, 0, 1, 112, 0, 2, 0].
- **Observed assertion:** `T.808 A.2.1 initial inherited-header defaults violated:<br>primed=false, bin=0: MalformedInput { offset: 0, kind: InvalidJpipHeader }`

**Full diagnosis:**

The first failed assertion is failures.is_empty() at src/jpip_ingest_contract_tests.rs:156, reporting specifically 'T.808 A.2.1 initial inherited-header defaults violated' and InvalidJpipHeader for the initial form-1 messages. This is not an arbitrary nonzero exit: the host successfully built the additively registered library test and observed all twelve target calls returning that error. Earlier explicit/in-response inherited-header controls succeeded.

The entry is realistic and admissible. Each failing response is only 7–9 bytes, with one final one-byte Data message at offset zero and a complete three-byte EOR. The selected case's other sequences are at most 38 bytes and four Data messages; each loop creates its own safe cache. Default limits are HARDENED: 2,147,483,648 input bytes, 16,777,216 metadata bytes, and 1,048,576 accepted fragments. At the first failing call the cache contains only one successfully ingested metadata byte (class 8, codestream 0); no precinct databin exists, so accepting class 0, codestream 0, bin 0 would not conflict. The optional priming path adds only a distinct three-byte metadata databin, through ordinary successful ingestion. There is no unsafe/private-state fabrication, deep nesting, capacity pressure, or skipped owning caller. This is a public direct-unit-contract call with complete, ordered, de-framed messages, comparable to the existing 13-byte native ingestion fixture; no production callers are listed. Payload contents are opaque at this byte-ingestion boundary, not a claim that one byte forms a decodable image.

The route is test -> ingest_jpip_response -> new response-local JpipMessageParser -> parse -> parse_jpip_message. The ingestion MIR confirms construction at bb0 and parser invocation/error propagation in the initial loop. New context has previous_class=None and previous_codestream=None. For [48,0,1,112,0,2,0], the leading 0x30 is permitted form 01 with final flag and in-class identifier zero; the remaining Data fields encode offset 0, length 1, payload 'p'. The parser reads the complete Bin-ID, then requires previous_class via ok_or_else and returns InvalidJpipHeader at offset zero, before fragment construction, insertion, or EOR. Consequently a valid standard message is rejected and its payload is not cached. The additional codestream branch likewise only supplies an initial zero for form 2, although the observed failure happens earlier at missing class.

Authority is the rfc_map parent, not the implementation-correlated lifecycle inference that describes rejection. I read its document manifest, finding and applicability, and independently retrieved the full cited A.2.1 section through the parent item. T.808 (12/2022), ISO/IEC 15444-9:2023, explicitly says absent Class with no previous message means class 0, and absent CSn with no previous message means codestream 0; Table A.1 permits indicator 01 with neither field. This applies to this ordered message-sequence consumer, not an HTTP-UDP packet entry with an independent-first-header requirement. The module advertises standard inherited-header support. Thus this exact standards-derived clause is sound for the cited edition, and neither test_error nor revision to match existing rejection is justified. Edition scope remains the supplied 12/2022 document (manifest SHA-256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc); this does not establish conformance or differences for the newer catalogued 2026 edition.

- **Production evidence:** [`src/jpip.rs:L4-L6`](../../src/jpip.rs#L4-L6)

```text
//! This proof-of-concept profile accepts the standard binary message-header
//! forms with inherited, explicit-class, or explicit-class-and-codestream
//! identifiers.
```

- **Production evidence:** [`src/jpip.rs:L3508`](../../src/jpip.rs#L3508)

```text
let mut parser = JpipMessageParser::new(cache.limits);
```

- **Production evidence:** [`src/jpip.rs:L109-L114`](../../src/jpip.rs#L109-L114)

```text
pub const fn new() -> Self {
        Self {
            previous_class: None,
            previous_codestream: None,
        }
    }
```

- **Production evidence:** [`src/jpip.rs:L262-L268`](../../src/jpip.rs#L262-L268)

```text
let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
```

- **Production evidence:** [`src/jpip.rs:L3519`](../../src/jpip.rs#L3519)

```text
let (message, consumed) = parser.parse(&input[position..])?;
```

- **Production evidence:** [`src/jpip.rs:L271-L277`](../../src/jpip.rs#L271-L277)

```text
} else if let Some(previous) = context.previous_codestream {
        previous
    } else if form == 2 {
        0
    } else {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipHeader));
    };
```

- **Production evidence:** [`src/jpip.rs:L3508`](../../src/jpip.rs#L3508)

```text
let mut parser = JpipMessageParser::new(cache.limits);
```

- **Production evidence:** [`src/jpip.rs:L109-L114`](../../src/jpip.rs#L109-L114)

```text
pub const fn new() -> Self {
        Self {
            previous_class: None,
            previous_codestream: None,
        }
    }
```

- **Production evidence:** [`src/jpip.rs:L262-L268`](../../src/jpip.rs#L262-L268)

```text
let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
```

- **Production evidence:** [`src/jpip.rs:L271-L277`](../../src/jpip.rs#L271-L277)

```text
} else if let Some(previous) = context.previous_codestream {
        previous
    } else if form == 2 {
        0
    } else {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipHeader));
    };
```

- **Harness evidence:** [`src/jpip_ingest_contract_tests.rs:L96`](tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L96): `let mut cache = DatabinCache::new(ResourceLimits::default());`
- **Harness evidence:** [`src/jpip_ingest_contract_tests.rs:L129-L131`](tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L129-L131): `let target = DatabinId::new(0, 0, bin).unwrap();<br>            let mut wire = contract_data(1, target, 0, b"p", true);<br>            wire.extend(contract_eor(2, b""));`
- **Harness evidence:** [`src/jpip_ingest_contract_tests.rs:L133`](tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L133): `match ingest_jpip_response(&wire, &mut cache) {`
- **Harness evidence:** [`src/jpip_ingest_contract_tests.rs:L141`](tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L141): `Err(error) => failures.push(format!("primed={primed}, bin={bin}: {error:?}")),`
- **Harness evidence:** [`src/jpip_ingest_contract_tests.rs:L156-L160`](tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L156-L160): `assert!(<br>        failures.is_empty(),<br>        "T.808 A.2.1 initial inherited-header defaults violated:\n{}",<br>        failures.join("\n")<br>    );`
- **Harness evidence:** [`src/jpip_ingest_contract_tests.rs:L2`](tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L2): `// These are de-framed, ordered JPP message sequences, not HTTP-UDP packets.`
- **Test source:** [`src/jpip_ingest_contract_tests.rs:L96`](tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L96)<br>[`src/jpip_ingest_contract_tests.rs:L129-L131`](tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L129-L131)<br>[`src/jpip_ingest_contract_tests.rs:L133`](tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L133)<br>[`src/jpip_ingest_contract_tests.rs:L141`](tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L141)<br>[`src/jpip_ingest_contract_tests.rs:L156-L160`](tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L156-L160)<br>[`src/jpip_ingest_contract_tests.rs:L2`](tests/DEV-772E004106/files/src/jpip_ingest_contract_tests.rs#L2)
- **Test source:** [`validation/register-jpip-contract-tests.py`](tests/DEV-772E004106/files/validation/register-jpip-contract-tests.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-CA85C0D7B5

- **Mapping:** `1982dbd9425738d5a0010c88` / `47eb9d79995385410e28c2ba4026bf488caa064e407acf7f02a641f9d6431f01`
- **Specification:** `e747fe080bc71ebe1d1c602a`
- **Deviation:** `fc071e2951e754e38ccd77ae`
- **Test item:** `088604b66ca5feb8fa8bce96`
- **Case:** `f7af47278d5dee22cba50ede`
- **Exact standards quote:**

> If the Class VBAS is
> not present, the message class identifier is unchanged from that associated with the previous message. If the Class VBAS
> is not present and there is no previous message, the message class identifier is 0.

- **Entry:** `src/jpip.rs:116:jpip::JpipHeaderContext::previous_class`
- **Command:** `cargo test --locked --lib jpip::tests::parses_all_three_standard_header_forms_and_auxiliary -- --exact --nocapture`
- **Setup:** Fresh safe public new() context has no predecessor, exactly the initial-message condition specified by T.808 A.2.1. The input is a complete three-byte form-01 header: Bin-ID 1, omitted Class and CSn defaulting to zero, offset zero, length zero, no Aux and no body. It is an admissible direct parser/context unit-contract entry, with ordered operations and no private-state writes, skipped state-owning caller, size violation, or payload-decoding claim.
- **Route:** Host cargo test selects jpip::tests::parses_all_three_standard_header_forms_and_auxiliary. The replay's test-only registration invokes check_inherited_class_contract(). After the successful seven-class sweep, its initial-default loop takes JpipHeaderContext::new(), builds and asserts initial == [0x21, 0, 0], and calls accept(). accept() calls the public parse_jpip_message with default resource limits. Parsing form 01 selects the omitted-class branch; None becomes InvalidJpipHeader and result.unwrap_or_else panics before the post-parse previous_class() assertion.
- **Observed assertion:** `valid dependent/independent header must parse: MalformedInput { offset: 0, kind: InvalidJpipHeader }`

**Full diagnosis:**

The first failing check is accept()'s requirement that parse_jpip_message succeed on [0x21, 0x00, 0x00] with a fresh JpipHeaderContext::new(); the host reports InvalidJpipHeader. This violates e747fe080bc71ebe1d1c602a's documented inherited-header support, interpreted using its rfc_map parent's explicitly applicable T.808 (12/2022), A.2.1, rather than the separate inferred observer-history clause. I read the parent provenance/document manifest and retrieved A.2.1 in full through the origin item. Table A.1 permits form 01 with neither Class nor CSn; the text explicitly defaults both identifiers to zero when there is no previous message. The test assumes one reliably ordered stream per context, which matches that section's applicability. No newer-edition conformance is claimed.

Entry realism: the seven-class, 24-iteration sweep runs sequentially with five positive calls and two negative calls per iteration (840 positive and 336 negative calls total), no recursion or accumulated reassembly state, bodies at most three bytes, messages at most 13 bytes, and offset+length at most 65,538. ResourceLimits::default() uses HARDENED.maximum_input_bytes = 2,147,483,648. Large codestream values in the sweep are syntactic indices, not requests to allocate that many codestreams; maximum_codestreams is a count/reassembly constraint, not a header-parser index bound. The actual failing entry is just three bytes, offset and length zero, codestream zero, and no allocation/pool/depth pressure. Real JpipMessageParser::new initializes this same public context and parse delegates directly to parse_jpip_message with it. Existing native tests exercise the same API with small explicit/inherited headers but do not cover the initial dependent-header default.

The replay patch only registers the helper inside the existing test module. The helper uses public constructors and parser calls, not fabricated private state. For 0x21 the continuation and final bits are clear, form=1, and in-class identifier=1; the remaining two single-byte VBAS values encode offset=0 and length=0, with no Aux for class 0 and exactly zero body bytes. Header interpretation does not require the fixture to decode a complete image or payload. MIR confirms that input/form validation and read_vbas succeed, then bb23/bb25 select the omitted-class branch, bb31 applies ok_or_else to previous_class=None, and bb32/bb33/bb35 propagate the error to the return bb94. This occurs before codestream selection and context installation. Thus the missing class-zero default specifically produces the observed rejection. The nearby codestream branch also lacks the initial form-1 default, but that later branch was not reached by this failing call and is not independently reproduced here.

This is a parser/context composition deviation under the documented clause, not an incorrect previous_class() accessor: describe_declaration shows that observer simply returns Option<u64> without callees, and the post-parse accessor assertions are not reached for this input. The output exposes both the failing input/prestate and the exact failed success check, so this is not attribution from exit status alone. The first new() case failed; default() and subsequent initial-default inheritance cases were not executed and remain coverage gaps. No production changes were made.

- **Production evidence:** [`src/jpip.rs:L4-L6`](../../src/jpip.rs#L4-L6)

```text
//! This proof-of-concept profile accepts the standard binary message-header
//! forms with inherited, explicit-class, or explicit-class-and-codestream
//! identifiers.
```

- **Production evidence:** [`src/jpip.rs:L109-L114`](../../src/jpip.rs#L109-L114)

```text
    pub const fn new() -> Self {
        Self {
            previous_class: None,
            previous_codestream: None,
        }
    }
```

- **Production evidence:** [`src/jpip.rs:L262-L268`](../../src/jpip.rs#L262-L268)

```text
    let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
```

- **Production evidence:** [`src/jpip.rs:L230-L232`](../../src/jpip.rs#L230-L232)

```text
    pub fn parse<'a>(&mut self, input: &'a [u8]) -> Result<(JpipMessage<'a>, usize), DecodeError> {
        parse_jpip_message(input, &mut self.context, &self.limits)
    }
```

- **Production evidence:** [`src/jpip.rs:L109-L118`](../../src/jpip.rs#L109-L118)

```text
pub const fn new() -> Self {
        Self {
            previous_class: None,
            previous_codestream: None,
        }
    }

    pub const fn previous_class(&self) -> Option<u64> {
        self.previous_class
    }
```

- **Production evidence:** [`src/jpip.rs:L262-L268`](../../src/jpip.rs#L262-L268)

```text
let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
```

- **Production evidence:** [`src/jpip.rs:L316-L317`](../../src/jpip.rs#L316-L317)

```text
context.previous_class = Some(class_identifier);
    context.previous_codestream = Some(codestream);
```

- **Production evidence:** [`src/refinement.rs:L2326-L2328`](../../src/refinement.rs#L2326-L2328)

```text
pub(crate) const fn jpip_header_has_explicit_class(form: u8) -> bool {
    form >= 2
}
```

- **Harness evidence:** [`inherited_class_registration.patch:L8`](tests/DEV-CA85C0D7B5/files/inherited_class_registration.patch#L8): `+        check_inherited_class_contract();`
- **Harness evidence:** [`src/jpip_inherited_class_contract.rs:L119-L124`](tests/DEV-CA85C0D7B5/files/src/jpip_inherited_class_contract.rs#L119-L124): `    for mut context in [JpipHeaderContext::new(), JpipHeaderContext::default()] {<br>        assert_eq!(context.previous_class(), None);<br>        assert_eq!(context.previous_codestream(), None);<br>        let initial = message(1, 0, 0, 0, &[]);<br>        assert_eq!(initial, [0x21, 0, 0]);<br>        accept(&initial, &mut context, 0, 0, 0, &[]);`
- **Harness evidence:** [`src/jpip_inherited_class_contract.rs:L46-L49`](tests/DEV-CA85C0D7B5/files/src/jpip_inherited_class_contract.rs#L46-L49): `        let result = parse_jpip_message(input, context, &ResourceLimits::default());<br>        let (parsed, consumed) = result.unwrap_or_else(\|error\| {<br>            panic!("valid dependent/independent header must parse: {error:?}")<br>        });`
- **Test source:** [`inherited_class_registration.patch:L8`](tests/DEV-CA85C0D7B5/files/inherited_class_registration.patch#L8)
- **Test source:** [`src/jpip_inherited_class_contract.rs:L119-L124`](tests/DEV-CA85C0D7B5/files/src/jpip_inherited_class_contract.rs#L119-L124)<br>[`src/jpip_inherited_class_contract.rs:L46-L49`](tests/DEV-CA85C0D7B5/files/src/jpip_inherited_class_contract.rs#L46-L49)
- **Exact mapping record:** [report.json](report.json)

## DEV-77C8A9E6C3

- **Mapping:** `319d99d303cc67f39612cec5` / `efb3e214df8de3d48080069d4f59081bd984838e70d3122af80edd3c408da786`
- **Specification:** `482717998449fb68d677ca32`
- **Deviation:** `6fa67f583032c8307d4bf6b1`
- **Test item:** `583f972673fddddde6328f24`
- **Case:** `c9864b4838ab5c38a5152150`
- **Exact standards quote:**

> The message class identifiers defined by this Recommendation | International Standard are the non-negative integers
> shown in Table A.2. The interpretation of the data-bin classes to which they refer is described in A.3. All other values of
> message class identifier are reserved, and the associated messages should be skipped by decoders not recognizing the
> value.

- **Entry:** `src/jpip.rs:33:jpip::DatabinClass::from_identifier`
- **Command:** `target/deepstar-jpip-contract jpip::tests::class_identifiers_follow_t808_table_a2 --exact --nocapture`
- **Setup:** No owning-caller fixture is bypassed: this is a direct unit_contract test of a pure scalar function with an unrestricted u64 entry domain. Identifiers 3/7 satisfy that entry and the callee's precondition-free contract, despite being reserved by the pinned standard; rejection of unsupported identifiers is part of classification. No parsed headers, resource-limited buffers, cache objects, stream-type combinations, or negotiated extension states are manufactured. There are 2231 bounded sequential comparisons, constant call depth, and only two diagnostic strings on this replay. The build helper inserted only test registration and compiled the unchanged production implementation. Applicability assumes no separately negotiated reserved-class extension, as in the originating standards finding.
- **Route:** The host ran target/deepstar-jpip-contract jpip::tests::class_identifiers_follow_t808_table_a2 --exact --nocapture. The build helper included src/jpip_class_contract_tests.rs inside the existing jpip cfg(test) module, with access to the private classifier. Its 0..=1024 loop calls check(3, None) and check(7, None); the closure directly invokes DatabinClass::from_identifier, collects unequal results, and the final failures.is_empty() assertion exposes precisely these two mismatches. The validity predicate accepts both, then the classifier's 2\|3 and 6\|7 arms return the observed semantic classes.
- **Observed assertion:** `semantic classification mismatches:<br>DatabinClass::from_identifier(3): expected None, got Some(TileHeader)<br>DatabinClass::from_identifier(7): expected None, got Some(MainHeader)`

**Full diagnosis:**

The first failed assertion is failures.is_empty() at src/jpip_class_contract_tests.rs:59, with the first mismatch 'DatabinClass::from_identifier(3): expected None, got Some(TileHeader)' and a second mismatch for 7/MainHeader. This violates the standards-derived semantic-classification clause 482717998449fb68d677ca32, interpreted with its rfc_map parent finding, not the separate implementation-inferred interval clause 23bb46fbb1035a00030a243f. I read clause_origin and independently retrieved the pinned T.808 (12/2022), A.2.2, version 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc: Table A.2 defines only 0,1,2,4,5,6,8; all other values are reserved. Odd parity supplies auxiliary-field syntax for parsing even unrecognized messages; it does not define extended tile-header or extended main-header semantics. This is classification, not a claim that the standard mandates this particular Option API or unconditional rejection rather than skipping. Under the originating finding's no-separately-negotiated-extension assumption, None faithfully expresses the absence of a supported standard semantic class. The module advertises standard header forms and classes 0 through 8 but provides no negotiated-extension premise for inventing meanings for 3/7. Its range predicate, native oracle and correlated inferred contracts do not override the cited standard. This conclusion is limited to that edition and assumption, not every future extension.

The entry is realistic and admissible before considering downstream results: the unit accepts any by-value u64 with no requires; there is no buffer, pool, recursion, per-call allocation, indexing, or stack-depth requirement. The entire harness performs 2231 sequential calls (7 defined values, 1018 other values through 1024, 180 power-of-two-neighbor values, 2 maxima, 1024 deterministic samples); it allocates only diagnostic strings for the two observed mismatches. Large integer values are scalar rejection probes, not requested buffer sizes. Identifiers 3/7 are tiny one-byte VBAS values within the representation actually supplied by public DatabinId::new and by parse_jpip_message's explicit-class read_vbas path. The unit test does not claim to replay that parser or fabricate any parser/cache state, and independent pure calls do not mix JPP/JPT stream fixtures.

Compiler declaration and CFG establish the exact failing route: direct private unit call -> bb0 jpip_class_identifier_valid(3 or 7), whose <=8 implementation returns true without violating its contract -> bb1 true -> bb2 identifier switch -> bb8 for 3 or bb6 for 7 -> bb10 returning Some(TileHeader/MainHeader). The replay's build succeeded and registered this test using only a cfg(test) include; no production executable statement was changed. Thus neither a build/fixture/precondition error nor a standards-specific clause misreading explains the failure. The consequence is actual semantic recognition of reserved identifiers; the public constructor stores this erroneous class as DatabinId.class, and the wire parser uses that constructor. Only the direct unit classification failure is claimed as host-reproduced, not an end-to-end wire/cache failure or a formal reachability proof.

- **Production evidence:** [`src/jpip.rs:L4-L6`](../../src/jpip.rs#L4-L6)

```text
//! This proof-of-concept profile accepts the standard binary message-header
//! forms with inherited, explicit-class, or explicit-class-and-codestream
//! identifiers. It reassembles classes 0 through 8 and can reconstruct a
```

- **Production evidence:** [`src/jpip.rs:L33-L45`](../../src/jpip.rs#L33-L45)

```text
    const fn from_identifier(identifier: u64) -> Option<Self> {
        if !crate::refinement::jpip_class_identifier_valid(identifier) {
            return None;
        }
        match identifier {
            0 | 1 => Some(Self::Precinct),
            2 | 3 => Some(Self::TileHeader),
            4 | 5 => Some(Self::TileData),
            6 | 7 => Some(Self::MainHeader),
            8 => Some(Self::Metadata),
            _ => None,
        }
    }
```

- **Production evidence:** [`src/refinement.rs:L2392-L2394`](../../src/refinement.rs#L2392-L2394)

```text
pub(crate) const fn jpip_class_identifier_valid(class_identifier: u64) -> bool {
    class_identifier <= 8
}
```

- **Production evidence:** [`src/jpip.rs:L63-L64`](../../src/jpip.rs#L63-L64)

```text
        let class = DatabinClass::from_identifier(class_identifier)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?;
```

- **Production evidence:** [`src/jpip.rs:L278`](../../src/jpip.rs#L278)

```text
    let databin = DatabinId::new(class_identifier, codestream, in_class)?;
```

- **Production evidence:** [`src/refinement.rs:L2392-L2394`](../../src/refinement.rs#L2392-L2394)

```text
pub(crate) const fn jpip_class_identifier_valid(class_identifier: u64) -> bool {
    class_identifier <= 8
}
```

- **Production evidence:** [`src/jpip.rs:L38-L42`](../../src/jpip.rs#L38-L42)

```text
            0 | 1 => Some(Self::Precinct),
            2 | 3 => Some(Self::TileHeader),
            4 | 5 => Some(Self::TileData),
            6 | 7 => Some(Self::MainHeader),
            8 => Some(Self::Metadata),
```

- **Production evidence:** [`src/jpip.rs:L278`](../../src/jpip.rs#L278)

```text
    let databin = DatabinId::new(class_identifier, codestream, in_class)?;
```

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Harness evidence:** [`src/jpip_class_contract_tests.rs:L16-L24`](tests/DEV-77C8A9E6C3/files/src/jpip_class_contract_tests.rs#L16-L24): `    let mut check = \|identifier, expected\| {<br>        let actual = DatabinClass::from_identifier(identifier);<br>        checks += 1;<br>        if actual != expected {<br>            failures.push(format!(<br>                "DatabinClass::from_identifier({identifier}): expected {expected:?}, got {actual:?}"<br>            ));<br>        }<br>    };`
- **Harness evidence:** [`src/jpip_class_contract_tests.rs:L31-L35`](tests/DEV-77C8A9E6C3/files/src/jpip_class_contract_tests.rs#L31-L35): `    for identifier in 0..=1024 {<br>        if ![0, 1, 2, 4, 5, 6, 8].contains(&identifier) {<br>            check(identifier, None);<br>        }<br>    }`
- **Harness evidence:** [`src/jpip_class_contract_tests.rs:L59-L63`](tests/DEV-77C8A9E6C3/files/src/jpip_class_contract_tests.rs#L59-L63): `    assert!(<br>        failures.is_empty(),<br>        "semantic classification mismatches:\n{}",<br>        failures.join("\n")<br>    );`
- **Harness evidence:** [`deepstar_jpip_contract.py:L9-L14`](tests/DEV-77C8A9E6C3/files/deepstar_jpip_contract.py#L9-L14): `anchor = "#[cfg(test)]\nmod tests {\n    use super::*;\n"<br>registration = '\n    include!("jpip_class_contract_tests.rs");\n'<br>assert original.count(anchor) == 1, "existing JPIP unit-test module not found uniquely"<br>assert registration not in original, "contract test already registered"<br>updated = original.replace(anchor, anchor + registration, 1)<br>assert updated.replace(registration, "", 1) == original`
- **Test source:** [`src/jpip_class_contract_tests.rs:L16-L24`](tests/DEV-77C8A9E6C3/files/src/jpip_class_contract_tests.rs#L16-L24)<br>[`src/jpip_class_contract_tests.rs:L31-L35`](tests/DEV-77C8A9E6C3/files/src/jpip_class_contract_tests.rs#L31-L35)<br>[`src/jpip_class_contract_tests.rs:L59-L63`](tests/DEV-77C8A9E6C3/files/src/jpip_class_contract_tests.rs#L59-L63)
- **Test source:** [`deepstar_jpip_contract.py:L9-L14`](tests/DEV-77C8A9E6C3/files/deepstar_jpip_contract.py#L9-L14)
- **Exact mapping record:** [report.json](report.json)

## DEV-E9BA9C12DC

- **Mapping:** `278a47b0ee3cd36f4c3d6f3c` / `9e6e834ae89961e7c331151f8550c908662d5bb78fb6b4b686d73f2f1be2311d`
- **Specification:** `15c602d57a59b4226e9d6c5e`
- **Deviation:** `7a7dd7e121a983e540bd582d`
- **Test item:** `e85f6e1595e685d9e480e9cd`
- **Case:** `d7f34344b2caaea10ce59061`
- **Exact standards quote:**

> Extended precinct data-bin messages have exactly the same interpretation as non-extended precinct data-bin messages
> and they refer to exactly the same precinct data-bins.

- **Entry:** `src/jpip.rs:137:jpip::JpipMessageHeader::databin`
- **Command:** `cargo test --locked --lib jpip::tests::parses_all_three_standard_header_forms_and_auxiliary -- --exact --nocapture`
- **Setup:** Public JpipMessageParser::new(ResourceLimits::default()) provides a valid fresh context, with neither previous class nor previous codestream. No private mutation or unsafe fixture is used. T.808 (12/2022) A.2.1 expressly permits first-message bb=01 with both omitted fields defaulting to zero. The four-byte message carries ordinary precinct bin 0, byte offset 0, body length 1, no Aux or final flag, and one zero payload byte. It is far below configured capacities, requires no predecessor, and matches the fresh-parser pattern used by ingest_jpip_response. The broader supported-forms clause applies even though the failed parse prevents reaching the getter.
- **Route:** Host runs cargo test --locked --lib jpip::tests::parses_all_three_standard_header_forms_and_auxiliary -- --exact --nocapture. The patched existing test calls databin_contract_sweep, whose final fresh-parser case calls contract_parse_precinct with form 1 and identity (0,0,0). The helper encodes [0x20,0x00,0x01,0x00] and invokes parser.parse. parse_jpip_message rejects the absent initial previous_class; the helper's unwrap_or_else panics before header.databin or fragment construction in this particular call. Earlier accessor/cache calls do not establish this failure; the exact parser diagnostic does.
- **Observed assertion:** `parse -> header.databin -> fragment: form=1 class=0 stream=0 bin=0 offset=0 last=false wire=[20, 00, 01, 00]: MalformedInput { offset: 0, kind: InvalidJpipHeader }`

**Full diagnosis:**

The first host-observed failing check is the helper's expectation that parsing succeeds for form=1 class=0 stream=0 bin=0 offset=0 last=false, wire=[20, 00, 01, 00]. It is not a failed databin identity comparison or a sanitizer/setup failure. The preceding 25-identity sweep completes before this final fresh-parser case. Resource realism was checked first: the failing message is four bytes with a one-byte body and end offset 1; the sweep uses sequential, nonrecursive fixtures, at most eleven bytes per encoded message, and each cache holds three bins/five bytes/seven accepted fragments. Default limits allow 2,147,483,648 input bytes and 1,048,576 JPIP fragments. Large sampled identifiers are scalar identifiers, not requests to allocate that many codestreams or precincts. The actual failing identity is entirely zero. Production ingest_jpip_response likewise creates a fresh JpipMessageParser and immediately parses the first message; the test does not fabricate private state or bypass a required initialization step.

Read clause_origin: this is rfc_map, not infer. Independently read its pinned T.808 (12/2022), SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc, section A.2.1 using the originating mapping item. Table A.1 permits bb=01, omitting Class and CSn. It explicitly states: 'If the Class VBAS is not present and there is no previous message, the message class identifier is 0.' It also states: 'If CSn VBAS is not present and there is no previous message, the codestream index is 0.' Those provisions apply to this first ordinary precinct message: Bin-ID=0, offset=0, length=1, no Aux, body=[0]. The documented supported inherited binary-header form therefore includes this entry. No claim about newer editions is needed. The parent's separate ordinary/extended logical-bin equivalence is not what fails here; those reassembly checks completed. Reclassifying the clause to require a predecessor would contradict this edition's explicit initial defaults, not repair a faulty oracle.

Exact route: existing libtest -> added databin_contract_sweep -> final JpipMessageParser::new(limits) -> contract_parse_precinct(form=1,(0,0,0),(0,false)) -> parser.parse(&wire) -> parse_jpip_message. Compiler declaration and MIR CFG confirm form 1 passes the form check, read_vbas extracts bin 0, then absence of explicit Class takes bb25 -> bb31. previous_class is None from the public constructor; ok_or_else yields InvalidJpipHeader and bb32 -> bb33 -> bb35 returns Err. Source src/jpip.rs:264-267 rejects this admissible first message rather than using zero. The codestream branch at 273-276 also lacks the required form-1 initial default, but the replay fails earlier at Class; that second branch is not claimed as separately reproduced. The consequence is rejection of a standard permitted supported header before creation of JpipMessage::Data, preventing identifier inspection/reassembly on this route. The subject getter's MIR is only a field copy and is not reached in this failing call; this diagnosis concerns the exact clause's supported-header-form guarantee, not an alleged incorrect getter result. The host output and replayed harness expose the specific failing operation sufficiently; no new local execution is being substituted for host replay.

- **Production evidence:** [`src/jpip.rs:L4-L6`](../../src/jpip.rs#L4-L6)

```text
//! This proof-of-concept profile accepts the standard binary message-header
//! forms with inherited, explicit-class, or explicit-class-and-codestream
//! identifiers.
```

- **Production evidence:** [`src/jpip.rs:L262-L268`](../../src/jpip.rs#L262-L268)

```text
    let class_identifier = if crate::refinement::jpip_header_has_explicit_class(form) {
        read_vbas(input, &mut position, None)?
    } else {
        context
            .previous_class
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?
    };
```

- **Production evidence:** [`src/jpip.rs:L219-L224`](../../src/jpip.rs#L219-L224)

```text
    pub const fn new(limits: ResourceLimits) -> Self {
        Self {
            context: JpipHeaderContext::new(),
            limits,
        }
    }
```

- **Production evidence:** [`src/jpip.rs:L3508`](../../src/jpip.rs#L3508)

```text
    let mut parser = JpipMessageParser::new(cache.limits);
```

- **Production evidence:** [`src/jpip.rs:L3519`](../../src/jpip.rs#L3519)

```text
        let (message, consumed) = parser.parse(&input[position..])?;
```

- **Production evidence:** [`src/jpip.rs:L137-L139`](../../src/jpip.rs#L137-L139)

```text
    pub const fn databin(&self) -> DatabinId {
        self.databin
    }
```

- **Production evidence:** [`src/jpip.rs:L38`](../../src/jpip.rs#L38)

```text
            0 | 1 => Some(Self::Precinct),
```

- **Production evidence:** [`src/jpip.rs:L90-L97`](../../src/jpip.rs#L90-L97)

```text
        crate::refinement::jpip_databin_identity(
            self.class_identifier,
            self.codestream,
            self.in_class,
            other.class_identifier,
            other.codestream,
            other.in_class,
        )
```

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Production evidence:** [`src/jpip.rs:L3433-L3435`](../../src/jpip.rs#L3433-L3435)

```text
            if self.databins[index].id.same_as(id) {
                return Some(index);
            }
```

- **Harness evidence:** [`jpip_databin_contract.patch:L280-L281`](tests/DEV-E9BA9C12DC/files/jpip_databin_contract.patch#L280-L281): `+        let mut initial = JpipMessageParser::new(limits);<br>+        contract_parse_precinct(&mut initial, 1, (0, 0, 0), (0, false));`
- **Harness evidence:** [`jpip_databin_contract.patch:L75-L76`](tests/DEV-E9BA9C12DC/files/jpip_databin_contract.patch#L75-L76): `+        let wire = contract_precinct_wire(form, identity, range);<br>+        let (message, consumed) = parser.parse(&wire).unwrap_or_else(\|error\| {`
- **Harness evidence:** [`jpip_databin_contract.patch:L7`](tests/DEV-E9BA9C12DC/files/jpip_databin_contract.patch#L7): `+        databin_contract_sweep();`
- **Test source:** [`jpip_databin_contract.patch:L280-L281`](tests/DEV-E9BA9C12DC/files/jpip_databin_contract.patch#L280-L281)<br>[`jpip_databin_contract.patch:L75-L76`](tests/DEV-E9BA9C12DC/files/jpip_databin_contract.patch#L75-L76)<br>[`jpip_databin_contract.patch:L7`](tests/DEV-E9BA9C12DC/files/jpip_databin_contract.patch#L7)
- **Exact mapping record:** [report.json](report.json)

## DEV-2D35E4323C

- **Mapping:** `00c7d5e1b334b8927c34b64f` / `0a8596243ceb0ae9efdb928be7c430822728aa229e2fabdd6efcf3f1d612bd37`
- **Specification:** `6dd97b323f90d87cebe6dafa`
- **Deviation:** `accd4e55f4de0c3590a4a0bd`
- **Test item:** `baf1d4fa2691d414510aeaae`
- **Case:** `99fed806685c9122fb266211`
- **Exact standards quote:**

> The message class identifiers defined by this Recommendation | International Standard are the non-negative integers
> shown in Table A.2. The interpretation of the data-bin classes to which they refer is described in A.3. All other values of
> message class identifier are reserved, and the associated messages should be skipped by decoders not recognizing the
> value.

- **Entry:** `src/jpip.rs:62:jpip::DatabinId::new`
- **Command:** `target/databin-contract-tests jpip::tests::standard_databin_class_assignment --exact --nocapture`
- **Setup:** A direct public unit-contract entry accepting three by-value u64 arguments and returning Result, with no safety or caller preconditions. No fabricated private state, payload, wire frame, cache, negotiated extension, or decoder setup is involved. Reserved identifiers are admissible inputs to this fallible validation boundary. Only small finite vectors and constant stack depth are required; even the first stream=0, bin=0 cases produce the failure. Large numeric identifiers do not request corresponding resources. Scope is standard T.808 (12/2022) semantic assignment, not mandatory raw-header rejection or 2026 conformance.
- **Route:** The host successfully applies a test-only include patch, compiles the existing library tests with cargo test --lib --no-run --locked, verifies exact libtest registration, and copies that executable. It runs target/databin-contract-tests jpip::tests::standard_databin_class_assignment --exact --nocapture. The test iterates 132 coordinates and calls the real DatabinId::new for raw 3 and 7; successful ordinary classifications are collected by class() and trigger the final forbidden.is_empty() assertion. MIR traces new through from_identifier and the Some/Continue/Ok branch; the zero-coordinate examples demonstrate the target operation itself.
- **Observed assertion:** `264 reserved-class assignments received ordinary standard semantics; first examples: [(3, 0, 0, TileHeader), (7, 0, 0, MainHeader), (3, 0, 1, TileHeader), (7, 0, 1, MainHeader), (3, 0, 15, TileHeader), (7, 0, 15, MainHeader), (3, 0, 16, TileHeader), (7, 0, 16, MainHeader), (3, 0, 127, TileHeader), (7, 0, 127, MainHeader), (3, 0, 128, TileHeader), (7, 0, 128, MainHeader)]`

**Full diagnosis:**

The first failed assertion is forbidden.is_empty() in standard_databin_class_assignment, not a build, setup, resource, or unrelated cache failure. Host replay reports 264 reserved-class assignments, including (3,0,0,TileHeader) and (7,0,0,MainHeader). The test invokes the real public DatabinId::new and reads class(); it does not fabricate a DatabinId or bypass an owning caller. Its 132 coordinates produce 1,188 scalar constructor calls (seven defined and two reserved classes per coordinate), with 264 small failure records, constant call depth, and only small Vec allocations. u64::MAX coordinates are opaque identifiers, not requested buffer sizes or allocation counts. There is no constructor ResourceLimits argument, indexing by these identifiers, or payload allocation. The all-zero coordinate already exposes both deviations, so the finding does not depend on large values, nonzero main-header bin identifiers, or caller capacities.

The constructor's three arguments are unrestricted u64 values; the retained constructor/classifier contracts have no entry requirements, and from_identifier is explicitly total over u64. Reserved class values are legitimate inputs to this fallible class-validation boundary, not a claim that the corresponding messages are standards-conforming. Real parse_jpip_message callers pass read_vbas class/codestream/bin values directly into this same constructor without prior class validation; internal reconstruction callers also use ordinary small identities. The direct unit route is sufficient and does not claim replay of the parser. Compiler declaration and MIR CFG establish new -> from_identifier (bb0), Some -> ok_or_else -> Try::Continue (bb1-bb3), storage of that semantic class in DatabinId and Ok return (bb5 -> bb7). Source maps 3 to TileHeader and 7 to MainHeader after accepting every identifier <=8. class() exposes the stored ordinary class. Thus the exact failed check observes the forbidden semantic assignment.

Authority is the rfc_map parent, not the implementation-derived mapping contract. Independently read the parent-pinned T.808 (12/2022) A.2.2, SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc: Table A.2 defines only 0,1,2,4,5,6,8, and all other class identifiers are reserved. Oddness determines presence of AUX for parsing/skipping; it does not allocate extended tile-header or main-header semantics to 3 or 7. The clause and harness are limited to that edition with no separately supported extension. The module advertises standard message forms; SUPPORTED.md says reserved fields are not supported extensions. The oracle allows rejection or an unknown representation and does not strengthen the conditional recommendation to skip unknown messages into mandatory rejection. Consequently this is neither a harness error nor a clause overreach. The correlated native/Verus classification of 0..=8 cannot override the cited standard. The repository's 2026 catalog reference is not acquired normative text or an edition-difference review; this diagnosis makes no assertion about the unacquired 2026 edition or full JPIP conformance.

- **Production evidence:** [`src/jpip.rs:L39-L41`](../../src/jpip.rs#L39-L41)

```text
            2 | 3 => Some(Self::TileHeader),
            4 | 5 => Some(Self::TileData),
            6 | 7 => Some(Self::MainHeader),
```

- **Production evidence:** [`src/jpip.rs:L63-L70`](../../src/jpip.rs#L63-L70)

```text
        let class = DatabinClass::from_identifier(class_identifier)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?;
        Ok(Self {
            class_identifier,
            class,
            codestream,
            in_class,
        })
```

- **Production evidence:** [`src/jpip.rs:L77-L79`](../../src/jpip.rs#L77-L79)

```text
    pub const fn class(&self) -> DatabinClass {
        self.class
    }
```

- **Production evidence:** [`src/refinement.rs:L2392-L2394`](../../src/refinement.rs#L2392-L2394)

```text
pub(crate) const fn jpip_class_identifier_valid(class_identifier: u64) -> bool {
    class_identifier <= 8
}
```

- **Production evidence:** [`SUPPORTED.md:L164-L165`](../../SUPPORTED.md#L164-L165)

```text
Unsupported capabilities remain typed errors. Reserved fields, contradictory
signaling, and malformed structures are not accepted as supported extensions.
```

- **Production evidence:** [`REFERENCES.md:L39-L40`](../../REFERENCES.md#L39-L40)

```text
The T.808 (06/2026) reference above identifies a catalog record, not an acquired
normative text or completed edition-difference review.
```

- **Production evidence:** [`src/refinement.rs:L2392-L2394`](../../src/refinement.rs#L2392-L2394)

```text
pub(crate) const fn jpip_class_identifier_valid(class_identifier: u64) -> bool {
    class_identifier <= 8
}
```

- **Production evidence:** [`src/jpip.rs:L38-L42`](../../src/jpip.rs#L38-L42)

```text
            0 | 1 => Some(Self::Precinct),
            2 | 3 => Some(Self::TileHeader),
            4 | 5 => Some(Self::TileData),
            6 | 7 => Some(Self::MainHeader),
            8 => Some(Self::Metadata),
```

- **Production evidence:** [`src/jpip.rs:L63-L64`](../../src/jpip.rs#L63-L64)

```text
        let class = DatabinClass::from_identifier(class_identifier)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?;
```

- **Production evidence:** [`src/jpip.rs:L278`](../../src/jpip.rs#L278)

```text
    let databin = DatabinId::new(class_identifier, codestream, in_class)?;
```

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Harness evidence:** [`src/jpip_databin_contract_tests.rs:L1-L3`](tests/DEV-2D35E4323C/files/src/jpip_databin_contract_tests.rs#L1-L3): `// T.808 (12/2022), A.2.2/Table A.2 and A.3.6.1. These tests construct<br>// identities directly: there is no wire framing, AUX interpretation, or<br>// negotiated extension. Each cache represents exactly one logical target.`
- **Harness evidence:** [`src/jpip_databin_contract_tests.rs:L61-L76`](tests/DEV-2D35E4323C/files/src/jpip_databin_contract_tests.rs#L61-L76): `        for raw in [3, 7] {<br>            // Rejection OR an explicit unknown representation is acceptable.<br>            // Do not impose a raw-header rejection/error-code requirement.<br>            if let Ok(identity) = DatabinId::new(raw, stream, bin) {<br>                if matches!(<br>                    identity.class(),<br>                    DatabinClass::Precinct<br>                        \| DatabinClass::TileHeader<br>                        \| DatabinClass::TileData<br>                        \| DatabinClass::MainHeader<br>                        \| DatabinClass::Metadata<br>                ) {<br>                    forbidden.push((raw, stream, bin, identity.class()));<br>                }<br>            }<br>        }`
- **Harness evidence:** [`src/jpip_databin_contract_tests.rs:L78-L83`](tests/DEV-2D35E4323C/files/src/jpip_databin_contract_tests.rs#L78-L83): `    assert!(<br>        forbidden.is_empty(),<br>        "{} reserved-class assignments received ordinary standard semantics; first examples: {:?}",<br>        forbidden.len(),<br>        &forbidden[..forbidden.len().min(12)]<br>    );`
- **Test source:** [`src/jpip_databin_contract_tests.rs:L1-L3`](tests/DEV-2D35E4323C/files/src/jpip_databin_contract_tests.rs#L1-L3)<br>[`src/jpip_databin_contract_tests.rs:L61-L76`](tests/DEV-2D35E4323C/files/src/jpip_databin_contract_tests.rs#L61-L76)<br>[`src/jpip_databin_contract_tests.rs:L78-L83`](tests/DEV-2D35E4323C/files/src/jpip_databin_contract_tests.rs#L78-L83)
- **Test source:** [`validation/databin-contract-tests.patch`](tests/DEV-2D35E4323C/files/validation/databin-contract-tests.patch)
- **Test source:** [`validation/databin-contract-runner.py`](tests/DEV-2D35E4323C/files/validation/databin-contract-runner.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-7A30EEC46F

- **Mapping:** `bb756e73b72f912ced067519` / `63fa5a6f23874c8dbc3e347c5f493af018aa381d7207da7af7acd555ecb89606`
- **Specification:** `cca7e9b1da20c54a63cf8dfc`
- **Deviation:** `bd27b4292b7e47daf8f8b67e`
- **Test item:** `c6736423f68eca94b5682885`
- **Case:** `3e5d68269d3492faf061a2e1`
- **Exact standards quote:**

> All other values of
> message class identifier are reserved

- **Entry:** `src/jpip.rs:77:jpip::DatabinId::class`
- **Command:** `python3 jpip_class_contract_suite.py jpip::tests::standard_assigned_classes_exclude_reserved_signaling`
- **Setup:** Public scalar-only checked-constructor inputs, initially codestream=0 and in_class=0, with no private-state edits, payloads, parser context or downstream plan invariants. Reserved codes are legitimate negative inputs to the checked constructor, whose job is admission. Only a bounded 54,340 sequential probes and small vectors are used; u64 identity extrema do not imply huge buffers. The runner's sole existing-source change is an asserted test-only include insertion and its host build succeeded.
- **Route:** Host build compiles a test-only include into the existing jpip::tests module; the replay runner executes standard_assigned_classes_exclude_reserved_signaling exactly. Its loop calls DatabinId::new(3,0,0), which admits identifier 3, maps it to TileHeader and returns Ok. The oracle returns None, so the mismatch arm records the observed typed class and the final violations.is_empty() assertion fails. Identifier 7 analogously maps to MainHeader. The accessor class() is called on assigned-code success branches, not these reserved-code branches; compiler MIR establishes that it unconditionally returns the same constructor-stored class.
- **Observed assertion:** `DatabinId::new(3, 0, 0) -> Ok(DatabinId { class_identifier: 3, class: TileHeader, codestream: 0, in_class: 0 }); Table A.2 semantic class expected None`

**Full diagnosis:**

The first failed assertion is violations.is_empty() at src/jpip_class_contract_cases.rs:58, not a setup failure or an arbitrary nonzero exit. The first recorded mismatch is DatabinId::new(3, 0, 0) returning Ok with class TileHeader when the independent Table A.2 oracle assigns no class; code 7 similarly returns MainHeader. This violates clause cca7e9b1da20c54a63cf8dfc's assigned-only supported semantic header guarantee.

Entry realism: this case performs 209 scalar identity pairs times 260 class codes = 54,340 sequential constructor probes. It retains only 209 pairs, 260 codes and at most 12 diagnostic strings, with no payload allocation, recursion, codestream parsing, cache or resource-pool use. Large u64 identities are labels, not allocation sizes. The first counterexamples already use codestream=0/in_class=0; these are ordinary small identities comparable to production callers' main-header construction (6, codestream, 0). DatabinId::new is a public checked constructor with scalar u64 arguments and no caller-side assigned-code requirement; supplying a reserved identifier to test its admission policy is admissible, not fabricating an invalid DatabinId. The constructor itself establishes all private fields. The target unit contract has no entry requires, and no owning parser/codestream-plan invariant is bypassed.

Route: the submitted Python runner inserts only an include in the existing cfg(test) module, builds the native library tests and executes this exact libtest. The test calls DatabinId::new for each code. from_identifier's validity helper accepts <=8, then explicitly maps 3 to TileHeader and 7 to MainHeader. Constructor MIR proceeds bb0->bb1->bb2->bb3->bb5->bb7, storing that class and returning Ok. The test's (None, Ok(...)) mismatch arm collects it and the final emptiness assertion fails. On these failing branches the harness does NOT separately call class(); the observed operation is successful admission and the exposed Debug class field. The class accessor's MIR independently shows a single bb0 copying that same stored class, without any further check. Thus this is a reproduced admission/classification deviation, not a claim that a failing accessor call or an end-to-end wire decode was replayed.

Authority: clause_origin is rfc_map, not infer. Reading the parent item's original T.808 (12/2022) A.2.2 at pinned sha256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc confirms Table A.2 assigns only 0,1,2,4,5,6,8 and reserves all other values. Header code 2 is JPP-only and code 6 is JPP/JPT; neither 3 nor 7 is assigned. SUPPORTED.md says reserved signaling is not a supported extension, and the module describes standard binary headers. The odd-identifier Aux rule is parsing syntax for skipping unrecognized messages, not a semantic assignment of every odd neighbor. This test does not impose a wire-level skip-versus-reject policy or exact error variant; DatabinId has no unknown/reserved class representation. Existing implementation/model agreement cannot override that standard and documented policy. The project lists a 2026 Part 9 catalog target and explicitly cautions about publication/edition traceability; this finding is limited to the acquired 2022 edition, as the parent finding assumes, and is not a claim about unavailable 2026 text. No production changes or additional replay are needed to attribute this already host-observed failure.

- **Production evidence:** [`src/jpip.rs:L39-L41`](../../src/jpip.rs#L39-L41)

```text
2 | 3 => Some(Self::TileHeader),
            4 | 5 => Some(Self::TileData),
            6 | 7 => Some(Self::MainHeader),
```

- **Production evidence:** [`src/refinement.rs:L2392-L2394`](../../src/refinement.rs#L2392-L2394)

```text
pub(crate) const fn jpip_class_identifier_valid(class_identifier: u64) -> bool {
    class_identifier <= 8
}
```

- **Production evidence:** [`src/jpip.rs:L63-L70`](../../src/jpip.rs#L63-L70)

```text
let class = DatabinClass::from_identifier(class_identifier)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidJpipHeader))?;
        Ok(Self {
            class_identifier,
            class,
            codestream,
            in_class,
        })
```

- **Production evidence:** [`src/jpip.rs:L77-L79`](../../src/jpip.rs#L77-L79)

```text
pub const fn class(&self) -> DatabinClass {
        self.class
    }
```

- **Production evidence:** [`SUPPORTED.md:L164-L165`](../../SUPPORTED.md#L164-L165)

```text
Reserved fields, contradictory
signaling, and malformed structures are not accepted as supported extensions.
```

- **Production evidence:** [`src/refinement.rs:L2392-L2394`](../../src/refinement.rs#L2392-L2394)

```text
pub(crate) const fn jpip_class_identifier_valid(class_identifier: u64) -> bool {
    class_identifier <= 8
}
```

- **Production evidence:** [`src/jpip.rs:L39-L41`](../../src/jpip.rs#L39-L41)

```text
2 | 3 => Some(Self::TileHeader),
            4 | 5 => Some(Self::TileData),
            6 | 7 => Some(Self::MainHeader),
```

- **Production evidence:** [`src/jpip.rs:L278`](../../src/jpip.rs#L278)

```text
let databin = DatabinId::new(class_identifier, codestream, in_class)?;
```

- **Production evidence:** [`src/jpip.rs:L762-L768`](../../src/jpip.rs#L762-L768)

```text
if !matches!(main_header.class(), DatabinClass::MainHeader)
            || main_header.in_class() != 0
            || !matches!(tile_header.class(), DatabinClass::TileHeader)
            || main_header.codestream() != tile_header.codestream()
        {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidJpipLayout));
        }
```

- **Harness evidence:** [`src/jpip_class_contract_cases.rs:L38-L39`](tests/DEV-7A30EEC46F/files/src/jpip_class_contract_cases.rs#L38-L39): `let result = DatabinId::new(code, codestream, in_class);<br>            match (standard_class_oracle(code), result) {`
- **Harness evidence:** [`src/jpip_class_contract_cases.rs:L5-L9`](tests/DEV-7A30EEC46F/files/src/jpip_class_contract_cases.rs#L5-L9): `2 => Some(DatabinClass::TileHeader),<br>        4 \| 5 => Some(DatabinClass::TileData),<br>        6 => Some(DatabinClass::MainHeader),<br>        8 => Some(DatabinClass::Metadata),<br>        _ => None,`
- **Harness evidence:** [`src/jpip_class_contract_cases.rs:L58-L62`](tests/DEV-7A30EEC46F/files/src/jpip_class_contract_cases.rs#L58-L62): `assert!(<br>        violations.is_empty(),<br>        "reserved signaling exposed as supported, or assigned class rejected:\n{}",<br>        violations.join("\n")<br>    );`
- **Harness evidence:** [`jpip_class_contract_suite.py:L24`](tests/DEV-7A30EEC46F/files/jpip_class_contract_suite.py#L24): `assert edited.replace(registration, "", 1) == original`
- **Harness evidence:** [`jpip_class_contract_suite.py:L59`](tests/DEV-7A30EEC46F/files/jpip_class_contract_suite.py#L59): `os.execv(binary, [binary, "--exact", sys.argv[1], "--nocapture"])`
- **Test source:** [`src/jpip_class_contract_cases.rs:L38-L39`](tests/DEV-7A30EEC46F/files/src/jpip_class_contract_cases.rs#L38-L39)<br>[`src/jpip_class_contract_cases.rs:L5-L9`](tests/DEV-7A30EEC46F/files/src/jpip_class_contract_cases.rs#L5-L9)<br>[`src/jpip_class_contract_cases.rs:L58-L62`](tests/DEV-7A30EEC46F/files/src/jpip_class_contract_cases.rs#L58-L62)
- **Test source:** [`jpip_class_contract_suite.py:L24`](tests/DEV-7A30EEC46F/files/jpip_class_contract_suite.py#L24)<br>[`jpip_class_contract_suite.py:L59`](tests/DEV-7A30EEC46F/files/jpip_class_contract_suite.py#L59)
- **Exact mapping record:** [report.json](report.json)

## DEV-76BF9F9717

- **Mapping:** `caf66b63dc43823a29f23597` / `7d4a5e87c97ead8f692d343dd588c200fc8ac35e130c20c44627fd8a16109989`
- **Specification:** `874546c124e6ad9dd9eab8de`
- **Deviation:** `004b7580baff7f50533ef449`
- **Test item:** `8a33f3c4299ccaa7ac8de80c`
- **Case:** `1b6ab2a4ad1f14f5081e6afd`
- **Exact standards quote:**

> Within each tile-component, precincts are assigned contiguous sequence numbers, s, as follows. All precincts of the
> lowest resolution level (that containing only the LL sub-band samples) are sequenced first, starting from 0, following a
> raster-scan order. The precincts from each successive resolution level are sequenced in turn, again following a raster-scan
> order within their resolution level.

- **Entry:** `src/jpip.rs:1845:jpip::DatabinCache::derive_precinct_map`
- **Command:** `cargo test --locked --lib jpip::tests::derive_precinct_map_matches_standard_geometry_contract -- --exact --nocapture`
- **Setup:** This is a direct public unit-contract entry with an API-reachable cache, not an internal planner fixture. Fixture13's seeded geometry is 56x47, origin/tile-origin (3,2), tiles 24x24, six tiles and three legal sample separations, one layer and D<=2. Completed bins contain SOC/SIZ/main COD/QCD and properly scoped first-tile-part COD/COC/QCD/QCC overrides and SOD. Coding-style precedence and explicit precinct exponents satisfy T.800 A.6.1/B.6; headers and resource requirements are far below default limits. Missing precinct payload bins are intentional for geometric derivation and consistent with native usage and the target's synthetic header-only template. The zero-count execution violating the packet constructor's predicate is created by the owning production planner, not supplied by the test.
- **Route:** The replayed registered libtest calls geometry.fixture(stream, progression, ResourceLimits::default()), whose marker constructors and DatabinCache::insert create and complete all required main/tile header bins. map_contract_observe then invokes cache.derive_precinct_map(stream). After header normalization/template/layout parsing, the first tile's full planner creates a zero-across LH execution at (tile=0,component=0,resolution=2,precinct=0); packet_precinct_from_subbands rejects it and the target propagates InvalidPacketHeader. The harness records that Err as a discrepancy against its independently computed 810 mappings and ultimately fails failures.is_empty().
- **Observed assertion:** `fixture=13 progression=0: expected 810 mappings, got MalformedInput { offset: 0, kind: InvalidPacketHeader }`

**Full diagnosis:**

The first failed check is the accumulated geometry-contract assertion at src/jpip_precinct_map_contract.rs:382, whose first recorded discrepancy is fixture=13 progression=0: expected 810 mappings, got MalformedInput { offset: 0, kind: InvalidPacketHeader }. This is a geometry failure of the actual public derive_precinct_map entry, not a build failure, sanitizer failure, or a fixture that directly injects invalid decoder state.

Realism was checked first. Fixture13 is a 56x47 image at origin (3,2), 24x24 tiles anchored at (3,2), six tiles, three components with sampling (1,1),(2,1),(1,3), one quality layer, and decomposition levels at most two. The independent geometry count is 810. Across the suite, the largest image is only 513x257, there are at most three components, four resolutions, and fewer than 32 tiles per cache. The fixtures execute sequentially, with bounded shallow loops and small completed header bins. ResourceLimits::default is HARDENED: 100,000,000 pixels, 64 components, 16,384 tiles, 33 resolutions, 16,777,216 precincts/code blocks, 67,108,864 packets, and 2,147,483,648 input bytes. These inputs are nowhere near those limits or implicit stack/pool capacities. Existing native JPIP callers likewise insert completed class-6/class-2 header bins and derive a map before receiving precinct bytes. The separately tested packet-limit rejection is expected and did not trigger the failing aggregate.

Compiler declaration and MIR CFG establish the target route: automatic_headers -> build_automatic_template -> parse_layout -> increasing tiles -> automatic_tile_part -> full plan_progression_tile_at_with_component_overrides, propagating its error through the ? branch before returning a map. The harness constructs legal SIZ/COD/COC/QCD/QCC markers, inserts bins with public constructors/insert, checks their completion, and then calls the public subject. It does not construct private grids or decoder executions. The synthetic-template code explicitly rewrites Psot to the header-only length; absent packet data is therefore not a missing fixture precondition.

Tracing fixture13's first tile gives the specific bad operation. Tile 0 component 0 uses its tile COD override: D=2, explicit precinct bytes [0x32,0x23,0x32]. At resolution 2, precinct 0, the component bounds are [3,2,27,26], the precinct's subband-domain rectangle is [0,0,2,4], the LH subband bounds are [2,1,14,13], and effective block size is [2,4]. derive_planned_subband_lattice sets start=(2,1), end=(2,4). Its only emptiness check compares start to the entire subband's upper bound BEFORE clipping to the precinct. That check passes, and it returns Some with blocks=[0,1] instead of omitting this empty subband intersection. plan_subband_execution copies those counts into PacketSubbandExecution. packet_precinct_from_subbands correctly rejects code_blocks_across==0 with InvalidPacketHeader, which propagates to derive_precinct_map and the observed failure. The invalid downstream execution is generated by production planning from admissible headers; the test does not violate that callee's contract.

Authority is the rfc_map origin caf66b63dc43823a29f23597, not agreement with a native reference or Verus model. I read its parent finding, applicability and document manifest, then retrieved T.808 (12/2022) A.3.2.1 and T.800 (06/2019) B.6/A.6.1. T.808 requires contiguous precinct sequence numbering by resolution and Equation A-1 tile/component interleaving. T.800 B.6 expressly permits precincts to have no contributing subband coefficients, and requires their packets when the geometric precinct count is nonzero. A.6.1 confirms tile COC > tile COD > main COC > main COD, matching the harness oracle. Thus this valid ordinary 2D JPP input cannot be dismissed because its first precinct has an empty intersection with one subband. No newer-edition conformance is claimed.

There is also a separately grounded exact-clause violation in the same replay: fixture45 progression=0 stream=62 returns four mappings rather than one. Its one-pixel image occupies [1,2)x[1,2), D=3; resolutions 0,1,2 have equal lower/upper sample bounds. T.800 B-16 explicitly gives zero precincts on equality, whereas derive_component_resolution_geometry_for subtracts floor(lower/precinct_size) from ceil(upper/precinct_size) without the equality case, producing phantom precincts and shifting resolution-prefix identifiers. This independently demonstrates wrong coordinates/IDs on a normally successful admissible call. The diagnostics aggregate failures rather than logging the internal bad execution; the numeric route above is source-assessed, not a claim that the output itself contains an internal trace or a formal reachability proof. The host-observed failing checks and exact replayed public-entry harness are the reproduction evidence. No production changes are proposed or made.

- **Production evidence:** [`src/jpip.rs:L1857-L1865`](../../src/jpip.rs#L1857-L1865)

```text
            let plan = crate::decode::plan_progression_tile_at_with_component_overrides(
                &layout.info.image_size,
                tile,
                tile_part.coding_style,
                &tile_part.component_coding_styles,
                &tile_part.component_quantizations,
                &tile_part.roi_shifts,
                &self.limits,
            )?;
```

- **Production evidence:** [`src/decode.rs:L2023-L2029`](../../src/decode.rs#L2023-L2029)

```text
    if start_x >= subband_x1 || start_y >= subband_y1 {
        return Ok(None);
    }
    let end_x = band[0]
        .checked_add(band[2])
        .ok_or_else(arithmetic_overflow)?
        .min(subband_x1);
```

- **Production evidence:** [`src/decode.rs:L2036-L2038`](../../src/decode.rs#L2036-L2038)

```text
    let blocks_across = ceil_div_u32(end_x, block_size[0])?
        .checked_sub(first_block_x)
        .ok_or_else(arithmetic_overflow)?;
```

- **Production evidence:** [`src/decode.rs:L2095-L2101`](../../src/decode.rs#L2095-L2101)

```text
    let execution = PacketSubbandExecution {
        orientation: subband.orientation,
        code_blocks_across: lattice.blocks[0],
        code_blocks_down: lattice.blocks[1],
        maximum_zero_bitplanes: subband.significant_bitplanes,
        decoders: planned.decoders,
    };
```

- **Production evidence:** [`src/packet.rs:L2221-L2227`](../../src/packet.rs#L2221-L2227)

```text
        if execution.code_blocks_across == 0
            || execution.code_blocks_down == 0
            || execution.maximum_zero_bitplanes == 0
            || execution.maximum_zero_bitplanes > crate::tier1::MAX_TIER1_BITPLANES
        {
            return Err(invalid_packet_header());
        }
```

- **Production evidence:** [`src/decode.rs:L491-L497`](../../src/decode.rs#L491-L497)

```text
    let precinct_x0 = floor_div_u32(resolution_x0, precinct_width)?;
    let precinct_y0 = floor_div_u32(resolution_y0, precinct_height)?;
    let precinct_x1 = ceil_div_u32(resolution_x1, precinct_width)?;
    let precinct_y1 = ceil_div_u32(resolution_y1, precinct_height)?;
    let precincts_across = precinct_x1
        .checked_sub(precinct_x0)
        .ok_or_else(arithmetic_overflow)?;
```

- **Production evidence:** [`src/jpip.rs:L8`](../../src/jpip.rs#L8)

```text
//! SIZ/COD/COC-derived precinct mappings across every progression order.
```

- **Production evidence:** [`src/jpip.rs:L1882-L1890`](../../src/jpip.rs#L1882-L1890)

```text
                let in_class = automatic_precinct_in_class(
                    headers.tile_count,
                    u64::from(headers.components),
                    u64::from(tile),
                    component,
                    resolution,
                    precinct,
                    grid,
                )?;
```

- **Production evidence:** [`src/jpip.rs:L4598-L4603`](../../src/jpip.rs#L4598-L4603)

```text
    while level < resolution {
        level_start = level_start
            .checked_add(u64::from(grid.precinct_count(component, level)?))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        level += 1;
    }
```

- **Production evidence:** [`src/jpip.rs:L4604-L4619`](../../src/jpip.rs#L4604-L4619)

```text
    let precinct_index = level_start
        .checked_add(u64::from(precinct))
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let without_tile = u64::from(component)
        .checked_add(
            precinct_index
                .checked_mul(components)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        )
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let product = without_tile
        .checked_mul(tile_count)
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let checked = tile
        .checked_add(product)
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
```

- **Harness evidence:** [`src/jpip_precinct_map_contract.rs:L161-L165`](tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L161-L165): `        let mut cache = DatabinCache::new(limits);<br>        for (id, bytes) in &bins {<br>            cache.insert(fragment(*id, 0, bytes, true)).unwrap();<br>            assert!(cache.is_complete(*id));<br>        }`
- **Harness evidence:** [`src/jpip_precinct_map_contract.rs:L241`](tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L241): `    let mapping = cache.derive_precinct_map(stream)?;`
- **Harness evidence:** [`src/jpip_precinct_map_contract.rs:L275-L280`](tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L275-L280): `    let mut seed = 0x4a50_4950_u32;<br>    for index in 0..32 {<br>        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);<br>        let width = 25 + seed % 42;<br>        let height = 25 + (seed >> 8) % 42;<br>        let origin = [seed % 4, (seed >> 4) % 4];`
- **Harness evidence:** [`src/jpip_precinct_map_contract.rs:L330-L342`](tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L330-L342): `            let (cache, bins) = geometry.fixture(stream, progression, ResourceLimits::default());<br>            let observed = match map_contract_observe(&cache, stream) {<br>                Ok(observed) => observed,<br>                Err(error) => {<br>                    let failure = format!(<br>                        "fixture={index} progression={progression}: expected {} mappings, got {error:?}",<br>                        expected.len()<br>                    );<br>                    eprintln!("{failure}");<br>                    failures.push(failure);<br>                    continue;<br>                }<br>            };`
- **Harness evidence:** [`src/jpip_precinct_map_contract.rs:L207-L212`](tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L207-L212): `                        // T.800 B-16 explicitly gives zero for empty resolution bounds.<br>                        spans[axis] = if hi == lo {<br>                            0<br>                        } else {<br>                            hi.div_ceil(width) - lo / width<br>                        };`
- **Test source:** [`src/jpip_precinct_map_contract.rs:L161-L165`](tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L161-L165)<br>[`src/jpip_precinct_map_contract.rs:L241`](tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L241)<br>[`src/jpip_precinct_map_contract.rs:L275-L280`](tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L275-L280)<br>[`src/jpip_precinct_map_contract.rs:L330-L342`](tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L330-L342)<br>[`src/jpip_precinct_map_contract.rs:L207-L212`](tests/DEV-76BF9F9717/files/src/jpip_precinct_map_contract.rs#L207-L212)
- **Test source:** [`precinct-map-test-registration.patch`](tests/DEV-76BF9F9717/files/precinct-map-test-registration.patch)
- **Exact mapping record:** [report.json](report.json)

## DEV-E2DF8B9FAC

- **Mapping:** `644e28e929d66e2e5c7a925f` / `ccf657f8eeb2033d55852c2f23812c3d8159ec0d6f9818c248434801aafdf084`
- **Specification:** `4047561ef847a95bf8edd735`
- **Deviation:** `31f0862daac4d7339a6b6163`
- **Test item:** `4bcf68dce311909d630c2985`
- **Case:** `33ce86fa2748896b94d81d14`
- **Exact standards quote:**

> Inclusion of SOD markers is optional.

- **Entry:** `src/jpip.rs:2843:jpip::DatabinCache::automatic_headers`
- **Command:** `target/automatic_headers_contract jpip::tests::automatic_headers_optional_sod_contract --exact --nocapture`
- **Setup:** Safe-Rust unit test of the actual owning automatic_headers function, with test-only private access and no production behavior changes. Stream 37 has a legal SOC/SIZ/COD/QCD/COM main header, 1 tile, 1 component, and a completed class-2 in-class-0 empty tile bin. contract_insert uses normal fragment insertion with final_byte=true, then asserts is_complete and byte equality. Empty raw JPP tile-header bins and omitted SOD are explicitly permitted by pinned T.808 A.3.3. The sweep remains far below resource limits, uses one tile-part/tile and valid non-packed Part-1 fixtures, and requires no precinct payload to derive headers. Paired SOD-present entries pass the same exact canonical-header assertions.
- **Route:** Host runs target/automatic_headers_contract jpip::tests::automatic_headers_optional_sod_contract --exact --nocapture. After the existing automatic mapping regression and independent source-layout checks, the test creates each API-built cache, inserts a completed main header and completed raw tile headers, calls cache.automatic_headers(stream), records any Err, and asserts failures.is_empty(). For the first failing case an explicitly completed empty tile-0 bin reaches automatic_headers -> normalize_automatic_tile_headers -> parse_tile_part_transitions(non-SOT branch) -> found_sod rejection; the error propagates to the recorded failing check.
- **Observed assertion:** `A.3.3 class-2 raw JPP bins: stream=37, tiles=1, components=1, marker_bytes=0, SOD=false: rejected legal bin: MalformedInput { offset: 0, kind: MissingRequiredStructure }`

**Full diagnosis:**

The first host-observed failing check is the final failures.is_empty() assertion in automatic_headers_optional_sod_contract; its first collected failure is stream=37, tiles=1, components=1, marker_bytes=0, SOD=false returning MissingRequiredStructure at offset 0. This is a valid direct unit-contract entry to automatic_headers, not a fixture bypassing the owning normalization caller. Realism check: each case has 1/2/7 tiles, 1/3 components, image dimensions at most 14x3 (42 pixels), one tile-part/tile and no recursive nesting. The largest cache in this failing sweep contains 78 main-header bytes plus seven 9-byte tile bins, 141 bytes total, at most eight bins and 16 insertions. Default limits are HARDENED (16,384 tiles, 64 components, 100,000,000 pixels, 2,147,483,648 input bytes, 1,048,576 fragments), so these are ordinary small inputs, not a capacity/stack stress case. The real derive_precinct_map caller obtains its headers by this same automatic_headers call; its existing regression also builds a cache from completed main and tile bins before mapping. Precinct packet data is not an entry requirement for deriving planning headers. The replay builds the unchanged implementation with only an additive test include, constructs main/class-6 and tile/class-2 bins through insert and JpipFragment::new, explicitly completes even empty bins, and checks completion and exact retrieved bytes. The independent full-codestream fixture passes parse_layout before the target call. The applicable authority is the rfc_map parent's pinned T.808 (12/2022), ISO/IEC 15444-9:2023, Annex A.3.3, independently read through read_rfc_section using origin item 644e28e929d66e2e5c7a925f: tile-header bins exclude SOT, 'Inclusion of SOD markers is optional', and a server must send a tile-header bin even when empty. These fixtures instantiate exactly that raw JPP representation, not the explicitly caller-reframed plan interface. SUPPORTED.md says automatic reconstruction performs its own normalization. Thus the clause is sound; requiring an input SOD would contradict this normative representation allowance, not repair the test. Trace: automatic_headers retrieves completed main and all required tile bins, computes SIZ cardinality, normalizes main, and passes each raw tile vector unchanged to normalize_automatic_tile_headers. The non-SOT parser branch returns one transition containing the original bytes even for empty input. The normalizer sets found_sod=false, consumes zero or all valid marker segments, then rejects on !found_sod before its output SOD synthesis. The target MIR confirms the normalizer call at bb51 and unchanged error propagation through bb53/bb54/bb56. MissingRequiredStructure is therefore caused by an unnecessary framing requirement, not missing/incomplete bins, a fixture assertion, or a parser/build failure. All 18 omitted-SOD cases fail at offsets 0/7/6, while the paired SOD-present calls satisfy the exact output oracle. Retained implementation-descriptive error clauses documenting SOD rejection have no caller requires; they do not override the standards-derived consumer contract or make this entry inadmissible. The aggregate assertion reports the actual Result failures adequately; this diagnosis uses that successful host replay, not the separately noted failed standalone deviation replay. Consequence: legal completed JPP header data is rejected, preventing automatic planning/reconstruction instead of synthesizing canonical SOT/SOD framing.

- **Production evidence:** [`src/jpip.rs:L2878-L2882`](../../src/jpip.rs#L2878-L2882)

```text
            let headers = normalize_automatic_tile_headers(
                &tiles[index],
                u16::try_from(index)
                    .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            )?;
```

- **Production evidence:** [`src/jpip.rs:L3916-L3923`](../../src/jpip.rs#L3916-L3923)

```text
        return Ok(vec![JpipTilePartTransition {
            tile: expected_tile,
            tile_part: 0,
            packet_count,
            data_length: None,
            functional,
            header: copy_bytes(input),
        }]);
```

- **Production evidence:** [`src/jpip.rs:L4341-L4347`](../../src/jpip.rs#L4341-L4347)

```text
        if !found_sod || position != source.len() {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::MissingRequiredStructure,
            ));
        }
        header.extend_from_slice(&[0xff, 0x93]);
```

- **Production evidence:** [`SUPPORTED.md:L142`](../../SUPPORTED.md#L142)

```text
Automatic reconstruction performs its own header normalization.
```

- **Production evidence:** [`src/jpip.rs:L2878-L2882`](../../src/jpip.rs#L2878-L2882)

```text
            let headers = normalize_automatic_tile_headers(
                &tiles[index],
                u16::try_from(index)
                    .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            )?;
```

- **Production evidence:** [`src/jpip.rs:L3922`](../../src/jpip.rs#L3922)

```text
            header: copy_bytes(input),
```

- **Production evidence:** [`src/jpip.rs:L3963`](../../src/jpip.rs#L3963)

```text
        let mut found_sod = false;
```

- **Production evidence:** [`src/jpip.rs:L4341-L4347`](../../src/jpip.rs#L4341-L4347)

```text
        if !found_sod || position != source.len() {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::MissingRequiredStructure,
            ));
        }
        header.extend_from_slice(&[0xff, 0x93]);
```

- **Harness evidence:** [`src/jpip_automatic_headers_contract_tests.rs:L31-L33`](tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L31-L33): `    cache<br>        .insert(fragment(target, split as u64, &bytes[split..], true))<br>        .unwrap();`
- **Harness evidence:** [`src/jpip_automatic_headers_contract_tests.rs:L39-L40`](tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L39-L40): `    assert!(cache.is_complete(target));<br>    assert_eq!(cache.databin_view(target).unwrap().as_bytes(), bytes);`
- **Harness evidence:** [`src/jpip_automatic_headers_contract_tests.rs:L203-L216`](tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L203-L216): `                for with_sod in [true, false] {<br>                    let mut raw = markers.clone();<br>                    if with_sod {<br>                        raw.extend_from_slice(&[0xff, 0x93]);<br>                    }<br>                    let mut cache = DatabinCache::new(ResourceLimits::default());<br>                    contract_insert(&mut cache, DatabinId::new(6, stream, 0).unwrap(), &main);<br>                    for tile in 0..tiles {<br>                        contract_insert(<br>                            &mut cache,<br>                            DatabinId::new(2, stream, u64::from(tile)).unwrap(),<br>                            &raw,<br>                        );<br>                    }`
- **Harness evidence:** [`src/jpip_automatic_headers_contract_tests.rs:L223`](tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L223): `                    match cache.automatic_headers(stream) {`
- **Harness evidence:** [`src/jpip_automatic_headers_contract_tests.rs:L242-L244`](tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L242-L244): `                        Err(error) => {<br>                            failures.push(format!("{context}: rejected legal bin: {error:?}"))<br>                        }`
- **Harness evidence:** [`src/jpip_automatic_headers_contract_tests.rs:L260-L264`](tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L260-L264): `    assert!(<br>        failures.is_empty(),<br>        "automatic header normalization failures:\n{}",<br>        failures.join("\n")<br>    );`
- **Test source:** [`src/jpip_automatic_headers_contract_tests.rs:L31-L33`](tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L31-L33)<br>[`src/jpip_automatic_headers_contract_tests.rs:L39-L40`](tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L39-L40)<br>[`src/jpip_automatic_headers_contract_tests.rs:L203-L216`](tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L203-L216)<br>[`src/jpip_automatic_headers_contract_tests.rs:L223`](tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L223)<br>[`src/jpip_automatic_headers_contract_tests.rs:L242-L244`](tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L242-L244)<br>[`src/jpip_automatic_headers_contract_tests.rs:L260-L264`](tests/DEV-E2DF8B9FAC/files/src/jpip_automatic_headers_contract_tests.rs#L260-L264)
- **Test source:** [`validation/build_automatic_headers_contract.py`](tests/DEV-E2DF8B9FAC/files/validation/build_automatic_headers_contract.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-DFA2D383B6

- **Mapping:** `c3e124e770eb767b4dca0e91` / `44c41bf8ab720f43e806b4d3a2fc72c0a32d6e6491cc433c49b66cd9e6426cb3`
- **Specification:** `ecde50e8b39ea19cc97ae9a9`
- **Deviation:** `66802981d80b00d878039753`
- **Test item:** `4ff071cfd7937f8ec9a893bc`
- **Case:** `819f3c90a11c1699f4f18609`
- **Exact standards quote:**

> Inclusion of SOD markers is optional.

- **Entry:** `src/jpip.rs:1912:jpip::DatabinCache::tile_part_transitions`
- **Command:** `cargo test --locked --lib jpip::tests::contract_exact_parse_and_errors -- --exact --nocapture`
- **Setup:** The tested header is 22 actual bytes: a 12-byte SOT with Lsot=10, tile=7, Psot=12, TPsot=0 and TNsot=1; an 8-byte COM segment with valid length and registration; and 2-byte SOD. There is no earlier parse failure. The malformed Psot is intentionally the format predicate this public parser query must reject, not a cache-entry precondition. contract_cache uses DatabinCache::new(ResourceLimits::default()), inserts one final class-2 fragment at offset zero via JpipFragment::new and DatabinCache::insert, unwraps successful acceptance, and asserts is_complete for the matching ID. All storage/count/platform bounds are comfortably satisfied, and the shared Rust borrow is valid. The fixture never mutates private cache fields or fabricates a transition supplied to the production query.
- **Route:** Host ran cargo test --locked --lib jpip::tests::contract_exact_parse_and_errors -- --exact --nocapture. The additive include registers the test without changing production logic. Its malformed-input loop builds each complete cache with contract_cache(&bytes, 77, 7), invokes cache.tile_part_transitions(77, 7), compares against the exact expected DecodeError, and accumulates the mismatch printed by its failing final assertion. The complete-cache path calls parse_tile_part_transitions; its checked subtraction turns nonzero Psot=12 minus retained header length 22 into None, then the query reports the singleton as Available.
- **Observed assertion:** `bytes=[ff, 90, 00, 0a, 00, 07, 00, 00, 00, 0c, 00, 01, ff, 64, 00, 06, 00, 01, 6f, 6b, ff, 93]: expected Err(MalformedInput { offset: 0, kind: InvalidTilePart }); got Ok(JpipTilePartTransitionReport { status: Available, transitions: [JpipTilePartTransition { tile: 7, tile_part: 0, packet_count: None, data_length: None, functional: false, header: [255, 144, 0, 10, 0, 7, 0, 0, 0, 12, 0, 1, 255, 100, 0, 6, 0, 1, 111, 107, 255, 147] }], needs: [], missing: [] })`

**Full diagnosis:**

The failed check is the final assert!(errors.is_empty(), "{}", errors.join("\n")) in contract_exact_parse_and_errors, which accumulates exact Result mismatches. The beginning of the host output tail is truncated, so I do not identify an unseen first mismatch; the first fully visible mismatch is a single 22-byte SOT/COM/SOD header for tile 7 with Psot=12: expected MalformedInput { offset: 0, kind: InvalidTilePart }, but the query returned Available with data_length=None. This directly contradicts clause ecde50e8b39ea19cc97ae9a9's explicit nonzero-Psot-underflow rejection and zero-Psot/None distinction. Entry realism: each failing fixture stores just 22 or 44 bytes in one complete bin using one public fragment insertion, with one or two headers, no recursion and no large allocation. Default HARDENED limits allow 2,147,483,648 input bytes, 1,048,576 fragments and 16,384 tile parts. Large Psot values elsewhere in this test are scalar declarations, not requests to allocate payloads; the tested operation reads header-only databins. Existing native callers/tests likewise construct complete class-2 header bins using fragment/insert and directly call this public query. No owning caller, main header, decoded image or payload must precede this independent unit-contract entry. Fragment construction checks ranges/resource limits, not codestream syntax; accepting malformed network bytes into the cache is admissible and their rejection belongs to parsing. Route confirmed by source, describe_declaration and Rust MIR CFG: contract_cache creates the default cache, inserts class 2 / stream 77 / tile 7 at offset zero with final=true and asserts completion; the direct query takes the complete branch, snapshots databin_view, and calls parse_tile_part_transitions. SOT length, tile identity and COM framing succeed; SOD establishes H=22. For nonzero L=12, declared.checked_sub(header_length) yields None, and map preserves None without producing DecodeError. The transition is pushed and returned; the query's index>0 condition is false for its singleton, so needs remains empty and status becomes Available. The 44-byte cases additionally demonstrate the same error after a valid first transition, at offset 22. This is not a harness setup failure or an oracle based on an unrelated exit code. clause_origin is infer, not rfc_map. The source documentation expressly says None denotes the legal final zero-Psot form, supporting the intended distinction independently of the failing implementation. The precise InvalidTilePart enum remains the review-marked inferred structural-error assumption (also used for wrong tile/SOT and nonfinal zero Psot); no independently quoted standard mandates that enum. Even under that uncertainty, returning successful None for a nonzero impossible length is the demonstrated deviation, not grounds to revise the clause merely to match the code. No production changes or new replay were needed: observation is the supplied host replay, not a local run.

- **Production evidence:** [`src/jpip.rs:L1148-L1150`](../../src/jpip.rs#L1148-L1150)

```text
    /// Original tile-part data bytes derived from a non-zero `Psot`.
    ///
    /// `None` denotes the legal final-tile-part form whose `Psot` is zero.
```

- **Production evidence:** [`src/jpip.rs:L4030-L4037`](../../src/jpip.rs#L4030-L4037)

```text
        let data_length = if tile_part_length == 0 {
            None
        } else {
            let declared = usize::try_from(tile_part_length).map_err(|_| {
                DecodeError::malformed(part_start as u64, MalformedKind::ArithmeticOverflow)
            })?;
            declared.checked_sub(header_length).map(|data| data as u64)
        };
```

- **Production evidence:** [`src/jpip.rs:L1929-L1930`](../../src/jpip.rs#L1929-L1930)

```text
        let bytes = self.databin_view(id)?.into_bytes();
        let transitions = parse_tile_part_transitions(&bytes, tile)?;
```

- **Production evidence:** [`src/jpip.rs:L1961-L1965`](../../src/jpip.rs#L1961-L1965)

```text
            status: if needs.is_empty() {
                JpipAvailability::Available
            } else {
                JpipAvailability::Partial
            },
```

- **Production evidence:** [`src/jpip.rs:L2300`](../../src/jpip.rs#L2300)

```text
normalized_tiles.push(normalize_automatic_tile_headers(
```

- **Production evidence:** [`src/jpip.rs:L4311-L4312`](../../src/jpip.rs#L4311-L4312)

```text
let mut position = marker_start;
        let mut found_sod = false;
```

- **Production evidence:** [`src/jpip.rs:L4341-L4347`](../../src/jpip.rs#L4341-L4347)

```text
if !found_sod || position != source.len() {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::MissingRequiredStructure,
            ));
        }
        header.extend_from_slice(&[0xff, 0x93]);
```

- **Production evidence:** [`src/jpip.rs:L3878`](../../src/jpip.rs#L3878)

```text
if !input.starts_with(&[0xff, 0x90]) {
```

- **Production evidence:** [`src/jpip.rs:L3922`](../../src/jpip.rs#L3922)

```text
header: copy_bytes(input),
```

- **Harness evidence:** [`src/jpip_transition_contract_tests.rs:L69-L74`](tests/DEV-DFA2D383B6/files/src/jpip_transition_contract_tests.rs#L69-L74): `fn contract_cache(bytes: &[u8], stream: u64, tile: u16) -> DatabinCache {<br>    let mut cache = DatabinCache::new(ResourceLimits::default());<br>    contract_insert(&mut cache, 2, stream, tile, 0, bytes, true);<br>    assert!(cache.is_complete(DatabinId::new(2, stream, u64::from(tile)).unwrap()));<br>    cache<br>}`
- **Harness evidence:** [`src/jpip_transition_contract_tests.rs:L257-L267`](tests/DEV-DFA2D383B6/files/src/jpip_transition_contract_tests.rs#L257-L267): `    for declared in [1, 12, 13, valid.len() as u32 - 1] {<br>        for prefix in [Vec::new(), valid.clone()] {<br>            let (index, total) = if prefix.is_empty() { (0, 1) } else { (1, 2) };<br>            let mut undersized = contract_part(7, index, total, Some(0), false, false).header;<br>            undersized[6..10].copy_from_slice(&declared.to_be_bytes());<br>            let offset = prefix.len() as u64;<br>            let mut bytes = prefix;<br>            bytes.extend(undersized);<br>            failures.push((bytes, offset, MalformedKind::InvalidTilePart));<br>        }<br>    }`
- **Harness evidence:** [`src/jpip_transition_contract_tests.rs:L269-L278`](tests/DEV-DFA2D383B6/files/src/jpip_transition_contract_tests.rs#L269-L278): `    for (bytes, offset, kind) in failures {<br>        let cache = contract_cache(&bytes, 77, 7);<br>        eprintln!("complete malformed bytes={bytes:02x?}; expected {kind:?} at {offset}");<br>        let expected = Err(DecodeError::MalformedInput { offset, kind });<br>        let actual = cache.tile_part_transitions(77, 7);<br>        if actual != expected {<br>            errors.push(format!("bytes={bytes:02x?}: expected {expected:?}; got {actual:?}"));<br>        }<br>    }<br>    assert!(errors.is_empty(), "{}", errors.join("\n"));`
- **Test source:** [`src/jpip_transition_contract_tests.rs:L69-L74`](tests/DEV-DFA2D383B6/files/src/jpip_transition_contract_tests.rs#L69-L74)<br>[`src/jpip_transition_contract_tests.rs:L257-L267`](tests/DEV-DFA2D383B6/files/src/jpip_transition_contract_tests.rs#L257-L267)<br>[`src/jpip_transition_contract_tests.rs:L269-L278`](tests/DEV-DFA2D383B6/files/src/jpip_transition_contract_tests.rs#L269-L278)
- **Test source:** [`validation/build-jpip-transition-contract-tests.py`](tests/DEV-DFA2D383B6/files/validation/build-jpip-transition-contract-tests.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-8115D45404

- **Mapping:** `c3e124e770eb767b4dca0e91` / `44c41bf8ab720f43e806b4d3a2fc72c0a32d6e6491cc433c49b66cd9e6426cb3`
- **Specification:** `3b21d8f452bd5242f422399e`
- **Deviation:** `6bc0aa42c164f206466ffd1d`
- **Test item:** `f3953d5e15029c5e0a4736bc`
- **Case:** `30609dc5558500d02003e8d6`
- **Exact standards quote:**

> Inclusion of SOD markers is optional.

- **Entry:** `src/jpip.rs:2229:jpip::DatabinCache::build_automatic_snapshot`
- **Command:** `cargo test --lib jpip::tests::snapshot_spec_received_header_normalization -- --exact --nocapture`
- **Setup:** All state is constructed with validated public constructors and cache insertion, without private-field mutation. Main header is SOC/SIZ/COD/QCD for one pixel, tile, component and resolution, Rsiz=0, supported progression and positive bounded layers. Received class-2 header is explicitly final and complete, excludes SOT and omits only SOD, which pinned T.808 A.3.3 makes optional; the empty-bin case is expressly permitted. One-byte zero packets are legal empty layer packets. Codestream id and tile/precinct ids are zero, selection is None/all-components, resource limits are default and comfortably exceed the entire setup. No explicit-plan framing contract applies to this automatic entry.
- **Route:** The replayed cargo test --lib jpip::tests::snapshot_spec_received_header_normalization -- --exact --nocapture runs the added direct unit test through a test-module-only include. Its variant loop constructs a complete SOT-free empty header for layers=1/progression=2/comment=false/SOD=false. snapshot_spec_cache inserts the complete main and tile bins and one precinct packet. snapshot_spec_call calls build_automatic_snapshot; completeness and main parsing succeed, then normalize_automatic_tile_headers rejects absent SOD. The Err branch records the exact per-case error and the final failures.is_empty assertion fails.
- **Observed assertion:** `legal complete JPP header rejected: layers=1 progression=2 comment=false SOD=false: malformed input at byte 0: MissingRequiredStructure`

**Full diagnosis:**

The failed check is the final assert!(failures.is_empty(), "{}", failures.join("\n")) in snapshot_spec_received_header_normalization, after snapshot_spec_call returned MissingRequiredStructure for SOD-less headers. This is a target-operation failure, not a setup failure or an inference from exit 101. The replay tail explicitly preserves multiple per-case errors, including the one-layer/progression-2/empty-header witness quoted below; its beginning is truncated, so I do not claim the chronologically first individual rejection is visible. The aggregate assertion and surviving individual messages identify the failed check sufficiently.

Entry realism was checked first: each new variant has a 1x1 image, one tile, one 8-bit component, no decompositions, one precinct, 1/2/7 layers and one zero-byte empty packet per layer. Its three bins contain a 65-byte SOC/SIZ/COD/QCD main header, 0/2/8/10 tile-header bytes and at most 7 packet bytes: at most 82 cached payload bytes in three fragments, and at most 96 reconstructed bytes. The 60 variants run sequentially. ResourceLimits::default is HARDENED, allowing 2,147,483,648 input bytes, 256 layers, 16,384 tiles, 64 components, 67,108,864 packets and 1,048,576 JPIP fragments. There is no excessive recursion, stack fixture, or implicit capacity stress. These are smaller versions of ordinary received bins; the real JpipAutomaticProgressiveDecoder::update_inner forwards the cache, codestream, selection, packet selection and options limits directly to this function without imposing SOD framing.

This clause has rfc_map origin, not infer origin. I read its parent finding and independently retrieved the pinned T.808 (12/2022), ISO/IEC 15444-9:2023, A.3.3 passage via read_rfc_section using origin item c3e124e770eb767b4dca0e91. It says tile-header bins shall not contain SOT, 'Inclusion of SOD markers is optional', and a server must send a tile-header bin even when empty. These fixtures meet that precise JPP applicability: class-2 tile-zero headers exclude SOT; main framing is complete Part 1 Rsiz=0; coding parameters are inherited from the main header; optional binary COM is legal; empty packets are inline. Empty headers are actually received and marked complete, not inferred from absence. The source documentation also explicitly assigns normalization to automatic reconstruction, unlike explicit caller-planned transcodes. The newer catalog edition is not established by this investigation; the finding is grounded in the cited pinned edition and documented API, not an assertion of exhaustive standards conformance.

Route: test-only inclusion inside the existing tests module exposes the private unit without production changes. snapshot_spec_cache uses DatabinId::new, JpipFragment::new and DatabinCache::insert, marking main and tile headers final and asserting is_complete. snapshot_spec_call invokes build_automatic_snapshot(0, None, all_components, default limits). Both header completeness gates pass; siz_cardinality returns one tile/one component, and main normalization accepts the main marker sequence. parse_tile_part_transitions accepts the SOT-free empty or COM-only input as one transition. normalize_automatic_tile_headers synthesizes SOT, starts found_sod=false, scans zero or one complete segments, then rejects solely because no optional SOD was found. The return precedes its own header.extend_from_slice(&[0xff, 0x93]), which could supply output framing. Compiler describe_declaration confirms the entry signature; its control_flow_graph shows the normalizer at bb83, Result branching at bb85/bb86 and error propagation at bb88. Thus the legal received input is rejected before template/packet reconstruction; the positive SOD-present counterparts do not accumulate errors. This violates automatic normalization for legal received headers, rather than a stronger incidental oracle about progression or output bytes. No callee entry requirement demands SOD: the stored normalizer summary describing rejection is an inferred output behavior, explicitly not a caller-prevalidation requirement, and cannot override A.3.3. Direct contract-valid unit testing is permitted here; no owning caller or private state invariant was bypassed.

- **Production evidence:** [`src/jpip.rs:L2300`](../../src/jpip.rs#L2300)

```text
normalized_tiles.push(normalize_automatic_tile_headers(
```

- **Production evidence:** [`src/jpip.rs:L4311-L4312`](../../src/jpip.rs#L4311-L4312)

```text
let mut position = marker_start;
        let mut found_sod = false;
```

- **Production evidence:** [`src/jpip.rs:L4341-L4347`](../../src/jpip.rs#L4341-L4347)

```text
if !found_sod || position != source.len() {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::MissingRequiredStructure,
            ));
        }
        header.extend_from_slice(&[0xff, 0x93]);
```

- **Production evidence:** [`src/jpip.rs:L3916-L3923`](../../src/jpip.rs#L3916-L3923)

```text
return Ok(vec![JpipTilePartTransition {
            tile: expected_tile,
            tile_part: 0,
            packet_count,
            data_length: None,
            functional,
            header: copy_bytes(input),
        }]);
```

- **Production evidence:** [`SUPPORTED.md:L140-L142`](../../SUPPORTED.md#L140-L142)

```text
Those plans require already-reframed headers;
original packet-length declarations must not be copied into a shortened
quality prefix. Automatic reconstruction performs its own header normalization.
```

- **Production evidence:** [`src/jpip.rs:L2300`](../../src/jpip.rs#L2300)

```text
normalized_tiles.push(normalize_automatic_tile_headers(
```

- **Production evidence:** [`src/jpip.rs:L4311-L4312`](../../src/jpip.rs#L4311-L4312)

```text
let mut position = marker_start;
        let mut found_sod = false;
```

- **Production evidence:** [`src/jpip.rs:L4341-L4347`](../../src/jpip.rs#L4341-L4347)

```text
if !found_sod || position != source.len() {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::MissingRequiredStructure,
            ));
        }
        header.extend_from_slice(&[0xff, 0x93]);
```

- **Production evidence:** [`src/jpip.rs:L3878`](../../src/jpip.rs#L3878)

```text
if !input.starts_with(&[0xff, 0x90]) {
```

- **Production evidence:** [`src/jpip.rs:L3922`](../../src/jpip.rs#L3922)

```text
header: copy_bytes(input),
```

- **Harness evidence:** [`src/jpip_snapshot_spec_tests.rs:L12-L13`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L12-L13): `cache.insert(fragment(id(6, 0), 0, main, true)).unwrap();<br>    cache.insert(fragment(id(2, 0), 0, header, true)).unwrap();`
- **Harness evidence:** [`src/jpip_snapshot_spec_tests.rs:L17-L18`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L17-L18): `assert!(cache.is_complete(id(6, 0)));<br>    assert!(cache.is_complete(id(2, 0)));`
- **Harness evidence:** [`src/jpip_snapshot_spec_tests.rs:L23-L25`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L23-L25): `cache.build_automatic_snapshot(<br>        0, None, &JpipPacketSelection::all_components(), &ResourceLimits::default(),<br>    )`
- **Harness evidence:** [`src/jpip_snapshot_spec_tests.rs:L143-L147`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L143-L147): `for include_sod in [true, false] {<br>                    let mut header = markers.clone();<br>                    if include_sod {<br>                        header.extend_from_slice(&[0xff, 0x93]);<br>                    }`
- **Harness evidence:** [`src/jpip_snapshot_spec_tests.rs:L149-L152`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L149-L152): `let cache = snapshot_spec_cache(<br>                        &original[..sot], &header, &vec![0; usize::from(layers)],<br>                    );<br>                    match snapshot_spec_call(&cache) {`
- **Harness evidence:** [`src/jpip_snapshot_spec_tests.rs:L169-L171`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L169-L171): `Err(error) => failures.push(format!(<br>                            "legal complete JPP header rejected: layers={layers} progression={progression} comment={with_comment} SOD={include_sod}: {error}"<br>                        )),`
- **Harness evidence:** [`src/jpip_snapshot_spec_tests.rs:L177`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L177): `assert!(failures.is_empty(), "{}", failures.join("\n"));`
- **Test source:** [`src/jpip_snapshot_spec_tests.rs:L12-L13`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L12-L13)<br>[`src/jpip_snapshot_spec_tests.rs:L17-L18`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L17-L18)<br>[`src/jpip_snapshot_spec_tests.rs:L23-L25`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L23-L25)<br>[`src/jpip_snapshot_spec_tests.rs:L143-L147`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L143-L147)<br>[`src/jpip_snapshot_spec_tests.rs:L149-L152`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L149-L152)<br>[`src/jpip_snapshot_spec_tests.rs:L169-L171`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L169-L171)<br>[`src/jpip_snapshot_spec_tests.rs:L177`](tests/DEV-8115D45404/files/src/jpip_snapshot_spec_tests.rs#L177)
- **Test source:** [`snapshot-tests.patch`](tests/DEV-8115D45404/files/snapshot-tests.patch)
- **Exact mapping record:** [report.json](report.json)

## DEV-76187B67F4

- **Mapping:** `985a0957d228f2a2cf94cfa0` / `31149aaec29317cb2f3b7aea78c7cc5068f932464f3a8357f4cf82803b99a330`
- **Specification:** `153c097eaf82d53210996129`
- **Deviation:** `6d98cc079e1e848463d4205f`
- **Test item:** `944e3d6310a427b8a34ed222`
- **Case:** `75d55d0e82420b19700d15de`
- **Exact standards quote:**

> This data-bin consists of markers and
> marker segments for tile n. It shall not contain an SOT marker segment. Inclusion of SOD markers is optional.

- **Entry:** `src/jpip.rs:4269:jpip::normalize_automatic_tile_headers`
- **Command:** `cargo test --lib --quiet -- jpip::tests::automatic_normalization_accepts_optional_wire_sod_contract --exact --nocapture`
- **Setup:** Admissible direct unit-contract input: tile 0, valid empty immutable Rust slice obtained from a complete, present class-2 bin, no SOT, omitted optional SOD, one transition, no malformed marker segments, and ordinary successful cache allocation/insertion. T.808 (12/2022) A.3.3 expressly permits optional SOD and requires empty tile-header bins to be sent. A valid main header can supply all coding defaults; no whole codestream is an input to this helper. The first failing entry requires no large tile count or resource stress. This assesses a valid unit entry, not full consumer reachability.
- **Route:** Host compiled the unchanged library after a test-only include registration and ran cargo test --lib --quiet -- jpip::tests::automatic_normalization_accepts_optional_wire_sod_contract --exact --nocapture. The test visits tile 0 and the empty retained header, first with SOD, then without it. It inserts the empty final fragment into a real class-2 cache bin, verifies completeness and unchanged view bytes, and directly invokes normalize_automatic_tile_headers(&source, 0). Parsing succeeds with one empty transition; the normalizer rejects found_sod=false at offset 0. The Err arm records the quoted failure and the final failures.is_empty() assertion panics.
- **Observed assertion:** `tile=0 retained=0 SOD=false: valid complete class-2 bin rejected: MalformedInput { offset: 0, kind: MissingRequiredStructure }`

**Full diagnosis:**

The first host-observed failed assertion is failures.is_empty() at src/jpip_normalization_contract_tests.rs:74. Its first collected failure is tile=0, retained=0, SOD=false: a complete empty class-2 bin returns MalformedInput/MissingRequiredStructure at offset 0. This specifically refutes standards-derived clause 153c097eaf82d53210996129, not merely an exit-code expectation.

Realism was assessed before tracing the implementation: the first case performs 352 sequential normalizations (22 tile identifiers x 8 headers x 2 optional-SOD variants), with a fresh cache and one fragment per call, at most 1032 input bytes, one parsed transition, heap-backed vectors, and no recursion or large stack fixture. Default HARDENED limits allow 2,147,483,648 input bytes and 1,048,576 JPIP fragments. Some boundary tile identifiers in the wider sweep exceed the default maximum_tiles=16,384 and are unnecessary to this diagnosis; the reproduced tile=0 empty case has no such issue, needs only a one-tile image, and is the smallest realistic case. Real automatic callers pass the same complete class-2 bin bytes directly to normalization. No pool exhaustion, setup panic, malformed segment, or missing-bin condition explains the first failure.

Authority is the rfc_map origin, not the conflicting inferred rejection clause. I read the original mapping's pinned T.808 (12/2022), A.3.3, SHA-256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc, through read_rfc_section using origin item 985a0957d228f2a2cf94cfa0. It expressly says a JPP tile-header bin shall not contain SOT, inclusion of SOD is optional, and the server must send a tile-header bin even when empty. These rules apply to the documented automatic JPP consumer. The explicit-plan already-reframed-header restriction in SUPPORTED.md is separate; automatic reconstruction performs its own normalization. Thus requiring SOD is not a justified precondition or grounds to revise this standards clause. No assertion is made about an unacquired later edition.

The replay is a direct unit_contract test, not a purported end-to-end reconstruction. It uses existing id/fragment/segment helpers, safe immutable bytes, normal resource limits and a final fragment, then establishes cache.is_complete and exact databin_view bytes before calling normalize_automatic_tile_headers. Mandatory coding defaults may be inherited from a valid main header; this helper receives only tile-header bytes and its parser contract expressly accepts an empty marker-only header. No owning-caller invariant is fabricated or bypassed. A complete automatic-consumer reconstruction is not claimed. Source callers build_automatic_snapshot and automatic_headers independently establish relevance: both pass raw bin bytes without inserting SOD.

describe_declaration and control_flow_graph confirm the failure route. parse_tile_part_transitions takes its non-SOT branch and returns one owned transition with an empty header. Normalization's count conversion succeeds (N=1), source is empty, marker_start and position are 0, and found_sod is false. MIR bb58 initializes that state; bb59 skips scanning; bb87 branches to bb89, and bb90 returns MissingRequiredStructure. The explicit !found_sod check rejects a valid bin before the later synthesized-SOD append/output push. The same defect rejects the seven nonempty, fully framed marker-only fixtures without SOD, producing 176 recorded failures. This is a reproduced code deviation under the pinned standard, not a test error or a standards misreading. The aggregate assertion gives no per-call stack, but the quoted diagnostic, deterministic loop order and compiler/source route identify the first failed call sufficiently; only the host's per-case replay output is used as execution evidence.

- **Production evidence:** [`src/jpip.rs:L3916-L3923`](../../src/jpip.rs#L3916-L3923)

```text
        return Ok(vec![JpipTilePartTransition {
            tile: expected_tile,
            tile_part: 0,
            packet_count,
            data_length: None,
            functional,
            header: copy_bytes(input),
        }]);
```

- **Production evidence:** [`src/jpip.rs:L4341-L4346`](../../src/jpip.rs#L4341-L4346)

```text
        if !found_sod || position != source.len() {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::MissingRequiredStructure,
            ));
        }
```

- **Production evidence:** [`src/jpip.rs:L4347-L4348`](../../src/jpip.rs#L4347-L4348)

```text
        header.extend_from_slice(&[0xff, 0x93]);
        output.push(header);
```

- **Production evidence:** [`src/jpip.rs:L2260-L2262`](../../src/jpip.rs#L2260-L2262)

```text
            let tile_id = DatabinId::new(2, codestream, tile)?;
            if self.is_complete(tile_id) {
                tile_headers.push(self.databin_view(tile_id)?.into_bytes());
```

- **Production evidence:** [`SUPPORTED.md:L141-L142`](../../SUPPORTED.md#L141-L142)

```text
original packet-length declarations must not be copied into a shortened
quality prefix. Automatic reconstruction performs its own header normalization.
```

- **Production evidence:** [`src/jpip.rs:L3916-L3923`](../../src/jpip.rs#L3916-L3923)

```text
        return Ok(vec![JpipTilePartTransition {
            tile: expected_tile,
            tile_part: 0,
            packet_count,
            data_length: None,
            functional,
            header: copy_bytes(input),
        }]);
```

- **Production evidence:** [`src/jpip.rs:L4341-L4346`](../../src/jpip.rs#L4341-L4346)

```text
        if !found_sod || position != source.len() {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::MissingRequiredStructure,
            ));
        }
```

- **Production evidence:** [`src/jpip.rs:L2260-L2262`](../../src/jpip.rs#L2260-L2262)

```text
            let tile_id = DatabinId::new(2, codestream, tile)?;
            if self.is_complete(tile_id) {
                tile_headers.push(self.databin_view(tile_id)?.into_bytes());
```

- **Harness evidence:** [`src/jpip_normalization_contract_tests.rs:L27`](tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs#L27): `    let mut headers = vec![Vec::new()];`
- **Harness evidence:** [`src/jpip_normalization_contract_tests.rs:L39-L51`](tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs#L39-L51): `            for sod in [true, false] {<br>                let mut wire = retained.clone();<br>                if sod {<br>                    wire.extend_from_slice(&[0xff, 0x93]);<br>                }<br>                assert!(!wire.windows(2).any(\|bytes\| bytes == [0xff, 0x90]));<br>                let target = id(2, u64::from(tile));<br>                let mut cache = DatabinCache::new(ResourceLimits::default());<br>                cache.insert(fragment(target, 0, &wire, true)).unwrap();<br>                assert!(cache.is_complete(target), "complete, not absent/incomplete");<br>                let source = cache.databin_view(target).unwrap().into_bytes();<br>                assert_eq!(source, wire);<br>                let result = normalize_automatic_tile_headers(&source, tile);`
- **Harness evidence:** [`src/jpip_normalization_contract_tests.rs:L62-L65`](tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs#L62-L65): `                    Err(error) => failures.push(format!(<br>                        "tile={tile} retained={} SOD={sod}: valid complete class-2 bin rejected: {error:?}",<br>                        retained.len()<br>                    )),`
- **Harness evidence:** [`src/jpip_normalization_contract_tests.rs:L74-L77`](tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs#L74-L77): `    assert!(<br>        failures.is_empty(),<br>        "{} contract failures; first examples:\n{}",<br>        failures.len(),`
- **Test source:** [`src/jpip_normalization_contract_tests.rs:L27`](tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs#L27)<br>[`src/jpip_normalization_contract_tests.rs:L39-L51`](tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs#L39-L51)<br>[`src/jpip_normalization_contract_tests.rs:L62-L65`](tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs#L62-L65)<br>[`src/jpip_normalization_contract_tests.rs:L74-L77`](tests/DEV-76187B67F4/files/src/jpip_normalization_contract_tests.rs#L74-L77)
- **Test source:** [`register_normalization_contract_tests.py`](tests/DEV-76187B67F4/files/register_normalization_contract_tests.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-EFECF7D00D

- **Mapping:** `644e28e929d66e2e5c7a925f` / `1b68e4c44e5afe2a468491ed8af2589b82f6fba780f7bef9b0c3e7e31da38b6c`
- **Specification:** `8f9b87d47d2934d844317263`
- **Deviation:** `0c3f10cc584d07b1e6229cb4`
- **Test item:** `362ea31d9df2c2de98a1835c`
- **Case:** `102f14d562fb2310b01ec046`
- **Exact standards quote:**

> The codestream index shall be ignored in any message
> which has the metadata-bin class identifier.

- **Entry:** `src/jpip.rs:2937:jpip::DatabinCache::availability`
- **Command:** `cargo test --locked --lib availability_contract_ -- --nocapture --test-threads=1`
- **Setup:** An admissible safe-public-API unit entry: a single logical target's root metadata bin 0, two non-overlapping eight-byte fragments of one 16-byte free box, valid explicit class/CSn framing, offsets 0 and 8, final endpoint 16, default resource limits, one checked positive-length 16-byte request. All parsing, construction and insertion return Ok; no private cache mutation, invalid final-length conflict, excessive resource use, or missing owning-caller requirement. Under T.808 (2022) A.3.6.1 the metadata CSn is semantically ignored, so 0 and 1 do not create different metadata bins.
- **Route:** The replay runs cargo test --locked --lib availability_contract_ -- --nocapture --test-threads=1. The metadata subtest uses explicit class-8 binary headers and a fresh cache/context for each second_stream in [0,1], parses two messages, converts their bodies through JpipFragment::from_message, inserts both, and calls availability on class=8/CSn=0/bin=0 range [0,16). Changed second CSn causes identity-sensitive insertion and lookup to exclude the second half, producing Partial/16/8 and gap [8,16); the assertion against Available/16/16/no gaps fails.
- **Observed assertion:** `assertion `left == right` failed: metadata CSn must not partition availability<br>  left: JpipAvailabilityReport { status: Partial, required_bytes: 16, available_bytes: 8, missing: [JpipMissingRange { databin: DatabinId { class_identifier: 8, class: Metadata, codestream: 0, in_class: 0 }, offset: 8, length: Some(8) }] }<br> right: JpipAvailabilityReport { status: Available, required_bytes: 16, available_bytes: 16, missing: [] }`

**Full diagnosis:**

The first failed assertion is reports[1] == expected, labelled "metadata CSn must not partition availability", at replayed src/jpip_availability_contract_tests.rs:323. The same-CSn control passed; the changed-CSn case returned Ok(Partial), required_bytes=16, available_bytes=8, with a spurious missing interval [8,16), despite receiving both halves of the same metadata bin. This is a deviation from the targeted documented supported-class reassembly/availability clause (8f9b87d47d2934d844317263), interpreted using its rfc_map origin, not a reason to revise that clause to match the implementation. I read the parent finding and independently retrieved pinned T.808 (2022), A.3.6.1 (hash 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc): it explicitly says the codestream index shall be ignored in any metadata-bin message. It applies to both JPP and JPT streams and to this single logical target/root metadata bin 0. No image-bin cross-codestream merging is asserted. The separately inferred clause describing codestream-sensitive identity cannot override this metadata-specific normative exception.

Entry realism: each variant uses two 13-byte wire messages (five header bytes plus eight body bytes), stores only 16 payload bytes in two fragments, and queries one positive-length range [0,16). CSn values are only 0 and 1, all arithmetic is representable, there is no recursive structure, and default limits are HARDENED: input 2,147,483,648 bytes, metadata 16,777,216 bytes, fragments 1,048,576, packets 67,108,864, work 1,000,000,000. The availability scan visits one stored interval in the failing case (one merged interval in the control); no capacity is approached. These are ordinary bounded finite-range requests like the native availability consumers, not oversized synthetic inputs. The direct public-unit contract requires no owning reconstruction caller. One fresh cache/context per variant establishes target isolation; root bin 0 carries a complete 16-byte free box, with non-overlapping offsets 0 and 8 and finality announced at endpoint 16. parse_jpip_message, JpipFragment::from_message, insert, and JpipDatabinRange::new all succeed normally. Test-only inclusion exposes the existing module's fixtures and report construction for assertions; it does not fabricate internal cache state or change production behavior.

Source and compiler evidence establish the route: parsing preserves raw CSn in DatabinId; from_message copies that identity; insert calls find_databin_index, whose same_as delegates to jpip_databin_identity. That predicate unconditionally requires equal codestreams, even for class 8, so the second fragment is stored under a second bin. availability's lookup selects only class=8/CSn=0/bin=0. Its MIR lookup path bb30 -> bb33 -> bb34 retrieves that bin, advances cursor to 8 via bb69-bb71, then emits the finite tail gap via bb76 and returns Partial via bb101 -> bb102 -> bb104 -> bb106 -> bb107. Thus the specific failed check observes a false missing range and undercounts available bytes, not a setup panic, resource rejection, arbitrary nonzero exit, or ungrounded entry claim. Host replay built successfully and observed the exact report before the assertion; the bitmap and resource subtests passed. The earlier candidate endpoint inconsistency mentioned in test notes is not used as reproduction evidence; this case's host replay is the evidence.

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Production evidence:** [`src/jpip.rs:L3433-L3435`](../../src/jpip.rs#L3433-L3435)

```text
            if self.databins[index].id.same_as(id) {
                return Some(index);
            }
```

- **Production evidence:** [`src/jpip.rs:L1453`](../../src/jpip.rs#L1453)

```text
        let index = self.find_databin_index(fragment.databin);
```

- **Production evidence:** [`src/jpip.rs:L2987`](../../src/jpip.rs#L2987)

```text
            let Some(databin_index) = self.find_databin_index(requested.databin) else {
```

- **Production evidence:** [`src/jpip.rs:L3041-L3047`](../../src/jpip.rs#L3041-L3047)

```text
            if cursor < requested_end {
                missing.push(JpipMissingRange {
                    databin: requested.databin,
                    offset: cursor,
                    length: Some(requested_end - cursor),
                });
            }
```

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Production evidence:** [`src/jpip.rs:L3433-L3435`](../../src/jpip.rs#L3433-L3435)

```text
            if self.databins[index].id.same_as(id) {
                return Some(index);
            }
```

- **Production evidence:** [`src/jpip.rs:L2987`](../../src/jpip.rs#L2987)

```text
            let Some(databin_index) = self.find_databin_index(requested.databin) else {
```

- **Production evidence:** [`src/jpip.rs:L1453`](../../src/jpip.rs#L1453)

```text
        let index = self.find_databin_index(fragment.databin);
```

- **Production evidence:** [`src/jpip.rs:L2065`](../../src/jpip.rs#L2065)

```text
        let id = DatabinId::new(8, metadata_codestream, bin)?;
```

- **Harness evidence:** [`src/jpip_availability_contract_tests.rs:L276-L280`](tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L276-L280): `    for second_stream in [0_u8, 1] {<br>        let limits = ResourceLimits::default();<br>        let mut cache = DatabinCache::new(limits);<br>        let mut context = JpipHeaderContext::new();<br>        for (offset, stream) in [(0_u8, 0), (8, second_stream)] {`
- **Harness evidence:** [`src/jpip_availability_contract_tests.rs:L283-L285`](tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L283-L285): `            let mut wire = vec![if final_byte { 0x70 } else { 0x60 }, 8, stream, offset, 8];<br>            wire.extend_from_slice(&metadata[offset as usize..offset as usize + 8]);<br>            let (message, used) = parse_jpip_message(&wire, &mut context, &limits).unwrap();`
- **Harness evidence:** [`src/jpip_availability_contract_tests.rs:L296-L298`](tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L296-L298): `            cache<br>                .insert(JpipFragment::from_message(header, body, &limits).unwrap())<br>                .unwrap();`
- **Harness evidence:** [`src/jpip_availability_contract_tests.rs:L308-L310`](tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L308-L310): `        let report = cache<br>            .availability(&[JpipDatabinRange::new(target, 0, 16).unwrap()])<br>            .unwrap();`
- **Harness evidence:** [`src/jpip_availability_contract_tests.rs:L322-L326`](tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L322-L326): `    assert_eq!(reports[0], expected, "same-CSn control");<br>    assert_eq!(<br>        reports[1], expected,<br>        "metadata CSn must not partition availability"<br>    );`
- **Test source:** [`src/jpip_availability_contract_tests.rs:L276-L280`](tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L276-L280)<br>[`src/jpip_availability_contract_tests.rs:L283-L285`](tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L283-L285)<br>[`src/jpip_availability_contract_tests.rs:L296-L298`](tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L296-L298)<br>[`src/jpip_availability_contract_tests.rs:L308-L310`](tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L308-L310)<br>[`src/jpip_availability_contract_tests.rs:L322-L326`](tests/DEV-EFECF7D00D/files/src/jpip_availability_contract_tests.rs#L322-L326)
- **Test source:** [`availability_contract.patch`](tests/DEV-EFECF7D00D/files/availability_contract.patch)
- **Exact mapping record:** [report.json](report.json)

## DEV-F487B6A88C

- **Mapping:** `00c7d5e1b334b8927c34b64f` / `d00089041d7a92072d1e2d91727cb5345a589d5d7be4b44834edfc01accb560b`
- **Specification:** `78ac2bcf8f8510fe8f2c9078`
- **Deviation:** `13063969ba37ce71dca53781`
- **Test item:** `baf1d4fa2691d414510aeaae`
- **Case:** `6430a2e679802b9144ea5a4b`
- **Exact standards quote:**

> The codestream index shall be ignored in any message
> which has the metadata-bin class identifier.

- **Entry:** `src/jpip.rs:62:jpip::DatabinId::new`
- **Command:** `target/databin-contract-tests jpip::tests::metadata_databin_identity_ignores_codestream --exact --nocapture`
- **Setup:** A direct public unit-contract test with no constructor preconditions, supported metadata class 8 and ordinary CSn 0/1, metadata bin 0, one logical target per fresh cache, complete well-framed LBox=12 XML payload, offset 0, final length 12, no auxiliary field, and default resource limits. All state is established through DatabinId::new, JpipFragment::new and DatabinCache::insert, without private-field mutation. The nonzero split cases use consistent complementary fragments and admissible insertion orders. No count or buffer limit is approached.
- **Route:** Host build adds only a test-module include, compiles the Cargo library test executable, and runs jpip::tests::metadata_databin_identity_ignores_codestream exactly. The first coordinates are (stream=0, bin=0). DatabinId::new constructs class-8 identities for CSn 0 and 1 with equal in-class ID. assert_semantic_aliases at split=0 skips the nonfinal empty prefix, builds one final fragment for second at offset 0 containing the complete 12-byte XML box, and inserts it into a fresh cache. status(first) calls find_databin_index -> same_as -> jpip_databin_identity, which returns false solely because 1 != 0. The None status is accumulated as a semantic failure and the final failures.is_empty() assertion fails.
- **Observed assertion:** `DatabinId { class_identifier: 8, class: Metadata, codestream: 0, in_class: 0 } / DatabinId { class_identifier: 8, class: Metadata, codestream: 1, in_class: 0 }, split=0, reverse=false: wrong final length/prefix/bytes/intervals for DatabinId { class_identifier: 8, class: Metadata, codestream: 0, in_class: 0 }: None`

**Full diagnosis:**

The first failing test assertion is the final failures.is_empty() assertion in metadata_databin_identity_ignores_codestream; its first retained counterexample is the split=0, reverse=false case at class 8, bin 0, CSn 0 versus 1. Only the second identity's complete 12-byte XML box is inserted, yet status(first) returns None instead of the completed metadata bin. This violates this exact clause's semantic lookup guarantee, not a requirement to erase raw CSn or make derived Rust equality ignore it. Entry realism: the suite sweeps 132 scalar coordinate pairs, with fresh caches for each of five split points and two orders. Each primary cache accepts at most two payload fragments (12 total bytes) before a one-byte conflict probe; a reference cache contains one 12-byte fragment. Empty-bin checks use final empty fragments. There is no recursive metadata expansion, large bin-index-based allocation, or large payload: u64 boundary identifiers are labels, not counts. Defaults allow 2,147,483,648 input bytes and 1,048,576 fragments. The observed first witness uses only CSn 0/1 and bin 0, so no extreme index or implicit capacity limit is needed. The public direct-unit entry has no requires; class 8 satisfies the classifier contract, fragments use checked public construction, nonfinal empty fragments are skipped, offsets and final lengths are consistent, AUX is None, and each cache represents a single logical target. Production parsing also passes explicit/inherited CSn directly into this constructor, and from_message preserves the resulting identity; the test does not bypass an owning caller invariant. Compiler describe_declaration and CFG show class 8 taking the successful bb0-bb1-bb2-bb3-bb5-bb7 route and retaining the raw CSn. Cache insert/status then call find_databin_index -> same_as -> jpip_databin_identity, whose unconditional codestream equality rejects the metadata alias. At split=0 this makes the completed bin invisible through CSn 0; at split=1 it partitions complementary bytes into two bins, as also observed. The rfc_map parent is authoritative for the stated standards contract: read_rfc_section independently returned T.808 (12/2022), A.3.6.1, pinned SHA 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc: 'The codestream index shall be ignored in any message\nwhich has the metadata-bin class identifier.' Its surrounding text explicitly applies to both JPP and JPT metadata from one logical target and allows arbitrary numeric metadata-bin IDs. Existing model agreement cannot override this exception. This assessment is bounded to that cited edition and one-target assumption; the parent notes that the project's newer selected 2026 edition has not been acquired/compared, so no whole-edition conformance conclusion is claimed. The output tail truncates the panic heading and total failure count, but retains the exact failing test, specific semantic-check diagnostics and FAILED verdict; the attribution rests on those concrete checks, not exit 101 alone.

- **Production evidence:** [`src/jpip.rs:L65-L70`](../../src/jpip.rs#L65-L70)

```text
        Ok(Self {
            class_identifier,
            class,
            codestream,
            in_class,
        })
```

- **Production evidence:** [`src/jpip.rs:L1453`](../../src/jpip.rs#L1453)

```text
        let index = self.find_databin_index(fragment.databin);
```

- **Production evidence:** [`src/jpip.rs:L1705-L1706`](../../src/jpip.rs#L1705-L1706)

```text
    pub fn status(&self, id: DatabinId) -> Option<DatabinStatus> {
        let index = self.find_databin_index(id)?;
```

- **Production evidence:** [`src/jpip.rs:L3433-L3435`](../../src/jpip.rs#L3433-L3435)

```text
            if self.databins[index].id.same_as(id) {
                return Some(index);
            }
```

- **Production evidence:** [`src/jpip.rs:L89-L98`](../../src/jpip.rs#L89-L98)

```text
    const fn same_as(&self, other: Self) -> bool {
        crate::refinement::jpip_databin_identity(
            self.class_identifier,
            self.codestream,
            self.in_class,
            other.class_identifier,
            other.codestream,
            other.in_class,
        )
    }
```

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Production evidence:** [`src/jpip.rs:L278`](../../src/jpip.rs#L278)

```text
    let databin = DatabinId::new(class_identifier, codestream, in_class)?;
```

- **Production evidence:** [`src/jpip.rs:L65-L70`](../../src/jpip.rs#L65-L70)

```text
        Ok(Self {
            class_identifier,
            class,
            codestream,
            in_class,
        })
```

- **Production evidence:** [`src/jpip.rs:L485-L492`](../../src/jpip.rs#L485-L492)

```text
        Self::new(
            header.databin,
            header.offset,
            copy_bytes(body),
            header.last_byte,
            header.auxiliary,
            limits,
        )
```

- **Production evidence:** [`src/jpip.rs:L1453`](../../src/jpip.rs#L1453)

```text
        let index = self.find_databin_index(fragment.databin);
```

- **Production evidence:** [`src/jpip.rs:L3433-L3435`](../../src/jpip.rs#L3433-L3435)

```text
            if self.databins[index].id.same_as(id) {
                return Some(index);
            }
```

- **Production evidence:** [`src/jpip.rs:L89-L98`](../../src/jpip.rs#L89-L98)

```text
    const fn same_as(&self, other: Self) -> bool {
        crate::refinement::jpip_databin_identity(
            self.class_identifier,
            self.codestream,
            self.in_class,
            other.class_identifier,
            other.codestream,
            other.in_class,
        )
    }
```

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Production evidence:** [`src/jpip.rs:L2065-L2066`](../../src/jpip.rs#L2065-L2066)

```text
        let id = DatabinId::new(8, metadata_codestream, bin)?;
        if !self.is_complete(id) {
```

- **Harness evidence:** [`src/jpip_databin_contract_tests.rs:L202-L203`](tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L202-L203): `    let payload = box32(*b"xml ", b"<a/>");<br>    assert_eq!(payload.len(), 12);`
- **Harness evidence:** [`src/jpip_databin_contract_tests.rs:L208-L209`](tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L208-L209): `        let first = DatabinId::new(8, stream, bin).unwrap();<br>        let second = DatabinId::new(8, stream ^ 1, bin).unwrap();`
- **Harness evidence:** [`src/jpip_databin_contract_tests.rs:L116-L127`](tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L116-L127): `            let mut cache = DatabinCache::new(ResourceLimits::default());<br>            let mut fragments = Vec::new();<br>            if split != 0 {<br>                fragments.push(fragment(first, 0, &payload[..split], false));<br>            }<br>            fragments.push(fragment(second, split as u64, &payload[split..], true));<br>            if reverse {<br>                fragments.reverse();<br>            }<br>            for value in fragments {<br>                cache.insert(value).unwrap();<br>            }`
- **Harness evidence:** [`src/jpip_databin_contract_tests.rs:L135-L136`](tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L135-L136): `            for identity in [first, second] {<br>                let status = cache.status(identity);`
- **Harness evidence:** [`src/jpip_databin_contract_tests.rs:L226-L231`](tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L226-L231): `    assert!(<br>        failures.is_empty(),<br>        "{} metadata semantic identity failures; first examples: {:#?}",<br>        failures.len(),<br>        &failures[..failures.len().min(12)]<br>    );`
- **Test source:** [`src/jpip_databin_contract_tests.rs:L202-L203`](tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L202-L203)<br>[`src/jpip_databin_contract_tests.rs:L208-L209`](tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L208-L209)<br>[`src/jpip_databin_contract_tests.rs:L116-L127`](tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L116-L127)<br>[`src/jpip_databin_contract_tests.rs:L135-L136`](tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L135-L136)<br>[`src/jpip_databin_contract_tests.rs:L226-L231`](tests/DEV-F487B6A88C/files/src/jpip_databin_contract_tests.rs#L226-L231)
- **Test source:** [`validation/databin-contract-tests.patch`](tests/DEV-F487B6A88C/files/validation/databin-contract-tests.patch)
- **Test source:** [`validation/databin-contract-runner.py`](tests/DEV-F487B6A88C/files/validation/databin-contract-runner.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-51871F3EEE

- **Mapping:** `278a47b0ee3cd36f4c3d6f3c` / `7432ed20a7d45eb935fb6ed2c48a488ef22fada2ae24609833155ffe020f9924`
- **Specification:** `f2b5e2fde27c8d427b93d091`
- **Deviation:** `1e2f5bdf4506354e38f4b60a`
- **Test item:** `76093ce8dd77c50a3b60f6e6`
- **Case:** `28a49192872db0fa40f803aa`
- **Exact standards quote:**

> The codestream index shall be ignored in any message
> which has the metadata-bin class identifier.

- **Entry:** `src/jpip.rs:1429:jpip::DatabinCache::stored_bytes`
- **Command:** `cargo test --locked --lib jpip::tests::stored_bytes_obeys_metadata_bin_identity_and_sparse_union -- --exact --nocapture`
- **Setup:** One logical target per cache, metadata class 8 and root ID 0, valid 8/9/15/39-byte free boxes, bounded nonempty spans or legal empty final announcements, consistent overlap bytes and final lengths. Caches originate in DatabinCache::new and fragments in public JpipFragment::new via the existing helper. Default byte/fragment limits exceed the tiny sweep by orders of magnitude. CSn changes only an ignored metadata field under pinned T.808 A.3.6.1. The selected accounting failure predates intentionally conflicting probes, so no inadmissible negative-probe state is needed.
- **Route:** Host cargo test --locked --lib jpip::tests::stored_bytes_obeys_metadata_bin_identity_and_sparse_union -- --exact --nocapture ran the additively registered native test. It builds canonical and CSn-relabeled caches, inserts identical sparse byte ranges through public constructors/insert, computes an independent boolean coverage union, and compares stored_bytes after each step. Recorded failed checks cause the final aggregate assertion to fail. At n=39 step=2, metadata bin 0 with CSn=7 is treated as a fresh bin instead of existing metadata coverage; the getter reports 41 for a 31-byte logical union.
- **Observed assertion:** `relabeled n=39 step=2 CSn=7: stored_bytes=41 expected union=31`

**Full diagnosis:**

The standards-derived clause is sound for same-target class-8 metadata. I read clause_origin (rfc_map), then independently retrieved its pinned T.808 (12/2022) A.3.6.1 through the originating map item 278a47b0ee3cd36f4c3d6f3c, anchor bb808185830df51cc635e32dd74dc317202aedeacc9e1a54cd6eacc7a0d5774c. It explicitly applies to both JPP and JPT and says: 'The codestream index shall be ignored in any message which has the metadata-bin class identifier.' Root bin 0 and box metadata match this fixture. This is not a request to normalize the raw header accessor, and agreement between implementation and refinement would not override this receiving-side rule. No claim about a newer standard edition is needed.

Realism: the additional fixtures contain 8, 9, 15, or 39 bytes of a valid free box, with one logical target per cache. Each sparse sweep has 18 spans plus one empty final announcement, followed by six negative probes; there is no recursive assembly or large stack allocation. Even five incorrectly separated CSn bins have at most 195 payload bytes before negative probes. Default limits are 2,147,483,648 input bytes and 1,048,576 fragments. The exact-budget cache deliberately admits n bytes and then a duplicate adding zero logical bytes; this is not excessive input. CSn values through 129 are irrelevant metadata fields, not 130 constructed codestreams. Existing callers ingest parsed messages via JpipFragment::from_message and cache.insert, without a metadata CSn normalization step. The direct cache-unit route is valid independently of wire parsing.

The first failed assertion is the test's final aggregate assert!(failures.is_empty(), ...), not a setup unwrap or sanitizer failure: prior checks collect diagnostics rather than aborting. The retained tail truncates the earliest collected diagnostic, so I do not claim its chronological identity. It nevertheless exposes the exact accounting check: 'relabeled n=39 step=2 CSn=7: stored_bytes=41 expected union=31', and later 'relabeled n=39 step=16 CSn=1: stored_bytes=169 expected union=39'. These checks occur before negative probes, so subsequent deliberate conflicts cannot explain them. An independent exact-budget probe reports a duplicate rejected with actual=78 at limit=39. These specifically refute the clause's requirement not to treat an irrelevant metadata codestream field as a new logical bin.

Route: public new -> public DatabinId::new(8, CSn, 0) -> existing fragment helper using public JpipFragment::new -> insert -> find_databin_index -> DatabinId::same_as -> refinement::jpip_databin_identity. The predicate unconditionally requires equal codestream fields. MIR for find_databin_index confirms false comparisons loop until None; insert then creates a fresh Databin, finds no replaced coverage, adds its span to the global stored_bytes, and commits it. The getter's declaration and MIR show only an immutable field read and return; its arithmetic is not the defect. The defect is the upstream semantic key that makes stored accounting and consistency operate on the wrong logical partition. Consequences include duplicate payload charging, split completeness, bypassed overlap conflicts, and false InputBytes rejections. Constructors and insert are public and all state is created through them; no private-state corruption or skipped owning initializer is involved. No source changes are proposed or made.

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Production evidence:** [`src/jpip.rs:L1453`](../../src/jpip.rs#L1453)

```text
        let index = self.find_databin_index(fragment.databin);
```

- **Production evidence:** [`src/jpip.rs:L3433-L3435`](../../src/jpip.rs#L3433-L3435)

```text
            if self.databins[index].id.same_as(id) {
                return Some(index);
            }
```

- **Production evidence:** [`src/jpip.rs:L1695-L1702`](../../src/jpip.rs#L1695-L1702)

```text
        if let Some(index) = index {
            self.databins[index] = databin;
        } else {
            self.databins.push(databin);
        }
        self.stored_bytes = new_stored_bytes;
        self.accepted_fragments = next_count;
        Ok(())
```

- **Production evidence:** [`src/jpip.rs:L553-L555`](../../src/jpip.rs#L553-L555)

```text
    pub const fn stored_bytes(&self) -> u64 {
        self.stored_bytes
    }
```

- **Production evidence:** [`src/jpip.rs:L137-L139`](../../src/jpip.rs#L137-L139)

```text
    pub const fn databin(&self) -> DatabinId {
        self.databin
    }
```

- **Production evidence:** [`src/jpip.rs:L485-L492`](../../src/jpip.rs#L485-L492)

```text
        Self::new(
            header.databin,
            header.offset,
            copy_bytes(body),
            header.last_byte,
            header.auxiliary,
            limits,
        )
```

- **Production evidence:** [`src/jpip.rs:L1453`](../../src/jpip.rs#L1453)

```text
        let index = self.find_databin_index(fragment.databin);
```

- **Production evidence:** [`src/jpip.rs:L3433-L3435`](../../src/jpip.rs#L3433-L3435)

```text
            if self.databins[index].id.same_as(id) {
                return Some(index);
            }
```

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Production evidence:** [`src/jpip.rs:L2065-L2066`](../../src/jpip.rs#L2065-L2066)

```text
        let id = DatabinId::new(8, metadata_codestream, bin)?;
        if !self.is_complete(id) {
```

- **Production evidence:** [`src/jpip.rs:L553-L555`](../../src/jpip.rs#L553-L555)

```text
    pub const fn stored_bytes(&self) -> u64 {
        self.stored_bytes
    }
```

- **Production evidence:** [`src/jpip.rs:L1599-L1601`](../../src/jpip.rs#L1599-L1601)

```text
        let new_stored_bytes = self.stored_bytes.checked_add(added_bytes).ok_or_else(|| {
            DecodeError::malformed(merged_start, MalformedKind::ArithmeticOverflow)
        })?;
```

- **Harness evidence:** [`src/jpip/stored_bytes_tests.rs:L31-L32`](tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L31-L32): `        let mut canonical = DatabinCache::new(ResourceLimits::default());<br>        let mut relabeled = DatabinCache::new(ResourceLimits::default());`
- **Harness evidence:** [`src/jpip/stored_bytes_tests.rs:L51-L52`](tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L51-L52): `            let csn = [0, 1, 7, 127, 128][step % 5];<br>            let alias = DatabinId::new(8, csn, 0).unwrap();`
- **Harness evidence:** [`src/jpip/stored_bytes_tests.rs:L59-L61`](tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L59-L61): `                let result = cache.insert(fragment(<br>                    target, start as u64, &bytes[start..end], end == n,<br>                ));`
- **Harness evidence:** [`src/jpip/stored_bytes_tests.rs:L64-L65`](tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L64-L65): `            seen[start..end].fill(true);<br>            let expected = seen.iter().filter(\|&&present\| present).count() as u64;`
- **Harness evidence:** [`src/jpip/stored_bytes_tests.rs:L67-L69`](tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L67-L69): `                check!(cache.stored_bytes() == expected,<br>                    "{name} n={n} step={step} CSn={csn}: stored_bytes={} expected union={expected}",<br>                    cache.stored_bytes());`
- **Harness evidence:** [`src/jpip/stored_bytes_tests.rs:L139`](tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L139): `    assert!(failures.is_empty(), "stored_bytes contract violations:\n{}", failures.join("\n"));`
- **Test source:** [`src/jpip/stored_bytes_tests.rs:L31-L32`](tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L31-L32)<br>[`src/jpip/stored_bytes_tests.rs:L51-L52`](tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L51-L52)<br>[`src/jpip/stored_bytes_tests.rs:L59-L61`](tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L59-L61)<br>[`src/jpip/stored_bytes_tests.rs:L64-L65`](tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L64-L65)<br>[`src/jpip/stored_bytes_tests.rs:L67-L69`](tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L67-L69)<br>[`src/jpip/stored_bytes_tests.rs:L139`](tests/DEV-51871F3EEE/files/src/jpip/stored_bytes_tests.rs#L139)
- **Test source:** [`stored-bytes-registration.patch`](tests/DEV-51871F3EEE/files/stored-bytes-registration.patch)
- **Exact mapping record:** [report.json](report.json)

## DEV-AAF82B7313

- **Mapping:** `c3e124e770eb767b4dca0e91` / `2594972eab1fa408a628b2ec7042c853d67263fa514b84a68ef12f1243a5f036`
- **Specification:** `113505b00b0d37d1dadf8a9f`
- **Deviation:** `5981c911d45a1015b8c730aa`
- **Test item:** `e1f075d7b5416d4f472c0f9a`
- **Case:** `b5687497c2e5f6f00a32be75`
- **Exact standards quote:**

> The codestream index shall be ignored in any message
> which has the metadata-bin class identifier.

- **Entry:** `src/jpip.rs:2021:jpip::DatabinCache::expand_metadata_bin`
- **Command:** `python3 validation/run_metadata_contract_suite.py metadata_identity_ignores_message_csn`
- **Setup:** One logical target per freshly initialized default-limit cache; four metadata IDs 0..3; valid JP2 root signature/brand, original-content placeholders and independently validated reconstruction; contents-only non-root bins; complementary out-of-order fragments with consistent final lengths; at most 342 transported bytes, eight insertions and depth 2. No production behavior is modified and no cache internals are forged. Root entry and fresh vectors match assemble_semantic_container exactly. T.808 A.3.6.1 requires ignoring metadata CSn, so selecting CSn 7 and mixing CSn 0/7 are admissible, rather than a requirement to supply absent codestream data.
- **Route:** The replay build adapter inserts only a nested #[cfg(test)]-module include into the native jpip tests and builds the library test executable. The exact metadata_identity_ignores_message_csn test builds validated placeholder fixtures, inserts fragments through the public cache API, then calls expand(&cache, selected). expand invokes expand_metadata_bin(selected,0,true,0,u32::MAX,...) with empty scratch vectors. In iteration 11 mode 0, selecting 7 after inserting all bins under 0 reaches the root lookup's incomplete branch and produces the quoted diagnostic. The test accumulates that mismatch and its final failures.is_empty() assertion fails.
- **Observed assertion:** `iteration=11 mode=0 selected-CSn=7: complete=false, output=0/282, missing=[JpipMissingRange { databin: DatabinId { class_identifier: 8, class: Metadata, codestream: 7, in_class: 0 }, offset: 0, length: None }], needs=[JpipSemanticNeed { kind: MetadataBin, identifier: 0 }]`

**Full diagnosis:**

The first failing assertion is the case's final assert!(failures.is_empty(), "metadata identity/expansion violated:\n{}", ...), not a build, setup, sanitizer, or resource failure. The retained host output specifically reports iteration=11 mode=0 selected-CSn=7 returning complete=false, zero output instead of 282 bytes, and a spurious missing root-bin request. Mode 0 inserted all metadata under CSn 0; selecting 7 must not hide that metadata. The test also exercises fragmented data spanning CSn 0 and 7, but the same-CSn mode-0 witness already suffices.

Realism/admission was checked before tracing: each fresh cache receives four logical metadata bins (at most eight implementation bins when CSn partitions fragments), at most eight fragments, at most 342 total transported payload bytes (160 root + 93 nested contents + 7 colour contents + 82 codestream bytes), and at most depth 2. Expected reconstructed output is at most 282 bytes. These are far below default/HARDENED limits: 2,147,483,648 input bytes, 16,777,216 metadata bytes, 1,048,576 fragments, and depth 32. remaining_depth=u32::MAX is the actual caller's recursion fuel, not requested nesting or allocation. CSn values are identifiers, not allocation counts; the decisive witness uses ordinary value 7, not u64::MAX. All insertions use public checked JpipFragment::new and DatabinCache::insert, with in-bounds complementary offsets, consistent final lengths, no conflicting overlaps, one logical target, and no fabricated private cache fields. The expected JP2 is independently constructed and parse_and_inspect succeeds before expansion. The direct unit entry uses exactly the root state initialized by assemble_semantic_container: bin=0, boxes=true, depth=0, remaining_depth=u32::MAX and fresh vectors. This is a valid direct unit test, not an owning-caller bypass with invented state.

Authority is the rfc_map parent, not the implementation-correlated identity contracts: its pinned T.808 (12/2022; ISO/IEC 15444-9:2023) A.3.6.1 was read through read_rfc_section using origin item c3e124e770eb767b4dca0e91. It explicitly says 'The codestream index shall be ignored in any message which has the metadata-bin class identifier.' This applies to both JPP/JPT metadata from one logical target and root bin 0; the fixture meets that scope. Thus the clause's metadata reassembly/resolution promise is sound under its parent finding, and codestream-sensitive implementation/model agreement cannot justify changing the oracle. This conclusion is scoped to the pinned edition, not unverified 2026 changes.

describe_declaration and the MIR CFG establish the route: root admission and the empty active-stack scan pass; bb19 constructs DatabinId(8, selected CSn, 0), bb23 calls is_complete, and bb25->bb27->bb32 appends missing/need information and returns Ok(false). Source lookup runs status/find_databin_index/same_as/jpip_databin_identity, whose unconditional codestream equality excludes the complete CSn-0 root when selected CSn is 7. insert uses this same lookup, so mixed-CSn fragments likewise fail to coalesce. No reached callee precondition requires a particular selected CSn or forbids this cache state. The consequence is false incompleteness and failure to resolve already available placeholder metadata. The output tail truncates the panic heading and earliest individual mismatch, but retains this precise failing-check diagnostic and failed test result; attribution uses those specifics, not exit code 101 alone.

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Production evidence:** [`src/jpip.rs:L2065-L2073`](../../src/jpip.rs#L2065-L2073)

```text
let id = DatabinId::new(8, metadata_codestream, bin)?;
        if !self.is_complete(id) {
            append_missing_ranges(missing, &self.complete_databin_missing(id));
            needs.push(JpipSemanticNeed {
                kind: JpipSemanticNeedKind::MetadataBin,
                identifier: bin,
            });
            return Ok(false);
        }
```

- **Production evidence:** [`src/jpip.rs:L3433-L3435`](../../src/jpip.rs#L3433-L3435)

```text
if self.databins[index].id.same_as(id) {
                return Some(index);
            }
```

- **Production evidence:** [`src/jpip.rs:L90-L97`](../../src/jpip.rs#L90-L97)

```text
crate::refinement::jpip_databin_identity(
            self.class_identifier,
            self.codestream,
            self.in_class,
            other.class_identifier,
            other.codestream,
            other.in_class,
        )
```

- **Production evidence:** [`src/jpip.rs:L2065`](../../src/jpip.rs#L2065)

```text
let id = DatabinId::new(8, metadata_codestream, bin)?;
```

- **Production evidence:** [`src/jpip.rs:L3433-L3435`](../../src/jpip.rs#L3433-L3435)

```text
if self.databins[index].id.same_as(id) {
                return Some(index);
            }
```

- **Production evidence:** [`src/jpip.rs:L90-L97`](../../src/jpip.rs#L90-L97)

```text
crate::refinement::jpip_databin_identity(
            self.class_identifier,
            self.codestream,
            self.in_class,
            other.class_identifier,
            other.codestream,
            other.in_class,
        )
```

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Harness evidence:** [`src/jpip_metadata_contract_tests.rs:L164-L169`](tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L164-L169): `let (first, last) = match mode {<br>                    0 => (0, 0),<br>                    1 => (7, 7),<br>                    _ => (0, 7),<br>                };<br>                insert_split(&mut cache, *bin, bytes, first, last, split);`
- **Harness evidence:** [`src/jpip_metadata_contract_tests.rs:L173-L175`](tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L173-L175): `for selected in [0, 7, u64::MAX] {<br>                let (complete, output, missing, needs) = expand(&cache, selected);<br>                if !complete \|\| output != f.expected \|\| !missing.is_empty() \|\| !needs.is_empty() {`
- **Harness evidence:** [`src/jpip_metadata_contract_tests.rs:L100-L110`](tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L100-L110): `.expand_metadata_bin(<br>            csn,<br>            0,<br>            true,<br>            0,<br>            u32::MAX,<br>            &mut active,<br>            &mut output,<br>            &mut missing,<br>            &mut needs,<br>        )`
- **Harness evidence:** [`src/jpip_metadata_contract_tests.rs:L184-L188`](tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L184-L188): `assert!(<br>        failures.is_empty(),<br>        "metadata identity/expansion violated:\n{}",<br>        failures.join("\n")<br>    );`
- **Harness evidence:** [`src/jpip_metadata_contract_tests.rs:L84`](tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L84): `crate::jp2::parse_and_inspect(&expected, &ResourceLimits::default()).unwrap();`
- **Test source:** [`src/jpip_metadata_contract_tests.rs:L164-L169`](tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L164-L169)<br>[`src/jpip_metadata_contract_tests.rs:L173-L175`](tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L173-L175)<br>[`src/jpip_metadata_contract_tests.rs:L100-L110`](tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L100-L110)<br>[`src/jpip_metadata_contract_tests.rs:L184-L188`](tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L184-L188)<br>[`src/jpip_metadata_contract_tests.rs:L84`](tests/DEV-AAF82B7313/files/src/jpip_metadata_contract_tests.rs#L84)
- **Test source:** [`validation/run_metadata_contract_suite.py`](tests/DEV-AAF82B7313/files/validation/run_metadata_contract_suite.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-9D6F9C61DE

- **Mapping:** `caf66b63dc43823a29f23597` / `e00a52503c0a25b51c66f7980acef91a1b6fbaf881aa122cf4104416902f9646`
- **Specification:** `0ac89b07817eea830103928b`
- **Deviation:** `639ba0bbcea6d6e5806f70dc`
- **Test item:** `981a9320e9a90ac6c78a0a2c`
- **Case:** `b5221dd13644ef774e61f99c`
- **Exact standards quote:**

> The codestream index shall be ignored in any message
> which has the metadata-bin class identifier.

- **Entry:** `src/jpip.rs:1741:jpip::DatabinCache::databin_view`
- **Command:** `python3 run_databin_view_case.py metadata_identity`
- **Setup:** One fresh public cache represents one logical target. Both IDs are constructed successfully by DatabinId::new, with Metadata class and root in-class ID 0. The payload is a length/type-checked JPEG 2000 family free box. The existing fragment helper calls JpipFragment::new with offset 0, final=true, no auxiliary field, and default limits. insert succeeds and the same-ID view equals the expected payload. Only CSn changes in the failing query, which T.808 12/2022 A.3.6.1 requires ignoring for metadata. No private fields, transport states, allocation-sized numeric labels, recursive nesting, or invalid downstream preconditions are introduced; the decisive cache has one fragment and 25 bytes.
- **Route:** run_databin_view_case.py metadata_identity executes the registered jpip::tests::databin_view_metadata_identity_ignores_codestream test with --exact. Its first iteration constructs class-8 root IDs for CSn 0 and 1, inserts a final 25-byte root fragment under CSn 0 through the checked fragment helper, verifies same-ID retrieval succeeds, then calls databin_view(root_alias). find_databin_index calls same_as and jpip_databin_identity; the unconditional CSn comparison fails and databin_view returns its offset-zero MissingRequiredStructure error. The test records that mismatch and its final failures.is_empty() assertion fails.
- **Observed assertion:** `complete root: stored CSn=0, queried CSn=1, expected=[0, 0, 0, 25, 102, 114, 101, 101, 109, 101, 116, 97, 100, 97, 116, 97, 32, 105, 100, 101, 110, 116, 105, 116, 121], actual=Err(MalformedInput { offset: 0, kind: MissingRequiredStructure })`

**Full diagnosis:**

The first failed assertion is the final failures.is_empty() assertion in databin_view_metadata_identity_ignores_codestream; its first recorded mismatch is a complete root metadata bin stored with CSn=0 returning MissingRequiredStructure at offset 0 when queried with CSn=1, instead of its 25 original bytes. This is a valid direct cache-unit entry, not a fabricated private representation or a skipped caller prerequisite. The fixture uses DatabinId::new(8, ..., 0), DatabinCache::new, and checked JpipFragment::new/insert; successful same-ID retrieval establishes receipt of the complete payload before the alias query. The free box has a checked length and type. Each metadata cache holds at most 25 payload bytes, at most three accepted fragments and three implementation bins, with no recursive metadata expansion. Across all five CSn pairs and four non-metadata controls the identity test inserts 103 fragments into separate short-lived caches, not one accumulating cache. Default limits allow 2,147,483,648 input bytes, 16,777,216 metadata bytes and 1,048,576 fragments. The decisive CSn pair 0/1 is small even relative to maximum_codestreams=64. Later large numeric labels size no allocation and are not necessary to establish this deviation.

The clause has rfc_map origin, not infer origin. I read its parent finding, pinned document manifest and T.808 (12/2022) A.3.6.1 through read_rfc_section using originating item caf66b63dc43823a29f23597. That section expressly says 'The codestream index shall be ignored in any message which has the metadata-bin class identifier.' It applies to metadata from one logical target, requires root bin ID 0, and expressly permits an empty root. The fixture uses one target per cache. Thus the alias oracle implements the cited metadata exception; no cross-target merging is claimed. The exact-codestream matching in the stored lookup/status contracts and correlated implementation model describes the same implementation behavior and does not override this normative exception. This conclusion is limited to acquired T.808 12/2022 (SHA-256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc), not the unacquired 06/2026 edition or general JPIP conformance.

The replay runner selects and executes the registered native test exactly; host build and source-overlay checks succeeded. The assessed route is checked fragment construction -> insert -> databin_view(root_alias) -> find_databin_index -> DatabinId::same_as -> jpip_databin_identity. The helper unconditionally compares codestreams, so the sole stored class-8/bin-0 entry does not match CSn=1. databin_view's MIR bb0 -> bb1 -> bb2 -> bb3 -> bb6 -> bb27 propagates the initial missing-index error, before any completeness or byte-copy branch. This rejects an already complete semantic metadata bin. The separately reported mixed-CSn fragments likewise remain split into incomplete implementation bins. Real ingestion does not supply a missing canonicalization premise: parse_jpip_message retains explicit/inherited CSn in DatabinId, JpipFragment::from_message preserves header.databin, and ingest_jpip_response directly inserts that fragment; metadata consumers construct class-8 IDs and use these same lookup paths. The replay is a direct public-unit-contract reproduction, not a claimed end-to-end wire/session reproduction. Host output is truncated to eight mismatch details by the harness, but the first complete-root mismatch includes the operation, both indices, expected bytes and actual error and is sufficient independently of the exit code.

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Production evidence:** [`src/jpip.rs:L90-L97`](../../src/jpip.rs#L90-L97)

```text
        crate::refinement::jpip_databin_identity(
            self.class_identifier,
            self.codestream,
            self.in_class,
            other.class_identifier,
            other.codestream,
            other.in_class,
        )
```

- **Production evidence:** [`src/jpip.rs:L3433-L3435`](../../src/jpip.rs#L3433-L3435)

```text
            if self.databins[index].id.same_as(id) {
                return Some(index);
            }
```

- **Production evidence:** [`src/jpip.rs:L1742-L1744`](../../src/jpip.rs#L1742-L1744)

```text
        let index = self
            .find_databin_index(id)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))?;
```

- **Production evidence:** [`src/jpip.rs:L485-L492`](../../src/jpip.rs#L485-L492)

```text
        Self::new(
            header.databin,
            header.offset,
            copy_bytes(body),
            header.last_byte,
            header.auxiliary,
            limits,
        )
```

- **Production evidence:** [`src/jpip.rs:L278`](../../src/jpip.rs#L278)

```text
    let databin = DatabinId::new(class_identifier, codestream, in_class)?;
```

- **Production evidence:** [`src/jpip.rs:L485-L492`](../../src/jpip.rs#L485-L492)

```text
        Self::new(
            header.databin,
            header.offset,
            copy_bytes(body),
            header.last_byte,
            header.auxiliary,
            limits,
        )
```

- **Production evidence:** [`src/jpip.rs:L3433-L3435`](../../src/jpip.rs#L3433-L3435)

```text
            if self.databins[index].id.same_as(id) {
                return Some(index);
            }
```

- **Production evidence:** [`src/jpip.rs:L90-L97`](../../src/jpip.rs#L90-L97)

```text
        crate::refinement::jpip_databin_identity(
            self.class_identifier,
            self.codestream,
            self.in_class,
            other.class_identifier,
            other.codestream,
            other.in_class,
        )
```

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Harness evidence:** [`databin_view_contract_tests.patch:L137-L138`](tests/DEV-9D6F9C61DE/files/databin_view_contract_tests.patch#L137-L138): `+            let root = DatabinId::new(8, stored_stream, 0).unwrap();<br>+            let root_alias = DatabinId::new(8, other_stream, 0).unwrap();`
- **Harness evidence:** [`databin_view_contract_tests.patch:L151-L154`](tests/DEV-9D6F9C61DE/files/databin_view_contract_tests.patch#L151-L154): `+            cache.insert(fragment(root, 0, &expected, true)).unwrap();<br>+            assert_eq!(cache.databin_view(root).unwrap().as_bytes(), expected);<br>+            let actual = cache.databin_view(root_alias).map(JpipByteView::into_bytes);<br>+            if actual != Ok(expected.clone()) {`
- **Harness evidence:** [`databin_view_contract_tests.patch:L238-L240`](tests/DEV-9D6F9C61DE/files/databin_view_contract_tests.patch#L238-L240): `+        assert!(<br>+            failures.is_empty(),<br>+            "insert -> databin_view violates metadata CSn independence:\n{}"`
- **Harness evidence:** [`run_databin_view_case.py:L33`](tests/DEV-9D6F9C61DE/files/run_databin_view_case.py#L33): `    result = subprocess.run([binary, name, "--exact", "--nocapture", "--test-threads=1"])`
- **Test source:** [`databin_view_contract_tests.patch:L137-L138`](tests/DEV-9D6F9C61DE/files/databin_view_contract_tests.patch#L137-L138)<br>[`databin_view_contract_tests.patch:L151-L154`](tests/DEV-9D6F9C61DE/files/databin_view_contract_tests.patch#L151-L154)<br>[`databin_view_contract_tests.patch:L238-L240`](tests/DEV-9D6F9C61DE/files/databin_view_contract_tests.patch#L238-L240)
- **Test source:** [`run_databin_view_case.py:L33`](tests/DEV-9D6F9C61DE/files/run_databin_view_case.py#L33)
- **Exact mapping record:** [report.json](report.json)

## DEV-A64A9EFAD1

- **Mapping:** `4bbbf5c259181f66f15ae648` / `109f09928a69f3e8a66188f70dea1456a4459175126dd1dc32104cd5a27e5af2`
- **Specification:** `14ba92bb9ab4b9fc4b9ed06e`
- **Deviation:** `d68b6e9d7a81b7b727f130cf`
- **Test item:** `797998cd69a411657edec5ec`
- **Case:** `bd44625aaae9e533298cf3ee`
- **Exact standards quote:**

> The codestream index shall be ignored in any message
> which has the metadata-bin class identifier.

- **Entry:** `src/jpip.rs:1200:jpip::JpipContainerAssembly::semantic_needs`
- **Command:** `target/semantic-needs-tests --exact jpip::tests::semantic_metadata_needs_ignore_metadata_codestream_index --nocapture`
- **Setup:** One cache contains exactly one logical JP2 target; there is no cross-target mixing. Root has signature, JP2 ftyp, one ordinary 28-byte flags=1 phld replacing jp2h, and literal jp2c bytes. Child contains the original jp2h contents with ihdr and colr. Placeholder OrigID is the matching eight-byte child ID and original length/type are preserved. Both fragments use offset zero and final=true via JpipFragment::new and successful cache.insert. Root=150 bytes, child=37 bytes, output=167 bytes; depth one and two fragments satisfy default limits. CSn values 0 and 1 are valid constructor inputs and must be ignored for class-8 identity under the cited edition. u64::MAX is an ID, not an allocation request. The test obtains the assembly from its real public producer; no private state or production behavior is changed.
- **Route:** The host builds the additive semantic-needs.patch with cargo test --locked --lib --no-run and invokes target/semantic-needs-tests --exact jpip::tests::semantic_metadata_needs_ignore_metadata_codestream_index --nocapture. The witness iteration creates a fresh cache, inserts finalized root bin 0 with CSn 0 and finalized child u64::MAX with CSn 1, calls assemble_semantic_container(0), then reads semantic_needs(). The child expansion looks up CSn 0, rejects the stored CSn 1 child, records a spurious need, and returns Partial/None. The actual tuple differs from the expected Available/empty-needs/exact-container tuple, is appended to violations, and fails the final violations.is_empty() assertion.
- **Observed assertion:** `bin=18446744073709551615 root_csn=0 child_csn=1 query_csn=0 complete=true: actual=(Partial, [(MetadataBin, 18446744073709551615)], None) expected=(Available, [],`

**Full diagnosis:**

The first failing assertion is the test's final assert!(violations.is_empty()), which collects mismatches between reported dependencies/status/container and the standard-derived expectation; it is not a constructor failure, timeout, or arbitrary exit. The retained host output specifically shows complete root/child metadata yielding Partial with a MetadataBin need instead of Available with no needs. This violates clause 14ba92bb9ab4b9fc4b9ed06e in its rfc_map context: reported outstanding dependencies must describe genuinely incomplete target metadata, not an artificial CSn partition. I read the parent finding and the pinned T.808 (12/2022) A.3.6.1 through read_rfc_section using origin item 4bbbf5c259181f66f15ae648. It explicitly requires ignoring CSn on metadata messages, allows arbitrary numeric metadata-bin IDs, and assigns root ID 0. No claim about the unacquired 2026 edition is needed.

Entry realism was checked before tracing: 25 independent fresh caches, at most two final offset-zero fragments per cache, 150 root bytes plus 37 child bytes, 167 reconstructed bytes, one placeholder descent, and a 1x1 single-component codestream. These are far below default HARDENED limits (2 GiB input, 16 MiB metadata, depth 32, 1,048,576 fragments). The large bin IDs occupy eight bytes and are not counts, indices into allocated arrays, or requested payload lengths. The standard permits arbitrary numeric IDs. The existing semantic_metadata_assembly_places_nested_boxes_and_codestreams caller uses the same safe fragment/insert/assembly/report route and comparable fixtures; it restricts CSn to zero, which masks rather than legitimizes the identity error. This direct unit-contract test needs no production reachability proof.

For the retained complete-child witness, insert stores root (class=8, CSn=0, ID=0) and child (class=8, CSn=1, ID=u64::MAX) for one logical target. assemble_semantic_container(0) finds the root, parses the ordinary flags=1 jp2h placeholder, and expands the child using metadata_codestream=0. is_complete -> status -> find_databin_index -> DatabinId::same_as -> jpip_databin_identity rejects the complete child solely because 0 != 1. expand_metadata_bin consequently pushes MetadataBin(u64::MAX), returns false, and assembly returns Partial/None. semantic_needs faithfully exposes that erroneous producer report. describe_declaration and the getter CFG confirm the accessor only borrows self.needs and dereferences the Vec; the deviation is in supporting identity/assembly logic, not slice mechanics. Root-CSn mismatch rows independently show a spurious MetadataBin(0). All setup uses public safe constructors and successful insert/assembly results, not private field fabrication or a bypass of the owning producer.

The host tail truncates the beginning of the aggregate panic, so I do not claim its earliest mismatching sweep row is visible. The exact retained mismatch and the replayed aggregation/assertion identify the failed check sufficiently. The earlier standalone report_deviation artifact/replay error is not used as evidence. Review labels on the original mapping do not invalidate the explicit single-target assumption established here.

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Production evidence:** [`src/jpip.rs:L2065-L2073`](../../src/jpip.rs#L2065-L2073)

```text
        let id = DatabinId::new(8, metadata_codestream, bin)?;
        if !self.is_complete(id) {
            append_missing_ranges(missing, &self.complete_databin_missing(id));
            needs.push(JpipSemanticNeed {
                kind: JpipSemanticNeedKind::MetadataBin,
                identifier: bin,
            });
            return Ok(false);
        }
```

- **Production evidence:** [`src/jpip.rs:L3433-L3435`](../../src/jpip.rs#L3433-L3435)

```text
            if self.databins[index].id.same_as(id) {
                return Some(index);
            }
```

- **Production evidence:** [`src/jpip.rs:L1182-L1184`](../../src/jpip.rs#L1182-L1184)

```text
    pub fn semantic_needs(&self) -> &[JpipSemanticNeed] {
        &self.needs
    }
```

- **Production evidence:** [`src/jpip.rs:L2065-L2073`](../../src/jpip.rs#L2065-L2073)

```text
        let id = DatabinId::new(8, metadata_codestream, bin)?;
        if !self.is_complete(id) {
            append_missing_ranges(missing, &self.complete_databin_missing(id));
            needs.push(JpipSemanticNeed {
                kind: JpipSemanticNeedKind::MetadataBin,
                identifier: bin,
            });
            return Ok(false);
        }
```

- **Production evidence:** [`src/jpip.rs:L1453`](../../src/jpip.rs#L1453)

```text
        let index = self.find_databin_index(fragment.databin);
```

- **Production evidence:** [`src/jpip.rs:L3433-L3435`](../../src/jpip.rs#L3433-L3435)

```text
            if self.databins[index].id.same_as(id) {
                return Some(index);
            }
```

- **Production evidence:** [`src/jpip.rs:L90-L97`](../../src/jpip.rs#L90-L97)

```text
        crate::refinement::jpip_databin_identity(
            self.class_identifier,
            self.codestream,
            self.in_class,
            other.class_identifier,
            other.codestream,
            other.in_class,
        )
```

- **Production evidence:** [`src/refinement.rs:L2418-L2420`](../../src/refinement.rs#L2418-L2420)

```text
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
```

- **Harness evidence:** [`semantic-needs.patch:L189`](tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L189): `+                [(0, 0, 0), (7, 7, 7), (0, 1, 0), (0, 0, 1), (3, 5, 7)]`
- **Harness evidence:** [`semantic-needs.patch:L200`](tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L200): `+                        DatabinId::new(8, root_csn, 0).unwrap(),`
- **Harness evidence:** [`semantic-needs.patch:L210`](tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L210): `+                                DatabinId::new(8, child_csn, bin).unwrap(),`
- **Harness evidence:** [`semantic-needs.patch:L222-L227`](tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L222-L227): `+                    let report = cache.assemble_semantic_container(query_csn).unwrap();<br>+                    let needs: Vec<_> = report<br>+                        .semantic_needs()<br>+                        .iter()<br>+                        .map(\|need\| (need.kind(), need.identifier()))<br>+                        .collect();`
- **Harness evidence:** [`semantic-needs.patch:L257-L261`](tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L257-L261): `+        assert!(<br>+            violations.is_empty(),<br>+            "metadata CSn must not create spurious dependencies:\n{}",<br>+            violations.join("\n")<br>+        );`
- **Test source:** [`semantic-needs.patch:L189`](tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L189)<br>[`semantic-needs.patch:L200`](tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L200)<br>[`semantic-needs.patch:L210`](tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L210)<br>[`semantic-needs.patch:L222-L227`](tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L222-L227)<br>[`semantic-needs.patch:L257-L261`](tests/DEV-A64A9EFAD1/files/semantic-needs.patch#L257-L261)
- **Exact mapping record:** [report.json](report.json)

## DEV-B80F74BB23

- **Mapping:** `b74f5a18aa024a0fe1bc915d` / `cea43b5d307aad42f64c79041cc622df464e320137f63c6aa7f4a99637c6a7db`
- **Specification:** `521d9216cac9df1209a1e7cf`
- **Deviation:** `51c401b27c255044d10cc7f0`
- **Test item:** `afef8cd7d6537e8a08814419`
- **Case:** `9105d99750798402cec08c7f`
- **Exact standards quote:**

> The value of the NCS field shall
>                                                   be treated as if it was set to "1" regardless of the actual value of that field when the
>                                                   field is present.

- **Entry:** `src/jpip.rs:3775:jpip::parse_jpip_placeholder`
- **Command:** `target/placeholder-contract-tests jpip::tests::placeholder_contract_stream_identifiers_and_counts --exact --nocapture`
- **Setup:** First rejected payload: flags=4, OrigID=7, ordinary OrigBH jp2c length 256, intermediate EquivID=9 and ordinary EquivBH jp2c length 256, CSID=0, present NCS=0. Total 48 payload bytes, outer ordinary phld length 56. Complete required fields, valid slices, supported low flags, no unknown extensions, no stream-ID overflow. T.808 (12/2022) Table A.3 requires effective NCS=1 for 01xx even with a present zero-valued NCS. Referenced lengths do not allocate content, and no stream-array expansion or cache-state fabrication occurs.
- **Route:** Host build adds only include!("jpip_placeholder_contract_tests.rs") inside the existing cfg(test) jpip tests module and builds the native library tests. Host runs target/placeholder-contract-tests jpip::tests::placeholder_contract_stream_identifiers_and_counts --exact --nocapture. contract_payload constructs and BoxReader-validates the outer phld, returns its complete payload, and the test directly calls parse_jpip_placeholder(&payload). The parser returns Err for present optional NCS in 01xx layouts; the test collects those errors and fails failures.is_empty(). This is a direct parser unit-contract route, not a claimed end-to-end reconstruction.
- **Observed assertion:** `736 legal placeholders rejected; first failures:<br>flags=0x4, CSID=0x0, NCS=Some(0), wide=false: MalformedInput { offset: 0, kind: InvalidBoxContents }`

**Full diagnosis:**

The first failed assertion is failures.is_empty() at src/jpip_placeholder_contract_tests.rs:173, reporting 736 legal placeholders rejected. Its first concrete case is flags=4, CSID=0, NCS=Some(0), wide=false, returning InvalidBoxContents. This is a valid direct unit-contract entry, not an incomplete DatabinCache fixture. The input is a complete 48-byte payload (56 bytes including ordinary phld framing): Flags/OrigID/8-byte OrigBH/EquivID/8-byte EquivBH/CSID/4-byte NCS. Both referenced headers declare length 256 and type jp2c; their content is not required inside a placeholder. The test validates framing and payload extraction using BoxReader before calling the parser. Zero is permitted in this unused NCS field: effective count is one. The output also independently reports rejection with NCS=Some(1), so the deviation does not depend on zero or an extreme count.
Resource realism: the main matrix is 2 header widths x 23 identifiers x 8 flag values x 5 count representations = 1840 independent parser calls. Each payload is at most 64 bytes (72 framed), with no nesting, recursive reconstruction, large referenced-content allocation, or stream enumeration. NCS and CSID are scalar wire fields; the single-stream failures always request one effective stream, including when encoded NCS is UINT32_MAX. These are far below HARDENED input/metadata limits (2147483648/16777216 bytes), depth 32, and codestream capacity 64. Multi-stream scalar parse tests do not invoke downstream reconstruction or claim it must accept billions of streams. Real caller expand_metadata_bin likewise passes box_ref.payload directly after BoxReader identifies phld. The owning parser has no cache-state or resource-limit parameter; this test respects its slice and read-helper requirements.
Authority is the rfc_map parent, not correlated implementation models. I read its pinned T.808 (12/2022) A.3.6.3, SHA-256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc. Table A.3 explicitly requires 01xx to treat NCS as 1 regardless of its actual value when present; surrounding prose permits termination after CSID but does not require it, and requires intermediate fields even if unused. The harness includes these intermediate fields. Thus this is a concrete refutation of the mapped extraction clause under its originating standards interpretation, not a clause requiring revision to match the parser. REFERENCES.md explicitly leaves the newer 2026 edition unaudited; this finding is bounded to the cited 2022 text and does not establish a newer-edition conformance failure.
Source and MIR identify the exact route: after complete headers, flags & 4 reads CSID and advances position to 44. Since flags & 8 is zero, MIR bb106 -> bb109 sets count=1 without consuming optional NCS. bb124 observes 44 != 48, bb125/bb126/bb127 bypass the provisional-layout diagnostic (48 != original_end+24, which is 44), and bb130/bb131 return InvalidBoxContents. The required result was successful extraction of CSID=0 and stream_count=1. The 736 failures equal both widths x 23 IDs x four 01xx flags x four present NCS values. The compatibility diagnostic remains an error, not an accepting path. No production modification, arbitrary nonzero exit, or unrelated unused-high-bit failure is used as evidence; the recorded successful build and concrete parser error establish this specific deviation.

- **Production evidence:** [`src/jpip.rs:L3834`](../../src/jpip.rs#L3834)

```text
        let count = if flags & 8 != 0 {
```

- **Production evidence:** [`src/jpip.rs:L3842-L3845`](../../src/jpip.rs#L3842-L3845)

```text
            count
        } else {
            1
        };
```

- **Production evidence:** [`src/jpip.rs:L3850-L3851`](../../src/jpip.rs#L3850-L3851)

```text
    if position != payload.len() {
        if flags == 4 && payload.len() == original_end + 24 {
```

- **Production evidence:** [`src/jpip.rs:L3857-L3861`](../../src/jpip.rs#L3857-L3861)

```text
        } else {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
        }
    }
    Ok(JpipPlaceholder {
```

- **Production evidence:** [`src/jpip.rs:L2098`](../../src/jpip.rs#L2098)

```text
                let placeholder = parse_jpip_placeholder(box_ref.payload)?;
```

- **Production evidence:** [`REFERENCES.md:L39-L40`](../../REFERENCES.md#L39-L40)

```text
The T.808 (06/2026) reference above identifies a catalog record, not an acquired
normative text or completed edition-difference review.
```

- **Production evidence:** [`src/jpip.rs:L3834`](../../src/jpip.rs#L3834)

```text
        let count = if flags & 8 != 0 {
```

- **Production evidence:** [`src/jpip.rs:L3842-L3845`](../../src/jpip.rs#L3842-L3845)

```text
            count
        } else {
            1
        };
```

- **Production evidence:** [`src/jpip.rs:L3850`](../../src/jpip.rs#L3850)

```text
    if position != payload.len() {
```

- **Production evidence:** [`src/jpip.rs:L3858-L3861`](../../src/jpip.rs#L3858-L3861)

```text
            return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
        }
    }
    Ok(JpipPlaceholder {
```

- **Harness evidence:** [`src/jpip_placeholder_contract_tests.rs:L115-L123`](tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs#L115-L123): `                    let payload = contract_payload(<br>                        flags,<br>                        7,<br>                        &header,<br>                        Some((9, &header)),<br>                        Some(stream),<br>                        count,<br>                    );<br>                    match parse_jpip_placeholder(&payload) {`
- **Harness evidence:** [`src/jpip_placeholder_contract_tests.rs:L50-L53`](tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs#L50-L53): `    let mut reader = crate::jp2::BoxReader::new(&framed, &ResourceLimits::default()).unwrap();<br>    let received = reader.next_box_ref().unwrap().unwrap();<br>    assert_eq!(received.payload, payload);<br>    assert!(reader.next_box_ref().is_none());`
- **Harness evidence:** [`src/jpip_placeholder_contract_tests.rs:L113-L114`](tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs#L113-L114): `                    let expected = if flags & 8 == 0 { 1 } else { count.unwrap() };<br>                    assert!(stream.checked_add(u64::from(expected) - 1).is_some());`
- **Harness evidence:** [`src/jpip_placeholder_contract_tests.rs:L173-L176`](tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs#L173-L176): `    assert!(<br>        failures.is_empty(),<br>        "{} legal placeholders rejected; first failures:\n{}",<br>        failures.len(),`
- **Test source:** [`src/jpip_placeholder_contract_tests.rs:L115-L123`](tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs#L115-L123)<br>[`src/jpip_placeholder_contract_tests.rs:L50-L53`](tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs#L50-L53)<br>[`src/jpip_placeholder_contract_tests.rs:L113-L114`](tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs#L113-L114)<br>[`src/jpip_placeholder_contract_tests.rs:L173-L176`](tests/DEV-B80F74BB23/files/src/jpip_placeholder_contract_tests.rs#L173-L176)
- **Test source:** [`validation/build_placeholder_contract_tests.py`](tests/DEV-B80F74BB23/files/validation/build_placeholder_contract_tests.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-6744E122C7

- **Mapping:** `c3e124e770eb767b4dca0e91` / `09d36f83bba2854c331cdb0358e423d6e7f8eb6415c5c26bae4dec30fb7aca8a`
- **Specification:** `fb0a3e26284355e86e6cfece`
- **Deviation:** `aa382efda7c3965118fc5f28`
- **Test item:** `e1f075d7b5416d4f472c0f9a`
- **Case:** `9c7d6b44332022f336f2ca0d`
- **Exact standards quote:**

> EquivBH: This field specifies the header of the stream-equivalent box (LBox, TBox and XLBox as needed) of the box
> referenced by this Placeholder box. The length of this field is 8 bytes if the equivalent box header's LBox field is not
> equal to 1 and 16 bytes otherwise.

- **Entry:** `src/jpip.rs:2021:jpip::DatabinCache::expand_metadata_bin`
- **Command:** `python3 validation/run_metadata_contract_suite.py stream_equivalent_uses_equivalent_box_identity`
- **Setup:** A direct helper unit entry over public-API-established cache state, with a legal compact phld, complete bin 1, complete contents-only bin 2 on the failing phase, CSn 0 throughout, flags=2, ignored unavailable OrigID 99, EquivID 2, original xref header, equivalent colr header, consistent depth/budget and noncyclic ancestor identity [0]. Only 67 bytes maximum are cached and maximum reached depth is 2. Bin 1 represents superbox contents, not a complete file; the unit assumes the standard-permitted JPX xref-to-colr context, without claiming public assembler acceptance or full JPX conformance. The helper has no requirement to retrieve or validate the ancestor bin or unavailable original contents.
- **Route:** The replay adapter registers a nested cfg(test) module in the existing native tests, builds the native library tests and runs exactly stream_equivalent_uses_equivalent_box_identity. That test publicly inserts a flags=2 phld in metadata bin 1, calls expand_metadata_bin with bin=1, boxes=true and depth=1, verifies it waits for equivalent bin 2, inserts the seven-byte colr contents into bin 2, and calls the same helper again. The second call selects equivalent contents, recursively copies the leaf payload, then appends the original xref header. After verifying completion and empty diagnostics, the test compares exact bytes against a separately constructed colr box and reaches the reported final assertion.
- **Observed assertion:** `xref->colr original-XLBox=false, equivalent-XLBox=false: got [0, 0, 0, 15, 120, 114, 101, 102, 1, 0, 0, 0, 0, 0, 17], expected [0, 0, 0, 15, 99, 111, 108, 114, 1, 0, 0, 0, 0, 0, 17]`

**Full diagnosis:**

The first host-observed failed assertion is the final failures.is_empty() check, reported as 'wrong stream-equivalent box header'. All preceding admission, missing-payload, completion, active-stack and diagnostic assertions passed. The first compact/compact case emits TBox xref (120,114,101,102) instead of colr (99,111,108,114), violating the exact correctly-identified-box clause; the other three cases also expose use of the original rather than equivalent header width.

Realism was checked before tracing: each of the four sequential cases uses one 44–60-byte placeholder and one 7-byte payload, at most 67 cached bytes, two bins, and recursion from depth 1 to 2 with no cycle. Output is at most 23 bytes. These are far below default maximum_input_bytes=2147483648, maximum_metadata_bytes=16777216 and maximum_box_depth=32. The u32::MAX-1 remaining_depth argument is the ordinary caller's decreasing arithmetic budget, not requested nesting or allocation. The tested context matches recursive box-contents expansion, not an oversized platform stress case.

This is an admissible direct unit-contract test, not a claimed public-assembler path. Public DatabinCache insertion and JpipFragment construction establish complete metadata bins at CSn 0; there is no private-cache corruption. A nonroot bin can contain a superbox's child sequence, and its sole placeholder is top-level within that bin. Entry active=[0], bin=1, boxes=true, depth=1 and remaining_depth=u32::MAX-1 are consistent recursion-context values; ancestor bytes are not a requirement of this helper, which only checks ancestor identities for cycles. A complete JPX file is not this helper's input. The test assumes a JPX host context permitting xref-to-colr substitution and does not establish full-file conformance, cross-reference resolution or pixel equivalence. Flags=2 disables original access, so absent OrigID=99 and unavailable original contents are permitted, not an unmet precondition.

Authority is the rfc_map origin, not the implementation-derived original-shell hypothesis. I read the pinned T.808 (12/2022), ISO/IEC 15444-9:2023, SHA256 0eabe7584ed48bce280a391dfa1bdd217038adc3917920194afd104f4e75d8cc, sections A.3.6.2 and A.3.6.3 through the originating mapping item. A.3.6.2 explicitly permits a Cross-reference box's stream equivalent to be a Colourspace Specification box, with a distinct equivalent box header and contents bin. A.3.6.3 defines EquivBH as that stream-equivalent header; Table A.3 says flags bit 0 clear means ignore OrigID, and bit 1 set supplies equivalent contents. Both compact and extended EquivBH are allowed, while the phld itself correctly uses compact framing. This supports the test oracle for the cited edition. It does not assert conformance to a newer catalog edition.

Compiler declaration and MIR CFG confirm the route: parse the placeholder; bypass flags&1; select flags&2, equivalent_bin=2 and equivalent_type=colr; verify equivalent_header exists but never use it; recursively copy bin 2 opaquely because colr is not a superbox; on successful replacement call append_reconstructed_box with original_header. That helper copies the supplied header and only rewrites its length. Thus valid colr contents are labeled xref, and original header width is retained. The host replay's exact byte assertion demonstrates this operation, rather than relying on a nonzero exit or the runner's printed route. Existing implementation-derived contracts explicitly qualify their original-shell policy as an unestablished equivalence hypothesis; they cannot override this standards evidence. No production changes or additional replay are needed to attribute this already observed mismatch.

- **Production evidence:** [`src/jpip.rs:L2124-L2126`](../../src/jpip.rs#L2124-L2126)

```text
let _header = placeholder.equivalent_header.as_ref().ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::InvalidBoxContents)
                    })?;
```

- **Production evidence:** [`src/jpip.rs:L2206`](../../src/jpip.rs#L2206)

```text
append_reconstructed_box(output, &placeholder.original_header, &payload)?;
```

- **Production evidence:** [`src/jpip.rs:L4113`](../../src/jpip.rs#L4113)

```text
let mut header = copy_bytes(original_header);
```

- **Production evidence:** [`src/jpip.rs:L4122-L4123`](../../src/jpip.rs#L4122-L4123)

```text
output.extend_from_slice(&header);
    output.extend_from_slice(payload);
```

- **Production evidence:** [`src/limits.rs:L133-L135`](../../src/limits.rs#L133-L135)

```text
maximum_input_bytes: 2_147_483_648,
        maximum_metadata_bytes: 16_777_216,
        maximum_box_depth: 32,
```

- **Production evidence:** [`src/jpip.rs:L2117-L2123`](../../src/jpip.rs#L2117-L2123)

```text
} else if placeholder.flags & 2 != 0 {
                    let bin = placeholder.equivalent_bin.ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::InvalidBoxContents)
                    })?;
                    let box_type = placeholder.equivalent_type.ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::InvalidBoxContents)
                    })?;
```

- **Production evidence:** [`src/jpip.rs:L2124-L2126`](../../src/jpip.rs#L2124-L2126)

```text
let _header = placeholder.equivalent_header.as_ref().ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::InvalidBoxContents)
                    })?;
```

- **Production evidence:** [`src/jpip.rs:L2206`](../../src/jpip.rs#L2206)

```text
append_reconstructed_box(output, &placeholder.original_header, &payload)?;
```

- **Production evidence:** [`src/jpip.rs:L4113`](../../src/jpip.rs#L4113)

```text
let mut header = copy_bytes(original_header);
```

- **Production evidence:** [`src/jpip.rs:L4122-L4123`](../../src/jpip.rs#L4122-L4123)

```text
output.extend_from_slice(&header);
    output.extend_from_slice(payload);
```

- **Harness evidence:** [`src/jpip_metadata_contract_tests.rs:L267-L273`](tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L267-L273): `let original = header(*b"xref", 32, original_extended);<br>            let equivalent = header(*b"colr", color.len(), equivalent_extended);<br>            let mut fields = 2_u32.to_be_bytes().to_vec();<br>            fields.extend_from_slice(&99_u64.to_be_bytes());<br>            fields.extend_from_slice(&original);<br>            fields.extend_from_slice(&2_u64.to_be_bytes());<br>            fields.extend_from_slice(&equivalent);`
- **Harness evidence:** [`src/jpip_metadata_contract_tests.rs:L282-L285`](tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L282-L285): `cache<br>                .insert(fragment(id(8, 1), 0, &placeholder, true))<br>                .unwrap();<br>            let mut active = vec![0];`
- **Harness evidence:** [`src/jpip_metadata_contract_tests.rs:L309-L326`](tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L309-L326): `cache.insert(fragment(id(8, 2), 0, &color, true)).unwrap();<br>            missing.clear();<br>            needs.clear();<br>            assert!(<br>                cache<br>                    .expand_metadata_bin(<br>                        0,<br>                        1,<br>                        true,<br>                        1,<br>                        u32::MAX - 1,<br>                        &mut active,<br>                        &mut output,<br>                        &mut missing,<br>                        &mut needs,<br>                    )<br>                    .unwrap()<br>            );`
- **Harness evidence:** [`src/jpip_metadata_contract_tests.rs:L330-L335`](tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L330-L335): `let expected = boxed(*b"colr", &color, equivalent_extended);<br>            if output != expected {<br>                failures.push(format!(<br>                    "xref->colr original-XLBox={original_extended}, equivalent-XLBox={equivalent_extended}: got {output:?}, expected {expected:?}"<br>                ));<br>            }`
- **Harness evidence:** [`src/jpip_metadata_contract_tests.rs:L338-L342`](tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L338-L342): `assert!(<br>        failures.is_empty(),<br>        "wrong stream-equivalent box header:\n{}",<br>        failures.join("\n")<br>    );`
- **Test source:** [`src/jpip_metadata_contract_tests.rs:L267-L273`](tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L267-L273)<br>[`src/jpip_metadata_contract_tests.rs:L282-L285`](tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L282-L285)<br>[`src/jpip_metadata_contract_tests.rs:L309-L326`](tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L309-L326)<br>[`src/jpip_metadata_contract_tests.rs:L330-L335`](tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L330-L335)<br>[`src/jpip_metadata_contract_tests.rs:L338-L342`](tests/DEV-6744E122C7/files/src/jpip_metadata_contract_tests.rs#L338-L342)
- **Test source:** [`validation/run_metadata_contract_suite.py`](tests/DEV-6744E122C7/files/validation/run_metadata_contract_suite.py)
- **Exact mapping record:** [report.json](report.json)

## DEV-174EC5FEE4

- **Mapping:** `15f84a01435cbc3e13b5bdeb` / `97c15e7636f058af255b13133a0052193dafd042f24365dafbeea5293c41abbb`
- **Specification:** `6a3078c49e79034ec262df2c`
- **Deviation:** `2979ee1372269d167a663f1d`
- **Test item:** `dcdc31654724c4c2ad030fd2`
- **Case:** `debd9f4cb2047d9570f27b9b`
- **Exact standards quote:**

> In contrast to the JP2 file format, if the UnkC field is non-zero it is not required that a Colour Specification box be present
> within the JP2 Header box.

- **Entry:** `src/decode.rs:3975:decode::decode_jph`
- **Command:** `cargo test --lib --locked decode::tests::jph_contract::typed_planes_independent_of_colour_conversion -- --exact --nocapture`
- **Setup:** Rust-valid byte slices and default options enter the public API, with no private-state corruption, bypassed owning caller or production modifications. Inputs are small, bounded, canonical JPH-branded containers with a single supported Part 15 payload, valid IHDR/BPCC and empty packets. Paired explicit-colour controls pass complete typed image checks. The sole metadata change is the T.814:2019 D.2-permitted combination UnkC=1, absent colr and absent cdef (hence no Typi=0). D.3 BR/MinV/CL are canonical. Max dimensions 16x14, one/three components and one tile are far below hardened limits. Unspecified colour does not require rendering because this API returns native component planes.
- **Route:** Host cargo test --lib --locked decode::tests::jph_contract::typed_planes_independent_of_colour_conversion -- --exact --nocapture runs the registered child module. Each loop builds and successfully checks an explicit-colour JPH control, then calls the public decode_jph on the same codestream wrapped with UnkC=1 and no colr/cdef. inspect_jph -> parse_and_inspect -> parse -> parse_jp2_header rejects absent color at jp2h payload offset 40; errors are collected, and the final missing_colour_failures.is_empty() assertion fails at line 181. The first rejected pair is index 0, size [1,1,0,0], one signed 1-bit component.
- **Observed assertion:** `D.2 component-only JPH rejected: MalformedInput { offset: 40, kind: MissingRequiredStructure }`

**Full diagnosis:**

The first failed assertion is missing_colour_failures.is_empty() at src/decode/tests/jph_contract.rs:181: every component-only counterpart returns MalformedInput { offset: 40, kind: MissingRequiredStructure }, instead of the same typed planes as its explicit-colour control. This is a reproduced admission deviation from clause 6a3078c49e79034ec262df2c, not a bad pixel oracle or an excessive fixture. Realism was checked first: 24 sequential cases, dimensions at most 16x14, origins at most 3, one or three components, one tile/layer/resolution, no decomposition, shallow box nesting, containers under 200 bytes, and at most 5,376 accounted sample bytes per image even using a deliberately loose three-full-size-plane bound. Default DecodeOptions uses ResourceLimits::HARDENED (1,000,000 per dimension, 100,000,000 pixels, 64 components, 16,384 tiles, 32 box depth, and 2,147,483,648 input/output bytes). Nothing approaches platform or configured capacity. The public validation_decoder likewise passes ordinary file bytes and options directly to decode_jph, and existing native JPH tests use the same public entry with 1x1 images and default options. No caller-owned state or internal fixture is fabricated.

Authority is the rfc_map origin, not agreement with implementation/model oracles. I read its parent finding and re-read pinned ITU-T T.814 (2019-06), hash 172d82969f9eaaa9976802944226579883b36bda02b22c307ee74c7bb8d08e58, D.2 and D.3 via the originating mapping item. D.2 explicitly permits absent Colour Specification with nonzero UnkC, leaving colourspace unspecified and prohibiting Typi=0. The harness sets UnkC=1 and omits cdef entirely, so there is no prohibited Typ field. D.3 branding requirements are satisfied by BR='jph ', MinV=0, CL='jph '. Its otherwise identical control includes colr and is successfully decoded and checked before each failing call; both share matching IHDR/BPCC, the same Part 15 Rsiz/CAP/COD, length-correct tile structure and empty packets. In particular index 0 is an ordinary 1x1 single signed component, not dependent on heterogeneous/subsampled cases. The assertion requires component samples, not implicit conversion of unspecified colour. README separates those APIs, and SUPPORTED.md selects this same Part 15 edition without excluding unspecified-colour component decoding. Thus revising the clause to require colr would hide the standards admission mismatch.

The assessed route is the direct valid unit-contract call decode_jph(&unspecified, &options) -> inspect_jph -> parse_and_inspect -> parse -> parse_jp2_header. parse passes the jp2h payload's base offset (40) without any JPH-specific admission context. parse_image_header accepts and records unknown_color_space=true; parse_jp2_header validates the present BPCC, encounters no palette, then unconditionally evaluates color.ok_or_else(...MissingRequiredStructure). That is the specific erroneous check. The error propagates unchanged through inspection and decode_jph's first '?', preventing raw decoding and returning no planes. Compiler declaration/CFG evidence confirms decode_jph bb1 calls inspect_jph and bb3 -> bb6 -> bb28 returns its rejection before decode_codestream. The host replay built successfully, executed the exact named test, reached all 24 paired controls, and failed at this aggregate check; this attribution does not rely on arbitrary exit status or setup failure. It is an agent-assessed reproduced public-entry deviation, not a formal reachability or general conformance proof.

- **Production evidence:** [`src/decode.rs:L3975-L3976`](../../src/decode.rs#L3975-L3976)

```text
pub fn decode_jph(input: &[u8], options: &DecodeOptions) -> Result<TypedImage, DecodeError> {
    let inspected = jp2::inspect_jph(input, options.limits())?;
```

- **Production evidence:** [`src/jp2.rs:L915`](../../src/jp2.rs#L915)

```text
let inspected = parse_and_inspect(input, limits)?;
```

- **Production evidence:** [`src/jp2.rs:L871`](../../src/jp2.rs#L871)

```text
let file = parse(input, limits)?;
```

- **Production evidence:** [`src/jp2.rs:L1052-L1056`](../../src/jp2.rs#L1052-L1056)

```text
            parsed_header = Some(parse_jp2_header(
                box_ref.payload,
                limits,
                box_ref.header.payload_offset()?,
            )?);
```

- **Production evidence:** [`src/jp2.rs:L2276-L2278`](../../src/jp2.rs#L2276-L2278)

```text
    let color = color.ok_or_else(|| {
        DecodeError::malformed(base_offset, MalformedKind::MissingRequiredStructure)
    })?;
```

- **Production evidence:** [`src/jp2.rs:L2366-L2368`](../../src/jp2.rs#L2366-L2368)

```text
    let unknown_color_space = match box_ref.payload[12] {
        0 => false,
        1 => true,
```

- **Production evidence:** [`README.md:L59-L61`](../../README.md#L59-L61)

```text
Color-conversion and
JPX layer/frame APIs are separate from component sample decoding; see the
[public exports](src/lib.rs).
```

- **Production evidence:** [`src/decode.rs:L3975-L3976`](../../src/decode.rs#L3975-L3976)

```text
pub fn decode_jph(input: &[u8], options: &DecodeOptions) -> Result<TypedImage, DecodeError> {
    let inspected = jp2::inspect_jph(input, options.limits())?;
```

- **Production evidence:** [`src/jp2.rs:L1052-L1056`](../../src/jp2.rs#L1052-L1056)

```text
            parsed_header = Some(parse_jp2_header(
                box_ref.payload,
                limits,
                box_ref.header.payload_offset()?,
            )?);
```

- **Production evidence:** [`src/jp2.rs:L2276-L2278`](../../src/jp2.rs#L2276-L2278)

```text
    let color = color.ok_or_else(|| {
        DecodeError::malformed(base_offset, MalformedKind::MissingRequiredStructure)
    })?;
```

- **Harness evidence:** [`src/decode/tests/jph_contract.rs:L63`](tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L63): `    ihdr.extend_from_slice(&[255, 7, u8::from(!colour), 0]);`
- **Harness evidence:** [`src/decode/tests/jph_contract.rs:L70-L75`](tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L70-L75): `    if colour {<br>        let space = if planes.len() == 1 { 17 } else { 16 };<br>        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, space]));<br>    }<br>    // D.2: no colr implies UnkC=1 and no channel definition with Typ=0.<br>    // There is no cdef box here, hence no Typ fields at all.`
- **Harness evidence:** [`src/decode/tests/jph_contract.rs:L163-L165`](tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L163-L165): `        let known = container(&cs, size, planes, b"jph \0\0\0\0jph ", true);<br>        let decoded = decode_jph(&known, &options).expect("supported JPH with explicit colour");<br>        assert_zero_image(&decoded, size, planes);`
- **Harness evidence:** [`src/decode/tests/jph_contract.rs:L169-L170`](tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L169-L170): `        let unspecified = container(&cs, size, planes, b"jph \0\0\0\0jph ", false);<br>        match decode_jph(&unspecified, &options) {`
- **Harness evidence:** [`src/decode/tests/jph_contract.rs:L181-L184`](tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L181-L184): `    assert!(<br>        missing_colour_failures.is_empty(),<br>        "UnkC=1, no colr, no cdef must return the same typed planes: {missing_colour_failures:?}"<br>    );`
- **Test source:** [`src/decode/tests/jph_contract.rs:L63`](tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L63)<br>[`src/decode/tests/jph_contract.rs:L70-L75`](tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L70-L75)<br>[`src/decode/tests/jph_contract.rs:L163-L165`](tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L163-L165)<br>[`src/decode/tests/jph_contract.rs:L169-L170`](tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L169-L170)<br>[`src/decode/tests/jph_contract.rs:L181-L184`](tests/DEV-174EC5FEE4/files/src/decode/tests/jph_contract.rs#L181-L184)
- **Exact mapping record:** [report.json](report.json)
