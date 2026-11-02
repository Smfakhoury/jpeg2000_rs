// Included in decode::tests; the existing wide reference uses raw CRG units.
// T.800 (06/2019) A.9.1 instead defines offsets in component-separation units.
fn crg_context_fixture(components: &[[u16; 4]]) -> Vec<u8> {
    assert!((1..=3).contains(&components.len()));
    assert!(
        components
            .iter()
            .all(|c| (1..=255).contains(&c[0]) && (1..=255).contains(&c[1]))
    );
    let mut siz = 0_u16.to_be_bytes().to_vec(); // Part 1, no extension capability.
    for value in [1024_u32, 1024, 0, 0, 1024, 1024, 0, 0] {
        siz.extend_from_slice(&value.to_be_bytes());
    }
    siz.extend_from_slice(&(components.len() as u16).to_be_bytes());
    for c in components {
        siz.extend_from_slice(&[7, c[0] as u8, c[1] as u8]);
    }
    let mut bytes = 0xff4f_u16.to_be_bytes().to_vec();
    bytes.extend(segment(0xff51, &siz));
    // One layer, no MCT, no decomposition, reversible transform, no quantization.
    bytes.extend(segment(0xff52, &[0, 0, 0, 1, 0, 0, 0, 0, 0, 1]));
    bytes.extend(segment(0xff5c, &[0x40, 0x40]));
    let mut registrations = Vec::new();
    for c in components {
        registrations.extend_from_slice(&c[2].to_be_bytes());
        registrations.extend_from_slice(&c[3].to_be_bytes());
    }
    let crg = segment(0xff63, &registrations);
    assert_eq!(crg.len(), 4 + 4 * components.len());
    assert_eq!(
        u16::from_be_bytes([crg[2], crg[3]]) as usize,
        2 + 4 * components.len()
    );
    assert!((6..=65_534).contains(&(crg.len() - 2)));
    // Exactly one CRG, in the main header, with one pair per SIZ component.
    bytes.extend(crg);
    let tile_start = bytes.len();
    let mut sot = 0_u16.to_be_bytes().to_vec();
    sot.extend_from_slice(&(14_u32 + components.len() as u32).to_be_bytes());
    sot.extend_from_slice(&[0, 1]);
    bytes.extend(segment(0xff90, &sot));
    bytes.extend_from_slice(&0xff93_u16.to_be_bytes());
    bytes.extend(vec![0; components.len()]); // One empty packet per component.
    assert_eq!(bytes.len() - tile_start, 14 + components.len());
    bytes.extend_from_slice(&0xffd9_u16.to_be_bytes());
    bytes
}

fn crg_standard_sample(coordinate: u32, separation: u16, registration: u16) -> u64 {
    let separation = u64::from(separation);
    let offset = separation * u64::from(registration);
    let position = u64::from(coordinate) * 65_536;
    // At native resolution and origin zero, sample i has physical centre
    // separation * (i + registration/65536). Use the project's predecessor
    // sampling policy, with edge extension before the first sample.
    position.saturating_sub(offset) / (separation * 65_536)
}

#[test]
fn selected_color_context_uses_crg_component_separation() {
    // At physical centre x=10, coded sample 2 (coded-grid coordinate 8)
    // must be retained. At x=9 or x=13, the preceding sample needs context
    // farther left than merely subtracting one separation from the ROI.
    let mut cases = (9..=14)
        .map(|x| (vec![[4, 4, 32_768, 32_768]], [x, x, x + 1, x + 1]))
        .collect::<Vec<_>>();
    for (index, sx) in [1_u16, 2, 3, 4, 8, 16, 128, 255].into_iter().enumerate() {
        let sy = [255_u16, 128, 16, 8, 4, 3, 2, 1][index];
        for rx in [0_u16, 1, 16_384, 32_767, 32_768, 32_769, 49_152, 65_535] {
            let ry = if rx == 0 { 0 } else { 65_535 - rx };
            let mut positions = vec![
                0,
                1,
                u32::from(sx) - 1,
                u32::from(sx),
                u32::from(sx) + 1,
                2 * u32::from(sx),
                1023,
            ];
            // Exact physical alignment and its two neighbors are discriminating
            // when registration * separation is an integer reference-grid offset.
            if u32::from(rx) * u32::from(sx) % 65_536 == 0 {
                let aligned = 2 * u32::from(sx) + u32::from(rx) * u32::from(sx) / 65_536;
                positions.extend([aligned - 1, aligned, aligned + 1]);
            }
            for x in positions {
                for y in [0, 1, u32::from(sy), 2 * u32::from(sy) + 1, 1023] {
                    for components in [
                        vec![[sx, sy, rx, ry]],
                        vec![[1, 1, 0, 0], [sx, sy, rx, ry], [3, 5, 0, 0]],
                    ] {
                        cases.push((components, [x, y, (x + 7).min(1024), (y + 3).min(1024)]));
                    }
                }
            }
            // Vary the vertical registration independently, including alignment.
            let aligned_y = 2 * u32::from(sy) + u32::from(rx) * u32::from(sy) / 65_536;
            for y in [aligned_y - 1, aligned_y, aligned_y + 1] {
                cases.push((vec![[sx, sy, rx, rx]], [513, y, 520, y + 1]));
            }
        }
    }
    let mut seed = 0x41c6_4e6d_u32;
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed
    };
    for _ in 0..128 {
        let c = [
            (next() % 255 + 1) as u16,
            (next() % 255 + 1) as u16,
            next() as u16,
            next() as u16,
        ];
        let x = next() % 1024;
        let y = next() % 1024;
        cases.push((vec![c], [x, y, x + 1, y + 1]));
    }
    cases.push((vec![[4, 8, 32_768, 32_768]], [0, 0, 1024, 1024]));
    let mut failures = 0;
    let mut examples = Vec::new();
    let total = cases.len();
    for (components, region) in cases {
        let bytes = crg_context_fixture(&components);
        let layout = parse_layout(&bytes, &ResourceLimits::HARDENED)
            .expect("standards-framed main-header CRG fixture must parse");
        let size = &layout.info.image_size;
        assert_eq!((size.x_origin, size.y_origin), (0, 0));
        assert_eq!((size.width, size.height), (1024, 1024));
        assert_eq!(usize::from(size.components), components.len());
        for (index, c) in components.iter().enumerate() {
            let info = codestream::image_component_info(size, index as u16).unwrap();
            assert_eq!(
                [
                    u16::from(info.x_subsampling),
                    u16::from(info.y_subsampling),
                    info.x_registration,
                    info.y_registration
                ],
                *c,
                "CRG parser must preserve the marker's units without conversion"
            );
        }
        assert!(region[0] < region[2] && region[2] <= 1024);
        assert!(region[1] < region[3] && region[3] <= 1024);
        let requested = DecodeSelection::full_image().with_region(crate::ImageRegion::new(
            region[0],
            region[1],
            region[2] - region[0],
            region[3] - region[1],
        ));
        let validated = validate_selection(size, &requested)
            .expect("nonempty in-image ROI on the sole tile is valid");
        assert_eq!(validated.region, region);
        assert_eq!(validated.tiles, vec![0]);
        assert_eq!(validated.discard_levels, 0);
        let (expanded, target) = color_selection_from_validated(size, validated)
            .expect("native-resolution context preparation must succeed");
        assert_eq!(
            target, region,
            "native-resolution crop coordinates must be preserved"
        );
        assert_eq!(expanded.discard_levels(), 0);
        assert!(matches!(expanded.tile_selection(), TileSelection::All));
        let context = expanded.region().expect("context must be an explicit ROI");
        assert!(context.x <= region[0] && context.y <= region[1]);
        assert_eq!(context.x + context.width, region[2]);
        assert_eq!(context.y + context.height, region[3]);
        for (index, c) in components.iter().enumerate() {
            for axis in 0..2 {
                let separation = u64::from(c[axis]);
                let start = u64::from(if axis == 0 { context.x } else { context.y });
                // A.9.1: CRG has NO effect on decoding the codestream. The
                // selected coded plane uses SIZ ceil-div geometry, not CRG.
                let first_available = start.div_ceil(separation);
                let end_available = u64::from(region[axis + 2]).div_ceil(separation);
                let full_sample_count = 1024_u64.div_ceil(separation);
                for coordinate in region[axis]..region[axis + 2] {
                    let required = crg_standard_sample(coordinate, c[axis], c[axis + 2])
                        .min(full_sample_count - 1);
                    if required < first_available || required >= end_available {
                        failures += 1;
                        if examples.len() < 12 {
                            examples.push(format!(
                                "parse_layout(CRG)->validate_selection->color_selection_from_validated: \
                                 components={components:?} ROI={region:?} context={context:?}; \
                                 component={index} axis={axis} coordinate={coordinate}: \
                                 physical sample {required} missing from coded range \
                                 {first_available}..{end_available}"
                            ));
                        }
                    }
                }
            }
        }
    }
    eprintln!("CRG physical-offset oracle: cases={total}, mismatches={failures}");
    assert_eq!(failures, 0, "{}", examples.join("\n"));
}
