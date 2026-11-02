{
    // T.814 7.3.4/7.3.6: non-initial pairs; kappa=1; both quads exist.
    // Isolated U-field fixture, not a complete signalled codestream.
    fn fields(u: u8) -> (Vec<bool>, Vec<bool>, Vec<bool>) {
        assert!(u <= 74);
        let (prefix, suffix, width, extension): (&[bool], u8, u8, Option<u8>) = match u {
            0 => (&[], 0, 0, None),
            1 => (&[true], 0, 0, None),
            2 => (&[false, true], 0, 0, None),
            3..=4 => (&[false, false, true], u - 3, 1, None),
            5..=32 => (&[false; 3], u - 5, 5, None),
            _ => (&[false; 3], 28 + (u - 33) % 4, 5, Some((u - 33) / 4)),
        };
        assert_eq!(extension.is_some(), suffix >= 28);
        let bits = |value: u8, n: u8| -> Vec<bool> {
            (0..n).map(|i| value & (1 << i) != 0).collect()
        };
        (prefix.to_vec(), bits(suffix, width), extension.map_or_else(Vec::new, |e| bits(e, 4)))
    }
    // Independent reverse-byte writer, including stuffing and physical positions.
    fn pack(bits: &[bool]) -> ([u8; 8], Vec<(usize, u8)>) {
        let mut input = [0; 8];
        input[6] = 8; // SCUP=8; low nibble is not VLC payload.
        let mut positions = Vec::new();
        let mut offset = 0;
        let mut later = false;
        for byte in (0..=6).rev() {
            let first = if byte == 6 { 4 } else { 0 };
            let short = if byte == 6 {
                (0..3).all(|i| bits.get(offset + i).copied().unwrap_or(false))
            } else {
                later && (0..7).all(|i| bits.get(offset + i).copied().unwrap_or(false))
            };
            let end = if short { 7 } else { 8 };
            for bit in first..end {
                if bits.get(offset).copied().unwrap_or(false) { input[byte] |= 1 << bit; }
                positions.push((byte, bit));
                offset += 1;
            }
            later = if byte == 6 { input[byte] | 15 > 0x8f } else { input[byte] > 0x8f };
        }
        assert!(bits.len() < positions.len());
        assert_eq!((usize::from(input[7]) << 4) | usize::from(input[6] & 15), 8);
        (input, positions)
    }
    let sentinel = [true, false, true, true, false, false, true, false];
    let mut count = 0;
    for first in 0..=32_u8 {
        for second in 0..=32_u8 {
            let mode = u8::from(first != 0) | (u8::from(second != 0) << 1);
            let (p0, s0, e0) = fields(first);
            let (p1, s1, e1) = fields(second);
            assert!(e0.is_empty() && e1.is_empty());
            let bits: Vec<bool> = p0.into_iter().chain(p1).chain(s0).chain(s1).collect();
            let n = bits.len();
            let payload: Vec<bool> = bits.into_iter().chain(sentinel).collect();
            let (input, positions) = pack(&payload);
            let expected = [first + 1, second + 1];
            for budget in 0..=n + 1 {
                let context = format!("mode={mode} residuals=({first},{second}) budget={budget} input={input:02x?}");
                let mut r = ht_cleanup_readers_from_inputs(&input[..7], &input, budget as u64).unwrap();
                let mel = r.mel;
                let want = if budget < n {
                    Err(DecodeError::LimitExceeded { resource: Resource::WorkUnits, limit: budget as u64, actual: budget as u64 + 1 })
                } else { Ok(expected) };
                assert_eq!(r.decode_non_initial_u(mode), want, "{context}");
                assert_eq!(r.mel, mel, "{context}");
                assert_eq!((r.mel_last_byte, r.mel_state, r.mel_zero_events, r.mel_terminating_one, r.mel_events), (None, 0, 0, false, 0), "{context}");
                let done = budget.min(n);
                assert_eq!(r.work_units, done as u64, "{context}");
                assert_eq!(r.vlc_last_byte, done.checked_sub(1).map(|i| positions[i].0 as u16), "{context}");
                let cursor = if budget < n {
                    assert_eq!(r.vlc.bits_read, budget as u64 + 1, "{context}");
                    let (byte, bit) = positions[budget];
                    (byte, bit + 1)
                } else {
                    assert_eq!(r.vlc.bits_read, n as u64, "{context}");
                    positions[n]
                };
                assert_eq!((r.vlc.current_byte, r.vlc.next_bit), cursor, "{context}");
            }
            let mut r = ht_cleanup_readers_from_inputs(&input[..7], &input, 100).unwrap();
            assert_eq!(r.decode_non_initial_u(mode), Ok(expected));
            for bit in sentinel { assert_eq!(r.read_vlc_bit(), Ok(bit)); }
            assert_eq!(r.vlc.bits_read, (n + sentinel.len()) as u64);
            count += 1;
        }
    }
    assert_eq!(count, 1089);
    for mode in 4..=255 {
        let mut r = ht_cleanup_readers_from_inputs(&[0], &[0x30, 0], 0).unwrap();
        let (mel, vlc) = (r.mel, r.vlc);
        assert_eq!(r.decode_non_initial_u(mode), Err(invalid_code_block()));
        assert_eq!((r.mel, r.vlc), (mel, vlc));
        assert_eq!((r.mel_last_byte, r.vlc_last_byte, r.work_units), (None, None, 0));
    }
    for (mode, terminal) in [(1, 0), (2, 0), (3, 0), (3, 0x10), (3, 0x40)] {
        let input = [terminal, 0];
        let mut r = ht_cleanup_readers_from_inputs(&[0], &input, 100).unwrap();
        let mel = r.mel;
        assert_eq!(r.decode_non_initial_u(mode), Err(invalid_code_block()), "exhaustion mode={mode} terminal={terminal:#x}");
        assert_eq!(r.mel, mel);
        assert_eq!((r.vlc.bits_read, r.work_units, r.vlc_last_byte, r.vlc.has_byte), (4, 4, Some(0), false));
    }
    eprintln!("decode_non_initial_u: {count} extension-free pairs, every bit-budget cutoff, invalid modes, physical exhaustion; now standard extensions");
    // Table 3: residual <=74 plus predictor one stays within exponent 75.
    for u in (33..=74).rev() {
        for other in [0, 1, 2, 3, 4, 5, 32, 33, 36, 37, 74] {
            for pair in [[u, other], [other, u]] {
                let [first, second] = pair;
                assert!(first <= 74 && second <= 74);
                let mode = u8::from(first != 0) | (u8::from(second != 0) << 1);
                let (p0, s0, e0) = fields(first);
                let (p1, s1, e1) = fields(second);
                let bits: Vec<bool> = p0.into_iter().chain(p1).chain(s0).chain(s1).chain(e0).chain(e1).collect();
                let n = bits.len();
                let payload: Vec<bool> = bits.into_iter().chain(sentinel).collect();
                let (input, positions) = pack(&payload);
                let mut r = ht_cleanup_readers_from_inputs(&input[..7], &input, 100).unwrap();
                let mel = r.mel;
                assert_eq!(r.decode_non_initial_u(mode), Ok([first + 1, second + 1]),
                    "T.814 7.3.4/7.3.6 non-initial pair={pair:?}, mode={mode}, folded predictor=1, exponent<=75, input={input:02x?}");
                assert_eq!(r.mel, mel);
                assert_eq!((r.vlc.bits_read, r.work_units), (n as u64, n as u64), "four extension bits required even for extension zero");
                assert_eq!((r.vlc.current_byte, r.vlc.next_bit), positions[n]);
                for bit in sentinel { assert_eq!(r.read_vlc_bit(), Ok(bit)); }
            }
        }
    }
}
