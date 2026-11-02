// Included inside jpip::tests. Oracle: T.808 (12/2022), A.3.6.3/Table A.3.

fn contract_header(extended: bool, box_type: [u8; 4], length: u64) -> Vec<u8> {
    let mut bytes = if extended {
        1_u32.to_be_bytes().to_vec()
    } else {
        u32::try_from(length).unwrap().to_be_bytes().to_vec()
    };
    bytes.extend_from_slice(&box_type);
    if extended {
        assert!(length >= 16);
        bytes.extend_from_slice(&length.to_be_bytes());
    } else {
        assert!(length == 0 || length >= 8);
    }
    bytes
}

fn contract_payload(
    flags: u32,
    original_id: u64,
    original: &[u8],
    equivalent: Option<(u64, &[u8])>,
    stream: Option<u64>,
    count: Option<u32>,
) -> Vec<u8> {
    let mut payload = flags.to_be_bytes().to_vec();
    payload.extend_from_slice(&original_id.to_be_bytes());
    payload.extend_from_slice(original);
    if let Some((id, header)) = equivalent {
        payload.extend_from_slice(&id.to_be_bytes());
        payload.extend_from_slice(header);
    }
    if let Some(id) = stream {
        assert!(equivalent.is_some(), "intermediate fields must be present");
        payload.extend_from_slice(&id.to_be_bytes());
    }
    if let Some(n) = count {
        assert!(stream.is_some());
        payload.extend_from_slice(&n.to_be_bytes());
    }
    // The outer phld MUST use LBox, not XLBox. Only OrigBH/EquivBH may be wide.
    let framed = box32(*b"phld", &payload);
    assert_eq!(
        u32::from_be_bytes(framed[..4].try_into().unwrap()) as usize,
        framed.len()
    );
    assert_ne!(&framed[..4], &1_u32.to_be_bytes());
    assert_eq!(&framed[4..8], b"phld");
    let mut reader = crate::jp2::BoxReader::new(&framed, &ResourceLimits::default()).unwrap();
    let received = reader.next_box_ref().unwrap().unwrap();
    assert_eq!(received.payload, payload);
    assert!(reader.next_box_ref().is_none());
    payload
}

fn contract_rejects(payload: &[u8], context: &str) {
    assert!(
        matches!(
            parse_jpip_placeholder(payload),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidBoxContents | MalformedKind::Truncated,
                ..
            })
        ),
        "{context}: required field missing or invalid, payload={payload:02x?}"
    );
}

fn contract_values() -> Vec<u64> {
    let mut values = vec![
        0,
        1,
        255,
        256,
        u32::MAX as u64,
        1_u64 << 32,
        0x0102_0304_0506_0708,
    ];
    let mut seed = 0x53b1_c9a4_672e_108f_u64;
    for _ in 0..16 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        values.push(seed >> 8);
    }
    values
}

#[test]
fn placeholder_contract_stream_identifiers_and_counts() {
    eprintln!("route: native unit -> parse_jpip_placeholder; A.3.6.3 01xx/11xx CSID/NCS");
    let mut failures = Vec::new();
    for extended in [false, true] {
        let header = contract_header(
            extended,
            *b"jp2c",
            if extended { 0x1_0000_0010 } else { 256 },
        );
        for stream in contract_values() {
            for flags in [4_u32, 5, 6, 7, 12, 13, 14, 15] {
                let counts = if flags & 8 == 0 {
                    vec![None, Some(0), Some(1), Some(2), Some(u32::MAX)]
                } else {
                    vec![
                        Some(1),
                        Some(2),
                        Some(255),
                        Some(u32::MAX),
                        Some((stream as u32).max(1)),
                    ]
                };
                for count in counts {
                    // Bound the last contiguous ID independently, without allocating streams.
                    let expected = if flags & 8 == 0 { 1 } else { count.unwrap() };
                    assert!(stream.checked_add(u64::from(expected) - 1).is_some());
                    let payload = contract_payload(
                        flags,
                        7,
                        &header,
                        Some((9, &header)),
                        Some(stream),
                        count,
                    );
                    match parse_jpip_placeholder(&payload) {
                        Ok(parsed) => {
                            assert_eq!(parsed.stream, Some(stream), "big-endian CSID, flags={flags}");
                            assert_eq!(parsed.stream_count, expected, "NCS, flags={flags}, encoded={count:?}");
                        }
                        Err(error) => failures.push(format!(
                            "flags={flags:#x}, CSID={stream:#x}, NCS={count:?}, wide={extended}: {error:?}"
                        )),
                    }
                }
            }
        }
        let invalid = contract_payload(12, 7, &header, Some((9, &header)), Some(0), Some(0));
        assert!(
            matches!(
                parse_jpip_placeholder(&invalid),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidBoxContents,
                    ..
                })
            ),
            "11xx requires one or more streams"
        );
        for flags in [4, 12] {
            let payload = contract_payload(
                flags,
                7,
                &header,
                Some((9, &header)),
                Some(123),
                (flags == 12).then_some(2),
            );
            let required_end = payload.len();
            let stream_start = 12 + header.len() + 8 + header.len();
            for end in stream_start..required_end {
                contract_rejects(&payload[..end], "truncated required CSID/NCS");
            }
        }
        let no_stream = contract_payload(1, 7, &header, None, None, None);
        let parsed = parse_jpip_placeholder(&no_stream).unwrap();
        assert_eq!(parsed.stream, None);
        assert_eq!(parsed.stream_count, 0);
    }
    for stream in [u64::MAX - 1, u64::MAX] {
        let header = contract_header(false, *b"jp2c", 256);
        let payload = contract_payload(4, 0, &header, Some((0, &header)), Some(stream), None);
        let parsed = parse_jpip_placeholder(&payload).unwrap();
        assert_eq!(parsed.stream, Some(stream));
        assert_eq!(parsed.stream_count, 1);
    }
    assert!(
        failures.is_empty(),
        "{} legal placeholders rejected; first failures:\n{}",
        failures.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn placeholder_contract_original_header_ownership() {
    eprintln!(
        "route: native unit -> parse_jpip_placeholder; exact OrigBH bytes, type, and owned lifetime"
    );
    for box_type in [*b"free", *b"abcd", *b"wxyz"] {
        for (extended, mut lengths) in [
            (false, vec![0, 8, 9, 65536, u32::MAX as u64]),
            (true, vec![16, 17, u32::MAX as u64, 1_u64 << 32, u64::MAX]),
        ] {
            lengths.extend(contract_values().into_iter().map(|value| {
                if extended {
                    value.max(16)
                } else {
                    u64::from((value as u32).max(8))
                }
            }));
            for length in lengths {
                let expected = contract_header(extended, box_type, length);
                let mut payload =
                    contract_payload(1, 0x0123_4567_89ab_cdef, &expected, None, None, None);
                let parsed = parse_jpip_placeholder(&payload).unwrap();
                assert_eq!(parsed.original_header, expected);
                assert_eq!(parsed.original_type, crate::jp2::BoxType::new(box_type));
                assert_eq!(parsed.bin, 0x0123_4567_89ab_cdef);
                for end in 0..payload.len() {
                    contract_rejects(&payload[..end], "incomplete Flags/OrigID/OrigBH");
                }
                payload.fill(0xa5);
                drop(payload);
                assert_eq!(
                    parsed.original_header, expected,
                    "OrigBH must outlive and not alias input"
                );
                assert_eq!(parsed.original_type, crate::jp2::BoxType::new(box_type));
            }
        }
    }
}

fn contract_assert_semantics(
    parsed: &JpipPlaceholder,
    flags: u32,
    original_id: u64,
    original: &[u8],
    equivalent_id: u64,
    equivalent: &[u8],
    stream: u64,
) {
    assert_eq!(parsed.original_header, original);
    assert_eq!(parsed.original_type, crate::jp2::BoxType::new(*b"jp2c"));
    // OrigID/EquivID are semantically meaningful only when their access bit is set.
    if flags & 1 != 0 {
        assert_eq!(parsed.bin, original_id);
    }
    if flags & 2 != 0 {
        assert_eq!(parsed.equivalent_bin, Some(equivalent_id));
        assert_eq!(parsed.equivalent_header.as_deref(), Some(equivalent));
        assert_eq!(
            parsed.equivalent_type,
            Some(crate::jp2::BoxType::new(*b"jp2c"))
        );
    }
    assert_eq!(parsed.stream, (flags & 4 != 0).then_some(stream));
    assert_eq!(
        parsed.stream_count,
        if flags & 8 != 0 {
            3
        } else {
            u32::from(flags & 4 != 0)
        }
    );
}

#[test]
fn placeholder_contract_received_semantics_ignore_unused_bits() {
    eprintln!(
        "route: native unit -> receiver parser; A.3.6.3 Table A.3 unused y bits are ignored by clients"
    );
    let mut failures = Vec::new();
    for original_wide in [false, true] {
        for equivalent_wide in [false, true] {
            let original = contract_header(
                original_wide,
                *b"jp2c",
                if original_wide {
                    0x1_0203_0405
                } else {
                    0x0102_0304
                },
            );
            let equivalent = contract_header(
                equivalent_wide,
                *b"jp2c",
                if equivalent_wide {
                    0x2_0304_0506
                } else {
                    0x0203_0405
                },
            );
            for flags in [0_u32, 1, 2, 3, 4, 5, 6, 7, 12, 13, 14, 15] {
                let original_id = 0x0123_4567_89ab_cdef;
                let equivalent_id = 0xfedc_ba98_7654_3210;
                let stream = 0x1234_5678_9abc_def0;
                let payload = contract_payload(
                    flags,
                    original_id,
                    &original,
                    (flags & 6 != 0).then_some((equivalent_id, equivalent.as_slice())),
                    (flags & 4 != 0).then_some(stream),
                    (flags & 8 != 0).then_some(3),
                );
                let baseline = parse_jpip_placeholder(&payload).unwrap();
                contract_assert_semantics(
                    &baseline,
                    flags,
                    original_id,
                    &original,
                    equivalent_id,
                    &equivalent,
                    stream,
                );
                let mut masks: Vec<u32> = (4..32).map(|bit| 1_u32 << bit).collect();
                masks.extend([0xffff_fff0, 0x5555_5550, 0xaaaa_aaa0]);
                for mask in masks {
                    let mut received = payload.clone();
                    received[..4].copy_from_slice(&(flags | mask).to_be_bytes());
                    // Receiver robustness relation, not permission for servers to set y bits.
                    // Only unused bits differ; headers, IDs, stream/count must not change.
                    match parse_jpip_placeholder(&received) {
                        Ok(parsed) => {
                            contract_assert_semantics(&parsed, flags, original_id, &original, equivalent_id, &equivalent, stream);
                            assert_eq!(parsed.original_header, baseline.original_header);
                            assert_eq!(parsed.stream, baseline.stream);
                            assert_eq!(parsed.stream_count, baseline.stream_count);
                        }
                        Err(error) => failures.push(format!(
                            "low flags={flags:#x}, unused mask={mask:#x}, widths={original_wide}/{equivalent_wide}: {error:?}"
                        )),
                    }
                }
                if flags & 2 != 0 {
                    let equivalent_start = 12 + original.len();
                    for end in equivalent_start..equivalent_start + 8 + equivalent.len() {
                        contract_rejects(&payload[..end], "required intermediate EquivID/EquivBH");
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} receiver placeholders rejected solely for unused bits; first failures:\n{}",
        failures.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
