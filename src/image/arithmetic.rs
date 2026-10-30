use super::FIXED_BITS;
use crate::{DecodeError, MalformedKind, Resource, ResourceLimits};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_fixed_product(value, factor),
))]
pub(super) fn fixed_mul(value: i64, factor: i64) -> Result<i64, DecodeError> {
    let bias = 1_i64 << 15;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert((1i64 << 15) == 32768) by(bit_vector);
        assert(FIXED_BITS == 16) by(compute);
    }
    let product = value.checked_mul(factor).ok_or_else(arithmetic_overflow)?;
    let biased = product.checked_add(bias).ok_or_else(arithmetic_overflow)?;
    Ok(biased >> FIXED_BITS)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_decoded_signed_value(value, fraction_bits, minimum, maximum),
))]
pub(super) fn decoded_signed_sample(
    value: i64,
    fraction_bits: u8,
    minimum: i64,
    maximum: i64,
) -> Result<i64, DecodeError> {
    Ok(round_fixed(value, fraction_bits)?.max(minimum).min(maximum))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_decoded_unsigned_value(value, fraction_bits, level_shift, maximum),
))]
pub(super) fn decoded_unsigned_sample(
    value: i64,
    fraction_bits: u8,
    level_shift: i64,
    maximum: u64,
) -> Result<u64, DecodeError> {
    let shifted = round_fixed(value, fraction_bits)?
        .checked_add(level_shift)
        .ok_or_else(arithmetic_overflow)?;
    Ok(u64::try_from(shifted.max(0))
        .map_err(|_| arithmetic_overflow())?
        .min(maximum))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_round_fixed(value, fraction_bits),
))]
fn round_fixed(value: i64, fraction_bits: u8) -> Result<i64, DecodeError> {
    if fraction_bits == 0 {
        return Ok(value);
    }
    if fraction_bits > 62 {
        return Err(invalid_image());
    }
    Ok(value
        .checked_add(1_i64 << u32::from(fraction_bits - 1))
        .ok_or_else(arithmetic_overflow)?
        >> u32::from(fraction_bits))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_ceil_div(value, divisor),
))]
pub(super) fn ceil_div(value: u32, divisor: u32) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(invalid_image());
    }

    Ok(crate::refinement::ceil_div_u32(value, divisor))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_assembly_registered_ceil(value, divisor, registration),
))]
pub(super) fn registered_ceil_div(
    value: u32,
    divisor: u32,
    registration: u16,
) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(invalid_image());
    }
    let scale = 65_536_u64;
    let numerator = u64::from(value)
        .checked_mul(scale)
        .ok_or_else(arithmetic_overflow)?;
    if numerator <= u64::from(registration) {
        return Ok(0);
    }
    let adjusted = numerator
        .checked_sub(u64::from(registration))
        .ok_or_else(arithmetic_overflow)?;
    let denominator = u64::from(divisor)
        .checked_mul(scale)
        .ok_or_else(arithmetic_overflow)?;
    let rounded = crate::refinement::registered_extent(adjusted, denominator);
    match u32::try_from(rounded) {
        Ok(value) => Ok(value),
        Err(_) => Err(arithmetic_overflow()),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_area(width, height),
))]
pub(super) fn checked_area(width: u32, height: u32) -> Result<usize, DecodeError> {
    crate::transform::checked_area(width, height)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_offset(y, width, x),
))]
pub(super) fn index(width: u32, x: u32, y: u32) -> Result<usize, DecodeError> {
    crate::transform::transform_offset(y, width, x)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_byte_budget(
        samples as nat, bytes_per_sample, crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
pub(super) fn validate_bytes(
    samples: usize,
    bytes_per_sample: u64,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let bytes = (samples as u64)
        .checked_mul(bytes_per_sample)
        .ok_or_else(arithmetic_overflow)?;
    if bytes > limits.maximum_decoded_bytes() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::DecodedBytes,
            limit: limits.maximum_decoded_bytes(),
            actual: bytes,
        });
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_error(false),
))]
pub(super) fn invalid_image() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidImageGeometry,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_error(false),
))]
pub(super) fn invalid_transform() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidTransform,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_error(true),
))]
pub(super) fn arithmetic_overflow() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}
