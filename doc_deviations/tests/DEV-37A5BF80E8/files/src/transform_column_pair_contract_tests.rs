#[test]
fn part1_column_pair_contract() {
    inverse_vertical_stage_matches_column_synthesis();
    complete_standard_planes_match_independent_reference_for_all_small_geometries();

    // T.800 (06/2019) F.3.5: vertical synthesis is independent 1D_SR per column.
    // F.3.6 specifies the singleton separately, for either selected filter.
    let mut failures = Vec::new();
    let mut seed = 0x5eed_c011_u64;
    let mut checked = 0;
    for width in [1_u32, 2, 3, 7] {
        for height in (0_u32..=17).chain([31, 32, 33, 64, 65]) {
            for low_first in [false, true] {
                for irreversible in [false, true] {
                    let origin = u32::from(!low_first);
                    let low_height = (origin + height).div_ceil(2) - origin.div_ceil(2);
                    let high_height = height - low_height;
                    assert_eq!(low_height + high_height, height);
                    assert_eq!(low_height, height / 2 + u32::from(low_first) * (height % 2));
                    let scale = if irreversible { 65_536 } else { 1 };
                    let mut make_columns = |rows: u32, band: i64| {
                        (0..width)
                            .map(|x| {
                                (0..rows)
                                    .map(|y| {
                                        seed =
                                            seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                                        let value = ((seed >> 32) % 257) as i64 - 128
                                            + i64::from(x) * 19
                                            + i64::from(y) * 7
                                            + band;
                                        // Even coefficients make singleton division exact.
                                        2 * value * scale
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>()
                    };
                    let low_columns = make_columns(low_height, 11);
                    let high_columns = make_columns(high_height, -23);
                    let flatten = |columns: &[Vec<i64>], rows: u32| {
                        (0..rows)
                            .flat_map(|y| columns.iter().map(move |column| column[y as usize]))
                            .collect::<Vec<_>>()
                    };
                    let low_samples = flatten(&low_columns, low_height);
                    let high_samples = flatten(&high_columns, high_height);
                    let low = WaveletBand {
                        width,
                        height: low_height,
                        samples: &low_samples,
                    };
                    let high = WaveletBand {
                        width,
                        height: high_height,
                        samples: &high_samples,
                    };
                    assert_eq!(low.samples.len(), (width * low_height) as usize);
                    assert_eq!(high.samples.len(), (width * high_height) as usize);
                    let mut expected_plane = vec![0; (width * height) as usize];
                    for x in 0..width {
                        assert!(x < low.width && x < high.width);
                        let l = &low_columns[x as usize];
                        let h = &high_columns[x as usize];
                        let expected = if height == 1 {
                            // F.3.6: X(i0)=Y(i0) for even i0, Y(i0)/2 for odd i0.
                            Ok(if low_first {
                                vec![l[0]]
                            } else {
                                vec![h[0] / 2]
                            })
                        } else {
                            reference_sequence(l, h, low_first, irreversible)
                        };
                        let expected = expected.expect("bounded coefficients must synthesize");
                        assert_eq!(expected.len(), height as usize);
                        for (y, value) in expected.iter().enumerate() {
                            expected_plane[y * width as usize + x as usize] = *value;
                        }
                        let actual =
                            inverse_column_pair(low, high, x, low_first, irreversible, None);
                        if actual.as_ref() != Ok(&expected) {
                            failures.push(format!(
                                "inverse_column_pair width={width} height={height} origin={origin} \
                                 x={x} irreversible={irreversible} low={l:?} high={h:?}: \
                                 expected={expected:?}, actual={actual:?}"
                            ));
                        }
                        // Repacking the same selected columns at width one must not change
                        // synthesis. Only the storage stride and selected x differ.
                        let packed = inverse_column_pair(
                            WaveletBand {
                                width: 1,
                                height: low_height,
                                samples: l,
                            },
                            WaveletBand {
                                width: 1,
                                height: high_height,
                                samples: h,
                            },
                            0,
                            low_first,
                            irreversible,
                            None,
                        );
                        assert_eq!(actual, packed, "column repacking x={x} width={width}");
                        checked += 1;
                    }
                    if height > 0 {
                        let plane = if irreversible {
                            inverse_vertical_9_7(
                                low,
                                high,
                                height,
                                low_first,
                                &ResourceLimits::default(),
                            )
                        } else {
                            inverse_vertical_5_3(
                                low,
                                high,
                                height,
                                low_first,
                                &ResourceLimits::default(),
                            )
                        };
                        let actual = observed_reconstruction(plane);
                        let expected = Ok((
                            width,
                            height,
                            if irreversible { 16 } else { 0 },
                            expected_plane,
                        ));
                        if actual != expected {
                            failures.push(format!(
                                "public vertical -> inverse_columns -> inverse_column_pair \
                                 width={width} height={height} origin={origin} irreversible={irreversible}: \
                                 expected={expected:?}, actual={actual:?}"
                            ));
                        }
                    }
                    assert_eq!(low_samples, flatten(&low_columns, low_height));
                    assert_eq!(high_samples, flatten(&high_columns, high_height));
                }
            }
        }
    }
    assert_eq!(
        checked, 1196,
        "all geometries and filter/phase combinations ran"
    );
    eprintln!(
        "checked {checked} columns; {} predicate violations",
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
