// Included inside jpip::tests by validation/run_jpip_contract.py.
// T.808 (12/2022), A.2.1/A.2.2: ordered Annex-A message headers, not
// HTTP-UDP chunks. D.3 supplies the separate EOR framing and opaque body.

fn contract_vbas(mut value: u64) -> Vec<u8> {
    let mut bytes = vec![(value % 128) as u8];
    value /= 128;
    while value != 0 {
        bytes.push((value % 128) as u8 | 0x80);
        value /= 128;
    }
    bytes.reverse();
    bytes
}

#[derive(Clone, Debug)]
struct ContractData {
    class: u64,
    stream: u64,
    bin: u64,
    offset: u64,
    last: bool,
    auxiliary: Option<u64>,
    body: Vec<u8>,
}

fn contract_data_bytes(data: &ContractData, form: u8) -> (Vec<u8>, usize) {
    assert!((1..=3).contains(&form));
    assert!([0, 1, 2, 4, 5, 6, 8].contains(&data.class));
    assert_eq!(data.auxiliary.is_some(), data.class % 2 == 1);
    if data.class == 6 {
        assert_eq!(data.bin, 0, "main-header bin identifier must be zero");
    }
    let mut value = data.bin;
    let mut tail = Vec::new();
    while value > 15 {
        tail.push((value % 128) as u8);
        value /= 128;
    }
    tail.reverse();
    let first = (form << 5)
        | if data.last { 0x10 } else { 0 }
        | value as u8
        | if tail.is_empty() { 0 } else { 0x80 };
    let mut bytes = vec![first];
    for (index, group) in tail.iter().enumerate() {
        bytes.push(group | if index + 1 == tail.len() { 0 } else { 0x80 });
    }
    if form >= 2 {
        bytes.extend(contract_vbas(data.class));
    }
    if form == 3 {
        bytes.extend(contract_vbas(data.stream));
    }
    bytes.extend(contract_vbas(data.offset));
    bytes.extend(contract_vbas(data.body.len() as u64));
    if let Some(auxiliary) = data.auxiliary {
        bytes.extend(contract_vbas(auxiliary));
    }
    let header_length = bytes.len();
    bytes.extend_from_slice(&data.body);
    (bytes, header_length)
}

fn contract_assert_data(
    data: &ContractData,
    form: u8,
    context: &mut JpipHeaderContext,
    limits: &ResourceLimits,
) {
    let (mut bytes, header_length) = contract_data_bytes(data, form);
    let extent = bytes.len();
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    assert!(bytes.len() as u64 <= limits.maximum_input_bytes());
    assert!(data.offset + data.body.len() as u64 <= limits.maximum_input_bytes());
    let result = parse_jpip_message(&bytes, context, limits);
    let (message, consumed) = result.unwrap_or_else(|error| {
        panic!("parse_jpip_message form={form} fixture={data:?} bytes={bytes:02x?}: {error:?}")
    });
    let JpipMessage::Data { header, body } = message else {
        panic!("ordinary message misclassified as EOR: {data:?}");
    };
    let expected_class = match data.class {
        0 | 1 => DatabinClass::Precinct,
        2 => DatabinClass::TileHeader,
        4 | 5 => DatabinClass::TileData,
        6 => DatabinClass::MainHeader,
        8 => DatabinClass::Metadata,
        _ => unreachable!(),
    };
    assert_eq!(header.databin().class_identifier(), data.class);
    assert_eq!(header.databin().class(), expected_class);
    assert_eq!(header.databin().codestream(), data.stream);
    assert_eq!(header.databin().in_class(), data.bin);
    assert_eq!(header.offset(), data.offset);
    assert_eq!(header.length(), data.body.len() as u64);
    assert_eq!(header.is_last_byte(), data.last);
    assert_eq!(header.auxiliary(), data.auxiliary);
    assert_eq!(header.header_length(), header_length);
    assert_eq!(
        header.final_length(),
        data.last.then_some(data.offset + data.body.len() as u64)
    );
    assert_eq!(body, data.body);
    assert_eq!(body.as_ptr(), bytes[header_length..].as_ptr());
    assert_eq!(consumed, extent, "must not consume the next message");
    assert_eq!(context.previous_class(), Some(data.class));
    assert_eq!(context.previous_codestream(), Some(data.stream));
}

fn contract_seed_context(limits: &ResourceLimits) -> JpipHeaderContext {
    let mut context = JpipHeaderContext::new();
    contract_assert_data(
        &ContractData {
            class: 6,
            stream: 9,
            bin: 0,
            offset: 0,
            last: false,
            auxiliary: None,
            body: Vec::new(),
        },
        3,
        &mut context,
        limits,
    );
    context
}

fn contract_assert_error(
    bytes: &[u8],
    context: &mut JpipHeaderContext,
    limits: &ResourceLimits,
    expected: DecodeError,
) {
    let before = *context;
    eprintln!(
        "negative parse_jpip_message len={} prefix={:02x?}, expected={expected:?}",
        bytes.len(),
        &bytes[..bytes.len().min(24)]
    );
    assert_eq!(parse_jpip_message(bytes, context, limits), Err(expected));
    assert_eq!(*context, before, "failed parse must not commit inheritance");
}

fn contract_data_range_boundaries(limits: &ResourceLimits) {
    let small = limits.with_maximum_input_bytes(64).unwrap();
    let mut context = contract_seed_context(limits);
    let mut data = ContractData {
        class: 0,
        stream: 0,
        bin: 0,
        offset: 0,
        last: false,
        auxiliary: None,
        body: Vec::new(),
    };
    for offset in [0, 1, 63, 64] {
        data.offset = offset;
        contract_assert_data(&data, 3, &mut context, &small);
    }
    data.offset = 65;
    let (bytes, _) = contract_data_bytes(&data, 3);
    contract_assert_error(
        &bytes,
        &mut context,
        &small,
        DecodeError::limit(Resource::InputBytes, 64, 65),
    );
    data.offset = 64;
    data.body = vec![1];
    let (bytes, _) = contract_data_bytes(&data, 3);
    contract_assert_error(
        &bytes,
        &mut context,
        &small,
        DecodeError::limit(Resource::InputBytes, 64, 65),
    );
}

#[test]
fn contract_bounded_message_and_eor() {
    parses_end_of_response();
    let limits = ResourceLimits::default()
        .with_maximum_input_bytes(65536)
        .unwrap();
    contract_data_range_boundaries(&limits);
    for reason in [1, 2, 3, 4, 5, 6, 7, 255] {
        for length in [0, 1, 2, 126, 127, 128, 129, 255, 256, 16383, 16384] {
            let body: Vec<u8> = (0..length)
                .map(|index| (index as u8).wrapping_mul(73).wrapping_add(reason))
                .collect();
            let mut bytes = vec![0, reason];
            bytes.extend(contract_vbas(length as u64));
            let header_length = bytes.len();
            bytes.extend_from_slice(&body);
            let extent = bytes.len();
            bytes.extend_from_slice(&[0x60, 6, 0, 0, 0]);
            assert_eq!(bytes[0], 0);
            assert!([1, 2, 3, 4, 5, 6, 7, 255].contains(&reason));
            for mut context in [JpipHeaderContext::new(), contract_seed_context(&limits)] {
                let before = context;
                let exact = limits.with_maximum_input_bytes(bytes.len() as u64).unwrap();
                let (message, consumed) = parse_jpip_message(&bytes, &mut context, &exact).unwrap();
                let JpipMessage::EndOfResponse(end) = message else {
                    panic!("D.3 EOR classified as ordinary data: reason={reason}, length={length}");
                };
                assert_eq!(end.reason_code(), reason);
                assert_eq!(end.body(), body);
                assert_eq!(message.body(), body);
                assert_eq!(end.header_length(), header_length);
                assert_eq!(end.body().as_ptr(), bytes[header_length..].as_ptr());
                assert_eq!(consumed, extent);
                assert_eq!(context, before, "EOR must not alter ordinary-header context");
                let below = limits
                    .with_maximum_input_bytes(bytes.len() as u64 - 1)
                    .unwrap();
                contract_assert_error(
                    &bytes,
                    &mut context,
                    &below,
                    DecodeError::limit(
                        Resource::InputBytes,
                        bytes.len() as u64 - 1,
                        bytes.len() as u64,
                    ),
                );
            }
            // Sample all header prefixes and the immediate body boundary.
            for cut in (0..header_length).chain([extent.saturating_sub(1)]) {
                if cut >= extent {
                    continue;
                }
                let offset = if cut < header_length { cut } else { header_length };
                contract_assert_error(
                    &bytes[..cut],
                    &mut contract_seed_context(&limits),
                    &limits,
                    DecodeError::malformed(offset as u64, MalformedKind::Truncated),
                );
            }
        }
    }
    for bytes in [
        vec![0, 1, 0x80, 0],
        [vec![0, 1], vec![0xff; 9], vec![0x7f]].concat(),
    ] {
        contract_assert_error(
            &bytes,
            &mut contract_seed_context(&limits),
            &limits,
            DecodeError::malformed(2, MalformedKind::InvalidJpipVbas),
        );
    }
    let mut impossible_extent = vec![0, 1];
    impossible_extent.extend(contract_vbas(u64::MAX));
    contract_assert_error(
        &impossible_extent,
        &mut contract_seed_context(&limits),
        &limits,
        DecodeError::malformed(
            impossible_extent.len() as u64,
            MalformedKind::ArithmeticOverflow,
        ),
    );
}

#[test]
fn contract_standard_header_forms() {
    parses_all_three_standard_header_forms_and_auxiliary();
    let limits = ResourceLimits::default()
        .with_maximum_input_bytes(65536)
        .unwrap();
    let mut context = JpipHeaderContext::new();
    let mut seed = 0x4a50_4950_u64;
    let boundaries = [0, 1, 15, 16, 127, 128, 2047, 2048, 16383, 16384];
    for iteration in 0..280 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let class = [0, 1, 2, 4, 5, 6, 8][iteration % 7];
        let data = ContractData {
            class,
            stream: boundaries[(iteration / 7) % boundaries.len()],
            bin: if class == 6 { 0 } else { boundaries[iteration % boundaries.len()] },
            offset: if iteration < 140 {
                boundaries[(iteration / 3) % boundaries.len()]
            } else {
                (seed >> 32) % 32000
            },
            last: iteration % 2 != 0,
            auxiliary: match class {
                1 => Some([1, 2, 127, 128, 255][iteration % 5]),
                5 => Some(0),
                _ => None,
            },
            body: (0..[0, 1, 127, 128, 255, 256][iteration % 6])
                .map(|index| (index as u8).wrapping_add(seed as u8))
                .collect(),
        };
        // Equal semantic headers must have equal named fields despite different
        // wire/header lengths. Each path is also checked against the fixture.
        contract_assert_data(&data, 3, &mut context, &limits);
        contract_assert_data(&data, 1, &mut context, &limits);
        let mut changed_class = data.clone();
        changed_class.class = 8;
        changed_class.auxiliary = None;
        contract_assert_data(&changed_class, 2, &mut context, &limits);
        contract_assert_data(&changed_class, 1, &mut context, &limits);
    }
    let data = ContractData {
        class: 0,
        stream: 0,
        bin: 0,
        offset: 0,
        last: false,
        auxiliary: None,
        body: Vec::new(),
    };
    contract_assert_data(&data, 2, &mut JpipHeaderContext::new(), &limits);
    contract_assert_error(
        &[1, 0, 0],
        &mut context,
        &limits,
        DecodeError::malformed(0, MalformedKind::InvalidJpipHeader),
    );
    contract_data_range_boundaries(&limits);
    let mut truncated = data.clone();
    truncated.class = 1;
    truncated.auxiliary = Some(128);
    truncated.body = vec![0x89, 0xab];
    let (bytes, header_length) = contract_data_bytes(&truncated, 3);
    for cut in 0..bytes.len() {
        contract_assert_error(
            &bytes[..cut],
            &mut context,
            &limits,
            DecodeError::malformed(cut.min(header_length) as u64, MalformedKind::Truncated),
        );
    }
    let mut overflow = data.clone();
    overflow.offset = u64::MAX;
    overflow.body = vec![1];
    let (bytes, header_length) = contract_data_bytes(&overflow, 3);
    contract_assert_error(
        &bytes,
        &mut context,
        &limits,
        DecodeError::malformed(header_length as u64, MalformedKind::ArithmeticOverflow),
    );
    // A.2.1 explicitly supplies zero for BOTH absent identifiers when no
    // previous message exists. This is general Annex-A parsing, not K.1 UDP.
    let mut rejected = Vec::new();
    for bin in boundaries {
        for last in [false, true] {
            let mut first = data.clone();
            first.bin = bin;
            first.last = last;
            first.body = vec![0x12, 0x34];
            let (bytes, header_length) = contract_data_bytes(&first, 1);
            assert!(bytes.len() as u64 <= limits.maximum_input_bytes());
            let mut fresh = JpipHeaderContext::new();
            match parse_jpip_message(&bytes, &mut fresh, &limits) {
                Ok((JpipMessage::Data { header, body }, consumed)) => {
                    assert_eq!(header.databin().class_identifier(), 0);
                    assert_eq!(header.databin().codestream(), 0);
                    assert_eq!(header.databin().in_class(), bin);
                    assert_eq!(header.offset(), 0);
                    assert_eq!(header.length(), 2);
                    assert_eq!(header.is_last_byte(), last);
                    assert_eq!(header.auxiliary(), None);
                    assert_eq!(header.header_length(), header_length);
                    assert_eq!(body, first.body);
                    assert_eq!(consumed, bytes.len());
                    assert_eq!(fresh.previous_class(), Some(0));
                    assert_eq!(fresh.previous_codestream(), Some(0));
                }
                other => {
                    eprintln!("A.2.1 first form-01 bytes={bytes:02x?}: expected class=0, CSn=0; got {other:?}");
                    rejected.push((bin, last));
                }
            }
        }
    }
    assert!(rejected.is_empty(), "A.2.1 first-header defaults rejected: {rejected:?}");
}
