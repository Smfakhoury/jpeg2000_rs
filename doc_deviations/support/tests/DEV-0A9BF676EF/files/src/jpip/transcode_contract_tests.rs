// Direct unit-contract route: safe plan/fragment constructors -> cache.insert
// -> DatabinCache::transcode_precinct_codestream. Private state is observed only.

#[derive(Debug, PartialEq, Eq)]
struct SavedBin {
    id: DatabinId,
    final_length: Option<u64>,
    fragments: Vec<(u64, Vec<u8>)>,
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    limits: ResourceLimits,
    stored_bytes: u64,
    accepted_fragments: u64,
    bins: Vec<SavedBin>,
}

fn snapshot(cache: &DatabinCache) -> Snapshot {
    Snapshot {
        limits: cache.limits,
        stored_bytes: cache.stored_bytes(),
        accepted_fragments: cache.accepted_fragments(),
        bins: cache
            .databins
            .iter()
            .map(|bin| SavedBin {
                id: bin.id,
                final_length: bin.final_length,
                fragments: bin
                    .fragments
                    .iter()
                    .map(|f| (f.offset, f.bytes.clone()))
                    .collect(),
            })
            .collect(),
    }
}

fn call(
    cache: &DatabinCache,
    plan: &JpipPrecinctCodestreamPlan,
    k: u16,
) -> Result<JpipPrecinctTranscode, DecodeError> {
    eprintln!(
        "route=DatabinCache::transcode_precinct_codestream layers={k}/{} bins={} stored={}",
        plan.layers(),
        cache.databin_count(),
        cache.stored_bytes()
    );
    let before = snapshot(cache);
    let saved_plan = plan.clone();
    let result = cache.transcode_precinct_codestream(plan, k);
    assert_eq!(snapshot(cache), before, "cache changed on transcode return");
    assert_eq!(*plan, saved_plan, "plan changed on transcode return");
    result
}

fn error(result: Result<JpipPrecinctTranscode, DecodeError>, expected: DecodeError) {
    match result {
        Err(actual) => assert_eq!(actual, expected),
        Ok(r) => panic!(
            "expected {expected:?}; got Ok, status={:?}, output={}",
            r.availability().status(),
            r.codestream().is_some()
        ),
    }
}

fn malformed(offset: u64, kind: MalformedKind) -> DecodeError {
    DecodeError::MalformedInput { offset, kind }
}

fn plan(packets: Vec<JpipPrecinctPacket>, layers: u16) -> JpipPrecinctCodestreamPlan {
    JpipPrecinctCodestreamPlan::new(
        id(6, 0),
        id(2, 0),
        packets,
        layers,
        &ResourceLimits::default(),
    )
    .unwrap()
}

fn simple_plan(layers: u16) -> JpipPrecinctCodestreamPlan {
    plan(
        (0..layers)
            .map(|layer| JpipPrecinctPacket::new(id(0, 0), layer, u64::from(layer), 1).unwrap())
            .collect(),
        layers,
    )
}

fn headers(layers: u16, tile_progression: Option<u8>) -> (Vec<u8>, Vec<u8>) {
    let bytes = decodable_codestream_with_layers(layers);
    let start = bytes.windows(2).position(|w| w == [0xff, 0x90]).unwrap();
    let mut main = bytes[..start].to_vec();
    // A marker-like payload tests segment-boundary patching, not byte searching.
    main.extend(segment(0xff64, &[0, 1, 0xff, 0x52, 0xab, 0xcd]));
    let mut tile = bytes[start..start + 12].to_vec();
    if let Some(progression) = tile_progression {
        let [hi, lo] = layers.to_be_bytes();
        tile.extend(segment(0xff52, &[0, progression, hi, lo, 0, 0, 0, 0, 0, 1]));
    }
    tile.extend([0xff, 0x93]);
    assert_eq!(&main[45..47], &[0xff, 0x52]);
    (main, tile)
}

fn complete_cache(
    main: &[u8],
    tile: &[u8],
    packets: &[u8],
    limits: ResourceLimits,
    chunked: bool,
) -> DatabinCache {
    let mut cache = DatabinCache::new(limits);
    for (target, bytes) in [(id(7, 0), main), (id(3, 0), tile), (id(1, 0), packets)] {
        if chunked && bytes.len() > 1 {
            let mid = bytes.len() / 2;
            cache
                .insert(fragment(target, mid as u64, &bytes[mid..], true))
                .unwrap();
            cache
                .insert(fragment(target, 0, &bytes[..mid], false))
                .unwrap();
        } else {
            cache.insert(fragment(target, 0, bytes, true)).unwrap();
        }
    }
    cache
}

// Independent byte oracle: known fixture COD boundaries, entry header copies,
// and packet intervals from the plan, never the subject's output or patch helpers.
fn expected_bytes(
    main: &[u8],
    tile: &[u8],
    raw: &[u8],
    p: &JpipPrecinctCodestreamPlan,
    k: u16,
) -> Vec<u8> {
    let mut h = main.to_vec();
    h[51..53].copy_from_slice(&k.to_be_bytes());
    let mut t = tile.to_vec();
    if t.get(12..14) == Some(&[0xff, 0x52]) {
        t[18..20].copy_from_slice(&k.to_be_bytes());
    }
    let mut selected = Vec::new();
    for packet in p.packets().iter().filter(|packet| packet.layer() < k) {
        let range = packet.range();
        let start = usize::try_from(range.offset()).unwrap();
        let end = start + usize::try_from(range.length()).unwrap();
        selected.extend_from_slice(&raw[start..end]);
    }
    let psot = u32::try_from(t.len() + selected.len()).unwrap();
    t[6..10].copy_from_slice(&psot.to_be_bytes());
    h.extend(t);
    h.extend(selected);
    h.extend([0xff, 0xd9]);
    h
}

fn assert_available(r: &JpipPrecinctTranscode, k: u16, expected: &[u8]) {
    assert_eq!(r.layers(), k);
    assert_eq!(r.availability().status(), JpipAvailability::Available);
    assert_eq!(r.availability().required_bytes(), expected.len() as u64 - 2);
    assert_eq!(
        r.availability().available_bytes(),
        expected.len() as u64 - 2
    );
    assert!(r.availability().missing_ranges().is_empty());
    assert_eq!(
        r.codestream()
            .expect("Available must own output")
            .as_bytes(),
        expected
    );
}

#[test]
fn invalid_layer_requests_precede_cache_and_header_checks() {
    for layers in [1, 2, 7, 255, 256] {
        let p = simple_plan(layers);
        let (h, t) = headers(layers, None);
        let raw = vec![0; usize::from(layers)];
        let good = complete_cache(&h, &t, &raw, ResourceLimits::default(), false);
        let bad = complete_cache(
            b"bad main",
            b"bad tile",
            &raw,
            ResourceLimits::default()
                .with_maximum_work_units(0)
                .unwrap(),
            false,
        );
        let empty = DatabinCache::new(ResourceLimits::default());
        for c in [&empty, &bad, &good] {
            for k in [0, layers + 1, u16::MAX] {
                error(
                    call(c, &p, k),
                    malformed(u64::from(k), MalformedKind::InvalidJpipLayout),
                );
            }
        }
        for k in [1, layers] {
            let expected = expected_bytes(&h, &t, &raw, &p, k);
            assert_available(&call(&good, &p, k).unwrap(), k, &expected);
        }
    }
}

struct HeaderState {
    pieces: Vec<(u64, &'static [u8], bool)>,
    required: u64,
    available: u64,
    missing: Vec<(u64, Option<u64>)>,
}

fn header_states() -> Vec<HeaderState> {
    vec![
        HeaderState {
            pieces: vec![],
            required: 0,
            available: 0,
            missing: vec![(0, None)],
        },
        // Unknown final length with an empty contiguous prefix, safely made
        // by receiving only a nonempty out-of-order fragment.
        HeaderState {
            pieces: vec![(2, b"x", false)],
            required: 0,
            available: 0,
            missing: vec![(0, None)],
        },
        HeaderState {
            pieces: vec![(0, b"bad", false), (5, b"x", false)],
            required: 0,
            available: 3,
            missing: vec![(3, None)],
        },
        HeaderState {
            pieces: vec![(0, b"", true)],
            required: 0,
            available: 0,
            missing: vec![],
        },
        HeaderState {
            pieces: vec![(0, b"a", false), (3, b"d", true)],
            required: 4,
            available: 2,
            missing: vec![(1, Some(2))],
        },
        HeaderState {
            pieces: vec![(0, b"bad", true)],
            required: 3,
            available: 3,
            missing: vec![],
        },
    ]
}

#[test]
fn availability_is_exact_for_occurrences_suffixes_and_headers() {
    let p = plan(
        vec![
            JpipPrecinctPacket::new(id(1, 7), 0, 0, 4).unwrap(),
            JpipPrecinctPacket::new(id(1, 7), 0, 0, 4).unwrap(),
            JpipPrecinctPacket::new(id(1, 7), 0, 2, 4).unwrap(),
            JpipPrecinctPacket::new(id(1, 7), 1, 100, 3).unwrap(),
        ],
        2,
    );
    for (hi, h) in header_states().iter().enumerate() {
        for (ti, t) in header_states().iter().enumerate() {
            for k in [1, 2] {
                eprintln!("header_states={hi},{ti} requested={k}");
                let mut c = DatabinCache::new(ResourceLimits::default());
                c.insert(fragment(id(0, 7), 1, b"bc", false)).unwrap();
                c.insert(fragment(id(0, 7), 5, b"f", false)).unwrap();
                for (target, state) in [(id(7, 0), h), (id(3, 0), t)] {
                    for &(offset, bytes, final_byte) in &state.pieces {
                        c.insert(fragment(target, offset, bytes, final_byte))
                            .unwrap();
                    }
                }
                let mut missing = vec![
                    (id(1, 7), 0, Some(1)),
                    (id(1, 7), 3, Some(1)),
                    (id(1, 7), 0, Some(1)),
                    (id(1, 7), 3, Some(1)),
                    (id(1, 7), 3, Some(2)),
                ];
                if k == 2 {
                    missing.push((id(1, 7), 100, Some(3)));
                }
                for (target, state) in [(id(6, 0), h), (id(2, 0), t)] {
                    missing.extend(state.missing.iter().map(|&(off, len)| (target, off, len)));
                }
                let expected = JpipAvailabilityReport {
                    status: JpipAvailability::Partial,
                    required_bytes: 12 + if k == 2 { 3 } else { 0 } + h.required + t.required,
                    available_bytes: 6 + h.available + t.available,
                    missing: missing
                        .into_iter()
                        .map(|(databin, offset, length)| JpipMissingRange {
                            databin,
                            offset,
                            length,
                        })
                        .collect(),
                };
                let r = call(&c, &p, k).expect("incomplete malformed headers must not be parsed");
                assert_eq!(r.layers(), k);
                assert_eq!(*r.availability(), expected);
                assert!(r.codestream().is_none());
            }
        }
    }
    // Covered packets must still wait for each absent, unknown or holed header.
    // In particular an unknown prefix may make available exceed required bytes.
    for (hi, h) in header_states().iter().enumerate() {
        for (ti, t) in header_states().iter().enumerate() {
            if h.missing.is_empty() && t.missing.is_empty() {
                continue;
            }
            for k in [1, 2] {
                eprintln!("covered_packets header_states={hi},{ti} requested={k}");
                let mut c = DatabinCache::new(ResourceLimits::default());
                c.insert(fragment(id(0, 7), 0, &[0; 103], true)).unwrap();
                let mut missing = Vec::new();
                for (target, requested, state) in [
                    (id(7, 0), id(6, 0), h),
                    (id(3, 0), id(2, 0), t),
                ] {
                    for &(offset, bytes, final_byte) in &state.pieces {
                        c.insert(fragment(target, offset, bytes, final_byte)).unwrap();
                    }
                    missing.extend(state.missing.iter().map(|&(offset, length)| {
                        JpipMissingRange { databin: requested, offset, length }
                    }));
                }
                let packet_bytes = if k == 1 { 12 } else { 15 };
                let expected = JpipAvailabilityReport {
                    status: JpipAvailability::Partial,
                    required_bytes: packet_bytes + h.required + t.required,
                    available_bytes: packet_bytes + h.available + t.available,
                    missing,
                };
                let r = call(&c, &p, k).expect("covered packets do not complete headers");
                assert_eq!(r.layers(), k);
                assert_eq!(*r.availability(), expected);
                assert!(r.codestream().is_none());
            }
        }
    }
    let empty = DatabinCache::new(ResourceLimits::default());
    let r = call(&empty, &p, 1).unwrap();
    assert_eq!(
        *r.availability(),
        JpipAvailabilityReport {
            status: JpipAvailability::Missing,
            required_bytes: 12,
            available_bytes: 0,
            missing: vec![
                JpipMissingRange {
                    databin: id(1, 7),
                    offset: 0,
                    length: Some(4)
                },
                JpipMissingRange {
                    databin: id(1, 7),
                    offset: 0,
                    length: Some(4)
                },
                JpipMissingRange {
                    databin: id(1, 7),
                    offset: 2,
                    length: Some(4)
                },
                JpipMissingRange {
                    databin: id(6, 0),
                    offset: 0,
                    length: None
                },
                JpipMissingRange {
                    databin: id(2, 0),
                    offset: 0,
                    length: None
                },
            ],
        }
    );
    assert!(r.codestream().is_none());

    let p = simple_plan(2);
    let (h, t) = headers(2, None);
    let c = complete_cache(&h, &t, &[0], ResourceLimits::default(), false);
    assert_available(
        &call(&c, &p, 1).unwrap(),
        1,
        &expected_bytes(&h, &t, &[0], &p, 1),
    );
    let r = call(&c, &p, 2).unwrap();
    assert_eq!(r.availability().status(), JpipAvailability::Partial);
    assert_eq!(
        r.availability().required_bytes(),
        (h.len() + t.len() + 2) as u64
    );
    assert_eq!(
        r.availability().available_bytes(),
        (h.len() + t.len() + 1) as u64
    );
    assert_eq!(
        r.availability().missing_ranges(),
        &[JpipMissingRange {
            databin: id(0, 0),
            offset: 1,
            length: Some(1)
        }]
    );
    assert!(r.codestream().is_none());
}

#[test]
fn errors_preserve_diagnostic_order_and_exact_limits() {
    let p = simple_plan(1);
    let (h, t) = headers(1, None);
    let work_limits = ResourceLimits::default()
        .with_maximum_work_units(0)
        .unwrap();
    let c = complete_cache(b"bad", b"bad", &[0], work_limits, false);
    error(
        call(&c, &p, 1),
        DecodeError::LimitExceeded {
            resource: Resource::WorkUnits,
            limit: 0,
            actual: 1,
        },
    );
    let c = complete_cache(b"bad", b"bad", &[0], ResourceLimits::default(), false);
    error(
        call(&c, &p, 1),
        malformed(0, MalformedKind::MissingRequiredStructure),
    );
    let c = complete_cache(
        &[0xff, 0x4f, 0xff, 0x52, 0, 6, 0, 0, 0, 1],
        b"bad",
        &[0],
        ResourceLimits::default(),
        false,
    );
    error(
        call(&c, &p, 1),
        malformed(2, MalformedKind::InvalidCodingStyle),
    );
    let c = complete_cache(&h, b"bad", &[0], ResourceLimits::default(), false);
    error(
        call(&c, &p, 1),
        malformed(0, MalformedKind::InvalidJpipLayout),
    );
    let mut bad_tile = t[..12].to_vec();
    bad_tile.extend(segment(0xff52, &[0, 0, 0, 1]));
    bad_tile.extend([0xff, 0x93]);
    let c = complete_cache(&h, &bad_tile, &[0], ResourceLimits::default(), false);
    error(
        call(&c, &p, 1),
        malformed(12, MalformedKind::InvalidCodingStyle),
    );

    let p3 = simple_plan(3);
    let c = complete_cache(
        b"bad",
        b"bad",
        &[0, 0, 0],
        ResourceLimits::default().with_maximum_packets(1).unwrap(),
        false,
    );
    error(
        call(&c, &p3, 3),
        DecodeError::LimitExceeded {
            resource: Resource::Packets,
            limit: 1,
            actual: 3,
        },
    );
    let p_far = plan(
        vec![JpipPrecinctPacket::new(id(0, 0), 0, 100, 1).unwrap()],
        1,
    );
    let c = complete_cache(
        b"bad",
        b"bad",
        &[0],
        ResourceLimits::default()
            .with_maximum_input_bytes(100)
            .unwrap(),
        false,
    );
    error(
        call(&c, &p_far, 1),
        DecodeError::LimitExceeded {
            resource: Resource::InputBytes,
            limit: 100,
            actual: 101,
        },
    );

    let expected = expected_bytes(&h, &t, &[0], &p, 1);
    let n = expected.len() as u64;
    for limit in [n - 2, n - 1, n, n + 1] {
        let limits = ResourceLimits::default()
            .with_maximum_input_bytes(limit)
            .unwrap();
        let c = complete_cache(&h, &t, &[0], limits, false);
        if limit < n {
            error(
                call(&c, &p, 1),
                DecodeError::LimitExceeded {
                    resource: Resource::InputBytes,
                    limit,
                    actual: n,
                },
            );
        } else {
            assert_available(&call(&c, &p, 1).unwrap(), 1, &expected);
        }
    }
    // Final profile rejection must not mask aggregate length failure.
    let mut bad_h = h.clone();
    bad_h[51..53].copy_from_slice(&[0, 1]);
    bad_h[50] = 4;
    let c = complete_cache(
        &bad_h,
        &t,
        &[0],
        ResourceLimits::default()
            .with_maximum_input_bytes(n - 1)
            .unwrap(),
        false,
    );
    error(
        call(&c, &p, 1),
        DecodeError::LimitExceeded {
            resource: Resource::InputBytes,
            limit: n - 1,
            actual: n,
        },
    );
    // Valid framing but missing SIZ reaches the final parser after patching.
    let c = complete_cache(&SOC, &t, &[0], ResourceLimits::default(), false);
    error(
        call(&c, &p, 1),
        malformed(2, MalformedKind::InvalidMarkerOrder),
    );
    // An empty PLT cannot account for this nonempty selected packet.
    // It is a parser rejection, not a parse-valid profile fixture.
    let mut empty_plt = t.clone();
    append_tile_segment(&mut empty_plt, 0xff58, &[0]);
    let c = complete_cache(&h, &empty_plt, &[0], ResourceLimits::default(), false);
    error(
        call(&c, &p, 1),
        malformed(h.len() as u64, MalformedKind::InvalidPacketLength),
    );
}

#[test]
fn exact_reconstruction_preserves_bytes_order_multiplicity_and_chunking() {
    transcodes_precinct_ranges_into_a_decodable_codestream();
    let mut seed = 0x12ab_34cd_u32;
    for layers in [1, 2, 3, 7, 16, 255, 256] {
        for tile_cod in [None, Some(0)] {
            let (h, t) = headers(layers, tile_cod);
            let raw: Vec<u8> = (0..32).map(|i| (i * 7 + 11) as u8).collect();
            let mut packets = Vec::new();
            for layer in 0..layers {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let offset = u64::from((seed >> 16) % 20);
                for (off, length) in [(offset, 3), (offset + 1, 2), (offset, 3)] {
                    packets.push(JpipPrecinctPacket::new(id(0, 0), layer, off, length).unwrap());
                }
            }
            let p = plan(packets, layers);
            let c = complete_cache(&h, &t, &raw, ResourceLimits::default(), false);
            let chunked = complete_cache(&h, &t, &raw, ResourceLimits::default(), true);
            for k in 1..=layers {
                let expected = expected_bytes(&h, &t, &raw, &p, k);
                let r = call(&c, &p, k).unwrap();
                assert_available(&r, k, &expected);
                let other = call(&chunked, &p, k).unwrap();
                assert_available(&other, k, &expected);
                // Same bytes and final extents, different legal ingestion order
                // and fragmentation. Only cache fragment counters may differ.
                assert_eq!(r.codestream(), other.codestream());
                assert_eq!(r.availability(), other.availability());
                assert!(expected.len() as u64 <= c.limits.maximum_input_bytes());
            }
        }
    }
}

fn append_tile_segment(tile: &mut Vec<u8>, marker: u16, payload: &[u8]) {
    tile.truncate(tile.len() - 2);
    tile.extend(segment(marker, payload));
    tile.extend([0xff, 0x93]);
}

#[test]
fn explicit_profile_rejects_all_non_lrcp_and_attached_sources() {
    let p = simple_plan(1);
    let (h, t) = headers(1, Some(0));
    let mut variants = vec![("valid".to_owned(), h.clone(), t.clone(), true)];
    for progression in 1..=4 {
        let mut main = h.clone();
        main[50] = progression;
        variants.push((
            format!("main progression {progression}"),
            main,
            t.clone(),
            false,
        ));
        let mut tile = t.clone();
        tile[17] = progression;
        variants.push((
            format!("effective tile progression {progression}"),
            h.clone(),
            tile,
            false,
        ));
    }
    let mut two_tiles = h.clone();
    two_tiles[8..12].copy_from_slice(&2_u32.to_be_bytes());
    variants.push(("two image tiles".into(), two_tiles, t.clone(), false));
    for in_main in [true, false] {
        let mut main = h.clone();
        let mut tile = t.clone();
        let poc = [0, 0, 0, 1, 1, 1, 0];
        if in_main {
            main.extend(segment(0xff5f, &poc));
        } else {
            append_tile_segment(&mut tile, 0xff5f, &poc);
        }
        variants.push((format!("POC main={in_main}"), main, tile, false));
    }
    for (name, marker, payload, in_main) in [
        ("PLT", 0xff58, vec![0, 1], false),
        ("empty PPT", 0xff61, vec![0], false),
        ("PPT", 0xff61, vec![0, 0], false),
        ("PLM", 0xff57, vec![0, 0, 0, 0, 1, 1], true),
        ("empty PPM", 0xff60, vec![0, 0, 0, 0, 0], true),
        ("PPM", 0xff60, vec![0, 0, 0, 0, 1, 0], true),
    ] {
        let mut main = h.clone();
        let mut tile = t.clone();
        if in_main {
            main.extend(segment(marker, &payload));
        } else {
            append_tile_segment(&mut tile, marker, &payload);
        }
        variants.push((name.into(), main, tile, false));
    }
    let mut failures = Vec::new();
    for (name, main, tile, eligible) in variants {
        eprintln!("profile variant={name}");
        let expected = expected_bytes(&main, &tile, &[0], &p, 1);
        let limits = ResourceLimits::default();
        let layout = crate::codestream::parse_layout(&expected, &limits)
            .unwrap_or_else(|e| panic!("profile fixture {name} must parse: {e:?}"));
        assert_eq!(layout.tile_parts.len(), 1, "fixture {name}");
        assert_eq!(layout.info.coding_style.layers, 1);
        let explicit = layout.info.image_size.tile_count().unwrap() == 1
            && matches!(
                layout.info.coding_style.progression_order,
                crate::codestream::ProgressionOrder::Lrcp
            )
            && matches!(
                layout.tile_parts[0].coding_style.progression_order,
                crate::codestream::ProgressionOrder::Lrcp
            )
            && !layout.info.progression_changes.is_present()
            && !layout.tile_parts[0].progression_changes.is_present()
            && !layout.tile_parts[0].packet_lengths.is_present()
            && !layout.tile_parts[0].packed_packet_headers.is_present();
        assert_eq!(explicit, eligible, "independent fixture premises {name}");
        let c = complete_cache(&main, &tile, &[0], ResourceLimits::default(), false);
        match call(&c, &p, 1) {
            Ok(r) if eligible => assert_available(&r, 1, &expected),
            Err(e) if !eligible && e == malformed(0, MalformedKind::InvalidJpipLayout) => {}
            Ok(r) => failures.push(format!(
                "{name}: published output={}, status={:?}",
                r.codestream().is_some(),
                r.availability().status()
            )),
            Err(e) => failures.push(format!("{name}: wrong error {e:?}")),
        }
    }
    assert!(
        failures.is_empty(),
        "profile contract violations:\n{}",
        failures.join("\n")
    );
}

#[test]
fn ownership_survives_input_mutation_destruction_and_error_returns() {
    let p = simple_plan(2);
    let (h, t) = headers(2, Some(0));
    let expected = expected_bytes(&h, &t, &[0, 0], &p, 1);
    let mut c = complete_cache(&h, &t, &[0, 0], ResourceLimits::default(), true);
    let result = call(&c, &p, 1).unwrap();
    assert_available(&result, 1, &expected);
    let report = result.availability().clone();
    c.insert(fragment(id(0, 99), 0, b"later unrelated bin", true))
        .unwrap();
    assert_available(&result, 1, &expected);
    let before_drop = snapshot(&c);
    let owned = result.into_codestream().unwrap().into_bytes();
    assert_eq!(snapshot(&c), before_drop);
    drop(p);
    drop(c);
    assert_eq!(owned, expected);
    assert_eq!(report.status(), JpipAvailability::Available);
    assert_eq!(report.required_bytes(), expected.len() as u64 - 2);

    let p = simple_plan(2);
    let mut c = DatabinCache::new(ResourceLimits::default());
    let missing = call(&c, &p, 1).unwrap();
    let expected_missing = vec![
        JpipMissingRange {
            databin: id(0, 0),
            offset: 0,
            length: Some(1),
        },
        JpipMissingRange {
            databin: id(6, 0),
            offset: 0,
            length: None,
        },
        JpipMissingRange {
            databin: id(2, 0),
            offset: 0,
            length: None,
        },
    ];
    assert_eq!(missing.availability().missing_ranges(), expected_missing);
    c.insert(fragment(id(6, 0), 0, &h, true)).unwrap();
    c.insert(fragment(id(2, 0), 0, &t, true)).unwrap();
    c.insert(fragment(id(0, 0), 0, &[0, 0], true)).unwrap();
    assert_eq!(missing.availability().missing_ranges(), expected_missing);
    let good = call(&c, &p, 1).unwrap();
    let saved = snapshot(&c);
    drop(good);
    assert_eq!(snapshot(&c), saved);
    error(
        call(&c, &p, 0),
        malformed(0, MalformedKind::InvalidJpipLayout),
    );
    assert_available(&call(&c, &p, 1).unwrap(), 1, &expected);
    drop(p);
    drop(c);
    assert_eq!(missing.layers(), 1);
    assert_eq!(missing.availability().status(), JpipAvailability::Missing);
    assert_eq!(missing.availability().required_bytes(), 1);
    assert_eq!(missing.availability().available_bytes(), 0);
    assert_eq!(missing.availability().missing_ranges(), expected_missing);
    assert!(missing.into_codestream().is_none());

    let p = simple_plan(1);
    let c = complete_cache(b"bad", b"bad", &[0], ResourceLimits::default(), false);
    for _ in 0..16 {
        error(
            call(&c, &p, 1),
            malformed(0, MalformedKind::MissingRequiredStructure),
        );
        error(
            call(&c, &p, 2),
            malformed(2, MalformedKind::InvalidJpipLayout),
        );
    }
    assert_eq!(c.databin_view(id(6, 0)).unwrap().as_bytes(), b"bad");
}
