//! Checked geometry, sample copying, resource charges, and errors.
#[cfg(verus_keep_ghost)]
use super::layer_owned_alpha;
use super::{DecodeError, MalformedKind, OwnedLayer, Resource};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::part2_native_registered_extent(samples, subsampling, grid_size, offset),
    )
)]
pub(super) fn registered_extent(
    samples: u32,
    subsampling: u32,
    grid_size: u32,
    offset: u32,
) -> Result<u32, DecodeError> {
    if grid_size == 0 {
        return Err(invalid_layer());
    }
    let product = u64::from(samples)
        .checked_mul(u64::from(subsampling))
        .ok_or_else(arithmetic_overflow)?;
    let numerator = product
        .checked_add(u64::from(offset))
        .ok_or_else(arithmetic_overflow)?;
    let grid = u64::from(grid_size);
    if numerator > u64::MAX - (grid - 1) {
        return Err(arithmetic_overflow());
    }
    match u32::try_from(crate::refinement::registered_extent(numerator, grid)) {
        Ok(extent) => Ok(extent),
        Err(_) => Err(arithmetic_overflow()),
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures (result, *final(decoded_bytes) as nat) == crate::specs::part2_charge_layer_bytes(
            (layer.width, layer.height, layer.color_space, crate::specs::part2_composition_planes(layer.colors@), layer_owned_alpha(layer.alpha)),
            *old(decoded_bytes) as nat, maximum_decoded_bytes),
    )
)]
pub(super) fn charge_layer_bytes(
    layer: &OwnedLayer,
    decoded_bytes: &mut u64,
    maximum_decoded_bytes: u64,
) -> Result<(), DecodeError> {
    let layer_bytes = layer_decoded_bytes(layer)?;
    *decoded_bytes = decoded_bytes
        .checked_add(layer_bytes)
        .ok_or_else(arithmetic_overflow)?;
    enforce_limit(
        Resource::DecodedBytes,
        maximum_decoded_bytes,
        *decoded_bytes,
    )
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures match result {
            Ok(bytes) => crate::specs::part2_render_layer_bytes(
                (layer.width, layer.height, layer.color_space, crate::specs::part2_composition_planes(layer.colors@), layer_owned_alpha(layer.alpha)))
                == Ok(bytes as nat),
            Err(error) => crate::specs::part2_render_layer_bytes(
                (layer.width, layer.height, layer.color_space, crate::specs::part2_composition_planes(layer.colors@), layer_owned_alpha(layer.alpha)))
                == Err(error),
        },
    )
)]
fn layer_decoded_bytes(layer: &OwnedLayer) -> Result<u64, DecodeError> {
    let channels = match (layer.colors.len() as u64).checked_add(1) {
        Some(channels) => channels,
        None => return Err(arithmetic_overflow()),
    };
    let bytes_per_pixel = match channels.checked_mul(2) {
        Some(bytes) => bytes,
        None => return Err(arithmetic_overflow()),
    };
    let pixels = checked_area(layer.width, layer.height)?;
    match (pixels as u64).checked_mul(bytes_per_pixel) {
        Some(bytes) => Ok(bytes),
        None => Err(arithmetic_overflow()),
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(index =>
        ensures index.ok() == crate::specs::part2_composition_pixel_index(width, x, y),
            index.is_err() ==> index.err().unwrap() == crate::specs::part2_composition_error(true),
    )
)]
pub(super) fn index_of(width: u32, x: u32, y: u32) -> Result<usize, DecodeError> {
    crate::refinement::composition_pixel_index(width, x, y).ok_or_else(arithmetic_overflow)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(area =>
        ensures area == crate::specs::part2_layer_area(width, height),
    )
)]
pub(super) fn checked_area(width: u32, height: u32) -> Result<usize, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part2_layer_area);
        vstd::arithmetic::mul::lemma_mul_inequality(width as int, 4294967295, height as int);
        vstd::arithmetic::mul::lemma_mul_inequality(height as int, 4294967295, 4294967295);
        assert((width as int) * (height as int) <= 18446744065119617025) by(nonlinear_arith);
    }
    let area = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(arithmetic_overflow)?;
    match usize::try_from(area) {
        Ok(value) => Ok(value),
        Err(_) => Err(arithmetic_overflow()),
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(output =>
        ensures output.is_ok(), output.ok().unwrap()@ == input@,
    )
)]
pub(super) fn copy_samples(input: &[u16]) -> Result<Vec<u16>, DecodeError> {
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= input.len(), output@ == input@.take(index as int),
            decreases input.len() - index,
        )
    )]
    while index < input.len() {
        output.push(*input.get(index).ok_or_else(arithmetic_overflow)?);
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(output)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == if actual > limit {
            Err(DecodeError::LimitExceeded { resource, limit, actual })
        } else { Ok(()) },
    )
)]
pub(super) fn enforce_limit(
    resource: Resource,
    limit: u64,
    actual: u64,
) -> Result<(), DecodeError> {
    if actual > limit {
        Err(DecodeError::LimitExceeded {
            resource,
            limit,
            actual,
        })
    } else {
        Ok(())
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(error => ensures error == crate::specs::part2_composition_error(true),)
)]
pub(super) fn arithmetic_overflow() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(error => ensures error == crate::specs::part2_composition_error(false),)
)]
pub(super) fn invalid_layer() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidBoxContents,
    }
}
