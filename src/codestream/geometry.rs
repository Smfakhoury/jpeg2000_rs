//! Image, tile and registered component geometry.

use super::{
    ComponentInfo, DecodeError, ImageSize, MalformedKind, TileBounds, TileComponentBounds,
};
#[cfg(verus_keep_ghost)]
use super::{image_size_component_count, image_size_component_infos, selection_geometry_view};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_size_component(size.components, size.component_info@, index),
    image_size_component_count(size) == size.components,
    image_size_component_infos(size) == size.component_info@,
))]
pub(crate) fn image_component_info(
    size: &ImageSize,
    index: u16,
) -> Result<ComponentInfo, DecodeError> {
    if index >= size.components {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidImageGeometry,
        });
    }
    let index = usize::from(index);
    if index >= size.component_info.len() {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidImageGeometry,
        });
    }
    Ok(size.component_info[index])
}
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_error(true),
))]
pub(super) fn selection_geometry_arithmetic() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_error(false),
))]
pub(super) fn selection_geometry_invalid() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidImageGeometry,
    }
}
impl ImageSize {
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_selection_tile_count(selection_geometry_view(&self)),
    ))]
    pub fn tile_count(self) -> Result<u64, DecodeError> {
        u64::from(self.tiles_across)
            .checked_mul(u64::from(self.tiles_down))
            .ok_or_else(selection_geometry_arithmetic)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_image_size_component(
            image_size_component_count(self), image_size_component_infos(self), index,
        ),
    ))]
    pub fn component(&self, index: u16) -> Result<ComponentInfo, DecodeError> {
        image_component_info(self, index)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_selection_tile_bounds(selection_geometry_view(self), tile_index),
    ))]
    pub fn tile_bounds(&self, tile_index: u16) -> Result<TileBounds, DecodeError> {
        if u64::from(tile_index) >= self.tile_count()? || self.tiles_across == 0 {
            return Err(selection_geometry_invalid());
        }
        let column = u32::from(tile_index) % self.tiles_across;
        let row = u32::from(tile_index) / self.tiles_across;
        let offset_x = column
            .checked_mul(self.tile_width)
            .ok_or_else(selection_geometry_arithmetic)?;
        let nominal_x0 = self
            .tile_x_origin
            .checked_add(offset_x)
            .ok_or_else(selection_geometry_arithmetic)?;
        let offset_y = row
            .checked_mul(self.tile_height)
            .ok_or_else(selection_geometry_arithmetic)?;
        let nominal_y0 = self
            .tile_y_origin
            .checked_add(offset_y)
            .ok_or_else(selection_geometry_arithmetic)?;
        let image_x1 = self
            .x_origin
            .checked_add(self.width)
            .ok_or_else(selection_geometry_arithmetic)?;
        let image_y1 = self
            .y_origin
            .checked_add(self.height)
            .ok_or_else(selection_geometry_arithmetic)?;
        let x0 = crate::refinement::tile_clip_start(nominal_x0, self.x_origin);
        let y0 = crate::refinement::tile_clip_start(nominal_y0, self.y_origin);
        let nominal_x1 = nominal_x0
            .checked_add(self.tile_width)
            .ok_or_else(selection_geometry_arithmetic)?;
        let nominal_y1 = nominal_y0
            .checked_add(self.tile_height)
            .ok_or_else(selection_geometry_arithmetic)?;
        let x1 = crate::refinement::tile_clip_end(nominal_x1, image_x1);
        let y1 = crate::refinement::tile_clip_end(nominal_y1, image_y1);
        if x0 >= x1 || y0 >= y1 {
            return Err(selection_geometry_invalid());
        }
        Ok(TileBounds {
            tile_index,
            x0,
            y0,
            x1,
            y1,
        })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_tile_component_bounds(
            selection_geometry_view(self), image_size_component_count(self),
            image_size_component_infos(self), tile_index, component,
        ),
    ))]
    pub fn tile_component_bounds(
        &self,
        tile_index: u16,
        component: u16,
    ) -> Result<TileComponentBounds, DecodeError> {
        let tile = self.tile_bounds(tile_index)?;
        let info = self.component(component)?;
        let x_subsampling = u32::from(info.x_subsampling);
        let y_subsampling = u32::from(info.y_subsampling);
        let component_image_x0 =
            registered_ceil_div(self.x_origin, x_subsampling, info.x_registration)?;
        let component_image_y0 =
            registered_ceil_div(self.y_origin, y_subsampling, info.y_registration)?;
        let x0 = registered_ceil_div(tile.x0, x_subsampling, info.x_registration)?;
        let y0 = registered_ceil_div(tile.y0, y_subsampling, info.y_registration)?;
        let x1 = registered_ceil_div(tile.x1, x_subsampling, info.x_registration)?;
        let y1 = registered_ceil_div(tile.y1, y_subsampling, info.y_registration)?;
        if x0 >= x1 || y0 >= y1 {
            return Err(selection_geometry_invalid());
        }
        Ok(TileComponentBounds {
            tile_index,
            component,
            x0,
            y0,
            x1,
            y1,
            image_x: x0
                .checked_sub(component_image_x0)
                .ok_or_else(selection_geometry_invalid)?,
            image_y: y0
                .checked_sub(component_image_y0)
                .ok_or_else(selection_geometry_invalid)?,
        })
    }
}
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_ceil_div(value, divisor),
))]
pub(super) fn ceil_div(value: u32, divisor: u32) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidImageGeometry,
        });
    }

    Ok(crate::refinement::ceil_div_u32(value, divisor))
}
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_registered_component_ceil(value, divisor, registration),
))]
pub(crate) fn registered_ceil_div(
    value: u32,
    divisor: u32,
    registration: u16,
) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(selection_geometry_invalid());
    }
    let scale = 65_536_u64;
    let numerator = u64::from(value)
        .checked_mul(scale)
        .ok_or_else(selection_geometry_arithmetic)?;
    if numerator <= u64::from(registration) {
        return Ok(0);
    }
    let adjusted = numerator
        .checked_sub(u64::from(registration))
        .ok_or_else(selection_geometry_arithmetic)?;
    let denominator = u64::from(divisor)
        .checked_mul(scale)
        .ok_or_else(selection_geometry_arithmetic)?;
    let quotient = adjusted / denominator;
    let increment = if adjusted.is_multiple_of(denominator) {
        0_u64
    } else {
        1_u64
    };
    let rounded = quotient
        .checked_add(increment)
        .ok_or_else(selection_geometry_arithmetic)?;
    match u32::try_from(rounded) {
        Ok(value) => Ok(value),
        Err(_) => Err(selection_geometry_arithmetic()),
    }
}
