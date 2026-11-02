// T.800 I.5.1-I.5.4/I.8 and T.801 M.11.1/M.11.6 are the oracles.
// Only box framing, placement, cardinality, or the explicitly tested field changes.

fn spec_parts() -> Vec<Vec<u8>> {
    let bytes = include_bytes!("../../validation/image-consumer-probe/tests/fixtures/gray.jp2");
    let mut parts = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        assert!(length >= 8 && offset + length <= bytes.len());
        parts.push(bytes[offset..offset + length].to_vec());
        offset += length;
    }
    assert_eq!(offset, bytes.len());
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0], b"\0\0\0\x0cjP  \r\n\x87\n");
    assert_eq!(parts[1], box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
    assert_eq!(&parts[2][4..8], b"jp2h");
    let ihdr = vec![0, 0, 0, 1, 0, 0, 0, 1, 0, 1, 7, 7, 0, 0];
    let mut header = box32(*b"ihdr", &ihdr);
    header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 17]));
    assert_eq!(parts[2], box32(*b"jp2h", &header));
    assert_eq!(&parts[3][4..10], b"jp2c\xff\x4f");
    assert_eq!(&parts[3][parts[3].len() - 2..], b"\xff\xd9");
    // The complete one-tile stream has one empty packet: the independent
    // reversible 8-bit reconstruction is the level-shift value 128.
    let image = crate::decode::decode_jp2(bytes, &crate::DecodeOptions::default()).unwrap();
    assert_eq!(image.components().len(), 1);
    assert_eq!(image.component(0).unwrap().width(), 1);
    assert_eq!(image.component(0).unwrap().height(), 1);
    assert_eq!(
        image.component(0).unwrap().samples(),
        &crate::image::TypedSamples::Unsigned(vec![128])
    );
    parts
}

fn spec_record(failures: &mut Vec<String>, predicate: bool, context: impl Into<String>) {
    let context = context.into();
    eprintln!("jp2::parse: {context}: {predicate}");
    if !predicate {
        failures.push(context);
    }
}

fn spec_reject(failures: &mut Vec<String>, bytes: &[u8], context: impl Into<String>) {
    let result = parse(bytes, &ResourceLimits::default());
    spec_record(
        failures,
        matches!(result, Err(DecodeError::MalformedInput { .. })),
        format!("{}; malformed rejection, got {result:?}", context.into()),
    );
}

fn spec_finish(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "clause violations:\n{}",
        failures.join("\n")
    );
}

#[test]
fn spec_signature_first_fixed_and_unique() {
    parses_minimal_file();
    let parts = spec_parts();
    let mut failures = Vec::new();
    assert!(parse(&parts.concat(), &ResourceLimits::default()).is_ok());
    for index in 4..12 {
        let mut damaged = parts.clone();
        damaged[0][index] ^= 1;
        spec_reject(
            &mut failures,
            &damaged.concat(),
            format!("signature byte {index}"),
        );
    }
    for payload in [&b"\r\n\x87"[..], &b"\r\n\x87\n\0"[..]] {
        let mut damaged = parts.clone();
        damaged[0] = box32(*b"jP  ", payload);
        spec_reject(&mut failures, &damaged.concat(), "signature fixed length");
    }
    spec_reject(&mut failures, &parts[1..].concat(), "signature missing");
    let mut late = vec![box32(*b"zzzz", &[])];
    late.extend(parts.clone());
    spec_reject(&mut failures, &late.concat(), "signature not first");
    for position in 2..=4 {
        let mut duplicate = parts.clone();
        duplicate.insert(position, parts[0].clone());
        spec_reject(
            &mut failures,
            &duplicate.concat(),
            format!("second signature at box {position}"),
        );
    }
    spec_finish(failures);
}

#[test]
fn spec_file_type_immediate_and_unique() {
    parses_minimal_file();
    rejects_invalid_length_and_order();
    let parts = spec_parts();
    let mut failures = Vec::new();
    assert!(parse(&parts.concat(), &ResourceLimits::default()).is_ok());
    let mut missing = parts.clone();
    missing.remove(1);
    spec_reject(&mut failures, &missing.concat(), "ftyp missing");
    let mut late = parts.clone();
    late.insert(1, box32(*b"zzzz", &[]));
    spec_reject(
        &mut failures,
        &late.concat(),
        "ftyp not immediately after signature",
    );
    for position in 2..=4 {
        let mut duplicate = parts.clone();
        duplicate.insert(position, parts[1].clone());
        spec_reject(
            &mut failures,
            &duplicate.concat(),
            format!("second ftyp at box {position}"),
        );
    }
    spec_finish(failures);
}

#[test]
fn spec_header_top_level_unique_and_ordered() {
    parses_minimal_file();
    rejects_invalid_length_and_order();
    let parts = spec_parts();
    let mut failures = Vec::new();
    for before in [0, 1, 3, 8] {
        for after in [0, 1, 3, 8] {
            let mut input = parts[..2].to_vec();
            input.extend((0..before).map(|_| box32(*b"zzzz", &[0xff])));
            input.push(parts[2].clone());
            input.extend((0..after).map(|_| box32(*b"zzzz", &[0xff])));
            input.push(parts[3].clone());
            let bytes = input.concat();
            let result = parse(&bytes, &ResourceLimits::default());
            spec_record(
                &mut failures,
                result.as_ref().is_ok_and(|file| {
                    file.header().image.width == 1
                        && file.header().image.height == 1
                        && file.codestreams()[0].data() == &parts[3][8..]
                }),
                format!("one header; intervening boxes before={before} after={after}; {result:?}"),
            );
        }
    }
    for order in [
        vec![0, 1, 3],
        vec![0, 1, 3, 2],
        vec![0, 2, 1, 3],
        vec![0, 1, 2, 2, 3],
        vec![0, 1, 2, 3, 2],
    ] {
        let bytes = order
            .iter()
            .flat_map(|&i| parts[i].clone())
            .collect::<Vec<_>>();
        spec_reject(
            &mut failures,
            &bytes,
            format!("header missing/order/duplicate: {order:?}"),
        );
    }
    let nested = [
        parts[0].clone(),
        parts[1].clone(),
        box32(*b"asoc", &parts[2]),
        parts[3].clone(),
    ]
    .concat();
    spec_reject(
        &mut failures,
        &nested,
        "nested header does not replace top-level header",
    );
    spec_finish(failures);
}

#[test]
fn spec_minor_version_retained_without_jph() {
    let parts = spec_parts();
    let mut versions = vec![0, 1, 255, 256, 65535, 65536, 0x8000_0000, u32::MAX];
    let mut seed = 0x4a50_3220_u32;
    for _ in 0..32 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        versions.push(seed);
    }
    for version in versions {
        let mut input = parts.clone();
        input[1][12..16].copy_from_slice(&version.to_be_bytes());
        assert_eq!(&input[1][8..12], b"jp2 ");
        assert_eq!(&input[1][16..], b"jp2 ");
        let bytes = input.concat();
        let parsed = parse(&bytes, &ResourceLimits::default())
            .unwrap_or_else(|error| panic!("ordinary JP2 MinV={version:#x}: {error:?}"));
        assert_eq!(parsed.file_type().minor_version, version);
        assert!(!parsed.file_type().is_jph_compatible());
        assert_eq!(parsed.header().image.width, 1);
        assert_eq!(parsed.codestreams()[0].data(), &parts[3][8..]);
        let decoded = crate::decode::decode_jp2(&bytes, &crate::DecodeOptions::default()).unwrap();
        assert_eq!(
            decoded.component(0).unwrap().samples(),
            &crate::image::TypedSamples::Unsigned(vec![128])
        );
    }
}

#[test]
fn spec_unknown_opaque_extent() {
    let parts = spec_parts();
    let mut payloads = vec![
        vec![],
        vec![0],
        vec![0xff; 7],
        box32(*b"jP  ", &SIGNATURE_MAGIC),
        box32(*b"jp2h", &[]),
        [1_u32.to_be_bytes().as_slice(), b"asoc", &[0xff; 3]].concat(),
    ];
    let mut seed = 0x1234_abcd_u32;
    for length in [8, 15, 16, 31, 256, 1024] {
        let mut payload = Vec::new();
        for _ in 0..length {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            payload.push((seed >> 24) as u8);
        }
        payloads.push(payload);
    }
    for payload in payloads {
        for position in 2..=4 {
            for extended in [false, true] {
                let opaque = if extended {
                    [
                        1_u32.to_be_bytes().as_slice(),
                        b"zzzz",
                        &((16 + payload.len()) as u64).to_be_bytes(),
                        &payload,
                    ]
                    .concat()
                } else {
                    box32(*b"zzzz", &payload)
                };
                let mut input = parts.clone();
                let expected_stream_offset = if position <= 3 {
                    (parts[..3].iter().map(Vec::len).sum::<usize>() + opaque.len()) as u64
                } else {
                    parts[..3].iter().map(Vec::len).sum::<usize>() as u64
                };
                input.insert(position, opaque);
                let bytes = input.concat();
                let parsed = parse(&bytes, &ResourceLimits::default()).unwrap_or_else(|error| {
                    panic!(
                        "opaque payload={} position={position} extended={extended}: {error:?}",
                        payload.len()
                    )
                });
                assert_eq!(parsed.codestream_count(), 1);
                assert_eq!(parsed.codestreams()[0].data(), &parts[3][8..]);
                assert_eq!(
                    parsed.codestreams()[0].box_header().offset(),
                    expected_stream_offset
                );
                assert_eq!(parsed.header().image.width, 1);
                assert_eq!(parsed.box_count, 5);
                assert_eq!(parsed.metadata_boxes(), 1);
                assert_eq!(parsed.metadata_depth(), 1);
            }
        }
    }
    let mut eof = parts.clone();
    eof.push([0_u32.to_be_bytes().as_slice(), b"zzzz", b"opaque"].concat());
    assert_eq!(
        parse(&eof.concat(), &ResourceLimits::default())
            .unwrap()
            .metadata_boxes(),
        1
    );
    let mut invalid = parts;
    invalid.push([100_u32.to_be_bytes().as_slice(), b"zzzz", b"short"].concat());
    let mut failures = Vec::new();
    spec_reject(
        &mut failures,
        &invalid.concat(),
        "unknown box extent past end is not skippable",
    );
    spec_finish(failures);
}

#[test]
fn spec_later_jp2_codestreams_preserve_first_display() {
    let parts = spec_parts();
    let rgb = include_bytes!("../../validation/image-consumer-probe/tests/fixtures/rgb.jp2");
    assert_eq!(&rgb[77..85], b"\0\0\0\x62jp2c");
    let rgb_image = crate::decode::decode_jp2(rgb, &crate::DecodeOptions::default()).unwrap();
    assert_eq!(rgb_image.components().len(), 3);
    for component in rgb_image.components() {
        assert_eq!(
            component.samples(),
            &crate::image::TypedSamples::Unsigned(vec![128])
        );
    }
    let limits = ResourceLimits::default()
        .with_maximum_codestreams(8)
        .unwrap();
    let mut failures = Vec::new();
    for count in [1, 2, 3, 7, 8] {
        let mut input = parts[..3].to_vec();
        for index in 0..count {
            input.push(if index == 0 {
                parts[3].clone()
            } else {
                rgb[77..].to_vec()
            });
            input.push(box32(*b"zzzz", &[0xff]));
        }
        let bytes = input.concat();
        let result = parse(&bytes, &limits);
        spec_record(
            &mut failures,
            result.as_ref().is_ok_and(|file| {
                file.codestream_count() >= 1 && file.codestreams()[0].data() == &parts[3][8..]
            }),
            format!(
                "ordinary JP2 accepts {count} complete well-framed codestreams within limit 8; {result:?}"
            ),
        );
        let decoded = crate::decode::decode_jp2(&bytes, &crate::DecodeOptions::new(limits));
        spec_record(
            &mut failures,
            decoded.as_ref().is_ok_and(|image| {
                image.components().len() == 1
                    && image.component(0).unwrap().width() == 1
                    && image.component(0).unwrap().height() == 1
                    && image.component(0).unwrap().samples()
                        == &crate::image::TypedSamples::Unsigned(vec![128])
            }),
            format!("first-codestream display unaffected by {count} streams; {decoded:?}"),
        );
    }
    let mut over_limit = parts[..3].to_vec();
    over_limit.extend((0..9).map(|_| parts[3].clone()));
    let bytes = over_limit.concat();
    let result = parse(&bytes, &limits);
    // Other validation bugs can mask this boundary; retain that failure rather
    // than accepting an unrelated earlier rejection as resource admission.
    spec_record(
        &mut failures,
        matches!(
            result,
            Err(DecodeError::LimitExceeded {
                resource: Resource::Codestreams,
                limit: 8,
                actual: 9
            })
        ),
        format!("ninth stream exceeds capacity 8; {result:?}"),
    );
    spec_reject(
        &mut failures,
        &[
            parts[0].clone(),
            parts[1].clone(),
            parts[3].clone(),
            parts[2].clone(),
        ]
        .concat(),
        "codestream before JP2 header forbidden",
    );
    spec_finish(failures);
}

#[test]
fn spec_jpx_header_bijection_and_defaults() {
    retains_and_inspects_explicit_jpx_codestreams_and_headers();
    let parts = spec_parts();
    let limits = ResourceLimits::default()
        .with_maximum_codestreams(9)
        .unwrap();
    let mut failures = Vec::new();
    for count in [1_usize, 2, 3, 8] {
        // M.11.1: ML=1, FUAM=DCM=1, NSF=1, SF=1 (no
        // codestream extensions), SM=1, NVF=0. No JPH signaling.
        let prefix = vec![
            parts[0].clone(),
            box32(*b"ftyp", b"jpx \0\0\0\0jp2 jpx "),
            box32(*b"rreq", &[1, 1, 1, 0, 1, 0, 1, 1, 0, 0]),
            parts[2].clone(),
        ];
        assert_eq!(&prefix[1][8..12], b"jpx ");
        assert_eq!(&prefix[1][16..], b"jp2 jpx ");
        assert_eq!(&prefix[2][4..8], b"rreq");
        let mut headers = Vec::new();
        for index in 0..=count {
            let mut header = parts[2][8..30].to_vec(); // ihdr only; defaults supply colour.
            header.extend(box32(*b"lbl ", format!("stream-{index}\0").as_bytes()));
            headers.push(header);
        }
        for header_count in [0, count, count + 1, count.saturating_sub(1)] {
            let mut input = prefix.clone();
            input.extend(
                headers[..header_count]
                    .iter()
                    .map(|header| box32(*b"jpch", header)),
            );
            input.extend((0..count).map(|_| parts[3].clone()));
            let bytes = input.concat();
            let result = parse(&bytes, &limits);
            if header_count != 0 && header_count != count {
                spec_record(
                    &mut failures,
                    matches!(result, Err(DecodeError::MalformedInput { .. })),
                    format!(
                        "JPX streams={count}, headers={header_count}: reject unequal nonempty sequence; {result:?}"
                    ),
                );
            } else {
                match result {
                    Ok(file) => {
                        assert_eq!(file.codestream_count(), count);
                        for (index, stream) in file.codestreams().iter().enumerate() {
                            assert_eq!(stream.data(), &parts[3][8..]);
                            if header_count == 0 {
                                assert!(stream.codestream_header().is_none());
                            } else {
                                assert_eq!(
                                    stream.codestream_header().unwrap().payload(),
                                    headers[index]
                                );
                            }
                            let selected =
                                associated_header_for_codestream(stream, file.header()).unwrap();
                            assert_eq!(selected.image, file.header().image);
                            assert_eq!(selected.color, file.header().color);
                        }
                        let inspected = parse_and_inspect(&bytes, &limits).unwrap();
                        assert_eq!(inspected.codestream_count(), count);
                        for info in inspected.codestreams() {
                            assert_eq!(info.image_size.width, 1);
                            assert_eq!(info.image_size.height, 1);
                            assert_eq!(info.image_size.components, 1);
                        }
                    }
                    Err(error) => spec_record(
                        &mut failures,
                        false,
                        format!("JPX valid streams={count}, headers={header_count}: {error:?}"),
                    ),
                }
            }
        }
    }
    spec_finish(failures);
}
