// Included in codestream::tests by the replay build; no production access changes.
// T.801 (06/2021), J.3.1 and J.3.1.1.3: non-NULL reversible
// decorrelation arrays contain (N + 1) * N integer coefficients.
#[test]
fn resolve_mct_collection_clause_7ca4daea() {
    // Reuse existing coverage of lookup, shapes, NULL arrays, dependency counts,
    // precision checks, full returned fields and unchanged parser state.
    actual_mct_graph_collections_preserve_lookup_shape_flags_and_all_fields();
    let limits = ResourceLimits::HARDENED;
    let parser = new_parser(&[], &limits).unwrap();
    let invalid = DecodeError::MalformedInput { offset: 0, kind: MalformedKind::InvalidTransform };
    let mut failures = Vec::new();
    let mut seed = 0x7ca4_daea_u32;
    let mut dimensions: Vec<usize> = (1..=8).collect();
    dimensions.extend([16, 31, 63]);
    let mut checks = 0;
    for n in dimensions {
        assert!(n <= usize::from(limits.maximum_components()));
        let count = (n + 1) * n;
        assert!(count <= usize::from(limits.maximum_components()).pow(2));
        let mut coefficients = Vec::with_capacity(count);
        for _ in 0..count {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            coefficients.push(i16::try_from(seed % 17).unwrap() - 8);
        }
        for row in 0..n { coefficients[row * n + n - 1 - row] = 2; }
        coefficients[count - 1] = 4;
        assert_eq!(coefficients.len(), (n + 1) * n);
        for row in 0..n { assert_eq!(coefficients[row * n + n - 1 - row], 2); }
        assert_eq!(coefficients[count - 1], 4);
        let offset_values: Vec<i16> = (0..n).map(|i| i16::try_from(i).unwrap() - 32).collect();
        let matrix_bytes = mct_i16(7, 1, &coefficients);
        let offset_bytes = mct_i16(8, 2, &offset_values);
        let parse_array = |bytes: &[u8]| {
            assert_eq!(u16::from_be_bytes([bytes[0], bytes[1]]), MCT);
            assert_eq!(usize::from(u16::from_be_bytes([bytes[2], bytes[3]])), bytes.len() - 2);
            assert_eq!(&bytes[4..6], &[0, 0]); // Zmct = 0.
            assert_eq!(&bytes[8..10], &[0, 0]); // Ymct = 0.
            parser.parse_mct(Segment { marker: MCT, offset: 0, payload: &bytes[4..] }).unwrap()
        };
        let matrix = parse_array(&matrix_bytes);
        let offsets = parse_array(&offset_bytes);
        assert_eq!(matrix.index, 7);
        assert_eq!(matrix.kind, MctArrayKind::Decorrelation);
        assert_eq!(matrix.numeric_format, MctNumericFormat::Int16);
        assert_eq!(matrix.fraction_bits, 0);
        assert_eq!(matrix.values, coefficients.iter().copied().map(i64::from).collect::<Vec<_>>());
        assert_eq!(offsets.index, 8);
        assert_eq!(offsets.kind, MctArrayKind::Offset);
        assert_eq!(offsets.fraction_bits, 0);
        assert_eq!(offsets.values.len(), n);
        let components: Vec<u16> = (0..n).map(|i| u16::try_from(i).unwrap()).collect();
        let mut outputs = components.clone();
        outputs.rotate_left(n / 2);
        let reference = MctCollectionReference {
            input_components: components, output_components: outputs,
            transform: MctCollectionReferenceTransform::Array {
                kind: MctArrayKind::Decorrelation, reversible: true, transform_index: 7, offset_index: 8,
            },
        };
        for with_offsets in [false, true] {
            let mut reference = reference.clone();
            if !with_offsets {
                reference.transform = MctCollectionReferenceTransform::Array {
                    kind: MctArrayKind::Decorrelation, reversible: true, transform_index: 7, offset_index: 0,
                };
            }
            let arrays = vec![matrix.clone(), offsets.clone()];
            let expected = Ok(MctCollection {
                reversible: true,
                input_components: reference.input_components.clone(),
                output_components: reference.output_components.clone(),
                kind: MctCollectionKind::Decorrelation, matrix: Some(matrix.clone()),
                offsets: if with_offsets { Some(offsets.clone()) } else { None }, wavelet: None,
            });
            let actual = parser.resolve_mct_collection(&reference, &arrays);
            checks += 1;
            if actual != expected {
                failures.push(format!("direct resolve_mct_collection: J.3.1.1.3 N={n}, integer coefficients={count}, offsets={with_offsets}: expected full collection, got {actual:?}"));
            }
            // Rejected-input tests, not standards-valid transforms; N*N targets
            // the admission boundary identified by the original mapping.
            for wrong_count in [0, count - 1, count + 1, n * n] {
                let mut bad = arrays.clone();
                bad[0].values.resize(wrong_count, 0);
                let actual = parser.resolve_mct_collection(&reference, &bad);
                checks += 1;
                if actual != Err(invalid) {
                    let summary = actual.as_ref().map(|collection| collection.matrix.as_ref().map(|array| array.values.len()));
                    failures.push(format!("N={n}, offsets={with_offsets}, coefficients={wrong_count}: expected InvalidTransform for length != {count}, got {summary:?}"));
                }
            }
            for floating_offset in [false, true] {
                if floating_offset && !with_offsets { continue; }
                let mut bad = arrays.clone();
                let index = usize::from(floating_offset);
                bad[index].numeric_format = MctNumericFormat::Float32;
                bad[index].fraction_bits = 32;
                bad[index].values = vec![1_i64 << 31; bad[index].values.len()];
                let actual = parser.resolve_mct_collection(&reference, &bad);
                checks += 1;
                if actual != Err(invalid) {
                    failures.push(format!("N={n}, fractional {}: expected InvalidTransform, got {actual:?}", if floating_offset { "offsets" } else { "matrix" }));
                }
            }
        }
        assert_eq!(parser.position, 0);
        assert_eq!(parser.work_units, 0);
        assert_eq!(parser.packet_length_bytes, 0);
        assert_eq!(parser.packet_lengths, 0);
    }
    assert_eq!(checks, 143);
    assert!(failures.is_empty(), "{} discriminating checks failed:\n{}", failures.len(), failures.join("\n"));
}
