use super::*;

// Direct decoded-kernel API fixtures: no codestream/header conformance is claimed.
// T.801 A.3.5: ARB, CON, signed 8-bit REV or binary32 IRR, two steps,
// legal offsets/exponents/residues; IRR has a nonzero scaling factor.
fn kernel(reversible: bool, odd: bool, rounded: bool) -> ArbitraryKernel {
    let coefficient = |value: i8| {
        if reversible {
            KernelCoefficient::for_test((value as i128) as u128, 1)
        } else {
            KernelCoefficient::for_test(u128::from(f32::from(value).to_bits()), 4)
        }
    };
    ArbitraryKernel::for_test(
        reversible,
        if reversible { 0 } else { 2 },
        false,
        odd,
        false,
        (!reversible).then(|| coefficient(2)),
        vec![
            KernelLiftingStep::for_test(
                0,
                if rounded { 2 } else { 0 },
                reversible.then(|| coefficient(if rounded { 1 } else { 0 })),
                vec![coefficient(-1)],
            ),
            KernelLiftingStep::for_test(
                0,
                if rounded { 1 } else { 0 },
                reversible.then(|| coefficient(if rounded { -1 } else { 0 })),
                vec![coefficient(2)],
            ),
        ],
    )
}

// T.801 H.2.1, H.2.2, H.2.5.1/2. Work in absolute sample coordinates:
// low samples occupy even coordinates even when local coordinate zero is odd.
// Rational IRR inputs below make all operations exact in Q16, so no tolerance
// or implementation-specific rounding rule is part of this oracle.
fn reference_line(values: &mut [i64], first: bool, reversible: bool, odd: bool, rounded: bool) {
    let origin = i64::from(!first);
    if values.len() == 1 {
        if !first {
            assert_eq!(values[0] % 2, 0);
            values[0] /= 2;
        }
        return;
    }
    if !reversible {
        for (index, value) in values.iter_mut().enumerate() {
            if (index as i64 + origin) % 2 == 0 {
                *value *= 2;
            } else {
                assert_eq!(*value % 2, 0);
                *value /= 2;
            }
        }
    }
    let steps = if rounded {
        [(2, -1, 2), (-1, 1, 4)]
    } else {
        [(2, 0, 1), (-1, 0, 1)]
    };
    for (pass, (alpha, beta, denominator)) in steps.into_iter().enumerate() {
        let parity = i64::from(odd ^ (pass == 1));
        let before = values.to_vec();
        for (index, value) in values.iter_mut().enumerate() {
            let coordinate = index as i64 + origin;
            if coordinate % 2 != parity {
                continue;
            }
            let n = (coordinate - parity) / 2;
            let source = 2 * n + 1 - parity;
            // T.801 H.2.4 CON extension of the opposite-parity subsequence.
            let first_source = origin + i64::from(origin % 2 == parity);
            let last = origin + before.len() as i64 - 1;
            let last_source = last - i64::from(last % 2 == parity);
            let source = source.clamp(first_source, last_source);
            let neighbour = before[(source - origin) as usize];
            let adjustment = if reversible {
                (alpha * neighbour + beta).div_euclid(denominator)
            } else {
                alpha * neighbour
            };
            *value -= adjustment;
        }
    }
}

fn reference_plane(
    interleaved: &[i64],
    width: usize,
    height: usize,
    phases: (bool, bool),
    reversible: bool,
    odd: bool,
) -> Vec<i64> {
    // T.800 F.3.2: horizontal first, then vertical. No production transform
    // or project specification model is called in calculating expected data.
    let mut output = interleaved.to_vec();
    let rounded = width > 1 && height > 1;
    for row in output.chunks_exact_mut(width) {
        reference_line(row, phases.0, reversible, odd, rounded);
    }
    for x in 0..width {
        let mut column: Vec<_> = (0..height).map(|y| output[y * width + x]).collect();
        reference_line(&mut column, phases.1, reversible, odd, rounded);
        for y in 0..height {
            output[y * width + x] = column[y];
        }
    }
    output
}

#[test]
fn arbitrary_level_matches_standard_equations_and_rejects_bad_geometry_or_budget() {
    let invalid = DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidTransform,
    };
    let mut seed = 0x5eed_2d01_u32;
    let mut failures = Vec::new();
    let mut contexts = 0;
    eprintln!("direct inverse_level_arbitrary: fixed-seed, exact standard-equation oracle");
    for width in [1_usize, 2, 3, 4, 5, 8, 17] {
        for height in [1_usize, 2, 3, 4, 7] {
            for first_x in [false, true] {
                for first_y in [false, true] {
                    for reversible in [false, true] {
                        for odd in [false, true] {
                            let context = format!(
                                "{width}x{height}, phases={first_x}/{first_y}, REV={reversible}, minit={odd}"
                            );
                            // Singleton REV axes require exact halves; linear integer
                            // steps preserve the input's factor of four on that route.
                            let kernel = kernel(reversible, odd, width > 1 && height > 1);
                            assert_eq!(kernel.reversible(), reversible);
                            assert!(!kernel.whole_sample_filter());
                            assert!(!kernel.whole_sample_extension());
                            assert_eq!(kernel.steps().len(), 2);
                            assert_eq!(kernel.initial_odd(), odd);
                            assert_eq!(kernel.coefficient_type(), if reversible { 0 } else { 2 });
                            assert_eq!(kernel.scaling().is_some(), !reversible);
                            let data: Vec<_> = (0..width * height)
                                .map(|index| {
                                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                                    // Nonnegative IRR data and integral scaling/lifting avoid
                                    // precision ambiguity; REV includes both signs and zero.
                                    let sample = if reversible {
                                        i64::from(seed % 129) - 64
                                    } else {
                                        i64::from(seed % 129)
                                    };
                                    if reversible {
                                        sample * 4
                                    } else {
                                        (sample + index as i64) * 65536
                                    }
                                })
                                .collect();
                            let mut bands = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
                            for y in 0..height {
                                for x in 0..width {
                                    let high_x = (x % 2 == 0) != first_x;
                                    let high_y = (y % 2 == 0) != first_y;
                                    bands[usize::from(high_x) + 2 * usize::from(high_y)]
                                        .push(data[y * width + x]);
                                }
                            }
                            let low_w = (width + usize::from(first_x)) / 2;
                            let low_h = (height + usize::from(first_y)) / 2;
                            let dimensions = [
                                (low_w, low_h),
                                (width - low_w, low_h),
                                (low_w, height - low_h),
                                (width - low_w, height - low_h),
                            ];
                            let band = |i: usize| WaveletBand {
                                width: dimensions[i].0 as u32,
                                height: dimensions[i].1 as u32,
                                samples: &bands[i],
                            };
                            let level = WaveletLevel {
                                ll: band(0),
                                hl: band(1),
                                lh: band(2),
                                hh: band(3),
                                output_width: width as u32,
                                output_height: height as u32,
                                low_pass_first_x: first_x,
                                low_pass_first_y: first_y,
                            };
                            assert_eq!(bands.iter().map(Vec::len).sum::<usize>(), width * height);
                            for i in 0..4 {
                                assert_eq!(bands[i].len(), dimensions[i].0 * dimensions[i].1);
                            }
                            let snapshot = bands.clone();
                            let expected = reference_plane(
                                &data,
                                width,
                                height,
                                (first_x, first_y),
                                reversible,
                                odd,
                            );
                            let bytes = (width * height * 8) as u64;
                            let limits = ResourceLimits::default()
                                .with_maximum_decoded_bytes(bytes)
                                .unwrap();
                            let result = inverse_level_arbitrary(level, &kernel, &limits);
                            match result {
                                Ok(plane) => {
                                    if (
                                        plane.width(),
                                        plane.height(),
                                        plane.fraction_bits(),
                                        plane.samples(),
                                    ) != (
                                        width as u32,
                                        height as u32,
                                        if reversible { 0 } else { 16 },
                                        expected.as_slice(),
                                    ) {
                                        failures.push(format!(
                                            "{context}: interleaved input={data:?}; expected {expected:?}; got {plane:?}"
                                        ));
                                    }
                                }
                                Err(error) => failures.push(format!(
                                    "{context}: expected reconstructed plane, got {error:?}"
                                )),
                            }
                            assert_eq!(bands, snapshot, "input mutation: {context}");
                            let limited = limits.with_maximum_decoded_bytes(bytes - 1).unwrap();
                            assert_eq!(
                                inverse_level_arbitrary(level, &kernel, &limited),
                                Err(DecodeError::LimitExceeded {
                                    resource: Resource::DecodedBytes,
                                    limit: bytes - 1,
                                    actual: bytes,
                                }),
                                "one-byte-short budget: {context}",
                            );
                            for malformed in [
                                WaveletLevel {
                                    output_width: 0,
                                    ..level
                                },
                                WaveletLevel {
                                    output_height: 0,
                                    ..level
                                },
                                WaveletLevel {
                                    output_width: level.output_width + 1,
                                    ..level
                                },
                                WaveletLevel {
                                    hh: WaveletBand {
                                        width: level.hh.width + 1,
                                        ..level.hh
                                    },
                                    ..level
                                },
                            ] {
                                assert_eq!(
                                    inverse_level_arbitrary(malformed, &kernel, &limits),
                                    Err(invalid),
                                    "invalid geometry: {context}",
                                );
                            }
                            for index in 0..4 {
                                for length in
                                    [bands[index].len().saturating_sub(1), bands[index].len() + 1]
                                {
                                    if length == bands[index].len() {
                                        continue;
                                    }
                                    let wrong_samples = vec![0; length];
                                    let mut malformed = level;
                                    let target = match index {
                                        0 => &mut malformed.ll,
                                        1 => &mut malformed.hl,
                                        2 => &mut malformed.lh,
                                        _ => &mut malformed.hh,
                                    };
                                    target.samples = &wrong_samples;
                                    assert_eq!(
                                        inverse_level_arbitrary(malformed, &kernel, &limits),
                                        Err(invalid),
                                        "band {index} sample length {length}: {context}",
                                    );
                                }
                            }
                            assert_eq!(bands, snapshot, "error-path input mutation: {context}");
                            contexts += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(contexts, 560);
    eprintln!(
        "completed {contexts} combinations; {} exact-output mismatches; geometry, sample-length, budget and immutability checks completed",
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "{} standard-equation mismatches; first examples:\n{}",
        failures.len(),
        failures
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
