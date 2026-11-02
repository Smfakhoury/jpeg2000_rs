// Included in jpip::tests; all access to private helpers is test-only.

fn normalization_contract_tiles() -> Vec<u16> {
    let mut tiles = vec![0, 1, 254, 255, 256, 257, 32767, 32768, 65533, 65534];
    let mut seed = 0x944e_3d63_u32;
    for _ in 0..12 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        tiles.push(u16::try_from(seed % 65535).unwrap());
    }
    tiles
}

fn normalization_contract_expected(tile: u16, index: u8, count: u8, retained: &[u8]) -> Vec<u8> {
    let mut expected = vec![0xff, 0x90, 0x00, 0x0a];
    expected.extend_from_slice(&tile.to_be_bytes());
    expected.extend_from_slice(&[0, 0, 0, 0, index, count]);
    expected.extend_from_slice(retained);
    expected.extend_from_slice(&[0xff, 0x93]);
    expected
}

#[test]
fn automatic_normalization_accepts_optional_wire_sod_contract() {
    // T.808 A.3.3: class 2, no SOT, complete even when empty, optional SOD.
    // COM uses Rcom=1 and Latin text; mandatory coding parameters are inherited
    // from a main header, so none need to appear in an otherwise empty tile bin.
    let mut headers = vec![Vec::new()];
    for length in [1, 2, 17, 127, 255, 1024] {
        let mut payload = vec![0, 1];
        payload.extend((0..length).map(|i| b'a' + u8::try_from(i % 26).unwrap()));
        headers.push(segment(0xff64, &payload));
    }
    headers.push(segment(0xff52, &[0, 0, 0, 1, 0, 0, 0, 0, 0, 1]));
    let mut failures = Vec::new();
    for tile in normalization_contract_tiles() {
        for retained in &headers {
            let expected = vec![normalization_contract_expected(tile, 0, 1, retained)];
            let mut paired_outputs = Vec::new();
            for sod in [true, false] {
                let mut wire = retained.clone();
                if sod {
                    wire.extend_from_slice(&[0xff, 0x93]);
                }
                assert!(!wire.windows(2).any(|bytes| bytes == [0xff, 0x90]));
                let target = id(2, u64::from(tile));
                let mut cache = DatabinCache::new(ResourceLimits::default());
                cache.insert(fragment(target, 0, &wire, true)).unwrap();
                assert!(cache.is_complete(target), "complete, not absent/incomplete");
                let source = cache.databin_view(target).unwrap().into_bytes();
                assert_eq!(source, wire);
                let result = normalize_automatic_tile_headers(&source, tile);
                match result {
                    Ok(actual) => {
                        if actual != expected {
                            failures.push(format!(
                                "tile={tile} retained={} SOD={sod}: expected {expected:02x?}, got {actual:02x?}",
                                retained.len()
                            ));
                        }
                        paired_outputs.push(actual);
                    }
                    Err(error) => failures.push(format!(
                        "tile={tile} retained={} SOD={sod}: valid complete class-2 bin rejected: {error:?}",
                        retained.len()
                    )),
                }
            }
            // Only optional wire framing differs; reconstructed bytes must not.
            if paired_outputs.len() == 2 {
                assert_eq!(paired_outputs[0], paired_outputs[1]);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} contract failures; first examples:\n{}",
        failures.len(),
        failures
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn automatic_normalization_sot_fields_contract() {
    // Retain and execute the existing regression's original fixture/assertions.
    automatic_header_normalization_removes_reframed_markers();
    for tile in normalization_contract_tiles() {
        let unframed = [0xff, 0x93];
        assert_eq!(
            normalize_automatic_tile_headers(&unframed, tile).unwrap(),
            vec![normalization_contract_expected(tile, 0, 1, &[])],
            "single SOT-free transition, tile={tile}"
        );
        // T.800 A.4.2 boundaries: Isot<=65534, TPsot<=254, TNsot<=255.
        // SOT-bearing inputs are the project's accepted extension, not JPP
        // wire bins. Source TNsot=0 is legal; reconstruction knows the count.
        for count in 1_u16..=255 {
            let mut source = Vec::new();
            let mut expected = Vec::new();
            for index in 0..count {
                let index = u8::try_from(index).unwrap();
                let count = u8::try_from(count).unwrap();
                let mut retained = Vec::new();
                if index % 3 == 1 {
                    retained.extend(segment(
                        0xff64,
                        &[0, 1, b'p', b'a', b'r', b't', b'A' + index % 26],
                    ));
                } else if index == 0 && count % 2 == 0 {
                    retained.extend(segment(0xff52, &[0, 0, 0, 1, 0, 0, 0, 0, 0, 1]));
                }
                let mut header = vec![0xff, 0x90, 0, 10];
                header.extend_from_slice(&tile.to_be_bytes());
                let length = u32::try_from(14 + retained.len()).unwrap();
                assert!(length >= 14);
                header.extend_from_slice(&length.to_be_bytes());
                header.extend_from_slice(&[index, 0]);
                header.extend_from_slice(&retained);
                header.extend_from_slice(&[0xff, 0x93]);
                source.extend_from_slice(&header);
                expected.push(normalization_contract_expected(
                    tile, index, count, &retained,
                ));
            }
            let actual = normalize_automatic_tile_headers(&source, tile)
                .unwrap_or_else(|error| panic!("tile={tile}, count={count}: {error:?}"));
            assert_eq!(
                actual, expected,
                "tile={tile}, count={count}: complete SOT/SOD headers, retained markers, indices, known counts and zero Psot"
            );
        }
    }
}
