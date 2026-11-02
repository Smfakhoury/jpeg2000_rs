#[test]
fn association_requires_two_immediate_children() {
    // T.801 (06/2021), M.11.11: BF plus at least one Bi. This unit
    // fixture is a JPX metadata subtree, not a complete file: file-level
    // branding, reader requirements and placement are outside this entry.
    let limits = ResourceLimits::default();
    let label = box32(*b"lbl ", b"subject\0");
    let xml = box32(*b"xml ", b"<metadata/>");
    let association = |children: &[Vec<u8>]| box32(*b"asoc", &children.concat());
    let valid_inner = association(&[label.clone(), xml.clone()]);
    let mut failures = Vec::new();
    let mut observations = 0;

    // Expectations describe the constructed tree, independently of inspection:
    // (total nodes including the root, deepest edge distance from the root).
    let mut cases = vec![
        ("empty".to_owned(), vec![], None),
        ("singleton-label".to_owned(), vec![label.clone()], None),
        (
            "singleton-nested-valid-four-total-nodes".to_owned(),
            vec![valid_inner.clone()],
            None,
        ),
        (
            "valid-nested-five-total-nodes".to_owned(),
            vec![label.clone(), valid_inner.clone()],
            Some((5, 2)),
        ),
        (
            "two-immediate-but-empty-descendant".to_owned(),
            vec![label.clone(), association(&[])],
            None,
        ),
        (
            "two-immediate-but-singleton-descendant".to_owned(),
            vec![label.clone(), association(std::slice::from_ref(&xml))],
            None,
        ),
        (
            "singleton-after-valid-recursion".to_owned(),
            vec![
                label.clone(),
                association(std::slice::from_ref(&valid_inner)),
            ],
            None,
        ),
    ];
    let mut cardinalities = vec![2_u32, 3, 4, 8, 31, 64];
    let mut seed = 0x4a50_5832_u32;
    for _ in 0..24 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        cardinalities.push(2 + seed % 32);
    }
    for children in cardinalities {
        let mut boxes = vec![label.clone()];
        for _ in 1..children {
            boxes.push(xml.clone());
        }
        cases.push((
            format!("valid-{children}-immediate"),
            boxes,
            Some((children + 1, 1)),
        ));
    }

    // The highest entry depth puts the deepest fixture exactly at the limit;
    // all negative fixtures remain within depth, work and byte admission.
    for depth in [1, 2, limits.maximum_box_depth() - 3] {
        for (name, children, expected) in &cases {
            let bytes = association(children);
            assert!((bytes.len() as u64) < limits.maximum_input_bytes());
            assert!((bytes.len() as u64) < limits.maximum_metadata_bytes());
            assert!(limits.maximum_work_units() > 128);
            assert!(depth + 3 <= limits.maximum_box_depth());
            let mut reader = BoxReader::new(&bytes, &limits).unwrap();
            let root = reader.next_box_ref().unwrap().unwrap();
            assert_eq!(root.header.box_type(), BoxType::ASSOCIATION);
            assert_eq!(root.header.total_length(), bytes.len() as u64);
            assert!(reader.next_box_ref().is_none());
            let mut child_reader = BoxReader::new(root.payload, &limits).unwrap();
            for child_bytes in children {
                let child = child_reader.next_box_ref().unwrap().unwrap();
                assert_eq!(child.header.total_length(), child_bytes.len() as u64);
                assert_eq!(child.payload, &child_bytes[8..]);
            }
            assert!(child_reader.next_box_ref().is_none());

            let observed = inspect_metadata_box(root, &limits, depth, u32::MAX - depth);
            eprintln!(
                "route=jp2::tests->inspect_metadata_box case={name} \
                 depth={depth} immediate={} bytes={} expected={expected:?} observed={observed:?}",
                children.len(),
                bytes.len()
            );
            observations += 1;
            let holds = match expected {
                Some((nodes, distance)) => observed == Ok((*nodes, depth + distance)),
                None => matches!(observed, Err(DecodeError::MalformedInput { .. })),
            };
            if !holds {
                failures.push(format!(
                    "{name} at depth {depth}: immediate={}, expected={expected:?}, \
                     observed={observed:?}",
                    children.len()
                ));
            }
        }
    }
    assert_eq!(observations, cases.len() * 3);
    assert!(
        failures.is_empty(),
        "M.11.11 immediate-child cardinality violations:\n{}",
        failures.join("\n")
    );
}
