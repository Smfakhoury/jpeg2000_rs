// Included in codestream::tests, reusing native fixtures and exhaustive test.
#[test]
fn nonlinear_lookup_local_contract() {
    actual_nonlinear_lookup_preserves_first_specific_then_first_default_and_all_raw_fields();
    let owner = Part2Extensions {
        nonlinear_transforms: vec![
            nonlinear_contract_descriptor(None, 8),
            nonlinear_contract_descriptor(Some(0), 9),
            nonlinear_contract_descriptor(Some(u16::MAX), 10),
            nonlinear_contract_descriptor(Some(0), 11),
        ],
        metadata_bytes: 97,
        ..Part2Extensions::default()
    };
    let before = owner.clone();
    for component in [0, 1, 2, 255, 256, 16383, 16384, u16::MAX - 1, u16::MAX] {
        let expected_index = match component { 0 => 1, u16::MAX => 2, _ => 0 };
        assert!(core::ptr::eq(owner.nonlinear_transform_for(component).unwrap(), &owner.nonlinear_transforms[expected_index]));
        assert_eq!(owner, before, "owner/payload mutation for component {component}");
    }
}
fn nonlinear_contract_descriptor(component: Option<u16>, precision: u8) -> NonlinearPointTransform {
    NonlinearPointTransform {
        component, precision, signed: false, transform_type: 2,
        parameters: vec![0, 1, 0, 0, 0, 0, 255, 255, 255, 255, 8, 0, 255],
        d_min: Some(0), d_max: Some(u32::MAX), point_bits: Some(8),
        points: vec![0, 255], gamma_parameters: None,
    }
}
fn nonlinear_contract_header(mask: u8, tile: bool) -> Vec<(Option<u16>, u8)> {
    let mut entries = Vec::new();
    for (bit, selector) in [None, Some(0), Some(2)].into_iter().enumerate() {
        if mask & (1 << bit) != 0 {
            entries.push((selector, 8 + u8::from(tile) * 2 + u8::from(selector.is_some())));
        }
    }
    if mask & 1 != 0 { entries.reverse(); }
    entries
}
fn nonlinear_contract_markers(entries: &[(Option<u16>, u8)]) -> Vec<Vec<u8>> {
    let mut seen = Vec::new();
    entries.iter().map(|&(selector, precision)| {
        assert!(!seen.contains(&selector), "A.3.10: unique selector per header");
        seen.push(selector);
        assert!(selector.is_none_or(|component| component < 3));
        assert!((1..=38).contains(&precision));
        let mut payload = selector.unwrap_or(u16::MAX).to_be_bytes().to_vec();
        payload.extend_from_slice(&[precision - 1, 2]);
        payload.extend_from_slice(&[0, 1]); // Two LUT points.
        payload.extend_from_slice(&0_u32.to_be_bytes());
        payload.extend_from_slice(&u32::MAX.to_be_bytes());
        payload.extend_from_slice(&[8, 0, 255]);
        let marker = segment(NLT, &payload);
        assert_eq!(&marker[..4], &[0xff, 0x76, 0, 19]);
        assert_eq!(marker.len(), 21);
        marker
    }).collect()
}
#[test]
fn nonlinear_lookup_standard_header_precedence() {
    // T.801 (06/2021) A.2.1: Part 2 MSB plus NLT capability bit 9.
    // A.3.10: tile-specific > tile-default > main-specific > main-default.
    // Legal LUTs avoid the conflicting minimum-length table for Tnlt=0.
    // Only header selection is tested, not numerical LUT reconstruction.
    let mut failures = Vec::new();
    let mut observations = 0;
    for main_mask in 0..8 {
        for tile_mask in 0..8 {
            let main = nonlinear_contract_header(main_mask, false);
            let tile = nonlinear_contract_header(tile_mask, true);
            let [cod, qcd] = baseline_headers();
            let mut main_markers = vec![cod, qcd];
            main_markers.extend(nonlinear_contract_markers(&main));
            let tile_markers = nonlinear_contract_markers(&tile);
            // One layer, resolution and precinct: three empty component packets.
            let mut bytes = codestream_with_headers(3, &main_markers, &tile_markers, &[0, 0, 0]);
            assert_eq!(&bytes[..6], &[0xff, 0x4f, 0xff, 0x51, 0, 47]);
            assert_eq!(&bytes[6..8], &[0, 0]);
            bytes[6..8].copy_from_slice(&0x8200_u16.to_be_bytes());
            let rsiz = u16::from_be_bytes([bytes[6], bytes[7]]);
            assert_eq!(rsiz & 0xc000, 0x8000, "Part 2, no CAP extension");
            assert_eq!(rsiz & 0x0200, 0x0200, "NLT capability required by A.3.10");
            assert_eq!(rsiz & !0x8200, 0, "no unrelated capabilities");
            let limits = ResourceLimits::default();
            let layout = parse_layout(&bytes, &limits).expect("bounded, capability-signalled NLT header fixture must parse");
            assert_eq!(layout.info.image_size.components, 3);
            assert_eq!(layout.tile_parts.len(), 1);
            assert_eq!(layout.tile_parts[0].tile_index, 0);
            assert_eq!(layout.tile_parts[0].tile_part_index, 0);
            assert_eq!(layout.tile_parts[0].tile_part_count, 1);
            let owner = &layout.tile_parts[0].part2_extensions;
            let before = owner.clone();
            for component in 0..3 {
                let expected_entry = tile.iter().find(|(s, _)| *s == Some(component))
                    .or_else(|| tile.iter().find(|(s, _)| s.is_none()))
                    .or_else(|| main.iter().find(|(s, _)| *s == Some(component)))
                    .or_else(|| main.iter().find(|(s, _)| s.is_none()));
                let expected = expected_entry.map(|&(s, p)| nonlinear_contract_descriptor(s, p));
                let actual = owner.nonlinear_transform_for(component);
                if actual != expected.as_ref() {
                    failures.push(format!("parse_layout -> first tile extensions -> nonlinear_transform_for: Rsiz={rsiz:#06x}, main={main:?}, tile={tile:?}, component={component}, expected={expected:?}, actual={actual:?}"));
                }
                assert_eq!(*owner, before, "selection must not change owner/payloads");
                observations += 1;
            }
        }
    }
    assert_eq!(observations, 192);
    assert!(failures.is_empty(), "NLT header precedence violations: {failures:#?}");
}
