// Header-only oracle: ITU-T T.808 (12/2022), A.2.1 and A.2.2.
// Each context represents one reliably ordered JPP- or JPT-stream; reserved
// classes 3 and 7 are deliberately excluded. No payload decoding is asserted.
fn check_inherited_class_contract() {
    fn vbas(mut value: u64, out: &mut Vec<u8>) {
        let mut bytes = vec![(value & 0x7f) as u8];
        value >>= 7;
        while value != 0 {
            bytes.push((value & 0x7f) as u8 | 0x80);
            value >>= 7;
        }
        out.extend(bytes.into_iter().rev());
    }

    fn message(form: u8, class: u64, stream: u64, offset: u64, body: &[u8]) -> Vec<u8> {
        assert!((1..=3).contains(&form));
        assert!([0, 1, 2, 4, 5, 6, 8].contains(&class));
        let mut bytes = vec![(form << 5) | 1]; // non-final fragment, in-class ID 1
        if form >= 2 {
            vbas(class, &mut bytes);
        }
        if form == 3 {
            vbas(stream, &mut bytes);
        }
        vbas(offset, &mut bytes);
        vbas(body.len() as u64, &mut bytes);
        if class & 1 != 0 {
            vbas(1, &mut bytes);
        }
        bytes.extend_from_slice(body);
        bytes
    }

    fn accept(
        input: &[u8],
        context: &mut JpipHeaderContext,
        class: u64,
        stream: u64,
        offset: u64,
        body_expected: &[u8],
    ) {
        eprintln!(
            "route=parse_jpip_message->previous_class class={class} stream={stream} \
             offset={offset} input={input:02x?} pre={context:?}"
        );
        let result = parse_jpip_message(input, context, &ResourceLimits::default());
        let (parsed, consumed) = result.unwrap_or_else(|error| {
            panic!("valid dependent/independent header must parse: {error:?}")
        });
        let JpipMessage::Data { header, body } = parsed else {
            panic!("expected data header, not EOR");
        };
        assert_eq!(header.databin().class_identifier(), class);
        assert_eq!(header.databin().codestream(), stream);
        assert_eq!(header.databin().in_class(), 1);
        assert_eq!(header.offset(), offset);
        assert_eq!(header.length(), body_expected.len() as u64);
        assert_eq!(header.auxiliary(), (class & 1 != 0).then_some(1));
        assert!(!header.is_last_byte());
        assert_eq!(body, body_expected);
        assert_eq!(consumed, input.len());
        assert_eq!(header.header_length(), input.len() - body_expected.len());
        assert_eq!(context.previous_class(), Some(class));
        assert_eq!(context.previous_codestream(), Some(stream));
    }

    let limits = ResourceLimits::default();
    let mut seed = 0x4a50_4950_u64;
    for class in [0, 1, 2, 4, 5, 6, 8] {
        let mut context = JpipHeaderContext::new();
        assert_eq!(context.previous_class(), None);
        for index in 0..24 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let stream = [0, 127, 128, 16383, 16384, seed & 0xffff][index % 6];
            let offset = [0, 127, 128, 16383, 16384, (seed >> 16) & 0xffff][index % 6];
            let body = vec![(seed >> 32) as u8; index % 4];
            let explicit = message(3, class, stream, offset, &body);
            accept(&explicit, &mut context, class, stream, offset, &body);

            let inherited = message(1, class, stream, offset, &body);
            assert_eq!(inherited[0] & 0x60, 0x20);
            accept(&inherited, &mut context, class, stream, offset, &body);

            // A rejected tentative class update must not poison inheritance.
            let replacement = if class == 8 { 0 } else { 8 };
            let mut truncated = message(2, replacement, stream, 0, &[0xaa]);
            truncated.pop();
            let before = context;
            assert!(matches!(
                parse_jpip_message(&truncated, &mut context, &limits),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::Truncated,
                    ..
                })
            ));
            assert_eq!(context, before);
            accept(&inherited, &mut context, class, stream, offset, &body);

            // Replacing the explicit class must change the subsequent inherited class.
            let changed = message(2, replacement, stream, offset, &body);
            accept(&changed, &mut context, replacement, stream, offset, &body);
            let inherited = message(1, replacement, stream, offset, &body);
            accept(&inherited, &mut context, replacement, stream, offset, &body);

            let before = context;
            assert!(matches!(
                parse_jpip_message(&[0x01, 0, 0], &mut context, &limits),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidJpipHeader,
                    ..
                })
            ));
            assert_eq!(context, before);
        }
    }

    // No Class or CSn VBAS and no predecessor: A.2.1 defaults both to zero.
    // These are legal dependent headers, not malformed-prefix negative cases.
    for mut context in [JpipHeaderContext::new(), JpipHeaderContext::default()] {
        assert_eq!(context.previous_class(), None);
        assert_eq!(context.previous_codestream(), None);
        let initial = message(1, 0, 0, 0, &[]);
        assert_eq!(initial, [0x21, 0, 0]);
        accept(&initial, &mut context, 0, 0, 0, &[]);
        let next = message(1, 0, 0, 128, &[0]);
        accept(&next, &mut context, 0, 0, 128, &[0]);
    }
}
