fn wave86_decoded_kernel() -> ArbitraryKernel {
    // T.801 A.3.5: REV/ARB/CON, minit=0, signed8, two one-coefficient steps.
    let payload = [0x10, 0xff, 2, 0, 0, 0, 1, 0xff, 0, 0, 0, 1, 2];
    let limits = ResourceLimits::default();
    let parser = new_parser(&[], &limits).unwrap();
    let kernel = parser
        .decode_atk_payload(Segment {
            marker: ATK,
            offset: 0,
            payload: &payload,
        })
        .unwrap();
    assert_eq!(kernel.index(), 255);
    assert_eq!(kernel.descriptor(), 0x10ff);
    assert!(kernel.reversible());
    assert_eq!(kernel.coefficient_type(), 0);
    assert!(!kernel.whole_sample_filter());
    assert!(!kernel.whole_sample_extension());
    assert!(!kernel.initial_odd());
    assert!(kernel.scaling().is_none());
    assert_eq!(kernel.steps().len(), 2);
    for (step, alpha) in kernel.steps().iter().zip([-1_i128, 2]) {
        assert_eq!(step.offset(), 0);
        assert_eq!(step.exponent(), 0);
        assert_eq!(*step.residue().unwrap().bytes(), [0; 16]);
        assert_eq!(step.coefficients().len(), 1);
        assert_eq!(step.coefficients()[0].width(), 1);
        assert_eq!(*step.coefficients()[0].bytes(), alpha.to_be_bytes());
    }
    kernel
}

#[test]
fn wave86_arbitrary_even_origin_control() {
    let kernel = wave86_decoded_kernel();
    let actual = crate::transform::inverse_arbitrary(&[-180, -80], &[256], true, &kernel).unwrap();
    // H-3: even targets subtract2*256, then the odd target adds the updated left even.
    assert_eq!(actual, [-692, -436, -592]);
}

#[test]
fn wave86_arbitrary_odd_origin_standard_witness() {
    use crate::transform::{WaveletBand, WaveletLevel, inverse_level_arbitrary};
    let kernel = wave86_decoded_kernel();
    let low = [256];
    let high = [-180, -80];
    let level = WaveletLevel {
        ll: WaveletBand {
            width: 0,
            height: 1,
            samples: &[],
        },
        hl: WaveletBand {
            width: 1,
            height: 1,
            samples: &low,
        },
        lh: WaveletBand {
            width: 0,
            height: 2,
            samples: &[],
        },
        hh: WaveletBand {
            width: 1,
            height: 2,
            samples: &high,
        },
        output_width: 1,
        output_height: 3,
        low_pass_first_x: false,
        low_pass_first_y: false,
    };
    let limits = ResourceLimits::default()
        .with_maximum_decoded_bytes(24)
        .unwrap();
    let plane = inverse_level_arbitrary(level, &kernel, &limits).unwrap();
    assert_eq!(
        (plane.width(), plane.height(), plane.fraction_bits()),
        (1, 3, 0)
    );
    assert_eq!(low, [256]);
    assert_eq!(high, [-180, -80]);
    // H.2.1 halves horizontal singletons to[-90,128,-40]. H-3 then gives
    // middle128-2*(-40)=208 and endpoints -90+208=118, -40+208=168.
    eprintln!(
        "T.801 H-3: expected[118,208,168], actual{:?}",
        plane.samples()
    );
    assert_eq!(plane.samples(), [118, 208, 168]);
}

#[test]
fn wave86_irreversible_singleton_standard_witness() {
    use crate::transform::{WaveletBand, inverse_horizontal_9_7};
    let input = [131072];
    let limits = ResourceLimits::default()
        .with_maximum_decoded_bytes(8)
        .unwrap();
    let plane = inverse_horizontal_9_7(
        WaveletBand {
            width: 0,
            height: 1,
            samples: &[],
        },
        WaveletBand {
            width: 1,
            height: 1,
            samples: &input,
        },
        1,
        false,
        &limits,
    )
    .unwrap();
    assert_eq!(
        (plane.width(), plane.height(), plane.fraction_bits()),
        (1, 1, 16)
    );
    assert_eq!(input, [131072]);
    eprintln!("T.800 F.3.6: expected[65536], actual{:?}", plane.samples());
    assert_eq!(plane.samples(), [65536]);
}
