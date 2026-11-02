// Included into the existing codestream::tests module by the suite build.
mod metadata_contract {
    use super::*;

    fn headers(layers: u8) -> [Vec<u8>; 2] {
        assert_ne!(layers, 0);
        [
            segment(COD, &[0, 0, 0, layers, 0, 0, 2, 2, 0, 1]),
            segment(QCD, &[0, 0]),
        ]
    }

    fn main_end(layers: u8) -> usize {
        2 + siz().len() + headers(layers).iter().map(Vec::len).sum::<usize>()
    }

    fn standard_plm(layers: u8) -> Vec<u8> {
        // T.800 (06/2019), A.7.2/Table A.35: Nplmi is ONE byte.
        // One tile-part, with one inline, byte-aligned empty packet per layer.
        let mut payload = vec![0, layers];
        payload.extend(vec![1; usize::from(layers)]);
        let result = segment(PLM, &payload);
        assert_eq!(result[4], 0); // Zplm
        assert_eq!(result[5], layers); // Nplmi, not a u32 dialect
        assert_eq!(result.len(), 6 + usize::from(layers));
        result
    }

    fn fixture(layers: u8, with_plm: bool, with_plt: bool) -> Vec<u8> {
        let [cod, qcd] = headers(layers);
        let mut main = vec![cod, qcd];
        if with_plm {
            main.push(standard_plm(layers));
        }
        let mut tile = Vec::new();
        if with_plt {
            let first = usize::from(layers).div_ceil(2);
            tile.push(plt(0, &vec![1; first]));
            if first < usize::from(layers) {
                tile.push(plt(1, &vec![1; usize::from(layers) - first]));
            }
        }
        // B.9 and B.10.3: one component, resolution and precinct means one
        // packet per layer; 0x00 is an empty header padded to a byte boundary.
        // No PPM/PPT/SOP/EPH, so each Iplm/Iplt includes that one header byte.
        let bytes = codestream_with_headers(1, &main, &tile, &vec![0; usize::from(layers)]);
        assert_eq!(&bytes[..2], &SOC.to_be_bytes());
        assert_eq!(&bytes[2..6], &siz()[..4]);
        assert_eq!(&bytes[6..8], &[0, 0]); // Rsiz: ordinary Part 1, no CAP needed
        assert_eq!(&bytes[40..42], &1_u16.to_be_bytes()); // Csiz
        assert_eq!(&bytes[42..45], &[7, 1, 1]); // unsigned 8-bit, no subsampling
        let sot = main_end(layers)
            + if with_plm { standard_plm(layers).len() } else { 0 };
        assert_eq!(&bytes[sot..sot + 2], &SOT.to_be_bytes());
        assert_eq!(&bytes[sot + 4..sot + 6], &[0, 0]); // Isot
        assert_eq!(&bytes[sot + 10..sot + 12], &[0, 1]); // TPsot/TNsot
        let psot = u32::from_be_bytes(bytes[sot + 6..sot + 10].try_into().unwrap());
        assert_eq!(usize::try_from(psot).unwrap(), bytes.len() - 2 - sot);
        assert!(bytes.len() < 2048);
        bytes
    }

    fn layers_to_check() -> Vec<u8> {
        let mut values = vec![1, 2, 3, 7, 31, 127, 128, 254, 255];
        let mut seed = 0x5eed_2026_u32;
        for _ in 0..12 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            values.push(u8::try_from(seed % 255 + 1).unwrap());
        }
        values.sort_unstable();
        values.dedup();
        values
    }

    fn assert_metadata(info: &CodestreamInfo, bytes: &[u8], layers: u8, plm: bool) {
        assert_eq!((info.image_size.width, info.image_size.height), (64, 32));
        assert_eq!((info.image_size.x_origin, info.image_size.y_origin), (0, 0));
        assert_eq!((info.image_size.tile_width, info.image_size.tile_height), (64, 32));
        assert_eq!((info.image_size.tiles_across, info.image_size.tiles_down), (1, 1));
        assert_eq!(info.image_size.components, 1);
        assert_eq!(info.image_size.uniform_bit_depth, Some(8));
        assert_eq!(info.image_size.uniform_signed, Some(false));
        assert_eq!(info.coding_style.layers, u16::from(layers));
        assert_eq!(info.coding_style.decomposition_levels, 0);
        assert_eq!(info.coding_style.progression_order, ProgressionOrder::Lrcp);
        assert_eq!(info.coding_style.transform, WaveletTransform::Reversible5x3);
        assert_eq!(info.quantization.style, QuantizationStyle::None);
        assert_eq!(info.tile_parts, 1);
        assert_eq!(info.main_header_segments, 3 + u64::from(plm));
        assert_eq!(info.consumed_bytes, u64::try_from(bytes.len()).unwrap());
    }

    fn rejected(
        failures: &mut Vec<String>,
        label: &str,
        bytes: &[u8],
        limits: &ResourceLimits,
        expected: DecodeError,
    ) {
        eprintln!("route=parse -> parse_layout; {label}; len={}; expected={expected:?}", bytes.len());
        let layout = parse_layout(bytes, limits);
        if layout.as_ref().err() != Some(&expected) {
            failures.push(format!("{label}: layout expected {expected:?}, got {layout:?}"));
        }
        let result = parse(bytes, limits);
        if result != Err(expected) {
            failures.push(format!("{label}: parse expected {expected:?}, got {result:?}"));
        }
    }

    fn malformed(offset: usize, kind: MalformedKind) -> DecodeError {
        DecodeError::MalformedInput { offset: offset as u64, kind }
    }

    fn accepted(
        failures: &mut Vec<String>,
        layers: u8,
        plm: bool,
        plt: bool,
    ) {
        let limits = ResourceLimits::default();
        let bytes = fixture(layers, plm, plt);
        eprintln!("route=parse -> parse_layout; layers={layers}, PLM={plm}, PLT={plt}, len={}", bytes.len());
        match parse(&bytes, &limits) {
            Ok(info) => {
                assert_metadata(&info, &bytes, layers, plm);
                // Adding optional, consistent packet lengths cannot change any
                // image/coding metadata. Only byte and main-segment counts differ.
                let plain = fixture(layers, false, false);
                let mut reference = parse(&plain, &limits).expect("unannotated legal control");
                assert_metadata(&reference, &plain, layers, false);
                reference.consumed_bytes = bytes.len() as u64;
                reference.main_header_segments = 3 + u64::from(plm);
                assert_eq!(info, reference, "optional packet-length metadata must be transparent");
            }
            Err(error) => failures.push(format!(
                "legal T.800 PLM={plm}, PLT={plt}, layers={layers}: expected metadata, got {error:?}"
            )),
        }
    }

    #[test]
    fn metadata_plm_plt_admission_and_error_propagation() {
        let limits = ResourceLimits::default();
        let mut failures = Vec::new();
        let [cod, qcd] = headers(1);
        let main = [cod, qcd];
        let sot = main_end(1);
        let plt_offset = sot + 12;
        for (label, marker, kind) in [
            ("PLT first index is not zero", segment(PLT, &[1, 1]), MalformedKind::InvalidMarkerOrder),
            ("PLT unterminated length", segment(PLT, &[0, 0x80]), MalformedKind::InvalidPacketLength),
            ("PLT overflowing length", segment(PLT, &[0, 0x90, 0x80, 0x80, 0x80, 0]), MalformedKind::InvalidPacketLength),
        ] {
            let bytes = codestream_with_headers(1, &main, &[marker], &[0]);
            rejected(&mut failures, label, &bytes, &limits, malformed(plt_offset, kind));
        }
        let bytes = codestream_with_headers(1, &main, &[plt(0, &[2])], &[0]);
        rejected(&mut failures, "PLT sum differs from payload", &bytes, &limits,
            malformed(sot, MalformedKind::InvalidPacketLength));
        let [cod, qcd] = headers(2);
        let first = plt(0, &[1]);
        let bytes = codestream_with_headers(1, &[cod, qcd], &[first.clone(), plt(0, &[1])], &[0, 0]);
        rejected(&mut failures, "duplicate PLT index", &bytes, &limits,
            malformed(main_end(2) + 12 + first.len(), MalformedKind::InvalidMarkerOrder));
        let bytes = fixture(2, false, true);
        let tight = limits.with_maximum_packets(1).unwrap();
        rejected(&mut failures, "packet count exceeds limit by one", &bytes, &tight,
            DecodeError::LimitExceeded { resource: Resource::Packets, limit: 1, actual: 2 });
        for layers in layers_to_check() {
            accepted(&mut failures, layers, false, false);
            accepted(&mut failures, layers, false, true);
            accepted(&mut failures, layers, true, true);
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn metadata_soc_is_at_input_start_and_errors_are_preserved() {
        let limits = ResourceLimits::default();
        let mut failures = Vec::new();
        for layers in layers_to_check() {
            accepted(&mut failures, layers, false, false);
        }
        let bytes = fixture(1, false, false);
        for short in [vec![], vec![0xff]] {
            rejected(&mut failures, "short SOC", &short, &limits,
                malformed(0, MalformedKind::Truncated));
        }
        // Sweep valid-looking non-SOC markers and malformed marker encodings.
        // The valid suffix remains intact, so scanning ahead would be a defect.
        for first in 0..=u8::MAX {
            for second in [0, 0x4e, 0x4f, 0x50, 0x90, 0xff] {
                if (first, second) == (0xff, 0x4f) {
                    continue;
                }
                let mut prefixed = vec![first, second];
                prefixed.extend_from_slice(&bytes);
                let kind = if first != 0xff || matches!(second, 0 | 0xff) {
                    MalformedKind::InvalidMarker
                } else {
                    MalformedKind::InvalidMarkerOrder
                };
                rejected(&mut failures, &format!("prefix {first:02x} {second:02x} before SOC"),
                    &prefixed, &limits, malformed(0, kind));
            }
        }
        let mut missing = bytes.clone();
        missing.drain(..2);
        rejected(&mut failures, "SOC removed", &missing, &limits,
            malformed(0, MalformedKind::InvalidMarkerOrder));
        let mut duplicate = SOC.to_be_bytes().to_vec();
        duplicate.extend_from_slice(&bytes);
        rejected(&mut failures, "duplicate SOC instead of SIZ", &duplicate, &limits,
            malformed(2, MalformedKind::InvalidMarkerOrder));
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn metadata_structural_admission_includes_standard_plm() {
        let limits = ResourceLimits::default();
        let mut failures = Vec::new();
        let [cod, qcd] = headers(3);
        let bytes = codestream_with_headers(1, &[cod, qcd, segment(PLM, &[1, 3, 1, 1, 1])], &[], &[0; 3]);
        rejected(&mut failures, "PLM Zplm starts at one", &bytes, &limits,
            malformed(main_end(3), MalformedKind::InvalidMarkerOrder));
        let [cod, qcd] = headers(1);
        let bytes = codestream_with_headers(1, &[cod, qcd, segment(PLM, &[0, 3, 1])], &[], &[0]);
        rejected(&mut failures, "PLM Nplmi extends beyond segment", &bytes, &limits,
            malformed(main_end(1), MalformedKind::InvalidMarkerLength));
        let plain = fixture(1, false, false);
        let mut trailing = plain.clone();
        trailing.push(0);
        rejected(&mut failures, "complete layout followed by junk", &trailing, &limits,
            malformed(plain.len(), MalformedKind::TrailingData));
        let mut truncated = plain.clone();
        truncated.truncate(truncated.len() - 1);
        rejected(&mut failures, "truncated EOC after valid headers", &truncated, &limits,
            malformed(plain.len() - 2, MalformedKind::Truncated));
        for layers in layers_to_check() {
            accepted(&mut failures, layers, true, false);
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
