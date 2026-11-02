// Oracle: ITU-T T.808 (12/2022), A.2.2, Table A.2.
fn standard_class_oracle(code: u64) -> Option<DatabinClass> {
    match code {
        0 | 1 => Some(DatabinClass::Precinct),
        2 => Some(DatabinClass::TileHeader),
        4 | 5 => Some(DatabinClass::TileData),
        6 => Some(DatabinClass::MainHeader),
        8 => Some(DatabinClass::Metadata),
        _ => None,
    }
}

fn class_contract_identities() -> Vec<(u64, u64)> {
    let edges = [0, 1, 127, 128, 255, 256, u32::MAX as u64, u64::MAX - 1, u64::MAX];
    let mut values = Vec::new();
    for codestream in edges {
        for in_class in edges {
            values.push((codestream, in_class));
        }
    }
    let mut seed = 0x4a50_4950_2022_1201_u64;
    for _ in 0..128 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let codestream = seed;
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        values.push((codestream, seed));
    }
    values
}

#[test]
fn standard_assigned_classes_exclude_reserved_signaling() {
    let mut codes: Vec<u64> = (0..=256).collect();
    codes.extend([u32::MAX as u64, u64::MAX - 1, u64::MAX]);
    let mut violations = Vec::new();
    for (codestream, in_class) in class_contract_identities() {
        for &code in &codes {
            let result = DatabinId::new(code, codestream, in_class);
            match (standard_class_oracle(code), result) {
                (Some(expected), Ok(id)) => {
                    assert_eq!(id.class(), expected, "class code={code}, id={id:?}");
                    assert_eq!(id.class_identifier(), code);
                    assert_eq!(id.codestream(), codestream);
                    assert_eq!(id.in_class(), in_class);
                }
                (None, Err(_)) => {}
                (expected, actual) => {
                    if violations.len() < 12 {
                        violations.push(format!(
                            "DatabinId::new({code}, {codestream}, {in_class}) -> {actual:?}; \
                             Table A.2 semantic class expected {expected:?}"
                        ));
                    }
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "reserved signaling exposed as supported, or assigned class rejected:\n{}",
        violations.join("\n")
    );
}

#[test]
fn standard_precinct_forms_share_semantics_and_packet_admission() {
    // Retain and execute the existing explicit/inherited/Aux regression too.
    parses_all_three_standard_header_forms_and_auxiliary();
    for (codestream, in_class) in class_contract_identities() {
        let plain = DatabinId::new(0, codestream, in_class).unwrap();
        let extended = DatabinId::new(1, codestream, in_class).unwrap();
        // Only semantic class is normalized; raw identifiers legitimately differ.
        assert_eq!(plain.class(), DatabinClass::Precinct);
        assert_eq!(extended.class(), DatabinClass::Precinct);
        assert_eq!(plain.class(), extended.class());
        for (code, id) in [(0, plain), (1, extended)] {
            assert_eq!(id.class_identifier(), code);
            assert_eq!(id.codestream(), codestream);
            assert_eq!(id.in_class(), in_class);
            for (layer, offset, length) in [
                (0, 0, 1),
                (1, 127, 128),
                (u16::MAX, u64::MAX - 1, 1),
                (0, 0, u64::MAX),
            ] {
                let packet = JpipPrecinctPacket::new(id, layer, offset, length)
                    .unwrap_or_else(|error| panic!(
                        "valid precinct code={code}, codestream={codestream}, \
                         in_class={in_class}, layer={layer}, offset={offset}, \
                         length={length}: {error:?}"
                    ));
                assert_eq!(packet.range().databin(), id);
                assert_eq!(packet.range().offset(), offset);
                assert_eq!(packet.range().length(), length);
                assert_eq!(packet.layer(), layer);
            }
        }
        for code in [2, 4, 5, 6, 8] {
            let id = DatabinId::new(code, codestream, in_class).unwrap();
            assert_eq!(Some(id.class()), standard_class_oracle(code));
            assert_ne!(id.class(), DatabinClass::Precinct);
            assert_eq!(
                JpipPrecinctPacket::new(id, 0, 17, 1),
                Err(DecodeError::malformed(17, MalformedKind::InvalidJpipLayout)),
                "non-precinct code={code} must not pass the semantic class guard"
            );
        }
    }
}
