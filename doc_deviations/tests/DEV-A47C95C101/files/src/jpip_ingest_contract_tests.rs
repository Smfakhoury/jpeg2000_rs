// Included in jpip::tests; the original regression tests remain unchanged.

fn contract_vbas(mut value: u64) -> Vec<u8> {
    let mut digits = vec![(value & 127) as u8];
    value >>= 7;
    while value != 0 {
        digits.push(((value & 127) as u8) | 128);
        value >>= 7;
    }
    digits.reverse();
    digits
}

fn contract_message(
    form: u8,
    target: DatabinId,
    offset: usize,
    body: &[u8],
    final_byte: bool,
) -> Vec<u8> {
    assert!((1..=3).contains(&form));
    assert!(matches!(target.class_identifier(), 0 | 1 | 6 | 8));
    assert!(offset + body.len() <= 4096);
    let mut bin = target.in_class();
    let mut rest = Vec::new();
    while bin > 15 {
        rest.push((bin & 127) as u8);
        bin >>= 7;
    }
    let mut bytes = vec![
        (form << 5)
            | (u8::from(final_byte) << 4)
            | bin as u8
            | if rest.is_empty() { 0 } else { 128 },
    ];
    rest.reverse();
    for (index, digit) in rest.iter().enumerate() {
        bytes.push(*digit | if index + 1 == rest.len() { 0 } else { 128 });
    }
    if form >= 2 {
        bytes.extend(contract_vbas(target.class_identifier()));
    }
    if form == 3 {
        bytes.extend(contract_vbas(target.codestream()));
    }
    bytes.extend(contract_vbas(offset as u64));
    bytes.extend(contract_vbas(body.len() as u64));
    if target.class_identifier() == 1 {
        // These precinct fixtures use one-byte empty packets, one per layer.
        bytes.extend(contract_vbas((offset + body.len()) as u64));
    }
    bytes.extend_from_slice(body);
    bytes
}

fn contract_ingest(cache: &mut DatabinCache, input: &[u8], messages: u64) {
    eprintln!(
        "route=ingest_jpip_response bytes={} expected_messages={messages}",
        input.len()
    );
    let result = ingest_jpip_response(input, cache);
    assert!(result.is_ok(), "input={input:02x?}, result={result:?}");
    let summary = result.unwrap();
    assert_eq!(summary.messages(), messages);
    assert_eq!(summary.end_of_response_reason(), None);
}

#[test]
fn contract_ingest_payload_reassembly() {
    reassembles_out_of_order_fragments();
    accepts_identical_duplicates_and_overlaps();
    rejects_conflicting_overlap_without_mutation();
    ingests_response_with_inherited_context();

    // Independent long headers may be reordered (T.808 A.2.1). Chunking,
    // order and identical retransmissions may change counts, but not bytes.
    let mut seed = 0x4a50_4950_u32;
    for length in [0, 1, 2, 15, 16, 127, 128, 255, 256, 1025] {
        let payload: Vec<u8> = (0..length)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 24) as u8
            })
            .collect();
        let target = id(8, 2048);
        let sentinel = id(8, 2047);
        let spans = [
            (length / 2, length, true),
            (0, length / 3, false),
            (0, length / 3, false),
            (length / 3, length / 2, false),
            (length / 4, length * 3 / 4, false),
        ];
        let mut results = Vec::new();
        for order in [[0, 1, 2, 3, 4], [4, 3, 2, 1, 0], [2, 0, 4, 1, 3]] {
            eprintln!("payload length={length}, order={order:?}");
            let mut cache = DatabinCache::new(ResourceLimits::default());
            contract_ingest(
                &mut cache,
                &contract_message(3, sentinel, 0, b"sentinel", true),
                1,
            );
            let mut seen = vec![false; length];
            let mut final_seen = false;
            let mut accepted = 1;
            for index in order {
                let (start, end, final_byte) = spans[index];
                // This profile admits an empty fragment only to declare EOF.
                if start == end && !final_byte {
                    continue;
                }
                let wire = contract_message(3, target, start, &payload[start..end], final_byte);
                contract_ingest(&mut cache, &wire, 1);
                accepted += 1;
                seen[start..end].fill(true);
                final_seen |= final_byte;
                let prefix = seen.iter().take_while(|&&present| present).count();
                let stored = seen.iter().filter(|&&present| present).count();
                let status = cache.status(target).expect("identified bin must exist");
                assert_eq!(status.final_length(), final_seen.then_some(length as u64));
                assert_eq!(status.stored_bytes(), stored as u64);
                assert_eq!(status.contiguous_prefix(), prefix as u64);
                let complete = final_seen && prefix == length;
                assert_eq!(cache.is_complete(target), complete);
                assert_eq!(cache.accepted_fragments(), accepted);
                assert_eq!(cache.stored_bytes(), stored as u64 + 8);
                assert_eq!(cache.databin_count(), 2);
                assert_eq!(
                    cache.databin_view(sentinel).unwrap().as_bytes(),
                    b"sentinel"
                );
                if complete {
                    assert_eq!(cache.databin_view(target).unwrap().as_bytes(), payload);
                } else {
                    assert!(matches!(
                        cache.databin_view(target),
                        Err(DecodeError::MalformedInput {
                            kind: MalformedKind::MissingRequiredStructure,
                            ..
                        })
                    ));
                }
            }
            assert!(cache.is_complete(target));
            results.push(cache.databin_view(target).unwrap());
        }
        assert_eq!(results[0], results[1]);
        assert_eq!(results[0], results[2]);
        let mut whole = DatabinCache::new(ResourceLimits::default());
        contract_ingest(
            &mut whole,
            &contract_message(3, target, 0, &payload, true),
            1,
        );
        assert_eq!(results[0], whole.databin_view(target).unwrap());
    }

    let target = id(8, 0);
    let mut cache = DatabinCache::new(ResourceLimits::default());
    contract_ingest(
        &mut cache,
        &contract_message(3, target, 0, b"abcd", true),
        1,
    );
    let before = cache.databin_view(target).unwrap();
    assert!(matches!(
        ingest_jpip_response(&contract_message(3, target, 2, b"XX", false), &mut cache),
        Err(DecodeError::MalformedInput {
            offset: 2,
            kind: MalformedKind::ConflictingJpipFragment
        })
    ));
    assert_eq!(cache.databin_view(target).unwrap(), before);
    assert_eq!(cache.accepted_fragments(), 1);
    assert_eq!(cache.stored_bytes(), 4);
    assert_eq!(cache.databin_count(), 1);
}

#[test]
fn contract_ingest_header_forms_response_local() {
    ingests_response_with_inherited_context();
    parses_all_three_standard_header_forms_and_auxiliary();
    let mut empty = DatabinCache::new(ResourceLimits::default());
    contract_ingest(&mut empty, &[], 0);
    assert_eq!(empty.databin_count(), 0);
    assert_eq!(empty.accepted_fragments(), 0);

    // Pin the independent test encoder against literal Annex A wire bytes.
    assert_eq!(
        contract_message(3, DatabinId::new(1, 1, 2).unwrap(), 0, &[0], true),
        [0x72, 1, 1, 0, 1, 1, 0]
    );
    assert_eq!(
        contract_message(1, id(0, 16), 0, &[0], true),
        [0xb0, 16, 0, 1, 0]
    );

    // Both streams are ordered. Changing only explicit/inherited encoding
    // must preserve bin identities, extents, payloads and message counts.
    for class in [0, 1, 8] {
        for bin in [0, 15, 16, 127, 128, 2047, 2048, u32::MAX as u64] {
            for stream in [0, 1, 127, 128, 16384, u32::MAX as u64] {
                let target = DatabinId::new(class, stream, bin).unwrap();
                eprintln!("header forms class={class} stream={stream} bin={bin}");
                let mut explicit = contract_message(3, target, 0, &[0], false);
                explicit.extend(contract_message(3, target, 1, &[0], true));
                let mut dependent = contract_message(3, target, 0, &[0], false);
                dependent.extend(contract_message(1, target, 1, &[0], true));
                // Class-only changes class while retaining the last CSn.
                let other = DatabinId::new(if class == 8 { 0 } else { 8 }, stream, bin).unwrap();
                explicit.extend(contract_message(3, other, 0, &[0], true));
                dependent.extend(contract_message(2, other, 0, &[0], true));
                let mut a = DatabinCache::new(ResourceLimits::default());
                let mut b = DatabinCache::new(ResourceLimits::default());
                contract_ingest(&mut a, &explicit, 3);
                contract_ingest(&mut b, &dependent, 3);
                for cache in [&a, &b] {
                    assert_eq!(cache.databin_count(), 2);
                    assert_eq!(cache.accepted_fragments(), 3);
                    assert_eq!(cache.stored_bytes(), 3);
                    assert_eq!(cache.databin_view(target).unwrap().as_bytes(), [0, 0]);
                    assert_eq!(cache.databin_view(other).unwrap().as_bytes(), [0]);
                }
                assert_eq!(
                    a.databin_view(target).unwrap(),
                    b.databin_view(target).unwrap()
                );
            }
        }
    }

    let mut cache = DatabinCache::new(ResourceLimits::default());
    for (wire, expected_kind, expected_offset) in [
        (&[0x10, 0, 0][..], MalformedKind::InvalidJpipHeader, 0),
        (&[0x60][..], MalformedKind::Truncated, 1),
        (&[0x60, 0, 0, 0, 2, 0][..], MalformedKind::Truncated, 5),
        (&[0, 1, 0, 0xff][..], MalformedKind::TrailingData, 3),
    ] {
        eprintln!("negative header wire={wire:02x?}");
        assert!(matches!(
            ingest_jpip_response(wire, &mut cache),
            Err(DecodeError::MalformedInput { offset, kind })
                if kind == expected_kind && offset == expected_offset
        ));
        assert_eq!(cache.databin_count(), 0);
        assert_eq!(cache.accepted_fragments(), 0);
    }
    let summary = ingest_jpip_response(&[0, 2, 3, b'e', b'o', b'r'], &mut cache).unwrap();
    assert_eq!(summary.messages(), 0);
    assert_eq!(summary.end_of_response_reason(), Some(2));
    assert_eq!(cache.databin_count(), 0);

    // Each new response starts with class/CSn defaults 0 (A.2.1), NOT
    // the preceding response's class/CSn, even with the same cache.
    let previous = DatabinId::new(8, 127, 0).unwrap();
    contract_ingest(
        &mut cache,
        &contract_message(3, previous, 0, b"old", true),
        1,
    );
    contract_ingest(&mut cache, &[0x50, 8, 0, 1, b'n'], 1);
    assert_eq!(cache.databin_view(id(8, 0)).unwrap().as_bytes(), b"n");
    let initial_dependent = [0x30, 0, 1, 0]; // precinct 0, one empty packet, final
    eprintln!("A.2.1 initial defaults: wire={initial_dependent:02x?}, expected class=0 CSn=0");
    contract_ingest(&mut cache, &initial_dependent, 1);
    assert_eq!(cache.databin_view(id(0, 0)).unwrap().as_bytes(), [0]);
    assert_eq!(cache.databin_view(previous).unwrap().as_bytes(), b"old");
    assert_eq!(cache.databin_count(), 3);
    assert_eq!(cache.accepted_fragments(), 3);
    assert_eq!(cache.stored_bytes(), 5);
}
