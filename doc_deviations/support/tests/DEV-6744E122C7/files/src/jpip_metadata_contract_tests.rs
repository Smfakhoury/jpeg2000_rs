// T.808 (12/2022), A.3.6.1-.3: one logical target per cache; metadata
// CSn is ignored; non-root bins contain box contents, not their headers.
fn header(kind: [u8; 4], payload_length: usize, extended: bool) -> Vec<u8> {
    let mut result = if extended {
        1_u32.to_be_bytes().to_vec()
    } else {
        u32::try_from(payload_length + 8)
            .unwrap()
            .to_be_bytes()
            .to_vec()
    };
    result.extend_from_slice(&kind);
    if extended {
        result.extend_from_slice(&u64::try_from(payload_length + 16).unwrap().to_be_bytes());
    }
    result
}

fn boxed(kind: [u8; 4], payload: &[u8], extended: bool) -> Vec<u8> {
    let mut result = header(kind, payload.len(), extended);
    result.extend_from_slice(payload);
    result
}

fn original_placeholder(kind: [u8; 4], length: usize, bin: u64, extended: bool) -> Vec<u8> {
    let mut fields = 1_u32.to_be_bytes().to_vec();
    fields.extend_from_slice(&bin.to_be_bytes());
    fields.extend(header(kind, length, extended));
    let result = box32(*b"phld", &fields);
    assert_eq!(&result[4..8], b"phld");
    assert_ne!(&result[..4], &1_u32.to_be_bytes()); // phld itself cannot use XLBox.
    result
}

struct Fixture {
    bins: Vec<(u64, Vec<u8>)>,
    expected: Vec<u8>,
}

fn fixture(seed: u32, extended: bool) -> Fixture {
    let mut ihdr = 1_u32.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&1_u32.to_be_bytes());
    ihdr.extend_from_slice(&1_u16.to_be_bytes());
    ihdr.extend_from_slice(&[7, 7, 0, 0]);
    let color = vec![1, 0, 0, 0, 0, 0, 17];
    let mut children = box32(*b"ihdr", &ihdr);
    children.extend(boxed(*b"colr", &color, extended));
    children.extend(box32(
        *b"xml ",
        format!("<inside>{seed}</inside>").as_bytes(),
    ));
    let mut transported_children = box32(*b"ihdr", &ihdr);
    transported_children.extend(original_placeholder(*b"colr", color.len(), 2, extended));
    transported_children.extend(box32(
        *b"xml ",
        format!("<inside>{seed}</inside>").as_bytes(),
    ));

    let code = decodable_codestream();
    let mut root = box32(*b"jP  ", &[13, 10, 135, 10]);
    root.extend(box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
    let mut expected = root.clone();
    root.extend(original_placeholder(*b"jp2h", children.len(), 1, extended));
    expected.extend(boxed(*b"jp2h", &children, extended));
    let xml = box32(*b"xml ", format!("<before>phld-{seed}</before>").as_bytes());
    root.extend_from_slice(&xml);
    expected.extend(xml);
    root.extend(original_placeholder(*b"jp2c", code.len(), 3, extended));
    expected.extend(boxed(*b"jp2c", &code, extended));
    let tail = box32(*b"xml ", b"<after/>");
    root.extend_from_slice(&tail);
    expected.extend(tail);

    // Independent premises: signature/brand, legal JP2 image header and raw
    // codestream, correct original lengths, and parent relocation for every
    // nested placeholder. Parsing is an additional check, not the oracle.
    assert_eq!(
        &expected[..12],
        &[0, 0, 0, 12, b'j', b'P', b' ', b' ', 13, 10, 135, 10]
    );
    assert_eq!(&expected[20..24], b"jp2 ");
    assert!(code.starts_with(&[0xff, 0x4f]));
    assert!(code.ends_with(&[0xff, 0xd9]));
    crate::jp2::parse_and_inspect(&expected, &ResourceLimits::default()).unwrap();
    Fixture {
        bins: vec![(0, root), (1, transported_children), (2, color), (3, code)],
        expected,
    }
}

fn expand(
    cache: &DatabinCache,
    csn: u64,
) -> (bool, Vec<u8>, Vec<JpipMissingRange>, Vec<JpipSemanticNeed>) {
    let mut active = Vec::new();
    let mut output = Vec::new();
    let mut missing = Vec::new();
    let mut needs = Vec::new();
    let complete = cache
        .expand_metadata_bin(
            csn,
            0,
            true,
            0,
            u32::MAX,
            &mut active,
            &mut output,
            &mut missing,
            &mut needs,
        )
        .expect("expand_metadata_bin(root=0, boxes=true, depth=0) must accept this fixture");
    assert!(
        active.is_empty(),
        "successful recursive expansion must balance its active stack"
    );
    (complete, output, missing, needs)
}

fn insert_split(
    cache: &mut DatabinCache,
    bin: u64,
    bytes: &[u8],
    first_csn: u64,
    last_csn: u64,
    split: usize,
) {
    assert!(split <= bytes.len());
    // Legal out-of-order arrival, including a zero-length final fragment.
    cache
        .insert(fragment(
            DatabinId::new(8, last_csn, bin).unwrap(),
            split as u64,
            &bytes[split..],
            true,
        ))
        .unwrap();
    if split != 0 {
        cache
            .insert(fragment(
                DatabinId::new(8, first_csn, bin).unwrap(),
                0,
                &bytes[..split],
                false,
            ))
            .unwrap();
    }
}

#[test]
fn metadata_identity_ignores_message_csn() {
    let mut failures = Vec::new();
    let mut seed = 0x4a50_4950_u32;
    for iteration in 0..12 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let f = fixture(seed, iteration % 2 == 1);
        for mode in 0..3 {
            let mut cache = DatabinCache::new(ResourceLimits::default());
            for (bin, bytes) in &f.bins {
                let split = match iteration % 3 {
                    0 => 0,
                    1 => bytes.len(),
                    _ => seed as usize % (bytes.len() + 1),
                };
                let (first, last) = match mode {
                    0 => (0, 0),
                    1 => (7, 7),
                    _ => (0, 7),
                };
                insert_split(&mut cache, *bin, bytes, first, last, split);
            }
            // A.3.6.1: only CSn and fragment arrival change, not the logical
            // target, bin IDs, byte offsets, final lengths, or expected bytes.
            for selected in [0, 7, u64::MAX] {
                let (complete, output, missing, needs) = expand(&cache, selected);
                if !complete || output != f.expected || !missing.is_empty() || !needs.is_empty() {
                    failures.push(format!(
                        "iteration={iteration} mode={mode} selected-CSn={selected}: complete={complete}, output={}/{}, missing={missing:?}, needs={needs:?}",
                        output.len(), f.expected.len(),
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "metadata identity/expansion violated:\n{}",
        failures.join("\n")
    );
}

#[test]
fn original_contents_preserve_nested_box_order() {
    // Retain and execute the existing regression, including its original
    // missing-metadata, missing-codestream and incremental-packet fixtures.
    semantic_metadata_assembly_places_nested_boxes_and_codestreams();
    let mut seed = 0x1234_5678_u32;
    for iteration in 0..16 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let f = fixture(seed, iteration % 2 == 1);
        let mut cache = DatabinCache::new(ResourceLimits::default());
        for (bin, bytes) in &f.bins {
            if *bin != 2 {
                insert_split(
                    &mut cache,
                    *bin,
                    bytes,
                    0,
                    0,
                    seed as usize % (bytes.len() + 1),
                );
            }
        }
        let (complete, _, _, needs) = expand(&cache, 0);
        assert!(
            !complete,
            "a missing referenced colr payload must not count as complete"
        );
        assert_eq!(needs.len(), 1);
        assert_eq!(needs[0].kind(), JpipSemanticNeedKind::MetadataBin);
        assert_eq!(needs[0].identifier(), 2);
        let color = &f.bins[2].1;
        // Final length is known but a real one-byte interior gap remains.
        cache
            .insert(fragment(id(8, 2), 4, &color[4..], true))
            .unwrap();
        cache
            .insert(fragment(id(8, 2), 0, &color[..3], false))
            .unwrap();
        let (complete, _, missing, needs) = expand(&cache, 0);
        assert!(!complete);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].databin(), id(8, 2));
        assert_eq!(missing[0].offset(), 3);
        // Semantic needs request the suffix from the contiguous prefix.
        assert_eq!(missing[0].length(), Some((color.len() - 3) as u64));
        assert_eq!(needs.len(), 1);
        assert_eq!(needs[0].identifier(), 2);
        cache
            .insert(fragment(id(8, 2), 3, &color[3..4], false))
            .unwrap();
        let (complete, output, missing, needs) = expand(&cache, 0);
        assert!(complete);
        assert!(missing.is_empty());
        assert!(needs.is_empty());
        assert_eq!(
            output, f.expected,
            "iteration={iteration}: exact box order/header/payload"
        );
        let assembled = cache.assemble_semantic_container(0).unwrap();
        assert_eq!(assembled.status(), JpipAvailability::Available);
        assert_eq!(assembled.container().unwrap().as_bytes(), f.expected);
        assert!(assembled.missing_ranges().is_empty());
        assert!(assembled.semantic_needs().is_empty());
    }
}

#[test]
fn stream_equivalent_uses_equivalent_box_identity() {
    let color = [1, 0, 0, 0, 0, 0, 17];
    let mut failures = Vec::new();
    for original_extended in [false, true] {
        for equivalent_extended in [false, true] {
            // A.3.6.2 explicitly permits xref -> colr. Original access is
            // disabled, so OrigID is ignored and need not be present in cache.
            // This unit fixture is a jp2h-contents bin, reached with the same
            // depth/active context as recursive expansion from metadata-bin 0.
            let original = header(*b"xref", 32, original_extended);
            let equivalent = header(*b"colr", color.len(), equivalent_extended);
            let mut fields = 2_u32.to_be_bytes().to_vec();
            fields.extend_from_slice(&99_u64.to_be_bytes());
            fields.extend_from_slice(&original);
            fields.extend_from_slice(&2_u64.to_be_bytes());
            fields.extend_from_slice(&equivalent);
            let placeholder = box32(*b"phld", &fields);
            assert_eq!(u32::from_be_bytes(fields[..4].try_into().unwrap()), 2);
            assert_ne!(&original[4..8], &equivalent[4..8]);
            assert_eq!(
                placeholder.len(),
                8 + 4 + 8 + original.len() + 8 + equivalent.len()
            );
            let mut cache = DatabinCache::new(ResourceLimits::default());
            cache
                .insert(fragment(id(8, 1), 0, &placeholder, true))
                .unwrap();
            let mut active = vec![0];
            let mut output = Vec::new();
            let mut missing = Vec::new();
            let mut needs = Vec::new();
            // No equivalent payload: must not publish the original shell.
            assert!(
                !cache
                    .expand_metadata_bin(
                        0,
                        1,
                        true,
                        1,
                        u32::MAX - 1,
                        &mut active,
                        &mut output,
                        &mut missing,
                        &mut needs,
                    )
                    .unwrap()
            );
            assert!(output.is_empty());
            assert_eq!(needs.len(), 1);
            assert_eq!(needs[0].identifier(), 2);
            assert_eq!(needs[0].kind(), JpipSemanticNeedKind::MetadataBin);
            cache.insert(fragment(id(8, 2), 0, &color, true)).unwrap();
            missing.clear();
            needs.clear();
            assert!(
                cache
                    .expand_metadata_bin(
                        0,
                        1,
                        true,
                        1,
                        u32::MAX - 1,
                        &mut active,
                        &mut output,
                        &mut missing,
                        &mut needs,
                    )
                    .unwrap()
            );
            assert_eq!(active, [0]);
            assert!(missing.is_empty());
            assert!(needs.is_empty());
            let expected = boxed(*b"colr", &color, equivalent_extended);
            if output != expected {
                failures.push(format!(
                    "xref->colr original-XLBox={original_extended}, equivalent-XLBox={equivalent_extended}: got {output:?}, expected {expected:?}"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "wrong stream-equivalent box header:\n{}",
        failures.join("\n")
    );
}
