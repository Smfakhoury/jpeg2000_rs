// Included in jpip::tests so the original regression fixtures remain in use.
fn availability_contract_oracle(
    cache: &DatabinCache,
    ranges: &[JpipDatabinRange],
    present: &[bool],
    target: DatabinId,
) {
    let before = (
        cache.stored_bytes(),
        cache.accepted_fragments(),
        cache.databin_count(),
        cache.status(target),
    );
    let mut required = 0;
    let mut available = 0;
    let mut missing = Vec::new();
    for range in ranges {
        required += range.length();
        let mut cursor = range.offset();
        let end = cursor + range.length();
        while cursor < end {
            let has_byte = |position: u64| {
                range.databin().class_identifier() / 2 == target.class_identifier() / 2
                    && range.databin().codestream() == target.codestream()
                    && range.databin().in_class() == target.in_class()
                    && present.get(position as usize).copied().unwrap_or(false)
            };
            if has_byte(cursor) {
                available += 1;
                cursor += 1;
            } else {
                let start = cursor;
                while cursor < end && !has_byte(cursor) {
                    cursor += 1;
                }
                missing.push(JpipMissingRange {
                    databin: range.databin(),
                    offset: start,
                    length: Some(cursor - start),
                });
            }
        }
    }
    let status = if available == required {
        JpipAvailability::Available
    } else if available == 0 {
        JpipAvailability::Missing
    } else {
        JpipAvailability::Partial
    };
    let expected = JpipAvailabilityReport {
        status,
        required_bytes: required,
        available_bytes: available,
        missing,
    };
    assert_eq!(
        cache.availability(ranges).unwrap(),
        expected,
        "class={} ranges={ranges:?} bitmap={present:?}",
        target.class_identifier(),
    );
    assert_eq!(
        cache.availability(ranges).unwrap(),
        expected,
        "repeat query"
    );
    assert_eq!(
        (
            cache.stored_bytes(),
            cache.accepted_fragments(),
            cache.databin_count(),
            cache.status(target),
        ),
        before,
        "availability must be observational",
    );
}

fn availability_contract_sweep(cache: &DatabinCache, target: DatabinId, present: &[bool]) {
    availability_contract_oracle(cache, &[], present, target);
    for start in 0..=33 {
        for end in start + 1..=34 {
            availability_contract_oracle(
                cache,
                &[JpipDatabinRange::new(target, start, end - start).unwrap()],
                present,
                target,
            );
        }
    }
    let absent = DatabinId::new(target.class_identifier(), 0, 99).unwrap();
    let other_stream = DatabinId::new(target.class_identifier(), 1, 0).unwrap();
    let mut ranges = vec![
        JpipDatabinRange::new(target, 0, 32).unwrap(),
        JpipDatabinRange::new(absent, 2, 4).unwrap(),
        JpipDatabinRange::new(target, 7, 9).unwrap(),
        JpipDatabinRange::new(target, 7, 9).unwrap(),
        JpipDatabinRange::new(id((target.class_identifier() + 2) % 8, 0), 0, 3).unwrap(),
    ];
    // Image data-bins remain codestream-scoped; metadata is tested separately.
    if target.class_identifier() != 8 {
        ranges.push(JpipDatabinRange::new(other_stream, 0, 32).unwrap());
        ranges.push(JpipDatabinRange::new(id(target.class_identifier() ^ 1, 0), 0, 32).unwrap());
    }
    availability_contract_oracle(cache, &ranges, present, target);
}

#[test]
fn availability_contract_bitmap_and_reassembly() {
    reports_exact_missing_ranges_for_incomplete_databins();
    reassembles_out_of_order_fragments();
    accepts_identical_duplicates_and_overlaps();
    rejects_conflicting_overlap_without_mutation();
    rejects_conflicting_final_lengths_and_bytes_past_final();

    let bytes: Vec<u8> = (0..32).map(|i| (i * 37 + 11) as u8).collect();
    let mut seed = 0x4a50_4950_u64;
    for class in 0..=8 {
        eprintln!("availability bitmap route: class={class}, seed={seed:#x}");
        let target = id(class, 0);
        let mut cache = DatabinCache::new(ResourceLimits::default());
        let mut present = [false; 32];
        availability_contract_sweep(&cache, target, &present);
        for step in 0..24 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let start = (seed >> 32) as usize % 32;
            let length = 1 + (seed as usize % 7).min(31 - start);
            let end = start + length;
            cache
                .insert(fragment(target, start as u64, &bytes[start..end], false))
                .unwrap();
            present[start..end].fill(true);
            availability_contract_sweep(&cache, target, &present);
            assert!(
                matches!(
                    cache.databin_view(target),
                    Err(DecodeError::MalformedInput {
                        kind: MalformedKind::MissingRequiredStructure,
                        ..
                    })
                ),
                "unannounced final length, class={class} step={step}"
            );
        }
        cache.insert(fragment(target, 0, &bytes, false)).unwrap();
        present.fill(true);
        availability_contract_sweep(&cache, target, &present);
        assert!(
            !cache.is_complete(target),
            "bytes alone do not announce finality"
        );
        cache.insert(fragment(target, 32, &[], true)).unwrap();
        availability_contract_sweep(&cache, target, &present);
        assert!(cache.is_complete(target));
        assert_eq!(cache.databin_view(target).unwrap().as_bytes(), bytes);

        let mut reverse = DatabinCache::new(ResourceLimits::default());
        for start in (0..32).rev() {
            reverse
                .insert(fragment(
                    target,
                    start as u64,
                    &bytes[start..start + 1],
                    start == 31,
                ))
                .unwrap();
        }
        assert_eq!(reverse.databin_view(target).unwrap().as_bytes(), bytes);
        availability_contract_sweep(&reverse, target, &present);
        let mut empty = DatabinCache::new(ResourceLimits::default());
        empty.insert(fragment(target, 0, &[], true)).unwrap();
        assert!(empty.is_complete(target));
        assert!(empty.databin_view(target).unwrap().is_empty());
        availability_contract_sweep(&empty, target, &[false; 32]);
    }
}

fn availability_contract_limit(
    result: Result<JpipAvailabilityReport, DecodeError>,
    resource: Resource,
    limit: u64,
    actual: u64,
) {
    assert_eq!(
        result,
        Err(DecodeError::LimitExceeded {
            resource,
            limit,
            actual
        })
    );
}

#[test]
fn availability_contract_resource_boundaries() {
    let target = id(0, 0);
    let range = JpipDatabinRange::new(target, 0, 1).unwrap();
    for cap in 0..=2 {
        let cache = DatabinCache::new(ResourceLimits::default().with_maximum_packets(cap).unwrap());
        availability_contract_oracle(&cache, &vec![range; cap as usize], &[], target);
        availability_contract_limit(
            cache.availability(&vec![range; cap as usize + 1]),
            Resource::Packets,
            cap,
            cap + 1,
        );
    }
    let limits = ResourceLimits::default()
        .with_maximum_input_bytes(16)
        .unwrap();
    let mut cache = DatabinCache::new(limits);
    cache
        .insert(fragment(target, 0, b"abcdefghijklmnop", true))
        .unwrap();
    let all = JpipDatabinRange::new(target, 0, 16).unwrap();
    availability_contract_oracle(&cache, &[all], &[true; 16], target);
    availability_contract_limit(
        cache.availability(&[JpipDatabinRange::new(target, 16, 1).unwrap()]),
        Resource::InputBytes,
        16,
        17,
    );
    availability_contract_limit(
        cache.availability(&[all, range]),
        Resource::InputBytes,
        16,
        17,
    );
    availability_contract_oracle(&cache, &[all], &[true; 16], target);
    assert_eq!(
        cache.databin_view(target).unwrap().as_bytes(),
        b"abcdefghijklmnop"
    );

    for cap in 0..=4 {
        let limits = ResourceLimits::default()
            .with_maximum_work_units(cap)
            .unwrap();
        let mut cache = DatabinCache::new(limits);
        cache.insert(fragment(target, 0, b"a", false)).unwrap();
        cache.insert(fragment(target, 2, b"c", false)).unwrap();
        cache.insert(fragment(target, 4, b"e", true)).unwrap();
        let ranges = [JpipDatabinRange::new(target, 0, 5).unwrap()];
        eprintln!("availability work-limit route: three disjoint intervals, cap={cap}");
        if cap < 3 {
            availability_contract_limit(
                cache.availability(&ranges),
                Resource::WorkUnits,
                cap,
                cap + 1,
            );
        } else {
            availability_contract_oracle(
                &cache,
                &ranges,
                &[true, false, true, false, true],
                target,
            );
        }
        assert_eq!(cache.stored_bytes(), 3);
        assert_eq!(cache.accepted_fragments(), 3);
        assert_eq!(cache.status(target).unwrap().interval_count(), 3);
    }
}

#[test]
fn availability_contract_metadata_ignores_codestream() {
    // T.808 (2022), A.3.6.1: one logical target, metadata root bin 0.
    // A complete 16-byte 'free' box; no placeholders or codestream references.
    let metadata = b"\0\0\0\x10freeabcdefgh";
    assert_eq!(u32::from_be_bytes(metadata[..4].try_into().unwrap()), 16);
    assert_eq!(&metadata[4..8], b"free");
    let target = id(8, 0);
    let mut reports = Vec::new();
    for second_stream in [0_u8, 1] {
        let limits = ResourceLimits::default();
        let mut cache = DatabinCache::new(limits);
        let mut context = JpipHeaderContext::new();
        for (offset, stream) in [(0_u8, 0), (8, second_stream)] {
            let final_byte = offset == 8;
            // Explicit class and CSn, in-class ID 0, no auxiliary for class 8.
            let mut wire = vec![if final_byte { 0x70 } else { 0x60 }, 8, stream, offset, 8];
            wire.extend_from_slice(&metadata[offset as usize..offset as usize + 8]);
            let (message, used) = parse_jpip_message(&wire, &mut context, &limits).unwrap();
            assert_eq!(used, wire.len());
            let JpipMessage::Data { header, body } = message else {
                panic!("metadata data message required");
            };
            assert_eq!(header.databin().class(), DatabinClass::Metadata);
            assert_eq!(header.databin().in_class(), 0);
            assert_eq!(header.offset(), u64::from(offset));
            assert_eq!(header.length(), 8);
            assert_eq!(header.auxiliary(), None);
            assert_eq!(header.is_last_byte(), final_byte);
            cache
                .insert(JpipFragment::from_message(header, body, &limits).unwrap())
                .unwrap();
            if offset == 0 {
                availability_contract_oracle(
                    &cache,
                    &[JpipDatabinRange::new(target, 0, 16).unwrap()],
                    &[true, true, true, true, true, true, true, true],
                    target,
                );
            }
        }
        let report = cache
            .availability(&[JpipDatabinRange::new(target, 0, 16).unwrap()])
            .unwrap();
        eprintln!(
            "parse -> from_message -> insert -> availability: same logical target, class=8 bin=0 CSn=[0,{second_stream}], report={report:?}"
        );
        reports.push(report);
    }
    let expected = JpipAvailabilityReport {
        status: JpipAvailability::Available,
        required_bytes: 16,
        available_bytes: 16,
        missing: Vec::new(),
    };
    assert_eq!(reports[0], expected, "same-CSn control");
    assert_eq!(
        reports[1], expected,
        "metadata CSn must not partition availability"
    );
    assert_eq!(
        reports[0], reports[1],
        "only raw CSn may differ, not availability"
    );
}
