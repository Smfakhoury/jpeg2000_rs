// Included in codestream::tests by the replay build, never in production.
fn tile_contract_stream(
    components: u16,
    rsiz: u16,
    main: &[Vec<u8>],
    tile: &[Vec<u8>],
    data: &[u8],
    unbounded: bool,
) -> Vec<u8> {
    let mut bytes = codestream_with_headers(components, main, tile, data);
    bytes[6..8].copy_from_slice(&rsiz.to_be_bytes());
    for offset in [8, 12, 24, 28] {
        bytes[offset..offset + 4].copy_from_slice(&1_u32.to_be_bytes());
    }
    let sot = 2 + siz_with_components(components).len() + main.iter().map(Vec::len).sum::<usize>();
    assert_eq!(&bytes[sot..sot + 2], &SOT.to_be_bytes());
    assert_eq!(&bytes[sot + 10..sot + 12], &[0, 1]);
    if unbounded {
        bytes[sot + 6..sot + 10].fill(0);
    }
    bytes
}

fn tile_contract_cod(layers: u16, order: u8, width: u8, style: u8) -> Vec<u8> {
    let [hi, lo] = layers.to_be_bytes();
    segment(COD, &[0, order, hi, lo, 0, 0, width, 2, style, 1])
}

// Direct unit entry: parse the genuine prefix and SOT before calling the subject.
// PLM's selected group is constructed from the standard's *8-bit* Nplm, not
// parse_plm's different framing. No parser fields or private state are mutated.
fn tile_contract_unit<'a>(
    bytes: &'a [u8],
    limits: &'a ResourceLimits,
) -> Result<(TilePartPayload<'a>, MctParameters, Part2Extensions), DecodeError> {
    let mut parser = new_parser(bytes, limits).unwrap();
    parser.expect_standalone(SOC).unwrap();
    let siz = parser.read_segment().unwrap();
    assert_eq!(siz.marker, SIZ);
    let (size, rsiz) = parser.parse_siz(siz).unwrap();
    assert!(rsiz == 0 || rsiz == 0x8100);
    assert_eq!((size.width, size.height), (1, 1));
    let cod = parser.read_segment().unwrap();
    assert_eq!(cod.marker, COD);
    let style = parser.parse_cod(cod).unwrap();
    let qcd = parser.read_segment().unwrap();
    assert_eq!(qcd.marker, QCD);
    let quant = parser.parse_qcd(qcd, style).unwrap();
    let mut main_lengths = PacketLengths::none();
    if parser.peek_marker().unwrap() == PLM {
        let plm = parser.read_segment().unwrap();
        assert_eq!(plm.payload[0], 0);
        assert_eq!(usize::from(plm.payload[1]), plm.payload.len() - 2);
        let encoded = &plm.payload[2..];
        assert!(encoded.iter().all(|&b| b == 1));
        main_lengths = PacketLengths {
            source: PacketLengthSource::Plm(encoded),
            packet_count: encoded.len() as u64,
            encoded_bytes: encoded.len() as u64,
        };
    }
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
    if result.is_ok() {
        assert_eq!(
            parser.position,
            bytes.len() - 2,
            "successful tile end cursor"
        );
        assert_eq!(parser.peek_marker().unwrap(), EOC);
    }
    result
}

fn tile_contract_lengths(lengths: PacketLengths<'_>, expected: &[u32]) {
    assert!(lengths.is_present());
    assert_eq!(lengths.packet_count(), expected.len() as u64);
    let mut iter = lengths.iter();
    for &length in expected {
        assert_eq!(iter.next_length().unwrap(), Some(length));
    }
    assert_eq!(iter.next_length().unwrap(), None);
}

#[test]
fn read_tile_part_contract_styles() {
    retains_main_and_tile_component_style_and_quantization_overrides();
    let limits = ResourceLimits::default();
    for components in [1, 2, 3, 64] {
        for mask in 0..8 {
            let last = u8::try_from(components - 1).unwrap();
            let mut main = vec![tile_contract_cod(1, 0, 2, 0), segment(QCD, &[0x40, 0x40])];
            let mut tile = vec![];
            if mask & 1 != 0 {
                main.push(segment(COC, &[last, 0, 0, 1, 2, 1, 1]));
            }
            if mask & 2 != 0 {
                tile.push(tile_contract_cod(1, 4, 0, 2));
            }
            if mask & 4 != 0 {
                tile.push(segment(COC, &[last, 0, 0, 3, 2, 4, 1]));
            }
            let bytes = tile_contract_stream(
                components,
                0,
                &main,
                &tile,
                &vec![0; usize::from(components)],
                false,
            );
            eprintln!("parse_layout -> read_tile_part: styles components={components} mask={mask}");
            let layout = parse_layout(&bytes, &limits).unwrap();
            for component in 0..components {
                let selected = if component == components - 1 && mask & 4 != 0 {
                    (32, 4)
                } else if mask & 2 != 0 {
                    (4, 2)
                } else if component == components - 1 && mask & 1 != 0 {
                    (8, 1)
                } else {
                    (16, 0)
                };
                let expected = CodingStyle {
                    progression_order: if mask & 2 != 0 {
                        ProgressionOrder::Cprl
                    } else {
                        ProgressionOrder::Lrcp
                    },
                    layers: 1,
                    multiple_component_transform: false,
                    decomposition_levels: 0,
                    decomposition_structure: None,
                    decomposition_directions: [DecompositionDirection::TwoDimensional; 32],
                    code_block_width: selected.0,
                    code_block_height: 16,
                    code_block_style: selected.1,
                    transform: WaveletTransform::Reversible5x3,
                    precincts_defined: false,
                    sop_markers: false,
                    eph_markers: false,
                    precinct_exponents: [(15, 15); 33],
                };
                assert_eq!(
                    layout.tile_parts[0]
                        .component_coding_styles
                        .style(component)
                        .unwrap(),
                    expected
                );
            }
        }
    }
}

#[test]
fn read_tile_part_contract_quantization() {
    tile_default_quantization_precedes_main_qcc_and_tile_qcc_precedes_default();
    validates_tile_default_quantization_duplicates_lengths_and_compatibility();
    retains_main_and_tile_component_style_and_quantization_overrides();
    let limits = ResourceLimits::default();
    let mut seed = 0x7351_u32;
    for components in [1, 2, 3, 64] {
        for mask in 0..8 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let base = if mask == 0 {
                0
            } else if mask == 7 {
                28
            } else {
                (seed % 29) as u8
            };
            let exponents = [base, base + 1, base + 2, base + 3];
            let last = u8::try_from(components - 1).unwrap();
            let mut main = vec![
                tile_contract_cod(1, 0, 2, 0),
                segment(QCD, &[0x40, exponents[0] << 3]),
            ];
            let mut tile = vec![];
            if mask & 1 != 0 {
                main.push(segment(QCC, &[last, 0x60, exponents[1] << 3]));
            }
            if mask & 2 != 0 {
                tile.push(segment(QCD, &[0x80, exponents[2] << 3]));
            }
            if mask & 4 != 0 {
                tile.push(segment(QCC, &[last, 0xa0, exponents[3] << 3]));
            }
            let bytes = tile_contract_stream(
                components,
                0,
                &main,
                &tile,
                &vec![0; usize::from(components)],
                false,
            );
            let layout = parse_layout(&bytes, &limits).unwrap();
            for component in 0..components {
                let index = if component == components - 1 && mask & 4 != 0 {
                    3
                } else if mask & 2 != 0 {
                    2
                } else if component == components - 1 && mask & 1 != 0 {
                    1
                } else {
                    0
                };
                let quant = layout.tile_parts[0]
                    .component_quantizations
                    .quantization(component)
                    .unwrap();
                assert_eq!(quant.style, QuantizationStyle::None);
                assert_eq!(quant.guard_bits, 2 + index as u8);
                assert_eq!(quant.step_count, 1);
                assert_eq!(
                    quant.step(0).unwrap(),
                    QuantizationStep {
                        exponent: exponents[index],
                        mantissa: 0
                    }
                );
            }
        }
    }
}

#[test]
fn read_tile_part_contract_mct() {
    let limits = ResourceLimits::default();
    let values = [i16::MIN, -1, 0, i16::MAX];
    for split in 0..=8 {
        let fragments = mct_i16_segmented(1, 1, &values, split);
        for unbounded in [false, true] {
            let make = |tile: &[Vec<u8>]| {
                tile_contract_stream(2, 0x8100, &baseline_headers(), tile, &[0, 0], unbounded)
            };
            let segmented = make(&fragments);
            let single = make(&[mct_i16(1, 1, &values)]);
            eprintln!("read_tile_part: unused MCT Rsiz=8100 split={split} Psot_zero={unbounded}");
            let (tile, params, _) = tile_contract_unit(&segmented, &limits).unwrap();
            let (_, whole, _) = tile_contract_unit(&single, &limits).unwrap();
            // Same descriptor and bytes, arbitrary fragment boundary: exact array
            // equality is required; segment framing and work accounting may differ.
            assert_eq!(params, whole);
            assert_eq!(params.arrays.len(), 1);
            assert_eq!(params.arrays[0].index(), 1);
            assert_eq!(params.arrays[0].values(), &values.map(i64::from));
            assert!(
                tile.multiple_component_transform.is_none(),
                "unreferenced arrays must not execute"
            );
            let incomplete = make(&fragments[..1]);
            assert!(matches!(
                tile_contract_unit(&incomplete, &limits),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::MissingRequiredStructure,
                    ..
                })
            ));
            let duplicate = make(&[fragments[0].clone(), fragments[0].clone()]);
            assert!(matches!(
                tile_contract_unit(&duplicate, &limits),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidTransform,
                    ..
                })
            ));
            let mut discontinuous = fragments[1].clone();
            discontinuous[4..6].copy_from_slice(&2_u16.to_be_bytes());
            let wrong = make(&[fragments[0].clone(), discontinuous]);
            assert!(matches!(
                tile_contract_unit(&wrong, &limits),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidTransform,
                    ..
                })
            ));
        }
    }
}

#[test]
fn read_tile_part_contract_plm_plt() {
    let limits = ResourceLimits::default();
    let mut failures = vec![];
    let wrong_sum = tile_contract_stream(1, 0, &baseline_headers(), &[plt(0, &[2])], &[0], false);
    assert!(matches!(
        tile_contract_unit(&wrong_sum, &limits),
        Err(DecodeError::MalformedInput {
            kind: MalformedKind::InvalidPacketLength,
            ..
        })
    ));
    let duplicate = tile_contract_stream(
        1,
        0,
        &baseline_headers(),
        &[plt(0, &[1]), plt(0, &[1])],
        &[0, 0],
        false,
    );
    assert!(matches!(
        tile_contract_unit(&duplicate, &limits),
        Err(DecodeError::MalformedInput {
            kind: MalformedKind::InvalidMarkerOrder,
            ..
        })
    ));
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
            eprintln!("read_tile_part unit: T.800 A.7.2/A.7.3 count={count} sources={sources}");
            match tile_contract_unit(&bytes, &limits) {
                Ok((tile, _, _)) => {
                    tile_contract_lengths(tile.packet_lengths, &lengths);
                    assert_eq!(tile.data, vec![0; usize::from(count)]);
                }
                Err(error) => failures.push(format!("count={count} sources={sources}: {error:?}")),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "consistent PLM/PLT must work separately AND together: {failures:?}"
    );
}

#[test]
fn read_tile_part_contract_lengths() {
    parses_retains_and_iterates_plm_and_plt_packet_lengths();
    let limits = ResourceLimits::default();
    let mut failures = vec![];
    // One 1x1 code-block, one pass, zero bitplanes=0, Lblock=3,
    // body length=1: packet header bits 1|1|1|0|0|001 = e1.
    for ppm_source in [false, true] {
        for unbounded in [false, true] {
            for declared in [1_u32, 2, 3, 4] {
                let mut main = vec![tile_contract_cod(1, 0, 2, 0), segment(QCD, &[0x40, 8])];
                let mut tile = vec![plt(0, &[declared])];
                if ppm_source {
                    main.push(ppm(0, &[0, 0, 0, 1, 0xe1]));
                } else {
                    tile.push(ppt(0, &[0xe1]));
                }
                let bytes = tile_contract_stream(1, 0, &main, &tile, &[0], unbounded);
                eprintln!(
                    "parse_layout -> read_tile_part: relocated header=e1 body=00 declared={declared} PPM={ppm_source} Psot_zero={unbounded}"
                );
                match parse_layout(&bytes, &limits) {
                    Ok(layout) if declared == 1 => {
                        tile_contract_lengths(layout.tile_parts[0].packet_lengths, &[1]);
                        assert_eq!(layout.tile_parts[0].data, &[0]);
                        assert_eq!(
                            layout.tile_parts[0]
                                .packed_packet_headers
                                .reassemble(&limits)
                                .unwrap(),
                            [0xe1]
                        );
                    }
                    Err(DecodeError::MalformedInput {
                        kind: MalformedKind::InvalidPacketLength,
                        ..
                    }) if declared != 1 => {}
                    result => {
                        let outcome = match result {
                            Ok(_) => "unexpected success".to_owned(),
                            Err(error) => format!("{error:?}"),
                        };
                        failures.push(format!(
                            "declared={declared} PPM={ppm_source} Psot_zero={unbounded}: {outcome}"
                        ));
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "T.800 A.7.3 excludes relocated packet headers: {failures:?}"
    );
}

#[test]
fn read_tile_part_contract_packed_sources() {
    parses_reassembles_and_validates_packed_packet_headers();
    let limits = ResourceLimits::default();
    for header in [vec![], vec![0], vec![0; 128]] {
        let count = u16::try_from(header.len().max(1)).unwrap();
        let mut frame = (header.len() as u32).to_be_bytes().to_vec();
        frame.extend(&header);
        let main = vec![
            tile_contract_cod(count, 0, 2, 0),
            segment(QCD, &[0, 0]),
            ppm(0, &frame),
        ];
        let bytes = tile_contract_stream(1, 0, &main, &[], &[], false);
        let layout = parse_layout(&bytes, &limits).unwrap();
        assert!(layout.tile_parts[0].packed_packet_headers.is_present());
        assert_eq!(
            layout.tile_parts[0]
                .packed_packet_headers
                .reassemble(&limits)
                .unwrap(),
            header
        );
        let both = tile_contract_stream(1, 0, &main, &[ppt(0, &[0])], &[], false);
        assert!(
            matches!(
                parse_layout(&both, &limits),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidMarkerOrder,
                    ..
                })
            ),
            "PPM presence, including a zero-length frame, excludes PPT"
        );
    }
}

#[test]
fn read_tile_part_contract_progression() {
    accepts_all_part_one_progression_orders();
    parses_retains_and_persists_progression_changes();
    let limits = ResourceLimits::default();
    for (order, expected) in [
        (0, ProgressionOrder::Lrcp),
        (1, ProgressionOrder::Rlcp),
        (2, ProgressionOrder::Rpcl),
        (3, ProgressionOrder::Pcrl),
        (4, ProgressionOrder::Cprl),
    ] {
        let bytes = tile_contract_stream(
            1,
            0,
            &baseline_headers(),
            &[tile_contract_cod(1, order, 2, 0)],
            &[0],
            false,
        );
        assert_eq!(
            parse_layout(&bytes, &limits).unwrap().tile_parts[0]
                .coding_style
                .progression_order,
            expected
        );
    }
    let invalid = tile_contract_stream(
        1,
        0,
        &baseline_headers(),
        &[tile_contract_cod(1, 5, 2, 0)],
        &[0],
        false,
    );
    assert!(matches!(
        parse_layout(&invalid, &limits),
        Err(DecodeError::MalformedInput {
            kind: MalformedKind::InvalidCodingStyle,
            ..
        })
    ));
    // B.12.3: each volume is announced before its packet, first POC in
    // TPsot=0. One empty packet per layer; no future volume is used early.
    let first = poc(&[[0, 0, 0, 1, 1, 1, 1]]);
    let second = poc(&[[0, 0, 0, 2, 1, 1, 4]]);
    let mut bytes = two_part_codestream(&[first], &[second]);
    for offset in [8, 12, 24, 28] {
        bytes[offset..offset + 4].copy_from_slice(&1_u32.to_be_bytes());
    }
    eprintln!(
        "parse_layout -> read_tile_part: B.12.3 two layers; first POC describes layer 0, second POC layer 1"
    );
    let layout = parse_layout(&bytes, &limits).expect("legal incrementally announced POC volumes");
    assert_eq!(layout.tile_parts.len(), 2);
    for (i, order) in [ProgressionOrder::Rlcp, ProgressionOrder::Cprl]
        .into_iter()
        .enumerate()
    {
        let records = layout.tile_parts[i].progression_changes.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].layer_end, (i + 1) as u16);
        assert_eq!(records[0].progression_order, order);
        assert_eq!(layout.tile_parts[i].data, [0]);
    }
}

#[test]
fn read_tile_part_contract_extensions() {
    tile_header_part2_markers_override_inherited_indexes();
    rejects_tile_header_cbd_as_malformed();
    let limits = ResourceLimits::default();
    let known = segment(NLT, &[0, 0, 7, 0]);
    let control = tile_contract_stream(
        1,
        0x8200,
        &baseline_headers(),
        &[known.clone()],
        &[0],
        false,
    );
    let control_layout = parse_layout(&control, &limits).unwrap();
    assert_eq!(
        control_layout.tile_parts[0]
            .part2_extensions
            .nonlinear_transforms()[0]
            .precision(),
        8
    );
    let mut failures = vec![];
    for payload in [
        vec![],
        vec![0],
        vec![0xff, 0x93, 0xff, 0x90],
        vec![0x55; 128],
    ] {
        let unknown = segment(0xff65, &payload);
        assert_eq!(
            u16::from_be_bytes([unknown[2], unknown[3]]) as usize,
            payload.len() + 2
        );
        let bytes = tile_contract_stream(
            1,
            0x8200,
            &baseline_headers(),
            &[unknown, known.clone()],
            &[0],
            false,
        );
        eprintln!(
            "parse_layout -> read_tile_part: T.801 A.1 Rsiz=8200 unknown FF65 payload_len={}",
            payload.len()
        );
        match parse_layout(&bytes, &limits) {
            Ok(layout) => {
                let actual = &layout.tile_parts[0];
                let expected = &control_layout.tile_parts[0];
                assert_eq!(actual.part2_extensions, expected.part2_extensions);
                assert_eq!(actual.coding_style, expected.coding_style);
                assert_eq!(actual.quantization, expected.quantization);
                assert_eq!(actual.data, expected.data);
                assert_eq!(layout.info.consumed_bytes, bytes.len() as u64);
            }
            Err(error) => failures.push(format!("payload_len={}: {error:?}", payload.len())),
        }
    }
    let malformed = tile_contract_stream(
        1,
        0x8200,
        &baseline_headers(),
        &[vec![0xff, 0x65, 0, 1]],
        &[0],
        false,
    );
    assert!(matches!(
        parse_layout(&malformed, &limits),
        Err(DecodeError::MalformedInput {
            kind: MalformedKind::InvalidMarkerLength,
            ..
        })
    ));
    assert!(
        failures.is_empty(),
        "unknown, well-framed segments must be discarded, not rejected: {failures:?}"
    );
}
