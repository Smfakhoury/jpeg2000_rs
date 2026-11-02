// Included in jpip::tests to reuse the original fixtures without public API changes.
fn snapshot_spec_sot(bytes: &[u8]) -> usize {
    bytes.windows(2).position(|w| w == [0xff, 0x90]).unwrap()
}

fn snapshot_spec_cache(main: &[u8], header: &[u8], packets: &[u8]) -> DatabinCache {
    assert!(main.starts_with(&SOC));
    assert_eq!(&main[6..8], &[0, 0], "Part 1 Rsiz, no capability extension");
    assert!(!header.windows(2).any(|w| w == [0xff, 0x90]),
        "T.808 A.3.3: JPP tile-header bins exclude SOT");
    let mut cache = DatabinCache::new(ResourceLimits::default());
    cache.insert(fragment(id(6, 0), 0, main, true)).unwrap();
    cache.insert(fragment(id(2, 0), 0, header, true)).unwrap();
    if !packets.is_empty() {
        cache.insert(fragment(id(0, 0), 0, packets, false)).unwrap();
    }
    assert!(cache.is_complete(id(6, 0)));
    assert!(cache.is_complete(id(2, 0)));
    cache
}

fn snapshot_spec_call(cache: &DatabinCache) -> Result<AutomaticSnapshotResult, DecodeError> {
    cache.build_automatic_snapshot(
        0, None, &JpipPacketSelection::all_components(), &ResourceLimits::default(),
    )
}

fn snapshot_spec_missing(result: AutomaticSnapshotResult, expected: &[(DatabinId, u64, Option<u64>)]) {
    assert!(result.ready.is_none(), "unavailable headers must not produce decoder input");
    let actual: Vec<_> = result.missing.iter()
        .map(|r| (r.databin(), r.offset(), r.length())).collect();
    assert_eq!(actual, expected, "header availability and missing ranges");
}

#[test]
fn snapshot_spec_reconstructed_lengths() {
    // Retain and execute the existing regression combinations unchanged.
    automatic_mapping_decodes_incomplete_precinct_quality_prefixes();
    automatic_mapping_interleaves_multiple_tiles_and_collapses_tile_parts();
    automatic_executes_functional_later_tile_part_without_plt();

    let mut seed = 0x06a9_48d1_u32;
    let mut layers_to_test = vec![1_u16, 2, 3, 7, 16, 31, 63];
    for _ in 0..8 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        layers_to_test.push(1 + (seed % 63) as u16);
    }
    for layers in layers_to_test {
        for progression in 0..=4 {
            let mut original = decodable_codestream_with_layers(layers);
            let cod = original.windows(2).position(|w| w == [0xff, 0x52]).unwrap();
            original[cod + 5] = progression;
            let sot = snapshot_spec_sot(&original);
            // A legal binary COM in the tile header must survive normalization.
            let comment = segment(0xff64, &[0, 0, progression, layers as u8]);
            let mut header = comment.clone();
            header.extend_from_slice(&[0xff, 0x93]);
            let mut expected = original.clone();
            expected.splice(sot + 12..sot + 12, comment.clone());
            let length = 14 + comment.len() + usize::from(layers);
            expected[sot + 6..sot + 10].copy_from_slice(&(length as u32).to_be_bytes());
            for received in [0, 1, layers / 2, layers.saturating_sub(1), layers] {
                eprintln!("build_automatic_snapshot: length route layers={layers} progression={progression} received={received}");
                let packets = vec![0; usize::from(received)];
                let cache = snapshot_spec_cache(&original[..sot], &header, &packets);
                let result = snapshot_spec_call(&cache).unwrap();
                assert!(result.missing.is_empty());
                let ready = result.ready.expect("all header bins are complete");
                assert_eq!(ready.layers, layers);
                assert_eq!(ready.available_layers, received);
                if received == layers {
                    assert!(ready.missing.is_empty());
                } else {
                    assert_eq!(ready.missing, vec![JpipMissingRange {
                        databin: id(0, 0), offset: u64::from(received), length: None,
                    }]);
                }
                if received == 0 {
                    assert!(ready.codestream.as_bytes().is_empty(),
                        "zero available layers is not a decodable codestream");
                    continue;
                }
                let output = ready.codestream.as_bytes();
                // Empty packets are one zero byte at every layer. Unavailable
                // layers are padded identically, not removed from COD's domain.
                assert_eq!(output, expected, "independent fixture oracle");
                assert_eq!(&output[sot..sot + 6], &[0xff, 0x90, 0, 10, 0, 0]);
                let psot = u32::from_be_bytes(output[sot + 6..sot + 10].try_into().unwrap());
                assert_eq!(psot as usize, length, "T.800 A.4.2 Psot includes SOT, excludes EOC");
                assert_eq!(sot + psot as usize, output.len() - 2);
                assert_eq!(&output[output.len() - 2..], &EOC);
                assert_eq!(&output[sot + 10..sot + 12], &[0, 1]);
                assert_eq!(&output[sot + 12..sot + 12 + comment.len()], &comment);
                assert_eq!(&output[sot + 12 + comment.len()..sot + 14 + comment.len()], &[0xff, 0x93]);

                if received == layers {
                    for bound in [expected.len() - 1, expected.len(), expected.len() + 1] {
                        let limits = ResourceLimits::default()
                            .with_maximum_input_bytes(bound as u64).unwrap();
                        let result = cache.build_automatic_snapshot(
                            0, None, &JpipPacketSelection::all_components(), &limits,
                        );
                        if bound < expected.len() {
                            assert!(matches!(result, Err(DecodeError::LimitExceeded {
                                resource: Resource::InputBytes, limit, actual,
                            }) if limit == bound as u64 && actual == expected.len() as u64),
                                "one-byte-over-bound output must return the precise input-byte limit error");
                        } else {
                            assert_eq!(result.unwrap().ready.unwrap().codestream.as_bytes(), expected);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn snapshot_spec_received_header_normalization() {
    automatic_header_normalization_removes_reframed_markers();
    automatic_mapping_supports_progressions_poc_sop_eph_and_reframed_headers();
    let mut failures = Vec::new();
    for layers in [1_u16, 2, 7] {
        for progression in 0..=4 {
            let mut original = decodable_codestream_with_layers(layers);
            let cod = original.windows(2).position(|w| w == [0xff, 0x52]).unwrap();
            original[cod + 5] = progression;
            let sot = snapshot_spec_sot(&original);
            for with_comment in [false, true] {
                let markers = if with_comment {
                    segment(0xff64, &[0, 0, 17, 29])
                } else {
                    Vec::new()
                };
                let mut expected = original.clone();
                expected.splice(sot + 12..sot + 12, markers.clone());
                let length = 14 + markers.len() + usize::from(layers);
                expected[sot + 6..sot + 10].copy_from_slice(&(length as u32).to_be_bytes());
                let mut with_sod_output = None;
                // T.808 A.3.3 permits SOD but does not require it. These
                // executions differ only in optional input framing; their
                // normalized codestream bytes must be identical.
                for include_sod in [true, false] {
                    let mut header = markers.clone();
                    if include_sod {
                        header.extend_from_slice(&[0xff, 0x93]);
                    }
                    eprintln!("build_automatic_snapshot: JPP header layers={layers} progression={progression} comment={with_comment} SOD={include_sod}, header={header:?}");
                    let cache = snapshot_spec_cache(
                        &original[..sot], &header, &vec![0; usize::from(layers)],
                    );
                    match snapshot_spec_call(&cache) {
                        Ok(result) => {
                            assert!(result.missing.is_empty());
                            let ready = result.ready.expect("received complete header is not missing");
                            assert_eq!(ready.available_layers, layers);
                            assert_eq!(ready.layers, layers);
                            assert!(ready.missing.is_empty());
                            assert_eq!(ready.codestream.as_bytes(), expected,
                                "automatic path must synthesize correct SOT/SOD and Psot");
                            if include_sod {
                                with_sod_output = Some(ready.codestream.into_bytes());
                            } else {
                                assert_eq!(ready.codestream.as_bytes(),
                                    with_sod_output.as_ref().unwrap(),
                                    "optional SOD cannot change normalized decoder input");
                            }
                        }
                        Err(error) => failures.push(format!(
                            "legal complete JPP header rejected: layers={layers} progression={progression} comment={with_comment} SOD={include_sod}: {error}"
                        )),
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn snapshot_spec_unavailable_headers() {
    let original = two_tile_codestream();
    let sot = snapshot_spec_sot(&original);
    assert_eq!(&original[6..8], &[0, 0], "Part 1 Rsiz, no capability extension");
    let mut cache = DatabinCache::new(ResourceLimits::default());
    // Image packets can arrive first; this is precisely the A.3.3 warning.
    cache.insert(fragment(id(0, 0), 0, &[0], false)).unwrap();
    cache.insert(fragment(id(0, 1), 0, &[0], false)).unwrap();
    snapshot_spec_missing(snapshot_spec_call(&cache).unwrap(), &[(id(6, 0), 0, None)]);
    for prefix in [1_usize, 2, 7, sot - 1] {
        eprintln!("build_automatic_snapshot: main header incomplete prefix={prefix}");
        cache.insert(fragment(id(6, 0), 0, &original[..prefix], false)).unwrap();
        snapshot_spec_missing(snapshot_spec_call(&cache).unwrap(),
            &[(id(6, 0), prefix as u64, None)]);
    }
    cache.insert(fragment(id(6, 0), sot as u64, &[], true)).unwrap();
    snapshot_spec_missing(snapshot_spec_call(&cache).unwrap(),
        &[(id(6, 0), (sot - 1) as u64, Some(1))]);
    cache.insert(fragment(id(6, 0), (sot - 1) as u64, &original[sot - 1..sot], false)).unwrap();
    snapshot_spec_missing(snapshot_spec_call(&cache).unwrap(),
        &[(id(2, 0), 0, None), (id(2, 1), 0, None)]);

    // A known final length does not imply a contiguous received header.
    cache.insert(fragment(id(2, 0), 1, &[0x93], true)).unwrap();
    snapshot_spec_missing(snapshot_spec_call(&cache).unwrap(),
        &[(id(2, 0), 0, Some(2)), (id(2, 1), 0, None)]);
    cache.insert(fragment(id(2, 0), 0, &[0xff], false)).unwrap();
    snapshot_spec_missing(snapshot_spec_call(&cache).unwrap(), &[(id(2, 1), 0, None)]);
    cache.insert(fragment(id(2, 1), 0, &[0xff], false)).unwrap();
    snapshot_spec_missing(snapshot_spec_call(&cache).unwrap(), &[(id(2, 1), 1, None)]);
    cache.insert(fragment(id(2, 1), 1, &[0x93], false)).unwrap();
    snapshot_spec_missing(snapshot_spec_call(&cache).unwrap(), &[(id(2, 1), 2, None)]);
    cache.insert(fragment(id(2, 1), 2, &[], true)).unwrap();
    eprintln!("build_automatic_snapshot: both SIZ-declared tile headers now complete");
    let result = snapshot_spec_call(&cache).unwrap();
    assert!(result.missing.is_empty());
    let ready = result.ready.expect("fully received headers must leave the waiting barrier");
    assert_eq!(ready.available_layers, 1);
    assert_eq!(ready.layers, 1);
    assert!(ready.missing.is_empty());
    assert_eq!(ready.codestream.as_bytes(), original);
    // Observation is passive: repeated reconstruction must not consume bins.
    assert_eq!(snapshot_spec_call(&cache).unwrap().ready.unwrap().codestream.as_bytes(), original);
    assert_eq!(cache.databin_view(id(2, 0)).unwrap().as_bytes(), &[0xff, 0x93]);
    assert_eq!(cache.databin_view(id(2, 1)).unwrap().as_bytes(), &[0xff, 0x93]);
}
