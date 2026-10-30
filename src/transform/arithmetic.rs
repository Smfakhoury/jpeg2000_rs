use crate::{DecodeError, MalformedKind, Resource, ResourceLimits};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_dimension(value),
))]
pub(super) fn transform_dimension(value: u32) -> Result<usize, DecodeError> {
    let index = value as usize;
    if index as u64 != value as u64 {
        return Err(arithmetic_overflow());
    }
    Ok(index)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_area(width, height),
))]
pub(crate) fn checked_area(width: u32, height: u32) -> Result<usize, DecodeError> {
    transform_dimension(width)?
        .checked_mul(transform_dimension(height)?)
        .ok_or_else(arithmetic_overflow)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_quantization_budget(
        samples as nat, crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
pub(super) fn validate_output_size(
    samples: usize,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    validate_quantization_size(samples, limits.maximum_decoded_bytes())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_quantization_budget(samples as nat, maximum),
))]
pub(super) fn validate_quantization_size(samples: usize, maximum: u64) -> Result<(), DecodeError> {
    let bytes = (samples as u64)
        .checked_mul(8)
        .ok_or_else(arithmetic_overflow)?;
    if bytes > maximum {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::DecodedBytes,
            limit: maximum,
            actual: bytes,
        });
    }
    Ok(())
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
    ensures result == crate::specs::part1_transform_error(true),
))]
pub(super) fn arithmetic_overflow() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}
