// Wire fixtures follow T.808 (12/2022), A.2.1--A.2.3 and D.3.
// These are de-framed, ordered JPP message sequences, not HTTP-UDP packets.
// Payloads are opaque to this byte-ingestion contract; image decoding is not tested.

fn contract_vbas(mut value: u64) -> Vec<u8> {
    let mut bytes = vec![(value & 127) as u8];
    value >>= 7;
    while value != 0 {
        bytes.push(((value & 127) as u8) | 128);
        value >>= 7;
    }
    bytes.reverse();
    bytes
}

fn contract_data(form: u8, target: DatabinId, offset: usize, body: &[u8], last: bool) -> Vec<u8> {
    assert!((1..=3).contains(&form));
    assert!([0, 8].contains(&target.class_identifier()));
    assert!(offset + body.len() <= 32768);
    let mut tail = Vec::new();
    let mut bin = target.in_class();
    while bin > 15 {
        tail.push((bin & 127) as u8);
        bin >>= 7;
    }
    let mut bytes = vec![
        (form << 5) | if last { 16 } else { 0 } | bin as u8 | if tail.is_empty() { 0 } else { 128 },
    ];
    tail.reverse();
    for (index, byte) in tail.iter().enumerate() {
        bytes.push(*byte | if index + 1 < tail.len() { 128 } else { 0 });
    }
    if form >= 2 {
        bytes.extend(contract_vbas(target.class_identifier()));
    }
    if form == 3 {
        bytes.extend(contract_vbas(target.codestream()));
    }
    bytes.extend(contract_vbas(offset as u64));
    bytes.extend(contract_vbas(body.len() as u64));
    bytes.extend_from_slice(body);
    bytes
}

fn contract_eor(reason: u8, body: &[u8]) -> Vec<u8> {
    assert!([1, 2, 3, 4, 5, 6, 7, 255].contains(&reason));
    let mut bytes = vec![0, reason];
    bytes.extend(contract_vbas(body.len() as u64));
    bytes.extend_from_slice(body);
    bytes
}

fn contract_ingest(cache: &mut DatabinCache, wire: &[u8], messages: u64, reason: u8) {
    eprintln!(
        "unit route ingest_jpip_response -> parser -> cache: wire={} bytes, data={messages}, EOR={reason}",
        wire.len()
    );
    let summary = ingest_jpip_response(wire, cache)
        .unwrap_or_else(|error| panic!("valid de-framed message sequence rejected: {error:?}"));
    assert_eq!(summary.messages(), messages);
    assert_eq!(summary.end_of_response_reason(), Some(reason));
}

fn contract_assert_bytes(cache: &DatabinCache, target: DatabinId, expected: &[u8]) {
    assert_eq!(
        cache.databin_view(target).unwrap().as_bytes(),
        expected,
        "payload at addressed databin {target:?}"
    );
    let status = cache.status(target).unwrap();
    assert_eq!(status.final_length(), Some(expected.len() as u64));
    assert_eq!(status.contiguous_prefix(), expected.len() as u64);
    assert_eq!(status.stored_bytes(), expected.len() as u64);
    assert!(status.is_complete());
}

#[test]
fn contract_ingest_standard_headers_and_response_local_defaults() {
    ingests_response_with_inherited_context();
    // Independent encoder checks include the first-byte four-bit Bin-ID boundary.
    assert_eq!(
        contract_data(3, DatabinId::new(8, 128, 16).unwrap(), 127, b"x", true),
        [0xf0, 0x10, 8, 0x81, 0, 127, 1, b'x']
    );
    for stream in [0, 1, 127, 128, 16384] {
        for bin in [0, 1, 15, 16, 127, 128, 2047, 2048, 16384] {
            let precinct = DatabinId::new(0, stream, bin).unwrap();
            let inherited = DatabinId::new(0, stream, bin + 1).unwrap();
            let metadata = DatabinId::new(8, stream, bin).unwrap();
            let metadata_next = DatabinId::new(8, stream, bin + 1).unwrap();
            let mut wire = contract_data(3, precinct, 0, b"a", true);
            wire.extend(contract_data(1, inherited, 0, b"bc", true));
            wire.extend(contract_data(2, metadata, 0, b"def", true));
            wire.extend(contract_data(1, metadata_next, 0, b"ghij", true));
            wire.extend(contract_eor(2, b""));
            let mut cache = DatabinCache::new(ResourceLimits::default());
            contract_ingest(&mut cache, &wire, 4, 2);
            for (target, bytes) in [
                (precinct, &b"a"[..]),
                (inherited, &b"bc"[..]),
                (metadata, &b"def"[..]),
                (metadata_next, &b"ghij"[..]),
            ] {
                contract_assert_bytes(&cache, target, bytes);
            }
            assert_eq!(cache.databin_count(), 4);
            assert_eq!(cache.stored_bytes(), 10);
        }
    }
    let mut failures = Vec::new();
    for primed in [false, true] {
        for bin in [0, 15, 16, 127, 128, 2048] {
            let mut cache = DatabinCache::new(ResourceLimits::default());
            if primed {
                let old = DatabinId::new(8, 128, 42).unwrap();
                let mut wire = contract_data(3, old, 0, b"old", true);
                wire.extend(contract_eor(2, b""));
                contract_ingest(&mut cache, &wire, 1, 2);
            }
            // A fresh invocation must not inherit the preceding response's CSn.
            let metadata = DatabinId::new(8, 0, bin).unwrap();
            let mut wire = contract_data(2, metadata, 0, b"m", true);
            wire.extend(contract_eor(2, b""));
            contract_ingest(&mut cache, &wire, 1, 2);
            contract_assert_bytes(&cache, metadata, b"m");

            // A.2.1 defaults BOTH omitted identifiers to zero when no previous
            // message exists, even when the shared cache already contains data.
            let target = DatabinId::new(0, 0, bin).unwrap();
            let mut wire = contract_data(1, target, 0, b"p", true);
            wire.extend(contract_eor(2, b""));
            eprintln!("first form-1 header: primed={primed}, bin={bin}, bytes={wire:?}");
            match ingest_jpip_response(&wire, &mut cache) {
                Ok(summary) => {
                    assert_eq!(summary.messages(), 1);
                    assert_eq!(summary.end_of_response_reason(), Some(2));
                    contract_assert_bytes(&cache, target, b"p");
                    assert_eq!(cache.databin_count(), if primed { 3 } else { 2 });
                    assert_eq!(cache.stored_bytes(), if primed { 5 } else { 2 });
                }
                Err(error) => failures.push(format!("primed={primed}, bin={bin}: {error:?}")),
            }
        }
    }
    // 00 is prohibited by Table A.1, rather than a dependent-header form.
    let mut cache = DatabinCache::new(ResourceLimits::default());
    assert!(matches!(
        ingest_jpip_response(&[0x10, 0, 1, b'x'], &mut cache),
        Err(DecodeError::MalformedInput {
            kind: MalformedKind::InvalidJpipHeader,
            ..
        })
    ));
    assert_eq!(cache.databin_count(), 0);
    assert_eq!(cache.stored_bytes(), 0);
    assert!(
        failures.is_empty(),
        "T.808 A.2.1 initial inherited-header defaults violated:\n{}",
        failures.join("\n")
    );
}

#[test]
fn contract_ingest_eor_reason_extent_and_no_cache_body() {
    parses_end_of_response();
    let target = id(8, 1);
    let decoy = id(8, 2);
    let embedded_message = contract_data(3, decoy, 0, b"not data", true);
    for reason in [1, 2, 3, 4, 5, 6, 7, 255] {
        for length in [0, 1, 2, 127, 128, 129, 255, 256, 16383, 16384] {
            let body: Vec<u8> = (0..length)
                .map(|index| embedded_message[index % embedded_message.len()])
                .collect();
            for with_data in [false, true] {
                let mut cache = DatabinCache::new(ResourceLimits::default());
                let mut wire = if with_data {
                    contract_data(3, target, 0, b"kept", true)
                } else {
                    Vec::new()
                };
                wire.extend(contract_eor(reason, &body));
                contract_ingest(&mut cache, &wire, u64::from(with_data), reason);
                assert_eq!(cache.databin_count(), usize::from(with_data));
                assert_eq!(cache.stored_bytes(), if with_data { 4 } else { 0 });
                assert_eq!(cache.accepted_fragments(), u64::from(with_data));
                assert!(cache.status(decoy).is_none(), "EOR body parsed as Data");
                if with_data {
                    contract_assert_bytes(&cache, target, b"kept");
                }
            }
        }
    }
    for length in [0, 1, 127, 128] {
        let mut wire = contract_data(3, target, 0, b"kept", true);
        wire.extend(contract_eor(2, &vec![0x72; length]));
        let response_end = wire.len();
        wire.extend(contract_data(3, decoy, 0, b"forbidden", true));
        let mut cache = DatabinCache::new(ResourceLimits::default());
        eprintln!("EOR termination boundary={response_end}, trailing valid Data");
        assert!(matches!(
            ingest_jpip_response(&wire, &mut cache),
            Err(DecodeError::MalformedInput {
                offset,
                kind: MalformedKind::TrailingData,
            }) if offset == response_end as u64
        ));
        contract_assert_bytes(&cache, target, b"kept");
        assert!(cache.status(decoy).is_none());
        assert_eq!(cache.stored_bytes(), 4);
        assert_eq!(cache.accepted_fragments(), 1);
    }
    for wire in [
        &[0][..],
        &[0, 2][..],
        &[0, 2, 0x81][..],
        &[0, 2, 1][..],
        &[0, 2, 0x81, 0][..],
    ] {
        let mut cache = DatabinCache::new(ResourceLimits::default());
        let mut seed = contract_data(3, target, 0, b"kept", true);
        seed.extend(contract_eor(2, b""));
        contract_ingest(&mut cache, &seed, 1, 2);
        eprintln!("truncated EOR fixture={wire:?}");
        assert!(matches!(
            ingest_jpip_response(wire, &mut cache),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::Truncated,
                ..
            })
        ));
        contract_assert_bytes(&cache, target, b"kept");
        assert_eq!(cache.databin_count(), 1);
        assert_eq!(cache.stored_bytes(), 4);
        assert_eq!(cache.accepted_fragments(), 1);
    }
}

#[test]
fn contract_ingest_payload_offsets_and_chunking() {
    let mut random = 0x1b87_39a5_u32;
    for class in [0, 8] {
        for stream in [0, 1, 127, 128] {
            for length in [0, 1, 2, 15, 16, 127, 128, 129, 257, 16384] {
                let expected: Vec<u8> = (0..length)
                    .map(|_| {
                        random = random.wrapping_mul(1664525).wrapping_add(1013904223);
                        (random >> 24) as u8
                    })
                    .collect();
                let target = DatabinId::new(class, stream, 16).unwrap();
                let mut whole = DatabinCache::new(ResourceLimits::default());
                let mut wire = contract_data(3, target, 0, &expected, true);
                wire.extend(contract_eor(2, b"excluded"));
                contract_ingest(&mut whole, &wire, 1, 2);
                contract_assert_bytes(&whole, target, &expected);

                let mut split = DatabinCache::new(ResourceLimits::default());
                let midpoint = length / 2;
                let mut high = contract_data(3, target, midpoint, &expected[midpoint..], true);
                high.extend(contract_eor(2, b"excluded"));
                contract_ingest(&mut split, &high, 1, 2);
                let status = split.status(target).unwrap();
                assert_eq!(status.final_length(), Some(length as u64));
                assert_eq!(status.stored_bytes(), (length - midpoint) as u64);
                assert_eq!(
                    status.contiguous_prefix(),
                    if midpoint == 0 { length as u64 } else { 0 }
                );
                // Observe actual stored offsets/bytes before filling the hole;
                // these are passive test-module reads, not private-state writes.
                assert_eq!(split.databins.len(), 1);
                assert_eq!(split.databins[0].fragments.len(), 1);
                assert_eq!(split.databins[0].fragments[0].offset, midpoint as u64);
                assert_eq!(split.databins[0].fragments[0].bytes, expected[midpoint..]);
                let overlap_end = (midpoint + 1).min(length);
                let mut low = contract_data(3, target, 0, &expected[..overlap_end], length == 0);
                low.extend(contract_data(
                    1,
                    target,
                    0,
                    &expected[..overlap_end],
                    length == 0,
                ));
                low.extend(contract_eor(2, b"excluded too"));
                contract_ingest(&mut split, &low, 2, 2);
                contract_assert_bytes(&split, target, &expected);
                // Chunking and compatible duplicates may change fragment counts,
                // but not retained byte values, offsets, final extent, or size.
                assert_eq!(
                    whole.databin_view(target).unwrap(),
                    split.databin_view(target).unwrap()
                );
                assert_eq!(whole.status(target), split.status(target));
                assert_eq!(whole.stored_bytes(), length as u64);
                assert_eq!(split.stored_bytes(), length as u64);
                assert_eq!(whole.databin_count(), 1);
                assert_eq!(split.databin_count(), 1);
                assert_eq!(whole.accepted_fragments(), 1);
                assert_eq!(split.accepted_fragments(), 3);
            }
        }
    }
}
