use super::*;

// T.800 A.6.1/A.6.2: ordinary Part 1 SPcoc, no Part 2 selectors or CAP modes.
fn default_style() -> CodingStyle {
    CodingStyle {
        progression_order: ProgressionOrder::Lrcp,
        layers: 1,
        multiple_component_transform: false,
        decomposition_levels: 3,
        decomposition_structure: None,
        decomposition_directions: [DecompositionDirection::TwoDimensional; 32],
        code_block_width: 64,
        code_block_height: 64,
        code_block_style: 0,
        transform: WaveletTransform::Reversible5x3,
        precincts_defined: true,
        sop_markers: false,
        eph_markers: false,
        precinct_exponents: [(7, 8); 33],
    }
}

fn payload(components: u16, component: u16, parameters: &[u8], explicit: bool) -> Vec<u8> {
    assert!((1..=16384).contains(&components));
    let mut bytes = if components <= 256 {
        vec![u8::try_from(component).unwrap()]
    } else {
        component.to_be_bytes().to_vec()
    };
    bytes.push(u8::from(explicit));
    bytes.extend_from_slice(parameters);
    bytes
}

fn parse(
    bytes: &[u8],
    components: u16,
    default: CodingStyle,
) -> Result<(u16, CodingStyle), DecodeError> {
    let limits = ResourceLimits::HARDENED;
    assert!((1..=limits.maximum_layers()).contains(&default.layers));
    assert!(default.decomposition_levels < limits.maximum_resolutions_per_component());
    // The helper receives a delimited segment, not a cursor into a whole image.
    // Use real marker framing/read_segment and check its exact consumption.
    let mut framed = segment(COM, &[0, 1]);
    framed.extend(segment(COC, bytes));
    let mut reader = new_parser(&framed, &limits).unwrap();
    assert_eq!(reader.read_segment().unwrap().marker, COM);
    let segment = reader.read_segment().unwrap();
    assert_eq!(segment.marker, COC);
    assert_eq!(segment.offset, 6);
    assert_eq!(segment.payload, bytes);
    assert_eq!(reader.position, framed.len());
    let before = (
        reader.position,
        reader.work_units,
        reader.packet_length_bytes,
        reader.packet_lengths,
    );
    let result = reader.parse_coc(segment, components, default);
    assert_eq!(
        (
            reader.position,
            reader.work_units,
            reader.packet_length_bytes,
            reader.packet_lengths,
        ),
        before,
        "parse_coc must be a non-consuming segment interpreter"
    );
    result
}

fn expected_style(default: CodingStyle, parameters: &[u8], explicit: bool) -> CodingStyle {
    assert!(parameters[0] <= 32);
    assert!(parameters[1] <= 8 && parameters[2] <= 8);
    assert!(u16::from(parameters[1]) + u16::from(parameters[2]) <= 8);
    assert!(parameters[3] < 64 && parameters[4] <= 1);
    let depth = usize::from(parameters[0]);
    assert_eq!(parameters.len(), 5 + if explicit { depth + 1 } else { 0 });
    let mut precinct_exponents = [(15, 15); 33];
    if explicit {
        for (resolution, &value) in parameters[5..].iter().enumerate() {
            let pair = (value % 16, value / 16);
            assert!(resolution == 0 || (pair.0 > 0 && pair.1 > 0));
            precinct_exponents[resolution] = pair;
        }
    }
    CodingStyle {
        progression_order: default.progression_order,
        layers: default.layers,
        multiple_component_transform: default.multiple_component_transform,
        sop_markers: default.sop_markers,
        eph_markers: default.eph_markers,
        decomposition_levels: parameters[0],
        decomposition_structure: None,
        decomposition_directions: [DecompositionDirection::TwoDimensional; 32],
        code_block_width: 4 * 2_u16.pow(u32::from(parameters[1])),
        code_block_height: 4 * 2_u16.pow(u32::from(parameters[2])),
        code_block_style: parameters[3],
        transform: if parameters[4] == 0 {
            WaveletTransform::Irreversible9x7
        } else {
            WaveletTransform::Reversible5x3
        },
        precincts_defined: explicit,
        precinct_exponents,
    }
}

#[test]
fn component_style_inherits_only_cod_wide_fields() {
    eprintln!("route: read_segment -> Parser::parse_coc; T.800 A.6.1/A.6.2");
    // Reuse the repository's existing raw-contract and installation regressions.
    actual_raw_component_styles_preserve_source_defaults_selectors_and_first_errors();
    retains_main_and_tile_component_style_and_quantization_overrides();
    let orders = [
        ProgressionOrder::Lrcp,
        ProgressionOrder::Rlcp,
        ProgressionOrder::Rpcl,
        ProgressionOrder::Pcrl,
        ProgressionOrder::Cprl,
    ];
    let mut state = 0xc0c0_2000_u32;
    let mut cases = 0;
    for order in orders {
        for bits in 0..8 {
            for layers in [1, 2, 255, 256] {
                let default = CodingStyle {
                    progression_order: order,
                    layers,
                    multiple_component_transform: bits & 1 != 0,
                    sop_markers: bits & 2 != 0,
                    eph_markers: bits & 4 != 0,
                    ..default_style()
                };
                for depth in [0, 1, 2, 16, 31, 32] {
                    for explicit in [false, true] {
                        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                        let x = (state % 9) as u8;
                        let y = ((state >> 8) % u32::from(9 - x)) as u8;
                        let mut parameters =
                            vec![depth, x, y, (state % 64) as u8, (state % 2) as u8];
                        if explicit {
                            parameters.push(0); // Zero exponents are legal only at resolution zero.
                            for resolution in 1..=depth {
                                let px = 1 + resolution % 15;
                                let py = 1 + (resolution * 7) % 15;
                                parameters.push(px + 16 * py);
                            }
                        }
                        // Component 3 is outside the optional MCT's components 0..2.
                        let bytes = payload(4, 3, &parameters, explicit);
                        let expected = expected_style(default, &parameters, explicit);
                        assert_eq!(
                            parse(&bytes, 4, default),
                            Ok((3, expected)),
                            "inheritance: bits={bits}, layers={layers}, parameters={parameters:?}"
                        );
                        // Component-local defaults may change without changing this COC result.
                        // COD-wide defaults remain fixed; no observable may legitimately differ.
                        let alternate = CodingStyle {
                            decomposition_levels: 0,
                            code_block_width: 4,
                            code_block_height: 8,
                            code_block_style: 63,
                            transform: WaveletTransform::Irreversible9x7,
                            precincts_defined: false,
                            precinct_exponents: [(15, 15); 33],
                            ..default
                        };
                        assert_eq!(parse(&bytes, 4, alternate), Ok((3, expected)));
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 1920);
}

#[test]
fn component_selector_and_ordinary_coc_admission() {
    eprintln!(
        "route: framed COC -> read_segment -> Parser::parse_coc; Csiz width boundary 256/257"
    );
    actual_raw_component_styles_preserve_source_defaults_selectors_and_first_errors();
    let default = default_style();
    let mut failures = Vec::new();
    let malformed = |kind| Err(DecodeError::MalformedInput { offset: 6, kind });
    let mut check = |label: &str,
                     bytes: &[u8],
                     components: u16,
                     expected: Result<(u16, CodingStyle), DecodeError>| {
        let actual = parse(bytes, components, default);
        if actual != expected {
            let summary = actual.map(|(component, style)| {
                (
                    component,
                    style.decomposition_levels,
                    style.precinct_exponents[..2].to_vec(),
                )
            });
            failures.push(format!(
                "{label}: Csiz={components}, payload={bytes:?}, expected={expected:?}, actual(component,depth,precincts[0..2])={summary:?}"
            ));
        }
    };
    let mut valid = 0;
    let mut invalid = 0;
    for components in [1, 2, 64, 255, 256, 257, 258, 513, 16384] {
        for component in [0, components / 2, components - 1] {
            for depth in 0..=32 {
                for explicit in [false, true] {
                    let mut parameters = vec![depth, 8, 0, depth % 64, depth % 2];
                    if explicit {
                        parameters.push(0xf0);
                        parameters
                            .extend((1..=depth).map(|r| (1 + r % 15) + 16 * (1 + (r * 7) % 15)));
                    }
                    let bytes = payload(components, component, &parameters, explicit);
                    // A.6.2 equation A-3, including the two-byte length field.
                    assert_eq!(
                        bytes.len() + 2,
                        (if components < 257 { 9 } else { 10 })
                            + if explicit { usize::from(depth) + 1 } else { 0 }
                    );
                    check(
                        "valid",
                        &bytes,
                        components,
                        Ok((component, expected_style(default, &parameters, explicit))),
                    );
                    valid += 1;
                    for length in 0..bytes.len() {
                        check(
                            "truncated",
                            &bytes[..length],
                            components,
                            malformed(MalformedKind::InvalidMarkerLength),
                        );
                        invalid += 1;
                    }
                    let mut extra = bytes.clone();
                    extra.push(0x11);
                    check(
                        "extra byte",
                        &extra,
                        components,
                        malformed(MalformedKind::InvalidMarkerLength),
                    );
                    invalid += 1;
                }
            }
        }
        let parameters = [0, 0, 8, 0, 1];
        // Csiz=256 has no representable out-of-range one-byte component.
        if components != 256 {
            for component in [components, if components < 256 { 255 } else { 65535 }] {
                let bytes = payload(components, component, &parameters, false);
                check(
                    "nonexistent component",
                    &bytes,
                    components,
                    malformed(MalformedKind::InvalidCodingStyle),
                );
                invalid += 1;
            }
        }
        let width = if components < 257 { 1 } else { 2 };
        for flags in 2..=255 {
            let mut bytes = payload(components, 0, &parameters, false);
            bytes[width] = flags;
            check(
                "reserved Scoc",
                &bytes,
                components,
                malformed(MalformedKind::InvalidCodingStyle),
            );
            invalid += 1;
        }
        for (x, y) in [(9, 0), (0, 9), (255, 0), (0, 255), (8, 1), (1, 8), (5, 4)] {
            let bytes = payload(components, 0, &[0, x, y, 0, 1], false);
            check(
                "invalid code-block dimensions",
                &bytes,
                components,
                malformed(MalformedKind::InvalidCodingStyle),
            );
            invalid += 1;
        }
        let bytes = payload(components, 0, &[33, 0, 0, 0, 1], false);
        check(
            "depth exceeds configured Part 1 maximum",
            &bytes,
            components,
            Err(DecodeError::LimitExceeded {
                resource: Resource::Resolutions,
                limit: 33,
                actual: 34,
            }),
        );
        invalid += 1;
        // A.6.1 Table A.21: neither exponent may be zero above NLLL.
        for precinct in [0x00, 0x01, 0x10, 0x0f, 0xf0] {
            let bytes = payload(components, 0, &[1, 0, 0, 0, 1, 0x00, precinct], true);
            check(
                "zero precinct exponent above resolution zero",
                &bytes,
                components,
                malformed(MalformedKind::InvalidCodingStyle),
            );
            invalid += 1;
        }
    }
    assert_eq!(valid, 1782);
    assert!(invalid > 20000, "boundary sweep unexpectedly omitted cases");
    // Source-grounded, within-policy path: one ordinary main-header COC,
    // Rsiz=0, no CAP, a valid COD/QCD, and no tile-header overrides.
    let cod = segment(COD, &[0, 0, 0, 1, 0, 1, 0, 0, 0, 1]);
    let qcd = segment(QCD, &[0, 0, 0, 0, 0]);
    let coc = segment(COC, &[0, 1, 1, 0, 0, 0, 1, 0x11, 0xf0]);
    let bytes = codestream_with_headers(1, &[cod, qcd, coc], &[], &[0, 0]);
    let limits = ResourceLimits::HARDENED;
    let mut reader = new_parser(&bytes, &limits).unwrap();
    reader.expect_standalone(SOC).unwrap();
    let siz = reader.read_segment().unwrap();
    assert_eq!(siz.marker, SIZ);
    let (size, rsiz) = reader.parse_siz(siz).unwrap();
    assert_eq!(size.components, 1);
    assert_eq!(rsiz, 0);
    assert_eq!(reader.read_segment().unwrap().marker, COD);
    assert_eq!(reader.read_segment().unwrap().marker, QCD);
    let coc = reader.read_segment().unwrap();
    assert_eq!(coc.marker, COC);
    assert_eq!(reader.peek_marker().unwrap(), SOT);
    let expected_error = DecodeError::MalformedInput {
        offset: coc.offset as u64,
        kind: MalformedKind::InvalidCodingStyle,
    };
    match parse_layout(&bytes, &limits) {
        Err(error) if error == expected_error => {}
        Err(error) => failures.push(format!(
            "main-header parse_layout: expected={expected_error:?}, actual={error:?}"
        )),
        Ok(layout) => failures.push(format!(
            "main-header parse_layout accepted illegal high-resolution precinct: {:?}",
            layout
                .info
                .component_coding_styles
                .style(0)
                .unwrap()
                .precinct_exponents[..2]
                .to_vec()
        )),
    }
    assert!(
        failures.is_empty(),
        "{} COC contract mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
