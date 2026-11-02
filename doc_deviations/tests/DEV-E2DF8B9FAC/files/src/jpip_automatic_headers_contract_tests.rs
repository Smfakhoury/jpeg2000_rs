// Included inside jpip::tests so the private subject and existing fixtures stay private.
fn contract_main(tiles: u16, components: u16) -> Vec<u8> {
    let mut siz = 0_u16.to_be_bytes().to_vec();
    for value in [u32::from(tiles) * 2, 3, 0, 0, 2, 3, 0, 0] {
        siz.extend_from_slice(&value.to_be_bytes());
    }
    siz.extend_from_slice(&components.to_be_bytes());
    for _ in 0..components {
        siz.extend_from_slice(&[7, 1, 1]);
    }
    let mut main = SOC.to_vec();
    main.extend(segment(0xff51, &siz));
    main.extend(segment(0xff52, &[0, 0, 0, 1, 0, 0, 0, 0, 0, 1]));
    main.extend(segment(0xff5c, &[0, 0x40]));
    main.extend(segment(0xff64, &[0, 1, b'm']));
    main
}

fn contract_tile(tile: u16, markers: &[u8]) -> Vec<u8> {
    let mut expected = vec![0xff, 0x90, 0, 10];
    expected.extend_from_slice(&tile.to_be_bytes());
    expected.extend_from_slice(&[0, 0, 0, 0, 0, 1]);
    expected.extend_from_slice(markers);
    expected.extend_from_slice(&[0xff, 0x93]);
    expected
}

fn contract_insert(cache: &mut DatabinCache, target: DatabinId, bytes: &[u8]) {
    // Complete even an empty bin; nonempty bins arrive tail-first.
    let split = bytes.len() / 2;
    cache
        .insert(fragment(target, split as u64, &bytes[split..], true))
        .unwrap();
    if split != 0 {
        cache
            .insert(fragment(target, 0, &bytes[..split], false))
            .unwrap();
    }
    assert!(cache.is_complete(target));
    assert_eq!(cache.databin_view(target).unwrap().as_bytes(), bytes);
}

fn contract_validate_source(main: &[u8], tiles: u16, components: u16, markers: &[u8]) {
    // Independent, legal Part-1 source: one tile-part and one empty packet
    // per component, no packed packet headers, no capability extensions.
    let mut source = main.to_vec();
    for tile in 0..tiles {
        let mut header = contract_tile(tile, markers);
        let length = u32::try_from(header.len() + usize::from(components)).unwrap();
        header[6..10].copy_from_slice(&length.to_be_bytes());
        source.extend(header);
        source.extend(vec![0; usize::from(components)]);
    }
    source.extend_from_slice(&EOC);
    let limits = ResourceLimits::default();
    let layout = crate::codestream::parse_layout(&source, &limits).unwrap();
    assert_eq!(layout.tile_parts.len(), usize::from(tiles));
    // A.3.5: main header starts at SOC and excludes SOT, SOD and EOC.
    assert!(main.starts_with(&SOC));
    let mut position = 2;
    while position < main.len() {
        let (marker, _, end) = marker_segment(main, position).unwrap();
        assert!(!matches!(marker, 0xff90 | 0xff93 | 0xffd9));
        position = end;
    }
    assert_eq!(position, main.len());
    // A.3.3: raw tile bins contain only tile-header markers, never SOT.
    let mut position = 0;
    while position < markers.len() {
        let (marker, _, end) = marker_segment(markers, position).unwrap();
        assert!(!matches!(marker, 0xff90 | 0xff93 | 0xffd9));
        position = end;
    }
    assert_eq!(position, markers.len());
}

fn contract_missing(result: Result<AutomaticHeaders, DecodeError>, context: &str) {
    assert!(
        matches!(
            result,
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::MissingRequiredStructure,
                ..
            })
        ),
        "{context}: must reject with MissingRequiredStructure"
    );
}

#[test]
fn automatic_headers_designated_main_contract() {
    // Retain and execute the existing normalization regression as well.
    automatic_header_normalization_removes_reframed_markers();
    let mut seed = 0x53a9_4f21_u32;
    for iteration in 0..16 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let tiles = [1, 2, 7, 17][iteration % 4];
        let components = [1, 2, 3, 8][(seed as usize >> 16) % 4];
        let stream = [0, 1, 127, 128, u64::from(u32::MAX)][iteration % 5];
        let main = contract_main(tiles, components);
        let retained = segment(0xff64, &[0, 1, b't']);
        contract_validate_source(&main, tiles, components, &retained);
        let mut cache = DatabinCache::new(ResourceLimits::default());
        // A valid but different stream must neither substitute for nor contaminate
        // the designated class-6/in-class-0 header.
        let decoy = DatabinId::new(6, stream + 1, 0).unwrap();
        contract_insert(&mut cache, decoy, &contract_main(1, 1));
        contract_missing(
            cache.automatic_headers(stream),
            "only another codestream cached",
        );
        let main_id = DatabinId::new(6, stream, 0).unwrap();
        cache.insert(fragment(main_id, 0, &main, false)).unwrap();
        contract_missing(cache.automatic_headers(stream), "main is not complete");
        cache
            .insert(fragment(main_id, main.len() as u64, &[], true))
            .unwrap();
        contract_missing(cache.automatic_headers(stream), "tile headers absent");
        let mut raw_main = main.clone();
        let mut tile_markers = retained.clone();
        let mut plt = vec![0];
        plt.extend(vec![1; usize::from(components)]);
        tile_markers.extend(segment(0xff58, &plt));
        // Legal TLM: Ztlm=0, Stlm=0x40, four-byte tile-part lengths.
        let mut tlm = vec![0, 0x40];
        let part_length = u32::try_from(14 + tile_markers.len() + usize::from(components)).unwrap();
        for _ in 0..tiles {
            tlm.extend_from_slice(&part_length.to_be_bytes());
        }
        raw_main.extend(segment(0xff55, &tlm));
        // Use a separate cache to avoid overwriting the completed main bin.
        let mut normalized_cache = DatabinCache::new(ResourceLimits::default());
        contract_insert(&mut normalized_cache, decoy, &contract_main(1, 1));
        contract_insert(&mut normalized_cache, main_id, &raw_main);
        contract_validate_source(&raw_main, tiles, components, &tile_markers);
        for tile in 0..tiles {
            let mut raw_tile = tile_markers.clone();
            // PLT: Zplt=0 and one one-byte empty-packet length per component.
            raw_tile.extend_from_slice(&[0xff, 0x93]);
            let tile_id = DatabinId::new(2, stream, u64::from(tile)).unwrap();
            if tile == tiles - 1 {
                normalized_cache
                    .insert(fragment(tile_id, 0, &raw_tile, false))
                    .unwrap();
                contract_missing(
                    normalized_cache.automatic_headers(stream),
                    "last tile header incomplete",
                );
                normalized_cache
                    .insert(fragment(tile_id, raw_tile.len() as u64, &[], true))
                    .unwrap();
            } else {
                contract_insert(&mut normalized_cache, tile_id, &raw_tile);
            }
        }
        let before = (
            normalized_cache.stored_bytes(),
            normalized_cache.accepted_fragments(),
            normalized_cache.databin_count(),
        );
        eprintln!("automatic_headers: stream={stream}, tiles={tiles}, components={components}");
        let headers = normalized_cache.automatic_headers(stream).unwrap();
        assert_eq!(
            headers.main, main,
            "remove TLM, preserve all other main markers"
        );
        assert_eq!(headers.tile_count, u64::from(tiles));
        assert_eq!(headers.components, components);
        let expected: Vec<_> = (0..tiles).map(|t| contract_tile(t, &retained)).collect();
        assert_eq!(headers.tiles, expected, "reframe each tile and remove PLT");
        assert_eq!(
            normalized_cache.databin_view(main_id).unwrap().as_bytes(),
            raw_main
        );
        assert_eq!(
            before,
            (
                normalized_cache.stored_bytes(),
                normalized_cache.accepted_fragments(),
                normalized_cache.databin_count(),
            ),
            "planning must not rewrite the cache"
        );
    }
}

#[test]
fn automatic_headers_optional_sod_contract() {
    automatic_mapping_decodes_incomplete_precinct_quality_prefixes();
    let mut failures = Vec::new();
    for tiles in [1, 2, 7] {
        for components in [1, 3] {
            for markers in [
                Vec::new(),
                segment(0xff64, &[0, 1, b't']),
                segment(0xff5c, &[0, 0x40]),
            ] {
                let stream = 37;
                let main = contract_main(tiles, components);
                contract_validate_source(&main, tiles, components, &markers);
                let expected: Vec<_> = (0..tiles).map(|t| contract_tile(t, &markers)).collect();
                let mut baseline = None;
                for with_sod in [true, false] {
                    let mut raw = markers.clone();
                    if with_sod {
                        raw.extend_from_slice(&[0xff, 0x93]);
                    }
                    let mut cache = DatabinCache::new(ResourceLimits::default());
                    contract_insert(&mut cache, DatabinId::new(6, stream, 0).unwrap(), &main);
                    for tile in 0..tiles {
                        contract_insert(
                            &mut cache,
                            DatabinId::new(2, stream, u64::from(tile)).unwrap(),
                            &raw,
                        );
                    }
                    let context = format!(
                        "A.3.3 class-2 raw JPP bins: stream={stream}, tiles={tiles}, \
                         components={components}, marker_bytes={}, SOD={with_sod}",
                        markers.len()
                    );
                    eprintln!("{context}");
                    match cache.automatic_headers(stream) {
                        Ok(headers) => {
                            assert_eq!(headers.main, main, "{context}");
                            assert_eq!(headers.tile_count, u64::from(tiles), "{context}");
                            assert_eq!(headers.components, components, "{context}");
                            assert_eq!(
                                headers.tiles, expected,
                                "{context}: exact canonical headers"
                            );
                            if with_sod {
                                baseline = Some(headers.tiles);
                            } else {
                                assert_eq!(
                                    baseline.as_ref(),
                                    Some(&headers.tiles),
                                    "{context}: optional SOD cannot change normalized headers"
                                );
                            }
                        }
                        Err(error) => {
                            failures.push(format!("{context}: rejected legal bin: {error:?}"))
                        }
                    }
                    for tile in 0..tiles {
                        assert_eq!(
                            cache
                                .databin_view(DatabinId::new(2, stream, u64::from(tile)).unwrap())
                                .unwrap()
                                .as_bytes(),
                            raw,
                            "{context}: cache must preserve raw bytes"
                        );
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "automatic header normalization failures:\n{}",
        failures.join("\n")
    );
}
