// Included in packet::tests so the existing literal fixtures and regressions
// remain the single source of test-only access to the private packet path.

fn contract_segment(stream: &mut Vec<u8>, marker: u16, payload: &[u8]) {
    stream.extend_from_slice(&marker.to_be_bytes());
    stream.extend_from_slice(&u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
    stream.extend_from_slice(payload);
}

fn contract_codestream(flags: u8, layers: u16, data: &[u8], packed: Option<&[u8]>) -> Vec<u8> {
    assert_eq!(flags & !6, 0);
    assert_ne!(layers, 0);
    let mut stream = vec![0xff, 0x4f];
    let mut siz = vec![0, 0]; // Part 1, no extended capability signalling.
    for value in [1_u32, 1, 0, 0, 1, 1, 0, 0] {
        siz.extend_from_slice(&value.to_be_bytes());
    }
    siz.extend_from_slice(&[0, 1, 7, 1, 1]);
    contract_segment(&mut stream, 0xff51, &siz);
    let [hi, lo] = layers.to_be_bytes();
    contract_segment(&mut stream, 0xff52, &[flags, 0, hi, lo, 0, 0, 0, 0, 0, 1]);
    contract_segment(&mut stream, 0xff5c, &[0x20, 0]);
    let mut tile_header = Vec::new();
    if let Some(headers) = packed {
        assert!(!headers.is_empty());
        let mut ppt = vec![0]; // Zppt=0, one complete sequence, no PPM.
        ppt.extend_from_slice(headers);
        contract_segment(&mut tile_header, 0xff61, &ppt);
    }
    let psot = u32::try_from(12 + tile_header.len() + 2 + data.len()).unwrap();
    let mut sot = vec![0, 0];
    sot.extend_from_slice(&psot.to_be_bytes());
    sot.extend_from_slice(&[0, 1]);
    contract_segment(&mut stream, 0xff90, &sot);
    stream.extend_from_slice(&tile_header);
    stream.extend_from_slice(&[0xff, 0x93]);
    stream.extend_from_slice(data);
    stream.extend_from_slice(&[0xff, 0xd9]);
    stream
}

fn contract_layout<'a>(
    stream: &'a [u8],
    flags: u8,
    layers: u16,
    packed: Option<&[u8]>,
) -> crate::codestream::CodestreamLayout<'a> {
    let limits = &ResourceLimits::HARDENED;
    let layout = crate::codestream::parse_layout(stream, limits).unwrap();
    assert_eq!(layout.tile_parts.len(), 1);
    let part = &layout.tile_parts[0];
    assert_eq!(
        (part.tile_index, part.tile_part_index, part.tile_part_count),
        (0, 0, 1)
    );
    assert_eq!(part.coding_style.layers, layers);
    assert_eq!(part.coding_style.decomposition_levels, 0);
    assert_eq!(part.coding_style.code_block_style, 0);
    assert_eq!(part.coding_style.code_block_width, 4);
    assert_eq!(part.coding_style.code_block_height, 4);
    assert_eq!(part.coding_style.sop_markers, flags & 2 != 0);
    assert_eq!(part.coding_style.eph_markers, flags & 4 != 0);
    assert_eq!(part.packed_packet_headers.is_present(), packed.is_some());
    assert_eq!(
        part.packed_packet_headers.reassemble(limits).unwrap(),
        packed.unwrap_or(&[])
    );
    layout
}

#[allow(clippy::too_many_arguments)]
fn contract_progression_call(
    parts: &[TilePartPayload<'_>],
    selection: Option<&[u8]>,
    coordinate: PacketCoordinate,
    offset: usize,
    packed_offset: usize,
    packets: u64,
    expected: Option<u32>,
    precinct: &mut PacketPrecinctDecoder,
) -> Result<(PacketBodyDecode, usize, usize), DecodeError> {
    let limits = ResourceLimits::default();
    assert!(packets <= limits.maximum_packets());
    assert!(offset <= parts[0].data.len());
    let components =
        u16::try_from(selection.map_or(usize::from(coordinate.component) + 1, <[u8]>::len))
            .unwrap();
    let resolutions = parts[0].coding_style.decomposition_levels + 1;
    let grid = packet_grid_new_with_component_resolutions(
        parts[0].coding_style.layers,
        components,
        vec![resolutions; usize::from(components)],
        vec![1; usize::from(components) * usize::from(resolutions)],
        &limits,
    )
    .unwrap();
    let input = prepare_progression_payloads(parts, parts[0].tile_index, &limits).unwrap();
    assert!(packed_offset <= input.packed[0].len());
    assert!(coordinate.component < components);
    assert!(coordinate.resolution < resolutions);
    assert!(coordinate.layer < grid.layers);
    assert_eq!(
        (precinct.component, precinct.resolution, precinct.precinct),
        (
            coordinate.component,
            coordinate.resolution,
            coordinate.precinct
        )
    );
    let params = ProgressionTileParameters {
        tile_parts: parts,
        grid: &grid,
        input: &input,
        selection,
        limits: &limits,
    };
    let mut progress = progression_tile_progress_new(&params).unwrap();
    // This is a unit-contract fixture, not a forged caller execution. The
    // subject has no requires; isolate its counter and cursor inputs directly.
    progress.offset = offset;
    progress.packed_offset = packed_offset;
    progress.packets = packets;
    eprintln!(
        "decode_progression_precinct_packet: coord={coordinate:?}, tile={offset}, packed={packed_offset}, packets={packets}, expected={expected:?}, selection={selection:?}, SOP={}, EPH={}",
        parts[0].coding_style.sop_markers, parts[0].coding_style.eph_markers
    );
    decode_progression_precinct_packet(
        &params, &parts[0], &progress, coordinate, expected, precinct,
    )
}

fn contract_coordinate(layer: u16) -> PacketCoordinate {
    PacketCoordinate {
        layer,
        component: 0,
        resolution: 0,
        precinct: 0,
    }
}

fn contract_body(present: bool, execute: bool, marker_bytes: u32) -> PacketBodyDecode {
    PacketBodyDecode {
        present,
        code_blocks: 1,
        contributions: u32::from(present),
        segments: u32::from(present && execute),
        passes: u32::from(present && execute),
        header_bytes: 1,
        body_bytes: u32::from(present),
        bytes_consumed: 1 + u32::from(present) + marker_bytes,
    }
}

fn contract_sop(sequence: u16) -> Vec<u8> {
    let [hi, lo] = sequence.to_be_bytes();
    vec![0xff, 0x91, 0, 4, hi, lo]
}

#[test]
fn progression_contract_sop_modulo_65536() {
    let mut counts = vec![
        0, 1, 255, 256, 32767, 65534, 65535, 65536, 65537, 131071, 131072,
    ];
    let mut seed = 0x51a7_2026_u32;
    for _ in 0..64 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        counts.push(u64::from(seed % 262144));
    }
    for count in counts {
        let sequence = u16::try_from(count % 65536).unwrap();
        for wrong in [false, true] {
            let mut data = contract_sop(if wrong {
                sequence.wrapping_add(1)
            } else {
                sequence
            });
            data.push(0);
            let stream = contract_codestream(2, 1, &data, None);
            let layout = contract_layout(&stream, 2, 1, None);
            let mut precinct = literal_packet_precinct();
            let before = exact_packet_precinct_snapshot(&precinct);
            let result = contract_progression_call(
                &layout.tile_parts,
                None,
                contract_coordinate(0),
                0,
                0,
                count,
                Some(7),
                &mut precinct,
            );
            assert_eq!(
                result,
                if wrong {
                    Err(invalid_packet_header())
                } else {
                    Ok((contract_body(false, true, 6), 7, 0))
                },
                "Nsop must equal packets % 65536: count={count}, wrong={wrong}"
            );
            assert_eq!(exact_packet_precinct_snapshot(&precinct), before);
        }
    }
}

#[test]
fn progression_contract_eph_cursors_and_preflight() {
    precinct_payload_literal_inline_and_packed_streams_account_for_markers_and_real_coefficients();
    for packed in [false, true] {
        for sop in [false, true] {
            for eph in [false, true] {
                for prior_packet in [false, true] {
                    // A real preceding empty packet makes the two cursors
                    // differ: its header lives in PPT, its SOP stays in SOD.
                    let mut tile = Vec::new();
                    let mut headers = Vec::new();
                    if prior_packet {
                        if sop {
                            tile.extend(contract_sop(0));
                        }
                        headers.push(0);
                        if eph {
                            headers.extend_from_slice(&[0xff, 0x92]);
                        }
                        if !packed {
                            tile.append(&mut headers);
                        }
                    }
                    let tile_offset = tile.len();
                    let packed_offset = headers.len();
                    if sop {
                        tile.extend(contract_sop(u16::from(prior_packet)));
                    }
                    let mut target_header = vec![0xe1];
                    if eph {
                        target_header.extend_from_slice(&[0xff, 0x92]);
                    }
                    if packed {
                        headers.extend_from_slice(&target_header);
                    } else {
                        tile.extend_from_slice(&target_header);
                    }
                    tile.push(0xff); // One legacy cleanup pass, one body byte.
                    let flags = u8::from(sop) * 2 + u8::from(eph) * 4;
                    let stream =
                        contract_codestream(flags, 2, &tile, packed.then_some(headers.as_slice()));
                    let layout =
                        contract_layout(&stream, flags, 2, packed.then_some(headers.as_slice()));
                    let mut precinct = literal_packet_precinct();
                    let markers = u32::from(sop) * 6 + u32::from(eph) * 2;
                    assert_eq!(
                        contract_progression_call(
                            &layout.tile_parts,
                            None,
                            contract_coordinate(u16::from(prior_packet)),
                            tile_offset,
                            packed_offset,
                            u64::from(prior_packet),
                            None,
                            &mut precinct,
                        ),
                        Ok((
                            contract_body(true, true, markers),
                            if packed {
                                usize::from(sop) * 6 + 1
                            } else {
                                markers as usize + 2
                            },
                            if packed { 1 + usize::from(eph) * 2 } else { 0 }
                        ))
                    );
                    assert_eq!(precinct.contexts[0].layer, u16::from(prior_packet));
                    assert!(precinct.states[0].included());
                    assert_eq!(precinct.states[0].total_passes(), 1);
                    assert_eq!(precinct.decoders[0].next_pass(), 1);
                    assert_eq!(precinct.decoders[0].decoded_segments(), 1);
                }
            }
        }
        for truncated in [false, true] {
            let target_header: &[u8] = if truncated {
                &[0xe1, 0xff]
            } else {
                &[0xe1, 0xff, 0x93]
            };
            let mut headers = vec![0, 0xff, 0x92];
            let mut tile = vec![0, 0xff, 0x92];
            let (tile_offset, packed_offset) = if packed {
                tile.clear();
                headers.extend_from_slice(target_header);
                tile.push(0xff);
                (0, 3)
            } else {
                tile.extend_from_slice(target_header);
                if !truncated {
                    tile.push(0xff);
                }
                (3, 0)
            };
            let stream = contract_codestream(4, 2, &tile, packed.then_some(headers.as_slice()));
            let layout = contract_layout(&stream, 4, 2, packed.then_some(headers.as_slice()));
            let mut precinct = literal_packet_precinct();
            let decoder_before = format!("{:?}", precinct.decoders);
            assert_eq!(
                contract_progression_call(
                    &layout.tile_parts,
                    None,
                    contract_coordinate(1),
                    tile_offset,
                    packed_offset,
                    1,
                    None,
                    &mut precinct,
                ),
                Err(DecodeError::MalformedInput {
                    offset: if truncated { tile_offset as u64 } else { 0 },
                    kind: if truncated {
                        MalformedKind::Truncated
                    } else {
                        MalformedKind::InvalidPacketHeader
                    },
                }),
                "EPH must be validated in the header stream before body execution"
            );
            assert_eq!(format!("{:?}", precinct.decoders), decoder_before);
            assert_eq!(precinct.contexts[0].layer, 1);
            assert!(precinct.states[0].included());
            assert_eq!(precinct.states[0].total_passes(), 1);
        }
    }
}

#[test]
fn progression_contract_selection_boundary_and_structure() {
    full_selected_progression_derives_literal_headers_and_skips_only_higher_resolution_coefficients(
    );
    let counts = [1_u8, 2, 4];
    let mut cases = Vec::new();
    for component in 0..3 {
        for resolution in 0..4 {
            cases.push((component, resolution));
        }
    }
    let mut seed = 0x9e37_79b9_u32;
    for _ in 0..32 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        cases.push(((seed % 3) as u16, ((seed >> 8) % 4) as u8));
    }
    for (component, resolution) in cases {
        for selection in [None, Some(counts.as_slice())] {
            for malformed in [0, 1, 2] {
                let data: &[u8] = match malformed {
                    0 => &[0xe1, 0xff, 0x92, 0xff],
                    1 => &[0xe1, 0xff, 0x93, 0xff],
                    _ => &[0xe1, 0xff, 0x92], // Declared body byte missing.
                };
                let mut style = coding_style(false, true);
                style.layers = 2;
                style.decomposition_levels = 3;
                let parts = [TilePartPayload::uniform_for_packet_decoding(
                    0,
                    0,
                    1,
                    3,
                    style,
                    crate::codestream::RoiShifts::none(),
                    data,
                )
                .unwrap()];
                let mut precinct = literal_packet_precinct();
                precinct.component = component;
                precinct.resolution = resolution;
                let decoder_before = format!("{:?}", precinct.decoders);
                let coordinate = PacketCoordinate {
                    component,
                    resolution,
                    layer: 1,
                    precinct: 0,
                };
                let execute = selection.is_none() || resolution < counts[usize::from(component)];
                let result = contract_progression_call(
                    &parts,
                    selection,
                    coordinate,
                    0,
                    0,
                    0,
                    None,
                    &mut precinct,
                );
                assert_eq!(
                    result,
                    match malformed {
                        0 => Ok((contract_body(true, execute, 2), 4, 0)),
                        1 => Err(invalid_packet_header()),
                        _ => Err(packet_reader_error(0, MalformedKind::Truncated)),
                    },
                    "selection must use resolution < counts[component], and still validate structure"
                );
                assert_eq!(precinct.contexts[0].layer, 1);
                assert!(precinct.states[0].included());
                assert_eq!(precinct.states[0].total_passes(), 1);
                assert_eq!(precinct.states[0].zero_bitplanes(), Some(0));
                if !execute || malformed != 0 {
                    assert_eq!(format!("{:?}", precinct.decoders), decoder_before);
                } else {
                    assert_ne!(format!("{:?}", precinct.decoders), decoder_before);
                    assert_eq!(precinct.decoders[0].next_pass(), 1);
                    assert_eq!(precinct.decoders[0].decoded_segments(), 1);
                }
            }
        }
    }
}

#[test]
fn progression_contract_sop_policy_and_optional_marker() {
    // T.800 A.6.1 Scod allows SOP, A.8.1 does not require one per
    // packet. Omission is intentionally a standards oracle, not the model.
    let mut omissions = Vec::new();
    for packed in [false, true] {
        for flags in [0_u8, 2, 4, 6] {
            let eph = flags & 4 != 0;
            let mut header = vec![0];
            if eph {
                header.extend_from_slice(&[0xff, 0x92]);
            }
            let tile: &[u8] = if packed { &[] } else { &header };
            let stream = contract_codestream(flags, 1, tile, packed.then_some(header.as_slice()));
            let layout = contract_layout(&stream, flags, 1, packed.then_some(header.as_slice()));
            let mut precinct = literal_packet_precinct();
            let before = exact_packet_precinct_snapshot(&precinct);
            let actual = contract_progression_call(
                &layout.tile_parts,
                None,
                contract_coordinate(0),
                0,
                0,
                0,
                None,
                &mut precinct,
            );
            let expected = Ok((
                contract_body(false, true, u32::from(eph) * 2),
                if packed { 0 } else { header.len() },
                if packed { header.len() } else { 0 },
            ));
            if actual != expected {
                omissions.push(format!(
                    "T.800 A.8.1 permits absent SOP with Scod bit 1 set: packed={packed}, flags={flags}, actual={actual:?}, expected={expected:?}"
                ));
            }
            assert_eq!(exact_packet_precinct_snapshot(&precinct), before);
            if flags & 2 != 0 {
                let mut marked = contract_sop(0);
                if !packed {
                    marked.extend_from_slice(&header);
                }
                let stream =
                    contract_codestream(flags, 1, &marked, packed.then_some(header.as_slice()));
                let layout =
                    contract_layout(&stream, flags, 1, packed.then_some(header.as_slice()));
                assert_eq!(
                    contract_progression_call(
                        &layout.tile_parts,
                        None,
                        contract_coordinate(0),
                        0,
                        0,
                        0,
                        None,
                        &mut precinct,
                    ),
                    Ok((
                        contract_body(false, true, 6 + u32::from(eph) * 2),
                        if packed { 6 } else { marked.len() },
                        if packed { header.len() } else { 0 }
                    ))
                );
                assert_eq!(exact_packet_precinct_snapshot(&precinct), before);
            }
        }
    }
    for (prefix, error) in [
        (
            &[0xff, 0x90, 0, 4, 0, 0][..],
            MalformedKind::InvalidPacketHeader,
        ),
        (
            &[0xff, 0x91, 0, 5, 0, 0][..],
            MalformedKind::InvalidPacketHeader,
        ),
        (
            &[0xff, 0x91, 0, 4, 0, 1][..],
            MalformedKind::InvalidPacketHeader,
        ),
        (&[0xff, 0x91, 0, 4][..], MalformedKind::Truncated),
    ] {
        let mut data = prefix.to_vec();
        if error != MalformedKind::Truncated {
            data.push(0);
        }
        let stream = contract_codestream(2, 1, &data, None);
        let layout = contract_layout(&stream, 2, 1, None);
        let mut precinct = literal_packet_precinct();
        let before = exact_packet_precinct_snapshot(&precinct);
        assert_eq!(
            contract_progression_call(
                &layout.tile_parts,
                None,
                contract_coordinate(0),
                0,
                0,
                0,
                None,
                &mut precinct,
            ),
            Err(packet_reader_error(0, error))
        );
        assert_eq!(exact_packet_precinct_snapshot(&precinct), before);
    }
    assert!(omissions.is_empty(), "{}", omissions.join("\n"));
}
