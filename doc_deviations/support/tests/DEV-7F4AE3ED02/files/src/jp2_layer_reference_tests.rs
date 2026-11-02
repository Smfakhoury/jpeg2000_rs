use super::*;

// These are scoped layer-header fixtures, not complete JPX files. T.801
// M.11.7 permits a top-level jplh with a local cgrp; M.11.7.7 permits
// zero or one creg, with XS=YS=XR=YR=1 and XO=YO=0. No file/profile,
// fragment, extension-group, rendering, or codestream-decoding claim is made.
fn layer_headers(references: &[Vec<u16>]) -> (Vec<u8>, Vec<u64>) {
    let mut bytes = Vec::new();
    let mut offsets = Vec::new();
    for entries in references {
        offsets.push(u64::try_from(bytes.len()).unwrap());
        let mut payload = box32(*b"cgrp", &box32(*b"colr", &[1, 0, 0, 0, 0, 0, 17]));
        if !entries.is_empty() {
            let mut registration = vec![0, 1, 0, 1];
            for index in entries {
                registration.extend_from_slice(&index.to_be_bytes());
                registration.extend_from_slice(&[1, 1, 0, 0]);
            }
            assert_eq!(registration.len(), 4 + 6 * entries.len());
            payload.extend(box32(*b"creg", &registration));
        }
        bytes.extend(box32(*b"jplh", &payload));
    }
    (bytes, offsets)
}

fn parsed_layers<'a>(bytes: &'a [u8], references: &[Vec<u16>]) -> Vec<CompositingLayer<'a>> {
    let limits = ResourceLimits::default();
    assert!(references.len() <= usize::from(limits.maximum_layers()));
    let mut reader = BoxReader::new(bytes, &limits).unwrap();
    let mut layers = Vec::new();
    for entries in references {
        let framed = reader.next_box_ref().unwrap().unwrap();
        assert_eq!(framed.header.box_type(), BoxType::COMPOSITING_LAYER_HEADER);
        let layer = parse_compositing_layer(framed, &limits).unwrap();
        assert!(layer.has_color_group());
        assert_eq!(layer.horizontal_grid_size(), 1);
        assert_eq!(layer.vertical_grid_size(), 1);
        assert_eq!(layer.registrations().len(), entries.len());
        for (registration, expected_index) in layer.registrations().iter().zip(entries) {
            assert_eq!(
                *registration,
                CodestreamRegistration {
                    codestream_index: *expected_index,
                    horizontal_subsampling: 1,
                    vertical_subsampling: 1,
                    horizontal_offset: 0,
                    vertical_offset: 0,
                }
            );
        }
        layers.push(layer);
    }
    assert!(reader.next_box_ref().is_none());
    layers
}

#[test]
fn no_registration_uses_ordinal_bounds() {
    // The subject has no requires predicates. In particular zero is a valid
    // unit input, though the enclosing file parser requires a codestream.
    for count in [0_usize, 1, 2, 3, 7, 31, 63, 64] {
        let mut lengths = vec![0, 1, count.saturating_sub(1), count, count + 1, 255, 256];
        lengths.sort_unstable();
        lengths.dedup();
        for length in lengths {
            let references = vec![Vec::new(); length];
            let (bytes, offsets) = layer_headers(&references);
            let layers = parsed_layers(&bytes, &references);
            eprintln!(
                "unit route BoxReader -> parse_compositing_layer -> \
                 validate_layer_references: no creg, count={count}, layers={length}"
            );
            let actual = validate_layer_references(&layers, count);
            let expected = if length <= count {
                Ok(())
            } else {
                Err(DecodeError::malformed(
                    offsets[count],
                    MalformedKind::InvalidBoxContents,
                ))
            };
            assert_eq!(actual, expected, "count={count}, layers={length}");
            assert!(layers.iter().all(|layer| layer.registrations().is_empty()));
        }
    }
}

#[test]
fn registration_presence_is_file_wide_and_references_exist() {
    // Retain and execute the existing regression's original fixture and checks.
    retains_validated_layer_registration_and_static_composition_options();

    let mut seed = 0x7b1d_209a_u32;
    for count in [1_u16, 2, 3, 7, 31, 63, 64] {
        for length in [1_usize, 2, 3, 7, 64, 256] {
            let mut references = Vec::new();
            for _ in 0..length {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                references.push(vec![u16::try_from(seed % u32::from(count)).unwrap()]);
            }
            // Explicit references need not equal the layer ordinal; 256
            // layers may all reference the sole available codestream.
            let (bytes, _) = layer_headers(&references);
            let layers = parsed_layers(&bytes, &references);
            assert_eq!(
                validate_layer_references(&layers, usize::from(count)),
                Ok(()),
                "all registered, count={count}, layers={length}"
            );
            for invalid in [count, count.saturating_add(1), u16::MAX] {
                for bad_layer in [0, length / 2, length - 1] {
                    let mut bad = references.clone();
                    // A valid first entry must not hide an invalid later one.
                    bad[bad_layer].push(invalid);
                    let (bytes, offsets) = layer_headers(&bad);
                    let layers = parsed_layers(&bytes, &bad);
                    eprintln!(
                        "unit validate_layer_references: count={count}, layers={length}, \
                         bad_layer={bad_layer}, last_reference={invalid}"
                    );
                    assert_eq!(
                        validate_layer_references(&layers, usize::from(count)),
                        Err(DecodeError::malformed(
                            offsets[bad_layer],
                            MalformedKind::InvalidBoxContents,
                        )),
                        "out-of-range last registration in layer {bad_layer}"
                    );
                }
            }
        }
    }

    // Enumerate all presence patterns, including both coherent controls.
    // References and unregistered ordinals are in range, so only the
    // M.11.7.7 all-or-none predicate distinguishes the mixed patterns.
    let mut accepted_mixed = Vec::new();
    for length in 2_usize..=8 {
        for mask in 0_usize..(1 << length) {
            let references: Vec<Vec<u16>> = (0..length)
                .map(|index| {
                    if mask & (1 << index) == 0 {
                        Vec::new()
                    } else {
                        vec![u16::try_from(length - 1 - index).unwrap()]
                    }
                })
                .collect();
            let (bytes, _) = layer_headers(&references);
            let layers = parsed_layers(&bytes, &references);
            let actual = validate_layer_references(&layers, length);
            if mask == 0 || mask == (1 << length) - 1 {
                assert_eq!(actual, Ok(()), "coherent presence mask={mask:#x}");
            } else if actual.is_ok() {
                eprintln!(
                    "FAIL unit validate_layer_references: all-or-none violated; \
                     count={length}, layers={length}, creg_presence={mask:#x}; \
                     all explicit references and implicit ordinals exist"
                );
                accepted_mixed.push((length, mask));
            } else {
                // The standard prescribes invalidity, not a particular error
                // variant or blame offset for mixed registration presence.
                eprintln!("mixed registration rejected: {actual:?}");
            }
        }
    }
    assert!(
        accepted_mixed.is_empty(),
        "M.11.7.7 all-or-none presence: accepted {} mixed patterns; first={:?}",
        accepted_mixed.len(),
        accepted_mixed.first()
    );
}
