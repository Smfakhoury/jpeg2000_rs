#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires divisor > 0,
        ensures returns (crate::specs::part1_ceil_div(value as nat, divisor as nat) as u32),
    )
)]
pub(crate) const fn ceil_div_u32(value: u32, divisor: u32) -> u32 {
    if value == 0 {
        0
    } else {
        1 + (value - 1) / divisor
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::part1_clip_start(nominal as nat, image_start as nat) as u32
        ),
    )
)]
pub(crate) const fn tile_clip_start(nominal: u32, image_start: u32) -> u32 {
    if nominal < image_start {
        image_start
    } else {
        nominal
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::part1_clip_end(nominal_end as nat, image_end as nat) as u32
        ),
    )
)]
pub(crate) const fn tile_clip_end(nominal_end: u32, image_end: u32) -> u32 {
    if nominal_end < image_end {
        nominal_end
    } else {
        image_end
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (crate::specs::part1_packet_data_bits(previous_was_ff) as u8),
    )
)]
pub(crate) const fn packet_data_bits(previous_was_ff: bool) -> u8 {
    if previous_was_ff { 7 } else { 8 }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires remaining < 8,
        ensures returns crate::specs::part1_packet_bit(byte, remaining),
    )
)]
pub(crate) const fn packet_bit(byte: u8, remaining: u8) -> bool {
    ((byte >> remaining) & 1) != 0
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::part1_tag_inherited_low(low as nat, inherited as nat) as u32
        ),
    )
)]
pub(crate) const fn tag_inherited_low(low: u32, inherited: u32) -> u32 {
    if low < inherited { inherited } else { low }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires !bit ==> low < u32::MAX,
        ensures returns (crate::specs::part1_tag_step_low(low as nat, bit) as u32),
    )
)]
pub(crate) const fn tag_step_low(low: u32, bit: bool) -> u32 {
    if bit { low } else { low + 1 }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires low <= value,
        ensures returns (
            crate::specs::part1_tag_step_value(low as nat, value as nat, bit) as u32
        ),
    )
)]
pub(crate) const fn tag_step_value(low: u32, value: u32, bit: bool) -> u32 {
    if bit { low } else { value }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires dimension > 0,
        ensures returns (
            crate::specs::part1_progression_digit(index as nat, dimension as nat) as u64
        ),
    )
)]
pub(crate) const fn progression_digit(index: u64, dimension: u64) -> u64 {
    index % dimension
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires dimension > 0,
        ensures returns (
            crate::specs::part1_progression_advance(index as nat, dimension as nat) as u64
        ),
    )
)]
pub(crate) const fn progression_advance(index: u64, dimension: u64) -> u64 {
    index / dimension
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_progression_emit_allowed(already_seen),
    )
)]
pub(crate) const fn progression_emit_allowed(already_seen: bool) -> bool {
    !already_seen
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_mq_decision(mps, lps),)
)]
pub(crate) const fn mq_decision(mps: bool, lps: bool) -> bool {
    if lps { !mps } else { mps }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_mq_next_mps(mps, switch_mps),)
)]
pub(crate) const fn mq_next_mps(mps: bool, switch_mps: bool) -> bool {
    if switch_mps { !mps } else { mps }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (crate::specs::part1_tier1_pass_code(pass_index as nat) as u8),
    )
)]
pub(crate) const fn tier1_pass_code(pass_index: u32) -> u8 {
    if pass_index == 0 {
        2
    } else {
        ((pass_index - 1) % 3) as u8
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::part1_tier1_refinement_context(refined, neighbors as nat) as u8
        ),
    )
)]
pub(crate) const fn tier1_refinement_context(refined: bool, neighbors: u16) -> u8 {
    if refined {
        16
    } else if neighbors > 0 {
        15
    } else {
        14
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires !refinement_bit || magnitude <= u64::MAX - delta,
        ensures returns (
            crate::specs::part1_tier1_refined_magnitude(
                magnitude as nat,
                delta as nat,
                refinement_bit,
            ) as u64
        ),
    )
)]
pub(crate) const fn tier1_refined_magnitude(
    magnitude: u64,
    delta: u64,
    refinement_bit: bool,
) -> u64 {
    if refinement_bit {
        magnitude + delta
    } else {
        magnitude
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires bitplane <= 64,
        ensures returns crate::specs::part1_tier1_midpoint_delta(bitplane),
    )
)]
pub(crate) const fn tier1_midpoint_delta(bitplane: u16) -> u64 {
    if bitplane == 0 {
        0
    } else {
        1_u64 << (bitplane - 1)
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_maxshift_restore(magnitude, shift),
    )
)]
pub(crate) const fn maxshift_restore(magnitude: u64, shift: u8) -> u64 {
    if shift >= 64 {
        magnitude
    } else {
        let threshold = 1_u64 << shift;
        if magnitude < threshold {
            magnitude
        } else {
            magnitude >> shift
        }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::part1_reversible_inverse_quantize(coefficient as int) as i64
        ),
    )
)]
pub(crate) const fn reversible_inverse_quantize(coefficient: i64) -> i64 {
    coefficient
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_five_three_predict(sum),
    )
)]
pub(crate) const fn five_three_predict(sum: i64) -> i64 {
    sum >> 1
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_five_three_update(biased_sum),
    )
)]
pub(crate) const fn five_three_update(biased_sum: i64) -> i64 {
    biased_sum >> 2
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires
            i64::MIN <= y as int - ((chroma_sum >> 2) as int),
            y as int - ((chroma_sum >> 2) as int) <= i64::MAX,
        ensures returns crate::specs::part1_rct_green(y, chroma_sum),
    )
)]
pub(crate) const fn rct_green(y: i64, chroma_sum: i64) -> i64 {
    y - (chroma_sum >> 2)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires
            i64::MIN <= green as int + cr as int,
            green as int + cr as int <= i64::MAX,
        ensures returns (crate::specs::part1_rct_red(green as int, cr as int) as i64),
    )
)]
pub(crate) const fn rct_red(green: i64, cr: i64) -> i64 {
    green + cr
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires
            i64::MIN <= green as int + cb as int,
            green as int + cb as int <= i64::MAX,
        ensures returns (crate::specs::part1_rct_blue(green as int, cb as int) as i64),
    )
)]
pub(crate) const fn rct_blue(green: i64, cb: i64) -> i64 {
    green + cb
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires precision > 0 && precision <= 63,
        ensures returns crate::specs::part1_level_shift(precision, signed),
    )
)]
pub(crate) const fn level_shift(precision: u8, signed: bool) -> i64 {
    if signed { 0 } else { 1_i64 << (precision - 1) }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_clip_sample(value, minimum, maximum),
    )
)]
pub(crate) const fn clip_sample(value: i64, minimum: i64, maximum: i64) -> i64 {
    if value < minimum {
        minimum
    } else if value > maximum {
        maximum
    } else {
        value
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_marker_value(high, low),)
)]
pub(crate) const fn marker_value(high: u8, low: u8) -> u16 {
    ((high as u16) << 8) | (low as u16)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_parser_progress(previous, current, limit),
    )
)]
pub(crate) const fn parser_progress(previous: usize, current: usize, limit: usize) -> bool {
    previous < current && current <= limit
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_main_header_boundary(marker),)
)]
pub(crate) const fn main_header_boundary(marker: u16) -> bool {
    marker == 0xff4f || marker == 0xff93 || marker == 0xffd9
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_tile_header_boundary(marker),)
)]
pub(crate) const fn tile_header_boundary(marker: u16) -> bool {
    marker == 0xff4f || marker == 0xff51 || marker == 0xff90 || marker == 0xffd9
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_packet_next_included(old, contribution_present),
    )
)]
pub(crate) const fn packet_next_included(old: bool, contribution_present: bool) -> bool {
    old || contribution_present
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires lblock < 31,
        ensures returns crate::specs::part1_packet_next_lblock(lblock),
    )
)]
pub(crate) const fn packet_next_lblock(lblock: u8) -> u8 {
    lblock + 1
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires old <= u32::MAX - added,
        ensures returns crate::specs::part1_packet_next_passes(old, added),
    )
)]
pub(crate) const fn packet_next_passes(old: u32, added: u32) -> u32 {
    old + added
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_exact_consumption(consumed, expected),)
)]
pub(crate) const fn exact_consumption(consumed: usize, expected: usize) -> bool {
    consumed == expected
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires tile <= usize::MAX - packed,
        ensures returns crate::specs::part1_packet_logical_bytes(tile, packed),
    )
)]
pub(crate) const fn packet_logical_bytes(tile: usize, packed: usize) -> usize {
    tile + packed
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires (qe as u32) <= interval,
        ensures returns crate::specs::part1_mq_interval_subtract(interval, qe),
    )
)]
pub(crate) const fn mq_interval_subtract(interval: u32, qe: u16) -> u32 {
    interval - (qe as u32)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires (qe as u64) * 65536 <= code as u64,
        ensures returns crate::specs::part1_mq_code_subtract(code, qe),
    )
)]
pub(crate) const fn mq_code_subtract(code: u32, qe: u16) -> u32 {
    ((code as u64) - (qe as u64) * 65_536) as u32
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_mq_byte_class(current, next),)
)]
pub(crate) const fn mq_byte_class(current: u8, next: u8) -> u8 {
    if current == 0xff && next > 0x8f {
        0
    } else if current == 0xff {
        1
    } else {
        2
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_mq_byte_addend(current, next),)
)]
pub(crate) const fn mq_byte_addend(current: u8, next: u8) -> u32 {
    if current == 0xff && next > 0x8f {
        0xff00
    } else if current == 0xff {
        (next as u32) << 9
    } else {
        (next as u32) << 8
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_mq_byte_ct(current, next),)
)]
pub(crate) const fn mq_byte_ct(current: u8, next: u8) -> u8 {
    if current == 0xff && next <= 0x8f {
        7
    } else {
        8
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_mq_byte_advances(current, next),)
)]
pub(crate) const fn mq_byte_advances(current: u8, next: u8) -> bool {
    !(current == 0xff && next > 0x8f)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires value <= u32::MAX / 2,
        ensures returns crate::specs::part1_mq_renorm_double(value),
    )
)]
pub(crate) const fn mq_renorm_double(value: u32) -> u32 {
    value * 2
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires ct > 0,
        ensures returns crate::specs::part1_mq_renorm_ct(ct),
    )
)]
pub(crate) const fn mq_renorm_ct(ct: u8) -> u8 {
    ct - 1
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires y < height,
        ensures returns crate::specs::part1_tier1_stripe_rows(height, y),
    )
)]
pub(crate) const fn tier1_stripe_rows(height: u16, y: u16) -> u16 {
    let remaining = height - y;
    if remaining < 4 { remaining } else { 4 }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_tier1_sigprop_eligible(significant, neighbors),
    )
)]
pub(crate) const fn tier1_sigprop_eligible(significant: bool, neighbors: u16) -> bool {
    !significant && neighbors > 0
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_tier1_magref_eligible(significant_before),
    )
)]
pub(crate) const fn tier1_magref_eligible(significant_before: bool) -> bool {
    significant_before
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_tier1_cleanup_eligible(significant, visited),
    )
)]
pub(crate) const fn tier1_cleanup_eligible(significant: bool, visited: bool) -> bool {
    !significant && !visited
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_tier1_pass_complete(kind),)
)]
pub(crate) const fn tier1_pass_complete(kind: u8) -> u8 {
    if kind == 0 {
        1
    } else if kind == 1 {
        2
    } else {
        4
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_tier1_zero_context(
            horizontal,
            vertical,
            diagonal,
            high_high,
        ),
    )
)]
pub(crate) const fn tier1_zero_context(
    horizontal: u16,
    vertical: u16,
    diagonal: u16,
    high_high: bool,
) -> u8 {
    if high_high {
        let direct = (horizontal as u32) + (vertical as u32);
        if diagonal == 0 {
            if direct == 0 {
                0
            } else if direct == 1 {
                1
            } else {
                2
            }
        } else if diagonal == 1 {
            if direct == 0 {
                3
            } else if direct == 1 {
                4
            } else {
                5
            }
        } else if diagonal == 2 {
            if direct == 0 { 6 } else { 7 }
        } else {
            8
        }
    } else if horizontal == 0 {
        if vertical == 0 {
            if diagonal == 0 {
                0
            } else if diagonal == 1 {
                1
            } else {
                2
            }
        } else if vertical == 1 {
            3
        } else {
            4
        }
    } else if horizontal == 1 {
        if vertical == 0 {
            if diagonal == 0 { 5 } else { 6 }
        } else {
            7
        }
    } else {
        8
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_tier1_sign_offset(horizontal, vertical),
    )
)]
pub(crate) const fn tier1_sign_offset(horizontal: i8, vertical: i8) -> u8 {
    if horizontal == 0 {
        if vertical == 0 { 0 } else { 1 }
    } else if vertical < 0 {
        2
    } else if vertical == 0 {
        3
    } else {
        4
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_tier1_predicted_negative(horizontal, vertical),
    )
)]
pub(crate) const fn tier1_predicted_negative(horizontal: i8, vertical: i8) -> bool {
    horizontal < 0 || (horizontal == 0 && vertical < 0)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_tier1_decoded_sign(symbol, predicted, raw),
    )
)]
pub(crate) const fn tier1_decoded_sign(symbol: bool, predicted: bool, raw: bool) -> bool {
    if raw { symbol } else { symbol ^ predicted }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_irreversible_centered(magnitude),
    )
)]
pub(crate) const fn irreversible_centered(magnitude: i64) -> i128 {
    (magnitude as i128) * 2 + 1
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires product >= 0,
        ensures returns crate::specs::part1_irreversible_scale(product),
    )
)]
pub(crate) const fn irreversible_scale(product: i128) -> i128 {
    product / 2
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_fixed_round_q16(adjusted),
    )
)]
pub(crate) const fn fixed_round_q16(adjusted: i64) -> i64 {
    adjusted >> 16
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_nine_seven_scale(low),)
)]
pub(crate) const fn nine_seven_scale(low: bool) -> i64 {
    if low { 80_621 } else { 53_274 }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_nine_seven_lift(stage),)
)]
pub(crate) const fn nine_seven_lift(stage: u8) -> i64 {
    if stage == 0 {
        29_066
    } else if stage == 1 {
        57_862
    } else if stage == 2 {
        -3_472
    } else {
        -103_949
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_wavelet_detail_advance(direction),)
)]
pub(crate) const fn wavelet_detail_advance(direction: u8) -> usize {
    if direction == 3 {
        3
    } else if direction == 0 {
        0
    } else {
        1
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires resolution < u8::MAX,
        ensures returns crate::specs::part1_wavelet_next_resolution(resolution),
    )
)]
pub(crate) const fn wavelet_next_resolution(resolution: u8) -> u8 {
    resolution + 1
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_wavelet_sample_count(width, height, samples),
    )
)]
pub(crate) const fn wavelet_sample_count(width: u32, height: u32, samples: usize) -> bool {
    if width == 0 {
        samples == 0
    } else {
        samples.is_multiple_of(width as usize) && samples / (width as usize) == height as usize
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_sop_sequence(high, low),)
)]
pub(crate) const fn sop_sequence(high: u8, low: u8) -> u16 {
    ((high as u16) << 8) | (low as u16)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_sop_valid(marker0, marker1, length0, length1),
    )
)]
pub(crate) const fn sop_valid(marker0: u8, marker1: u8, length0: u8, length1: u8) -> bool {
    marker0 == 0xff && marker1 == 0x91 && length0 == 0 && length1 == 4
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_eph_valid(marker0, marker1),)
)]
pub(crate) const fn eph_valid(marker0: u8, marker1: u8) -> bool {
    marker0 == 0xff && marker1 == 0x92
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_packet_length_matches(actual, expected),
    )
)]
pub(crate) const fn packet_length_matches(actual: usize, expected: usize) -> bool {
    actual == expected
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_decode_output_shape_valid(
            input_bytes,
            tile_parts,
            expected_components,
            output_components,
            width,
            height,
        ),
    )
)]
pub(crate) const fn decode_output_shape_valid(
    input_bytes: usize,
    tile_parts: usize,
    expected_components: u16,
    output_components: usize,
    width: u32,
    height: u32,
) -> bool {
    input_bytes > 0
        && tile_parts > 0
        && expected_components > 0
        && expected_components as usize == output_components
        && width > 0
        && height > 0
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_component_output_shape_valid(width, height, samples),
    )
)]
pub(crate) const fn component_output_shape_valid(width: u32, height: u32, samples: usize) -> bool {
    width > 0 && height > 0 && crate::refinement::wavelet_sample_count(width, height, samples)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part2_tlm_tile_index_bytes(style),)
)]
pub(crate) const fn tlm_tile_index_bytes(style: u8) -> usize {
    ((style >> 4) & 3) as usize
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part2_tlm_length_bytes(style),)
)]
pub(crate) const fn tlm_length_bytes(style: u8) -> usize {
    if style & 0x40 == 0 { 2 } else { 4 }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part2_crg_payload_bytes(components),)
)]
pub(crate) const fn crg_payload_bytes(components: u16) -> usize {
    components as usize * 4
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires elements <= usize::MAX - 3,
        ensures returns crate::specs::part2_packed_two_bit_bytes(elements),
    )
)]
#[allow(clippy::manual_div_ceil)]
pub(crate) const fn packed_two_bit_bytes(elements: usize) -> usize {
    (elements + 3) / 4
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires slot < 4,
        ensures returns crate::specs::part2_dfs_direction_code(byte, slot),
    )
)]
pub(crate) const fn dfs_direction_code(byte: u8, slot: u8) -> u8 {
    (byte >> (6 - 2 * slot)) & 3
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part2_cbd_precision(encoded),)
)]
pub(crate) const fn cbd_precision(encoded: u8) -> u8 {
    ((encoded & 0x7f) as u16 + 1) as u8
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part2_cbd_signed(encoded),)
)]
pub(crate) const fn cbd_signed(encoded: u8) -> bool {
    encoded & 0x80 != 0
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires source != i128::MIN, 0 <= half, half < i128::MAX,
        ensures returns crate::specs::part2_nlt_binary_complement(source, half),
    )
)]
pub(crate) const fn nlt_binary_complement(source: i128, half: i128) -> i128 {
    if source < 0 {
        -source - (half + 1)
    } else {
        source
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires remaining > 0,
        ensures returns crate::specs::part2_atk_reconstruction_step(remaining),
    )
)]
pub(crate) const fn atk_reconstruction_step(remaining: usize) -> usize {
    remaining - 1
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part2_atk_next_parity(current),)
)]
pub(crate) const fn atk_next_parity(current: bool) -> bool {
    !current
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires index < u16::MAX,
        ensures returns crate::specs::part2_fragment_next(index),
    )
)]
pub(crate) const fn mct_fragment_next(index: u16) -> u16 {
    index + 1
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part2_fragment_complete(next, final_index),)
)]
pub(crate) const fn mct_fragment_complete(next: u16, final_index: u16) -> bool {
    next as u32 == final_index as u32 + 1
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires remaining > 0,
        ensures returns crate::specs::part2_mct_reverse_stage(remaining),
    )
)]
pub(crate) const fn mct_reverse_stage(remaining: usize) -> usize {
    remaining - 1
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires
            i32::MIN + 32 <= binary_exponent,
            binary_exponent <= i32::MAX - 32,
        ensures returns crate::specs::part2_binary_fixed_shift(binary_exponent),
    )
)]
pub(crate) const fn binary_fixed_shift(binary_exponent: i32) -> i32 {
    binary_exponent + 32
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part2_palette_coordinates_valid(
            entry,
            entries,
            column,
            columns,
        ),
    )
)]
pub(crate) const fn palette_coordinates_valid(
    entry: u16,
    entries: u16,
    column: u8,
    columns: u8,
) -> bool {
    entry < entries && column < columns
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part2_component_mapping_valid(
            component,
            components,
            mapping_type,
            palette_column,
            palette_columns,
        ),
    )
)]
pub(crate) const fn component_mapping_valid(
    component: u16,
    components: u16,
    mapping_type: u8,
    palette_column: u8,
    palette_columns: u8,
) -> bool {
    component < components
        && ((mapping_type == 0 && palette_column == 0)
            || (mapping_type == 1 && palette_columns > 0 && palette_column < palette_columns))
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part2_channel_definition_valid(
            channel,
            channel_type,
            association,
            output_channels,
        ),
    )
)]
pub(crate) const fn channel_definition_valid(
    channel: u16,
    channel_type: u16,
    association: u16,
    output_channels: u16,
) -> bool {
    channel < output_channels
        && matches!(channel_type, 0 | 1 | 2 | 0xffff)
        && (association <= output_channels || association == 0xffff)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part2_first_override_allowed(already_overridden),)
)]
pub(crate) const fn first_override_allowed(already_overridden: bool) -> bool {
    !already_overridden
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires
            level >= 0,
            value <= i64::MAX - level,
        ensures returns crate::specs::part2_signed_to_nominal(value, level),
    )
)]
pub(crate) const fn signed_to_nominal(value: i64, level: i64) -> i64 {
    value + level
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part2_effective_layer_count(
            explicit_layers,
            codestreams,
        ),
    )
)]
pub(crate) const fn effective_layer_count(explicit_layers: usize, codestreams: usize) -> usize {
    if explicit_layers == 0 {
        codestreams
    } else {
        explicit_layers
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires grid > 0,
        ensures returns crate::specs::part2_registered_extent(numerator, grid),
    )
)]
pub(crate) const fn registered_extent(numerator: u64, grid: u64) -> u64 {
    if numerator == 0 {
        0
    } else {
        1 + (numerator - 1) / grid
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires
            subsampling > 0,
            offset <= numerator,
        ensures returns crate::specs::part2_registered_source_coordinate(
            numerator,
            offset,
            subsampling,
        ),
    )
)]
pub(crate) const fn registered_source_coordinate(
    numerator: u32,
    offset: u32,
    subsampling: u32,
) -> u32 {
    (numerator - offset) / subsampling
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part2_composition_clip(offset, extent, canvas),)
)]
pub(crate) const fn composition_clip(offset: u32, extent: u32, canvas: u32) -> u32 {
    if offset >= canvas {
        0
    } else if extent <= canvas - offset {
        extent
    } else {
        canvas - offset
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires frames > 0,
        ensures returns crate::specs::part2_loop_frame(index, frames),
    )
)]
pub(crate) const fn loop_frame(index: usize, frames: usize) -> usize {
    index % frames
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part2_inverse_alpha(source_alpha),)
)]
pub(crate) const fn inverse_alpha(source_alpha: u16) -> u16 {
    u16::MAX - source_alpha
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(output =>
        ensures output as nat == crate::specs::part2_source_over_alpha_numerator(source_alpha, destination_alpha),
            output <= 4294836225,
    )
)]
pub(crate) const fn source_over_alpha_numerator(source_alpha: u16, destination_alpha: u16) -> u64 {
    let source = source_alpha as u64;
    let destination = destination_alpha as u64;
    let inverse = inverse_alpha(source_alpha) as u64;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_source_over_term_bounds(source, 65535, 0);
        crate::proofs::production_impl::production_source_over_term_bounds(destination, inverse, 0);
        crate::proofs::spec_properties::source_over_numerators_are_bounded(
            0, 0, source_alpha, destination_alpha, false,
        );
    }
    source * 65_535 + destination * inverse
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(output =>
        ensures output as nat == crate::specs::part2_source_over_color_numerator(
                source, destination, source_alpha, destination_alpha, premultiplied,
            ),
            (!premultiplied || source <= source_alpha) ==> output as nat
                <= 65535 * crate::specs::part2_source_over_alpha_numerator(source_alpha, destination_alpha),
    )
)]
pub(crate) const fn source_over_color_numerator(
    source: u16,
    destination: u16,
    source_alpha: u16,
    destination_alpha: u16,
    premultiplied: bool,
) -> u64 {
    #[cfg(verus_keep_ghost)]
    proof! {
        if !premultiplied || source <= source_alpha {
            crate::proofs::spec_properties::source_over_numerators_are_bounded(
                source, destination, source_alpha, destination_alpha, premultiplied,
            );
        }
    }
    let source = source as u64;
    let destination = destination as u64;
    let inverse = inverse_alpha(source_alpha) as u64;
    let source_alpha = source_alpha as u64;
    let destination_alpha = destination_alpha as u64;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_source_over_term_bounds(source, 65535, 65535);
        crate::proofs::production_impl::production_source_over_term_bounds(source, source_alpha, 65535);
        crate::proofs::production_impl::production_source_over_term_bounds(destination, destination_alpha, inverse);
    }
    let source_term = if premultiplied {
        source * 65_535 * 65_535
    } else {
        source * source_alpha * 65_535
    };
    source_term + destination * destination_alpha * inverse
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(output =>
        ensures output as nat == crate::specs::part2_source_over_rounded_ratio(numerator, denominator),
            denominator == 0 ==> output == 0,
            denominator > 0 ==> 2 * ((denominator as nat) * (output as nat))
                <= 2 * (numerator as nat) + denominator as nat,
            denominator > 0 ==> 2 * (numerator as nat)
                < 2 * ((denominator as nat) * (output as nat)) + denominator as nat,
            numerator as nat <= 65535 * (denominator as nat) ==> output <= 65535,
    )
)]
pub(crate) const fn source_over_rounded_ratio(numerator: u64, denominator: u64) -> u128 {
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::spec_properties::source_over_rounding_is_nearest(numerator, denominator);
        if numerator as nat <= 65535 * (denominator as nat) {
            crate::proofs::spec_properties::source_over_rounding_preserves_sample_bound(numerator, denominator);
        }
    }
    if denominator == 0 {
        0
    } else {
        ((numerator as u128) + (denominator as u128) / 2) / (denominator as u128)
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part2_chroma_key_alpha(matches),)
)]
pub(crate) const fn chroma_key_alpha(matches: bool) -> u16 {
    if matches { 0 } else { u16::MAX }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires parent <= u32::MAX - child,
        ensures returns crate::specs::part2_metadata_count(parent, child),
    )
)]
pub(crate) const fn metadata_count(parent: u32, child: u32) -> u32 {
    parent + child
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::valid_jph_branding(
                primary_is_jph,
                minor_version as nat,
                compatible_brands as nat,
                sole_compatible_is_jph,
            )
        ),
    )
)]
pub(crate) const fn part15_jph_branding(
    primary_is_jph: bool,
    minor_version: u32,
    compatible_brands: usize,
    sole_compatible_is_jph: bool,
) -> bool {
    primary_is_jph && minor_version == 0 && compatible_brands == 1 && sole_compatible_is_jph
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part15_cap_payload_length(capabilities),
    )
)]
pub(crate) const fn part15_cap_payload_length(capabilities: u8) -> usize {
    4 + 2 * capabilities as usize
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part15_signaling_valid(
            rsiz_ht,
            pcap_ht,
            styles_ht,
            corresponding_profile,
        ),
    )
)]
pub(crate) const fn part15_signaling_valid(
    rsiz_ht: bool,
    pcap_ht: bool,
    styles_ht: bool,
    corresponding_profile: bool,
) -> bool {
    rsiz_ht == pcap_ht && (!styles_ht || pcap_ht) && (!corresponding_profile || pcap_ht)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::part15_codestream_class(has_legacy, has_ht, has_mixed) as u8
        ),
    )
)]
pub(crate) const fn part15_codestream_class(has_legacy: bool, has_ht: bool, has_mixed: bool) -> u8 {
    if has_mixed || (has_legacy && has_ht) {
        2
    } else if has_ht {
        1
    } else {
        0
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part15_mode_allows(
            mode as nat,
            has_legacy,
            has_mixed,
        ),
    )
)]
pub(crate) const fn part15_mode_allows(mode: u8, has_legacy: bool, has_mixed: bool) -> bool {
    matches!(mode, 0 | 2 | 3) && (mode != 0 || !has_legacy) && (mode != 2 || !has_mixed)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::part15_magnitude_bound(parameter as nat) as u8
        ),
    )
)]
pub(crate) const fn part15_magnitude_bound(parameter: u8) -> u8 {
    match parameter {
        0 => 8,
        1..=19 => parameter + 8,
        20..=30 => 4 * (parameter - 19) + 27,
        _ => 74,
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::part15_cleanup_u_bound(
                missing_msbs as nat, placeholder_bitplanes as nat, capability_bound as nat,
            ) as u8
        ),
    )
)]
pub(crate) const fn part15_cleanup_u_bound(
    missing_msbs: u16,
    placeholder_bitplanes: u16,
    capability_bound: u8,
) -> u8 {
    let bound = missing_msbs as u32 + placeholder_bitplanes as u32 + 2;
    // MagSgn precision includes one sign-coding bit beyond the coefficient bound.
    let capability_limit = if capability_bound < u8::MAX {
        capability_bound + 1
    } else {
        u8::MAX
    };
    if bound <= capability_limit as u32 {
        bound as u8
    } else {
        capability_limit
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part15_component_allowed(
            roi_allowed,
            has_roi,
            roi_shift as nat,
            irreversible_allowed,
            uses_ht,
            irreversible,
        ),
    )
)]
pub(crate) const fn part15_component_allowed(
    roi_allowed: bool,
    has_roi: bool,
    roi_shift: u8,
    irreversible_allowed: bool,
    uses_ht: bool,
    irreversible: bool,
) -> bool {
    (!has_roi || (roi_allowed && roi_shift <= 37))
        && (irreversible_allowed || !uses_ht || !irreversible)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part15_capability_fields_valid(
            mode as nat,
            reserved as nat,
            magnitude_parameter as nat,
        ),
    )
)]
pub(crate) const fn part15_capability_fields_valid(
    mode: u8,
    reserved: u8,
    magnitude_parameter: u8,
) -> bool {
    matches!(mode, 0 | 2 | 3) && reserved == 0 && magnitude_parameter < 32
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part15_headers_allowed(
            heterogeneous_allowed,
            has_functional_tile_header,
        ),
    )
)]
pub(crate) const fn part15_headers_allowed(
    heterogeneous_allowed: bool,
    has_functional_tile_header: bool,
) -> bool {
    heterogeneous_allowed || !has_functional_tile_header
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires passes > 0,
        ensures returns (
            crate::specs::ht_placeholder_passes(passes as nat) as u8
        ),
    )
)]
pub(crate) const fn ht_placeholder_passes(passes: u8) -> u8 {
    ((passes - 1) / 3) * 3
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires passes > 0,
        ensures returns (
            crate::specs::ht_substantive_passes(passes as nat) as u8
        ),
    )
)]
pub(crate) const fn ht_substantive_passes(passes: u8) -> u8 {
    passes - crate::refinement::ht_placeholder_passes(passes)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires phase < 3,
        ensures returns (
            crate::specs::ht_segment_passes(phase as nat, remaining as nat) as u32
        ),
    )
)]
pub(crate) const fn ht_segment_passes(phase: u32, remaining: u32) -> u32 {
    if phase == 0 {
        1
    } else if remaining < 3 - phase {
        remaining
    } else {
        3 - phase
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_mixed_selects_legacy(
            segment_length as nat,
            legacy_length as nat,
            cleanup_phase,
            lblock_gt_three,
            leading_zero,
        ),
    )
)]
pub(crate) const fn ht_mixed_selects_legacy(
    segment_length: u32,
    legacy_length: u32,
    cleanup_phase: bool,
    lblock_gt_three: bool,
    leading_zero: bool,
) -> bool {
    (segment_length == 0 && legacy_length != 0)
        || (segment_length != 0 && !(cleanup_phase && lblock_gt_three && leading_zero))
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::ht_scup(penultimate, last)
        ),
    )
)]
pub(crate) const fn ht_scup(penultimate: u8, last: u8) -> usize {
    last as usize * 16 + penultimate as usize % 16
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires scup <= cleanup_length,
        ensures returns (
            crate::specs::ht_magsgn_end(cleanup_length as nat, scup as nat) as usize
        ),
    )
)]
pub(crate) const fn ht_magsgn_end(cleanup_length: usize, scup: usize) -> usize {
    cleanup_length - scup
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::ht_forward_data_bits(stuffed) as u8
        ),
    )
)]
pub(crate) const fn ht_forward_data_bits(stuffed: bool) -> u8 {
    if stuffed { 7 } else { 8 }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_forward_stuffing_valid(stuffed, value),
    )
)]
pub(crate) const fn ht_forward_stuffing_valid(stuffed: bool, value: u8) -> bool {
    !stuffed || value < 128
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_mel_stuffing_valid(previous_ff, value as nat),
    )
)]
pub(crate) const fn ht_mel_stuffing_valid(previous_ff: bool, value: u8) -> bool {
    !previous_ff || value <= 0x8f
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::ht_reverse_data_bits(needs_unstuffing, value) as u8
        ),
    )
)]
pub(crate) const fn ht_reverse_data_bits(needs_unstuffing: bool, value: u8) -> u8 {
    if needs_unstuffing && value % 128 == 127 {
        7
    } else {
        8
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_cleanup_byte_claims_valid(
            mel_byte as nat,
            vlc_byte as nat,
            terminal_byte as nat,
            value,
        ),
    )
)]
pub(crate) const fn ht_cleanup_byte_claims_valid(
    mel_byte: u16,
    vlc_byte: u16,
    terminal_byte: u16,
    value: u8,
) -> bool {
    // A fused final data byte is shared; the Scup-bearing byte cannot be fused.
    mel_byte <= terminal_byte
        && vlc_byte <= terminal_byte
        && (mel_byte < vlc_byte
            || (mel_byte == vlc_byte && mel_byte < terminal_byte && value != 0xff))
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_noninitial_vlc_context(
            west_context, north_west, north, north_east,
        ),
    )
)]
pub(crate) const fn ht_noninitial_vlc_context(
    west_context: u8,
    north_west: u8,
    north: u8,
    north_east: u8,
) -> u8 {
    let west = if west_context & 2 != 0 { 2_u8 } else { 0 };
    let left = if north_west & 8 != 0 || north & 2 != 0 {
        1_u8
    } else {
        0
    };
    let right = if north & 8 != 0 || north_east & 2 != 0 {
        4_u8
    } else {
        0
    };
    west + left + right
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (crate::specs::ht_mel_exponent(state as nat) as u8),
    )
)]
pub(crate) const fn ht_mel_exponent(state: u8) -> u8 {
    match state {
        0..=2 => 0,
        3..=5 => 1,
        6..=8 => 2,
        9..=10 => 3,
        11 => 4,
        _ => 5,
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (crate::specs::ht_mel_next_state(state as nat, run) as u8),
    )
)]
pub(crate) const fn ht_mel_next_state(state: u8, run: bool) -> u8 {
    if run {
        if state < 12 { state + 1 } else { 12 }
    } else if state > 0 {
        state - 1
    } else {
        0
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::ht_vlc_table_index(context as nat, lookahead as nat) as usize
        ),
    )
)]
pub(crate) const fn ht_vlc_table_index(context: u8, lookahead: u8) -> usize {
    context as usize * 128 + lookahead as usize
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_vlc_word_valid(word as nat, length as nat),
    )
)]
pub(crate) const fn ht_vlc_word_valid(word: u16, length: u8) -> bool {
    length <= 7 && (word == 0 || length > 0)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::ht_uvlc_value(base as nat, suffix as nat, bias as nat) as u16
        ),
    )
)]
pub(crate) const fn ht_uvlc_value(base: u8, suffix: u8, bias: u8) -> u16 {
    base as u16 + suffix as u16 + bias as u16
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_quad_sample_in_bounds(
            x as nat,
            y as nat,
            sample as nat,
            width as nat,
            height as nat,
        ),
    )
)]
pub(crate) const fn ht_quad_sample_in_bounds(
    x: u16,
    y: u16,
    sample: u8,
    width: u16,
    height: u16,
) -> bool {
    sample < 4
        && x as u32 + (sample as u32 / 2) < width as u32
        && y as u32 + (sample as u32 % 2) < height as u32
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_quad_origin_valid(
            x as nat,
            y as nat,
            width as nat,
            height as nat,
        ),
    )
)]
pub(crate) const fn ht_quad_origin_valid(x: u16, y: u16, width: u16, height: u16) -> bool {
    x < width && y < height && x.is_multiple_of(2) && y.is_multiple_of(2)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires u_base <= u8::MAX - exponent,
        ensures returns (
            crate::specs::ht_kappa_completion(u_base as nat, gamma, exponent as nat) as u8
        ),
    )
)]
pub(crate) const fn ht_kappa_completion(u_base: u8, gamma: bool, exponent: u8) -> u8 {
    u_base
        + if gamma && exponent > 2 {
            exponent - 2
        } else {
            0
        }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (crate::specs::ht_magsgn_fill_value() as u8),
    )
)]
pub(crate) const fn ht_magsgn_fill_value() -> u8 {
    0xff
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires completed_u > 0,
        ensures returns (
            crate::specs::ht_magnitude_payload_bits(completed_u as nat, embedded) as u8
        ),
    )
)]
pub(crate) const fn ht_magnitude_payload_bits(completed_u: u8, embedded: bool) -> u8 {
    if embedded {
        completed_u - 1
    } else {
        completed_u
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_magnitude_sign(bit as nat, decoded),
    )
)]
pub(crate) const fn ht_magnitude_sign(bit: u8, decoded: bool) -> bool {
    bit == 0 && decoded
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::ht_bin_center_odd(value as nat) as u64
        ),
    )
)]
pub(crate) const fn ht_bin_center_odd(value: u64) -> u64 {
    if value.is_multiple_of(2) {
        value + 1
    } else {
        value
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_claim_available(already_claimed),
    )
)]
pub(crate) const fn ht_claim_available(already_claimed: bool) -> bool {
    !already_claimed
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires stripe_y <= height,
        ensures returns (
            crate::specs::ht_stripe_rows(height as nat, stripe_y as nat) as u16
        ),
    )
)]
pub(crate) const fn ht_stripe_rows(height: u16, stripe_y: u16) -> u16 {
    let remaining = height - stripe_y;
    if remaining < 4 { remaining } else { 4 }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires patch_x <= width,
        ensures returns (
            crate::specs::ht_patch_end(patch_x as nat, width as nat) as u16
        ),
    )
)]
pub(crate) const fn ht_patch_end(patch_x: u16, width: u16) -> u16 {
    if width - patch_x < 4 {
        width
    } else {
        patch_x + 4
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_sigprop_eligible(significant, neighbors as nat),
    )
)]
pub(crate) const fn ht_sigprop_eligible(significant: bool, neighbors: u16) -> bool {
    !significant && neighbors > 0
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_magref_eligible(significant_at_cleanup),
    )
)]
pub(crate) const fn ht_magref_eligible(significant_at_cleanup: bool) -> bool {
    significant_at_cleanup
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires existing <= u8::MAX - additional,
        ensures returns (
            crate::specs::ht_refinement_pass_total(existing as nat, additional as nat) as u8
        ),
    )
)]
pub(crate) const fn ht_refinement_pass_total(existing: u8, additional: u8) -> u8 {
    existing + additional
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_multiple_set_allowed(
            sets_seen as nat,
            multiple_allowed,
        ),
    )
)]
pub(crate) const fn ht_multiple_set_allowed(sets_seen: u16, multiple_allowed: bool) -> bool {
    sets_seen == 0 || multiple_allowed
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::ht_replacement_allowed(
            phase as nat,
            has_cleanup,
            empty_segment,
        ),
    )
)]
pub(crate) const fn ht_replacement_allowed(
    phase: u32,
    has_cleanup: bool,
    empty_segment: bool,
) -> bool {
    (phase == 0 && empty_segment) || (phase != 0 && has_cleanup)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part9_header_form_valid(form as nat),)
)]
pub(crate) const fn jpip_header_form_valid(form: u8) -> bool {
    form >= 1 && form <= 3
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part9_header_has_explicit_class(form as nat),)
)]
pub(crate) const fn jpip_header_has_explicit_class(form: u8) -> bool {
    form >= 2
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part9_header_has_explicit_codestream(form as nat),)
)]
pub(crate) const fn jpip_header_has_explicit_codestream(form: u8) -> bool {
    form == 3
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part9_vbas_first_is_canonical(
            payload as nat,
            continued,
        ),
    )
)]
pub(crate) const fn jpip_vbas_first_is_canonical(payload: u8, continued: bool) -> bool {
    !continued || payload > 0
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires value <= (u64::MAX - group as u64) / 128,
        ensures returns (
            crate::specs::part9_vbas_step(value as nat, group as nat) as u64
        ),
    )
)]
pub(crate) const fn jpip_vbas_step(value: u64, group: u8) -> u64 {
    value * 128 + group as u64
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part9_auxiliary_present(class_identifier as nat),
    )
)]
pub(crate) const fn jpip_auxiliary_present(class_identifier: u64) -> bool {
    class_identifier % 2 == 1
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::part9_eor_extent(header_length as nat, body_length as nat) as u128
        ),
    )
)]
pub(crate) const fn jpip_eor_extent(header_length: usize, body_length: usize) -> u128 {
    header_length as u128 + body_length as u128
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part9_class_identifier_valid(class_identifier as nat),
    )
)]
pub(crate) const fn jpip_class_identifier_valid(class_identifier: u64) -> bool {
    class_identifier <= 8
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part9_databin_identity(
            first_class as nat,
            first_codestream as nat,
            first_in_class as nat,
            second_class as nat,
            second_codestream as nat,
            second_in_class as nat,
        ),
    )
)]
pub(crate) const fn jpip_databin_identity(
    first_class: u64,
    first_codestream: u64,
    first_in_class: u64,
    second_class: u64,
    second_codestream: u64,
    second_in_class: u64,
) -> bool {
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::valid_jpip_fragment(
            offset as nat,
            length as nat,
            limit as nat,
        ),
    )
)]
pub(crate) const fn jpip_fragment_valid(offset: u64, length: u64, limit: u64) -> bool {
    offset <= limit && length <= limit - offset
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::consistent_jpip_final_length(
            existing as nat,
            announced as nat,
        ),
    )
)]
pub(crate) const fn jpip_final_length_consistent(existing: u64, announced: u64) -> bool {
    existing == announced
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part9_overlap_consistent(bytes_equal),)
)]
pub(crate) const fn jpip_overlap_consistent(bytes_equal: bool) -> bool {
    bytes_equal
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part9_transaction_commit(validation_complete),)
)]
pub(crate) const fn jpip_transaction_commit(validation_complete: bool) -> bool {
    validation_complete
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::part9_merge_start(first as nat, second as nat) as u64
        ),
    )
)]
pub(crate) const fn jpip_merge_start(first: u64, second: u64) -> u64 {
    if first < second { first } else { second }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::part9_merge_end(first as nat, second as nat) as u64
        ),
    )
)]
pub(crate) const fn jpip_merge_end(first: u64, second: u64) -> u64 {
    if first > second { first } else { second }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::complete_jpip_databin(
            contiguous_prefix as nat,
            final_length as nat,
        ),
    )
)]
pub(crate) const fn jpip_databin_complete(contiguous_prefix: u64, final_length: u64) -> bool {
    contiguous_prefix == final_length
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires available_end <= required_end,
        ensures returns (
            crate::specs::part9_missing_length(required_end as nat, available_end as nat) as u64
        ),
    )
)]
pub(crate) const fn jpip_missing_length(required_end: u64, available_end: u64) -> u64 {
    required_end - available_end
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires missing <= required,
        ensures returns (
            crate::specs::part9_available_bytes(required as nat, missing as nat) as u64
        ),
    )
)]
pub(crate) const fn jpip_available_bytes(required: u64, missing: u64) -> u64 {
    required - missing
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::complete_jpip_range(
            available as nat,
            required as nat,
        ),
    )
)]
pub(crate) const fn jpip_range_complete(available: u64, required: u64) -> bool {
    available == required
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires
            product as nat == (_component_precinct as nat) * (_tile_count as nat),
            tile <= u64::MAX - product,
        ensures returns (
            crate::specs::part9_precinct_flat_identity(
                tile as nat,
                _tile_count as nat,
                _component_precinct as nat,
            ) as u64
        ),
    )
)]
pub(crate) const fn jpip_precinct_in_class(
    tile: u64,
    _tile_count: u64,
    _component_precinct: u64,
    product: u64,
) -> u64 {
    tile + product
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::jpip_quality_prefix_available(
            available as nat,
            requested as nat,
        ),
    )
)]
pub(crate) const fn jpip_quality_prefix_available(available: u16, requested: u16) -> bool {
    requested <= available
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part9_metadata_descent_allowed(
            depth as nat,
            maximum_depth as nat,
            cycle,
        ),
    )
)]
pub(crate) const fn jpip_metadata_descent_allowed(
    depth: u32,
    maximum_depth: u32,
    cycle: bool,
) -> bool {
    depth <= maximum_depth && !cycle
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::part9_reconstruction_length(
                main_header as nat,
                tile_headers as nat,
                packet_data as nat,
                eoc as nat,
            ) as u128
        ),
    )
)]
pub(crate) const fn jpip_reconstruction_length(
    main_header: u64,
    tile_headers: u64,
    packet_data: u64,
    eoc: u64,
) -> u128 {
    main_header as u128 + tile_headers as u128 + packet_data as u128 + eoc as u128
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::jpip_reconstructed_box_length(
            header as nat,
            payload as nat,
        ) as u128,
    )
)]
pub(crate) const fn jpip_box_length(header: u64, payload: u64) -> u128 {
    header as u128 + payload as u128
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part9_reframed_marker_removed(
            marker as nat,
            tile_header,
        ),
    )
)]
pub(crate) const fn jpip_reframed_marker_removed(marker: u16, tile_header: bool) -> bool {
    if tile_header {
        marker == 0xff58 || marker == 0xff61
    } else {
        marker == 0xff55 || marker == 0xff57 || marker == 0xff60
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part9_progression_supported(order as nat),)
)]
pub(crate) const fn jpip_progression_supported(order: u8) -> bool {
    order <= 4
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires payload <= usize::MAX - 8,
        ensures returns (
            crate::specs::part9_packet_reframed_length(payload as nat, sop, eph) as usize
        ),
    )
)]
pub(crate) const fn jpip_packet_reframed_length(payload: usize, sop: bool, eph: bool) -> usize {
    payload + if sop { 6 } else { 0 } + if eph { 2 } else { 0 }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::jpip_tile_transition_sufficient(
            later_functional,
            preceding_packet_count_known,
            preceding_exact_consumption_known,
        ),
    )
)]
pub(crate) const fn jpip_tile_transition_sufficient(
    later_functional: bool,
    preceding_packet_count_known: bool,
    preceding_exact_consumption_known: bool,
) -> bool {
    !later_functional || preceding_packet_count_known || preceding_exact_consumption_known
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::valid_jpip_progressive_update(
            previous_layers as nat,
            available_layers as nat,
            planned_layers as nat,
        ),
    )
)]
pub(crate) const fn jpip_progressive_update_valid(
    previous_layers: u16,
    available_layers: u16,
    planned_layers: u16,
) -> bool {
    previous_layers <= available_layers && available_layers <= planned_layers
}
