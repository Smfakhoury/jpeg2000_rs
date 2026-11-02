// Included in jpip::tests; all state changes below use the public cache API.
fn contract_insert(
    cache: &mut DatabinCache,
    class: u64,
    stream: u64,
    tile: u16,
    offset: usize,
    bytes: &[u8],
    final_byte: bool,
) {
    cache
        .insert(fragment(
            DatabinId::new(class, stream, u64::from(tile)).unwrap(),
            offset as u64,
            bytes,
            final_byte,
        ))
        .unwrap();
}

fn contract_part(
    tile: u16,
    index: u8,
    total: u8,
    payload: Option<u32>,
    plt: bool,
    functional: bool,
) -> JpipTilePartTransition {
    let mut markers = Vec::new();
    if plt {
        // Three lengths, one encoded with two bytes. Zplt is not a packet.
        markers.extend(segment(0xff58, &[0, 1, 0x81, 0, 2]));
        markers.extend(segment(0xff58, &[1, 3]));
    }
    markers.extend(segment(0xff64, &[0, 1, b'o', b'k']));
    if functional {
        markers.extend(segment(0xff52, &[0, 0, 0, 1, 0, 0, 0, 0, 0, 1]));
    }
    let length = 14 + u32::try_from(markers.len()).unwrap();
    let mut header = vec![0xff, 0x90, 0, 10];
    header.extend(tile.to_be_bytes());
    header.extend(payload.map_or(0, |data| length + data).to_be_bytes());
    header.extend([index, total]);
    header.extend(markers);
    header.extend([0xff, 0x93]);
    JpipTilePartTransition {
        tile,
        tile_part: index,
        packet_count: if plt { Some(4) } else { None },
        data_length: payload.map(u64::from),
        functional,
        header,
    }
}

fn contract_bytes(parts: &[JpipTilePartTransition]) -> Vec<u8> {
    parts.iter().flat_map(|part| part.header.clone()).collect()
}

fn contract_available(parts: Vec<JpipTilePartTransition>) -> JpipTilePartTransitionReport {
    JpipTilePartTransitionReport {
        status: JpipAvailability::Available,
        transitions: parts,
        needs: Vec::new(),
        missing: Vec::new(),
    }
}

fn contract_cache(bytes: &[u8], stream: u64, tile: u16) -> DatabinCache {
    let mut cache = DatabinCache::new(ResourceLimits::default());
    contract_insert(&mut cache, 2, stream, tile, 0, bytes, true);
    assert!(cache.is_complete(DatabinId::new(2, stream, u64::from(tile)).unwrap()));
    cache
}

fn contract_random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed
}

#[test]
fn contract_missing_completion_suffix() {
    let mut seed = 0x67a8_9bc0;
    for iteration in 0..40 {
        let tile = match iteration {
            0 => 0,
            1 => u16::MAX,
            _ => contract_random(&mut seed) as u16,
        };
        let stream = if iteration == 1 { u64::MAX } else { contract_random(&mut seed) };
        let target = DatabinId::new(2, stream, u64::from(tile)).unwrap();
        let bytes = [0xff; 24]; // Invalid complete syntax must not be parsed while incomplete.
        for class in [2, 3] {
            for mode in 0..5 {
                let mut cache = DatabinCache::new(ResourceLimits::default());
                // Same tile in other streams/classes and other tile in this stream.
                contract_insert(&mut cache, 2, stream ^ 1, tile, 0, &[], true);
                contract_insert(&mut cache, 4, stream, tile, 0, &[], true);
                contract_insert(&mut cache, 2, stream, tile.wrapping_add(1), 0, &[], true);
                let prefix = if mode == 0 { 0 } else { (iteration % 8) as usize };
                let (expected_prefix, expected_length) = match mode {
                    0 => (0, None),
                    1 => {
                        if prefix != 0 {
                            contract_insert(&mut cache, class, stream, tile, 0, &bytes[..prefix], false);
                        }
                        (prefix, None)
                    }
                    2 => {
                        if prefix != 0 {
                            contract_insert(&mut cache, class, stream, tile, 0, &bytes[..prefix], false);
                        }
                        contract_insert(&mut cache, class, stream, tile, 20, &bytes[20..], true);
                        (prefix, Some(24 - prefix))
                    }
                    3 => {
                        contract_insert(&mut cache, class, stream, tile, 12, &bytes[12..16], false);
                        contract_insert(&mut cache, class, stream, tile, 22, &bytes[22..], true);
                        (0, Some(24))
                    }
                    _ => {
                        // Fully contiguous but no final-length announcement.
                        contract_insert(&mut cache, class, stream, tile, 0, &bytes, false);
                        (24, None)
                    }
                };
                assert!(!cache.is_complete(target));
                eprintln!("missing: stream={stream} tile={tile} raw_class={class} mode={mode}");
                let report = cache.tile_part_transitions(stream, tile).unwrap();
                assert_eq!(
                    report,
                    JpipTilePartTransitionReport {
                        status: JpipAvailability::Missing,
                        transitions: Vec::new(),
                        needs: vec![JpipSemanticNeed {
                            kind: JpipSemanticNeedKind::TilePartTransition,
                            identifier: u64::from(tile),
                        }],
                        missing: vec![JpipMissingRange {
                            databin: target,
                            offset: expected_prefix as u64,
                            length: expected_length.map(|length| length as u64),
                        }],
                    }
                );
            }
        }
    }
}

#[test]
fn contract_boundary_availability() {
    // Retain and execute the existing regression's original fixture combinations.
    tile_part_transition_status_distinguishes_sufficient_packet_boundaries();
    let mut seed = 0x1234_5678;
    for tile in [0, 1, 257, u16::MAX] {
        for count in [1_u8, 2, 3, 17, u8::MAX] {
            for variant in 0..8 {
                let parts: Vec<_> = (0..count)
                    .map(|index| {
                        let final_zero = variant & 1 != 0 && index == count - 1;
                        let data = match index % 3 {
                            0 => 0,
                            1 => 1,
                            _ => contract_random(&mut seed) as u32 % 10000,
                        };
                        contract_part(
                            tile,
                            index,
                            count,
                            if final_zero { None } else { Some(data) },
                            variant & 2 != 0,
                            variant & 4 != 0,
                        )
                    })
                    .collect();
                let cache = contract_cache(&contract_bytes(&parts), 9, tile);
                eprintln!("availability: tile={tile} count={count} variant={variant}");
                let report = cache.tile_part_transitions(9, tile).unwrap();
                assert_eq!(report, contract_available(parts));
            }
        }
        for bytes in [
            Vec::new(),
            segment(0xff64, &[0, 1, b'x']),
            segment(0xff52, &[0, 0, 0, 1, 0, 0, 0, 0, 0, 1]),
        ] {
            let cache = contract_cache(&bytes, 9, tile);
            let report = cache.tile_part_transitions(9, tile).unwrap();
            assert_eq!(report.status(), JpipAvailability::Available);
            assert!(report.semantic_needs().is_empty());
            assert_eq!(report.transitions().len(), 1);
            assert!(report.missing_ranges().is_empty());
        }
    }
    // Under the legal-header premise every nonfinal Psot supplies a known
    // length. Thus no successful Partial state is reachable in that domain;
    // asserting Available also forbids spurious/out-of-order transition needs.
}

#[test]
fn contract_exact_parse_and_errors() {
    let mut seed = 0xdead_beef;
    for iteration in 0..64 {
        let tile = match iteration {
            0 => 0,
            1 => u16::MAX,
            _ => contract_random(&mut seed) as u16,
        };
        let count = (iteration % 7 + 1) as u8;
        let parts: Vec<_> = (0..count)
            .map(|index| contract_part(
                tile, index, count,
                if index == count - 1 && iteration % 2 == 0 { None }
                else { Some([0, 1, 255, u32::MAX - 1024][iteration % 4]) },
                index % 2 == 0, index % 3 == 0,
            ))
            .collect();
        let bytes = contract_bytes(&parts);
        let cache = contract_cache(&bytes, 77, tile);
        eprintln!("exact parse: iteration={iteration} tile={tile} parts={count}");
        let report = cache.tile_part_transitions(77, tile).unwrap();
        assert_eq!(report.transitions(), parts);
        assert!(report.missing_ranges().is_empty());
        assert_eq!(contract_bytes(report.transitions()), bytes);
    }
    for (bytes, packets, functional) in [
        (Vec::new(), None, false),
        (segment(0xff64, &[0, 1, b'x']), None, false),
        (segment(0xff58, &[0, 1, 0x81, 0]), Some(2), false),
        (segment(0xff52, &[0, 0, 0, 1, 0, 0, 0, 0, 0, 1]), None, true),
        (vec![0xff, 0x93], None, false),
    ] {
        let cache = contract_cache(&bytes, 3, u16::MAX);
        assert_eq!(cache.tile_part_transitions(3, u16::MAX).unwrap(),
            contract_available(vec![JpipTilePartTransition {
                tile: u16::MAX, tile_part: 0, packet_count: packets,
                data_length: None, functional, header: bytes,
            }]));
    }
    let valid = contract_part(7, 0, 2, Some(0), false, false).header;
    let mut failures: Vec<(Vec<u8>, u64, MalformedKind)> = vec![
        (vec![0xff], 0, MalformedKind::Truncated),
        (vec![0xff, 0x64, 0, 1], 0, MalformedKind::InvalidMarkerLength),
        (contract_part(8, 0, 1, Some(0), false, false).header, 0, MalformedKind::InvalidTilePart),
        (valid[..12].to_vec(), 12, MalformedKind::MissingRequiredStructure),
    ];
    let mut zero_before_end = contract_part(7, 0, 2, None, false, false).header;
    zero_before_end.extend(contract_part(7, 1, 2, Some(0), false, false).header);
    failures.push((zero_before_end, 0, MalformedKind::InvalidTilePart));
    let mut bad_plt = contract_part(7, 0, 1, Some(0), true, false).header;
    bad_plt[16] = 1;
    failures.push((bad_plt, 12, MalformedKind::InvalidPacketLength));
    let mut unterminated_plt = vec![0xff, 0x58, 0, 4, 0, 0x80];
    failures.push((std::mem::take(&mut unterminated_plt), 0, MalformedKind::InvalidPacketLength));
    // Exercise errors both before and after accumulating a valid transition.
    for declared in [1, 12, 13, valid.len() as u32 - 1] {
        for prefix in [Vec::new(), valid.clone()] {
            let (index, total) = if prefix.is_empty() { (0, 1) } else { (1, 2) };
            let mut undersized = contract_part(7, index, total, Some(0), false, false).header;
            undersized[6..10].copy_from_slice(&declared.to_be_bytes());
            let offset = prefix.len() as u64;
            let mut bytes = prefix;
            bytes.extend(undersized);
            failures.push((bytes, offset, MalformedKind::InvalidTilePart));
        }
    }
    let mut errors = Vec::new();
    for (bytes, offset, kind) in failures {
        let cache = contract_cache(&bytes, 77, 7);
        eprintln!("complete malformed bytes={bytes:02x?}; expected {kind:?} at {offset}");
        let expected = Err(DecodeError::MalformedInput { offset, kind });
        let actual = cache.tile_part_transitions(77, 7);
        if actual != expected {
            errors.push(format!("bytes={bytes:02x?}: expected {expected:?}; got {actual:?}"));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[derive(Debug, PartialEq)]
struct ContractCacheSnapshot {
    limits: ResourceLimits,
    bins: Vec<(DatabinId, Option<u64>, Vec<(u64, Vec<u8>)>)>,
    stored_bytes: u64,
    accepted_fragments: u64,
}

fn contract_snapshot(cache: &DatabinCache) -> ContractCacheSnapshot {
    ContractCacheSnapshot {
        limits: cache.limits,
        bins: cache.databins.iter().map(|bin| (
            bin.id, bin.final_length,
            bin.fragments.iter().map(|fragment| (fragment.offset, fragment.bytes.clone())).collect(),
        )).collect(),
        stored_bytes: cache.stored_bytes(),
        accepted_fragments: cache.accepted_fragments(),
    }
}

fn contract_disjoint(a: &[u8], b: &[u8]) {
    if !a.is_empty() && !b.is_empty() {
        let a_start = a.as_ptr() as usize;
        let b_start = b.as_ptr() as usize;
        assert!(a_start + a.len() <= b_start || b_start + b.len() <= a_start,
            "nonempty owned byte ranges overlap");
    }
}

#[test]
fn contract_owned_results_and_cache_frame() {
    let parts = vec![
        contract_part(42, 0, 2, Some(0), true, false),
        contract_part(42, 1, 2, None, false, true),
    ];
    let expected = contract_available(parts.clone());
    let bytes = contract_bytes(&parts);
    let retained;
    {
        let mut cache = contract_cache(&bytes, 5, 42);
        contract_insert(&mut cache, 8, 7, 99, 2, &[1, 2, 3], false);
        let before = contract_snapshot(&cache);
        retained = cache.tile_part_transitions(5, 42).unwrap();
        assert_eq!(retained, expected);
        assert_eq!(contract_snapshot(&cache), before);
        for (index, transition) in retained.transitions.iter().enumerate() {
            for other in retained.transitions.iter().skip(index + 1) {
                contract_disjoint(&transition.header, &other.header);
            }
            for bin in &cache.databins {
                for fragment in &bin.fragments {
                    contract_disjoint(&transition.header, &fragment.bytes);
                }
            }
        }
        let mut disposable = cache.tile_part_transitions(5, 42).unwrap();
        disposable.transitions[0].header.fill(0);
        disposable.transitions[0].tile = 123;
        disposable.transitions.clear();
        disposable.needs.push(JpipSemanticNeed {
            kind: JpipSemanticNeedKind::Codestream, identifier: 999,
        });
        disposable.missing.push(JpipMissingRange {
            databin: id(8, 999), offset: 9, length: Some(3),
        });
        drop(disposable);
        assert_eq!(retained, expected);
        assert_eq!(contract_snapshot(&cache), before);
        // A missing report is also independent and read-only.
        let mut missing = cache.tile_part_transitions(5, 43).unwrap();
        assert_eq!(missing.status(), JpipAvailability::Missing);
        missing.needs.clear();
        missing.missing.clear();
        drop(missing);
        assert_eq!(contract_snapshot(&cache), before);
        contract_insert(&mut cache, 3, 5, 42, 0, &bytes, true);
        contract_insert(&mut cache, 8, 7, 100, 0, &[4, 5], true);
        assert_eq!(retained, expected);
        assert_eq!(cache.tile_part_transitions(5, 42).unwrap(), expected);
    }
    assert_eq!(retained, expected); // The cache has been destroyed.
    let mut malformed = parts[0].header.clone();
    malformed.extend(contract_part(41, 1, 2, Some(0), false, false).header);
    let mut cache = contract_cache(&malformed, 5, 42);
    for _ in 0..16 {
        let before = contract_snapshot(&cache);
        assert_eq!(cache.tile_part_transitions(5, 42),
            Err(DecodeError::MalformedInput {
                offset: parts[0].header.len() as u64,
                kind: MalformedKind::InvalidTilePart,
            }));
        assert_eq!(contract_snapshot(&cache), before);
    }
    contract_insert(&mut cache, 8, 1, 0, 0, &[7], true);
    assert_eq!(retained, expected);
}

#[test]
fn contract_history_and_idempotence() {
    // Only insertion history, unrelated bins and limits differ; the target's
    // complete bytes and requested tile are equal. Addresses/capacities may differ.
    let mut seed = 0x0987_6543;
    for iteration in 0..48 {
        let tile = if iteration == 0 { u16::MAX } else { contract_random(&mut seed) as u16 };
        let stream = contract_random(&mut seed);
        let mut bytes = match iteration % 4 {
            0 => Vec::new(),
            1 => segment(0xff64, &[0, 1, b'x']),
            _ => contract_bytes(&[
                contract_part(tile, 0, 2, Some(0), true, false),
                contract_part(tile, 1, 2, None, false, true),
            ]),
        };
        if iteration % 7 == 0 {
            bytes = vec![0xff]; // Equal errors, not only equal successful reports.
        }
        let a = contract_cache(&bytes, stream, tile);
        let limits = ResourceLimits::default().with_maximum_input_bytes(4096).unwrap()
            .with_maximum_jpip_fragments(4096).unwrap();
        let mut b = DatabinCache::new(limits);
        contract_insert(&mut b, 8, stream, tile, 0, &[1, 2, 3], true);
        // Announce length before filling the target in reverse one-byte chunks.
        contract_insert(&mut b, 3, stream, tile, bytes.len(), &[], true);
        for index in (0..bytes.len()).rev() {
            contract_insert(&mut b, if index % 2 == 0 { 2 } else { 3 },
                stream, tile, index, &bytes[index..index + 1], false);
        }
        contract_insert(&mut b, 3, stream, tile, 0, &bytes, true);
        assert!(b.is_complete(DatabinId::new(2, stream, u64::from(tile)).unwrap()));
        assert_ne!(a.accepted_fragments(), b.accepted_fragments());
        assert_ne!(a.databin_count(), b.databin_count());
        assert_ne!(a.limits, b.limits);
        let before_a = contract_snapshot(&a);
        let before_b = contract_snapshot(&b);
        let expected = a.tile_part_transitions(stream, tile);
        eprintln!("history equivalence: iteration={iteration} bytes={}", bytes.len());
        for _ in 0..5 {
            assert_eq!(b.tile_part_transitions(stream, tile), expected);
            assert_eq!(a.tile_part_transitions(stream, tile), expected);
            assert_eq!(contract_snapshot(&a), before_a);
            assert_eq!(contract_snapshot(&b), before_b);
        }
    }
}

#[test]
fn contract_overlapping_immutable_queries() {
    use std::sync::Barrier;
    let parts = vec![
        contract_part(u16::MAX, 0, 2, Some(0), true, false),
        contract_part(u16::MAX, 1, 2, None, false, true),
    ];
    let bytes = contract_bytes(&parts);
    for mode in 0..3 {
        let cache = match mode {
            0 => contract_cache(&bytes, 9, u16::MAX),
            1 => DatabinCache::new(ResourceLimits::default()),
            _ => contract_cache(&[0xff], 9, u16::MAX),
        };
        let expected = match mode {
            0 => Ok(contract_available(parts.clone())),
            1 => Ok(JpipTilePartTransitionReport {
                status: JpipAvailability::Missing,
                transitions: Vec::new(),
                needs: vec![JpipSemanticNeed {
                    kind: JpipSemanticNeedKind::TilePartTransition,
                    identifier: u64::from(u16::MAX),
                }],
                missing: vec![JpipMissingRange {
                    databin: DatabinId::new(2, 9, u64::from(u16::MAX)).unwrap(),
                    offset: 0, length: None,
                }],
            }),
            _ => Err(DecodeError::MalformedInput { offset: 0, kind: MalformedKind::Truncated }),
        };
        let before = contract_snapshot(&cache);
        let barrier = Barrier::new(2);
        eprintln!("concurrency: mode={mode}, two shared borrowers, no cache lock");
        std::thread::scope(|scope| {
            let worker = || {
                let mut errors = Vec::new();
                for _ in 0..64 {
                    barrier.wait();
                    let mut report = cache.tile_part_transitions(9, u16::MAX);
                    if report != expected {
                        errors.push(format!("query expected {expected:?}; got {report:?}"));
                    }
                    // Keep both results live, then destroy/mutate only one.
                    barrier.wait();
                    if std::thread::current().name() == Some("contract-writer") {
                        if let Ok(report) = &mut report {
                            for transition in &mut report.transitions {
                                transition.header.fill(0);
                            }
                            report.transitions.clear();
                            report.needs.clear();
                            report.missing.clear();
                        }
                        drop(report);
                        barrier.wait();
                    } else {
                        barrier.wait();
                        if report != expected {
                            errors.push(format!("other report mutation changed result: {report:?}"));
                        }
                    }
                }
                assert!(errors.is_empty(), "{}", errors.join("\n"));
            };
            let writer = std::thread::Builder::new().name("contract-writer".into())
                .spawn_scoped(scope, worker).unwrap();
            let reader = scope.spawn(worker);
            writer.join().unwrap();
            reader.join().unwrap();
        });
        assert_eq!(contract_snapshot(&cache), before);
    }
}
