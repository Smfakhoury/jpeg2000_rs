// T.800 (06/2019), I.5.3.4 and Tables I.12/I.13. These are local pclr
// unit contracts, not claims that a standalone box is a complete JP2 file.

fn contract_palette_fixture(
    entries: u16,
    columns: &[PaletteColumn],
    seed: u64,
) -> (Vec<u8>, Vec<i64>) {
    assert!((1..=255).contains(&columns.len()));
    let mut payload = entries.to_be_bytes().to_vec();
    payload.push(u8::try_from(columns.len()).unwrap());
    for column in columns {
        assert!((1..=38).contains(&column.precision));
        payload.push((column.precision - 1) | if column.signed { 0x80 } else { 0 });
    }

    let mut expected = Vec::new();
    let mut state = seed;
    for entry in 0..entries {
        for (index, column) in columns.iter().enumerate() {
            let modulus = 1_i64 << column.precision;
            let (minimum, maximum) = if column.signed {
                (-(modulus / 2), modulus / 2 - 1)
            } else {
                (0, modulus - 1)
            };
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let value = match (usize::from(entry) + index) % 8 {
                0 => minimum,
                1 => maximum,
                2 => 0,
                3 => minimum + 1,
                4 => maximum - 1,
                5 if column.signed => -1,
                5 => modulus / 2,
                _ => minimum + i64::try_from(state % u64::try_from(modulus).unwrap()).unwrap(),
            };
            assert!((minimum..=maximum).contains(&value));
            expected.push(value);

            // Encode the independently selected integer into its low precision
            // bits; even negative samples have zero *storage* padding.
            let raw = u64::try_from(if value < 0 { value + modulus } else { value }).unwrap();
            let width = usize::from(column.precision).div_ceil(8);
            let encoded = raw.to_be_bytes();
            payload.extend_from_slice(&encoded[8 - width..]);
            assert_eq!(raw >> column.precision, 0);
        }
    }
    let row_bytes: usize = columns
        .iter()
        .map(|column| usize::from(column.precision).div_ceil(8))
        .sum();
    assert_eq!(
        payload.len(),
        3 + columns.len() + usize::from(entries) * row_bytes
    );
    (box32(*b"pclr", &payload), expected)
}

fn contract_parse_palette_box(bytes: &[u8]) -> Result<Palette, DecodeError> {
    let mut reader = BoxReader::new(bytes, &ResourceLimits::default()).unwrap();
    let box_ref = reader.next_box_ref().unwrap().unwrap();
    assert_eq!(box_ref.header.box_type(), BoxType::PALETTE);
    assert_eq!(box_ref.header.header_length(), 8);
    assert_eq!(
        box_ref.header.total_length(),
        u64::try_from(bytes.len()).unwrap()
    );
    assert_eq!(
        box_ref.header.payload_length().unwrap(),
        box_ref.payload.len() as u64
    );
    assert!(reader.next_box_ref().is_none());
    parse_palette(box_ref, 0)
}

fn contract_assert_retained_palette(
    palette: &Palette,
    entries: u16,
    columns: &[PaletteColumn],
    expected: &[i64],
) {
    assert_eq!(palette.entries, entries, "retained NE={entries}");
    assert_eq!(palette.entries(), entries);
    assert_eq!(palette.columns, columns, "retained Bi, NE={entries}");
    assert_eq!(palette.columns(), columns);
    assert_eq!(
        palette.values.len(),
        usize::from(entries) * columns.len(),
        "complete table, NE={entries}"
    );
    assert_eq!(palette.values, expected, "entry-major table, NE={entries}");
    for entry in 0..entries {
        for column in 0..columns.len() {
            assert_eq!(
                palette.value(entry, u8::try_from(column).unwrap()),
                Ok(expected[usize::from(entry) * columns.len() + column]),
                "wire sample NE={entries}, entry={entry}, column={column}, Bi={:?}",
                columns[column]
            );
        }
    }
}

#[test]
fn palette_contract_wire_order_precision_and_signedness() {
    // Keep the existing unsigned-8 and signed-5 regression fixtures in this
    // clause's execution, in addition to the complete precision sweep.
    validates_palette_mapping_relationships();
    retains_signed_palette_and_channel_definition_records();

    let mut columns = Vec::new();
    for precision in 1..=38 {
        columns.push(PaletteColumn {
            precision,
            signed: false,
        });
        columns.push(PaletteColumn {
            precision: 39 - precision,
            signed: true,
        });
    }
    let mut contexts = 0;
    for entries in [1, 2, 8, 17, 127, 1024] {
        for count in [1, 2, 3, 76, 255] {
            let descriptors: Vec<_> = (0..count)
                .map(|index| columns[(index * 17 + usize::from(entries)) % columns.len()])
                .collect();
            let (bytes, expected) =
                contract_palette_fixture(entries, &descriptors, 0x05ee_d012_3456_789a);
            eprintln!(
                "unit route BoxReader -> parse_palette: NE={entries}, NPC={count}, bytes={}",
                bytes.len()
            );
            let palette = contract_parse_palette_box(&bytes).unwrap();
            contract_assert_retained_palette(&palette, entries, &descriptors, &expected);
            contexts += 1;
        }
    }
    assert_eq!(contexts, 30);
}

#[test]
fn palette_contract_t800_entry_count_domain_and_complete_table() {
    // Every legal NE, including both ends, must retain the whole table.
    let mut admitted = 0;
    for entries in 1..=1024 {
        let columns = [
            PaletteColumn {
                precision: 8,
                signed: false,
            },
            PaletteColumn {
                precision: 9,
                signed: true,
            },
            PaletteColumn {
                precision: 38,
                signed: false,
            },
        ];
        let (bytes, expected) = contract_palette_fixture(entries, &columns, 0x006e_e123_4567_89ab);
        let palette = contract_parse_palette_box(&bytes).unwrap_or_else(|error| {
            panic!("unit route BoxReader -> parse_palette: legal NE={entries}: {error:?}")
        });
        contract_assert_retained_palette(&palette, entries, &columns, &expected);
        admitted += 1;
    }
    assert_eq!(admitted, 1024);
    eprintln!("retained all entries and samples for every legal NE=1..1024");

    // Only NE is invalid: NPC=1, unsigned 8-bit Bi, exact framing, full
    // declared table, no padding, and at most 65,547 bytes including LBox.
    // The clause promises rejection, not a particular DecodeError variant.
    let mut wrongly_admitted = Vec::new();
    for entries in [0, 1025, 1026, 2048, 32768, u16::MAX] {
        let columns = [PaletteColumn {
            precision: 8,
            signed: false,
        }];
        let (bytes, expected) = contract_palette_fixture(entries, &columns, 0x006e_e123_4567_89ab);
        assert!(bytes.len() <= 65_547);
        eprintln!(
            "unit route BoxReader -> parse_palette: forbidden NE={entries}, NPC=1, bytes={}",
            bytes.len()
        );
        match contract_parse_palette_box(&bytes) {
            Err(error) => eprintln!("rejected NE={entries}: {error:?}"),
            Ok(palette) => {
                contract_assert_retained_palette(&palette, entries, &columns, &expected);
                wrongly_admitted.push(entries);
                eprintln!("predicate violated: admitted forbidden NE={entries}");
            }
        }
    }
    assert!(
        wrongly_admitted.is_empty(),
        "T.800 (06/2019) I.5.3.4 requires NE in 1..1024; admitted {wrongly_admitted:?}"
    );
}
