// Included in the existing JPIP tests to reuse their real byte-stream fixtures.
fn precinct_contract_marker(bytes: &[u8], marker: u16) -> usize {
    bytes
        .windows(2)
        .position(|pair| pair == marker.to_be_bytes())
        .unwrap()
}

fn precinct_contract_stream(
    layers: u16,
    main_order: u8,
    main_headers: &[Vec<u8>],
    tile_headers: &[Vec<u8>],
    payload: &[u8],
) -> Vec<u8> {
    let base = decodable_codestream_with_layers(layers);
    let sot = precinct_contract_marker(&base, 0xff90);
    let mut bytes = base[..sot].to_vec();
    let cod = precinct_contract_marker(&bytes, 0xff52);
    bytes[cod + 5] = main_order;
    for header in main_headers {
        bytes.extend(header);
    }
    let start = bytes.len();
    bytes.extend(segment(0xff90, &[0, 0, 0, 0, 0, 0, 0, 1]));
    for header in tile_headers {
        bytes.extend(header);
    }
    bytes.extend_from_slice(&[0xff, 0x93]);
    bytes.extend_from_slice(payload);
    let length = u32::try_from(bytes.len() - start).unwrap();
    bytes[start + 6..start + 10].copy_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(&EOC);
    bytes
}

fn precinct_contract_two_parts() -> Vec<u8> {
    let base = decodable_codestream_with_layers(2);
    let sot = precinct_contract_marker(&base, 0xff90);
    let mut bytes = base[..sot].to_vec();
    for part in 0..2 {
        bytes.extend(segment(0xff90, &[0, 0, 0, 0, 0, 15, part, 2]));
        bytes.extend_from_slice(&[0xff, 0x93, 0]);
    }
    bytes.extend_from_slice(&EOC);
    bytes
}

fn precinct_contract_invalid() -> Result<(), DecodeError> {
    Err(DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidJpipLayout,
    })
}

#[test]
fn precinct_contract_structural_eligibility() {
    use crate::codestream::{ProgressionOrder, parse_layout};
    let limits = ResourceLimits::default();
    let mut failures = Vec::new();
    let mut check = |label: &str, bytes: &[u8], layers, limits: &ResourceLimits, eligible| {
        // Parsing is a fixture premise, not the eligibility oracle.
        let layout = parse_layout(bytes, limits)
            .unwrap_or_else(|error| panic!("bad fixture {label}: {error:?}"));
        layout.info.image_size.tile_count().unwrap();
        let expected = if eligible {
            Ok(())
        } else {
            precinct_contract_invalid()
        };
        let actual = validate_precinct_codestream(bytes, layers, limits);
        eprintln!(
            "direct validate_precinct_codestream: {label}, bytes={}, requested={layers}, expected={expected:?}, actual={actual:?}",
            bytes.len()
        );
        if actual != expected {
            failures.push(format!("{label}: expected {expected:?}, got {actual:?}"));
        }
    };

    // Existing caller-path regression checks the accepted real transcode route.
    transcodes_precinct_ranges_into_a_decodable_codestream();
    let extended = limits.with_coding_style_limits_for_test(u16::MAX, 33);
    let mut layer_counts = vec![1, 2, 255, 256, 257, u16::MAX];
    let mut seed = 0x71a4_09c3_u32;
    for _ in 0..32 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        layer_counts.push((seed % 256 + 1) as u16);
    }
    for layers in layer_counts {
        let bytes = decodable_codestream_with_layers(layers);
        check("matching layer count", &bytes, layers, &extended, true);
        for requested in [0, layers - 1, layers.saturating_add(1), u16::MAX] {
            check(
                "layer equality boundary",
                &bytes,
                requested,
                &extended,
                requested == layers,
            );
        }
    }

    let orders = [
        ProgressionOrder::Lrcp,
        ProgressionOrder::Rlcp,
        ProgressionOrder::Rpcl,
        ProgressionOrder::Pcrl,
        ProgressionOrder::Cprl,
    ];
    for main in 0..5_u8 {
        for tile in 0..5_u8 {
            let override_cod = segment(0xff52, &[0, tile, 0, 1, 0, 0, 0, 0, 0, 1]);
            let bytes = precinct_contract_stream(1, main, &[], &[override_cod], &[0]);
            let layout = parse_layout(&bytes, &limits).unwrap();
            assert_eq!(
                layout.info.coding_style.progression_order,
                orders[usize::from(main)]
            );
            assert_eq!(
                layout.tile_parts[0].coding_style.progression_order,
                orders[usize::from(tile)]
            );
            check(
                &format!("main progression={main}, effective tile progression={tile}"),
                &bytes,
                1,
                &limits,
                main == 0 && tile == 0,
            );
        }
    }
    let bytes = two_tile_codestream();
    let layout = parse_layout(&bytes, &limits).unwrap();
    assert_eq!(layout.info.image_size.tile_count(), Ok(2));
    check("two tiles", &bytes, 1, &limits, false);
    let bytes = precinct_contract_two_parts();
    let layout = parse_layout(&bytes, &limits).unwrap();
    assert_eq!(layout.info.image_size.tile_count(), Ok(1));
    assert_eq!(layout.tile_parts.len(), 2);
    check("two parts of one tile", &bytes, 2, &limits, false);

    let poc = segment(0xff5f, &[0, 0, 0, 1, 1, 1, 0]);
    for main_scope in [true, false] {
        let (main, tile) = if main_scope {
            (vec![poc.clone()], vec![])
        } else {
            (vec![], vec![poc.clone()])
        };
        let bytes = precinct_contract_stream(1, 0, &main, &tile, &[0]);
        let layout = parse_layout(&bytes, &limits).unwrap();
        assert_eq!(layout.info.progression_changes.is_present(), main_scope);
        assert!(layout.tile_parts[0].progression_changes.is_present());
        check(
            if main_scope { "main POC" } else { "tile POC" },
            &bytes,
            1,
            &limits,
            false,
        );
    }
    for empty in [true, false] {
        for marker in [0xff57, 0xff58, 0xff60, 0xff61] {
            let payload = match (marker, empty) {
                (0xff57, true) => vec![0, 0, 0, 0, 0],
                (0xff57, false) => vec![0, 0, 0, 0, 1, 1],
                (0xff58, true) | (0xff61, true) => vec![0],
                (0xff58, false) | (0xff61, false) => vec![0, 1],
                (0xff60, true) => vec![0, 0, 0, 0, 0],
                (0xff60, false) => vec![0, 0, 0, 0, 1, 1],
                _ => unreachable!(),
            };
            let header = segment(marker, &payload);
            let (main, tile) = if marker == 0xff57 || marker == 0xff60 {
                (vec![header], vec![])
            } else {
                (vec![], vec![header])
            };
            let data: &[u8] = if empty { &[] } else { &[0] };
            let bytes = precinct_contract_stream(1, 0, &main, &tile, data);
            let layout = parse_layout(&bytes, &limits).unwrap();
            let part = &layout.tile_parts[0];
            if marker == 0xff57 || marker == 0xff58 {
                assert!(part.packet_lengths.is_present());
                assert_eq!(part.packet_lengths.packet_count(), u64::from(!empty));
            } else {
                assert!(part.packed_packet_headers.is_present());
                assert_eq!(part.packed_packet_headers.len(), usize::from(!empty));
            }
            check(
                &format!("attached marker={marker:#x}, empty={empty}"),
                &bytes,
                1,
                &limits,
                false,
            );
        }
    }
    // Arbitrary entropy bytes are permitted by this structural gate.
    for data in [&[][..], &[0][..], &[0x7f, 0x81, 0][..]] {
        let bytes = precinct_contract_stream(1, 0, &[], &[], data);
        check(
            "structural, not entropy validation",
            &bytes,
            1,
            &limits,
            true,
        );
    }
    assert!(
        failures.is_empty(),
        "eligibility failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn precinct_contract_error_propagation_and_frame() {
    use crate::codestream::parse_layout;
    let limits = ResourceLimits::default();
    let valid = decodable_codestream();
    let mut seen_nonzero_offset = false;
    let mut seen_limit = false;
    let mut check = |label: &str,
                     bytes: &[u8],
                     layers,
                     limits: &ResourceLimits,
                     expected: Result<(), DecodeError>| {
        let before = bytes.to_vec();
        let before_limits = *limits;
        let result = validate_precinct_codestream(bytes, layers, limits);
        eprintln!(
            "direct validate_precinct_codestream: {label}, bytes={}, layers={layers}, expected={expected:?}, actual={result:?}",
            bytes.len()
        );
        assert_eq!(result, expected, "{label}");
        assert_eq!(bytes, before, "input changed: {label}");
        assert_eq!(*limits, before_limits, "limits changed: {label}");
        if let Err(DecodeError::MalformedInput { offset, .. }) = result {
            seen_nonzero_offset |= offset != 0;
        }
        seen_limit |= matches!(result, Err(DecodeError::LimitExceeded { .. }));
    };
    // Paired calls must preserve all parser error fields, even when requested
    // layers differ; structural eligibility must not replace a parse failure.
    for length in 0..valid.len() {
        let bytes = &valid[..length];
        let error = parse_layout(bytes, &limits).expect_err("truncated stream parsed");
        for layers in [0, 1, u16::MAX] {
            check("every truncated prefix", bytes, layers, &limits, Err(error));
        }
    }
    let mut malformed = valid.clone();
    let cod = precinct_contract_marker(&malformed, 0xff52);
    malformed[cod + 5] = 5;
    let mut trailing = valid.clone();
    trailing.push(0);
    for bytes in [&malformed, &trailing] {
        let error = parse_layout(bytes, &limits).expect_err("malformed fixture parsed");
        check("malformed parser diagnostic", bytes, 0, &limits, Err(error));
    }
    for tightened in [
        limits.with_maximum_tile_parts(0).unwrap(),
        limits.with_maximum_work_units(0).unwrap(),
        limits.with_coding_style_limits_for_test(0, 33),
    ] {
        let error = parse_layout(&valid, &tightened).expect_err("resource fixture parsed");
        assert!(matches!(error, DecodeError::LimitExceeded { .. }));
        check("parser resource error", &valid, 0, &tightened, Err(error));
    }
    for bytes in [&valid[..], &malformed[..], &[][..], &[0, 1, 2][..]] {
        let n = bytes.len() as u64;
        let mut boundaries = vec![0, n, n + 1];
        if n > 0 {
            boundaries.push(n - 1);
        }
        for m in boundaries {
            let tightened = limits.with_maximum_input_bytes(m).unwrap();
            for layers in [0, 1, u16::MAX] {
                let expected = if n > m {
                    Err(DecodeError::LimitExceeded {
                        resource: crate::Resource::InputBytes,
                        limit: m,
                        actual: n,
                    })
                } else if bytes == valid && layers == 1 {
                    Ok(())
                } else if bytes == valid {
                    precinct_contract_invalid()
                } else {
                    Err(parse_layout(bytes, &tightened).expect_err("malformed fixture parsed"))
                };
                check(
                    "input byte admission boundary",
                    bytes,
                    layers,
                    &tightened,
                    expected,
                );
            }
        }
    }
    check("successful frame", &valid, 1, &limits, Ok(()));
    check(
        "eligibility rejection frame",
        &valid,
        0,
        &limits,
        precinct_contract_invalid(),
    );
    assert!(
        seen_nonzero_offset,
        "error-offset preservation was not exercised"
    );
    assert!(seen_limit, "resource errors were not exercised");
}

#[test]
fn precinct_contract_ownership_and_cleanup() {
    use precinct_contract_alloc::measure;
    // Positive and negative controls ensure retained allocations are visible.
    let (control, counts) = measure(|| vec![17_u8; 1024]);
    assert!(counts.allocations > 0);
    assert_eq!(counts.live_blocks, 1);
    assert_eq!(counts.live_bytes, 1024);
    drop(control);
    let (_, counts) = measure(|| drop(vec![17_u8; 1024]));
    assert_eq!((counts.live_blocks, counts.live_bytes), (0, 0));

    let limits = ResourceLimits::default();
    let valid = decodable_codestream_with_layers(2);
    let truncated = valid[..valid.len() - 1].to_vec();
    let plt = precinct_contract_stream(1, 0, &[], &[segment(0xff58, &[0, 1])], &[0]);
    let invalid = precinct_contract_invalid();
    let parser_error = crate::codestream::parse_layout(&truncated, &limits).unwrap_err();
    let early_limits = limits.with_maximum_input_bytes(0).unwrap();
    let fixtures = [
        ("success", valid.clone(), 2, limits, Ok(()), true),
        ("post-parse rejection", valid, 1, limits, invalid, true),
        ("attached packet table", plt, 1, limits, invalid, true),
        (
            "late parser failure",
            truncated,
            2,
            limits,
            Err(parser_error),
            true,
        ),
        (
            "early parser failure",
            vec![0],
            1,
            early_limits,
            Err(DecodeError::LimitExceeded {
                resource: crate::Resource::InputBytes,
                limit: 0,
                actual: 1,
            }),
            false,
        ),
    ];
    for (label, input, layers, supplied_limits, expected, allocates) in fixtures {
        for iteration in 0..32 {
            let mut owned = input.clone();
            let snapshot = owned.clone();
            let pointer = owned.as_ptr();
            let capacity = owned.capacity();
            let before_limits = supplied_limits;
            eprintln!("ownership route: direct validator, {label}, iteration={iteration}");
            // Fixture construction, logging and assertions are outside the
            // measured interval. Layouts are created/dropped on this thread.
            let (result, counts) =
                measure(|| validate_precinct_codestream(&owned, layers, &supplied_limits));
            assert_eq!(result, expected, "{label}");
            assert_eq!(
                counts.live_blocks, 0,
                "retained allocation: {label}: {counts:?}"
            );
            assert_eq!(counts.live_bytes, 0, "retained bytes: {label}: {counts:?}");
            if allocates {
                assert!(counts.allocations > 0, "no allocation path: {label}");
            }
            assert_eq!(owned, snapshot, "input changed: {label}");
            assert_eq!(owned.as_ptr(), pointer);
            assert_eq!(owned.capacity(), capacity);
            assert_eq!(supplied_limits, before_limits);
            // The Result remains usable after the caller mutates and releases
            // its owners: no payload or parser cleanup is returned to it.
            owned.fill(0x5a);
            drop(owned);
            assert_eq!(result, expected);
        }
    }
}
