// Included in jp2::tests so the private subject and existing regression stay native.
fn spec_ihdr(width: u32, height: u32, components: u16, depth: u8) -> Vec<u8> {
    let mut payload = height.to_be_bytes().to_vec();
    payload.extend_from_slice(&width.to_be_bytes());
    payload.extend_from_slice(&components.to_be_bytes());
    payload.extend_from_slice(&[depth, 7, 0, 0]);
    box32(*b"ihdr", &payload)
}

fn spec_colr() -> Vec<u8> {
    box32(*b"colr", &[1, 0, 0, 0, 0, 0, 16])
}

fn spec_gray_channel() -> Vec<u8> {
    let mut boxes = box32(*b"colr", &[1, 0, 0, 0, 0, 0, 17]);
    // One unsigned greyscale channel, even when the codestream has many components.
    boxes.extend(box32(*b"cmap", &[0, 0, 0, 0]));
    boxes
}

fn spec_expect_malformed(
    result: Result<Jp2Header, DecodeError>,
    offset: u64,
    kind: MalformedKind,
    context: &str,
) {
    assert_eq!(
        result,
        Err(DecodeError::MalformedInput { offset, kind }),
        "{context}"
    );
}

#[test]
fn spec_header_first_child_is_ihdr() {
    let limits = ResourceLimits::default();
    let mut seed = 0x5eed_u32;
    for base in [0, 8, 1024, 1 << 32] {
        for _ in 0..16 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let width = 1 + seed % 256;
            let height = 1 + (seed >> 16) % 256;
            let ihdr = spec_ihdr(width, height, 3, 7);
            let mut valid = ihdr.clone();
            valid.extend(spec_colr());
            eprintln!("unit parse_jp2_header: first-child base={base} {width}x{height}");
            let parsed = parse_jp2_header(&valid, &limits, base).unwrap();
            assert_eq!(
                parsed.image,
                ImageHeader {
                    width,
                    height,
                    components: 3,
                    bit_depth: ComponentBitDepth::Uniform {
                        bits: 8,
                        signed: false
                    },
                    unknown_color_space: false,
                    intellectual_property: false,
                }
            );
            assert_eq!(parsed.color, ColorSpecification::enumerated(16));
            assert_eq!(parsed.nested_box_count, 2);

            // Same valid metadata bytes under a wrong tag must not be interpreted.
            for tag in [*b"free", *b"colr", *b"bpcc", *b"jp2h"] {
                let mut wrong = ihdr.clone();
                wrong[4..8].copy_from_slice(&tag);
                wrong.extend(spec_colr());
                spec_expect_malformed(
                    parse_jp2_header(&wrong, &limits, base),
                    base,
                    MalformedKind::InvalidBoxOrder,
                    &format!("first tag={tag:?}, base={base}"),
                );
            }
            let mut reordered = spec_colr();
            reordered.extend(ihdr.clone());
            spec_expect_malformed(
                parse_jp2_header(&reordered, &limits, base),
                base,
                MalformedKind::InvalidBoxOrder,
                "colr before real ihdr",
            );
            let mut duplicate = valid;
            let duplicate_offset = base + duplicate.len() as u64;
            duplicate.extend(ihdr);
            spec_expect_malformed(
                parse_jp2_header(&duplicate, &limits, base),
                duplicate_offset,
                MalformedKind::InvalidBoxOrder,
                "second ihdr",
            );
        }
        spec_expect_malformed(
            parse_jp2_header(&[], &limits, base),
            base,
            MalformedKind::MissingRequiredStructure,
            "empty header",
        );
    }
}

#[test]
fn spec_header_ignores_later_colour_semantics() {
    let limits = ResourceLimits::default();
    // T.800 I.5.3.3 Table I.9 explicitly requires ignoring reserved methods,
    // including optional uninterpreted bytes after the three common fields.
    // No truncated METH=1 or invalid ICC profile is used as a conforming fixture.
    let mut failures = Vec::new();
    for base in [0, 40, 1 << 32] {
        let mut baseline = spec_ihdr(13, 17, 3, 7);
        baseline.extend(spec_colr());
        let expected = parse_jp2_header(&baseline, &limits, base).unwrap();
        assert_eq!(expected.color, ColorSpecification::enumerated(16));
        for method in [0, 4, 5, 127, 254, 255] {
            for extra in [0, 1, 7, 31] {
                let mut ignored = vec![method, 0, 0];
                ignored.extend((0..extra).map(|i| (i * 17 + 3) as u8));
                assert!(method != 1 && method != 2);
                assert_eq!(&ignored[1..3], &[0, 0]);
                let encoded = box32(*b"colr", &ignored);
                assert_eq!(
                    u32::from_be_bytes(encoded[..4].try_into().unwrap()) as usize,
                    encoded.len()
                );
                let mut bytes = baseline.clone();
                bytes.extend(encoded);
                bytes.extend(spec_colr());
                eprintln!(
                    "unit parse_jp2_header: contiguous colr method={method}, extra={extra}, base={base}"
                );
                match parse_jp2_header(&bytes, &limits, base) {
                    Ok(mut actual) => {
                        assert_eq!(actual.nested_box_count, 4);
                        // Only the number of framed children may differ.
                        actual.nested_box_count = expected.nested_box_count;
                        assert_eq!(actual, expected, "later colour altered selected metadata");
                    }
                    Err(error) => failures.push(format!(
                        "method={method} extra={extra} base={base}: expected ignored colr, got {error:?}"
                    )),
                }
            }
        }
        let mut absent = spec_ihdr(13, 17, 3, 7);
        spec_expect_malformed(
            parse_jp2_header(&absent, &limits, base),
            base,
            MalformedKind::MissingRequiredStructure,
            "no first colour",
        );
        let offset = base + absent.len() as u64;
        absent.extend(box32(*b"colr", &[1, 0, 0]));
        spec_expect_malformed(
            parse_jp2_header(&absent, &limits, base),
            offset,
            MalformedKind::InvalidBoxLength,
            "selected colour must still be parsed",
        );
    }
    assert!(
        failures.is_empty(),
        "JP2 reader ignore predicate violated:\n{}",
        failures.join("\n")
    );
}

#[test]
fn spec_header_bpcc_presence_and_multiplicity() {
    // Retain and execute the original signed-depth regression unchanged.
    requires_bpcc_for_variable_component_depths();
    let limits = ResourceLimits::default();
    for base in [0, 40, 1 << 32] {
        for components in [1, 2, 3, limits.maximum_components()] {
            for bits in 1_u8..=38 {
                let mut uniform = spec_ihdr(1, 1, components, bits - 1);
                uniform.extend(spec_gray_channel());
                eprintln!("unit parse_jp2_header: bpcc base={base}, NC={components}, bits={bits}");
                let parsed = parse_jp2_header(&uniform, &limits, base).unwrap();
                assert_eq!(parsed.image.components, components);
                assert_eq!(
                    parsed.image.bit_depth,
                    ComponentBitDepth::Uniform {
                        bits,
                        signed: false
                    }
                );
                assert_eq!(parsed.nested_box_count, 3);
                assert_eq!(parsed.color, ColorSpecification::enumerated(17));
                assert_eq!(parsed.component_mapping_entries, 1);
                let depths = vec![bits - 1; usize::from(components)];
                uniform.extend(box32(*b"bpcc", &depths));
                spec_expect_malformed(
                    parse_jp2_header(&uniform, &limits, base),
                    base,
                    MalformedKind::MissingRequiredStructure,
                    "uniform ihdr must not admit even a matching bpcc",
                );
                if components < 2 {
                    continue;
                }
                // Two distinct unsigned precisions establish actual variation.
                let mut depths = depths;
                depths[1] = bits % 38;
                assert_ne!(depths[0], depths[1]);
                for after_color in [false, true] {
                    let mut variable = spec_ihdr(1, 1, components, 255);
                    variable.extend(spec_gray_channel());
                    spec_expect_malformed(
                        parse_jp2_header(&variable, &limits, base),
                        base,
                        MalformedKind::MissingRequiredStructure,
                        "missing variable bpcc",
                    );
                    let mut variable = spec_ihdr(1, 1, components, 255);
                    if after_color {
                        variable.extend(spec_gray_channel());
                    }
                    variable.extend(box32(*b"bpcc", &depths));
                    if !after_color {
                        variable.extend(spec_gray_channel());
                    }
                    let parsed = parse_jp2_header(&variable, &limits, base).unwrap();
                    assert_eq!(parsed.image.bit_depth, ComponentBitDepth::Variable);
                    assert_eq!(parsed.image.components, components);
                    assert_eq!(parsed.color, ColorSpecification::enumerated(17));
                    assert_eq!(parsed.nested_box_count, 4);
                    assert_eq!(parsed.component_mapping_entries, 1);
                    let offset = base + variable.len() as u64;
                    variable.extend(box32(*b"bpcc", &depths));
                    spec_expect_malformed(
                        parse_jp2_header(&variable, &limits, base),
                        offset,
                        MalformedKind::InvalidBoxOrder,
                        "duplicate variable bpcc",
                    );
                }
            }
        }
    }
}
