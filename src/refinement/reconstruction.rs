#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

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
        requires
            level >= 0,
            value <= i64::MAX - level,
        ensures returns crate::specs::part2_signed_to_nominal(value, level),
    )
)]
pub(crate) const fn signed_to_nominal(value: i64, level: i64) -> i64 {
    value + level
}
