// Byte fixtures enter the public decoder; no parsed-layout or reconstruction state is patched.
fn selected_contract_grid(levels: u8, components: &[[u8; 3]]) -> Vec<u8> {
    let mut siz = 0_u16.to_be_bytes().to_vec();
    for value in [32_u32, 16, 1, 1, 8, 8, 0, 0] {
        siz.extend_from_slice(&value.to_be_bytes());
    }
    siz.extend_from_slice(&u16::try_from(components.len()).unwrap().to_be_bytes());
    for component in components {
        siz.extend_from_slice(component);
    }
    let mut bytes = 0xff4f_u16.to_be_bytes().to_vec();
    bytes.extend(segment(0xff51, &siz));
    bytes.extend(segment(0xff52, &[0, 0, 0, 1, 0, levels, 2, 2, 0, 1]));
    bytes.extend(segment(0xff5c, &vec![0x40; 2 + 3 * usize::from(levels)]));
    for tile in 0_u16..8 {
        let packets = components.len() * (usize::from(levels) + 1);
        let mut sot = tile.to_be_bytes().to_vec();
        sot.extend_from_slice(&u32::try_from(14 + packets).unwrap().to_be_bytes());
        sot.extend_from_slice(&[0, 1]);
        bytes.extend(segment(0xff90, &sot));
        bytes.extend_from_slice(&[0xff, 0x93]);
        bytes.extend(vec![0; packets]);
    }
    bytes.extend_from_slice(&[0xff, 0xd9]);
    bytes
}

fn selected_contract_tile_header(bytes: &mut Vec<u8>, tile: u16, header: Vec<u8>) {
    let sot = bytes
        .windows(2)
        .enumerate()
        .find(|(i, marker)| *marker == [0xff, 0x90] && bytes[*i + 4..*i + 6] == tile.to_be_bytes())
        .map(|(i, _)| i)
        .unwrap();
    let old = u32::from_be_bytes(bytes[sot + 6..sot + 10].try_into().unwrap());
    let length = old + u32::try_from(header.len()).unwrap();
    bytes.splice(sot + 12..sot + 12, header);
    bytes[sot + 6..sot + 10].copy_from_slice(&length.to_be_bytes());
}

fn selected_contract_ceil(value: u32, divisor: u32) -> u32 {
    value / divisor + u32::from(value % divisor != 0)
}

fn selected_contract_gamma(slope: u8) -> Vec<u8> {
    let mut payload = vec![255, 255, 7, 1];
    for integer in [1_u8, slope, 1, 1, 0] {
        payload.extend_from_slice(&[integer, 0, 0]);
    }
    segment(0xff76, &payload)
}

fn selected_contract_patterned() -> Vec<u8> {
    let mut bytes = selected_contract_grid(1, &[[7, 1, 1]]);
    for (tile, slope) in [1_u8, 2, 4, 8, 2, 4, 8, 1].iter().enumerate() {
        selected_contract_tile_header(&mut bytes, tile as u16, selected_contract_gamma(*slope));
    }
    bytes
}

fn selected_contract_assert_image(
    image: &TypedImage,
    bounds: [u32; 4],
    discard: u8,
    components: &[[u8; 3]],
    unsigned_value: u64,
) {
    let scale = 1_u32 << discard;
    let [x, y, end_x, end_y] = bounds.map(|z| selected_contract_ceil(z, scale));
    assert_eq!(
        (
            image.x_origin(),
            image.y_origin(),
            image.width(),
            image.height()
        ),
        (x, y, end_x - x, end_y - y),
        "absolute reference-grid bounds={bounds:?}, discard={discard}"
    );
    assert!(image.width() > 0 && image.height() > 0);
    assert_eq!(image.components().len(), components.len());
    for (index, descriptor) in components.iter().enumerate() {
        let component = image.component(u16::try_from(index).unwrap()).unwrap();
        let [depth, dx, dy] = *descriptor;
        let width = selected_contract_ceil(bounds[2], u32::from(dx) * scale)
            - selected_contract_ceil(bounds[0], u32::from(dx) * scale);
        let height = selected_contract_ceil(bounds[3], u32::from(dy) * scale)
            - selected_contract_ceil(bounds[1], u32::from(dy) * scale);
        assert!(width > 0 && height > 0);
        assert_eq!((component.width(), component.height()), (width, height));
        let info = component.info();
        assert_eq!(info.precision, (depth & 127) + 1);
        assert_eq!(info.signed, depth & 128 != 0);
        assert_eq!((info.x_subsampling, info.y_subsampling), (dx, dy));
        assert_eq!((info.x_registration, info.y_registration), (0, 0));
        let count = usize::try_from(u64::from(width) * u64::from(height)).unwrap();
        let expected = if info.signed {
            TypedSamples::Signed(vec![0; count])
        } else {
            TypedSamples::Unsigned(vec![unsigned_value; count])
        };
        assert_eq!(component.samples(), &expected, "component {index}");
        assert!(component.channel_definitions().is_empty());
    }
}

#[test]
fn selected_contract_geometry_and_native_samples() {
    selected_tiles_and_regions_match_full_decode_crops();
    reduced_selection_uses_native_resolution_and_selected_limits();
    selected_decode_rejects_malformed_selection_deterministically();
    let options = DecodeOptions::default();
    let components = [[7, 1, 1]];
    let bytes = selected_contract_grid(2, &components);
    let snapshot = bytes.clone();
    for discard in [3, 31, 32, 255] {
        assert_eq!(
            decode_codestream_selected(
                &bytes,
                &options,
                &DecodeSelection::full_image().with_discard_levels(discard),
            ),
            Err(DecodeError::InvalidSelection {
                reason: SelectionError::ReductionTooLarge
            }),
        );
    }
    let mut seed = 0x51ec_7ed1_u32;
    for discard in 0..=2 {
        for (selection, bounds) in [
            (DecodeSelection::full_image(), [1, 1, 32, 16]),
            (DecodeSelection::tile(0), [1, 1, 8, 8]),
            (DecodeSelection::tile(7), [24, 8, 32, 16]),
            (DecodeSelection::tiles(vec![6, 1, 2, 5]), [8, 1, 24, 16]),
        ] {
            let selection = selection.with_discard_levels(discard);
            let before = selection.clone();
            let image = decode_codestream_selected(&bytes, &options, &selection).unwrap();
            selected_contract_assert_image(&image, bounds, discard, &components, 128);
            assert_eq!(selection, before);
        }
        for _ in 0..64 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let x = 1 + seed % 27;
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let y = 1 + seed % 11;
            let end_x = (x + 4 + seed % 12).min(32);
            let end_y = (y + 4 + seed % 6).min(16);
            let selection = DecodeSelection::full_image()
                .with_region(crate::ImageRegion::new(x, y, end_x - x, end_y - y))
                .with_discard_levels(discard);
            let image = decode_codestream_selected(&bytes, &options, &selection).unwrap();
            selected_contract_assert_image(&image, [x, y, end_x, end_y], discard, &components, 128);
        }
    }
    for discard in 0..=1 {
        let components = [[7, 1, 1], [0x87, 2, 2]];
        let bytes = selected_contract_grid(1, &components);
        for (selection, bounds) in [
            (DecodeSelection::full_image(), [1, 1, 32, 16]),
            (DecodeSelection::tile(7), [24, 8, 32, 16]),
        ] {
            let image = decode_codestream_selected(
                &bytes,
                &options,
                &selection.with_discard_levels(discard),
            )
            .unwrap();
            selected_contract_assert_image(&image, bounds, discard, &components, 128);
        }
    }
    // Linear branch of inverse gamma: slope 2 divides the typed value by 2.
    // At 128 this gives 64; omission gives 128, and double application gives 32.
    let mut nonlinear = bytes.clone();
    let sot = nonlinear.windows(2).position(|m| m == [255, 144]).unwrap();
    nonlinear.splice(sot..sot, selected_contract_gamma(2));
    for discard in 0..=2 {
        let selection = DecodeSelection::tiles(vec![5, 1])
            .with_region(crate::ImageRegion::new(9, 3, 6, 12))
            .with_discard_levels(discard);
        let image = decode_codestream_selected(&nonlinear, &options, &selection).unwrap();
        selected_contract_assert_image(&image, [9, 3, 15, 15], discard, &components, 64);
    }
    // Existing independently OpenJPEG-encoded packets, exercised through this entry.
    for (value, packet) in [
        (1, &[0xcf, 0xb4, 0x04, 0x09][..]),
        (129, &[0xc0, 0x21, 0x03][..]),
        (200, &[0xcf, 0xb4, 0x08, 0x04, 0x4f][..]),
    ] {
        let mut encoded = prepared_codestream(1, 1, 1, &[0x40, 0x40], packet);
        let cod = encoded.windows(2).position(|m| m == [255, 82]).unwrap();
        encoded[cod + 9] = 0;
        let image =
            decode_codestream_selected(&encoded, &options, &DecodeSelection::full_image()).unwrap();
        selected_contract_assert_image(&image, [0, 0, 1, 1], 0, &components, value);
    }
    assert_eq!(bytes, snapshot);
    assert_eq!(options, DecodeOptions::default());
    // Tile boundaries are multiples of eight, hence remain aligned at d=1.
    // Each tile's zero coefficients type to 128; inverse gamma gives 128/slope.
    let patterned = selected_contract_patterned();
    let slopes = [1_u8, 2, 4, 8, 2, 4, 8, 1];
    let mut mismatches = Vec::new();
    for discard in 0..=1 {
        for region in [None, Some(crate::ImageRegion::new(3, 3, 26, 12))] {
            let selection =
                crate::options::selection_from_parts(TileSelection::All, region, discard);
            let image = decode_codestream_selected(&patterned, &options, &selection).unwrap();
            let bounds = if region.is_some() {
                [3, 3, 29, 15]
            } else {
                [1, 1, 32, 16]
            };
            let scale = 1 << discard;
            let [x, y, ex, ey] = bounds.map(|z| selected_contract_ceil(z, scale));
            assert_eq!(
                (
                    image.x_origin(),
                    image.y_origin(),
                    image.width(),
                    image.height()
                ),
                (x, y, ex - x, ey - y),
            );
            let TypedSamples::Unsigned(samples) = image.component(0).unwrap().samples() else {
                panic!("unsigned SIZ component must remain unsigned");
            };
            assert_eq!(samples.len(), ((ex - x) * (ey - y)) as usize);
            let mut first_mismatch = None;
            for py in y..ey {
                for px in x..ex {
                    let tile = (py * scale / 8) * 4 + px * scale / 8;
                    let expected = 128 / u64::from(slopes[tile as usize]);
                    let actual = samples[((py - y) * (ex - x) + px - x) as usize];
                    if actual != expected && first_mismatch.is_none() {
                        first_mismatch = Some(format!(
                            "d={discard}, region={region:?}, native=({px},{py}), tile={tile}, expected={expected}, actual={actual}",
                        ));
                    }
                }
            }
            if let Some(mismatch) = first_mismatch {
                mismatches.push(mismatch);
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "native tile-specific nonlinear transform mismatch: {mismatches:?}"
    );
}

#[test]
fn selected_contract_cumulative_resource_boundaries() {
    reduced_selection_uses_native_resolution_and_selected_limits();
    let bytes = selected_contract_grid(1, &[[7, 1, 1]]);
    for discard in 0..=1 {
        // One precinct per retained resolution, one layer; LL plus three detail blocks.
        let per_tile_packets = if discard == 0 { 2 } else { 1 };
        let per_tile_blocks = if discard == 0 { 4 } else { 1 };
        for count in [2_u16, 3, 4, 8] {
            let selection =
                DecodeSelection::tiles((0..count).rev().collect()).with_discard_levels(discard);
            for resource in [Resource::Packets, Resource::Precincts, Resource::CodeBlocks] {
                // Planning a tile needs four full-resolution block descriptors even at d=1.
                // Keep that earlier admission below every tested cumulative threshold.
                if discard == 1 && resource == Resource::CodeBlocks && count < 8 {
                    continue;
                }
                let per_tile = if resource == Resource::CodeBlocks {
                    per_tile_blocks
                } else {
                    per_tile_packets
                };
                let total = u64::from(count) * per_tile;
                for limit in [total - 1, total, total + 1] {
                    let limits = match resource {
                        Resource::Packets => ResourceLimits::HARDENED
                            .with_maximum_packets(limit)
                            .unwrap(),
                        Resource::Precincts => ResourceLimits::HARDENED
                            .with_maximum_precincts_for_selected_contract_test(limit),
                        Resource::CodeBlocks => ResourceLimits::HARDENED
                            .with_maximum_code_blocks(limit)
                            .unwrap(),
                        _ => unreachable!(),
                    };
                    let options = DecodeOptions::new(limits);
                    eprintln!(
                        "cumulative route: {count} tiles d={discard} {resource:?} limit={limit} total={total}"
                    );
                    let result = decode_codestream_selected(&bytes, &options, &selection);
                    if limit < total {
                        assert_eq!(
                            result,
                            Err(DecodeError::LimitExceeded {
                                resource,
                                limit,
                                actual: total,
                            })
                        );
                    } else {
                        let image = result.unwrap();
                        assert!(!image.components().is_empty());
                    }
                    assert_eq!(*options.limits(), limits);
                }
            }
        }
        let scale = 1 << discard;
        let width = selected_contract_ceil(32, scale) - selected_contract_ceil(1, scale);
        let height = selected_contract_ceil(16, scale) - selected_contract_ceil(1, scale);
        let charge = 8 * u64::from(width) * u64::from(height);
        for limit in [charge - 1, charge, charge + 1] {
            let limits = ResourceLimits::HARDENED
                .with_maximum_decoded_bytes(limit)
                .unwrap();
            let options = DecodeOptions::new(limits);
            let result = decode_codestream_selected(
                &bytes,
                &options,
                &DecodeSelection::full_image().with_discard_levels(discard),
            );
            if limit < charge {
                assert_eq!(
                    result,
                    Err(DecodeError::LimitExceeded {
                        resource: Resource::DecodedBytes,
                        limit,
                        actual: charge,
                    })
                );
            } else {
                let image = result.unwrap();
                selected_contract_assert_image(&image, [1, 1, 32, 16], discard, &[[7, 1, 1]], 128);
                assert!(charge <= options.limits().maximum_decoded_bytes());
            }
            assert_eq!(*options.limits(), limits);
        }
    }
    let selection =
        DecodeSelection::tiles(vec![1, 0]).with_region(crate::ImageRegion::new(7, 2, 2, 2));
    let options = DecodeOptions::new(
        ResourceLimits::HARDENED
            .with_maximum_code_blocks(7)
            .unwrap(),
    );
    assert_eq!(
        decode_codestream_selected(&bytes, &options, &selection),
        Err(DecodeError::LimitExceeded {
            resource: Resource::CodeBlocks,
            limit: 7,
            actual: 8
        }),
        "crop pixels must not substitute for retained native-resolution code blocks",
    );
    for (packet_limit, first_resource) in [(7, Resource::Packets), (8, Resource::Precincts)] {
        let limits = ResourceLimits::HARDENED
            .with_maximum_packets(packet_limit)
            .unwrap()
            .with_maximum_precincts_for_selected_contract_test(7)
            .with_maximum_code_blocks(7)
            .unwrap();
        assert_eq!(
            decode_codestream_selected(
                &bytes,
                &DecodeOptions::new(limits),
                &DecodeSelection::full_image().with_discard_levels(1),
            ),
            Err(DecodeError::LimitExceeded {
                resource: first_resource,
                limit: 7,
                actual: 8
            }),
            "accounting order when the same prefix crosses several budgets",
        );
    }
    let bytes = selected_contract_grid(1, &[[7, 1, 1], [0x87, 2, 2]]);
    // At d=1 the two grids contain 15*7 and 7*3 samples, all charged at eight bytes.
    let charge = 8 * (15 * 7 + 7 * 3);
    for limit in [charge - 1, charge] {
        let options = DecodeOptions::new(
            ResourceLimits::HARDENED
                .with_maximum_decoded_bytes(limit)
                .unwrap(),
        );
        let result = decode_codestream_selected(
            &bytes,
            &options,
            &DecodeSelection::full_image().with_discard_levels(1),
        );
        if limit < charge {
            assert_eq!(
                result,
                Err(DecodeError::LimitExceeded {
                    resource: Resource::DecodedBytes,
                    limit,
                    actual: charge,
                })
            );
        } else {
            selected_contract_assert_image(
                &result.unwrap(),
                [1, 1, 32, 16],
                1,
                &[[7, 1, 1], [0x87, 2, 2]],
                128,
            );
        }
    }
}

#[test]
fn selected_contract_admission_and_first_error() {
    selected_decode_rejects_malformed_selection_deterministically();
    let valid = two_tile_codestream();
    let invalid_selection = DecodeSelection::tiles(vec![]);
    for bytes in [vec![], vec![0], vec![0, 0], vec![255, 79], valid.clone()] {
        let length = bytes.len() as u64;
        let before = bytes.clone();
        for limit in [length.saturating_sub(1), length, length + 1] {
            let options = DecodeOptions::new(
                ResourceLimits::HARDENED
                    .with_maximum_input_bytes(limit)
                    .unwrap(),
            );
            let options_before = options;
            let result = decode_codestream_selected(&bytes, &options, &invalid_selection);
            let expected = if length > limit {
                DecodeError::LimitExceeded {
                    resource: Resource::InputBytes,
                    limit,
                    actual: length,
                }
            } else if bytes == valid {
                DecodeError::InvalidSelection {
                    reason: SelectionError::EmptyTileSet,
                }
            } else {
                match bytes.len() {
                    0 => DecodeError::MalformedInput {
                        offset: 0,
                        kind: MalformedKind::Truncated,
                    },
                    1 => DecodeError::MalformedInput {
                        offset: 0,
                        kind: MalformedKind::Truncated,
                    },
                    _ if bytes == [0, 0] => DecodeError::MalformedInput {
                        offset: 0,
                        kind: MalformedKind::InvalidMarker,
                    },
                    _ => DecodeError::MalformedInput {
                        offset: 2,
                        kind: MalformedKind::Truncated,
                    },
                }
            };
            assert_eq!(result, Err(expected), "input={bytes:?}, limit={limit}");
            assert_eq!(bytes, before);
            assert_eq!(options, options_before);
            assert_eq!(invalid_selection, DecodeSelection::tiles(vec![]));
        }
    }
    // The later tile references an absent arbitrary kernel. Its payload is unselected.
    let mut metadata = valid.clone();
    selected_contract_tile_header(
        &mut metadata,
        1,
        segment(0xff52, &[0, 0, 0, 1, 0, 0, 0, 0, 0, 2]),
    );
    let layout = parse_layout(&metadata, &ResourceLimits::HARDENED).unwrap();
    assert_eq!(layout.tile_parts.len(), 2);
    assert_eq!(
        layout.tile_parts[1].coding_style.transform,
        WaveletTransform::Arbitrary(2)
    );
    for selection in [DecodeSelection::tile(0), invalid_selection] {
        assert_eq!(
            decode_codestream_selected(&metadata, &DecodeOptions::default(), &selection),
            Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::MissingRequiredStructure,
            }),
            "whole-layout execution admission must precede spatial selection"
        );
    }
    let n = valid.len() as u64;
    let options = DecodeOptions::new(
        ResourceLimits::HARDENED
            .with_maximum_input_bytes(n)
            .unwrap(),
    );
    let image =
        decode_codestream_selected(&valid, &options, &DecodeSelection::full_image()).unwrap();
    selected_contract_assert_image(&image, [1, 0, 6, 1], 0, &[[7, 1, 1]], 128);
}

#[test]
fn selected_contract_owned_result_survives_inputs_and_errors() {
    let (image, independent) = {
        let mut bytes = selected_contract_grid(1, &[[7, 1, 1], [0x87, 2, 2]]);
        let options = DecodeOptions::default();
        let selection = DecodeSelection::tiles(vec![6, 1, 2, 5]).with_discard_levels(1);
        let before = bytes.clone();
        let selection_before = selection.clone();
        let image = decode_codestream_selected(&bytes, &options, &selection).unwrap();
        let independent = decode_codestream_selected(&bytes, &options, &selection).unwrap();
        for bad in [
            DecodeSelection::tiles(vec![]),
            DecodeSelection::tile(8),
            DecodeSelection::full_image().with_discard_levels(255),
        ] {
            assert!(matches!(
                decode_codestream_selected(&bytes, &options, &bad),
                Err(DecodeError::InvalidSelection { .. })
            ));
            assert_eq!(bytes, before);
            assert_eq!(options, DecodeOptions::default());
            assert_eq!(selection, selection_before);
            assert_eq!(
                decode_codestream_selected(&bytes, &options, &selection).unwrap(),
                image
            );
        }
        let tight = DecodeOptions::new(ResourceLimits::HARDENED.with_maximum_packets(3).unwrap());
        assert_eq!(
            decode_codestream_selected(&bytes, &tight, &selection),
            Err(DecodeError::LimitExceeded {
                resource: Resource::Packets,
                limit: 3,
                actual: 4
            }),
        );
        assert_eq!(bytes, before);
        assert_eq!(
            decode_codestream_selected(&bytes, &options, &selection).unwrap(),
            image
        );
        // Borrow checking permits replacement before the image's last use.
        bytes.fill(0xa5);
        drop(bytes);
        drop(selection);
        (image, independent)
    };
    selected_contract_assert_image(&image, [8, 1, 24, 16], 1, &[[7, 1, 1], [0x87, 2, 2]], 128);
    assert_eq!(image, independent);
    drop(independent);
    selected_contract_assert_image(&image, [8, 1, 24, 16], 1, &[[7, 1, 1], [0x87, 2, 2]], 128);
    drop(image);
}

#[test]
fn selected_contract_tile_permutation_equal_results() {
    let bytes = selected_contract_patterned();
    let before = bytes.clone();
    let mut seed = 0x7135_5eed_u32;
    for discard in 0..=1 {
        for region in [None, Some(crate::ImageRegion::new(3, 3, 26, 12))] {
            for limits in [
                ResourceLimits::HARDENED,
                ResourceLimits::HARDENED.with_maximum_packets(5).unwrap(),
                ResourceLimits::HARDENED
                    .with_maximum_code_blocks(7)
                    .unwrap(),
                ResourceLimits::HARDENED
                    .with_maximum_decoded_bytes(1)
                    .unwrap(),
            ] {
                let options = DecodeOptions::new(limits);
                let canonical = crate::options::selection_from_parts(
                    TileSelection::Set((0..8).collect()),
                    region,
                    discard,
                );
                let baseline = decode_codestream_selected(&bytes, &options, &canonical);
                if limits == ResourceLimits::HARDENED {
                    baseline.as_ref().unwrap();
                } else {
                    assert!(
                        baseline.is_err(),
                        "error comparisons must actually reach an error"
                    );
                }
                // Equal regions/levels, all eight distinct tiles; only ordering changes.
                for _ in 0..32 {
                    let mut permutation: Vec<u16> = (0..8).collect();
                    for i in (1..8).rev() {
                        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                        permutation.swap(i, seed as usize % (i + 1));
                    }
                    let selection = crate::options::selection_from_parts(
                        TileSelection::Set(permutation),
                        region,
                        discard,
                    );
                    let saved = selection.clone();
                    let result = decode_codestream_selected(&bytes, &options, &selection);
                    assert_eq!(
                        result, baseline,
                        "permutation={selection:?}, policy={limits:?}"
                    );
                    assert_eq!(selection, saved);
                    assert_eq!(*options.limits(), limits);
                }
            }
        }
    }
    assert_eq!(bytes, before);
    // Distinct payload errors in different tiles make first-failure ordering observable.
    let mut damaged = selected_contract_grid(1, &[[7, 1, 1]]);
    let starts: Vec<usize> = damaged
        .windows(2)
        .enumerate()
        .filter_map(|(i, m)| (m == [255, 144]).then_some(i))
        .collect();
    damaged[starts[1] + 14..starts[1] + 16].copy_from_slice(&[255, 255]);
    damaged[starts[6] + 14..starts[6] + 16].copy_from_slice(&[255, 0]);
    parse_layout(&damaged, &ResourceLimits::HARDENED).unwrap();
    let options = DecodeOptions::default();
    let error1 =
        decode_codestream_selected(&damaged, &options, &DecodeSelection::tile(1)).unwrap_err();
    let error6 =
        decode_codestream_selected(&damaged, &options, &DecodeSelection::tile(6)).unwrap_err();
    assert_ne!(
        error1, error6,
        "fixture must expose distinct later-tile errors"
    );
    for permutation in [
        vec![0, 1, 2, 3, 4, 5, 6, 7],
        vec![7, 6, 5, 4, 3, 2, 1, 0],
        vec![6, 1, 7, 0, 2, 3, 4, 5],
    ] {
        let selection = DecodeSelection::tiles(permutation);
        assert_eq!(
            decode_codestream_selected(&damaged, &options, &selection),
            Err(error1)
        );
    }
}
