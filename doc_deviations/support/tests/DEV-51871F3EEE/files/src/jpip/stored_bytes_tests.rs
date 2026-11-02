#[test]
fn stored_bytes_obeys_metadata_bin_identity_and_sparse_union() {
    reassembles_out_of_order_fragments();
    accepts_identical_duplicates_and_overlaps();
    rejects_conflicting_overlap_without_mutation();
    rejects_conflicting_final_lengths_and_bytes_past_final();
    semantic_metadata_assembly_places_nested_boxes_and_codestreams();

    let mut failures = Vec::new();
    macro_rules! check {
        ($condition:expr, $($message:tt)*) => {
            if !$condition {
                failures.push(format!($($message)*));
            }
        };
    }

    // T.808 A.3.6.1: one logical target per cache, root bin 0 exists,
    // metadata is box data, and CSn does not distinguish metadata bins.
    // Relabeling CSn may change raw IDs, never payload accounting or views.
    let mut empty = DatabinCache::new(ResourceLimits::default());
    empty.insert(fragment(id(8, 0), 0, b"", true)).unwrap();
    assert_eq!(empty.stored_bytes(), 0);
    assert!(empty.is_complete(id(8, 0)));
    for payload_length in [0, 1, 7, 31] {
        let payload = vec![b'x'; payload_length];
        let bytes = box32(*b"free", &payload);
        let n = bytes.len();
        assert_eq!(u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize, n);
        assert_eq!(&bytes[4..8], b"free");
        let mut canonical = DatabinCache::new(ResourceLimits::default());
        let mut relabeled = DatabinCache::new(ResourceLimits::default());
        for cache in [&mut canonical, &mut relabeled] {
            assert_eq!(cache.stored_bytes(), 0);
            cache.insert(fragment(id(8, 0), n as u64, b"", true)).unwrap();
            assert_eq!(cache.stored_bytes(), 0);
            assert!(!cache.is_complete(id(8, 0)));
        }
        let mut spans = vec![(n / 2, n), (0, 1), (n / 4, 3 * n / 4)];
        let mut seed = 0x5eed_u64;
        for _ in 0..12 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let start = (seed % n as u64) as usize;
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let end = start + 1 + (seed % (n - start) as u64) as usize;
            spans.push((start, end));
        }
        spans.extend([(0, n), (0, n), (n, n)]);
        let mut seen = vec![false; n];
        for (step, &(start, end)) in spans.iter().enumerate() {
            let csn = [0, 1, 7, 127, 128][step % 5];
            let alias = DatabinId::new(8, csn, 0).unwrap();
            assert_eq!(alias.class(), DatabinClass::Metadata);
            assert_eq!(alias.in_class(), 0);
            for (name, cache, target) in [
                ("canonical", &mut canonical, id(8, 0)),
                ("relabeled", &mut relabeled, alias),
            ] {
                let result = cache.insert(fragment(
                    target, start as u64, &bytes[start..end], end == n,
                ));
                check!(result.is_ok(), "{name} n={n} step={step} CSn={csn} range={start}..{end}: {result:?}");
            }
            seen[start..end].fill(true);
            let expected = seen.iter().filter(|&&present| present).count() as u64;
            for (name, cache) in [("canonical", &canonical), ("relabeled", &relabeled)] {
                check!(cache.stored_bytes() == expected,
                    "{name} n={n} step={step} CSn={csn}: stored_bytes={} expected union={expected}",
                    cache.stored_bytes());
                check!(cache.databin_count() == 1,
                    "{name} n={n} step={step}: logical bin count={} expected=1", cache.databin_count());
                check!(cache.accepted_fragments() == step as u64 + 2,
                    "{name} n={n} step={step}: accepted fragment count");
            }
            check!(canonical.stored_bytes() == relabeled.stored_bytes(),
                "CSn relabeling changed accounting n={n} step={step}");
        }
        for csn in [0, 1, 7, 127, 128] {
            let alias = DatabinId::new(8, csn, 0).unwrap();
            check!(relabeled.is_complete(alias), "n={n} CSn={csn}: metadata bin incomplete");
            match relabeled.databin_view(alias) {
                Ok(view) => check!(view.as_bytes() == bytes,
                    "n={n} CSn={csn}: reconstructed bytes differ from original box"),
                Err(error) => failures.push(format!("n={n} CSn={csn}: view failed: {error:?}")),
            }
        }
        // A contradictory byte, final length, or byte past final must be
        // rejected even when it arrives with a previously unseen CSn.
        for csn in [0, 129] {
            let target = DatabinId::new(8, csn, 0).unwrap();
            for bad in [
                fragment(target, 0, &[bytes[0] ^ 1], false),
                fragment(target, (n - 1) as u64, b"", true),
                fragment(target, n as u64, b"x", false),
            ] {
                let before_count = relabeled.accepted_fragments();
                let before_bins = relabeled.databin_count();
                let before_bytes = relabeled.stored_bytes();
                let result = relabeled.insert(bad);
                check!(matches!(result, Err(DecodeError::MalformedInput {
                    kind: MalformedKind::ConflictingJpipFragment, ..
                })), "n={n} CSn={csn}: conflicting metadata must reject, got {result:?}");
                check!(relabeled.stored_bytes() == before_bytes
                    && relabeled.accepted_fragments() == before_count
                    && relabeled.databin_count() == before_bins,
                    "n={n} CSn={csn}: rejection changed accounting/state");
                match relabeled.databin_view(id(8, 0)) {
                    Ok(view) => check!(view.as_bytes() == bytes,
                        "n={n} CSn={csn}: conflict changed existing payload"),
                    Err(error) => failures.push(format!("n={n} CSn={csn}: conflict lost existing payload: {error:?}")),
                }
            }
        }
        let other_bin = DatabinId::new(8, 0, 1).unwrap();
        canonical.insert(fragment(other_bin, 0, &bytes, true)).unwrap();
        assert_eq!(canonical.stored_bytes(), 2 * n as u64);

        let limits = ResourceLimits::default().with_maximum_input_bytes(n as u64).unwrap();
        let mut bounded = DatabinCache::new(limits);
        bounded.insert(fragment(id(8, 0), 0, &bytes, true)).unwrap();
        let duplicate = bounded.insert(fragment(DatabinId::new(8, 1, 0).unwrap(), 0, &bytes, true));
        check!(duplicate.is_ok() && bounded.stored_bytes() == n as u64,
            "n={n}: metadata duplicate at exact byte budget: {duplicate:?}, stored={}", bounded.stored_bytes());
        let result = bounded.insert(fragment(other_bin, 0, &bytes[..1], false));
        check!(matches!(result, Err(DecodeError::LimitExceeded {
            resource: Resource::InputBytes, limit, actual,
        }) if limit == n as u64 && actual == n as u64 + 1),
            "n={n}: distinct-bin byte beyond budget: {result:?}");
        check!(bounded.stored_bytes() == n as u64,
            "n={n}: byte-budget rejection changed accounting");
    }
    // Unlike metadata, actual codestream payloads must remain CSn-qualified.
    let mut cache = DatabinCache::new(ResourceLimits::default());
    for csn in [0, 1] {
        cache.insert(fragment(DatabinId::new(6, csn, 0).unwrap(), 0, &SOC, true)).unwrap();
    }
    assert_eq!(cache.stored_bytes(), 4);
    assert_eq!(cache.databin_count(), 2);
    assert!(failures.is_empty(), "stored_bytes contract violations:\n{}", failures.join("\n"));
}
