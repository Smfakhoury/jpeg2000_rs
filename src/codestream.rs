use crate::{
    DecodeError, MalformedKind, Resource, ResourceLimits, tier1::Tier1BlockCoding,
    tier1::Tier1CodingStyle, transform::QuantizationStep,
};

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

const MAX_QUANTIZATION_STEPS: usize = 97;
const MAX_COMPONENTS: usize = 64;

const SOC: u16 = 0xff4f;
const SIZ: u16 = 0xff51;
const CAP: u16 = 0xff50;
const COD: u16 = 0xff52;
const COC: u16 = 0xff53;
const TLM: u16 = 0xff55;
const PRF: u16 = 0xff56;
const PLM: u16 = 0xff57;
const PLT: u16 = 0xff58;
const PPM: u16 = 0xff60;
const PPT: u16 = 0xff61;
const POC: u16 = 0xff5f;
const CRG: u16 = 0xff63;
const COM: u16 = 0xff64;
const QCD: u16 = 0xff5c;
const QCC: u16 = 0xff5d;
const RGN: u16 = 0xff5e;
const MCT: u16 = 0xff74;
const MCC: u16 = 0xff75;
const DFS: u16 = 0xff72;
const ADS: u16 = 0xff73;
const NLT: u16 = 0xff76;
const MCO: u16 = 0xff77;
const CBD: u16 = 0xff78;
const ATK: u16 = 0xff79;
const SOT: u16 = 0xff90;
const SOD: u16 = 0xff93;
const EOC: u16 = 0xffd9;
const CPF: u16 = 0xff59;
const HT_RSIZ_FLAG: u16 = 0x4000;
const PART_15_CAPABILITY_FLAG: u32 = 0x0002_0000;
pub const MAX_ROI_SHIFT: u8 = u8::MAX;
pub(crate) const MCT_FIXED_BITS: u8 = 32;

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
    pub x_origin: u32,
    pub y_origin: u32,
    pub components: u16,
    pub uniform_bit_depth: Option<u8>,
    pub uniform_signed: Option<bool>,
    pub tile_width: u32,
    pub tile_height: u32,
    pub tile_x_origin: u32,
    pub tile_y_origin: u32,
    pub tiles_across: u32,
    pub tiles_down: u32,
    pub(crate) component_info: [ComponentInfo; MAX_COMPONENTS],
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ComponentInfo {
    pub precision: u8,
    pub signed: bool,
    pub x_subsampling: u8,
    pub y_subsampling: u8,
    pub x_registration: u16,
    pub y_registration: u16,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn image_size_component_infos(size: &ImageSize) -> Seq<ComponentInfo> {
        size.component_info@
    }

    pub closed spec fn image_size_component_count(size: &ImageSize) -> u16 {
        size.components
    }

    pub closed spec fn decomposition_selector_view(style: CodingStyle) -> (u8, Seq<DecompositionDirection>) {
        (style.decomposition_levels, style.decomposition_directions@)
    }
}

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

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct TileBounds {
    pub tile_index: u16,
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct TileComponentBounds {
    pub tile_index: u16,
    pub component: u16,
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
    pub image_x: u32,
    pub image_y: u32,
}

impl ImageSize {
    pub fn tile_count(self) -> Result<u64, DecodeError> {
        u64::from(self.tiles_across)
            .checked_mul(u64::from(self.tiles_down))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_image_size_component(
            image_size_component_count(self), image_size_component_infos(self), index,
        ),
    ))]
    pub fn component(&self, index: u16) -> Result<ComponentInfo, DecodeError> {
        image_component_info(self, index)
    }

    pub fn tile_bounds(&self, tile_index: u16) -> Result<TileBounds, DecodeError> {
        if u64::from(tile_index) >= self.tile_count()? || self.tiles_across == 0 {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidImageGeometry,
            ));
        }
        let column = u32::from(tile_index) % self.tiles_across;
        let row = u32::from(tile_index) / self.tiles_across;
        let nominal_x0 = column
            .checked_mul(self.tile_width)
            .and_then(|offset| self.tile_x_origin.checked_add(offset))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let nominal_y0 = row
            .checked_mul(self.tile_height)
            .and_then(|offset| self.tile_y_origin.checked_add(offset))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let image_x1 = self
            .x_origin
            .checked_add(self.width)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let image_y1 = self
            .y_origin
            .checked_add(self.height)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let x0 = crate::refinement::tile_clip_start(nominal_x0, self.x_origin);
        let y0 = crate::refinement::tile_clip_start(nominal_y0, self.y_origin);
        let nominal_x1 = nominal_x0
            .checked_add(self.tile_width)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let nominal_y1 = nominal_y0
            .checked_add(self.tile_height)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let x1 = crate::refinement::tile_clip_end(nominal_x1, image_x1);
        let y1 = crate::refinement::tile_clip_end(nominal_y1, image_y1);
        if x0 >= x1 || y0 >= y1 {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidImageGeometry,
            ));
        }
        Ok(TileBounds {
            tile_index,
            x0,
            y0,
            x1,
            y1,
        })
    }

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
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidImageGeometry,
            ));
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
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidImageGeometry))?,
            image_y: y0
                .checked_sub(component_image_y0)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidImageGeometry))?,
        })
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum ProgressionOrder {
    Lrcp,
    Rlcp,
    Rpcl,
    Pcrl,
    Cprl,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ProgressionChange {
    pub resolution_start: u8,
    pub component_start: u16,
    pub layer_end: u16,
    pub resolution_end: u8,
    pub component_end: u16,
    pub progression_order: ProgressionOrder,
}

impl ProgressionChange {
    pub const fn contains(self, layer: u16, resolution: u8, component: u16) -> bool {
        layer < self.layer_end
            && resolution >= self.resolution_start
            && resolution < self.resolution_end
            && component >= self.component_start
            && component < self.component_end
    }
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ProgressionChanges {
    records: Vec<ProgressionChange>,
}

impl ProgressionChanges {
    pub const fn none() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn records(&self) -> &[ProgressionChange] {
        &self.records
    }

    #[cfg(test)]
    pub(crate) fn from_records(records: Vec<ProgressionChange>) -> Self {
        Self { records }
    }

    pub const fn is_present(&self) -> bool {
        !self.records.is_empty()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn equals(&self, other: &Self) -> bool {
        if self.records.len() != other.records.len() {
            return false;
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.records.len(),
                decreases self.records.len() - index,
            )
        )]
        while index < self.records.len() {
            let Some(left) = self.records.get(index) else {
                return false;
            };
            let Some(right) = other.records.get(index) else {
                return false;
            };
            if left.resolution_start != right.resolution_start
                || left.component_start != right.component_start
                || left.layer_end != right.layer_end
                || left.resolution_end != right.resolution_end
                || left.component_end != right.component_end
                || !progression_orders_equal(left.progression_order, right.progression_order)
            {
                return false;
            }
            index += 1;
        }
        true
    }
}

impl Default for ProgressionChanges {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum WaveletTransform {
    Irreversible9x7,
    Reversible5x3,
    Arbitrary(u8),
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodingStyle {
    pub progression_order: ProgressionOrder,
    pub layers: u16,
    pub multiple_component_transform: bool,
    pub decomposition_levels: u8,
    pub decomposition_structure: Option<u8>,
    pub decomposition_directions: [DecompositionDirection; 32],
    pub code_block_width: u16,
    pub code_block_height: u16,
    pub code_block_style: u8,
    pub transform: WaveletTransform,
    pub precincts_defined: bool,
    pub sop_markers: bool,
    pub eph_markers: bool,
    pub precinct_exponents: [(u8, u8); 33],
}

impl CodingStyle {
    pub const fn resolution_levels(self) -> u16 {
        (self.decomposition_levels as u16) + 1
    }

    pub fn subband_count(self) -> u16 {
        let mut count = 1_u16;
        let mut stage = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant stage <= self.decomposition_levels,
                decreases self.decomposition_levels - stage,
            )
        )]
        while stage < self.decomposition_levels && stage < 32 {
            let Some(direction) = self.decomposition_directions.get(stage as usize) else {
                return count;
            };
            let increment = match direction {
                DecompositionDirection::None => 0,
                DecompositionDirection::Horizontal | DecompositionDirection::Vertical => 1,
                DecompositionDirection::TwoDimensional => 3,
            };
            count = match count.checked_add(increment) {
                Some(value) => value,
                None => return count,
            };
            stage += 1;
        }
        count
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_decomposition_direction(
            decomposition_selector_view(self).0, decomposition_selector_view(self).1, stage_from_finest,
        ),
    ))]
    pub fn decomposition_direction(
        self,
        stage_from_finest: u8,
    ) -> Result<DecompositionDirection, DecodeError> {
        if stage_from_finest >= self.decomposition_levels {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodingStyle,
            });
        }
        match self
            .decomposition_directions
            .get(usize::from(stage_from_finest))
        {
            Some(direction) => Ok(*direction),
            None => Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodingStyle,
            }),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_direction_for_resolution(
            decomposition_selector_view(self).0, decomposition_selector_view(self).1, resolution,
        ),
    ))]
    pub fn direction_for_resolution(
        self,
        resolution: u8,
    ) -> Result<DecompositionDirection, DecodeError> {
        if resolution == 0 || resolution > self.decomposition_levels {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodingStyle,
            });
        }
        let stage = self
            .decomposition_levels
            .checked_sub(resolution)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        self.decomposition_direction(stage)
    }

    pub fn resolution_downsampling(self, resolution: u8) -> Result<(u32, u32), DecodeError> {
        if resolution > self.decomposition_levels {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
        }
        let discarded = self
            .decomposition_levels
            .checked_sub(resolution)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let mut horizontal = 1_u32;
        let mut vertical = 1_u32;
        let mut stage = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant stage <= discarded,
                decreases discarded - stage,
            )
        )]
        while stage < discarded {
            match self.decomposition_direction(stage)? {
                DecompositionDirection::None => {}
                DecompositionDirection::Horizontal => {
                    horizontal = horizontal.checked_mul(2).ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                    })?;
                }
                DecompositionDirection::Vertical => {
                    vertical = vertical.checked_mul(2).ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                    })?;
                }
                DecompositionDirection::TwoDimensional => {
                    horizontal = horizontal.checked_mul(2).ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                    })?;
                    vertical = vertical.checked_mul(2).ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                    })?;
                }
            }
            stage = stage
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        }
        Ok((horizontal, vertical))
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_subband_count_through_resolution(
            decomposition_selector_view(self).0, decomposition_selector_view(self).1, resolution,
        ),
    ))]
    pub fn subband_count_through_resolution(self, resolution: u8) -> Result<usize, DecodeError> {
        if resolution > self.decomposition_levels {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodingStyle,
            });
        }
        let mut count = 1_usize;
        let mut current = 1_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant current <= resolution + 1,
                current >= 1,
                resolution <= self.decomposition_levels,
                crate::specs::part1_subband_count_prefix(
                    decomposition_selector_view(self).0, decomposition_selector_view(self).1, (current - 1) as nat,
                ) == Ok(count),
                decreases resolution + 1 - current,
            )
        )]
        while current <= resolution {
            let direction = match self.direction_for_resolution(current) {
                Ok(direction) => direction,
                Err(error) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        crate::proofs::production_impl::production_subband_count_error_persists(
                            decomposition_selector_view(self).0, decomposition_selector_view(self).1,
                            current as nat, resolution as nat, error,
                        );
                    }
                    return Err(error);
                }
            };
            let added = match direction {
                DecompositionDirection::None => 0,
                DecompositionDirection::Horizontal | DecompositionDirection::Vertical => 1,
                DecompositionDirection::TwoDimensional => 3,
            };
            count = match count.checked_add(added) {
                Some(next) => next,
                None => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        crate::proofs::production_impl::production_subband_count_error_persists(
                            decomposition_selector_view(self).0, decomposition_selector_view(self).1,
                            current as nat, resolution as nat, crate::specs::part1_selector_error(true),
                        );
                    }
                    return Err(DecodeError::MalformedInput {
                        offset: 0,
                        kind: MalformedKind::ArithmeticOverflow,
                    });
                }
            };
            current = match current.checked_add(1) {
                Some(next) => next,
                None => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        crate::proofs::production_impl::production_subband_count_error_persists(
                            decomposition_selector_view(self).0, decomposition_selector_view(self).1,
                            current as nat, resolution as nat, crate::specs::part1_selector_error(true),
                        );
                    }
                    return Err(DecodeError::MalformedInput {
                        offset: 0,
                        kind: MalformedKind::ArithmeticOverflow,
                    });
                }
            };
        }
        Ok(count)
    }

    pub fn maximum_isotropic_reduction(self) -> u8 {
        let mut count = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant count <= self.decomposition_levels,
                decreases self.decomposition_levels - count,
            )
        )]
        while count < self.decomposition_levels && count < 32 {
            let Some(direction) = self.decomposition_directions.get(count as usize) else {
                return count;
            };
            if !matches!(direction, DecompositionDirection::TwoDimensional) {
                break;
            }
            count += 1;
        }
        count
    }

    pub fn tier1_coding_style(self) -> Result<Tier1CodingStyle, DecodeError> {
        Tier1CodingStyle::from_code_block_style(self.code_block_style)
    }

    pub fn precinct_exponents(self, resolution: u8) -> Result<(u8, u8), DecodeError> {
        if resolution > self.decomposition_levels {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
        }
        let exponents = self
            .precinct_exponents
            .get(usize::from(resolution))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidCodingStyle))?;
        Ok(*exponents)
    }

    pub fn equals(self, other: Self) -> bool {
        if !progression_orders_equal(self.progression_order, other.progression_order)
            || self.layers != other.layers
            || self.multiple_component_transform != other.multiple_component_transform
            || self.decomposition_levels != other.decomposition_levels
            || self.decomposition_structure != other.decomposition_structure
            || self.code_block_width != other.code_block_width
            || self.code_block_height != other.code_block_height
            || self.code_block_style != other.code_block_style
            || !wavelet_transforms_equal(self.transform, other.transform)
            || self.precincts_defined != other.precincts_defined
            || self.sop_markers != other.sop_markers
            || self.eph_markers != other.eph_markers
        {
            return false;
        }
        let mut stage = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant stage <= self.decomposition_directions.len(),
                decreases self.decomposition_directions.len() - stage,
            )
        )]
        while stage < self.decomposition_directions.len() {
            if !decomposition_directions_equal(
                self.decomposition_directions[stage],
                other.decomposition_directions[stage],
            ) {
                return false;
            }
            stage += 1;
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.precinct_exponents.len(),
                decreases self.precinct_exponents.len() - index,
            )
        )]
        while index < self.precinct_exponents.len() {
            if self.precinct_exponents[index].0 != other.precinct_exponents[index].0
                || self.precinct_exponents[index].1 != other.precinct_exponents[index].1
            {
                return false;
            }
            index += 1;
        }
        true
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum QuantizationStyle {
    None,
    ScalarDerived,
    ScalarExpounded,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Quantization {
    pub style: QuantizationStyle,
    pub guard_bits: u8,
    pub step_count: u16,
    steps: [QuantizationStep; MAX_QUANTIZATION_STEPS],
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ComponentCodingStyles {
    components: u16,
    styles: [CodingStyle; MAX_COMPONENTS],
}

impl ComponentCodingStyles {
    fn new(components: u16, default: CodingStyle) -> Result<Self, DecodeError> {
        if components == 0 || usize::from(components) > MAX_COMPONENTS {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidImageGeometry,
            ));
        }

        Ok(Self {
            components,
            styles: [default; MAX_COMPONENTS],
        })
    }

    pub(crate) fn uniform(components: u16, default: CodingStyle) -> Result<Self, DecodeError> {
        Self::new(components, default)
    }

    pub fn style(&self, component: u16) -> Result<CodingStyle, DecodeError> {
        if component >= self.components {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
        }
        match self.styles.get(usize::from(component)) {
            Some(style) => Ok(*style),
            None => Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle)),
        }
    }

    fn set(&mut self, component: u16, style: CodingStyle) -> Result<(), DecodeError> {
        if component >= self.components {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
        }
        let slot = self
            .styles
            .get_mut(usize::from(component))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidCodingStyle))?;
        *slot = style;
        Ok(())
    }

    pub fn equals(&self, other: &Self) -> bool {
        if self.components != other.components {
            return false;
        }
        let mut component = 0_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= self.components,
                decreases self.components - component,
            )
        )]
        while component < self.components {
            let left = match self.style(component) {
                Ok(value) => value,
                Err(_) => return false,
            };
            let right = match other.style(component) {
                Ok(value) => value,
                Err(_) => return false,
            };
            if !left.equals(right) {
                return false;
            }
            component += 1;
        }
        true
    }
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
/// Bounded per-component quantization snapshots, stored off-stack.
pub struct ComponentQuantizations {
    components: u16,
    quantizations: Vec<Quantization>,
}

impl ComponentQuantizations {
    fn new(components: u16, default: Quantization) -> Result<Self, DecodeError> {
        if components == 0 || usize::from(components) > MAX_COMPONENTS {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidImageGeometry,
            ));
        }

        Ok(Self {
            components,
            quantizations: vec![default; usize::from(components)],
        })
    }

    pub(crate) fn uniform(components: u16, default: Quantization) -> Result<Self, DecodeError> {
        Self::new(components, default)
    }

    pub fn quantization(&self, component: u16) -> Result<Quantization, DecodeError> {
        if component >= self.components {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidQuantization,
            ));
        }
        match self.quantizations.get(usize::from(component)) {
            Some(quantization) => Ok(*quantization),
            None => Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidQuantization,
            )),
        }
    }

    fn set(&mut self, component: u16, quantization: Quantization) -> Result<(), DecodeError> {
        if component >= self.components {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidQuantization,
            ));
        }
        let slot = self
            .quantizations
            .get_mut(usize::from(component))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidQuantization))?;
        *slot = quantization;
        Ok(())
    }

    pub fn equals(&self, other: &Self) -> bool {
        if self.components != other.components {
            return false;
        }
        let mut component = 0_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= self.components,
                decreases self.components - component,
            )
        )]
        while component < self.components {
            let left = match self.quantization(component) {
                Ok(value) => value,
                Err(_) => return false,
            };
            let right = match other.quantization(component) {
                Ok(value) => value,
                Err(_) => return false,
            };
            if !left.equals(&right) {
                return false;
            }
            component += 1;
        }
        true
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct RoiShifts {
    shifts: [u8; MAX_COMPONENTS],
}

impl RoiShifts {
    pub const fn none() -> Self {
        Self {
            shifts: [0; MAX_COMPONENTS],
        }
    }

    pub fn shift(&self, component: u16) -> Result<u8, DecodeError> {
        match self.shifts.get(usize::from(component)) {
            Some(shift) => Ok(*shift),
            None => Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidRegionOfInterest,
            )),
        }
    }

    pub fn equals(&self, other: &Self) -> bool {
        let mut component = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= self.shifts.len(),
                decreases self.shifts.len() - component,
            )
        )]
        while component < self.shifts.len() {
            if self.shifts[component] != other.shifts[component] {
                return false;
            }
            component += 1;
        }
        true
    }

    fn set(&mut self, component: u16, shift: u8) -> Result<(), DecodeError> {
        let value = self
            .shifts
            .get_mut(usize::from(component))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidRegionOfInterest))?;
        *value = shift;
        Ok(())
    }
}

impl Default for RoiShifts {
    fn default() -> Self {
        Self::none()
    }
}

impl Quantization {
    pub(crate) fn reversible_default(coding_style: CodingStyle) -> Self {
        Self {
            style: QuantizationStyle::None,
            guard_bits: 0,
            step_count: coding_style.subband_count(),
            steps: [QuantizationStep {
                exponent: 0,
                mantissa: 0,
            }; MAX_QUANTIZATION_STEPS],
        }
    }

    pub const fn style(&self) -> QuantizationStyle {
        self.style
    }

    pub const fn guard_bits(&self) -> u8 {
        self.guard_bits
    }

    pub fn step(&self, index: u16) -> Result<QuantizationStep, DecodeError> {
        if index >= self.step_count {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidQuantization,
            ));
        }
        let index = usize::from(index);
        if index >= self.steps.len() {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidQuantization,
            ));
        }
        Ok(self.steps[index])
    }

    pub fn equals(&self, other: &Self) -> bool {
        if !quantization_styles_equal(self.style, other.style)
            || self.guard_bits != other.guard_bits
            || self.step_count != other.step_count
        {
            return false;
        }
        let mut index = 0_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.step_count,
                decreases self.step_count - index,
            )
        )]
        while index < self.step_count {
            let left = match self.step(index) {
                Ok(value) => value,
                Err(_) => return false,
            };
            let right = match other.step(index) {
                Ok(value) => value,
                Err(_) => return false,
            };
            if left.exponent != right.exponent || left.mantissa != right.mantissa {
                return false;
            }
            index += 1;
        }
        true
    }

    pub fn is_compatible_with(self, coding_style: CodingStyle) -> bool {
        match self.style {
            QuantizationStyle::None | QuantizationStyle::ScalarExpounded => {
                self.step_count == coding_style.subband_count()
            }
            QuantizationStyle::ScalarDerived => self.step_count == 1,
        }
    }
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodestreamInfo {
    pub image_size: ImageSize,
    pub coding_style: CodingStyle,
    pub quantization: Quantization,
    pub component_coding_styles: ComponentCodingStyles,
    pub component_quantizations: ComponentQuantizations,
    pub roi_shifts: RoiShifts,
    pub progression_changes: ProgressionChanges,
    pub multiple_component_transform: Option<MultipleComponentTransform>,
    pub part2_extensions: Part2Extensions,
    pub profile: CodestreamProfile,
    pub main_header_segments: u64,
    pub tile_parts: u32,
    pub consumed_bytes: u64,
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum DecompositionDirection {
    #[default]
    None,
    TwoDimensional,
    Horizontal,
    Vertical,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct DecompositionStructure {
    index: u8,
    directions: Vec<DecompositionDirection>,
}

impl DecompositionStructure {
    pub const fn index(&self) -> u8 {
        self.index
    }

    pub fn directions(&self) -> &[DecompositionDirection] {
        &self.directions
    }
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ArbitraryDecomposition {
    index: u8,
    sublevel_depths: Vec<u8>,
    subband_splits: Vec<DecompositionDirection>,
}

impl ArbitraryDecomposition {
    pub const fn index(&self) -> u8 {
        self.index
    }

    pub fn sublevel_depths(&self) -> &[u8] {
        &self.sublevel_depths
    }

    pub fn subband_splits(&self) -> &[DecompositionDirection] {
        &self.subband_splits
    }
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct NonlinearPointTransform {
    component: Option<u16>,
    precision: u8,
    signed: bool,
    transform_type: u8,
    parameters: Vec<u8>,
    d_min: Option<u32>,
    d_max: Option<u32>,
    point_bits: Option<u8>,
    points: Vec<u32>,
    gamma_parameters: Option<[u32; 5]>,
}

impl NonlinearPointTransform {
    pub const fn component(&self) -> Option<u16> {
        self.component
    }

    pub const fn precision(&self) -> u8 {
        self.precision
    }

    pub const fn signed(&self) -> bool {
        self.signed
    }

    pub const fn transform_type(&self) -> u8 {
        self.transform_type
    }

    pub fn parameters(&self) -> &[u8] {
        &self.parameters
    }

    pub const fn d_min(&self) -> Option<u32> {
        self.d_min
    }

    pub const fn d_max(&self) -> Option<u32> {
        self.d_max
    }

    pub const fn point_bits(&self) -> Option<u8> {
        self.point_bits
    }

    pub fn points(&self) -> &[u32] {
        &self.points
    }

    pub const fn gamma_parameters(&self) -> Option<[u32; 5]> {
        self.gamma_parameters
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        precision: u8,
        signed: bool,
        transform_type: u8,
        d_min: Option<u32>,
        d_max: Option<u32>,
        points: Vec<u32>,
    ) -> Self {
        Self {
            component: None,
            precision,
            signed,
            transform_type,
            parameters: Vec::new(),
            d_min,
            d_max,
            point_bits: None,
            points,
            gamma_parameters: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn gamma_for_test(precision: u8, signed: bool, parameters: [u32; 5]) -> Self {
        Self {
            component: None,
            precision,
            signed,
            transform_type: 1,
            parameters: Vec::new(),
            d_min: None,
            d_max: None,
            point_bits: None,
            points: Vec::new(),
            gamma_parameters: Some(parameters),
        }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ComponentBitDepth {
    pub precision: u8,
    pub signed: bool,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ComponentRegistration {
    pub horizontal: u16,
    pub vertical: u16,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ArbitraryKernel {
    index: u8,
    reversible: bool,
    descriptor: u16,
    coefficient_type: u8,
    whole_sample_filter: bool,
    initial_odd: bool,
    whole_sample_extension: bool,
    scaling: Option<KernelCoefficient>,
    steps: Vec<KernelLiftingStep>,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct KernelCoefficient {
    bytes: [u8; 16],
    width: u8,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn raw_kernel_coefficient_view(value: KernelCoefficient) -> crate::specs::Part2RawKernelCoefficient {
        (value.bytes@, value.width)
    }

    pub open spec fn raw_kernel_coefficient_option(value: Option<KernelCoefficient>)
        -> Option<crate::specs::Part2RawKernelCoefficient>
    {
        match value {
            Some(coefficient) => Some(raw_kernel_coefficient_view(coefficient)),
            None => None,
        }
    }

    pub open spec fn raw_kernel_coefficients_view(values: Seq<KernelCoefficient>)
        -> Seq<crate::specs::Part2RawKernelCoefficient>
    {
        Seq::new(values.len(), |i: int| raw_kernel_coefficient_view(values[i]))
    }

    pub closed spec fn raw_kernel_step_view(value: KernelLiftingStep) -> crate::specs::Part2RawKernelStep {
        (value.offset, value.exponent, raw_kernel_coefficient_option(value.residue),
            raw_kernel_coefficients_view(value.coefficients@))
    }

    pub open spec fn raw_kernel_steps_view(values: Seq<KernelLiftingStep>) -> Seq<crate::specs::Part2RawKernelStep> {
        Seq::new(values.len(), |i: int| raw_kernel_step_view(values[i]))
    }

    pub closed spec fn raw_kernel_view(value: ArbitraryKernel) -> crate::specs::Part2RawKernel {
        (value.reversible, value.coefficient_type, value.whole_sample_filter, value.initial_odd,
            value.whole_sample_extension, raw_kernel_coefficient_option(value.scaling), raw_kernel_steps_view(value.steps@))
    }
}

impl KernelCoefficient {
    #[cfg(test)]
    pub(crate) fn for_test(bits: u128, width: u8) -> Self {
        Self {
            bytes: bits.to_be_bytes(),
            width,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result@ == raw_kernel_coefficient_view(*self).0,
    ))]
    pub const fn bytes(&self) -> &[u8; 16] {
        &self.bytes
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == raw_kernel_coefficient_view(*self).1,
    ))]
    pub const fn width(&self) -> u8 {
        self.width
    }
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct KernelLiftingStep {
    offset: i8,
    exponent: u8,
    residue: Option<KernelCoefficient>,
    coefficients: Vec<KernelCoefficient>,
}

impl KernelLiftingStep {
    #[cfg(test)]
    pub(crate) fn for_test(
        offset: i8,
        exponent: u8,
        residue: Option<KernelCoefficient>,
        coefficients: Vec<KernelCoefficient>,
    ) -> Self {
        Self {
            offset,
            exponent,
            residue,
            coefficients,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == raw_kernel_step_view(*self).0,
    ))]
    pub const fn offset(&self) -> i8 {
        self.offset
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == raw_kernel_step_view(*self).1,
    ))]
    pub const fn exponent(&self) -> u8 {
        self.exponent
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures raw_kernel_coefficient_option(result) == raw_kernel_step_view(*self).2,
    ))]
    pub const fn residue(&self) -> Option<KernelCoefficient> {
        self.residue
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures raw_kernel_coefficients_view(result@) == raw_kernel_step_view(*self).3,
    ))]
    pub fn coefficients(&self) -> &[KernelCoefficient] {
        &self.coefficients
    }
}

impl ArbitraryKernel {
    #[cfg(test)]
    pub(crate) fn for_test(
        reversible: bool,
        coefficient_type: u8,
        whole_sample_filter: bool,
        initial_odd: bool,
        whole_sample_extension: bool,
        scaling: Option<KernelCoefficient>,
        steps: Vec<KernelLiftingStep>,
    ) -> Self {
        Self {
            index: u8::MAX,
            reversible,
            descriptor: u16::MAX,
            coefficient_type,
            whole_sample_filter,
            initial_odd,
            whole_sample_extension,
            scaling,
            steps,
        }
    }

    pub const fn index(&self) -> u8 {
        self.index
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == raw_kernel_view(*self).0,
    ))]
    pub const fn reversible(&self) -> bool {
        self.reversible
    }

    pub const fn descriptor(&self) -> u16 {
        self.descriptor
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == raw_kernel_view(*self).1,
    ))]
    pub const fn coefficient_type(&self) -> u8 {
        self.coefficient_type
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == raw_kernel_view(*self).2,
    ))]
    pub const fn whole_sample_filter(&self) -> bool {
        self.whole_sample_filter
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == raw_kernel_view(*self).3,
    ))]
    pub const fn initial_odd(&self) -> bool {
        self.initial_odd
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == raw_kernel_view(*self).4,
    ))]
    pub const fn whole_sample_extension(&self) -> bool {
        self.whole_sample_extension
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures raw_kernel_coefficient_option(result) == raw_kernel_view(*self).5,
    ))]
    pub const fn scaling(&self) -> Option<KernelCoefficient> {
        self.scaling
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures raw_kernel_steps_view(result@) == raw_kernel_view(*self).6,
    ))]
    pub fn steps(&self) -> &[KernelLiftingStep] {
        &self.steps
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct TilePartLength {
    pub tile_index: Option<u16>,
    pub length: u32,
}

#[derive(Clone, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Part2Extensions {
    decomposition_structures: Vec<DecompositionStructure>,
    arbitrary_decompositions: Vec<ArbitraryDecomposition>,
    nonlinear_transforms: Vec<NonlinearPointTransform>,
    component_bit_depths: Vec<ComponentBitDepth>,
    arbitrary_kernels: Vec<ArbitraryKernel>,
    component_registrations: Vec<ComponentRegistration>,
    tile_part_lengths: Vec<TilePartLength>,
    metadata_bytes: u64,
}

impl Part2Extensions {
    pub fn decomposition_structures(&self) -> &[DecompositionStructure] {
        &self.decomposition_structures
    }

    pub fn arbitrary_decompositions(&self) -> &[ArbitraryDecomposition] {
        &self.arbitrary_decompositions
    }

    pub fn nonlinear_transforms(&self) -> &[NonlinearPointTransform] {
        &self.nonlinear_transforms
    }

    pub fn component_bit_depths(&self) -> &[ComponentBitDepth] {
        &self.component_bit_depths
    }

    pub fn arbitrary_kernels(&self) -> &[ArbitraryKernel] {
        &self.arbitrary_kernels
    }

    pub fn component_registrations(&self) -> &[ComponentRegistration] {
        &self.component_registrations
    }

    pub fn tile_part_lengths(&self) -> &[TilePartLength] {
        &self.tile_part_lengths
    }

    pub(crate) fn nonlinear_transform_for(
        &self,
        component: u16,
    ) -> Option<&NonlinearPointTransform> {
        if let Some(transform) = self
            .nonlinear_transforms
            .iter()
            .find(|transform| transform.component == Some(component))
        {
            return Some(transform);
        }

        self.nonlinear_transforms
            .iter()
            .find(|transform| transform.component.is_none())
    }

    pub(crate) fn decomposition_structure(&self, index: u8) -> Option<&DecompositionStructure> {
        self.decomposition_structures
            .iter()
            .find(|structure| structure.index == index)
    }

    pub(crate) fn arbitrary_kernel(&self, index: u8) -> Option<&ArbitraryKernel> {
        self.arbitrary_kernels
            .iter()
            .find(|kernel| kernel.index == index)
    }

    pub(crate) fn arbitrary_decomposition(&self, index: u8) -> Option<&ArbitraryDecomposition> {
        self.arbitrary_decompositions
            .iter()
            .find(|decomposition| decomposition.index == index)
    }

    #[cfg(test)]
    pub(crate) fn with_nonlinear_transform_for_test(transform: NonlinearPointTransform) -> Self {
        Self {
            nonlinear_transforms: vec![transform],
            ..Self::default()
        }
    }

    pub(crate) fn component_bit_depths_equal(&self, other: &Self) -> bool {
        if self.component_bit_depths.len() != other.component_bit_depths.len() {
            return false;
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.component_bit_depths.len(),
                decreases self.component_bit_depths.len() - index,
            )
        )]
        while index < self.component_bit_depths.len() {
            let Some(left) = self.component_bit_depths.get(index) else {
                return false;
            };
            let Some(right) = other.component_bit_depths.get(index) else {
                return false;
            };
            if left.precision != right.precision || left.signed != right.signed {
                return false;
            }
            index += 1;
        }
        true
    }

    pub(crate) fn validate_executable(&self) -> Result<(), DecodeError> {
        Ok(())
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum CodestreamBlockCoding {
    Legacy,
    HighThroughput,
    Mixed,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum Part15BlockMode {
    HtOnly,
    HtDeclared,
    Mixed,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ExtendedCapabilities {
    p_cap: u32,
    c_capabilities: Vec<u16>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ProfileNumber {
    words: Vec<u16>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodestreamProfile {
    pub r_siz: u16,
    pub block_coding: CodestreamBlockCoding,
    extended_capabilities: Option<ExtendedCapabilities>,
    profile: Option<ProfileNumber>,
    corresponding_profile: Option<ProfileNumber>,
}

impl ExtendedCapabilities {
    pub const fn p_cap(&self) -> u32 {
        self.p_cap
    }

    pub fn c_capabilities(&self) -> &[u16] {
        &self.c_capabilities
    }

    pub const fn advertises_part15(&self) -> bool {
        self.p_cap & PART_15_CAPABILITY_FLAG != 0
    }

    pub fn part15_value(&self) -> Option<u16> {
        self.capability_for_part(15)
    }

    pub fn part15_block_mode(&self) -> Option<Part15BlockMode> {
        match self.part15_value()? >> 14 {
            0 => Some(Part15BlockMode::HtOnly),
            2 => Some(Part15BlockMode::HtDeclared),
            3 => Some(Part15BlockMode::Mixed),
            _ => None,
        }
    }

    pub fn part15_allows_multiple_ht_sets(&self) -> bool {
        matches!(self.part15_value(), Some(value) if value & 0x2000 != 0)
    }

    pub fn part15_allows_roi(&self) -> bool {
        matches!(self.part15_value(), Some(value) if value & 0x1000 != 0)
    }

    pub fn part15_allows_heterogeneous_headers(&self) -> bool {
        matches!(self.part15_value(), Some(value) if value & 0x0800 != 0)
    }

    pub fn part15_allows_irreversible_ht(&self) -> bool {
        matches!(self.part15_value(), Some(value) if value & 0x0020 != 0)
    }

    pub fn part15_magnitude_bound(&self) -> Option<u8> {
        let parameter = (self.part15_value()? & 0x1f) as u8;
        Some(crate::refinement::part15_magnitude_bound(parameter))
    }

    #[allow(clippy::manual_map, clippy::question_mark)]
    pub fn capability_for_part(&self, part: u8) -> Option<u16> {
        if part == 0 || part > 32 {
            return None;
        }
        let mut index = 0_usize;
        let mut current = 1_u8;
        let mut mask = 0x8000_0000_u32;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant current <= part,
                decreases part - current,
            )
        )]
        while current < part {
            if self.p_cap & mask != 0 {
                index = match index.checked_add(1) {
                    Some(value) => value,
                    None => return None,
                };
            }
            mask >>= 1;
            current = match current.checked_add(1) {
                Some(value) => value,
                None => return None,
            };
        }
        if self.p_cap & mask == 0 {
            return None;
        }
        match self.c_capabilities.get(index) {
            Some(value) => Some(*value),
            None => None,
        }
    }
}

impl ProfileNumber {
    pub fn words(&self) -> &[u16] {
        &self.words
    }
}

impl CodestreamProfile {
    pub fn is_part15(&self) -> bool {
        match self.extended_capabilities.as_ref() {
            Some(capabilities) => capabilities.advertises_part15(),
            None => false,
        }
    }

    pub const fn uses_high_throughput(&self) -> bool {
        !matches!(self.block_coding, CodestreamBlockCoding::Legacy)
    }

    pub const fn extended_capabilities(&self) -> Option<&ExtendedCapabilities> {
        self.extended_capabilities.as_ref()
    }

    pub const fn profile(&self) -> Option<&ProfileNumber> {
        self.profile.as_ref()
    }

    pub const fn corresponding_profile(&self) -> Option<&ProfileNumber> {
        self.corresponding_profile.as_ref()
    }

    pub fn part15_allows_multiple_ht_sets(&self) -> bool {
        match self.extended_capabilities.as_ref() {
            Some(capabilities) => capabilities.part15_allows_multiple_ht_sets(),
            None => false,
        }
    }

    pub fn part15_magnitude_bound(&self) -> u8 {
        self.extended_capabilities
            .as_ref()
            .and_then(ExtendedCapabilities::part15_magnitude_bound)
            .unwrap_or(74)
    }

    pub fn part15_requires_all_ht_blocks(&self) -> bool {
        matches!(
            self.extended_capabilities
                .as_ref()
                .and_then(ExtendedCapabilities::part15_block_mode),
            Some(Part15BlockMode::HtOnly)
        )
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum MctArrayKind {
    Dependency,
    Decorrelation,
    Offset,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum MctNumericFormat {
    Int16,
    Int32,
    Float32,
    Float64,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum MctCollectionKind {
    Dependency,
    Decorrelation,
    Wavelet,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum MctWaveletType {
    Irreversible9x7,
    Reversible5x3,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct MctWavelet {
    transform: MctWaveletType,
    levels: u8,
    component_offset: i32,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct MctArray {
    index: u8,
    kind: MctArrayKind,
    numeric_format: MctNumericFormat,
    fraction_bits: u8,
    values: Vec<i64>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct MultipleComponentTransform {
    stages: Vec<MctStage>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct MctStage {
    index: u8,
    collections: Vec<MctCollection>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct MctCollection {
    reversible: bool,
    input_components: Vec<u16>,
    output_components: Vec<u16>,
    kind: MctCollectionKind,
    matrix: Option<MctArray>,
    offsets: Option<MctArray>,
    wavelet: Option<MctWavelet>,
}

impl MctArray {
    pub const fn index(&self) -> u8 {
        self.index
    }

    pub const fn kind(&self) -> MctArrayKind {
        self.kind
    }

    pub const fn numeric_format(&self) -> MctNumericFormat {
        self.numeric_format
    }

    pub const fn fraction_bits(&self) -> u8 {
        self.fraction_bits
    }

    pub fn values(&self) -> &[i64] {
        &self.values
    }

    pub fn equals(&self, other: &Self) -> bool {
        if self.index != other.index
            || !mct_array_kinds_equal(self.kind, other.kind)
            || !mct_numeric_formats_equal(self.numeric_format, other.numeric_format)
            || self.fraction_bits != other.fraction_bits
            || self.values.len() != other.values.len()
        {
            return false;
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.values.len(),
                decreases self.values.len() - index,
            )
        )]
        while index < self.values.len() {
            if self.values.get(index) != other.values.get(index) {
                return false;
            }
            index += 1;
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn from_values(
        index: u8,
        kind: MctArrayKind,
        numeric_format: MctNumericFormat,
        fraction_bits: u8,
        values: Vec<i64>,
    ) -> Self {
        Self {
            index,
            kind,
            numeric_format,
            fraction_bits,
            values,
        }
    }
}

impl MultipleComponentTransform {
    pub fn stages(&self) -> &[MctStage] {
        &self.stages
    }

    pub fn equals(&self, other: &Self) -> bool {
        if self.stages.len() != other.stages.len() {
            return false;
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.stages.len(),
                decreases self.stages.len() - index,
            )
        )]
        while index < self.stages.len() {
            let Some(left) = self.stages.get(index) else {
                return false;
            };
            let Some(right) = other.stages.get(index) else {
                return false;
            };
            if !left.equals(right) {
                return false;
            }
            index += 1;
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn from_arrays(
        reversible: bool,
        components: u16,
        matrix: MctArray,
        offsets: Option<MctArray>,
    ) -> Self {
        let mut references = Vec::with_capacity(usize::from(components));
        let mut component = 0_u16;
        while component < components {
            references.push(component);
            component += 1;
        }
        Self {
            stages: vec![MctStage {
                index: 1,
                collections: vec![MctCollection {
                    reversible,
                    input_components: references.clone(),
                    output_components: references,
                    kind: MctCollectionKind::Decorrelation,
                    matrix: Some(matrix),
                    offsets,
                    wavelet: None,
                }],
            }],
        }
    }

    #[cfg(test)]
    pub(crate) fn from_stages(stages: Vec<MctStage>) -> Self {
        Self { stages }
    }
}

impl MctStage {
    pub const fn index(&self) -> u8 {
        self.index
    }

    pub fn collections(&self) -> &[MctCollection] {
        &self.collections
    }

    pub fn equals(&self, other: &Self) -> bool {
        if self.index != other.index || self.collections.len() != other.collections.len() {
            return false;
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.collections.len(),
                decreases self.collections.len() - index,
            )
        )]
        while index < self.collections.len() {
            let Some(left) = self.collections.get(index) else {
                return false;
            };
            let Some(right) = other.collections.get(index) else {
                return false;
            };
            if !left.equals(right) {
                return false;
            }
            index += 1;
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn from_collections(index: u8, collections: Vec<MctCollection>) -> Self {
        Self { index, collections }
    }
}

impl MctCollection {
    pub const fn reversible(&self) -> bool {
        self.reversible
    }

    pub fn input_components(&self) -> &[u16] {
        &self.input_components
    }

    pub fn output_components(&self) -> &[u16] {
        &self.output_components
    }

    pub const fn kind(&self) -> MctCollectionKind {
        self.kind
    }

    pub const fn matrix(&self) -> Option<&MctArray> {
        self.matrix.as_ref()
    }

    pub const fn offsets(&self) -> Option<&MctArray> {
        self.offsets.as_ref()
    }

    pub const fn wavelet(&self) -> Option<MctWavelet> {
        self.wavelet
    }

    pub fn equals(&self, other: &Self) -> bool {
        if self.reversible != other.reversible
            || !u16_lists_equal(&self.input_components, &other.input_components)
            || !u16_lists_equal(&self.output_components, &other.output_components)
            || !mct_collection_kinds_equal(self.kind, other.kind)
        {
            return false;
        }
        let matrices_equal = match (&self.matrix, &other.matrix) {
            (Some(left), Some(right)) => left.equals(right),
            (None, None) => true,
            _ => false,
        };
        let offsets_equal = match (&self.offsets, &other.offsets) {
            (Some(left), Some(right)) => left.equals(right),
            (None, None) => true,
            _ => false,
        };
        matrices_equal && offsets_equal && mct_wavelets_equal(self.wavelet, other.wavelet)
    }

    #[cfg(test)]
    pub(crate) fn from_arrays(
        reversible: bool,
        input_components: Vec<u16>,
        output_components: Vec<u16>,
        matrix: MctArray,
        offsets: Option<MctArray>,
    ) -> Self {
        Self {
            reversible,
            input_components,
            output_components,
            kind: MctCollectionKind::Decorrelation,
            matrix: Some(matrix),
            offsets,
            wavelet: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_dependency(
        reversible: bool,
        components: Vec<u16>,
        coefficients: Option<MctArray>,
        offsets: Option<MctArray>,
    ) -> Self {
        Self {
            reversible,
            input_components: components.clone(),
            output_components: components,
            kind: MctCollectionKind::Dependency,
            matrix: coefficients,
            offsets,
            wavelet: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_wavelet(
        input_components: Vec<u16>,
        output_components: Vec<u16>,
        transform: MctWaveletType,
        levels: u8,
        component_offset: i32,
    ) -> Self {
        Self {
            reversible: matches!(transform, MctWaveletType::Reversible5x3),
            input_components,
            output_components,
            kind: MctCollectionKind::Wavelet,
            matrix: None,
            offsets: None,
            wavelet: Some(MctWavelet {
                transform,
                levels,
                component_offset,
            }),
        }
    }
}

impl MctWavelet {
    pub const fn transform(self) -> MctWaveletType {
        self.transform
    }

    pub const fn levels(self) -> u8 {
        self.levels
    }

    pub const fn component_offset(self) -> i32 {
        self.component_offset
    }
}

const fn mct_collection_kinds_equal(left: MctCollectionKind, right: MctCollectionKind) -> bool {
    matches!(
        (left, right),
        (MctCollectionKind::Dependency, MctCollectionKind::Dependency)
            | (
                MctCollectionKind::Decorrelation,
                MctCollectionKind::Decorrelation
            )
            | (MctCollectionKind::Wavelet, MctCollectionKind::Wavelet)
    )
}

const fn mct_wavelets_equal(left: Option<MctWavelet>, right: Option<MctWavelet>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            mct_wavelet_types_equal(left.transform, right.transform)
                && left.levels == right.levels
                && left.component_offset == right.component_offset
        }
        (None, None) => true,
        _ => false,
    }
}

const fn mct_wavelet_types_equal(left: MctWaveletType, right: MctWaveletType) -> bool {
    matches!(
        (left, right),
        (
            MctWaveletType::Irreversible9x7,
            MctWaveletType::Irreversible9x7
        ) | (MctWaveletType::Reversible5x3, MctWaveletType::Reversible5x3)
    )
}

const fn mct_array_kinds_equal(left: MctArrayKind, right: MctArrayKind) -> bool {
    matches!(
        (left, right),
        (MctArrayKind::Dependency, MctArrayKind::Dependency)
            | (MctArrayKind::Decorrelation, MctArrayKind::Decorrelation)
            | (MctArrayKind::Offset, MctArrayKind::Offset)
    )
}

const fn mct_numeric_formats_equal(left: MctNumericFormat, right: MctNumericFormat) -> bool {
    matches!(
        (left, right),
        (MctNumericFormat::Int16, MctNumericFormat::Int16)
            | (MctNumericFormat::Int32, MctNumericFormat::Int32)
            | (MctNumericFormat::Float32, MctNumericFormat::Float32)
            | (MctNumericFormat::Float64, MctNumericFormat::Float64)
    )
}

fn u16_lists_equal(left: &[u16], right: &[u16]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= left.len(),
            decreases left.len() - index,
        )
    )]
    while index < left.len() {
        if left.get(index) != right.get(index) {
            return false;
        }
        index += 1;
    }
    true
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
enum PacketLengthSource<'a> {
    None,
    Plm(&'a [u8]),
    Plt(&'a [u8]),
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
enum PackedPacketHeaderSource<'a> {
    None,
    MarkerSegments {
        header: &'a [u8],
        marker: u16,
        logical_start: usize,
    },
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct PackedPacketHeaders<'a> {
    source: PackedPacketHeaderSource<'a>,
    length: usize,
    fragments: u16,
}

impl PackedPacketHeaders<'_> {
    pub const fn none() -> Self {
        Self {
            source: PackedPacketHeaderSource::None,
            length: 0,
            fragments: 0,
        }
    }

    pub const fn is_present(&self) -> bool {
        !matches!(self.source, PackedPacketHeaderSource::None)
    }

    pub const fn len(&self) -> usize {
        self.length
    }

    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub const fn fragment_count(&self) -> u16 {
        self.fragments
    }

    pub(crate) fn reassemble(&self, limits: &ResourceLimits) -> Result<Vec<u8>, DecodeError> {
        if self.length as u64 > limits.maximum_packet_header_bytes() {
            return Err(DecodeError::limit(
                Resource::PacketHeaderBytes,
                limits.maximum_packet_header_bytes(),
                self.length as u64,
            ));
        }
        let PackedPacketHeaderSource::MarkerSegments {
            header,
            marker,
            logical_start,
        } = self.source
        else {
            return Ok(Vec::new());
        };
        let logical_end = logical_start
            .checked_add(self.length)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let mut output = Vec::with_capacity(self.length);
        let mut position = 0_usize;
        let mut logical_position = 0_usize;
        let mut remaining_segments = header.len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases remaining_segments,)
        )]
        while position < header.len() {
            if remaining_segments == 0 {
                return Err(DecodeError::malformed(
                    position as u64,
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            remaining_segments -= 1;
            if header.len().saturating_sub(position) < 4 {
                return Err(DecodeError::malformed(
                    position as u64,
                    MalformedKind::InvalidPackedPacketHeader,
                ));
            }
            let current_marker = read_u16([header[position], header[position + 1]]);
            let segment_length =
                usize::from(read_u16([header[position + 2], header[position + 3]]));
            if segment_length < 2 {
                return Err(DecodeError::malformed(
                    position as u64,
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            let end = position
                .checked_add(2)
                .and_then(|value| value.checked_add(segment_length))
                .ok_or_else(|| {
                    DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
                })?;
            if end > header.len() {
                return Err(DecodeError::malformed(
                    position as u64,
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            if current_marker == marker {
                let payload_start = position.checked_add(5).ok_or_else(|| {
                    DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
                })?;
                if payload_start > end {
                    return Err(DecodeError::malformed(
                        position as u64,
                        MalformedKind::InvalidMarkerLength,
                    ));
                }
                let fragment = &header[payload_start..end];
                let fragment_end =
                    logical_position
                        .checked_add(fragment.len())
                        .ok_or_else(|| {
                            DecodeError::malformed(
                                position as u64,
                                MalformedKind::ArithmeticOverflow,
                            )
                        })?;
                let copy_start = logical_start.max(logical_position);
                let copy_end = logical_end.min(fragment_end);
                if copy_start < copy_end {
                    let local_start = copy_start - logical_position;
                    let local_end = copy_end - logical_position;
                    output.extend_from_slice(&fragment[local_start..local_end]);
                }
                logical_position = fragment_end;
            }
            position = end;
        }
        if output.len() != self.length {
            return Err(DecodeError::malformed(
                logical_position as u64,
                MalformedKind::InvalidPackedPacketHeader,
            ));
        }
        Ok(output)
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct PacketLengths<'a> {
    source: PacketLengthSource<'a>,
    packet_count: u64,
    encoded_bytes: u64,
}

impl PacketLengths<'_> {
    pub const fn none() -> Self {
        Self {
            source: PacketLengthSource::None,
            packet_count: 0,
            encoded_bytes: 0,
        }
    }

    pub const fn is_present(&self) -> bool {
        !matches!(self.source, PacketLengthSource::None)
    }

    pub const fn packet_count(&self) -> u64 {
        self.packet_count
    }

    pub const fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }

    pub(crate) const fn iter(&self) -> PacketLengthIter<'_> {
        PacketLengthIter {
            source: self.source,
            position: 0,
            segment_end: 0,
        }
    }
}

pub(crate) struct PacketLengthIter<'a> {
    source: PacketLengthSource<'a>,
    position: usize,
    segment_end: usize,
}

impl PacketLengthIter<'_> {
    pub(crate) fn next_length(&mut self) -> Result<Option<u32>, DecodeError> {
        match self.source {
            PacketLengthSource::None => Ok(None),
            PacketLengthSource::Plm(bytes) => {
                if self.position == bytes.len() {
                    return Ok(None);
                }
                let (length, next) = decode_packet_length(bytes, self.position, bytes.len())?;
                self.position = next;
                Ok(Some(length))
            }
            PacketLengthSource::Plt(header) =>
            {
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(decreases header.len() as int - self.position as int,)
                )]
                loop {
                    let previous_position = self.position;
                    if self.position < self.segment_end {
                        let (length, next) =
                            decode_packet_length(header, self.position, self.segment_end)?;
                        self.position = next;
                        return Ok(Some(length));
                    }
                    if self.position == header.len() {
                        return Ok(None);
                    }
                    if header.len().saturating_sub(self.position) < 4 {
                        return Err(DecodeError::malformed(
                            self.position as u64,
                            MalformedKind::InvalidPacketLength,
                        ));
                    }
                    let marker = read_u16([header[self.position], header[self.position + 1]]);
                    let marker_length = usize::from(read_u16([
                        header[self.position + 2],
                        header[self.position + 3],
                    ]));
                    if marker_length < 2 {
                        return Err(DecodeError::malformed(
                            self.position as u64,
                            MalformedKind::InvalidMarkerLength,
                        ));
                    }
                    let end = self
                        .position
                        .checked_add(2)
                        .and_then(|value| value.checked_add(marker_length))
                        .ok_or_else(|| {
                            DecodeError::malformed(
                                self.position as u64,
                                MalformedKind::ArithmeticOverflow,
                            )
                        })?;
                    if end > header.len() {
                        return Err(DecodeError::malformed(
                            self.position as u64,
                            MalformedKind::InvalidMarkerLength,
                        ));
                    }
                    self.position = end;
                    if marker == PLT {
                        let data_start = self
                            .position
                            .checked_sub(marker_length)
                            .and_then(|value| value.checked_add(3))
                            .ok_or_else(|| {
                                DecodeError::malformed(
                                    self.position as u64,
                                    MalformedKind::ArithmeticOverflow,
                                )
                            })?;
                        if data_start > end {
                            return Err(DecodeError::malformed(
                                self.position as u64,
                                MalformedKind::InvalidMarkerLength,
                            ));
                        }
                        self.position = data_start;
                        self.segment_end = end;
                    }
                    if self.position <= previous_position || self.position > header.len() {
                        return Err(DecodeError::malformed(
                            previous_position as u64,
                            MalformedKind::InvalidPacketLength,
                        ));
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
struct Segment<'a> {
    marker: u16,
    offset: usize,
    payload: &'a [u8],
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
struct MctParameters {
    arrays: Vec<MctArray>,
    records: Vec<MctRecord>,
    order: Vec<u8>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
struct MctRecord {
    index: u8,
    collections: Vec<MctCollectionReference>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
struct MctCollectionReference {
    input_components: Vec<u16>,
    output_components: Vec<u16>,
    transform: MctCollectionReferenceTransform,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
enum MctCollectionReferenceTransform {
    Array {
        kind: MctArrayKind,
        reversible: bool,
        transform_index: u8,
        offset_index: u8,
    },
    Wavelet(MctWavelet),
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
struct MctFragment {
    descriptor: u16,
    next_index: u16,
    final_index: u16,
    offset: usize,
    data: Vec<u8>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
struct MccFragment {
    record_index: u8,
    next_index: u16,
    final_index: u16,
    collection_count: u16,
    offset: usize,
    data: Vec<u8>,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
struct TilePart {
    start: usize,
    length: u32,
    tile_index: u16,
    tile_part_index: u8,
    tile_part_count: u8,
}

#[derive(Clone, Copy)]
struct PackedHeaderFrame {
    start: usize,
    length: usize,
}

pub fn parse(input: &[u8], limits: &ResourceLimits) -> Result<CodestreamInfo, DecodeError> {
    Ok(parse_layout(input, limits)?.info)
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodestreamLayout<'a> {
    pub info: CodestreamInfo,
    pub tile_parts: Vec<TilePartPayload<'a>>,
}

#[derive(Clone)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct TilePartPayload<'a> {
    pub tile_index: u16,
    pub tile_part_index: u8,
    pub tile_part_count: u8,
    pub coding_style: CodingStyle,
    pub quantization: Quantization,
    pub component_coding_styles: ComponentCodingStyles,
    pub component_quantizations: ComponentQuantizations,
    pub roi_shifts: RoiShifts,
    pub progression_changes: ProgressionChanges,
    pub multiple_component_transform: Option<MultipleComponentTransform>,
    pub part2_extensions: Part2Extensions,
    pub packet_lengths: PacketLengths<'a>,
    pub packed_packet_headers: PackedPacketHeaders<'a>,
    pub has_functional_header_marker: bool,
    pub has_roi_marker: bool,
    pub data: &'a [u8],
}

impl<'a> TilePartPayload<'a> {
    pub fn uniform_for_packet_decoding(
        tile_index: u16,
        tile_part_index: u8,
        tile_part_count: u8,
        components: u16,
        coding_style: CodingStyle,
        roi_shifts: RoiShifts,
        data: &'a [u8],
    ) -> Result<Self, DecodeError> {
        let quantization = Quantization::reversible_default(coding_style);
        Ok(Self {
            tile_index,
            tile_part_index,
            tile_part_count,
            coding_style,
            quantization,
            component_coding_styles: ComponentCodingStyles::uniform(components, coding_style)?,
            component_quantizations: ComponentQuantizations::uniform(components, quantization)?,
            roi_shifts,
            progression_changes: ProgressionChanges::none(),
            multiple_component_transform: None,
            part2_extensions: Part2Extensions::default(),
            packet_lengths: PacketLengths::none(),
            packed_packet_headers: PackedPacketHeaders::none(),
            has_functional_header_marker: false,
            has_roi_marker: false,
            data,
        })
    }
}

pub fn parse_layout<'a>(
    input: &'a [u8],
    limits: &'a ResourceLimits,
) -> Result<CodestreamLayout<'a>, DecodeError> {
    Parser::new(input, limits)?.parse_layout()
}

struct Parser<'a> {
    input: &'a [u8],
    limits: &'a ResourceLimits,
    position: usize,
    work_units: u64,
    packet_length_bytes: u64,
    packet_lengths: u64,
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8], limits: &'a ResourceLimits) -> Result<Self, DecodeError> {
        let input_length = input.len() as u64;
        if input_length > limits.maximum_input_bytes() {
            return Err(DecodeError::limit(
                Resource::InputBytes,
                limits.maximum_input_bytes(),
                input_length,
            ));
        }
        Ok(Self {
            input,
            limits,
            position: 0,
            work_units: 0,
            packet_length_bytes: 0,
            packet_lengths: 0,
        })
    }

    fn parse_layout(self) -> Result<CodestreamLayout<'a>, DecodeError> {
        let mut parser = self;
        parser.expect_standalone(SOC)?;
        let siz = parser.read_segment()?;
        if siz.marker != SIZ {
            return Err(DecodeError::malformed(
                parser.offset(siz.offset),
                MalformedKind::InvalidMarkerOrder,
            ));
        }
        let (mut image_size, r_siz) = parser.parse_siz(siz)?;
        let main_header_start = parser.position;
        let mut main_header_segments = 1_u64;
        let mut coding_style = None;
        let mut quantization = None;
        let mut coc_segments = [None; MAX_COMPONENTS];
        let mut qcc_segments = [None; MAX_COMPONENTS];
        let mut roi_shifts = RoiShifts::none();
        let mut roi_seen = [false; MAX_COMPONENTS];
        let mut plm_groups = Vec::new();
        let mut next_plm_index = 0_u16;
        let mut plm_seen = false;
        let mut ppm_bytes = Vec::new();
        let mut next_ppm_index = 0_u16;
        let mut ppm_fragments = 0_u16;
        let mut progression_changes = ProgressionChanges::none();
        let mut poc_seen = false;
        let mut mct_arrays = Vec::new();
        let mut mct_records = Vec::new();
        let mut mct_order = None;
        let mut pending_mct = Vec::new();
        let mut completed_mct = Vec::new();
        let mut pending_mcc = Vec::new();
        let mut completed_mcc = Vec::new();
        let mut extended_capabilities = None;
        let mut profile = None;
        let mut corresponding_profile = None;
        let mut part2_extensions = Part2Extensions::default();
        let mut next_tlm_index = 0_u16;
        let input_length = parser.input.len();

        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases input_length as int - parser.position as int,)
        )]
        loop {
            let previous_position = parser.position;
            let marker = parser.peek_marker()?;
            if marker == SOT {
                break;
            }
            if crate::refinement::main_header_boundary(marker) {
                return Err(parser.malformed_here(MalformedKind::InvalidMarkerOrder));
            }
            let segment = parser.read_segment()?;
            main_header_segments = main_header_segments
                .checked_add(1)
                .ok_or_else(|| parser.malformed_here(MalformedKind::ArithmeticOverflow))?;
            if !is_known_segment_marker(segment.marker) {
                return Err(DecodeError::malformed(
                    parser.offset(segment.offset),
                    MalformedKind::InvalidMarker,
                ));
            }
            if !is_allowed_main_header(segment.marker) {
                return Err(DecodeError::malformed(
                    parser.offset(segment.offset),
                    MalformedKind::InvalidMarkerOrder,
                ));
            }
            match segment.marker {
                CAP if extended_capabilities.is_some() => {
                    return Err(DecodeError::malformed(
                        parser.offset(segment.offset),
                        MalformedKind::InvalidMarkerOrder,
                    ));
                }
                CAP => extended_capabilities = Some(parser.parse_cap(segment)?),
                PRF if profile.is_some() => {
                    return Err(DecodeError::malformed(
                        parser.offset(segment.offset),
                        MalformedKind::InvalidMarkerOrder,
                    ));
                }
                PRF => profile = Some(parser.parse_profile_number(segment)?),
                CPF if corresponding_profile.is_some() => {
                    return Err(DecodeError::malformed(
                        parser.offset(segment.offset),
                        MalformedKind::InvalidMarkerOrder,
                    ));
                }
                CPF => corresponding_profile = Some(parser.parse_profile_number(segment)?),
                COD if coding_style.is_some() => {
                    return Err(DecodeError::malformed(
                        parser.offset(segment.offset),
                        MalformedKind::InvalidMarkerOrder,
                    ));
                }
                COD => coding_style = Some(parser.parse_cod(segment)?),
                COC => {
                    let (component, _) = parser.parse_component_index(
                        segment,
                        image_size.components,
                        MalformedKind::InvalidCodingStyle,
                    )?;
                    let slot = coc_segments
                        .get_mut(usize::from(component))
                        .ok_or_else(|| {
                            DecodeError::malformed(
                                parser.offset(segment.offset),
                                MalformedKind::InvalidCodingStyle,
                            )
                        })?;
                    if slot.is_some() {
                        return Err(DecodeError::malformed(
                            parser.offset(segment.offset),
                            MalformedKind::InvalidMarkerOrder,
                        ));
                    }
                    *slot = Some(segment);
                }
                QCD if quantization.is_some() => {
                    return Err(DecodeError::malformed(
                        parser.offset(segment.offset),
                        MalformedKind::InvalidMarkerOrder,
                    ));
                }
                QCD => {
                    let style = coding_style.ok_or_else(|| {
                        DecodeError::malformed(
                            parser.offset(segment.offset),
                            MalformedKind::InvalidMarkerOrder,
                        )
                    })?;
                    quantization = Some(parser.parse_qcd(segment, style)?);
                }
                QCC => {
                    let (component, _) = parser.parse_component_index(
                        segment,
                        image_size.components,
                        MalformedKind::InvalidQuantization,
                    )?;
                    let slot = qcc_segments
                        .get_mut(usize::from(component))
                        .ok_or_else(|| {
                            DecodeError::malformed(
                                parser.offset(segment.offset),
                                MalformedKind::InvalidQuantization,
                            )
                        })?;
                    if slot.is_some() {
                        return Err(DecodeError::malformed(
                            parser.offset(segment.offset),
                            MalformedKind::InvalidMarkerOrder,
                        ));
                    }
                    *slot = Some(segment);
                }
                RGN => {
                    let (component, shift) = parser.parse_rgn(segment, image_size.components)?;
                    let index = usize::from(component);
                    if index >= roi_seen.len() || roi_seen[index] {
                        return Err(DecodeError::malformed(
                            parser.offset(segment.offset),
                            MalformedKind::InvalidMarkerOrder,
                        ));
                    }
                    roi_seen[index] = true;
                    roi_shifts.set(component, shift)?;
                }
                PLM => {
                    plm_seen = true;
                    parser.parse_plm(segment, &mut next_plm_index, &mut plm_groups)?;
                }
                PPM => {
                    parser.parse_packed_fragment(
                        segment,
                        PPM,
                        &mut next_ppm_index,
                        &mut ppm_fragments,
                        &mut ppm_bytes,
                    )?;
                }
                POC => {
                    if poc_seen {
                        return Err(DecodeError::malformed(
                            parser.offset(segment.offset),
                            MalformedKind::InvalidMarkerOrder,
                        ));
                    }
                    poc_seen = true;
                    progression_changes = parser.parse_poc(segment, image_size.components)?;
                }
                TLM => parser.parse_tlm(
                    segment,
                    &mut next_tlm_index,
                    image_size.tile_count()?,
                    &mut part2_extensions,
                )?,
                CRG => parser.parse_crg(segment, image_size.components, &mut part2_extensions)?,
                DFS => parser.parse_dfs(segment, &mut part2_extensions)?,
                ADS => parser.parse_ads(segment, &mut part2_extensions)?,
                NLT => parser.parse_nlt(segment, image_size.components, &mut part2_extensions)?,
                CBD => parser.parse_cbd(segment, image_size.components, &mut part2_extensions)?,
                ATK => parser.parse_atk(segment, &mut part2_extensions)?,
                MCT => {
                    if let Some(array) = parser.consume_mct_fragment(
                        segment,
                        &mut pending_mct,
                        &mut completed_mct,
                    )? {
                        mct_arrays.push(array);
                        validate_unique_mct_array_indexes(&mct_arrays, segment.offset)?;
                    }
                }
                MCC => {
                    if let Some(record) = parser.consume_mcc_fragment(
                        segment,
                        image_size.components,
                        &mut pending_mcc,
                        &mut completed_mcc,
                    )? {
                        mct_records.push(record);
                        validate_unique_mct_record_indexes(&mct_records, segment.offset)?;
                    }
                }
                MCO => {
                    if mct_order.is_some() {
                        return Err(DecodeError::malformed(
                            parser.offset(segment.offset),
                            MalformedKind::InvalidMarkerOrder,
                        ));
                    }
                    mct_order = Some(parser.parse_mco(segment)?);
                }
                0xff64 => {}
                marker => {
                    let _ = marker;
                    return Err(DecodeError::malformed(
                        parser.offset(segment.offset),
                        MalformedKind::InvalidMarker,
                    ));
                }
            }
            if !crate::refinement::parser_progress(previous_position, parser.position, input_length)
            {
                return Err(parser.malformed_here(MalformedKind::InvalidMarkerLength));
            }
        }
        if !pending_mct.is_empty() || !pending_mcc.is_empty() {
            return Err(parser.malformed_here(MalformedKind::MissingRequiredStructure));
        }
        if !part2_extensions.component_bit_depths.is_empty() {
            let mut component = 0_u16;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant component <= image_size.components,
                    decreases image_size.components - component,
                )
            )]
            while component < image_size.components {
                let depth = *part2_extensions
                    .component_bit_depths
                    .get(usize::from(component))
                    .ok_or_else(|| {
                        parser.malformed_here(MalformedKind::MissingRequiredStructure)
                    })?;
                let info = image_size
                    .component_info
                    .get_mut(usize::from(component))
                    .ok_or_else(|| parser.malformed_here(MalformedKind::InvalidImageGeometry))?;
                info.precision = depth.precision;
                info.signed = depth.signed;
                component = component
                    .checked_add(1)
                    .ok_or_else(|| parser.malformed_here(MalformedKind::ArithmeticOverflow))?;
            }
            let first = image_size.component(0)?;
            let uniform = (1..image_size.components).all(|component| {
                matches!(
                    image_size.component(component),
                    Ok(info) if info.precision == first.precision && info.signed == first.signed
                )
            });
            image_size.uniform_bit_depth = if uniform { Some(first.precision) } else { None };
            image_size.uniform_signed = if uniform { Some(first.signed) } else { None };
        }
        let main_header_end = parser.position;
        let ppm_frames = parser.parse_ppm_frames(&ppm_bytes, ppm_fragments)?;
        let main_header = parser
            .input
            .get(main_header_start..main_header_end)
            .ok_or_else(|| parser.malformed_here(MalformedKind::InvalidPackedPacketHeader))?;

        let coding_style = coding_style
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))?;
        let quantization = quantization
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))?;
        let mut component_coding_styles =
            ComponentCodingStyles::new(image_size.components, coding_style)?;
        let mut component = 0_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= image_size.components,
                decreases image_size.components - component,
            )
        )]
        while component < image_size.components {
            let coc_segment = match coc_segments.get(usize::from(component)) {
                Some(segment) => *segment,
                None => None,
            };
            if let Some(segment) = coc_segment {
                let (_, style) = parser.parse_coc(segment, image_size.components, coding_style)?;
                component_coding_styles.set(component, style)?;
            }
            component = component
                .checked_add(1)
                .ok_or_else(|| parser.malformed_here(MalformedKind::ArithmeticOverflow))?;
        }
        parser.resolve_decomposition_structures(&mut component_coding_styles, &part2_extensions)?;
        let mut component_quantizations =
            ComponentQuantizations::new(image_size.components, quantization)?;
        component = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= image_size.components,
                decreases image_size.components - component,
            )
        )]
        while component < image_size.components {
            let qcc_segment = match qcc_segments.get(usize::from(component)) {
                Some(segment) => *segment,
                None => None,
            };
            if let Some(segment) = qcc_segment {
                let style = component_coding_styles.style(component)?;
                let (_, component_quantization) =
                    parser.parse_qcc(segment, image_size.components, style)?;
                component_quantizations.set(component, component_quantization)?;
            }
            if !component_quantizations
                .quantization(component)?
                .is_compatible_with(component_coding_styles.style(component)?)
            {
                return Err(DecodeError::malformed(
                    0,
                    MalformedKind::InvalidQuantization,
                ));
            }
            component = component
                .checked_add(1)
                .ok_or_else(|| parser.malformed_here(MalformedKind::ArithmeticOverflow))?;
        }
        parser.validate_progression_changes(
            &progression_changes,
            coding_style.layers,
            &component_coding_styles,
            0,
        )?;
        if (!mct_arrays.is_empty() || !mct_records.is_empty()) && mct_order.is_none() {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::MissingRequiredStructure,
            ));
        }
        if !part2_extensions.component_registrations.is_empty() {
            let mut component = 0_u16;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant component <= image_size.components,
                    decreases image_size.components - component,
                )
            )]
            while component < image_size.components {
                let registration = *part2_extensions
                    .component_registrations
                    .get(usize::from(component))
                    .ok_or_else(|| {
                        parser.malformed_here(MalformedKind::MissingRequiredStructure)
                    })?;
                let info = image_size
                    .component_info
                    .get_mut(usize::from(component))
                    .ok_or_else(|| parser.malformed_here(MalformedKind::InvalidImageGeometry))?;
                info.x_registration = registration.horizontal;
                info.y_registration = registration.vertical;
                component = component
                    .checked_add(1)
                    .ok_or_else(|| parser.malformed_here(MalformedKind::ArithmeticOverflow))?;
            }
        }
        let main_mct_parameters = MctParameters {
            arrays: mct_arrays,
            records: mct_records,
            order: mct_order.unwrap_or_default(),
        };
        let multiple_component_transform =
            parser.resolve_multiple_component_transform(&main_mct_parameters)?;
        let tile_count = image_size.tile_count()?;
        let mut tile_parts = 0_u32;
        let mut tile_part_payloads = Vec::new();
        let tile_state_count = usize::try_from(tile_count)
            .map_err(|_| parser.malformed_here(MalformedKind::ArithmeticOverflow))?;
        let mut tile_roi_shifts = vec![None; tile_state_count];
        let mut tile_coding_styles = vec![None; tile_state_count];
        let mut tile_quantizations = vec![None; tile_state_count];
        let mut tile_component_coding_styles = vec![None; tile_state_count];
        let mut tile_component_quantizations: Vec<Option<ComponentQuantizations>> =
            vec![None; tile_state_count];
        let mut tile_progression_changes: Vec<Option<ProgressionChanges>> =
            vec![None; tile_state_count];
        let mut tile_mct_parameters: Vec<Option<MctParameters>> = vec![None; tile_state_count];
        let mut tile_part2_extensions: Vec<Option<Part2Extensions>> = vec![None; tile_state_count];

        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases input_length as int - parser.position as int,)
        )]
        loop {
            let previous_position = parser.position;
            match parser.peek_marker()? {
                SOT => {
                    tile_parts = tile_parts
                        .checked_add(1)
                        .ok_or_else(|| parser.malformed_here(MalformedKind::ArithmeticOverflow))?;
                    if tile_parts > parser.limits.maximum_tile_parts() {
                        return Err(DecodeError::limit(
                            Resource::TileParts,
                            u64::from(parser.limits.maximum_tile_parts()),
                            u64::from(tile_parts),
                        ));
                    }
                    let tile_part = parser.read_sot(tile_count)?;
                    if !part2_extensions.tile_part_lengths.is_empty() {
                        let entry_index = usize::try_from(tile_parts - 1).map_err(|_| {
                            parser.malformed_here(MalformedKind::ArithmeticOverflow)
                        })?;
                        let entry = part2_extensions
                            .tile_part_lengths
                            .get(entry_index)
                            .ok_or_else(|| {
                                DecodeError::malformed(
                                    parser.offset(tile_part.start),
                                    MalformedKind::InvalidTilePart,
                                )
                            })?;
                        if entry.length != tile_part.length
                            || matches!(entry.tile_index, Some(index) if index != tile_part.tile_index)
                        {
                            return Err(DecodeError::malformed(
                                parser.offset(tile_part.start),
                                MalformedKind::InvalidTilePart,
                            ));
                        }
                    }
                    let main_packet_lengths = if plm_seen {
                        *plm_groups
                            .get(usize::try_from(tile_parts - 1).map_err(|_| {
                                parser.malformed_here(MalformedKind::ArithmeticOverflow)
                            })?)
                            .ok_or_else(|| {
                                DecodeError::malformed(
                                    parser.offset(tile_part.start),
                                    MalformedKind::InvalidPacketLength,
                                )
                            })?
                    } else {
                        PacketLengths::none()
                    };
                    let packed_packet_headers = if ppm_fragments != 0 {
                        let frame = *ppm_frames
                            .get(usize::try_from(tile_parts - 1).map_err(|_| {
                                parser.malformed_here(MalformedKind::ArithmeticOverflow)
                            })?)
                            .ok_or_else(|| {
                                DecodeError::malformed(
                                    parser.offset(tile_part.start),
                                    MalformedKind::InvalidPackedPacketHeader,
                                )
                            })?;
                        PackedPacketHeaders {
                            source: PackedPacketHeaderSource::MarkerSegments {
                                header: main_header,
                                marker: PPM,
                                logical_start: frame.start,
                            },
                            length: frame.length,
                            fragments: ppm_fragments,
                        }
                    } else {
                        PackedPacketHeaders::none()
                    };
                    let tile_index = usize::from(tile_part.tile_index);
                    let previous_roi_shifts = match tile_roi_shifts.get(tile_index) {
                        Some(state) => *state,
                        None => {
                            return Err(parser.malformed_here(MalformedKind::InvalidTilePart));
                        }
                    };
                    let inherited_roi_shifts = match previous_roi_shifts {
                        Some(shifts) => shifts,
                        None => roi_shifts,
                    };
                    let inherited_default_style = match tile_coding_styles.get(tile_index) {
                        Some(Some(style)) => *style,
                        _ => coding_style,
                    };
                    let inherited_default_quantization = match tile_quantizations.get(tile_index) {
                        Some(Some(value)) => *value,
                        _ => quantization,
                    };
                    let inherited_component_styles =
                        match tile_component_coding_styles.get(tile_index) {
                            Some(Some(styles)) => *styles,
                            _ => component_coding_styles,
                        };
                    let inherited_component_quantizations =
                        match tile_component_quantizations.get(tile_index) {
                            Some(Some(quantizations)) => quantizations.clone(),
                            _ => component_quantizations.clone(),
                        };
                    let inherited_progression_changes =
                        match tile_progression_changes.get(tile_index) {
                            Some(Some(changes)) => changes.clone(),
                            _ => progression_changes.clone(),
                        };
                    let inherited_mct_parameters = match tile_mct_parameters.get(tile_index) {
                        Some(Some(parameters)) => parameters.clone(),
                        _ => main_mct_parameters.clone(),
                    };
                    let inherited_part2_extensions = match tile_part2_extensions.get(tile_index) {
                        Some(Some(extensions)) => extensions.clone(),
                        _ => part2_extensions.clone(),
                    };
                    let (payload, tile_parameters, tile_extensions) = parser.read_tile_part(
                        tile_part,
                        (inherited_default_style, inherited_default_quantization),
                        inherited_component_styles,
                        inherited_component_quantizations,
                        inherited_roi_shifts,
                        inherited_progression_changes,
                        inherited_mct_parameters,
                        inherited_part2_extensions,
                        image_size.components,
                        main_packet_lengths,
                        packed_packet_headers,
                    )?;
                    let roi_consistent = match previous_roi_shifts {
                        Some(previous) => previous.equals(&payload.roi_shifts),
                        None => true,
                    };
                    if !roi_consistent {
                        return Err(DecodeError::malformed(
                            parser.offset(tile_part.start),
                            MalformedKind::InvalidRegionOfInterest,
                        ));
                    }
                    let state = tile_roi_shifts
                        .get_mut(tile_index)
                        .ok_or_else(|| parser.malformed_here(MalformedKind::InvalidTilePart))?;
                    *state = Some(payload.roi_shifts);
                    *tile_coding_styles
                        .get_mut(tile_index)
                        .ok_or_else(|| parser.malformed_here(MalformedKind::InvalidTilePart))? =
                        Some(payload.coding_style);
                    *tile_quantizations
                        .get_mut(tile_index)
                        .ok_or_else(|| parser.malformed_here(MalformedKind::InvalidTilePart))? =
                        Some(payload.quantization);
                    *tile_component_coding_styles
                        .get_mut(tile_index)
                        .ok_or_else(|| parser.malformed_here(MalformedKind::InvalidTilePart))? =
                        Some(payload.component_coding_styles);
                    *tile_component_quantizations
                        .get_mut(tile_index)
                        .ok_or_else(|| parser.malformed_here(MalformedKind::InvalidTilePart))? =
                        Some(payload.component_quantizations.clone());
                    *tile_progression_changes
                        .get_mut(tile_index)
                        .ok_or_else(|| parser.malformed_here(MalformedKind::InvalidTilePart))? =
                        Some(payload.progression_changes.clone());
                    *tile_mct_parameters
                        .get_mut(tile_index)
                        .ok_or_else(|| parser.malformed_here(MalformedKind::InvalidTilePart))? =
                        Some(tile_parameters);
                    *tile_part2_extensions
                        .get_mut(tile_index)
                        .ok_or_else(|| parser.malformed_here(MalformedKind::InvalidTilePart))? =
                        Some(tile_extensions);
                    tile_part_payloads.push(payload);
                }
                EOC => {
                    if tile_parts == 0 {
                        return Err(parser.malformed_here(MalformedKind::InvalidMarkerOrder));
                    }
                    parser.expect_standalone(EOC)?;
                    if plm_seen && plm_groups.len() != tile_parts as usize {
                        return Err(parser.malformed_here(MalformedKind::InvalidPacketLength));
                    }
                    if ppm_fragments != 0 && ppm_frames.len() != tile_parts as usize {
                        return Err(parser.malformed_here(MalformedKind::InvalidPackedPacketHeader));
                    }
                    if !part2_extensions.tile_part_lengths.is_empty()
                        && part2_extensions.tile_part_lengths.len() != tile_parts as usize
                    {
                        return Err(parser.malformed_here(MalformedKind::InvalidTilePart));
                    }
                    if parser.position != parser.input.len() {
                        return Err(parser.malformed_here(MalformedKind::TrailingData));
                    }
                    break;
                }
                _ => return Err(parser.malformed_here(MalformedKind::InvalidMarkerOrder)),
            }
            if !crate::refinement::parser_progress(previous_position, parser.position, input_length)
            {
                return Err(parser.malformed_here(MalformedKind::InvalidTilePart));
            }
        }

        let codestream_profile = classify_codestream_profile(
            r_siz,
            extended_capabilities,
            profile,
            corresponding_profile,
            &component_coding_styles,
            &roi_shifts,
            &tile_part_payloads,
            image_size.components,
            roi_seen.iter().any(|seen| *seen),
        )?;
        Ok(CodestreamLayout {
            info: CodestreamInfo {
                image_size,
                coding_style,
                quantization,
                component_coding_styles,
                component_quantizations,
                roi_shifts,
                progression_changes,
                multiple_component_transform,
                part2_extensions,
                profile: codestream_profile,
                main_header_segments,
                tile_parts,
                consumed_bytes: parser.offset(parser.position),
            },
            tile_parts: tile_part_payloads,
        })
    }

    fn charge_extension_metadata(
        &mut self,
        segment: Segment<'a>,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        let bytes = (segment.payload.len() as u64)
            .checked_add(4)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        extensions.metadata_bytes = extensions
            .metadata_bytes
            .checked_add(bytes)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        if extensions.metadata_bytes > self.limits.maximum_metadata_bytes() {
            return Err(DecodeError::limit(
                Resource::MetadataBytes,
                self.limits.maximum_metadata_bytes(),
                extensions.metadata_bytes,
            ));
        }
        self.charge_work(bytes)
    }

    fn segment_byte(
        &self,
        segment: Segment<'a>,
        position: usize,
        relative: usize,
    ) -> Result<u8, DecodeError> {
        let index = position
            .checked_add(relative)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        match segment.payload.get(index) {
            Some(value) => Ok(*value),
            None => Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            )),
        }
    }

    fn parse_tlm(
        &mut self,
        segment: Segment<'a>,
        next_index: &mut u16,
        tile_count: u64,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        self.charge_extension_metadata(segment, extensions)?;
        if segment.payload.len() < 2 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let index = u16::from(segment.payload[0]);
        if index != *next_index {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerOrder,
            ));
        }
        *next_index = next_index.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        let style = segment.payload[1];
        let tile_bytes = crate::refinement::tlm_tile_index_bytes(style);
        let length_bytes = crate::refinement::tlm_length_bytes(style);
        if style & 0x8f != 0 || tile_bytes == 3 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTilePart,
            ));
        }
        let record_bytes = tile_bytes
            .checked_add(length_bytes)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        let records = segment.payload.len() - 2;
        if record_bytes == 0 || !records.is_multiple_of(record_bytes) {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let mut position = 2_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= segment.payload.len(),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            let tile_index = if tile_bytes == 0 {
                None
            } else if tile_bytes == 1 {
                Some(u16::from(self.segment_byte(segment, position, 0)?))
            } else {
                Some(read_u16([
                    self.segment_byte(segment, position, 0)?,
                    self.segment_byte(segment, position, 1)?,
                ]))
            };
            if matches!(tile_index, Some(value) if u64::from(value) >= tile_count) {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTilePart,
                ));
            }
            let length_position = position
                .checked_add(tile_bytes)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            let length = if length_bytes == 2 {
                u32::from(read_u16([
                    self.segment_byte(segment, length_position, 0)?,
                    self.segment_byte(segment, length_position, 1)?,
                ]))
            } else {
                read_u32([
                    self.segment_byte(segment, length_position, 0)?,
                    self.segment_byte(segment, length_position, 1)?,
                    self.segment_byte(segment, length_position, 2)?,
                    self.segment_byte(segment, length_position, 3)?,
                ])
            };
            if length < 14 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTilePart,
                ));
            }
            extensions
                .tile_part_lengths
                .push(TilePartLength { tile_index, length });
            let next_position = position
                .checked_add(record_bytes)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            if next_position <= position || next_position > segment.payload.len() {
                return Err(self.malformed_here(MalformedKind::InvalidMarkerLength));
            }
            position = next_position;
        }
        Ok(())
    }

    fn resolve_decomposition_structures(
        &self,
        styles: &mut ComponentCodingStyles,
        extensions: &Part2Extensions,
    ) -> Result<(), DecodeError> {
        let mut component = 0_u16;
        let components = styles.components;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= components,
                decreases components - component,
            )
        )]
        while component < components {
            let mut style = styles.style(component)?;
            if let Some(index) = style.decomposition_structure {
                let Some(structure) = extensions.decomposition_structure(index) else {
                    let decomposition =
                        extensions.arbitrary_decomposition(index).ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::MissingRequiredStructure)
                        })?;
                    if decomposition
                        .sublevel_depths
                        .iter()
                        .any(|depth| *depth == 0 || *depth > 3)
                    {
                        return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
                    }
                    styles.set(component, style)?;
                    component = component
                        .checked_add(1)
                        .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
                    continue;
                };
                if structure.directions.is_empty() {
                    return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
                }
                let mut stage = 0_u8;
                let levels = style.decomposition_levels;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant stage <= levels,
                        decreases levels - stage,
                    )
                )]
                while stage < levels {
                    let available = structure.directions.len();
                    let requested = usize::from(stage);
                    let last = available.checked_sub(1).ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::InvalidCodingStyle)
                    })?;
                    let direction_index = requested.min(last);
                    let direction =
                        *structure.directions.get(direction_index).ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::InvalidCodingStyle)
                        })?;
                    *style
                        .decomposition_directions
                        .get_mut(requested)
                        .ok_or_else(|| {
                            DecodeError::malformed(0, MalformedKind::InvalidCodingStyle)
                        })? = direction;
                    stage = stage.checked_add(1).ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                    })?;
                }
                styles.set(component, style)?;
            }
            component = component
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        }
        Ok(())
    }

    fn parse_crg(
        &mut self,
        segment: Segment<'a>,
        components: u16,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        self.charge_extension_metadata(segment, extensions)?;
        let expected = crate::refinement::crg_payload_bytes(components);
        if segment.payload.len() != expected || !extensions.component_registrations.is_empty() {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                if segment.payload.len() == expected {
                    MalformedKind::InvalidMarkerOrder
                } else {
                    MalformedKind::InvalidMarkerLength
                },
            ));
        }
        let mut position = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= segment.payload.len(),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            extensions
                .component_registrations
                .push(ComponentRegistration {
                    horizontal: read_u16([
                        self.segment_byte(segment, position, 0)?,
                        self.segment_byte(segment, position, 1)?,
                    ]),
                    vertical: read_u16([
                        self.segment_byte(segment, position, 2)?,
                        self.segment_byte(segment, position, 3)?,
                    ]),
                });
            let next_position = position
                .checked_add(4)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            if next_position > segment.payload.len() {
                return Err(self.malformed_here(MalformedKind::InvalidMarkerLength));
            }
            position = next_position;
        }
        Ok(())
    }

    fn parse_dfs(
        &mut self,
        segment: Segment<'a>,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        self.charge_extension_metadata(segment, extensions)?;
        if segment.payload.len() < 3 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let descriptor = read_u16([segment.payload[0], segment.payload[1]]);
        let index = u8::try_from(descriptor & 0x0f)
            .map_err(|_| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        let levels = segment.payload[2];
        let packed = usize::from(levels)
            .checked_add(3)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?
            / 4;
        let expected = 3_usize
            .checked_add(packed)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        if descriptor & !0x000f != 0 || index == 0 || levels == 0 || levels > 32 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidCodingStyle,
            ));
        }
        if segment.payload.len() != expected {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let mut directions = Vec::with_capacity(usize::from(levels));
        let mut level = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant level <= levels as usize,
                decreases levels as usize - level,
            )
        )]
        while level < usize::from(levels) {
            let byte = self.segment_byte(segment, 3, level / 4)?;
            let slot = u8::try_from(level % 4)
                .map_err(|_| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            directions.push(match crate::refinement::dfs_direction_code(byte, slot) {
                0 => DecompositionDirection::None,
                1 => DecompositionDirection::TwoDimensional,
                2 => DecompositionDirection::Horizontal,
                _ => DecompositionDirection::Vertical,
            });
            level = level
                .checked_add(1)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        }
        let replacement = DecompositionStructure { index, directions };
        if let Some(existing) = extensions
            .decomposition_structures
            .iter_mut()
            .find(|structure| structure.index == index)
        {
            *existing = replacement;
        } else {
            extensions.decomposition_structures.push(replacement);
        }
        Ok(())
    }

    fn parse_ads(
        &mut self,
        segment: Segment<'a>,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        self.charge_extension_metadata(segment, extensions)?;
        if segment.payload.len() < 3 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let index = segment.payload[0];
        if index == 0 || index > 127 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidCodingStyle,
            ));
        }
        let depth_count = usize::from(segment.payload[1]);
        let depth_bytes = crate::refinement::packed_two_bit_bytes(depth_count);
        let split_count_position = 2_usize
            .checked_add(depth_bytes)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        let split_count =
            usize::from(*segment.payload.get(split_count_position).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                )
            })?);
        let split_bytes = crate::refinement::packed_two_bit_bytes(split_count);
        let expected = split_count_position
            .checked_add(1)
            .and_then(|value| value.checked_add(split_bytes))
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        if segment.payload.len() != expected {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let sublevel_depths = self.unpack_two_bit_values(segment, 2, depth_count, false)?;
        let split_values =
            self.unpack_two_bit_values(segment, split_count_position + 1, split_count, true)?;
        let mut subband_splits = Vec::with_capacity(split_values.len());
        let mut split = 0_usize;
        #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant split <= split_values.len(),
                    decreases split_values.len() - split,
                )
            )]
        while split < split_values.len() {
            subband_splits.push(match split_values[split] {
                0 => DecompositionDirection::None,
                1 => DecompositionDirection::TwoDimensional,
                2 => DecompositionDirection::Horizontal,
                _ => DecompositionDirection::Vertical,
            });
            split = split
                .checked_add(1)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        }
        let replacement = ArbitraryDecomposition {
            index,
            sublevel_depths,
            subband_splits,
        };
        if let Some(existing) = extensions
            .arbitrary_decompositions
            .iter_mut()
            .find(|record| record.index == index)
        {
            *existing = replacement;
        } else {
            extensions.arbitrary_decompositions.push(replacement);
        }
        Ok(())
    }

    fn unpack_two_bit_values(
        &self,
        segment: Segment<'a>,
        start: usize,
        count: usize,
        allow_terminal: bool,
    ) -> Result<Vec<u8>, DecodeError> {
        let mut values = Vec::with_capacity(count);
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= count,
                decreases count - index,
            )
        )]
        while index < count {
            let byte = self.segment_byte(segment, start, index / 4)?;
            let shift = 6_u32
                .checked_sub(
                    2_u32
                        .checked_mul(
                            u32::try_from(index % 4).map_err(|_| {
                                self.malformed_here(MalformedKind::ArithmeticOverflow)
                            })?,
                        )
                        .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?,
                )
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            let value = (byte >> shift) & 3;
            if !allow_terminal && value == 0 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidCodingStyle,
                ));
            }
            values.push(value);
            index = index
                .checked_add(1)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        }
        if !count.is_multiple_of(4) {
            let last = self.segment_byte(segment, start, count / 4)?;
            let unused = 2_u32
                .checked_mul(
                    u32::try_from(4 - (count % 4))
                        .map_err(|_| self.malformed_here(MalformedKind::ArithmeticOverflow))?,
                )
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            let mask = (1_u8 << unused).wrapping_sub(1);
            if last & mask != 0 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidCodingStyle,
                ));
            }
        }
        Ok(values)
    }

    fn parse_nlt(
        &mut self,
        segment: Segment<'a>,
        components: u16,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        self.charge_extension_metadata(segment, extensions)?;
        if segment.payload.len() < 4 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let raw_component = read_u16([segment.payload[0], segment.payload[1]]);
        let component = if raw_component == u16::MAX {
            None
        } else if raw_component < components {
            Some(raw_component)
        } else {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        };
        let precision = (segment.payload[2] & 0x7f)
            .checked_add(1)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        let transform_type = segment.payload[3];
        if precision > 38 || transform_type > 4 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        if matches!(transform_type, 0 | 3) && segment.payload.len() != 4 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let mut d_min = None;
        let mut d_max = None;
        let mut point_bits = None;
        let mut points = Vec::new();
        let mut gamma_parameters = None;
        if transform_type == 1 {
            if segment.payload.len() != 19 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            let mut parameters = [0_u32; 5];
            let mut parameter = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant parameter <= parameters.len(),
                    decreases parameters.len() - parameter,
                )
            )]
            while parameter < parameters.len() {
                let position =
                    4_usize
                        .checked_add(parameter.checked_mul(3).ok_or_else(|| {
                            self.malformed_here(MalformedKind::ArithmeticOverflow)
                        })?)
                        .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
                parameters[parameter] = (u32::from(self.segment_byte(segment, position, 0)?) << 16)
                    | (u32::from(self.segment_byte(segment, position, 1)?) << 8)
                    | u32::from(self.segment_byte(segment, position, 2)?);
                parameter = parameter
                    .checked_add(1)
                    .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            }
            if parameters[0] == 0 || parameters[1] == 0 || parameters[3] == 0 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            gamma_parameters = Some(parameters);
        }
        if matches!(transform_type, 2 | 4) {
            if segment.payload.len() < 15 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            let stored_points = read_u16([
                self.segment_byte(segment, 4, 0)?,
                self.segment_byte(segment, 4, 1)?,
            ]);
            if stored_points == 0 || stored_points > 8191 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            let count = stored_points
                .checked_add(1)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            let minimum = read_u32([
                self.segment_byte(segment, 6, 0)?,
                self.segment_byte(segment, 6, 1)?,
                self.segment_byte(segment, 6, 2)?,
                self.segment_byte(segment, 6, 3)?,
            ]);
            let maximum = read_u32([
                self.segment_byte(segment, 10, 0)?,
                self.segment_byte(segment, 10, 1)?,
                self.segment_byte(segment, 10, 2)?,
                self.segment_byte(segment, 10, 3)?,
            ]);
            let bits = segment.payload[14];
            if minimum >= maximum || bits == 0 || bits > 32 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            let width = if bits <= 8 {
                1_usize
            } else if bits <= 16 {
                2_usize
            } else {
                4_usize
            };
            let point_bytes = usize::from(count)
                .checked_mul(width)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            let expected = 15_usize
                .checked_add(point_bytes)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            if segment.payload.len() != expected {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            let mut point = 0_u16;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant point <= count,
                    decreases count - point,
                )
            )]
            while point < count {
                let position =
                    15_usize
                        .checked_add(usize::from(point).checked_mul(width).ok_or_else(|| {
                            self.malformed_here(MalformedKind::ArithmeticOverflow)
                        })?)
                        .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
                let value = if width == 1 {
                    u32::from(self.segment_byte(segment, position, 0)?)
                } else if width == 2 {
                    u32::from(read_u16([
                        self.segment_byte(segment, position, 0)?,
                        self.segment_byte(segment, position, 1)?,
                    ]))
                } else {
                    read_u32([
                        self.segment_byte(segment, position, 0)?,
                        self.segment_byte(segment, position, 1)?,
                        self.segment_byte(segment, position, 2)?,
                        self.segment_byte(segment, position, 3)?,
                    ])
                };
                let maximum_point = if bits == 32 {
                    u32::MAX
                } else {
                    maximum_u32_for_bits(bits)?
                };
                if value > maximum_point {
                    return Err(DecodeError::malformed(
                        self.offset(segment.offset),
                        MalformedKind::InvalidTransform,
                    ));
                }
                points.push(value);
                point = point
                    .checked_add(1)
                    .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            }
            d_min = Some(minimum);
            d_max = Some(maximum);
            point_bits = Some(bits);
        }
        let replacement = NonlinearPointTransform {
            component,
            precision,
            signed: segment.payload[2] & 0x80 != 0,
            transform_type,
            parameters: self.copy_segment_tail(segment, 4)?,
            d_min,
            d_max,
            point_bits,
            points,
            gamma_parameters,
        };
        if let Some(existing) = extensions
            .nonlinear_transforms
            .iter_mut()
            .find(|record| record.component == component)
        {
            *existing = replacement;
        } else {
            extensions.nonlinear_transforms.push(replacement);
        }
        Ok(())
    }

    fn parse_cbd(
        &mut self,
        segment: Segment<'a>,
        components: u16,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        self.charge_extension_metadata(segment, extensions)?;
        if segment.payload.len() < 2 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let descriptor = read_u16([segment.payload[0], segment.payload[1]]);
        let repeated = descriptor & 0x8000 != 0;
        let count = descriptor & 0x7fff;
        let descriptor_count = if repeated {
            1_usize
        } else {
            usize::from(count)
        };
        let expected = 2_usize
            .checked_add(descriptor_count)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        if count != components || segment.payload.len() != expected {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        extensions.component_bit_depths.clear();
        let mut position = 2_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= segment.payload.len(),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            let packed = *segment.payload.get(position).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                )
            })?;
            let precision = crate::refinement::cbd_precision(packed);
            if precision > 38 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            extensions.component_bit_depths.push(ComponentBitDepth {
                precision,
                signed: crate::refinement::cbd_signed(packed),
            });
            position = position
                .checked_add(1)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        }
        if repeated {
            let depth = *extensions.component_bit_depths.first().ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                )
            })?;
            let component_count = usize::from(components);
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    decreases component_count as int
                        - extensions.component_bit_depths.len() as int,
                )
            )]
            while extensions.component_bit_depths.len() < component_count {
                extensions.component_bit_depths.push(depth);
            }
        }
        Ok(())
    }

    fn parse_atk(
        &mut self,
        segment: Segment<'a>,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        self.charge_extension_metadata(segment, extensions)?;
        if segment.payload.len() < 7 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let descriptor = read_u16([segment.payload[0], segment.payload[1]]);
        let index = u8::try_from(descriptor & 0xff)
            .map_err(|_| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        let coefficient_type = u8::try_from((descriptor >> 8) & 7)
            .map_err(|_| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        if index < 2 || coefficient_type > 4 || descriptor & 0x8000 != 0 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let width = match coefficient_type {
            0 => 1_usize,
            1 => 2,
            2 => 4,
            3 => 8,
            4 => 16,
            _ => {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
        };
        let whole_sample_filter = descriptor & 0x0800 != 0;
        let reversible = descriptor & 0x1000 != 0;
        let initial_odd = descriptor & 0x2000 != 0;
        let whole_sample_extension = descriptor & 0x4000 != 0;
        let mut position = 2_usize;
        let scaling = if reversible {
            None
        } else {
            let coefficient =
                self.read_kernel_coefficient(segment, position, width, coefficient_type)?;
            position = position
                .checked_add(width)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            Some(coefficient)
        };
        let step_count = usize::from(self.segment_byte(segment, position, 0)?);
        position = position
            .checked_add(1)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        let mut steps = Vec::with_capacity(step_count);
        let mut step = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant step <= step_count,
                decreases step_count - step,
            )
        )]
        while step < step_count {
            let offset = if whole_sample_filter {
                0
            } else {
                self.segment_byte(segment, position, 0)? as i8
            };
            if !whole_sample_filter {
                position = position
                    .checked_add(1)
                    .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            }
            let exponent = if reversible {
                let value = self.segment_byte(segment, position, 0)?;
                position = position
                    .checked_add(1)
                    .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
                value
            } else {
                0
            };
            let residue = if reversible {
                let value =
                    self.read_kernel_coefficient(segment, position, width, coefficient_type)?;
                position = position
                    .checked_add(width)
                    .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
                Some(value)
            } else {
                None
            };
            let coefficient_count = usize::from(self.segment_byte(segment, position, 0)?);
            position = position
                .checked_add(1)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            if coefficient_count == 0 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            let mut coefficients = Vec::with_capacity(coefficient_count);
            let mut coefficient = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant coefficient <= coefficient_count,
                    decreases coefficient_count - coefficient,
                )
            )]
            while coefficient < coefficient_count {
                coefficients.push(self.read_kernel_coefficient(
                    segment,
                    position,
                    width,
                    coefficient_type,
                )?);
                position = position
                    .checked_add(width)
                    .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
                coefficient = coefficient
                    .checked_add(1)
                    .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            }
            steps.push(KernelLiftingStep {
                offset,
                exponent,
                residue,
                coefficients,
            });
            step = step
                .checked_add(1)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        }
        if position != segment.payload.len() {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let replacement = ArbitraryKernel {
            index,
            reversible,
            descriptor,
            coefficient_type,
            whole_sample_filter,
            initial_odd,
            whole_sample_extension,
            scaling,
            steps,
        };
        if let Some(existing) = extensions
            .arbitrary_kernels
            .iter_mut()
            .find(|kernel| kernel.index == index)
        {
            *existing = replacement;
        } else {
            extensions.arbitrary_kernels.push(replacement);
        }
        Ok(())
    }

    fn read_kernel_coefficient(
        &self,
        segment: Segment<'a>,
        position: usize,
        width: usize,
        coefficient_type: u8,
    ) -> Result<KernelCoefficient, DecodeError> {
        if width == 0 || width > 16 {
            return Err(self.malformed_here(MalformedKind::InvalidTransform));
        }
        let end = position
            .checked_add(width)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        let source = segment.payload.get(position..end).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            )
        })?;
        let mut bytes = [0_u8; 16];
        let start = bytes
            .len()
            .checked_sub(width)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        bytes[start..].copy_from_slice(source);
        let negative_integer = match source.first() {
            Some(byte) => *byte & 0x80 != 0,
            None => false,
        };
        if coefficient_type <= 1 && negative_integer {
            let mut fill = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant fill <= start,
                    decreases start - fill,
                )
            )]
            while fill < start {
                *bytes
                    .get_mut(fill)
                    .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))? = 0xff;
                fill = fill
                    .checked_add(1)
                    .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            }
        }
        Ok(KernelCoefficient {
            bytes,
            width: u8::try_from(width)
                .map_err(|_| self.malformed_here(MalformedKind::ArithmeticOverflow))?,
        })
    }

    fn copy_segment_tail(
        &self,
        segment: Segment<'a>,
        start: usize,
    ) -> Result<Vec<u8>, DecodeError> {
        if start > segment.payload.len() {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let mut copied = Vec::with_capacity(segment.payload.len() - start);
        let mut position = start;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= segment.payload.len(),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            copied.push(*segment.payload.get(position).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                )
            })?);
            position = position.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        }
        Ok(copied)
    }

    fn parse_cap(&self, segment: Segment<'a>) -> Result<ExtendedCapabilities, DecodeError> {
        if segment.payload.len() < 4 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let p_cap = read_u32([
            segment.payload[0],
            segment.payload[1],
            segment.payload[2],
            segment.payload[3],
        ]);
        if p_cap == 0 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidProfile,
            ));
        }
        let capability_count = count_set_bits(p_cap);
        let expected_length = crate::refinement::part15_cap_payload_length(capability_count);
        if segment.payload.len() != expected_length {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let mut c_capabilities = Vec::with_capacity(usize::from(capability_count));
        let mut position = 4_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= segment.payload.len(),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            let next = position.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            let first = *segment.payload.get(position).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                )
            })?;
            let second = *segment.payload.get(next).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                )
            })?;
            c_capabilities.push(read_u16([first, second]));
            position = position.checked_add(2).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        }
        Ok(ExtendedCapabilities {
            p_cap,
            c_capabilities,
        })
    }

    fn parse_profile_number(&self, segment: Segment<'a>) -> Result<ProfileNumber, DecodeError> {
        if segment.payload.len() < 2 || !segment.payload.len().is_multiple_of(2) {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let mut words = Vec::with_capacity(segment.payload.len() / 2);
        let mut position = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= segment.payload.len(),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            let next = position.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            let first = *segment.payload.get(position).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                )
            })?;
            let second = *segment.payload.get(next).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                )
            })?;
            words.push(read_u16([first, second]));
            position = position.checked_add(2).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        }
        if matches!(words.last(), Some(value) if *value == 0) {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidProfile,
            ));
        }
        Ok(ProfileNumber { words })
    }

    fn consume_mct_fragment(
        &self,
        segment: Segment<'a>,
        pending: &mut Vec<MctFragment>,
        completed: &mut Vec<u8>,
    ) -> Result<Option<MctArray>, DecodeError> {
        if segment.payload.len() < 4 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let fragment_index = read_u16([segment.payload[0], segment.payload[1]]);
        let descriptor = read_u16([segment.payload[2], segment.payload[3]]);
        let array_index = (descriptor & 0xff) as u8;
        if array_index == 0 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let maximum_bytes = u64::from(self.limits.maximum_components())
            .checked_mul(u64::from(self.limits.maximum_components()))
            .and_then(|value| value.checked_mul(8))
            .ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        let mut state_index = None;
        let mut search_index = 0_usize;
        #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant search_index <= pending.len(),
                    decreases pending.len() - search_index,
                )
            )]
        while search_index < pending.len() {
            let state = pending.get(search_index).ok_or_else(|| {
                DecodeError::malformed(self.offset(segment.offset), MalformedKind::InvalidTransform)
            })?;
            if (state.descriptor & 0xff) as u8 == array_index {
                state_index = Some(search_index);
                break;
            }
            search_index = search_index.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        }
        let mut already_completed = false;
        search_index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant search_index <= completed.len(),
                decreases completed.len() - search_index,
            )
        )]
        while search_index < completed.len() {
            if completed.get(search_index) == Some(&array_index) {
                already_completed = true;
                break;
            }
            search_index = search_index.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        }
        let active_index;
        if fragment_index == 0 {
            if segment.payload.len() < 6 || state_index.is_some() || already_completed {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            let final_index = read_u16([segment.payload[4], segment.payload[5]]);
            if u64::from(final_index).saturating_add(1) > maximum_bytes {
                return Err(DecodeError::limit(
                    Resource::TransformElements,
                    maximum_bytes,
                    u64::from(final_index).saturating_add(1),
                ));
            }
            if segment.payload[6..].len() as u64 > maximum_bytes {
                return Err(DecodeError::limit(
                    Resource::TransformElements,
                    maximum_bytes,
                    segment.payload[6..].len() as u64,
                ));
            }
            let mut data = Vec::new();
            data.extend_from_slice(&segment.payload[6..]);
            active_index = pending.len();
            pending.push(MctFragment {
                descriptor,
                next_index: 1,
                final_index,
                offset: segment.offset,
                data,
            });
        } else {
            let index = state_index.ok_or_else(|| {
                DecodeError::malformed(self.offset(segment.offset), MalformedKind::InvalidTransform)
            })?;
            let state = pending.get_mut(index).ok_or_else(|| {
                DecodeError::malformed(self.offset(segment.offset), MalformedKind::InvalidTransform)
            })?;
            if state.descriptor != descriptor
                || fragment_index != state.next_index
                || fragment_index > state.final_index
            {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            state.data.extend_from_slice(&segment.payload[4..]);
            if state.data.len() as u64 > maximum_bytes {
                return Err(DecodeError::limit(
                    Resource::TransformElements,
                    maximum_bytes,
                    state.data.len() as u64,
                ));
            }
            if state.next_index == u16::MAX {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                ));
            }
            state.next_index = crate::refinement::mct_fragment_next(state.next_index);
            active_index = index;
        }
        let complete = {
            let state = pending.get(active_index).ok_or_else(|| {
                DecodeError::malformed(self.offset(segment.offset), MalformedKind::InvalidTransform)
            })?;
            crate::refinement::mct_fragment_complete(state.next_index, state.final_index)
        };
        if !complete {
            return Ok(None);
        }
        let state = pending.swap_remove(active_index);
        if state.data.len() as u64 > maximum_bytes {
            return Err(DecodeError::limit(
                Resource::TransformElements,
                maximum_bytes,
                state.data.len() as u64,
            ));
        }
        let mut payload = Vec::with_capacity(state.data.len().saturating_add(6));
        payload.extend_from_slice(&[0, 0]);
        payload.push((state.descriptor >> 8) as u8);
        payload.push(state.descriptor as u8);
        payload.extend_from_slice(&[0, 0]);
        payload.extend_from_slice(&state.data);
        let array = self.parse_mct(Segment {
            marker: MCT,
            offset: state.offset,
            payload: &payload,
        })?;
        completed.push(array_index);
        Ok(Some(array))
    }

    fn consume_mcc_fragment(
        &self,
        segment: Segment<'a>,
        components: u16,
        pending: &mut Vec<MccFragment>,
        completed: &mut Vec<u8>,
    ) -> Result<Option<MctRecord>, DecodeError> {
        if segment.payload.len() < 3 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let fragment_index = read_u16([segment.payload[0], segment.payload[1]]);
        let record_index = segment.payload[2];
        if record_index == 0 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let maximum_bytes = u64::from(components)
            .checked_mul(
                u64::from(components)
                    .checked_mul(4)
                    .and_then(|value| value.checked_add(12))
                    .ok_or_else(|| {
                        DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::ArithmeticOverflow,
                        )
                    })?,
            )
            .ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        let mut state_index = None;
        let mut search_index = 0_usize;
        #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant search_index <= pending.len(),
                    decreases pending.len() - search_index,
                )
            )]
        while search_index < pending.len() {
            let state = pending.get(search_index).ok_or_else(|| {
                DecodeError::malformed(self.offset(segment.offset), MalformedKind::InvalidTransform)
            })?;
            if state.record_index == record_index {
                state_index = Some(search_index);
                break;
            }
            search_index = search_index.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        }
        let mut already_completed = false;
        search_index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant search_index <= completed.len(),
                decreases completed.len() - search_index,
            )
        )]
        while search_index < completed.len() {
            if completed.get(search_index) == Some(&record_index) {
                already_completed = true;
                break;
            }
            search_index = search_index.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        }
        let active_index;
        if fragment_index == 0 {
            if segment.payload.len() < 7 || state_index.is_some() || already_completed {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            let final_index = read_u16([segment.payload[3], segment.payload[4]]);
            let collection_count = read_u16([segment.payload[5], segment.payload[6]]);
            if u64::from(final_index).saturating_add(1) > maximum_bytes {
                return Err(DecodeError::limit(
                    Resource::TransformElements,
                    maximum_bytes,
                    u64::from(final_index).saturating_add(1),
                ));
            }
            if segment.payload[7..].len() as u64 > maximum_bytes {
                return Err(DecodeError::limit(
                    Resource::TransformElements,
                    maximum_bytes,
                    segment.payload[7..].len() as u64,
                ));
            }
            let mut data = Vec::new();
            data.extend_from_slice(&segment.payload[7..]);
            active_index = pending.len();
            pending.push(MccFragment {
                record_index,
                next_index: 1,
                final_index,
                collection_count,
                offset: segment.offset,
                data,
            });
        } else {
            let index = state_index.ok_or_else(|| {
                DecodeError::malformed(self.offset(segment.offset), MalformedKind::InvalidTransform)
            })?;
            let state = pending.get_mut(index).ok_or_else(|| {
                DecodeError::malformed(self.offset(segment.offset), MalformedKind::InvalidTransform)
            })?;
            if fragment_index != state.next_index || fragment_index > state.final_index {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            state.data.extend_from_slice(&segment.payload[3..]);
            if state.data.len() as u64 > maximum_bytes {
                return Err(DecodeError::limit(
                    Resource::TransformElements,
                    maximum_bytes,
                    state.data.len() as u64,
                ));
            }
            if state.next_index == u16::MAX {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                ));
            }
            state.next_index = crate::refinement::mct_fragment_next(state.next_index);
            active_index = index;
        }
        let complete = {
            let state = pending.get(active_index).ok_or_else(|| {
                DecodeError::malformed(self.offset(segment.offset), MalformedKind::InvalidTransform)
            })?;
            crate::refinement::mct_fragment_complete(state.next_index, state.final_index)
        };
        if !complete {
            return Ok(None);
        }
        let state = pending.swap_remove(active_index);
        if state.data.len() as u64 > maximum_bytes {
            return Err(DecodeError::limit(
                Resource::TransformElements,
                maximum_bytes,
                state.data.len() as u64,
            ));
        }
        let mut payload = Vec::with_capacity(state.data.len().saturating_add(7));
        payload.extend_from_slice(&[0, 0]);
        payload.push(state.record_index);
        payload.extend_from_slice(&[0, 0]);
        payload.push((state.collection_count >> 8) as u8);
        payload.push(state.collection_count as u8);
        payload.extend_from_slice(&state.data);
        let record = self.parse_mcc(
            Segment {
                marker: MCC,
                offset: state.offset,
                payload: &payload,
            },
            components,
        )?;
        completed.push(record_index);
        Ok(Some(record))
    }

    fn parse_mct(&self, segment: Segment<'a>) -> Result<MctArray, DecodeError> {
        if segment.payload.len() < 8 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let z = read_u16([segment.payload[0], segment.payload[1]]);
        let descriptor = read_u16([segment.payload[2], segment.payload[3]]);
        let y = read_u16([segment.payload[4], segment.payload[5]]);
        if z != 0 || y != 0 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        if descriptor & 0xf000 != 0 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let index = u8::try_from(descriptor & 0xff).map_err(|_| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        if index == 0 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let kind = match (descriptor >> 8) & 3 {
            0 => MctArrayKind::Dependency,
            1 => MctArrayKind::Decorrelation,
            2 => MctArrayKind::Offset,
            _ => {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
        };
        let (numeric_format, element_bytes) = match (descriptor >> 10) & 3 {
            0 => (MctNumericFormat::Int16, 2_usize),
            1 => (MctNumericFormat::Int32, 4_usize),
            2 => (MctNumericFormat::Float32, 4_usize),
            _ => (MctNumericFormat::Float64, 8_usize),
        };
        let data = &segment.payload[6..];
        if data.is_empty() || !data.len().is_multiple_of(element_bytes) {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let element_count = data.len() / element_bytes;
        let maximum = u64::from(self.limits.maximum_components())
            .checked_mul(u64::from(self.limits.maximum_components()))
            .ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        if element_count as u64 > maximum {
            return Err(DecodeError::limit(
                Resource::TransformElements,
                maximum,
                element_count as u64,
            ));
        }
        let fraction_bits = if matches!(
            numeric_format,
            MctNumericFormat::Float32 | MctNumericFormat::Float64
        ) {
            MCT_FIXED_BITS
        } else {
            0
        };
        let mut values = Vec::with_capacity(element_count);
        let mut position = 0_usize;
        let mut remaining_elements = element_count;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases remaining_elements,)
        )]
        while position < data.len() {
            if remaining_elements == 0 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            remaining_elements -= 1;
            if data.len().saturating_sub(position) < element_bytes {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            let position_1 = position.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            let byte_0 = *data.get(position).ok_or_else(|| {
                DecodeError::malformed(self.offset(segment.offset), MalformedKind::InvalidTransform)
            })?;
            let byte_1 = *data.get(position_1).ok_or_else(|| {
                DecodeError::malformed(self.offset(segment.offset), MalformedKind::InvalidTransform)
            })?;
            let value = match numeric_format {
                MctNumericFormat::Int16 => signed_u16(read_u16([byte_0, byte_1])),
                MctNumericFormat::Int32 => {
                    let position_2 = position.checked_add(2).ok_or_else(|| {
                        DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::ArithmeticOverflow,
                        )
                    })?;
                    let position_3 = position.checked_add(3).ok_or_else(|| {
                        DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::ArithmeticOverflow,
                        )
                    })?;
                    signed_u32(read_u32([
                        byte_0,
                        byte_1,
                        *data.get(position_2).ok_or_else(|| {
                            DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidTransform,
                            )
                        })?,
                        *data.get(position_3).ok_or_else(|| {
                            DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidTransform,
                            )
                        })?,
                    ]))
                }
                MctNumericFormat::Float32 => {
                    let position_2 = position.checked_add(2).ok_or_else(|| {
                        DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::ArithmeticOverflow,
                        )
                    })?;
                    let position_3 = position.checked_add(3).ok_or_else(|| {
                        DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::ArithmeticOverflow,
                        )
                    })?;
                    let bits = read_u32([
                        byte_0,
                        byte_1,
                        *data.get(position_2).ok_or_else(|| {
                            DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidTransform,
                            )
                        })?,
                        *data.get(position_3).ok_or_else(|| {
                            DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidTransform,
                            )
                        })?,
                    ]);
                    float32_to_fixed(bits).map_err(|_| {
                        DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::InvalidTransform,
                        )
                    })?
                }
                MctNumericFormat::Float64 => {
                    let mut bytes = [0_u8; 8];
                    let mut byte_index = 0_usize;
                    #[cfg_attr(
                        verus_keep_ghost,
                        verus_spec(
                            invariant byte_index <= bytes.len(),
                            decreases bytes.len() - byte_index,
                        )
                    )]
                    while byte_index < bytes.len() {
                        bytes[byte_index] = *data
                            .get(position.checked_add(byte_index).ok_or_else(|| {
                                DecodeError::malformed(
                                    self.offset(segment.offset),
                                    MalformedKind::ArithmeticOverflow,
                                )
                            })?)
                            .ok_or_else(|| {
                                DecodeError::malformed(
                                    self.offset(segment.offset),
                                    MalformedKind::InvalidTransform,
                                )
                            })?;
                        byte_index = byte_index.checked_add(1).ok_or_else(|| {
                            DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::ArithmeticOverflow,
                            )
                        })?;
                    }
                    float64_to_fixed(read_u64(bytes)).map_err(|_| {
                        DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::InvalidTransform,
                        )
                    })?
                }
            };
            values.push(value);
            position = position.checked_add(element_bytes).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        }
        Ok(MctArray {
            index,
            kind,
            numeric_format,
            fraction_bits,
            values,
        })
    }

    fn parse_mcc(&self, segment: Segment<'a>, components: u16) -> Result<MctRecord, DecodeError> {
        if segment.payload.len() < 7 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        if read_u16([segment.payload[0], segment.payload[1]]) != 0
            || read_u16([segment.payload[3], segment.payload[4]]) != 0
        {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let index = segment.payload[2];
        let collection_count = read_u16([segment.payload[5], segment.payload[6]]);
        if index == 0 || collection_count == 0 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        if collection_count > self.limits.maximum_components() {
            return Err(DecodeError::limit(
                Resource::TransformElements,
                u64::from(self.limits.maximum_components()),
                u64::from(collection_count),
            ));
        }
        let mut position = 7_usize;
        let mut collections = Vec::with_capacity(usize::from(collection_count));
        let mut collection = 0_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant collection <= collection_count,
                decreases collection_count - collection,
            )
        )]
        while collection < collection_count {
            let transform_kind_byte = *segment.payload.get(position).ok_or_else(|| {
                DecodeError::malformed(self.offset(segment.offset), MalformedKind::InvalidTransform)
            })?;
            position = position.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            if transform_kind_byte & 0xfc != 0 || transform_kind_byte & 3 == 2 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            let transform_kind = transform_kind_byte & 3;
            let input_components = parse_mcc_component_list(
                segment.payload,
                &mut position,
                components,
                segment.offset,
            )?;
            let output_components = parse_mcc_component_list(
                segment.payload,
                &mut position,
                components,
                segment.offset,
            )?;
            if input_components.is_empty() || segment.payload.len().saturating_sub(position) < 3 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            let transform = (u32::from(segment.payload[position]) << 16)
                | (u32::from(segment.payload[position + 1]) << 8)
                | u32::from(segment.payload[position + 2]);
            position = position.checked_add(3).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            let reference_transform = match transform_kind {
                0 | 1 => {
                    if transform & 0xfe0000 != 0
                        || input_components.len() != output_components.len()
                        || (transform_kind == 0 && input_components != output_components)
                    {
                        return Err(DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::InvalidTransform,
                        ));
                    }
                    MctCollectionReferenceTransform::Array {
                        kind: if transform_kind == 0 {
                            MctArrayKind::Dependency
                        } else {
                            MctArrayKind::Decorrelation
                        },
                        reversible: transform & 0x010000 != 0,
                        transform_index: (transform & 0xff) as u8,
                        offset_index: ((transform >> 8) & 0xff) as u8,
                    }
                }
                3 => {
                    if transform & 0xc0ff00 != 0
                        || input_components.len() != output_components.len()
                        || segment.payload.len().saturating_sub(position) < 4
                    {
                        return Err(DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::InvalidTransform,
                        ));
                    }
                    let wavelet_type = match (transform & 0xff) as u8 {
                        0 => MctWaveletType::Irreversible9x7,
                        1 => MctWaveletType::Reversible5x3,
                        _ => {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidTransform,
                            ));
                        }
                    };
                    let levels = ((transform >> 16) & 0x3f) as u8;
                    if levels > 32 {
                        return Err(DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::InvalidTransform,
                        ));
                    }
                    let offset_end = position.checked_add(4).ok_or_else(|| {
                        DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::ArithmeticOverflow,
                        )
                    })?;
                    let offset_bytes =
                        segment.payload.get(position..offset_end).ok_or_else(|| {
                            DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidTransform,
                            )
                        })?;
                    let component_offset = signed_u32(read_u32([
                        offset_bytes[0],
                        offset_bytes[1],
                        offset_bytes[2],
                        offset_bytes[3],
                    ]));
                    position = offset_end;
                    MctCollectionReferenceTransform::Wavelet(MctWavelet {
                        transform: wavelet_type,
                        levels,
                        component_offset: i32::try_from(component_offset).map_err(|_| {
                            DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::ArithmeticOverflow,
                            )
                        })?,
                    })
                }
                _ => {
                    return Err(DecodeError::malformed(
                        self.offset(segment.offset),
                        MalformedKind::InvalidTransform,
                    ));
                }
            };
            collections.push(MctCollectionReference {
                input_components,
                output_components,
                transform: reference_transform,
            });
            collection = collection.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        }
        if position != segment.payload.len() {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        validate_parallel_collections(&collections, segment.offset)?;
        Ok(MctRecord { index, collections })
    }

    fn parse_mco(&self, segment: Segment<'a>) -> Result<Vec<u8>, DecodeError> {
        let count = *segment.payload.first().ok_or_else(|| {
            DecodeError::malformed(self.offset(segment.offset), MalformedKind::InvalidTransform)
        })? as usize;
        if segment.payload.len() != count.saturating_add(1) {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let mut order = Vec::with_capacity(count);
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= count,
                decreases count - index,
            )
        )]
        while index < count {
            let stage = *segment.payload.get(index + 1).ok_or_else(|| {
                DecodeError::malformed(self.offset(segment.offset), MalformedKind::InvalidTransform)
            })?;
            let mut previous = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant previous <= order.len(),
                    decreases order.len() - previous,
                )
            )]
            while previous < order.len() {
                if order.get(previous) == Some(&stage) {
                    return Err(DecodeError::malformed(
                        self.offset(segment.offset),
                        MalformedKind::InvalidTransform,
                    ));
                }
                previous = previous.checked_add(1).ok_or_else(|| {
                    DecodeError::malformed(
                        self.offset(segment.offset),
                        MalformedKind::ArithmeticOverflow,
                    )
                })?;
            }
            if stage == 0 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            order.push(stage);
            index = index.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        }
        Ok(order)
    }

    fn resolve_multiple_component_transform(
        &self,
        parameters: &MctParameters,
    ) -> Result<Option<MultipleComponentTransform>, DecodeError> {
        if parameters.order.is_empty() {
            if parameters.arrays.is_empty() && parameters.records.is_empty() {
                return Ok(None);
            }
            return Ok(None);
        }
        let mut stages = Vec::with_capacity(parameters.order.len());
        let mut order_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant order_index <= parameters.order.len(),
                decreases parameters.order.len() - order_index,
            )
        )]
        while order_index < parameters.order.len() {
            let record_index = parameters.order[order_index];
            let record = parameters
                .records
                .iter()
                .find(|record| record.index == record_index)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTransform))?;
            let mut collections = Vec::with_capacity(record.collections.len());
            let mut collection_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant collection_index <= record.collections.len(),
                    decreases record.collections.len() - collection_index,
                )
            )]
            while collection_index < record.collections.len() {
                let reference = record
                    .collections
                    .get(collection_index)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTransform))?;
                let rows = reference.input_components.len();
                let columns = reference.output_components.len();
                if rows == 0 || rows != columns {
                    return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
                }
                let collection = match reference.transform {
                    MctCollectionReferenceTransform::Array {
                        kind,
                        reversible,
                        transform_index,
                        offset_index,
                    } => {
                        let offsets = if offset_index == 0 {
                            None
                        } else {
                            Some(
                                referenced_mct_array(
                                    &parameters.arrays,
                                    offset_index,
                                    MctArrayKind::Offset,
                                )?
                                .clone(),
                            )
                        };
                        let invalid_offset_dimensions = match &offsets {
                            Some(array) => array.values.len() != rows,
                            None => false,
                        };
                        let floating_offsets = match &offsets {
                            Some(array) => array.fraction_bits != 0,
                            None => false,
                        };
                        let matrix = if transform_index == 0 {
                            None
                        } else {
                            Some(
                                referenced_mct_array(&parameters.arrays, transform_index, kind)?
                                    .clone(),
                            )
                        };
                        let expected_elements =
                            if mct_array_kinds_equal(kind, MctArrayKind::Decorrelation) {
                                rows.checked_mul(columns).ok_or_else(|| {
                                    DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                                })?
                            } else if reversible {
                                rows.checked_mul(rows.checked_add(1).ok_or_else(|| {
                                    DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                                })?)
                                .and_then(|value| value.checked_div(2))
                                .and_then(|value| value.checked_sub(1))
                                .ok_or_else(|| {
                                    DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                                })?
                            } else {
                                rows.checked_mul(rows.saturating_sub(1))
                                    .and_then(|value| value.checked_div(2))
                                    .ok_or_else(|| {
                                        DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
                                    })?
                            };
                        let invalid_matrix_dimensions = match &matrix {
                            Some(array) => array.values.len() != expected_elements,
                            None => false,
                        };
                        let floating_matrix = match &matrix {
                            Some(array) => array.fraction_bits != 0,
                            None => false,
                        };
                        if invalid_offset_dimensions
                            || invalid_matrix_dimensions
                            || (reversible && (floating_matrix || floating_offsets))
                        {
                            return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
                        }
                        MctCollection {
                            reversible,
                            input_components: reference.input_components.clone(),
                            output_components: reference.output_components.clone(),
                            kind: if mct_array_kinds_equal(kind, MctArrayKind::Dependency) {
                                MctCollectionKind::Dependency
                            } else {
                                MctCollectionKind::Decorrelation
                            },
                            matrix,
                            offsets,
                            wavelet: None,
                        }
                    }
                    MctCollectionReferenceTransform::Wavelet(wavelet) => MctCollection {
                        reversible: matches!(wavelet.transform, MctWaveletType::Reversible5x3),
                        input_components: reference.input_components.clone(),
                        output_components: reference.output_components.clone(),
                        kind: MctCollectionKind::Wavelet,
                        matrix: None,
                        offsets: None,
                        wavelet: Some(wavelet),
                    },
                };
                collections.push(collection);
                collection_index = collection_index
                    .checked_add(1)
                    .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            }
            stages.push(MctStage {
                index: record.index,
                collections,
            });
            order_index = order_index
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        }
        Ok(Some(MultipleComponentTransform { stages }))
    }

    #[cfg(any())]
    fn resolve_multiple_component_transform_legacy(
        &self,
        components: u16,
        coding_style: CodingStyle,
        arrays: &[MctArray],
        mcc: Option<Segment<'a>>,
        mco: Option<Segment<'a>>,
    ) -> Result<Option<MultipleComponentTransform>, DecodeError> {
        if arrays.is_empty() && mcc.is_none() && mco.is_none() {
            return Ok(None);
        }
        let mcc =
            mcc.ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))?;
        let mco =
            mco.ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))?;
        if coding_style.multiple_component_transform {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
        }
        if mcc.payload.len() < 12
            || read_u16([mcc.payload[0], mcc.payload[1]]) != 0
            || read_u16([mcc.payload[3], mcc.payload[4]]) != 0
        {
            return Err(DecodeError::malformed(
                self.offset(mcc.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let collection_index = mcc.payload[2];
        let collection_count = read_u16([mcc.payload[5], mcc.payload[6]]);
        if collection_count != 1 || mcc.payload[7] != 1 {
            return Err(DecodeError::malformed(
                self.offset(mcc.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let mut position = 8_usize;
        let input_components =
            parse_mcc_component_list(mcc.payload, &mut position, components, mcc.offset)?;
        let output_components =
            parse_mcc_component_list(mcc.payload, &mut position, components, mcc.offset)?;
        if mcc.payload.len().saturating_sub(position) != 3 {
            return Err(DecodeError::malformed(
                self.offset(mcc.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let transform = (u32::from(mcc.payload[position]) << 16)
            | (u32::from(mcc.payload[position + 1]) << 8)
            | u32::from(mcc.payload[position + 2]);
        if transform & 0xfe0000 != 0 {
            return Err(DecodeError::malformed(
                self.offset(mcc.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let reversible = transform & 0x010000 != 0;
        let matrix_index = (transform & 0xff) as u8;
        let offset_index = ((transform >> 8) & 0xff) as u8;
        if matrix_index == 0 {
            return Err(DecodeError::malformed(
                self.offset(mcc.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        if mco.payload.is_empty() {
            return Err(DecodeError::malformed(
                self.offset(mco.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let stages = usize::from(mco.payload[0]);
        if stages != 1 {
            return Err(DecodeError::malformed(
                self.offset(mco.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        if mco.payload.len() != 2 || mco.payload[1] != collection_index {
            return Err(DecodeError::malformed(
                self.offset(mco.offset),
                MalformedKind::InvalidTransform,
            ));
        }

        let mut matrix = None;
        let mut offsets = None;
        let mut array_index = 0_usize;
        while array_index < arrays.len() {
            let array = arrays.get(array_index).ok_or_else(|| {
                DecodeError::malformed(self.offset(mcc.offset), MalformedKind::InvalidTransform)
            })?;
            if array.index == matrix_index {
                if matrix.is_some() || !matches!(array.kind, MctArrayKind::Decorrelation) {
                    return Err(DecodeError::malformed(
                        self.offset(mcc.offset),
                        MalformedKind::InvalidTransform,
                    ));
                }
                matrix = Some(array.clone());
            } else if offset_index != 0 && array.index == offset_index {
                if offsets.is_some() || !matches!(array.kind, MctArrayKind::Offset) {
                    return Err(DecodeError::malformed(
                        self.offset(mcc.offset),
                        MalformedKind::InvalidTransform,
                    ));
                }
                offsets = Some(array.clone());
            } else {
                return Err(DecodeError::malformed(
                    self.offset(mcc.offset),
                    MalformedKind::InvalidTransform,
                ));
            }
            array_index = array_index.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(self.offset(mcc.offset), MalformedKind::ArithmeticOverflow)
            })?;
        }
        let matrix = matrix.ok_or_else(|| {
            DecodeError::malformed(self.offset(mcc.offset), MalformedKind::InvalidTransform)
        })?;
        if offset_index != 0 && offsets.is_none() {
            return Err(DecodeError::malformed(
                self.offset(mcc.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let count = usize::from(components);
        let matrix_elements = count.checked_mul(count).ok_or_else(|| {
            DecodeError::malformed(self.offset(mcc.offset), MalformedKind::ArithmeticOverflow)
        })?;
        let invalid_offset_dimensions = match &offsets {
            Some(array) => array.values.len() != count,
            None => false,
        };
        if matrix.values.len() != matrix_elements || invalid_offset_dimensions {
            return Err(DecodeError::malformed(
                self.offset(mcc.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        let floating_offsets = match &offsets {
            Some(array) => array.fraction_bits != 0,
            None => false,
        };
        if reversible && (matrix.fraction_bits != 0 || floating_offsets) {
            return Err(DecodeError::malformed(
                self.offset(mcc.offset),
                MalformedKind::InvalidTransform,
            ));
        }
        Ok(Some(MultipleComponentTransform {
            collection_index,
            reversible,
            input_components,
            output_components,
            matrix,
            offsets,
        }))
    }

    fn parse_cod(&self, segment: Segment<'a>) -> Result<CodingStyle, DecodeError> {
        self.parse_cod_payload(segment.offset, segment.payload)
    }

    fn parse_cod_payload(
        &self,
        segment_offset: usize,
        payload: &[u8],
    ) -> Result<CodingStyle, DecodeError> {
        if payload.len() < 10 {
            return Err(DecodeError::malformed(
                self.offset(segment_offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let flags = payload[0];
        if flags & 0xf8 != 0 {
            return Err(DecodeError::malformed(
                self.offset(segment_offset),
                MalformedKind::InvalidCodingStyle,
            ));
        }
        let progression_order = match payload[1] {
            0 => ProgressionOrder::Lrcp,
            1 => ProgressionOrder::Rlcp,
            2 => ProgressionOrder::Rpcl,
            3 => ProgressionOrder::Pcrl,
            4 => ProgressionOrder::Cprl,
            _ => {
                return Err(DecodeError::malformed(
                    self.offset(segment_offset),
                    MalformedKind::InvalidCodingStyle,
                ));
            }
        };
        let layers = read_u16([payload[2], payload[3]]);
        if layers == 0 || layers > self.limits.maximum_layers() {
            return Err(if layers > self.limits.maximum_layers() {
                DecodeError::limit(
                    Resource::Layers,
                    u64::from(self.limits.maximum_layers()),
                    u64::from(layers),
                )
            } else {
                DecodeError::malformed(
                    self.offset(segment_offset),
                    MalformedKind::InvalidCodingStyle,
                )
            });
        }
        let multiple_component_transform = match payload[4] {
            0 => false,
            1 => true,
            _ => {
                return Err(DecodeError::malformed(
                    self.offset(segment_offset),
                    MalformedKind::InvalidCodingStyle,
                ));
            }
        };
        let decomposition_levels = payload[5];
        let resolution_levels = decomposition_levels.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment_offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        if resolution_levels > self.limits.maximum_resolutions_per_component() {
            return Err(DecodeError::limit(
                Resource::Resolutions,
                u64::from(self.limits.maximum_resolutions_per_component()),
                u64::from(resolution_levels),
            ));
        }
        let width_exponent = payload[6];
        let height_exponent = payload[7];
        if width_exponent > 8 || height_exponent > 8 {
            return Err(DecodeError::malformed(
                self.offset(segment_offset),
                MalformedKind::InvalidCodingStyle,
            ));
        }
        let code_block_width = 1_u16 << (u32::from(width_exponent) + 2);
        let code_block_height = 1_u16 << (u32::from(height_exponent) + 2);
        let code_block_samples = u32::from(code_block_width)
            .checked_mul(u32::from(code_block_height))
            .ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment_offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        if code_block_samples > 4096 || Tier1BlockCoding::from_code_block_style(payload[8]).is_err()
        {
            return Err(DecodeError::malformed(
                self.offset(segment_offset),
                MalformedKind::InvalidCodingStyle,
            ));
        }
        let transform = match payload[9] {
            0 => WaveletTransform::Irreversible9x7,
            1 => WaveletTransform::Reversible5x3,
            index => WaveletTransform::Arbitrary(index),
        };
        let precincts_defined = flags & 1 != 0;
        let precinct_count = usize::from(resolution_levels);
        let expected_length = if precincts_defined {
            10_usize.checked_add(precinct_count).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment_offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?
        } else {
            10
        };
        if payload.len() != expected_length {
            return Err(DecodeError::malformed(
                self.offset(segment_offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let mut precinct_exponents = [(15_u8, 15_u8); 33];
        if precincts_defined {
            let mut resolution = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant resolution <= precinct_count,
                    decreases precinct_count - resolution,
                )
            )]
            while resolution < precinct_count {
                let index = 10_usize.checked_add(resolution).ok_or_else(|| {
                    DecodeError::malformed(
                        self.offset(segment_offset),
                        MalformedKind::ArithmeticOverflow,
                    )
                })?;
                if index >= payload.len() || resolution >= precinct_exponents.len() {
                    return Err(DecodeError::malformed(
                        self.offset(segment_offset),
                        MalformedKind::InvalidCodingStyle,
                    ));
                }
                precinct_exponents[resolution] = (payload[index] & 0x0f, payload[index] >> 4);
                resolution = resolution.checked_add(1).ok_or_else(|| {
                    DecodeError::malformed(
                        self.offset(segment_offset),
                        MalformedKind::ArithmeticOverflow,
                    )
                })?;
            }
        }
        Ok(CodingStyle {
            progression_order,
            layers,
            multiple_component_transform,
            decomposition_levels,
            decomposition_structure: None,
            decomposition_directions: [DecompositionDirection::TwoDimensional; 32],
            code_block_width,
            code_block_height,
            code_block_style: payload[8],
            transform,
            precincts_defined,
            sop_markers: flags & 2 != 0,
            eph_markers: flags & 4 != 0,
            precinct_exponents,
        })
    }

    fn parse_component_index(
        &self,
        segment: Segment<'a>,
        components: u16,
        invalid_kind: MalformedKind,
    ) -> Result<(u16, usize), DecodeError> {
        let component_bytes = if components < 257 { 1_usize } else { 2_usize };
        if segment.payload.len() < component_bytes {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let component = if component_bytes == 1 {
            u16::from(segment.payload[0])
        } else {
            read_u16([segment.payload[0], segment.payload[1]])
        };
        if component >= components {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                invalid_kind,
            ));
        }
        Ok((component, component_bytes))
    }

    fn parse_poc(
        &self,
        segment: Segment<'a>,
        components: u16,
    ) -> Result<ProgressionChanges, DecodeError> {
        let component_bytes = if components < 257 { 1_usize } else { 2_usize };
        let record_bytes = component_bytes
            .checked_mul(2)
            .and_then(|value| value.checked_add(5))
            .ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        if segment.payload.is_empty() || !segment.payload.len().is_multiple_of(record_bytes) {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let mut records = Vec::with_capacity(segment.payload.len() / record_bytes);
        let mut position = 0_usize;
        let mut remaining_records = segment.payload.len() / record_bytes;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases remaining_records,)
        )]
        while position < segment.payload.len() {
            if remaining_records == 0 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            remaining_records -= 1;
            let resolution_start = self.read_poc_byte(segment, position)?;
            position = position.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            let component_start = self.read_poc_component(segment, position, component_bytes)?;
            position = position.checked_add(component_bytes).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            let layer_end_position = position.checked_add(2).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            let layer_bytes = segment
                .payload
                .get(position..layer_end_position)
                .ok_or_else(|| {
                    DecodeError::malformed(
                        self.offset(segment.offset),
                        MalformedKind::InvalidMarkerLength,
                    )
                })?;
            let layer_end = read_u16([layer_bytes[0], layer_bytes[1]]);
            position = position.checked_add(2).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            let resolution_end = self.read_poc_byte(segment, position)?;
            position = position.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            let encoded_component_end =
                self.read_poc_component(segment, position, component_bytes)?;
            let component_end =
                if component_bytes == 1 && encoded_component_end == 0 && components == 256 {
                    256
                } else {
                    encoded_component_end
                };
            position = position.checked_add(component_bytes).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            let progression_order = match self.read_poc_byte(segment, position)? {
                0 => ProgressionOrder::Lrcp,
                1 => ProgressionOrder::Rlcp,
                2 => ProgressionOrder::Rpcl,
                3 => ProgressionOrder::Pcrl,
                4 => ProgressionOrder::Cprl,
                _ => {
                    return Err(DecodeError::malformed(
                        self.offset(segment.offset),
                        MalformedKind::InvalidProgressionOrderChange,
                    ));
                }
            };
            position = position.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            records.push(ProgressionChange {
                resolution_start,
                component_start,
                layer_end,
                resolution_end,
                component_end,
                progression_order,
            });
        }
        Ok(ProgressionChanges { records })
    }

    fn read_poc_component(
        &self,
        segment: Segment<'a>,
        position: usize,
        width: usize,
    ) -> Result<u16, DecodeError> {
        if width == 1 {
            let value = segment.payload.get(position).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                )
            })?;
            return Ok(*value as u16);
        }
        let end = position.checked_add(2).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        let bytes = segment.payload.get(position..end).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            )
        })?;
        Ok(read_u16([bytes[0], bytes[1]]))
    }

    fn read_poc_byte(&self, segment: Segment<'a>, position: usize) -> Result<u8, DecodeError> {
        let value = segment.payload.get(position).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            )
        })?;
        Ok(*value)
    }

    fn validate_progression_changes(
        &self,
        changes: &ProgressionChanges,
        layers: u16,
        component_styles: &ComponentCodingStyles,
        offset: usize,
    ) -> Result<(), DecodeError> {
        if changes.is_empty() {
            return Ok(());
        }
        let components = component_styles.components;
        let mut active_coordinates = 0_usize;
        let mut maximum_resolutions = 0_u8;
        let mut component = 0_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= components,
                decreases components - component,
            )
        )]
        while component < components {
            let resolutions = component_styles
                .style(component)?
                .decomposition_levels
                .checked_add(1)
                .ok_or_else(|| {
                    DecodeError::malformed(self.offset(offset), MalformedKind::ArithmeticOverflow)
                })?;
            maximum_resolutions = maximum_resolutions.max(resolutions);
            active_coordinates = active_coordinates
                .checked_add(
                    usize::from(layers)
                        .checked_mul(usize::from(resolutions))
                        .ok_or_else(|| {
                            DecodeError::malformed(
                                self.offset(offset),
                                MalformedKind::ArithmeticOverflow,
                            )
                        })?,
                )
                .ok_or_else(|| {
                    DecodeError::malformed(self.offset(offset), MalformedKind::ArithmeticOverflow)
                })?;
            component = component.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(self.offset(offset), MalformedKind::ArithmeticOverflow)
            })?;
        }
        let validation_work = (changes.records.len() as u64)
            .checked_mul(active_coordinates as u64)
            .ok_or_else(|| {
                DecodeError::malformed(self.offset(offset), MalformedKind::ArithmeticOverflow)
            })?;
        if validation_work > self.limits.maximum_work_units() {
            return Err(DecodeError::limit(
                Resource::WorkUnits,
                self.limits.maximum_work_units(),
                validation_work,
            ));
        }
        let mut owners = vec![false; active_coordinates];
        let mut record_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant record_index <= changes.records.len(),
                decreases changes.records.len() - record_index,
            )
        )]
        while record_index < changes.records.len() {
            let record = *changes.records.get(record_index).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(offset),
                    MalformedKind::InvalidProgressionOrderChange,
                )
            })?;
            if record.layer_end == 0
                || record.layer_end > layers
                || record.component_start >= record.component_end
                || record.component_end > components
                || record.resolution_start >= record.resolution_end
                || record.resolution_end > maximum_resolutions
            {
                return Err(DecodeError::malformed(
                    self.offset(offset),
                    MalformedKind::InvalidProgressionOrderChange,
                ));
            }
            let mut contributed = false;
            let mut linear = 0_usize;
            component = 0;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant component <= components,
                    decreases components - component,
                )
            )]
            while component < components {
                let resolutions = component_styles
                    .style(component)?
                    .decomposition_levels
                    .checked_add(1)
                    .ok_or_else(|| {
                        DecodeError::malformed(
                            self.offset(offset),
                            MalformedKind::ArithmeticOverflow,
                        )
                    })?;
                let mut layer = 0_u16;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant layer <= layers,
                        decreases layers - layer,
                    )
                )]
                while layer < layers {
                    let mut resolution = 0_u8;
                    #[cfg_attr(
                        verus_keep_ghost,
                        verus_spec(
                            invariant resolution <= resolutions,
                            decreases resolutions - resolution,
                        )
                    )]
                    while resolution < resolutions {
                        if record.contains(layer, resolution, component) {
                            let owner = owners.get_mut(linear).ok_or_else(|| {
                                DecodeError::malformed(
                                    self.offset(offset),
                                    MalformedKind::ArithmeticOverflow,
                                )
                            })?;
                            if !*owner {
                                *owner = true;
                                contributed = true;
                            }
                        }
                        linear = linear.checked_add(1).ok_or_else(|| {
                            DecodeError::malformed(
                                self.offset(offset),
                                MalformedKind::ArithmeticOverflow,
                            )
                        })?;
                        resolution = resolution.checked_add(1).ok_or_else(|| {
                            DecodeError::malformed(
                                self.offset(offset),
                                MalformedKind::ArithmeticOverflow,
                            )
                        })?;
                    }
                    layer = layer.checked_add(1).ok_or_else(|| {
                        DecodeError::malformed(
                            self.offset(offset),
                            MalformedKind::ArithmeticOverflow,
                        )
                    })?;
                }
                component = component.checked_add(1).ok_or_else(|| {
                    DecodeError::malformed(self.offset(offset), MalformedKind::ArithmeticOverflow)
                })?;
            }
            if !contributed {
                return Err(DecodeError::malformed(
                    self.offset(offset),
                    MalformedKind::InvalidProgressionOrderChange,
                ));
            }
            record_index = record_index.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(self.offset(offset), MalformedKind::ArithmeticOverflow)
            })?;
        }
        if owners.iter().any(|covered| !covered) {
            return Err(DecodeError::malformed(
                self.offset(offset),
                MalformedKind::InvalidProgressionOrderChange,
            ));
        }
        Ok(())
    }

    fn parse_coc(
        &self,
        segment: Segment<'a>,
        components: u16,
        default: CodingStyle,
    ) -> Result<(u16, CodingStyle), DecodeError> {
        let (component, component_bytes) =
            self.parse_component_index(segment, components, MalformedKind::InvalidCodingStyle)?;
        let minimum = component_bytes.checked_add(6).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        if segment.payload.len() < minimum {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let scoc = segment.payload[component_bytes];
        if scoc & 0xfe != 0 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidCodingStyle,
            ));
        }
        let parameters = segment.payload.get(component_bytes + 1..).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            )
        })?;
        let decomposition_structure = if parameters[0] & 0x80 != 0 {
            let index = parameters[0] & 0x7f;
            if index == 0 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidCodingStyle,
                ));
            }
            Some(index)
        } else {
            None
        };
        let mut cod_payload = [0_u8; 43];
        cod_payload[0] = (scoc & 1)
            | if default.sop_markers { 2 } else { 0 }
            | if default.eph_markers { 4 } else { 0 };
        cod_payload[1] = match default.progression_order {
            ProgressionOrder::Lrcp => 0,
            ProgressionOrder::Rlcp => 1,
            ProgressionOrder::Rpcl => 2,
            ProgressionOrder::Pcrl => 3,
            ProgressionOrder::Cprl => 4,
        };
        cod_payload[2] = u8::try_from(default.layers >> 8).map_err(|_| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        cod_payload[3] = u8::try_from(default.layers & 0xff).map_err(|_| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        cod_payload[4] = u8::from(default.multiple_component_transform);
        let end = 5_usize.checked_add(parameters.len()).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        if end > cod_payload.len() {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        cod_payload[5..end].copy_from_slice(parameters);
        if decomposition_structure.is_some() {
            cod_payload[5] = default.decomposition_levels;
        }
        let mut style = self.parse_cod_payload(segment.offset, &cod_payload[..end])?;
        style.decomposition_structure = decomposition_structure;
        Ok((component, style))
    }

    fn parse_qcd(
        &self,
        segment: Segment<'a>,
        coding_style: CodingStyle,
    ) -> Result<Quantization, DecodeError> {
        self.parse_qcd_payload(segment.offset, segment.payload, coding_style)
    }

    fn parse_qcd_payload(
        &self,
        segment_offset: usize,
        payload: &[u8],
        coding_style: CodingStyle,
    ) -> Result<Quantization, DecodeError> {
        if payload.is_empty() {
            return Err(DecodeError::malformed(
                self.offset(segment_offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }

        let guard_bits = payload[0] >> 5;
        let style = match payload[0] & 0x1f {
            0 => QuantizationStyle::None,
            1 => QuantizationStyle::ScalarDerived,
            2 => QuantizationStyle::ScalarExpounded,
            _ => {
                return Err(DecodeError::malformed(
                    self.offset(segment_offset),
                    MalformedKind::InvalidQuantization,
                ));
            }
        };
        let subband_count = coding_style.subband_count();
        let step_bytes = payload.len() - 1;
        let expected_step_bytes = match style {
            QuantizationStyle::None => usize::from(subband_count),
            QuantizationStyle::ScalarDerived => 2,
            QuantizationStyle::ScalarExpounded => {
                usize::from(subband_count).checked_mul(2).ok_or_else(|| {
                    DecodeError::malformed(
                        self.offset(segment_offset),
                        MalformedKind::ArithmeticOverflow,
                    )
                })?
            }
        };
        if step_bytes != expected_step_bytes {
            return Err(DecodeError::malformed(
                self.offset(segment_offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        if matches!(style, QuantizationStyle::None) {
            let mut index = 1_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(decreases payload.len() - index,)
            )]
            while index < payload.len() {
                if payload[index] & 0x07 != 0 {
                    return Err(DecodeError::malformed(
                        self.offset(segment_offset),
                        MalformedKind::InvalidQuantization,
                    ));
                }
                index = index.checked_add(1).ok_or_else(|| {
                    DecodeError::malformed(
                        self.offset(segment_offset),
                        MalformedKind::ArithmeticOverflow,
                    )
                })?;
            }
        }
        let step_count = match style {
            QuantizationStyle::None | QuantizationStyle::ScalarExpounded => subband_count,
            QuantizationStyle::ScalarDerived => 1,
        };
        if usize::from(step_count) > MAX_QUANTIZATION_STEPS {
            return Err(DecodeError::malformed(
                self.offset(segment_offset),
                MalformedKind::InvalidQuantization,
            ));
        }
        let mut steps = [QuantizationStep {
            exponent: 0,
            mantissa: 0,
        }; MAX_QUANTIZATION_STEPS];
        let step_total = usize::from(step_count);
        let mut step_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases step_total - step_index,)
        )]
        while step_index < step_total {
            let payload_index = if matches!(style, QuantizationStyle::None) {
                step_index.checked_add(1).ok_or_else(|| {
                    DecodeError::malformed(
                        self.offset(segment_offset),
                        MalformedKind::ArithmeticOverflow,
                    )
                })?
            } else {
                step_index
                    .checked_mul(2)
                    .and_then(|index| index.checked_add(1))
                    .ok_or_else(|| {
                        DecodeError::malformed(
                            self.offset(segment_offset),
                            MalformedKind::ArithmeticOverflow,
                        )
                    })?
            };
            let step = if matches!(style, QuantizationStyle::None) {
                if payload_index >= payload.len() {
                    return Err(DecodeError::malformed(
                        self.offset(segment_offset),
                        MalformedKind::InvalidQuantization,
                    ));
                }
                QuantizationStep {
                    exponent: payload[payload_index] >> 3,
                    mantissa: 0,
                }
            } else {
                let next_index = payload_index.checked_add(1).ok_or_else(|| {
                    DecodeError::malformed(
                        self.offset(segment_offset),
                        MalformedKind::ArithmeticOverflow,
                    )
                })?;
                if next_index >= payload.len() {
                    return Err(DecodeError::malformed(
                        self.offset(segment_offset),
                        MalformedKind::InvalidQuantization,
                    ));
                }
                let encoded = read_u16([payload[payload_index], payload[next_index]]);
                QuantizationStep {
                    exponent: u8::try_from(encoded >> 11).map_err(|_| {
                        DecodeError::malformed(
                            self.offset(segment_offset),
                            MalformedKind::ArithmeticOverflow,
                        )
                    })?,
                    mantissa: encoded & 0x07ff,
                }
            };
            if step_index >= steps.len() {
                return Err(DecodeError::malformed(
                    self.offset(segment_offset),
                    MalformedKind::InvalidQuantization,
                ));
            }
            steps[step_index] = step;
            step_index = step_index.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment_offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        }
        Ok(Quantization {
            style,
            guard_bits,
            step_count,
            steps,
        })
    }

    fn parse_qcc(
        &self,
        segment: Segment<'a>,
        components: u16,
        coding_style: CodingStyle,
    ) -> Result<(u16, Quantization), DecodeError> {
        let (component, component_bytes) =
            self.parse_component_index(segment, components, MalformedKind::InvalidQuantization)?;
        let payload = segment.payload.get(component_bytes..).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            )
        })?;
        let quantization = self.parse_qcd_payload(segment.offset, payload, coding_style)?;
        Ok((component, quantization))
    }

    fn parse_rgn(&self, segment: Segment<'a>, components: u16) -> Result<(u16, u8), DecodeError> {
        let component_bytes = if components < 257 { 1_usize } else { 2_usize };
        let expected_length = component_bytes.checked_add(2).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        if segment.payload.len() != expected_length {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let component = if component_bytes == 1 {
            u16::from(segment.payload[0])
        } else {
            read_u16([segment.payload[0], segment.payload[1]])
        };
        let style_index = component_bytes;
        let shift_index = component_bytes.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        if component >= components
            || style_index >= segment.payload.len()
            || shift_index >= segment.payload.len()
            || segment.payload[style_index] != 0
        {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidRegionOfInterest,
            ));
        }
        Ok((component, segment.payload[shift_index]))
    }

    fn parse_plm(
        &mut self,
        segment: Segment<'a>,
        next_index: &mut u16,
        groups: &mut Vec<PacketLengths<'a>>,
    ) -> Result<(), DecodeError> {
        if segment.payload.len() < 5 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let index = u16::from(segment.payload[0]);
        if index != *next_index || index > u16::from(u8::MAX) {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerOrder,
            ));
        }
        *next_index = next_index.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;

        let mut position = 1_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= segment.payload.len(),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            let previous_position = position;
            if segment.payload.len() - position < 4 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            let encoded_length = usize::try_from(read_u32([
                segment.payload[position],
                segment.payload[position + 1],
                segment.payload[position + 2],
                segment.payload[position + 3],
            ]))
            .map_err(|_| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            position = position.checked_add(4).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            let end = position.checked_add(encoded_length).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            let encoded = segment.payload.get(position..end).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                )
            })?;
            let (packet_count, _) = self.validate_packet_lengths(encoded, segment.offset)?;
            groups.push(PacketLengths {
                source: PacketLengthSource::Plm(encoded),
                packet_count,
                encoded_bytes: encoded.len() as u64,
            });
            if groups.len() as u64 > u64::from(self.limits.maximum_tile_parts()) {
                return Err(DecodeError::limit(
                    Resource::TileParts,
                    u64::from(self.limits.maximum_tile_parts()),
                    groups.len() as u64,
                ));
            }
            position = end;
            if position <= previous_position || position > segment.payload.len() {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                ));
            }
        }
        Ok(())
    }

    fn parse_packed_fragment(
        &mut self,
        segment: Segment<'a>,
        expected_marker: u16,
        next_index: &mut u16,
        fragments: &mut u16,
        bytes: &mut Vec<u8>,
    ) -> Result<(), DecodeError> {
        if segment.marker != expected_marker || segment.payload.is_empty() {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let index = u16::from(segment.payload[0]);
        if index != *next_index || index > u16::from(u8::MAX) {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerOrder,
            ));
        }
        *next_index = next_index.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        *fragments = fragments.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        let fragment = segment.payload.get(1..).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            )
        })?;
        let new_length = bytes.len().checked_add(fragment.len()).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        let total_header_bytes = self
            .packet_length_bytes
            .checked_add(fragment.len() as u64)
            .ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        if total_header_bytes > self.limits.maximum_packet_header_bytes() {
            return Err(DecodeError::limit(
                Resource::PacketHeaderBytes,
                self.limits.maximum_packet_header_bytes(),
                total_header_bytes,
            ));
        }
        self.packet_length_bytes = total_header_bytes;
        self.charge_work(fragment.len() as u64)?;
        bytes.reserve(new_length.saturating_sub(bytes.len()));
        bytes.extend_from_slice(fragment);
        Ok(())
    }

    fn parse_ppm_frames(
        &self,
        bytes: &[u8],
        fragments: u16,
    ) -> Result<Vec<PackedHeaderFrame>, DecodeError> {
        if fragments == 0 {
            return Ok(Vec::new());
        }
        let mut frames = Vec::new();
        let mut position = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= bytes.len(),
                decreases bytes.len() - position,
            )
        )]
        while position < bytes.len() {
            if bytes.len().saturating_sub(position) < 4 {
                return Err(DecodeError::malformed(
                    position as u64,
                    MalformedKind::InvalidPackedPacketHeader,
                ));
            }
            let length = usize::try_from(read_u32([
                bytes[position],
                bytes[position + 1],
                bytes[position + 2],
                bytes[position + 3],
            ]))
            .map_err(|_| {
                DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
            })?;
            position = position.checked_add(4).ok_or_else(|| {
                DecodeError::malformed(position as u64, MalformedKind::ArithmeticOverflow)
            })?;
            let start = position;
            position = position.checked_add(length).ok_or_else(|| {
                DecodeError::malformed(start as u64, MalformedKind::ArithmeticOverflow)
            })?;
            if position > bytes.len() {
                return Err(DecodeError::malformed(
                    start as u64,
                    MalformedKind::InvalidPackedPacketHeader,
                ));
            }
            frames.push(PackedHeaderFrame { start, length });
            if frames.len() as u64 > u64::from(self.limits.maximum_tile_parts()) {
                return Err(DecodeError::limit(
                    Resource::TileParts,
                    u64::from(self.limits.maximum_tile_parts()),
                    frames.len() as u64,
                ));
            }
        }
        Ok(frames)
    }

    fn validate_packet_lengths(
        &mut self,
        encoded: &[u8],
        marker_offset: usize,
    ) -> Result<(u64, u64), DecodeError> {
        let encoded_bytes = encoded.len() as u64;
        self.packet_length_bytes = self
            .packet_length_bytes
            .checked_add(encoded_bytes)
            .ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(marker_offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        if self.packet_length_bytes > self.limits.maximum_packet_header_bytes() {
            return Err(DecodeError::limit(
                Resource::PacketHeaderBytes,
                self.limits.maximum_packet_header_bytes(),
                self.packet_length_bytes,
            ));
        }
        self.charge_work(encoded_bytes)?;
        let mut position = 0_usize;
        let mut count = 0_u64;
        let mut total = 0_u64;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= encoded.len(),
                decreases encoded.len() - position,
            )
        )]
        while position < encoded.len() {
            let (length, next) =
                decode_packet_length(encoded, position, encoded.len()).map_err(|_| {
                    DecodeError::malformed(
                        self.offset(marker_offset),
                        MalformedKind::InvalidPacketLength,
                    )
                })?;
            if next <= position || next > encoded.len() {
                return Err(DecodeError::malformed(
                    self.offset(marker_offset),
                    MalformedKind::InvalidPacketLength,
                ));
            }
            count = count.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(marker_offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            total = total.checked_add(u64::from(length)).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(marker_offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            position = next;
        }
        self.packet_lengths = self.packet_lengths.checked_add(count).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(marker_offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        if self.packet_lengths > self.limits.maximum_packets() {
            return Err(DecodeError::limit(
                Resource::Packets,
                self.limits.maximum_packets(),
                self.packet_lengths,
            ));
        }
        Ok((count, total))
    }

    fn parse_siz(&self, segment: Segment<'a>) -> Result<(ImageSize, u16), DecodeError> {
        if segment.payload.len() < 36 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidMarkerLength,
            ));
        }
        let xsiz = read_u32([
            segment.payload[2],
            segment.payload[3],
            segment.payload[4],
            segment.payload[5],
        ]);
        let ysiz = read_u32([
            segment.payload[6],
            segment.payload[7],
            segment.payload[8],
            segment.payload[9],
        ]);
        let xosiz = read_u32([
            segment.payload[10],
            segment.payload[11],
            segment.payload[12],
            segment.payload[13],
        ]);
        let yosiz = read_u32([
            segment.payload[14],
            segment.payload[15],
            segment.payload[16],
            segment.payload[17],
        ]);
        let xtsiz = read_u32([
            segment.payload[18],
            segment.payload[19],
            segment.payload[20],
            segment.payload[21],
        ]);
        let ytsiz = read_u32([
            segment.payload[22],
            segment.payload[23],
            segment.payload[24],
            segment.payload[25],
        ]);
        let xtosiz = read_u32([
            segment.payload[26],
            segment.payload[27],
            segment.payload[28],
            segment.payload[29],
        ]);
        let ytosiz = read_u32([
            segment.payload[30],
            segment.payload[31],
            segment.payload[32],
            segment.payload[33],
        ]);
        let components = read_u16([segment.payload[34], segment.payload[35]]);
        let first_tile_x1 = xtosiz.checked_add(xtsiz).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        let first_tile_y1 = ytosiz.checked_add(ytsiz).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        let component_bytes = usize::from(components).checked_mul(3).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        let expected_length = 36_usize.checked_add(component_bytes).ok_or_else(|| {
            DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::ArithmeticOverflow,
            )
        })?;
        if segment.payload.len() != expected_length
            || xsiz <= xosiz
            || ysiz <= yosiz
            || xtsiz == 0
            || ytsiz == 0
            || xtosiz > xosiz
            || ytosiz > yosiz
            || first_tile_x1 <= xosiz
            || first_tile_y1 <= yosiz
            || components == 0
        {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidImageGeometry,
            ));
        }
        if components > self.limits.maximum_components() {
            return Err(DecodeError::limit(
                Resource::Components,
                u64::from(self.limits.maximum_components()),
                u64::from(components),
            ));
        }

        let mut component_offset = 36_usize;
        let mut first_precision = 0_u8;
        let mut first_signed = false;
        let mut components_uniform = true;
        let mut component_index = 0_u16;
        let mut component_info = [ComponentInfo::default(); MAX_COMPONENTS];
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_offset <= segment.payload.len(),
                decreases segment.payload.len() - component_offset,
            )
        )]
        while component_offset < segment.payload.len() {
            if segment.payload.len() - component_offset < 3 {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            let precision_value = u16::from(segment.payload[component_offset] & 0x7f) + 1;
            let signed = segment.payload[component_offset] & 0x80 != 0;
            if precision_value > 38
                || segment.payload[component_offset + 1] == 0
                || segment.payload[component_offset + 2] == 0
            {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidImageGeometry,
                ));
            }
            let precision = u8::try_from(precision_value).map_err(|_| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            let info_index = usize::from(component_index);
            if info_index >= component_info.len() {
                return Err(DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::InvalidImageGeometry,
                ));
            }
            component_info[info_index] = ComponentInfo {
                precision,
                signed,
                x_subsampling: segment.payload[component_offset + 1],
                y_subsampling: segment.payload[component_offset + 2],
                x_registration: 0,
                y_registration: 0,
            };
            if component_index == 0 {
                first_precision = precision;
                first_signed = signed;
            } else if precision != first_precision || signed != first_signed {
                components_uniform = false;
            }
            component_index = component_index.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
            component_offset = component_offset.checked_add(3).ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        }

        let width = xsiz - xosiz;
        let height = ysiz - yosiz;
        if width > self.limits.maximum_width() {
            return Err(DecodeError::limit(
                Resource::Width,
                u64::from(self.limits.maximum_width()),
                u64::from(width),
            ));
        }
        if height > self.limits.maximum_height() {
            return Err(DecodeError::limit(
                Resource::Height,
                u64::from(self.limits.maximum_height()),
                u64::from(height),
            ));
        }
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        if pixels > self.limits.maximum_pixels() {
            return Err(DecodeError::limit(
                Resource::Pixels,
                self.limits.maximum_pixels(),
                pixels,
            ));
        }

        let tiles_across = ceil_div(xsiz - xtosiz, xtsiz)?;
        let tiles_down = ceil_div(ysiz - ytosiz, ytsiz)?;
        let tiles = u64::from(tiles_across)
            .checked_mul(u64::from(tiles_down))
            .ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(segment.offset),
                    MalformedKind::ArithmeticOverflow,
                )
            })?;
        if tiles > u64::from(self.limits.maximum_tiles()) {
            return Err(DecodeError::limit(
                Resource::Tiles,
                u64::from(self.limits.maximum_tiles()),
                tiles,
            ));
        }

        Ok((
            ImageSize {
                width,
                height,
                x_origin: xosiz,
                y_origin: yosiz,
                components,
                uniform_bit_depth: if components_uniform {
                    Some(first_precision)
                } else {
                    None
                },
                uniform_signed: if components_uniform {
                    Some(first_signed)
                } else {
                    None
                },
                tile_width: xtsiz,
                tile_height: ytsiz,
                tile_x_origin: xtosiz,
                tile_y_origin: ytosiz,
                tiles_across,
                tiles_down,
                component_info,
            },
            read_u16([segment.payload[0], segment.payload[1]]),
        ))
    }

    fn read_sot(&mut self, tile_count: u64) -> Result<TilePart, DecodeError> {
        let segment = self.read_segment()?;
        if segment.marker != SOT || segment.payload.len() != 8 {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTilePart,
            ));
        }
        let tile_index = read_u16([segment.payload[0], segment.payload[1]]);
        let length = read_u32([
            segment.payload[2],
            segment.payload[3],
            segment.payload[4],
            segment.payload[5],
        ]);
        let tile_part_index = segment.payload[6];
        let tile_part_count = segment.payload[7];
        if u64::from(tile_index) >= tile_count
            || (tile_part_count != 0 && tile_part_index >= tile_part_count)
            || (length != 0 && length < 14)
        {
            return Err(DecodeError::malformed(
                self.offset(segment.offset),
                MalformedKind::InvalidTilePart,
            ));
        }
        Ok(TilePart {
            start: segment.offset,
            length,
            tile_index,
            tile_part_index,
            tile_part_count,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn read_tile_part(
        &mut self,
        tile_part: TilePart,
        default_parameters: (CodingStyle, Quantization),
        inherited_coding_styles: ComponentCodingStyles,
        inherited_quantizations: ComponentQuantizations,
        inherited_roi_shifts: RoiShifts,
        inherited_progression_changes: ProgressionChanges,
        mut mct_parameters: MctParameters,
        mut part2_extensions: Part2Extensions,
        components: u16,
        main_packet_lengths: PacketLengths<'a>,
        main_packed_packet_headers: PackedPacketHeaders<'a>,
    ) -> Result<(TilePartPayload<'a>, MctParameters, Part2Extensions), DecodeError> {
        let tile_header_start = self.position;
        let tile_header_end;
        let mut tile_coding_style = default_parameters.0;
        let mut tile_cod_seen = false;
        let mut qcd_segment = None;
        let mut coc_segments = [None; MAX_COMPONENTS];
        let mut qcc_segments = [None; MAX_COMPONENTS];
        let mut tile_roi_shifts = inherited_roi_shifts;
        let mut roi_seen = [false; MAX_COMPONENTS];
        let mut tile_rgn_seen = false;
        let mut has_functional_header_marker = false;
        let mut plt_seen = false;
        let mut next_plt_index = 0_u16;
        let mut plt_packet_count = 0_u64;
        let mut plt_encoded_bytes = 0_u64;
        let mut plt_payload_bytes = 0_u64;
        let mut ppt_bytes = Vec::new();
        let mut next_ppt_index = 0_u16;
        let mut ppt_fragments = 0_u16;
        let mut progression_changes = inherited_progression_changes;
        let mut poc_seen = false;
        let mut tile_mco_seen = false;
        let mut pending_mct = Vec::new();
        let mut completed_mct = Vec::new();
        let mut pending_mcc = Vec::new();
        let mut completed_mcc = Vec::new();
        let input_length = self.input.len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases input_length as int - self.position as int,)
        )]
        loop {
            let previous_position = self.position;
            match self.peek_marker()? {
                SOD => {
                    tile_header_end = self.position;
                    self.expect_standalone(SOD)?;
                    break;
                }
                marker if crate::refinement::tile_header_boundary(marker) => {
                    return Err(self.malformed_here(MalformedKind::InvalidMarkerOrder));
                }
                _ => {
                    let segment = self.read_segment()?;
                    if !is_known_segment_marker(segment.marker) {
                        return Err(DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::InvalidMarker,
                        ));
                    }
                    if !is_allowed_tile_header(segment.marker) {
                        return Err(DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::InvalidMarkerOrder,
                        ));
                    }
                    if segment.marker != COM && segment.marker != PLT {
                        has_functional_header_marker = true;
                    }
                    if segment.marker == COD {
                        if tile_cod_seen {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerOrder,
                            ));
                        }
                        tile_cod_seen = true;
                        tile_coding_style = self.parse_cod(segment)?;
                    } else if segment.marker == COC {
                        let (component, _) = self.parse_component_index(
                            segment,
                            components,
                            MalformedKind::InvalidCodingStyle,
                        )?;
                        let slot =
                            coc_segments
                                .get_mut(usize::from(component))
                                .ok_or_else(|| {
                                    DecodeError::malformed(
                                        self.offset(segment.offset),
                                        MalformedKind::InvalidCodingStyle,
                                    )
                                })?;
                        if slot.is_some() {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerOrder,
                            ));
                        }
                        *slot = Some(segment);
                    } else if segment.marker == QCD {
                        if qcd_segment.is_some() {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerOrder,
                            ));
                        }
                        qcd_segment = Some(segment);
                    } else if segment.marker == QCC {
                        let (component, _) = self.parse_component_index(
                            segment,
                            components,
                            MalformedKind::InvalidQuantization,
                        )?;
                        let slot =
                            qcc_segments
                                .get_mut(usize::from(component))
                                .ok_or_else(|| {
                                    DecodeError::malformed(
                                        self.offset(segment.offset),
                                        MalformedKind::InvalidQuantization,
                                    )
                                })?;
                        if slot.is_some() {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerOrder,
                            ));
                        }
                        *slot = Some(segment);
                    } else if segment.marker == RGN {
                        let (component, shift) = self.parse_rgn(segment, components)?;
                        let index = usize::from(component);
                        if index >= roi_seen.len() || roi_seen[index] {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerOrder,
                            ));
                        }
                        roi_seen[index] = true;
                        tile_rgn_seen = true;
                        tile_roi_shifts.set(component, shift)?;
                    } else if segment.marker == MCT {
                        if let Some(array) = self.consume_mct_fragment(
                            segment,
                            &mut pending_mct,
                            &mut completed_mct,
                        )? {
                            replace_mct_array(&mut mct_parameters.arrays, array);
                        }
                    } else if segment.marker == MCC {
                        if let Some(record) = self.consume_mcc_fragment(
                            segment,
                            components,
                            &mut pending_mcc,
                            &mut completed_mcc,
                        )? {
                            replace_mct_record(&mut mct_parameters.records, record);
                        }
                    } else if segment.marker == MCO {
                        if tile_mco_seen {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerOrder,
                            ));
                        }
                        tile_mco_seen = true;
                        mct_parameters.order = self.parse_mco(segment)?;
                    } else if segment.marker == PLT {
                        if main_packet_lengths.is_present() {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerOrder,
                            ));
                        }
                        if segment.payload.is_empty() {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerLength,
                            ));
                        }
                        let index = u16::from(segment.payload[0]);
                        if index != next_plt_index || index > u16::from(u8::MAX) {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerOrder,
                            ));
                        }
                        next_plt_index = next_plt_index.checked_add(1).ok_or_else(|| {
                            DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::ArithmeticOverflow,
                            )
                        })?;
                        let encoded = segment.payload.get(1..).ok_or_else(|| {
                            DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerLength,
                            )
                        })?;
                        let (count, total) =
                            self.validate_packet_lengths(encoded, segment.offset)?;
                        plt_seen = true;
                        plt_packet_count =
                            plt_packet_count.checked_add(count).ok_or_else(|| {
                                DecodeError::malformed(
                                    self.offset(segment.offset),
                                    MalformedKind::ArithmeticOverflow,
                                )
                            })?;
                        plt_encoded_bytes = plt_encoded_bytes
                            .checked_add(encoded.len() as u64)
                            .ok_or_else(|| {
                                DecodeError::malformed(
                                    self.offset(segment.offset),
                                    MalformedKind::ArithmeticOverflow,
                                )
                            })?;
                        plt_payload_bytes =
                            plt_payload_bytes.checked_add(total).ok_or_else(|| {
                                DecodeError::malformed(
                                    self.offset(segment.offset),
                                    MalformedKind::ArithmeticOverflow,
                                )
                            })?;
                    } else if segment.marker == PPT {
                        if main_packed_packet_headers.is_present() {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerOrder,
                            ));
                        }
                        self.parse_packed_fragment(
                            segment,
                            PPT,
                            &mut next_ppt_index,
                            &mut ppt_fragments,
                            &mut ppt_bytes,
                        )?;
                    } else if segment.marker == POC {
                        if poc_seen || tile_part.tile_part_index != 0 {
                            return Err(DecodeError::malformed(
                                self.offset(segment.offset),
                                MalformedKind::InvalidMarkerOrder,
                            ));
                        }
                        poc_seen = true;
                        progression_changes = self.parse_poc(segment, components)?;
                    } else if segment.marker == DFS {
                        self.parse_dfs(segment, &mut part2_extensions)?;
                    } else if segment.marker == ADS {
                        self.parse_ads(segment, &mut part2_extensions)?;
                    } else if segment.marker == NLT {
                        self.parse_nlt(segment, components, &mut part2_extensions)?;
                    } else if segment.marker == CBD {
                        return Err(DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::InvalidMarkerOrder,
                        ));
                    } else if segment.marker == ATK {
                        self.parse_atk(segment, &mut part2_extensions)?;
                    } else if segment.marker != 0xff64 {
                        return Err(DecodeError::malformed(
                            self.offset(segment.offset),
                            MalformedKind::InvalidMarker,
                        ));
                    }
                }
            }
            if !crate::refinement::parser_progress(previous_position, self.position, input_length) {
                return Err(self.malformed_here(MalformedKind::InvalidTilePart));
            }
        }
        if !pending_mct.is_empty() || !pending_mcc.is_empty() {
            return Err(self.malformed_here(MalformedKind::MissingRequiredStructure));
        }
        let mut component_coding_styles = if tile_cod_seen {
            ComponentCodingStyles::new(components, tile_coding_style)?
        } else {
            inherited_coding_styles
        };
        let mut component = 0_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= components,
                decreases components - component,
            )
        )]
        while component < components {
            let coc_segment = match coc_segments.get(usize::from(component)) {
                Some(segment) => *segment,
                None => None,
            };
            if let Some(segment) = coc_segment {
                let (_, style) = self.parse_coc(segment, components, tile_coding_style)?;
                component_coding_styles.set(component, style)?;
            }
            component = component
                .checked_add(1)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        }
        self.resolve_decomposition_structures(&mut component_coding_styles, &part2_extensions)?;
        let tile_quantization = match qcd_segment {
            Some(segment) => self.parse_qcd(segment, tile_coding_style)?,
            None => default_parameters.1,
        };
        let mut component_quantizations = if qcd_segment.is_some() {
            ComponentQuantizations::new(components, tile_quantization)?
        } else {
            inherited_quantizations
        };
        component = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= components,
                decreases components - component,
            )
        )]
        while component < components {
            let qcc_segment = match qcc_segments.get(usize::from(component)) {
                Some(segment) => *segment,
                None => None,
            };
            if let Some(segment) = qcc_segment {
                let style = component_coding_styles.style(component)?;
                let (_, quantization) = self.parse_qcc(segment, components, style)?;
                component_quantizations.set(component, quantization)?;
            }
            if !component_quantizations
                .quantization(component)?
                .is_compatible_with(component_coding_styles.style(component)?)
            {
                return Err(DecodeError::malformed(
                    0,
                    MalformedKind::InvalidQuantization,
                ));
            }
            component = component
                .checked_add(1)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        }
        self.validate_progression_changes(
            &progression_changes,
            tile_coding_style.layers,
            &component_coding_styles,
            tile_part.start,
        )?;
        let multiple_component_transform =
            self.resolve_multiple_component_transform(&mct_parameters)?;

        let data_start = self.position;
        if tile_part.length == 0 {
            let end = self.find_eoc()?;
            self.position = end;
            let data = self
                .input
                .get(data_start..end)
                .ok_or_else(|| self.malformed_here(MalformedKind::InvalidTilePart))?;
            let packet_lengths = self.finish_packet_lengths(
                tile_header_start,
                tile_header_end,
                main_packet_lengths,
                plt_seen,
                plt_packet_count,
                plt_encoded_bytes,
                plt_payload_bytes,
                data.len(),
                if main_packed_packet_headers.is_present() {
                    main_packed_packet_headers.len()
                } else {
                    ppt_bytes.len()
                },
                tile_part.start,
            )?;
            let packed_packet_headers = self.finish_packed_packet_headers(
                tile_header_start,
                tile_header_end,
                main_packed_packet_headers,
                ppt_fragments,
                ppt_bytes.len(),
                tile_part.start,
            )?;
            return Ok((
                TilePartPayload {
                    tile_index: tile_part.tile_index,
                    tile_part_index: tile_part.tile_part_index,
                    tile_part_count: tile_part.tile_part_count,
                    coding_style: tile_coding_style,
                    quantization: tile_quantization,
                    component_coding_styles,
                    component_quantizations,
                    roi_shifts: tile_roi_shifts,
                    progression_changes,
                    multiple_component_transform,
                    part2_extensions: part2_extensions.clone(),
                    packet_lengths,
                    packed_packet_headers,
                    has_functional_header_marker,
                    has_roi_marker: tile_rgn_seen,
                    data,
                },
                mct_parameters,
                part2_extensions,
            ));
        }

        let tile_part_length = usize::try_from(tile_part.length)
            .map_err(|_| self.malformed_here(MalformedKind::InvalidTilePart))?;
        let end = tile_part
            .start
            .checked_add(tile_part_length)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        if end < self.position || end > self.input.len() {
            return Err(DecodeError::malformed(
                self.offset(tile_part.start),
                MalformedKind::InvalidTilePart,
            ));
        }
        let data = self
            .input
            .get(data_start..end)
            .ok_or_else(|| self.malformed_here(MalformedKind::InvalidTilePart))?;
        let packet_lengths = self.finish_packet_lengths(
            tile_header_start,
            tile_header_end,
            main_packet_lengths,
            plt_seen,
            plt_packet_count,
            plt_encoded_bytes,
            plt_payload_bytes,
            data.len(),
            if main_packed_packet_headers.is_present() {
                main_packed_packet_headers.len()
            } else {
                ppt_bytes.len()
            },
            tile_part.start,
        )?;
        let packed_packet_headers = self.finish_packed_packet_headers(
            tile_header_start,
            tile_header_end,
            main_packed_packet_headers,
            ppt_fragments,
            ppt_bytes.len(),
            tile_part.start,
        )?;
        self.position = end;
        Ok((
            TilePartPayload {
                tile_index: tile_part.tile_index,
                tile_part_index: tile_part.tile_part_index,
                tile_part_count: tile_part.tile_part_count,
                coding_style: tile_coding_style,
                quantization: tile_quantization,
                component_coding_styles,
                component_quantizations,
                roi_shifts: tile_roi_shifts,
                progression_changes,
                multiple_component_transform,
                part2_extensions: part2_extensions.clone(),
                packet_lengths,
                packed_packet_headers,
                has_functional_header_marker,
                has_roi_marker: tile_rgn_seen,
                data,
            },
            mct_parameters,
            part2_extensions,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_packet_lengths(
        &self,
        tile_header_start: usize,
        tile_header_end: usize,
        main_packet_lengths: PacketLengths<'a>,
        plt_seen: bool,
        plt_packet_count: u64,
        plt_encoded_bytes: u64,
        plt_payload_bytes: u64,
        data_length: usize,
        packed_header_length: usize,
        tile_part_offset: usize,
    ) -> Result<PacketLengths<'a>, DecodeError> {
        let packet_lengths = if plt_seen {
            PacketLengths {
                source: PacketLengthSource::Plt(
                    self.input
                        .get(tile_header_start..tile_header_end)
                        .ok_or_else(|| {
                            DecodeError::malformed(
                                self.offset(tile_part_offset),
                                MalformedKind::InvalidTilePart,
                            )
                        })?,
                ),
                packet_count: plt_packet_count,
                encoded_bytes: plt_encoded_bytes,
            }
        } else {
            main_packet_lengths
        };
        if packet_lengths.is_present() {
            let declared_payload_bytes = if plt_seen {
                plt_payload_bytes
            } else {
                sum_packet_lengths(packet_lengths.source)?
            };
            let logical_payload_bytes = (data_length as u64)
                .checked_add(packed_header_length as u64)
                .ok_or_else(|| {
                    DecodeError::malformed(
                        self.offset(tile_part_offset),
                        MalformedKind::ArithmeticOverflow,
                    )
                })?;
            if declared_payload_bytes != logical_payload_bytes {
                return Err(DecodeError::malformed(
                    self.offset(tile_part_offset),
                    MalformedKind::InvalidPacketLength,
                ));
            }
        }
        Ok(packet_lengths)
    }

    fn finish_packed_packet_headers(
        &self,
        tile_header_start: usize,
        tile_header_end: usize,
        main: PackedPacketHeaders<'a>,
        ppt_fragments: u16,
        ppt_length: usize,
        tile_part_offset: usize,
    ) -> Result<PackedPacketHeaders<'a>, DecodeError> {
        if ppt_fragments == 0 {
            return Ok(main);
        }
        let header = self
            .input
            .get(tile_header_start..tile_header_end)
            .ok_or_else(|| {
                DecodeError::malformed(
                    self.offset(tile_part_offset),
                    MalformedKind::InvalidTilePart,
                )
            })?;
        Ok(PackedPacketHeaders {
            source: PackedPacketHeaderSource::MarkerSegments {
                header,
                marker: PPT,
                logical_start: 0,
            },
            length: ppt_length,
            fragments: ppt_fragments,
        })
    }

    fn find_eoc(&mut self) -> Result<usize, DecodeError> {
        let input_len = self.input.len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                decreases input_len as int - self.position as int,
            )
        )]
        while self.position < input_len {
            if input_len - self.position < 2 {
                break;
            }
            let next_position = self
                .position
                .checked_add(1)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            if self.position >= self.input.len() || next_position >= self.input.len() {
                return Err(self.malformed_here(MalformedKind::Truncated));
            }
            let is_eoc = self.input[self.position] == 0xff && self.input[next_position] == 0xd9;
            self.charge_work(1)?;
            if is_eoc {
                return Ok(self.position);
            }
            if next_position <= self.position || next_position > input_len {
                return Err(self.malformed_here(MalformedKind::ArithmeticOverflow));
            }
            self.position = next_position;
        }
        Err(self.malformed_here(MalformedKind::Truncated))
    }

    fn read_segment(&mut self) -> Result<Segment<'a>, DecodeError> {
        let offset = self.position;
        let marker = self.peek_marker()?;
        self.charge_work(1)?;
        if is_standalone_marker(marker) && marker != SOT {
            return Err(self.malformed_here(MalformedKind::InvalidMarkerOrder));
        }
        let length_offset = offset
            .checked_add(2)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        if self.input.len().saturating_sub(length_offset) < 2 {
            return Err(self.malformed_here(MalformedKind::Truncated));
        }
        let length = usize::from(read_u16([
            self.input[length_offset],
            self.input[length_offset + 1],
        ]));
        if length < 2 {
            return Err(self.malformed_here(MalformedKind::InvalidMarkerLength));
        }
        let end = length_offset
            .checked_add(length)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        if end > self.input.len() {
            return Err(self.malformed_here(MalformedKind::Truncated));
        }
        let payload_start = length_offset + 2;
        if payload_start > end {
            return Err(self.malformed_here(MalformedKind::ArithmeticOverflow));
        }
        self.position = end;
        Ok(Segment {
            marker,
            offset,
            payload: &self.input[payload_start..end],
        })
    }

    fn expect_standalone(&mut self, expected: u16) -> Result<(), DecodeError> {
        if self.peek_marker()? != expected {
            return Err(self.malformed_here(MalformedKind::InvalidMarkerOrder));
        }
        self.position = self
            .position
            .checked_add(2)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        self.charge_work(1)
    }

    fn peek_marker(&self) -> Result<u16, DecodeError> {
        if self.input.len().saturating_sub(self.position) < 2 {
            return Err(self.malformed_here(MalformedKind::Truncated));
        }
        if self.input[self.position] != 0xff || matches!(self.input[self.position + 1], 0x00 | 0xff)
        {
            return Err(self.malformed_here(MalformedKind::InvalidMarker));
        }
        Ok(read_u16([
            self.input[self.position],
            self.input[self.position + 1],
        ]))
    }

    fn charge_work(&mut self, amount: u64) -> Result<(), DecodeError> {
        self.work_units = self
            .work_units
            .checked_add(amount)
            .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        if self.work_units > self.limits.maximum_work_units() {
            return Err(DecodeError::limit(
                Resource::WorkUnits,
                self.limits.maximum_work_units(),
                self.work_units,
            ));
        }
        Ok(())
    }

    fn malformed_here(&self, kind: MalformedKind) -> DecodeError {
        DecodeError::malformed(self.offset(self.position), kind)
    }

    fn offset(&self, value: usize) -> u64 {
        value as u64
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_codestream_profile(
    r_siz: u16,
    extended_capabilities: Option<ExtendedCapabilities>,
    profile: Option<ProfileNumber>,
    corresponding_profile: Option<ProfileNumber>,
    main_styles: &ComponentCodingStyles,
    main_roi_shifts: &RoiShifts,
    tile_parts: &[TilePartPayload<'_>],
    components: u16,
    main_has_roi_marker: bool,
) -> Result<CodestreamProfile, DecodeError> {
    let mut has_legacy = false;
    let mut has_ht_only = false;
    let mut has_mixed = false;
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= components,
            decreases components - component,
        )
    )]
    while component < components {
        observe_block_coding(
            main_styles.style(component)?.code_block_style,
            &mut has_legacy,
            &mut has_ht_only,
            &mut has_mixed,
        )?;
        component = component
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }
    let mut tile_part_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant tile_part_index <= tile_parts.len(),
            decreases tile_parts.len() - tile_part_index,
        )
    )]
    while tile_part_index < tile_parts.len() {
        let tile_part = tile_parts
            .get(tile_part_index)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        component = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= components,
                decreases components - component,
            )
        )]
        while component < components {
            observe_block_coding(
                tile_part
                    .component_coding_styles
                    .style(component)?
                    .code_block_style,
                &mut has_legacy,
                &mut has_ht_only,
                &mut has_mixed,
            )?;
            component = component
                .checked_add(1)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        }
        tile_part_index = tile_part_index
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }

    let cap_part15 = match &extended_capabilities {
        Some(capabilities) => capabilities.advertises_part15(),
        None => false,
    };
    let r_siz_part15 = r_siz & HT_RSIZ_FLAG != 0;
    let styles_part15 = has_ht_only || has_mixed;
    if !crate::refinement::part15_signaling_valid(
        r_siz_part15,
        cap_part15,
        styles_part15,
        corresponding_profile.is_some(),
    ) {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    if cap_part15 {
        let capabilities = extended_capabilities
            .as_ref()
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidProfile))?;
        let value = capabilities
            .part15_value()
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidProfile))?;
        if !crate::refinement::part15_capability_fields_valid(
            (value >> 14) as u8,
            ((value & 0x07c0) >> 6) as u8,
            (value & 0x1f) as u8,
        ) {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
        }
        let block_mode = capabilities
            .part15_block_mode()
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidProfile))?;
        let mode = match block_mode {
            Part15BlockMode::HtOnly => 0,
            Part15BlockMode::HtDeclared => 2,
            Part15BlockMode::Mixed => 3,
        };
        if !crate::refinement::part15_mode_allows(mode, has_legacy, has_mixed) {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
        }
        if !capabilities.part15_allows_roi()
            && (main_has_roi_marker || tile_parts.iter().any(|part| part.has_roi_marker))
        {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
        }
        if !crate::refinement::part15_headers_allowed(
            capabilities.part15_allows_heterogeneous_headers(),
            tile_parts
                .iter()
                .any(|part| part.has_functional_header_marker),
        ) {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
        }
        validate_part15_component_capabilities(
            capabilities,
            main_styles,
            main_roi_shifts,
            components,
            main_has_roi_marker,
        )?;
        let mut tile_part_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant tile_part_index <= tile_parts.len(),
                decreases tile_parts.len() - tile_part_index,
            )
        )]
        while tile_part_index < tile_parts.len() {
            let tile_part = tile_parts
                .get(tile_part_index)
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
            validate_part15_component_capabilities(
                capabilities,
                &tile_part.component_coding_styles,
                &tile_part.roi_shifts,
                components,
                tile_part.has_roi_marker,
            )?;
            tile_part_index += 1;
        }
    } else if styles_part15 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    let block_coding =
        match crate::refinement::part15_codestream_class(has_legacy, has_ht_only, has_mixed) {
            2 => CodestreamBlockCoding::Mixed,
            1 => CodestreamBlockCoding::HighThroughput,
            _ => CodestreamBlockCoding::Legacy,
        };
    Ok(CodestreamProfile {
        r_siz,
        block_coding,
        extended_capabilities,
        profile,
        corresponding_profile,
    })
}

fn validate_part15_component_capabilities(
    capabilities: &ExtendedCapabilities,
    styles: &ComponentCodingStyles,
    roi_shifts: &RoiShifts,
    components: u16,
    has_roi_marker: bool,
) -> Result<(), DecodeError> {
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= components,
            decreases components - component,
        )
    )]
    while component < components {
        let style = styles.style(component)?;
        let roi_shift = roi_shifts.shift(component)?;
        let block_coding = Tier1BlockCoding::from_code_block_style(style.code_block_style)
            .map_err(|_| DecodeError::malformed(0, MalformedKind::InvalidCodingStyle))?;
        if !crate::refinement::part15_component_allowed(
            capabilities.part15_allows_roi(),
            has_roi_marker,
            roi_shift,
            capabilities.part15_allows_irreversible_ht(),
            block_coding.uses_high_throughput(),
            matches!(style.transform, WaveletTransform::Irreversible9x7),
        ) {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
        }
        component = component
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }
    if has_roi_marker && !capabilities.part15_allows_roi() {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    Ok(())
}

fn observe_block_coding(
    style: u8,
    has_legacy: &mut bool,
    has_ht_only: &mut bool,
    has_mixed: &mut bool,
) -> Result<(), DecodeError> {
    match Tier1BlockCoding::from_code_block_style(style) {
        Ok(Tier1BlockCoding::Legacy) => *has_legacy = true,
        Ok(Tier1BlockCoding::HighThroughput) => *has_ht_only = true,
        Ok(Tier1BlockCoding::Mixed) => *has_mixed = true,
        Err(_) => return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle)),
    }
    Ok(())
}

fn count_set_bits(value: u32) -> u8 {
    let mut count = 0_u8;
    let mut bit = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant bit <= 32,
            decreases 32 - bit,
        )
    )]
    while bit < 32 {
        if value & (1_u32 << u32::from(bit)) != 0 {
            count = match count.checked_add(1) {
                Some(value) => value,
                None => return count,
            };
        }

        bit += 1;
    }

    count
}

fn maximum_u32_for_bits(bits: u8) -> Result<u32, DecodeError> {
    if bits == 0 || bits > 31 {
        return Err(DecodeError::malformed(0, MalformedKind::ArithmeticOverflow));
    }
    let mut value = 0_u32;
    let mut bit = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant bit <= bits,
            decreases bits - bit,
        )
    )]
    while bit < bits {
        value = value
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        bit = bit
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    }
    Ok(value)
}

fn validate_unique_mct_array_indexes(
    arrays: &[MctArray],
    marker_offset: usize,
) -> Result<(), DecodeError> {
    let Some(last) = arrays.last() else {
        return Ok(());
    };
    if arrays[..arrays.len() - 1]
        .iter()
        .any(|array| array.index == last.index)
    {
        return Err(DecodeError::malformed(
            marker_offset as u64,
            MalformedKind::InvalidTransform,
        ));
    }
    Ok(())
}

fn validate_unique_mct_record_indexes(
    records: &[MctRecord],
    marker_offset: usize,
) -> Result<(), DecodeError> {
    let Some(last) = records.last() else {
        return Ok(());
    };
    if records[..records.len() - 1]
        .iter()
        .any(|record| record.index == last.index)
    {
        return Err(DecodeError::malformed(
            marker_offset as u64,
            MalformedKind::InvalidTransform,
        ));
    }
    Ok(())
}

fn replace_mct_array(arrays: &mut Vec<MctArray>, replacement: MctArray) {
    if let Some(array) = arrays
        .iter_mut()
        .find(|array| array.index == replacement.index)
    {
        *array = replacement;
    } else {
        arrays.push(replacement);
    }
}

fn replace_mct_record(records: &mut Vec<MctRecord>, replacement: MctRecord) {
    if let Some(record) = records
        .iter_mut()
        .find(|record| record.index == replacement.index)
    {
        *record = replacement;
    } else {
        records.push(replacement);
    }
}

fn referenced_mct_array(
    arrays: &[MctArray],
    index: u8,
    kind: MctArrayKind,
) -> Result<&MctArray, DecodeError> {
    let mut matches = arrays
        .iter()
        .filter(|array| array.index == index && mct_array_kinds_equal(array.kind, kind));
    let array = matches
        .next()
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTransform))?;
    if matches.next().is_some() {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
    }
    Ok(array)
}

fn validate_parallel_collections(
    collections: &[MctCollectionReference],
    marker_offset: usize,
) -> Result<(), DecodeError> {
    let mut collection_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant collection_index <= collections.len(),
            decreases collections.len() - collection_index,
        )
    )]
    while collection_index < collections.len() {
        let collection = collections.get(collection_index).ok_or_else(|| {
            DecodeError::malformed(marker_offset as u64, MalformedKind::InvalidTransform)
        })?;
        let mut previous_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant previous_index <= collection_index,
                decreases collection_index - previous_index,
            )
        )]
        while previous_index < collection_index {
            let previous = collections.get(previous_index).ok_or_else(|| {
                DecodeError::malformed(marker_offset as u64, MalformedKind::InvalidTransform)
            })?;
            if component_lists_overlap(&collection.input_components, &previous.input_components)
                || component_lists_overlap(
                    &collection.input_components,
                    &previous.output_components,
                )
                || component_lists_overlap(
                    &previous.input_components,
                    &collection.output_components,
                )
                || component_lists_overlap(
                    &previous.output_components,
                    &collection.output_components,
                )
            {
                return Err(DecodeError::malformed(
                    marker_offset as u64,
                    MalformedKind::InvalidTransform,
                ));
            }

            fn component_lists_overlap(left: &[u16], right: &[u16]) -> bool {
                let mut left_index = 0_usize;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant left_index <= left.len(),
                        decreases left.len() - left_index,
                    )
                )]
                while left_index < left.len() {
                    let mut right_index = 0_usize;
                    #[cfg_attr(
                        verus_keep_ghost,
                        verus_spec(
                            invariant right_index <= right.len(),
                            decreases right.len() - right_index,
                        )
                    )]
                    while right_index < right.len() {
                        if left.get(left_index) == right.get(right_index) {
                            return true;
                        }
                        right_index += 1;
                    }
                    left_index += 1;
                }
                false
            }
            previous_index = previous_index.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(marker_offset as u64, MalformedKind::ArithmeticOverflow)
            })?;
        }
        collection_index = collection_index.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(marker_offset as u64, MalformedKind::ArithmeticOverflow)
        })?;
    }
    Ok(())
}

fn parse_mcc_component_list(
    payload: &[u8],
    position: &mut usize,
    components: u16,
    marker_offset: usize,
) -> Result<Vec<u16>, DecodeError> {
    if payload.len().saturating_sub(*position) < 2 {
        return Err(DecodeError::malformed(
            marker_offset as u64,
            MalformedKind::InvalidTransform,
        ));
    }
    let encoded_count = read_u16([payload[*position], payload[*position + 1]]);
    *position = position.checked_add(2).ok_or_else(|| {
        DecodeError::malformed(marker_offset as u64, MalformedKind::ArithmeticOverflow)
    })?;
    let reference_bytes = if encoded_count & 0x8000 == 0 {
        1_usize
    } else {
        2_usize
    };
    let count = encoded_count & 0x7fff;
    if count == 0 || count > components {
        return Err(DecodeError::malformed(
            marker_offset as u64,
            MalformedKind::InvalidTransform,
        ));
    }
    let required = usize::from(count)
        .checked_mul(reference_bytes)
        .ok_or_else(|| {
            DecodeError::malformed(marker_offset as u64, MalformedKind::ArithmeticOverflow)
        })?;
    if payload.len().saturating_sub(*position) < required {
        return Err(DecodeError::malformed(
            marker_offset as u64,
            MalformedKind::InvalidTransform,
        ));
    }
    let mut references = Vec::with_capacity(usize::from(count));
    let mut reference_index = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant reference_index <= count,
            decreases count - reference_index,
        )
    )]
    while reference_index < count {
        let current = *payload.get(*position).ok_or_else(|| {
            DecodeError::malformed(marker_offset as u64, MalformedKind::InvalidTransform)
        })?;
        let reference = if reference_bytes == 1 {
            u16::from(current)
        } else {
            let next = position.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(marker_offset as u64, MalformedKind::ArithmeticOverflow)
            })?;
            read_u16([
                current,
                *payload.get(next).ok_or_else(|| {
                    DecodeError::malformed(marker_offset as u64, MalformedKind::InvalidTransform)
                })?,
            ])
        };
        if reference >= components {
            return Err(DecodeError::malformed(
                marker_offset as u64,
                MalformedKind::InvalidTransform,
            ));
        }
        if u16_list_contains(&references, reference) {
            return Err(DecodeError::malformed(
                marker_offset as u64,
                MalformedKind::InvalidTransform,
            ));
        }

        fn u16_list_contains(values: &[u16], value: u16) -> bool {
            let mut index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant index <= values.len(),
                    decreases values.len() - index,
                )
            )]
            while index < values.len() {
                if values.get(index) == Some(&value) {
                    return true;
                }
                index += 1;
            }
            false
        }
        references.push(reference);
        *position = position.checked_add(reference_bytes).ok_or_else(|| {
            DecodeError::malformed(marker_offset as u64, MalformedKind::ArithmeticOverflow)
        })?;
        reference_index = reference_index.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(marker_offset as u64, MalformedKind::ArithmeticOverflow)
        })?;
    }
    Ok(references)
}

fn float32_to_fixed(bits: u32) -> Result<i64, ()> {
    let negative = bits >> 31 != 0;
    let exponent = (bits >> 23) & 0xff;
    let fraction = bits & 0x7f_ffff;
    if exponent == 0xff {
        return Err(());
    }
    if exponent == 0 && fraction == 0 {
        return Ok(0);
    }
    let (significand, binary_exponent) = if exponent == 0 {
        (u128::from(fraction), -149_i32)
    } else {
        (
            u128::from((1_u32 << 23) | fraction),
            i32::try_from(exponent).map_err(|_| ())? - 150,
        )
    };
    fixed_from_binary(negative, significand, binary_exponent)
}

fn float64_to_fixed(bits: u64) -> Result<i64, ()> {
    let negative = bits >> 63 != 0;
    let exponent = (bits >> 52) & 0x7ff;
    let fraction = bits & 0x000f_ffff_ffff_ffff;
    if exponent == 0x7ff {
        return Err(());
    }
    if exponent == 0 && fraction == 0 {
        return Ok(0);
    }
    let (significand, binary_exponent) = if exponent == 0 {
        (u128::from(fraction), -1074_i32)
    } else {
        (
            u128::from((1_u64 << 52) | fraction),
            i32::try_from(exponent).map_err(|_| ())? - 1075,
        )
    };
    fixed_from_binary(negative, significand, binary_exponent)
}

fn fixed_from_binary(negative: bool, significand: u128, binary_exponent: i32) -> Result<i64, ()> {
    if binary_exponent < i32::MIN + i32::from(MCT_FIXED_BITS)
        || binary_exponent > i32::MAX - i32::from(MCT_FIXED_BITS)
    {
        return Err(());
    }
    let shift = crate::refinement::binary_fixed_shift(binary_exponent);
    let magnitude = if shift >= 0 {
        let shift = u32::try_from(shift).map_err(|_| ())?;
        if shift >= 128 || significand > (u128::MAX >> shift) {
            return Err(());
        }
        significand << shift
    } else {
        let right = u32::try_from(0_i32.checked_sub(shift).ok_or(())?).map_err(|_| ())?;
        if right >= 128 {
            0
        } else {
            let truncated = significand >> right;
            let remainder_mask = if right == 0 {
                0
            } else {
                (1_u128 << right).checked_sub(1).ok_or(())?
            };
            let remainder = significand & remainder_mask;
            let half = if right == 0 { 0 } else { 1_u128 << (right - 1) };
            let round_up = remainder > half || (remainder == half && truncated & 1 != 0);
            truncated.checked_add(u128::from(round_up)).ok_or(())?
        }
    };
    if negative {
        let signed = i128::try_from(magnitude).map_err(|_| ())?;
        i64::try_from(0_i128.checked_sub(signed).ok_or(())?).map_err(|_| ())
    } else {
        i64::try_from(magnitude).map_err(|_| ())
    }
}

const fn is_standalone_marker(marker: u16) -> bool {
    matches!(marker, SOC | SOD | EOC | 0xff92)
}

const fn is_known_segment_marker(marker: u16) -> bool {
    matches!(
        marker,
        CAP | SIZ
            | 0xff52
            | 0xff53
            | TLM
            | PRF
            | 0xff57
            | 0xff58
            | CPF
            | 0xff5c
            | 0xff5d
            | RGN
            | POC
            | 0xff60
            | 0xff61
            | CRG
            | 0xff64
            | DFS
            | ADS
            | 0xff74
            | 0xff75
            | NLT
            | 0xff77
            | CBD
            | ATK
            | SOT
    )
}

const fn is_allowed_main_header(marker: u16) -> bool {
    matches!(
        marker,
        CAP | COD
            | 0xff53
            | TLM
            | PRF
            | 0xff57
            | CPF
            | QCD
            | 0xff5d
            | RGN
            | POC
            | 0xff60
            | CRG
            | 0xff64
            | DFS
            | ADS
            | 0xff74
            | 0xff75
            | NLT
            | 0xff77
            | CBD
            | ATK
    )
}

const fn is_allowed_tile_header(marker: u16) -> bool {
    matches!(
        marker,
        COD | 0xff53
            | 0xff58
            | QCD
            | 0xff5d
            | RGN
            | POC
            | 0xff61
            | 0xff64
            | DFS
            | ADS
            | 0xff74
            | 0xff75
            | NLT
            | 0xff77
            | CBD
            | ATK
    )
}

const fn progression_orders_equal(left: ProgressionOrder, right: ProgressionOrder) -> bool {
    matches!(
        (left, right),
        (ProgressionOrder::Lrcp, ProgressionOrder::Lrcp)
            | (ProgressionOrder::Rlcp, ProgressionOrder::Rlcp)
            | (ProgressionOrder::Rpcl, ProgressionOrder::Rpcl)
            | (ProgressionOrder::Pcrl, ProgressionOrder::Pcrl)
            | (ProgressionOrder::Cprl, ProgressionOrder::Cprl)
    )
}

const fn decomposition_directions_equal(
    left: DecompositionDirection,
    right: DecompositionDirection,
) -> bool {
    matches!(
        (left, right),
        (DecompositionDirection::None, DecompositionDirection::None)
            | (
                DecompositionDirection::TwoDimensional,
                DecompositionDirection::TwoDimensional
            )
            | (
                DecompositionDirection::Horizontal,
                DecompositionDirection::Horizontal
            )
            | (
                DecompositionDirection::Vertical,
                DecompositionDirection::Vertical
            )
    )
}

const fn wavelet_transforms_equal(left: WaveletTransform, right: WaveletTransform) -> bool {
    match (left, right) {
        (WaveletTransform::Irreversible9x7, WaveletTransform::Irreversible9x7)
        | (WaveletTransform::Reversible5x3, WaveletTransform::Reversible5x3) => true,
        (WaveletTransform::Arbitrary(left), WaveletTransform::Arbitrary(right)) => left == right,
        _ => false,
    }
}

const fn quantization_styles_equal(left: QuantizationStyle, right: QuantizationStyle) -> bool {
    matches!(
        (left, right),
        (QuantizationStyle::None, QuantizationStyle::None)
            | (
                QuantizationStyle::ScalarDerived,
                QuantizationStyle::ScalarDerived
            )
            | (
                QuantizationStyle::ScalarExpounded,
                QuantizationStyle::ScalarExpounded
            )
    )
}

fn ceil_div(value: u32, divisor: u32) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::InvalidImageGeometry,
        ));
    }

    Ok(crate::refinement::ceil_div_u32(value, divisor))
}

fn registered_ceil_div(value: u32, divisor: u32, registration: u16) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::InvalidImageGeometry,
        ));
    }
    let scale = 65_536_u64;
    let numerator = u64::from(value)
        .checked_mul(scale)
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    if numerator <= u64::from(registration) {
        return Ok(0);
    }
    let adjusted = numerator
        .checked_sub(u64::from(registration))
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let denominator = u64::from(divisor)
        .checked_mul(scale)
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    let quotient = adjusted / denominator;
    let rounded = quotient
        .checked_add(u64::from(!adjusted.is_multiple_of(denominator)))
        .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
    u32::try_from(rounded).map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))
}

fn read_u16(bytes: [u8; 2]) -> u16 {
    crate::refinement::marker_value(bytes[0], bytes[1])
}

fn read_u32(bytes: [u8; 4]) -> u32 {
    (u32::from(bytes[0]) << 24)
        | (u32::from(bytes[1]) << 16)
        | (u32::from(bytes[2]) << 8)
        | u32::from(bytes[3])
}

fn read_u64(bytes: [u8; 8]) -> u64 {
    (u64::from(bytes[0]) << 56)
        | (u64::from(bytes[1]) << 48)
        | (u64::from(bytes[2]) << 40)
        | (u64::from(bytes[3]) << 32)
        | (u64::from(bytes[4]) << 24)
        | (u64::from(bytes[5]) << 16)
        | (u64::from(bytes[6]) << 8)
        | u64::from(bytes[7])
}

fn signed_u16(value: u16) -> i64 {
    let value = i64::from(value);
    if value < 32_768 {
        value
    } else {
        value - 65_536
    }
}

fn signed_u32(value: u32) -> i64 {
    let value = i64::from(value);
    if value < 2_147_483_648 {
        value
    } else {
        value - 4_294_967_296
    }
}

fn decode_packet_length(
    encoded: &[u8],
    start: usize,
    end: usize,
) -> Result<(u32, usize), DecodeError> {
    if start >= end || end > encoded.len() {
        return Err(DecodeError::malformed(
            start as u64,
            MalformedKind::InvalidPacketLength,
        ));
    }
    let mut position = start;
    let mut value = 0_u32;
    let mut groups = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant position <= end,
            decreases end - position,
        )
    )]
    loop {
        if position >= end || position >= encoded.len() {
            return Err(DecodeError::malformed(
                start as u64,
                MalformedKind::InvalidPacketLength,
            ));
        }
        let byte = encoded[position];
        groups = groups.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(start as u64, MalformedKind::ArithmeticOverflow)
        })?;
        if groups > 5 || (groups == 5 && value > 0x01ff_ffff) {
            return Err(DecodeError::malformed(
                start as u64,
                MalformedKind::InvalidPacketLength,
            ));
        }
        value = value
            .checked_mul(128)
            .and_then(|prefix| prefix.checked_add(u32::from(byte & 0x7f)))
            .ok_or_else(|| {
                DecodeError::malformed(start as u64, MalformedKind::InvalidPacketLength)
            })?;
        position = position.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(start as u64, MalformedKind::ArithmeticOverflow)
        })?;
        if byte & 0x80 == 0 {
            if value == 0 {
                return Err(DecodeError::malformed(
                    start as u64,
                    MalformedKind::InvalidPacketLength,
                ));
            }
            return Ok((value, position));
        }
    }
}

fn sum_packet_lengths(source: PacketLengthSource<'_>) -> Result<u64, DecodeError> {
    let bytes = match source {
        PacketLengthSource::Plm(bytes) => bytes,
        PacketLengthSource::None => return Ok(0),
        PacketLengthSource::Plt(_) => {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidPacketLength,
            ));
        }
    };
    let mut position = 0_usize;
    let mut total = 0_u64;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant position <= bytes.len(),
            decreases bytes.len() - position,
        )
    )]
    while position < bytes.len() {
        let (length, next) = decode_packet_length(bytes, position, bytes.len())?;
        if next <= position || next > bytes.len() {
            return Err(DecodeError::malformed(
                position as u64,
                MalformedKind::InvalidPacketLength,
            ));
        }
        total = total
            .checked_add(u64::from(length))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        position = next;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_component_getter_covers_entire_u16_index_domain() {
        let information = core::array::from_fn(|index| {
            let index = u8::try_from(index).unwrap();
            ComponentInfo {
                precision: index % 39,
                signed: index % 2 != 0,
                x_subsampling: index,
                y_subsampling: 255 - index,
                x_registration: u16::from(index) * 997,
                y_registration: u16::MAX - u16::from(index) * 997,
            }
        });
        for components in [0, 1, 63, 64, 65, u16::MAX] {
            let size = ImageSize {
                width: 0,
                height: u32::MAX,
                x_origin: u32::MAX,
                y_origin: 1,
                components,
                uniform_bit_depth: None,
                uniform_signed: None,
                tile_width: 0,
                tile_height: 0,
                tile_x_origin: 0,
                tile_y_origin: 0,
                tiles_across: 0,
                tiles_down: 0,
                component_info: information,
            };
            for index in 0..=u16::MAX {
                let expected = if index < components && usize::from(index) < information.len() {
                    Ok(information[usize::from(index)])
                } else {
                    Err(DecodeError::MalformedInput {
                        offset: 0,
                        kind: MalformedKind::InvalidImageGeometry,
                    })
                };
                assert_eq!(size.component(index), expected);
                assert_eq!(image_component_info(&size, index), expected);
            }
        }
    }

    #[test]
    fn decomposition_selectors_cover_entire_u8_level_stage_resolution_domains() {
        use DecompositionDirection::{Horizontal, None, TwoDimensional, Vertical};
        for pattern in 0..4 {
            let directions = core::array::from_fn(|index| {
                [None, Horizontal, Vertical, TwoDimensional][(index + pattern) % 4]
            });
            for levels in 0..=u8::MAX {
                let style = CodingStyle {
                    progression_order: ProgressionOrder::Lrcp,
                    layers: 0,
                    multiple_component_transform: false,
                    decomposition_levels: levels,
                    decomposition_structure: Option::None,
                    decomposition_directions: directions,
                    code_block_width: 0,
                    code_block_height: 0,
                    code_block_style: 0,
                    transform: WaveletTransform::Reversible5x3,
                    precincts_defined: false,
                    sop_markers: false,
                    eph_markers: false,
                    precinct_exponents: [(0, 0); 33],
                };
                let error = DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::InvalidCodingStyle,
                };
                for value in 0..=u8::MAX {
                    assert_eq!(
                        style.decomposition_direction(value),
                        if value < levels && usize::from(value) < directions.len() {
                            Ok(directions[usize::from(value)])
                        } else {
                            Err(error)
                        }
                    );
                    let stage = i16::from(levels) - i16::from(value);
                    assert_eq!(
                        style.direction_for_resolution(value),
                        if value != 0 && value <= levels && stage < 32 {
                            Ok(directions[usize::try_from(stage).unwrap()])
                        } else {
                            Err(error)
                        }
                    );
                    let expected = if value == 0 {
                        Ok(1)
                    } else if value > levels || levels > 32 {
                        Err(error)
                    } else {
                        let increments: usize = directions
                            [usize::from(levels - value)..usize::from(levels)]
                            .iter()
                            .map(|direction| match direction {
                                None => 0,
                                Horizontal | Vertical => 1,
                                TwoDimensional => 3,
                            })
                            .sum();
                        Ok(1 + increments)
                    };
                    assert_eq!(style.subband_count_through_resolution(value), expected);
                }
            }
        }
    }

    type MccCollectionFixture<'a> = (&'a [u8], &'a [u8], bool, u8, u8);

    fn segment(marker: u16, payload: &[u8]) -> Vec<u8> {
        let length = u16::try_from(payload.len() + 2).unwrap();
        let mut bytes = marker.to_be_bytes().to_vec();
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    fn encode_packet_lengths(lengths: &[u32]) -> Vec<u8> {
        let mut encoded = Vec::new();
        for &length in lengths {
            assert_ne!(length, 0);
            let mut groups = [0_u8; 5];
            let mut value = length;
            let mut start = groups.len() - 1;
            groups[start] = u8::try_from(value & 0x7f).unwrap();
            value >>= 7;
            while value != 0 {
                start -= 1;
                groups[start] = u8::try_from(value & 0x7f).unwrap() | 0x80;
                value >>= 7;
            }
            encoded.extend_from_slice(&groups[start..]);
        }
        encoded
    }

    fn plm(index: u8, groups: &[&[u32]]) -> Vec<u8> {
        let mut payload = vec![index];
        for lengths in groups {
            let encoded = encode_packet_lengths(lengths);
            payload.extend_from_slice(&u32::try_from(encoded.len()).unwrap().to_be_bytes());
            payload.extend(encoded);
        }
        segment(PLM, &payload)
    }

    fn plt(index: u8, lengths: &[u32]) -> Vec<u8> {
        let mut payload = vec![index];
        payload.extend(encode_packet_lengths(lengths));
        segment(PLT, &payload)
    }

    fn ppm(index: u8, bytes: &[u8]) -> Vec<u8> {
        let mut payload = vec![index];
        payload.extend_from_slice(bytes);
        segment(PPM, &payload)
    }

    fn ppt(index: u8, bytes: &[u8]) -> Vec<u8> {
        let mut payload = vec![index];
        payload.extend_from_slice(bytes);
        segment(PPT, &payload)
    }

    fn siz() -> Vec<u8> {
        siz_with_components(1)
    }

    fn siz_with_components(components: u16) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&0_u16.to_be_bytes());
        payload.extend_from_slice(&64_u32.to_be_bytes());
        payload.extend_from_slice(&32_u32.to_be_bytes());
        payload.extend_from_slice(&0_u32.to_be_bytes());
        payload.extend_from_slice(&0_u32.to_be_bytes());
        payload.extend_from_slice(&64_u32.to_be_bytes());
        payload.extend_from_slice(&32_u32.to_be_bytes());
        payload.extend_from_slice(&0_u32.to_be_bytes());
        payload.extend_from_slice(&0_u32.to_be_bytes());
        payload.extend_from_slice(&components.to_be_bytes());
        let mut component = 0_u16;
        while component < components {
            payload.extend_from_slice(&[7, 1, 1]);
            component += 1;
        }
        segment(SIZ, &payload)
    }

    fn codestream_with_headers(
        components: u16,
        main_segments: &[Vec<u8>],
        tile_segments: &[Vec<u8>],
        tile_data: &[u8],
    ) -> Vec<u8> {
        let mut bytes = SOC.to_be_bytes().to_vec();
        bytes.extend(siz_with_components(components));
        for marker_segment in main_segments {
            bytes.extend(marker_segment);
        }
        let tile_part_start = bytes.len();
        let mut sot_payload = Vec::new();
        sot_payload.extend_from_slice(&0_u16.to_be_bytes());
        sot_payload.extend_from_slice(&0_u32.to_be_bytes());
        sot_payload.extend_from_slice(&[0, 1]);
        bytes.extend(segment(SOT, &sot_payload));
        for marker_segment in tile_segments {
            bytes.extend(marker_segment);
        }
        bytes.extend_from_slice(&SOD.to_be_bytes());
        bytes.extend_from_slice(tile_data);
        let tile_part_length = u32::try_from(bytes.len() - tile_part_start).unwrap();
        bytes[tile_part_start + 6..tile_part_start + 10]
            .copy_from_slice(&tile_part_length.to_be_bytes());
        bytes.extend_from_slice(&EOC.to_be_bytes());
        bytes
    }

    fn baseline_headers() -> [Vec<u8>; 2] {
        [
            segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]),
            segment(QCD, &[0, 0]),
        ]
    }

    #[test]
    fn retains_selected_part2_marker_families() {
        let [cod, qcd] = baseline_headers();
        let dfs = segment(DFS, &[0, 1, 4, 0x1b]);
        let ads = segment(ADS, &[2, 1, 0x40, 1, 0]);
        let nlt = segment(NLT, &[0xff, 0xff, 7, 0]);
        let cbd = segment(CBD, &[0, 2, 7, 0x87]);
        let atk = segment(ATK, &[0x4a, 2, 0x3f, 0x80, 0, 0, 0]);
        let crg = segment(CRG, &[0, 1, 0, 2, 0, 3, 0, 4]);
        let bytes = codestream_with_headers(2, &[cod, qcd, dfs, ads, nlt, cbd, atk, crg], &[], &[]);
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        let extensions = &layout.info.part2_extensions;
        assert_eq!(extensions.decomposition_structures().len(), 1);
        assert_eq!(
            extensions.decomposition_structures()[0].directions(),
            &[
                DecompositionDirection::None,
                DecompositionDirection::TwoDimensional,
                DecompositionDirection::Horizontal,
                DecompositionDirection::Vertical,
            ]
        );
        assert_eq!(
            extensions.arbitrary_decompositions()[0].sublevel_depths(),
            &[1]
        );
        assert_eq!(
            extensions.arbitrary_decompositions()[0].subband_splits(),
            &[DecompositionDirection::None]
        );
        assert_eq!(extensions.nonlinear_transforms()[0].component(), None);
        assert_eq!(extensions.component_bit_depths()[1].precision, 8);
        assert!(extensions.component_bit_depths()[1].signed);
        assert_eq!(extensions.arbitrary_kernels()[0].index(), 2);
        assert_eq!(extensions.component_registrations()[1].horizontal, 3);
        assert_eq!(
            layout.info.image_size.component(1).unwrap().x_registration,
            3
        );
        assert_eq!(
            layout.info.image_size.component(1).unwrap().y_registration,
            4
        );
        assert_eq!(layout.tile_parts[0].part2_extensions, *extensions);
    }

    #[test]
    fn tile_header_part2_markers_override_inherited_indexes() {
        let [cod, qcd] = baseline_headers();
        let main_dfs = segment(DFS, &[0, 1, 1, 0x40]);
        let tile_dfs = segment(DFS, &[0, 1, 1, 0xc0]);
        let main_nlt = segment(NLT, &[0, 0, 7, 0]);
        let tile_nlt = segment(NLT, &[0, 0, 8, 0]);
        let bytes = codestream_with_headers(
            1,
            &[cod, qcd, main_dfs, main_nlt],
            &[tile_dfs, tile_nlt],
            &[],
        );
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        let tile = &layout.tile_parts[0].part2_extensions;
        assert_eq!(
            tile.decomposition_structures()[0].directions(),
            &[DecompositionDirection::Vertical]
        );
        assert_eq!(tile.nonlinear_transforms()[0].precision(), 9);
    }

    #[test]
    fn parses_bounded_nlt_lookup_table() {
        let [cod, qcd] = baseline_headers();
        let mut payload = vec![0xff, 0xff, 7, 2];
        payload.extend_from_slice(&1_u16.to_be_bytes());
        payload.extend_from_slice(&0_u32.to_be_bytes());
        payload.extend_from_slice(&u32::MAX.to_be_bytes());
        payload.push(8);
        payload.extend_from_slice(&[0, 255]);
        let bytes = codestream_with_headers(1, &[cod, qcd, segment(NLT, &payload)], &[], &[]);
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        let transform = &layout.info.part2_extensions.nonlinear_transforms()[0];
        assert_eq!(transform.transform_type(), 2);
        assert_eq!(transform.d_min(), Some(0));
        assert_eq!(transform.d_max(), Some(u32::MAX));
        assert_eq!(transform.point_bits(), Some(8));
        assert_eq!(transform.points(), &[0, 255]);
    }

    #[test]
    fn parses_gamma_nlt_fixed_point_parameters() {
        let [cod, qcd] = baseline_headers();
        let one = [1_u8, 0, 0];
        let mut payload = vec![0xff, 0xff, 7, 1];
        payload.extend_from_slice(&one);
        payload.extend_from_slice(&one);
        payload.extend_from_slice(&one);
        payload.extend_from_slice(&one);
        payload.extend_from_slice(&[0, 0, 0]);
        let bytes = codestream_with_headers(1, &[cod, qcd, segment(NLT, &payload)], &[], &[]);
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        assert_eq!(
            layout.info.part2_extensions.nonlinear_transforms()[0].gamma_parameters(),
            Some([1 << 16, 1 << 16, 1 << 16, 1 << 16, 0])
        );
    }

    #[test]
    fn parses_uniform_cbd_descriptor() {
        let [cod, qcd] = baseline_headers();
        let cbd = segment(CBD, &[0x80, 2, 0x87]);
        let bytes = codestream_with_headers(2, &[cod, qcd, cbd], &[], &[]);
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        assert_eq!(
            layout.info.part2_extensions.component_bit_depths(),
            &[
                ComponentBitDepth {
                    precision: 8,
                    signed: true,
                },
                ComponentBitDepth {
                    precision: 8,
                    signed: true,
                },
            ]
        );
    }

    #[test]
    fn rejects_tile_header_cbd_as_malformed() {
        let [cod, qcd] = baseline_headers();
        let bytes = codestream_with_headers(1, &[cod, qcd], &[segment(CBD, &[0, 1, 7])], &[]);
        assert!(matches!(
            parse_layout(&bytes, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));
    }

    #[test]
    fn parses_and_executes_extended_atk_forms() {
        let [cod, qcd] = baseline_headers();
        let mut binary128 = vec![0x4c, 2];
        binary128.extend_from_slice(&(0x3fff_u128 << 112).to_be_bytes());
        binary128.push(0);
        let bytes = codestream_with_headers(1, &[cod, qcd, segment(ATK, &binary128)], &[], &[]);
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        let kernel = &layout.info.part2_extensions.arbitrary_kernels()[0];
        assert_eq!(kernel.coefficient_type(), 4);
        assert!(kernel.whole_sample_filter());
        assert!(kernel.whole_sample_extension());
        assert_eq!(
            crate::transform::inverse_arbitrary(&[3, 7], &[2], true, kernel).unwrap(),
            [3, 2, 7]
        );

        let reversible = segment(ATK, &[0x58, 3, 2, 0, 0, 1, 0xff, 1, 1, 1, 1]);
        let bytes = codestream_with_headers(
            1,
            &[
                baseline_headers()[0].clone(),
                baseline_headers()[1].clone(),
                reversible,
            ],
            &[],
            &[],
        );
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        let kernel = &layout.info.part2_extensions.arbitrary_kernels()[0];
        assert!(kernel.reversible());
        assert_eq!(
            crate::transform::inverse_arbitrary(&[3, 7], &[2], true, kernel)
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn retains_dfs_and_atk_coding_style_references() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 1, 2, 2, 0, 2]);
        let qcd = segment(QCD, &[0, 0, 0, 0, 0]);
        let dfs = segment(DFS, &[0, 1, 1, 0x40]);
        let atk = segment(ATK, &[0x58, 2, 2, 0, 0, 1, 0xff, 1, 1, 1, 1]);
        let coc = segment(COC, &[0, 0, 0x81, 2, 2, 0, 1]);
        let bytes = codestream_with_headers(1, &[cod, qcd, dfs, atk, coc], &[], &[]);
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        assert_eq!(
            layout
                .info
                .component_coding_styles
                .style(0)
                .unwrap()
                .decomposition_structure,
            Some(1)
        );
        assert_eq!(
            layout.info.coding_style.transform,
            WaveletTransform::Arbitrary(2)
        );
    }

    #[test]
    fn resolves_mixed_dfs_sequence_into_bounded_topology() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 4, 2, 2, 0, 1]);
        let mut qcd_payload = vec![0];
        qcd_payload.extend_from_slice(&[0x40; 13]);
        let qcd = segment(QCD, &qcd_payload);
        let dfs = segment(DFS, &[0, 1, 4, 0x1b]);
        let coc = segment(COC, &[0, 0, 0x81, 2, 2, 0, 1]);
        let qcc = segment(QCC, &[0, 0, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40]);
        let bytes = codestream_with_headers(1, &[cod, qcd, dfs, coc, qcc], &[], &[]);
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        let style = layout.info.component_coding_styles.style(0).unwrap();
        assert_eq!(style.subband_count(), 6);
        assert_eq!(style.resolution_downsampling(0).unwrap(), (4, 4));
        assert_eq!(style.resolution_downsampling(2).unwrap(), (2, 2));
        assert_eq!(
            style.direction_for_resolution(1).unwrap(),
            DecompositionDirection::Vertical
        );
        assert_eq!(
            style.direction_for_resolution(2).unwrap(),
            DecompositionDirection::Horizontal
        );
        assert_eq!(
            style.direction_for_resolution(3).unwrap(),
            DecompositionDirection::TwoDimensional
        );
        assert_eq!(
            style.direction_for_resolution(4).unwrap(),
            DecompositionDirection::None
        );
        let quantization = layout.info.component_quantizations.quantization(0).unwrap();
        let subbands = crate::decode::derive_component_subbands(
            &layout.info.image_size,
            style,
            &quantization,
            0,
        )
        .unwrap();
        assert_eq!(subbands.len(), 6);
        assert_eq!(
            subbands
                .iter()
                .map(|band| (band.resolution, band.orientation))
                .collect::<Vec<_>>(),
            vec![
                (0, crate::tier1::SubbandOrientation::Ll),
                (1, crate::tier1::SubbandOrientation::Lh),
                (2, crate::tier1::SubbandOrientation::Hl),
                (3, crate::tier1::SubbandOrientation::Hl),
                (3, crate::tier1::SubbandOrientation::Lh),
                (3, crate::tier1::SubbandOrientation::Hh),
            ]
        );
        let plan = crate::decode::plan_progression_tile_at_with_component_overrides(
            &layout.info.image_size,
            0,
            layout.info.coding_style,
            &layout.info.component_coding_styles,
            &layout.info.component_quantizations,
            &layout.info.roi_shifts,
            &limits,
        )
        .unwrap();
        assert_eq!(plan.grid().packet_count(), 4);
        assert_eq!(plan.precincts().len(), 4);
        assert_eq!(
            plan.precincts()
                .iter()
                .map(crate::packet::PacketPrecinctDecoder::subband_count)
                .sum::<usize>(),
            6
        );
        let prepared = crate::decode::prepare_tile_component(
            &plan.components()[0],
            plan.precincts(),
            style,
            &limits,
        )
        .unwrap();
        assert_eq!((prepared.width(), prepared.height()), (64, 32));
        assert!(prepared.samples().iter().all(|sample| *sample == 0));
    }

    #[test]
    fn validates_tlm_against_sot_tile_part_length() {
        let [cod, qcd] = baseline_headers();
        let tlm = segment(TLM, &[0, 0x40, 0, 0, 0, 14]);
        let bytes = codestream_with_headers(1, &[cod, qcd, tlm], &[], &[]);
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        assert_eq!(
            layout.info.part2_extensions.tile_part_lengths(),
            &[TilePartLength {
                tile_index: None,
                length: 14,
            }]
        );

        let [cod, qcd] = baseline_headers();
        let bad_tlm = segment(TLM, &[0, 0x40, 0, 0, 0, 15]);
        let bad = codestream_with_headers(1, &[cod, qcd, bad_tlm], &[], &[]);
        assert!(matches!(
            parse_layout(&bad, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidTilePart,
                ..
            })
        ));
    }

    #[test]
    fn rejects_malformed_selected_part2_markers() {
        let [cod, qcd] = baseline_headers();
        for invalid in [
            segment(DFS, &[0, 0, 1, 0]),
            segment(NLT, &[0, 2, 7, 0]),
            segment(CBD, &[0, 2, 7]),
            segment(ATK, &[0, 1, 0]),
            segment(CRG, &[0, 0, 0]),
        ] {
            let bytes = codestream_with_headers(1, &[cod.clone(), qcd.clone(), invalid], &[], &[]);
            assert!(matches!(
                parse_layout(&bytes, &ResourceLimits::default()),
                Err(DecodeError::MalformedInput { .. })
            ));
        }
    }

    fn mct_i16(index: u8, kind: u8, values: &[i16]) -> Vec<u8> {
        let descriptor = u16::from(index) | (u16::from(kind) << 8);
        let mut payload = 0_u16.to_be_bytes().to_vec();
        payload.extend_from_slice(&descriptor.to_be_bytes());
        payload.extend_from_slice(&0_u16.to_be_bytes());
        for value in values {
            payload.extend_from_slice(&value.to_be_bytes());
        }
        segment(MCT, &payload)
    }

    fn mct_i16_segmented(index: u8, kind: u8, values: &[i16], split: usize) -> [Vec<u8>; 2] {
        let descriptor = u16::from(index) | (u16::from(kind) << 8);
        let mut data = Vec::new();
        for value in values {
            data.extend_from_slice(&value.to_be_bytes());
        }
        let mut first = 0_u16.to_be_bytes().to_vec();
        first.extend_from_slice(&descriptor.to_be_bytes());
        first.extend_from_slice(&1_u16.to_be_bytes());
        first.extend_from_slice(&data[..split]);
        let mut second = 1_u16.to_be_bytes().to_vec();
        second.extend_from_slice(&descriptor.to_be_bytes());
        second.extend_from_slice(&data[split..]);
        [segment(MCT, &first), segment(MCT, &second)]
    }

    fn mct_f32(index: u8, kind: u8, values: &[u32]) -> Vec<u8> {
        let descriptor = u16::from(index) | (u16::from(kind) << 8) | (2_u16 << 10);
        let mut payload = 0_u16.to_be_bytes().to_vec();
        payload.extend_from_slice(&descriptor.to_be_bytes());
        payload.extend_from_slice(&0_u16.to_be_bytes());
        for value in values {
            payload.extend_from_slice(&value.to_be_bytes());
        }
        segment(MCT, &payload)
    }

    fn mct_f64(index: u8, kind: u8, values: &[u64]) -> Vec<u8> {
        let descriptor = u16::from(index) | (u16::from(kind) << 8) | (3_u16 << 10);
        let mut payload = 0_u16.to_be_bytes().to_vec();
        payload.extend_from_slice(&descriptor.to_be_bytes());
        payload.extend_from_slice(&0_u16.to_be_bytes());
        for value in values {
            payload.extend_from_slice(&value.to_be_bytes());
        }
        segment(MCT, &payload)
    }

    fn mcc_collections(record: u8, collections: &[MccCollectionFixture<'_>]) -> Vec<u8> {
        let mut payload = 0_u16.to_be_bytes().to_vec();
        payload.push(record);
        payload.extend_from_slice(&0_u16.to_be_bytes());
        payload.extend_from_slice(&u16::try_from(collections.len()).unwrap().to_be_bytes());
        for &(inputs, outputs, reversible, matrix, offsets) in collections {
            payload.push(1);
            payload.extend_from_slice(&u16::try_from(inputs.len()).unwrap().to_be_bytes());
            payload.extend_from_slice(inputs);
            payload.extend_from_slice(&u16::try_from(outputs.len()).unwrap().to_be_bytes());
            payload.extend_from_slice(outputs);
            payload.extend_from_slice(&[u8::from(reversible), offsets, matrix]);
        }
        segment(MCC, &payload)
    }

    fn mcc(collection: u8, components: u16, reversible: bool, matrix: u8, offsets: u8) -> Vec<u8> {
        let mut payload = 0_u16.to_be_bytes().to_vec();
        payload.push(collection);
        payload.extend_from_slice(&0_u16.to_be_bytes());
        payload.extend_from_slice(&1_u16.to_be_bytes());
        payload.push(1);
        payload.extend_from_slice(&components.to_be_bytes());
        for component in 0..components {
            payload.push(u8::try_from(component).unwrap());
        }
        payload.extend_from_slice(&components.to_be_bytes());
        for component in 0..components {
            payload.push(u8::try_from(component).unwrap());
        }
        payload.extend_from_slice(&[u8::from(reversible), offsets, matrix]);
        segment(MCC, &payload)
    }

    fn segmented_mcc(marker: &[u8], split: usize) -> [Vec<u8>; 2] {
        let payload = &marker[4..];
        let body = &payload[7..];
        let mut first = payload[..3].to_vec();
        first.extend_from_slice(&1_u16.to_be_bytes());
        first.extend_from_slice(&payload[5..7]);
        first.extend_from_slice(&body[..split]);
        let mut second = 1_u16.to_be_bytes().to_vec();
        second.push(payload[2]);
        second.extend_from_slice(&body[split..]);
        [segment(MCC, &first), segment(MCC, &second)]
    }

    fn dependency_mcc(
        record: u8,
        components: u16,
        reversible: bool,
        coefficients: u8,
        offsets: u8,
    ) -> Vec<u8> {
        let mut marker = mcc(record, components, reversible, coefficients, offsets);
        marker[11] = 0;
        marker
    }

    fn wavelet_mcc(
        record: u8,
        components: u16,
        transform: u8,
        levels: u8,
        component_offset: i32,
    ) -> Vec<u8> {
        let mut payload = 0_u16.to_be_bytes().to_vec();
        payload.push(record);
        payload.extend_from_slice(&0_u16.to_be_bytes());
        payload.extend_from_slice(&1_u16.to_be_bytes());
        payload.push(3);
        payload.extend_from_slice(&components.to_be_bytes());
        for component in 0..components {
            payload.push(u8::try_from(component).unwrap());
        }
        payload.extend_from_slice(&components.to_be_bytes());
        for component in 0..components {
            payload.push(u8::try_from(component).unwrap());
        }
        payload.extend_from_slice(&[levels & 0x3f, 0, transform]);
        payload.extend_from_slice(&component_offset.to_be_bytes());
        segment(MCC, &payload)
    }

    fn mco(collections: &[u8]) -> Vec<u8> {
        let mut payload = vec![u8::try_from(collections.len()).unwrap()];
        payload.extend_from_slice(collections);
        segment(MCO, &payload)
    }

    fn ht_codestream(style: u8) -> Vec<u8> {
        ht_codestream_with_capability(style, 0x0008)
    }

    fn ht_codestream_with_capability(style: u8, capability: u16) -> Vec<u8> {
        let cap = segment(
            CAP,
            &[
                0x00,
                0x02,
                0x00,
                0x00, // Pcap^15
                capability.to_be_bytes()[0],
                capability.to_be_bytes()[1],
            ],
        );
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, style, 1]);
        let qcd = segment(QCD, &[0, 0x40]);
        let mut bytes = codestream_with_headers(1, &[cap, cod, qcd], &[], &[]);
        bytes[6..8].copy_from_slice(&HT_RSIZ_FLAG.to_be_bytes());
        bytes
    }

    #[test]
    fn detects_ht_and_mixed_profiles_without_entering_the_block_decoder() {
        let ht = parse(&ht_codestream(0x40), &ResourceLimits::default()).unwrap();
        assert_eq!(
            ht.profile.block_coding,
            CodestreamBlockCoding::HighThroughput
        );
        assert!(ht.profile.uses_high_throughput());
        let capabilities = ht.profile.extended_capabilities().unwrap();
        assert_eq!(capabilities.p_cap(), PART_15_CAPABILITY_FLAG);
        assert_eq!(capabilities.c_capabilities(), &[8]);
        assert_eq!(capabilities.capability_for_part(15), Some(8));
        assert!(ht.profile.part15_requires_all_ht_blocks());

        let mixed = parse(&ht_codestream(0xc0), &ResourceLimits::default()).unwrap();
        assert_eq!(mixed.profile.block_coding, CodestreamBlockCoding::Mixed);
    }

    #[test]
    fn rejects_malformed_or_inconsistent_ht_profile_signaling() {
        let invalid_style = ht_codestream(0x80);
        assert!(matches!(
            parse(&invalid_style, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidCodingStyle,
                ..
            })
        ));

        let mut missing_r_siz = ht_codestream(0x40);
        missing_r_siz[6..8].copy_from_slice(&0_u16.to_be_bytes());
        assert!(matches!(
            parse(&missing_r_siz, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidProfile,
                ..
            })
        ));

        let cap = segment(CAP, &[0, 2, 0, 0]);
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0x40, 1]);
        let qcd = segment(QCD, &[0, 0x40]);
        let mut malformed_cap = codestream_with_headers(1, &[cap, cod, qcd], &[], &[]);
        malformed_cap[6..8].copy_from_slice(&HT_RSIZ_FLAG.to_be_bytes());
        assert!(matches!(
            parse(&malformed_cap, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerLength,
                ..
            })
        ));
    }

    #[test]
    fn validates_part15_capability_combinations() {
        let declared_legacy = parse(
            &ht_codestream_with_capability(0x00, 0x8008),
            &ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(
            declared_legacy
                .profile
                .extended_capabilities()
                .unwrap()
                .part15_block_mode(),
            Some(Part15BlockMode::HtDeclared)
        );
        assert!(!declared_legacy.profile.uses_high_throughput());

        let multi = parse(
            &ht_codestream_with_capability(0x40, 0x2008),
            &ResourceLimits::default(),
        )
        .unwrap();
        let capabilities = multi.profile.extended_capabilities().unwrap();
        assert!(capabilities.part15_allows_multiple_ht_sets());
        assert_eq!(capabilities.part15_magnitude_bound(), Some(16));
        assert!(!capabilities.part15_allows_roi());
        assert!(!capabilities.part15_allows_heterogeneous_headers());

        for malformed in [
            ht_codestream_with_capability(0x40, 0x4008),
            ht_codestream_with_capability(0x40, 0x0048),
            ht_codestream_with_capability(0x00, 0x0008),
            ht_codestream_with_capability(0xc0, 0x8008),
        ] {
            assert!(matches!(
                parse(&malformed, &ResourceLimits::default()),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidProfile,
                    ..
                })
            ));
        }
    }

    #[test]
    fn retains_bounded_profile_numbers_and_rejects_zero_most_significant_words() {
        let prf = segment(PRF, &[0x12, 0x34, 0x56, 0x78]);
        let cpf = segment(CPF, &[0x00, 0x01, 0x00, 0x02]);
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0x40, 1]);
        let qcd = segment(QCD, &[0, 0x40]);
        let cap = segment(CAP, &[0, 2, 0, 0, 0, 8]);
        let mut bytes = codestream_with_headers(
            1,
            &[cap.clone(), prf, cpf, cod.clone(), qcd.clone()],
            &[],
            &[],
        );
        bytes[6..8].copy_from_slice(&HT_RSIZ_FLAG.to_be_bytes());
        let parsed = parse(&bytes, &ResourceLimits::default()).unwrap();
        assert_eq!(parsed.profile.profile().unwrap().words(), &[0x1234, 0x5678]);
        assert_eq!(
            parsed.profile.corresponding_profile().unwrap().words(),
            &[1, 2]
        );

        let bad_cpf = segment(CPF, &[0, 1, 0, 0]);
        let mut invalid = codestream_with_headers(1, &[cap, bad_cpf, cod, qcd], &[], &[]);
        invalid[6..8].copy_from_slice(&HT_RSIZ_FLAG.to_be_bytes());
        assert!(matches!(
            parse(&invalid, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidProfile,
                ..
            })
        ));
    }

    #[test]
    fn parses_and_retains_bounded_part2_matrix_and_offset_transform() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let bytes = codestream_with_headers(
            2,
            &[
                cod,
                qcd,
                mct_i16(1, 1, &[1, 2, 3, 4]),
                mct_i16(2, 2, &[5, 6]),
                mcc(7, 2, true, 1, 2),
                mco(&[7]),
            ],
            &[],
            &[],
        );
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        let transform = layout.info.multiple_component_transform.unwrap();
        let stage = &transform.stages()[0];
        let collection = &stage.collections()[0];
        assert_eq!(stage.index(), 7);
        assert!(collection.reversible());
        assert_eq!(collection.input_components(), &[0, 1]);
        assert_eq!(collection.output_components(), &[0, 1]);
        assert_eq!(collection.matrix().unwrap().index(), 1);
        assert_eq!(collection.matrix().unwrap().values(), &[1, 2, 3, 4]);
        assert_eq!(collection.offsets().unwrap().values(), &[5, 6]);
    }

    #[test]
    fn reassembles_segmented_mct_and_mcc_with_contiguous_indexes() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let matrix = mct_i16_segmented(1, 1, &[1, 2, 3, 4], 3);
        let record = segmented_mcc(&mcc(7, 2, true, 1, 0), 5);
        let bytes = codestream_with_headers(
            2,
            &[
                cod,
                qcd,
                matrix[0].clone(),
                record[0].clone(),
                matrix[1].clone(),
                record[1].clone(),
                mco(&[7]),
            ],
            &[],
            &[],
        );
        let transform = parse_layout(&bytes, &ResourceLimits::default())
            .unwrap()
            .info
            .multiple_component_transform
            .unwrap();
        assert_eq!(
            transform.stages()[0].collections()[0]
                .matrix()
                .unwrap()
                .values(),
            &[1, 2, 3, 4]
        );
    }

    #[test]
    fn rejects_duplicate_discontinuous_and_incomplete_mct_fragments() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let parts = mct_i16_segmented(1, 1, &[1], 1);
        let cases = [
            vec![cod.clone(), qcd.clone(), parts[0].clone(), parts[0].clone()],
            {
                let mut wrong = parts[1].clone();
                wrong[4..6].copy_from_slice(&2_u16.to_be_bytes());
                vec![cod.clone(), qcd.clone(), parts[0].clone(), wrong]
            },
            vec![cod, qcd, parts[0].clone()],
        ];
        for headers in cases {
            let bytes = codestream_with_headers(1, &headers, &[], &[]);
            assert!(matches!(
                parse_layout(&bytes, &ResourceLimits::default()),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidTransform | MalformedKind::MissingRequiredStructure,
                    ..
                })
            ));
        }
    }

    #[test]
    fn resolves_dependency_and_wavelet_collection_semantics() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let dependency = codestream_with_headers(
            2,
            &[
                cod.clone(),
                qcd.clone(),
                mct_i16(1, 0, &[1, 2]),
                dependency_mcc(3, 2, true, 1, 0),
                mco(&[3]),
            ],
            &[],
            &[],
        );
        let transform = parse_layout(&dependency, &ResourceLimits::default())
            .unwrap()
            .info
            .multiple_component_transform
            .unwrap();
        let collection = &transform.stages()[0].collections()[0];
        assert_eq!(collection.kind(), MctCollectionKind::Dependency);
        assert_eq!(collection.matrix().unwrap().values(), &[1, 2]);

        let wavelet = codestream_with_headers(
            3,
            &[cod, qcd, wavelet_mcc(4, 3, 1, 1, -1), mco(&[4])],
            &[],
            &[],
        );
        let transform = parse_layout(&wavelet, &ResourceLimits::default())
            .unwrap()
            .info
            .multiple_component_transform
            .unwrap();
        let collection = &transform.stages()[0].collections()[0];
        assert_eq!(collection.kind(), MctCollectionKind::Wavelet);
        let wavelet = collection.wavelet().unwrap();
        assert_eq!(wavelet.transform(), MctWaveletType::Reversible5x3);
        assert_eq!(wavelet.levels(), 1);
        assert_eq!(wavelet.component_offset(), -1);
    }

    #[test]
    fn rejects_reserved_mcc_transform_type_as_malformed_not_unsupported() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let mut reserved = mcc(3, 1, true, 0, 0);
        reserved[11] = 2;
        let bytes = codestream_with_headers(1, &[cod, qcd, reserved, mco(&[3])], &[], &[]);
        assert!(matches!(
            parse_layout(&bytes, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidTransform,
                ..
            })
        ));
    }

    #[test]
    fn rejects_unknown_or_reserved_markers_as_malformed() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let reserved = segment(0xff65, &[]);
        let main =
            codestream_with_headers(1, &[cod.clone(), qcd.clone(), reserved.clone()], &[], &[]);
        assert!(matches!(
            parse_layout(&main, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarker,
                ..
            })
        ));
        let tile = codestream_with_headers(1, &[cod, qcd], &[reserved], &[]);
        assert!(matches!(
            parse_layout(&tile, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarker,
                ..
            })
        ));
    }

    #[test]
    fn converts_f32_mct_values_to_deterministic_fixed_point() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let bytes = codestream_with_headers(
            1,
            &[
                cod,
                qcd,
                mct_f32(1, 1, &[1.5_f32.to_bits()]),
                mcc(3, 1, false, 1, 0),
                mco(&[3]),
            ],
            &[],
            &[],
        );
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        let transform = layout.info.multiple_component_transform.unwrap();
        let matrix = transform.stages()[0].collections()[0].matrix().unwrap();
        assert_eq!(matrix.fraction_bits(), MCT_FIXED_BITS);
        assert_eq!(matrix.values(), &[6_442_450_944]);
    }

    #[test]
    fn converts_finite_binary64_and_rejects_non_finite_or_overflow() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let valid = codestream_with_headers(
            1,
            &[
                cod.clone(),
                qcd.clone(),
                mct_f64(1, 1, &[1.25_f64.to_bits()]),
                mcc(3, 1, false, 1, 0),
                mco(&[3]),
            ],
            &[],
            &[],
        );
        let transform = parse_layout(&valid, &ResourceLimits::default())
            .unwrap()
            .info
            .multiple_component_transform
            .unwrap();
        let matrix = transform.stages()[0].collections()[0].matrix().unwrap();
        assert_eq!(matrix.numeric_format(), MctNumericFormat::Float64);
        assert_eq!(matrix.fraction_bits(), MCT_FIXED_BITS);
        assert_eq!(matrix.values(), &[5_368_709_120]);

        for bits in [
            f64::INFINITY.to_bits(),
            f64::NAN.to_bits(),
            f64::MAX.to_bits(),
        ] {
            let invalid = codestream_with_headers(
                1,
                &[
                    cod.clone(),
                    qcd.clone(),
                    mct_f64(1, 1, &[bits]),
                    mcc(3, 1, false, 1, 0),
                    mco(&[3]),
                ],
                &[],
                &[],
            );
            assert!(matches!(
                parse_layout(&invalid, &ResourceLimits::default()),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidTransform,
                    ..
                })
            ));
        }
    }

    #[test]
    fn resolves_multiple_records_collections_subsets_permutations_and_tile_override() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let bytes = codestream_with_headers(
            3,
            &[
                cod,
                qcd,
                mct_i16(1, 1, &[1, 0, 0, 1]),
                mct_i16(2, 1, &[1]),
                mcc_collections(7, &[(&[0, 2], &[2, 0], true, 1, 0)]),
                mcc_collections(8, &[(&[0], &[0], true, 2, 0), (&[1], &[1], true, 2, 0)]),
                mco(&[7, 8]),
            ],
            &[mct_i16(2, 1, &[2]), mco(&[8, 7])],
            &[],
        );
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        let main = layout.info.multiple_component_transform.unwrap();
        assert_eq!(main.stages().len(), 2);
        assert_eq!(main.stages()[0].index(), 7);
        assert_eq!(
            main.stages()[0].collections()[0].input_components(),
            &[0, 2]
        );
        assert_eq!(
            main.stages()[0].collections()[0].output_components(),
            &[2, 0]
        );
        assert_eq!(main.stages()[1].collections().len(), 2);
        let tile = layout.tile_parts[0]
            .multiple_component_transform
            .as_ref()
            .unwrap();
        assert_eq!(tile.stages()[0].index(), 8);
        assert_eq!(
            tile.stages()[0].collections()[0].matrix().unwrap().values(),
            &[2]
        );
    }

    #[test]
    fn accepts_combined_part1_and_ordered_part2_inverse_transforms() {
        let cod = segment(COD, &[0, 0, 0, 1, 1, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let bytes = codestream_with_headers(
            3,
            &[
                cod,
                qcd,
                mct_i16(1, 1, &[1, 0, 0, 0, 1, 0, 0, 0, 1]),
                mcc(7, 3, true, 1, 0),
                mco(&[7]),
            ],
            &[],
            &[],
        );
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        assert!(layout.info.coding_style.multiple_component_transform);
        assert_eq!(
            layout
                .info
                .multiple_component_transform
                .unwrap()
                .stages()
                .len(),
            1
        );
    }

    #[test]
    fn rejects_invalid_or_unsupported_part2_transform_graphs() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let malformed_matrix = codestream_with_headers(
            2,
            &[
                cod.clone(),
                qcd.clone(),
                mct_i16(1, 1, &[1, 0, 1]),
                mcc(7, 2, true, 1, 0),
                mco(&[7]),
            ],
            &[],
            &[],
        );
        assert!(matches!(
            parse_layout(&malformed_matrix, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidTransform,
                ..
            })
        ));

        let dependency_chain = codestream_with_headers(
            2,
            &[
                cod.clone(),
                qcd.clone(),
                mct_i16(1, 1, &[1, 0, 0, 1]),
                mcc(7, 2, true, 1, 0),
                mco(&[7, 7]),
            ],
            &[],
            &[],
        );
        assert!(matches!(
            parse_layout(&dependency_chain, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidTransform,
                ..
            })
        ));

        let nan = codestream_with_headers(
            1,
            &[
                cod,
                qcd,
                mct_f32(1, 1, &[f32::NAN.to_bits()]),
                mcc(3, 1, false, 1, 0),
                mco(&[3]),
            ],
            &[],
            &[],
        );
        assert!(matches!(
            parse_layout(&nan, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidTransform,
                ..
            })
        ));
    }

    fn codestream(tile_data: &[u8], zero_length_tile_part: bool) -> Vec<u8> {
        codestream_with_styles(
            tile_data,
            zero_length_tile_part,
            &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1],
            &[0, 0],
        )
    }

    fn codestream_with_styles(
        tile_data: &[u8],
        zero_length_tile_part: bool,
        cod_payload: &[u8],
        qcd_payload: &[u8],
    ) -> Vec<u8> {
        let mut bytes = SOC.to_be_bytes().to_vec();
        bytes.extend(siz());
        bytes.extend(segment(COD, cod_payload));
        bytes.extend(segment(QCD, qcd_payload));
        let tile_part_start = bytes.len();
        let mut sot_payload = Vec::new();
        sot_payload.extend_from_slice(&0_u16.to_be_bytes());
        sot_payload.extend_from_slice(&0_u32.to_be_bytes());
        sot_payload.extend_from_slice(&[0, 1]);
        bytes.extend(segment(SOT, &sot_payload));
        bytes.extend_from_slice(&SOD.to_be_bytes());
        bytes.extend_from_slice(tile_data);
        let tile_part_length = u32::try_from(bytes.len() - tile_part_start).unwrap();
        if !zero_length_tile_part {
            let length_offset = tile_part_start + 6;
            bytes[length_offset..length_offset + 4]
                .copy_from_slice(&tile_part_length.to_be_bytes());
        }
        bytes.extend_from_slice(&EOC.to_be_bytes());
        bytes
    }

    fn poc(records: &[[u8; 7]]) -> Vec<u8> {
        let mut payload = Vec::new();
        for record in records {
            payload.extend_from_slice(record);
        }
        segment(POC, &payload)
    }

    fn two_part_codestream(first_header: &[Vec<u8>], second_header: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes = SOC.to_be_bytes().to_vec();
        bytes.extend(siz());
        bytes.extend(segment(COD, &[0, 0, 0, 2, 0, 0, 2, 2, 0, 1]));
        bytes.extend(segment(QCD, &[0, 0]));
        for (index, header) in [first_header, second_header].iter().enumerate() {
            let start = bytes.len();
            let mut sot = Vec::new();
            sot.extend_from_slice(&0_u16.to_be_bytes());
            sot.extend_from_slice(&0_u32.to_be_bytes());
            sot.extend_from_slice(&[u8::try_from(index).unwrap(), 2]);
            bytes.extend(segment(SOT, &sot));
            for marker in *header {
                bytes.extend(marker);
            }
            bytes.extend_from_slice(&SOD.to_be_bytes());
            bytes.push(0);
            let length = u32::try_from(bytes.len() - start).unwrap();
            bytes[start + 6..start + 10].copy_from_slice(&length.to_be_bytes());
        }
        bytes.extend_from_slice(&EOC.to_be_bytes());
        bytes
    }

    #[test]
    fn parses_bounded_tile_part() {
        let bytes = codestream(&[1, 2, 3], false);
        let info = parse(&bytes, &ResourceLimits::default()).unwrap();
        assert_eq!(info.image_size.width, 64);
        assert_eq!(info.image_size.height, 32);
        assert_eq!(info.image_size.components, 1);
        assert_eq!(info.coding_style.progression_order, ProgressionOrder::Lrcp);
        assert_eq!(info.coding_style.transform, WaveletTransform::Reversible5x3);
        assert_eq!(info.coding_style.code_block_width, 16);
        assert_eq!(
            info.coding_style.tier1_coding_style().unwrap(),
            Tier1CodingStyle::default()
        );
        assert_eq!(info.quantization.style, QuantizationStyle::None);
        assert_eq!(info.quantization.step_count, 1);
        assert_eq!(
            info.quantization.step(0).unwrap(),
            QuantizationStep {
                exponent: 0,
                mantissa: 0,
            }
        );
        assert_eq!(info.tile_parts, 1);
        let geometry = crate::decode::derive_component_resolution_geometry(
            &info.image_size,
            info.coding_style,
            0,
            0,
        )
        .unwrap();
        assert_eq!(geometry.width, 64);
        assert_eq!(geometry.height, 32);
        assert_eq!(geometry.code_blocks, 8);
        assert_eq!(
            crate::decode::derive_packet_grid(
                &info.image_size,
                info.coding_style,
                &ResourceLimits::default(),
            )
            .unwrap()
            .packet_count(),
            1
        );
        assert_eq!(usize::try_from(info.consumed_bytes).unwrap(), bytes.len());
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        assert_eq!(layout.tile_parts.len(), 1);
        assert_eq!(layout.tile_parts[0].tile_index, 0);
        assert_eq!(layout.tile_parts[0].tile_part_index, 0);
        assert_eq!(layout.tile_parts[0].data, [1, 2, 3]);
    }

    #[test]
    fn parses_retains_and_persists_progression_changes() {
        let first = poc(&[[0, 0, 0, 1, 1, 1, 1], [0, 0, 0, 2, 1, 1, 0]]);
        let bytes = two_part_codestream(&[first], &[]);
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        assert_eq!(layout.info.progression_changes.len(), 0);
        assert_eq!(layout.tile_parts.len(), 2);
        assert_eq!(layout.tile_parts[0].progression_changes.len(), 2);
        assert!(
            layout.tile_parts[0]
                .progression_changes
                .equals(&layout.tile_parts[1].progression_changes)
        );
        let records = layout.tile_parts[1].progression_changes.records();
        assert_eq!(records[0].layer_end, 1);
        assert_eq!(records[0].progression_order, ProgressionOrder::Rlcp);
        assert_eq!(records[1].layer_end, 2);
        assert_eq!(records[1].progression_order, ProgressionOrder::Lrcp);
    }

    #[test]
    fn validates_progression_change_ranges_coverage_duplicates_and_scope() {
        let cod = segment(COD, &[0, 0, 0, 2, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let incomplete = codestream_with_headers(
            1,
            &[cod.clone(), qcd.clone(), poc(&[[0, 0, 0, 1, 1, 1, 0]])],
            &[],
            &[0],
        );
        assert!(matches!(
            parse_layout(&incomplete, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidProgressionOrderChange,
                ..
            })
        ));

        let duplicate_record = codestream_with_headers(
            1,
            &[
                cod.clone(),
                qcd.clone(),
                poc(&[[0, 0, 0, 2, 1, 1, 0], [0, 0, 0, 2, 1, 1, 1]]),
            ],
            &[],
            &[0],
        );
        assert!(matches!(
            parse_layout(&duplicate_record, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidProgressionOrderChange,
                ..
            })
        ));

        let duplicate_marker = codestream_with_headers(
            1,
            &[
                cod,
                qcd,
                poc(&[[0, 0, 0, 2, 1, 1, 0]]),
                poc(&[[0, 0, 0, 2, 1, 1, 0]]),
            ],
            &[],
            &[0],
        );
        assert!(matches!(
            parse_layout(&duplicate_marker, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));

        let second_part =
            two_part_codestream(&[], &[poc(&[[0, 0, 0, 1, 1, 1, 1], [0, 0, 0, 2, 1, 1, 0]])]);
        assert!(matches!(
            parse_layout(&second_part, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));
    }

    #[test]
    fn progression_change_component_indexes_follow_csiz_width() {
        let limits = ResourceLimits::default();
        let parser = Parser::new(&[], &limits).unwrap();
        let payload = [0, 1, 0, 0, 1, 1, 1, 1, 0];
        let changes = parser
            .parse_poc(
                Segment {
                    marker: POC,
                    offset: 0,
                    payload: &payload,
                },
                300,
            )
            .unwrap();
        assert_eq!(changes.records()[0].component_start, 256);
        assert_eq!(changes.records()[0].component_end, 257);

        let one_byte_endpoint = [0, 255, 0, 1, 1, 0, 0];
        let changes = parser
            .parse_poc(
                Segment {
                    marker: POC,
                    offset: 0,
                    payload: &one_byte_endpoint,
                },
                256,
            )
            .unwrap();
        assert_eq!(changes.records()[0].component_start, 255);
        assert_eq!(changes.records()[0].component_end, 256);
    }

    #[test]
    fn parses_retains_and_iterates_plm_and_plt_packet_lengths() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let tile_data = vec![0; 129];
        let limits = ResourceLimits::default();

        let plm_bytes = codestream_with_headers(
            1,
            &[cod.clone(), qcd.clone(), plm(0, &[&[1, 128]])],
            &[],
            &tile_data,
        );
        let plm_layout = parse_layout(&plm_bytes, &limits).unwrap();
        let plm_lengths = plm_layout.tile_parts[0].packet_lengths;
        assert!(plm_lengths.is_present());
        assert_eq!(plm_lengths.packet_count(), 2);
        assert_eq!(plm_lengths.encoded_bytes(), 3);
        let mut plm_iter = plm_lengths.iter();
        assert_eq!(plm_iter.next_length().unwrap(), Some(1));
        assert_eq!(plm_iter.next_length().unwrap(), Some(128));
        assert_eq!(plm_iter.next_length().unwrap(), None);

        let plt_bytes =
            codestream_with_headers(1, &[cod, qcd], &[plt(0, &[1]), plt(1, &[128])], &tile_data);
        let plt_layout = parse_layout(&plt_bytes, &limits).unwrap();
        let plt_lengths = plt_layout.tile_parts[0].packet_lengths;
        assert!(plt_lengths.is_present());
        assert_eq!(plt_lengths.packet_count(), 2);
        assert_eq!(plt_lengths.encoded_bytes(), 3);
        let mut plt_iter = plt_lengths.iter();
        assert_eq!(plt_iter.next_length().unwrap(), Some(1));
        assert_eq!(plt_iter.next_length().unwrap(), Some(128));
        assert_eq!(plt_iter.next_length().unwrap(), None);

        let empty_plm = codestream_with_headers(
            1,
            &[
                segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]),
                segment(QCD, &[0, 0]),
                plm(0, &[&[]]),
            ],
            &[],
            &[],
        );
        let empty_layout = parse_layout(&empty_plm, &limits).unwrap();
        assert!(empty_layout.tile_parts[0].packet_lengths.is_present());
        assert_eq!(empty_layout.tile_parts[0].packet_lengths.packet_count(), 0);

        let empty_plt = codestream_with_headers(
            1,
            &[
                segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]),
                segment(QCD, &[0, 0]),
            ],
            &[segment(PLT, &[0])],
            &[],
        );
        let empty_layout = parse_layout(&empty_plt, &limits).unwrap();
        assert!(empty_layout.tile_parts[0].packet_lengths.is_present());
        assert_eq!(empty_layout.tile_parts[0].packet_lengths.packet_count(), 0);
    }

    #[test]
    fn validates_packet_length_scope_indexes_coding_and_payload_sum() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let limits = ResourceLimits::default();

        let duplicate_plt = codestream_with_headers(
            1,
            &[cod.clone(), qcd.clone()],
            &[plt(0, &[1]), plt(0, &[1])],
            &[0, 0],
        );
        assert!(matches!(
            parse_layout(&duplicate_plt, &limits),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));
        let duplicate_plm = codestream_with_headers(
            1,
            &[cod.clone(), qcd.clone(), plm(0, &[&[1]]), plm(0, &[&[1]])],
            &[],
            &[0],
        );
        assert!(matches!(
            parse_layout(&duplicate_plm, &limits),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));

        let incomplete = codestream_with_headers(
            1,
            &[cod.clone(), qcd.clone()],
            &[segment(PLT, &[0, 0x81])],
            &[0],
        );
        assert!(matches!(
            parse_layout(&incomplete, &limits),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidPacketLength,
                ..
            })
        ));

        let overflow = codestream_with_headers(
            1,
            &[cod.clone(), qcd.clone()],
            &[segment(PLT, &[0, 0x81, 0x80, 0x80, 0x80, 0x80, 0])],
            &[0],
        );
        assert!(matches!(
            parse_layout(&overflow, &limits),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidPacketLength,
                ..
            })
        ));
        let truncated_plm_group = codestream_with_headers(
            1,
            &[cod.clone(), qcd.clone(), segment(PLM, &[0, 0, 0, 0, 2, 1])],
            &[],
            &[0],
        );
        assert!(matches!(
            parse_layout(&truncated_plm_group, &limits),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerLength,
                ..
            })
        ));

        let wrong_sum =
            codestream_with_headers(1, &[cod.clone(), qcd.clone()], &[plt(0, &[2])], &[0]);
        assert!(matches!(
            parse_layout(&wrong_sum, &limits),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidPacketLength,
                ..
            })
        ));

        let plt_in_main =
            codestream_with_headers(1, &[cod.clone(), qcd.clone(), plt(0, &[1])], &[], &[0]);
        assert!(matches!(
            parse_layout(&plt_in_main, &limits),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));
        let plm_in_tile =
            codestream_with_headers(1, &[cod.clone(), qcd.clone()], &[plm(0, &[&[1]])], &[0]);
        assert!(matches!(
            parse_layout(&plm_in_tile, &limits),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));
        let both_scopes =
            codestream_with_headers(1, &[cod, qcd, plm(0, &[&[1]])], &[plt(0, &[1])], &[0]);
        assert!(matches!(
            parse_layout(&both_scopes, &limits),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));
    }

    #[test]
    fn accounts_packet_length_bytes_packets_and_plm_tile_groups() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let bytes = codestream_with_headers(
            1,
            &[cod.clone(), qcd.clone(), plm(0, &[&[1, 1]])],
            &[],
            &[0, 0],
        );
        let byte_limits = ResourceLimits::default()
            .with_maximum_packet_header_bytes(1)
            .unwrap();
        assert!(matches!(
            parse_layout(&bytes, &byte_limits),
            Err(DecodeError::LimitExceeded {
                resource: Resource::PacketHeaderBytes,
                ..
            })
        ));
        let packet_limits = ResourceLimits::default().with_maximum_packets(1).unwrap();
        assert!(matches!(
            parse_layout(&bytes, &packet_limits),
            Err(DecodeError::LimitExceeded {
                resource: Resource::Packets,
                ..
            })
        ));

        let extra_group = codestream_with_headers(1, &[cod, qcd, plm(0, &[&[1], &[1]])], &[], &[0]);
        assert!(matches!(
            parse_layout(&extra_group, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidPacketLength,
                ..
            })
        ));
    }

    #[test]
    fn parses_last_zero_length_tile_part() {
        let bytes = codestream(&[0xff, 0x00, 3], true);
        let info = parse(&bytes, &ResourceLimits::default()).unwrap();
        assert_eq!(info.tile_parts, 1);
        let limits = ResourceLimits::default();
        assert_eq!(
            parse_layout(&bytes, &limits).unwrap().tile_parts[0].data,
            [0xff, 0x00, 3]
        );
    }

    #[test]
    fn retains_main_and_tile_maxshift_overrides_per_component() {
        let limits = ResourceLimits::default();
        let mut main = codestream(&[0], true);
        let sot = main
            .windows(2)
            .position(|bytes| bytes == SOT.to_be_bytes())
            .unwrap();
        main.splice(sot..sot, segment(RGN, &[0, 0, 5]));
        let layout = parse_layout(&main, &limits).unwrap();
        assert_eq!(layout.info.roi_shifts.shift(0).unwrap(), 5);
        assert_eq!(layout.tile_parts[0].roi_shifts.shift(0).unwrap(), 5);

        let mut tile = codestream(&[0], true);
        let sot = tile
            .windows(2)
            .position(|bytes| bytes == SOT.to_be_bytes())
            .unwrap();
        tile.splice(sot..sot, segment(RGN, &[0, 0, 3]));
        let sod = tile
            .windows(2)
            .position(|bytes| bytes == SOD.to_be_bytes())
            .unwrap();
        tile.splice(sod..sod, segment(RGN, &[0, 0, 6]));
        let layout = parse_layout(&tile, &limits).unwrap();
        assert_eq!(layout.info.roi_shifts.shift(0).unwrap(), 3);
        assert_eq!(layout.tile_parts[0].roi_shifts.shift(0).unwrap(), 6);
    }

    #[test]
    fn retains_main_and_tile_component_style_and_quantization_overrides() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let main_coc = segment(COC, &[1, 0, 1, 1, 1, 1, 0]);
        let main_qcc = segment(QCC, &[1, 0, 8, 16, 24, 32]);
        let tile_cod = segment(COD, &[0, 0, 0, 1, 0, 2, 0, 0, 0, 1]);
        let tile_coc = segment(COC, &[1, 0, 2, 0, 0, 2, 1]);
        let tile_qcc_zero = segment(QCC, &[0, 0, 40, 48, 56, 64, 72, 80, 88]);
        let tile_qcc = segment(QCC, &[1, 0, 40, 48, 56, 64, 72, 80, 88]);
        let bytes = codestream_with_headers(
            2,
            &[cod, main_coc, qcd, main_qcc],
            &[tile_cod, tile_coc, tile_qcc_zero, tile_qcc],
            &[0, 0, 0, 0],
        );
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();

        let main_zero = layout.info.component_coding_styles.style(0).unwrap();
        let main_one = layout.info.component_coding_styles.style(1).unwrap();
        assert_eq!(main_zero.decomposition_levels, 0);
        assert_eq!(main_one.decomposition_levels, 1);
        assert_eq!(main_one.code_block_width, 8);
        assert_eq!(main_one.code_block_style, 1);
        assert_eq!(main_one.transform, WaveletTransform::Irreversible9x7);
        assert_eq!(
            layout
                .info
                .component_quantizations
                .quantization(0)
                .unwrap()
                .step_count,
            1
        );
        let main_one_quantization = layout.info.component_quantizations.quantization(1).unwrap();
        assert_eq!(main_one_quantization.step_count, 4);
        assert_eq!(main_one_quantization.step(3).unwrap().exponent, 4);

        let tile = &layout.tile_parts[0];
        let tile_one = tile.component_coding_styles.style(1).unwrap();
        assert_eq!(
            tile.component_coding_styles
                .style(0)
                .unwrap()
                .decomposition_levels,
            2
        );
        assert_eq!(tile_one.decomposition_levels, 2);
        assert_eq!(tile_one.code_block_width, 4);
        assert_eq!(tile_one.code_block_style, 2);
        assert_eq!(tile_one.transform, WaveletTransform::Reversible5x3);
        assert_eq!(
            tile.component_quantizations
                .quantization(0)
                .unwrap()
                .step_count,
            7
        );
        let tile_one_quantization = tile.component_quantizations.quantization(1).unwrap();
        assert_eq!(tile_one_quantization.step_count, 7);
        assert_eq!(tile_one_quantization.step(6).unwrap().exponent, 11);
    }

    #[test]
    fn component_quantization_storage_scales_with_components_and_clones_independently() {
        assert!(core::mem::size_of::<ComponentQuantizations>() <= 64);
        let info = parse(&codestream(&[0], false), &ResourceLimits::default()).unwrap();
        let default = info.component_quantizations.quantization(0).unwrap();
        for count in [1_u16, 3, MAX_COMPONENTS as u16] {
            let mut table = ComponentQuantizations::uniform(count, default).unwrap();
            assert_eq!(table.quantizations.len(), usize::from(count));
            let snapshot = table.clone();
            let mut changed = default;
            changed.guard_bits += 1;
            table.set(count - 1, changed).unwrap();
            assert_eq!(
                snapshot.quantization(count - 1).unwrap().guard_bits,
                default.guard_bits
            );
            assert_eq!(
                table.quantization(count - 1).unwrap().guard_bits,
                changed.guard_bits
            );
            assert!(!table.equals(&snapshot));
            assert!(table.quantization(count).is_err());
            assert!(table.set(count, default).is_err());
        }
        assert!(ComponentQuantizations::uniform(0, default).is_err());
        assert!(ComponentQuantizations::uniform((MAX_COMPONENTS + 1) as u16, default).is_err());
    }

    #[test]
    fn tile_default_quantization_precedes_main_qcc_and_tile_qcc_precedes_default() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let main_qcd = segment(QCD, &[0x40, 0x40]);
        let main_qcc = segment(QCC, &[1, 0x40, 0x48]);
        let tile_qcd = segment(QCD, &[0x40, 0x50]);
        let tile_qcc = segment(QCC, &[2, 0x40, 0x58]);
        let bytes = codestream_with_headers(
            3,
            &[cod, main_qcd, main_qcc],
            &[tile_qcd, tile_qcc],
            &[0, 0, 0],
        );
        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        let tile = &layout.tile_parts[0];

        assert_eq!(
            layout
                .info
                .component_quantizations
                .quantization(1)
                .unwrap()
                .step(0)
                .unwrap()
                .exponent,
            9
        );
        assert_eq!(tile.quantization.step(0).unwrap().exponent, 10);
        assert_eq!(
            tile.component_quantizations
                .quantization(0)
                .unwrap()
                .step(0)
                .unwrap()
                .exponent,
            10
        );
        assert_eq!(
            tile.component_quantizations
                .quantization(1)
                .unwrap()
                .step(0)
                .unwrap()
                .exponent,
            10
        );
        assert_eq!(
            tile.component_quantizations
                .quantization(2)
                .unwrap()
                .step(0)
                .unwrap()
                .exponent,
            11
        );
    }

    #[test]
    fn validates_tile_default_quantization_duplicates_lengths_and_compatibility() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0x40, 0x40]);

        let duplicate = codestream_with_headers(
            1,
            &[cod.clone(), qcd.clone()],
            &[qcd.clone(), qcd.clone()],
            &[0],
        );
        assert!(matches!(
            parse_layout(&duplicate, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));

        let invalid_length = codestream_with_headers(
            1,
            &[cod.clone(), qcd.clone()],
            &[segment(QCD, &[0x40])],
            &[0],
        );
        assert!(matches!(
            parse_layout(&invalid_length, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerLength,
                ..
            })
        ));

        let incompatible = codestream_with_headers(
            2,
            &[cod, qcd],
            &[
                segment(QCD, &[0x40, 0x48]),
                segment(COC, &[1, 0, 1, 2, 2, 0, 1]),
            ],
            &[0],
        );
        assert!(matches!(
            parse_layout(&incompatible, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidQuantization,
                ..
            })
        ));
    }

    #[test]
    fn retains_tile_default_quantization_across_tile_parts() {
        let mut bytes = SOC.to_be_bytes().to_vec();
        bytes.extend(siz());
        bytes.extend(segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]));
        bytes.extend(segment(QCD, &[0x40, 0x40]));
        let mut part = 0_u8;
        while part < 2 {
            let start = bytes.len();
            let mut sot = Vec::new();
            sot.extend_from_slice(&0_u16.to_be_bytes());
            sot.extend_from_slice(&0_u32.to_be_bytes());
            sot.extend_from_slice(&[part, 2]);
            bytes.extend(segment(SOT, &sot));
            if part == 0 {
                bytes.extend(segment(QCD, &[0x40, 0x58]));
            }
            bytes.extend_from_slice(&SOD.to_be_bytes());
            let length = u32::try_from(bytes.len() - start).unwrap();
            bytes[start + 6..start + 10].copy_from_slice(&length.to_be_bytes());
            part += 1;
        }
        bytes.extend_from_slice(&EOC.to_be_bytes());

        let limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &limits).unwrap();
        assert_eq!(layout.tile_parts.len(), 2);
        assert_eq!(
            layout.tile_parts[0].quantization.step(0).unwrap().exponent,
            11
        );
        assert_eq!(
            layout.tile_parts[1].quantization.step(0).unwrap().exponent,
            11
        );
        assert_eq!(
            layout.tile_parts[1]
                .component_quantizations
                .quantization(0)
                .unwrap()
                .step(0)
                .unwrap()
                .exponent,
            11
        );
    }

    #[test]
    fn validates_component_override_indexes_duplicates_and_lengths() {
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let coc = segment(COC, &[1, 0, 1, 1, 1, 0, 1]);

        let out_of_range = codestream_with_headers(
            2,
            &[
                cod.clone(),
                segment(COC, &[2, 0, 0, 2, 2, 0, 1]),
                qcd.clone(),
            ],
            &[],
            &[0, 0],
        );
        assert!(matches!(
            parse_layout(&out_of_range, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidCodingStyle,
                ..
            })
        ));

        let out_of_range_qcc = codestream_with_headers(
            2,
            &[cod.clone(), qcd.clone(), segment(QCC, &[2, 0, 0])],
            &[],
            &[0, 0],
        );
        assert!(matches!(
            parse_layout(&out_of_range_qcc, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidQuantization,
                ..
            })
        ));

        let duplicate = codestream_with_headers(
            2,
            &[cod.clone(), coc.clone(), coc.clone(), qcd.clone()],
            &[],
            &[0, 0, 0],
        );
        assert!(matches!(
            parse_layout(&duplicate, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));

        let duplicate_qcc = codestream_with_headers(
            2,
            &[
                cod.clone(),
                qcd.clone(),
                segment(QCC, &[1, 0, 0]),
                segment(QCC, &[1, 0, 0]),
            ],
            &[],
            &[0, 0],
        );
        assert!(matches!(
            parse_layout(&duplicate_qcc, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));

        let duplicate_tile_coc = codestream_with_headers(
            2,
            &[cod.clone(), qcd.clone()],
            &[coc.clone(), coc.clone()],
            &[0, 0, 0],
        );
        assert!(matches!(
            parse_layout(&duplicate_tile_coc, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));

        let short_coc = codestream_with_headers(
            2,
            &[cod.clone(), segment(COC, &[1, 0, 0]), qcd.clone()],
            &[],
            &[0, 0],
        );
        assert!(matches!(
            parse_layout(&short_coc, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerLength,
                ..
            })
        ));

        let short_qcc = codestream_with_headers(
            2,
            &[cod, coc, qcd, segment(QCC, &[1, 0, 8])],
            &[],
            &[0, 0, 0],
        );
        assert!(matches!(
            parse_layout(&short_qcc, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerLength,
                ..
            })
        ));
    }

    #[test]
    fn validates_maxshift_component_style_shift_and_duplicates() {
        let limits = ResourceLimits::default();
        for payload in [[1, 0, 1], [0, 1, 1]] {
            let mut bytes = codestream(&[0], true);
            let sot = bytes
                .windows(2)
                .position(|value| value == SOT.to_be_bytes())
                .unwrap();
            bytes.splice(sot..sot, segment(RGN, &payload));
            assert!(matches!(
                parse_layout(&bytes, &limits),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidRegionOfInterest,
                    ..
                })
            ));
        }

        let mut maximum = codestream(&[0], true);
        let sot = maximum
            .windows(2)
            .position(|value| value == SOT.to_be_bytes())
            .unwrap();
        maximum.splice(sot..sot, segment(RGN, &[0, 0, MAX_ROI_SHIFT]));
        let parsed = parse_layout(&maximum, &limits).unwrap();
        assert_eq!(parsed.info.roi_shifts.shift(0).unwrap(), u8::MAX);

        let mut duplicate = codestream(&[0], true);
        let sot = duplicate
            .windows(2)
            .position(|value| value == SOT.to_be_bytes())
            .unwrap();
        let mut markers = segment(RGN, &[0, 0, 2]);
        markers.extend(segment(RGN, &[0, 0, 2]));
        duplicate.splice(sot..sot, markers);
        assert!(matches!(
            parse_layout(&duplicate, &limits),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));
    }

    #[test]
    fn parses_reassembles_and_validates_packed_packet_headers() {
        let limits = ResourceLimits::default();
        let cod = segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]);
        let qcd = segment(QCD, &[0, 0]);
        let ppm_bytes = codestream_with_headers(
            1,
            &[
                cod.clone(),
                qcd.clone(),
                ppm(0, &[0, 0]),
                ppm(1, &[0, 1, 0]),
            ],
            &[],
            &[],
        );
        let ppm_layout = parse_layout(&ppm_bytes, &limits).unwrap();
        let packed = ppm_layout.tile_parts[0].packed_packet_headers;
        assert!(packed.is_present());
        assert_eq!(packed.len(), 1);
        assert_eq!(packed.fragment_count(), 2);
        assert_eq!(packed.reassemble(&limits).unwrap(), [0]);

        let ppt_bytes = codestream_with_headers(
            1,
            &[cod.clone(), qcd.clone()],
            &[ppt(0, &[0xaa]), ppt(1, &[0xbb])],
            &[],
        );
        let ppt_layout = parse_layout(&ppt_bytes, &limits).unwrap();
        let packed = ppt_layout.tile_parts[0].packed_packet_headers;
        assert_eq!(packed.len(), 2);
        assert_eq!(packed.fragment_count(), 2);
        assert_eq!(packed.reassemble(&limits).unwrap(), [0xaa, 0xbb]);

        for invalid in [
            codestream_with_headers(
                1,
                &[cod.clone(), qcd.clone(), ppm(1, &[0, 0, 0, 1, 0])],
                &[],
                &[],
            ),
            codestream_with_headers(
                1,
                &[cod.clone(), qcd.clone(), ppm(0, &[0, 0, 0, 2, 0])],
                &[],
                &[],
            ),
            codestream_with_headers(
                1,
                &[cod.clone(), qcd.clone(), ppm(0, &[0, 0, 0, 1, 0])],
                &[ppt(0, &[0])],
                &[],
            ),
        ] {
            assert!(matches!(
                parse_layout(&invalid, &limits),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidMarkerOrder
                        | MalformedKind::InvalidPackedPacketHeader,
                    ..
                })
            ));
        }

        let ppm_in_tile =
            codestream_with_headers(1, &[cod.clone(), qcd.clone()], &[ppm(0, &[])], &[]);
        let ppt_in_main = codestream_with_headers(1, &[cod, qcd, ppt(0, &[])], &[], &[]);
        for invalid in [ppm_in_tile, ppt_in_main] {
            assert!(matches!(
                parse_layout(&invalid, &limits),
                Err(DecodeError::MalformedInput {
                    kind: MalformedKind::InvalidMarkerOrder,
                    ..
                })
            ));
        }

        let bounded = codestream_with_headers(
            1,
            &[
                segment(COD, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1]),
                segment(QCD, &[0, 0]),
            ],
            &[ppt(0, &[0, 0])],
            &[],
        );
        let tight = limits.with_maximum_packet_header_bytes(1).unwrap();
        assert_eq!(
            parse_layout(&bounded, &tight),
            Err(DecodeError::LimitExceeded {
                resource: Resource::PacketHeaderBytes,
                limit: 1,
                actual: 2,
            })
        );
    }

    #[test]
    fn rejects_missing_siz_and_trailing_bytes() {
        let mut missing = SOC.to_be_bytes().to_vec();
        missing.extend(segment(SOT, &[0, 0, 0, 0, 0, 14, 0, 1]));
        assert!(matches!(
            parse(&missing, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));

        let mut trailing = codestream(&[], false);
        trailing.push(0);
        assert!(matches!(
            parse(&trailing, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::TrailingData,
                ..
            })
        ));
    }

    #[test]
    fn rejects_impossible_geometry() {
        let mut bytes = codestream(&[], false);
        let xsiz_offset = 2 + 4 + 2;
        bytes[xsiz_offset..xsiz_offset + 4].copy_from_slice(&0_u32.to_be_bytes());
        assert!(matches!(
            parse(&bytes, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidImageGeometry,
                ..
            })
        ));
    }

    #[test]
    fn every_prefix_fails_without_panicking() {
        let bytes = codestream(&[1, 2, 3], false);
        for length in 0..bytes.len() {
            assert!(parse(&bytes[..length], &ResourceLimits::default()).is_err());
        }
    }

    #[test]
    fn enforces_work_limit_for_zero_length_tile_part() {
        let bytes = codestream(&[0; 64], true);
        let limits = ResourceLimits::default()
            .with_maximum_work_units(8)
            .unwrap();
        assert!(matches!(
            parse(&bytes, &limits),
            Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                ..
            })
        ));
    }

    #[test]
    fn accepts_all_part_one_progression_orders() {
        let expected = [
            ProgressionOrder::Lrcp,
            ProgressionOrder::Rlcp,
            ProgressionOrder::Rpcl,
            ProgressionOrder::Pcrl,
            ProgressionOrder::Cprl,
        ];
        for (value, expected_order) in expected.into_iter().enumerate() {
            let cod = [0, u8::try_from(value).unwrap(), 0, 1, 0, 0, 2, 2, 0, 1];
            let bytes = codestream_with_styles(&[], false, &cod, &[0, 0]);
            let info = parse(&bytes, &ResourceLimits::default()).unwrap();
            assert_eq!(info.coding_style.progression_order, expected_order);
        }
    }

    #[test]
    fn rejects_invalid_coding_and_quantization_styles() {
        let invalid_progression =
            codestream_with_styles(&[], false, &[0, 5, 0, 1, 0, 0, 2, 2, 0, 1], &[0, 0]);
        assert!(matches!(
            parse(&invalid_progression, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidCodingStyle,
                ..
            })
        ));

        let invalid_quantization =
            codestream_with_styles(&[], false, &[0, 0, 0, 1, 0, 0, 2, 2, 0, 1], &[3, 0]);
        assert!(matches!(
            parse(&invalid_quantization, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidQuantization,
                ..
            })
        ));
    }
}
