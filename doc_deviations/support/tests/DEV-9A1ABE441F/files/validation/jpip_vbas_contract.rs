// T.808 (12/2022), A.2.1: concatenate payload bits, without a shortest-form
// requirement. Bin-ID reserves first-byte bits 4..6 for flags. The supported
// implementation profile bounds fields to ten bytes and values to u64.
#[test]
fn bounded_standard_vbas_contract() {
    fn encode(value: u128, first_bits: u32) -> Vec<u8> {
        let mut tail = Vec::new();
        let mut rest = value;
        while rest >= (1_u128 << first_bits) {
            tail.push(u8::try_from(rest % 128).unwrap());
            rest /= 128;
        }
        let mut bytes = vec![u8::try_from(rest).unwrap()];
        bytes.extend(tail.into_iter().rev());
        let last = bytes.len() - 1;
        for byte in &mut bytes[..last] {
            *byte |= 0x80;
        }
        bytes
    }

    fn check_value(field: &[u8], mask: Option<u8>, expected: u64, failures: &mut Vec<String>) {
        assert!(!field.is_empty() && field.len() <= 10);
        assert!(field[..field.len() - 1].iter().all(|b| b & 0x80 != 0));
        assert_eq!(field[field.len() - 1] & 0x80, 0);
        for prefix in [0, 3] {
            // Delimit with an entire second field: the first call must neither
            // consume its bytes nor let their payload affect the first value.
            let mut input = vec![0x55; prefix];
            input.extend_from_slice(field);
            input.extend_from_slice(&[0x81, 0x02, 0x5a]);
            let mut position = prefix;
            let actual = read_vbas(&input, &mut position, mask);
            if actual != Ok(expected) || position != prefix + field.len() {
                failures.push(format!(
                    "read_vbas mask={mask:?}, field={field:02x?}, start={prefix}: \
                     expected Ok({expected}) at {}, got {actual:?} at {position}",
                    prefix + field.len()
                ));
            } else {
                assert_eq!(read_vbas(&input, &mut position, None), Ok(130));
                assert_eq!(position, prefix + field.len() + 2);
                assert_eq!(input[position], 0x5a);
            }
        }
    }

    fn check_error(field: &[u8], mask: Option<u8>, kind: MalformedKind) {
        for prefix in [0, 3] {
            let mut input = vec![0x55; prefix];
            input.extend_from_slice(field);
            let mut position = prefix;
            let actual = read_vbas(&input, &mut position, mask);
            let offset = if kind == MalformedKind::Truncated {
                input.len()
            } else {
                prefix
            };
            assert_eq!(
                actual,
                Err(DecodeError::MalformedInput {
                    offset: offset as u64,
                    kind,
                }),
                "read_vbas rejection: field={field:02x?}, mask={mask:?}, start={prefix}"
            );
        }
    }

    let mut failures = Vec::new();
    let mut values: Vec<u64> = (0..=255).collect();
    values.extend([u64::MAX - 1, u64::MAX]);
    for bit in 1..64 {
        let boundary = 1_u64 << bit;
        values.extend([boundary - 1, boundary, boundary + 1]);
    }
    let mut seed = 0x6a09_e667_f3bc_c909_u64;
    for _ in 0..256 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        values.push(seed);
    }
    for value in values {
        let ordinary = encode(u128::from(value), 7);
        check_value(&ordinary, None, value, &mut failures);
        let bin = encode(u128::from(value), 4);
        for flags in [0x20, 0x30, 0x40, 0x50, 0x60, 0x70] {
            let mut flagged = bin.clone();
            flagged[0] |= flags;
            assert!((1..=3).contains(&((flagged[0] >> 5) & 3)));
            check_value(&flagged, Some(0x0f), value, &mut failures);
        }
    }

    // Error paths are reached with small, real slices, not impossible cursors.
    for mask in [None, Some(0x0f)] {
        let full = encode(u128::from(u64::MAX), if mask.is_some() { 4 } else { 7 });
        for length in 0..full.len() {
            check_error(&full[..length], mask, MalformedKind::Truncated);
        }
        for value in [1_u128 << 64, (1_u128 << 64) + 1, (1_u128 << 67) - 1] {
            let too_large = encode(value, if mask.is_some() { 4 } else { 7 });
            assert_eq!(too_large.len(), 10);
            check_error(&too_large, mask, MalformedKind::InvalidJpipVbas);
        }
    }
    let mut too_long = vec![0x80; 10];
    too_long.push(0);
    check_error(&too_long, Some(0x0f), MalformedKind::InvalidJpipVbas);

    // Leading zero groups do not change either the payload or the field's
    // terminator. Keep every alternate encoding inside the ten-byte profile.
    for value in [0_u64, 1, 15, 16, 127, 128, 16383, 16384, 1 << 56] {
        let ordinary = encode(u128::from(value), 7);
        for zero_groups in 1..=10 - ordinary.len() {
            let mut padded = vec![0x80; zero_groups];
            padded.extend_from_slice(&ordinary);
            check_value(&padded, None, value, &mut failures);
            padded[0] |= 0x60; // Independent Bin-ID: explicit Class and CSn.
            check_value(&padded, Some(0x0f), value, &mut failures);
        }
    }

    // Also demonstrate the disputed encoding in a complete legal header:
    // independent form, metadata class 8 (JPP or JPT), CSn 0, offset 0,
    // length 1, no Aux, not final. No capability marker or inherited state
    // is required by A.2.1/A.2.2 for this binary message.
    parses_all_three_standard_header_forms_and_auxiliary();
    for (class, length) in [
        (&[8][..], &[1][..]),
        (&[0x80, 8][..], &[1][..]),
        (&[8][..], &[0x80, 1][..]),
    ] {
        let mut input = vec![0x60];
        input.extend_from_slice(class);
        input.extend_from_slice(&[0, 0]);
        input.extend_from_slice(length);
        let header_length = input.len();
        input.extend_from_slice(&[0xab, 0x5a]);
        assert_eq!((input[0] >> 5) & 3, 3);
        let mut context = JpipHeaderContext::new();
        match parse_jpip_message(&input, &mut context, &ResourceLimits::default()) {
            Ok((JpipMessage::Data { header, body }, consumed)) => {
                assert_eq!(header.databin(), DatabinId::new(8, 0, 0).unwrap());
                assert_eq!(header.offset(), 0);
                assert_eq!(header.length(), 1);
                assert_eq!(header.auxiliary(), None);
                assert!(!header.is_last_byte());
                assert_eq!(header.header_length(), header_length);
                assert_eq!(body, [0xab]);
                assert_eq!(consumed, header_length + 1);
                assert_eq!(input[consumed], 0x5a);
                assert_eq!(context.previous_class(), Some(8));
                assert_eq!(context.previous_codestream(), Some(0));
            }
            other => failures.push(format!(
                "parse_jpip_message -> read_vbas, independent metadata header \
                 {input:02x?}: expected value-preserving Data, got {other:?}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} standard VBAS checks failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
