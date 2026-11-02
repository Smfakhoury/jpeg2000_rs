// Oracle: pinned T.808 (12/2022), A.2.2, Table A.2.
// This tests classification, not wire framing or the recommended skipping policy.
#[test]
fn class_identifiers_follow_t808_table_a2() {
    let defined = [
        (0, DatabinClass::Precinct),
        (1, DatabinClass::Precinct),
        (2, DatabinClass::TileHeader),
        (4, DatabinClass::TileData),
        (5, DatabinClass::TileData),
        (6, DatabinClass::MainHeader),
        (8, DatabinClass::Metadata),
    ];
    let mut failures = Vec::new();
    let mut checks = 0;
    let mut check = |identifier, expected| {
        let actual = DatabinClass::from_identifier(identifier);
        checks += 1;
        if actual != expected {
            failures.push(format!(
                "DatabinClass::from_identifier({identifier}): expected {expected:?}, got {actual:?}"
            ));
        }
    };

    // JPP-only precinct/tile-header and JPT-only tile-data classes are
    // independent calls, not a mixed-stream fixture. No extension is negotiated.
    for (identifier, class) in defined {
        check(identifier, Some(class));
    }
    for identifier in 0..=1024 {
        if ![0, 1, 2, 4, 5, 6, 8].contains(&identifier) {
            check(identifier, None);
        }
    }
    for bit in 4..64 {
        let boundary = 1_u64 << bit;
        for identifier in [boundary - 1, boundary, boundary + 1] {
            check(identifier, None);
        }
    }
    for identifier in [u64::MAX - 1, u64::MAX] {
        check(identifier, None);
    }
    let mut seed = 0x4827_1799_8449_fb68_u64;
    for _ in 0..1024 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        if seed > 8 {
            check(seed, None);
        }
    }
    eprintln!(
        "route=jpip::tests -> DatabinClass::from_identifier; \
         oracle=T.808(12/2022) A.2.2 Table A.2; \
         no negotiated reserved-class extension; {checks} exact checks"
    );
    assert!(
        failures.is_empty(),
        "semantic classification mismatches:\n{}",
        failures.join("\n")
    );
}
