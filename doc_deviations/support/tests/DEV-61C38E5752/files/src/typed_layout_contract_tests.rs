use super::*;
use crate::codestream::parse_layout;

fn segment(marker: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = marker.to_be_bytes().to_vec();
    bytes.extend_from_slice(&u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

// T.801 (06/2021) A.2.1, A.3.10: Rsiz NLT capability, one component-specific
// main-header NLT, legal BDnlt/PTval, two points, Dmin=0 < Dmax=1.
// Packet decoding is deliberately outside this unit fixture: assemble_selected_image
// supplies the already reconstructed input that typed_layout's contract accepts.
fn headers(
    source_signed: bool,
    subsampling: u8,
    inverse_rct: bool,
    nlt: Option<(u8, bool, bool)>,
) -> Vec<u8> {
    let mut siz = 0x8200_u16.to_be_bytes().to_vec();
    for value in [35_u32, 9, 3, 1, 35, 9, 0, 0] {
        siz.extend_from_slice(&value.to_be_bytes());
    }
    siz.extend_from_slice(&3_u16.to_be_bytes());
    for _ in 0..3 {
        siz.extend_from_slice(&[3 | if source_signed { 0x80 } else { 0 }, subsampling, 1]);
    }
    let mut bytes = 0xff4f_u16.to_be_bytes().to_vec();
    bytes.extend(segment(0xff51, &siz));
    bytes.extend(segment(
        0xff52,
        &[0, 0, 0, 1, u8::from(inverse_rct), 0, 0, 0, 0, 1],
    ));
    bytes.extend(segment(0xff5c, &[0x40, 4 << 3]));
    if let Some((precision, signed, reversed)) = nlt {
        assert!((1..=38).contains(&precision));
        let mut payload = vec![0, 0, precision - 1 | if signed { 0x80 } else { 0 }, 2];
        payload.extend_from_slice(&1_u16.to_be_bytes());
        payload.extend_from_slice(&0_u32.to_be_bytes());
        payload.extend_from_slice(&u32::MAX.to_be_bytes());
        payload.push(if reversed { 8 } else { 1 });
        payload.extend_from_slice(if reversed { &[255, 0] } else { &[0, 0] });
        assert_eq!(payload.len() + 2, 19);
        bytes.extend(segment(0xff76, &payload));
    }
    bytes.extend(segment(0xff90, &[0, 0, 0, 0, 0, 17, 0, 1]));
    bytes.extend_from_slice(&[0xff, 0x93, 0, 0, 0, 0xff, 0xd9]);
    assert_eq!(u16::from_be_bytes([bytes[6], bytes[7]]) & 0x8200, 0x8200);
    bytes
}

fn assert_layout_premises(layout: &CodestreamLayout<'_>, nlt: Option<(u8, bool, bool)>) {
    assert_eq!(layout.info.image_size.components, 3);
    assert_eq!(layout.tile_parts.len(), 1);
    assert_eq!(layout.tile_parts[0].tile_index, 0);
    let transforms = layout.info.part2_extensions.nonlinear_transforms();
    assert_eq!(transforms.len(), usize::from(nlt.is_some()));
    assert_eq!(
        layout.tile_parts[0].part2_extensions,
        layout.info.part2_extensions
    );
    if let Some((precision, signed, reversed)) = nlt {
        let transform = &transforms[0];
        assert_eq!(transform.component(), Some(0));
        assert_eq!(transform.precision(), precision);
        assert_eq!(transform.signed(), signed);
        assert_eq!(transform.transform_type(), 2);
        assert_eq!(transform.d_min(), Some(0));
        assert_eq!(transform.d_max(), Some(u32::MAX));
        assert_eq!(transform.point_bits(), Some(if reversed { 8 } else { 1 }));
        assert_eq!(
            transform.points(),
            if reversed { &[255, 0] } else { &[0, 0] }
        );
        for component in [1, 2] {
            assert!(
                layout
                    .info
                    .part2_extensions
                    .nonlinear_transform_for(component)
                    .is_none()
            );
        }
    }
}

fn reconstruction(
    layout: &CodestreamLayout<'_>,
    region: [u32; 4],
    fraction_bits: u8,
    inverse_rct: bool,
) -> (DecodedImage, Vec<Vec<i64>>) {
    let size = &layout.info.image_size;
    assert_eq!(
        layout.info.coding_style.multiple_component_transform,
        inverse_rct
    );
    assert_eq!(
        layout.info.coding_style.transform,
        WaveletTransform::Reversible5x3
    );
    let sx = u32::from(size.component(0).unwrap().x_subsampling);
    let x0 = region[0].div_ceil(sx);
    let x1 = region[2].div_ceil(sx);
    let width = x1 - x0;
    let height = region[3] - region[1];
    let mut state = 0x9e37_79b9_u32;
    let ranks: Vec<i64> = (0..width * height)
        .map(|index| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            if index < 16 {
                i64::from(index)
            } else {
                i64::from(state % 16)
            }
        })
        .collect();
    let centered: Vec<i64> = ranks.iter().map(|rank| rank - 8).collect();
    let raw: Vec<Vec<i64>> = if inverse_rct {
        assert_eq!(fraction_bits, 0);
        vec![
            centered.iter().map(|value| value + 1).collect(),
            vec![1; ranks.len()],
            vec![-1; ranks.len()],
        ]
    } else {
        (0..3)
            .map(|component| {
                centered
                    .iter()
                    .map(|value| {
                        let shifted = if component == 0 { *value } else { -*value - 1 };
                        shifted * (1_i64 << fraction_bits)
                    })
                    .collect()
            })
            .collect()
    };
    let tiles: Vec<_> = raw
        .iter()
        .enumerate()
        .map(|(component, samples)| TileComponent {
            component: u16::try_from(component).unwrap(),
            x: x0 - size.x_origin.div_ceil(sx),
            y: region[1] - size.y_origin,
            width,
            height,
            fraction_bits,
            samples,
        })
        .collect();
    let mut decoded = assemble_selected_image(size, &tiles, region, 0, &ResourceLimits::HARDENED)
        .expect("valid reconstructed selected-image fixture");
    if inverse_rct {
        apply_reversible_component_transform(&mut decoded).unwrap();
    }
    // Independent RCT algebra: R=Y+Cr, G=Y-floor((Cb+Cr)/4), B=Y+Cb.
    let post_rct: Vec<Vec<i64>> = if inverse_rct {
        vec![
            centered.clone(),
            centered.iter().map(|value| value + 1).collect(),
            centered.iter().map(|value| value + 2).collect(),
        ]
    } else {
        (0..3)
            .map(|component| {
                centered
                    .iter()
                    .map(|value| if component == 0 { *value } else { -*value - 1 })
                    .collect()
            })
            .collect()
    };
    let expected = post_rct
        .iter()
        .map(|samples| {
            samples
                .iter()
                .map(|value| {
                    if size.component(0).unwrap().signed {
                        (*value).clamp(-8, 7)
                    } else {
                        (*value + 8).clamp(0, 15)
                    }
                })
                .collect()
        })
        .collect();
    (decoded, expected)
}

fn values(component: &TypedComponent) -> Vec<i64> {
    match component.samples() {
        TypedSamples::Signed(values) => values.clone(),
        TypedSamples::Unsigned(values) => values
            .iter()
            .map(|value| i64::try_from(*value).unwrap())
            .collect(),
    }
}

fn assert_geometry(
    image: &TypedImage,
    geometry: &[(u32, u32, codestream::ComponentInfo)],
    region: [u32; 4],
) {
    assert_eq!(
        (image.width(), image.height()),
        (region[2] - region[0], region[3] - region[1])
    );
    assert_eq!((image.x_origin(), image.y_origin()), (region[0], region[1]));
    assert_eq!(image.components().len(), geometry.len());
    for (output, &(width, height, info)) in image.components().iter().zip(geometry) {
        assert_eq!((output.width(), output.height()), (width, height));
        assert_eq!(values(output).len(), (width * height) as usize);
        let a = output.info();
        let b = info;
        assert_eq!(
            (
                a.x_subsampling,
                a.y_subsampling,
                a.x_registration,
                a.y_registration
            ),
            (
                b.x_subsampling,
                b.y_subsampling,
                b.x_registration,
                b.y_registration
            )
        );
        assert!(output.channel_definitions().is_empty());
    }
}

#[test]
fn typed_layout_nlt_descriptors_and_untouched_components() {
    let limits = ResourceLimits::HARDENED;
    for source_signed in [false, true] {
        for target_signed in [false, true] {
            for precision in 1..=38 {
                let nlt = Some((precision, target_signed, false));
                let bytes = headers(source_signed, 2, false, nlt);
                let layout = parse_layout(&bytes, &limits).unwrap();
                assert_layout_premises(&layout, nlt);
                let region = [5, 2, 35, 5];
                let (decoded, expected) = reconstruction(&layout, region, 2, false);
                eprintln!(
                    "typed_layout descriptor: source_signed={source_signed} target=({precision},{target_signed}) region={region:?}"
                );
                let output = typed_layout(&layout, decoded, &limits).unwrap();
                for component in 0..3 {
                    let output = output.component(component).unwrap();
                    let source = layout.info.image_size.component(component).unwrap();
                    let (p, signed) = if component == 0 {
                        (precision, target_signed)
                    } else {
                        (source.precision, source.signed)
                    };
                    assert_eq!((output.info().precision, output.info().signed), (p, signed));
                    assert_eq!(matches!(output.samples(), TypedSamples::Signed(_)), signed);
                    if component == 0 {
                        let value = if signed { -(1_i64 << (p - 1)) } else { 0 };
                        assert_eq!(values(output), vec![value; expected[0].len()]);
                    } else {
                        assert_eq!(values(output), expected[usize::from(component)]);
                    }
                }
            }
        }
        let bytes = headers(source_signed, 1, false, None);
        let layout = parse_layout(&bytes, &limits).unwrap();
        assert_layout_premises(&layout, None);
        let (decoded, expected) = reconstruction(&layout, [3, 1, 35, 9], 0, false);
        let output = typed_layout(&layout, decoded, &limits).unwrap();
        for component in 0..3 {
            let output = output.component(component).unwrap();
            assert_eq!(
                output.info(),
                layout.info.image_size.component(component).unwrap()
            );
            assert_eq!(values(output), expected[usize::from(component)]);
        }
    }
}

#[test]
fn typed_layout_inverse_nlt_after_reconstruction_preserves_geometry() {
    let limits = ResourceLimits::HARDENED;
    let mut discrepancies = Vec::new();
    for source_signed in [false, true] {
        for target_signed in [false, true] {
            for (region, subsampling) in [([3, 1, 35, 9], 1), ([5, 2, 35, 5], 2), ([7, 3, 8, 4], 1)]
            {
                for (fraction_bits, inverse_rct) in [(0, false), (16, false), (0, true)] {
                    for active in [false, true] {
                        let nlt = active.then_some((8, target_signed, true));
                        let bytes = headers(source_signed, subsampling, inverse_rct, nlt);
                        let layout = parse_layout(&bytes, &limits).unwrap();
                        assert_layout_premises(&layout, nlt);
                        let (decoded, mut expected) =
                            reconstruction(&layout, region, fraction_bits, inverse_rct);
                        if active {
                            // T.801 K-7..K-9: normalize by 2^4-1, linearly interpolate
                            // [255,0]/255, then scale by 2^8-1 and signed-shift.
                            // All results are integral; no rounding convention is assumed.
                            for value in &mut expected[0] {
                                let rank = *value + if source_signed { 8 } else { 0 };
                                *value = 255 - 17 * rank - if target_signed { 128 } else { 0 };
                            }
                        }
                        let route = format!(
                            "typed_layout source_signed={source_signed} target_signed={target_signed} region={region:?} subsampling={subsampling} fraction_bits={fraction_bits} inverse_rct={inverse_rct} active={active}"
                        );
                        eprintln!("{route}");
                        let geometry: Vec<_> = decoded
                            .components()
                            .iter()
                            .map(|component| {
                                (component.width(), component.height(), component.info())
                            })
                            .collect();
                        let output = typed_layout(&layout, decoded, &limits);
                        let output = output.expect(&route);
                        assert_geometry(&output, &geometry, region);
                        for component in 0..3 {
                            let actual = values(output.component(component).unwrap());
                            if actual != expected[usize::from(component)] {
                                discrepancies.push(format!(
                                    "{route} component={component}: actual={:?} expected={:?}",
                                    &actual[..actual.len().min(16)],
                                    &expected[usize::from(component)][..actual.len().min(16)]
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        discrepancies.is_empty(),
        "standards-derived NLT mismatch ({} scenarios):\n{}",
        discrepancies.len(),
        discrepancies.join("\n")
    );
}
