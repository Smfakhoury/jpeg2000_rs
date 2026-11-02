// Included inside jpip's existing test module; no production API is exposed.

fn contract_part(tile: u16, part: u8, psot: u32, markers: &[u8]) -> Vec<u8> {
    let mut payload = tile.to_be_bytes().to_vec();
    payload.extend_from_slice(&psot.to_be_bytes());
    payload.extend_from_slice(&[part, 0]);
    let mut header = segment(0xff90, &payload);
    header.extend_from_slice(markers);
    header.extend_from_slice(&[0xff, 0x93]);
    header
}

fn contract_random(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *seed
}

fn contract_transition(
    actual: &JpipTilePartTransition,
    header: &[u8],
    tile: u16,
    part: u8,
    packets: Option<u64>,
    data: Option<u64>,
    functional: bool,
) {
    assert_eq!(actual.tile(), tile, "tile; header={header:02x?}");
    assert_eq!(actual.tile_part(), part, "part index; header={header:02x?}");
    assert_eq!(actual.header_bytes(), header, "exact retained header");
    assert_eq!(
        actual.packet_count(),
        packets,
        "packet count; header={header:02x?}"
    );
    assert_eq!(
        actual.data_length(),
        data,
        "data length; header={header:02x?}"
    );
    assert_eq!(
        actual.has_functional_change(),
        functional,
        "functional; header={header:02x?}"
    );
}

fn contract_error(input: &[u8], tile: u16, offset: usize, kind: MalformedKind) {
    let before = input.to_vec();
    let sentinel = vec![0x31_u8; 97];
    let result = parse_tile_part_transitions(input, tile);
    assert_eq!(input, before, "error path mutated input");
    assert_eq!(sentinel, vec![0x31; 97], "error path mutated caller state");
    assert_eq!(
        result,
        Err(DecodeError::MalformedInput {
            offset: offset as u64,
            kind
        }),
        "route=parse_tile_part_transitions(tile={tile}); input={input:02x?}"
    );
}

fn contract_disjoint(a: &[u8], b: &[u8]) {
    if !a.is_empty() && !b.is_empty() {
        let a_start = a.as_ptr() as usize;
        let b_start = b.as_ptr() as usize;
        assert!(
            a_start + a.len() <= b_start || b_start + b.len() <= a_start,
            "initialized byte ranges overlap: {a_start:#x}+{}, {b_start:#x}+{}",
            a.len(),
            b.len()
        );
    }
}

#[test]
fn contract_transition_shared_read_isolation() {
    use std::sync::Barrier;

    let mut markers = Vec::new();
    for _ in 0..2048 {
        markers.extend(segment(0xff64, &[0, 1, 0xff, 0x58]));
    }
    let mut inputs = vec![markers.clone(), contract_part(65535, 255, 0, &markers)];
    inputs.push(Vec::new());
    inputs.push([contract_part(65535, 0, 14, &[]), vec![0xff, 0x90]].concat());
    for input in inputs {
        let before = input.clone();
        let caller_state = (vec![0x73; 33], String::from("caller-owned"));
        let caller_before = caller_state.clone();
        for round in 0..8 {
            let barrier = Barrier::new(2);
            let (left, right) = std::thread::scope(|scope| {
                let run = || {
                    barrier.wait();
                    let result = parse_tile_part_transitions(&input, 65535);
                    barrier.wait();
                    result
                };
                let left = scope.spawn(run);
                let right = scope.spawn(run);
                (left.join().unwrap(), right.join().unwrap())
            });
            assert_eq!(input, before, "shared input; round={round}");
            assert_eq!(caller_state, caller_before);
            if input.ends_with(&[0xff, 0x90]) {
                let expected = Err(DecodeError::MalformedInput {
                    offset: 16,
                    kind: MalformedKind::Truncated,
                });
                assert_eq!(left, expected);
                assert_eq!(right, expected);
            } else {
                let mut left = left.unwrap();
                let right = right.unwrap();
                assert_eq!(left.len(), 1);
                assert_eq!(right.len(), 1);
                let is_sot = input.starts_with(&[0xff, 0x90]);
                for result in [&left, &right] {
                    contract_transition(
                        &result[0],
                        &input,
                        65535,
                        if is_sot { 255 } else { 0 },
                        None,
                        None,
                        false,
                    );
                }
                contract_disjoint(left[0].header_bytes(), right[0].header_bytes());
                contract_disjoint(&input, left[0].header_bytes());
                contract_disjoint(&input, right[0].header_bytes());
                if !input.is_empty() {
                    left[0].header[0] ^= 0x7f;
                    assert_eq!(right[0].header_bytes(), input);
                    assert_eq!(input, before);
                }
            }
        }
    }
}

#[test]
fn contract_transition_functional_classification() {
    tile_part_transition_status_distinguishes_sufficient_packet_boundaries();
    let markers = [0xff58_u16, 0xff61, 0xff64, 0xff52, 0xff53, 0xff5c, 0xff5f];
    let mut seed = 0x3815_abcd;
    for trial in 0..128 {
        let mut bytes = Vec::new();
        let mut functional = false;
        let mut packets = None;
        let mut plt_index = 0;
        let count = if trial < 8 {
            trial
        } else {
            (contract_random(&mut seed) % 20) as usize
        };
        for index in 0..count {
            let marker = if trial < 8 {
                markers[(trial + index) % markers.len()]
            } else {
                markers[(contract_random(&mut seed) as usize) % markers.len()]
            };
            if marker == 0xff58 {
                bytes.extend(segment(marker, &[plt_index]));
                plt_index += 1;
                packets = Some(0);
            } else {
                bytes.extend(segment(marker, &[0, 0, 0xff, 0x58]));
                functional |= !matches!(marker, 0xff61 | 0xff64);
            }
        }
        let plain = parse_tile_part_transitions(&bytes, 9).unwrap();
        assert_eq!(plain.len(), 1);
        contract_transition(&plain[0], &bytes, 9, 0, packets, None, functional);
        // After marker-only SOD even malformed or functional bytes are opaque.
        let mut opaque = bytes.clone();
        opaque.extend_from_slice(&[0xff, 0x93, 0xff, 0x52, 0, 1, 0xff, 0x58]);
        let stopped = parse_tile_part_transitions(&opaque, 9).unwrap();
        contract_transition(&stopped[0], &opaque, 9, 0, packets, None, functional);
        let first = contract_part(9, 17, (14 + bytes.len()) as u32, &bytes);
        let second = contract_part(9, 3, 0, &segment(0xff64, &[0, 0]));
        let joined = [first.clone(), second.clone()].concat();
        let result = parse_tile_part_transitions(&joined, 9).unwrap();
        assert_eq!(result.len(), 2);
        contract_transition(&result[0], &first, 9, 17, packets, Some(0), functional);
        contract_transition(&result[1], &second, 9, 3, None, None, false);
    }
    for marker in markers {
        let payload = if marker == 0xff58 { vec![0] } else { vec![] };
        let bytes = segment(marker, &payload);
        let expected = !matches!(marker, 0xff58 | 0xff61 | 0xff64);
        assert_eq!(
            parse_tile_part_transitions(&bytes, 0).unwrap()[0].has_functional_change(),
            expected
        );
    }
}

#[test]
fn contract_transition_psot_lengths() {
    tile_part_transition_status_distinguishes_sufficient_packet_boundaries();
    let mut seed = 0xb81f_937a;
    for payload_len in [0, 1, 2, 17, 127, 255] {
        let markers = segment(0xff64, &vec![0x44; payload_len]);
        let h = 14 + markers.len();
        let mut lengths = vec![h as u32, h as u32 + 1, h as u32 + 255, u32::MAX];
        for _ in 0..24 {
            lengths.push(h as u32 + contract_random(&mut seed) % 100_000);
        }
        for length in lengths {
            let header = contract_part(7, 241, length, &markers);
            let result = parse_tile_part_transitions(&header, 7).unwrap();
            assert_eq!(result.len(), 1);
            contract_transition(
                &result[0],
                &header,
                7,
                241,
                None,
                Some(u64::from(length) - h as u64),
                false,
            );
        }
        let zero = contract_part(7, 255, 0, &markers);
        let result = parse_tile_part_transitions(&zero, 7).unwrap();
        contract_transition(&result[0], &zero, 7, 255, None, None, false);
        let mut nonfinal = zero;
        nonfinal.extend(contract_part(7, 0, 14, &[]));
        contract_error(&nonfinal, 7, 0, MalformedKind::InvalidTilePart);
    }
    // Both the smallest nonzero value and the H-1 boundary must reject.
    // Complete marker framing, matching tile, and final SOD all precede this check.
    for markers in [vec![], segment(0xff64, &[0, 0, 3])] {
        let h = 14 + markers.len();
        for length in [h as u32 - 1, 1] {
            let bad = contract_part(7, 5, length, &markers);
            for prefix in [vec![], contract_part(7, 0, 14, &[])] {
                let offset = prefix.len();
                contract_error(
                    &[prefix, bad.clone()].concat(),
                    7,
                    offset,
                    MalformedKind::InvalidTilePart,
                );
            }
        }
    }
}

#[test]
fn contract_transition_validation_errors() {
    use MalformedKind::{
        InvalidMarkerLength, InvalidPacketLength, InvalidTilePart, MissingRequiredStructure,
        Truncated,
    };
    for payload_len in [0, 1, 7, 9, 16] {
        let malformed_sot = segment(0xff90, &vec![0; payload_len]);
        contract_error(&malformed_sot, 0, 0, InvalidTilePart);
    }
    for prefix in [vec![], contract_part(4, 17, 14, &[])] {
        let p = prefix.len();
        let wrong_tile = contract_part(5, 0, 0, &[]);
        contract_error(
            &[prefix.clone(), wrong_tile].concat(),
            4,
            p,
            InvalidTilePart,
        );
        let mut missing = contract_part(4, 9, 0, &[]);
        missing.truncate(12);
        contract_error(
            &[prefix.clone(), missing.clone()].concat(),
            4,
            p + 12,
            MissingRequiredStructure,
        );
        missing.extend(segment(0xff64, &[0, 0]));
        contract_error(
            &[prefix.clone(), missing.clone()].concat(),
            4,
            p + missing.len(),
            MissingRequiredStructure,
        );
        let nonfinal = contract_part(4, 9, 0, &[]);
        contract_error(
            &[prefix.clone(), nonfinal, vec![0xff]].concat(),
            4,
            p,
            InvalidTilePart,
        );
        if p > 0 {
            for replacement in [segment(0xff64, &[0, 0]), vec![0xff, 0xd9]] {
                contract_error(
                    &[prefix.clone(), replacement].concat(),
                    4,
                    p,
                    InvalidTilePart,
                );
            }
        }
    }
    for (tail, relative, kind) in [
        (vec![0xff], 0, Truncated),
        (vec![0xff, 0x58], 2, Truncated),
        (vec![0xff, 0x58, 0], 2, Truncated),
        (vec![0xff, 0x58, 0, 0], 0, InvalidMarkerLength),
        (vec![0xff, 0x58, 0, 1], 0, InvalidMarkerLength),
        (vec![0xff, 0x58, 0, 5, 0], 0, Truncated),
    ] {
        contract_error(&tail, 4, relative, kind);
        let mut framed = contract_part(4, 2, 0, &[]);
        framed.truncate(12);
        framed.extend_from_slice(&tail);
        contract_error(&framed, 4, 12 + relative, kind);
    }
    for (input, offset, kind) in [
        (vec![0xff, 0x90], 2, Truncated),
        (vec![0xff, 0x90, 0], 2, Truncated),
        (vec![0xff, 0x90, 0, 10], 0, Truncated),
        (vec![0xff, 0x90, 0, 1], 0, InvalidMarkerLength),
    ] {
        contract_error(&input, 0, offset, kind);
    }
    let mut prefix = contract_part(4, 0, 14, &[]);
    let bad_payload = contract_part(4, 1, 0, &segment(0xff58, &[0, 0x80]));
    prefix.extend(bad_payload);
    contract_error(&prefix, 4, 0, InvalidPacketLength);
}

#[test]
fn contract_transition_framing_and_preservation() {
    let mut seed = 0x9183_47ae;
    for tile in [0, 1, 255, 256, u16::MAX] {
        for bytes in [
            vec![],
            segment(0xff64, &[0, 0, 0xff, 0x90]),
            vec![0xff, 0x93],
            vec![0xff, 0x93, 0, 0xff, 0x90, 0xff],
        ] {
            let before = bytes.clone();
            let result = parse_tile_part_transitions(&bytes, tile).unwrap();
            assert_eq!(result.len(), 1);
            contract_transition(&result[0], &bytes, tile, 0, None, None, false);
            assert_eq!(bytes, before);
        }
        for trial in 0..64 {
            let mut input = Vec::new();
            let mut expected = Vec::new();
            for index in 0..1 + trial % 8 {
                let part = [0, 255, 17, 1, 128, 9, 3, 251][index];
                let payload = vec![(contract_random(&mut seed) >> 24) as u8; index * 3];
                let markers = if index % 2 == 0 {
                    vec![]
                } else {
                    segment(0xff64, &payload)
                };
                let h = 14 + markers.len();
                let final_zero = index == trial % 8 && trial % 3 == 0;
                let length = if final_zero { 0 } else { h as u32 + 10_000 };
                let header = contract_part(tile, part, length, &markers);
                input.extend_from_slice(&header);
                expected.push((header, part, if final_zero { None } else { Some(10_000) }));
            }
            let before = input.clone();
            let sentinel = vec![0x8a; 67];
            let result = parse_tile_part_transitions(&input, tile).unwrap();
            assert_eq!(result.len(), expected.len());
            for (actual, (header, part, data)) in result.iter().zip(&expected) {
                contract_transition(actual, header, tile, *part, None, *data, false);
            }
            let concatenated: Vec<u8> = result
                .iter()
                .flat_map(|part| part.header_bytes().iter().copied())
                .collect();
            assert_eq!(
                concatenated, input,
                "no packet bytes supplied; Psot must not skip"
            );
            assert_eq!(input, before);
            assert_eq!(sentinel, vec![0x8a; 67]);
        }
    }
    // Bounded fixed-seed arbitrary slices exercise totality and the byte frame.
    // The structured checks above, not mere non-panicking, establish framing.
    for len in 0..512 {
        let bytes: Vec<u8> = (0..len)
            .map(|_| (contract_random(&mut seed) >> 24) as u8)
            .collect();
        let before = bytes.clone();
        let result = std::panic::catch_unwind(|| parse_tile_part_transitions(&bytes, 3));
        assert!(result.is_ok(), "input-dependent panic for {bytes:02x?}");
        assert_eq!(bytes, before);
    }
}

fn contract_encode_length(mut value: u32) -> Vec<u8> {
    let mut bytes = vec![(value & 0x7f) as u8];
    value >>= 7;
    while value != 0 {
        bytes.push(((value & 0x7f) as u8) | 0x80);
        value >>= 7;
    }
    bytes.reverse();
    bytes
}

#[test]
fn contract_transition_plt_counts_and_indices() {
    tile_part_transition_status_distinguishes_sufficient_packet_boundaries();
    let mut seed = 0x7acf_1248;
    for count in [0, 1, 2, 7, 127, 255] {
        let values: Vec<u32> = (0..count)
            .map(|index| {
                [
                    0,
                    1,
                    127,
                    128,
                    16383,
                    16384,
                    u32::MAX,
                    contract_random(&mut seed),
                ][index % 8]
            })
            .collect();
        for chunk_size in [1, 2, 7, 256] {
            let mut markers = Vec::new();
            let groups: Vec<&[u32]> = if values.is_empty() {
                vec![&[]]
            } else {
                values.chunks(chunk_size).collect()
            };
            for (index, group) in groups.iter().enumerate() {
                let mut payload = vec![index as u8];
                for value in *group {
                    payload.extend(contract_encode_length(*value));
                }
                markers.extend(segment(0xff58, &payload));
            }
            // Rechunking the same complete encodings changes no packet count.
            let plain = parse_tile_part_transitions(&markers, 3).unwrap();
            contract_transition(&plain[0], &markers, 3, 0, Some(count as u64), None, false);
            let h = 14 + markers.len();
            let first = contract_part(3, 8, h as u32, &markers);
            let second = contract_part(3, 2, 0, &segment(0xff58, &[0]));
            let result =
                parse_tile_part_transitions(&[first.clone(), second.clone()].concat(), 3).unwrap();
            assert_eq!(result.len(), 2);
            contract_transition(&result[0], &first, 3, 8, Some(count as u64), Some(0), false);
            contract_transition(&result[1], &second, 3, 2, Some(0), None, false);
        }
    }
    for bytes in [vec![], segment(0xff61, &[0]), segment(0xff64, &[0, 0])] {
        assert_eq!(
            parse_tile_part_transitions(&bytes, 0).unwrap()[0].packet_count(),
            None
        );
    }
    let arbitrary_indices = [
        segment(0xff58, &[255, 1]),
        segment(0xff58, &[255, 0x81, 0]),
        segment(0xff58, &[3]),
    ]
    .concat();
    assert_eq!(
        parse_tile_part_transitions(&arbitrary_indices, 3).unwrap()[0].packet_count(),
        Some(2)
    );
    for payload in [vec![], vec![0, 0x80], vec![0, 1, 0xff]] {
        contract_error(
            &segment(0xff58, &payload),
            3,
            0,
            MalformedKind::InvalidPacketLength,
        );
        let framed = contract_part(3, 0, 0, &segment(0xff58, &payload));
        let offset = if payload.is_empty() { 12 } else { 0 };
        contract_error(&framed, 3, offset, MalformedKind::InvalidPacketLength);
    }
    for indices in [vec![1], vec![0, 0], vec![0, 2], vec![0, 1, 3]] {
        let mut markers = Vec::new();
        for index in &indices {
            markers.extend(segment(0xff58, &[*index]));
        }
        let bad_at = if indices[0] != 0 {
            0
        } else {
            indices.len() - 1
        };
        contract_error(
            &contract_part(3, 0, 0, &markers),
            3,
            12 + 5 * bad_at,
            MalformedKind::InvalidPacketLength,
        );
    }
    let mut markers = Vec::new();
    for index in 0..=255 {
        markers.extend(segment(0xff58, &[index]));
    }
    let accepted = contract_part(3, 0, 0, &markers);
    assert_eq!(
        parse_tile_part_transitions(&accepted, 3).unwrap()[0].packet_count(),
        Some(0)
    );
    markers.extend(segment(0xff58, &[0]));
    contract_error(
        &contract_part(3, 0, 0, &markers),
        3,
        12 + 256 * 5,
        MalformedKind::InvalidPacketLength,
    );
    let mut stopped = segment(0xff58, &[0, 1]);
    stopped.extend_from_slice(&[0xff, 0x93, 0xff, 0x58, 0, 2]);
    assert_eq!(
        parse_tile_part_transitions(&stopped, 3).unwrap()[0].packet_count(),
        Some(1)
    );
}

#[test]
fn contract_transition_owned_storage() {
    for input in [
        vec![],
        segment(0xff64, &[0, 0, 17]),
        [
            contract_part(7, 4, 14, &[]),
            contract_part(7, 2, 0, &segment(0xff58, &[0, 1])),
        ]
        .concat(),
    ] {
        let mut owner = input.clone();
        let mut result = parse_tile_part_transitions(&owner, 7).unwrap();
        let snapshots: Vec<Vec<u8>> = result
            .iter()
            .map(|part| part.header_bytes().to_vec())
            .collect();
        for (index, part) in result.iter().enumerate() {
            contract_disjoint(&owner, part.header_bytes());
            for other in &result[index + 1..] {
                contract_disjoint(part.header_bytes(), other.header_bytes());
            }
        }
        owner.fill(0x5a);
        for (part, expected) in result.iter().zip(&snapshots) {
            assert_eq!(part.header_bytes(), expected);
        }
        drop(owner);
        for (index, expected) in snapshots.iter().enumerate() {
            assert_eq!(
                result[index].header_bytes(),
                expected,
                "survives input destruction"
            );
        }
        if !result[0].header.is_empty() {
            result[0].header[0] ^= 0x44;
            for index in 1..result.len() {
                assert_eq!(result[index].header_bytes(), snapshots[index]);
            }
        }
        let mut owner = input.clone();
        let mut second = parse_tile_part_transitions(&owner, 7).unwrap();
        if !second[0].header.is_empty() {
            second[0].header[0] ^= 0x33;
        }
        assert_eq!(owner, input, "result mutation cannot change input");
        drop(second);
        assert_eq!(owner, input, "result drop cannot change input");
        owner.push(0x42);
        assert_eq!(&owner[..input.len()], input);
    }
    // Error after an assembled header: no partial result is transferred.
    for _ in 0..128 {
        let mut input = contract_part(7, 0, 14, &[]);
        input.extend(contract_part(8, 1, 0, &[]));
        contract_error(&input, 7, 14, MalformedKind::InvalidTilePart);
        input[0] = 0;
        assert_eq!(input[0], 0, "input owner remains usable after Err");
    }
}
