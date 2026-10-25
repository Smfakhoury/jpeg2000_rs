use crate::{
    DecodeError, MalformedKind, Resource, ResourceLimits, tier1::Tier1BlockCoding,
    tier1::Tier1CodingStyle, transform::QuantizationStep,
};

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

const MAX_QUANTIZATION_STEPS: usize = 97;
pub(crate) const MAX_COMPONENTS: usize = 64;

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

    pub closed spec fn selection_geometry_view(size: &ImageSize) -> crate::specs::Part2SelectionGeometry {
        (size.width, size.height, size.x_origin, size.y_origin, size.tile_width, size.tile_height,
            size.tile_x_origin, size.tile_y_origin, size.tiles_across, size.tiles_down)
    }

    pub(crate) closed spec fn parsed_image_size_view(size: &ImageSize) -> crate::specs::Part1ParsedImageSize {
        crate::specs::Part1ParsedImageSize {
            geometry: selection_geometry_view(size), components: size.components,
            uniform_bit_depth: size.uniform_bit_depth, uniform_signed: size.uniform_signed, infos: size.component_info@,
        }
    }

    pub(crate) proof fn selection_geometry_known_fields(size: &ImageSize)
        ensures selection_geometry_view(size)
            == (size.width, size.height, size.x_origin, size.y_origin, size.tile_width, size.tile_height,
                size.tile_x_origin, size.tile_y_origin, size.tiles_across, size.tiles_down),
            image_size_component_count(size) == size.components,
    {
        reveal(selection_geometry_view);
    }

    pub closed spec fn decomposition_selector_view(style: CodingStyle) -> (u8, Seq<DecompositionDirection>) {
        (style.decomposition_levels, style.decomposition_directions@)
    }

    pub closed spec fn precinct_selector_view(style: CodingStyle) -> (u8, Seq<(u8, u8)>) {
        (style.decomposition_levels, style.precinct_exponents@)
    }

    pub(crate) proof fn resolution_selectors_known_fields(style: CodingStyle)
        ensures
            decomposition_selector_view(style) == (style.decomposition_levels, style.decomposition_directions@),
            precinct_selector_view(style) == (style.decomposition_levels, style.precinct_exponents@),
    {
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_error(true),
))]
fn selection_geometry_arithmetic() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_error(false),
))]
fn selection_geometry_invalid() -> DecodeError {
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

#[cfg(verus_keep_ghost)]
verus! {
    pub open spec fn progression_change_contains(record: ProgressionChange, layer: u16, resolution: u8, component: u16) -> bool {
        layer < record.layer_end
            && resolution >= record.resolution_start && resolution < record.resolution_end
            && component >= record.component_start && component < record.component_end
    }

    pub closed spec fn progression_changes_view(changes: &ProgressionChanges) -> Seq<ProgressionChange> {
        changes.records@
    }
}

impl ProgressionChange {
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == progression_change_contains(self, layer, resolution, component),
    ))]
    pub const fn contains(self, layer: u16, resolution: u8, component: u16) -> bool {
        layer < self.layer_end
            && resolution >= self.resolution_start
            && resolution < self.resolution_end
            && component >= self.component_start
            && component < self.component_end
    }
}

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

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result@ == progression_changes_view(self),))]
    pub fn records(&self) -> &[ProgressionChange] {
        &self.records
    }

    #[cfg(test)]
    pub(crate) fn from_records(records: Vec<ProgressionChange>) -> Self {
        Self { records }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == (progression_changes_view(self).len() != 0),))]
    pub const fn is_present(&self) -> bool {
        !self.records.is_empty()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result as nat == progression_changes_view(self).len(),))]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == (progression_changes_view(self).len() == 0),))]
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

struct ComponentCodingStylePayload {
    component: u16,
    payload: [u8; 43],
    length: usize,
    decomposition_structure: Option<u8>,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub(crate) closed spec fn parsed_coding_style_view(style: CodingStyle) -> crate::specs::Part1ParsedCodingStyle {
        crate::specs::Part1ParsedCodingStyle {
            progression_order: style.progression_order, layers: style.layers,
            multiple_component_transform: style.multiple_component_transform,
            decomposition_levels: style.decomposition_levels, decomposition_structure: style.decomposition_structure,
            decomposition_directions: style.decomposition_directions@,
            code_block_width: style.code_block_width, code_block_height: style.code_block_height,
            code_block_style: style.code_block_style, transform: style.transform,
            precincts_defined: style.precincts_defined, sop_markers: style.sop_markers, eph_markers: style.eph_markers,
            precinct_exponents: style.precinct_exponents@,
        }
    }

    closed spec fn parsed_coding_style_outcome(result: Result<CodingStyle, DecodeError>)
        -> Result<crate::specs::Part1ParsedCodingStyle, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(style) => Ok(parsed_coding_style_view(style)) }
    }

    closed spec fn parsed_component_coding_style_outcome(result: Result<(u16, CodingStyle), DecodeError>)
        -> Result<(u16, crate::specs::Part1ParsedCodingStyle), DecodeError>
    {
        match result { Err(error) => Err(error), Ok((component, style)) => Ok((component, parsed_coding_style_view(style))) }
    }

    closed spec fn component_coding_style_payload_is_bounded(result: Result<ComponentCodingStylePayload, DecodeError>) -> bool {
        match result { Err(_) => true, Ok(prepared) => prepared.length <= 43 }
    }

    closed spec fn parsed_component_coding_style_payload_outcome(result: Result<ComponentCodingStylePayload, DecodeError>)
        -> Result<crate::specs::Part1CocPrepared, DecodeError>
    {
        match result {
            Err(error) => Err(error),
            Ok(prepared) => Ok(crate::specs::Part1CocPrepared {
                component: prepared.component,
                payload: prepared.payload@.subrange(0, prepared.length as int),
                decomposition_structure: prepared.decomposition_structure,
            }),
        }
    }

    pub closed spec fn component_coding_style_result(styles: &ComponentCodingStyles, component: u16) -> Result<CodingStyle, DecodeError> {
        if component >= styles.components {
            Err(DecodeError::MalformedInput { offset: 0, kind: MalformedKind::InvalidCodingStyle })
        } else if component as nat >= styles.styles@.len() {
            Err(DecodeError::MalformedInput { offset: 0, kind: MalformedKind::InvalidCodingStyle })
        } else { Ok(styles.styles@[component as int]) }
    }

    pub closed spec fn coding_style_tier1_result(style: CodingStyle) -> Result<Tier1CodingStyle, DecodeError> {
        crate::specs::part1_tier1_coding_style_byte(style.code_block_style)
    }

    pub closed spec fn coding_style_packet_markers(style: &CodingStyle) -> (bool, bool) {
        (style.sop_markers, style.eph_markers)
    }

    pub proof fn coding_style_known_packet_markers(style: &CodingStyle)
        ensures coding_style_packet_markers(style) == (style.sop_markers, style.eph_markers),
    {
    }
}

impl CodingStyle {
    pub const fn resolution_levels(self) -> u16 {
        (self.decomposition_levels as u16) + 1
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_subband_count(
            decomposition_selector_view(self).0, decomposition_selector_view(self).1,
        ),
    ))]
    pub fn subband_count(self) -> u16 {
        let mut count = 1_u16;
        let mut stage = 0_u8;
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part2_subband_count_run); }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    stage <= self.decomposition_levels,
                    crate::specs::part2_subband_count(decomposition_selector_view(self).0, decomposition_selector_view(self).1)
                        == crate::specs::part2_subband_count_run(
                            decomposition_selector_view(self).0, decomposition_selector_view(self).1, stage, count,
                        ),
                decreases self.decomposition_levels - stage,
            )
        )]
        while stage < self.decomposition_levels && stage < 32 {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part2_subband_count_run); }
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
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part2_subband_count_run); }
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_resolution_scale_step(
            decomposition_selector_view(self).0, decomposition_selector_view(self).1, stage, scales,
        ),
    ))]
    pub(crate) fn resolution_downsampling_step(
        self,
        stage: u8,
        scales: (u32, u32),
    ) -> Result<(u32, u32), DecodeError> {
        let mut horizontal = scales.0;
        let mut vertical = scales.1;
        match self.decomposition_direction(stage)? {
            DecompositionDirection::None => {}
            DecompositionDirection::Horizontal => {
                horizontal = horizontal
                    .checked_mul(2)
                    .ok_or_else(selection_geometry_arithmetic)?;
            }
            DecompositionDirection::Vertical => {
                vertical = vertical
                    .checked_mul(2)
                    .ok_or_else(selection_geometry_arithmetic)?;
            }
            DecompositionDirection::TwoDimensional => {
                horizontal = horizontal
                    .checked_mul(2)
                    .ok_or_else(selection_geometry_arithmetic)?;
                vertical = vertical
                    .checked_mul(2)
                    .ok_or_else(selection_geometry_arithmetic)?;
            }
        }
        Ok((horizontal, vertical))
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_resolution_downsampling(
            decomposition_selector_view(self).0, decomposition_selector_view(self).1, resolution,
        ),
    ))]
    pub fn resolution_downsampling(self, resolution: u8) -> Result<(u32, u32), DecodeError> {
        if resolution > self.decomposition_levels {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodingStyle,
            });
        }
        let discarded = self
            .decomposition_levels
            .checked_sub(resolution)
            .ok_or_else(selection_geometry_arithmetic)?;
        let mut horizontal = 1_u32;
        let mut vertical = 1_u32;
        let mut stage = 0_u8;
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost selector = decomposition_selector_view(self); }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_resolution_scales_empty(selector.0, selector.1);
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    stage <= discarded,
                    resolution <= self.decomposition_levels,
                    discarded == self.decomposition_levels as int - resolution as int,
                    selector == decomposition_selector_view(self),
                    crate::specs::part2_resolution_scale_prefix(selector.0, selector.1, stage as nat)
                        == Ok((horizontal, vertical)),
                decreases discarded - stage,
            )
        )]
        while stage < discarded {
            let next = self.resolution_downsampling_step(stage, (horizontal, vertical));
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_resolution_scales_step(
                    selector.0, selector.1, stage as nat,
                );
                if next.is_err() {
                    assert(crate::specs::part2_resolution_scale_prefix(
                        selector.0, selector.1, stage as nat + 1,
                    ) == next);
                    crate::proofs::production_impl::production_resolution_scales_error(
                        selector.0, selector.1, stage as nat + 1, discarded as nat,
                    );
                }
            }
            let next = next?;
            horizontal = next.0;
            vertical = next.1;
            stage = stage
                .checked_add(1)
                .ok_or_else(selection_geometry_arithmetic)?;
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_maximum_isotropic_reduction(
            decomposition_selector_view(self).0, decomposition_selector_view(self).1, 0,
        ),
    ))]
    pub fn maximum_isotropic_reduction(self) -> u8 {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(decomposition_selector_view); }
        let mut count = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant count <= self.decomposition_levels,
                    crate::specs::part2_maximum_isotropic_reduction(self.decomposition_levels, self.decomposition_directions@, 0)
                        == crate::specs::part2_maximum_isotropic_reduction(self.decomposition_levels, self.decomposition_directions@, count),
                decreases self.decomposition_levels - count,
            )
        )]
        while count < self.decomposition_levels && count < 32 {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part2_maximum_isotropic_reduction, 1); }
            let Some(direction) = self.decomposition_directions.get(count as usize) else {
                return count;
            };
            if !matches!(direction, DecompositionDirection::TwoDimensional) {
                return count;
            }
            count += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part2_maximum_isotropic_reduction, 1); }
        count
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == coding_style_tier1_result(self),
    ))]
    pub fn tier1_coding_style(self) -> Result<Tier1CodingStyle, DecodeError> {
        crate::tier1::tier1_coding_style_from_code_block_style(self.code_block_style)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_precinct_exponents(
            precinct_selector_view(self).0, precinct_selector_view(self).1, resolution,
        ),
    ))]
    pub fn precinct_exponents(self, resolution: u8) -> Result<(u8, u8), DecodeError> {
        if resolution > self.decomposition_levels {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodingStyle,
            });
        }
        match self.precinct_exponents.get(usize::from(resolution)) {
            Some(exponents) => Ok(*exponents),
            None => Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodingStyle,
            }),
        }
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == component_coding_style_result(self, component),
    ))]
    pub fn style(&self, component: u16) -> Result<CodingStyle, DecodeError> {
        if component >= self.components {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodingStyle,
            });
        }
        match self.styles.get(usize::from(component)) {
            Some(style) => Ok(*style),
            None => Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodingStyle,
            }),
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

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
/// Bounded per-component quantization snapshots, stored off-stack.
pub struct ComponentQuantizations {
    components: u16,
    quantizations: Vec<Quantization>,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn component_quantization_result(values: &ComponentQuantizations, component: u16)
        -> Result<Quantization, DecodeError>
    {
        if component >= values.components || component as nat >= values.quantizations@.len() {
            Err(DecodeError::MalformedInput { offset: 0, kind: MalformedKind::InvalidQuantization })
        } else { Ok(values.quantizations@[component as int]) }
    }
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_component_quantization(self, component),
    ))]
    pub fn quantization(&self, component: u16) -> Result<Quantization, DecodeError> {
        if component >= self.components {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidQuantization,
            });
        }
        match self.quantizations.get(usize::from(component)) {
            Some(quantization) => Ok(*quantization),
            None => Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidQuantization,
            }),
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

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn roi_shifts_view(value: &RoiShifts) -> Seq<u8> { value.shifts@ }
}

impl RoiShifts {
    pub const fn none() -> Self {
        Self {
            shifts: [0; MAX_COMPONENTS],
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_roi_shift(roi_shifts_view(self), component),
    ))]
    pub fn shift(&self, component: u16) -> Result<u8, DecodeError> {
        match self.shifts.get(usize::from(component)) {
            Some(shift) => Ok(*shift),
            None => Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidRegionOfInterest,
            }),
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

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn quantization_view(value: &Quantization) -> crate::specs::Part2Quantization {
        (value.style, value.guard_bits, value.step_count, value.steps@)
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == quantization_view(self).0,
    ))]
    pub const fn style(&self) -> QuantizationStyle {
        self.style
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == quantization_view(self).1,
    ))]
    pub const fn guard_bits(&self) -> u8 {
        self.guard_bits
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_quantization_lookup(quantization_view(self), index),
    ))]
    pub fn step(&self, index: u16) -> Result<QuantizationStep, DecodeError> {
        if index >= self.step_count {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidQuantization,
            });
        }
        let index = usize::from(index);
        if index >= self.steps.len() {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidQuantization,
            });
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
    #[cfg_attr(verus_keep_ghost, verus_spec(value => ensures value == owned_nonlinear_view(self).component,))]
    pub const fn component(&self) -> Option<u16> {
        self.component
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(value => ensures value == owned_nonlinear_view(self).precision,))]
    pub const fn precision(&self) -> u8 {
        self.precision
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(value => ensures value == owned_nonlinear_view(self).signed,))]
    pub const fn signed(&self) -> bool {
        self.signed
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(value => ensures value == owned_nonlinear_view(self).transform_type,))]
    pub const fn transform_type(&self) -> u8 {
        self.transform_type
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(value => ensures value@ == owned_nonlinear_view(self).parameters,))]
    pub fn parameters(&self) -> &[u8] {
        &self.parameters
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(value => ensures value == owned_nonlinear_view(self).d_min,))]
    pub const fn d_min(&self) -> Option<u32> {
        self.d_min
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(value => ensures value == owned_nonlinear_view(self).d_max,))]
    pub const fn d_max(&self) -> Option<u32> {
        self.d_max
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(value => ensures value == owned_nonlinear_view(self).point_bits,))]
    pub const fn point_bits(&self) -> Option<u8> {
        self.point_bits
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(value => ensures value@ == owned_nonlinear_view(self).points,))]
    pub fn points(&self) -> &[u32] {
        &self.points
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(value => ensures value == owned_nonlinear_view(self).gamma,))]
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

#[derive(Default)]
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

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn part2_execution_registry(extensions: &Part2Extensions) -> (Seq<u8>, Seq<u8>) {
        (Seq::new(extensions.decomposition_structures@.len(), |index: int| extensions.decomposition_structures@[index].index),
            Seq::new(extensions.arbitrary_decompositions@.len(), |index: int| extensions.arbitrary_decompositions@[index].index))
    }

    pub open spec fn component_depth_views(values: Seq<ComponentBitDepth>) -> Seq<(u8, bool)> {
        Seq::new(values.len(), |index: int| (values[index].precision, values[index].signed))
    }

    pub closed spec fn part2_component_depths(extensions: &Part2Extensions) -> Seq<(u8, bool)> {
        component_depth_views(extensions.component_bit_depths@)
    }

    pub closed spec fn part2_kernel_registry(extensions: &Part2Extensions) -> Seq<(u8, crate::specs::Part2RawKernel)> {
        Seq::new(extensions.arbitrary_kernels@.len(), |index: int|
            (extensions.arbitrary_kernels@[index].index, raw_kernel_view(extensions.arbitrary_kernels@[index])))
    }
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures component_depth_views(result@) == part2_component_depths(self),
    ))]
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures nonlinear_transform_option_view(result) == crate::specs::part2_nonlinear_transform_for(
            owned_extensions_view(self).nonlinear, component,
        ),
    ))]
    pub(crate) fn nonlinear_transform_for(
        &self,
        component: u16,
    ) -> Option<&NonlinearPointTransform> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(owned_extensions_view); reveal(owned_nonlinear_view); }
        let mut position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant position <= self.nonlinear_transforms.len(),
                crate::specs::part2_nonlinear_find(owned_extensions_view(self).nonlinear, Some(component), 0)
                    == crate::specs::part2_nonlinear_find(owned_extensions_view(self).nonlinear, Some(component), position as nat),
            decreases self.nonlinear_transforms.len() - position,
        ))]
        while position < self.nonlinear_transforms.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part2_nonlinear_find, 1); }
            let transform = &self.nonlinear_transforms[position];
            if transform.component == Some(component) {
                return Some(transform);
            }
            position += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part2_nonlinear_find, 1); }
        position = 0;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant position <= self.nonlinear_transforms.len(),
                crate::specs::part2_nonlinear_find(owned_extensions_view(self).nonlinear, Some(component), 0) == None,
                crate::specs::part2_nonlinear_find(owned_extensions_view(self).nonlinear, None, 0)
                    == crate::specs::part2_nonlinear_find(owned_extensions_view(self).nonlinear, None, position as nat),
            decreases self.nonlinear_transforms.len() - position,
        ))]
        while position < self.nonlinear_transforms.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part2_nonlinear_find, 1); }
            let transform = &self.nonlinear_transforms[position];
            if transform.component.is_none() {
                return Some(transform);
            }
            position += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part2_nonlinear_find, 1); }
        None
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result.is_some() == part2_execution_registry(self).0.contains(index),
    ))]
    pub(crate) fn decomposition_structure(&self, index: u8) -> Option<&DecompositionStructure> {
        let mut position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant position <= self.decomposition_structures.len(),
                forall|prior: int| 0 <= prior < position ==> self.decomposition_structures@[prior].index != index,
            decreases self.decomposition_structures.len() - position,
        ))]
        while position < self.decomposition_structures.len() {
            let structure = &self.decomposition_structures[position];
            if structure.index == index {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(part2_execution_registry(self).0[position as int] == index);
                    assert(part2_execution_registry(self).0.contains(index));
                }
                return Some(structure);
            }
            position += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            if part2_execution_registry(self).0.contains(index) {
                let prior = choose|prior: int| 0 <= prior < part2_execution_registry(self).0.len()
                    && part2_execution_registry(self).0[prior] == index;
                assert(self.decomposition_structures@[prior].index != index);
                assert(false);
            }
        }
        None
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures crate::transform::arbitrary_kernel_option(result)
            == crate::specs::part2_kernel_registry_lookup(part2_kernel_registry(self), index),
    ))]
    pub(crate) fn arbitrary_kernel(&self, index: u8) -> Option<&ArbitraryKernel> {
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_kernel_registry_empty(part2_kernel_registry(self), index); }
        let mut position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant position <= self.arbitrary_kernels.len(),
            crate::specs::part2_kernel_registry_prefix(part2_kernel_registry(self), index, position as nat) == None,
            decreases self.arbitrary_kernels.len() - position,
        ))]
        while position < self.arbitrary_kernels.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_kernel_registry_step(part2_kernel_registry(self), index, position as nat);
                crate::proofs::production_impl::production_kernel_registry_found(part2_kernel_registry(self), index, position as nat + 1, self.arbitrary_kernels@.len());
            }
            let kernel = &self.arbitrary_kernels[position];
            if kernel.index == index {
                return Some(kernel);
            }
            position += 1;
        }
        None
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result.is_some() == part2_execution_registry(self).1.contains(index),
    ))]
    pub(crate) fn arbitrary_decomposition(&self, index: u8) -> Option<&ArbitraryDecomposition> {
        let mut position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant position <= self.arbitrary_decompositions.len(),
                forall|prior: int| 0 <= prior < position ==> self.arbitrary_decompositions@[prior].index != index,
            decreases self.arbitrary_decompositions.len() - position,
        ))]
        while position < self.arbitrary_decompositions.len() {
            let decomposition = &self.arbitrary_decompositions[position];
            if decomposition.index == index {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(part2_execution_registry(self).1[position as int] == index);
                    assert(part2_execution_registry(self).1.contains(index));
                }
                return Some(decomposition);
            }
            position += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            if part2_execution_registry(self).1.contains(index) {
                let prior = choose|prior: int| 0 <= prior < part2_execution_registry(self).1.len()
                    && part2_execution_registry(self).1[prior] == index;
                assert(self.arbitrary_decompositions@[prior].index != index);
                assert(false);
            }
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn with_nonlinear_transform_for_test(transform: NonlinearPointTransform) -> Self {
        Self {
            nonlinear_transforms: vec![transform],
            ..Self::default()
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_component_depths_equal(part2_component_depths(self), part2_component_depths(other)),
    ))]
    pub(crate) fn component_bit_depths_equal(&self, other: &Self) -> bool {
        if self.component_bit_depths.len() != other.component_bit_depths.len() {
            return false;
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.component_bit_depths.len(),
                    self.component_bit_depths.len() == other.component_bit_depths.len(),
                    forall|position: int| 0 <= position < index ==>
                        part2_component_depths(self)[position] == part2_component_depths(other)[position],
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
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(part2_component_depths(self)[index as int] == (left.precision, left.signed));
                assert(part2_component_depths(other)[index as int] == (right.precision, right.signed));
            }
            if left.precision != right.precision || left.signed != right.signed {
                #[cfg(verus_keep_ghost)]
                proof! { assert(part2_component_depths(self) != part2_component_depths(other)); }
                return false;
            }
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { assert(part2_component_depths(self) =~= part2_component_depths(other)); }
        true
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == Ok(()),))]
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

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn extended_capabilities_view(value: &ExtendedCapabilities) -> (u32, Seq<u16>) {
        (value.p_cap, value.c_capabilities@)
    }

    pub closed spec fn codestream_profile_capabilities(value: &CodestreamProfile) -> Option<(u32, Seq<u16>)> {
        match value.extended_capabilities { None => None, Some(capabilities) => Some(extended_capabilities_view(&capabilities)) }
    }
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_capability_value(extended_capabilities_view(self), 15),
    ))]
    pub fn part15_value(&self) -> Option<u16> {
        self.capability_for_part(15)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_capability_block_mode(extended_capabilities_view(self)),
    ))]
    pub fn part15_block_mode(&self) -> Option<Part15BlockMode> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part2_capability_block_mode); }
        match self.part15_value()? >> 14 {
            0 => Some(Part15BlockMode::HtOnly),
            2 => Some(Part15BlockMode::HtDeclared),
            3 => Some(Part15BlockMode::Mixed),
            _ => None,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_capability_multiple_sets(extended_capabilities_view(self)),
    ))]
    pub fn part15_allows_multiple_ht_sets(&self) -> bool {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part2_capability_multiple_sets); }
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_capability_magnitude_bound(extended_capabilities_view(self)),
    ))]
    pub fn part15_magnitude_bound(&self) -> Option<u8> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part2_capability_magnitude_bound); }
        let value = self.part15_value()?;
        #[cfg(verus_keep_ghost)]
        proof! { assert(value & 0x1fu16 <= 31u16) by(bit_vector); }
        let parameter = (value & 0x1f) as u8;
        Some(crate::refinement::part15_magnitude_bound(parameter))
    }

    #[allow(clippy::manual_map, clippy::question_mark)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_capability_value(extended_capabilities_view(self), part),
    ))]
    pub fn capability_for_part(&self, part: u8) -> Option<u16> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(extended_capabilities_view); reveal(crate::specs::part2_capability_value); }
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
                    part > 0, part <= 32,
                    crate::specs::part2_capability_value(extended_capabilities_view(self), part)
                        == crate::specs::part2_capability_lookup_run(
                            self.p_cap, self.c_capabilities@, part, current, mask, index,
                        ),
                decreases part - current,
            )
        )]
        while current < part {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part2_capability_lookup_run, 1); }
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
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal_with_fuel(crate::specs::part2_capability_lookup_run, 1);
            assert(current == part);
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_profile_multiple_sets(codestream_profile_capabilities(self)),
    ))]
    pub fn part15_allows_multiple_ht_sets(&self) -> bool {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(codestream_profile_capabilities); reveal(crate::specs::part2_profile_multiple_sets); }
        match self.extended_capabilities.as_ref() {
            Some(capabilities) => capabilities.part15_allows_multiple_ht_sets(),
            None => false,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_profile_magnitude_bound(codestream_profile_capabilities(self)),
    ))]
    pub fn part15_magnitude_bound(&self) -> u8 {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(codestream_profile_capabilities); reveal(crate::specs::part2_profile_magnitude_bound); }
        match self.extended_capabilities.as_ref() {
            None => 74,
            Some(capabilities) => capabilities.part15_magnitude_bound().unwrap_or(74),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_profile_force_ht(codestream_profile_capabilities(self)),
    ))]
    pub fn part15_requires_all_ht_blocks(&self) -> bool {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(codestream_profile_capabilities); reveal(crate::specs::part2_profile_force_ht); }
        match self.extended_capabilities.as_ref() {
            None => false,
            Some(capabilities) => matches!(
                capabilities.part15_block_mode(),
                Some(Part15BlockMode::HtOnly)
            ),
        }
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

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct MctArray {
    index: u8,
    kind: MctArrayKind,
    numeric_format: MctNumericFormat,
    fraction_bits: u8,
    values: Vec<i64>,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct MultipleComponentTransform {
    stages: Vec<MctStage>,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct MctStage {
    index: u8,
    collections: Vec<MctCollection>,
}

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

#[cfg(verus_keep_ghost)]
verus! {
    pub(crate) proof fn mct_fixed_precision_is_q32()
        ensures MCT_FIXED_BITS == 32,
    {
    }

    pub closed spec fn mct_array_view(array: &MctArray) -> crate::specs::Part2MctArray {
        (array.index, array.kind, array.numeric_format, array.fraction_bits, array.values@)
    }

    pub open spec fn mct_array_option(value: Option<&MctArray>) -> Option<crate::specs::Part2MctArray> {
        match value { None => None, Some(array) => Some(mct_array_view(array)) }
    }

    pub closed spec fn mct_wavelet_view(wavelet: MctWavelet) -> crate::specs::Part2MctWavelet {
        (wavelet.transform, wavelet.levels, wavelet.component_offset)
    }

    pub open spec fn mct_wavelet_option(value: Option<MctWavelet>) -> Option<crate::specs::Part2MctWavelet> {
        match value { None => None, Some(wavelet) => Some(mct_wavelet_view(wavelet)) }
    }

    pub closed spec fn mct_collection_view(collection: &MctCollection) -> crate::specs::Part2MctCollection {
        (collection.reversible, collection.input_components@, collection.output_components@, collection.kind,
            match collection.matrix { None => None, Some(array) => Some(mct_array_view(&array)) },
            match collection.offsets { None => None, Some(array) => Some(mct_array_view(&array)) },
            mct_wavelet_option(collection.wavelet))
    }

    pub closed spec fn mct_stage_view(stage: &MctStage) -> crate::specs::Part2MctStage {
        (stage.index, Seq::new(stage.collections@.len(), |index: int| mct_collection_view(&stage.collections@[index])))
    }

    pub closed spec fn multiple_component_transform_view(transform: &MultipleComponentTransform) -> Seq<crate::specs::Part2MctStage> {
        Seq::new(transform.stages@.len(), |index: int| mct_stage_view(&transform.stages@[index]))
    }
}

impl MctArray {
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == mct_array_view(self).0,))]
    pub const fn index(&self) -> u8 {
        self.index
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == mct_array_view(self).1,))]
    pub const fn kind(&self) -> MctArrayKind {
        self.kind
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == mct_array_view(self).2,))]
    pub const fn numeric_format(&self) -> MctNumericFormat {
        self.numeric_format
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == mct_array_view(self).3,))]
    pub const fn fraction_bits(&self) -> u8 {
        self.fraction_bits
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result@ == mct_array_view(self).4,))]
    pub fn values(&self) -> &[i64] {
        &self.values
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_mct_array_identity(mct_array_view(self), mct_array_view(other)),
    ))]
    pub fn equals(&self, other: &Self) -> bool {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(mct_array_view); }
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
                    self.index == other.index, self.kind == other.kind, self.numeric_format == other.numeric_format,
                    self.fraction_bits == other.fraction_bits, self.values.len() == other.values.len(),
                    forall |i: int| 0 <= i < index ==> self.values@[i] == other.values@[i],
                decreases self.values.len() - index,
            )
        )]
        while index < self.values.len() {
            if self.values[index] != other.values[index] {
                return false;
            }
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { assert(self.values@ =~= other.values@); }
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
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures Seq::new(result@.len(), |index: int| mct_stage_view(&result@[index]))
            == multiple_component_transform_view(self),
    ))]
    pub fn stages(&self) -> &[MctStage] {
        &self.stages
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_mct_identity(multiple_component_transform_view(self), multiple_component_transform_view(other)),
    ))]
    pub fn equals(&self, other: &Self) -> bool {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(multiple_component_transform_view); }
        if self.stages.len() != other.stages.len() {
            return false;
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.stages.len(),
                    self.stages.len() == other.stages.len(),
                    forall |i: int| 0 <= i < index ==> mct_stage_view(&self.stages@[i]) == mct_stage_view(&other.stages@[i]),
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
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(multiple_component_transform_view(self)[index as int]
                        != multiple_component_transform_view(other)[index as int]);
                }
                return false;
            }
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { assert(multiple_component_transform_view(self) =~= multiple_component_transform_view(other)); }
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
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == mct_stage_view(self).0,))]
    pub const fn index(&self) -> u8 {
        self.index
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures Seq::new(result@.len(), |index: int| mct_collection_view(&result@[index])) == mct_stage_view(self).1,
    ))]
    pub fn collections(&self) -> &[MctCollection] {
        &self.collections
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_mct_stage_identity(mct_stage_view(self), mct_stage_view(other)),
    ))]
    pub fn equals(&self, other: &Self) -> bool {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(mct_stage_view);
            assert(mct_stage_view(self).1.len() == self.collections@.len());
            assert(mct_stage_view(other).1.len() == other.collections@.len());
        }
        if self.index != other.index || self.collections.len() != other.collections.len() {
            return false;
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.collections.len(),
                    self.index == other.index, self.collections.len() == other.collections.len(),
                    forall |i: int| 0 <= i < index ==> mct_collection_view(&self.collections@[i]) == mct_collection_view(&other.collections@[i]),
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
                #[cfg(verus_keep_ghost)]
                proof! { assert(mct_stage_view(self).1[index as int] != mct_stage_view(other).1[index as int]); }
                return false;
            }
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { assert(mct_stage_view(self).1 =~= mct_stage_view(other).1); }
        true
    }

    #[cfg(test)]
    pub(crate) fn from_collections(index: u8, collections: Vec<MctCollection>) -> Self {
        Self { index, collections }
    }
}

impl MctCollection {
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == mct_collection_view(self).0,))]
    pub const fn reversible(&self) -> bool {
        self.reversible
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result@ == mct_collection_view(self).1,))]
    pub fn input_components(&self) -> &[u16] {
        &self.input_components
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result@ == mct_collection_view(self).2,))]
    pub fn output_components(&self) -> &[u16] {
        &self.output_components
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == mct_collection_view(self).3,))]
    pub const fn kind(&self) -> MctCollectionKind {
        self.kind
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures mct_array_option(result) == mct_collection_view(self).4,))]
    pub const fn matrix(&self) -> Option<&MctArray> {
        self.matrix.as_ref()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures mct_array_option(result) == mct_collection_view(self).5,))]
    pub const fn offsets(&self) -> Option<&MctArray> {
        self.offsets.as_ref()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures mct_wavelet_option(result) == mct_collection_view(self).6,))]
    pub const fn wavelet(&self) -> Option<MctWavelet> {
        self.wavelet
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_mct_collection_identity(mct_collection_view(self), mct_collection_view(other)),
    ))]
    pub fn equals(&self, other: &Self) -> bool {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(mct_collection_view); }
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
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == mct_wavelet_view(self).0,))]
    pub const fn transform(self) -> MctWaveletType {
        self.transform
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == mct_wavelet_view(self).1,))]
    pub const fn levels(self) -> u8 {
        self.levels
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == mct_wavelet_view(self).2,))]
    pub const fn component_offset(self) -> i32 {
        self.component_offset
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == (left == right),))]
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == (mct_wavelet_option(left) == mct_wavelet_option(right)),
))]
const fn mct_wavelets_equal(left: Option<MctWavelet>, right: Option<MctWavelet>) -> bool {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(mct_wavelet_view); }
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

#[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == (left == right),))]
const fn mct_wavelet_types_equal(left: MctWaveletType, right: MctWaveletType) -> bool {
    matches!(
        (left, right),
        (
            MctWaveletType::Irreversible9x7,
            MctWaveletType::Irreversible9x7
        ) | (MctWaveletType::Reversible5x3, MctWaveletType::Reversible5x3)
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == (left == right),))]
const fn mct_array_kinds_equal(left: MctArrayKind, right: MctArrayKind) -> bool {
    matches!(
        (left, right),
        (MctArrayKind::Dependency, MctArrayKind::Dependency)
            | (MctArrayKind::Decorrelation, MctArrayKind::Decorrelation)
            | (MctArrayKind::Offset, MctArrayKind::Offset)
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == (left == right),))]
const fn mct_numeric_formats_equal(left: MctNumericFormat, right: MctNumericFormat) -> bool {
    matches!(
        (left, right),
        (MctNumericFormat::Int16, MctNumericFormat::Int16)
            | (MctNumericFormat::Int32, MctNumericFormat::Int32)
            | (MctNumericFormat::Float32, MctNumericFormat::Float32)
            | (MctNumericFormat::Float64, MctNumericFormat::Float64)
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == (left@ == right@),))]
fn u16_lists_equal(left: &[u16], right: &[u16]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= left.len(),
                left.len() == right.len(),
                forall |i: int| 0 <= i < index ==> left@[i] == right@[i],
            decreases left.len() - index,
        )
    )]
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! { assert(left@ =~= right@); }
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

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn packed_packet_headers_view(headers: &PackedPacketHeaders) -> crate::specs::Part1PackedHeaders {
        crate::specs::Part1PackedHeaders {
            source: match headers.source {
                PackedPacketHeaderSource::None => crate::specs::Part1PackedHeaderSource::None,
                PackedPacketHeaderSource::MarkerSegments { header, marker, logical_start } =>
                    crate::specs::Part1PackedHeaderSource::MarkerSegments { header: header@, marker, logical_start },
            },
            length: headers.length, fragments: headers.fragments,
        }
    }

    pub open spec fn packed_header_reassembly_outcome(result: Result<Vec<u8>, DecodeError>) -> Result<Seq<u8>, DecodeError> {
        match result { Ok(bytes) => Ok(bytes@), Err(error) => Err(error) }
    }

    pub closed spec fn packed_packet_headers_present(headers: &PackedPacketHeaders) -> bool {
        !matches!(headers.source, PackedPacketHeaderSource::None)
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == (DecodeError::MalformedInput { offset, kind }),
))]
fn packet_input_error(offset: u64, kind: MalformedKind) -> DecodeError {
    DecodeError::MalformedInput { offset, kind }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_packet_input_limit(resource, limit, actual),
))]
fn packet_input_limit(resource: Resource, limit: u64, actual: u64) -> DecodeError {
    DecodeError::LimitExceeded {
        resource,
        limit,
        actual,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures packed_packet_headers_view(&result) == (crate::specs::Part1PackedHeaders {
        source: crate::specs::Part1PackedHeaderSource::None, length: 0, fragments: 0 }),
        !packed_packet_headers_present(&result),
))]
pub(crate) const fn packed_packet_headers_none<'a>() -> PackedPacketHeaders<'a> {
    PackedPacketHeaders {
        source: PackedPacketHeaderSource::None,
        length: 0,
        fragments: 0,
    }
}

impl PackedPacketHeaders<'_> {
    pub const fn none() -> Self {
        packed_packet_headers_none()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == packed_packet_headers_present(self),
            result == !matches!(packed_packet_headers_view(self).source, crate::specs::Part1PackedHeaderSource::None),
    ))]
    pub const fn is_present(&self) -> bool {
        !matches!(self.source, PackedPacketHeaderSource::None)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == packed_packet_headers_view(self).length,))]
    pub const fn len(&self) -> usize {
        self.length
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == (packed_packet_headers_view(self).length == 0),))]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == packed_packet_headers_view(self).fragments,))]
    pub const fn fragment_count(&self) -> u16 {
        self.fragments
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures packed_header_reassembly_outcome(result) == crate::specs::part1_packed_header_reassembly(
            packed_packet_headers_view(self), crate::limits::resource_packet_header_byte_limit(limits)),
    ))]
    pub(crate) fn reassemble(&self, limits: &ResourceLimits) -> Result<Vec<u8>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_packed_header_reassembly); }
        if self.length as u64 > limits.maximum_packet_header_bytes() {
            return Err(packet_input_limit(
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
        let logical_end = match logical_start.checked_add(self.length) {
            Some(end) => end,
            None => return Err(packet_input_error(0, MalformedKind::ArithmeticOverflow)),
        };
        let mut output = Vec::with_capacity(self.length);
        let mut position = 0_usize;
        let mut logical_position = 0_usize;
        let mut remaining_segments = header.len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= header.len(), logical_position <= position,
                    remaining_segments <= header.len(),
                    logical_end as int == logical_start as int + self.length as int,
                    output@.len() == (if logical_position <= logical_start { 0int }
                        else if logical_position < logical_end { logical_position as int - logical_start as int }
                        else { self.length as int }),
                    crate::specs::part1_packed_header_reassembly(packed_packet_headers_view(self), crate::limits::resource_packet_header_byte_limit(limits))
                        == crate::specs::part1_packed_header_scan(header@, marker, logical_start, logical_end, self.length,
                            crate::specs::Part1PackedHeaderScan { position, logical_position, remaining_segments, output: output@ }),
                decreases remaining_segments,
            )
        )]
        while position < header.len() {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = crate::specs::Part1PackedHeaderScan {
            position, logical_position, remaining_segments, output: output@ }; }
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part1_packed_header_scan); }
            if remaining_segments == 0 {
                return Err(packet_input_error(
                    position as u64,
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            remaining_segments -= 1;
            if header.len().saturating_sub(position) < 4 {
                return Err(packet_input_error(
                    position as u64,
                    MalformedKind::InvalidPackedPacketHeader,
                ));
            }
            let current_marker =
                crate::refinement::marker_value(header[position], header[position + 1]);
            let segment_length = usize::from(crate::refinement::marker_value(
                header[position + 2],
                header[position + 3],
            ));
            if segment_length < 2 {
                return Err(packet_input_error(
                    position as u64,
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            let after_marker = match position.checked_add(2) {
                Some(value) => value,
                None => {
                    return Err(packet_input_error(
                        position as u64,
                        MalformedKind::ArithmeticOverflow,
                    ));
                }
            };
            let end = match after_marker.checked_add(segment_length) {
                Some(end) => end,
                None => {
                    return Err(packet_input_error(
                        position as u64,
                        MalformedKind::ArithmeticOverflow,
                    ));
                }
            };
            if end > header.len() {
                return Err(packet_input_error(
                    position as u64,
                    MalformedKind::InvalidMarkerLength,
                ));
            }
            if current_marker == marker {
                let payload_start = match position.checked_add(5) {
                    Some(start) => start,
                    None => {
                        return Err(packet_input_error(
                            position as u64,
                            MalformedKind::ArithmeticOverflow,
                        ));
                    }
                };
                if payload_start > end {
                    return Err(packet_input_error(
                        position as u64,
                        MalformedKind::InvalidMarkerLength,
                    ));
                }
                let fragment = &header[payload_start..end];
                let fragment_end = match logical_position.checked_add(fragment.len()) {
                    Some(end) => end,
                    None => {
                        return Err(packet_input_error(
                            position as u64,
                            MalformedKind::ArithmeticOverflow,
                        ));
                    }
                };
                let copy_start = logical_start.max(logical_position);
                let copy_end = logical_end.min(fragment_end);
                if copy_start < copy_end {
                    let local_start = copy_start - logical_position;
                    let local_end = copy_end - logical_position;
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(fragment@.subrange(local_start as int, local_end as int)
                            =~= header@.subrange(payload_start as int + local_start as int, payload_start as int + local_end as int));
                    }
                    output.extend_from_slice(&fragment[local_start..local_end]);
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(output@ =~= before.output + header@.subrange(
                            payload_start as int + local_start as int, payload_start as int + local_end as int));
                    }
                }
                logical_position = fragment_end;
            }
            position = end;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(crate::specs::part1_packed_header_scan(header@, marker, logical_start, logical_end, self.length, before)
                    == crate::specs::part1_packed_header_scan(header@, marker, logical_start, logical_end, self.length,
                        crate::specs::Part1PackedHeaderScan { position, logical_position, remaining_segments, output: output@ }));
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_packed_header_scan); }
        if output.len() != self.length {
            return Err(packet_input_error(
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

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn packet_length_source_view(source: PacketLengthSource<'_>) -> crate::specs::Part1PacketLengthSource {
        match source {
            PacketLengthSource::None => crate::specs::Part1PacketLengthSource::None,
            PacketLengthSource::Plm(bytes) => crate::specs::Part1PacketLengthSource::Plm(bytes@),
            PacketLengthSource::Plt(header) => crate::specs::Part1PacketLengthSource::Plt(header@),
        }
    }

    pub closed spec fn packet_lengths_view(lengths: &PacketLengths) -> crate::specs::Part1PacketLengths {
        crate::specs::Part1PacketLengths {
            source: packet_length_source_view(lengths.source), packet_count: lengths.packet_count, encoded_bytes: lengths.encoded_bytes,
        }
    }

    pub closed spec fn packet_length_iter_view(iter: &PacketLengthIter) -> crate::specs::Part1PacketLengthIter {
        crate::specs::Part1PacketLengthIter {
            source: packet_length_source_view(iter.source), position: iter.position, segment_end: iter.segment_end,
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures packet_lengths_view(&result) == (crate::specs::Part1PacketLengths {
        source: crate::specs::Part1PacketLengthSource::None, packet_count: 0, encoded_bytes: 0 }),
))]
pub(crate) const fn packet_lengths_none<'a>() -> PacketLengths<'a> {
    PacketLengths {
        source: PacketLengthSource::None,
        packet_count: 0,
        encoded_bytes: 0,
    }
}

impl PacketLengths<'_> {
    pub const fn none() -> Self {
        packet_lengths_none()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == !matches!(packet_lengths_view(self).source, crate::specs::Part1PacketLengthSource::None),
    ))]
    pub const fn is_present(&self) -> bool {
        !matches!(self.source, PacketLengthSource::None)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == packet_lengths_view(self).packet_count,))]
    pub const fn packet_count(&self) -> u64 {
        self.packet_count
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == packet_lengths_view(self).encoded_bytes,))]
    pub const fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures packet_length_iter_view(&result) == crate::specs::part1_packet_lengths_iter(packet_lengths_view(self)),
    ))]
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
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, packet_length_iter_view(final(self))) == crate::specs::part1_packet_length_next(packet_length_iter_view(old(self))),
            match result { Ok(Some(value)) => value > 0, _ => true },
            match result { Ok(Some(_)) => packet_length_iter_view(final(self)).position > packet_length_iter_view(old(self)).position, _ => true },
    ))]
    pub(crate) fn next_length(&mut self) -> Result<Option<u32>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost initial = packet_length_iter_view(self); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_packet_length_next); }
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
                    verus_spec(
                        invariant initial == packet_length_iter_view(old(self)),
                            packet_length_iter_view(self).source == initial.source,
                            initial.source == crate::specs::Part1PacketLengthSource::Plt(header@),
                            initial.position <= self.position,
                            crate::specs::part1_packet_length_next(initial)
                                == crate::specs::part1_packet_length_plt(packet_length_iter_view(self)),
                        decreases header.len() as int - self.position as int,
                    )
                )]
                loop {
                    #[cfg(verus_keep_ghost)]
                    proof! { reveal(crate::specs::part1_packet_length_plt); }
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
                        return Err(packet_input_error(
                            self.position as u64,
                            MalformedKind::InvalidPacketLength,
                        ));
                    }
                    let marker = crate::refinement::marker_value(
                        header[self.position],
                        header[self.position + 1],
                    );
                    let marker_length = usize::from(crate::refinement::marker_value(
                        header[self.position + 2],
                        header[self.position + 3],
                    ));
                    if marker_length < 2 {
                        return Err(packet_input_error(
                            self.position as u64,
                            MalformedKind::InvalidMarkerLength,
                        ));
                    }
                    let after_marker = match self.position.checked_add(2) {
                        Some(value) => value,
                        None => {
                            return Err(packet_input_error(
                                self.position as u64,
                                MalformedKind::ArithmeticOverflow,
                            ));
                        }
                    };
                    let end = match after_marker.checked_add(marker_length) {
                        Some(end) => end,
                        None => {
                            return Err(packet_input_error(
                                self.position as u64,
                                MalformedKind::ArithmeticOverflow,
                            ));
                        }
                    };
                    if end > header.len() {
                        return Err(packet_input_error(
                            self.position as u64,
                            MalformedKind::InvalidMarkerLength,
                        ));
                    }
                    self.position = end;
                    if marker == PLT {
                        let before_index = match self.position.checked_sub(marker_length) {
                            Some(value) => value,
                            None => {
                                return Err(packet_input_error(
                                    self.position as u64,
                                    MalformedKind::ArithmeticOverflow,
                                ));
                            }
                        };
                        let data_start = match before_index.checked_add(3) {
                            Some(start) => start,
                            None => {
                                return Err(packet_input_error(
                                    self.position as u64,
                                    MalformedKind::ArithmeticOverflow,
                                ));
                            }
                        };
                        if data_start > end {
                            return Err(packet_input_error(
                                self.position as u64,
                                MalformedKind::InvalidMarkerLength,
                            ));
                        }
                        self.position = data_start;
                        self.segment_end = end;
                    }
                    if self.position <= previous_position || self.position > header.len() {
                        return Err(packet_input_error(
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

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn quantization_owner_view(value: &ComponentQuantizations) -> (u16, Seq<Quantization>) {
        (value.components, value.quantizations@)
    }

    pub closed spec fn owned_decomposition_view(value: &DecompositionStructure) -> (u8, Seq<DecompositionDirection>) {
        (value.index, value.directions@)
    }

    pub closed spec fn owned_arbitrary_decomposition_view(value: &ArbitraryDecomposition) -> (u8, Seq<u8>, Seq<DecompositionDirection>) {
        (value.index, value.sublevel_depths@, value.subband_splits@)
    }

    pub closed spec fn owned_nonlinear_view(value: &NonlinearPointTransform) -> crate::specs::Part2OwnedNonlinear {
        crate::specs::Part2OwnedNonlinear {
            component: value.component, precision: value.precision, signed: value.signed, transform_type: value.transform_type,
            parameters: value.parameters@, d_min: value.d_min, d_max: value.d_max, point_bits: value.point_bits,
            points: value.points@, gamma: value.gamma_parameters,
        }
    }

    pub open spec fn nonlinear_transform_option_view(value: Option<&NonlinearPointTransform>)
        -> Option<crate::specs::Part2OwnedNonlinear>
    {
        match value { None => None, Some(value) => Some(owned_nonlinear_view(value)) }
    }

    pub closed spec fn owned_kernel_view(value: &ArbitraryKernel) -> (u8, u16, crate::specs::Part2RawKernel) {
        (value.index, value.descriptor, raw_kernel_view(*value))
    }

    pub closed spec fn owned_extensions_view(value: &Part2Extensions) -> crate::specs::Part2OwnedExtensions {
        crate::specs::Part2OwnedExtensions {
            decompositions: value.decomposition_structures@.map_values(|entry: DecompositionStructure| owned_decomposition_view(&entry)),
            arbitrary: value.arbitrary_decompositions@.map_values(|entry: ArbitraryDecomposition| owned_arbitrary_decomposition_view(&entry)),
            nonlinear: value.nonlinear_transforms@.map_values(|entry: NonlinearPointTransform| owned_nonlinear_view(&entry)),
            depths: value.component_bit_depths@,
            kernels: value.arbitrary_kernels@.map_values(|entry: ArbitraryKernel| owned_kernel_view(&entry)),
            registrations: value.component_registrations@, lengths: value.tile_part_lengths@, metadata_bytes: value.metadata_bytes,
        }
    }

    pub open spec fn tile_part_owned_view(value: &TilePartPayload<'_>) -> crate::specs::Part2OwnedTilePart {
        crate::specs::Part2OwnedTilePart {
            tile_index: value.tile_index, tile_part_index: value.tile_part_index, tile_part_count: value.tile_part_count,
            coding_style: value.coding_style, quantization: value.quantization, component_coding_styles: value.component_coding_styles,
            component_quantizations: quantization_owner_view(&value.component_quantizations), roi_shifts: value.roi_shifts,
            progression_changes: progression_changes_view(&value.progression_changes),
            multiple_component_transform: match value.multiple_component_transform { None => None, Some(transform) => Some(multiple_component_transform_view(&transform)) },
            part2_extensions: owned_extensions_view(&value.part2_extensions), packet_lengths: packet_lengths_view(&value.packet_lengths),
            packed_packet_headers: packed_packet_headers_view(&value.packed_packet_headers),
            has_functional_header_marker: value.has_functional_header_marker, has_roi_marker: value.has_roi_marker, data: value.data@,
        }
    }

    pub open spec fn tile_parts_owned_view(values: Seq<TilePartPayload<'_>>) -> Seq<crate::specs::Part2OwnedTilePart> {
        values.map_values(|value: TilePartPayload| tile_part_owned_view(&value))
    }

    pub proof fn tile_part_owned_tile_known(value: &TilePartPayload<'_>)
        ensures tile_part_owned_view(value).tile_index == value.tile_index,
    {
        reveal(tile_part_owned_view);
    }

    pub proof fn packed_header_presence_matches_view(value: &PackedPacketHeaders<'_>)
        ensures packed_packet_headers_present(value)
            == !matches!(packed_packet_headers_view(value).source, crate::specs::Part1PackedHeaderSource::None),
    {
        reveal(packed_packet_headers_present);
        reveal(packed_packet_headers_view);
    }

    pub proof fn owned_extensions_registry_known(value: &Part2Extensions)
        ensures crate::specs::part2_owned_kernel_registry(owned_extensions_view(value)) == part2_kernel_registry(value),
    {
        reveal(owned_extensions_view);
        reveal(owned_kernel_view);
        reveal(part2_kernel_registry);
        assert(crate::specs::part2_owned_kernel_registry(owned_extensions_view(value)) =~= part2_kernel_registry(value));
    }
}

impl Clone for ProgressionChanges {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures progression_changes_view(&result) == progression_changes_view(self),))]
    fn clone(&self) -> Self {
        Self {
            records: self.records.clone(),
        }
    }
}

impl Clone for ComponentQuantizations {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures quantization_owner_view(&result) == quantization_owner_view(self),))]
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(quantization_owner_view); }
        Self {
            components: self.components,
            quantizations: self.quantizations.clone(),
        }
    }
}

impl Clone for DecompositionStructure {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures owned_decomposition_view(&result) == owned_decomposition_view(self),))]
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(owned_decomposition_view); }
        Self {
            index: self.index,
            directions: self.directions.clone(),
        }
    }
}

impl Clone for ArbitraryDecomposition {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures owned_arbitrary_decomposition_view(&result) == owned_arbitrary_decomposition_view(self),))]
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(owned_arbitrary_decomposition_view); }
        Self {
            index: self.index,
            sublevel_depths: self.sublevel_depths.clone(),
            subband_splits: self.subband_splits.clone(),
        }
    }
}

impl Clone for NonlinearPointTransform {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures owned_nonlinear_view(&result) == owned_nonlinear_view(self),))]
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(owned_nonlinear_view); }
        Self {
            component: self.component,
            precision: self.precision,
            signed: self.signed,
            transform_type: self.transform_type,
            parameters: self.parameters.clone(),
            d_min: self.d_min,
            d_max: self.d_max,
            point_bits: self.point_bits,
            points: self.points.clone(),
            gamma_parameters: self.gamma_parameters,
        }
    }
}

impl Clone for KernelLiftingStep {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures raw_kernel_step_view(result) == raw_kernel_step_view(*self),))]
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(raw_kernel_step_view); }
        Self {
            offset: self.offset,
            exponent: self.exponent,
            residue: self.residue,
            coefficients: self.coefficients.clone(),
        }
    }
}

impl Clone for ArbitraryKernel {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures owned_kernel_view(&result) == owned_kernel_view(self),))]
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(owned_kernel_view); reveal(raw_kernel_view); }
        Self {
            index: self.index,
            reversible: self.reversible,
            descriptor: self.descriptor,
            coefficient_type: self.coefficient_type,
            whole_sample_filter: self.whole_sample_filter,
            initial_odd: self.initial_odd,
            whole_sample_extension: self.whole_sample_extension,
            scaling: self.scaling,
            steps: self.steps.clone(),
        }
    }
}

impl Clone for Part2Extensions {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures owned_extensions_view(&result) == owned_extensions_view(self),))]
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(owned_extensions_view); }
        let copied = Self {
            decomposition_structures: self.decomposition_structures.clone(),
            arbitrary_decompositions: self.arbitrary_decompositions.clone(),
            nonlinear_transforms: self.nonlinear_transforms.clone(),
            component_bit_depths: self.component_bit_depths.clone(),
            arbitrary_kernels: self.arbitrary_kernels.clone(),
            component_registrations: self.component_registrations.clone(),
            tile_part_lengths: self.tile_part_lengths.clone(),
            metadata_bytes: self.metadata_bytes,
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            let a = owned_extensions_view(&copied);
            let b = owned_extensions_view(self);
            assert(a.decompositions =~= b.decompositions);
            assert(a.arbitrary =~= b.arbitrary);
            assert(a.nonlinear =~= b.nonlinear);
            assert(a.depths =~= b.depths);
            assert(a.kernels =~= b.kernels);
            assert(a.registrations =~= b.registrations);
            assert(a.lengths =~= b.lengths);
        }
        copied
    }
}

impl Clone for MctArray {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures mct_array_view(&result) == mct_array_view(self),))]
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(mct_array_view); }
        Self {
            index: self.index,
            kind: self.kind,
            numeric_format: self.numeric_format,
            fraction_bits: self.fraction_bits,
            values: self.values.clone(),
        }
    }
}

impl Clone for MctCollection {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures mct_collection_view(&result) == mct_collection_view(self),))]
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(mct_collection_view); }
        Self {
            reversible: self.reversible,
            input_components: self.input_components.clone(),
            output_components: self.output_components.clone(),
            kind: self.kind,
            matrix: self.matrix.clone(),
            offsets: self.offsets.clone(),
            wavelet: self.wavelet,
        }
    }
}

impl Clone for MctStage {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures mct_stage_view(&result) == mct_stage_view(self),))]
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(mct_stage_view); }
        Self {
            index: self.index,
            collections: self.collections.clone(),
        }
    }
}

impl Clone for MultipleComponentTransform {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures multiple_component_transform_view(&result) == multiple_component_transform_view(self),))]
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(multiple_component_transform_view); }
        Self {
            stages: self.stages.clone(),
        }
    }
}

impl<'a> Clone for TilePartPayload<'a> {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures tile_part_owned_view(&result) == crate::specs::part2_tile_part_copy(tile_part_owned_view(self)),
    ))]
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(tile_part_owned_view); }
        Self {
            tile_index: self.tile_index,
            tile_part_index: self.tile_part_index,
            tile_part_count: self.tile_part_count,
            coding_style: self.coding_style,
            quantization: self.quantization,
            component_coding_styles: self.component_coding_styles,
            component_quantizations: self.component_quantizations.clone(),
            roi_shifts: self.roi_shifts,
            progression_changes: self.progression_changes.clone(),
            multiple_component_transform: self.multiple_component_transform.clone(),
            part2_extensions: self.part2_extensions.clone(),
            packet_lengths: self.packet_lengths,
            packed_packet_headers: self.packed_packet_headers,
            has_functional_header_marker: self.has_functional_header_marker,
            has_roi_marker: self.has_roi_marker,
            data: self.data,
        }
    }
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
    new_parser(input, limits)?.parse_layout()
}

struct Parser<'a> {
    input: &'a [u8],
    limits: &'a ResourceLimits,
    position: usize,
    work_units: u64,
    packet_length_bytes: u64,
    packet_lengths: u64,
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn parser_view<'a>(parser: &Parser<'a>) -> crate::specs::Part1CodestreamReader<'a> {
        crate::specs::Part1CodestreamReader {
            input: parser.input@, limits: parser.limits, position: parser.position,
            work_units: parser.work_units, packet_length_bytes: parser.packet_length_bytes, packet_lengths: parser.packet_lengths,
        }
    }

    closed spec fn parser_initial_outcome<'a>(result: Result<Parser<'a>, DecodeError>)
        -> Result<crate::specs::Part1CodestreamReader<'a>, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(parser) => Ok(parser_view(&parser)) }
    }

    closed spec fn segment_view(segment: Segment<'_>) -> crate::specs::Part1CodestreamSegment {
        crate::specs::Part1CodestreamSegment { marker: segment.marker, offset: segment.offset, payload: segment.payload@ }
    }

    closed spec fn segment_outcome(result: Result<Segment<'_>, DecodeError>)
        -> Result<crate::specs::Part1CodestreamSegment, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(segment) => Ok(segment_view(segment)) }
    }

    closed spec fn parsed_siz_outcome(result: Result<(ImageSize, u16), DecodeError>)
        -> Result<(crate::specs::Part1ParsedImageSize, u16), DecodeError>
    {
        match result { Err(error) => Err(error), Ok((size, r_siz)) => Ok((parsed_image_size_view(&size), r_siz)) }
    }

    closed spec fn siz_components_view(
        infos: Seq<ComponentInfo>, offset: usize, component: u16, first_precision: u8, first_signed: bool, uniform: bool,
    ) -> crate::specs::Part1SizComponents {
        crate::specs::Part1SizComponents { infos, offset, component, first_precision, first_signed, uniform }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures parser_initial_outcome(result) == crate::specs::part1_codestream_reader_new(input@, limits),
))]
fn new_parser<'a>(input: &'a [u8], limits: &'a ResourceLimits) -> Result<Parser<'a>, DecodeError> {
    let input_length = input.len() as u64;
    if input_length > limits.maximum_input_bytes() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::InputBytes,
            limit: limits.maximum_input_bytes(),
            actual: input_length,
        });
    }
    Ok(Parser {
        input,
        limits,
        position: 0,
        work_units: 0,
        packet_length_bytes: 0,
        packet_lengths: 0,
    })
}

impl<'a> Parser<'a> {
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
        let mut qcd_segment = None;
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
                QCD if qcd_segment.is_some() => {
                    return Err(DecodeError::malformed(
                        parser.offset(segment.offset),
                        MalformedKind::InvalidMarkerOrder,
                    ));
                }
                QCD => qcd_segment = Some(segment),
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
        let qcd_segment = qcd_segment
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))?;
        let quantization = parser.parse_qcd(qcd_segment, coding_style)?;
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures parsed_coding_style_outcome(result) == crate::specs::part1_parse_cod_payload(segment.offset, segment.payload@, self.limits),
    ))]
    fn parse_cod(&self, segment: Segment<'a>) -> Result<CodingStyle, DecodeError> {
        self.parse_cod_payload(segment.offset, segment.payload)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures parsed_coding_style_outcome(result) == crate::specs::part1_parse_cod_payload(segment_offset, payload@, self.limits),
    ))]
    fn parse_cod_payload(
        &self,
        segment_offset: usize,
        payload: &[u8],
    ) -> Result<CodingStyle, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_parse_cod_payload);
            reveal(crate::specs::part1_cod_header);
        }
        if payload.len() < 10 {
            return Err(self.malformed_at(segment_offset, MalformedKind::InvalidMarkerLength));
        }
        let flags = payload[0];
        if flags & 0xf8 != 0 {
            return Err(self.malformed_at(segment_offset, MalformedKind::InvalidCodingStyle));
        }
        let progression_order = match payload[1] {
            0 => ProgressionOrder::Lrcp,
            1 => ProgressionOrder::Rlcp,
            2 => ProgressionOrder::Rpcl,
            3 => ProgressionOrder::Pcrl,
            4 => ProgressionOrder::Cprl,
            _ => {
                return Err(self.malformed_at(segment_offset, MalformedKind::InvalidCodingStyle));
            }
        };
        let layers = read_u16([payload[2], payload[3]]);
        if layers == 0 || layers > self.limits.maximum_layers() {
            return Err(if layers > self.limits.maximum_layers() {
                DecodeError::LimitExceeded {
                    resource: Resource::Layers,
                    limit: u64::from(self.limits.maximum_layers()),
                    actual: u64::from(layers),
                }
            } else {
                self.malformed_at(segment_offset, MalformedKind::InvalidCodingStyle)
            });
        }
        let multiple_component_transform = match payload[4] {
            0 => false,
            1 => true,
            _ => {
                return Err(self.malformed_at(segment_offset, MalformedKind::InvalidCodingStyle));
            }
        };
        let decomposition_levels = payload[5];
        let resolution_levels = match decomposition_levels.checked_add(1) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment_offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if resolution_levels > self.limits.maximum_resolutions_per_component() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Resolutions,
                limit: u64::from(self.limits.maximum_resolutions_per_component()),
                actual: u64::from(resolution_levels),
            });
        }
        let width_exponent = payload[6];
        let height_exponent = payload[7];
        if width_exponent > 8 || height_exponent > 8 {
            return Err(self.malformed_at(segment_offset, MalformedKind::InvalidCodingStyle));
        }
        let code_block_width = 1_u16 << (u32::from(width_exponent) + 2);
        let code_block_height = 1_u16 << (u32::from(height_exponent) + 2);
        let code_block_samples = match u32::from(code_block_width)
            .checked_mul(u32::from(code_block_height))
        {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment_offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if code_block_samples > 4096
            || crate::tier1::tier1_block_coding_from_code_block_style(payload[8]).is_err()
        {
            return Err(self.malformed_at(segment_offset, MalformedKind::InvalidCodingStyle));
        }
        let transform = match payload[9] {
            0 => WaveletTransform::Irreversible9x7,
            1 => WaveletTransform::Reversible5x3,
            index => WaveletTransform::Arbitrary(index),
        };
        let precincts_defined = flags & 1 != 0;
        let precinct_count = usize::from(resolution_levels);
        let expected_length = if precincts_defined {
            match 10_usize.checked_add(precinct_count) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment_offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            }
        } else {
            10
        };
        if payload.len() != expected_length {
            return Err(self.malformed_at(segment_offset, MalformedKind::InvalidMarkerLength));
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost h = crate::specs::Part1CodHeader {
                flags, progression_order, layers, multiple_component_transform, decomposition_levels,
                code_block_width, code_block_height, code_block_style: payload@[8], transform, precinct_count,
            };
            let ghost initial_precincts = Seq::new(33, |i: int| (15u8, 15u8));
            let ghost precinct_goal = if precincts_defined {
                crate::specs::part1_cod_precincts(segment_offset, payload@, precinct_count, 0, initial_precincts)
            } else { Ok(initial_precincts) };
            let ghost goal = crate::specs::part1_parse_cod_payload(segment_offset, payload@, self.limits);
        }
        #[cfg(verus_keep_ghost)]
        proof! { assert(crate::specs::part1_cod_header(segment_offset, payload@, self.limits) == Ok(h)); }
        let mut precinct_exponents = [(15_u8, 15_u8); 33];
        #[cfg(verus_keep_ghost)]
        proof! { assert(precinct_exponents@ =~= initial_precincts); }
        if precincts_defined {
            let mut resolution = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant resolution <= precinct_count,
                    payload.len() as int == 10 + precinct_count as int,
                    precincts_defined,
                    flags & 1 != 0,
                    h == (crate::specs::Part1CodHeader {
                        flags, progression_order, layers, multiple_component_transform, decomposition_levels,
                        code_block_width, code_block_height, code_block_style: payload@[8], transform, precinct_count,
                    }),
                    goal == crate::specs::part1_parse_cod_payload(segment_offset, payload@, self.limits),
                    goal == match precinct_goal { Err(error) => Err(error), Ok(values) => Ok(crate::specs::part1_cod_from_header(h, values)) },
                    precinct_goal == crate::specs::part1_cod_precincts(
                        segment_offset, payload@, precinct_count, resolution, precinct_exponents@,
                    ),
                    decreases precinct_count - resolution,
                )
            )]
            while resolution < precinct_count {
                #[cfg(verus_keep_ghost)]
                proof! {
                    reveal(crate::specs::part1_parse_cod_payload);
                    reveal_with_fuel(crate::specs::part1_cod_precincts, 1);
                }
                let index = match 10_usize.checked_add(resolution) {
                    Some(value) => value,
                    None => {
                        return Err(
                            self.malformed_at(segment_offset, MalformedKind::ArithmeticOverflow)
                        );
                    }
                };
                if index >= payload.len() || resolution >= precinct_exponents.len() {
                    return Err(
                        self.malformed_at(segment_offset, MalformedKind::InvalidCodingStyle)
                    );
                }
                precinct_exponents[resolution] = (payload[index] & 0x0f, payload[index] >> 4);
                resolution = match resolution.checked_add(1) {
                    Some(value) => value,
                    None => {
                        return Err(
                            self.malformed_at(segment_offset, MalformedKind::ArithmeticOverflow)
                        );
                    }
                };
            }
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_cod_precincts, 1); }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_parse_cod_payload);
            assert(precinct_goal == Ok(precinct_exponents@));
            assert(goal == Ok(crate::specs::part1_cod_from_header(h, precinct_exponents@)));
        }
        let style = CodingStyle {
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
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(style.decomposition_directions@ =~= Seq::new(32, |i: int| DecompositionDirection::TwoDimensional));
            assert(goal == Ok(parsed_coding_style_view(style)));
        }
        Ok(style)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_parse_component_index(segment.offset, segment.payload@, components, invalid_kind),
    ))]
    fn parse_component_index(
        &self,
        segment: Segment<'a>,
        components: u16,
        invalid_kind: MalformedKind,
    ) -> Result<(u16, usize), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_component_index); }
        let component_bytes = if components < 257 { 1_usize } else { 2_usize };
        if segment.payload.len() < component_bytes {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let component = if component_bytes == 1 {
            u16::from(segment.payload[0])
        } else {
            read_u16([segment.payload[0], segment.payload[1]])
        };
        if component >= components {
            return Err(self.malformed_at(segment.offset, invalid_kind));
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

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures parsed_component_coding_style_outcome(result) == crate::specs::part1_parse_coc(
            segment.offset, segment.payload@, components, parsed_coding_style_view(default), self.limits),
    ))]
    fn parse_coc(
        &self,
        segment: Segment<'a>,
        components: u16,
        default: CodingStyle,
    ) -> Result<(u16, CodingStyle), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_coc); }
        let prepared = self.prepare_coc_payload(segment, components, default)?;
        let mut style =
            self.parse_cod_payload(segment.offset, &prepared.payload[..prepared.length])?;
        style.decomposition_structure = prepared.decomposition_structure;
        Ok((prepared.component, style))
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures parsed_component_coding_style_payload_outcome(result) == crate::specs::part1_coc_prepare(
            segment.offset, segment.payload@, components, parsed_coding_style_view(default)),
        component_coding_style_payload_is_bounded(result),
    ))]
    fn prepare_coc_payload(
        &self,
        segment: Segment<'a>,
        components: u16,
        default: CodingStyle,
    ) -> Result<ComponentCodingStylePayload, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_coc_prepare);
            reveal(crate::specs::part1_parse_component_index);
        }
        let (component, component_bytes) =
            self.parse_component_index(segment, components, MalformedKind::InvalidCodingStyle)?;
        let minimum = component_bytes
            .checked_add(6)
            .ok_or_else(|| self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow))?;
        if segment.payload.len() < minimum {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let scoc = segment.payload[component_bytes];
        if scoc & 0xfe != 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidCodingStyle));
        }
        let parameters = segment
            .payload
            .get(component_bytes + 1..)
            .ok_or_else(|| self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength))?;
        let decomposition_structure = if parameters[0] & 0x80 != 0 {
            let index = parameters[0] & 0x7f;
            if index == 0 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidCodingStyle));
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
        #[cfg(verus_keep_ghost)]
        proof! {
            let layers = default.layers;
            assert(layers >> 8 <= 255) by (bit_vector);
            assert(layers & 0xff <= 255) by (bit_vector);
        }
        cod_payload[2] = u8::try_from(default.layers >> 8)
            .map_err(|_| self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow))?;
        cod_payload[3] = u8::try_from(default.layers & 0xff)
            .map_err(|_| self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow))?;
        cod_payload[4] = if default.multiple_component_transform {
            1
        } else {
            0
        };
        let end = match 5_usize.checked_add(parameters.len()) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if end > cod_payload.len() {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost before_parameters = cod_payload@; }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(cod_payload@[0] == (scoc & 1) | (if default.sop_markers { 2u8 } else { 0u8 }) | (if default.eph_markers { 4u8 } else { 0u8 }));
            assert(cod_payload@[1] == crate::specs::part1_cod_progression_byte(default.progression_order));
            assert(cod_payload@[2] == (default.layers >> 8) as u8);
            assert(cod_payload@[3] == (default.layers & 255) as u8);
            assert(cod_payload@[4] == if default.multiple_component_transform { 1u8 } else { 0u8 });
        }
        cod_payload[5..end].copy_from_slice(parameters);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(cod_payload@.subrange(0, 5) =~= before_parameters.subrange(0, 5));
            assert(cod_payload@.subrange(5, end as int) =~= parameters@);
        }
        if decomposition_structure.is_some() {
            cod_payload[5] = default.decomposition_levels;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(cod_payload@.subrange(0, end as int) =~= crate::specs::part1_coc_cod_payload(
                parameters@, scoc, parsed_coding_style_view(default), decomposition_structure));
        }
        Ok(ComponentCodingStylePayload {
            component,
            payload: cod_payload,
            length: end,
            decomposition_structure,
        })
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures parsed_siz_outcome(result) == crate::specs::part1_parse_siz(segment_view(segment), self.limits),
    ))]
    fn parse_siz(&self, segment: Segment<'a>) -> Result<(ImageSize, u16), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_parse_siz);
            reveal(crate::specs::part1_siz_header);
        }
        if segment.payload.len() < 36 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
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
        let first_tile_x1 = match xtosiz.checked_add(xtsiz) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let first_tile_y1 = match ytosiz.checked_add(ytsiz) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let component_bytes = match usize::from(components).checked_mul(3) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let expected_length = match 36_usize.checked_add(component_bytes) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
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
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidImageGeometry));
        }
        if components > self.limits.maximum_components() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Components,
                limit: u64::from(self.limits.maximum_components()),
                actual: u64::from(components),
            });
        }

        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost h = crate::specs::Part1SizHeader {
                xsiz, ysiz, xosiz, yosiz, xtsiz, ytsiz, xtosiz, ytosiz, components,
                r_siz: crate::specs::part1_marker_value(segment.payload@[0], segment.payload@[1]),
            };
            let ghost component_goal = crate::specs::part1_siz_components(
                segment.payload@, segment.offset, crate::specs::part1_siz_components_initial(),
            );
            let ghost goal = crate::specs::part1_parse_siz(segment_view(segment), self.limits);
        }
        #[cfg(verus_keep_ghost)]
        proof! { assert(crate::specs::part1_siz_header(segment_view(segment), self.limits) == Ok(h)); }
        let mut component_offset = 36_usize;
        let mut first_precision = 0_u8;
        let mut first_signed = false;
        let mut components_uniform = true;
        let mut component_index = 0_u16;
        let mut component_info = [ComponentInfo {
            precision: 0,
            signed: false,
            x_subsampling: 0,
            y_subsampling: 0,
            x_registration: 0,
            y_registration: 0,
        }; MAX_COMPONENTS];
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(component_info@ =~= Seq::new(MAX_COMPONENTS as nat, |i: int| crate::specs::part1_empty_component_info()));
            assert(siz_components_view(component_info@, component_offset, component_index,
                first_precision, first_signed, components_uniform) == crate::specs::part1_siz_components_initial());
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_offset <= segment.payload.len(),
                component_index <= components,
                component_offset as int == 36 + 3 * component_index as int,
                segment.payload.len() as int == 36 + 3 * components as int,
                h == (crate::specs::Part1SizHeader {
                    xsiz, ysiz, xosiz, yosiz, xtsiz, ytsiz, xtosiz, ytosiz, components,
                    r_siz: crate::specs::part1_marker_value(segment.payload@[0], segment.payload@[1]),
                }),
                crate::specs::part1_siz_header(segment_view(segment), self.limits) == Ok(h),
                xsiz > xosiz, ysiz > yosiz, xosiz >= xtosiz, yosiz >= ytosiz, xtsiz > 0, ytsiz > 0,
                goal == crate::specs::part1_parse_siz(segment_view(segment), self.limits),
                goal == match component_goal {
                    Err(error) => Err(error),
                    Ok(state) => crate::specs::part1_siz_finish(h, state, segment.offset, self.limits),
                },
                component_goal == crate::specs::part1_siz_components(
                    segment.payload@, segment.offset,
                    siz_components_view(component_info@, component_offset, component_index, first_precision, first_signed, components_uniform),
                ),
                decreases segment.payload.len() - component_offset,
            )
        )]
        while component_offset < segment.payload.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal(crate::specs::part1_parse_siz);
                reveal_with_fuel(crate::specs::part1_siz_components, 1);
            }
            if segment.payload.len() - component_offset < 3 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
            }
            let precision_value = u16::from(segment.payload[component_offset] & 0x7f) + 1;
            let signed = segment.payload[component_offset] & 0x80 != 0;
            if precision_value > 38
                || segment.payload[component_offset + 1] == 0
                || segment.payload[component_offset + 2] == 0
            {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidImageGeometry));
            }
            let precision = match u8::try_from(precision_value) {
                Ok(value) => value,
                Err(_) => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            let info_index = usize::from(component_index);
            if info_index >= component_info.len() {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidImageGeometry));
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
            component_index = match component_index.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            component_offset = match component_offset.checked_add(3) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal_with_fuel(crate::specs::part1_siz_components, 1);
            reveal(crate::specs::part1_siz_finish);
        }

        let width = xsiz - xosiz;
        let height = ysiz - yosiz;
        if width > self.limits.maximum_width() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Width,
                limit: u64::from(self.limits.maximum_width()),
                actual: u64::from(width),
            });
        }
        if height > self.limits.maximum_height() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Height,
                limit: u64::from(self.limits.maximum_height()),
                actual: u64::from(height),
            });
        }
        let pixels = match u64::from(width).checked_mul(u64::from(height)) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if pixels > self.limits.maximum_pixels() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Pixels,
                limit: self.limits.maximum_pixels(),
                actual: pixels,
            });
        }

        let tiles_across = ceil_div(xsiz - xtosiz, xtsiz)?;
        let tiles_down = ceil_div(ysiz - ytosiz, ytsiz)?;
        let tiles = match u64::from(tiles_across).checked_mul(u64::from(tiles_down)) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if tiles > u64::from(self.limits.maximum_tiles()) {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Tiles,
                limit: u64::from(self.limits.maximum_tiles()),
                actual: tiles,
            });
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self))) == crate::specs::part1_codestream_find_eoc(parser_view(old(self))),
    ))]
    fn find_eoc(&mut self) -> Result<usize, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost goal = crate::specs::part1_codestream_find_eoc(input);
        }
        let input_len = self.input.len();
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant input == parser_view(old(self)),
                    self.input@ == input.input, self.limits == input.limits,
                    self.packet_length_bytes == input.packet_length_bytes,
                    self.packet_lengths == input.packet_lengths,
                    input_len == input.input.len(),
                    goal == crate::specs::part1_codestream_find_eoc(input),
                    goal == crate::specs::part1_codestream_find_eoc(parser_view(self)),
                decreases input_len as int - self.position as int,
            )
        )]
        while self.position < input_len {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_codestream_find_eoc, 1); }
            if input_len - self.position < 2 {
                return Err(self.malformed_here(MalformedKind::Truncated));
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
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(self.position >= input_len || (input_len as int) - (self.position as int) < 2);
            reveal_with_fuel(crate::specs::part1_codestream_find_eoc, 1);
            assert(goal == (Err(crate::specs::part1_codestream_malformed(self.position, MalformedKind::Truncated)), parser_view(self)));
        }
        Err(self.malformed_here(MalformedKind::Truncated))
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (segment_outcome(result), parser_view(final(self))) == crate::specs::part1_codestream_read_segment(parser_view(old(self))),
    ))]
    fn read_segment(&mut self) -> Result<Segment<'a>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_codestream_read_segment); }
        let offset = self.position;
        let marker = self.peek_marker()?;
        self.charge_work(1)?;
        if is_standalone_marker(marker) && marker != SOT {
            return Err(self.malformed_here(MalformedKind::InvalidMarkerOrder));
        }
        let length_offset = match offset.checked_add(2) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
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
        let end = match length_offset.checked_add(length) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self))) == crate::specs::part1_codestream_expect_standalone(parser_view(old(self)), expected),
    ))]
    fn expect_standalone(&mut self, expected: u16) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_codestream_expect_standalone); }
        if self.peek_marker()? != expected {
            return Err(self.malformed_here(MalformedKind::InvalidMarkerOrder));
        }
        self.position = match self.position.checked_add(2) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        self.charge_work(1)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_codestream_peek_marker(parser_view(self)),
    ))]
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self))) == crate::specs::part1_codestream_charge_work(parser_view(old(self)), amount),
    ))]
    fn charge_work(&mut self, amount: u64) -> Result<(), DecodeError> {
        self.work_units = match self.work_units.checked_add(amount) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        if self.work_units > self.limits.maximum_work_units() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: self.limits.maximum_work_units(),
                actual: self.work_units,
            });
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_codestream_malformed(self.position, kind),
    ))]
    fn malformed_here(&self, kind: MalformedKind) -> DecodeError {
        self.malformed_at(self.position, kind)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_codestream_malformed(value, kind),
    ))]
    fn malformed_at(&self, value: usize, kind: MalformedKind) -> DecodeError {
        DecodeError::MalformedInput {
            offset: self.offset(value),
            kind,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_codestream_offset(value),
    ))]
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_codestream_standalone(marker),
))]
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_ceil_div(value, divisor),
))]
fn ceil_div(value: u32, divisor: u32) -> Result<u32, DecodeError> {
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_marker_value(bytes[0], bytes[1]),
))]
fn read_u16(bytes: [u8; 2]) -> u16 {
    crate::refinement::marker_value(bytes[0], bytes[1])
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_codestream_u32(bytes[0], bytes[1], bytes[2], bytes[3]),
))]
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_packet_length_decode(encoded@, start, end),
        match result { Ok((value, next)) => value > 0 && start < next && next <= end, Err(_) => true },
))]
fn decode_packet_length(
    encoded: &[u8],
    start: usize,
    end: usize,
) -> Result<(u32, usize), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_packet_length_decode); }
    if start >= end || end > encoded.len() {
        return Err(packet_input_error(
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
            invariant start <= position, position <= end, end <= encoded.len(),
                crate::specs::part1_packet_length_decode(encoded@, start, end)
                    == crate::specs::part1_packet_length_groups(encoded@, start, end, position, value, groups),
            decreases end - position,
        )
    )]
    loop {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_packet_length_groups); }
        if position >= end || position >= encoded.len() {
            return Err(packet_input_error(
                start as u64,
                MalformedKind::InvalidPacketLength,
            ));
        }
        let byte = encoded[position];
        groups = match groups.checked_add(1) {
            Some(count) => count,
            None => {
                return Err(packet_input_error(
                    start as u64,
                    MalformedKind::ArithmeticOverflow,
                ));
            }
        };
        if groups > 5 || (groups == 5 && value > 0x01ff_ffff) {
            return Err(packet_input_error(
                start as u64,
                MalformedKind::InvalidPacketLength,
            ));
        }
        let prefix = match value.checked_mul(128) {
            Some(prefix) => prefix,
            None => {
                return Err(packet_input_error(
                    start as u64,
                    MalformedKind::InvalidPacketLength,
                ));
            }
        };
        value = match prefix.checked_add(u32::from(byte & 0x7f)) {
            Some(word) => word,
            None => {
                return Err(packet_input_error(
                    start as u64,
                    MalformedKind::InvalidPacketLength,
                ));
            }
        };
        position = match position.checked_add(1) {
            Some(next) => next,
            None => {
                return Err(packet_input_error(
                    start as u64,
                    MalformedKind::ArithmeticOverflow,
                ));
            }
        };
        if byte & 0x80 == 0 {
            if value == 0 {
                return Err(packet_input_error(
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
    use crate::tier1::SubbandOrientation;

    #[test]
    fn raw_component_quantization_and_roi_getters_preserve_logical_physical_policy() {
        let quantization = raw_subband_quantization(QuantizationStyle::ScalarDerived, u16::MAX, 7);
        let roi = RoiShifts {
            shifts: core::array::from_fn(|index| u8::try_from(index).unwrap().wrapping_mul(17)),
        };
        for components in [0, 1, 2, 63, 64, 65, u16::MAX] {
            for physical in [0, 1, 2, 64, 65] {
                let values = ComponentQuantizations {
                    components,
                    quantizations: vec![quantization; physical],
                };
                for component in 0..=u16::MAX {
                    let expected = if component < components && usize::from(component) < physical {
                        Ok(quantization)
                    } else {
                        Err(DecodeError::MalformedInput {
                            offset: 0,
                            kind: MalformedKind::InvalidQuantization,
                        })
                    };
                    assert_eq!(values.quantization(component), expected);
                }
            }
        }
        for component in 0..=u16::MAX {
            let expected = roi.shifts.get(usize::from(component)).copied().ok_or(
                DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::InvalidRegionOfInterest,
                },
            );
            assert_eq!(roi.shift(component), expected);
        }
    }

    fn reference_component_plan_metadata(
        size: &ImageSize,
        styles: &ComponentCodingStyles,
        quantizations: &ComponentQuantizations,
        roi: &RoiShifts,
        component: u16,
        selection: (u16, u8),
        force: bool,
    ) -> Result<crate::decode::PlannedComponentMetadata, DecodeError> {
        let invalid_style = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodingStyle,
        };
        if component >= styles.components {
            return Err(invalid_style);
        }
        let style = *styles
            .styles
            .get(usize::from(component))
            .ok_or(invalid_style)?;
        if selection.1 > style.decomposition_levels {
            return Err(DecodeError::InvalidSelection {
                reason: crate::SelectionError::ReductionTooLarge,
            });
        }
        let output = style.decomposition_levels - selection.1;
        let invalid_quant = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidQuantization,
        };
        if component >= quantizations.components {
            return Err(invalid_quant);
        }
        let quantization = quantizations
            .quantizations
            .get(usize::from(component))
            .ok_or(invalid_quant)?;
        let mut tier1_style =
            crate::tier1::tier1_coding_style_from_code_block_style(style.code_block_style)?;
        if force
            && matches!(
                tier1_style.block_coding,
                crate::tier1::Tier1BlockCoding::HighThroughput
                    | crate::tier1::Tier1BlockCoding::Mixed
            )
        {
            tier1_style.force_high_throughput = true;
        }
        let bounds = reference_tile_component_bounds(size, selection.0, component)?;
        let geometry =
            reference_resolution_geometry(size, style, component, output, Some(selection.0))?;
        let scale = (0..selection.1).try_fold((1, 1), |previous, stage| {
            reference_resolution_scale_step(style, stage, previous)
        })?;
        let invalid_geometry = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidImageGeometry,
        };
        if component >= size.components {
            return Err(invalid_geometry);
        }
        let info = size
            .component_info
            .get(usize::from(component))
            .ok_or(invalid_geometry)?;
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodeBlock,
        };
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let ceil = |value: u32, divisor: u32| {
            if divisor == 0 {
                return Err(invalid);
            }
            Ok(u32::try_from(u64::from(value).div_ceil(u64::from(divisor))).unwrap())
        };
        let origin_x = ceil(size.x_origin, u32::from(info.x_subsampling))?;
        let origin_y = ceil(size.y_origin, u32::from(info.y_subsampling))?;
        let x = u32::try_from(
            i64::from(ceil(bounds.x0, scale.0)?) - i64::from(ceil(origin_x, scale.0)?),
        )
        .map_err(|_| arithmetic)?;
        let y = u32::try_from(
            i64::from(ceil(bounds.y0, scale.1)?) - i64::from(ceil(origin_y, scale.1)?),
        )
        .map_err(|_| arithmetic)?;
        let shift = *roi
            .shifts
            .get(usize::from(component))
            .ok_or(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidRegionOfInterest,
            })?;
        let subbands = reference_component_subbands(
            size,
            style,
            quantization,
            component,
            Some(selection.0),
            shift,
        )?;
        Ok(crate::decode::PlannedComponentMetadata {
            component_coding_style: style,
            output_resolution: output,
            tier1_style,
            bounds,
            component_width: geometry.width,
            component_height: geometry.height,
            component_x: x,
            component_y: y,
            subbands,
        })
    }

    #[test]
    fn actual_component_plan_metadata_preserves_all_ordered_raw_admission_and_output_fields() {
        let mut cases = 0;
        let mut successes = 0;
        for components in [0, 1, 2, 64, 65, u16::MAX] {
            let mut size = raw_resolution_size(33, 1, (1, 1), 0, components);
            for info in &mut size.component_info {
                info.precision = 8;
            }
            for style_owner in [0, 1, 2, 64, u16::MAX] {
                for quant_owner in [0, 1, 2, u16::MAX] {
                    for physical in [0, 1, 2] {
                        let quantizations = ComponentQuantizations {
                            components: quant_owner,
                            quantizations: vec![
                                raw_subband_quantization(
                                    QuantizationStyle::ScalarDerived,
                                    1,
                                    4
                                );
                                physical
                            ],
                        };
                        for component in [0, 1, 63, 64, u16::MAX] {
                            for tile in [0, u16::MAX] {
                                for discard in [0, 1, 2, u8::MAX] {
                                    let roi = RoiShifts {
                                        shifts: [if discard == 1 { u8::MAX } else { 0 };
                                            MAX_COMPONENTS],
                                    };
                                    for code_style in [0, 0x40, 0xc0, 0xff] {
                                        let mut style = raw_resolution_style(1, 3);
                                        style.code_block_style = code_style;
                                        style.code_block_width = 4;
                                        style.code_block_height = 4;
                                        style.precinct_exponents = [(0, 0); 33];
                                        let styles = ComponentCodingStyles {
                                            components: style_owner,
                                            styles: [style; MAX_COMPONENTS],
                                        };
                                        for force in [false, true] {
                                            let expected = reference_component_plan_metadata(
                                                &size,
                                                &styles,
                                                &quantizations,
                                                &roi,
                                                component,
                                                (tile, discard),
                                                force,
                                            );
                                            assert_eq!(
                                                crate::decode::prepare_component_plan_metadata(
                                                    &size,
                                                    &styles,
                                                    &quantizations,
                                                    &roi,
                                                    component,
                                                    (tile, discard),
                                                    force
                                                ),
                                                expected,
                                            );
                                            successes += usize::from(expected.is_ok());
                                            cases += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 115_200);
        assert!(
            successes > 100,
            "{successes} full component metadata owners checked"
        );
    }

    fn reference_resolution_count(
        style: CodingStyle,
        resolution: u8,
    ) -> Result<usize, DecodeError> {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodingStyle,
        };
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        if resolution > style.decomposition_levels {
            return Err(invalid);
        }
        let mut count = 1_u128;
        for current in 1..=u16::from(resolution) {
            let stage = u16::from(style.decomposition_levels) - current;
            count += match style
                .decomposition_directions
                .get(usize::from(stage))
                .ok_or(invalid)?
            {
                DecompositionDirection::None => 0,
                DecompositionDirection::Horizontal | DecompositionDirection::Vertical => 1,
                DecompositionDirection::TwoDimensional => 3,
            };
            if count > usize::MAX as u128 {
                return Err(arithmetic);
            }
        }
        Ok(usize::try_from(count).unwrap())
    }

    fn reference_resolution_plan_metadata(
        size: &ImageSize,
        style: CodingStyle,
        component: u16,
        selection: (u16, u8),
        origin: [u32; 2],
    ) -> Result<crate::decode::ResolutionPlanMetadata, DecodeError> {
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodeBlock,
        };
        let invalid_style = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodingStyle,
        };
        let resolution = selection.1;
        let geometry =
            reference_resolution_geometry(size, style, component, resolution, Some(selection.0))?;
        let first = if resolution == 0 {
            0
        } else {
            reference_resolution_count(style, resolution - 1)?
        };
        let last = reference_resolution_count(style, resolution)?;
        let count = usize::try_from(last as i128 - first as i128).map_err(|_| arithmetic)?;
        let precincts = if count == 0 {
            0
        } else {
            u32::try_from(geometry.precincts).map_err(|_| arithmetic)?
        };
        let p = *style
            .precinct_exponents
            .get(usize::from(resolution))
            .ok_or(invalid_style)?;
        if p.0 > 31 || p.1 > 31 {
            return Err(invalid);
        }
        let scale = (0..style.decomposition_levels - resolution)
            .try_fold((1, 1), |before, stage| {
                reference_resolution_scale_step(style, stage, before)
            })?;
        let x0 = u32::try_from(u64::from(origin[0]).div_ceil(u64::from(scale.0))).unwrap();
        let y0 = u32::try_from(u64::from(origin[1]).div_ceil(u64::from(scale.1))).unwrap();
        let pw = 1_u32 << p.0;
        let ph = 1_u32 << p.1;
        let first_precinct = [x0 / pw, y0 / ph];
        let direction = if resolution == 0 {
            DecompositionDirection::None
        } else {
            *style
                .decomposition_directions
                .get(usize::from(style.decomposition_levels - resolution))
                .ok_or(invalid_style)?
        };
        let split_x = matches!(
            direction,
            DecompositionDirection::Horizontal | DecompositionDirection::TwoDimensional
        );
        let split_y = matches!(
            direction,
            DecompositionDirection::Vertical | DecompositionDirection::TwoDimensional
        );
        let bx = if split_x {
            p.0.checked_sub(1).ok_or(invalid)?
        } else {
            p.0
        };
        let by = if split_y {
            p.1.checked_sub(1).ok_or(invalid)?
        } else {
            p.1
        };
        if bx > 31 || by > 31 {
            return Err(invalid);
        }
        let bw = 1_u32 << bx;
        let bh = 1_u32 << by;
        let cw = bw.min(u32::from(style.code_block_width));
        let ch = bh.min(u32::from(style.code_block_height));
        if pw == 0 || ph == 0 || cw == 0 || ch == 0 {
            return Err(invalid);
        }
        Ok(crate::decode::ResolutionPlanMetadata {
            geometry,
            first_subband: first,
            subband_count: count,
            precincts_in_resolution: precincts,
            first_precinct,
            band_size: [bw, bh],
            block_size: [cw, ch],
        })
    }

    #[test]
    fn actual_resolution_plan_metadata_preserves_complete_raw_geometry_counts_split_and_origin_fields()
     {
        let mut cases = 0;
        let mut successes = 0;
        let mut lazy_narrowing = 0;
        for large in [false, true] {
            let mut size = raw_resolution_size(if large { 65_536 } else { 33 }, 0, (1, 1), 0, 2);
            for info in &mut size.component_info {
                info.x_registration = 0;
                info.y_registration = 0;
            }
            if large {
                size.tile_width = 65_536;
                size.tile_height = 65_536;
                size.tiles_across = 1;
                size.tiles_down = 1;
            }
            for levels in [0, 1, 2, 31, 32, u8::MAX] {
                for resolution in [0, 1, 2, 31, u8::MAX] {
                    for pattern in 0..8 {
                        for p in [
                            (0, 0),
                            (1, 1),
                            (2, 3),
                            (31, 31),
                            (32, 0),
                            (u8::MAX, u8::MAX),
                        ] {
                            for (cw, ch) in [(1, 1), (0, 1), (4, 8), (u16::MAX, u16::MAX)] {
                                let mut style = raw_resolution_style(levels, pattern);
                                style.code_block_width = cw;
                                style.code_block_height = ch;
                                style.precinct_exponents = [p; 33];
                                for component in [0, 1, 64] {
                                    for tile in [0, u16::MAX] {
                                        for origin in [[0, 0], [1, 3], [u32::MAX, u32::MAX]] {
                                            let expected = reference_resolution_plan_metadata(
                                                &size,
                                                style,
                                                component,
                                                (tile, resolution),
                                                origin,
                                            );
                                            assert_eq!(
                                                crate::decode::prepare_resolution_plan_metadata(
                                                    &size,
                                                    style,
                                                    component,
                                                    (tile, resolution),
                                                    origin
                                                ),
                                                expected
                                            );
                                            if let Ok(value) = expected {
                                                successes += 1;
                                                lazy_narrowing += usize::from(
                                                    value.subband_count == 0
                                                        && value.geometry.precincts
                                                            > u64::from(u32::MAX),
                                                );
                                            }
                                            cases += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 207_360);
        assert!(successes > 100 && lazy_narrowing > 0);
    }

    fn raw_mct_identity_fixture(variant: usize) -> MultipleComponentTransform {
        let matrix = MctArray {
            index: 1,
            kind: MctArrayKind::Decorrelation,
            numeric_format: MctNumericFormat::Int16,
            fraction_bits: 0,
            values: vec![1, 0, 0, 1],
        };
        let offsets = MctArray {
            index: 2,
            kind: MctArrayKind::Offset,
            numeric_format: MctNumericFormat::Int32,
            fraction_bits: 0,
            values: vec![0, 0],
        };
        let collection = MctCollection {
            reversible: true,
            input_components: vec![0, 1],
            output_components: vec![0, 1],
            kind: MctCollectionKind::Decorrelation,
            matrix: Some(matrix),
            offsets: Some(offsets),
            wavelet: None,
        };
        let mut value = MultipleComponentTransform {
            stages: vec![MctStage {
                index: 1,
                collections: vec![collection],
            }],
        };
        match variant {
            0 => {}
            1 => value.stages.clear(),
            2 => value.stages.push(value.stages[0].clone()),
            3 => value.stages[0].index = u8::MAX,
            4 => value.stages[0].collections.clear(),
            5 => {
                let collection = value.stages[0].collections[0].clone();
                value.stages[0].collections.push(collection);
            }
            6 => value.stages[0].collections[0].reversible = false,
            7 => value.stages[0].collections[0].input_components.reverse(),
            8 => value.stages[0].collections[0]
                .output_components
                .push(u16::MAX),
            9 => value.stages[0].collections[0].kind = MctCollectionKind::Dependency,
            10 => value.stages[0].collections[0].matrix = None,
            11 => {
                value.stages[0].collections[0]
                    .matrix
                    .as_mut()
                    .unwrap()
                    .index = 2
            }
            12 => {
                value.stages[0].collections[0].matrix.as_mut().unwrap().kind =
                    MctArrayKind::Dependency
            }
            13 => {
                value.stages[0].collections[0]
                    .matrix
                    .as_mut()
                    .unwrap()
                    .numeric_format = MctNumericFormat::Float32
            }
            14 => {
                value.stages[0].collections[0]
                    .matrix
                    .as_mut()
                    .unwrap()
                    .fraction_bits = u8::MAX
            }
            15 => {
                value.stages[0].collections[0]
                    .matrix
                    .as_mut()
                    .unwrap()
                    .values[0] = i64::MIN
            }
            16 => {
                value.stages[0].collections[0]
                    .matrix
                    .as_mut()
                    .unwrap()
                    .values
                    .pop();
            }
            17 => value.stages[0].collections[0].offsets = None,
            18 => {
                value.stages[0].collections[0]
                    .offsets
                    .as_mut()
                    .unwrap()
                    .values[0] = i64::MAX
            }
            19 => {
                value.stages[0].collections[0].wavelet = Some(MctWavelet {
                    transform: MctWaveletType::Reversible5x3,
                    levels: 0,
                    component_offset: 0,
                })
            }
            20 => {
                value.stages[0].collections[0].wavelet = Some(MctWavelet {
                    transform: MctWaveletType::Irreversible9x7,
                    levels: 0,
                    component_offset: 0,
                })
            }
            21 => {
                value.stages[0].collections[0].wavelet = Some(MctWavelet {
                    transform: MctWaveletType::Reversible5x3,
                    levels: u8::MAX,
                    component_offset: i32::MIN,
                })
            }
            22 => {
                value.stages[0].collections[0]
                    .offsets
                    .as_mut()
                    .unwrap()
                    .numeric_format = MctNumericFormat::Float64
            }
            _ => value.stages[0].collections[0].kind = MctCollectionKind::Wavelet,
        }
        value
    }

    #[test]
    fn actual_mct_identity_compares_every_raw_graph_array_stage_and_collection_field() {
        let mut cases = 0;
        for left in 0..24 {
            for right in 0..24 {
                let left = raw_mct_identity_fixture(left);
                let right = raw_mct_identity_fixture(right);
                assert_eq!(left.equals(&right), left == right);
                assert_eq!(
                    crate::decode::optional_multiple_component_transforms_equal(
                        Some(&left),
                        Some(&right)
                    ),
                    left == right
                );
                for (a, b) in left.stages.iter().zip(&right.stages) {
                    assert_eq!(a.equals(b), a == b);
                    for (a, b) in a.collections.iter().zip(&b.collections) {
                        assert_eq!(a.equals(b), a == b);
                        for (a, b) in [
                            a.matrix.as_ref().zip(b.matrix.as_ref()),
                            a.offsets.as_ref().zip(b.offsets.as_ref()),
                        ]
                        .into_iter()
                        .flatten()
                        {
                            assert_eq!(a.equals(b), a == b);
                        }
                    }
                }
                cases += 1;
            }
        }
        assert_eq!(cases, 576);
        let empty = MultipleComponentTransform { stages: Vec::new() };
        assert!(!crate::decode::optional_multiple_component_transforms_equal(Some(&empty), None));
        assert!(crate::decode::optional_multiple_component_transforms_equal(
            None, None
        ));
    }

    #[test]
    fn actual_maximum_isotropic_reduction_preserves_full_u8_levels_and_physical_prefix_laziness() {
        let mut cases = 0;
        for levels in 0..=u8::MAX {
            for pattern in 0..8 {
                for stop in 0..=32 {
                    let mut style = raw_resolution_style(levels, pattern);
                    for direction in style.decomposition_directions.iter_mut().take(stop) {
                        *direction = DecompositionDirection::TwoDimensional;
                    }
                    if stop < 32 {
                        style.decomposition_directions[stop] = DecompositionDirection::None;
                    }
                    let expected = style
                        .decomposition_directions
                        .iter()
                        .take(usize::from(levels).min(32))
                        .take_while(|direction| {
                            matches!(direction, DecompositionDirection::TwoDimensional)
                        })
                        .count();
                    assert_eq!(usize::from(style.maximum_isotropic_reduction()), expected);
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 67_584);
    }

    fn reference_tile_component_resolutions(
        styles: &ComponentCodingStyles,
        main: CodingStyle,
        components: u16,
        discard: Option<u8>,
    ) -> Result<Vec<u8>, DecodeError> {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodingStyle,
        };
        let reduction = DecodeError::InvalidSelection {
            reason: crate::SelectionError::ReductionTooLarge,
        };
        let count = if discard.is_some() {
            components
        } else if main.multiple_component_transform {
            components.min(3)
        } else {
            0
        };
        let mut output = Vec::new();
        for component in 0..count {
            if component >= styles.components {
                return Err(invalid);
            }
            let style = styles.styles.get(usize::from(component)).ok_or(invalid)?;
            if let Some(discard) = discard {
                let maximum = style
                    .decomposition_directions
                    .iter()
                    .take(usize::from(style.decomposition_levels).min(32))
                    .take_while(|direction| {
                        matches!(direction, DecompositionDirection::TwoDimensional)
                    })
                    .count();
                if usize::from(discard) > maximum {
                    return Err(reduction);
                }
            }
            let agreement = matches!(
                (style.transform, main.transform),
                (
                    WaveletTransform::Reversible5x3,
                    WaveletTransform::Reversible5x3
                ) | (
                    WaveletTransform::Irreversible9x7,
                    WaveletTransform::Irreversible9x7
                )
            );
            if main.multiple_component_transform && component < 3 && !agreement {
                return Err(invalid);
            }
            if let Some(discard) = discard {
                if style.decomposition_levels == u8::MAX {
                    return Err(reduction);
                }
                output.push(
                    u8::try_from(i16::from(style.decomposition_levels) + 1 - i16::from(discard))
                        .map_err(|_| reduction)?,
                );
            }
        }
        Ok(output)
    }

    #[test]
    fn actual_tile_style_resolution_preparation_preserves_raw_modes_owners_and_arbitrary_agreement_policy()
     {
        let mut cases = 0;
        let modes = [
            None,
            Some(0),
            Some(1),
            Some(2),
            Some(31),
            Some(32),
            Some(u8::MAX),
        ];
        for levels in [0, 1, 2, 31, 32, u8::MAX] {
            for pattern in 0..8 {
                for logical in [0, 1, 2, 3, 64, 65, u16::MAX] {
                    for components in [0, 1, 2, 3, 64, u16::MAX] {
                        for flag in [false, true] {
                            for main_transform in [
                                WaveletTransform::Reversible5x3,
                                WaveletTransform::Irreversible9x7,
                                WaveletTransform::Arbitrary(3),
                            ] {
                                let mut main = raw_resolution_style(levels, pattern);
                                main.multiple_component_transform = flag;
                                main.transform = main_transform;
                                for transform in [
                                    main_transform,
                                    WaveletTransform::Reversible5x3,
                                    WaveletTransform::Arbitrary(3),
                                ] {
                                    let mut style = raw_resolution_style(levels, pattern);
                                    style.transform = transform;
                                    let styles = ComponentCodingStyles {
                                        components: logical,
                                        styles: [style; MAX_COMPONENTS],
                                    };
                                    for mode in modes {
                                        assert_eq!(
                                            crate::decode::prepare_tile_component_resolutions(
                                                &styles, main, components, mode
                                            ),
                                            reference_tile_component_resolutions(
                                                &styles, main, components, mode
                                            )
                                        );
                                        cases += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 254_016);
    }

    fn reference_tile_decode_metadata(
        parts: &[TilePartPayload<'_>],
        components: u16,
        discard: Option<u8>,
    ) -> Result<((bool, bool), Vec<u8>), DecodeError> {
        let first = parts.first().ok_or(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidTilePart,
        })?;
        if parts
            .iter()
            .any(|part| part.multiple_component_transform != first.multiple_component_transform)
        {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidTransform,
            });
        }
        let irreversible = match first.coding_style.transform {
            WaveletTransform::Irreversible9x7 => true,
            WaveletTransform::Reversible5x3 => false,
            WaveletTransform::Arbitrary(index) => {
                !first
                    .part2_extensions
                    .arbitrary_kernels
                    .iter()
                    .find(|kernel| kernel.index == index)
                    .ok_or(DecodeError::MalformedInput {
                        offset: 0,
                        kind: MalformedKind::MissingRequiredStructure,
                    })?
                    .reversible
            }
        };
        Ok((
            (
                first.coding_style.multiple_component_transform,
                irreversible,
            ),
            reference_tile_component_resolutions(
                &first.component_coding_styles,
                first.coding_style,
                components,
                discard,
            )?,
        ))
    }

    #[test]
    fn actual_tile_metadata_derives_first_payload_mct_kernel_and_ordered_style_results() {
        let bytes = codestream(&[0], false);
        let layout = parse_layout(&bytes, &ResourceLimits::HARDENED).unwrap();
        let template = &layout.tile_parts[0];
        let modes = [
            None,
            Some(0),
            Some(1),
            Some(2),
            Some(31),
            Some(32),
            Some(u8::MAX),
        ];
        let mut cases = 0;
        let mut successes = 0;
        for shape in 0..6 {
            for components in [0, 1, 3, u16::MAX] {
                for mode in modes {
                    for transform in [
                        WaveletTransform::Reversible5x3,
                        WaveletTransform::Irreversible9x7,
                        WaveletTransform::Arbitrary(3),
                        WaveletTransform::Arbitrary(4),
                    ] {
                        for flag in [false, true] {
                            for logical in [0, 1, 64, u16::MAX] {
                                for pattern in [0, 3, 7] {
                                    for registry in 0..3 {
                                        let mut first = template.clone();
                                        first.coding_style.transform = transform;
                                        first.coding_style.multiple_component_transform = flag;
                                        let mut style = raw_resolution_style(
                                            if pattern == 7 { u8::MAX } else { 2 },
                                            pattern,
                                        );
                                        style.transform = transform;
                                        first.component_coding_styles = ComponentCodingStyles {
                                            components: logical,
                                            styles: [style; MAX_COMPONENTS],
                                        };
                                        let kernel = |reversible| ArbitraryKernel {
                                            index: 3,
                                            reversible,
                                            descriptor: u16::MAX,
                                            coefficient_type: u8::MAX,
                                            whole_sample_filter: false,
                                            initial_odd: true,
                                            whole_sample_extension: false,
                                            scaling: None,
                                            steps: Vec::new(),
                                        };
                                        first.part2_extensions.arbitrary_kernels = match registry {
                                            0 => Vec::new(),
                                            1 => vec![kernel(true)],
                                            _ => vec![kernel(false), kernel(true)],
                                        };
                                        let mut second = first.clone();
                                        match shape {
                                            3 => {
                                                first.multiple_component_transform =
                                                    Some(MultipleComponentTransform {
                                                        stages: Vec::new(),
                                                    })
                                            }
                                            4 => {
                                                first.multiple_component_transform =
                                                    Some(raw_mct_identity_fixture(0));
                                                second.multiple_component_transform =
                                                    Some(raw_mct_identity_fixture(3));
                                            }
                                            5 => {
                                                first.multiple_component_transform =
                                                    Some(raw_mct_identity_fixture(0));
                                                second.multiple_component_transform =
                                                    Some(raw_mct_identity_fixture(0));
                                            }
                                            _ => {}
                                        }
                                        let parts = match shape {
                                            0 => Vec::new(),
                                            1 => vec![first],
                                            _ => vec![first, second],
                                        };
                                        let expected = reference_tile_decode_metadata(
                                            &parts, components, mode,
                                        );
                                        let actual = crate::decode::prepare_tile_decode_metadata(
                                            &parts, components, mode,
                                        )
                                        .map(|value| {
                                            assert!(core::ptr::eq(value.first, &parts[0]));
                                            (value.standard, value.resolution_counts)
                                        });
                                        successes += usize::from(expected.is_ok());
                                        assert_eq!(actual, expected);
                                        cases += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 48_384);
        assert!(successes > 100);
    }

    fn reference_capability_word(caps: &ExtendedCapabilities, part: u8) -> Option<u16> {
        if part == 0 || part > 32 {
            return None;
        }
        if caps.p_cap & (1_u32 << (32 - u32::from(part))) == 0 {
            return None;
        }
        let index = (1..part)
            .filter(|prior| caps.p_cap & (1_u32 << (32 - u32::from(*prior))) != 0)
            .count();
        caps.c_capabilities.get(index).copied()
    }

    #[test]
    fn actual_capability_lookup_preserves_all_u8_parts_raw_masks_and_ragged_physical_words() {
        let masks = [
            0,
            1,
            0x8000_0000,
            PART_15_CAPABILITY_FLAG,
            PART_15_CAPABILITY_FLAG | 0x8000_0000,
            u32::MAX,
            0xaaaa_5555,
            0x5555_aaaa,
            u32::MAX ^ PART_15_CAPABILITY_FLAG,
        ];
        let mut cases = 0;
        for mask in masks {
            for physical in [0, 1, 2, 14, 15, 31, 32, 33] {
                let caps = ExtendedCapabilities {
                    p_cap: mask,
                    c_capabilities: (0..physical)
                        .map(|index| u16::try_from(index).unwrap().wrapping_mul(2_177))
                        .collect(),
                };
                for part in 0..=u8::MAX {
                    assert_eq!(
                        caps.capability_for_part(part),
                        reference_capability_word(&caps, part)
                    );
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 18_432);
    }

    #[test]
    fn actual_ht_profile_policy_preserves_every_raw_ccap_word_physical_lookup_and_unused_owner_field()
     {
        let mut cases = 0;
        for word in 0..=u16::MAX {
            for mask in [
                PART_15_CAPABILITY_FLAG,
                PART_15_CAPABILITY_FLAG | 0x8000_0000,
                u32::MAX,
                0,
            ] {
                let target = (1..15_u8)
                    .filter(|part| mask & (1_u32 << (32 - u32::from(*part))) != 0)
                    .count();
                for physical in [target, target + 1, 34] {
                    let mut values = vec![u16::MAX; physical];
                    if let Some(value) = values.get_mut(target) {
                        *value = word;
                    }
                    let caps = ExtendedCapabilities {
                        p_cap: mask,
                        c_capabilities: values,
                    };
                    let expected_word = reference_capability_word(&caps, 15);
                    let expected_mode = expected_word.and_then(|word| match word >> 14 {
                        0 => Some(Part15BlockMode::HtOnly),
                        2 => Some(Part15BlockMode::HtDeclared),
                        3 => Some(Part15BlockMode::Mixed),
                        _ => None,
                    });
                    let expected_multiple = expected_word.is_some_and(|word| word & 0x2000 != 0);
                    let expected_bound = expected_word.map(|word| {
                        let value = u8::try_from(word & 31).unwrap();
                        match value {
                            0 => 8,
                            1..=19 => value + 8,
                            20..=30 => (value - 19) * 4 + 27,
                            _ => 74,
                        }
                    });
                    assert_eq!(caps.part15_value(), expected_word);
                    assert_eq!(caps.part15_block_mode(), expected_mode);
                    assert_eq!(caps.part15_allows_multiple_ht_sets(), expected_multiple);
                    assert_eq!(caps.part15_magnitude_bound(), expected_bound);
                    for unrelated in [false, true] {
                        let profile = CodestreamProfile {
                            r_siz: if unrelated { u16::MAX } else { 0 },
                            block_coding: if unrelated {
                                CodestreamBlockCoding::Mixed
                            } else {
                                CodestreamBlockCoding::Legacy
                            },
                            extended_capabilities: Some(caps.clone()),
                            profile: unrelated.then(|| ProfileNumber {
                                words: vec![u16::MAX, 0, 17],
                            }),
                            corresponding_profile: unrelated.then(|| ProfileNumber {
                                words: vec![31, u16::MAX],
                            }),
                        };
                        assert_eq!(profile.part15_allows_multiple_ht_sets(), expected_multiple);
                        assert_eq!(
                            profile.part15_magnitude_bound(),
                            expected_bound.unwrap_or(74)
                        );
                        assert_eq!(
                            profile.part15_requires_all_ht_blocks(),
                            matches!(expected_mode, Some(Part15BlockMode::HtOnly))
                        );
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 1_572_864);
        let absent = CodestreamProfile {
            r_siz: u16::MAX,
            block_coding: CodestreamBlockCoding::HighThroughput,
            extended_capabilities: None,
            profile: Some(ProfileNumber {
                words: vec![u16::MAX],
            }),
            corresponding_profile: None,
        };
        assert!(!absent.part15_allows_multiple_ht_sets());
        assert_eq!(absent.part15_magnitude_bound(), 74);
        assert!(!absent.part15_requires_all_ht_blocks());
    }

    fn raw_owned_tile_part_fixture(variant: usize) -> TilePartPayload<'static> {
        let coefficient = KernelCoefficient {
            bytes: [u8::MAX; 16],
            width: u8::MAX,
        };
        let kernel = ArbitraryKernel {
            index: u8::MAX,
            reversible: variant.is_multiple_of(2),
            descriptor: u16::MAX,
            coefficient_type: u8::MAX,
            whole_sample_filter: true,
            initial_odd: false,
            whole_sample_extension: true,
            scaling: Some(coefficient),
            steps: vec![KernelLiftingStep {
                offset: i8::MIN,
                exponent: u8::MAX,
                residue: Some(coefficient),
                coefficients: vec![coefficient; variant % 4],
            }],
        };
        let nonlinear = NonlinearPointTransform {
            component: Some(u16::MAX),
            precision: 0,
            signed: true,
            transform_type: u8::MAX,
            parameters: vec![0, u8::MAX, 17],
            d_min: Some(u32::MAX),
            d_max: Some(0),
            point_bits: Some(u8::MAX),
            points: vec![0, u32::MAX, 31],
            gamma_parameters: Some([u32::MAX, 0, 1, 2, 3]),
        };
        let style = raw_resolution_style(u8::MAX, variant % 8);
        let quantization = raw_subband_quantization(QuantizationStyle::ScalarDerived, u16::MAX, 3);
        TilePartPayload {
            tile_index: u16::try_from(variant).unwrap(),
            tile_part_index: u8::try_from(variant).unwrap(),
            tile_part_count: 0,
            coding_style: style,
            quantization,
            component_coding_styles: ComponentCodingStyles {
                components: u16::MAX,
                styles: [style; MAX_COMPONENTS],
            },
            component_quantizations: ComponentQuantizations {
                components: u16::MAX,
                quantizations: vec![quantization; variant % 4],
            },
            roi_shifts: RoiShifts {
                shifts: [u8::MAX; MAX_COMPONENTS],
            },
            progression_changes: ProgressionChanges {
                records: vec![ProgressionChange {
                    resolution_start: u8::MAX,
                    component_start: u16::MAX,
                    layer_end: 0,
                    resolution_end: 0,
                    component_end: 0,
                    progression_order: ProgressionOrder::Cprl,
                }],
            },
            multiple_component_transform: Some(raw_mct_identity_fixture(variant % 24)),
            part2_extensions: Part2Extensions {
                decomposition_structures: vec![DecompositionStructure {
                    index: u8::MAX,
                    directions: vec![
                        DecompositionDirection::None,
                        DecompositionDirection::Horizontal,
                        DecompositionDirection::TwoDimensional,
                    ],
                }],
                arbitrary_decompositions: vec![ArbitraryDecomposition {
                    index: 0,
                    sublevel_depths: vec![0, u8::MAX],
                    subband_splits: vec![
                        DecompositionDirection::Vertical,
                        DecompositionDirection::None,
                    ],
                }],
                nonlinear_transforms: vec![nonlinear],
                component_bit_depths: vec![
                    ComponentBitDepth {
                        precision: 0,
                        signed: true,
                    },
                    ComponentBitDepth {
                        precision: u8::MAX,
                        signed: false,
                    },
                ],
                arbitrary_kernels: vec![kernel],
                component_registrations: vec![ComponentRegistration {
                    horizontal: u16::MAX,
                    vertical: 0,
                }],
                tile_part_lengths: vec![
                    TilePartLength {
                        tile_index: None,
                        length: u32::MAX,
                    },
                    TilePartLength {
                        tile_index: Some(u16::MAX),
                        length: 0,
                    },
                ],
                metadata_bytes: u64::MAX,
            },
            packet_lengths: PacketLengths {
                source: PacketLengthSource::Plm(&[0, u8::MAX, 0x81]),
                packet_count: u64::MAX,
                encoded_bytes: 0,
            },
            packed_packet_headers: PackedPacketHeaders {
                source: PackedPacketHeaderSource::MarkerSegments {
                    header: &[0xff, 0x61, 0, 3, 0],
                    marker: 0xff61,
                    logical_start: usize::MAX,
                },
                length: usize::MAX,
                fragments: u16::MAX,
            },
            has_functional_header_marker: true,
            has_roi_marker: false,
            data: &[0, 0xff, 0x81, 0x7f],
        }
    }

    #[test]
    fn actual_owned_tile_part_clone_preserves_every_raw_field_and_independent_nested_buffers() {
        for variant in 0..24 {
            let source = raw_owned_tile_part_fixture(variant);
            let before = format!("{source:?}");
            let mut copied = source.clone();
            assert_eq!(copied, source);
            assert!(core::ptr::eq(copied.data, source.data));
            copied.component_quantizations.quantizations.clear();
            copied.progression_changes.records.clear();
            copied.part2_extensions.decomposition_structures[0]
                .directions
                .clear();
            copied.part2_extensions.arbitrary_decompositions[0]
                .sublevel_depths
                .clear();
            copied.part2_extensions.arbitrary_decompositions[0]
                .subband_splits
                .clear();
            copied.part2_extensions.nonlinear_transforms[0]
                .parameters
                .clear();
            copied.part2_extensions.nonlinear_transforms[0]
                .points
                .clear();
            copied.part2_extensions.arbitrary_kernels[0].steps[0]
                .coefficients
                .clear();
            copied.part2_extensions.arbitrary_kernels[0].steps.clear();
            copied.part2_extensions.component_bit_depths.clear();
            copied.part2_extensions.component_registrations.clear();
            copied.part2_extensions.tile_part_lengths.clear();
            if let Some(transform) = &mut copied.multiple_component_transform {
                for stage in &mut transform.stages {
                    for collection in &mut stage.collections {
                        collection.input_components.clear();
                        collection.output_components.clear();
                        if let Some(array) = &mut collection.matrix {
                            array.values.clear();
                        }
                        if let Some(array) = &mut collection.offsets {
                            array.values.clear();
                        }
                    }
                    stage.collections.clear();
                }
                transform.stages.clear();
            }
            assert_eq!(format!("{source:?}"), before);
            assert_ne!(copied, source);
        }
    }

    #[test]
    fn actual_ordered_tile_collection_copies_raw_non_normalized_fields_and_retains_duplicate_order()
    {
        let mut cases = 0;
        for length in 0..=64 {
            for pattern in 0..4 {
                let source = (0..length)
                    .map(|index| {
                        let mut value = raw_owned_tile_part_fixture(index % 24);
                        value.tile_index = match pattern {
                            0 => 0,
                            1 => u16::try_from(index % 2).unwrap(),
                            2 => u16::MAX,
                            _ => u16::try_from(index % 3).unwrap(),
                        };
                        value.tile_part_index = u8::try_from(index).unwrap();
                        value
                    })
                    .collect::<Vec<_>>();
                for tile in [0, 1, 2, u16::MAX] {
                    let actual = crate::decode::collect_tile_parts(&source, tile).unwrap();
                    let expected = source
                        .iter()
                        .filter(|part| part.tile_index == tile)
                        .collect::<Vec<_>>();
                    assert_eq!(actual.len(), expected.len());
                    for (copied, original) in actual.iter().zip(expected) {
                        assert_eq!(copied, original);
                        assert!(core::ptr::eq(copied.data, original.data));
                    }
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 1_040);
    }

    fn reference_truncated_subband_count(style: CodingStyle) -> u16 {
        let total = style
            .decomposition_directions
            .iter()
            .take(usize::from(style.decomposition_levels.min(32)))
            .fold(1_u32, |count, direction| {
                count
                    + match direction {
                        DecompositionDirection::None => 0,
                        DecompositionDirection::Horizontal | DecompositionDirection::Vertical => 1,
                        DecompositionDirection::TwoDimensional => 3,
                    }
            });
        u16::try_from(total).unwrap()
    }

    fn reference_subband_resolution(
        style: CodingStyle,
        bounds: [u32; 4],
        depth: [u8; 2],
        quantization: &Quantization,
        resolution: u8,
        mut before: Vec<crate::decode::SubbandGeometry>,
    ) -> Result<Vec<crate::decode::SubbandGeometry>, DecodeError> {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodingStyle,
        };
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        if resolution > style.decomposition_levels {
            return Err(invalid);
        }
        let scale = (0..style.decomposition_levels - resolution)
            .try_fold((1, 1), |previous, stage| {
                reference_resolution_scale_step(style, stage, previous)
            })?;
        let ceil = |value: u32, divisor: u32| {
            u32::try_from(u64::from(value).div_ceil(u64::from(divisor))).unwrap()
        };
        let [x0, y0, x1, y1] = [
            ceil(bounds[0], scale.0),
            ceil(bounds[1], scale.1),
            ceil(bounds[2], scale.0),
            ceil(bounds[3], scale.1),
        ];
        let dimensions = [
            u32::try_from(i64::from(x1) - i64::from(x0)).map_err(|_| arithmetic)?,
            u32::try_from(i64::from(y1) - i64::from(y0)).map_err(|_| arithmetic)?,
        ];
        if resolution == 0 {
            before.push(reference_subband_admission(
                0,
                SubbandOrientation::Ll,
                [x0, y0, x1, y1],
                dimensions,
                [true, true],
                depth,
                quantization,
                0,
            )?);
            return Ok(before);
        }
        let low = [ceil(x0, 2), ceil(y0, 2), ceil(x1, 2), ceil(y1, 2)];
        let direction = style
            .decomposition_directions
            .get(usize::from(style.decomposition_levels - resolution))
            .ok_or(invalid)?;
        let orientations: &[SubbandOrientation] = match direction {
            DecompositionDirection::None => &[],
            DecompositionDirection::Horizontal => &[SubbandOrientation::Hl],
            DecompositionDirection::Vertical => &[SubbandOrientation::Lh],
            DecompositionDirection::TwoDimensional => &[
                SubbandOrientation::Hl,
                SubbandOrientation::Lh,
                SubbandOrientation::Hh,
            ],
        };
        let both = *direction == DecompositionDirection::TwoDimensional;
        let phase = [x0 & 1 == 0, y0 & 1 == 0];
        for orientation in orientations {
            let index = u16::try_from(before.len()).map_err(|_| arithmetic)?;
            let band = match orientation {
                SubbandOrientation::Hl => [
                    x0 / 2,
                    if both { low[1] } else { y0 },
                    x1 / 2,
                    if both { low[3] } else { y1 },
                ],
                SubbandOrientation::Lh => [
                    if both { low[0] } else { x0 },
                    y0 / 2,
                    if both { low[2] } else { x1 },
                    y1 / 2,
                ],
                SubbandOrientation::Hh => [x0 / 2, y0 / 2, x1 / 2, y1 / 2],
                SubbandOrientation::Ll => unreachable!(),
            };
            before.push(reference_subband_admission(
                resolution,
                *orientation,
                band,
                dimensions,
                phase,
                depth,
                quantization,
                index,
            )?);
        }
        Ok(before)
    }

    fn reference_component_subbands(
        size: &ImageSize,
        style: CodingStyle,
        quantization: &Quantization,
        component: u16,
        tile: Option<u16>,
        roi: u8,
    ) -> Result<Vec<crate::decode::SubbandGeometry>, DecodeError> {
        let invalid_geometry = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidImageGeometry,
        };
        if component >= size.components {
            return Err(invalid_geometry);
        }
        let info = size
            .component_info
            .get(usize::from(component))
            .ok_or(invalid_geometry)?;
        let bounds = reference_component_coordinate_bounds(size, component, tile)?;
        let expected = reference_truncated_subband_count(style);
        let mut bands = Vec::new();
        for resolution in 0_u16..=u16::from(style.decomposition_levels) {
            bands = reference_subband_resolution(
                style,
                bounds,
                [info.precision, roi],
                quantization,
                u8::try_from(resolution).unwrap(),
                bands,
            )?;
            if resolution == u16::from(u8::MAX) {
                return Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::ArithmeticOverflow,
                });
            }
        }
        if bands.len() != usize::from(expected) {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodeBlock,
            });
        }
        Ok(bands)
    }

    #[test]
    fn complete_subband_vectors_match_raw_coordinates_direction_order_and_every_field() {
        let mut cases = 0;
        let mut successes = 0;
        for levels in 0..=u8::MAX {
            for pattern in 0..8 {
                let style = raw_resolution_style(levels, pattern);
                assert_eq!(
                    style.subband_count(),
                    reference_truncated_subband_count(style)
                );
            }
        }
        for extent in [0, 1, 5, 33] {
            for origin in [0, 1, u32::MAX] {
                for sampling in [(0, 1), (1, 0), (1, 1), (2, 3)] {
                    for precision in [0, 1, 8, 38, u8::MAX] {
                        let mut size = raw_resolution_size(extent, origin, sampling, u16::MAX, 1);
                        for info in &mut size.component_info {
                            info.precision = precision;
                        }
                        for quantization_style in [
                            QuantizationStyle::None,
                            QuantizationStyle::ScalarDerived,
                            QuantizationStyle::ScalarExpounded,
                        ] {
                            for count in [0, 1, 97, u16::MAX] {
                                let quantization =
                                    raw_subband_quantization(quantization_style, count, 4);
                                for (levels, pattern) in
                                    [(0, 0), (1, 1), (2, 2), (2, 3), (32, 0), (u8::MAX, 0)]
                                {
                                    let style = raw_resolution_style(levels, pattern);
                                    for tile in [None, Some(0), Some(u16::MAX)] {
                                        for roi in [0, 1, 73, u8::MAX] {
                                            let expected = reference_component_subbands(
                                                &size,
                                                style,
                                                &quantization,
                                                0,
                                                tile,
                                                roi,
                                            );
                                            assert_eq!(
                                                crate::decode::derive_component_subbands_for(
                                                    &size,
                                                    style,
                                                    &quantization,
                                                    0,
                                                    tile,
                                                    roi
                                                ),
                                                expected,
                                            );
                                            if tile.is_none() && roi == 0 {
                                                assert_eq!(
                                                    crate::decode::derive_component_subbands(
                                                        &size,
                                                        style,
                                                        &quantization,
                                                        0
                                                    ),
                                                    expected,
                                                );
                                            }
                                            successes += usize::from(expected.is_ok());
                                            cases += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 207_360);
        assert!(
            successes > 1000,
            "{successes} complete subband vectors checked"
        );
    }

    #[test]
    fn subband_vectors_preserve_component_admission_and_raw_prefix_index_boundaries() {
        let quantization = raw_subband_quantization(QuantizationStyle::ScalarDerived, 1, 4);
        let mut cases = 0;
        for components in [0, 1, 64, 65, u16::MAX] {
            for component in [0, 1, 63, 64, u16::MAX] {
                for tile in [None, Some(0), Some(u16::MAX)] {
                    for extent in [1, u32::MAX] {
                        for (levels, pattern) in [(0, 0), (u8::MAX, 0)] {
                            for sampling in [(0, 1), (1, 0), (1, 1)] {
                                let size =
                                    raw_resolution_size(extent, 1, sampling, u16::MAX, components);
                                let style = raw_resolution_style(levels, pattern);
                                assert_eq!(
                                    crate::decode::derive_component_subbands_for(
                                        &size,
                                        style,
                                        &quantization,
                                        component,
                                        tile,
                                        0
                                    ),
                                    reference_component_subbands(
                                        &size,
                                        style,
                                        &quantization,
                                        component,
                                        tile,
                                        0
                                    ),
                                );
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 900);
        let dummy = reference_subband_admission(
            0,
            SubbandOrientation::Ll,
            [0, 0, 1, 1],
            [1, 1],
            [true, false],
            [8, 0],
            &quantization,
            u16::MAX,
        )
        .unwrap();
        for length in [0, 1, usize::from(u16::MAX), usize::from(u16::MAX) + 1] {
            let before = vec![dummy; length];
            for pattern in 0..4 {
                let style = raw_resolution_style(1, pattern);
                for resolution in [0, 1] {
                    assert_eq!(
                        crate::decode::derive_subband_resolution(
                            style,
                            [1, 3, 5, 9],
                            [8, 0],
                            &quantization,
                            resolution,
                            before.clone()
                        ),
                        reference_subband_resolution(
                            style,
                            [1, 3, 5, 9],
                            [8, 0],
                            &quantization,
                            resolution,
                            before.clone()
                        ),
                    );
                }
            }
        }
    }

    fn raw_subband_quantization(
        style: QuantizationStyle,
        count: u16,
        pattern: usize,
    ) -> Quantization {
        Quantization {
            style,
            guard_bits: [0, 1, 7, u8::MAX, 2, 0, 73, 74][pattern],
            step_count: count,
            steps: core::array::from_fn(|index| QuantizationStep {
                exponent: match pattern {
                    0 => 0,
                    1 => u8::try_from(index % 74).unwrap(),
                    2 => u8::MAX,
                    3 => 31,
                    4 => 7,
                    5 => 1,
                    6 => 73,
                    _ => 74,
                },
                mantissa: u16::try_from(index)
                    .unwrap()
                    .wrapping_mul(997)
                    .wrapping_add(u16::MAX),
            }),
        }
    }

    fn reference_quantization_lookup(
        value: &Quantization,
        index: u16,
    ) -> Result<QuantizationStep, DecodeError> {
        if index >= value.step_count || usize::from(index) >= value.steps.len() {
            Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidQuantization,
            })
        } else {
            Ok(value.steps[usize::from(index)])
        }
    }

    fn reference_quantization_at_index(
        value: &Quantization,
        resolution: u8,
        index: u16,
    ) -> Result<QuantizationStep, DecodeError> {
        if value.style != QuantizationStyle::ScalarDerived {
            return reference_quantization_lookup(value, index);
        }
        let base = reference_quantization_lookup(value, 0)?;
        let reduction = if resolution == 0 {
            0
        } else {
            u16::from(resolution) - 1
        };
        let exponent = (i32::from(base.exponent) - i32::from(reduction)).max(0);
        Ok(QuantizationStep {
            exponent: u8::try_from(exponent).unwrap(),
            mantissa: base.mantissa,
        })
    }

    #[test]
    fn quantization_lookup_and_public_subband_index_preserve_raw_words_and_lazy_policy() {
        use SubbandOrientation::{Hh, Hl, Lh, Ll};
        for count in [0, 1, 2, 97, 98, u16::MAX] {
            let value = raw_subband_quantization(QuantizationStyle::ScalarDerived, count, 7);
            for index in 0..=u16::MAX {
                assert_eq!(
                    value.step(index),
                    reference_quantization_lookup(&value, index)
                );
            }
        }
        let mut cases = 0;
        for style in [
            QuantizationStyle::None,
            QuantizationStyle::ScalarDerived,
            QuantizationStyle::ScalarExpounded,
        ] {
            for count in [0, 1, 2, 97, 98, u16::MAX] {
                for pattern in 0..8 {
                    let value = raw_subband_quantization(style, count, pattern);
                    assert_eq!(value.style(), style);
                    assert_eq!(value.guard_bits(), value.guard_bits);
                    for resolution in 0..=u8::MAX {
                        for orientation in [Ll, Hl, Lh, Hh] {
                            let offset = match orientation {
                                Ll | Hl => 0,
                                Lh => 1,
                                Hh => 2,
                            };
                            let index = if resolution == 0 && orientation == Ll {
                                Some(0)
                            } else if resolution != 0 && orientation != Ll {
                                Some(u16::from(resolution) * 3 - 2 + offset)
                            } else {
                                None
                            };
                            let expected = if let Some(index) = index {
                                reference_quantization_at_index(&value, resolution, index)
                                    .map(|step| (index, step))
                            } else {
                                Err(DecodeError::MalformedInput {
                                    offset: 0,
                                    kind: MalformedKind::InvalidCodeBlock,
                                })
                            };
                            assert_eq!(
                                crate::decode::quantization_step_for_subband(
                                    &value,
                                    resolution,
                                    orientation
                                ),
                                expected,
                            );
                            let raw_index = u16::from(resolution).wrapping_mul(997);
                            assert_eq!(
                                crate::decode::quantization_step_at_index(
                                    &value, resolution, raw_index
                                ),
                                reference_quantization_at_index(&value, resolution, raw_index),
                            );
                            cases += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 147_456);
    }

    #[allow(clippy::too_many_arguments)]
    fn reference_subband_admission(
        resolution: u8,
        orientation: SubbandOrientation,
        bounds: [u32; 4],
        dimensions: [u32; 2],
        phase: [bool; 2],
        depth: [u8; 2],
        quantization: &Quantization,
        index: u16,
    ) -> Result<crate::decode::SubbandGeometry, DecodeError> {
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodeBlock,
        };
        let gain = match orientation {
            SubbandOrientation::Ll => 0,
            SubbandOrientation::Hl | SubbandOrientation::Lh => 1,
            SubbandOrientation::Hh => 2,
        };
        let dynamic_range = u8::try_from(u16::from(depth[0]) + gain).map_err(|_| arithmetic)?;
        if dynamic_range == 0 {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidQuantization,
            });
        }
        let step = reference_quantization_at_index(quantization, resolution, index)?;
        let raw = u32::from(quantization.guard_bits) + u32::from(step.exponent);
        if raw == 0 {
            return Err(invalid);
        }
        let bitplanes = raw - 1 + u32::from(depth[1]);
        if bitplanes == 0 || bitplanes > u32::from(crate::tier1::MAX_TIER1_BITPLANES) {
            return Err(invalid);
        }
        let width =
            u32::try_from(i64::from(bounds[2]) - i64::from(bounds[0])).map_err(|_| arithmetic)?;
        let height =
            u32::try_from(i64::from(bounds[3]) - i64::from(bounds[1])).map_err(|_| arithmetic)?;
        Ok(crate::decode::SubbandGeometry {
            resolution,
            orientation,
            x0: bounds[0],
            y0: bounds[1],
            width,
            height,
            resolution_width: dimensions[0],
            resolution_height: dimensions[1],
            quantization_index: index,
            quantization_step: step,
            dynamic_range,
            significant_bitplanes: u16::try_from(bitplanes).unwrap(),
            roi_shift: depth[1],
            low_pass_first_x: phase[0],
            low_pass_first_y: phase[1],
        })
    }

    #[test]
    fn subband_admission_matches_every_field_and_raw_error_priority() {
        use SubbandOrientation::{Hh, Hl, Lh, Ll};
        let mut cases = 0;
        let mut successes = 0;
        for style in [
            QuantizationStyle::None,
            QuantizationStyle::ScalarDerived,
            QuantizationStyle::ScalarExpounded,
        ] {
            for count in [0, 1, 2, 97, 98, u16::MAX] {
                for pattern in 0..8 {
                    let value = raw_subband_quantization(style, count, pattern);
                    for precision in 0..=u8::MAX {
                        for seed in 0_u8..16 {
                            let orientation =
                                [Ll, Hl, Lh, Hh][usize::from(seed.wrapping_add(precision) % 4)];
                            let resolution = [0, 1, 2, 31, 32, u8::MAX][usize::from(seed % 6)];
                            let index = [0, 1, 2, 96, 97, 98, u16::MAX][usize::from(seed % 7)];
                            let bounds = [
                                [0, 0, 3, 5],
                                [1, 1, 1, 1],
                                [u32::MAX, 0, 0, u32::MAX],
                                [0, u32::MAX, u32::MAX, 0],
                            ][usize::from(seed % 4)];
                            let dimensions = [u32::from(seed), u32::MAX];
                            let phase = [seed & 1 != 0, seed & 2 != 0];
                            let depth = [precision, seed * 17];
                            let expected = reference_subband_admission(
                                resolution,
                                orientation,
                                bounds,
                                dimensions,
                                phase,
                                depth,
                                &value,
                                index,
                            );
                            assert_eq!(
                                crate::decode::make_subband_geometry(
                                    resolution,
                                    orientation,
                                    bounds,
                                    dimensions,
                                    phase,
                                    depth,
                                    &value,
                                    index,
                                ),
                                expected,
                            );
                            successes += usize::from(expected.is_ok());
                            cases += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 589_824);
        assert!(
            successes > 1000,
            "{successes} complete subband outputs checked"
        );
    }

    fn raw_resolution_style(levels: u8, pattern: usize) -> CodingStyle {
        use DecompositionDirection::{Horizontal, None, TwoDimensional, Vertical};
        let directions = core::array::from_fn(|index| match pattern {
            0 => None,
            1 => Horizontal,
            2 => Vertical,
            3 => TwoDimensional,
            4 => [Horizontal, Vertical][index % 2],
            5 => [None, TwoDimensional][index % 2],
            6 => [Horizontal, None, Vertical][index % 3],
            _ => {
                if index == 31 {
                    TwoDimensional
                } else {
                    None
                }
            }
        });
        CodingStyle {
            progression_order: ProgressionOrder::Cprl,
            layers: 0,
            multiple_component_transform: true,
            decomposition_levels: levels,
            decomposition_structure: Some(u8::MAX),
            decomposition_directions: directions,
            code_block_width: 0,
            code_block_height: u16::MAX,
            code_block_style: u8::MAX,
            transform: WaveletTransform::Arbitrary(u8::MAX),
            precincts_defined: false,
            sop_markers: true,
            eph_markers: true,
            precinct_exponents: core::array::from_fn(|index| {
                let index = u8::try_from(index).unwrap();
                (index.wrapping_mul(17), u8::MAX - index.wrapping_mul(7))
            }),
        }
    }

    fn reference_resolution_scale_step(
        style: CodingStyle,
        stage: u8,
        scales: (u32, u32),
    ) -> Result<(u32, u32), DecodeError> {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodingStyle,
        };
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        if stage >= style.decomposition_levels {
            return Err(invalid);
        }
        let direction = style
            .decomposition_directions
            .get(usize::from(stage))
            .ok_or(invalid)?;
        let (horizontal, vertical) = match direction {
            DecompositionDirection::None => (u64::from(scales.0), u64::from(scales.1)),
            DecompositionDirection::Horizontal => (u64::from(scales.0) * 2, u64::from(scales.1)),
            DecompositionDirection::Vertical => (u64::from(scales.0), u64::from(scales.1) * 2),
            DecompositionDirection::TwoDimensional => {
                (u64::from(scales.0) * 2, u64::from(scales.1) * 2)
            }
        };
        Ok((
            u32::try_from(horizontal).map_err(|_| arithmetic)?,
            u32::try_from(vertical).map_err(|_| arithmetic)?,
        ))
    }

    fn reference_component_resolution_counts(
        components: u16,
        styles: &ComponentCodingStyles,
        discard: u8,
    ) -> Result<(u8, Vec<u8>, Vec<u8>), DecodeError> {
        let mut maximum = 0;
        let mut available = Vec::new();
        let mut selected = Vec::new();
        for component in 0..components {
            if component >= styles.components || usize::from(component) >= styles.styles.len() {
                return Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::InvalidCodingStyle,
                });
            }
            let count = u8::try_from(
                u16::from(styles.styles[usize::from(component)].decomposition_levels) + 1,
            )
            .map_err(|_| DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::ArithmeticOverflow,
            })?;
            maximum = maximum.max(count);
            available.push(count);
            if discard >= count {
                return Err(DecodeError::InvalidSelection {
                    reason: crate::SelectionError::ReductionTooLarge,
                });
            }
            selected.push(count - discard);
        }
        Ok((maximum, available, selected))
    }

    fn raw_grid_styles(components: u16, pattern: usize) -> ComponentCodingStyles {
        ComponentCodingStyles {
            components,
            styles: core::array::from_fn(|index| {
                let levels = match pattern {
                    0 => 0,
                    1 => u8::try_from(index % 3).unwrap(),
                    2 => u8::try_from(index % 4).unwrap(),
                    3 => {
                        if index == 2 {
                            u8::MAX
                        } else {
                            1
                        }
                    }
                    4 => u8::MAX,
                    5 => {
                        if index == 63 {
                            u8::MAX
                        } else {
                            1
                        }
                    }
                    6 => [0, 2, 31, 254][index % 4],
                    _ => 32,
                };
                let mut style = raw_resolution_style(levels, if pattern == 2 { 0 } else { 3 });
                style.code_block_width = 4;
                style.code_block_height = 4;
                style.precinct_exponents = [(0, 0); 33];
                style
            }),
        }
    }

    #[test]
    fn component_resolution_preparation_retains_raw_owners_discard_and_first_errors() {
        let mut cases = 0;
        for count in [0, 1, 2, 63, 64, 65, u16::MAX] {
            for owner in [0, 1, 2, 63, 64, 65, u16::MAX] {
                for discard in [0, 1, 2, 31, 32, 254, u8::MAX] {
                    for pattern in 0..8 {
                        let styles = raw_grid_styles(owner, pattern);
                        assert_eq!(
                            crate::decode::prepare_component_resolution_counts(
                                count, &styles, discard
                            ),
                            reference_component_resolution_counts(count, &styles, discard),
                        );
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 2744);
    }

    fn reference_subband_count(style: CodingStyle, resolution: u8) -> Result<usize, DecodeError> {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodingStyle,
        };
        if resolution > style.decomposition_levels {
            return Err(invalid);
        }
        let mut count = 1;
        for current in 1..=resolution {
            let stage = style.decomposition_levels - current;
            let direction = style
                .decomposition_directions
                .get(usize::from(stage))
                .ok_or(invalid)?;
            count += match direction {
                DecompositionDirection::None => 0,
                DecompositionDirection::Horizontal | DecompositionDirection::Vertical => 1,
                DecompositionDirection::TwoDimensional => 3,
            };
        }
        Ok(count)
    }

    fn reference_component_packet_grid(
        size: &ImageSize,
        styles: &ComponentCodingStyles,
        layers: u16,
        limits: &ResourceLimits,
        tile: Option<u16>,
        discard: u8,
    ) -> Result<crate::packet::PacketGrid, DecodeError> {
        let (maximum, resolutions, selected) =
            reference_component_resolution_counts(size.components, styles, discard)?;
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        usize::try_from(u128::from(size.components) * u128::from(maximum))
            .map_err(|_| arithmetic)?;
        let mut counts = Vec::new();
        let mut blocks = 0_u64;
        for component in 0..size.components {
            let style = styles.styles[usize::from(component)];
            let available = resolutions[usize::from(component)];
            for resolution in 0..maximum {
                if resolution >= available {
                    counts.push(0);
                    continue;
                }
                let geometry =
                    reference_resolution_geometry(size, style, component, resolution, tile)?;
                let current = reference_subband_count(style, resolution)?;
                let previous = if resolution == 0 {
                    0
                } else {
                    reference_subband_count(style, resolution - 1)?
                };
                let subbands = current.checked_sub(previous).ok_or(arithmetic)?;
                if resolution < selected[usize::from(component)] {
                    blocks = u64::try_from(u128::from(blocks) + u128::from(geometry.code_blocks))
                        .map_err(|_| arithmetic)?;
                    if blocks > limits.maximum_code_blocks() {
                        return Err(DecodeError::LimitExceeded {
                            resource: crate::Resource::CodeBlocks,
                            limit: limits.maximum_code_blocks(),
                            actual: blocks,
                        });
                    }
                }
                counts.push(if subbands == 0 {
                    0
                } else {
                    u32::try_from(geometry.precincts).map_err(|_| arithmetic)?
                });
            }
        }
        crate::packet::packet_grid_new_with_selected_resolutions(
            layers,
            size.components,
            resolutions,
            counts,
            if discard == 0 { None } else { Some(&selected) },
            limits,
        )
    }

    #[test]
    fn component_packet_grid_matches_real_geometry_padding_selection_and_limit_order() {
        let limits = [
            ResourceLimits::HARDENED,
            ResourceLimits::HARDENED
                .with_maximum_code_blocks(1)
                .unwrap(),
            ResourceLimits::HARDENED.with_maximum_work_units(1).unwrap(),
            ResourceLimits::HARDENED.with_maximum_packets(1).unwrap(),
        ];
        let mut cases = 0;
        let mut successes = 0;
        for extent in [0, 1, 17, 33] {
            for sampling in [(1, 1), (1, 3), (2, 2)] {
                for components in [0, 1, 2, 3, 4] {
                    let size = raw_resolution_size(extent, 1, sampling, u16::MAX, components);
                    for owner in [0, 1, 2, 4, u16::MAX] {
                        for pattern in 0..4 {
                            let styles = raw_grid_styles(owner, pattern);
                            for layers in [0, 1, 2] {
                                for discard in [0, 1, 2, 3] {
                                    for tile in [None, Some(0), Some(1), Some(u16::MAX)] {
                                        for limit in &limits {
                                            let actual =
                                                crate::decode::derive_packet_grid_for_components(
                                                    &size, &styles, layers, limit, tile, discard,
                                                );
                                            let expected = reference_component_packet_grid(
                                                &size, &styles, layers, limit, tile, discard,
                                            );
                                            successes += usize::from(expected.is_ok());
                                            assert_eq!(actual, expected);
                                            cases += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 230_400);
        assert!(successes > 100, "{successes} full grid states checked");
    }

    fn raw_resolution_size(
        extent: u32,
        origin: u32,
        sampling: (u8, u8),
        registration: u16,
        components: u16,
    ) -> ImageSize {
        ImageSize {
            width: extent,
            height: extent,
            x_origin: origin,
            y_origin: origin.min(1),
            components,
            uniform_bit_depth: None,
            uniform_signed: None,
            tile_width: 17,
            tile_height: 31,
            tile_x_origin: origin.saturating_sub(1),
            tile_y_origin: 0,
            tiles_across: 2,
            tiles_down: 2,
            component_info: [ComponentInfo {
                precision: registration as u8,
                signed: true,
                x_subsampling: sampling.0,
                y_subsampling: sampling.1,
                x_registration: registration,
                y_registration: u16::MAX - registration,
            }; MAX_COMPONENTS],
        }
    }

    fn reference_component_coordinate_bounds(
        size: &ImageSize,
        component: u16,
        tile_index: Option<u16>,
    ) -> Result<[u32; 4], DecodeError> {
        if let Some(tile) = tile_index {
            let bounds = reference_tile_component_bounds(size, tile, component)?;
            return Ok([bounds.x0, bounds.y0, bounds.x1, bounds.y1]);
        }
        let geometry = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidImageGeometry,
        };
        if component >= size.components {
            return Err(geometry);
        }
        let info = size
            .component_info
            .get(usize::from(component))
            .ok_or(geometry)?;
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let x1 = u32::try_from(u128::from(size.x_origin) + u128::from(size.width))
            .map_err(|_| arithmetic)?;
        let y1 = u32::try_from(u128::from(size.y_origin) + u128::from(size.height))
            .map_err(|_| arithmetic)?;
        let ceil = |value: u32, divisor: u8| {
            if divisor == 0 {
                return Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::InvalidCodeBlock,
                });
            }
            Ok(u32::try_from(u64::from(value).div_ceil(u64::from(divisor))).unwrap())
        };
        Ok([
            ceil(size.x_origin, info.x_subsampling)?,
            ceil(size.y_origin, info.y_subsampling)?,
            ceil(x1, info.x_subsampling)?,
            ceil(y1, info.y_subsampling)?,
        ])
    }

    fn reference_resolution_geometry(
        size: &ImageSize,
        style: CodingStyle,
        component: u16,
        resolution: u8,
        tile_index: Option<u16>,
    ) -> Result<crate::decode::ComponentResolutionGeometry, DecodeError> {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodeBlock,
        };
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        if resolution > style.decomposition_levels {
            return Err(invalid);
        }
        let bounds = reference_component_coordinate_bounds(size, component, tile_index)?;
        let scale = (0..style.decomposition_levels - resolution)
            .try_fold((1, 1), |before, stage| {
                reference_resolution_scale_step(style, stage, before)
            })?;
        let ceil = |value: u32, divisor: u32| {
            u32::try_from(u128::from(value).div_ceil(u128::from(divisor))).unwrap()
        };
        let x0 = ceil(bounds[0], scale.0);
        let y0 = ceil(bounds[1], scale.1);
        let x1 = ceil(bounds[2], scale.0);
        let y1 = ceil(bounds[3], scale.1);
        let width = x1.checked_sub(x0).ok_or(arithmetic)?;
        let height = y1.checked_sub(y0).ok_or(arithmetic)?;
        if style.code_block_width == 0 || style.code_block_height == 0 {
            return Err(invalid);
        }
        let cells = |start: u32, end: u32, step: u32| {
            let right = u64::from(end).div_ceil(u64::from(step));
            let left = u64::from(start) / u64::from(step);
            u32::try_from(right.checked_sub(left).ok_or(arithmetic)?).map_err(|_| arithmetic)
        };
        let code_blocks_across = cells(x0, x1, u32::from(style.code_block_width))?;
        let code_blocks_down = cells(y0, y1, u32::from(style.code_block_height))?;
        let code_blocks =
            u64::try_from(u128::from(code_blocks_across) * u128::from(code_blocks_down))
                .map_err(|_| arithmetic)?;
        let precinct = style
            .precinct_exponents
            .get(usize::from(resolution))
            .ok_or(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodingStyle,
            })?;
        if precinct.0 > 31 || precinct.1 > 31 {
            return Err(invalid);
        }
        let precincts_across = cells(x0, x1, 1_u32 << precinct.0)?;
        let precincts_down = cells(y0, y1, 1_u32 << precinct.1)?;
        let precincts = u64::try_from(u128::from(precincts_across) * u128::from(precincts_down))
            .map_err(|_| arithmetic)?;
        Ok(crate::decode::ComponentResolutionGeometry {
            component,
            resolution,
            width,
            height,
            subbands: if resolution == 0 { 1 } else { 3 },
            code_blocks_across,
            code_blocks_down,
            code_blocks,
            precincts_across,
            precincts_down,
            precincts,
        })
    }

    #[test]
    fn resolution_geometry_matches_complete_raw_whole_and_registered_tile_outputs() {
        let mut cases = 0;
        let mut successful = 0;
        for extent in [0, 1, 33, 1025, u32::MAX] {
            for origin in [0, 1, u32::MAX] {
                for sampling in [(1, 1), (1, 3), (2, 255), (255, 2)] {
                    for registration in [0, 1, u16::MAX] {
                        let size = raw_resolution_size(extent, origin, sampling, registration, 1);
                        for tile in [None, Some(0), Some(1)] {
                            assert_eq!(
                                crate::decode::component_coordinate_bounds(&size, 0, tile),
                                reference_component_coordinate_bounds(&size, 0, tile),
                            );
                            for (levels, resolution, pattern) in [
                                (0, 0, 0),
                                (1, 0, 1),
                                (1, 1, 2),
                                (5, 2, 3),
                                (31, 0, 1),
                                (32, 0, 3),
                                (33, 0, 0),
                                (255, 255, 7),
                            ] {
                                for (block_x, block_y) in
                                    [(0, 1), (1, 0), (1, 1), (3, 7), (u16::MAX, u16::MAX)]
                                {
                                    for precinct in
                                        [(0, 0), (1, 31), (31, 1), (32, 0), (0, 32), (255, 255)]
                                    {
                                        let mut style = raw_resolution_style(levels, pattern);
                                        style.code_block_width = block_x;
                                        style.code_block_height = block_y;
                                        if let Some(slot) = style
                                            .precinct_exponents
                                            .get_mut(usize::from(resolution))
                                        {
                                            *slot = precinct;
                                        }
                                        let expected = reference_resolution_geometry(
                                            &size, style, 0, resolution, tile,
                                        );
                                        assert_eq!(
                                            crate::decode::derive_component_resolution_geometry_for(
                                                &size, style, 0, resolution, tile,
                                            ),
                                            expected,
                                        );
                                        if tile.is_none() {
                                            assert_eq!(
                                                crate::decode::derive_component_resolution_geometry(
                                                    &size, style, 0, resolution
                                                ),
                                                expected,
                                            );
                                        }
                                        successful += usize::from(expected.is_ok());
                                        cases += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 129_600);
        assert!(
            successful > 1000,
            "{successful} successful whole-owner outputs"
        );
    }

    #[test]
    fn resolution_geometry_preserves_raw_admission_and_sampling_error_priority() {
        let mut cases = 0;
        for extent in [0, 1, 33, 1025, u32::MAX] {
            for origin in [0, 1, u32::MAX] {
                for sampling in [(0, 0), (0, 1), (1, 0), (1, 1), (2, 255), (255, 2)] {
                    for count in [0, 1, 64, 65, u16::MAX] {
                        let size = raw_resolution_size(extent, origin, sampling, u16::MAX, count);
                        for component in [0, 1, 63, 64, u16::MAX] {
                            for tile in [None, Some(0), Some(1), Some(u16::MAX)] {
                                assert_eq!(
                                    crate::decode::component_coordinate_bounds(
                                        &size, component, tile
                                    ),
                                    reference_component_coordinate_bounds(&size, component, tile),
                                );
                                for (levels, resolution, pattern) in
                                    [(0, 255, 0), (255, 255, 7), (32, 0, 1)]
                                {
                                    let mut style = raw_resolution_style(levels, pattern);
                                    style.code_block_width = 3;
                                    style.code_block_height = 7;
                                    assert_eq!(
                                        crate::decode::derive_component_resolution_geometry_for(
                                            &size, style, component, resolution, tile,
                                        ),
                                        reference_resolution_geometry(
                                            &size, style, component, resolution, tile
                                        ),
                                    );
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 27_000);
    }

    #[test]
    fn resolution_styles_preserve_all_raw_level_resolution_and_lazy_precinct_cases() {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodingStyle,
        };
        let mut cases = 0;
        for pattern in 0..8 {
            for levels in 0..=u8::MAX {
                let style = raw_resolution_style(levels, pattern);
                for resolution in 0..=u8::MAX {
                    let expected = if resolution > levels {
                        Err(invalid)
                    } else {
                        (0..levels - resolution).try_fold((1, 1), |scales, stage| {
                            reference_resolution_scale_step(style, stage, scales)
                        })
                    };
                    assert_eq!(style.resolution_downsampling(resolution), expected);
                    let expected_precinct = if resolution > levels {
                        Err(invalid)
                    } else {
                        style
                            .precinct_exponents
                            .get(usize::from(resolution))
                            .copied()
                            .ok_or(invalid)
                    };
                    assert_eq!(style.precinct_exponents(resolution), expected_precinct);
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 524_288);
        for horizontal in 0..=u8::MAX {
            for vertical in 0..=u8::MAX {
                let mut style = raw_resolution_style(0, 0);
                style.precinct_exponents[0] = (horizontal, vertical);
                assert_eq!(style.precinct_exponents(0), Ok((horizontal, vertical)));
            }
        }
    }

    #[test]
    fn resolution_scale_step_preserves_raw_scales_selector_priority_and_overflow() {
        let scales = [0, 1, 2, (1_u32 << 31) - 1, 1_u32 << 31, u32::MAX];
        let mut cases = 0;
        for pattern in 0..8 {
            for levels in [0, 1, 31, 32, 33, u8::MAX] {
                let style = raw_resolution_style(levels, pattern);
                for stage in 0..=u8::MAX {
                    for horizontal in scales {
                        for vertical in scales {
                            assert_eq!(
                                style.resolution_downsampling_step(stage, (horizontal, vertical)),
                                reference_resolution_scale_step(
                                    style,
                                    stage,
                                    (horizontal, vertical)
                                ),
                            );
                            cases += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 442_368);
    }

    fn independent_execution_header(
        style: CodingStyle,
        extensions: &Part2Extensions,
    ) -> Result<(), DecodeError> {
        match style.decomposition_structure {
            Some(index)
                if !extensions
                    .decomposition_structures
                    .iter()
                    .any(|value| value.index == index)
                    && !extensions
                        .arbitrary_decompositions
                        .iter()
                        .any(|value| value.index == index) =>
            {
                return Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::MissingRequiredStructure,
                });
            }
            _ => {}
        }
        match style.transform {
            WaveletTransform::Arbitrary(index)
                if !extensions
                    .arbitrary_kernels
                    .iter()
                    .any(|value| value.index == index) =>
            {
                return Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::MissingRequiredStructure,
                });
            }
            _ => {}
        }
        Ok(())
    }

    #[test]
    fn actual_layout_execution_admission_preserves_raw_count_and_global_error_order() {
        let headers = baseline_headers();
        let bytes = codestream_with_headers(1, &headers, &[], &[]);
        let limits = ResourceLimits::default();
        let mut layout = parse_layout(&bytes, &limits).unwrap();
        let baseline = layout.info.coding_style;
        let template = layout.tile_parts[0].clone();
        layout.tile_parts = vec![template.clone(), template];
        let fixture = |mode| {
            let mut style = baseline;
            style.layers = 0;
            style.decomposition_levels = u8::MAX;
            style.decomposition_structure = None;
            style.transform = WaveletTransform::Reversible5x3;
            let mut extensions = Part2Extensions {
                metadata_bytes: u64::MAX,
                ..Part2Extensions::default()
            };
            match mode {
                1 => style.decomposition_structure = Some(7),
                2 => style.transform = WaveletTransform::Arbitrary(8),
                3 => {
                    style.decomposition_structure = Some(7);
                    extensions
                        .decomposition_structures
                        .push(DecompositionStructure {
                            index: 7,
                            directions: vec![],
                        });
                }
                4 => {
                    style.decomposition_structure = Some(7);
                    extensions
                        .arbitrary_decompositions
                        .push(ArbitraryDecomposition {
                            index: 7,
                            sublevel_depths: vec![u8::MAX],
                            subband_splits: vec![],
                        });
                }
                _ => {}
            }
            (style, extensions)
        };
        let main_depths = vec![
            ComponentBitDepth {
                precision: 0,
                signed: false,
            },
            ComponentBitDepth {
                precision: u8::MAX,
                signed: true,
            },
        ];
        let mut cases = 0;
        for count in [0, 1, 2, 64, 65, u16::MAX] {
            for main_count in [0, 1, 64, 65] {
                for tile_count in [0, 1, 64, 65] {
                    for main_mode in [0, 1, 2] {
                        for first_mode in [0, 1, 3] {
                            for late_mode in [0, 2, 4] {
                                for depth_mode in 0..5 {
                                    let (main_style, mut main) = fixture(main_mode);
                                    main.component_bit_depths = main_depths.clone();
                                    layout.info.image_size.components = count;
                                    layout.info.component_coding_styles = ComponentCodingStyles {
                                        components: main_count,
                                        styles: [main_style; MAX_COMPONENTS],
                                    };
                                    layout.info.part2_extensions = main;
                                    let (first_style, mut first) = fixture(first_mode);
                                    first.component_bit_depths = match depth_mode {
                                        0 => vec![],
                                        1 => main_depths.clone(),
                                        2 => vec![
                                            ComponentBitDepth {
                                                precision: 1,
                                                signed: false,
                                            },
                                            main_depths[1],
                                        ],
                                        3 => vec![
                                            ComponentBitDepth {
                                                precision: 0,
                                                signed: true,
                                            },
                                            main_depths[1],
                                        ],
                                        _ => vec![main_depths[0]],
                                    };
                                    layout.tile_parts[0].component_coding_styles =
                                        ComponentCodingStyles {
                                            components: tile_count,
                                            styles: [first_style; MAX_COMPONENTS],
                                        };
                                    layout.tile_parts[0].part2_extensions = first;
                                    let (late_style, late) = fixture(late_mode);
                                    layout.tile_parts[1].component_coding_styles =
                                        ComponentCodingStyles {
                                            components: tile_count,
                                            styles: [late_style; MAX_COMPONENTS],
                                        };
                                    layout.tile_parts[1].part2_extensions = late;
                                    let expected = (|| {
                                        for component in 0..count {
                                            independent_execution_header(
                                                layout
                                                    .info
                                                    .component_coding_styles
                                                    .style(component)?,
                                                &layout.info.part2_extensions,
                                            )?;
                                        }
                                        for part in &layout.tile_parts {
                                            for component in 0..count {
                                                independent_execution_header(
                                                    part.component_coding_styles
                                                        .style(component)?,
                                                    &part.part2_extensions,
                                                )?;
                                            }
                                            if !part
                                                .part2_extensions
                                                .component_bit_depths
                                                .is_empty()
                                                && part.part2_extensions.component_bit_depths
                                                    != layout
                                                        .info
                                                        .part2_extensions
                                                        .component_bit_depths
                                            {
                                                return Err(DecodeError::MalformedInput {
                                                    offset: 0,
                                                    kind: MalformedKind::InvalidMarkerOrder,
                                                });
                                            }
                                        }
                                        Ok(())
                                    })();
                                    assert_eq!(
                                        crate::decode::validate_part2_execution(&layout),
                                        expected
                                    );
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 12960);
    }

    #[test]
    fn actual_execution_style_admission_matches_raw_registry_membership_and_lazy_fields() {
        let headers = baseline_headers();
        let bytes = codestream_with_headers(1, &headers, &[], &[]);
        let limits = ResourceLimits::default();
        let baseline = parse_layout(&bytes, &limits).unwrap().info.coding_style;
        let mut cases = 0;
        for stored in 0..=u8::MAX {
            let ads_index = stored.wrapping_add(2);
            let atk_index = stored.wrapping_add(3);
            let extensions = Part2Extensions {
                decomposition_structures: vec![
                    DecompositionStructure {
                        index: stored,
                        directions: vec![baseline.decomposition_directions[0]; 33],
                    },
                    DecompositionStructure {
                        index: stored.wrapping_add(1),
                        directions: vec![],
                    },
                    DecompositionStructure {
                        index: stored,
                        directions: vec![],
                    },
                ],
                arbitrary_decompositions: vec![
                    ArbitraryDecomposition {
                        index: ads_index,
                        sublevel_depths: vec![u8::MAX],
                        subband_splits: vec![],
                    },
                    ArbitraryDecomposition {
                        index: ads_index,
                        sublevel_depths: vec![],
                        subband_splits: vec![],
                    },
                ],
                arbitrary_kernels: vec![ArbitraryKernel {
                    index: atk_index,
                    reversible: false,
                    descriptor: u16::MAX,
                    coefficient_type: u8::MAX,
                    whole_sample_filter: false,
                    initial_odd: true,
                    whole_sample_extension: false,
                    scaling: None,
                    steps: vec![],
                }],
                metadata_bytes: u64::MAX,
                ..Part2Extensions::default()
            };
            extensions.validate_executable().unwrap();
            for index in 0..=u8::MAX {
                let dsc_position = if index == stored {
                    Some(0)
                } else if index == stored.wrapping_add(1) {
                    Some(1)
                } else {
                    None
                };
                let dsc = extensions.decomposition_structure(index);
                assert_eq!(dsc.is_some(), dsc_position.is_some());
                if let Some(position) = dsc_position {
                    assert!(std::ptr::eq(
                        dsc.unwrap(),
                        &extensions.decomposition_structures[position]
                    ));
                }
                let ads = extensions.arbitrary_decomposition(index);
                assert_eq!(ads.is_some(), index == ads_index);
                if index == ads_index {
                    assert!(std::ptr::eq(
                        ads.unwrap(),
                        &extensions.arbitrary_decompositions[0]
                    ));
                }
                for transform in [
                    WaveletTransform::Reversible5x3,
                    WaveletTransform::Irreversible9x7,
                    WaveletTransform::Arbitrary(atk_index),
                    WaveletTransform::Arbitrary(atk_index.wrapping_add(1)),
                ] {
                    let mut style = baseline;
                    style.decomposition_structure = Some(index);
                    style.transform = transform;
                    style.layers = 0;
                    style.decomposition_levels = u8::MAX;
                    style.code_block_width = 0;
                    style.code_block_height = u16::MAX;
                    style.code_block_style = u8::MAX;
                    let decomposition_exists = dsc_position.is_some() || index == ads_index;
                    let kernel_exists = !matches!(transform, WaveletTransform::Arbitrary(requested) if requested != atk_index);
                    let expected = if decomposition_exists && kernel_exists {
                        Ok(())
                    } else {
                        Err(DecodeError::MalformedInput {
                            offset: 0,
                            kind: MalformedKind::MissingRequiredStructure,
                        })
                    };
                    assert_eq!(
                        crate::decode::validate_part2_coding_style(style, &extensions),
                        expected
                    );
                    cases += 1;
                }
            }
            let mut style = baseline;
            style.decomposition_structure = None;
            style.transform = WaveletTransform::Arbitrary(atk_index);
            crate::decode::validate_part2_coding_style(style, &extensions).unwrap();
        }
        assert_eq!(cases, 262144);
    }

    #[test]
    fn actual_component_depth_comparison_preserves_raw_order_precision_and_sign() {
        let mut cases = 0;
        for precision in 0..=u8::MAX {
            for signed in [false, true] {
                let values = vec![
                    ComponentBitDepth { precision, signed },
                    ComponentBitDepth {
                        precision: 0,
                        signed: !signed,
                    },
                    ComponentBitDepth { precision, signed },
                ];
                let left = Part2Extensions {
                    component_bit_depths: values.clone(),
                    metadata_bytes: u64::MAX,
                    ..Part2Extensions::default()
                };
                for variant in 0..5 {
                    let mut other_values = values.clone();
                    match variant {
                        1 => other_values[2].precision = precision.wrapping_add(1),
                        2 => other_values[2].signed = !signed,
                        3 => {
                            other_values.pop();
                        }
                        4 => other_values.push(ComponentBitDepth { precision, signed }),
                        _ => {}
                    }
                    let expected = values == other_values;
                    let right = Part2Extensions {
                        component_bit_depths: other_values,
                        ..Part2Extensions::default()
                    };
                    assert_eq!(left.component_bit_depths_equal(&right), expected);
                    assert_eq!(right.component_bit_depths_equal(&left), expected);
                    assert_eq!(left.component_bit_depths(), &values);
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 2560);
        let empty = Part2Extensions::default();
        assert!(empty.component_bit_depths_equal(&empty));
    }

    #[test]
    fn registered_component_ceiling_matches_wide_arithmetic_and_all_registration_words() {
        let oracle = |value: u32, divisor: u32, registration: u16| {
            if divisor == 0 {
                return Err(selection_geometry_invalid());
            }
            let numerator = (u128::from(value) * 65_536).saturating_sub(u128::from(registration));
            let denominator = u128::from(divisor) * 65_536;
            Ok(u32::try_from(numerator.div_ceil(denominator)).unwrap())
        };
        for value in [0, 1, 2, 255, 65_535, 65_536, u32::MAX - 1, u32::MAX] {
            for divisor in [0, 1, 2, 3, 255, 65_536, u32::MAX - 1, u32::MAX] {
                for registration in [0, 1, 32_767, 32_768, 65_534, u16::MAX] {
                    let expected = oracle(value, divisor, registration);
                    assert_eq!(registered_ceil_div(value, divisor, registration), expected,);
                    assert_eq!(
                        crate::decode::registered_ceil_div_u32(value, divisor, registration),
                        if divisor == 0 {
                            Err(DecodeError::MalformedInput {
                                offset: 0,
                                kind: MalformedKind::InvalidCodeBlock,
                            })
                        } else {
                            expected
                        },
                    );
                }
            }
        }
        for registration in 0..=u16::MAX {
            for value in [0, 1, u32::MAX] {
                for divisor in [0, 1, 2, 255, u32::MAX] {
                    let expected = oracle(value, divisor, registration);
                    assert_eq!(registered_ceil_div(value, divisor, registration), expected,);
                    assert_eq!(
                        crate::decode::registered_ceil_div_u32(value, divisor, registration),
                        if divisor == 0 {
                            Err(DecodeError::MalformedInput {
                                offset: 0,
                                kind: MalformedKind::InvalidCodeBlock,
                            })
                        } else {
                            expected
                        },
                    );
                }
            }
        }
    }

    fn reference_tile_component_bounds(
        size: &ImageSize,
        tile_index: u16,
        component: u16,
    ) -> Result<TileComponentBounds, DecodeError> {
        let invalid = || DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidImageGeometry,
        };
        let checked = |value: u128| {
            u32::try_from(value).map_err(|_| DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::ArithmeticOverflow,
            })
        };
        if u64::from(tile_index) >= u64::from(size.tiles_across) * u64::from(size.tiles_down)
            || size.tiles_across == 0
        {
            return Err(invalid());
        }
        let nx = checked(
            u128::from(size.tile_x_origin)
                + u128::from(u32::from(tile_index) % size.tiles_across)
                    * u128::from(size.tile_width),
        )?;
        let ny = checked(
            u128::from(size.tile_y_origin)
                + u128::from(u32::from(tile_index) / size.tiles_across)
                    * u128::from(size.tile_height),
        )?;
        let image_x1 = checked(u128::from(size.x_origin) + u128::from(size.width))?;
        let image_y1 = checked(u128::from(size.y_origin) + u128::from(size.height))?;
        let tx0 = nx.max(size.x_origin);
        let ty0 = ny.max(size.y_origin);
        let tx1 = checked(u128::from(nx) + u128::from(size.tile_width))?.min(image_x1);
        let ty1 = checked(u128::from(ny) + u128::from(size.tile_height))?.min(image_y1);
        if tx0 >= tx1 || ty0 >= ty1 {
            return Err(invalid());
        }
        if component >= size.components {
            return Err(invalid());
        }
        let info = size
            .component_info
            .get(usize::from(component))
            .ok_or_else(invalid)?;
        let ceil = |value: u32, divisor: u8, registration: u16| {
            if divisor == 0 {
                return Err(invalid());
            }
            let numerator = (u128::from(value) * 65_536).saturating_sub(u128::from(registration));
            Ok(u32::try_from(numerator.div_ceil(u128::from(divisor) * 65_536)).unwrap())
        };
        let image_x0 = ceil(size.x_origin, info.x_subsampling, info.x_registration)?;
        let image_y0 = ceil(size.y_origin, info.y_subsampling, info.y_registration)?;
        let x0 = ceil(tx0, info.x_subsampling, info.x_registration)?;
        let y0 = ceil(ty0, info.y_subsampling, info.y_registration)?;
        let x1 = ceil(tx1, info.x_subsampling, info.x_registration)?;
        let y1 = ceil(ty1, info.y_subsampling, info.y_registration)?;
        if x0 >= x1 || y0 >= y1 {
            return Err(invalid());
        }
        Ok(TileComponentBounds {
            tile_index,
            component,
            x0,
            y0,
            x1,
            y1,
            image_x: x0.checked_sub(image_x0).ok_or_else(invalid)?,
            image_y: y0.checked_sub(image_y0).ok_or_else(invalid)?,
        })
    }

    #[test]
    fn tile_component_bounds_match_raw_geometry_and_ordered_descriptor_admission() {
        let mut cases = 0;
        for extent in [0, 1, 3, 31, u32::MAX] {
            for span in [0, 1, 2, 31, u32::MAX] {
                for origin in [0, 1, u32::MAX] {
                    for tile_index in [0, 1, 5, 6, u16::MAX] {
                        for components in [0, 1, 64, 65, u16::MAX] {
                            for component in [0, 1, 63, 64, u16::MAX] {
                                for (x_subsampling, y_subsampling) in
                                    [(0, 0), (0, 1), (1, 0), (1, 1), (2, 255), (255, 2)]
                                {
                                    for registration in [0, 1, u16::MAX] {
                                        let size = ImageSize {
                                            width: extent,
                                            height: extent,
                                            x_origin: origin,
                                            y_origin: origin.min(1),
                                            components,
                                            uniform_bit_depth: None,
                                            uniform_signed: None,
                                            tile_width: span,
                                            tile_height: span.wrapping_add(1),
                                            tile_x_origin: origin.saturating_sub(1),
                                            tile_y_origin: 0,
                                            tiles_across: if span == u32::MAX { span } else { 3 },
                                            tiles_down: span,
                                            component_info: [ComponentInfo {
                                                precision: registration as u8,
                                                signed: true,
                                                x_subsampling,
                                                y_subsampling,
                                                x_registration: registration,
                                                y_registration: u16::MAX - registration,
                                            };
                                                MAX_COMPONENTS],
                                        };
                                        assert_eq!(
                                            size.tile_component_bounds(tile_index, component),
                                            reference_tile_component_bounds(
                                                &size, tile_index, component
                                            ),
                                            "extent={extent} span={span} origin={origin} tile={tile_index} count={components} component={component} sampling=({x_subsampling},{y_subsampling}) registration={registration}",
                                        );
                                        cases += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 168_750);
    }

    #[test]
    fn actual_kernel_registry_retains_first_duplicates_and_every_raw_index() {
        let kernel = |index, reversible| ArbitraryKernel {
            index,
            reversible,
            descriptor: u16::MAX,
            coefficient_type: u8::MAX,
            whole_sample_filter: false,
            initial_odd: true,
            whole_sample_extension: false,
            scaling: None,
            steps: Vec::new(),
        };
        for first in 0..=u8::MAX {
            let extensions = Part2Extensions {
                arbitrary_kernels: vec![
                    kernel(first, true),
                    kernel(first.wrapping_add(1), false),
                    kernel(first, false),
                ],
                ..Part2Extensions::default()
            };
            for index in 0..=u8::MAX {
                let expected = if index == first {
                    Some(0)
                } else if index == first.wrapping_add(1) {
                    Some(1)
                } else {
                    None
                };
                let actual = extensions.arbitrary_kernel(index);
                assert_eq!(actual.is_some(), expected.is_some());
                if let Some(position) = expected {
                    assert!(std::ptr::eq(
                        actual.unwrap(),
                        &extensions.arbitrary_kernels[position]
                    ));
                }
            }
        }
    }

    fn input_error_literal(offset: usize, kind: MalformedKind) -> DecodeError {
        DecodeError::MalformedInput {
            offset: offset as u64,
            kind,
        }
    }

    fn packet_length_wide_window(
        encoded: &[u8],
        start: usize,
        end: usize,
    ) -> Result<(u32, usize), DecodeError> {
        let invalid = input_error_literal(start, MalformedKind::InvalidPacketLength);
        if start >= end || end > encoded.len() {
            return Err(invalid);
        }
        let window = &encoded[start..end];
        let Some(last) = window.iter().position(|byte| byte & 0x80 == 0) else {
            return Err(invalid);
        };
        if last >= 5 {
            return Err(invalid);
        }
        let wide: u128 = window[..=last]
            .iter()
            .enumerate()
            .map(|(index, byte)| u128::from(byte & 0x7f) << (7 * (last - index)))
            .sum();
        if wide == 0 || wide > u128::from(u32::MAX) {
            Err(invalid)
        } else {
            Ok((wide as u32, start + last + 1))
        }
    }

    #[test]
    fn packet_lengths_exhaustive_two_byte_windows_and_every_raw_iterator_field() {
        for word in 0_u16..=u16::MAX {
            let bytes = word.to_be_bytes();
            for position in [0, 1, 2, 3, usize::MAX] {
                for segment_end in [0, 1, 2, 3, usize::MAX] {
                    assert_eq!(
                        decode_packet_length(&bytes, position, segment_end),
                        packet_length_wide_window(&bytes, position, segment_end)
                    );
                    for source in [
                        PacketLengthSource::None,
                        PacketLengthSource::Plm(&bytes),
                        PacketLengthSource::Plt(&bytes),
                    ] {
                        let lengths = PacketLengths {
                            source,
                            packet_count: u64::MAX,
                            encoded_bytes: u64::MAX,
                        };
                        assert_eq!(lengths.is_present(), source != PacketLengthSource::None);
                        assert_eq!(lengths.packet_count(), u64::MAX);
                        assert_eq!(lengths.encoded_bytes(), u64::MAX);
                        let fresh = lengths.iter();
                        assert_eq!(
                            (fresh.source, fresh.position, fresh.segment_end),
                            (source, 0, 0)
                        );
                        let mut actual = PacketLengthIter {
                            source,
                            position,
                            segment_end,
                        };
                        let expected = match source {
                            PacketLengthSource::None => Ok(None),
                            PacketLengthSource::Plm(_) if position == bytes.len() => Ok(None),
                            PacketLengthSource::Plm(_) => {
                                packet_length_wide_window(&bytes, position, bytes.len()).map(Some)
                            }
                            PacketLengthSource::Plt(_) if position < segment_end => {
                                packet_length_wide_window(&bytes, position, segment_end).map(Some)
                            }
                            PacketLengthSource::Plt(_) if position == bytes.len() => Ok(None),
                            PacketLengthSource::Plt(_) => Err(input_error_literal(
                                position,
                                MalformedKind::InvalidPacketLength,
                            )),
                        };
                        let (expected_result, expected_position) = match expected {
                            Ok(Some((value, next))) => (Ok(Some(value)), next),
                            Ok(None) => (Ok(None), position),
                            Err(error) => (Err(error), position),
                        };
                        assert_eq!(actual.next_length(), expected_result);
                        assert_eq!(
                            (actual.source, actual.position, actual.segment_end),
                            (source, expected_position, segment_end)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn packet_length_full_width_noncanonical_and_unterminated_groups_match_wide_words() {
        let inputs: &[&[u8]] = &[
            &[],
            &[0],
            &[0x80],
            &[0x81, 0],
            &[0x80, 0x80, 0x80, 0x80, 1],
            &[0x8f, 0xff, 0xff, 0xff, 0x7f],
            &[0x90, 0x80, 0x80, 0x80, 0],
            &[0xff, 0xff, 0xff, 0xff, 0xff],
            &[0x80, 0x80, 0x80, 0x80, 0x80, 1],
        ];
        for encoded in inputs {
            for start in (0..=encoded.len() + 1).chain([usize::MAX]) {
                for end in (0..=encoded.len() + 1).chain([usize::MAX]) {
                    assert_eq!(
                        decode_packet_length(encoded, start, end),
                        packet_length_wide_window(encoded, start, end)
                    );
                }
            }
        }
        assert_eq!(decode_packet_length(inputs[5], 0, 5), Ok((u32::MAX, 5)));
        assert_eq!(decode_packet_length(inputs[4], 0, 5), Ok((1, 5)));
    }

    #[test]
    fn plt_real_marker_traversal_keeps_late_cursor_commits_and_repeated_error_states() {
        type Fixture<'a> = (&'a [u8], Result<Option<u32>, DecodeError>, usize, usize);
        let fixtures: &[Fixture<'_>] = &[
            (&[], Ok(None), 0, 0),
            (&[0xff, 0x55, 0, 2], Ok(None), 4, 0),
            (
                &[0xff, 0x58, 0, 2],
                Err(input_error_literal(4, MalformedKind::InvalidMarkerLength)),
                4,
                0,
            ),
            (&[0xff, 0x58, 0, 3, 9], Ok(None), 5, 5),
            (
                &[0xff, 0x58, 0, 4, 9, 0],
                Err(input_error_literal(5, MalformedKind::InvalidPacketLength)),
                5,
                6,
            ),
            (
                &[0xff, 0x55, 0, 2, 0xff, 0x58, 0, 2],
                Err(input_error_literal(8, MalformedKind::InvalidMarkerLength)),
                8,
                0,
            ),
            (
                &[0xff, 0x58, 0, 7],
                Err(input_error_literal(0, MalformedKind::InvalidMarkerLength)),
                0,
                0,
            ),
        ];
        for (header, expected, position, end) in fixtures {
            let source = PacketLengthSource::Plt(header);
            let mut actual = PacketLengthIter {
                source,
                position: 0,
                segment_end: 0,
            };
            assert_eq!(actual.next_length(), *expected);
            assert_eq!(
                (actual.source, actual.position, actual.segment_end),
                (source, *position, *end)
            );
        }
        let header = [
            0xff, 0x58, 0, 5, 0, 0x81, 0, 0xff, 0x55, 0, 2, 0xff, 0x58, 0, 4, 0, 5,
        ];
        let mut actual = PacketLengthIter {
            source: PacketLengthSource::Plt(&header),
            position: 0,
            segment_end: 0,
        };
        for (result, position, end) in [
            (Ok(Some(128)), 7, 7),
            (Ok(Some(5)), 17, 17),
            (Ok(None), 17, 17),
        ] {
            assert_eq!(actual.next_length(), result);
            assert_eq!((actual.position, actual.segment_end), (position, end));
        }
        let header = [0xff, 0x58, 0, 4, 0, 0];
        let mut actual = PacketLengthIter {
            source: PacketLengthSource::Plt(&header),
            position: 0,
            segment_end: 0,
        };
        for _ in 0..2 {
            assert_eq!(
                actual.next_length(),
                Err(input_error_literal(5, MalformedKind::InvalidPacketLength))
            );
            assert_eq!((actual.position, actual.segment_end), (5, 6));
        }
    }

    #[test]
    fn packed_reassembly_literal_fragments_ranges_and_late_errors_have_exact_bytes() {
        type Fixture<'a> = (&'a [u8], &'a [u8], Option<(usize, MalformedKind)>);
        let fixtures: &[Fixture<'_>] = &[
            (&[], &[], None),
            (
                &[0xff],
                &[],
                Some((0, MalformedKind::InvalidPackedPacketHeader)),
            ),
            (
                &[0xff, 0x60, 0, 1],
                &[],
                Some((0, MalformedKind::InvalidMarkerLength)),
            ),
            (
                &[0xff, 0x60, 0, 2],
                &[],
                Some((0, MalformedKind::InvalidMarkerLength)),
            ),
            (&[0xff, 0x60, 0, 3, 0], &[], None),
            (
                &[
                    0xff, 0x60, 0, 5, 0, 10, 11, 0xff, 0x55, 0, 2, 0xff, 0x60, 0, 4, 9, 12,
                ],
                &[10, 11, 12],
                None,
            ),
            (
                &[0xff, 0x60, 0, 4, 0, 10, 0xff],
                &[10],
                Some((6, MalformedKind::InvalidPackedPacketHeader)),
            ),
            (
                &[0xff, 0x60, 0, 4, 0],
                &[],
                Some((0, MalformedKind::InvalidMarkerLength)),
            ),
        ];
        for (header, flat, parse_error) in fixtures {
            for logical_start in [0, 1, 2, 3, 4, usize::MAX] {
                for length in 0..=5 {
                    for fragments in [0, 1, u16::MAX] {
                        for cap in [
                            1,
                            2,
                            5,
                            ResourceLimits::default().maximum_packet_header_bytes(),
                        ] {
                            let limits = ResourceLimits::default()
                                .with_maximum_packet_header_bytes(cap)
                                .unwrap();
                            let packed = PackedPacketHeaders {
                                source: PackedPacketHeaderSource::MarkerSegments {
                                    header,
                                    marker: 0xff60,
                                    logical_start,
                                },
                                length,
                                fragments,
                            };
                            let expected = if length as u64 > cap {
                                Err(DecodeError::LimitExceeded {
                                    resource: Resource::PacketHeaderBytes,
                                    limit: cap,
                                    actual: length as u64,
                                })
                            } else if logical_start.checked_add(length).is_none() {
                                Err(input_error_literal(0, MalformedKind::ArithmeticOverflow))
                            } else if let Some((offset, kind)) = parse_error {
                                Err(input_error_literal(*offset, *kind))
                            } else if length == 0 {
                                Ok(Vec::new())
                            } else if let Some(bytes) =
                                flat.get(logical_start..logical_start + length)
                            {
                                Ok(bytes.to_vec())
                            } else {
                                Err(input_error_literal(
                                    flat.len(),
                                    MalformedKind::InvalidPackedPacketHeader,
                                ))
                            };
                            assert!(packed.is_present());
                            assert_eq!(
                                (packed.len(), packed.is_empty(), packed.fragment_count()),
                                (length, length == 0, fragments)
                            );
                            assert_eq!(packed.reassemble(&limits), expected);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn packed_raw_source_marker_and_absent_metadata_do_not_invent_shape_invariants() {
        let header = [0xff, 0x60, 0, 2];
        let limits = ResourceLimits::default();
        for marker in 0..=u16::MAX {
            let packed = PackedPacketHeaders {
                source: PackedPacketHeaderSource::MarkerSegments {
                    header: &header,
                    marker,
                    logical_start: usize::MAX,
                },
                length: 0,
                fragments: u16::MAX,
            };
            let expected = if marker == 0xff60 {
                Err(input_error_literal(0, MalformedKind::InvalidMarkerLength))
            } else {
                Ok(Vec::new())
            };
            assert_eq!(packed.reassemble(&limits), expected);
        }
        for cap in [1, ResourceLimits::default().maximum_packet_header_bytes()] {
            let limits = limits.with_maximum_packet_header_bytes(cap).unwrap();
            for length in [0, 1, usize::MAX] {
                let packed = PackedPacketHeaders {
                    source: PackedPacketHeaderSource::None,
                    length,
                    fragments: u16::MAX,
                };
                assert!(!packed.is_present());
                assert_eq!(
                    (packed.len(), packed.is_empty(), packed.fragment_count()),
                    (length, length == 0, u16::MAX)
                );
                let expected = if length as u64 > cap {
                    Err(DecodeError::LimitExceeded {
                        resource: Resource::PacketHeaderBytes,
                        limit: cap,
                        actual: length as u64,
                    })
                } else {
                    Ok(Vec::new())
                };
                assert_eq!(packed.reassemble(&limits), expected);
            }
        }
        let packed = PackedPacketHeaders {
            source: PackedPacketHeaderSource::MarkerSegments {
                header: &[],
                marker: 0,
                logical_start: usize::MAX,
            },
            length: 1,
            fragments: 0,
        };
        assert_eq!(
            packed.reassemble(&limits),
            Err(input_error_literal(0, MalformedKind::ArithmeticOverflow))
        );
        assert_eq!(packed_packet_headers_none(), PackedPacketHeaders::none());
        assert_eq!(packet_lengths_none(), PacketLengths::none());
    }

    #[test]
    fn raw_progression_owner_records_preserve_inverted_windows_duplicates_and_boundaries() {
        let orders = [
            ProgressionOrder::Lrcp,
            ProgressionOrder::Rlcp,
            ProgressionOrder::Rpcl,
            ProgressionOrder::Pcrl,
            ProgressionOrder::Cprl,
        ];
        let mut records = Vec::new();
        for progression_order in orders {
            for resolution_start in [0, 1, u8::MAX] {
                for resolution_end in [0, 1, u8::MAX] {
                    for component_start in [0, 1, u16::MAX] {
                        for component_end in [0, 1, u16::MAX] {
                            for layer_end in [0, 1, u16::MAX] {
                                let record = ProgressionChange {
                                    resolution_start,
                                    component_start,
                                    layer_end,
                                    resolution_end,
                                    component_end,
                                    progression_order,
                                };
                                for layer in [0, 1, u16::MAX] {
                                    for resolution in [0, 1, u8::MAX] {
                                        for component in [0, 1, u16::MAX] {
                                            let expected = (0..layer_end).contains(&layer)
                                                && (resolution_start..resolution_end)
                                                    .contains(&resolution)
                                                && (component_start..component_end)
                                                    .contains(&component);
                                            assert_eq!(
                                                record.contains(layer, resolution, component),
                                                expected
                                            );
                                        }
                                    }
                                }
                                records.extend_from_slice(&[record, record]);
                            }
                        }
                    }
                }
            }
        }
        let changes = ProgressionChanges::from_records(records.clone());
        assert_eq!(changes.records(), records);
        assert_eq!(changes.len(), records.len());
        assert!(changes.is_present());
        assert!(!changes.is_empty());
        let empty = ProgressionChanges::from_records(Vec::new());
        assert!(empty.records().is_empty());
        assert_eq!(empty.len(), 0);
        assert!(!empty.is_present());
        assert!(empty.is_empty());
    }

    #[test]
    fn complete_packet_style_owners_preserve_raw_counts_styles_and_header_presence() {
        let mut style = CodingStyle {
            progression_order: ProgressionOrder::Lrcp,
            layers: 0,
            multiple_component_transform: true,
            decomposition_levels: u8::MAX,
            decomposition_structure: Some(u8::MAX),
            decomposition_directions: [DecompositionDirection::None; 32],
            code_block_width: 0,
            code_block_height: u16::MAX,
            code_block_style: u8::MAX,
            transform: WaveletTransform::Arbitrary(u8::MAX),
            precincts_defined: false,
            sop_markers: true,
            eph_markers: true,
            precinct_exponents: [(u8::MAX, u8::MAX); 33],
        };
        let styles = core::array::from_fn(|index| CodingStyle {
            layers: u16::try_from(index).unwrap() * 999,
            code_block_style: u8::try_from(index).unwrap(),
            ..style
        });
        let mut table = ComponentCodingStyles {
            components: 0,
            styles,
        };
        for count in 0..=u16::MAX {
            table.components = count;
            for index in [0, 1, 63, 64, 65, u16::MAX - 1, u16::MAX] {
                let expected = if index < count && usize::from(index) < styles.len() {
                    Ok(styles[usize::from(index)])
                } else {
                    Err(DecodeError::MalformedInput {
                        offset: 0,
                        kind: MalformedKind::InvalidCodingStyle,
                    })
                };
                assert_eq!(table.style(index), expected);
            }
        }
        for count in [0, 1, 63, 64, 65, u16::MAX] {
            table.components = count;
            for index in 0..=u16::MAX {
                let expected = if index < count && usize::from(index) < styles.len() {
                    Ok(styles[usize::from(index)])
                } else {
                    Err(DecodeError::MalformedInput {
                        offset: 0,
                        kind: MalformedKind::InvalidCodingStyle,
                    })
                };
                assert_eq!(table.style(index), expected);
            }
        }
        for value in 0..=u8::MAX {
            style.code_block_style = value;
            assert_eq!(
                style.tier1_coding_style(),
                Tier1CodingStyle::from_code_block_style(value)
            );
        }
        for length in [0, 1, usize::MAX] {
            for fragments in [0, 1, u16::MAX] {
                let absent = PackedPacketHeaders {
                    source: PackedPacketHeaderSource::None,
                    length,
                    fragments,
                };
                assert!(!absent.is_present());
                for bytes in [&[][..], &[0xff, 0x60][..]] {
                    for marker in [0, u16::MAX] {
                        for logical_start in [0, usize::MAX] {
                            let present = PackedPacketHeaders {
                                source: PackedPacketHeaderSource::MarkerSegments {
                                    header: bytes,
                                    marker,
                                    logical_start,
                                },
                                length,
                                fragments,
                            };
                            assert!(present.is_present());
                        }
                    }
                }
            }
        }
    }

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
    fn main_header_quantization_may_precede_coding_style() {
        let [cod, qcd] = baseline_headers();
        let canonical = codestream_with_headers(1, &[cod.clone(), qcd.clone()], &[], &[]);
        let reordered = codestream_with_headers(1, &[qcd, cod], &[], &[]);
        let limits = ResourceLimits::default();
        let canonical = parse_layout(&canonical, &limits).unwrap();
        let reordered = parse_layout(&reordered, &limits).unwrap();
        assert_eq!(reordered.info.coding_style, canonical.info.coding_style);
        assert_eq!(reordered.info.quantization, canonical.info.quantization);
    }

    #[test]
    fn reordered_main_header_quantization_is_still_validated() {
        let [cod, qcd] = baseline_headers();
        let duplicate = codestream_with_headers(1, &[qcd.clone(), cod.clone(), qcd], &[], &[]);
        assert!(matches!(
            parse_layout(&duplicate, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerOrder,
                ..
            })
        ));
        let malformed = codestream_with_headers(1, &[segment(QCD, &[0]), cod], &[], &[]);
        assert!(matches!(
            parse_layout(&malformed, &ResourceLimits::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidMarkerLength,
                ..
            })
        ));
    }

    #[test]
    fn actual_nonlinear_getters_expose_complete_raw_descriptors_without_validation() {
        let mut cases = 0;
        for component in [None, Some(0), Some(u16::MAX)] {
            for precision in [0, 1, 38, 39, u8::MAX] {
                for transform_type in [0, 1, 2, 3, 4, u8::MAX] {
                    for signed in [false, true] {
                        let transform = NonlinearPointTransform {
                            component,
                            precision,
                            signed,
                            transform_type,
                            parameters: if signed { vec![] } else { vec![0, u8::MAX, 17] },
                            d_min: Some(u32::MAX),
                            d_max: None,
                            point_bits: Some(u8::MAX),
                            points: if signed {
                                vec![u32::MAX, 0, 17]
                            } else {
                                vec![]
                            },
                            gamma_parameters: Some([0, 17, u32::MAX, 3, 1]),
                        };
                        assert_eq!(transform.component(), component);
                        assert_eq!(transform.precision(), precision);
                        assert_eq!(transform.signed(), signed);
                        assert_eq!(transform.transform_type(), transform_type);
                        assert_eq!(transform.parameters(), transform.parameters);
                        assert_eq!(transform.d_min(), Some(u32::MAX));
                        assert_eq!(transform.d_max(), None);
                        assert_eq!(transform.point_bits(), Some(u8::MAX));
                        assert_eq!(transform.points(), transform.points);
                        assert_eq!(transform.gamma_parameters(), Some([0, 17, u32::MAX, 3, 1]));
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 180);
    }

    #[test]
    fn actual_nonlinear_lookup_preserves_first_specific_then_first_default_and_all_raw_fields() {
        let selectors = [
            vec![],
            vec![None],
            vec![Some(0)],
            vec![Some(u16::MAX)],
            vec![None, Some(0)],
            vec![None, None, Some(0), Some(0)],
            vec![Some(1), Some(u16::MAX), None, Some(0)],
            vec![Some(0), None, Some(0), None],
            vec![None, Some(1), None],
        ];
        let mut cases = 0;
        for selectors in selectors {
            let extensions = Part2Extensions {
                nonlinear_transforms: selectors
                    .iter()
                    .enumerate()
                    .map(|(index, component)| NonlinearPointTransform {
                        component: *component,
                        precision: 0,
                        signed: index % 2 == 0,
                        transform_type: u8::MAX,
                        parameters: vec![0xa5, index as u8],
                        d_min: Some(u32::MAX),
                        d_max: Some(index as u32),
                        point_bits: Some(u8::MAX),
                        points: vec![u32::MAX, 0, index as u32],
                        gamma_parameters: Some([u32::MAX, 17, index as u32, 0, 1]),
                    })
                    .collect(),
                ..Part2Extensions::default()
            };
            for component in 0..=u16::MAX {
                let mut specific = None;
                let mut default = None;
                for (index, selector) in selectors.iter().enumerate() {
                    if *selector == Some(component) && specific.is_none() {
                        specific = Some(index);
                    }
                    if selector.is_none() && default.is_none() {
                        default = Some(index);
                    }
                }
                let expected = specific
                    .or(default)
                    .map(|index| &extensions.nonlinear_transforms[index]);
                let actual = extensions.nonlinear_transform_for(component);
                assert_eq!(actual, expected);
                if let (Some(actual), Some(expected)) = (actual, expected) {
                    assert!(core::ptr::eq(actual, expected));
                }
                cases += 1;
            }
        }
        assert_eq!(cases, 589_824);
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
    fn actual_component_index_preserves_width_threshold_error_kind_and_marker_independence() {
        let limits = ResourceLimits::HARDENED;
        let parser = new_parser(&[], &limits).unwrap();
        let mut cases = 0;
        for components in [0, 1, 256, 257, u16::MAX] {
            let width = if components < 257 { 1 } else { 2 };
            for word in 0..=u16::MAX {
                let [high, low] = word.to_be_bytes();
                let bytes = [high, low, 0xa5];
                let component = if width == 1 { u16::from(high) } else { word };
                for length in 0..=bytes.len() {
                    for kind in [
                        MalformedKind::InvalidCodingStyle,
                        MalformedKind::InvalidQuantization,
                    ] {
                        for offset in [0, usize::MAX] {
                            let expected = if length < width {
                                Err(DecodeError::MalformedInput {
                                    offset: offset as u64,
                                    kind: MalformedKind::InvalidMarkerLength,
                                })
                            } else if component >= components {
                                Err(DecodeError::MalformedInput {
                                    offset: offset as u64,
                                    kind,
                                })
                            } else {
                                Ok((component, width))
                            };
                            let segment = Segment {
                                marker: if high % 2 == 0 { 0xff53 } else { 0x1234 },
                                offset,
                                payload: &bytes[..length],
                            };
                            assert_eq!(
                                parser.parse_component_index(segment, components, kind),
                                expected
                            );
                            cases += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 5_242_880);
    }

    #[test]
    fn actual_raw_component_styles_preserve_source_defaults_selectors_and_first_errors() {
        fn reference(
            bytes: &[u8],
            offset: usize,
            components: u16,
            default: CodingStyle,
            limits: &ResourceLimits,
        ) -> Result<(u16, CodingStyle), DecodeError> {
            let malformed = |kind| DecodeError::MalformedInput {
                offset: offset as u64,
                kind,
            };
            let width = if components < 257 { 1 } else { 2 };
            if bytes.len() < width {
                return Err(malformed(MalformedKind::InvalidMarkerLength));
            }
            let component = if width == 1 {
                u16::from(bytes[0])
            } else {
                u16::from_be_bytes([bytes[0], bytes[1]])
            };
            if component >= components {
                return Err(malformed(MalformedKind::InvalidCodingStyle));
            }
            if bytes.len() < width + 6 {
                return Err(malformed(MalformedKind::InvalidMarkerLength));
            }
            let scoc = bytes[width];
            if scoc > 1 {
                return Err(malformed(MalformedKind::InvalidCodingStyle));
            }
            let parameters = &bytes[width + 1..];
            let structure = if parameters[0] >= 128 {
                let index = parameters[0] - 128;
                if index == 0 {
                    return Err(malformed(MalformedKind::InvalidCodingStyle));
                }
                Some(index)
            } else {
                None
            };
            if parameters.len() > 38 {
                return Err(malformed(MalformedKind::InvalidMarkerLength));
            }
            let mut cod = vec![
                scoc + 2 * u8::from(default.sop_markers) + 4 * u8::from(default.eph_markers),
                match default.progression_order {
                    ProgressionOrder::Lrcp => 0,
                    ProgressionOrder::Rlcp => 1,
                    ProgressionOrder::Rpcl => 2,
                    ProgressionOrder::Pcrl => 3,
                    ProgressionOrder::Cprl => 4,
                },
            ];
            cod.extend(default.layers.to_be_bytes());
            cod.push(u8::from(default.multiple_component_transform));
            cod.extend(parameters);
            if structure.is_some() {
                cod[5] = default.decomposition_levels;
            }
            let mut style = reference_raw_coding_style(&cod, offset, limits)?;
            style.decomposition_structure = structure;
            Ok((component, style))
        }

        let base = ResourceLimits::HARDENED;
        let limits = [
            base,
            base.with_coding_style_limits_for_test(0, 0),
            base.with_coding_style_limits_for_test(u16::MAX, u8::MAX),
        ];
        let seed = reference_raw_coding_style(&[6, 0, 0, 1, 0, 2, 0, 0, 0, 1], 0, &base).unwrap();
        let orders = [
            ProgressionOrder::Lrcp,
            ProgressionOrder::Rlcp,
            ProgressionOrder::Rpcl,
            ProgressionOrder::Pcrl,
            ProgressionOrder::Cprl,
        ];
        let mut defaults = Vec::new();
        for (index, order) in orders.into_iter().enumerate() {
            let mut default = seed;
            default.progression_order = order;
            default.layers = [0, 1, 256, u16::MAX, 2][index];
            default.multiple_component_transform = index % 2 == 1;
            default.sop_markers = index % 2 == 1;
            default.eph_markers = index / 2 % 2 == 1;
            default.decomposition_levels = [0, 1, 32, 255, 2][index];
            default.decomposition_structure = Some(127);
            default.code_block_width = 0;
            default.code_block_height = 0;
            default.code_block_style = 255;
            default.transform = WaveletTransform::Arbitrary(255);
            default.precincts_defined = true;
            default.precinct_exponents = [(0, 0); 33];
            defaults.push(default);
        }
        defaults.push(seed);
        let mut cases = 0;
        let mut successes = 0;
        for default in &defaults {
            for components in [0, 1, 256, 257, u16::MAX] {
                let width = if components < 257 { 1 } else { 2 };
                for variant in 0..3 {
                    let mut source = vec![0; width];
                    source.push(u8::from(variant == 2));
                    source.extend([if variant == 1 { 0x81 } else { 0 }, 0, 0, 0, 1]);
                    if variant == 2 {
                        source.push(0x21);
                    }
                    for position in 0..9 {
                        for byte in 0..=u8::MAX {
                            let mut bytes = source.clone();
                            bytes.resize(bytes.len().max(position + 1), 0);
                            bytes[position] = byte;
                            for limit in &limits {
                                let parser = new_parser(&[], limit).unwrap();
                                for offset in [0, usize::MAX] {
                                    let expected =
                                        reference(&bytes, offset, components, *default, limit);
                                    let actual = parser.parse_coc(
                                        Segment {
                                            marker: 0x1234,
                                            offset,
                                            payload: &bytes,
                                        },
                                        components,
                                        *default,
                                    );
                                    assert_eq!(actual, expected);
                                    successes += usize::from(actual.is_ok());
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 1_244_160);
        assert!(successes > 1_000);
        let mut frontiers = 0;
        for default in &defaults {
            for components in [0, 1, 256, 257, u16::MAX] {
                let width = if components < 257 { 1 } else { 2 };
                for length in 0..=45 {
                    for scoc in [0, 1, 2, 255] {
                        for selector in [0, 128, 129, 255] {
                            let mut bytes = vec![0; length];
                            if let Some(value) = bytes.get_mut(width) {
                                *value = scoc;
                            }
                            if let Some(value) = bytes.get_mut(width + 1) {
                                *value = selector;
                            }
                            for limit in &limits {
                                let parser = new_parser(&[], limit).unwrap();
                                for offset in [0, usize::MAX] {
                                    assert_eq!(
                                        parser.parse_coc(
                                            Segment {
                                                marker: 0xff53,
                                                offset,
                                                payload: &bytes
                                            },
                                            components,
                                            *default,
                                        ),
                                        reference(&bytes, offset, components, *default, limit),
                                    );
                                    frontiers += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(frontiers, 132_480);
        let payload = [0, 0, 1, 0x81, 0, 0, 0, 1, 0x11, 0x22, 0x33];
        let mut input = vec![255, 83];
        input.extend(u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
        input.extend(payload);
        let mut parser = new_parser(&input, &base).unwrap();
        let segment = parser.read_segment().unwrap();
        let actual = parser.parse_coc(segment, 257, seed).unwrap();
        assert_eq!(Ok(actual), reference(&payload, 0, 257, seed, &base));
        assert_eq!(actual.1.decomposition_structure, Some(1));
        assert_eq!(actual.1.decomposition_levels, 2);
        assert_eq!(actual.1.precinct_exponents[..3], [(1, 1), (2, 2), (3, 3)]);
        assert_eq!(parser.position, input.len());
        assert_eq!(parser.work_units, 1);
    }

    fn reference_raw_coding_style(
        bytes: &[u8],
        offset: usize,
        limits: &ResourceLimits,
    ) -> Result<CodingStyle, DecodeError> {
        fn malformed(offset: usize, kind: MalformedKind) -> DecodeError {
            DecodeError::MalformedInput {
                offset: offset as u64,
                kind,
            }
        }
        if bytes.len() < 10 {
            return Err(malformed(offset, MalformedKind::InvalidMarkerLength));
        }
        let flags = bytes[0];
        if flags >= 8 {
            return Err(malformed(offset, MalformedKind::InvalidCodingStyle));
        }
        let order = [
            ProgressionOrder::Lrcp,
            ProgressionOrder::Rlcp,
            ProgressionOrder::Rpcl,
            ProgressionOrder::Pcrl,
            ProgressionOrder::Cprl,
        ]
        .get(usize::from(bytes[1]))
        .copied()
        .ok_or_else(|| malformed(offset, MalformedKind::InvalidCodingStyle))?;
        let layers = u16::from_be_bytes([bytes[2], bytes[3]]);
        if layers > limits.maximum_layers() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Layers,
                limit: u64::from(limits.maximum_layers()),
                actual: u64::from(layers),
            });
        }
        if layers == 0 || bytes[4] > 1 {
            return Err(malformed(offset, MalformedKind::InvalidCodingStyle));
        }
        let resolutions = u16::from(bytes[5]) + 1;
        if resolutions > u16::from(u8::MAX) {
            return Err(malformed(offset, MalformedKind::ArithmeticOverflow));
        }
        if resolutions > u16::from(limits.maximum_resolutions_per_component()) {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Resolutions,
                limit: u64::from(limits.maximum_resolutions_per_component()),
                actual: u64::from(resolutions),
            });
        }
        if bytes[6] > 8 || bytes[7] > 8 {
            return Err(malformed(offset, MalformedKind::InvalidCodingStyle));
        }
        let width = 2_u32.pow(u32::from(bytes[6]) + 2);
        let height = 2_u32.pow(u32::from(bytes[7]) + 2);
        let style_valid = match bytes[8] / 64 {
            0 => true,
            1 => (bytes[8] % 64) & 0x37 == 0,
            2 => false,
            _ => (bytes[8] % 64) & 5 == 0,
        };
        if u64::from(width) * u64::from(height) > 4096 || !style_valid {
            return Err(malformed(offset, MalformedKind::InvalidCodingStyle));
        }
        let defined = flags % 2 == 1;
        if bytes.len() != 10 + if defined { usize::from(resolutions) } else { 0 } {
            return Err(malformed(offset, MalformedKind::InvalidMarkerLength));
        }
        let mut precincts = [(15, 15); 33];
        if defined {
            for (index, byte) in bytes[10..].iter().enumerate() {
                let slot = precincts
                    .get_mut(index)
                    .ok_or_else(|| malformed(offset, MalformedKind::InvalidCodingStyle))?;
                *slot = (byte % 16, byte / 16);
            }
        }
        Ok(CodingStyle {
            progression_order: order,
            layers,
            multiple_component_transform: bytes[4] == 1,
            decomposition_levels: bytes[5],
            decomposition_structure: None,
            decomposition_directions: [DecompositionDirection::TwoDimensional; 32],
            code_block_width: u16::try_from(width).unwrap(),
            code_block_height: u16::try_from(height).unwrap(),
            code_block_style: bytes[8],
            transform: match bytes[9] {
                0 => WaveletTransform::Irreversible9x7,
                1 => WaveletTransform::Reversible5x3,
                index => WaveletTransform::Arbitrary(index),
            },
            precincts_defined: defined,
            sop_markers: flags / 2 % 2 == 1,
            eph_markers: flags / 4 % 2 == 1,
            precinct_exponents: precincts,
        })
    }

    #[test]
    fn actual_raw_cod_derives_full_style_defaults_precincts_and_ordered_admission() {
        fn malformed(offset: usize, kind: MalformedKind) -> DecodeError {
            DecodeError::MalformedInput {
                offset: offset as u64,
                kind,
            }
        }
        fn payload(header: [u8; 10], variant: usize) -> Vec<u8> {
            let mut bytes = header.to_vec();
            if header[0] % 2 == 1 {
                bytes.extend(
                    (0..usize::from(header[5]) + 1).map(|index| (index as u8).wrapping_mul(17)),
                );
            }
            if variant == 1 {
                bytes.truncate(9);
            } else if variant == 2 {
                bytes.push(0);
            }
            bytes
        }
        let base = ResourceLimits::HARDENED;
        let limits = [
            base,
            base.with_coding_style_limits_for_test(0, 33),
            base.with_coding_style_limits_for_test(1, 33),
            base.with_coding_style_limits_for_test(u16::MAX, 0),
            base.with_coding_style_limits_for_test(u16::MAX, 1),
            base.with_coding_style_limits_for_test(u16::MAX, 33),
            base.with_coding_style_limits_for_test(u16::MAX, u8::MAX),
        ];
        let mut cases = 0;
        let mut successes = 0;
        for flags in [0, 1, 7] {
            for index in 0..10 {
                for byte in 0..=u8::MAX {
                    let mut header = [flags, 0, 0, 1, 0, 0, 0, 0, 0, 1];
                    header[index] = byte;
                    for variant in 0..3 {
                        let bytes = payload(header, variant);
                        for limit in &limits {
                            let parser = new_parser(&[], limit).unwrap();
                            for offset in [0, 17, usize::MAX] {
                                let expected = reference_raw_coding_style(&bytes, offset, limit);
                                assert_eq!(parser.parse_cod_payload(offset, &bytes), expected);
                                assert_eq!(
                                    parser.parse_cod(Segment {
                                        marker: 0x1234,
                                        offset,
                                        payload: &bytes
                                    }),
                                    expected
                                );
                                successes += usize::from(expected.is_ok());
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 483_840);
        assert!(successes > 1_000);
        let parser = new_parser(&[], &base).unwrap();
        for width in 0..10 {
            for height in 0..10 {
                let bytes = [0, 0, 0, 1, 0, 0, width, height, 0, 1];
                assert_eq!(
                    parser.parse_cod_payload(31, &bytes),
                    reference_raw_coding_style(&bytes, 31, &base)
                );
            }
        }
        let bytes = payload([7, 4, 0, 2, 1, 2, 4, 4, 0xc0, 255], 0);
        let mut input = vec![255, 82];
        input.extend(u16::try_from(bytes.len() + 2).unwrap().to_be_bytes());
        input.extend(&bytes);
        let mut parser = new_parser(&input, &base).unwrap();
        let segment = parser.read_segment().unwrap();
        let actual = parser.parse_cod(segment).unwrap();
        assert_eq!(
            actual,
            reference_raw_coding_style(&bytes, 0, &base).unwrap()
        );
        assert!(matches!(actual.transform, WaveletTransform::Arbitrary(255)));
        assert!(
            actual.precinct_exponents[3..]
                .iter()
                .all(|entry| *entry == (15, 15))
        );
        assert_eq!(parser.work_units, 1);
        assert_eq!(parser.position, input.len());
        let raw = base.with_coding_style_limits_for_test(u16::MAX, u8::MAX);
        let parser = new_parser(&[], &raw).unwrap();
        let no_precincts = payload([0, 0, 255, 255, 0, 254, 0, 0, 0, 255], 0);
        assert_eq!(
            parser
                .parse_cod_payload(0, &no_precincts)
                .unwrap()
                .decomposition_levels,
            254
        );
        let explicit = payload([1, 0, 0, 1, 0, 254, 0, 0, 0, 255], 0);
        assert_eq!(
            parser.parse_cod_payload(0, &explicit),
            Err(malformed(0, MalformedKind::InvalidCodingStyle))
        );
        let overflow = payload([0, 0, 0, 1, 0, 255, 0, 0, 0, 1], 0);
        assert_eq!(
            parser.parse_cod_payload(0, &overflow),
            Err(malformed(0, MalformedKind::ArithmeticOverflow))
        );
    }

    #[test]
    fn actual_raw_siz_derives_every_image_field_uniform_policy_unused_slot_and_first_limit_error() {
        fn malformed(offset: usize, kind: MalformedKind) -> DecodeError {
            DecodeError::MalformedInput {
                offset: offset as u64,
                kind,
            }
        }
        fn payload(geometry: [u32; 8], components: &[[u8; 3]], r_siz: u16) -> Vec<u8> {
            let mut bytes = r_siz.to_be_bytes().to_vec();
            for coordinate in geometry {
                bytes.extend(coordinate.to_be_bytes());
            }
            bytes.extend(u16::try_from(components.len()).unwrap().to_be_bytes());
            for component in components {
                bytes.extend(component);
            }
            bytes
        }
        fn reference(
            bytes: &[u8],
            offset: usize,
            limits: &ResourceLimits,
        ) -> Result<(ImageSize, u16), DecodeError> {
            if bytes.len() < 36 {
                return Err(malformed(offset, MalformedKind::InvalidMarkerLength));
            }
            let mut words = bytes[2..34]
                .chunks_exact(4)
                .map(|word| u32::from_be_bytes(word.try_into().unwrap()));
            let xsiz = words.next().unwrap();
            let ysiz = words.next().unwrap();
            let xosiz = words.next().unwrap();
            let yosiz = words.next().unwrap();
            let xtsiz = words.next().unwrap();
            let ytsiz = words.next().unwrap();
            let xtosiz = words.next().unwrap();
            let ytosiz = words.next().unwrap();
            let count = u16::from_be_bytes([bytes[34], bytes[35]]);
            let first_x = u64::from(xtosiz) + u64::from(xtsiz);
            let first_y = u64::from(ytosiz) + u64::from(ytsiz);
            let component_bytes = u128::from(count) * 3;
            if first_x > u64::from(u32::MAX)
                || first_y > u64::from(u32::MAX)
                || component_bytes > usize::MAX as u128
                || component_bytes + 36 > usize::MAX as u128
            {
                return Err(malformed(offset, MalformedKind::ArithmeticOverflow));
            }
            if bytes.len() as u128 != 36 + component_bytes
                || xsiz <= xosiz
                || ysiz <= yosiz
                || xtsiz == 0
                || ytsiz == 0
                || xtosiz > xosiz
                || ytosiz > yosiz
                || first_x <= u64::from(xosiz)
                || first_y <= u64::from(yosiz)
                || count == 0
            {
                return Err(malformed(offset, MalformedKind::InvalidImageGeometry));
            }
            if count > limits.maximum_components() {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::Components,
                    limit: u64::from(limits.maximum_components()),
                    actual: u64::from(count),
                });
            }
            let mut infos = [ComponentInfo::default(); MAX_COMPONENTS];
            let mut first = None;
            let mut uniform = true;
            for (index, entry) in bytes[36..].chunks_exact(3).enumerate() {
                let precision = u16::from(entry[0] % 128) + 1;
                let signed = entry[0] >= 128;
                if precision > 38 || entry[1] == 0 || entry[2] == 0 || index >= infos.len() {
                    return Err(malformed(offset, MalformedKind::InvalidImageGeometry));
                }
                let precision = u8::try_from(precision).unwrap();
                infos[index] = ComponentInfo {
                    precision,
                    signed,
                    x_subsampling: entry[1],
                    y_subsampling: entry[2],
                    x_registration: 0,
                    y_registration: 0,
                };
                match first {
                    None => first = Some((precision, signed)),
                    Some(expected) if expected != (precision, signed) => uniform = false,
                    _ => {}
                }
            }
            let width = xsiz - xosiz;
            let height = ysiz - yosiz;
            if width > limits.maximum_width() {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::Width,
                    limit: u64::from(limits.maximum_width()),
                    actual: u64::from(width),
                });
            }
            if height > limits.maximum_height() {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::Height,
                    limit: u64::from(limits.maximum_height()),
                    actual: u64::from(height),
                });
            }
            let pixels = u128::from(width) * u128::from(height);
            if pixels > u128::from(u64::MAX) {
                return Err(malformed(offset, MalformedKind::ArithmeticOverflow));
            }
            if pixels > u128::from(limits.maximum_pixels()) {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::Pixels,
                    limit: limits.maximum_pixels(),
                    actual: pixels as u64,
                });
            }
            let across = u64::from(xsiz - xtosiz).div_ceil(u64::from(xtsiz));
            let down = u64::from(ysiz - ytosiz).div_ceil(u64::from(ytsiz));
            let tiles = u128::from(across) * u128::from(down);
            if tiles > u128::from(u64::MAX) {
                return Err(malformed(offset, MalformedKind::ArithmeticOverflow));
            }
            if tiles > u128::from(limits.maximum_tiles()) {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::Tiles,
                    limit: u64::from(limits.maximum_tiles()),
                    actual: tiles as u64,
                });
            }
            let first = first.unwrap();
            Ok((
                ImageSize {
                    width,
                    height,
                    x_origin: xosiz,
                    y_origin: yosiz,
                    components: count,
                    uniform_bit_depth: if uniform { Some(first.0) } else { None },
                    uniform_signed: if uniform { Some(first.1) } else { None },
                    tile_width: xtsiz,
                    tile_height: ytsiz,
                    tile_x_origin: xtosiz,
                    tile_y_origin: ytosiz,
                    tiles_across: u32::try_from(across).unwrap(),
                    tiles_down: u32::try_from(down).unwrap(),
                    component_info: infos,
                },
                u16::from_be_bytes([bytes[0], bytes[1]]),
            ))
        }
        let geometries = [
            [17, 11, 3, 1, 8, 6, 0, 0],
            [u32::MAX, u32::MAX, 0, 0, u32::MAX, u32::MAX, 0, 0],
            [0; 8],
            [3, 11, 3, 1, 8, 6, 0, 0],
            [17, 1, 3, 1, 8, 6, 0, 0],
            [17, 11, 3, 1, 0, 6, 0, 0],
            [17, 11, 3, 1, 8, 0, 0, 0],
            [17, 11, 3, 1, 8, 6, 4, 0],
            [17, 11, 3, 1, 8, 6, 0, 2],
            [17, 11, 3, 1, 2, 6, 0, 0],
            [17, 11, 3, 1, 8, 1, 0, 0],
            [17, 11, 3, 1, 1, 6, u32::MAX, 0],
            [17, 11, 3, 1, 8, 1, 0, u32::MAX],
            [1_000, 1_000, 0, 0, 1, 1, 0, 0],
            [1, 1, 0, 0, 1, 1, 0, 0],
            [17, 11, 3, 1, 8, 6, 3, 1],
        ];
        let components = [
            vec![],
            vec![[7, 1, 1]],
            vec![[0x87, 2, 3]],
            vec![[7, 1, 1], [8, 3, 2]],
            vec![[7, 1, 1], [0x87, 1, 1]],
            vec![[37, 255, 255]],
            vec![[38, 1, 1]],
            vec![[255, 1, 1]],
            vec![[7, 0, 1]],
            vec![[7, 1, 0]],
            vec![[7, 1, 1], [8, 1, 1], [7, 1, 1]],
            vec![[7, 1, 1]; MAX_COMPONENTS],
            vec![[7, 1, 1]; MAX_COMPONENTS + 1],
        ];
        let base = ResourceLimits::HARDENED;
        let limits = [
            base,
            base.with_geometry_limits_for_test(
                1,
                base.maximum_width(),
                base.maximum_height(),
                base.maximum_pixels(),
                base.maximum_tiles(),
            ),
            base.with_geometry_limits_for_test(
                base.maximum_components(),
                8,
                base.maximum_height(),
                base.maximum_pixels(),
                base.maximum_tiles(),
            ),
            base.with_geometry_limits_for_test(
                base.maximum_components(),
                base.maximum_width(),
                4,
                base.maximum_pixels(),
                base.maximum_tiles(),
            ),
            base.with_geometry_limits_for_test(
                base.maximum_components(),
                base.maximum_width(),
                base.maximum_height(),
                16,
                base.maximum_tiles(),
            ),
            base.with_geometry_limits_for_test(
                base.maximum_components(),
                base.maximum_width(),
                base.maximum_height(),
                base.maximum_pixels(),
                2,
            ),
            base.with_geometry_limits_for_test(u16::MAX, 8, 4, 16, 2),
        ];
        let mut cases = 0;
        let mut successes = 0;
        for geometry in geometries {
            for entries in &components {
                for r_siz in [0, 1, 0x4000, u16::MAX] {
                    for variant in 0..4 {
                        let mut bytes = payload(geometry, entries, r_siz);
                        match variant {
                            0 => {}
                            1 => bytes.truncate(35),
                            2 => bytes.push(0),
                            _ => bytes[34..36].copy_from_slice(&u16::MAX.to_be_bytes()),
                        }
                        for limit in &limits {
                            let parser = new_parser(&[], limit).unwrap();
                            for offset in [0, 17, usize::MAX] {
                                let expected = reference(&bytes, offset, limit);
                                assert_eq!(
                                    parser.parse_siz(Segment {
                                        marker: 0x1234,
                                        offset,
                                        payload: &bytes
                                    }),
                                    expected
                                );
                                successes += usize::from(expected.is_ok());
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 69_888);
        assert!(successes > 100);
        let mut descriptor_cases = 0;
        let limit = ResourceLimits::HARDENED;
        let parser = new_parser(&[], &limit).unwrap();
        for ssiz in 0..=u8::MAX {
            for x in [0, 1, u8::MAX] {
                for y in [0, 1, u8::MAX] {
                    let bytes = payload(geometries[0], &[[ssiz, x, y]], u16::MAX);
                    assert_eq!(
                        parser.parse_siz(Segment {
                            marker: SIZ,
                            offset: 19,
                            payload: &bytes
                        }),
                        reference(&bytes, 19, &limit)
                    );
                    descriptor_cases += 1;
                }
            }
        }
        assert_eq!(descriptor_cases, 2_304);
        let mut words = 0;
        for base in [
            [0; 4],
            [u8::MAX; 4],
            [0x12, 0x34, 0x56, 0x78],
            [0x80, 0, 0, 1],
        ] {
            for index in 0..4 {
                for byte in 0..=u8::MAX {
                    let mut bytes = base;
                    bytes[index] = byte;
                    assert_eq!(read_u32(bytes), u32::from_be_bytes(bytes));
                    words += 1;
                }
            }
        }
        assert_eq!(words, 4_096);
        for value in [0, 1, 2, u32::MAX - 1, u32::MAX] {
            for divisor in [0, 1, 2, 3, u32::MAX] {
                let expected = if divisor == 0 {
                    Err(malformed(0, MalformedKind::InvalidImageGeometry))
                } else {
                    Ok(u32::try_from(u64::from(value).div_ceil(u64::from(divisor))).unwrap())
                };
                assert_eq!(ceil_div(value, divisor), expected);
            }
        }
        let bytes = payload(geometries[0], &components[4], 0x4000);
        let mut input = vec![255, 81];
        input.extend(u16::try_from(bytes.len() + 2).unwrap().to_be_bytes());
        input.extend(&bytes);
        let mut parser = new_parser(&input, &limit).unwrap();
        let segment = parser.read_segment().unwrap();
        let actual = parser.parse_siz(segment).unwrap();
        assert_eq!(actual, reference(&bytes, 0, &limit).unwrap());
        assert_eq!(actual.0.uniform_bit_depth, None);
        assert_eq!(actual.0.uniform_signed, None);
        assert!(
            actual.0.component_info[2..]
                .iter()
                .all(|info| *info == ComponentInfo::default())
        );
        assert_eq!(parser.work_units, 1);
        assert_eq!(parser.position, input.len());
    }

    #[test]
    fn actual_raw_reader_derives_marker_payload_cursor_work_and_partial_error_state() {
        type Counters = (usize, u64, u64, u64);
        type SegmentObservation = (u16, usize, Vec<u8>);

        fn malformed(position: usize, kind: MalformedKind) -> DecodeError {
            DecodeError::MalformedInput {
                offset: position as u64,
                kind,
            }
        }
        fn counters(parser: &Parser<'_>) -> Counters {
            (
                parser.position,
                parser.work_units,
                parser.packet_length_bytes,
                parser.packet_lengths,
            )
        }
        fn raw<'a>(
            input: &'a [u8],
            limits: &'a ResourceLimits,
            position: usize,
            work: u64,
        ) -> Parser<'a> {
            Parser {
                input,
                limits,
                position,
                work_units: work,
                packet_length_bytes: u64::MAX,
                packet_lengths: 17,
            }
        }
        fn frame(parser: &Parser<'_>, input: &[u8], limits: &ResourceLimits, expected: Counters) {
            assert_eq!(counters(parser), expected);
            assert_eq!(parser.input, input);
            assert!(core::ptr::eq(parser.limits, limits));
        }
        fn peek(input: &[u8], position: usize) -> Result<u16, DecodeError> {
            let tail = input
                .get(position..)
                .ok_or_else(|| malformed(position, MalformedKind::Truncated))?;
            let bytes = tail
                .get(..2)
                .ok_or_else(|| malformed(position, MalformedKind::Truncated))?;
            if bytes[0] != 255 || bytes[1] == 0 || bytes[1] == 255 {
                Err(malformed(position, MalformedKind::InvalidMarker))
            } else {
                Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
            }
        }
        fn charge(
            position: usize,
            work: &mut u64,
            amount: u64,
            limits: &ResourceLimits,
        ) -> Result<(), DecodeError> {
            let next = u128::from(*work) + u128::from(amount);
            if next > u128::from(u64::MAX) {
                return Err(malformed(position, MalformedKind::ArithmeticOverflow));
            }
            *work = next as u64;
            if *work > limits.maximum_work_units() {
                Err(DecodeError::LimitExceeded {
                    resource: Resource::WorkUnits,
                    limit: limits.maximum_work_units(),
                    actual: *work,
                })
            } else {
                Ok(())
            }
        }
        fn standalone(
            input: &[u8],
            state: &mut Counters,
            expected: u16,
            limits: &ResourceLimits,
        ) -> Result<(), DecodeError> {
            if peek(input, state.0)? != expected {
                return Err(malformed(state.0, MalformedKind::InvalidMarkerOrder));
            }
            let next = state.0 as u128 + 2;
            if next > usize::MAX as u128 {
                return Err(malformed(state.0, MalformedKind::ArithmeticOverflow));
            }
            state.0 = next as usize;
            charge(state.0, &mut state.1, 1, limits)
        }
        fn segment(
            input: &[u8],
            state: &mut Counters,
            limits: &ResourceLimits,
        ) -> Result<SegmentObservation, DecodeError> {
            let start = state.0;
            let marker = peek(input, start)?;
            charge(start, &mut state.1, 1, limits)?;
            if [SOC, SOD, EOC, 0xff92].contains(&marker) && marker != SOT {
                return Err(malformed(start, MalformedKind::InvalidMarkerOrder));
            }
            let length_offset = start as u128 + 2;
            if length_offset > usize::MAX as u128 {
                return Err(malformed(start, MalformedKind::ArithmeticOverflow));
            }
            let length_offset = length_offset as usize;
            let word = input
                .get(length_offset..)
                .and_then(|tail| tail.get(..2))
                .ok_or_else(|| malformed(start, MalformedKind::Truncated))?;
            let length = u16::from_be_bytes([word[0], word[1]]);
            if length < 2 {
                return Err(malformed(start, MalformedKind::InvalidMarkerLength));
            }
            let end = length_offset as u128 + u128::from(length);
            if end > usize::MAX as u128 {
                return Err(malformed(start, MalformedKind::ArithmeticOverflow));
            }
            if end > input.len() as u128 {
                return Err(malformed(start, MalformedKind::Truncated));
            }
            let payload = input[length_offset + 2..end as usize].to_vec();
            state.0 = end as usize;
            Ok((marker, start, payload))
        }
        fn eoc(
            input: &[u8],
            state: &mut Counters,
            limits: &ResourceLimits,
        ) -> Result<usize, DecodeError> {
            let tail = input
                .get(state.0..)
                .ok_or_else(|| malformed(state.0, MalformedKind::Truncated))?;
            for word in tail.windows(2) {
                charge(state.0, &mut state.1, 1, limits)?;
                if word == [255, 217] {
                    return Ok(state.0);
                }
                state.0 += 1;
            }
            Err(malformed(state.0, MalformedKind::Truncated))
        }

        let zero_work = ResourceLimits::HARDENED.with_maximum_work_units(0).unwrap();
        let mut words = 0;
        for word in 0..=u16::MAX {
            let bytes = word.to_be_bytes();
            let parser = raw(&bytes, &zero_work, 0, u64::MAX);
            assert_eq!(parser.peek_marker(), peek(&bytes, 0));
            assert_eq!(read_u16(bytes), u16::from_be_bytes(bytes));
            assert_eq!(
                is_standalone_marker(word),
                [SOC, SOD, EOC, 0xff92].contains(&word)
            );
            frame(&parser, &bytes, &zero_work, (0, u64::MAX, u64::MAX, 17));
            words += 1;
        }
        assert_eq!(words, 65_536);
        let mut constructors = 0;
        for length in 0..=64 {
            let bytes = vec![0xa5; length];
            for maximum in [
                0,
                1,
                2,
                17,
                64,
                ResourceLimits::HARDENED.maximum_input_bytes(),
            ] {
                let limits = ResourceLimits::HARDENED
                    .with_maximum_input_bytes(maximum)
                    .unwrap();
                match new_parser(&bytes, &limits) {
                    Ok(parser) => {
                        assert!(length as u64 <= maximum);
                        frame(&parser, &bytes, &limits, (0, 0, 0, 0));
                    }
                    Err(error) => assert_eq!(
                        error,
                        DecodeError::LimitExceeded {
                            resource: Resource::InputBytes,
                            limit: maximum,
                            actual: length as u64,
                        }
                    ),
                }
                constructors += 1;
            }
        }
        assert_eq!(constructors, 390);
        let fixtures = [
            vec![],
            vec![0],
            vec![255],
            vec![255, 0],
            vec![255, 255],
            vec![0, 144],
            vec![255, 79],
            vec![255, 217],
            vec![255, 144, 0, 2],
            vec![255, 81, 0, 1],
            vec![255, 81, 0, 0],
            vec![255, 81, 0, 5, 170, 187],
            vec![255, 81, 0, 4, 170, 187],
            vec![255, 0, 255, 217],
            vec![17, 255, 217, 255, 217],
            vec![255, 81, 255, 255],
        ];
        let limits = [
            ResourceLimits::HARDENED,
            zero_work,
            ResourceLimits::HARDENED.with_maximum_work_units(1).unwrap(),
            ResourceLimits::HARDENED.with_maximum_work_units(2).unwrap(),
        ];
        let mut state_cases = 0;
        for input in fixtures {
            for position in (0..=input.len() + 2).chain(core::iter::once(usize::MAX)) {
                for work in [0, 1, u64::MAX - 1, u64::MAX] {
                    for limit in &limits {
                        let initial = (position, work, u64::MAX, 17);
                        let parser = raw(&input, limit, position, work);
                        assert_eq!(parser.peek_marker(), peek(&input, position));
                        for kind in [
                            MalformedKind::Truncated,
                            MalformedKind::InvalidMarker,
                            MalformedKind::ArithmeticOverflow,
                        ] {
                            assert_eq!(parser.malformed_here(kind), malformed(position, kind));
                        }
                        assert_eq!(parser.offset(position), position as u64);
                        for amount in [0, 1, 2, u64::MAX] {
                            let mut actual = raw(&input, limit, position, work);
                            let mut expected = initial;
                            assert_eq!(
                                actual.charge_work(amount),
                                charge(position, &mut expected.1, amount, limit)
                            );
                            frame(&actual, &input, limit, expected);
                        }
                        for marker in [SOC, SIZ, EOC, 0x1234] {
                            let mut actual = raw(&input, limit, position, work);
                            let mut expected = initial;
                            assert_eq!(
                                actual.expect_standalone(marker),
                                standalone(&input, &mut expected, marker, limit)
                            );
                            frame(&actual, &input, limit, expected);
                        }
                        let mut actual = raw(&input, limit, position, work);
                        let mut expected = initial;
                        let read = actual.read_segment().map(|segment| {
                            (segment.marker, segment.offset, segment.payload.to_vec())
                        });
                        assert_eq!(read, segment(&input, &mut expected, limit));
                        frame(&actual, &input, limit, expected);
                        let mut actual = raw(&input, limit, position, work);
                        let mut expected = initial;
                        assert_eq!(actual.find_eoc(), eoc(&input, &mut expected, limit));
                        frame(&actual, &input, limit, expected);
                        state_cases += 1;
                    }
                }
            }
        }
        assert_eq!(state_cases, 1_808);
    }

    #[test]
    fn progression_change_component_indexes_follow_csiz_width() {
        let limits = ResourceLimits::default();
        let parser = new_parser(&[], &limits).unwrap();
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
