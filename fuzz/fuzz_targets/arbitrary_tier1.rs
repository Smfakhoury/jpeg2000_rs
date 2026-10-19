#![no_main]

use jpeg2000_rs::{
    ResourceLimits,
    mq::MqDecoder,
    tier1::{
        CodeBlockPlane, CodeBlockSegmentAccounting, HtForwardMagSgnReader,
        HtForwardRefinementReader, HtPassGroup, HtReverseRefinementReader, RawBitReader,
        SubbandOrientation, Tier1BitplaneState, Tier1BlockCoding, Tier1CodeBlockDecoder,
        Tier1CodingStyle, complete_ht_kappa,
        decode_cleanup_pass_with_style, decode_ht_cleanup_metadata,
        decode_magnitude_refinement_pass_with_style, decode_raw_magnitude_refinement_pass,
        decode_raw_significance_propagation_pass,
        decode_significance_propagation_pass_with_style, initialize_tier1_contexts,
        magnitude_refinement_context, parse_ht_block_segments, sign_context,
        zero_coding_context,
    },
    transform::{QuantizationStep, inverse_5_3, inverse_9_7, inverse_quantize, undo_maxshift},
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let limits = ResourceLimits::default();
    if let Ok(mut magsgn) = HtForwardMagSgnReader::new(data, &limits) {
        let requested = data.first().copied().unwrap_or(0) % 40;
        let _ = magsgn.read_bits(requested);
        let _ = magsgn.finish();
    }
    if let (Ok(mut forward), Ok(mut reverse)) = (
        HtForwardRefinementReader::new(data, &limits),
        HtReverseRefinementReader::new(data, &limits),
    ) {
        let decisions = usize::from(data.first().copied().unwrap_or(0));
        for _ in 0..decisions {
            let _ = forward.read_bit();
            let _ = reverse.read_bit();
        }
    }
    let _ = complete_ht_kappa(
        data.first().copied().unwrap_or(1).min(39),
        data.get(1).copied().unwrap_or(0) & 0x0f,
        data.get(2).copied().unwrap_or(0) % 40,
        data.get(3).copied().unwrap_or(0) % 40,
        data.get(4).copied().unwrap_or(39).clamp(1, 39),
    );
    let Ok(mut reader) = RawBitReader::new(data, &limits) else {
        return;
    };
    let decisions = usize::from(data.first().copied().unwrap_or(0));
    for _ in 0..decisions {
        if reader.read_bit().is_err() {
            break;
        }

        let low = [i64::from(data.first().copied().unwrap_or(0)) - 128];
        let high = [i64::from(data.get(1).copied().unwrap_or(0)) - 128];
        let _ = inverse_5_3(&low, &high, true);
        let _ = inverse_9_7(&low, &high, true);
        let _ = undo_maxshift(low[0], data.get(2).copied().unwrap_or(0));
        let _ = inverse_quantize(
            &low,
            QuantizationStep {
                exponent: data.get(3).copied().unwrap_or(0) % 39,
                mantissa: u16::from(data.get(4).copied().unwrap_or(0)),
            },
            data.get(5).copied().unwrap_or(1) % 38 + 1,
            data.get(6).copied().unwrap_or(0) & 1 != 0,
            data.get(7).copied().unwrap_or(0),
            &limits,
        );
    }

    let maximum_passes = u32::from(data.get(1).copied().unwrap_or(0)) + 1;
    let Ok(mut accounting) = CodeBlockSegmentAccounting::new(maximum_passes, &limits) else {
        return;
    };
    for chunk in data.get(2..).unwrap_or_default().chunks(2) {
        let passes = u32::from(chunk[0] % 38) + 1;
        let bytes = u32::from(chunk.get(1).copied().unwrap_or(0)) + 1;
        if accounting.add_segment(passes, bytes).is_err() {
            break;
        }

        let split = data.len().saturating_sub(2) / 2;
        let cleanup_end = split.saturating_add(2).min(data.len());
        let cleanup = data.get(..cleanup_end).unwrap_or_default();
        let refinement = data.get(cleanup_end..);
        let _ = parse_ht_block_segments(cleanup, refinement, &limits);
        let width = u16::from(data.get(2).copied().unwrap_or(0) % 64) + 1;
        let height = u16::from(data.get(3).copied().unwrap_or(0) % 64) + 1;
        if u32::from(width) * u32::from(height) <= 4096 {
            let maximum_u = data.get(4).copied().unwrap_or(0) % 74 + 1;
            let _ = decode_ht_cleanup_metadata(cleanup, width, height, maximum_u, &limits);
        }
        let packet_passes = data.first().copied().unwrap_or(0) % 164 + 1;
        if let Ok(group) = HtPassGroup::from_packet_passes(packet_passes) {
            let style = Tier1CodingStyle::from_code_block_style(0x40).unwrap();
            assert_eq!(style.block_coding, Tier1BlockCoding::HighThroughput);
            if let Ok(mut decoder) =
                Tier1CodeBlockDecoder::new(
                    1,
                    1,
                    u16::from(data.get(5).copied().unwrap_or(0)) + 37,
                    SubbandOrientation::Ll,
                    style,
                )
            {
                decoder.set_multiple_ht_sets_allowed(data.get(6).copied().unwrap_or(0) & 1 != 0);
                let _ = decoder
                    .set_ht_magnitude_bound(data.get(7).copied().unwrap_or(0) % 74 + 1);
                let refinement = (group.refinement_passes > 0).then_some(refinement.unwrap_or_default());
                let _ = decoder.decode_high_throughput_segments(
                    cleanup,
                    group.cleanup_passes,
                    refinement,
                    group.refinement_passes,
                    &limits,
                );
                if group.refinement_passes == 0 {
                    let _ = decoder.decode_segment(cleanup, packet_passes, &limits);
                }
            }

            let _ = decode_ht_cleanup_metadata(&[0x00, 0x63, 0x00], 1, 1, 1, &limits);
        }
    }

    let width = u16::from(data.first().copied().unwrap_or(0) % 64) + 1;
    let height = u16::from(data.get(1).copied().unwrap_or(0) % 64) + 1;
    if u32::from(width) * u32::from(height) <= 4096 {
        let mut plane = CodeBlockPlane::new(width, height).unwrap();
        let x = u16::from(data.get(2).copied().unwrap_or(0)) % width;
        let y = u16::from(data.get(3).copied().unwrap_or(0)) % height;
        let orientation = match data.get(5).copied().unwrap_or(0) % 4 {
            0 => SubbandOrientation::Ll,
            1 => SubbandOrientation::Lh,
            2 => SubbandOrientation::Hl,
            _ => SubbandOrientation::Hh,
        };
        let _ = plane.mark_significant(x, y, 8, data.get(4).copied().unwrap_or(0) & 1 != 0);
        if let Ok(summary) = plane.neighbor_summary(x, y) {
            let _ = zero_coding_context(summary, orientation);
            let _ = sign_context(summary);
            if let Ok(coefficient) = plane.coefficient(x, y) {
                let _ = magnitude_refinement_context(coefficient, summary);
            }
        }
        let _ = plane.refine(x, y, 4);
        let style =
            Tier1CodingStyle::from_code_block_style(data.get(6).copied().unwrap_or(0) & 0x3f)
                .unwrap();

        if let (Ok(mut pass_state), Ok(mut decoder)) = (
            Tier1BitplaneState::new(&plane, 2),
            MqDecoder::new(data, &limits),
        ) {
            if initialize_tier1_contexts(&mut decoder).is_ok()
                && decode_significance_propagation_pass_with_style(
                    &mut plane,
                    &mut pass_state,
                    &mut decoder,
                    orientation,
                    style,
                )
                .is_ok()
                && decode_magnitude_refinement_pass_with_style(
                    &mut plane,
                    &mut pass_state,
                    &mut decoder,
                    style,
                )
                .is_ok()
            {
                let _ = decode_cleanup_pass_with_style(
                    &mut plane,
                    &mut pass_state,
                    &mut decoder,
                    orientation,
                    style,
                );
            }
        }

        if style.uses_raw_bypass(10) {
            let mut raw_plane = CodeBlockPlane::new(2, 1).unwrap();
            raw_plane.mark_significant(0, 0, 8, false).unwrap();
            let mut raw_state = Tier1BitplaneState::new(&raw_plane, 4).unwrap();
            let mut raw_reader = RawBitReader::new(data, &limits).unwrap();
            if decode_raw_significance_propagation_pass(
                &mut raw_plane,
                &mut raw_state,
                &mut raw_reader,
                orientation,
                style,
            )
            .is_ok()
            {
                let _ = decode_raw_magnitude_refinement_pass(
                    &mut raw_plane,
                    &mut raw_state,
                    &mut raw_reader,
                    style,
                );
            }
        }
    }
});
