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

    pub open spec fn parsed_progression_outcome(value: Result<ProgressionChanges, DecodeError>)
        -> Result<Seq<ProgressionChange>, DecodeError>
    {
        match value { Err(error) => Err(error), Ok(value) => Ok(progression_changes_view(&value)) }
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

    pub closed spec fn component_coding_style_count(styles: &ComponentCodingStyles) -> u16 {
        styles.components
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

    closed spec fn parsed_quantization_outcome(result: Result<Quantization, DecodeError>)
        -> Result<crate::specs::Part2Quantization, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(value) => Ok(quantization_view(&value)) }
    }

    closed spec fn parsed_component_quantization_outcome(result: Result<(u16, Quantization), DecodeError>)
        -> Result<(u16, crate::specs::Part2Quantization), DecodeError>
    {
        match result { Err(error) => Err(error), Ok((component, value)) => Ok((component, quantization_view(&value))) }
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

    pub open spec fn kernel_coefficient_outcome(value: Result<KernelCoefficient, DecodeError>)
        -> Result<crate::specs::Part2RawKernelCoefficient, DecodeError>
    {
        match value { Err(error) => Err(error), Ok(value) => Ok(raw_kernel_coefficient_view(value)) }
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

    pub open spec fn kernel_step_outcome(value: Result<(KernelLiftingStep, usize), DecodeError>)
        -> Result<(crate::specs::Part2RawKernelStep, usize), DecodeError>
    {
        match value { Err(error) => Err(error), Ok((step, next)) => Ok((raw_kernel_step_view(step), next)) }
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

    pub closed spec fn profile_number_view(value: &ProfileNumber) -> Seq<u16> {
        value.words@
    }

    pub open spec fn cap_payload_outcome(value: Result<ExtendedCapabilities, DecodeError>)
        -> Result<(u32, Seq<u16>), DecodeError>
    {
        match value { Err(error) => Err(error), Ok(capabilities) => Ok(extended_capabilities_view(&capabilities)) }
    }

    pub open spec fn profile_number_outcome(value: Result<ProfileNumber, DecodeError>)
        -> Result<Seq<u16>, DecodeError>
    {
        match value { Err(error) => Err(error), Ok(profile) => Ok(profile_number_view(&profile)) }
    }

    pub open spec fn marker_words_outcome(value: Result<Vec<u16>, DecodeError>)
        -> Result<Seq<u16>, DecodeError>
    {
        match value { Err(error) => Err(error), Ok(words) => Ok(words@) }
    }

    pub closed spec fn codestream_profile_capabilities(value: &CodestreamProfile) -> Option<(u32, Seq<u16>)> {
        match value.extended_capabilities { None => None, Some(capabilities) => Some(extended_capabilities_view(&capabilities)) }
    }
}

impl ExtendedCapabilities {
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == extended_capabilities_view(self).0,
    ))]
    pub const fn p_cap(&self) -> u32 {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(extended_capabilities_view); }
        self.p_cap
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result@ == extended_capabilities_view(self).1,
    ))]
    pub fn c_capabilities(&self) -> &[u16] {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(extended_capabilities_view); }
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
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result@ == profile_number_view(self),
    ))]
    pub fn words(&self) -> &[u16] {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(profile_number_view); }
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

    closed spec fn mct_array_outcome(value: Result<MctArray, DecodeError>) -> Result<crate::specs::Part2MctArray, DecodeError> {
        match value { Err(error) => Err(error), Ok(value) => Ok(mct_array_view(&value)) }
    }

    closed spec fn mct_arrays_view(values: Seq<MctArray>) -> Seq<crate::specs::Part2MctArray> {
        values.map_values(|value: MctArray| mct_array_view(&value))
    }

    closed spec fn mct_record_view(value: &MctRecord) -> crate::specs::Part1MctRecord {
        (value.index, mcc_collection_references_view(value.collections@))
    }

    closed spec fn mct_records_view(values: Seq<MctRecord>) -> Seq<crate::specs::Part1MctRecord> {
        values.map_values(|value: MctRecord| mct_record_view(&value))
    }

    closed spec fn mct_array_reference_outcome(value: Result<&MctArray, DecodeError>)
        -> Result<crate::specs::Part2MctArray, DecodeError>
    {
        match value { Err(error) => Err(error), Ok(array) => Ok(mct_array_view(array)) }
    }

    closed spec fn mct_parameters_view(value: &MctParameters) -> crate::specs::Part1MctParameters {
        crate::specs::Part1MctParameters {
            arrays: mct_arrays_view(value.arrays@), records: mct_records_view(value.records@), order: value.order@,
        }
    }

    closed spec fn mct_collections_view(values: Seq<MctCollection>) -> Seq<crate::specs::Part2MctCollection> {
        values.map_values(|value: MctCollection| mct_collection_view(&value))
    }

    closed spec fn mct_stages_view(values: Seq<MctStage>) -> Seq<crate::specs::Part2MctStage> {
        values.map_values(|value: MctStage| mct_stage_view(&value))
    }

    closed spec fn mct_collection_outcome(value: Result<MctCollection, DecodeError>)
        -> Result<crate::specs::Part2MctCollection, DecodeError>
    {
        match value { Err(error) => Err(error), Ok(collection) => Ok(mct_collection_view(&collection)) }
    }

    closed spec fn mct_stage_outcome(value: Result<MctStage, DecodeError>)
        -> Result<crate::specs::Part2MctStage, DecodeError>
    {
        match value { Err(error) => Err(error), Ok(stage) => Ok(mct_stage_view(&stage)) }
    }

    closed spec fn mct_graph_outcome(value: Result<Option<MultipleComponentTransform>, DecodeError>)
        -> Result<Option<Seq<crate::specs::Part2MctStage>>, DecodeError>
    {
        match value {
            Err(error) => Err(error), Ok(None) => Ok(None),
            Ok(Some(graph)) => Ok(Some(multiple_component_transform_view(&graph))),
        }
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

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
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

    closed spec fn packet_length_groups_view(groups: Seq<PacketLengths>) -> Seq<crate::specs::Part1PacketLengths> {
        groups.map(|_i: int, group: PacketLengths| packet_lengths_view(&group))
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

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn mcc_reference_transform_view(value: MctCollectionReferenceTransform) -> crate::specs::Part1MccTransformReference {
        match value {
            MctCollectionReferenceTransform::Array { kind, reversible, transform_index, offset_index } =>
                crate::specs::Part1MccTransformReference::Array { kind, reversible, transform_index, offset_index },
            MctCollectionReferenceTransform::Wavelet(wavelet) =>
                crate::specs::Part1MccTransformReference::Wavelet(mct_wavelet_view(wavelet)),
        }
    }

    closed spec fn mcc_collection_reference_view(value: &MctCollectionReference) -> crate::specs::Part1MccCollectionReference {
        crate::specs::Part1MccCollectionReference {
            inputs: value.input_components@, outputs: value.output_components@, transform: mcc_reference_transform_view(value.transform),
        }
    }

    closed spec fn mcc_collection_references_view(values: Seq<MctCollectionReference>) -> Seq<crate::specs::Part1MccCollectionReference> {
        values.map_values(|value: MctCollectionReference| mcc_collection_reference_view(&value))
    }

    closed spec fn mcc_collection_outcome(value: Result<(MctCollectionReference, usize), DecodeError>)
        -> Result<(crate::specs::Part1MccCollectionReference, usize), DecodeError>
    {
        match value { Err(error) => Err(error), Ok((collection, next)) => Ok((mcc_collection_reference_view(&collection), next)) }
    }

    closed spec fn mcc_record_outcome(value: Result<MctRecord, DecodeError>)
        -> Result<(u8, Seq<crate::specs::Part1MccCollectionReference>), DecodeError>
    {
        match value { Err(error) => Err(error), Ok(record) => Ok((record.index, mcc_collection_references_view(record.collections@))) }
    }
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

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn mct_fragment_view(value: &MctFragment) -> crate::specs::Part1MctFragment {
        crate::specs::Part1MctFragment { descriptor: value.descriptor, next_index: value.next_index,
            final_index: value.final_index, offset: value.offset, data: value.data@ }
    }

    closed spec fn mct_pending_view(values: Seq<MctFragment>) -> Seq<crate::specs::Part1MctFragment> {
        values.map_values(|value: MctFragment| mct_fragment_view(&value))
    }

    closed spec fn mct_fragment_outcome(value: Result<Option<MctArray>, DecodeError>)
        -> Result<Option<crate::specs::Part2MctArray>, DecodeError>
    {
        match value {
            Err(error) => Err(error),
            Ok(None) => Ok(None),
            Ok(Some(array)) => Ok(Some(mct_array_view(&array))),
        }
    }

    closed spec fn mcc_fragment_view(value: &MccFragment) -> crate::specs::Part1MccFragment {
        crate::specs::Part1MccFragment { record_index: value.record_index, next_index: value.next_index,
            final_index: value.final_index, collection_count: value.collection_count, offset: value.offset, data: value.data@ }
    }

    closed spec fn mcc_pending_view(values: Seq<MccFragment>) -> Seq<crate::specs::Part1MccFragment> {
        values.map_values(|value: MccFragment| mcc_fragment_view(&value))
    }

    closed spec fn mcc_fragment_outcome(value: Result<Option<MctRecord>, DecodeError>)
        -> Result<Option<(u8, Seq<crate::specs::Part1MccCollectionReference>)>, DecodeError>
    {
        match value {
            Err(error) => Err(error),
            Ok(None) => Ok(None),
            Ok(Some(record)) => Ok(Some((record.index, mcc_collection_references_view(record.collections@)))),
        }
    }
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_mct_fragment_find(mct_pending_view(pending@), index, 0),
        match result { Some(position) => position < pending.len(), None => true },
))]
fn find_mct_fragment(pending: &[MctFragment], index: u8) -> Option<usize> {
    let mut position = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant position <= pending.len(),
            crate::specs::part1_mct_fragment_find(mct_pending_view(pending@), index, 0)
                == crate::specs::part1_mct_fragment_find(mct_pending_view(pending@), index, position as nat),
        decreases pending.len() - position,
    ))]
    while position < pending.len() {
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_mct_fragment_find, 1); reveal(mct_fragment_view); }
        if (pending[position].descriptor & 255) as u8 == index {
            return Some(position);
        }
        position += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part1_mct_fragment_find, 1); }
    None
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_mcc_fragment_find(mcc_pending_view(pending@), index, 0),
        match result { Some(position) => position < pending.len(), None => true },
))]
fn find_mcc_fragment(pending: &[MccFragment], index: u8) -> Option<usize> {
    let mut position = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant position <= pending.len(),
            crate::specs::part1_mcc_fragment_find(mcc_pending_view(pending@), index, 0)
                == crate::specs::part1_mcc_fragment_find(mcc_pending_view(pending@), index, position as nat),
        decreases pending.len() - position,
    ))]
    while position < pending.len() {
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_mcc_fragment_find, 1); reveal(mcc_fragment_view); }
        if pending[position].record_index == index {
            return Some(position);
        }
        position += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part1_mcc_fragment_find, 1); }
    None
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_byte_list_contains(values@, value),
))]
fn u8_list_contains(values: &[u8], value: u8) -> bool {
    let mut position = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant position <= values.len(),
            forall|index: int| 0 <= index < position ==> values@[index] != value,
        decreases values.len() - position,
    ))]
    while position < values.len() {
        if values[position] == value {
            return true;
        }
        position += 1;
    }
    false
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

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn packed_header_frames_view(values: Seq<PackedHeaderFrame>) -> Seq<(usize, usize)> {
        values.map_values(|value: PackedHeaderFrame| (value.start, value.length))
    }

    closed spec fn packed_header_frames_outcome(value: Result<Vec<PackedHeaderFrame>, DecodeError>)
        -> Result<Seq<(usize, usize)>, DecodeError>
    {
        match value { Err(error) => Err(error), Ok(frames) => Ok(packed_header_frames_view(frames@)) }
    }
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

    closed spec fn dfs_payload_outcome(result: Result<DecompositionStructure, DecodeError>)
        -> Result<(u8, Seq<DecompositionDirection>), DecodeError>
    {
        match result { Err(error) => Err(error), Ok(value) => Ok(owned_decomposition_view(&value)) }
    }

    pub closed spec fn owned_arbitrary_decomposition_view(value: &ArbitraryDecomposition) -> (u8, Seq<u8>, Seq<DecompositionDirection>) {
        (value.index, value.sublevel_depths@, value.subband_splits@)
    }

    closed spec fn ads_payload_outcome(result: Result<ArbitraryDecomposition, DecodeError>)
        -> Result<(u8, Seq<u8>, Seq<DecompositionDirection>), DecodeError>
    {
        match result { Err(error) => Err(error), Ok(value) => Ok(owned_arbitrary_decomposition_view(&value)) }
    }

    closed spec fn two_bit_values_outcome(result: Result<Vec<u8>, DecodeError>) -> Result<Seq<u8>, DecodeError> {
        match result { Err(error) => Err(error), Ok(values) => Ok(values@) }
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

    pub open spec fn nlt_payload_outcome(value: Result<NonlinearPointTransform, DecodeError>)
        -> Result<crate::specs::Part2OwnedNonlinear, DecodeError>
    {
        match value { Err(error) => Err(error), Ok(value) => Ok(owned_nonlinear_view(&value)) }
    }

    pub open spec fn segment_tail_outcome(value: Result<Vec<u8>, DecodeError>) -> Result<Seq<u8>, DecodeError> {
        match value { Err(error) => Err(error), Ok(value) => Ok(value@) }
    }

    pub closed spec fn owned_kernel_view(value: &ArbitraryKernel) -> (u8, u16, crate::specs::Part2RawKernel) {
        (value.index, value.descriptor, raw_kernel_view(*value))
    }

    pub open spec fn atk_payload_outcome(value: Result<ArbitraryKernel, DecodeError>)
        -> Result<(u8, u16, crate::specs::Part2RawKernel), DecodeError>
    {
        match value { Err(error) => Err(error), Ok(value) => Ok(owned_kernel_view(&value)) }
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

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), owned_extensions_view(final(extensions)))
            == crate::specs::part1_charge_extension_metadata(
                parser_view(old(self)), segment_view(segment), owned_extensions_view(old(extensions))),
    ))]
    fn charge_extension_metadata(
        &mut self,
        segment: Segment<'a>,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_charge_extension_metadata); }
        let bytes = match (segment.payload.len() as u64).checked_add(4) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        extensions.metadata_bytes = match extensions.metadata_bytes.checked_add(bytes) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        if extensions.metadata_bytes > self.limits.maximum_metadata_bytes() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::MetadataBytes,
                limit: self.limits.maximum_metadata_bytes(),
                actual: extensions.metadata_bytes,
            });
        }
        self.charge_work(bytes)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_segment_byte(self.position, segment_view(segment), position, relative),
    ))]
    fn segment_byte(
        &self,
        segment: Segment<'a>,
        position: usize,
        relative: usize,
    ) -> Result<u8, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_segment_byte); }
        let index = match position.checked_add(relative) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        match segment.payload.get(index) {
            Some(value) => Ok(*value),
            None => Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength)),
        }
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), *final(next_index), owned_extensions_view(final(extensions)))
            == crate::specs::part1_parse_tlm(parser_view(old(self)), segment_view(segment),
                *old(next_index), tile_count, owned_extensions_view(old(extensions))),
    ))]
    fn parse_tlm(
        &mut self,
        segment: Segment<'a>,
        next_index: &mut u16,
        tile_count: u64,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost extension_input = owned_extensions_view(extensions);
            let ghost index_input = *next_index;
            let ghost goal = crate::specs::part1_parse_tlm(input, segment_view(segment), index_input, tile_count, extension_input);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_tlm); }
        self.charge_extension_metadata(segment, extensions)?;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost charged_parser = parser_view(self);
            let ghost charged_extensions = owned_extensions_view(extensions);
        }
        if segment.payload.len() < 2 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let index = u16::from(segment.payload[0]);
        if index != *next_index {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerOrder));
        }
        *next_index = match next_index.checked_add(1) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let style = segment.payload[1];
        let tile_bytes = crate::refinement::tlm_tile_index_bytes(style);
        let length_bytes = crate::refinement::tlm_length_bytes(style);
        #[cfg(verus_keep_ghost)]
        proof! { assert((style >> 4) & 3 <= 3u8) by (bit_vector); }
        if style & 0x8f != 0 || tile_bytes == 3 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTilePart));
        }
        let record_bytes = match tile_bytes.checked_add(length_bytes) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        let records = segment.payload.len() - 2;
        if record_bytes == 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let remainder = records % record_bytes;
        if remainder != 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost target = crate::specs::part1_tlm_records(segment_view(segment), tile_count,
                tile_bytes, length_bytes, 2, charged_extensions.lengths);
        }
        let mut position = 2_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    2 <= position <= segment.payload.len(),
                    tile_bytes <= 2, length_bytes == 2 || length_bytes == 4,
                    2 <= record_bytes <= 6,
                    record_bytes == tile_bytes + length_bytes,
                    (segment.payload.len() - position) % (record_bytes as int) == 0,
                    input == parser_view(old(self)),
                    extension_input == owned_extensions_view(old(extensions)),
                    index_input == *old(next_index),
                    index_input <= 255, *next_index == index_input + 1,
                    parser_view(self) == charged_parser,
                    owned_extensions_view(extensions) == crate::specs::part1_extension_lengths(
                        charged_extensions, extensions.tile_part_lengths@),
                    target == crate::specs::part1_tlm_records(segment_view(segment), tile_count,
                        tile_bytes, length_bytes, position as nat, extensions.tile_part_lengths@),
                    goal == crate::specs::part1_parse_tlm(input, segment_view(segment), index_input, tile_count, extension_input),
                    goal == (target.0, charged_parser, *next_index,
                        crate::specs::part1_extension_lengths(charged_extensions, target.1)),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(crate::specs::part1_tlm_records, 1);
                reveal(crate::specs::part1_segment_byte);
                if segment.payload.len() - position < record_bytes {
                    vstd::arithmetic::div_mod::lemma_small_mod(
                        (segment.payload.len() - position) as nat, record_bytes as nat);
                }
                assert(position + record_bytes <= segment.payload.len());
            }
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
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTilePart));
            }
            let length_position = match position.checked_add(tile_bytes) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
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
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTilePart));
            }
            extensions
                .tile_part_lengths
                .push(TilePartLength { tile_index, length });
            let next_position = match position.checked_add(record_bytes) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
            if next_position <= position || next_position > segment.payload.len() {
                return Err(self.malformed_here(MalformedKind::InvalidMarkerLength));
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                vstd::arithmetic::div_mod::lemma_mod_sub_multiples_vanish(
                    segment.payload.len() as int - position as int, record_bytes as int);
            }
            position = next_position;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_tlm_records, 1); }
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

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), owned_extensions_view(final(extensions)))
            == crate::specs::part1_parse_crg(parser_view(old(self)), segment_view(segment),
                components, owned_extensions_view(old(extensions))),
    ))]
    fn parse_crg(
        &mut self,
        segment: Segment<'a>,
        components: u16,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost extension_input = owned_extensions_view(extensions);
            let ghost goal = crate::specs::part1_parse_crg(input, segment_view(segment), components, extension_input);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_crg); }
        self.charge_extension_metadata(segment, extensions)?;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost charged_parser = parser_view(self);
            let ghost charged_extensions = owned_extensions_view(extensions);
        }
        let expected = crate::refinement::crg_payload_bytes(components);
        if segment.payload.len() != expected || !extensions.component_registrations.is_empty() {
            return Err(self.malformed_at(
                segment.offset,
                if segment.payload.len() == expected {
                    MalformedKind::InvalidMarkerOrder
                } else {
                    MalformedKind::InvalidMarkerLength
                },
            ));
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(goal == (Ok(()), charged_parser, crate::specs::part1_extension_components(
                charged_extensions, charged_extensions.metadata_bytes, crate::specs::part1_crg_registrations(segment.payload@))));
            assert(charged_extensions.registrations =~= Seq::empty());
            assert(crate::specs::part1_crg_registrations(segment.payload@.subrange(0, 0)) =~= Seq::empty());
        }
        let mut position = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    position <= segment.payload.len(),
                    position % 4 == 0,
                    segment.payload.len() == 4int * components as int,
                    input == parser_view(old(self)),
                    extension_input == owned_extensions_view(old(extensions)),
                    parser_view(self) == charged_parser,
                    owned_extensions_view(extensions) == crate::specs::part1_extension_components(
                        charged_extensions, charged_extensions.metadata_bytes,
                        crate::specs::part1_crg_registrations(segment.payload@.subrange(0, position as int))),
                    goal == crate::specs::part1_parse_crg(input, segment_view(segment), components, extension_input),
                    goal == (Ok(()), charged_parser, crate::specs::part1_extension_components(
                        charged_extensions, charged_extensions.metadata_bytes, crate::specs::part1_crg_registrations(segment.payload@))),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal(crate::specs::part1_segment_byte);
                assert(position as int + 4 <= segment.payload.len());
            }
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
            #[cfg(verus_keep_ghost)]
            proof! {
                let raw = segment.payload@;
                let previous = crate::specs::part1_crg_registrations(raw.subrange(0, position as int));
                let next = crate::specs::part1_crg_registrations(raw.subrange(0, next_position as int));
                let entry = ComponentRegistration {
                    horizontal: crate::specs::part1_marker_value(raw[position as int], raw[position as int + 1]),
                    vertical: crate::specs::part1_marker_value(raw[position as int + 2], raw[position as int + 3]),
                };
                assert(next =~= previous.push(entry)) by {
                    assert forall|index: int| 0 <= index < next.len() implies
                        next[index] == previous.push(entry)[index] by {
                        if index < previous.len() {
                        } else {
                            assert(4 * index == position as int);
                        }
                    };
                }
                assert(extensions.component_registrations@ =~= next);
            }
            position = next_position;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(segment.payload@.subrange(0, position as int) =~= segment.payload@);
            assert((Ok(()), parser_view(self), owned_extensions_view(extensions)) == goal);
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), owned_extensions_view(final(extensions)))
            == crate::specs::part1_parse_dfs(parser_view(old(self)), segment_view(segment),
                owned_extensions_view(old(extensions))),
    ))]
    fn parse_dfs(
        &mut self,
        segment: Segment<'a>,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost extension_input = owned_extensions_view(extensions);
            let ghost goal = crate::specs::part1_parse_dfs(input, segment_view(segment), extension_input);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_dfs); }
        self.charge_extension_metadata(segment, extensions)?;
        let replacement = self.decode_dfs_payload(segment)?;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost charged_parser = parser_view(self);
            let ghost charged_extensions = owned_extensions_view(extensions);
            let ghost decoded = owned_decomposition_view(&replacement);
            let ghost target = crate::specs::part1_dfs_replace(charged_extensions.decompositions, decoded, 0);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(goal == (Ok(()), charged_parser, crate::specs::part1_extension_decompositions(
                charged_extensions, target, charged_extensions.arbitrary)));
        }
        let mut position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                position <= extensions.decomposition_structures.len(),
                input == parser_view(old(self)),
                extension_input == owned_extensions_view(old(extensions)),
                parser_view(self) == charged_parser,
                owned_extensions_view(extensions) == charged_extensions,
                owned_decomposition_view(&replacement) == decoded,
                target == crate::specs::part1_dfs_replace(charged_extensions.decompositions, decoded, position as nat),
                goal == crate::specs::part1_parse_dfs(input, segment_view(segment), extension_input),
                goal == (Ok(()), charged_parser, crate::specs::part1_extension_decompositions(
                    charged_extensions, target, charged_extensions.arbitrary)),
            decreases extensions.decomposition_structures.len() - position,
        ))]
        while position < extensions.decomposition_structures.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_dfs_replace, 1); }
            if extensions.decomposition_structures[position].index == replacement.index {
                extensions.decomposition_structures[position] = replacement;
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(owned_extensions_view(extensions).decompositions =~=
                        charged_extensions.decompositions.update(position as int, decoded));
                }
                return Ok(());
            }
            position += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_dfs_replace, 1); }
        extensions.decomposition_structures.push(replacement);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(owned_extensions_view(extensions).decompositions =~=
                charged_extensions.decompositions.push(decoded));
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), owned_extensions_view(final(extensions)))
            == crate::specs::part1_parse_ads(parser_view(old(self)), segment_view(segment),
                owned_extensions_view(old(extensions))),
    ))]
    fn parse_ads(
        &mut self,
        segment: Segment<'a>,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost extension_input = owned_extensions_view(extensions);
            let ghost goal = crate::specs::part1_parse_ads(input, segment_view(segment), extension_input);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_ads); }
        self.charge_extension_metadata(segment, extensions)?;
        let replacement = self.decode_ads_payload(segment)?;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost charged_parser = parser_view(self);
            let ghost charged_extensions = owned_extensions_view(extensions);
            let ghost decoded = owned_arbitrary_decomposition_view(&replacement);
            let ghost target = crate::specs::part1_ads_replace(charged_extensions.arbitrary, decoded, 0);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(goal == (Ok(()), charged_parser, crate::specs::part1_extension_decompositions(
                charged_extensions, charged_extensions.decompositions, target)));
        }
        let mut position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                position <= extensions.arbitrary_decompositions.len(),
                input == parser_view(old(self)),
                extension_input == owned_extensions_view(old(extensions)),
                parser_view(self) == charged_parser,
                owned_extensions_view(extensions) == charged_extensions,
                owned_arbitrary_decomposition_view(&replacement) == decoded,
                target == crate::specs::part1_ads_replace(charged_extensions.arbitrary, decoded, position as nat),
                goal == crate::specs::part1_parse_ads(input, segment_view(segment), extension_input),
                goal == (Ok(()), charged_parser, crate::specs::part1_extension_decompositions(
                    charged_extensions, charged_extensions.decompositions, target)),
            decreases extensions.arbitrary_decompositions.len() - position,
        ))]
        while position < extensions.arbitrary_decompositions.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_ads_replace, 1); }
            if extensions.arbitrary_decompositions[position].index == replacement.index {
                extensions.arbitrary_decompositions[position] = replacement;
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(owned_extensions_view(extensions).arbitrary =~=
                        charged_extensions.arbitrary.update(position as int, decoded));
                }
                return Ok(());
            }
            position += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_ads_replace, 1); }
        extensions.arbitrary_decompositions.push(replacement);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(owned_extensions_view(extensions).arbitrary =~=
                charged_extensions.arbitrary.push(decoded));
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures dfs_payload_outcome(result) == crate::specs::part1_dfs_payload(segment_view(segment)),
    ))]
    fn decode_dfs_payload(
        &self,
        segment: Segment<'a>,
    ) -> Result<DecompositionStructure, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_dfs_payload); }
        if segment.payload.len() < 3 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let descriptor = read_u16([segment.payload[0], segment.payload[1]]);
        #[cfg(verus_keep_ghost)]
        proof! { assert(descriptor & 15 <= 255) by (bit_vector); }
        let index = match u8::try_from(descriptor & 0x0f) {
            Ok(value) => value,
            Err(_) => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        let levels = segment.payload[2];
        let expanded = match usize::from(levels).checked_add(3) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        let packed = expanded / 4;
        let expected = match 3_usize.checked_add(packed) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(!0x000fu16 == 65520u16) by (bit_vector);
            assert(index == (descriptor & 15) as u8);
            assert(packed as int == (levels as int + 3) / 4);
            assert(expected as int == 3 + (levels as int + 3) / 4);
        }
        if descriptor & !0x000f != 0 || index == 0 || levels == 0 || levels > 32 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidCodingStyle));
        }
        if segment.payload.len() != expected {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost target = Seq::new(levels as nat, |level: int| crate::specs::part1_decomposition_code(
                crate::specs::part2_dfs_direction_code(segment.payload@[3 + level / 4], (level % 4) as u8)));
            let ghost goal = crate::specs::part1_dfs_payload(segment_view(segment));
        }
        #[cfg(verus_keep_ghost)]
        proof! { assert(goal == Ok((index, target))); }
        let mut directions = Vec::with_capacity(usize::from(levels));
        #[cfg(verus_keep_ghost)]
        proof! { assert(directions@ =~= target.subrange(0, 0)); }
        let mut level = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    level <= levels as usize,
                    0 < levels <= 32,
                    segment.payload.len() == 3int + (levels as int + 3) / 4,
                    goal == crate::specs::part1_dfs_payload(segment_view(segment)),
                    goal == Ok((index, target)),
                    target == Seq::new(levels as nat, |i: int| crate::specs::part1_decomposition_code(
                        crate::specs::part2_dfs_direction_code(segment.payload@[3 + i / 4], (i % 4) as u8))),
                    directions@ == target.subrange(0, level as int),
                decreases levels as usize - level,
            )
        )]
        while level < usize::from(levels) {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part1_segment_byte); }
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
        #[cfg(verus_keep_ghost)]
        proof! { assert(directions@ =~= target); }
        Ok(DecompositionStructure { index, directions })
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures ads_payload_outcome(result) == crate::specs::part1_ads_payload(self.position, segment_view(segment)),
    ))]
    fn decode_ads_payload(
        &self,
        segment: Segment<'a>,
    ) -> Result<ArbitraryDecomposition, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_ads_payload); }
        if segment.payload.len() < 3 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let index = segment.payload[0];
        if index == 0 || index > 127 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidCodingStyle));
        }
        let depth_count = usize::from(segment.payload[1]);
        let depth_bytes = crate::refinement::packed_two_bit_bytes(depth_count);
        let split_count_position = match 2_usize.checked_add(depth_bytes) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        let split_count = match segment.payload.get(split_count_position) {
            Some(value) => usize::from(*value),
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
            }
        };
        let split_bytes = crate::refinement::packed_two_bit_bytes(split_count);
        let fixed = match split_count_position.checked_add(1) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        let expected = match fixed.checked_add(split_bytes) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        if segment.payload.len() != expected {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let sublevel_depths = self.unpack_two_bit_values(segment, 2, depth_count, false)?;
        let split_values =
            self.unpack_two_bit_values(segment, split_count_position + 1, split_count, true)?;
        let mut subband_splits = Vec::with_capacity(split_values.len());
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost target = split_values@.map_values(|code: u8| crate::specs::part1_decomposition_code(code));
            let ghost goal = crate::specs::part1_ads_payload(self.position, segment_view(segment));
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(goal == Ok((index, sublevel_depths@, target)));
            assert(subband_splits@ =~= target.subrange(0, 0));
        }
        let mut split = 0_usize;
        #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant
                        split <= split_values.len(),
                        goal == crate::specs::part1_ads_payload(self.position, segment_view(segment)),
                        goal == Ok((index, sublevel_depths@, target)),
                        target == split_values@.map_values(|code: u8| crate::specs::part1_decomposition_code(code)),
                        subband_splits@ == target.subrange(0, split as int),
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
        #[cfg(verus_keep_ghost)]
        proof! { assert(subband_splits@ =~= target); }
        Ok(ArbitraryDecomposition {
            index,
            sublevel_depths,
            subband_splits,
        })
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures two_bit_values_outcome(result) == crate::specs::part1_unpack_two_bit_values(
            self.position, segment_view(segment), start, count, allow_terminal),
    ))]
    fn unpack_two_bit_values(
        &self,
        segment: Segment<'a>,
        start: usize,
        count: usize,
        allow_terminal: bool,
    ) -> Result<Vec<u8>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost goal = crate::specs::part1_unpack_two_bit_values(
                self.position, segment_view(segment), start, count, allow_terminal);
        }
        let mut values = Vec::with_capacity(count);
        #[cfg(verus_keep_ghost)]
        proof! { assert(values@ =~= Seq::empty()); }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    index <= count,
                    goal == crate::specs::part1_unpack_two_bit_values(
                        self.position, segment_view(segment), start, count, allow_terminal),
                    goal == crate::specs::part1_unpack_two_bit_run(
                        self.position, segment_view(segment), start, count, allow_terminal, index, values@),
                decreases count - index,
            )
        )]
        while index < count {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_unpack_two_bit_run, 1); }
            let byte = self.segment_byte(segment, start, index / 4)?;
            let slot = u8::try_from(index % 4)
                .map_err(|_| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            let value = crate::refinement::dfs_direction_code(byte, slot);
            if !allow_terminal && value == 0 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidCodingStyle));
            }
            values.push(value);
            index = index
                .checked_add(1)
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_unpack_two_bit_run, 1); }
        let remainder = count % 4;
        if remainder != 0 {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(goal == match crate::specs::part1_segment_byte(
                    self.position, segment_view(segment), start, count / 4) {
                    Err(error) => Err(error),
                    Ok(last) => if last & crate::specs::part1_two_bit_padding_mask(count) != 0 {
                        Err(crate::specs::part1_codestream_malformed(segment.offset, MalformedKind::InvalidCodingStyle))
                    } else { Ok(values@) },
                });
            }
            let last = self.segment_byte(segment, start, count / 4)?;
            let unused = 2_u32
                .checked_mul(
                    u32::try_from(4 - remainder)
                        .map_err(|_| self.malformed_here(MalformedKind::ArithmeticOverflow))?,
                )
                .ok_or_else(|| self.malformed_here(MalformedKind::ArithmeticOverflow))?;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(unused == 2 || unused == 4 || unused == 6);
                assert((unused == 2 || unused == 4 || unused == 6) ==>
                    (1u8 << unused) == if unused == 2 { 4u8 } else if unused == 4 { 16u8 } else { 64u8 })
                    by (bit_vector);
            }
            let mask = (1_u8 << unused) - 1;
            #[cfg(verus_keep_ghost)]
            proof! {
                if remainder == 1 {
                    assert(unused == 6);
                    assert(mask == 63);
                } else if remainder == 2 {
                    assert(unused == 4);
                    assert(mask == 15);
                } else {
                    assert(remainder == 3);
                    assert(unused == 2);
                    assert(mask == 3);
                }
                assert(unused == (2 * (4 - remainder)) as u32);
                assert((unused == 2 || unused == 4 || unused == 6) ==>
                    ((1u16 << unused) - 1) as u8 == if unused == 2 { 3u8 } else if unused == 4 { 15u8 } else { 63u8 })
                    by (bit_vector);
                assert(mask == crate::specs::part1_two_bit_padding_mask(count));
            }
            if last & mask != 0 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidCodingStyle));
            }
        }
        Ok(values)
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), owned_extensions_view(final(extensions)))
            == crate::specs::part1_parse_nlt(parser_view(old(self)), segment_view(segment),
                components, owned_extensions_view(old(extensions))),
    ))]
    fn parse_nlt(
        &mut self,
        segment: Segment<'a>,
        components: u16,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost extension_input = owned_extensions_view(extensions);
            let ghost goal = crate::specs::part1_parse_nlt(input, segment_view(segment), components, extension_input);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_nlt); }
        self.charge_extension_metadata(segment, extensions)?;
        let replacement = self.decode_nlt_payload(segment, components)?;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost charged_parser = parser_view(self);
            let ghost charged_extensions = owned_extensions_view(extensions);
            let ghost decoded = owned_nonlinear_view(&replacement);
            let ghost target = crate::specs::part1_nlt_replace(charged_extensions.nonlinear, decoded, 0);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(owned_nonlinear_view);
            assert(goal == (Ok(()), charged_parser, crate::specs::part1_extension_nonlinear(charged_extensions, target)));
        }
        let mut position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant position <= extensions.nonlinear_transforms.len(),
                input == parser_view(old(self)),
                extension_input == owned_extensions_view(old(extensions)),
                parser_view(self) == charged_parser,
                owned_extensions_view(extensions) == charged_extensions,
                owned_nonlinear_view(&replacement) == decoded,
                target == crate::specs::part1_nlt_replace(charged_extensions.nonlinear, decoded, position as nat),
                goal == crate::specs::part1_parse_nlt(input, segment_view(segment), components, extension_input),
                goal == (Ok(()), charged_parser, crate::specs::part1_extension_nonlinear(charged_extensions, target)),
            decreases extensions.nonlinear_transforms.len() - position,
        ))]
        while position < extensions.nonlinear_transforms.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_nlt_replace, 1); reveal(owned_nonlinear_view); }
            if extensions.nonlinear_transforms[position].component == replacement.component {
                extensions.nonlinear_transforms[position] = replacement;
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(owned_extensions_view(extensions).nonlinear =~=
                        charged_extensions.nonlinear.update(position as int, decoded));
                }
                return Ok(());
            }
            position += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_nlt_replace, 1); }
        extensions.nonlinear_transforms.push(replacement);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(owned_extensions_view(extensions).nonlinear =~= charged_extensions.nonlinear.push(decoded));
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures nlt_payload_outcome(result) == crate::specs::part1_nlt_payload(segment_view(segment), components),
    ))]
    fn decode_nlt_payload(
        &self,
        segment: Segment<'a>,
        components: u16,
    ) -> Result<NonlinearPointTransform, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_nlt_payload); reveal(crate::specs::part1_nlt_header);
            reveal(crate::specs::part1_nlt_points); reveal(crate::specs::part1_nlt_points_header);
            reveal(crate::specs::part1_copy_segment_tail); reveal(owned_nonlinear_view);
        }
        if segment.payload.len() < 4 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let raw_component = read_u16([segment.payload[0], segment.payload[1]]);
        let component = if raw_component == u16::MAX {
            None
        } else if raw_component < components {
            Some(raw_component)
        } else {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        };
        let precision = crate::refinement::cbd_precision(segment.payload[2]);
        let transform_type = segment.payload[3];
        if precision > 38 || transform_type > 4 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        if matches!(transform_type, 0 | 3) && segment.payload.len() != 4 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let mut d_min = None;
        let mut d_max = None;
        let mut point_bits = None;
        let mut points = Vec::new();
        let mut gamma_parameters = None;
        if transform_type == 1 {
            gamma_parameters = Some(self.decode_nlt_gamma(segment)?);
        }
        if matches!(transform_type, 2 | 4) {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part1_segment_byte); }
            if segment.payload.len() < 15 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
            }
            let stored_points = read_u16([
                self.segment_byte(segment, 4, 0)?,
                self.segment_byte(segment, 4, 1)?,
            ]);
            if stored_points == 0 || stored_points > 8191 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
            }
            let count = match stored_points.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
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
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
            }
            let width = if bits <= 8 {
                1_usize
            } else if bits <= 16 {
                2_usize
            } else {
                4_usize
            };
            let point_bytes = match usize::from(count).checked_mul(width) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
            let expected = match 15_usize.checked_add(point_bytes) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
            if segment.payload.len() != expected {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
            }
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost header = (count, minimum, maximum, bits, width);
                let ghost values = Seq::new(count as nat, |index: int|
                    crate::specs::part1_nlt_point_value(segment.payload@, width, index));
            }
            let mut point = 0_u16;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant point <= count, 1 < count <= 8192,
                        0 < bits <= 32,
                        width == crate::specs::part1_nlt_point_width(bits),
                        segment.payload.len() == 15 + count as int * width as int,
                        crate::specs::part1_nlt_points_header(segment_view(segment)) == Ok(header),
                        crate::specs::part1_nlt_header(segment_view(segment), components) ==
                            Ok((component, precision, crate::specs::part2_cbd_signed(segment.payload@[2]), transform_type)),
                        transform_type == 2 || transform_type == 4,
                        header == (count, minimum, maximum, bits, width),
                        values == Seq::new(count as nat, |index: int|
                            crate::specs::part1_nlt_point_value(segment.payload@, width, index)),
                        points@ == values.subrange(0, point as int),
                        forall|index: int| 0 <= index < point ==>
                            values[index] <= crate::specs::part1_nlt_point_maximum(bits),
                    decreases count - point,
                )
            )]
            while point < count {
                #[cfg(verus_keep_ghost)]
                proof! {
                    reveal(crate::specs::part1_segment_byte);
                    reveal(crate::specs::part1_nlt_payload); reveal(crate::specs::part1_nlt_header);
                    reveal(crate::specs::part1_nlt_points);
                }
                let relative = match usize::from(point).checked_mul(width) {
                    Some(value) => value,
                    None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
                };
                let position = match 15_usize.checked_add(relative) {
                    Some(value) => value,
                    None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
                };
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
                #[cfg(verus_keep_ghost)]
                proof! { assert(value == values[point as int]); }
                let maximum_point = if bits == 32 {
                    u32::MAX
                } else {
                    maximum_u32_for_bits(bits)?
                };
                if value > maximum_point {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(values[point as int] > crate::specs::part1_nlt_point_maximum(bits));
                        assert(crate::specs::part1_nlt_points(segment_view(segment)) ==
                            Err(crate::specs::part1_codestream_malformed(segment.offset, MalformedKind::InvalidTransform)));
                    }
                    return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
                }
                points.push(value);
                #[cfg(verus_keep_ghost)]
                proof! { assert(values.subrange(0, point as int + 1) =~= values.subrange(0, point as int).push(value)); }
                point = match point.checked_add(1) {
                    Some(value) => value,
                    None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
                };
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(values.subrange(0, point as int) =~= values);
                assert(crate::specs::part1_nlt_points(segment_view(segment)) == Ok((minimum, maximum, bits, points@)));
            }
            d_min = Some(minimum);
            d_max = Some(maximum);
            point_bits = Some(bits);
        }
        Ok(NonlinearPointTransform {
            component,
            precision,
            signed: crate::refinement::cbd_signed(segment.payload[2]),
            transform_type,
            parameters: self.copy_segment_tail(segment, 4)?,
            d_min,
            d_max,
            point_bits,
            points,
            gamma_parameters,
        })
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), owned_extensions_view(final(extensions)))
            == crate::specs::part1_parse_cbd(parser_view(old(self)), segment_view(segment),
                components, owned_extensions_view(old(extensions))),
    ))]
    fn parse_cbd(
        &mut self,
        segment: Segment<'a>,
        components: u16,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost extension_input = owned_extensions_view(extensions);
            let ghost goal = crate::specs::part1_parse_cbd(input, segment_view(segment), components, extension_input);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_cbd); }
        self.charge_extension_metadata(segment, extensions)?;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost charged_parser = parser_view(self);
            let ghost charged_extensions = owned_extensions_view(extensions);
        }
        if segment.payload.len() < 2 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let descriptor = read_u16([segment.payload[0], segment.payload[1]]);
        let repeated = descriptor & 0x8000 != 0;
        let count = descriptor & 0x7fff;
        let descriptor_count = if repeated {
            1_usize
        } else {
            usize::from(count)
        };
        #[cfg(verus_keep_ghost)]
        proof! { assert(descriptor & 0x7fff <= 32767u16) by (bit_vector); }
        let expected = match 2_usize.checked_add(descriptor_count) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        if count != components || segment.payload.len() != expected {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost target = crate::specs::part1_cbd_run(
                segment_view(segment), 2, Seq::empty(), repeated, components);
        }
        extensions.component_bit_depths.clear();
        let mut position = 2_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    2 <= position <= segment.payload.len(),
                    repeated ==> segment.payload.len() == 3,
                    extensions.component_bit_depths.len() == position - 2,
                    input == parser_view(old(self)),
                    extension_input == owned_extensions_view(old(extensions)),
                    parser_view(self) == charged_parser,
                    owned_extensions_view(extensions) == crate::specs::part1_extension_depths(
                        charged_extensions, extensions.component_bit_depths@),
                    target == crate::specs::part1_cbd_run(segment_view(segment), position as nat,
                        extensions.component_bit_depths@, repeated, components),
                    goal == crate::specs::part1_parse_cbd(input, segment_view(segment), components, extension_input),
                    goal == (target.0, charged_parser,
                        crate::specs::part1_extension_depths(charged_extensions, target.1)),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(crate::specs::part1_cbd_run, 1);
                reveal(crate::specs::part1_segment_byte);
            }
            let packed = self.segment_byte(segment, position, 0)?;
            let precision = crate::refinement::cbd_precision(packed);
            if precision > 38 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
            }
            extensions.component_bit_depths.push(ComponentBitDepth {
                precision,
                signed: crate::refinement::cbd_signed(packed),
            });
            position = match position.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_cbd_run, 1); }
        if repeated {
            let depth = match extensions.component_bit_depths.first() {
                Some(value) => *value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength)
                    );
                }
            };
            let component_count = usize::from(components);
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant
                        1 <= extensions.component_bit_depths.len(),
                        extensions.component_bit_depths.len() <= if component_count == 0 { 1int } else { component_count as int },
                        forall|index: int| 0 <= index < extensions.component_bit_depths.len() ==>
                            extensions.component_bit_depths@[index] == depth,
                        target.0 == Ok(()),
                        target.1 == Seq::new(if component_count == 0 { 1nat } else { component_count as nat },
                            |index: int| depth),
                        input == parser_view(old(self)),
                        extension_input == owned_extensions_view(old(extensions)),
                        parser_view(self) == charged_parser,
                        owned_extensions_view(extensions) == crate::specs::part1_extension_depths(
                            charged_extensions, extensions.component_bit_depths@),
                        goal == crate::specs::part1_parse_cbd(input, segment_view(segment), components, extension_input),
                        goal == (target.0, charged_parser,
                            crate::specs::part1_extension_depths(charged_extensions, target.1)),
                    decreases component_count as int
                        - extensions.component_bit_depths.len() as int,
                )
            )]
            while extensions.component_bit_depths.len() < component_count {
                extensions.component_bit_depths.push(depth);
            }
            #[cfg(verus_keep_ghost)]
            proof! { assert(extensions.component_bit_depths@ =~= target.1); }
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), owned_extensions_view(final(extensions)))
            == crate::specs::part1_parse_atk(parser_view(old(self)), segment_view(segment), owned_extensions_view(old(extensions))),
    ))]
    fn parse_atk(
        &mut self,
        segment: Segment<'a>,
        extensions: &mut Part2Extensions,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost extension_input = owned_extensions_view(extensions);
            let ghost goal = crate::specs::part1_parse_atk(input, segment_view(segment), extension_input);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_atk); }
        self.charge_extension_metadata(segment, extensions)?;
        let replacement = self.decode_atk_payload(segment)?;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost charged_parser = parser_view(self);
            let ghost charged_extensions = owned_extensions_view(extensions);
            let ghost decoded = owned_kernel_view(&replacement);
            let ghost target = crate::specs::part1_atk_replace(charged_extensions.kernels, decoded, 0);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(owned_kernel_view);
            assert(goal == (Ok(()), charged_parser, crate::specs::part1_extension_kernels(charged_extensions, target)));
        }
        let mut position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant position <= extensions.arbitrary_kernels.len(),
                input == parser_view(old(self)), extension_input == owned_extensions_view(old(extensions)),
                parser_view(self) == charged_parser, owned_extensions_view(extensions) == charged_extensions,
                owned_kernel_view(&replacement) == decoded,
                target == crate::specs::part1_atk_replace(charged_extensions.kernels, decoded, position as nat),
                goal == crate::specs::part1_parse_atk(input, segment_view(segment), extension_input),
                goal == (Ok(()), charged_parser, crate::specs::part1_extension_kernels(charged_extensions, target)),
            decreases extensions.arbitrary_kernels.len() - position,
        ))]
        while position < extensions.arbitrary_kernels.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_atk_replace, 1); reveal(owned_kernel_view); }
            if extensions.arbitrary_kernels[position].index == replacement.index {
                extensions.arbitrary_kernels[position] = replacement;
                #[cfg(verus_keep_ghost)]
                proof! { assert(owned_extensions_view(extensions).kernels =~= charged_extensions.kernels.update(position as int, decoded)); }
                return Ok(());
            }
            position += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_atk_replace, 1); }
        extensions.arbitrary_kernels.push(replacement);
        #[cfg(verus_keep_ghost)]
        proof! { assert(owned_extensions_view(extensions).kernels =~= charged_extensions.kernels.push(decoded)); }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures atk_payload_outcome(result) == crate::specs::part1_atk_payload(self.position, segment_view(segment)),
    ))]
    fn decode_atk_payload(&self, segment: Segment<'a>) -> Result<ArbitraryKernel, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost goal = crate::specs::part1_atk_payload(self.position, segment_view(segment)); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_atk_payload); reveal(crate::specs::part1_atk_header); }
        if segment.payload.len() < 7 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let descriptor = read_u16([segment.payload[0], segment.payload[1]]);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(descriptor & 0xff <= 255u16) by (bit_vector);
            assert((descriptor >> 8) & 7 <= 7u16) by (bit_vector);
        }
        let index = (descriptor & 0xff) as u8;
        let coefficient_type = ((descriptor >> 8) & 7) as u8;
        if index < 2 || coefficient_type > 4 || descriptor & 0x8000 != 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        let width = match coefficient_type {
            0 => 1_usize,
            1 => 2,
            2 => 4,
            3 => 8,
            4 => 16,
            _ => {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
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
            position = match position.checked_add(width) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
            Some(coefficient)
        };
        let step_count = usize::from(self.segment_byte(segment, position, 0)?);
        position = match position.checked_add(1) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost header = crate::specs::Part1AtkHeader {
                index, descriptor, coefficient_type, width, reversible, whole_sample_filter, initial_odd, whole_sample_extension,
                scaling: raw_kernel_coefficient_option(scaling), step_count, position,
            };
            let ghost target = crate::specs::part1_atk_steps(self.position, segment_view(segment), position,
                width, coefficient_type, reversible, whole_sample_filter, step_count as nat, Seq::empty());
        }
        let mut steps = Vec::with_capacity(step_count);
        #[cfg(verus_keep_ghost)]
        proof! { assert(raw_kernel_steps_view(steps@) =~= Seq::empty()); }
        let mut step = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant step <= step_count <= 255,
                    header.index == index, header.descriptor == descriptor, header.coefficient_type == coefficient_type,
                    header.width == width, header.reversible == reversible, header.whole_sample_filter == whole_sample_filter,
                    header.initial_odd == initial_odd, header.whole_sample_extension == whole_sample_extension,
                    header.scaling == raw_kernel_coefficient_option(scaling),
                    target == crate::specs::part1_atk_steps(self.position, segment_view(segment), position,
                        width, coefficient_type, reversible, whole_sample_filter, (step_count - step) as nat, raw_kernel_steps_view(steps@)),
                    goal == crate::specs::part1_atk_payload(self.position, segment_view(segment)),
                    goal == match target {
                        Err(error) => Err(error),
                        Ok((values, next)) => if next != segment.payload.len() {
                            Err(crate::specs::part1_codestream_malformed(segment.offset, MalformedKind::InvalidMarkerLength))
                        } else { Ok(crate::specs::part1_atk_kernel(header, values)) },
                    },
                decreases step_count - step,
            )
        )]
        while step < step_count {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_atk_steps, 1); }
            let (decoded, next) = self.decode_atk_step(
                segment,
                position,
                width,
                coefficient_type,
                reversible,
                whole_sample_filter,
            )?;
            position = next;
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost previous = raw_kernel_steps_view(steps@); let ghost value = raw_kernel_step_view(decoded); }
            steps.push(decoded);
            #[cfg(verus_keep_ghost)]
            proof! { assert(raw_kernel_steps_view(steps@) =~= previous.push(value)); }
            step = match step.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal_with_fuel(crate::specs::part1_atk_steps, 1);
            reveal(owned_kernel_view); reveal(raw_kernel_view);
        }
        if position != segment.payload.len() {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        Ok(ArbitraryKernel {
            index,
            reversible,
            descriptor,
            coefficient_type,
            whole_sample_filter,
            initial_odd,
            whole_sample_extension,
            scaling,
            steps,
        })
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures kernel_step_outcome(result) == crate::specs::part1_atk_step(
            self.position, segment_view(segment), start, width, coefficient_type, reversible, whole_sample_filter),
    ))]
    fn decode_atk_step(
        &self,
        segment: Segment<'a>,
        start: usize,
        width: usize,
        coefficient_type: u8,
        reversible: bool,
        whole_sample_filter: bool,
    ) -> Result<(KernelLiftingStep, usize), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost goal = crate::specs::part1_atk_step(
                self.position, segment_view(segment), start, width, coefficient_type, reversible, whole_sample_filter);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_atk_step); reveal(crate::specs::part1_atk_step_header); }
        let mut position = start;
        let offset = if whole_sample_filter {
            0
        } else {
            self.segment_byte(segment, position, 0)? as i8
        };
        if !whole_sample_filter {
            position = match position.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
        }
        let exponent = if reversible {
            let value = self.segment_byte(segment, position, 0)?;
            position = match position.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
            value
        } else {
            0
        };
        let residue = if reversible {
            let value = self.read_kernel_coefficient(segment, position, width, coefficient_type)?;
            position = match position.checked_add(width) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
            Some(value)
        } else {
            None
        };
        let coefficient_count = usize::from(self.segment_byte(segment, position, 0)?);
        position = match position.checked_add(1) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        if coefficient_count == 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost target = crate::specs::part1_atk_coefficients(
                self.position, segment_view(segment), position, width, coefficient_type, coefficient_count as nat, Seq::empty());
        }
        let mut coefficients = Vec::with_capacity(coefficient_count);
        #[cfg(verus_keep_ghost)]
        proof! { assert(raw_kernel_coefficients_view(coefficients@) =~= Seq::empty()); }
        let mut coefficient = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant coefficient <= coefficient_count <= 255,
                target == crate::specs::part1_atk_coefficients(self.position, segment_view(segment), position,
                    width, coefficient_type, (coefficient_count - coefficient) as nat,
                    raw_kernel_coefficients_view(coefficients@)),
                goal == crate::specs::part1_atk_step(self.position, segment_view(segment), start,
                    width, coefficient_type, reversible, whole_sample_filter),
                goal == match target {
                    Err(error) => Err(error),
                    Ok((values, next)) => Ok(((offset, exponent, raw_kernel_coefficient_option(residue), values), next)),
                },
            decreases coefficient_count - coefficient,
        ))]
        while coefficient < coefficient_count {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_atk_coefficients, 1); }
            let value = self.read_kernel_coefficient(segment, position, width, coefficient_type)?;
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost previous = raw_kernel_coefficients_view(coefficients@); }
            coefficients.push(value);
            #[cfg(verus_keep_ghost)]
            proof! { assert(raw_kernel_coefficients_view(coefficients@) =~= previous.push(raw_kernel_coefficient_view(value))); }
            position = match position.checked_add(width) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
            coefficient = match coefficient.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_atk_coefficients, 1); reveal(raw_kernel_step_view); }
        Ok((
            KernelLiftingStep {
                offset,
                exponent,
                residue,
                coefficients,
            },
            position,
        ))
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures kernel_coefficient_outcome(result) == crate::specs::part1_kernel_coefficient(
            self.position, segment_view(segment), position, width, coefficient_type),
    ))]
    fn read_kernel_coefficient(
        &self,
        segment: Segment<'a>,
        position: usize,
        width: usize,
        coefficient_type: u8,
    ) -> Result<KernelCoefficient, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_kernel_coefficient); reveal(raw_kernel_coefficient_view); }
        if width == 0 || width > 16 {
            return Err(self.malformed_here(MalformedKind::InvalidTransform));
        }
        let end = match position.checked_add(width) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        let source = match segment.payload.get(position..end) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
            }
        };
        let mut bytes = [0_u8; 16];
        let start = match bytes.len().checked_sub(width) {
            Some(value) => value,
            None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
        };
        let mut copied = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant 0 < width <= 16, start == 16 - width, copied <= width,
                source.len() == width,
                source@ == segment.payload@.subrange(position as int, end as int),
                forall|index: int| 0 <= index < 16 ==> bytes@[index] ==
                    if start <= index < start + copied { source@[index - start as int] } else { 0u8 },
            decreases width - copied,
        ))]
        while copied < width {
            bytes[start + copied] = source[copied];
            copied += 1;
        }
        let negative_integer = match source.first() {
            Some(byte) => *byte & 0x80 != 0,
            None => false,
        };
        if coefficient_type <= 1 && negative_integer {
            let mut fill = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant fill <= start, start == 16 - width, 0 < width <= 16,
                        source.len() == width,
                        source@ == segment.payload@.subrange(position as int, end as int),
                        coefficient_type <= 1, source@[0] & 0x80 != 0,
                        forall|index: int| 0 <= index < 16 ==> bytes@[index] ==
                            if index < fill { 255u8 } else if index < start { 0u8 } else { source@[index - start as int] },
                    decreases start - fill,
                )
            )]
            while fill < start {
                bytes[fill] = 0xff;
                fill += 1;
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(bytes@ =~= crate::specs::part1_kernel_coefficient_bytes(
                segment.payload@, position, width, coefficient_type));
        }
        Ok(KernelCoefficient {
            bytes,
            width: width as u8,
        })
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_nlt_gamma(segment_view(segment)),
    ))]
    fn decode_nlt_gamma(&self, segment: Segment<'a>) -> Result<[u32; 5], DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_nlt_gamma); }
        if segment.payload.len() != 19 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let mut parameters = [0_u32; 5];
        let mut parameter = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant parameter <= 5, segment.payload.len() == 19,
                forall|index: int| 0 <= index < parameter ==>
                    parameters@[index] == crate::specs::part1_nlt_gamma_word(segment.payload@, index),
            decreases 5 - parameter,
        ))]
        while parameter < parameters.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part1_segment_byte); }
            let relative = match parameter.checked_mul(3) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
            let position = match 4_usize.checked_add(relative) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
            parameters[parameter] = (u32::from(self.segment_byte(segment, position, 0)?) << 16)
                | (u32::from(self.segment_byte(segment, position, 1)?) << 8)
                | u32::from(self.segment_byte(segment, position, 2)?);
            parameter = match parameter.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_here(MalformedKind::ArithmeticOverflow)),
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            let expected = [
                crate::specs::part1_nlt_gamma_word(segment.payload@, 0),
                crate::specs::part1_nlt_gamma_word(segment.payload@, 1),
                crate::specs::part1_nlt_gamma_word(segment.payload@, 2),
                crate::specs::part1_nlt_gamma_word(segment.payload@, 3),
                crate::specs::part1_nlt_gamma_word(segment.payload@, 4),
            ];
            assert(parameters@ =~= expected@);
            assert(parameters =~= expected);
        }
        if parameters[0] == 0 || parameters[1] == 0 || parameters[3] == 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        Ok(parameters)
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures segment_tail_outcome(result) == crate::specs::part1_copy_segment_tail(segment_view(segment), start),
    ))]
    fn copy_segment_tail(
        &self,
        segment: Segment<'a>,
        start: usize,
    ) -> Result<Vec<u8>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_copy_segment_tail); }
        if start > segment.payload.len() {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let mut copied = Vec::with_capacity(segment.payload.len() - start);
        let mut position = start;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant start <= position <= segment.payload.len(),
                    copied@ == segment.payload@.subrange(start as int, position as int),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part1_segment_byte); }
            copied.push(self.segment_byte(segment, position, 0)?);
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(segment.payload@.subrange(start as int, position as int + 1) =~=
                    segment.payload@.subrange(start as int, position as int).push(segment.payload@[position as int]));
            }
            position = match position.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
        }
        Ok(copied)
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures cap_payload_outcome(result) == crate::specs::part1_parse_cap(segment_view(segment)),
    ))]
    fn parse_cap(&self, segment: Segment<'a>) -> Result<ExtendedCapabilities, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_cap); reveal(extended_capabilities_view); }
        if segment.payload.len() < 4 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let p_cap = read_u32([
            segment.payload[0],
            segment.payload[1],
            segment.payload[2],
            segment.payload[3],
        ]);
        if p_cap == 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidProfile));
        }
        let capability_count = count_set_bits(p_cap);
        let expected_length = crate::refinement::part15_cap_payload_length(capability_count);
        if segment.payload.len() != expected_length {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let c_capabilities = self.read_marker_words(segment, 4)?;
        Ok(ExtendedCapabilities {
            p_cap,
            c_capabilities,
        })
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures profile_number_outcome(result) == crate::specs::part1_parse_profile_number(segment_view(segment)),
    ))]
    fn parse_profile_number(&self, segment: Segment<'a>) -> Result<ProfileNumber, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_profile_number); reveal(profile_number_view); }
        let remainder = segment.payload.len() % 2;
        if segment.payload.len() < 2 || remainder != 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let words = self.read_marker_words(segment, 0)?;
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_marker_words); }
        if matches!(words.last(), Some(value) if *value == 0) {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidProfile));
        }
        Ok(ProfileNumber { words })
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures marker_words_outcome(result) == crate::specs::part1_marker_words(segment_view(segment), start),
    ))]
    fn read_marker_words(
        &self,
        segment: Segment<'a>,
        start: usize,
    ) -> Result<Vec<u16>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_marker_words); }
        if start > segment.payload.len() {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let mut words = Vec::with_capacity((segment.payload.len() - start) / 2);
        let mut position = start;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    start <= position <= segment.payload.len(),
                    (position - start) % 2 == 0,
                    words@ == crate::specs::part1_marker_word_sequence(
                        segment.payload@.subrange(start as int, position as int)),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal(crate::specs::part1_segment_byte);
                reveal(crate::specs::part1_marker_words);
            }
            let next = match position.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            let first = self.segment_byte(segment, position, 0)?;
            let second = match self.segment_byte(segment, next, 0) {
                Ok(value) => value,
                Err(error) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(next == segment.payload.len());
                        assert((segment.payload.len() - start) % 2 == 1);
                    }
                    return Err(error);
                }
            };
            words.push(read_u16([first, second]));
            let next_position = match position.checked_add(2) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            #[cfg(verus_keep_ghost)]
            proof! {
                let raw = segment.payload@;
                let previous = crate::specs::part1_marker_word_sequence(raw.subrange(start as int, position as int));
                let next_words = crate::specs::part1_marker_word_sequence(raw.subrange(start as int, next_position as int));
                assert(next_words =~= previous.push(crate::specs::part1_marker_value(raw[position as int], raw[position as int + 1]))) by {
                    assert forall|index: int| 0 <= index < next_words.len() implies
                        next_words[index] == previous.push(crate::specs::part1_marker_value(raw[position as int], raw[position as int + 1]))[index] by {
                        if index >= previous.len() { assert(2 * index == position - start); }
                    };
                }
            }
            position = next_position;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(segment.payload@.subrange(start as int, position as int)
                =~= segment.payload@.subrange(start as int, segment.payload.len() as int));
        }
        Ok(words)
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (mct_fragment_outcome(result), mct_pending_view(final(pending)@), final(completed)@)
            == crate::specs::part1_consume_mct_fragment(segment_view(segment), self.limits,
                mct_pending_view(old(pending)@), old(completed)@),
    ))]
    fn consume_mct_fragment(
        &self,
        segment: Segment<'a>,
        pending: &mut Vec<MctFragment>,
        completed: &mut Vec<u8>,
    ) -> Result<Option<MctArray>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = mct_pending_view(pending@);
            let ghost completed_input = completed@;
            let ghost goal = crate::specs::part1_consume_mct_fragment(
                segment_view(segment), self.limits, input, completed_input);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_consume_mct_fragment); }
        let (active_index, maximum_bytes) =
            self.prepare_mct_fragment(segment, pending, completed)?;
        let complete = {
            let state = &pending[active_index];
            crate::refinement::mct_fragment_complete(state.next_index, state.final_index)
        };
        if !complete {
            return Ok(None);
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost prepared = mct_pending_view(pending@); }
        let state = pending.swap_remove(active_index);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(mct_fragment_view(&state) == prepared[active_index as int]);
            assert(mct_pending_view(pending@) =~= crate::specs::part1_mct_fragment_swap_remove(prepared, active_index));
        }
        if state.data.len() as u64 > maximum_bytes {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::TransformElements,
                limit: maximum_bytes,
                actual: state.data.len() as u64,
            });
        }
        let mut payload = Vec::with_capacity(state.data.len().saturating_add(6));
        payload.extend_from_slice(&[0, 0]);
        let descriptor = state.descriptor;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(descriptor >> 8 <= 255u16) by (bit_vector);
            assert(descriptor & 255 <= 255u16) by (bit_vector);
        }
        payload.push((descriptor >> 8) as u8);
        payload.push((descriptor & 255) as u8);
        payload.extend_from_slice(&[0, 0]);
        payload.extend_from_slice(&state.data);
        #[cfg(verus_keep_ghost)]
        proof! { assert(payload@ =~= crate::specs::part1_mct_fragment_payload(mct_fragment_view(&state))); }
        let array = self.parse_mct(Segment {
            marker: MCT,
            offset: state.offset,
            payload: &payload,
        })?;
        completed.push(segment.payload[3]);
        Ok(Some(array))
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, mct_pending_view(final(pending)@)) == crate::specs::part1_prepare_mct_fragment(
            segment_view(segment), self.limits, mct_pending_view(old(pending)@), completed@),
        match result { Ok((index, maximum)) => segment.payload.len() >= 4 && index < final(pending).len()
            && maximum == crate::specs::part1_mct_fragment_maximum_bytes(self.limits), Err(_) => true },
    ))]
    fn prepare_mct_fragment(
        &self,
        segment: Segment<'a>,
        pending: &mut Vec<MctFragment>,
        completed: &[u8],
    ) -> Result<(usize, u64), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost input = mct_pending_view(pending@); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_prepare_mct_fragment); reveal(mct_fragment_view); }
        if segment.payload.len() < 4 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        let fragment_index = read_u16([segment.payload[0], segment.payload[1]]);
        let descriptor = read_u16([segment.payload[2], segment.payload[3]]);
        #[cfg(verus_keep_ghost)]
        proof! { assert(descriptor & 255 <= 255u16) by (bit_vector); }
        let array_index = (descriptor & 0xff) as u8;
        if array_index == 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        let components = self.limits.maximum_components();
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(0 <= components as int <= 65535 ==>
                components as int * components as int <= 4294967296
                && components as int * components as int * 8 <= 34359738368) by (nonlinear_arith);
        }
        let square = match u64::from(components).checked_mul(u64::from(components)) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let maximum_bytes = match square.checked_mul(8) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let state_index = find_mct_fragment(pending, array_index);
        let already_completed = u8_list_contains(completed, array_index);
        let active_index;
        if fragment_index == 0 {
            if segment.payload.len() < 6 || state_index.is_some() || already_completed {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
            }
            let final_index = read_u16([segment.payload[4], segment.payload[5]]);
            if u64::from(final_index) + 1 > maximum_bytes {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::TransformElements,
                    limit: maximum_bytes,
                    actual: u64::from(final_index) + 1,
                });
            }
            if segment.payload[6..].len() as u64 > maximum_bytes {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::TransformElements,
                    limit: maximum_bytes,
                    actual: segment.payload[6..].len() as u64,
                });
            }
            let mut data = Vec::new();
            data.extend_from_slice(&segment.payload[6..]);
            #[cfg(verus_keep_ghost)]
            proof! { assert(data@ =~= segment.payload@.subrange(6, segment.payload.len() as int)); }
            active_index = pending.len();
            pending.push(MctFragment {
                descriptor,
                next_index: 1,
                final_index,
                offset: segment.offset,
                data,
            });
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(mct_pending_view(pending@) =~= input.push(crate::specs::Part1MctFragment {
                    descriptor, next_index: 1, final_index, offset: segment.offset,
                    data: segment.payload@.subrange(6, segment.payload.len() as int),
                }));
            }
        } else {
            let index = match state_index {
                Some(index) => index,
                None => {
                    return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
                }
            };
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost before = mct_fragment_view(&pending[index as int]);
                let ghost appended_data = before.data + segment.payload@.subrange(4, segment.payload.len() as int);
            }
            let state = &mut pending[index];
            if state.descriptor != descriptor
                || fragment_index != state.next_index
                || fragment_index > state.final_index
            {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
            }
            state.data.extend_from_slice(&segment.payload[4..]);
            #[cfg(verus_keep_ghost)]
            proof! { assert(state.data@ =~= appended_data); }
            let actual_bytes = state.data.len() as u64;
            if actual_bytes > maximum_bytes {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(mct_pending_view(pending@) =~= input.update(index as int,
                        crate::specs::part1_mct_fragment_updated(before, appended_data, before.next_index)));
                }
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::TransformElements,
                    limit: maximum_bytes,
                    actual: actual_bytes,
                });
            }
            let next_index = state.next_index;
            if next_index == u16::MAX {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(mct_pending_view(pending@) =~= input.update(index as int,
                        crate::specs::part1_mct_fragment_updated(before, appended_data, before.next_index)));
                }
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
            state.next_index = crate::refinement::mct_fragment_next(next_index);
            active_index = index;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(mct_pending_view(pending@) =~= input.update(index as int,
                    crate::specs::part1_mct_fragment_updated(before, appended_data, (next_index + 1) as u16)));
            }
        }
        Ok((active_index, maximum_bytes))
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (mcc_fragment_outcome(result), mcc_pending_view(final(pending)@), final(completed)@)
            == crate::specs::part1_consume_mcc_fragment(segment_view(segment), components, self.limits,
                mcc_pending_view(old(pending)@), old(completed)@),
    ))]
    fn consume_mcc_fragment(
        &self,
        segment: Segment<'a>,
        components: u16,
        pending: &mut Vec<MccFragment>,
        completed: &mut Vec<u8>,
    ) -> Result<Option<MctRecord>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = mcc_pending_view(pending@);
            let ghost completed_input = completed@;
            let ghost goal = crate::specs::part1_consume_mcc_fragment(
                segment_view(segment), components, self.limits, input, completed_input);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_consume_mcc_fragment); }
        let (active_index, maximum_bytes) =
            self.prepare_mcc_fragment(segment, components, pending, completed)?;
        let complete = {
            let state = &pending[active_index];
            crate::refinement::mct_fragment_complete(state.next_index, state.final_index)
        };
        if !complete {
            return Ok(None);
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost prepared = mcc_pending_view(pending@); }
        let state = pending.swap_remove(active_index);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(mcc_fragment_view(&state) == prepared[active_index as int]);
            assert(mcc_pending_view(pending@) =~= crate::specs::part1_mcc_fragment_swap_remove(prepared, active_index));
        }
        if state.data.len() as u64 > maximum_bytes {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::TransformElements,
                limit: maximum_bytes,
                actual: state.data.len() as u64,
            });
        }
        let mut payload = Vec::with_capacity(state.data.len().saturating_add(7));
        payload.extend_from_slice(&[0, 0]);
        payload.push(state.record_index);
        payload.extend_from_slice(&[0, 0]);
        let count = state.collection_count;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(count >> 8 <= 255u16) by (bit_vector);
            assert(count & 255 <= 255u16) by (bit_vector);
        }
        payload.push((count >> 8) as u8);
        payload.push((count & 255) as u8);
        payload.extend_from_slice(&state.data);
        #[cfg(verus_keep_ghost)]
        proof! { assert(payload@ =~= crate::specs::part1_mcc_fragment_payload(mcc_fragment_view(&state))); }
        let record = self.parse_mcc(
            Segment {
                marker: MCC,
                offset: state.offset,
                payload: &payload,
            },
            components,
        )?;
        completed.push(segment.payload[2]);
        Ok(Some(record))
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, mcc_pending_view(final(pending)@)) == crate::specs::part1_prepare_mcc_fragment(
            segment_view(segment), components, mcc_pending_view(old(pending)@), completed@),
        match result { Ok((index, maximum)) => segment.payload.len() >= 3 && index < final(pending).len()
            && maximum == crate::specs::part1_mcc_fragment_maximum_bytes(components), Err(_) => true },
    ))]
    fn prepare_mcc_fragment(
        &self,
        segment: Segment<'a>,
        components: u16,
        pending: &mut Vec<MccFragment>,
        completed: &[u8],
    ) -> Result<(usize, u64), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost input = mcc_pending_view(pending@); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_prepare_mcc_fragment); reveal(mcc_fragment_view); }
        if segment.payload.len() < 3 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        let fragment_index = read_u16([segment.payload[0], segment.payload[1]]);
        let record_index = segment.payload[2];
        if record_index == 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        let scaled = match u64::from(components).checked_mul(4) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let per_component = match scaled.checked_add(12) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(0 <= components as int <= 65535 ==>
                components as int * (components as int * 4 + 12) <= 34359738368) by (nonlinear_arith);
        }
        let maximum_bytes = match u64::from(components).checked_mul(per_component) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let state_index = find_mcc_fragment(pending, record_index);
        let already_completed = u8_list_contains(completed, record_index);
        let active_index;
        if fragment_index == 0 {
            if segment.payload.len() < 7 || state_index.is_some() || already_completed {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
            }
            let final_index = read_u16([segment.payload[3], segment.payload[4]]);
            let collection_count = read_u16([segment.payload[5], segment.payload[6]]);
            if u64::from(final_index) + 1 > maximum_bytes {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::TransformElements,
                    limit: maximum_bytes,
                    actual: u64::from(final_index) + 1,
                });
            }
            if segment.payload[7..].len() as u64 > maximum_bytes {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::TransformElements,
                    limit: maximum_bytes,
                    actual: segment.payload[7..].len() as u64,
                });
            }
            let mut data = Vec::new();
            data.extend_from_slice(&segment.payload[7..]);
            #[cfg(verus_keep_ghost)]
            proof! { assert(data@ =~= segment.payload@.subrange(7, segment.payload.len() as int)); }
            active_index = pending.len();
            pending.push(MccFragment {
                record_index,
                next_index: 1,
                final_index,
                collection_count,
                offset: segment.offset,
                data,
            });
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(mcc_pending_view(pending@) =~= input.push(crate::specs::Part1MccFragment {
                    record_index, next_index: 1, final_index, collection_count, offset: segment.offset,
                    data: segment.payload@.subrange(7, segment.payload.len() as int),
                }));
            }
        } else {
            let index = match state_index {
                Some(index) => index,
                None => {
                    return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
                }
            };
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost before = mcc_fragment_view(&pending[index as int]);
                let ghost appended_data = before.data + segment.payload@.subrange(3, segment.payload.len() as int);
            }
            let state = &mut pending[index];
            if fragment_index != state.next_index || fragment_index > state.final_index {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
            }
            state.data.extend_from_slice(&segment.payload[3..]);
            #[cfg(verus_keep_ghost)]
            proof! { assert(state.data@ =~= appended_data); }
            let actual_bytes = state.data.len() as u64;
            if actual_bytes > maximum_bytes {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(mcc_pending_view(pending@) =~= input.update(index as int,
                        crate::specs::part1_mcc_fragment_updated(before, appended_data, before.next_index)));
                }
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::TransformElements,
                    limit: maximum_bytes,
                    actual: actual_bytes,
                });
            }
            let next_index = state.next_index;
            if next_index == u16::MAX {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(mcc_pending_view(pending@) =~= input.update(index as int,
                        crate::specs::part1_mcc_fragment_updated(before, appended_data, before.next_index)));
                }
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
            state.next_index = crate::refinement::mct_fragment_next(next_index);
            active_index = index;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(mcc_pending_view(pending@) =~= input.update(index as int,
                    crate::specs::part1_mcc_fragment_updated(before, appended_data, (next_index + 1) as u16)));
            }
        }
        Ok((active_index, maximum_bytes))
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures mct_array_outcome(result) == crate::specs::part1_parse_mct(segment_view(segment), self.limits),
    ))]
    fn parse_mct(&self, segment: Segment<'a>) -> Result<MctArray, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_parse_mct); reveal(crate::specs::part1_mct_header);
            mct_fixed_precision_is_q32();
        }
        if segment.payload.len() < 8 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        let z = read_u16([segment.payload[0], segment.payload[1]]);
        let descriptor = read_u16([segment.payload[2], segment.payload[3]]);
        let y = read_u16([segment.payload[4], segment.payload[5]]);
        if z != 0 || y != 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        if descriptor & 0xf000 != 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        #[cfg(verus_keep_ghost)]
        proof! { assert(descriptor & 255 <= 255u16) by (bit_vector); }
        let index = (descriptor & 0xff) as u8;
        if index == 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        let kind = match (descriptor >> 8) & 3 {
            0 => MctArrayKind::Dependency,
            1 => MctArrayKind::Decorrelation,
            2 => MctArrayKind::Offset,
            _ => {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
            }
        };
        let (numeric_format, element_bytes) = match (descriptor >> 10) & 3 {
            0 => (MctNumericFormat::Int16, 2_usize),
            1 => (MctNumericFormat::Int32, 4_usize),
            2 => (MctNumericFormat::Float32, 4_usize),
            _ => (MctNumericFormat::Float64, 8_usize),
        };
        let data = &segment.payload[6..];
        let remainder = data.len() % element_bytes;
        if data.is_empty() || remainder != 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        let element_count = data.len() / element_bytes;
        let cap = self.limits.maximum_components();
        #[cfg(verus_keep_ghost)]
        proof! { assert(0 <= cap as int <= 65535 ==> cap as int * cap as int <= 4294836225) by (nonlinear_arith); }
        let maximum = match u64::from(cap).checked_mul(u64::from(cap)) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if element_count as u64 > maximum {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::TransformElements,
                limit: maximum,
                actual: element_count as u64,
            });
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
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost goal = crate::specs::part1_parse_mct(segment_view(segment), self.limits);
            let ghost target = crate::specs::part1_mct_values(segment.offset, data@, numeric_format, 0, Seq::empty());
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod(data.len() as int, element_bytes as int);
            assert(element_bytes as int * remaining_elements as int == remaining_elements as int * element_bytes as int) by (nonlinear_arith);
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= data.len(), remaining_elements <= element_count,
                    element_bytes == crate::specs::part1_mct_element_width(numeric_format),
                    element_bytes == 2 || element_bytes == 4 || element_bytes == 8,
                    position + remaining_elements as int * element_bytes as int == data.len(),
                    target == crate::specs::part1_mct_values(segment.offset, data@, numeric_format, position as nat, values@),
                    goal == crate::specs::part1_parse_mct(segment_view(segment), self.limits),
                    goal == match target { Err(error) => Err(error), Ok(parsed) => Ok((index, kind, numeric_format, fraction_bits, parsed)) },
                decreases remaining_elements,
            )
        )]
        while position < data.len() {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost start = position; }
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(crate::specs::part1_mct_values, 1);
                assert(element_bytes > 0 && position + remaining_elements as int * element_bytes as int == data.len()
                    && position < data.len() ==> remaining_elements > 0 && position + element_bytes <= data.len()) by (nonlinear_arith);
            }
            if remaining_elements == 0 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
            }
            remaining_elements -= 1;
            let value = self.decode_mct_element(data, position, numeric_format, segment.offset)?;
            values.push(value);
            position = match position.checked_add(element_bytes) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(position == start + element_bytes);
                assert((remaining_elements as int + 1) * element_bytes as int ==
                    remaining_elements as int * element_bytes as int + element_bytes as int) by (nonlinear_arith);
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_mct_values, 1); reveal(mct_array_view); }
        Ok(MctArray {
            index,
            kind,
            numeric_format,
            fraction_bits,
            values,
        })
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_mct_element(offset, data@, position, numeric_format),
    ))]
    fn decode_mct_element(
        &self,
        data: &[u8],
        position: usize,
        numeric_format: MctNumericFormat,
        offset: usize,
    ) -> Result<i64, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_mct_element); }
        let width = match numeric_format {
            MctNumericFormat::Int16 => 2usize,
            MctNumericFormat::Int32 | MctNumericFormat::Float32 => 4,
            MctNumericFormat::Float64 => 8,
        };
        if position > data.len() || data.len() - position < width {
            return Err(self.malformed_at(offset, MalformedKind::InvalidTransform));
        }
        let position_1 = match position.checked_add(1) {
            Some(value) => value,
            None => return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow)),
        };
        let byte_0 = data[position];
        let byte_1 = data[position_1];
        match numeric_format {
            MctNumericFormat::Int16 => Ok(signed_u16(read_u16([byte_0, byte_1]))),
            MctNumericFormat::Int32 | MctNumericFormat::Float32 => {
                let position_2 = match position.checked_add(2) {
                    Some(value) => value,
                    None => {
                        return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow));
                    }
                };
                let position_3 = match position.checked_add(3) {
                    Some(value) => value,
                    None => {
                        return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow));
                    }
                };
                let bits = read_u32([byte_0, byte_1, data[position_2], data[position_3]]);
                if matches!(numeric_format, MctNumericFormat::Int32) {
                    Ok(signed_u32(bits))
                } else {
                    match float32_to_fixed(bits) {
                        Ok(value) => Ok(value),
                        Err(()) => Err(self.malformed_at(offset, MalformedKind::InvalidTransform)),
                    }
                }
            }
            MctNumericFormat::Float64 => {
                let mut bytes = [0_u8; 8];
                let mut byte_index = 0_usize;
                #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant byte_index <= 8, position as int + 8 <= data.len(),
                        forall|index: int| 0 <= index < byte_index ==> bytes@[index] == data@[position as int + index],
                    decreases 8 - byte_index,
                ))]
                while byte_index < bytes.len() {
                    let source = match position.checked_add(byte_index) {
                        Some(value) => value,
                        None => {
                            return Err(
                                self.malformed_at(offset, MalformedKind::ArithmeticOverflow)
                            );
                        }
                    };
                    bytes[byte_index] = data[source];
                    byte_index += 1;
                }
                #[cfg(verus_keep_ghost)]
                proof! { assert(bytes@ =~= data@.subrange(position as int, position as int + 8)); }
                match float64_to_fixed(read_u64(bytes)) {
                    Ok(value) => Ok(value),
                    Err(()) => Err(self.malformed_at(offset, MalformedKind::InvalidTransform)),
                }
            }
        }
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures mcc_record_outcome(result) == crate::specs::part1_parse_mcc(segment_view(segment), components, self.limits),
    ))]
    fn parse_mcc(&self, segment: Segment<'a>, components: u16) -> Result<MctRecord, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_mcc); }
        if segment.payload.len() < 7 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        if read_u16([segment.payload[0], segment.payload[1]]) != 0
            || read_u16([segment.payload[3], segment.payload[4]]) != 0
        {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        let index = segment.payload[2];
        let collection_count = read_u16([segment.payload[5], segment.payload[6]]);
        if index == 0 || collection_count == 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        if collection_count > self.limits.maximum_components() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::TransformElements,
                limit: u64::from(self.limits.maximum_components()),
                actual: u64::from(collection_count),
            });
        }
        let mut position = 7_usize;
        let mut collections = Vec::with_capacity(usize::from(collection_count));
        #[cfg(verus_keep_ghost)]
        proof! { assert(mcc_collection_references_view(collections@) =~= Seq::empty()); }
        let mut collection = 0_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant collection <= collection_count,
                    segment.payload.len() >= 7, index == segment.payload@[2],
                    crate::specs::part1_parse_mcc(segment_view(segment), components, self.limits)
                        == crate::specs::part1_mcc_records(segment_view(segment), components, position,
                            (collection_count - collection) as nat, mcc_collection_references_view(collections@)),
                decreases collection_count - collection,
            )
        )]
        while collection < collection_count {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_mcc_records, 1); }
            let (decoded, next) = self.decode_mcc_collection(segment, components, position)?;
            position = next;
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost previous = mcc_collection_references_view(collections@);
                let ghost value = mcc_collection_reference_view(&decoded);
            }
            collections.push(decoded);
            #[cfg(verus_keep_ghost)]
            proof! { assert(mcc_collection_references_view(collections@) =~= previous.push(value)); }
            collection = match collection.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_mcc_records, 1); }
        if position != segment.payload.len() {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        validate_parallel_collections(&collections, segment.offset)?;
        Ok(MctRecord { index, collections })
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures mcc_collection_outcome(result) == crate::specs::part1_mcc_collection(segment_view(segment), components, start),
    ))]
    fn decode_mcc_collection(
        &self,
        segment: Segment<'a>,
        components: u16,
        start: usize,
    ) -> Result<(MctCollectionReference, usize), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_mcc_collection); reveal(mcc_collection_reference_view);
            reveal(mcc_reference_transform_view); reveal(mct_wavelet_view);
        }
        let mut position = start;
        let transform_kind_byte = match segment.payload.get(position) {
            Some(value) => *value,
            None => return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform)),
        };
        position = match position.checked_add(1) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if transform_kind_byte & 0xfc != 0 || transform_kind_byte & 3 == 2 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(transform_kind_byte & 0xfc == 0 ==> transform_kind_byte == (transform_kind_byte & 3)) by (bit_vector);
            assert(transform_kind_byte & 3 <= 3u8) by (bit_vector);
        }
        let transform_kind = transform_kind_byte & 3;
        let input_components =
            parse_mcc_component_list(segment.payload, &mut position, components, segment.offset)?;
        let output_components =
            parse_mcc_component_list(segment.payload, &mut position, components, segment.offset)?;
        if input_components.is_empty()
            || position > segment.payload.len()
            || segment.payload.len() - position < 3
        {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        let transform = (u32::from(segment.payload[position]) << 16)
            | (u32::from(segment.payload[position + 1]) << 8)
            | u32::from(segment.payload[position + 2]);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(transform & 255 <= 255u32) by (bit_vector);
            assert((transform >> 8) & 255 <= 255u32) by (bit_vector);
            assert((transform >> 16) & 63 <= 63u32) by (bit_vector);
        }
        position = match position.checked_add(3) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let reference_transform = match transform_kind {
            0 | 1 => {
                if transform & 0xfe0000 != 0
                    || input_components.len() != output_components.len()
                    || (transform_kind == 0
                        && !u16_lists_equal(&input_components, &output_components))
                {
                    return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
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
                    || position > segment.payload.len()
                    || segment.payload.len() - position < 4
                {
                    return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
                }
                let wavelet_type = match (transform & 0xff) as u8 {
                    0 => MctWaveletType::Irreversible9x7,
                    1 => MctWaveletType::Reversible5x3,
                    _ => {
                        return Err(
                            self.malformed_at(segment.offset, MalformedKind::InvalidTransform)
                        );
                    }
                };
                let levels = ((transform >> 16) & 0x3f) as u8;
                if levels > 32 {
                    return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
                }
                let offset_end = match position.checked_add(4) {
                    Some(value) => value,
                    None => {
                        return Err(
                            self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                        );
                    }
                };
                let offset_bytes = match segment.payload.get(position..offset_end) {
                    Some(value) => value,
                    None => {
                        return Err(
                            self.malformed_at(segment.offset, MalformedKind::InvalidTransform)
                        );
                    }
                };
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
                    component_offset: component_offset as i32,
                })
            }
            _ => return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform)),
        };
        Ok((
            MctCollectionReference {
                input_components,
                output_components,
                transform: reference_transform,
            },
            position,
        ))
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures segment_tail_outcome(result) == crate::specs::part1_parse_mco(segment_view(segment)),
    ))]
    fn parse_mco(&self, segment: Segment<'a>) -> Result<Vec<u8>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_mco); }
        let count = match segment.payload.first() {
            Some(value) => usize::from(*value),
            None => return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform)),
        };
        if segment.payload.len() != count + 1 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
        }
        let mut order = Vec::with_capacity(count);
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= count <= 255, order.len() == index,
                    segment.payload.len() == count + 1,
                    crate::specs::part1_parse_mco(segment_view(segment))
                        == crate::specs::part1_mco_run(segment_view(segment), index as nat, order@),
                decreases count - index,
            )
        )]
        while index < count {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_mco_run, 1); }
            let stage = match segment.payload.get(index + 1) {
                Some(value) => *value,
                None => {
                    return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
                }
            };
            let mut previous = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant previous <= order.len(), order.len() == index,
                        index < count <= 255, segment.payload.len() == count + 1,
                        stage == segment.payload@[index as int + 1],
                        forall|position: int| 0 <= position < previous ==> order@[position] != stage,
                        crate::specs::part1_parse_mco(segment_view(segment))
                            == crate::specs::part1_mco_run(segment_view(segment), index as nat, order@),
                    decreases order.len() - previous,
                )
            )]
            while previous < order.len() {
                #[cfg(verus_keep_ghost)]
                proof! { reveal_with_fuel(crate::specs::part1_mco_run, 1); }
                if order[previous] == stage {
                    #[cfg(verus_keep_ghost)]
                    proof! { assert(order@.contains(stage)); }
                    return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
                }
                previous = match previous.checked_add(1) {
                    Some(value) => value,
                    None => {
                        return Err(
                            self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                        );
                    }
                };
            }
            #[cfg(verus_keep_ghost)]
            proof! { assert(!order@.contains(stage)); }
            if stage == 0 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidTransform));
            }
            order.push(stage);
            index = match index.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_mco_run, 1); }
        Ok(order)
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures mct_graph_outcome(result) == crate::specs::part1_mct_graph(mct_parameters_view(parameters)),
    ))]
    fn resolve_multiple_component_transform(
        &self,
        parameters: &MctParameters,
    ) -> Result<Option<MultipleComponentTransform>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost goal = crate::specs::part1_mct_graph(mct_parameters_view(parameters)); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_mct_graph); reveal(mct_parameters_view); }
        if parameters.order.is_empty() {
            return Ok(None);
        }
        let mut stages = Vec::with_capacity(parameters.order.len());
        #[cfg(verus_keep_ghost)]
        proof! { assert(mct_stages_view(stages@) =~= Seq::empty()); }
        let mut order_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant order_index <= parameters.order.len(),
                    goal == crate::specs::part1_mct_graph(mct_parameters_view(parameters)),
                    goal == crate::specs::part1_mct_graph_from(
                        mct_parameters_view(parameters), order_index as nat, mct_stages_view(stages@)),
                decreases parameters.order.len() - order_index,
            )
        )]
        while order_index < parameters.order.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_mct_graph_from, 1); reveal(mct_parameters_view); }
            let record_index = parameters.order[order_index];
            let position = match find_mct_record_index(&parameters.records, record_index) {
                Some(position) => position,
                None => return Err(self.malformed_at(0, MalformedKind::InvalidTransform)),
            };
            let stage =
                self.resolve_mct_stage(&parameters.records[position], &parameters.arrays)?;
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = mct_stages_view(stages@); let ghost value = mct_stage_view(&stage); }
            stages.push(stage);
            #[cfg(verus_keep_ghost)]
            proof! { assert(mct_stages_view(stages@) =~= before.push(value)); }
            order_index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_mct_graph_from, 1); }
        let graph = MultipleComponentTransform { stages };
        #[cfg(verus_keep_ghost)]
        proof! { assert(multiple_component_transform_view(&graph) =~= mct_stages_view(graph.stages@)); }
        Ok(Some(graph))
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures mct_stage_outcome(result) == crate::specs::part1_mct_graph_stage(
            mct_record_view(record), mct_arrays_view(arrays@)),
    ))]
    fn resolve_mct_stage(
        &self,
        record: &MctRecord,
        arrays: &[MctArray],
    ) -> Result<MctStage, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost goal = crate::specs::part1_mct_graph_stage(mct_record_view(record), mct_arrays_view(arrays@)); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_mct_graph_stage); reveal(mct_record_view); }
        let mut collections = Vec::with_capacity(record.collections.len());
        #[cfg(verus_keep_ghost)]
        proof! { assert(mct_collections_view(collections@) =~= Seq::empty()); }
        let mut position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant position <= record.collections.len(),
                goal == crate::specs::part1_mct_graph_stage(mct_record_view(record), mct_arrays_view(arrays@)),
                goal == crate::specs::part1_mct_graph_stage_from(
                    mct_record_view(record), mct_arrays_view(arrays@), position as nat, mct_collections_view(collections@)),
            decreases record.collections.len() - position,
        ))]
        while position < record.collections.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_mct_graph_stage_from, 1); reveal(mct_record_view); }
            let collection = self.resolve_mct_collection(&record.collections[position], arrays)?;
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = mct_collections_view(collections@); let ghost value = mct_collection_view(&collection); }
            collections.push(collection);
            #[cfg(verus_keep_ghost)]
            proof! { assert(mct_collections_view(collections@) =~= before.push(value)); }
            position += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_mct_graph_stage_from, 1); }
        let stage = MctStage {
            index: record.index,
            collections,
        };
        #[cfg(verus_keep_ghost)]
        proof! { assert(mct_stage_view(&stage).1 =~= mct_collections_view(stage.collections@)); }
        Ok(stage)
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures mct_collection_outcome(result) == crate::specs::part1_mct_graph_collection(
            mcc_collection_reference_view(reference), mct_arrays_view(arrays@)),
    ))]
    fn resolve_mct_collection(
        &self,
        reference: &MctCollectionReference,
        arrays: &[MctArray],
    ) -> Result<MctCollection, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_mct_graph_collection);
            reveal(mcc_collection_reference_view); reveal(mcc_reference_transform_view);
            reveal(mct_array_view); reveal(mct_wavelet_view);
        }
        let rows = reference.input_components.len();
        let columns = reference.output_components.len();
        if rows == 0 || rows != columns {
            return Err(self.malformed_at(0, MalformedKind::InvalidTransform));
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
                    Some(referenced_mct_array(arrays, offset_index, MctArrayKind::Offset)?.clone())
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
                    Some(referenced_mct_array(arrays, transform_index, kind)?.clone())
                };
                let expected_elements =
                    self.expected_mct_elements(rows, columns, kind, reversible)?;
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
                    return Err(self.malformed_at(0, MalformedKind::InvalidTransform));
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
        Ok(collection)
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_mct_graph_element_count(rows, columns, kind, reversible),
    ))]
    fn expected_mct_elements(
        &self,
        rows: usize,
        columns: usize,
        kind: MctArrayKind,
        reversible: bool,
    ) -> Result<usize, DecodeError> {
        if mct_array_kinds_equal(kind, MctArrayKind::Decorrelation) {
            match rows.checked_mul(columns) {
                Some(value) => Ok(value),
                None => Err(self.malformed_at(0, MalformedKind::ArithmeticOverflow)),
            }
        } else if reversible {
            let partner = match rows.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_at(0, MalformedKind::ArithmeticOverflow)),
            };
            let product = match rows.checked_mul(partner) {
                Some(value) => value,
                None => return Err(self.malformed_at(0, MalformedKind::ArithmeticOverflow)),
            };
            match (product / 2).checked_sub(1) {
                Some(value) => Ok(value),
                None => Err(self.malformed_at(0, MalformedKind::ArithmeticOverflow)),
            }
        } else {
            let partner = rows.saturating_sub(1);
            let product = match rows.checked_mul(partner) {
                Some(value) => value,
                None => return Err(self.malformed_at(0, MalformedKind::ArithmeticOverflow)),
            };
            Ok(product / 2)
        }
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

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures parsed_progression_outcome(result) == crate::specs::part1_parse_poc(segment_view(segment), components),
    ))]
    fn parse_poc(
        &self,
        segment: Segment<'a>,
        components: u16,
    ) -> Result<ProgressionChanges, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_poc); }
        let component_bytes = if components < 257 { 1_usize } else { 2_usize };
        let doubled = match component_bytes.checked_mul(2) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let record_bytes = match doubled.checked_add(5) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let remainder = segment.payload.len() % record_bytes;
        if segment.payload.is_empty() || remainder != 0 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let mut records = Vec::with_capacity(segment.payload.len() / record_bytes);
        let mut position = 0_usize;
        let mut remaining_records = segment.payload.len() / record_bytes;
        #[cfg(verus_keep_ghost)]
        proof! {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod(segment.payload.len() as int, record_bytes as int);
            assert((record_bytes as int) * (remaining_records as int) ==
                (remaining_records as int) * (record_bytes as int)) by (nonlinear_arith);
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= segment.payload.len(),
                    component_bytes == if components < 257 { 1usize } else { 2usize },
                    record_bytes == 2 * component_bytes + 5,
                    record_bytes == 7 || record_bytes == 9,
                    position + remaining_records as int * record_bytes as int == segment.payload.len(),
                    crate::specs::part1_parse_poc(segment_view(segment), components)
                        == crate::specs::part1_poc_records(segment_view(segment), components, position as nat, records@),
                decreases remaining_records,
            )
        )]
        while position < segment.payload.len() {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost record_start = position; }
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(crate::specs::part1_poc_records, 1);
                reveal(crate::specs::part1_poc_byte); reveal(crate::specs::part1_poc_component);
                assert(record_bytes > 0 && position + remaining_records as int * record_bytes as int == segment.payload.len()
                    && position < segment.payload.len() ==> remaining_records > 0
                        && position + record_bytes <= segment.payload.len()) by (nonlinear_arith);
                assert(remaining_records > 0);
                assert(position + record_bytes <= segment.payload.len());
            }
            if remaining_records == 0 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
            }
            remaining_records -= 1;
            let resolution_start = self.read_poc_byte(segment, position)?;
            position = match position.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            let component_start = self.read_poc_component(segment, position, component_bytes)?;
            position = match position.checked_add(component_bytes) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            let layer_end_position = match position.checked_add(2) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            let layer_bytes = match segment.payload.get(position..layer_end_position) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength)
                    );
                }
            };
            let layer_end = read_u16([layer_bytes[0], layer_bytes[1]]);
            position = match position.checked_add(2) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            let resolution_end = self.read_poc_byte(segment, position)?;
            position = match position.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            let encoded_component_end =
                self.read_poc_component(segment, position, component_bytes)?;
            let component_end =
                if component_bytes == 1 && encoded_component_end == 0 && components == 256 {
                    256
                } else {
                    encoded_component_end
                };
            position = match position.checked_add(component_bytes) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            let progression_order = match self.read_poc_byte(segment, position)? {
                0 => ProgressionOrder::Lrcp,
                1 => ProgressionOrder::Rlcp,
                2 => ProgressionOrder::Rpcl,
                3 => ProgressionOrder::Pcrl,
                4 => ProgressionOrder::Cprl,
                _ => {
                    return Err(self.malformed_at(
                        segment.offset,
                        MalformedKind::InvalidProgressionOrderChange,
                    ));
                }
            };
            position = match position.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            records.push(ProgressionChange {
                resolution_start,
                component_start,
                layer_end,
                resolution_end,
                component_end,
                progression_order,
            });
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(position == record_start + record_bytes);
                assert((remaining_records as int + 1) * (record_bytes as int) ==
                    (remaining_records as int) * (record_bytes as int) + record_bytes as int) by (nonlinear_arith);
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_poc_records, 1); reveal(progression_changes_view); }
        Ok(ProgressionChanges { records })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_poc_component(segment_view(segment), position, width),
    ))]
    fn read_poc_component(
        &self,
        segment: Segment<'a>,
        position: usize,
        width: usize,
    ) -> Result<u16, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_poc_component); }
        if width == 1 {
            let value = match segment.payload.get(position) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength)
                    );
                }
            };
            return Ok(*value as u16);
        }
        let end = match position.checked_add(2) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let bytes = match segment.payload.get(position..end) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
            }
        };
        Ok(read_u16([bytes[0], bytes[1]]))
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_poc_byte(segment_view(segment), position),
    ))]
    fn read_poc_byte(&self, segment: Segment<'a>, position: usize) -> Result<u8, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_poc_byte); }
        let value = match segment.payload.get(position) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
            }
        };
        Ok(*value)
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_validate_progression_changes(
            offset, progression_changes_view(changes), layers, component_styles, self.limits),
    ))]
    fn validate_progression_changes(
        &self,
        changes: &ProgressionChanges,
        layers: u16,
        component_styles: &ComponentCodingStyles,
        offset: usize,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_validate_progression_changes); }
        if changes.is_empty() {
            return Ok(());
        }
        let components = component_styles.components;
        let (active_coordinates, maximum_resolutions) =
            self.progression_validation_shape(layers, component_styles, offset)?;
        let validation_work =
            match (changes.records.len() as u64).checked_mul(active_coordinates as u64) {
                Some(value) => value,
                None => return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow)),
            };
        if validation_work > self.limits.maximum_work_units() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: self.limits.maximum_work_units(),
                actual: validation_work,
            });
        }
        let mut owners = vec![false; active_coordinates];
        #[cfg(verus_keep_ghost)]
        proof! { assert(owners@ =~= Seq::new(active_coordinates as nat, |index: int| false)); }
        let mut record_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant record_index <= changes.records.len(),
                    components == component_coding_style_count(component_styles),
                    crate::specs::part1_validate_progression_changes(
                        offset, progression_changes_view(changes), layers, component_styles, self.limits)
                        == crate::specs::part1_poc_admit_records(offset, progression_changes_view(changes),
                            layers, component_styles, maximum_resolutions, record_index as nat, owners@),
                decreases changes.records.len() - record_index,
            )
        )]
        while record_index < changes.records.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_poc_admit_records, 1); }
            let record = changes.records[record_index];
            if record.layer_end == 0
                || record.layer_end > layers
                || record.component_start >= record.component_end
                || record.component_end > components
                || record.resolution_start >= record.resolution_end
                || record.resolution_end > maximum_resolutions
            {
                return Err(self.malformed_at(offset, MalformedKind::InvalidProgressionOrderChange));
            }
            let contributed = self.mark_progression_record(
                record,
                layers,
                component_styles,
                &mut owners,
                offset,
            )?;
            if !contributed {
                return Err(self.malformed_at(offset, MalformedKind::InvalidProgressionOrderChange));
            }
            record_index = match record_index.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow)),
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_poc_admit_records, 1); }
        if !progression_owners_complete(&owners) {
            return Err(self.malformed_at(offset, MalformedKind::InvalidProgressionOrderChange));
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_poc_validation_shape(offset, layers, component_styles, 0, 0, 0),
    ))]
    fn progression_validation_shape(
        &self,
        layers: u16,
        component_styles: &ComponentCodingStyles,
        offset: usize,
    ) -> Result<(usize, u8), DecodeError> {
        let components = component_styles.components;
        let mut active_coordinates = 0_usize;
        let mut maximum_resolutions = 0_u8;
        let mut component = 0_u16;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant component <= components,
                components == component_coding_style_count(component_styles),
                crate::specs::part1_poc_validation_shape(offset, layers, component_styles, 0, 0, 0)
                    == crate::specs::part1_poc_validation_shape(offset, layers, component_styles,
                        component, active_coordinates, maximum_resolutions),
            decreases components - component,
        ))]
        while component < components {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_poc_validation_shape, 1); }
            let depth = component_styles.style(component)?.decomposition_levels;
            let resolutions = match depth.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow)),
            };
            if resolutions > maximum_resolutions {
                maximum_resolutions = resolutions;
            }
            let amount = match usize::from(layers).checked_mul(usize::from(resolutions)) {
                Some(value) => value,
                None => return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow)),
            };
            active_coordinates = match active_coordinates.checked_add(amount) {
                Some(value) => value,
                None => return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow)),
            };
            component = match component.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow)),
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_poc_validation_shape, 1); }
        Ok((active_coordinates, maximum_resolutions))
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, final(owners)@) == crate::specs::part1_poc_mark_resolutions(
            offset, record, coordinate, 0, old(owners)@, progress),
    ))]
    fn mark_progression_resolutions(
        &self,
        record: ProgressionChange,
        coordinate: (u16, u16, u8),
        owners: &mut [bool],
        progress: (usize, bool),
        offset: usize,
    ) -> Result<(usize, bool), DecodeError> {
        let mut linear = progress.0;
        let mut contributed = progress.1;
        let mut resolution = 0_u8;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = owners@;
            let ghost goal = crate::specs::part1_poc_mark_resolutions(offset, record, coordinate, 0, input, progress);
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant resolution <= coordinate.2, input == old(owners)@,
                goal == crate::specs::part1_poc_mark_resolutions(offset, record, coordinate, 0, input, progress),
                goal == crate::specs::part1_poc_mark_resolutions(offset, record, coordinate, resolution, owners@, (linear, contributed)),
            decreases coordinate.2 - resolution,
        ))]
        while resolution < coordinate.2 {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_poc_mark_resolutions, 1); }
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = owners@; }
            if record.contains(coordinate.1, resolution, coordinate.0) {
                let owner = match owners.get_mut(linear) {
                    Some(value) => value,
                    None => {
                        return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow));
                    }
                };
                if !*owner {
                    *owner = true;
                    contributed = true;
                }
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                let fresh = progression_change_contains(record, coordinate.1, resolution, coordinate.0)
                    && !before[linear as int];
                assert(owners@ =~= if fresh { before.update(linear as int, true) } else { before });
            }
            linear = match linear.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow)),
            };
            resolution = match resolution.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow)),
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_poc_mark_resolutions, 1); }
        Ok((linear, contributed))
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, final(owners)@) == crate::specs::part1_poc_mark_layers(
            offset, record, coordinate, layers, 0, old(owners)@, progress),
    ))]
    fn mark_progression_layers(
        &self,
        record: ProgressionChange,
        coordinate: (u16, u8),
        owners: &mut [bool],
        progress: (usize, bool),
        layers: u16,
        offset: usize,
    ) -> Result<(usize, bool), DecodeError> {
        let mut linear = progress.0;
        let mut contributed = progress.1;
        let mut layer = 0_u16;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = owners@;
            let ghost goal = crate::specs::part1_poc_mark_layers(offset, record, coordinate, layers, 0, input, progress);
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant layer <= layers, input == old(owners)@,
                goal == crate::specs::part1_poc_mark_layers(offset, record, coordinate, layers, 0, input, progress),
                goal == crate::specs::part1_poc_mark_layers(offset, record, coordinate, layers, layer, owners@, (linear, contributed)),
            decreases layers - layer,
        ))]
        while layer < layers {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_poc_mark_layers, 1); }
            let next = self.mark_progression_resolutions(
                record,
                (coordinate.0, layer, coordinate.1),
                owners,
                (linear, contributed),
                offset,
            )?;
            linear = next.0;
            contributed = next.1;
            layer = match layer.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow)),
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_poc_mark_layers, 1); }
        Ok((linear, contributed))
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, final(owners)@) == crate::specs::part1_poc_mark_record(
            offset, record, layers, component_styles, old(owners)@),
    ))]
    fn mark_progression_record(
        &self,
        record: ProgressionChange,
        layers: u16,
        component_styles: &ComponentCodingStyles,
        owners: &mut [bool],
        offset: usize,
    ) -> Result<bool, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_poc_mark_record); }
        let components = component_styles.components;
        let mut linear = 0_usize;
        let mut contributed = false;
        let mut component = 0_u16;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = owners@;
            let ghost goal = crate::specs::part1_poc_mark_components(offset, record, layers, component_styles, 0, input, (0, false));
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant component <= components, components == component_coding_style_count(component_styles),
                input == old(owners)@,
                goal == crate::specs::part1_poc_mark_components(offset, record, layers, component_styles, 0, input, (0, false)),
                goal == crate::specs::part1_poc_mark_components(offset, record, layers, component_styles, component, owners@, (linear, contributed)),
            decreases components - component,
        ))]
        while component < components {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_poc_mark_components, 1); }
            let depth = component_styles.style(component)?.decomposition_levels;
            let resolutions = match depth.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow)),
            };
            let next = self.mark_progression_layers(
                record,
                (component, resolutions),
                owners,
                (linear, contributed),
                layers,
                offset,
            )?;
            linear = next.0;
            contributed = next.1;
            component = match component.checked_add(1) {
                Some(value) => value,
                None => return Err(self.malformed_at(offset, MalformedKind::ArithmeticOverflow)),
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_poc_mark_components, 1); }
        Ok(contributed)
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

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures parsed_quantization_outcome(result) == crate::specs::part1_parse_qcd_payload(
            segment.offset, segment.payload@, parsed_coding_style_view(coding_style)),
    ))]
    fn parse_qcd(
        &self,
        segment: Segment<'a>,
        coding_style: CodingStyle,
    ) -> Result<Quantization, DecodeError> {
        self.parse_qcd_payload(segment.offset, segment.payload, coding_style)
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures parsed_quantization_outcome(result) == crate::specs::part1_parse_qcd_payload(
            segment_offset, payload@, parsed_coding_style_view(coding_style)),
    ))]
    fn parse_qcd_payload(
        &self,
        segment_offset: usize,
        payload: &[u8],
        coding_style: CodingStyle,
    ) -> Result<Quantization, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_parse_qcd_payload);
            reveal(crate::specs::part1_qcd_header);
        }
        if payload.is_empty() {
            return Err(self.malformed_at(segment_offset, MalformedKind::InvalidMarkerLength));
        }

        let guard_bits = payload[0] >> 5;
        let style = match payload[0] & 0x1f {
            0 => QuantizationStyle::None,
            1 => QuantizationStyle::ScalarDerived,
            2 => QuantizationStyle::ScalarExpounded,
            _ => {
                return Err(self.malformed_at(segment_offset, MalformedKind::InvalidQuantization));
            }
        };
        let subband_count = coding_style.subband_count();
        let step_bytes = payload.len() - 1;
        let expected_step_bytes = match style {
            QuantizationStyle::None => usize::from(subband_count),
            QuantizationStyle::ScalarDerived => 2,
            QuantizationStyle::ScalarExpounded => match usize::from(subband_count).checked_mul(2) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment_offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            },
        };
        if step_bytes != expected_step_bytes {
            return Err(self.malformed_at(segment_offset, MalformedKind::InvalidMarkerLength));
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost h = crate::specs::Part1QcdHeader { style, guard_bits, subband_count };
            let ghost goal = crate::specs::part1_parse_qcd_payload(
                segment_offset, payload@, parsed_coding_style_view(coding_style));
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::part1_qcd_header(segment_offset, payload@, parsed_coding_style_view(coding_style)) == Ok(h));
        }
        if matches!(style, QuantizationStyle::None) {
            let mut index = 1_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant
                        1 <= index <= payload.len(),
                        style == QuantizationStyle::None,
                        h == (crate::specs::Part1QcdHeader { style, guard_bits, subband_count }),
                        goal == crate::specs::part1_parse_qcd_payload(
                            segment_offset, payload@, parsed_coding_style_view(coding_style)),
                        goal == match crate::specs::part1_qcd_validate_unscaled(segment_offset, payload@, index) {
                            Err(error) => Err(error),
                            Ok(()) => crate::specs::part1_qcd_after_validation(segment_offset, payload@, h),
                        },
                    decreases payload.len() - index,
                )
            )]
            while index < payload.len() {
                #[cfg(verus_keep_ghost)]
                proof! { reveal_with_fuel(crate::specs::part1_qcd_validate_unscaled, 1); }
                if payload[index] & 0x07 != 0 {
                    return Err(
                        self.malformed_at(segment_offset, MalformedKind::InvalidQuantization)
                    );
                }
                index = match index.checked_add(1) {
                    Some(value) => value,
                    None => {
                        return Err(
                            self.malformed_at(segment_offset, MalformedKind::ArithmeticOverflow)
                        );
                    }
                };
            }
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_qcd_validate_unscaled, 1); }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(goal == crate::specs::part1_qcd_after_validation(segment_offset, payload@, h));
            reveal(crate::specs::part1_qcd_after_validation);
        }
        let step_count = match style {
            QuantizationStyle::None | QuantizationStyle::ScalarExpounded => subband_count,
            QuantizationStyle::ScalarDerived => 1,
        };
        if usize::from(step_count) > MAX_QUANTIZATION_STEPS {
            return Err(self.malformed_at(segment_offset, MalformedKind::InvalidQuantization));
        }
        let mut steps = [QuantizationStep {
            exponent: 0,
            mantissa: 0,
        }; MAX_QUANTIZATION_STEPS];
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(steps@ =~= Seq::new(97, |i: int| QuantizationStep { exponent: 0, mantissa: 0 }));
        }
        let step_total = usize::from(step_count);
        let mut step_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    step_index <= step_total <= MAX_QUANTIZATION_STEPS,
                    step_total == step_count as usize,
                    goal == crate::specs::part1_parse_qcd_payload(
                        segment_offset, payload@, parsed_coding_style_view(coding_style)),
                    goal == match crate::specs::part1_qcd_steps(
                        segment_offset, payload@, style, step_total, step_index, steps@) {
                        Err(error) => Err(error),
                        Ok(values) => Ok((style, guard_bits, step_count, values)),
                    },
                decreases step_total - step_index,
            )
        )]
        while step_index < step_total {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(crate::specs::part1_qcd_steps, 1);
                reveal(crate::specs::part1_qcd_step);
            }
            let payload_index =
                if matches!(style, QuantizationStyle::None) {
                    match step_index.checked_add(1) {
                        Some(value) => value,
                        None => {
                            return Err(self
                                .malformed_at(segment_offset, MalformedKind::ArithmeticOverflow));
                        }
                    }
                } else {
                    let doubled = match step_index.checked_mul(2) {
                        Some(value) => value,
                        None => {
                            return Err(self
                                .malformed_at(segment_offset, MalformedKind::ArithmeticOverflow));
                        }
                    };
                    match doubled.checked_add(1) {
                        Some(value) => value,
                        None => {
                            return Err(self
                                .malformed_at(segment_offset, MalformedKind::ArithmeticOverflow));
                        }
                    }
                };
            let step = if matches!(style, QuantizationStyle::None) {
                if payload_index >= payload.len() {
                    return Err(
                        self.malformed_at(segment_offset, MalformedKind::InvalidQuantization)
                    );
                }
                QuantizationStep {
                    exponent: payload[payload_index] >> 3,
                    mantissa: 0,
                }
            } else {
                let next_index = match payload_index.checked_add(1) {
                    Some(value) => value,
                    None => {
                        return Err(
                            self.malformed_at(segment_offset, MalformedKind::ArithmeticOverflow)
                        );
                    }
                };
                if next_index >= payload.len() {
                    return Err(
                        self.malformed_at(segment_offset, MalformedKind::InvalidQuantization)
                    );
                }
                let encoded = read_u16([payload[payload_index], payload[next_index]]);
                #[cfg(verus_keep_ghost)]
                proof! { assert(encoded >> 11 <= 255) by (bit_vector); }
                QuantizationStep {
                    exponent: u8::try_from(encoded >> 11).map_err(|_| {
                        self.malformed_at(segment_offset, MalformedKind::ArithmeticOverflow)
                    })?,
                    mantissa: encoded & 0x07ff,
                }
            };
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(crate::specs::part1_qcd_step(segment_offset, payload@, style, step_index) == Ok(step));
            }
            if step_index >= steps.len() {
                return Err(self.malformed_at(segment_offset, MalformedKind::InvalidQuantization));
            }
            steps[step_index] = step;
            step_index = match step_index.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment_offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_qcd_steps, 1); }
        Ok(Quantization {
            style,
            guard_bits,
            step_count,
            steps,
        })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures parsed_component_quantization_outcome(result) == crate::specs::part1_parse_qcc(
            segment.offset, segment.payload@, components, parsed_coding_style_view(coding_style)),
    ))]
    fn parse_qcc(
        &self,
        segment: Segment<'a>,
        components: u16,
        coding_style: CodingStyle,
    ) -> Result<(u16, Quantization), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_parse_qcc);
            reveal(crate::specs::part1_parse_component_index);
        }
        let (component, component_bytes) =
            self.parse_component_index(segment, components, MalformedKind::InvalidQuantization)?;
        let payload = segment
            .payload
            .get(component_bytes..)
            .ok_or_else(|| self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength))?;
        let quantization = self.parse_qcd_payload(segment.offset, payload, coding_style)?;
        Ok((component, quantization))
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_parse_rgn(segment.offset, segment.payload@, components),
    ))]
    fn parse_rgn(&self, segment: Segment<'a>, components: u16) -> Result<(u16, u8), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_rgn); }
        let component_bytes = if components < 257 { 1_usize } else { 2_usize };
        let expected_length = match component_bytes.checked_add(2) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if segment.payload.len() != expected_length {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let component = if component_bytes == 1 {
            u16::from(segment.payload[0])
        } else {
            read_u16([segment.payload[0], segment.payload[1]])
        };
        let style_index = component_bytes;
        let shift_index = match component_bytes.checked_add(1) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if component >= components
            || style_index >= segment.payload.len()
            || shift_index >= segment.payload.len()
            || segment.payload[style_index] != 0
        {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidRegionOfInterest));
        }
        Ok((component, segment.payload[shift_index]))
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), *final(next_index), packet_length_groups_view(final(groups)@))
            == crate::specs::part1_parse_plm(parser_view(old(self)), segment_view(segment),
                *old(next_index), packet_length_groups_view(old(groups)@)),
    ))]
    fn parse_plm(
        &mut self,
        segment: Segment<'a>,
        next_index: &mut u16,
        groups: &mut Vec<PacketLengths<'a>>,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost initial_index = *next_index;
            let ghost initial_groups = packet_length_groups_view(groups@);
            let ghost goal = crate::specs::part1_parse_plm(input, segment_view(segment), initial_index, initial_groups);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_plm); }
        if segment.payload.len() < 5 {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let index = u16::from(segment.payload[0]);
        if index != *next_index || index > u16::from(u8::MAX) {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerOrder));
        }
        *next_index += 1;

        let mut position = 1_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= segment.payload.len(),
                    input == parser_view(old(self)),
                    initial_index == *old(next_index),
                    initial_groups == packet_length_groups_view(old(groups)@),
                    goal == crate::specs::part1_parse_plm(input, segment_view(segment), initial_index, initial_groups),
                    goal == crate::specs::part1_plm_groups_from(parser_view(self), segment_view(segment),
                        *next_index, position as nat, packet_length_groups_view(groups@)),
                decreases segment.payload.len() - position,
            )
        )]
        while position < segment.payload.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_plm_groups_from, 1); }
            if segment.payload.len() - position < 4 {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
            }
            let encoded_length = match usize::try_from(read_u32([
                segment.payload[position],
                segment.payload[position + 1],
                segment.payload[position + 2],
                segment.payload[position + 3],
            ])) {
                Ok(value) => value,
                Err(_) => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            position += 4;
            let end = match position.checked_add(encoded_length) {
                Some(value) => value,
                None => {
                    return Err(
                        self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow)
                    );
                }
            };
            if end > segment.payload.len() {
                return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
            }
            let encoded = &segment.payload[position..end];
            let (packet_count, _) = self.validate_packet_lengths(encoded, segment.offset)?;
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = packet_length_groups_view(groups@); }
            groups.push(PacketLengths {
                source: PacketLengthSource::Plm(encoded),
                packet_count,
                encoded_bytes: encoded.len() as u64,
            });
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(packet_length_groups_view(groups@) =~= before.push(crate::specs::Part1PacketLengths {
                    source: crate::specs::Part1PacketLengthSource::Plm(encoded@), packet_count,
                    encoded_bytes: encoded.len() as u64,
                }));
            }
            if groups.len() as u64 > u64::from(self.limits.maximum_tile_parts()) {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::TileParts,
                    limit: u64::from(self.limits.maximum_tile_parts()),
                    actual: groups.len() as u64,
                });
            }
            position = end;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_plm_groups_from, 1); }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self)), *final(next_index), *final(fragments), final(bytes)@)
            == crate::specs::part1_parse_packed_fragment(parser_view(old(self)), segment_view(segment), expected_marker,
                *old(next_index), *old(fragments), old(bytes)@),
    ))]
    fn parse_packed_fragment(
        &mut self,
        segment: Segment<'a>,
        expected_marker: u16,
        next_index: &mut u16,
        fragments: &mut u16,
        bytes: &mut Vec<u8>,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_packed_fragment); }
        if segment.marker != expected_marker || segment.payload.is_empty() {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerLength));
        }
        let index = u16::from(segment.payload[0]);
        if index != *next_index || index > u16::from(u8::MAX) {
            return Err(self.malformed_at(segment.offset, MalformedKind::InvalidMarkerOrder));
        }
        *next_index += 1;
        *fragments = match fragments.checked_add(1) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let fragment = &segment.payload[1..];
        let new_length = match bytes.len().checked_add(fragment.len()) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        let total_header_bytes = match self.packet_length_bytes.checked_add(fragment.len() as u64) {
            Some(value) => value,
            None => {
                return Err(self.malformed_at(segment.offset, MalformedKind::ArithmeticOverflow));
            }
        };
        if total_header_bytes > self.limits.maximum_packet_header_bytes() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::PacketHeaderBytes,
                limit: self.limits.maximum_packet_header_bytes(),
                actual: total_header_bytes,
            });
        }
        self.packet_length_bytes = total_header_bytes;
        self.charge_work(fragment.len() as u64)?;
        bytes.reserve(new_length - bytes.len());
        bytes.extend_from_slice(fragment);
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures packed_header_frames_outcome(result) == crate::specs::part1_parse_ppm_frames(
            bytes@, fragments, self.limits),
    ))]
    fn parse_ppm_frames(
        &self,
        bytes: &[u8],
        fragments: u16,
    ) -> Result<Vec<PackedHeaderFrame>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost goal = crate::specs::part1_parse_ppm_frames(bytes@, fragments, self.limits); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_parse_ppm_frames); }
        if fragments == 0 {
            let empty = Vec::new();
            #[cfg(verus_keep_ghost)]
            proof! { assert(packed_header_frames_view(empty@) =~= Seq::empty()); }
            return Ok(empty);
        }
        let mut frames = Vec::new();
        #[cfg(verus_keep_ghost)]
        proof! { assert(packed_header_frames_view(frames@) =~= Seq::empty()); }
        let mut position = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= bytes.len(),
                    goal == crate::specs::part1_parse_ppm_frames(bytes@, fragments, self.limits),
                    goal == crate::specs::part1_ppm_frames_from(bytes@,
                        crate::limits::resource_tile_part_limit(self.limits), position as nat, packed_header_frames_view(frames@)),
                decreases bytes.len() - position,
            )
        )]
        while position < bytes.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_ppm_frames_from, 1); }
            if bytes.len() - position < 4 {
                return Err(self.malformed_at(position, MalformedKind::InvalidPackedPacketHeader));
            }
            let length = match usize::try_from(read_u32([
                bytes[position],
                bytes[position + 1],
                bytes[position + 2],
                bytes[position + 3],
            ])) {
                Ok(value) => value,
                Err(_) => {
                    return Err(self.malformed_at(position, MalformedKind::ArithmeticOverflow));
                }
            };
            position += 4;
            let start = position;
            position = match position.checked_add(length) {
                Some(value) => value,
                None => return Err(self.malformed_at(start, MalformedKind::ArithmeticOverflow)),
            };
            if position > bytes.len() {
                return Err(self.malformed_at(start, MalformedKind::InvalidPackedPacketHeader));
            }
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = packed_header_frames_view(frames@); }
            frames.push(PackedHeaderFrame { start, length });
            #[cfg(verus_keep_ghost)]
            proof! { assert(packed_header_frames_view(frames@) =~= before.push((start, length))); }
            if frames.len() as u64 > u64::from(self.limits.maximum_tile_parts()) {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::TileParts,
                    limit: u64::from(self.limits.maximum_tile_parts()),
                    actual: frames.len() as u64,
                });
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_ppm_frames_from, 1); }
        Ok(frames)
    }

    #[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, parser_view(final(self))) == crate::specs::part1_validate_packet_lengths(
            parser_view(old(self)), encoded@, marker_offset),
    ))]
    fn validate_packet_lengths(
        &mut self,
        encoded: &[u8],
        marker_offset: usize,
    ) -> Result<(u64, u64), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost input = parser_view(self);
            let ghost goal = crate::specs::part1_validate_packet_lengths(input, encoded@, marker_offset);
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_validate_packet_lengths); }
        let encoded_bytes = encoded.len() as u64;
        self.packet_length_bytes = match self.packet_length_bytes.checked_add(encoded_bytes) {
            Some(value) => value,
            None => return Err(self.malformed_at(marker_offset, MalformedKind::ArithmeticOverflow)),
        };
        if self.packet_length_bytes > self.limits.maximum_packet_header_bytes() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::PacketHeaderBytes,
                limit: self.limits.maximum_packet_header_bytes(),
                actual: self.packet_length_bytes,
            });
        }
        self.charge_work(encoded_bytes)?;
        let mut position = 0_usize;
        let mut count = 0_u64;
        let mut total = 0_u64;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant position <= encoded.len(),
                    input == parser_view(old(self)),
                    goal == crate::specs::part1_validate_packet_lengths(input, encoded@, marker_offset),
                    goal == crate::specs::part1_packet_length_admission_from(parser_view(self), encoded@,
                        marker_offset, position, count, total),
                decreases encoded.len() - position,
            )
        )]
        while position < encoded.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part1_packet_length_admission_from, 1); }
            let (length, next) = match decode_packet_length(encoded, position, encoded.len()) {
                Ok(value) => value,
                Err(_) => {
                    return Err(
                        self.malformed_at(marker_offset, MalformedKind::InvalidPacketLength)
                    );
                }
            };
            if next <= position || next > encoded.len() {
                return Err(self.malformed_at(marker_offset, MalformedKind::InvalidPacketLength));
            }
            count = match count.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(self.malformed_at(marker_offset, MalformedKind::ArithmeticOverflow));
                }
            };
            total = match total.checked_add(u64::from(length)) {
                Some(value) => value,
                None => {
                    return Err(self.malformed_at(marker_offset, MalformedKind::ArithmeticOverflow));
                }
            };
            position = next;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_packet_length_admission_from, 1); }
        self.packet_lengths = match self.packet_lengths.checked_add(count) {
            Some(value) => value,
            None => return Err(self.malformed_at(marker_offset, MalformedKind::ArithmeticOverflow)),
        };
        if self.packet_lengths > self.limits.maximum_packets() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Packets,
                limit: self.limits.maximum_packets(),
                actual: self.packet_lengths,
            });
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result as nat == crate::specs::part1_set_bits_prefix(value, 32),
        result <= 32,
))]
fn count_set_bits(value: u32) -> u8 {
    let mut count = 0_u8;
    let mut bit = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant bit <= 32, count <= bit,
                count as nat == crate::specs::part1_set_bits_prefix(value, bit as nat),
            decreases 32 - bit,
        )
    )]
    while bit < 32 {
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_set_bits_prefix, 1); }
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

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_maximum_u32_for_bits(bits),
))]
fn maximum_u32_for_bits(bits: u8) -> Result<u32, DecodeError> {
    if bits == 0 || bits > 31 {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        });
    }
    let mut value = 0_u32;
    let mut bit = 0_u8;
    #[cfg(verus_keep_ghost)]
    proof! { assert((1u64 << 0u8) == 1u64) by (bit_vector); }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant bit <= bits, 0 < bits <= 31,
                value as u64 == (1u64 << bit) - 1,
            decreases bits - bit,
        )
    )]
    while bit < bits {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(bit < 31 ==> (1u64 << bit) - 1 <= 1073741823u64) by (bit_vector);
            assert(bit < 31 ==> ((1u64 << bit) - 1) * 2 + 1 == (1u64 << (bit + 1)) - 1) by (bit_vector);
        }
        let doubled = match value.checked_mul(2) {
            Some(value) => value,
            None => {
                return Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::ArithmeticOverflow,
                });
            }
        };
        value = match doubled.checked_add(1) {
            Some(value) => value,
            None => {
                return Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::ArithmeticOverflow,
                });
            }
        };
        bit = match bit.checked_add(1) {
            Some(value) => value,
            None => {
                return Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::ArithmeticOverflow,
                });
            }
        };
    }
    Ok(value)
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_mct_registry_array_find(mct_arrays_view(arrays@), index, 0),
        match result { Some(position) => position < arrays.len(), None => true },
))]
fn find_mct_array_index(arrays: &[MctArray], index: u8) -> Option<usize> {
    let mut position = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant position <= arrays.len(),
            crate::specs::part1_mct_registry_array_find(mct_arrays_view(arrays@), index, 0)
                == crate::specs::part1_mct_registry_array_find(mct_arrays_view(arrays@), index, position as nat),
        decreases arrays.len() - position,
    ))]
    while position < arrays.len() {
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_mct_registry_array_find, 1); reveal(mct_array_view); }
        if arrays[position].index == index {
            return Some(position);
        }
        position += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part1_mct_registry_array_find, 1); }
    None
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_mct_registry_record_find(mct_records_view(records@), index, 0),
        match result { Some(position) => position < records.len(), None => true },
))]
fn find_mct_record_index(records: &[MctRecord], index: u8) -> Option<usize> {
    let mut position = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant position <= records.len(),
            crate::specs::part1_mct_registry_record_find(mct_records_view(records@), index, 0)
                == crate::specs::part1_mct_registry_record_find(mct_records_view(records@), index, position as nat),
        decreases records.len() - position,
    ))]
    while position < records.len() {
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_mct_registry_record_find, 1); reveal(mct_record_view); }
        if records[position].index == index {
            return Some(position);
        }
        position += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part1_mct_registry_record_find, 1); }
    None
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_mct_registry_unique_array(mct_arrays_view(arrays@), marker_offset),
))]
fn validate_unique_mct_array_indexes(
    arrays: &[MctArray],
    marker_offset: usize,
) -> Result<(), DecodeError> {
    if arrays.is_empty() {
        return Ok(());
    }
    let last = arrays.len() - 1;
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(mct_array_view);
        assert(mct_arrays_view(arrays@.subrange(0, last as int)) =~= mct_arrays_view(arrays@).drop_last());
    }
    if find_mct_array_index(&arrays[..last], arrays[last].index).is_some() {
        return Err(DecodeError::MalformedInput {
            offset: marker_offset as u64,
            kind: MalformedKind::InvalidTransform,
        });
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_mct_registry_unique_record(mct_records_view(records@), marker_offset),
))]
fn validate_unique_mct_record_indexes(
    records: &[MctRecord],
    marker_offset: usize,
) -> Result<(), DecodeError> {
    if records.is_empty() {
        return Ok(());
    }
    let last = records.len() - 1;
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(mct_record_view);
        assert(mct_records_view(records@.subrange(0, last as int)) =~= mct_records_view(records@).drop_last());
    }
    if find_mct_record_index(&records[..last], records[last].index).is_some() {
        return Err(DecodeError::MalformedInput {
            offset: marker_offset as u64,
            kind: MalformedKind::InvalidTransform,
        });
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(_result =>
    ensures ((), mct_arrays_view(final(arrays)@)) == crate::specs::part1_mct_registry_replace_array(
        mct_arrays_view(old(arrays)@), mct_array_view(&replacement)),
))]
fn replace_mct_array(arrays: &mut Vec<MctArray>, replacement: MctArray) {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost input = mct_arrays_view(arrays@); let ghost value = mct_array_view(&replacement); }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(mct_array_view); }
    match find_mct_array_index(arrays, replacement.index) {
        Some(index) => {
            arrays[index] = replacement;
            #[cfg(verus_keep_ghost)]
            proof! { assert(mct_arrays_view(arrays@) =~= input.update(index as int, value)); }
        }
        None => {
            arrays.push(replacement);
            #[cfg(verus_keep_ghost)]
            proof! { assert(mct_arrays_view(arrays@) =~= input.push(value)); }
        }
    }
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(_result =>
    ensures ((), mct_records_view(final(records)@)) == crate::specs::part1_mct_registry_replace_record(
        mct_records_view(old(records)@), mct_record_view(&replacement)),
))]
fn replace_mct_record(records: &mut Vec<MctRecord>, replacement: MctRecord) {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost input = mct_records_view(records@); let ghost value = mct_record_view(&replacement); }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(mct_record_view); }
    match find_mct_record_index(records, replacement.index) {
        Some(index) => {
            records[index] = replacement;
            #[cfg(verus_keep_ghost)]
            proof! { assert(mct_records_view(records@) =~= input.update(index as int, value)); }
        }
        None => {
            records.push(replacement);
            #[cfg(verus_keep_ghost)]
            proof! { assert(mct_records_view(records@) =~= input.push(value)); }
        }
    }
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures mct_array_reference_outcome(result) == crate::specs::part1_mct_registry_reference(
        mct_arrays_view(arrays@), index, kind),
))]
fn referenced_mct_array(
    arrays: &[MctArray],
    index: u8,
    kind: MctArrayKind,
) -> Result<&MctArray, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost goal = crate::specs::part1_mct_registry_reference(mct_arrays_view(arrays@), index, kind); }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_mct_registry_reference); }
    let mut position = 0_usize;
    let mut found: Option<usize> = None;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant position <= arrays.len(),
            match found { Some(candidate) => candidate < position, None => true },
            goal == crate::specs::part1_mct_registry_reference(mct_arrays_view(arrays@), index, kind),
            goal == crate::specs::part1_mct_registry_reference_from(
                mct_arrays_view(arrays@), index, kind, position as nat, found),
        decreases arrays.len() - position,
    ))]
    while position < arrays.len() {
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part1_mct_registry_reference_from, 1); reveal(mct_array_view); }
        let array = &arrays[position];
        if array.index == index && mct_array_kinds_equal(array.kind, kind) {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(mct_arrays_view(arrays@)[position as int] == mct_array_view(array));
                assert(mct_arrays_view(arrays@)[position as int].0 == index);
                assert(mct_arrays_view(arrays@)[position as int].1 == kind);
            }
            if found.is_some() {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(goal == Err(crate::specs::part1_codestream_malformed(
                        0, MalformedKind::InvalidTransform)));
                }
                return Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::InvalidTransform,
                });
            }
            found = Some(position);
        }
        position += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part1_mct_registry_reference_from, 1); }
    match found {
        Some(candidate) => Ok(&arrays[candidate]),
        None => Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidTransform,
        }),
    }
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_mcc_parallel_collections(mcc_collection_references_view(collections@), marker_offset),
))]
fn validate_parallel_collections(
    collections: &[MctCollectionReference],
    marker_offset: usize,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(mcc_collection_reference_view); }
    let mut collection_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant collection_index <= collections.len(),
                forall|index: int, previous: int| 0 <= previous < index < collection_index ==>
                    !crate::specs::part1_mcc_parallel_conflict(
                        mcc_collection_references_view(collections@)[index],
                        mcc_collection_references_view(collections@)[previous]),
            decreases collections.len() - collection_index,
        )
    )]
    while collection_index < collections.len() {
        let collection = &collections[collection_index];
        let mut previous_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant previous_index <= collection_index, collection_index < collections.len(),
                    mcc_collection_reference_view(collection) == mcc_collection_references_view(collections@)[collection_index as int],
                    forall|previous: int| 0 <= previous < previous_index ==>
                        !crate::specs::part1_mcc_parallel_conflict(
                            mcc_collection_references_view(collections@)[collection_index as int],
                            mcc_collection_references_view(collections@)[previous]),
                    forall|index: int, previous: int| 0 <= previous < index < collection_index ==>
                        !crate::specs::part1_mcc_parallel_conflict(
                            mcc_collection_references_view(collections@)[index],
                            mcc_collection_references_view(collections@)[previous]),
                decreases collection_index - previous_index,
            )
        )]
        while previous_index < collection_index {
            let previous = &collections[previous_index];
            #[cfg(verus_keep_ghost)]
            proof! { reveal(mcc_collection_reference_view); }
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
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(crate::specs::part1_mcc_parallel_conflict(
                        mcc_collection_references_view(collections@)[collection_index as int],
                        mcc_collection_references_view(collections@)[previous_index as int]));
                    assert(!(forall|index: int, earlier: int| 0 <= earlier < index < collections.len() ==>
                        !crate::specs::part1_mcc_parallel_conflict(
                            mcc_collection_references_view(collections@)[index],
                            mcc_collection_references_view(collections@)[earlier])));
                }
                return Err(DecodeError::MalformedInput {
                    offset: marker_offset as u64,
                    kind: MalformedKind::InvalidTransform,
                });
            }
            previous_index = match previous_index.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(DecodeError::MalformedInput {
                        offset: marker_offset as u64,
                        kind: MalformedKind::ArithmeticOverflow,
                    });
                }
            };
        }
        collection_index = match collection_index.checked_add(1) {
            Some(value) => value,
            None => {
                return Err(DecodeError::MalformedInput {
                    offset: marker_offset as u64,
                    kind: MalformedKind::ArithmeticOverflow,
                });
            }
        };
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_mcc_lists_overlap(left@, right@),
))]
fn component_lists_overlap(left: &[u16], right: &[u16]) -> bool {
    let mut left_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant left_index <= left.len(),
            forall|i: int, j: int| 0 <= i < left_index && 0 <= j < right.len() ==> left@[i] != right@[j],
        decreases left.len() - left_index,
    ))]
    while left_index < left.len() {
        let mut right_index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant left_index < left.len(), right_index <= right.len(),
                forall|j: int| 0 <= j < right_index ==> left@[left_index as int] != right@[j],
                forall|i: int, j: int| 0 <= i < left_index && 0 <= j < right.len() ==> left@[i] != right@[j],
            decreases right.len() - right_index,
        ))]
        while right_index < right.len() {
            if left[left_index] == right[right_index] {
                return true;
            }
            right_index += 1;
        }
        left_index += 1;
    }
    false
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (marker_words_outcome(result), *final(position)) == crate::specs::part1_parse_mcc_component_list(
        payload@, *old(position), components, marker_offset),
))]
fn parse_mcc_component_list(
    payload: &[u8],
    position: &mut usize,
    components: u16,
    marker_offset: usize,
) -> Result<Vec<u16>, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost start = *position;
        let ghost goal = crate::specs::part1_parse_mcc_component_list(payload@, start, components, marker_offset);
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_parse_mcc_component_list); }
    if *position > payload.len() || payload.len() - *position < 2 {
        return Err(DecodeError::MalformedInput {
            offset: marker_offset as u64,
            kind: MalformedKind::InvalidTransform,
        });
    }
    let encoded_count = read_u16([payload[*position], payload[*position + 1]]);
    *position = match position.checked_add(2) {
        Some(value) => value,
        None => {
            return Err(DecodeError::MalformedInput {
                offset: marker_offset as u64,
                kind: MalformedKind::ArithmeticOverflow,
            });
        }
    };
    let reference_bytes = if encoded_count & 0x8000 == 0 {
        1_usize
    } else {
        2_usize
    };
    let count = encoded_count & 0x7fff;
    #[cfg(verus_keep_ghost)]
    proof! { assert(encoded_count & 0x7fff <= 32767u16) by (bit_vector); }
    if count == 0 || count > components {
        return Err(DecodeError::MalformedInput {
            offset: marker_offset as u64,
            kind: MalformedKind::InvalidTransform,
        });
    }
    let required = match usize::from(count).checked_mul(reference_bytes) {
        Some(value) => value,
        None => {
            return Err(DecodeError::MalformedInput {
                offset: marker_offset as u64,
                kind: MalformedKind::ArithmeticOverflow,
            });
        }
    };
    if *position > payload.len() || payload.len() - *position < required {
        return Err(DecodeError::MalformedInput {
            offset: marker_offset as u64,
            kind: MalformedKind::InvalidTransform,
        });
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost end = *position as int + required as int;
        let ghost target = crate::specs::part1_mcc_reference_run(
            payload@, components, marker_offset, *position, reference_bytes, count as nat, Seq::empty());
    }
    let mut references = Vec::with_capacity(usize::from(count));
    let mut reference_index = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant reference_index <= count <= 32767, references.len() == reference_index,
                reference_bytes == 1 || reference_bytes == 2,
                start == *old(position),
                *position as int + (count - reference_index) * reference_bytes as int == end,
                end <= payload.len(),
                goal == crate::specs::part1_parse_mcc_component_list(payload@, start, components, marker_offset),
                goal == target,
                target == crate::specs::part1_mcc_reference_run(
                    payload@, components, marker_offset, *position, reference_bytes, (count - reference_index) as nat, references@),
            decreases count - reference_index,
        )
    )]
    while reference_index < count {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal_with_fuel(crate::specs::part1_mcc_reference_run, 1);
            assert(*position + reference_bytes <= payload.len());
        }
        let current = match payload.get(*position) {
            Some(value) => *value,
            None => {
                return Err(DecodeError::MalformedInput {
                    offset: marker_offset as u64,
                    kind: MalformedKind::InvalidTransform,
                });
            }
        };
        let reference = if reference_bytes == 1 {
            u16::from(current)
        } else {
            let next = match position.checked_add(1) {
                Some(value) => value,
                None => {
                    return Err(DecodeError::MalformedInput {
                        offset: marker_offset as u64,
                        kind: MalformedKind::ArithmeticOverflow,
                    });
                }
            };
            read_u16([
                current,
                match payload.get(next) {
                    Some(value) => *value,
                    None => {
                        return Err(DecodeError::MalformedInput {
                            offset: marker_offset as u64,
                            kind: MalformedKind::InvalidTransform,
                        });
                    }
                },
            ])
        };
        if reference >= components {
            return Err(DecodeError::MalformedInput {
                offset: marker_offset as u64,
                kind: MalformedKind::InvalidTransform,
            });
        }
        if u16_list_contains(&references, reference) {
            return Err(DecodeError::MalformedInput {
                offset: marker_offset as u64,
                kind: MalformedKind::InvalidTransform,
            });
        }
        references.push(reference);
        *position = match position.checked_add(reference_bytes) {
            Some(value) => value,
            None => {
                return Err(DecodeError::MalformedInput {
                    offset: marker_offset as u64,
                    kind: MalformedKind::ArithmeticOverflow,
                });
            }
        };
        reference_index = match reference_index.checked_add(1) {
            Some(value) => value,
            None => {
                return Err(DecodeError::MalformedInput {
                    offset: marker_offset as u64,
                    kind: MalformedKind::ArithmeticOverflow,
                });
            }
        };
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part1_mcc_reference_run, 1); }
    Ok(references)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_mcc_reference_present(values@, value),
))]
fn u16_list_contains(values: &[u16], value: u16) -> bool {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= values.len(),
            forall|position: int| 0 <= position < index ==> values@[position] != value,
        decreases values.len() - index,
    ))]
    while index < values.len() {
        if values[index] == value {
            return true;
        }
        index += 1;
    }
    false
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_float32_to_fixed(bits),
))]
fn float32_to_fixed(bits: u32) -> Result<i64, ()> {
    let negative = bits >> 31 != 0;
    let exponent = (bits >> 23) & 0xff;
    let fraction = bits & 0x7f_ffff;
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part2_atk_binary_format);
        assert(((1u128 << 8) - 1) as u128 == 255u128) by (bit_vector);
        assert(((1u128 << 23) - 1) as u128 == 0x7fffffu128) by (bit_vector);
        assert(((bits as u128) >> 31 != 0) == (bits >> 31 != 0)) by (bit_vector);
        assert((((bits as u128) >> 23) & (((1u128 << 8) - 1) as u128)) == (((bits >> 23) & 255) as u128)) by (bit_vector);
        assert(((bits as u128) & (((1u128 << 23) - 1) as u128)) == ((bits & 0x7fffff) as u128)) by (bit_vector);
        assert(((1u32 << 23) | fraction) as u128 == (1u128 << 23) | fraction as u128) by (bit_vector);
        assert((bits >> 23) & 255 <= 255u32) by (bit_vector);
    }
    if exponent == 0xff {
        return Err(());
    }
    if exponent == 0 && fraction == 0 {
        return Ok(0);
    }
    let (significand, binary_exponent) = if exponent == 0 {
        (u128::from(fraction), -149_i32)
    } else {
        (u128::from((1_u32 << 23) | fraction), exponent as i32 - 150)
    };
    fixed_from_binary(negative, significand, binary_exponent)
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_float64_to_fixed(bits),
))]
fn float64_to_fixed(bits: u64) -> Result<i64, ()> {
    let negative = bits >> 63 != 0;
    let exponent = (bits >> 52) & 0x7ff;
    let fraction = bits & 0x000f_ffff_ffff_ffff;
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part2_atk_binary_format);
        assert(((1u128 << 11) - 1) as u128 == 2047u128) by (bit_vector);
        assert(((1u128 << 52) - 1) as u128 == 0x000fffffffffffffu128) by (bit_vector);
        assert(((bits as u128) >> 63 != 0) == (bits >> 63 != 0)) by (bit_vector);
        assert((((bits as u128) >> 52) & (((1u128 << 11) - 1) as u128)) == (((bits >> 52) & 2047) as u128)) by (bit_vector);
        assert(((bits as u128) & (((1u128 << 52) - 1) as u128)) == ((bits & 0x000fffffffffffff) as u128)) by (bit_vector);
        assert(((1u64 << 52) | fraction) as u128 == (1u128 << 52) | fraction as u128) by (bit_vector);
        assert((bits >> 52) & 2047 <= 2047u64) by (bit_vector);
    }
    if exponent == 0x7ff {
        return Err(());
    }
    if exponent == 0 && fraction == 0 {
        return Ok(0);
    }
    let (significand, binary_exponent) = if exponent == 0 {
        (u128::from(fraction), -1074_i32)
    } else {
        (u128::from((1_u64 << 52) | fraction), exponent as i32 - 1075)
    };
    fixed_from_binary(negative, significand, binary_exponent)
}

#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_fixed_from_binary(negative, significand, binary_exponent),
))]
fn fixed_from_binary(negative: bool, significand: u128, binary_exponent: i32) -> Result<i64, ()> {
    #[cfg(verus_keep_ghost)]
    proof! { mct_fixed_precision_is_q32(); }
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
            #[cfg(verus_keep_ghost)]
            proof! { assert(right < 128 ==> (1u128 << right) > 0u128) by (bit_vector); }
            let remainder_mask = if right == 0 {
                0
            } else {
                (1_u128 << right).checked_sub(1).ok_or(())?
            };
            let remainder = significand & remainder_mask;
            let half = if right == 0 { 0 } else { 1_u128 << (right - 1) };
            let round_up = remainder > half || (remainder == half && truncated & 1 != 0);
            truncated
                .checked_add(if round_up { 1u128 } else { 0u128 })
                .ok_or(())?
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_progression_owners_complete(owners@),
))]
fn progression_owners_complete(owners: &[bool]) -> bool {
    let mut position = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant position <= owners.len(),
            forall|index: int| 0 <= index < position ==> owners@[index],
        decreases owners.len() - position,
    ))]
    while position < owners.len() {
        if !owners[position] {
            return false;
        }
        position += 1;
    }
    true
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_codestream_u64(bytes@),
))]
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_signed_u16(value),
))]
fn signed_u16(value: u16) -> i64 {
    let value = i64::from(value);
    if value < 32_768 {
        value
    } else {
        value - 65_536
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_signed_u32(value),
        i32::MIN <= result <= i32::MAX,
))]
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
mod tests;
