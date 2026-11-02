// Included in packet::tests so the existing fixtures and private subject are used.
fn jpip_template(sop: bool, eph: bool) -> TilePartPayload<'static> {
    jpip_template_layers(sop, eph, 2)
}

fn jpip_template_layers(sop: bool, eph: bool, layers: u16) -> TilePartPayload<'static> {
    let mut style = coding_style(sop, eph);
    style.layers = layers;
    let tile = TilePartPayload::uniform_for_packet_decoding(
        0, 0, 1, 1, style, crate::codestream::RoiShifts::none(), &[],
    ).unwrap();
    assert_eq!(tile.coding_style.sop_markers, sop);
    assert_eq!(tile.coding_style.eph_markers, eph);
    assert!(!tile.packed_packet_headers.is_present());
    tile
}

fn jpip_framed(header: &[u8], body: &[u8], sop: bool, eph: bool, sequence: u16) -> Vec<u8> {
    assert!(!header.is_empty());
    assert_ne!(header.last(), Some(&0xff), "T.800 B.10.1 final-header-byte premise");
    let mut bytes = Vec::new();
    if sop {
        bytes.extend_from_slice(&[0xff, 0x91, 0, 4]);
        bytes.extend_from_slice(&sequence.to_be_bytes());
    }
    bytes.extend_from_slice(header);
    if eph {
        bytes.extend_from_slice(&[0xff, 0x92]);
    }
    bytes.extend_from_slice(body);
    bytes
}

fn jpip_assert_inspect(
    template: &TilePartPayload<'_>, bytes: &[u8], sequence: u16, layer: u16,
    precinct: &mut PacketPrecinctDecoder, limits: &ResourceLimits,
    expected: Result<usize, DecodeError>,
) {
    let decoders = format!("{:?}", precinct.decoders);
    let coordinates = (precinct.component, precinct.resolution, precinct.precinct);
    let mut contexts = precinct.contexts.clone();
    for context in &mut contexts {
        context.layer = layer;
    }
    eprintln!(
        "route=packet::decode_jpip_precinct_packet_inner bytes={bytes:02x?} \
         SOP={} EPH={} sequence={sequence} layer={layer} expected={expected:?}",
        template.coding_style.sop_markers, template.coding_style.eph_markers,
    );
    assert_eq!(
        decode_jpip_precinct_packet_inner(template, bytes, sequence, layer, precinct, limits),
        expected,
        "inline inspection result/error/prefix"
    );
    assert_eq!(precinct.contexts, contexts, "install every context's layer only");
    assert_eq!(format!("{:?}", precinct.decoders), decoders, "inspection must not run Tier-1");
    assert_eq!((precinct.component, precinct.resolution, precinct.precinct), coordinates);
}

fn jpip_terminal_ff_packet(eph: bool) -> (PacketPrecinctDecoder, Vec<u8>) {
    let mut precinct = literal_packet_precinct();
    precinct.contexts[0].maximum_zero_bitplanes = 8;
    precinct.decoders[0] = Tier1CodeBlockDecoder::new(
        1, 1, 8, SubbandOrientation::Ll, Tier1CodingStyle::default(),
    ).unwrap();
    // B.10: presence=1, inclusion=1, ZBP=0000001 (six), passes=0 (one),
    // Lblock=111110 (3+5), length=11111111 (255). These are exactly 24
    // information bits, C0 BE FF. B.10.1 requires the final stuffed 00.
    let mut bits = vec![true, true];
    bits.extend([false; 6]);
    bits.extend([true, false]);
    bits.extend([true; 5]);
    bits.push(false);
    bits.extend([true; 8]);
    assert_eq!(bits.len(), 24);
    assert_eq!(pack_packet_bits(&bits), [0xc0, 0xbe, 0xff]);
    let header = [0xc0, 0xbe, 0xff, 0];
    assert_eq!(header[3] & 0x80, 0, "mandatory stuffed zero");
    let body = vec![0xff; 255];
    (precinct, jpip_framed(&header, &body, false, eph, 0))
}

#[test]
fn jpip_contract_sop_policy_and_exact_parser_errors() {
    let limits = ResourceLimits::default();
    let template = jpip_template(true, false);
    let mut seed = 0x52a7_u32;
    let mut sequences = vec![0, 1, 255, 256, 32767, 32768, 65534, 65535];
    for _ in 0..24 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        sequences.push((seed >> 16) as u16);
    }
    for sequence in sequences {
        for present in [false, true] {
            let header = [if present { 0xe1 } else { 0 }];
            let body: &[u8] = if present { &[0xff] } else { &[] };
            let bytes = jpip_framed(&header, body, true, false, sequence);
            jpip_assert_inspect(&template, &bytes, sequence, 0, &mut literal_packet_precinct(),
                &limits, Ok(if present { 8 } else { 7 }));
            jpip_assert_inspect(&template, &bytes, sequence.wrapping_add(1), 0,
                &mut literal_packet_precinct(), &limits, Err(invalid_packet_header()));
        }
    }
    let valid = jpip_framed(&[0], &[], true, false, 0x1234);
    for end in 0..valid.len() {
        jpip_assert_inspect(&template, &valid[..end], 0x1234, 0,
            &mut literal_packet_precinct(), &limits,
            Err(packet_reader_error(0, MalformedKind::Truncated)));
    }
    for (index, value) in [(0, 0xfe), (1, 0x90), (2, 1), (3, 3), (3, 5)] {
        let mut bad = valid.clone();
        bad[index] = value;
        jpip_assert_inspect(&template, &bad, 0x1234, 0, &mut literal_packet_precinct(),
            &limits, Err(invalid_packet_header()));
    }
    jpip_assert_inspect(&jpip_template(false, false), &[0], u16::MAX, 0,
        &mut literal_packet_precinct(), &limits, Ok(1));
}

#[test]
fn jpip_contract_retained_trees_layers_and_no_tier1() {
    let limits = ResourceLimits::default();
    for first_layer in [0_u16, 1, 2, 7, 15] {
        let template = jpip_template_layers(false, false, 32);
        let mut precinct = literal_packet_precinct();
        precinct.contexts[0].maximum_zero_bitplanes = 16;
        precinct.decoders[0] = Tier1CodeBlockDecoder::new(
            1, 1, 16, SubbandOrientation::Ll, Tier1CodingStyle::default(),
        ).unwrap();
        let pristine_decoder = format!("{:?}", precinct.decoders);
        let mut passes = 0;
        for layer in 0..32_u16 {
            let included = layer >= first_layer;
            let contributes = included && (layer == first_layer || layer % 3 != 0);
            // First inclusion: tree=1, zero-plane tree=1, one pass=0,
            // Lblock stop=0, length=001. Later inclusion omits both tree codes.
            let bytes: &[u8] = if !contributes { &[0x80] }
                else if layer == first_layer { &[0xe1, 0xff] }
                else { &[0xc2, 0xff] };
            jpip_assert_inspect(&template, bytes, 0, layer, &mut precinct, &limits,
                Ok(if contributes { 2 } else { 1 }));
            passes += u32::from(contributes);
            assert_eq!(precinct.states[0], CodeBlockPacketState {
                included, zero_bitplanes: included.then_some(0), lblock: 3,
                total_passes: passes, high_throughput: included.then_some(false),
            });
            assert_eq!(precinct.subbands[0].inclusion_tree.nodes[0], TagTreeNode {
                low: u32::from(if included { first_layer } else { layer + 1 }),
                value: if included { u32::from(first_layer) } else { u32::MAX },
            });
            assert_eq!(precinct.subbands[0].zero_bitplane_tree.nodes[0], TagTreeNode {
                low: 0, value: if included { 0 } else { u32::MAX },
            });
            assert_eq!(format!("{:?}", precinct.decoders), pristine_decoder);
        }
    }
    for data in [&[0xe1][..], &[0xe1, 0xff, 0x93, 0xff][..]] {
        let eph = data.len() > 1;
        let mut precinct = literal_packet_precinct();
        jpip_assert_inspect(&jpip_template(false, eph), data, 0, 0, &mut precinct,
            &limits, Err(if eph { invalid_packet_header() }
                else { packet_reader_error(0, MalformedKind::Truncated) }));
        assert_eq!(precinct.states[0].total_passes(), 1, "inner adapter retains header commits on error");
        assert!(precinct.states[0].included());
        assert_eq!(precinct.states[0].zero_bitplanes(), Some(0));
        for tree in [&precinct.subbands[0].inclusion_tree, &precinct.subbands[0].zero_bitplane_tree] {
            assert_eq!(tree.nodes[0], TagTreeNode { low: 0, value: 0 });
        }
    }
    // Retain the repository's existing raw-state/rollback regression as well.
    actual_jpip_outer_restores_all_raw_fields_and_clone_preserves_unused_tree_words();
}

#[test]
fn jpip_contract_eph_policy_errors_and_prefix() {
    let limits = ResourceLimits::default();
    for sop in [false, true] {
        for present in [false, true] {
            let header = [if present { 0xe1 } else { 0 }];
            let body: &[u8] = if present { &[0xff] } else { &[] };
            let prefix = usize::from(sop) * 6 + 1;
            for eph in [false, true] {
                let template = jpip_template_layers(sop, eph, 32);
                let mut bytes = jpip_framed(&header, body, sop, eph, 17);
                let expected = prefix + usize::from(eph) * 2 + body.len();
                bytes.extend_from_slice(&[0x5a, 0xa5]);
                jpip_assert_inspect(&template, &bytes, 17, 0,
                    &mut literal_packet_precinct(), &limits, Ok(expected));
                if eph {
                    for end in [prefix, prefix + 1] {
                        jpip_assert_inspect(&template, &bytes[..end], 17, 0,
                            &mut literal_packet_precinct(), &limits,
                            Err(packet_reader_error(0, MalformedKind::Truncated)));
                    }
                    for marker in [[0xff, 0x93], [0, 0x92], [0xff, 0], [0x92, 0xff]] {
                        let mut bad = bytes.clone();
                        bad[prefix..prefix + 2].copy_from_slice(&marker);
                        jpip_assert_inspect(&template, &bad, 17, 0,
                            &mut literal_packet_precinct(), &limits, Err(invalid_packet_header()));
                    }
                }
            }
        }
    }
    let (mut precinct, data) = jpip_terminal_ff_packet(true);
    assert_eq!(&data[4..6], &[0xff, 0x92], "EPH follows the complete stuffed header");
    jpip_assert_inspect(&jpip_template(false, true), &data, 0, 0, &mut precinct,
        &limits, Ok(4 + 2 + 255));
}

#[test]
fn jpip_contract_empty_presence_preserves_contributions() {
    let limits = ResourceLimits::default();
    let mut seed = 0x341e_7091_u32;
    for sop in [false, true] {
        for eph in [false, true] {
            for retained in [false, true] {
                let template = jpip_template_layers(sop, eph, 32);
                let mut precinct = literal_packet_precinct();
                if retained {
                    let data = jpip_framed(&[0xe1], &[0xff], sop, eph, 0);
                    jpip_assert_inspect(&template, &data, 0, 0, &mut precinct, &limits, Ok(data.len()));
                }
                let states = precinct.states.clone();
                let trees = format!("{:?}", (&precinct.subbands[0].inclusion_tree,
                    &precinct.subbands[0].zero_bitplane_tree));
                for sequence in 1..=24 {
                    let mut data = jpip_framed(&[0], &[], sop, eph, sequence);
                    let prefix = data.len();
                    for _ in 0..usize::from(sequence % 7) {
                        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                        data.push((seed >> 24) as u8);
                    }
                    jpip_assert_inspect(&template, &data, sequence, sequence, &mut precinct,
                        &limits, Ok(prefix));
                    assert_eq!(precinct.states, states, "empty packet contributes no passes or bytes");
                    assert_eq!(format!("{:?}", (&precinct.subbands[0].inclusion_tree,
                        &precinct.subbands[0].zero_bitplane_tree)), trees, "empty packet skips tag trees");
                }
            }
        }
    }
    jpip_assert_inspect(&jpip_template(false, false), &[], 0, 0,
        &mut literal_packet_precinct(), &limits,
        Err(packet_reader_error(0, MalformedKind::Truncated)));
    // Nonempty control discriminates an inspector that always reports an empty packet.
    let mut precinct = literal_packet_precinct();
    jpip_assert_inspect(&jpip_template(false, false), &[0xe1, 0xff], 0, 0,
        &mut precinct, &limits, Ok(2));
    assert_eq!(precinct.states[0].total_passes(), 1);
    jpip_assert_inspect(&jpip_template_layers(false, false, u16::MAX), &[0], 0,
        u16::MAX - 1, &mut literal_packet_precinct(), &limits, Ok(1));
}

// Observe the real parser's accounting on an independent, equal pre-state.
// Expected counts/errors still come from the fixture, never from this call.
fn jpip_assert_parser_accounting(
    template: &TilePartPayload<'_>, data: &[u8], expected: Result<usize, DecodeError>,
) {
    assert!(!template.packed_packet_headers.is_present());
    assert!(!template.packet_lengths.is_present());
    let inline = TilePartPayload::uniform_for_packet_decoding(
        0, 0, 1, 1, template.coding_style, crate::codestream::RoiShifts::none(), data,
    ).unwrap();
    let mut precinct = literal_packet_precinct();
    set_progression_context_layer(&mut precinct.contexts, 0).unwrap();
    let actual = decode_precinct_packet(
        &inline, 0, &[], 0, None, 255, &mut precinct, false, &ResourceLimits::default(),
    );
    eprintln!("route=decode_precinct_packet(inline, decode_tier1=false) accounting={actual:?}");
    match expected {
        Ok(prefix) => {
            let (decoded, physical, packed) = actual.expect("bounded complete inline packet");
            assert_eq!(packed, 0, "inline inspection cannot consume packed header bytes");
            assert_eq!(physical, prefix, "physical prefix from explicit fixture framing");
            let logical = usize::try_from(decoded.bytes_consumed).unwrap();
            assert_eq!(logical, prefix,
                "decoded logical count from explicit fixture framing");
            assert_eq!(physical, logical,
                "adapter's physical/logical equality premise");
        }
        Err(error) => assert_eq!(actual, Err(error), "exact native parser failure to propagate"),
    }
}

#[test]
fn jpip_contract_exact_inline_lengths_and_no_partial_success() {
    let limits = ResourceLimits::default();
    for sop in [false, true] {
        for eph in [false, true] {
            let template = jpip_template(sop, eph);
            let empty = jpip_framed(&[0], &[], sop, eph, 255);
            let empty_prefix = usize::from(sop) * 6 + 1 + usize::from(eph) * 2;
            jpip_assert_parser_accounting(&template, &empty, Ok(empty_prefix));
            jpip_assert_inspect(&template, &empty, 255, 0,
                &mut literal_packet_precinct(), &limits, Ok(empty_prefix));
            for end in 0..empty_prefix {
                let expected = Err(packet_reader_error(0, MalformedKind::Truncated));
                jpip_assert_parser_accounting(&template, &empty[..end], expected);
                jpip_assert_inspect(&template, &empty[..end], 255, 0,
                    &mut literal_packet_precinct(), &limits, expected);
            }
            for length in 1_u8..=7 {
                let body = vec![0xff; usize::from(length)];
                let data = jpip_framed(&[0xe0 | length], &body, sop, eph, 255);
                let prefix = usize::from(sop) * 6 + 1 + usize::from(eph) * 2 + usize::from(length);
                for suffix in [&[][..], &[0][..], &[0xff, 0x91, 0, 4, 0, 0, 0][..]] {
                    let mut concatenated = data.clone();
                    concatenated.extend_from_slice(suffix);
                    jpip_assert_parser_accounting(&template, &concatenated, Ok(prefix));
                    jpip_assert_inspect(&template, &concatenated, 255, 0,
                        &mut literal_packet_precinct(), &limits, Ok(prefix));
                }
                let body_start = prefix - usize::from(length);
                for end in body_start..prefix {
                    jpip_assert_parser_accounting(&template, &data[..end],
                        Err(packet_reader_error(0, MalformedKind::Truncated)));
                    jpip_assert_inspect(&template, &data[..end], 255, 0,
                        &mut literal_packet_precinct(), &limits,
                        Err(packet_reader_error(0, MalformedKind::Truncated)));
                }
            }
        }
    }
    let limits = limits.with_maximum_packet_header_bytes(1).unwrap();
    jpip_assert_inspect(&jpip_template(false, false), &[0xe1, 0xff], 0, 0,
        &mut literal_packet_precinct(), &limits, Ok(2));
    // Two code-blocks force a second header byte; all structure remains valid.
    let mut precinct = literal_packet_precinct();
    precinct.states.push(code_block_packet_state_new());
    precinct.contexts.push(CodeBlockPacketContext {
        leaf_x: 1, ..literal_precinct_header_context()
    });
    for context in &mut precinct.contexts {
        context.layer = 9;
    }
    precinct.decoders.push(one_bit_legacy_decoder(false));
    precinct.subbands[0].decoder_count = 2;
    precinct.subbands[0].code_blocks_across = 2;
    precinct.subbands[0].inclusion_tree = tag_tree_new(2, 1, &ResourceLimits::default()).unwrap();
    precinct.subbands[0].zero_bitplane_tree = tag_tree_new(2, 1, &ResourceLimits::default()).unwrap();
    let mut data = two_block_first_layer_header();
    data.extend_from_slice(&[0xff; 4]);
    jpip_assert_inspect(&jpip_template(false, false), &data, 0, 0,
        &mut precinct, &limits, Err(DecodeError::LimitExceeded {
            resource: Resource::PacketHeaderBytes, limit: 1, actual: 2,
        }));
}

// Separate standards oracle: not evidence of a mismatch between the adapter's
// two internal counts, which can both omit the same terminal stuffing byte.
#[test]
fn jpip_contract_normative_terminal_stuffing_prefix() {
    let (mut precinct, data) = jpip_terminal_ff_packet(false);
    jpip_assert_inspect(&jpip_template(false, false), &data, 0, 0,
        &mut precinct, &ResourceLimits::default(), Ok(4 + 255));
    assert_eq!(precinct.states[0].zero_bitplanes(), Some(6));
    assert_eq!(precinct.states[0].lblock, 8);
    assert_eq!(precinct.states[0].total_passes(), 1);
}

fn jpip_packed_template_stream() -> Vec<u8> {
    fn segment(stream: &mut Vec<u8>, marker: u16, payload: &[u8]) {
        stream.extend_from_slice(&marker.to_be_bytes());
        stream.extend_from_slice(&u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
        stream.extend_from_slice(payload);
    }
    let mut stream = vec![0xff, 0x4f];
    let mut siz = vec![0, 0];
    for value in [1_u32, 1, 0, 0, 1, 1, 0, 0] {
        siz.extend_from_slice(&value.to_be_bytes());
    }
    siz.extend_from_slice(&[0, 1, 7, 1, 1]);
    segment(&mut stream, 0xff51, &siz);
    segment(&mut stream, 0xff52, &[0, 0, 0, 2, 0, 0, 0, 0, 0, 1]);
    segment(&mut stream, 0xff5c, &[0x20, 0]);
    // Two empty packets: logical lengths 1,1, with headers packed into PPT.
    segment(&mut stream, 0xff90, &[0, 0, 0, 0, 0, 28, 0, 1]);
    segment(&mut stream, 0xff58, &[0, 1, 1]);
    segment(&mut stream, 0xff61, &[0, 0, 0]);
    stream.extend_from_slice(&[0xff, 0x93, 0xff, 0xd9]);
    stream
}

#[test]
fn jpip_contract_template_storage_independence() {
    // Existing integrated PPT/PLT regression must retain all original assertions.
    full_progression_uses_real_ppt_reassembly_and_plt_iteration_without_tile_body_bytes();
    let limits = ResourceLimits::default();
    let stream = jpip_packed_template_stream();
    let layout = crate::codestream::parse_layout(&stream, &limits).unwrap();
    assert_eq!(layout.tile_parts.len(), 1);
    let packed = &layout.tile_parts[0];
    assert!(packed.data.is_empty());
    assert!(packed.packed_packet_headers.is_present());
    assert!(packed.packet_lengths.is_present());
    assert!(!packed.coding_style.sop_markers && !packed.coding_style.eph_markers);
    assert_eq!(packed.coding_style.layers, 2);
    let template_before = format!("{packed:?}");
    for initial_include in [false, true] {
        for length in 1_u8..=7 {
            // Equal coding-style fields and equal logical precinct pre-states;
            // only irrelevant template payload/PPT/PLT storage differs.
            let payloads: [&[u8]; 3] = [&[], &[0], &[0xff, 0x91, 0, 4, 0, 7, 0]];
            for payload in payloads {
                let plain = TilePartPayload::uniform_for_packet_decoding(
                    0, 0, 1, 1, packed.coding_style,
                    crate::codestream::RoiShifts::none(), payload,
                ).unwrap();
                assert_eq!(plain.coding_style, packed.coding_style);
                assert_eq!(plain.component_coding_styles.style(0).unwrap(),
                    packed.component_coding_styles.style(0).unwrap());
                assert!(!plain.packed_packet_headers.is_present());
                assert!(!plain.packet_lengths.is_present());
                for truncated in [false, true] {
                    let mut a = literal_packet_precinct();
                    let mut b = literal_packet_precinct();
                    for p in [&mut a, &mut b] {
                        p.contexts[0].maximum_zero_bitplanes = 2;
                        p.decoders[0] = Tier1CodeBlockDecoder::new(
                            1, 1, 2, SubbandOrientation::Ll, Tier1CodingStyle::default(),
                        ).unwrap();
                    }
                    if initial_include {
                        for p in [&mut a, &mut b] {
                            jpip_assert_inspect(&plain, &[0xe1, 0xff], 0, 0, p, &limits, Ok(2));
                        }
                    }
                    assert_eq!(exact_packet_precinct_snapshot(&a), exact_packet_precinct_snapshot(&b));
                    let mut bytes = vec![if initial_include { 0xc0 | (length << 1) } else { 0xe0 | length }];
                    bytes.extend(vec![0xff; usize::from(length) - usize::from(truncated)]);
                    let expected = if truncated { Err(packet_reader_error(0, MalformedKind::Truncated)) }
                        else { Ok(1 + usize::from(length)) };
                    let layer = u16::from(initial_include);
                    jpip_assert_inspect(&plain, &bytes, 0, layer, &mut a, &limits, expected);
                    jpip_assert_inspect(packed, &bytes, 0, layer, &mut b, &limits, expected);
                    assert_eq!(exact_packet_precinct_snapshot(&a), exact_packet_precinct_snapshot(&b),
                        "same result AND full final state despite template storage differences");
                    assert_eq!(a.states[0].total_passes(), 1 + u32::from(initial_include));
                }
            }
        }
    }
    assert_eq!(format!("{packed:?}"), template_before);
}
