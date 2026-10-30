#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

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
