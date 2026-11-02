use super::*;

// T.800 (06/2019), A.7.2, Tables A.35/A.36: Nplm is ONE octet,
// and marker boundaries may split a group, but never a length codeword.
fn standard_group(lengths: &[u32]) -> Vec<u8> {
    let encoded = encode_packet_lengths(lengths);
    let mut group = vec![u8::try_from(encoded.len()).expect("Nplm <= 255")];
    group.extend(encoded);
    group
}

fn standard_marker(index: u8, body: &[u8]) -> Vec<u8> {
    assert!(!body.is_empty());
    let mut payload = vec![index];
    payload.extend_from_slice(body);
    let bytes = segment(PLM, &payload);
    assert_eq!(&bytes[..2], &0xff57_u16.to_be_bytes());
    let length = usize::from(u16::from_be_bytes([bytes[2], bytes[3]]));
    assert!((4..=65535).contains(&length));
    assert_eq!(length + 2, bytes.len());
    bytes
}

fn observed_lengths(lengths: PacketLengths<'_>) -> Vec<u32> {
    let mut iterator = lengths.iter();
    let mut values = Vec::new();
    while let Some(value) = iterator.next_length().expect("completed length codeword") {
        values.push(value);
    }
    assert_eq!(iterator.next_length().unwrap(), None);
    values
}

#[test]
fn standard_plm_octet_counts_and_wire_order() {
    let mut cases = vec![
        vec![vec![]],
        vec![vec![1]],
        vec![vec![127, 128, 16383, 16384, u32::MAX]],
        vec![vec![1; 255]],
        vec![vec![128; 127]],
        vec![vec![9, 128, 2], vec![], vec![16384, 127, 1]],
    ];
    let mut seed = 0xc777_9586_u32;
    for _ in 0..32 {
        let mut groups = Vec::new();
        for _ in 0..3 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let count = (seed % 31) as usize;
            let mut values = Vec::new();
            for _ in 0..count {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                values.push(seed.max(1));
            }
            groups.push(values);
        }
        cases.push(groups);
    }
    let limits = ResourceLimits::default();
    let mut failures = Vec::new();
    for (case, expected) in cases.iter().enumerate() {
        let mut body = Vec::new();
        for group in expected {
            body.extend(standard_group(group));
        }
        let bytes = standard_marker(0, &body);
        let mut parser = new_parser(&bytes, &limits).unwrap();
        let raw = parser.read_segment().unwrap();
        assert_eq!(raw.marker, PLM);
        assert_eq!(raw.offset, 0);
        let mut next = 0;
        let mut groups = Vec::new();
        eprintln!("unit parse_plm case={case}, Nplm/Iplm={body:?}");
        if let Err(error) = parser.parse_plm(raw, &mut next, &mut groups) {
            failures.push(format!(
                "case {case}: valid A.35 octet framing rejected: {error:?}"
            ));
            continue;
        }
        assert_eq!(next, 1, "case {case}: Zplm successor");
        assert_eq!(groups.len(), expected.len(), "case {case}: tile-part count");
        for (index, (actual, values)) in groups.iter().zip(expected).enumerate() {
            assert!(matches!(actual.source, PacketLengthSource::Plm(_)));
            assert_eq!(actual.packet_count(), values.len() as u64);
            assert_eq!(
                actual.encoded_bytes(),
                standard_group(values).len() as u64 - 1
            );
            assert_eq!(
                observed_lengths(*actual),
                *values,
                "case {case}, group {index}"
            );
        }
        assert_eq!(parser.position, bytes.len());
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// A legal Part 1 stream with one component, one resolution/precinct, and
// one empty (one-byte) packet per layer. Tile-parts partition those layers.
fn standard_stream(markers: &[Vec<u8>], counts: &[usize]) -> Vec<u8> {
    let layers = u16::try_from(counts.iter().sum::<usize>()).unwrap();
    assert!(layers > 0);
    let mut cod = vec![0, 0];
    cod.extend(layers.to_be_bytes());
    cod.extend([0, 0, 2, 2, 0, 1]);
    let mut bytes = SOC.to_be_bytes().to_vec();
    let size = siz();
    assert_eq!(&size[4..6], &[0, 0], "Rsiz=0: baseline Part 1");
    bytes.extend(size);
    bytes.extend(segment(COD, &cod));
    bytes.extend(segment(QCD, &[0, 0]));
    for marker in markers {
        bytes.extend(marker);
    }
    for (index, &count) in counts.iter().enumerate() {
        let mut sot = 0_u16.to_be_bytes().to_vec();
        sot.extend(u32::try_from(14 + count).unwrap().to_be_bytes());
        sot.extend([
            u8::try_from(index).unwrap(),
            u8::try_from(counts.len()).unwrap(),
        ]);
        bytes.extend(segment(SOT, &sot));
        bytes.extend(SOD.to_be_bytes());
        bytes.extend(vec![0; count]);
    }
    bytes.extend(EOC.to_be_bytes());
    bytes
}

fn assert_standard_layout(layout: &CodestreamLayout<'_>, counts: &[usize]) -> Vec<Vec<u32>> {
    assert_eq!(layout.tile_parts.len(), counts.len());
    layout
        .tile_parts
        .iter()
        .zip(counts)
        .enumerate()
        .map(|(index, (part, &count))| {
            assert_eq!(part.tile_index, 0);
            assert_eq!(usize::from(part.tile_part_index), index);
            assert_eq!(part.data, vec![0; count]);
            assert!(part.packet_lengths.is_present());
            assert_eq!(part.packet_lengths.packet_count(), count as u64);
            assert_eq!(part.packet_lengths.encoded_bytes(), count as u64);
            let values = observed_lengths(part.packet_lengths);
            assert_eq!(
                values,
                vec![1; count],
                "tile-part {index}: exact packet list"
            );
            values
        })
        .collect()
}

#[test]
fn standard_plm_continued_groups_reach_tile_part_consumers() {
    let limits = ResourceLimits::default();
    let mut failures = Vec::new();
    for counts in [vec![3, 1, 2], vec![1, 4, 2], vec![255, 1]] {
        // Establish all non-PLM framing and input limits via the actual caller.
        let control = standard_stream(&[], &counts);
        let control_layout = parse_layout(&control, &limits).expect("non-PLM legal fixture");
        assert_eq!(control_layout.tile_parts.len(), counts.len());
        let mut body = Vec::new();
        let mut splits = Vec::new();
        for &count in &counts {
            body.push(u8::try_from(count).unwrap());
            for packet in 0..count {
                body.push(1);
                if packet + 1 < count {
                    splits.push(body.len());
                }
            }
        }
        let unsplit = standard_stream(&[standard_marker(0, &body)], &counts);
        let reference = match parse_layout(&unsplit, &limits) {
            Ok(layout) => Some(assert_standard_layout(&layout, &counts)),
            Err(error) => {
                failures.push(format!(
                    "counts={counts:?}: unsplit valid PLM rejected: {error:?}"
                ));
                None
            }
        };
        for split in splits {
            assert_eq!(
                body[split - 1] & 0x80,
                0,
                "marker ends at completed codeword"
            );
            assert_eq!(
                body[split], 1,
                "next marker starts with continued Iplm, not Nplm"
            );
            let markers = [
                standard_marker(0, &body[..split]),
                standard_marker(1, &body[split..]),
            ];
            let bytes = standard_stream(&markers, &counts);
            eprintln!(
                "Parser::parse_layout -> parse_plm: counts={counts:?}, split={split}, Zplm=0,1"
            );
            match parse_layout(&bytes, &limits) {
                Ok(layout) => {
                    let actual = assert_standard_layout(&layout, &counts);
                    // Repartitioning only the PLM bytes cannot change tile-part
                    // lists. Absolute offsets and marker/work counts may differ.
                    if let Some(expected) = &reference {
                        assert_eq!(&actual, expected, "marker partition invariance");
                    }
                }
                Err(error) => failures.push(format!(
                    "counts={counts:?}, split={split}: legal continued Iplm rejected: {error:?}"
                )),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
