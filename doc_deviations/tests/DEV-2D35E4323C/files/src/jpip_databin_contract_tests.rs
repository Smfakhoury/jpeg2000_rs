// T.808 (12/2022), A.2.2/Table A.2 and A.3.6.1. These tests construct
// identities directly: there is no wire framing, AUX interpretation, or
// negotiated extension. Each cache represents exactly one logical target.

fn contract_coordinates() -> Vec<(u64, u64)> {
    let boundaries = [
        0,
        1,
        15,
        16,
        127,
        128,
        16_383,
        16_384,
        u64::MAX - 1,
        u64::MAX,
    ];
    let mut coordinates = Vec::new();
    for stream in boundaries {
        for bin in boundaries {
            coordinates.push((stream, bin));
        }
    }
    let mut seed = 0x8082_0220_5eed_u64;
    for _ in 0..32 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let stream = seed;
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        coordinates.push((stream, seed));
    }
    coordinates
}

#[test]
fn standard_databin_class_assignment() {
    eprintln!("route: DatabinId::new -> class(); T.808 Table A.2, no extension");
    let defined = [
        (0, DatabinClass::Precinct),
        (1, DatabinClass::Precinct),
        (2, DatabinClass::TileHeader),
        (4, DatabinClass::TileData),
        (5, DatabinClass::TileData),
        (6, DatabinClass::MainHeader),
        (8, DatabinClass::Metadata),
    ];
    let mut forbidden = Vec::new();
    for (stream, bin) in contract_coordinates() {
        for (raw, expected) in defined {
            let identity = DatabinId::new(raw, stream, bin).unwrap();
            assert_eq!(
                identity.class(),
                expected,
                "class={raw}, CSn={stream}, bin={bin}"
            );
            assert_eq!(identity.class_identifier(), raw);
            assert_eq!(identity.in_class(), bin);
            if raw != 8 {
                assert_eq!(identity.codestream(), stream);
            }
        }
        for raw in [3, 7] {
            // Rejection OR an explicit unknown representation is acceptable.
            // Do not impose a raw-header rejection/error-code requirement.
            if let Ok(identity) = DatabinId::new(raw, stream, bin) {
                if matches!(
                    identity.class(),
                    DatabinClass::Precinct
                        | DatabinClass::TileHeader
                        | DatabinClass::TileData
                        | DatabinClass::MainHeader
                        | DatabinClass::Metadata
                ) {
                    forbidden.push((raw, stream, bin, identity.class()));
                }
            }
        }
    }
    assert!(
        forbidden.is_empty(),
        "{} reserved-class assignments received ordinary standard semantics; first examples: {:?}",
        forbidden.len(),
        &forbidden[..forbidden.len().min(12)]
    );
}

fn assert_semantic_aliases(
    first: DatabinId,
    second: DatabinId,
    separate: DatabinId,
    payload: &[u8],
    failures: &mut Vec<String>,
) {
    // Both paths describe the same bytes and final length. Only raw class/CSn
    // and fragment order/chunking may differ, not semantic lookup or content.
    for split in [0, 1, payload.len() / 2, payload.len() - 1, payload.len()] {
        for reverse in [false, true] {
            let mut reference = DatabinCache::new(ResourceLimits::default());
            reference.insert(fragment(first, 0, payload, true)).unwrap();
            let expected = (
                Some(payload.len() as u64),
                payload.len() as u64,
                payload.len() as u64,
                1,
            );
            let reference_status = reference.status(first).unwrap();
            assert_eq!(
                (
                    reference_status.final_length(),
                    reference_status.contiguous_prefix(),
                    reference_status.stored_bytes(),
                    reference_status.interval_count(),
                ),
                expected
            );
            assert_eq!(reference.databin_view(first).unwrap().as_bytes(), payload);
            let mut cache = DatabinCache::new(ResourceLimits::default());
            let mut fragments = Vec::new();
            if split != 0 {
                fragments.push(fragment(first, 0, &payload[..split], false));
            }
            fragments.push(fragment(second, split as u64, &payload[split..], true));
            if reverse {
                fragments.reverse();
            }
            for value in fragments {
                cache.insert(value).unwrap();
            }
            let context = format!("{first:?} / {second:?}, split={split}, reverse={reverse}");
            if cache.databin_count() != 1 {
                failures.push(format!(
                    "{context}: expected one semantic bin, got {}",
                    cache.databin_count()
                ));
            }
            for identity in [first, second] {
                let status = cache.status(identity);
                if status.map(|value| {
                    (
                        value.final_length(),
                        value.contiguous_prefix(),
                        value.stored_bytes(),
                        value.interval_count(),
                    )
                }) != Some(expected)
                {
                    failures.push(format!("{context}: wrong final length/prefix/bytes/intervals for {identity:?}: {status:?}"));
                }
                match cache.databin_view(identity) {
                    Ok(view) if view.as_bytes() == payload => {}
                    result => failures.push(format!(
                        "{context}: expected exact bytes {payload:?}, got {result:?}"
                    )),
                }
                let range = JpipDatabinRange::new(identity, 0, payload.len() as u64).unwrap();
                assert_eq!(
                    range.databin().class_identifier(),
                    identity.class_identifier()
                );
                let report = cache.availability(&[range]).unwrap();
                if report.status() != JpipAvailability::Available
                    || report.available_bytes() != payload.len() as u64
                    || !report.missing_ranges().is_empty()
                {
                    failures.push(format!("{context}: complete alias range reported missing"));
                }
            }
            assert!(
                cache.status(separate).is_none(),
                "{context}: distinct bin was aliased"
            );
            let before = cache.status(first);
            let accepted = cache.accepted_fragments();
            let conflict = cache.insert(fragment(second, 0, &[payload[0] ^ 0xff], false));
            if !matches!(
                conflict,
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::ConflictingJpipFragment,
                    ..
                })
            ) {
                failures.push(format!(
                    "{context}: conflicting alias bytes were not rejected: {conflict:?}"
                ));
            } else {
                assert_eq!(
                    cache.status(first),
                    before,
                    "{context}: rejection changed status"
                );
                assert_eq!(cache.accepted_fragments(), accepted);
            }
        }
    }
}

#[test]
fn metadata_databin_identity_ignores_codestream() {
    eprintln!(
        "route: DatabinId::new(8) -> JpipFragment::new -> DatabinCache::insert/status/view/availability"
    );
    // A complete XML box (LBox=12), not arbitrary malformed metadata.
    let payload = box32(*b"xml ", b"<a/>");
    assert_eq!(payload.len(), 12);
    assert_eq!(&payload[..4], &12_u32.to_be_bytes());
    assert_eq!(&payload[4..8], b"xml ");
    let mut failures = Vec::new();
    for (stream, bin) in contract_coordinates() {
        let first = DatabinId::new(8, stream, bin).unwrap();
        let second = DatabinId::new(8, stream ^ 1, bin).unwrap();
        let separate = DatabinId::new(8, stream, bin ^ 1).unwrap();
        assert_eq!(first.class(), DatabinClass::Metadata);
        assert_eq!(second.class(), DatabinClass::Metadata);
        assert_eq!(first.in_class(), bin);
        assert_eq!(second.in_class(), bin);
        assert_semantic_aliases(first, second, separate, &payload, &mut failures);

        let mut empty = DatabinCache::new(ResourceLimits::default());
        empty.insert(fragment(first, 0, b"", true)).unwrap();
        match empty.databin_view(second) {
            Ok(view) if view.is_empty() => {}
            result => failures.push(format!(
                "empty metadata alias {first:?}/{second:?}: {result:?}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} metadata semantic identity failures; first examples: {:#?}",
        failures.len(),
        &failures[..failures.len().min(12)]
    );
}

#[test]
fn ordinary_extended_databin_identity_and_raw_class() {
    eprintln!(
        "route: DatabinId::new(0/1 or 4/5) -> range -> cache semantic lookup; pairs tested in separate JPP/JPT caches"
    );
    let mut failures = Vec::new();
    for (ordinary, extended, expected) in [
        (0, 1, DatabinClass::Precinct),
        (4, 5, DatabinClass::TileData),
    ] {
        for (stream, bin) in contract_coordinates() {
            for (a, b) in [(ordinary, extended), (extended, ordinary)] {
                let first = DatabinId::new(a, stream, bin).unwrap();
                let second = DatabinId::new(b, stream, bin).unwrap();
                assert_eq!(first.class_identifier(), a);
                assert_eq!(second.class_identifier(), b);
                assert_eq!(first.class(), expected);
                assert_eq!(second.class(), expected);
                assert_eq!(first.codestream(), stream);
                assert_eq!(second.codestream(), stream);
                assert_eq!(first.in_class(), bin);
                assert_eq!(second.in_class(), bin);
                let separate = DatabinId::new(a, stream, bin ^ 1).unwrap();
                assert_semantic_aliases(first, second, separate, b"abcdef", &mut failures);
                let mut cache = DatabinCache::new(ResourceLimits::default());
                cache.insert(fragment(first, 0, b"", true)).unwrap();
                assert!(cache.databin_view(second).unwrap().is_empty());
                let other_stream = DatabinId::new(b, stream ^ 1, bin).unwrap();
                assert!(
                    cache.status(other_stream).is_none(),
                    "non-metadata CSn must distinguish bins"
                );
                let other_class = DatabinId::new(if a < 2 { 4 } else { 0 }, stream, bin).unwrap();
                assert!(
                    cache.status(other_class).is_none(),
                    "different data-bin classes must remain distinct"
                );
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} ordinary/extended semantic identity failures; first examples: {:#?}",
        failures.len(),
        &failures[..failures.len().min(12)]
    );
}
