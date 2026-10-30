use super::{DecodeError, MalformedKind, WaveletTransform, codestream};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(error =>
        ensures error == crate::specs::part1_block_placement_error(false),
            error == crate::specs::color_selection_target_error(),
    )
)]
pub(super) fn invalid_decode() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidCodeBlock,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_decode_wavelet_agreement(left, right),
))]
pub(super) const fn wavelet_transforms_equal(
    left: WaveletTransform,
    right: WaveletTransform,
) -> bool {
    matches!(
        (left, right),
        (
            WaveletTransform::Irreversible9x7,
            WaveletTransform::Irreversible9x7
        ) | (
            WaveletTransform::Reversible5x3,
            WaveletTransform::Reversible5x3
        )
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_block_placement_error(true),
))]
pub(super) fn arithmetic_overflow() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_offset(y, width, x),
))]
pub(super) fn checked_index(width: u32, x: u32, y: u32) -> Result<usize, DecodeError> {
    crate::transform::transform_offset(y, width, x)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_area(width, height),
))]
pub(super) fn checked_area_u32(width: u32, height: u32) -> Result<usize, DecodeError> {
    crate::transform::checked_area(width, height)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_selection_ceil(value, divisor),
))]
pub(super) fn ceil_div_u32(value: u32, divisor: u32) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(invalid_decode());
    }

    let remainder = value.checked_rem(divisor).ok_or_else(arithmetic_overflow)?;
    (value / divisor)
        .checked_add(if remainder != 0 { 1 } else { 0 })
        .ok_or_else(arithmetic_overflow)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_registered_decode_ceil(value, divisor, registration),
))]
pub(crate) fn registered_ceil_div_u32(
    value: u32,
    divisor: u32,
    registration: u16,
) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(invalid_decode());
    }
    codestream::registered_ceil_div(value, divisor, registration)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_selection_floor(value, divisor),
))]
pub(super) fn floor_div_u32(value: u32, divisor: u32) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(invalid_decode());
    }
    Ok(value / divisor)
}
