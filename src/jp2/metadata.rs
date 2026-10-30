//! Bounded nested metadata inspection and resolution metadata parsing.

use super::{
    BoxReader, BoxRef, BoxType, DecodeError, MalformedKind, Resolution, Resource, ResourceLimits,
    add_offset, read_u16, rebase_error,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(decreases remaining_depth,))]
pub(super) fn inspect_metadata_box(
    box_ref: BoxRef<'_>,
    limits: &ResourceLimits,
    depth: u32,
    remaining_depth: u32,
) -> Result<(u32, u32), DecodeError> {
    if depth > limits.maximum_box_depth() {
        return Err(DecodeError::limit(
            Resource::BoxDepth,
            u64::from(limits.maximum_box_depth()),
            u64::from(depth),
        ));
    }
    if !box_ref.header.box_type().equals(BoxType::ASSOCIATION) {
        return Ok((1, depth));
    }
    let child_depth = depth
        .checked_add(1)
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let mut reader = BoxReader::new(box_ref.payload, limits)?;
    let mut count = 1_u32;
    let mut maximum_depth = depth;
    let mut remaining_boxes = box_ref.payload.len();
    #[cfg_attr(verus_keep_ghost, verus_spec(decreases remaining_boxes,))]
    loop {
        let Some(next) = reader.next_box_ref() else {
            break;
        };
        if remaining_boxes == 0 {
            return Err(DecodeError::malformed(
                box_ref.header.offset(),
                MalformedKind::InvalidBoxLength,
            ));
        }
        remaining_boxes -= 1;
        let child = next?;
        let child_remaining = remaining_depth
            .checked_sub(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let (child_count, observed_depth) =
            inspect_metadata_box(child, limits, child_depth, child_remaining)?;
        if count > u32::MAX - child_count {
            return Err(DecodeError::malformed(0, MalformedKind::ArithmeticOverflow));
        }
        count = crate::refinement::metadata_count(count, child_count);
        if u64::from(count) > limits.maximum_work_units() {
            return Err(DecodeError::limit(
                Resource::WorkUnits,
                limits.maximum_work_units(),
                u64::from(count),
            ));
        }
        maximum_depth = maximum_depth.max(observed_depth);
    }
    Ok((count, maximum_depth))
}

pub(super) fn copy_bytes(input: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= input.len(),
            decreases input.len() - index,
        )
    )]
    while index < input.len() {
        output.push(
            *input
                .get(index)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        );
        index = index
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }
    Ok(output)
}

pub(super) fn parse_resolution_superbox(
    payload: &[u8],
    limits: &ResourceLimits,
    base_offset: u64,
) -> Result<(Option<Resolution>, Option<Resolution>), DecodeError> {
    let mut reader = BoxReader::new(payload, limits)?;
    let mut capture = None;
    let mut display = None;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases payload.len() as int - reader.position as int,)
    )]
    loop {
        let previous_position = reader.position;
        let Some(next) = reader.next_box_ref() else {
            break;
        };
        if reader.position <= previous_position || reader.position > payload.len() {
            return Err(DecodeError::malformed(
                add_offset(base_offset, previous_position as u64)?,
                MalformedKind::InvalidBoxLength,
            ));
        }
        let box_ref = next.map_err(|error| rebase_error(error, base_offset))?;
        let box_type = box_ref.header.box_type();
        if box_type.equals(BoxType::CAPTURE_RESOLUTION) {
            if capture.is_some() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            capture = Some(parse_resolution(box_ref, base_offset)?);
        } else if box_type.equals(BoxType::DISPLAY_RESOLUTION) {
            if display.is_some() {
                return Err(DecodeError::malformed(
                    add_offset(base_offset, box_ref.header.offset())?,
                    MalformedKind::InvalidBoxOrder,
                ));
            }
            display = Some(parse_resolution(box_ref, base_offset)?);
        }
    }
    if capture.is_none() && display.is_none() {
        return Err(DecodeError::malformed(
            base_offset,
            MalformedKind::MissingRequiredStructure,
        ));
    }
    Ok((capture, display))
}

fn parse_resolution(box_ref: BoxRef<'_>, base_offset: u64) -> Result<Resolution, DecodeError> {
    if box_ref.payload.len() != 10 {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxLength,
        ));
    }
    let vertical_numerator = read_u16([box_ref.payload[0], box_ref.payload[1]]);
    let vertical_denominator = read_u16([box_ref.payload[2], box_ref.payload[3]]);
    let horizontal_numerator = read_u16([box_ref.payload[4], box_ref.payload[5]]);
    let horizontal_denominator = read_u16([box_ref.payload[6], box_ref.payload[7]]);
    if vertical_numerator == 0
        || vertical_denominator == 0
        || horizontal_numerator == 0
        || horizontal_denominator == 0
    {
        return Err(DecodeError::malformed(
            add_offset(base_offset, box_ref.header.offset())?,
            MalformedKind::InvalidBoxContents,
        ));
    }
    Ok(Resolution {
        vertical_numerator,
        vertical_denominator,
        horizontal_numerator,
        horizontal_denominator,
        vertical_exponent: box_ref.payload[8] as i8,
        horizontal_exponent: box_ref.payload[9] as i8,
    })
}
