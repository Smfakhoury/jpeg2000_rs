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
