#[test]
fn fresh_header_context_preserves_absence_and_independent_standard_history() {
    // T.808 (12/2022), A.2.1/Table A.1: forms 01/10/11, canonical VBAS,
    // no Aux for the even JPP classes below, zero offset/length, non-final.
    fn wire(form: u8, class: u64, codestream: u64) -> Vec<u8> {
        assert!((1..=3).contains(&form));
        assert!([0, 2, 6, 8].contains(&class));
        assert!(codestream < 64);
        let mut bytes = vec![form << 5];
        if form >= 2 {
            bytes.push(class as u8);
        }
        if form == 3 {
            bytes.push(codestream as u8);
        }
        bytes.extend([0, 0]);
        bytes
    }

    fn check(
        context: &mut JpipHeaderContext,
        limits: &ResourceLimits,
        form: u8,
        class: u64,
        codestream: u64,
    ) -> Result<(), DecodeError> {
        let bytes = wire(form, class, codestream);
        assert!((bytes.len() as u64) <= limits.maximum_input_bytes());
        eprintln!(
            "new -> parse_jpip_message: form={form}, expected class={class}, \
             codestream={codestream}, bin=0, bytes={bytes:02x?}"
        );
        let (message, consumed) = parse_jpip_message(&bytes, context, limits)?;
        let JpipMessage::Data { header, body } = message else {
            panic!("ordinary header must not become EOR");
        };
        assert_eq!(consumed, bytes.len());
        assert_eq!(header.header_length(), bytes.len());
        assert_eq!(header.databin().class_identifier(), class);
        assert_eq!(header.databin().codestream(), codestream);
        assert_eq!(header.databin().in_class(), 0);
        assert_eq!(header.offset(), 0);
        assert_eq!(header.length(), 0);
        assert_eq!(header.auxiliary(), None);
        assert!(!header.is_last_byte());
        assert_eq!(body, []);
        assert_eq!(context.previous_class(), Some(class));
        assert_eq!(context.previous_codestream(), Some(codestream));
        Ok(())
    }

    // Keep the original explicit/inherited/class-only and odd-class Aux oracle.
    parses_all_three_standard_header_forms_and_auxiliary();
    const EMPTY: JpipHeaderContext = JpipHeaderContext::new();
    assert_eq!(EMPTY.previous_class(), None);
    assert_eq!(EMPTY.previous_codestream(), None);
    let limits = ResourceLimits::default();
    assert_eq!(limits.maximum_codestreams(), 64);
    for codestream in 0..64 {
        let mut selected = JpipHeaderContext::new();
        let untouched = JpipHeaderContext::new();
        assert_eq!(selected, EMPTY);
        assert_eq!(untouched, EMPTY);
        for class in [0, 2, 6, 8] {
            check(&mut selected, &limits, 3, class, codestream).unwrap();
            assert_ne!(selected, EMPTY);
            assert_eq!(untouched, EMPTY);
            assert_eq!(JpipHeaderContext::new(), EMPTY);
            let snapshot = selected;
            let mut other = JpipHeaderContext::new();
            check(&mut other, &limits, 3, 8, (codestream + 1) % 64).unwrap();
            assert_eq!(selected, snapshot);
            check(&mut selected, &limits, 2, 8, codestream).unwrap();
            check(&mut selected, &limits, 1, 8, codestream).unwrap();
            assert_eq!(untouched, EMPTY);
        }
    }
    for class in [0, 2, 6, 8] {
        let mut first_class_only = JpipHeaderContext::new();
        check(&mut first_class_only, &limits, 2, class, 0).unwrap();
    }
    let mut selected_zero = JpipHeaderContext::new();
    check(&mut selected_zero, &limits, 3, 0, 0).unwrap();
    assert_ne!(selected_zero, EMPTY, "Some(0) is not absent history");
    assert_eq!(JpipHeaderContext::new(), EMPTY);
    let mut first_inherited = JpipHeaderContext::new();
    // A.2.1 supplies class=0 and CSn=0 when there is no previous message.
    // No codestream body/marker validation is required for header parsing.
    check(&mut first_inherited, &limits, 1, 0, 0)
        .expect("fresh inherited header must select standard initial identity (0,0)");
}
