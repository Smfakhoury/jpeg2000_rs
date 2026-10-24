use crate::{
    DecodeError, DecodeOptions, DecodeSelection, MalformedKind, Resource, ResourceLimits,
    SelectionError, TileSelection,
    codestream::{
        self, ArbitraryKernel, CodestreamLayout, CodingStyle, ComponentCodingStyles,
        ComponentQuantizations, ImageSize, Quantization, QuantizationStyle, RoiShifts,
        TilePartPayload, WaveletTransform,
    },
    image::{
        DecodedImage, NonlinearRegion, TileComponent, TransformComponentMut, TypedComponent,
        TypedImage, TypedSamples, apply_irreversible_component_transform,
        apply_multiple_component_transform_components, apply_reversible_component_transform,
        assemble_image, assemble_selected_image,
    },
    jp2,
    packet::{
        CodeBlockContribution, PacketGrid, PacketPrecinctDecoder, PacketSubbandExecution,
        decode_code_block_contribution_body,
    },
    tier1::{CodeBlockPlane, SubbandOrientation, Tier1CodeBlockDecoder, Tier1CodingStyle},
    transform::{
        QuantizationStep, SignedMagnitude, WaveletBand, WaveletLevel, inverse_horizontal_5_3,
        inverse_horizontal_9_7, inverse_horizontal_arbitrary, inverse_level_5_3, inverse_level_9_7,
        inverse_level_arbitrary, inverse_quantize_magnitudes, inverse_vertical_5_3,
        inverse_vertical_9_7, inverse_vertical_arbitrary,
    },
};

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodeBlockLayer<'a> {
    pub contribution: &'a CodeBlockContribution,
    pub body: &'a [u8],
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodeBlockDecodeRequest<'a> {
    pub width: u16,
    pub height: u16,
    pub significant_bitplanes: u16,
    pub orientation: SubbandOrientation,
    pub style: Tier1CodingStyle,
    pub layers: &'a [CodeBlockLayer<'a>],
    pub quantization_step: QuantizationStep,
    pub dynamic_range: u8,
    pub reversible: bool,
    pub roi_shift: u8,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct DecodedCodeBlock {
    width: u16,
    height: u16,
    fraction_bits: u8,
    samples: Vec<i64>,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct PlacedCodeBlock<'a> {
    pub x: u32,
    pub y: u32,
    pub request: CodeBlockDecodeRequest<'a>,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct DecodedSubband {
    width: u32,
    height: u32,
    fraction_bits: u8,
    samples: Vec<i64>,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum ComponentTransform {
    None,
    Reversible,
    Irreversible,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ComponentResolutionGeometry {
    pub component: u16,
    pub resolution: u8,
    pub width: u32,
    pub height: u32,
    pub subbands: u8,
    pub code_blocks_across: u32,
    pub code_blocks_down: u32,
    pub code_blocks: u64,
    pub precincts_across: u32,
    pub precincts_down: u32,
    pub precincts: u64,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub open spec fn planning_components_view(values: Seq<PlannedTileComponent>) -> Seq<crate::specs::Part1PreparePlan> {
        values.map_values(|value: PlannedTileComponent| prepare_plan_view(&value))
    }

    closed spec fn tile_plan_buffers_view(
        precincts: Seq<PacketPrecinctDecoder>, components: Seq<PlannedTileComponent>, total: u64,
    ) -> crate::specs::Part2TilePlanBuffers {
        crate::specs::Part2TilePlanBuffers {
            precincts: planning_precincts_view(precincts), components: planning_components_view(components),
            planned_code_blocks: total,
        }
    }

    pub closed spec fn progression_plan_view(value: &ProgressionTilePlan) -> crate::specs::Part2ProgressionTilePlan {
        crate::specs::Part2ProgressionTilePlan {
            tile: value.tile_index, coding_style: value.coding_style, coding_styles: value.component_coding_styles,
            grid: crate::packet::packet_grid_view(&value.grid), precincts: planning_precincts_view(value.precincts@),
            components: planning_components_view(value.components@),
        }
    }

    pub open spec fn progression_plan_outcome(result: Result<ProgressionTilePlan, DecodeError>)
        -> Result<crate::specs::Part2ProgressionTilePlan, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(value) => Ok(progression_plan_view(&value)) }
    }

    pub closed spec fn planned_subband_lattice_view(value: PlannedSubbandLattice) -> crate::specs::Part2PlannedSubbandLattice {
        crate::specs::Part2PlannedSubbandLattice { grid: value.grid, clip: value.clip, blocks: value.blocks }
    }

    pub open spec fn planned_subband_lattice_outcome(result: Result<Option<PlannedSubbandLattice>, DecodeError>)
        -> Result<Option<crate::specs::Part2PlannedSubbandLattice>, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(None) => Ok(None), Ok(Some(value)) => Ok(Some(planned_subband_lattice_view(value))) }
    }

    pub closed spec fn resolution_geometry_style(style: CodingStyle) -> crate::specs::Part2ResolutionStyle {
        let selector = codestream::decomposition_selector_view(style);
        (selector.0, selector.1, style.code_block_width, style.code_block_height,
            codestream::precinct_selector_view(style).1)
    }

    pub open spec fn resolution_counts_outcome(
        result: Result<(u8, Vec<u8>, Vec<u8>), DecodeError>,
    ) -> Result<crate::specs::Part2ComponentResolutionCounts, DecodeError> {
        match result {
            Err(error) => Err(error),
            Ok(counts) => Ok((counts.0, counts.1@, counts.2@)),
        }
    }

    pub open spec fn subbands_outcome(result: Result<Vec<SubbandGeometry>, DecodeError>)
        -> Result<Seq<SubbandGeometry>, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(values) => Ok(values@) }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct SubbandGeometry {
    pub resolution: u8,
    pub orientation: SubbandOrientation,
    pub x0: u32,
    pub y0: u32,
    pub width: u32,
    pub height: u32,
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub quantization_index: u16,
    pub quantization_step: QuantizationStep,
    pub dynamic_range: u8,
    pub significant_bitplanes: u16,
    pub roi_shift: u8,
    pub low_pass_first_x: bool,
    pub low_pass_first_y: bool,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ProgressionCodeBlockPlacement {
    pub precinct_index: usize,
    pub decoder_index: usize,
    pub subband_index: usize,
    pub x: u32,
    pub y: u32,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct PlannedTileComponent {
    pub component: u16,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    output_resolution: u8,
    subbands: Vec<SubbandGeometry>,
    placements: Vec<ProgressionCodeBlockPlacement>,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub(crate) struct PlannedComponentMetadata {
    pub(crate) component_coding_style: CodingStyle,
    pub(crate) output_resolution: u8,
    pub(crate) tier1_style: Tier1CodingStyle,
    pub(crate) bounds: crate::codestream::TileComponentBounds,
    pub(crate) component_width: u32,
    pub(crate) component_height: u32,
    pub(crate) component_x: u32,
    pub(crate) component_y: u32,
    pub(crate) subbands: Vec<SubbandGeometry>,
}

pub(crate) struct PlannedCodeBlocks {
    pub(crate) decoders: Vec<Tier1CodeBlockDecoder>,
    pub(crate) placements: Vec<ProgressionCodeBlockPlacement>,
    pub(crate) decoder_index: usize,
    pub(crate) planned_code_blocks: u64,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub(crate) struct PlannedSubbandLattice {
    pub(crate) grid: [u32; 4],
    pub(crate) clip: [u32; 4],
    pub(crate) blocks: [u32; 2],
}

pub(crate) struct PlannedSubbandExecution {
    pub(crate) execution: Option<PacketSubbandExecution>,
    pub(crate) placements: Vec<ProgressionCodeBlockPlacement>,
    pub(crate) decoder_index: usize,
    pub(crate) planned_code_blocks: u64,
}

pub(crate) struct PlannedPrecinct {
    pub(crate) precinct: PacketPrecinctDecoder,
    pub(crate) placements: Vec<ProgressionCodeBlockPlacement>,
    pub(crate) planned_code_blocks: u64,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub(crate) struct ResolutionPlanMetadata {
    pub(crate) geometry: ComponentResolutionGeometry,
    pub(crate) first_subband: usize,
    pub(crate) subband_count: usize,
    pub(crate) precincts_in_resolution: u32,
    pub(crate) first_precinct: [u32; 2],
    pub(crate) band_size: [u32; 2],
    pub(crate) block_size: [u32; 2],
}

pub(crate) struct PlannedComponent {
    pub(crate) component: PlannedTileComponent,
    pub(crate) precincts: Vec<PacketPrecinctDecoder>,
    pub(crate) planned_code_blocks: u64,
}

struct ComponentPlanBuilder {
    precincts: Vec<PacketPrecinctDecoder>,
    placements: Vec<ProgressionCodeBlockPlacement>,
    planned_code_blocks: u64,
}

struct PrecinctPlanBuilder {
    executions: Vec<PacketSubbandExecution>,
    placements: Vec<ProgressionCodeBlockPlacement>,
    decoder_index: usize,
    planned_code_blocks: u64,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub open spec fn planning_precincts_view(values: Seq<PacketPrecinctDecoder>) -> Seq<crate::specs::Part1PacketPrecinctState> {
        values.map_values(|value: PacketPrecinctDecoder| crate::packet::packet_precinct_view(&value))
    }

    closed spec fn component_plan_builder_view(value: &ComponentPlanBuilder) -> crate::specs::Part2ComponentPlanBuffers {
        crate::specs::Part2ComponentPlanBuffers {
            precincts: planning_precincts_view(value.precincts@), placements: value.placements@,
            planned_code_blocks: value.planned_code_blocks,
        }
    }

    pub closed spec fn planned_component_view(value: &PlannedComponent) -> crate::specs::Part2PlannedComponent {
        crate::specs::Part2PlannedComponent {
            component: prepare_plan_view(&value.component), precincts: planning_precincts_view(value.precincts@),
            planned_code_blocks: value.planned_code_blocks,
        }
    }

    pub open spec fn planned_component_outcome(result: Result<PlannedComponent, DecodeError>)
        -> Result<crate::specs::Part2PlannedComponent, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(value) => Ok(planned_component_view(&value)) }
    }

    pub closed spec fn resolution_plan_metadata_view(value: &ResolutionPlanMetadata) -> crate::specs::Part2ResolutionPlanMetadata {
        crate::specs::Part2ResolutionPlanMetadata {
            geometry: value.geometry, first_subband: value.first_subband, subband_count: value.subband_count,
            precincts_in_resolution: value.precincts_in_resolution, first_precinct: value.first_precinct,
            band_size: value.band_size, block_size: value.block_size,
        }
    }

    pub open spec fn resolution_plan_metadata_outcome(result: Result<ResolutionPlanMetadata, DecodeError>)
        -> Result<crate::specs::Part2ResolutionPlanMetadata, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(value) => Ok(resolution_plan_metadata_view(&value)) }
    }

    closed spec fn precinct_plan_builder_view(value: &PrecinctPlanBuilder) -> crate::specs::Part2PrecinctPlanBuilder {
        crate::specs::Part2PrecinctPlanBuilder {
            executions: crate::packet::packet_executions_view(value.executions@),
            placements: value.placements@, decoder_index: value.decoder_index,
            planned_code_blocks: value.planned_code_blocks,
        }
    }

    pub closed spec fn planned_precinct_view(value: &PlannedPrecinct) -> crate::specs::Part2PlannedPrecinct {
        crate::specs::Part2PlannedPrecinct {
            precinct: crate::packet::packet_precinct_view(&value.precinct),
            placements: value.placements@, planned_code_blocks: value.planned_code_blocks,
        }
    }

    pub open spec fn planned_precinct_outcome(result: Result<PlannedPrecinct, DecodeError>)
        -> Result<crate::specs::Part2PlannedPrecinct, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(value) => Ok(planned_precinct_view(&value)) }
    }

    pub closed spec fn planned_subband_execution_view(value: &PlannedSubbandExecution) -> crate::specs::Part2PlannedSubbandExecution {
        crate::specs::Part2PlannedSubbandExecution {
            execution: match value.execution { None => None, Some(execution) => Some(crate::packet::packet_execution_view(&execution)) },
            placements: value.placements@, decoder_index: value.decoder_index,
            planned_code_blocks: value.planned_code_blocks,
        }
    }

    pub open spec fn planned_subband_execution_outcome(result: Result<PlannedSubbandExecution, DecodeError>)
        -> Result<crate::specs::Part2PlannedSubbandExecution, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(value) => Ok(planned_subband_execution_view(&value)) }
    }

    pub closed spec fn planned_codeblocks_view(value: &PlannedCodeBlocks) -> crate::specs::Part2PlannedCodeBlocks {
        crate::specs::Part2PlannedCodeBlocks {
            decoders: value.decoders@.map_values(|decoder: Tier1CodeBlockDecoder| crate::tier1::tier1_decoder_state(&decoder)),
            placements: value.placements@, decoder_index: value.decoder_index,
            planned_code_blocks: value.planned_code_blocks,
        }
    }

    pub open spec fn planned_codeblocks_outcome(result: Result<PlannedCodeBlocks, DecodeError>)
        -> Result<crate::specs::Part2PlannedCodeBlocks, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(value) => Ok(planned_codeblocks_view(&value)) }
    }

    pub closed spec fn component_plan_metadata_view(value: &PlannedComponentMetadata) -> crate::specs::Part2ComponentPlanMetadata {
        crate::specs::Part2ComponentPlanMetadata {
            coding_style: value.component_coding_style, output_resolution: value.output_resolution,
            tier1_style: value.tier1_style, bounds: value.bounds, width: value.component_width,
            height: value.component_height, x: value.component_x, y: value.component_y, subbands: value.subbands@,
        }
    }

    pub open spec fn component_plan_metadata_outcome(result: Result<PlannedComponentMetadata, DecodeError>)
        -> Result<crate::specs::Part2ComponentPlanMetadata, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(value) => Ok(component_plan_metadata_view(&value)) }
    }
}

pub struct ProgressionTilePlan {
    tile_index: u16,
    coding_style: CodingStyle,
    component_coding_styles: ComponentCodingStyles,
    grid: PacketGrid,
    precincts: Vec<PacketPrecinctDecoder>,
    components: Vec<PlannedTileComponent>,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct PreparedTileComponent {
    component: u16,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    fraction_bits: u8,
    samples: Vec<i64>,
}

pub(crate) struct DecodedTileComponents {
    pub(crate) prepared: Vec<PreparedTileComponent>,
    pub(crate) mct_work: u64,
}

pub(crate) struct DecodedLayoutTile {
    pub(crate) prepared: Vec<PreparedTileComponent>,
    pub(crate) counters: (u64, u64, u64),
    pub(crate) mct_work: u64,
}

pub(crate) struct TileDecodeMetadata<'a, 'data> {
    pub(crate) first: &'a TilePartPayload<'data>,
    pub(crate) standard: (bool, bool),
    pub(crate) resolution_counts: Vec<u8>,
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_resolution_geometry(
        codestream::selection_geometry_view(size), codestream::image_size_component_count(size),
        codestream::image_size_component_infos(size), resolution_geometry_style(coding_style),
        component, resolution, None,
    ),
))]
pub fn derive_component_resolution_geometry(
    size: &ImageSize,
    coding_style: CodingStyle,
    component: u16,
    resolution: u8,
) -> Result<ComponentResolutionGeometry, DecodeError> {
    derive_component_resolution_geometry_for(size, coding_style, component, resolution, None)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_resolution_geometry(
        codestream::selection_geometry_view(size), codestream::image_size_component_count(size),
        codestream::image_size_component_infos(size), resolution_geometry_style(coding_style),
        component, resolution, tile_index,
    ),
))]
pub(crate) fn derive_component_resolution_geometry_for(
    size: &ImageSize,
    coding_style: CodingStyle,
    component: u16,
    resolution: u8,
    tile_index: Option<u16>,
) -> Result<ComponentResolutionGeometry, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        codestream::resolution_selectors_known_fields(coding_style);
    }
    if resolution > coding_style.decomposition_levels {
        return Err(invalid_decode());
    }

    let component_bounds = component_coordinate_bounds(size, component, tile_index)?;
    let component_x0 = component_bounds[0];
    let component_y0 = component_bounds[1];
    let component_x1 = component_bounds[2];
    let component_y1 = component_bounds[3];
    let (scale_x, scale_y) = coding_style.resolution_downsampling(resolution)?;
    let resolution_x0 = ceil_div_u32(component_x0, scale_x)?;
    let resolution_y0 = ceil_div_u32(component_y0, scale_y)?;
    let resolution_x1 = ceil_div_u32(component_x1, scale_x)?;
    let resolution_y1 = ceil_div_u32(component_y1, scale_y)?;
    let width = resolution_x1
        .checked_sub(resolution_x0)
        .ok_or_else(arithmetic_overflow)?;
    let height = resolution_y1
        .checked_sub(resolution_y0)
        .ok_or_else(arithmetic_overflow)?;
    let code_block_width = u32::from(coding_style.code_block_width);
    let code_block_height = u32::from(coding_style.code_block_height);
    if code_block_width == 0 || code_block_height == 0 {
        return Err(invalid_decode());
    }
    let code_blocks_across = ceil_div_u32(resolution_x1, code_block_width)?
        .checked_sub(floor_div_u32(resolution_x0, code_block_width)?)
        .ok_or_else(arithmetic_overflow)?;
    let code_blocks_down = ceil_div_u32(resolution_y1, code_block_height)?
        .checked_sub(floor_div_u32(resolution_y0, code_block_height)?)
        .ok_or_else(arithmetic_overflow)?;
    let code_blocks = u64::from(code_blocks_across)
        .checked_mul(u64::from(code_blocks_down))
        .ok_or_else(arithmetic_overflow)?;
    let (precinct_x, precinct_y) = coding_style.precinct_exponents(resolution)?;
    if precinct_x > 31 || precinct_y > 31 {
        return Err(invalid_decode());
    }
    let precinct_width = 1_u32 << u32::from(precinct_x);
    let precinct_height = 1_u32 << u32::from(precinct_y);
    let precinct_x0 = floor_div_u32(resolution_x0, precinct_width)?;
    let precinct_y0 = floor_div_u32(resolution_y0, precinct_height)?;
    let precinct_x1 = ceil_div_u32(resolution_x1, precinct_width)?;
    let precinct_y1 = ceil_div_u32(resolution_y1, precinct_height)?;
    let precincts_across = precinct_x1
        .checked_sub(precinct_x0)
        .ok_or_else(arithmetic_overflow)?;
    let precincts_down = precinct_y1
        .checked_sub(precinct_y0)
        .ok_or_else(arithmetic_overflow)?;
    let precincts = u64::from(precincts_across)
        .checked_mul(u64::from(precincts_down))
        .ok_or_else(arithmetic_overflow)?;
    Ok(ComponentResolutionGeometry {
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

pub fn derive_packet_grid(
    size: &ImageSize,
    coding_style: CodingStyle,
    limits: &ResourceLimits,
) -> Result<PacketGrid, DecodeError> {
    derive_packet_grid_for(size, coding_style, limits, None)
}

fn derive_packet_grid_for(
    size: &ImageSize,
    coding_style: CodingStyle,
    limits: &ResourceLimits,
    tile_index: Option<u16>,
) -> Result<PacketGrid, DecodeError> {
    let resolutions = coding_style
        .decomposition_levels
        .checked_add(1)
        .ok_or_else(arithmetic_overflow)?;
    let capacity = usize::from(size.components)
        .checked_mul(usize::from(resolutions))
        .ok_or_else(arithmetic_overflow)?;
    let mut precinct_counts = Vec::with_capacity(capacity);
    let mut total_code_blocks = 0_u64;
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= size.components,
            decreases size.components - component,
        )
    )]
    while component < size.components {
        let mut resolution = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant resolution <= resolutions,
                decreases resolutions - resolution,
            )
        )]
        while resolution < resolutions {
            let geometry = derive_component_resolution_geometry_for(
                size,
                coding_style,
                component,
                resolution,
                tile_index,
            )?;
            let subband_count = coding_style
                .subband_count_through_resolution(resolution)?
                .checked_sub(if resolution == 0 {
                    0
                } else {
                    coding_style.subband_count_through_resolution(
                        resolution.checked_sub(1).ok_or_else(arithmetic_overflow)?,
                    )?
                })
                .ok_or_else(arithmetic_overflow)?;
            total_code_blocks = total_code_blocks
                .checked_add(geometry.code_blocks)
                .ok_or_else(arithmetic_overflow)?;
            if total_code_blocks > limits.maximum_code_blocks() {
                return Err(DecodeError::limit(
                    crate::Resource::CodeBlocks,
                    limits.maximum_code_blocks(),
                    total_code_blocks,
                ));
            }

            precinct_counts.push(if subband_count == 0 {
                0
            } else {
                u32::try_from(geometry.precincts).map_err(|_| arithmetic_overflow())?
            });
            if resolution == u8::MAX {
                return Err(arithmetic_overflow());
            }
            let modeled_resolution = crate::refinement::wavelet_next_resolution(resolution);
            resolution = resolution.checked_add(1).ok_or_else(arithmetic_overflow)?;
            if resolution != modeled_resolution {
                return Err(arithmetic_overflow());
            }
        }
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    PacketGrid::new(
        coding_style.layers,
        size.components,
        resolutions,
        precinct_counts,
        limits,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures resolution_counts_outcome(result)
        == crate::specs::part2_component_resolution_counts(components, coding_styles, discard_levels),
))]
pub(crate) fn prepare_component_resolution_counts(
    components: u16,
    coding_styles: &ComponentCodingStyles,
    discard_levels: u8,
) -> Result<(u8, Vec<u8>, Vec<u8>), DecodeError> {
    let mut maximum_resolutions = 0_u8;
    let mut component_resolutions = Vec::with_capacity(usize::from(components));
    let mut selected_resolution_counts = Vec::with_capacity(usize::from(components));
    let mut component = 0_u16;
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part2_component_resolution_counts_run); }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant
                component <= components,
                crate::specs::part2_component_resolution_counts(components, coding_styles, discard_levels)
                    == crate::specs::part2_component_resolution_counts_run(
                        components, coding_styles, discard_levels, component, maximum_resolutions,
                        component_resolutions@, selected_resolution_counts@,
                    ),
            decreases components - component,
        )
    )]
    while component < components {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part2_component_resolution_counts_run); }
        let resolutions = coding_styles
            .style(component)?
            .decomposition_levels
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        maximum_resolutions = maximum_resolutions.max(resolutions);
        component_resolutions.push(resolutions);
        let selected_resolutions =
            resolutions
                .checked_sub(discard_levels)
                .ok_or(DecodeError::InvalidSelection {
                    reason: SelectionError::ReductionTooLarge,
                })?;
        if selected_resolutions == 0 {
            return Err(DecodeError::InvalidSelection {
                reason: SelectionError::ReductionTooLarge,
            });
        }
        selected_resolution_counts.push(selected_resolutions);
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part2_component_resolution_counts_run); }
    Ok((
        maximum_resolutions,
        component_resolutions,
        selected_resolution_counts,
    ))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures crate::packet::packet_grid_outcome(result) == crate::specs::part2_packet_grid_from_components(
        size, coding_styles, layers, limits, tile_index, discard_levels,
    ),
))]
pub(crate) fn derive_packet_grid_for_components(
    size: &ImageSize,
    coding_styles: &ComponentCodingStyles,
    layers: u16,
    limits: &ResourceLimits,
    tile_index: Option<u16>,
    discard_levels: u8,
) -> Result<PacketGrid, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { codestream::selection_geometry_known_fields(size); }
    let (maximum_resolutions, component_resolutions, selected_resolution_counts) =
        prepare_component_resolution_counts(size.components, coding_styles, discard_levels)?;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost descriptors = (maximum_resolutions, component_resolutions@, selected_resolution_counts@);
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part2_packet_grid_components_run); }
    let capacity = usize::from(size.components)
        .checked_mul(usize::from(maximum_resolutions))
        .ok_or_else(arithmetic_overflow)?;
    let mut precinct_counts = Vec::with_capacity(capacity);
    let mut total_code_blocks = 0_u64;
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant
                component <= size.components,
                codestream::image_size_component_count(size) == size.components,
                descriptors == (maximum_resolutions, component_resolutions@, selected_resolution_counts@),
                crate::specs::part2_packet_grid_from_components(size, coding_styles, layers, limits, tile_index, discard_levels)
                    == crate::specs::part2_packet_grid_components_run(
                        size, coding_styles, layers, limits, tile_index, discard_levels,
                        descriptors, component, precinct_counts@, total_code_blocks,
                    ),
            decreases size.components - component,
        )
    )]
    while component < size.components {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part2_packet_grid_components_run); }
        let style = coding_styles.style(component)?;
        let resolutions = style
            .decomposition_levels
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        let mut resolution = 0_u8;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(codestream::component_coding_style_result(coding_styles, component) == Ok(style));
            assert(style.decomposition_levels < u8::MAX);
            assert(resolutions as int == style.decomposition_levels as int + 1);
            crate::proofs::production_impl::production_packet_grid_component_start(
                size, coding_styles, layers, limits, tile_index, discard_levels,
                descriptors, component, precinct_counts@, total_code_blocks, style, resolutions,
            );
            assert(crate::specs::part2_packet_grid_components_run(
                size, coding_styles, layers, limits, tile_index, discard_levels,
                descriptors, component, precinct_counts@, total_code_blocks,
            ) == crate::specs::part2_packet_grid_resolutions_run(
                size, coding_styles, layers, limits, tile_index, discard_levels,
                descriptors, component, style, resolutions, 0, precinct_counts@, total_code_blocks,
            ));
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    component < size.components,
                    codestream::image_size_component_count(size) == size.components,
                    resolution <= maximum_resolutions,
                    descriptors == (maximum_resolutions, component_resolutions@, selected_resolution_counts@),
                    crate::specs::part2_packet_grid_from_components(size, coding_styles, layers, limits, tile_index, discard_levels)
                        == crate::specs::part2_packet_grid_resolutions_run(
                            size, coding_styles, layers, limits, tile_index, discard_levels,
                            descriptors, component, style, resolutions, resolution,
                            precinct_counts@, total_code_blocks,
                        ),
                decreases maximum_resolutions - resolution,
            )
        )]
        while resolution < maximum_resolutions {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part2_packet_grid_resolutions_run); }
            if resolution < resolutions {
                let geometry = derive_component_resolution_geometry_for(
                    size, style, component, resolution, tile_index,
                )?;
                let subband_count = style
                    .subband_count_through_resolution(resolution)?
                    .checked_sub(if resolution == 0 {
                        0
                    } else {
                        style.subband_count_through_resolution(
                            resolution.checked_sub(1).ok_or_else(arithmetic_overflow)?,
                        )?
                    })
                    .ok_or_else(arithmetic_overflow)?;
                let selected_resolutions = resolutions.checked_sub(discard_levels).ok_or(
                    DecodeError::InvalidSelection {
                        reason: SelectionError::ReductionTooLarge,
                    },
                )?;
                if resolution < selected_resolutions {
                    total_code_blocks = total_code_blocks
                        .checked_add(geometry.code_blocks)
                        .ok_or_else(arithmetic_overflow)?;
                    if total_code_blocks > limits.maximum_code_blocks() {
                        return Err(DecodeError::LimitExceeded {
                            resource: crate::Resource::CodeBlocks,
                            limit: limits.maximum_code_blocks(),
                            actual: total_code_blocks,
                        });
                    }
                }
                precinct_counts.push(if subband_count == 0 {
                    0
                } else {
                    match u32::try_from(geometry.precincts) {
                        Ok(count) => count,
                        Err(_) => return Err(arithmetic_overflow()),
                    }
                });
            } else {
                precinct_counts.push(0);
            }
            resolution = resolution.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part2_packet_grid_resolutions_run);
        }
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part2_packet_grid_components_run); }
    crate::packet::packet_grid_new_with_selected_resolutions(
        layers,
        size.components,
        component_resolutions,
        precinct_counts,
        if discard_levels == 0 {
            None
        } else {
            Some(&selected_resolution_counts)
        },
        limits,
    )
}

impl PlannedTileComponent {
    pub fn subbands(&self) -> &[SubbandGeometry] {
        &self.subbands
    }

    pub fn placements(&self) -> &[ProgressionCodeBlockPlacement] {
        &self.placements
    }
}

impl ProgressionTilePlan {
    pub const fn tile_index(&self) -> u16 {
        self.tile_index
    }

    pub const fn grid(&self) -> &PacketGrid {
        &self.grid
    }

    pub fn precincts(&self) -> &[PacketPrecinctDecoder] {
        &self.precincts
    }

    pub fn precincts_mut(&mut self) -> &mut [PacketPrecinctDecoder] {
        &mut self.precincts
    }

    pub fn packet_execution(&mut self) -> (&PacketGrid, &mut [PacketPrecinctDecoder]) {
        (&self.grid, &mut self.precincts)
    }

    pub fn components(&self) -> &[PlannedTileComponent] {
        &self.components
    }
}

impl PreparedTileComponent {
    pub const fn component(&self) -> u16 {
        self.component
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn fraction_bits(&self) -> u8 {
        self.fraction_bits
    }

    pub fn samples(&self) -> &[i64] {
        &self.samples
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures crate::image::assembly_tile_view(result) == crate::specs::part1_prepared_as_tile(prepared_component_view(*self)),
    ))]
    pub fn as_tile_component(&self) -> TileComponent<'_> {
        TileComponent {
            component: self.component,
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            fraction_bits: self.fraction_bits,
            samples: &self.samples,
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures
        (result, Seq::new((final(components)@.len() - old(components)@.len()) as nat, |index: int|
            crate::image::transform_component_mut_view(&final(components)@[old(components)@.len() as int + index])))
            == crate::specs::part2_mct_prepared_inputs(
                Seq::new(old(prepared)@.len(), |index: int| prepared_component_view(old(prepared)@[index])),
                codestream::image_size_component_count(size), codestream::image_size_component_infos(size)),
        final(components)@.len() == old(components)@.len()
            + crate::specs::part2_mct_prepared_inputs(
                Seq::new(old(prepared)@.len(), |index: int| prepared_component_view(old(prepared)@[index])),
                codestream::image_size_component_count(size), codestream::image_size_component_infos(size)).1.len(),
        final(components)@.take(old(components)@.len() as int) == old(components)@,
        final(prepared)@.len() == old(prepared)@.len(),
        forall |index: int| 0 <= index < old(prepared)@.len() ==>
            prepared_component_view(final(prepared)@[index])
                == if index < final(components)@.len() - old(components)@.len() {
                    (old(prepared)@[index].component, old(prepared)@[index].x, old(prepared)@[index].y,
                        old(prepared)@[index].width, old(prepared)@[index].height,
                        crate::image::transform_component_final_precision(&final(components)@[old(components)@.len() as int + index]),
                        crate::image::transform_component_final_samples(&final(components)@[old(components)@.len() as int + index]))
                } else { prepared_component_view(old(prepared)@[index]) },
    decreases prepared.len(),
))]
fn collect_prepared_transform_components<'a>(
    prepared: &'a mut [PreparedTileComponent],
    size: &ImageSize,
    components: &mut Vec<TransformComponentMut<'a>>,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original = prepared@; let ghost prefix = components@; }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost input = Seq::new(original.len(), |index: int| prepared_component_view(original[index])); }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part2_mct_prepared_inputs, 1); }
    let Some((component, remaining)) = prepared.split_first_mut() else {
        return Ok(());
    };
    let info = size.component(component.component)?;
    components.push(TransformComponentMut {
        component: component.component,
        x: component.x,
        y: component.y,
        width: component.width,
        height: component.height,
        x_subsampling: info.x_subsampling,
        y_subsampling: info.y_subsampling,
        fraction_bits: &mut component.fraction_bits,
        samples: &mut component.samples,
    });
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost added = components@[prefix.len() as int]; }
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost added_view = crate::image::transform_component_mut_view(&added);
        let ghost tail_input = Seq::new(remaining@.len(), |index: int| prepared_component_view(remaining@[index]));
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(input.drop_first() =~= tail_input);
        assert(crate::specs::part2_mct_prepared_input(input[0], codestream::image_size_component_count(size),
            codestream::image_size_component_infos(size)) == Ok(added_view));
    }
    let result = collect_prepared_transform_components(remaining, size, components);
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(components@[prefix.len() as int] == added);
        assert(components@.take(prefix.len() as int) =~= prefix);
        let tail = crate::specs::part2_mct_prepared_inputs(tail_input, codestream::image_size_component_count(size),
            codestream::image_size_component_infos(size));
        assert(components@.len() == prefix.len() + 1 + tail.1.len());
        assert(Seq::new((components@.len() - prefix.len()) as nat, |index: int|
            crate::image::transform_component_mut_view(&components@[prefix.len() as int + index]))
            =~= seq![added_view] + tail.1);
    }
    result
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result,
        Seq::new(final(prepared)@.len(), |index: int| prepared_component_view(final(prepared)@[index])),
        *final(work)) == crate::specs::part2_mct_prepared_tiles(
            Seq::new(old(prepared)@.len(), |index: int| prepared_component_view(old(prepared)@[index])),
            codestream::image_size_component_count(size), codestream::image_size_component_infos(size),
            codestream::multiple_component_transform_view(transform), *old(work),
            crate::limits::resource_work_limit(limits), crate::limits::resource_decoded_bytes_limit(limits)),
))]
fn apply_tile_multiple_component_transform(
    prepared: &mut [PreparedTileComponent],
    size: &ImageSize,
    transform: &codestream::MultipleComponentTransform,
    limits: &ResourceLimits,
    work: &mut u64,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost before = Seq::new(prepared@.len(), |index: int| prepared_component_view(prepared@[index]));
        let ghost initial_work = *work;
    }
    let mut components = Vec::with_capacity(prepared.len());
    let collected_result = collect_prepared_transform_components(prepared, size, &mut components);
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_mct_prepared_inputs_shape(
            before, codestream::image_size_component_count(size), codestream::image_size_component_infos(size));
    }
    match collected_result {
        Ok(()) => {}
        Err(error) => {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert forall |index: int| 0 <= index < components@.len() implies
                    crate::image::transform_component_final_samples(&components@[index])
                        == crate::image::transform_component_mut_view(&components@[index]).3
                        && crate::image::transform_component_final_precision(&components@[index])
                            == crate::image::transform_component_mut_view(&components@[index]).2 by {}
                assert(Seq::new(final(prepared)@.len(), |index: int| prepared_component_view(final(prepared)@[index])) =~= before);
            }
            return Err(error);
        }
    }
    #[cfg(verus_keep_ghost)]
    proof! { assert(components@.len() == before.len()); }
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost collected = components@;
        let ghost input = Seq::new(components@.len(), |index: int| crate::image::transform_component_mut_view(&components@[index]));
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_mct_unit_result(collected_result);
        assert(collected_result == Result::<(), DecodeError>::Ok(()));
        assert(crate::specs::part2_mct_prepared_inputs(before,
            codestream::image_size_component_count(size), codestream::image_size_component_infos(size)).0 == collected_result);
        assert(input =~= crate::specs::part2_mct_prepared_inputs(before,
            codestream::image_size_component_count(size), codestream::image_size_component_infos(size)).1);
        assert(crate::specs::part2_mct_prepared_inputs(before,
            codestream::image_size_component_count(size), codestream::image_size_component_infos(size)) == (Result::<(), DecodeError>::Ok(()), input));
    }
    let result =
        apply_multiple_component_transform_components(&mut components, transform, limits, work);
    #[cfg(verus_keep_ghost)]
    proof! {
        assert((result, Seq::new(components@.len(), |index: int| crate::image::transform_component_mut_view(&components@[index])), *work)
            == crate::specs::part2_mct_transform(input, codestream::multiple_component_transform_view(transform),
                initial_work, crate::limits::resource_work_limit(limits), crate::limits::resource_decoded_bytes_limit(limits)));
        assert forall |index: int| 0 <= index < components@.len() implies
            crate::image::transform_component_final_samples(&collected[index])
                == crate::image::transform_component_mut_view(&components@[index]).3
                && crate::image::transform_component_final_precision(&collected[index])
                    == crate::image::transform_component_mut_view(&components@[index]).2 by {}
        assert(Seq::new(final(prepared)@.len(), |index: int| prepared_component_view(final(prepared)@[index]))
            =~= crate::specs::part2_mct_prepared_result(before,
                Seq::new(components@.len(), |index: int| crate::image::transform_component_mut_view(&components@[index]))));
    }
    result
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_quantization_step_at_index(
        codestream::quantization_view(quantization), resolution, index,
    ),
))]
pub(crate) fn quantization_step_at_index(
    quantization: &Quantization,
    resolution: u8,
    index: u16,
) -> Result<QuantizationStep, DecodeError> {
    match quantization.style() {
        QuantizationStyle::None | QuantizationStyle::ScalarExpounded => quantization.step(index),
        QuantizationStyle::ScalarDerived => {
            let base = quantization.step(0)?;
            Ok(QuantizationStep {
                exponent: base.exponent.saturating_sub(resolution.saturating_sub(1)),
                mantissa: base.mantissa,
            })
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_quantization_for_subband(
        codestream::quantization_view(quantization), resolution, orientation,
    ),
))]
pub fn quantization_step_for_subband(
    quantization: &Quantization,
    resolution: u8,
    orientation: SubbandOrientation,
) -> Result<(u16, QuantizationStep), DecodeError> {
    let orientation_index = if resolution == 0 {
        if !matches!(orientation, SubbandOrientation::Ll) {
            return Err(invalid_decode());
        }
        0_u16
    } else {
        let offset = match orientation {
            SubbandOrientation::Hl => 0_u16,
            SubbandOrientation::Lh => 1_u16,
            SubbandOrientation::Hh => 2_u16,
            SubbandOrientation::Ll => return Err(invalid_decode()),
        };
        let level = u16::from(resolution)
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?;
        let index = level.checked_mul(3).ok_or_else(arithmetic_overflow)?;
        let index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        index.checked_add(offset).ok_or_else(arithmetic_overflow)?
    };
    let step = quantization_step_at_index(quantization, resolution, orientation_index)?;
    Ok((orientation_index, step))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures subbands_outcome(result) == crate::specs::part2_component_subbands(
        size, coding_style, quantization, component, None, 0,
    ),
))]
pub fn derive_component_subbands(
    size: &ImageSize,
    coding_style: CodingStyle,
    quantization: &Quantization,
    component: u16,
) -> Result<Vec<SubbandGeometry>, DecodeError> {
    derive_component_subbands_for(size, coding_style, quantization, component, None, 0)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures subbands_outcome(result) == crate::specs::part2_component_subbands(
        size, coding_style, quantization, component, tile_index, roi_shift,
    ),
))]
pub(crate) fn derive_component_subbands_for(
    size: &ImageSize,
    coding_style: CodingStyle,
    quantization: &Quantization,
    component: u16,
    tile_index: Option<u16>,
    roi_shift: u8,
) -> Result<Vec<SubbandGeometry>, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { codestream::resolution_selectors_known_fields(coding_style); }
    let info = size.component(component)?;
    let component_bounds = component_coordinate_bounds(size, component, tile_index)?;
    let expected_count = coding_style.subband_count();
    let mut subbands = Vec::with_capacity(usize::from(expected_count));
    let mut resolution = 0_u8;
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part2_subband_vector_run); }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant
                resolution as int <= coding_style.decomposition_levels as int + 1,
                crate::specs::part2_component_subbands(size, coding_style, quantization, component, tile_index, roi_shift)
                    == crate::specs::part2_subband_vector_run(
                        coding_style, component_bounds, [info.precision, roi_shift],
                        codestream::quantization_view(quantization), expected_count, resolution, subbands@,
                    ),
            decreases coding_style.decomposition_levels as int + 1 - resolution as int,
        )
    )]
    while resolution <= coding_style.decomposition_levels {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part2_subband_vector_run); }
        subbands = derive_subband_resolution(
            coding_style,
            component_bounds,
            [info.precision, roi_shift],
            quantization,
            resolution,
            subbands,
        )?;
        resolution = resolution.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part2_subband_vector_run); }
    if subbands.len() != usize::from(expected_count) {
        return Err(invalid_decode());
    }
    Ok(subbands)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures subbands_outcome(result) == crate::specs::part2_subband_resolution(
        coding_style, component_bounds, component_depth, codestream::quantization_view(quantization), resolution, before@,
    ),
))]
pub(crate) fn derive_subband_resolution(
    coding_style: CodingStyle,
    component_bounds: [u32; 4],
    component_depth: [u8; 2],
    quantization: &Quantization,
    resolution: u8,
    before: Vec<SubbandGeometry>,
) -> Result<Vec<SubbandGeometry>, DecodeError> {
    let component_x0 = component_bounds[0];
    let component_y0 = component_bounds[1];
    let component_x1 = component_bounds[2];
    let component_y1 = component_bounds[3];
    let mut subbands = before;
    let (scale_x, scale_y) = coding_style.resolution_downsampling(resolution)?;
    let resolution_x0 = ceil_div_u32(component_x0, scale_x)?;
    let resolution_y0 = ceil_div_u32(component_y0, scale_y)?;
    let resolution_x1 = ceil_div_u32(component_x1, scale_x)?;
    let resolution_y1 = ceil_div_u32(component_y1, scale_y)?;
    let resolution_width = resolution_x1
        .checked_sub(resolution_x0)
        .ok_or_else(arithmetic_overflow)?;
    let resolution_height = resolution_y1
        .checked_sub(resolution_y0)
        .ok_or_else(arithmetic_overflow)?;
    if resolution == 0 {
        subbands.push(make_subband_geometry(
            resolution,
            SubbandOrientation::Ll,
            [resolution_x0, resolution_y0, resolution_x1, resolution_y1],
            [resolution_width, resolution_height],
            [true, true],
            component_depth,
            quantization,
            0,
        )?);
    } else {
        let low_x0 = ceil_div_u32(resolution_x0, 2)?;
        let low_y0 = ceil_div_u32(resolution_y0, 2)?;
        let low_x1 = ceil_div_u32(resolution_x1, 2)?;
        let low_y1 = ceil_div_u32(resolution_y1, 2)?;
        let high_x0 = resolution_x0 / 2;
        let high_y0 = resolution_y0 / 2;
        let high_x1 = resolution_x1 / 2;
        let high_y1 = resolution_y1 / 2;
        let phase = [
            resolution_x0.is_multiple_of(2),
            resolution_y0.is_multiple_of(2),
        ];
        let direction = coding_style.direction_for_resolution(resolution)?;
        if matches!(
            direction,
            crate::codestream::DecompositionDirection::TwoDimensional
                | crate::codestream::DecompositionDirection::Horizontal
        ) {
            let index = match u16::try_from(subbands.len()) {
                Ok(index) => index,
                Err(_) => return Err(arithmetic_overflow()),
            };
            let y_bounds = if matches!(
                direction,
                crate::codestream::DecompositionDirection::TwoDimensional
            ) {
                [low_y0, low_y1]
            } else {
                [resolution_y0, resolution_y1]
            };
            subbands.push(make_subband_geometry(
                resolution,
                SubbandOrientation::Hl,
                [high_x0, y_bounds[0], high_x1, y_bounds[1]],
                [resolution_width, resolution_height],
                phase,
                component_depth,
                quantization,
                index,
            )?);
        }
        if matches!(
            direction,
            crate::codestream::DecompositionDirection::TwoDimensional
                | crate::codestream::DecompositionDirection::Vertical
        ) {
            let index = match u16::try_from(subbands.len()) {
                Ok(index) => index,
                Err(_) => return Err(arithmetic_overflow()),
            };
            let x_bounds = if matches!(
                direction,
                crate::codestream::DecompositionDirection::TwoDimensional
            ) {
                [low_x0, low_x1]
            } else {
                [resolution_x0, resolution_x1]
            };
            subbands.push(make_subband_geometry(
                resolution,
                SubbandOrientation::Lh,
                [x_bounds[0], high_y0, x_bounds[1], high_y1],
                [resolution_width, resolution_height],
                phase,
                component_depth,
                quantization,
                index,
            )?);
        }
        if matches!(
            direction,
            crate::codestream::DecompositionDirection::TwoDimensional
        ) {
            let index = match u16::try_from(subbands.len()) {
                Ok(index) => index,
                Err(_) => return Err(arithmetic_overflow()),
            };
            subbands.push(make_subband_geometry(
                resolution,
                SubbandOrientation::Hh,
                [high_x0, high_y0, high_x1, high_y1],
                [resolution_width, resolution_height],
                phase,
                component_depth,
                quantization,
                index,
            )?);
        }
    }
    Ok(subbands)
}

pub fn plan_progression_tile(
    size: &ImageSize,
    coding_style: CodingStyle,
    quantization: &Quantization,
    limits: &ResourceLimits,
) -> Result<ProgressionTilePlan, DecodeError> {
    plan_progression_tile_at_with_roi(
        size,
        0,
        coding_style,
        quantization,
        &RoiShifts::none(),
        limits,
    )
}

pub fn plan_progression_tile_at(
    size: &ImageSize,
    tile_index: u16,
    coding_style: CodingStyle,
    quantization: &Quantization,
    limits: &ResourceLimits,
) -> Result<ProgressionTilePlan, DecodeError> {
    plan_progression_tile_at_with_roi(
        size,
        tile_index,
        coding_style,
        quantization,
        &RoiShifts::none(),
        limits,
    )
}

pub fn plan_progression_tile_at_with_roi(
    size: &ImageSize,
    tile_index: u16,
    coding_style: CodingStyle,
    quantization: &Quantization,
    roi_shifts: &RoiShifts,
    limits: &ResourceLimits,
) -> Result<ProgressionTilePlan, DecodeError> {
    let coding_styles = ComponentCodingStyles::uniform(size.components, coding_style)?;
    let quantizations = ComponentQuantizations::uniform(size.components, *quantization)?;
    plan_progression_tile_at_with_component_overrides(
        size,
        tile_index,
        coding_style,
        &coding_styles,
        &quantizations,
        roi_shifts,
        limits,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures progression_plan_outcome(result) == crate::specs::part2_progression_tile_plan(
        size, coding_style, coding_styles, quantizations, roi_shifts, (tile_index, 0), (false, 74, false), limits,
    ),
))]
pub fn plan_progression_tile_at_with_component_overrides(
    size: &ImageSize,
    tile_index: u16,
    coding_style: CodingStyle,
    coding_styles: &ComponentCodingStyles,
    quantizations: &ComponentQuantizations,
    roi_shifts: &RoiShifts,
    limits: &ResourceLimits,
) -> Result<ProgressionTilePlan, DecodeError> {
    plan_progression_tile_at_selected(
        size,
        tile_index,
        coding_style,
        coding_styles,
        quantizations,
        roi_shifts,
        0,
        false,
        74,
        false,
        limits,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures component_plan_metadata_outcome(result) == crate::specs::part2_component_plan_metadata(
        size, coding_styles, quantizations, roi_shifts, component, selection, force_high_throughput,
    ),
))]
pub(crate) fn prepare_component_plan_metadata(
    size: &ImageSize,
    coding_styles: &ComponentCodingStyles,
    quantizations: &ComponentQuantizations,
    roi_shifts: &RoiShifts,
    component: u16,
    selection: (u16, u8),
    force_high_throughput: bool,
) -> Result<PlannedComponentMetadata, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { codestream::selection_geometry_known_fields(size); }
    let (tile_index, discard_levels) = selection;
    let component_coding_style = coding_styles.style(component)?;
    let output_resolution = component_coding_style
        .decomposition_levels
        .checked_sub(discard_levels)
        .ok_or(DecodeError::InvalidSelection {
            reason: SelectionError::ReductionTooLarge,
        })?;
    let component_quantization = quantizations.quantization(component)?;
    let mut tier1_style = component_coding_style.tier1_coding_style()?;
    if tier1_style.block_coding.uses_high_throughput() && force_high_throughput {
        tier1_style.set_force_high_throughput(true);
    }
    let bounds = size.tile_component_bounds(tile_index, component)?;
    let output_geometry = derive_component_resolution_geometry_for(
        size,
        component_coding_style,
        component,
        output_resolution,
        Some(tile_index),
    )?;
    let component_width = output_geometry.width;
    let component_height = output_geometry.height;
    let (scale_x, scale_y) = component_coding_style.resolution_downsampling(output_resolution)?;
    let info = size.component(component)?;
    let component_image_x0 = ceil_div_u32(size.x_origin, u32::from(info.x_subsampling))?;
    let component_image_y0 = ceil_div_u32(size.y_origin, u32::from(info.y_subsampling))?;
    let component_x = ceil_div_u32(bounds.x0, scale_x)?
        .checked_sub(ceil_div_u32(component_image_x0, scale_x)?)
        .ok_or_else(arithmetic_overflow)?;
    let component_y = ceil_div_u32(bounds.y0, scale_y)?
        .checked_sub(ceil_div_u32(component_image_y0, scale_y)?)
        .ok_or_else(arithmetic_overflow)?;
    let subbands = derive_component_subbands_for(
        size,
        component_coding_style,
        &component_quantization,
        component,
        Some(tile_index),
        roi_shifts.shift(component)?,
    )?;
    Ok(PlannedComponentMetadata {
        component_coding_style,
        output_resolution,
        tier1_style,
        bounds,
        component_width,
        component_height,
        component_x,
        component_y,
        subbands,
    })
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures progression_plan_outcome(result) == crate::specs::part2_progression_tile_plan(
        size, coding_style, coding_styles, quantizations, roi_shifts, (tile_index, discard_levels),
        (allow_multiple_ht_sets, ht_magnitude_bound, force_high_throughput), limits,
    ),
))]
fn plan_progression_tile_at_selected(
    size: &ImageSize,
    tile_index: u16,
    coding_style: CodingStyle,
    coding_styles: &ComponentCodingStyles,
    quantizations: &ComponentQuantizations,
    roi_shifts: &RoiShifts,
    discard_levels: u8,
    allow_multiple_ht_sets: bool,
    ht_magnitude_bound: u8,
    force_high_throughput: bool,
    limits: &ResourceLimits,
) -> Result<ProgressionTilePlan, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { codestream::selection_geometry_known_fields(size); }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost goal = crate::specs::part2_progression_tile_plan(
        size, coding_style, coding_styles, quantizations, roi_shifts, (tile_index, discard_levels),
        (allow_multiple_ht_sets, ht_magnitude_bound, force_high_throughput), limits,
    ); }
    size.tile_bounds(tile_index)?;
    let grid = derive_packet_grid_for_components(
        size,
        coding_styles,
        coding_style.layers,
        limits,
        Some(tile_index),
        discard_levels,
    )?;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original_grid = crate::packet::packet_grid_view(&grid); }
    let mut precincts = Vec::new();
    let mut components = Vec::with_capacity(usize::from(size.components));
    let mut planned_code_blocks = 0_u64;
    let mut component = 0_u16;
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(tile_plan_buffers_view);
        assert(planning_precincts_view(precincts@) =~= Seq::empty());
        assert(planning_components_view(components@) =~= Seq::empty());
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= size.components,
                codestream::image_size_component_count(size) == size.components,
                crate::packet::packet_grid_view(&grid) == original_grid,
                goal == crate::specs::part2_progression_tile_plan(
                    size, coding_style, coding_styles, quantizations, roi_shifts, (tile_index, discard_levels),
                    (allow_multiple_ht_sets, ht_magnitude_bound, force_high_throughput), limits,
                ),
                goal == crate::specs::part2_tile_plan_components(
                    size, coding_style, coding_styles, codestream::quantization_owner_view(quantizations), roi_shifts, (tile_index, discard_levels),
                    (allow_multiple_ht_sets, ht_magnitude_bound, force_high_throughput), limits, original_grid,
                    component, tile_plan_buffers_view(precincts@, components@, planned_code_blocks),
                ),
            decreases size.components - component,
        )
    )]
    while component < size.components {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost before = tile_plan_buffers_view(precincts@, components@, planned_code_blocks); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part2_tile_plan_components, 1); }
        let planned = plan_tile_component(
            size,
            coding_styles,
            quantizations,
            roi_shifts,
            component,
            (tile_index, discard_levels),
            (
                allow_multiple_ht_sets,
                ht_magnitude_bound,
                force_high_throughput,
            ),
            precincts,
            planned_code_blocks,
            limits,
        )?;
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost row = planned_component_view(&planned); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(planned_component_view); reveal(tile_plan_buffers_view); }
        precincts = planned.precincts;
        planned_code_blocks = planned.planned_code_blocks;
        components.push(planned.component);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(planning_components_view(components@) =~= before.components.push(row.component));
        }
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal_with_fuel(crate::specs::part2_tile_plan_components, 1);
        reveal(tile_plan_buffers_view);
        reveal(progression_plan_view);
    }
    Ok(ProgressionTilePlan {
        tile_index,
        coding_style,
        component_coding_styles: *coding_styles,
        grid,
        precincts,
        components,
    })
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures planned_component_outcome(result) == crate::specs::part2_planned_component(
        size, coding_styles, quantizations, roi_shifts, component, selection, policy,
        planning_precincts_view(precincts@), planned_code_blocks, limits,
    ),
))]
pub(crate) fn plan_tile_component(
    size: &ImageSize,
    coding_styles: &ComponentCodingStyles,
    quantizations: &ComponentQuantizations,
    roi_shifts: &RoiShifts,
    component: u16,
    selection: (u16, u8),
    policy: (bool, u8, bool),
    precincts: Vec<PacketPrecinctDecoder>,
    planned_code_blocks: u64,
    limits: &ResourceLimits,
) -> Result<PlannedComponent, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost goal = crate::specs::part2_planned_component(
        size, coding_styles, quantizations, roi_shifts, component, selection, policy,
        planning_precincts_view(precincts@), planned_code_blocks, limits,
    ); }
    let metadata = prepare_component_plan_metadata(
        size,
        coding_styles,
        quantizations,
        roi_shifts,
        component,
        selection,
        policy.2,
    )?;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost meta = component_plan_metadata_view(&metadata); }
    let mut builder = ComponentPlanBuilder {
        precincts,
        placements: Vec::new(),
        planned_code_blocks,
    };
    let mut resolution = 0_u8;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant resolution <= metadata.component_coding_style.decomposition_levels as int + 1,
            meta == component_plan_metadata_view(&metadata),
            goal == crate::specs::part2_planned_component(
                size, coding_styles, quantizations, roi_shifts, component, selection, policy,
                planning_precincts_view(precincts@), planned_code_blocks, limits,
            ),
            goal == crate::specs::part2_component_plan_resolutions(
                size, component, selection.0, meta, policy, limits, resolution as nat, component_plan_builder_view(&builder),
            ),
        decreases metadata.component_coding_style.decomposition_levels as int + 1 - resolution as int,
    ))]
    while resolution <= metadata.component_coding_style.decomposition_levels {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(component_plan_metadata_view);
            reveal(component_plan_builder_view);
            reveal_with_fuel(crate::specs::part2_component_plan_resolutions, 1);
        }
        let data = prepare_resolution_plan_metadata(
            size,
            metadata.component_coding_style,
            component,
            (selection.0, resolution),
            [metadata.bounds.x0, metadata.bounds.y0],
        )?;
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost resolution_data = resolution_plan_metadata_view(&data); }
        let mut precinct = 0_u32;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant precinct <= data.precincts_in_resolution,
                resolution <= metadata.component_coding_style.decomposition_levels,
                meta == component_plan_metadata_view(&metadata),
                resolution_data == resolution_plan_metadata_view(&data),
                goal == crate::specs::part2_planned_component(
                    size, coding_styles, quantizations, roi_shifts, component, selection, policy,
                    planning_precincts_view(precincts@), planned_code_blocks, limits,
                ),
                goal == match crate::specs::part2_component_plan_precincts(
                    component, resolution, meta, resolution_data, policy, limits, precinct as nat, component_plan_builder_view(&builder),
                ) {
                    Err(error) => Err(error),
                    Ok(after) => if resolution == u8::MAX { Err(crate::specs::part1_block_placement_error(true)) }
                        else { crate::specs::part2_component_plan_resolutions(
                            size, component, selection.0, meta, policy, limits, resolution as nat + 1, after) },
                },
            decreases data.precincts_in_resolution - precinct,
        ))]
        while precinct < data.precincts_in_resolution {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = component_plan_builder_view(&builder); }
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal(component_plan_metadata_view);
                reveal(resolution_plan_metadata_view);
                reveal(component_plan_builder_view);
                reveal_with_fuel(crate::specs::part2_component_plan_precincts, 1);
            }
            let origin = derive_planned_precinct_origin(
                data.first_precinct,
                data.geometry.precincts_across,
                precinct,
                data.band_size,
            )?;
            let planned = plan_packet_precinct(
                [origin[0], origin[1], data.band_size[0], data.band_size[1]],
                data.block_size,
                &metadata.subbands,
                (data.first_subband, data.subband_count),
                metadata.tier1_style,
                (policy.0, policy.1),
                (component, resolution, precinct, builder.precincts.len()),
                builder.placements,
                builder.planned_code_blocks,
                resolution <= metadata.output_resolution,
                limits,
            )?;
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost row = planned_precinct_view(&planned); }
            #[cfg(verus_keep_ghost)]
            proof! { reveal(planned_precinct_view); }
            builder.placements = planned.placements;
            builder.planned_code_blocks = planned.planned_code_blocks;
            builder.precincts.push(planned.precinct);
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(planning_precincts_view(builder.precincts@) =~= before.precincts.push(row.precinct));
                assert(crate::specs::part2_component_plan_precinct(
                    component, resolution, meta, resolution_data, policy, limits, precinct, before,
                ) == Ok(component_plan_builder_view(&builder)));
            }
            precinct = precinct.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(resolution_plan_metadata_view);
            reveal_with_fuel(crate::specs::part2_component_plan_precincts, 1);
        }
        resolution = resolution.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(component_plan_metadata_view);
        reveal(component_plan_builder_view);
        reveal(planned_component_view);
        reveal(prepare_plan_view);
        reveal_with_fuel(crate::specs::part2_component_plan_resolutions, 1);
    }
    Ok(PlannedComponent {
        component: PlannedTileComponent {
            component,
            x: metadata.component_x,
            y: metadata.component_y,
            width: metadata.component_width,
            height: metadata.component_height,
            output_resolution: metadata.output_resolution,
            subbands: metadata.subbands,
            placements: builder.placements,
        },
        precincts: builder.precincts,
        planned_code_blocks: builder.planned_code_blocks,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_planned_precinct_origin(first, across, precinct, band_size),
))]
pub(crate) fn derive_planned_precinct_origin(
    first: [u32; 2],
    across: u32,
    precinct: u32,
    band_size: [u32; 2],
) -> Result<[u32; 2], DecodeError> {
    let x = first[0]
        .checked_add(
            precinct
                .checked_rem(across)
                .ok_or_else(arithmetic_overflow)?,
        )
        .ok_or_else(arithmetic_overflow)?;
    let y = first[1]
        .checked_add(
            precinct
                .checked_div(across)
                .ok_or_else(arithmetic_overflow)?,
        )
        .ok_or_else(arithmetic_overflow)?;
    let band_x = x
        .checked_mul(band_size[0])
        .ok_or_else(arithmetic_overflow)?;
    let band_y = y
        .checked_mul(band_size[1])
        .ok_or_else(arithmetic_overflow)?;
    Ok([band_x, band_y])
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures resolution_plan_metadata_outcome(result) == crate::specs::part2_resolution_plan_metadata(
        size, style, component, selection, component_origin,
    ),
))]
pub(crate) fn prepare_resolution_plan_metadata(
    size: &ImageSize,
    style: CodingStyle,
    component: u16,
    selection: (u16, u8),
    component_origin: [u32; 2],
) -> Result<ResolutionPlanMetadata, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { codestream::resolution_selectors_known_fields(style); }
    let (tile, resolution) = selection;
    let geometry =
        derive_component_resolution_geometry_for(size, style, component, resolution, Some(tile))?;
    let first_subband = if resolution == 0 {
        0
    } else {
        style.subband_count_through_resolution(
            resolution.checked_sub(1).ok_or_else(arithmetic_overflow)?,
        )?
    };
    let subband_count = style
        .subband_count_through_resolution(resolution)?
        .checked_sub(first_subband)
        .ok_or_else(arithmetic_overflow)?;
    let precincts_in_resolution = if subband_count == 0 {
        0
    } else {
        match u32::try_from(geometry.precincts) {
            Ok(count) => count,
            Err(_) => return Err(arithmetic_overflow()),
        }
    };
    let (px, py) = style.precinct_exponents(resolution)?;
    if px > 31 || py > 31 {
        return Err(invalid_decode());
    }
    let scale = style.resolution_downsampling(resolution)?;
    let rx0 = ceil_div_u32(component_origin[0], scale.0)?;
    let ry0 = ceil_div_u32(component_origin[1], scale.1)?;
    let precinct_width = 1_u32 << u32::from(px);
    let precinct_height = 1_u32 << u32::from(py);
    let first_x = floor_div_u32(rx0, precinct_width)?;
    let first_y = floor_div_u32(ry0, precinct_height)?;
    let direction = if resolution == 0 {
        codestream::DecompositionDirection::None
    } else {
        style.direction_for_resolution(resolution)?
    };
    let split_x = matches!(
        direction,
        codestream::DecompositionDirection::Horizontal
            | codestream::DecompositionDirection::TwoDimensional
    );
    let split_y = matches!(
        direction,
        codestream::DecompositionDirection::Vertical
            | codestream::DecompositionDirection::TwoDimensional
    );
    let bx = if split_x {
        px.checked_sub(1).ok_or_else(invalid_decode)?
    } else {
        px
    };
    let by = if split_y {
        py.checked_sub(1).ok_or_else(invalid_decode)?
    } else {
        py
    };
    if bx > 31 || by > 31 {
        return Err(invalid_decode());
    }
    let band_width = 1_u32 << u32::from(bx);
    let band_height = 1_u32 << u32::from(by);
    let block_width = band_width.min(u32::from(style.code_block_width));
    let block_height = band_height.min(u32::from(style.code_block_height));
    if precinct_width == 0 || precinct_height == 0 || block_width == 0 || block_height == 0 {
        return Err(invalid_decode());
    }
    Ok(ResolutionPlanMetadata {
        geometry,
        first_subband,
        subband_count,
        precincts_in_resolution,
        first_precinct: [first_x, first_y],
        band_size: [band_width, band_height],
        block_size: [block_width, block_height],
    })
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures planned_precinct_outcome(result) == crate::specs::part2_planned_precinct(
        band, block_size, subbands@, range, style, policy, location, placements@, planned_code_blocks, selected, limits,
    ),
))]
pub(crate) fn plan_packet_precinct(
    band: [u32; 4],
    block_size: [u32; 2],
    subbands: &[SubbandGeometry],
    range: (usize, usize),
    style: Tier1CodingStyle,
    policy: (bool, u8),
    location: (u16, u8, u32, usize),
    placements: Vec<ProgressionCodeBlockPlacement>,
    planned_code_blocks: u64,
    selected: bool,
    limits: &ResourceLimits,
) -> Result<PlannedPrecinct, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost goal = crate::specs::part2_planned_precinct(
        band, block_size, subbands@, range, style, policy, location, placements@, planned_code_blocks, selected, limits,
    ); }
    let mut builder = PrecinctPlanBuilder {
        executions: Vec::with_capacity(range.1),
        placements,
        decoder_index: 0,
        planned_code_blocks,
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(precinct_plan_builder_view);
        assert(crate::packet::packet_executions_view(builder.executions@) =~= Seq::empty());
    }
    let mut local_subband = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant local_subband <= range.1,
            goal == crate::specs::part2_planned_precinct(
                band, block_size, subbands@, range, style, policy, location, placements@, planned_code_blocks, selected, limits,
            ),
            goal == crate::specs::part2_planned_precinct_rows(
                band, block_size, subbands@, range, style, policy, location, selected, limits,
                local_subband as nat, precinct_plan_builder_view(&builder),
            ),
        decreases range.1 - local_subband,
    ))]
    while local_subband < range.1 {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost before = precinct_plan_builder_view(&builder); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part2_planned_precinct_rows, 1); }
        let subband_index = range
            .0
            .checked_add(local_subband)
            .ok_or_else(arithmetic_overflow)?;
        let subband = *subbands.get(subband_index).ok_or_else(invalid_decode)?;
        let planned = plan_subband_execution(
            band,
            block_size,
            subband,
            style,
            policy,
            (location.3, subband_index, builder.decoder_index),
            builder.placements,
            builder.planned_code_blocks,
            selected,
            limits,
        )?;
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost row = planned_subband_execution_view(&planned); }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(precinct_plan_builder_view);
            reveal(planned_subband_execution_view);
        }
        builder.placements = planned.placements;
        builder.decoder_index = planned.decoder_index;
        builder.planned_code_blocks = planned.planned_code_blocks;
        if let Some(execution) = planned.execution {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost execution_view = crate::packet::packet_execution_view(&execution); }
            builder.executions.push(execution);
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(row.execution == Some(execution_view));
                assert(crate::packet::packet_executions_view(builder.executions@)
                    =~= before.executions.push(execution_view));
            }
        } else {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(row.execution.is_none());
                assert(crate::packet::packet_executions_view(builder.executions@) =~= before.executions);
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(precinct_plan_builder_view);
            reveal(planned_subband_execution_view);
            assert(crate::specs::part2_planned_precinct_subband(
                band, block_size, subbands@, range.0, local_subband, style, policy, location.3, selected, limits, before,
            ) == Ok(precinct_plan_builder_view(&builder)));
        }
        local_subband = local_subband
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal_with_fuel(crate::specs::part2_planned_precinct_rows, 1);
        reveal(precinct_plan_builder_view);
        reveal(planned_precinct_view);
    }
    let precinct = crate::packet::packet_precinct_from_subbands(
        location.0,
        location.1,
        location.2,
        style,
        builder.executions,
        limits,
    )?;
    Ok(PlannedPrecinct {
        precinct,
        placements: builder.placements,
        planned_code_blocks: builder.planned_code_blocks,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures planned_subband_lattice_outcome(result) == crate::specs::part2_planned_subband_lattice(band, block_size, subband),
))]
pub(crate) fn derive_planned_subband_lattice(
    band: [u32; 4],
    block_size: [u32; 2],
    subband: SubbandGeometry,
) -> Result<Option<PlannedSubbandLattice>, DecodeError> {
    let start_x = band[0].max(subband.x0);
    let start_y = band[1].max(subband.y0);
    let subband_x1 = subband
        .x0
        .checked_add(subband.width)
        .ok_or_else(arithmetic_overflow)?;
    let subband_y1 = subband
        .y0
        .checked_add(subband.height)
        .ok_or_else(arithmetic_overflow)?;
    if start_x >= subband_x1 || start_y >= subband_y1 {
        return Ok(None);
    }
    let end_x = band[0]
        .checked_add(band[2])
        .ok_or_else(arithmetic_overflow)?
        .min(subband_x1);
    let end_y = band[1]
        .checked_add(band[3])
        .ok_or_else(arithmetic_overflow)?
        .min(subband_y1);
    let first_block_x = floor_div_u32(start_x, block_size[0])?;
    let first_block_y = floor_div_u32(start_y, block_size[1])?;
    let blocks_across = ceil_div_u32(end_x, block_size[0])?
        .checked_sub(first_block_x)
        .ok_or_else(arithmetic_overflow)?;
    let blocks_down = ceil_div_u32(end_y, block_size[1])?
        .checked_sub(first_block_y)
        .ok_or_else(arithmetic_overflow)?;
    Ok(Some(PlannedSubbandLattice {
        grid: [first_block_x, first_block_y, block_size[0], block_size[1]],
        clip: [start_x, start_y, end_x, end_y],
        blocks: [blocks_across, blocks_down],
    }))
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures planned_subband_execution_outcome(result) == crate::specs::part2_planned_subband_execution(
        band, block_size, subband, style, policy, indices, placements@, planned_code_blocks, selected, limits,
    ),
))]
pub(crate) fn plan_subband_execution(
    band: [u32; 4],
    block_size: [u32; 2],
    subband: SubbandGeometry,
    style: Tier1CodingStyle,
    policy: (bool, u8),
    indices: (usize, usize, usize),
    placements: Vec<ProgressionCodeBlockPlacement>,
    planned_code_blocks: u64,
    selected: bool,
    limits: &ResourceLimits,
) -> Result<PlannedSubbandExecution, DecodeError> {
    let lattice = match derive_planned_subband_lattice(band, block_size, subband)? {
        None => {
            return Ok(PlannedSubbandExecution {
                execution: None,
                placements,
                decoder_index: indices.2,
                planned_code_blocks,
            });
        }
        Some(lattice) => lattice,
    };
    #[cfg(verus_keep_ghost)]
    proof! { reveal(planned_subband_lattice_view); }
    let planned = plan_codeblock_decoders(
        lattice.grid,
        lattice.clip,
        lattice.blocks,
        subband,
        style,
        policy,
        indices,
        placements,
        planned_code_blocks,
        selected,
        limits,
    )?;
    #[cfg(verus_keep_ghost)]
    proof! { reveal(planned_codeblocks_view); reveal(planned_subband_execution_view); }
    let execution = PacketSubbandExecution {
        orientation: subband.orientation,
        code_blocks_across: lattice.blocks[0],
        code_blocks_down: lattice.blocks[1],
        maximum_zero_bitplanes: subband.significant_bitplanes,
        decoders: planned.decoders,
    };
    #[cfg(verus_keep_ghost)]
    proof! { crate::packet::packet_execution_known_fields(&execution); }
    Ok(PlannedSubbandExecution {
        execution: Some(execution),
        placements: planned.placements,
        decoder_index: planned.decoder_index,
        planned_code_blocks: planned.planned_code_blocks,
    })
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures planned_codeblocks_outcome(result) == crate::specs::part2_planned_codeblock_cell(
        grid, clip, block, subband, style, policy, indices, planned_codeblocks_view(&state),
    ),
))]
pub(crate) fn append_planned_codeblock(
    grid: [u32; 4],
    clip: [u32; 4],
    block: [u32; 2],
    subband: SubbandGeometry,
    style: Tier1CodingStyle,
    policy: (bool, u8),
    indices: (usize, usize),
    mut state: PlannedCodeBlocks,
) -> Result<PlannedCodeBlocks, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost before = planned_codeblocks_view(&state); }
    let (x, y, width, height) = derive_planned_codeblock_geometry(grid, clip, block)?;
    let decoder = crate::tier1::tier1_configured_decoder(
        width,
        height,
        subband.significant_bitplanes,
        subband.orientation,
        style,
        policy.0,
        policy.1,
    )?;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost decoded = crate::tier1::tier1_decoder_state(&decoder); }
    state.decoders.push(decoder);
    state.placements.push(ProgressionCodeBlockPlacement {
        precinct_index: indices.0,
        decoder_index: state.decoder_index,
        subband_index: indices.1,
        x: x.checked_sub(subband.x0).ok_or_else(arithmetic_overflow)?,
        y: y.checked_sub(subband.y0).ok_or_else(arithmetic_overflow)?,
    });
    state.decoder_index = state
        .decoder_index
        .checked_add(1)
        .ok_or_else(arithmetic_overflow)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(planned_codeblocks_view);
        assert(state.decoders@.map_values(|decoder: Tier1CodeBlockDecoder| crate::tier1::tier1_decoder_state(&decoder))
            =~= before.decoders.push(decoded));
    }
    Ok(state)
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures planned_codeblocks_outcome(result) == crate::specs::part2_planned_codeblocks(
        grid, clip, blocks, subband, style, policy, indices, placements@, planned_code_blocks, selected, limits,
    ),
))]
pub(crate) fn plan_codeblock_decoders(
    grid: [u32; 4],
    clip: [u32; 4],
    blocks: [u32; 2],
    subband: SubbandGeometry,
    style: Tier1CodingStyle,
    policy: (bool, u8),
    indices: (usize, usize, usize),
    placements: Vec<ProgressionCodeBlockPlacement>,
    planned_code_blocks: u64,
    selected: bool,
    limits: &ResourceLimits,
) -> Result<PlannedCodeBlocks, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost goal = crate::specs::part2_planned_codeblocks(
        grid, clip, blocks, subband, style, policy, indices, placements@, planned_code_blocks, selected, limits,
    ); }
    let mut total = planned_code_blocks;
    let count = u64::from(blocks[0])
        .checked_mul(u64::from(blocks[1]))
        .ok_or_else(arithmetic_overflow)?;
    if selected {
        total = total.checked_add(count).ok_or_else(arithmetic_overflow)?;
        if total > limits.maximum_code_blocks() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::CodeBlocks,
                limit: limits.maximum_code_blocks(),
                actual: total,
            });
        }
        if total > limits.maximum_work_units() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: limits.maximum_work_units(),
                actual: total,
            });
        }
    }
    let capacity = match usize::try_from(count) {
        Ok(capacity) => capacity,
        Err(_) => return Err(arithmetic_overflow()),
    };
    let mut state = PlannedCodeBlocks {
        decoders: Vec::with_capacity(capacity),
        placements,
        decoder_index: indices.2,
        planned_code_blocks: total,
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(planned_codeblocks_view);
        assert(state.decoders@.map_values(|decoder: Tier1CodeBlockDecoder| crate::tier1::tier1_decoder_state(&decoder))
            =~= Seq::empty());
    }
    let mut block_y = 0_u32;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant block_y <= blocks[1],
            goal == crate::specs::part2_planned_codeblocks(
                grid, clip, blocks, subband, style, policy, indices, placements@, planned_code_blocks, selected, limits,
            ),
            goal == crate::specs::part2_planned_codeblock_rows(
                grid, clip, blocks, subband, style, policy, (indices.0, indices.1),
                block_y as nat, planned_codeblocks_view(&state),
            ),
        decreases blocks[1] - block_y,
    ))]
    while block_y < blocks[1] {
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part2_planned_codeblock_rows, 1); }
        let mut block_x = 0_u32;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant block_x <= blocks[0], block_y < blocks[1],
                goal == crate::specs::part2_planned_codeblocks(
                    grid, clip, blocks, subband, style, policy, indices, placements@, planned_code_blocks, selected, limits,
                ),
                goal == match crate::specs::part2_planned_codeblock_columns(
                    grid, clip, blocks[0], subband, style, policy, (indices.0, indices.1),
                    block_y, block_x as nat, planned_codeblocks_view(&state),
                ) {
                    Err(error) => Err(error),
                    Ok(after) => crate::specs::part2_planned_codeblock_rows(
                        grid, clip, blocks, subband, style, policy, (indices.0, indices.1),
                        block_y as nat + 1, after,
                    ),
                },
            decreases blocks[0] - block_x,
        ))]
        while block_x < blocks[0] {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part2_planned_codeblock_columns, 1); }
            state = append_planned_codeblock(
                grid,
                clip,
                [block_x, block_y],
                subband,
                style,
                policy,
                (indices.0, indices.1),
                state,
            )?;
            block_x = block_x.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part2_planned_codeblock_columns, 1); }
        block_y = block_y.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part2_planned_codeblock_rows, 1); }
    Ok(state)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_planned_codeblock_geometry(grid, clip, block),
))]
pub(crate) fn derive_planned_codeblock_geometry(
    grid: [u32; 4],
    clip: [u32; 4],
    block: [u32; 2],
) -> Result<(u32, u32, u16, u16), DecodeError> {
    let index_x = grid[0]
        .checked_add(block[0])
        .ok_or_else(arithmetic_overflow)?;
    let nominal_x = index_x
        .checked_mul(grid[2])
        .ok_or_else(arithmetic_overflow)?;
    let index_y = grid[1]
        .checked_add(block[1])
        .ok_or_else(arithmetic_overflow)?;
    let nominal_y = index_y
        .checked_mul(grid[3])
        .ok_or_else(arithmetic_overflow)?;
    let x = nominal_x.max(clip[0]);
    let y = nominal_y.max(clip[1]);
    let nominal_x1 = nominal_x
        .checked_add(grid[2])
        .ok_or_else(arithmetic_overflow)?;
    let nominal_y1 = nominal_y
        .checked_add(grid[3])
        .ok_or_else(arithmetic_overflow)?;
    let width = nominal_x1
        .min(clip[2])
        .checked_sub(x)
        .ok_or_else(arithmetic_overflow)?;
    let width = match u16::try_from(width) {
        Ok(width) => width,
        Err(_) => return Err(arithmetic_overflow()),
    };
    let height = nominal_y1
        .min(clip[3])
        .checked_sub(y)
        .ok_or_else(arithmetic_overflow)?;
    let height = match u16::try_from(height) {
        Ok(height) => height,
        Err(_) => return Err(arithmetic_overflow()),
    };
    Ok((x, y, width, height))
}

struct WorkingSubband {
    width: u32,
    height: u32,
    fraction_bits: u8,
    samples: Vec<i64>,
    visited: Vec<bool>,
}

struct PrepareWorking {
    bands: Vec<WorkingSubband>,
    bytes: u64,
}

struct PrepareFold {
    width: u32,
    height: u32,
    samples: Vec<i64>,
    first_detail: usize,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn prepare_plan_view(plan: &PlannedTileComponent) -> crate::specs::Part1PreparePlan {
        crate::specs::Part1PreparePlan {
            component: plan.component, x: plan.x, y: plan.y, width: plan.width, height: plan.height,
            resolution: plan.output_resolution, subbands: plan.subbands@, placements: plan.placements@,
        }
    }

    pub closed spec fn prepare_style_view(style: CodingStyle) -> crate::specs::Part1PrepareStyle {
        let selector = crate::codestream::decomposition_selector_view(style);
        (style.transform, selector.0, selector.1)
    }

    pub open spec fn prepare_precincts_view(precincts: Seq<PacketPrecinctDecoder>) -> Seq<Seq<crate::specs::Tier1RawPlane>> {
        Seq::new(precincts.len(), |i: int| {
            let decoders = crate::packet::packet_precinct_decoders(&precincts[i]);
            Seq::new(decoders.len(), |j: int| crate::tier1::tier1_decoder_plane_view(&decoders[j]))
        })
    }

    pub proof fn planning_precinct_planes_known_fields(values: Seq<PacketPrecinctDecoder>)
        ensures prepare_precincts_view(values) == crate::specs::part2_precinct_decoder_planes(planning_precincts_view(values)),
    {
        assert forall |i: int| 0 <= i < values.len() implies
            prepare_precincts_view(values)[i]
                == crate::specs::part2_precinct_decoder_planes(planning_precincts_view(values))[i] by {
            crate::packet::packet_precinct_decoder_states(&values[i]);
            let decoders = crate::packet::packet_precinct_decoders(&values[i]);
            assert forall |j: int| 0 <= j < decoders.len() implies
                crate::tier1::tier1_decoder_plane_view(&decoders[j])
                    == crate::tier1::tier1_decoder_state(&decoders[j]).plane by {
                crate::tier1::tier1_decoder_plane_matches_state(&decoders[j]);
            }
            assert(prepare_precincts_view(values)[i]
                =~= crate::specs::part2_precinct_decoder_planes(planning_precincts_view(values))[i]);
        }
        assert(prepare_precincts_view(values) =~= crate::specs::part2_precinct_decoder_planes(planning_precincts_view(values)));
    }

    pub closed spec fn decoded_tile_components_view(value: &DecodedTileComponents) -> crate::specs::Part2DecodedTileComponents {
        crate::specs::Part2DecodedTileComponents { prepared: prepared_components_view(value.prepared@), mct_work: value.mct_work }
    }

    pub open spec fn decoded_tile_components_outcome(result: Result<DecodedTileComponents, DecodeError>)
        -> Result<crate::specs::Part2DecodedTileComponents, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(value) => Ok(decoded_tile_components_view(&value)) }
    }

    pub closed spec fn prepared_component_view(value: PreparedTileComponent) -> crate::specs::Part1AssemblyTile {
        (value.component, value.x, value.y, value.width, value.height, value.fraction_bits, value.samples@)
    }

    pub open spec fn prepared_components_view(values: Seq<PreparedTileComponent>) -> Seq<crate::specs::Part1AssemblyTile> {
        Seq::new(values.len(), |index: int| prepared_component_view(values[index]))
    }

    pub open spec fn tile_mct_option(value: Option<&codestream::MultipleComponentTransform>) -> Option<Seq<crate::specs::Part2MctStage>> {
        match value { None => None, Some(transform) => Some(codestream::multiple_component_transform_view(transform)) }
    }

    pub open spec fn tile_part_mct_view(first: &TilePartPayload) -> Option<Seq<crate::specs::Part2MctStage>> {
        match first.multiple_component_transform {
            None => None,
            Some(transform) => Some(codestream::multiple_component_transform_view(&transform)),
        }
    }

    pub closed spec fn planned_components_view(plan: &ProgressionTilePlan) -> Seq<crate::specs::Part1PreparePlan> {
        Seq::new(plan.components@.len(), |index: int| prepare_plan_view(&plan.components@[index]))
    }

    pub closed spec fn progression_reconstruction_view(plan: &ProgressionTilePlan)
        -> (Seq<crate::specs::Part1PreparePlan>, Seq<Seq<crate::specs::Tier1RawPlane>>, ComponentCodingStyles, CodingStyle) {
        (planned_components_view(plan), prepare_precincts_view(plan.precincts@), plan.component_coding_styles, plan.coding_style)
    }

    spec fn prepared_tiles_outcome(result: Result<Vec<TileComponent<'_>>, DecodeError>)
        -> Result<Seq<crate::specs::Part1AssemblyTile>, DecodeError> {
        match result { Ok(tiles) => Ok(crate::image::assembly_tiles_view(tiles@)), Err(error) => Err(error) }
    }

    pub open spec fn prepared_component_outcome(result: Result<PreparedTileComponent, DecodeError>)
        -> Result<crate::specs::Part1AssemblyTile, DecodeError>
    {
        match result { Ok(value) => Ok(prepared_component_view(value)), Err(error) => Err(error) }
    }

    closed spec fn working_subband_view(value: WorkingSubband) -> crate::specs::Part1WorkingSubbandModel {
        (value.width, value.height, value.fraction_bits, value.samples@, value.visited@)
    }

    impl View for WorkingSubband {
        type V = crate::specs::Part1WorkingSubbandModel;

        closed spec fn view(&self) -> Self::V {
            working_subband_view(*self)
        }
    }

        closed spec fn working_bands_view(values: Seq<WorkingSubband>) -> Seq<crate::specs::Part1WorkingSubbandModel> {
            Seq::new(values.len(), |i: int| working_subband_view(values[i]))
        }

        impl View for PrepareWorking {
            type V = crate::specs::Part1PrepareWorking;
            closed spec fn view(&self) -> Self::V { (working_bands_view(self.bands@), self.bytes) }
        }

        closed spec fn prepare_working_outcome(result: Result<PrepareWorking, DecodeError>)
            -> Result<crate::specs::Part1PrepareWorking, DecodeError>
        {
            match result { Ok(value) => Ok(value@), Err(error) => Err(error) }
        }

        impl View for PrepareFold {
            type V = crate::specs::Part1PrepareFold;
            closed spec fn view(&self) -> Self::V { (self.width, self.height, self.samples@, self.first_detail) }
        }

        closed spec fn prepare_fold_outcome(result: Result<PrepareFold, DecodeError>)
            -> Result<crate::specs::Part1PrepareFold, DecodeError>
        {
            match result { Ok(value) => Ok(value@), Err(error) => Err(error) }
        }

        closed spec fn prepare_cell_outcome(result: Result<SignedMagnitude, DecodeError>)
            -> Result<crate::specs::Part1QuantizedCoefficient, DecodeError>
        {
            match result { Ok(value) => Ok((crate::magnitude::magnitude_limbs(value.magnitude), value.negative)), Err(error) => Err(error) }
        }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(destination)@) == crate::specs::part1_quantized_block_pixel(
        old(destination)@, source@, block_width, placement_x, placement_y, x, y,
    ),
))]
fn write_quantized_pixel(
    destination: &mut WorkingSubband,
    source: &[i64],
    block_width: u32,
    placement_x: u32,
    placement_y: u32,
    x: u32,
    y: u32,
) -> Result<(), DecodeError> {
    let destination_index = checked_index(
        destination.width,
        placement_x.checked_add(x).ok_or_else(arithmetic_overflow)?,
        placement_y.checked_add(y).ok_or_else(arithmetic_overflow)?,
    )?;
    let source_index = checked_index(block_width, x, y)?;
    if destination_index >= destination.samples.len()
        || destination_index >= destination.visited.len()
        || source_index >= source.len()
        || destination.visited[destination_index]
    {
        return Err(invalid_decode());
    }
    destination.samples[destination_index] =
        *source.get(source_index).ok_or_else(invalid_decode)?;
    destination.visited[destination_index] = true;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(destination)@) == crate::specs::part1_quantized_block_row(
        old(destination)@, source@, block_width, placement_x, placement_y, y, block_width as nat,
    ),
))]
fn write_quantized_row(
    destination: &mut WorkingSubband,
    source: &[i64],
    block_width: u32,
    placement_x: u32,
    placement_y: u32,
    y: u32,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original = destination@; }
    let mut x = 0_u32;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant x <= block_width,
        original == old(destination)@,
        crate::specs::part1_quantized_block_row(
            original, source@, block_width, placement_x, placement_y, y, x as nat,
        ) == (Ok(()), destination@),
        decreases block_width - x,
    ))]
    while x < block_width {
        match write_quantized_pixel(
            destination,
            source,
            block_width,
            placement_x,
            placement_y,
            x,
            y,
        ) {
            Ok(()) => {}
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_quantized_row_error_persists(
                        original, source@, block_width, placement_x, placement_y, y,
                        x as nat + 1, block_width as nat,
                    );
                }
                return Err(error);
            }
        }
        x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(destination)@) == crate::specs::part1_quantized_block_rows(
        old(destination)@, source@, block_width, placement_x, placement_y, block_height as nat,
    ),
))]
fn write_quantized_block(
    destination: &mut WorkingSubband,
    source: &[i64],
    block_width: u32,
    block_height: u32,
    placement_x: u32,
    placement_y: u32,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original = destination@; }
    let mut y = 0_u32;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant y <= block_height,
        original == old(destination)@,
        crate::specs::part1_quantized_block_rows(
            original, source@, block_width, placement_x, placement_y, y as nat,
        ) == (Ok(()), destination@),
        decreases block_height - y,
    ))]
    while y < block_height {
        match write_quantized_row(
            destination,
            source,
            block_width,
            placement_x,
            placement_y,
            y,
        ) {
            Ok(()) => {}
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_quantized_block_error_persists(
                        original, source@, block_width, placement_x, placement_y,
                        y as nat + 1, block_height as nat,
                    );
                }
                return Err(error);
            }
        }
        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_make_subband_geometry(
        resolution, orientation, bounds, resolution_dimensions, low_pass_first, component_depth,
        codestream::quantization_view(quantization), quantization_index,
    ),
))]
pub(crate) fn make_subband_geometry(
    resolution: u8,
    orientation: SubbandOrientation,
    bounds: [u32; 4],
    resolution_dimensions: [u32; 2],
    low_pass_first: [bool; 2],
    component_depth: [u8; 2],
    quantization: &Quantization,
    quantization_index: u16,
) -> Result<SubbandGeometry, DecodeError> {
    let precision = component_depth[0];
    let roi_shift = component_depth[1];
    let gain = match orientation {
        SubbandOrientation::Ll => 0_u8,
        SubbandOrientation::Hl | SubbandOrientation::Lh => 1_u8,
        SubbandOrientation::Hh => 2_u8,
    };
    let dynamic_range = precision
        .checked_add(gain)
        .ok_or_else(arithmetic_overflow)?;
    if dynamic_range == 0 {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidQuantization,
        });
    }
    let quantization_step =
        quantization_step_at_index(quantization, resolution, quantization_index)?;
    let significant_bitplanes = u16::from(quantization.guard_bits())
        .checked_add(u16::from(quantization_step.exponent))
        .ok_or_else(invalid_decode)?;
    let significant_bitplanes = significant_bitplanes
        .checked_sub(1)
        .ok_or_else(invalid_decode)?;
    let significant_bitplanes = significant_bitplanes
        .checked_add(u16::from(roi_shift))
        .ok_or_else(invalid_decode)?;
    if significant_bitplanes == 0 || significant_bitplanes > crate::tier1::MAX_TIER1_BITPLANES {
        return Err(invalid_decode());
    }
    Ok(SubbandGeometry {
        resolution,
        orientation,
        x0: bounds[0],
        y0: bounds[1],
        width: bounds[2]
            .checked_sub(bounds[0])
            .ok_or_else(arithmetic_overflow)?,
        height: bounds[3]
            .checked_sub(bounds[1])
            .ok_or_else(arithmetic_overflow)?,
        resolution_width: resolution_dimensions[0],
        resolution_height: resolution_dimensions[1],
        quantization_index,
        quantization_step,
        dynamic_range,
        significant_bitplanes,
        roi_shift,
        low_pass_first_x: low_pass_first[0],
        low_pass_first_y: low_pass_first[1],
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures prepared_component_outcome(result) == crate::specs::part1_prepare_standard_component(
        prepare_plan_view(plan), prepare_precincts_view(precincts@), prepare_style_view(coding_style),
        crate::limits::resource_decoded_bytes_limit(limits),
    ),
    prepared_component_outcome(result) == crate::specs::part2_prepare_component(
        prepare_plan_view(plan), prepare_precincts_view(precincts@), prepare_style_view(coding_style), None,
        crate::limits::resource_decoded_bytes_limit(limits)),
))]
pub fn prepare_tile_component(
    plan: &PlannedTileComponent,
    precincts: &[PacketPrecinctDecoder],
    coding_style: CodingStyle,
    limits: &ResourceLimits,
) -> Result<PreparedTileComponent, DecodeError> {
    prepare_tile_component_with_kernel(plan, precincts, coding_style, None, limits)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures prepare_working_outcome(result) == crate::specs::part1_prepare_init_prefix(bands@, fraction, crate::limits::resource_decoded_bytes_limit(limits), selected as nat),
    result.is_ok() ==> result->Ok_0.bands.len() == selected,
))]
fn initialize_prepare_working(
    bands: &[SubbandGeometry],
    selected: usize,
    fraction: u8,
    limits: &ResourceLimits,
) -> Result<PrepareWorking, DecodeError> {
    let mut working = PrepareWorking {
        bands: Vec::with_capacity(selected),
        bytes: 0,
    };
    let mut index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_prepare_init_empty(bands@, fraction, crate::limits::resource_decoded_bytes_limit(limits));
        assert(working_bands_view(working.bands@) =~= Seq::empty());
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= selected, working.bands.len() == index,
        crate::specs::part1_prepare_init_prefix(bands@, fraction, crate::limits::resource_decoded_bytes_limit(limits), index as nat) == Ok(working@),
        decreases selected - index,
    ))]
    while index < selected {
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_prepare_init_step(bands@, fraction, crate::limits::resource_decoded_bytes_limit(limits), index as nat); }
        let operation = append_prepare_subband(&mut working, bands, index, fraction, limits);
        match operation {
            Ok(()) => {}
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! { crate::proofs::production_impl::production_prepare_init_error(
                    bands@, fraction, crate::limits::resource_decoded_bytes_limit(limits), index as nat + 1, selected as nat, error);
                }
                return Err(error);
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(operation.is_ok());
            assert(operation->Ok_0 == ());
            assert(operation == Ok(()));
        }
        index += 1;
    }
    Ok(working)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    requires old(working).bands.len() < usize::MAX,
    ensures (result, final(working)@) == crate::specs::part1_prepare_append_band(old(working)@, bands@, index, fraction, crate::limits::resource_decoded_bytes_limit(limits)),
    result.is_ok() ==> final(working).bands.len() == old(working).bands.len() + 1,
))]
fn append_prepare_subband(
    working: &mut PrepareWorking,
    bands: &[SubbandGeometry],
    index: usize,
    fraction: u8,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let geometry = *bands.get(index).ok_or_else(invalid_decode)?;
    let length = checked_area_u32(geometry.width, geometry.height)?;
    let bytes = (length as u64)
        .checked_mul(9)
        .ok_or_else(arithmetic_overflow)?;
    working.bytes = working
        .bytes
        .checked_add(bytes)
        .ok_or_else(arithmetic_overflow)?;
    if working.bytes > limits.maximum_decoded_bytes() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::DecodedBytes,
            limit: limits.maximum_decoded_bytes(),
            actual: working.bytes,
        });
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost before = working.bands@; }
    working.bands.push(WorkingSubband {
        width: geometry.width,
        height: geometry.height,
        fraction_bits: fraction,
        samples: vec![0_i64; length],
        visited: vec![false; length],
    });
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(working.bands@[working.bands.len() - 1].samples@ =~= Seq::new(length as nat, |i: int| 0i64));
        assert(working.bands@[working.bands.len() - 1].visited@ =~= Seq::new(length as nat, |i: int| false));
        assert(working_bands_view(working.bands@) =~= working_bands_view(before).push(working_subband_view(working.bands@[working.bands.len() - 1])));
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures crate::transform::quantized_coefficients_outcome(result) == crate::specs::part1_prepare_collect_coefficients(crate::tier1::code_block_plane_view(plane)),
))]
fn collect_prepare_coefficients(
    plane: &CodeBlockPlane,
) -> Result<Vec<SignedMagnitude>, DecodeError> {
    let count = usize::from(plane.width())
        .checked_mul(usize::from(plane.height()))
        .ok_or_else(arithmetic_overflow)?;
    let mut coefficients = Vec::with_capacity(count);
    let height = plane.height();
    let width = plane.width();
    let mut y = 0_u16;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::transform::representation_proofs::quantized_coefficients_fields(coefficients@);
        crate::proofs::production_impl::production_prepare_rows_empty(crate::tier1::code_block_plane_view(plane));
        assert(crate::transform::quantized_coefficients_view(coefficients@) =~= Seq::empty());
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant y <= height,
        width == crate::tier1::code_block_plane_view(plane).width,
        height == crate::tier1::code_block_plane_view(plane).height,
        count as nat == width as nat * height as nat,
        coefficients.len() as nat == y as nat * width as nat,
        crate::specs::part1_prepare_coefficient_rows(crate::tier1::code_block_plane_view(plane), y as nat) == Ok(crate::transform::quantized_coefficients_view(coefficients@)),
        decreases height - y,
    ))]
    while y < height {
        #[cfg(verus_keep_ghost)]
        proof! { crate::transform::representation_proofs::quantized_coefficients_fields(coefficients@); }
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost row_start = crate::transform::quantized_coefficients_view(coefficients@); }
        let mut x = 0_u16;
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_prepare_row_empty(crate::tier1::code_block_plane_view(plane), y, row_start); }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant x <= width, y < height,
            width == crate::tier1::code_block_plane_view(plane).width,
            height == crate::tier1::code_block_plane_view(plane).height,
            count as nat == width as nat * height as nat,
            row_start.len() == y as nat * width as nat,
            coefficients.len() as nat == y as nat * width as nat + x as nat,
            crate::specs::part1_prepare_coefficient_rows(crate::tier1::code_block_plane_view(plane), y as nat) == Ok(row_start),
            crate::specs::part1_prepare_coefficient_row(crate::tier1::code_block_plane_view(plane), y, row_start, x as nat) == Ok(crate::transform::quantized_coefficients_view(coefficients@)),
            decreases width - x,
        ))]
        while x < width {
            #[cfg(verus_keep_ghost)]
            proof! { crate::proofs::production_impl::production_prepare_row_step(crate::tier1::code_block_plane_view(plane), y, row_start, x as nat); }
            let value = match collect_prepare_cell(plane, x, y) {
                Ok(value) => value,
                Err(error) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        crate::proofs::production_impl::production_prepare_row_error(crate::tier1::code_block_plane_view(plane), y, row_start, x as nat + 1, width as nat, error);
                        crate::proofs::production_impl::production_prepare_rows_step(crate::tier1::code_block_plane_view(plane), y as nat);
                        crate::proofs::production_impl::production_prepare_rows_error(crate::tier1::code_block_plane_view(plane), y as nat + 1, height as nat, error);
                    }
                    return Err(error);
                }
            };
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before_values = coefficients@; }
            coefficients.push(value);
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::transform::representation_proofs::quantized_coefficients_fields(coefficients@);
                crate::transform::representation_proofs::quantized_coefficients_push(before_values, value);
            }
            x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_prepare_rows_step(crate::tier1::code_block_plane_view(plane), y as nat);
            assert(y as nat * width as nat + width as nat == (y as nat + 1) * width as nat) by(nonlinear_arith);
        }
        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(coefficients)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures prepare_cell_outcome(result) == crate::specs::part1_prepare_signed_cell(crate::tier1::code_block_plane_view(plane), x, y),
))]
fn collect_prepare_cell(
    plane: &CodeBlockPlane,
    x: u16,
    y: u16,
) -> Result<SignedMagnitude, DecodeError> {
    let coefficient = plane.coefficient(x, y)?;
    coefficient.signed_magnitude()
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(working)@) == crate::specs::part1_prepare_placement(
        old(working)@, bands@, prepare_precincts_view(precincts@), placement, reversible, crate::limits::resource_decoded_bytes_limit(limits),
    ),
    final(working).bands.len() == old(working).bands.len(),
))]
fn place_prepare_block(
    working: &mut PrepareWorking,
    bands: &[SubbandGeometry],
    precincts: &[PacketPrecinctDecoder],
    placement: ProgressionCodeBlockPlacement,
    reversible: bool,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_prepare_placement); }
    if placement.subband_index >= working.bands.len() {
        return Ok(());
    }
    let geometry = *bands
        .get(placement.subband_index)
        .ok_or_else(invalid_decode)?;
    let precinct = precincts
        .get(placement.precinct_index)
        .ok_or_else(invalid_decode)?;
    let decoder = precinct.decoder(placement.decoder_index)?;
    let plane = decoder.plane();
    let width = u32::from(plane.width());
    let height = u32::from(plane.height());
    if placement
        .x
        .checked_add(width)
        .ok_or_else(arithmetic_overflow)?
        > geometry.width
        || placement
            .y
            .checked_add(height)
            .ok_or_else(arithmetic_overflow)?
            > geometry.height
    {
        return Err(invalid_decode());
    }
    let coefficients = collect_prepare_coefficients(plane)?;
    let quantized = inverse_quantize_magnitudes(
        &coefficients,
        geometry.quantization_step,
        geometry.dynamic_range,
        reversible,
        geometry.roi_shift,
        limits,
    )?;
    if quantized.fraction_bits() != if reversible { 0 } else { 16 } {
        return Err(invalid_decode());
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost before = working.bands@; }
    let destination = working
        .bands
        .get_mut(placement.subband_index)
        .ok_or_else(invalid_decode)?;
    let result = write_quantized_block(
        destination,
        quantized.samples(),
        width,
        height,
        placement.x,
        placement.y,
    );
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(working_bands_view(working.bands@) =~= working_bands_view(before).update(
            placement.subband_index as int, working_subband_view(working.bands@[placement.subband_index as int])));
    }
    result
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_prepare_coverage(working_bands_view(bands@)),
))]
fn validate_prepare_coverage(bands: &[WorkingSubband]) -> Result<(), DecodeError> {
    let mut band = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant band <= bands.len(),
        forall |b: int, i: int| 0 <= b < band && 0 <= i < bands@[b].visited.len() ==> bands@[b].visited@[i],
        decreases bands.len() - band,
    ))]
    while band < bands.len() {
        let subband = &bands[band];
        let mut sample = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant band < bands.len(), sample <= subband.visited.len(),
            subband == &bands@[band as int],
            forall |b: int, i: int| 0 <= b < band && 0 <= i < bands@[b].visited.len() ==> bands@[b].visited@[i],
            forall |i: int| 0 <= i < sample ==> subband.visited@[i],
            decreases subband.visited.len() - sample,
        ))]
        while sample < subband.visited.len() {
            if !subband.visited[sample] {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(working_bands_view(bands@)[band as int].4 == subband.visited@);
                    assert(!working_bands_view(bands@)[band as int].4[sample as int]);
                }
                return Err(invalid_decode());
            }
            sample += 1;
        }
        band += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures prepared_component_outcome(result) == crate::specs::part2_prepare_component(
        prepare_plan_view(plan), prepare_precincts_view(precincts@), prepare_style_view(coding_style),
        crate::transform::arbitrary_kernel_option(kernel), crate::limits::resource_decoded_bytes_limit(limits),
    ),
    kernel.is_none() ==> prepared_component_outcome(result) == crate::specs::part1_prepare_standard_component(
        prepare_plan_view(plan), prepare_precincts_view(precincts@), prepare_style_view(coding_style),
        crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
fn prepare_tile_component_with_kernel(
    plan: &PlannedTileComponent,
    precincts: &[PacketPrecinctDecoder],
    coding_style: CodingStyle,
    kernel: Option<&ArbitraryKernel>,
    limits: &ResourceLimits,
) -> Result<PreparedTileComponent, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part1_prepare_standard_component);
        reveal(crate::specs::part2_prepare_component);
    }
    if plan.subbands.is_empty() {
        return Err(invalid_decode());
    }
    let reversible = match coding_style.transform {
        WaveletTransform::Reversible5x3 => true,
        WaveletTransform::Irreversible9x7 => false,
        WaveletTransform::Arbitrary(_) => kernel.ok_or_else(invalid_decode)?.reversible(),
    };
    let expected_fraction_bits = if reversible { 0_u8 } else { 16_u8 };
    let selected_subbands =
        coding_style.subband_count_through_resolution(plan.output_resolution)?;
    if selected_subbands > plan.subbands.len() {
        return Err(invalid_decode());
    }
    let mut working = initialize_prepare_working(
        &plan.subbands,
        selected_subbands,
        expected_fraction_bits,
        limits,
    )?;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost initial_working = working@; }

    let mut placement_index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_prepare_placements_empty(
            initial_working, prepare_plan_view(plan), prepare_precincts_view(precincts@), reversible,
            crate::limits::resource_decoded_bytes_limit(limits),
        );
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant placement_index <= plan.placements.len(),
            working.bands.len() == selected_subbands,
            plan.subbands.len() > 0,
            selected_subbands <= plan.subbands.len(),
            expected_fraction_bits == crate::specs::part1_prepare_fraction(reversible),
            crate::specs::part1_subband_count_through_resolution(
                prepare_style_view(coding_style).1, prepare_style_view(coding_style).2, plan.output_resolution,
            ) == Ok(selected_subbands),
            crate::specs::part1_prepare_init_prefix(plan.subbands@, expected_fraction_bits, crate::limits::resource_decoded_bytes_limit(limits), selected_subbands as nat) == Ok(initial_working),
            kernel.is_none() ==> !matches!(coding_style.transform, WaveletTransform::Arbitrary(_)),
            kernel.is_none() ==> reversible == matches!(coding_style.transform, WaveletTransform::Reversible5x3),
            crate::specs::part2_prepare_reversible(prepare_style_view(coding_style), crate::transform::arbitrary_kernel_option(kernel)) == Ok(reversible),
            crate::specs::part1_prepare_placements(
                initial_working, prepare_plan_view(plan), prepare_precincts_view(precincts@), reversible,
                crate::limits::resource_decoded_bytes_limit(limits), placement_index as nat,
            ) == (Ok(()), working@),
            decreases plan.placements.len() - placement_index,
        )
    )]
    while placement_index < plan.placements.len() {
        let placement = *plan
            .placements
            .get(placement_index)
            .ok_or_else(invalid_decode)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_prepare_placements_step(
                initial_working, prepare_plan_view(plan), prepare_precincts_view(precincts@), reversible,
                crate::limits::resource_decoded_bytes_limit(limits), placement_index as nat,
            );
        }
        let operation = place_prepare_block(
            &mut working,
            &plan.subbands,
            precincts,
            placement,
            reversible,
            limits,
        );
        match operation {
            Ok(()) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(operation.is_ok());
                    assert(operation->Ok_0 == ());
                    assert(operation == Ok(()));
                }
            }
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_prepare_placements_error(
                        initial_working, prepare_plan_view(plan), prepare_precincts_view(precincts@), reversible,
                        crate::limits::resource_decoded_bytes_limit(limits), placement_index as nat + 1, plan.placements.len() as nat,
                    );
                    assert(crate::specs::part1_prepare_placements(
                        initial_working, prepare_plan_view(plan), prepare_precincts_view(precincts@), reversible,
                        crate::limits::resource_decoded_bytes_limit(limits), plan.placements.len() as nat,
                    ).0 == Err(error));
                    reveal(crate::specs::part1_prepare_standard_component);
                    reveal(crate::specs::part2_prepare_component);
                    assert(crate::specs::part2_prepare_component(
                        prepare_plan_view(plan), prepare_precincts_view(precincts@), prepare_style_view(coding_style),
                        crate::transform::arbitrary_kernel_option(kernel), crate::limits::resource_decoded_bytes_limit(limits),
                    ) == Err(error));
                    assert(kernel.is_none() ==> crate::specs::part1_prepare_standard_component(
                        prepare_plan_view(plan), prepare_precincts_view(precincts@), prepare_style_view(coding_style),
                        crate::limits::resource_decoded_bytes_limit(limits),
                    ) == Err(error));
                }
                return Err(error);
            }
        }
        placement_index = placement_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    validate_prepare_coverage(&working.bands)?;

    let folded = reconstruct_prepare_working(
        &working.bands,
        plan,
        coding_style,
        reversible,
        kernel,
        limits,
    )?;
    finish_prepared_component(plan, folded, expected_fraction_bits)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures prepare_fold_outcome(result) == crate::specs::part2_prepare_reconstruct(
        working_bands_view(working@), prepare_plan_view(plan), prepare_style_view(coding_style), reversible,
        crate::transform::arbitrary_kernel_option(kernel), crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
fn reconstruct_prepare_working(
    working: &[WorkingSubband],
    plan: &PlannedTileComponent,
    coding_style: CodingStyle,
    reversible: bool,
    kernel: Option<&ArbitraryKernel>,
    limits: &ResourceLimits,
) -> Result<PrepareFold, DecodeError> {
    let ll = working.first().ok_or_else(invalid_decode)?;
    let mut folded = PrepareFold {
        width: ll.width,
        height: ll.height,
        samples: ll.samples.clone(),
        first_detail: 1,
    };
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost initial = folded@; }
    let mut resolution = 1_u8;
    #[cfg(verus_keep_ghost)]
    proof! { crate::proofs::production_impl::production_prepare_kernel_fold_empty(initial, working_bands_view(working@), plan.subbands@, prepare_style_view(coding_style), reversible, crate::transform::arbitrary_kernel_option(kernel), crate::limits::resource_decoded_bytes_limit(limits)); }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant 1 <= resolution as int <= plan.output_resolution as int + 1,
        working.len() > 0, ll == &working@[0],
        initial == (ll.width, ll.height, ll.samples@, 1usize),
        crate::specs::part2_prepare_fold(
            initial, working_bands_view(working@), plan.subbands@, prepare_style_view(coding_style), reversible,
            crate::transform::arbitrary_kernel_option(kernel), crate::limits::resource_decoded_bytes_limit(limits), (resolution - 1) as nat,
        ) == Ok(folded@),
        decreases plan.output_resolution as int + 1 - resolution as int,
    ))]
    while resolution <= plan.output_resolution {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_prepare_kernel_fold_step(initial, working_bands_view(working@), plan.subbands@, prepare_style_view(coding_style), reversible, crate::transform::arbitrary_kernel_option(kernel), crate::limits::resource_decoded_bytes_limit(limits), (resolution - 1) as nat);
            crate::proofs::production_impl::production_prepare_kernel_fold_error_guard(initial, working_bands_view(working@), plan.subbands@, prepare_style_view(coding_style), reversible, crate::transform::arbitrary_kernel_option(kernel), crate::limits::resource_decoded_bytes_limit(limits), resolution as nat, plan.output_resolution as nat);
        }
        let direction = coding_style.direction_for_resolution(resolution)?;
        if matches!(direction, crate::codestream::DecompositionDirection::None) {
            resolution = resolution.checked_add(1).ok_or_else(arithmetic_overflow)?;
            continue;
        }
        folded = reconstruct_prepare_resolution(
            working, plan, folded, direction, reversible, kernel, limits,
        )?;
        if resolution == u8::MAX {
            return Err(arithmetic_overflow());
        }
        let modeled_resolution = crate::refinement::wavelet_next_resolution(resolution);
        resolution = resolution.checked_add(1).ok_or_else(arithmetic_overflow)?;
        if resolution != modeled_resolution {
            return Err(arithmetic_overflow());
        }
    }
    Ok(folded)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures prepare_fold_outcome(result) == crate::specs::part2_prepare_resolution(
        before@, working_bands_view(working@), plan.subbands@, direction, reversible,
        crate::transform::arbitrary_kernel_option(kernel), crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
fn reconstruct_prepare_resolution(
    working: &[WorkingSubband],
    plan: &PlannedTileComponent,
    before: PrepareFold,
    direction: crate::codestream::DecompositionDirection,
    reversible: bool,
    kernel: Option<&ArbitraryKernel>,
    limits: &ResourceLimits,
) -> Result<PrepareFold, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part2_prepare_resolution);
        reveal(crate::specs::part2_kernel_inverse_horizontal);
        reveal(crate::specs::part2_kernel_inverse_vertical);
        reveal(crate::specs::part2_kernel_inverse_level);
    }
    let mut first_detail = before.first_detail;
    let reconstructed_width = before.width;
    let reconstructed_height = before.height;
    let reconstructed = before.samples;
    let expected_fraction_bits = if reversible { 0 } else { 16 };
    let detail = working.get(first_detail).ok_or_else(invalid_decode)?;
    if detail.fraction_bits != expected_fraction_bits {
        return Err(invalid_decode());
    }
    let geometry = *plan.subbands.get(first_detail).ok_or_else(invalid_decode)?;
    let low = WaveletBand {
        width: reconstructed_width,
        height: reconstructed_height,
        samples: &reconstructed,
    };
    let plane = match direction {
        crate::codestream::DecompositionDirection::Horizontal => {
            first_detail = first_detail
                .checked_add(crate::refinement::wavelet_detail_advance(1))
                .ok_or_else(arithmetic_overflow)?;
            let high = WaveletBand {
                width: detail.width,
                height: detail.height,
                samples: &detail.samples,
            };
            if let Some(kernel) = kernel {
                inverse_horizontal_arbitrary(
                    low,
                    high,
                    geometry.resolution_width,
                    geometry.low_pass_first_x,
                    kernel,
                    limits,
                )?
            } else if reversible {
                inverse_horizontal_5_3(
                    low,
                    high,
                    geometry.resolution_width,
                    geometry.low_pass_first_x,
                    limits,
                )?
            } else {
                inverse_horizontal_9_7(
                    low,
                    high,
                    geometry.resolution_width,
                    geometry.low_pass_first_x,
                    limits,
                )?
            }
        }
        crate::codestream::DecompositionDirection::Vertical => {
            first_detail = first_detail
                .checked_add(crate::refinement::wavelet_detail_advance(2))
                .ok_or_else(arithmetic_overflow)?;
            let high = WaveletBand {
                width: detail.width,
                height: detail.height,
                samples: &detail.samples,
            };
            if let Some(kernel) = kernel {
                inverse_vertical_arbitrary(
                    low,
                    high,
                    geometry.resolution_height,
                    geometry.low_pass_first_y,
                    kernel,
                    limits,
                )?
            } else if reversible {
                inverse_vertical_5_3(
                    low,
                    high,
                    geometry.resolution_height,
                    geometry.low_pass_first_y,
                    limits,
                )?
            } else {
                inverse_vertical_9_7(
                    low,
                    high,
                    geometry.resolution_height,
                    geometry.low_pass_first_y,
                    limits,
                )?
            }
        }
        crate::codestream::DecompositionDirection::TwoDimensional => {
            let lh = working
                .get(
                    first_detail
                        .checked_add(1)
                        .ok_or_else(arithmetic_overflow)?,
                )
                .ok_or_else(invalid_decode)?;
            let hh = working
                .get(
                    first_detail
                        .checked_add(2)
                        .ok_or_else(arithmetic_overflow)?,
                )
                .ok_or_else(invalid_decode)?;
            if lh.fraction_bits != expected_fraction_bits
                || hh.fraction_bits != expected_fraction_bits
            {
                return Err(invalid_decode());
            }
            first_detail = first_detail
                .checked_add(crate::refinement::wavelet_detail_advance(3))
                .ok_or_else(arithmetic_overflow)?;
            let level = WaveletLevel {
                ll: low,
                hl: WaveletBand {
                    width: detail.width,
                    height: detail.height,
                    samples: &detail.samples,
                },
                lh: WaveletBand {
                    width: lh.width,
                    height: lh.height,
                    samples: &lh.samples,
                },
                hh: WaveletBand {
                    width: hh.width,
                    height: hh.height,
                    samples: &hh.samples,
                },
                output_width: geometry.resolution_width,
                output_height: geometry.resolution_height,
                low_pass_first_x: geometry.low_pass_first_x,
                low_pass_first_y: geometry.low_pass_first_y,
            };
            if let Some(kernel) = kernel {
                inverse_level_arbitrary(level, kernel, limits)?
            } else if reversible {
                inverse_level_5_3(level, limits)?
            } else {
                inverse_level_9_7(level, limits)?
            }
        }
        crate::codestream::DecompositionDirection::None => {
            return Err(invalid_decode());
        }
    };
    if plane.fraction_bits() != expected_fraction_bits {
        return Err(invalid_decode());
    }
    copy_prepare_plane(plane, first_detail)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures prepare_fold_outcome(result) == crate::specs::part1_prepare_copy_plane(crate::transform::reconstructed_plane_view(plane), first_detail),
))]
fn copy_prepare_plane(
    plane: crate::transform::ReconstructedPlane,
    first_detail: usize,
) -> Result<PrepareFold, DecodeError> {
    let reconstructed_width = plane.width();
    let reconstructed_height = plane.height();
    let plane_sample_count = plane.samples().len();
    let mut next_samples = Vec::with_capacity(plane_sample_count);
    let mut sample_index = 0_usize;
    #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant sample_index <= plane_sample_count,
                    plane_sample_count == crate::transform::reconstructed_plane_view(plane).3.len(),
                    reconstructed_width == crate::transform::reconstructed_plane_view(plane).0,
                    reconstructed_height == crate::transform::reconstructed_plane_view(plane).1,
                    next_samples@ == crate::transform::reconstructed_plane_view(plane).3.subrange(0, sample_index as int),
                decreases plane_sample_count - sample_index,
            )
        )]
    while sample_index < plane_sample_count {
        next_samples.push(
            *plane
                .samples()
                .get(sample_index)
                .ok_or_else(invalid_decode)?,
        );
        sample_index = sample_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { assert(next_samples@ =~= crate::transform::reconstructed_plane_view(plane).3); }
    Ok(PrepareFold {
        width: reconstructed_width,
        height: reconstructed_height,
        samples: next_samples,
        first_detail,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures prepared_component_outcome(result) == crate::specs::part1_prepare_finish(prepare_plan_view(plan), fraction, folded@),
))]
fn finish_prepared_component(
    plan: &PlannedTileComponent,
    folded: PrepareFold,
    fraction: u8,
) -> Result<PreparedTileComponent, DecodeError> {
    if folded.width != plan.width
        || folded.height != plan.height
        || folded.samples.len() != checked_area_u32(plan.width, plan.height)?
    {
        return Err(invalid_decode());
    }
    Ok(PreparedTileComponent {
        component: plan.component,
        x: plan.x,
        y: plan.y,
        width: plan.width,
        height: plan.height,
        fraction_bits: fraction,
        samples: folded.samples,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures prepared_tiles_outcome(result) == Ok(prepared_components_view(prepared@)),
))]
fn prepared_tile_views<'a>(
    prepared: &'a [PreparedTileComponent],
) -> Result<Vec<TileComponent<'a>>, DecodeError> {
    let mut tiles = Vec::with_capacity(prepared.len());
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= prepared.len(),
        tiles@.len() == index,
        crate::image::assembly_tiles_view(tiles@) == prepared_components_view(prepared@).take(index as int),
        decreases prepared.len() - index,
    ))]
    while index < prepared.len() {
        let tile = prepared
            .get(index)
            .ok_or_else(invalid_decode)?
            .as_tile_component();
        #[cfg(verus_keep_ghost)]
        proof! { assert(crate::image::assembly_tile_view(tile) == prepared_components_view(prepared@)[index as int]); }
        tiles.push(tile);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::image::assembly_tiles_view(tiles@) =~= prepared_components_view(prepared@).take(index as int + 1));
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { assert(prepared_components_view(prepared@).take(prepared@.len() as int) =~= prepared_components_view(prepared@)); }
    Ok(tiles)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures crate::image::decoded_image_outcome(result) == crate::specs::part2_assemble_prepared(
        crate::image::assembly_size_view(size), prepared_components_view(prepared@), selection,
        crate::limits::resource_decoded_bytes_limit(limits)),
))]
fn assemble_prepared_components(
    size: &ImageSize,
    prepared: &[PreparedTileComponent],
    selection: Option<([u32; 4], u8)>,
    limits: &ResourceLimits,
) -> Result<DecodedImage, DecodeError> {
    let tiles = prepared_tile_views(prepared)?;
    match selection {
        None => assemble_image(size, &tiles, limits),
        Some((region, levels)) => assemble_selected_image(size, &tiles, region, levels, limits),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures crate::image::decoded_image_outcome(result) == crate::specs::part1_progression_prepared_image(
        crate::image::assembly_size_view(size), progression_reconstruction_view(plan).0, progression_reconstruction_view(plan).1,
        progression_reconstruction_view(plan).2, progression_reconstruction_view(plan).3, crate::limits::resource_decoded_bytes_limit(limits)),
))]
pub fn decode_progression_prepared_image(
    size: &ImageSize,
    plan: &ProgressionTilePlan,
    limits: &ResourceLimits,
) -> Result<DecodedImage, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost plans = planned_components_view(plan); let ghost precincts = prepare_precincts_view(plan.precincts@); }
    let mut prepared = Vec::with_capacity(plan.components.len());
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_reconstruct_components_empty(Seq::empty(), plans, precincts,
            plan.component_coding_styles, Seq::empty(), crate::limits::resource_decoded_bytes_limit(limits));
    }
    let mut component_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component_index <= plan.components.len(),
            plans == planned_components_view(plan), precincts == prepare_precincts_view(plan.precincts@),
            crate::specs::part2_reconstruct_components_prefix(Seq::empty(), plans, precincts, plan.component_coding_styles,
                Seq::empty(), crate::limits::resource_decoded_bytes_limit(limits), component_index as nat)
                    == (Ok(()), prepared_components_view(prepared@)),
            decreases plan.components.len() - component_index,
        )
    )]
    while component_index < plan.components.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_reconstruct_components_step(Seq::empty(), plans, precincts, plan.component_coding_styles,
                Seq::empty(), crate::limits::resource_decoded_bytes_limit(limits), component_index as nat);
            crate::proofs::production_impl::production_reconstruct_components_error(Seq::empty(), plans, precincts, plan.component_coding_styles,
                Seq::empty(), crate::limits::resource_decoded_bytes_limit(limits), component_index as nat + 1, plans.len());
        }
        let component = plan
            .components
            .get(component_index)
            .ok_or_else(invalid_decode)?;
        let style = plan.component_coding_styles.style(component.component)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            if let WaveletTransform::Arbitrary(index) = style.transform {
                crate::proofs::production_impl::production_kernel_registry_empty(Seq::empty(), index);
            }
        }
        prepared.push(prepare_tile_component(
            component,
            &plan.precincts,
            style,
            limits,
        )?);
        component_index = component_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    let tile_components = prepared_tile_views(&prepared)?;
    let component_transform = if !plan.coding_style.multiple_component_transform {
        ComponentTransform::None
    } else {
        match plan.coding_style.transform {
            WaveletTransform::Reversible5x3 => ComponentTransform::Reversible,
            WaveletTransform::Irreversible9x7 => ComponentTransform::Irreversible,
            WaveletTransform::Arbitrary(_) => {
                return Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::MissingRequiredStructure,
                });
            }
        }
    };
    decode_prepared_image(size, &tile_components, component_transform, limits)
}

pub fn decode_codestream(input: &[u8], options: &DecodeOptions) -> Result<TypedImage, DecodeError> {
    let layout = codestream::parse_layout(input, options.limits())?;
    let image = typed_layout(
        &layout,
        decode_layout(&layout, options.limits())?,
        options.limits(),
    )?;
    validate_part1_output_shape(input.len(), &layout, &image)?;
    Ok(image)
}

pub fn decode_codestream_selected(
    input: &[u8],
    options: &DecodeOptions,
    selection: &DecodeSelection,
) -> Result<TypedImage, DecodeError> {
    let layout = codestream::parse_layout(input, options.limits())?;
    let image = typed_layout(
        &layout,
        decode_layout_selected(&layout, selection, options.limits())?,
        options.limits(),
    )?;
    validate_part1_output_shape(input.len(), &layout, &image)?;
    Ok(image)
}

fn validate_part1_output_shape(
    input_bytes: usize,
    layout: &CodestreamLayout<'_>,
    image: &TypedImage,
) -> Result<(), DecodeError> {
    if !crate::refinement::decode_output_shape_valid(
        input_bytes,
        layout.tile_parts.len(),
        layout.info.image_size.components,
        image.components().len(),
        image.width(),
        image.height(),
    ) {
        return Err(invalid_decode());
    }
    let component_count = image.components().len();
    let mut component = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= component_count,
            decreases component_count - component,
        )
    )]
    while component < component_count {
        let output = image
            .components()
            .get(component)
            .ok_or_else(invalid_decode)?;
        let samples = match output.samples() {
            TypedSamples::Signed(samples) => samples.len(),
            TypedSamples::Unsigned(samples) => samples.len(),
        };
        if !crate::refinement::component_output_shape_valid(
            output.width(),
            output.height(),
            samples,
        ) {
            return Err(invalid_decode());
        }
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

fn typed_layout(
    layout: &CodestreamLayout<'_>,
    decoded: DecodedImage,
    limits: &ResourceLimits,
) -> Result<TypedImage, DecodeError> {
    let mut image = decoded.typed(limits)?;
    let mut regions = Vec::new();
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= layout.info.image_size.components,
            decreases layout.info.image_size.components - component,
        )
    )]
    while component < layout.info.image_size.components {
        let info = layout.info.image_size.component(component)?;
        let output = image.component(component)?;
        let output_x0 = registered_ceil_div_u32(
            image.x_origin(),
            u32::from(info.x_subsampling),
            info.x_registration,
        )?;
        let output_y0 = registered_ceil_div_u32(
            image.y_origin(),
            u32::from(info.y_subsampling),
            info.y_registration,
        )?;
        let output_x1 = output_x0
            .checked_add(output.width())
            .ok_or_else(arithmetic_overflow)?;
        let output_y1 = output_y0
            .checked_add(output.height())
            .ok_or_else(arithmetic_overflow)?;
        let tile_count = layout.info.image_size.tile_count()?;
        let mut tile = 0_u64;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant tile <= tile_count,
                decreases tile_count - tile,
            )
        )]
        while tile < tile_count {
            let tile_index = u16::try_from(tile).map_err(|_| arithmetic_overflow())?;
            let mut extensions = &layout.info.part2_extensions;
            let mut part = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant part <= layout.tile_parts.len(),
                    decreases layout.tile_parts.len() - part,
                )
            )]
            while part < layout.tile_parts.len() {
                let tile_part = layout.tile_parts.get(part).ok_or_else(invalid_decode)?;
                if tile_part.tile_index == tile_index {
                    extensions = &tile_part.part2_extensions;
                }
                part = part.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            if let Some(transform) = extensions.nonlinear_transform_for(component) {
                let bounds = layout
                    .info
                    .image_size
                    .tile_component_bounds(tile_index, component)?;
                let x0 = bounds.x0.max(output_x0);
                let y0 = bounds.y0.max(output_y0);
                let x1 = bounds.x1.min(output_x1);
                let y1 = bounds.y1.min(output_y1);
                if x0 < x1 && y0 < y1 {
                    regions.push(NonlinearRegion {
                        component,
                        x0: x0.checked_sub(output_x0).ok_or_else(arithmetic_overflow)?,
                        y0: y0.checked_sub(output_y0).ok_or_else(arithmetic_overflow)?,
                        x1: x1.checked_sub(output_x0).ok_or_else(arithmetic_overflow)?,
                        y1: y1.checked_sub(output_y0).ok_or_else(arithmetic_overflow)?,
                        transform,
                    });
                }
            }
            tile = tile.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    if !regions.is_empty() {
        image.apply_nonlinear_regions(&regions, limits)?;
    }
    Ok(image)
}

pub fn decode_jp2(input: &[u8], options: &DecodeOptions) -> Result<TypedImage, DecodeError> {
    let inspected = jp2::parse_and_inspect(input, options.limits())?;
    if !inspected
        .file()
        .file_type()
        .is_compatible_with(jp2::BoxType::JP2_BRAND)
    {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    if inspected.file().codestream_count() != 1 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    let decoded = decode_codestream(inspected.file().codestream(0)?.data(), options)?;
    apply_jp2_output(decoded, inspected.file().header(), options.limits())
}

pub fn decode_jp2_selected(
    input: &[u8],
    options: &DecodeOptions,
    selection: &DecodeSelection,
) -> Result<TypedImage, DecodeError> {
    let inspected = jp2::parse_and_inspect(input, options.limits())?;
    if !inspected
        .file()
        .file_type()
        .is_compatible_with(jp2::BoxType::JP2_BRAND)
        || inspected.file().codestream_count() != 1
    {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    let decoded =
        decode_codestream_selected(inspected.file().codestream(0)?.data(), options, selection)?;
    apply_jp2_output(decoded, inspected.file().header(), options.limits())
}

pub fn decode_jp2_color(
    input: &[u8],
    options: &DecodeOptions,
) -> Result<crate::color::ConvertedImage, DecodeError> {
    let inspected = jp2::parse_and_inspect(input, options.limits())?;
    if !inspected
        .file()
        .file_type()
        .is_compatible_with(jp2::BoxType::JP2_BRAND)
    {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    if inspected.file().codestream_count() != 1 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    let decoded = decode_codestream(inspected.file().codestream(0)?.data(), options)?;
    let mapped = apply_jp2_output(decoded, inspected.file().header(), options.limits())?;
    crate::color::convert_jp2_color(&mapped, &inspected.file().header().color, options.limits())
}

pub fn decode_jp2_color_selected(
    input: &[u8],
    options: &DecodeOptions,
    selection: &DecodeSelection,
) -> Result<crate::color::ConvertedImage, DecodeError> {
    let inspected = jp2::parse_and_inspect(input, options.limits())?;
    if !inspected
        .file()
        .file_type()
        .is_compatible_with(jp2::BoxType::JP2_BRAND)
        || inspected.file().codestream_count() != 1
    {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    let codestream = inspected.file().codestream(0)?.data();
    let (expanded, target) = color_selection(codestream, selection, options.limits())?;
    let decoded = decode_codestream_selected(codestream, options, &expanded)?;
    finish_associated_selected_output(
        decoded,
        Some(inspected.file().header()),
        target,
        options.limits(),
    )
}

pub fn decode_jph(input: &[u8], options: &DecodeOptions) -> Result<TypedImage, DecodeError> {
    let inspected = jp2::inspect_jph(input, options.limits())?;
    if inspected.file().codestream_count() != 1 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    let decoded = decode_codestream(inspected.file().codestream(0)?.data(), options)?;
    apply_jp2_output(decoded, inspected.file().header(), options.limits())
}

pub fn decode_jph_selected(
    input: &[u8],
    options: &DecodeOptions,
    selection: &DecodeSelection,
) -> Result<TypedImage, DecodeError> {
    let inspected = jp2::inspect_jph(input, options.limits())?;
    if inspected.file().codestream_count() != 1 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    let decoded =
        decode_codestream_selected(inspected.file().codestream(0)?.data(), options, selection)?;
    apply_jp2_output(decoded, inspected.file().header(), options.limits())
}

pub fn decode_jph_color(
    input: &[u8],
    options: &DecodeOptions,
) -> Result<crate::color::ConvertedImage, DecodeError> {
    let inspected = jp2::inspect_jph(input, options.limits())?;
    if inspected.file().codestream_count() != 1 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    let decoded = decode_codestream(inspected.file().codestream(0)?.data(), options)?;
    let mapped = apply_jp2_output(decoded, inspected.file().header(), options.limits())?;
    crate::color::convert_jp2_color(&mapped, &inspected.file().header().color, options.limits())
}

pub fn decode_jph_color_selected(
    input: &[u8],
    options: &DecodeOptions,
    selection: &DecodeSelection,
) -> Result<crate::color::ConvertedImage, DecodeError> {
    let inspected = jp2::inspect_jph(input, options.limits())?;
    if inspected.file().codestream_count() != 1 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    let codestream = inspected.file().codestream(0)?.data();
    let (expanded, target) = color_selection(codestream, selection, options.limits())?;
    let decoded = decode_codestream_selected(codestream, options, &expanded)?;
    finish_associated_selected_output(
        decoded,
        Some(inspected.file().header()),
        target,
        options.limits(),
    )
}

pub fn decode_jpx_codestream(
    input: &[u8],
    codestream_index: usize,
    options: &DecodeOptions,
) -> Result<TypedImage, DecodeError> {
    let file = jp2::parse(input, options.limits())?;
    let (decoded, _) = decode_jpx_file(&file, codestream_index, options)?;
    Ok(decoded)
}

pub fn decode_jpx_codestream_selected(
    input: &[u8],
    codestream_index: usize,
    options: &DecodeOptions,
    selection: &DecodeSelection,
) -> Result<TypedImage, DecodeError> {
    let file = jp2::parse(input, options.limits())?;
    let (decoded, _) = decode_jpx_file_selected(&file, codestream_index, options, selection)?;
    Ok(decoded)
}

pub fn decode_jpx_codestream_color(
    input: &[u8],
    codestream_index: usize,
    options: &DecodeOptions,
) -> Result<crate::color::ConvertedImage, DecodeError> {
    let file = jp2::parse(input, options.limits())?;
    let (decoded, header) = decode_jpx_file(&file, codestream_index, options)?;
    convert_associated_color(&decoded, header, options.limits())
}

pub fn decode_jpx_codestream_color_selected(
    input: &[u8],
    codestream_index: usize,
    options: &DecodeOptions,
    selection: &DecodeSelection,
) -> Result<crate::color::ConvertedImage, DecodeError> {
    let file = jp2::parse(input, options.limits())?;
    if !file.file_type().is_jpx_compatible() {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    let (expanded, target) = color_selection(
        file.codestream(codestream_index)?.data(),
        selection,
        options.limits(),
    )?;
    let (decoded, header) = decode_jpx_file_selected(&file, codestream_index, options, &expanded)?;
    convert_associated_selected_color(&decoded, header, target, options.limits())
}

fn color_selection(
    input: &[u8],
    requested: &DecodeSelection,
    limits: &ResourceLimits,
) -> Result<(DecodeSelection, [u32; 4]), DecodeError> {
    let layout = codestream::parse_layout(input, limits)?;
    let size = &layout.info.image_size;
    color_selection_for_size(size, requested)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures color_selection_outcome(result) == crate::specs::part2_color_selection_requested(
        crate::codestream::selection_geometry_view(size), crate::codestream::image_size_component_infos(size),
        size.components, crate::options::decode_selection_view(requested),
    ),
))]
fn color_selection_for_size(
    size: &ImageSize,
    requested: &DecodeSelection,
) -> Result<(DecodeSelection, [u32; 4]), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { crate::codestream::selection_geometry_known_fields(size); }
    let validated = validate_selection(size, requested)?;
    color_selection_from_validated(size, validated)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures color_selection_outcome(result) == crate::specs::part2_color_selection_context(
        size.x_origin, size.y_origin, crate::codestream::image_size_component_infos(size), size.components,
        validated_selection_view(validated),
    ),
))]
fn color_selection_from_validated(
    size: &ImageSize,
    validated: ValidatedSelection,
) -> Result<(DecodeSelection, [u32; 4]), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part2_color_target); }
    let discard_levels = validated.discard_levels;
    if discard_levels >= 32 {
        return Err(DecodeError::InvalidSelection {
            reason: SelectionError::ReductionTooLarge,
        });
    }
    let scale = 1_u32 << u32::from(discard_levels);
    let target = [
        ceil_div_u32(validated.region[0], scale)?,
        ceil_div_u32(validated.region[1], scale)?,
        ceil_div_u32(validated.region[2], scale)?,
        ceil_div_u32(validated.region[3], scale)?,
    ];
    let mut horizontal = 0_u32;
    let mut vertical = 0_u32;
    let mut component = 0_u16;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(scale == 1u32 << (discard_levels as u32));
        assert(crate::specs::part2_selection_ceil(validated.region[0], scale) == Ok(target[0]));
        assert(crate::specs::part2_selection_ceil(validated.region[1], scale) == Ok(target[1]));
        assert(crate::specs::part2_selection_ceil(validated.region[2], scale) == Ok(target[2]));
        assert(crate::specs::part2_selection_ceil(validated.region[3], scale) == Ok(target[3]));
        assert(target@ =~= seq![target[0], target[1], target[2], target[3]]);
        assert(crate::specs::part2_color_target(validated.region@, discard_levels) == Ok(target@));
        crate::proofs::production_impl::production_color_sampling_empty(
            crate::codestream::image_size_component_infos(size), size.components, target@);
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= size.components,
            validated.discard_levels < 32,
            discard_levels == validated.discard_levels,
            scale == 1u32 << (discard_levels as u32),
            crate::specs::part2_color_target(validated.region@, discard_levels) == Ok(target@),
            crate::specs::part2_color_sampling_prefix(
                crate::codestream::image_size_component_infos(size), size.components, target@, component as nat,
            ) == Ok((horizontal, vertical)),
            decreases size.components - component,
        )
    )]
    while component < size.components {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_color_sampling_step(
                crate::codestream::image_size_component_infos(size), size.components, target@, component as nat);
            crate::proofs::production_impl::production_color_sampling_error_guard(
                crate::codestream::image_size_component_infos(size), size.components, target@, component as nat + 1, size.components as nat);
        }
        let info = codestream::image_component_info(size, component)?;
        let x_step = u64::from(info.x_subsampling)
            .checked_mul(65_536)
            .ok_or_else(arithmetic_overflow)?;
        let y_step = u64::from(info.y_subsampling)
            .checked_mul(65_536)
            .ok_or_else(arithmetic_overflow)?;
        let x = u64::from(target[0])
            .checked_mul(65_536)
            .ok_or_else(arithmetic_overflow)?
            .saturating_sub(u64::from(info.x_registration));
        let y = u64::from(target[1])
            .checked_mul(65_536)
            .ok_or_else(arithmetic_overflow)?
            .saturating_sub(u64::from(info.y_registration));
        if x.checked_rem(x_step).ok_or_else(invalid_decode)? != 0 {
            horizontal = horizontal.max(u32::from(info.x_subsampling));
        }
        if y.checked_rem(y_step).ok_or_else(invalid_decode)? != 0 {
            vertical = vertical.max(u32::from(info.y_subsampling));
        }
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let x0 = validated.region[0]
        .saturating_sub(horizontal.saturating_mul(scale))
        .max(size.x_origin);
    let y0 = validated.region[1]
        .saturating_sub(vertical.saturating_mul(scale))
        .max(size.y_origin);
    // Retain resampling context across ROI/tile boundaries; validate the original selection first.
    let expanded = crate::options::selection_from_parts(
        TileSelection::All,
        Some(crate::ImageRegion {
            x: x0,
            y: y0,
            width: validated.region[2]
                .checked_sub(x0)
                .ok_or_else(arithmetic_overflow)?,
            height: validated.region[3]
                .checked_sub(y0)
                .ok_or_else(arithmetic_overflow)?,
        }),
        discard_levels,
    );
    Ok((expanded, target))
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures crate::color::converted_image_result(result) == crate::specs::color_convert_selected(
            crate::image::typed_image_component_views(image), crate::image::typed_image_definition_views(image),
            crate::image::typed_image_geometry(image), crate::jp2::color_specification_method(color), target,
            crate::limits::resource_decoded_bytes_limit(limits), crate::limits::resource_work_limit(limits),
        ),
    )
)]
fn convert_selected_color(
    image: &TypedImage,
    color: &jp2::ColorSpecification,
    target: [u32; 4],
    limits: &ResourceLimits,
) -> Result<crate::color::ConvertedImage, DecodeError> {
    let converted = crate::color::convert_jp2_color(image, color, limits)?;
    converted.crop(
        target[0]
            .checked_sub(image.x_origin())
            .ok_or_else(invalid_decode)?,
        target[1]
            .checked_sub(image.y_origin())
            .ok_or_else(invalid_decode)?,
        target[2]
            .checked_sub(target[0])
            .ok_or_else(invalid_decode)?,
        target[3]
            .checked_sub(target[1])
            .ok_or_else(invalid_decode)?,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures associated_typed_outcome(result) == crate::specs::part2_apply_associated_output(
        crate::image::typed_image_value_view(decoded), jp2::associated_header_option(header),
        crate::limits::resource_component_limit(limits), crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
fn apply_associated_output<'a>(
    decoded: TypedImage,
    header: Option<&'a jp2::Jp2Header>,
    limits: &ResourceLimits,
) -> Result<(TypedImage, Option<&'a jp2::Jp2Header>), DecodeError> {
    let image = match header {
        Some(header) => apply_jp2_output(decoded, header, limits)?,
        None => decoded,
    };
    Ok((image, header))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures crate::color::converted_image_result(result) == crate::specs::part2_convert_associated_selected(
        crate::image::typed_image_value_view(*image), jp2::associated_header_option(header), target,
        crate::limits::resource_decoded_bytes_limit(limits), crate::limits::resource_work_limit(limits),
    ),
))]
fn convert_associated_selected_color(
    image: &TypedImage,
    header: Option<&jp2::Jp2Header>,
    target: [u32; 4],
    limits: &ResourceLimits,
) -> Result<crate::color::ConvertedImage, DecodeError> {
    let header = match header {
        Some(header) => header,
        None => {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::MissingRequiredStructure,
            });
        }
    };
    convert_selected_color(image, &header.color, target, limits)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures crate::color::converted_image_result(result) == crate::specs::part2_convert_associated(
        crate::image::typed_image_value_view(*image), jp2::associated_header_option(header),
        crate::limits::resource_decoded_bytes_limit(limits), crate::limits::resource_work_limit(limits),
    ),
))]
fn convert_associated_color(
    image: &TypedImage,
    header: Option<&jp2::Jp2Header>,
    limits: &ResourceLimits,
) -> Result<crate::color::ConvertedImage, DecodeError> {
    let header = match header {
        Some(header) => header,
        None => {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::MissingRequiredStructure,
            });
        }
    };
    crate::color::convert_jp2_color(image, &header.color, limits)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures crate::color::converted_image_result(result) == crate::specs::part2_finish_associated_selected_output(
        crate::image::typed_image_value_view(decoded), jp2::associated_header_option(header), target,
        crate::limits::resource_component_limit(limits), crate::limits::resource_decoded_bytes_limit(limits),
        crate::limits::resource_work_limit(limits),
    ),
))]
fn finish_associated_selected_output(
    decoded: TypedImage,
    header: Option<&jp2::Jp2Header>,
    target: [u32; 4],
    limits: &ResourceLimits,
) -> Result<crate::color::ConvertedImage, DecodeError> {
    let (mapped, header) = apply_associated_output(decoded, header, limits)?;
    convert_associated_selected_color(&mapped, header, target, limits)
}

pub(crate) fn decode_jpx_file<'a>(
    file: &'a jp2::Jp2File<'_>,
    codestream_index: usize,
    options: &DecodeOptions,
) -> Result<(TypedImage, Option<&'a jp2::Jp2Header>), DecodeError> {
    if !file.file_type().is_jpx_compatible() {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }

    #[cfg(any())]
    fn decode_jpx_file_selected<'a>(
        file: &'a jp2::Jp2File<'_>,
        codestream_index: usize,
        options: &DecodeOptions,
        selection: &DecodeSelection,
    ) -> Result<(TypedImage, Option<&'a jp2::Jp2Header>), DecodeError> {
        if !file.file_type().is_jpx_compatible() {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
        }
        let selected = file.codestream(codestream_index)?;
        let associated_header = Some(jp2::associated_header_for_codestream(
            selected,
            file.header(),
        )?);
        let layout = codestream::parse_layout(selected.data(), options.limits())?;
        if let Some(header) = associated_header {
            jp2::validate_selected_header(header, &layout.info, selected.box_header().offset())?;
        }
        let decoded = typed_layout(
            &layout,
            decode_layout_selected(&layout, selection, options.limits())?,
            options.limits(),
        )?;
        apply_associated_output(decoded, associated_header, options.limits())
    }
    let selected = file.codestream(codestream_index)?;
    let associated_header = Some(jp2::associated_header_for_codestream(
        selected,
        file.header(),
    )?);
    let info = codestream::parse(selected.data(), options.limits())?;
    if let Some(header) = associated_header {
        jp2::validate_selected_header(header, &info, selected.box_header().offset())?;
    }
    let decoded = decode_codestream(selected.data(), options)?;
    apply_associated_output(decoded, associated_header, options.limits())
}

fn decode_jpx_file_selected<'a>(
    file: &'a jp2::Jp2File<'_>,
    codestream_index: usize,
    options: &DecodeOptions,
    selection: &DecodeSelection,
) -> Result<(TypedImage, Option<&'a jp2::Jp2Header>), DecodeError> {
    if !file.file_type().is_jpx_compatible() {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidProfile));
    }
    let selected = file.codestream(codestream_index)?;
    let associated_header = match selected.codestream_header() {
        Some(association) => match association.parsed_header() {
            Some(header) => Some(header),
            None => {
                return Err(DecodeError::malformed(
                    association.box_header().offset(),
                    MalformedKind::MissingRequiredStructure,
                ));
            }
        },
        None => Some(file.header()),
    };
    let layout = codestream::parse_layout(selected.data(), options.limits())?;
    if let Some(header) = associated_header {
        jp2::validate_selected_header(header, &layout.info, selected.box_header().offset())?;
    }
    let decoded = typed_layout(
        &layout,
        decode_layout_selected(&layout, selection, options.limits())?,
        options.limits(),
    )?;
    let image = match associated_header {
        Some(header) => apply_jp2_output(decoded, header, options.limits())?,
        None => decoded,
    };
    Ok((image, associated_header))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures crate::image::decoded_typed_image_outcome(result) == crate::specs::part2_apply_output_header(
        crate::image::typed_image_value_view(decoded), jp2::output_header_view(header),
        crate::limits::resource_component_limit(limits), crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
fn apply_jp2_output(
    decoded: TypedImage,
    header: &jp2::Jp2Header,
    limits: &ResourceLimits,
) -> Result<TypedImage, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost input_components = crate::image::typed_components_output_view(crate::image::typed_image_components(&decoded));
        let ghost raw_header = jp2::output_header_view(header);
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(input_components =~= Seq::new(crate::image::typed_image_value_view(decoded).1.len(),
            |i: int| (crate::image::typed_image_value_view(decoded).1[i], crate::image::typed_image_value_view(decoded).2[i])));
    }
    let mut components = if header.component_mappings.is_empty() {
        let mut copied = Vec::with_capacity(decoded.components().len());
        let component_count = decoded.components().len();
        let mut component_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_index <= component_count,
                component_count == input_components.len(),
                raw_header == jp2::output_header_view(header),
                raw_header.1.len() == 0,
                input_components == crate::image::typed_components_output_view(crate::image::typed_image_components(&decoded)),
                input_components == Seq::new(crate::image::typed_image_value_view(decoded).1.len(),
                    |i: int| (crate::image::typed_image_value_view(decoded).1[i], crate::image::typed_image_value_view(decoded).2[i])),
                crate::image::typed_components_output_view(copied@) == input_components.take(component_index as int),
                decreases component_count - component_index,
            )
        )]
        while component_index < component_count {
            let component = copy_typed_component(
                decoded
                    .components()
                    .get(component_index)
                    .ok_or_else(invalid_jp2_output)?,
            )?;
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_typed_components_output_push(copied@, component);
                assert(input_components.take(component_index as int + 1) =~= input_components.take(component_index as int).push(input_components[component_index as int]));
            }
            copied.push(component);
            component_index = component_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::image::typed_components_output_view(copied@) =~= input_components);
        }
        copied
    } else {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_output_mapping_empty(input_components, raw_header);
        }
        let mut mapped = Vec::with_capacity(header.component_mappings.len());
        #[cfg(verus_keep_ghost)]
        proof! { assert(crate::image::typed_components_output_view(mapped@) =~= Seq::empty()); }
        let mut mapping_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant mapping_index <= header.component_mappings.len(),
                raw_header == jp2::output_header_view(header),
                raw_header.1.len() > 0,
                input_components == crate::image::typed_components_output_view(crate::image::typed_image_components(&decoded)),
                input_components == Seq::new(crate::image::typed_image_value_view(decoded).1.len(),
                    |i: int| (crate::image::typed_image_value_view(decoded).1[i], crate::image::typed_image_value_view(decoded).2[i])),
                crate::specs::part2_output_mapping_prefix(input_components, raw_header, mapping_index as nat)
                    == Ok(crate::image::typed_components_output_view(mapped@)),
                decreases header.component_mappings.len() - mapping_index,
            )
        )]
        while mapping_index < header.component_mappings.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_output_mapping_step(input_components, raw_header, mapping_index as nat);
                crate::proofs::production_impl::production_output_mapping_error(input_components, raw_header, mapping_index as nat + 1, raw_header.1.len());
            }
            let mapping = header
                .component_mappings
                .get(mapping_index)
                .ok_or_else(invalid_jp2_output)?;
            let source = decoded.component(mapping.component)?;
            let component = match mapping.mapping_type {
                jp2::MappingType::Direct => copy_typed_component(source)?,
                jp2::MappingType::Palette { column } => {
                    let palette = header.palette.as_ref().ok_or_else(invalid_jp2_output)?;
                    map_palette_component(source, palette, column)?
                }
            };
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_typed_components_output_push(mapped@, component);
            }
            mapped.push(component);
            mapping_index = mapping_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        mapped
    };

    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost mapped_start = crate::image::typed_components_output_view(components@); }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_output_definition_empty(mapped_start, raw_header.2);
    }
    let mut definition_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant definition_index <= header.channel_definition_records.len(),
            raw_header == jp2::output_header_view(header),
            input_components == crate::image::typed_components_output_view(crate::image::typed_image_components(&decoded)),
            input_components == Seq::new(crate::image::typed_image_value_view(decoded).1.len(),
                |i: int| (crate::image::typed_image_value_view(decoded).1[i], crate::image::typed_image_value_view(decoded).2[i])),
            (if raw_header.1.len() == 0 { Ok(input_components) }
                else { crate::specs::part2_output_mapping_prefix(input_components, raw_header, raw_header.1.len()) })
                == Ok(mapped_start),
            crate::specs::part2_output_definition_prefix(mapped_start, raw_header.2, definition_index as nat)
                == Ok(crate::image::typed_components_output_view(components@)),
            decreases header.channel_definition_records.len() - definition_index,
        )
    )]
    while definition_index < header.channel_definition_records.len() {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost previous_components = crate::image::typed_components_output_view(components@); }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_output_definition_step(mapped_start, raw_header.2, definition_index as nat);
            crate::proofs::production_impl::production_output_definition_error(mapped_start, raw_header.2, definition_index as nat + 1, raw_header.2.len());
        }
        let record = header
            .channel_definition_records
            .get(definition_index)
            .ok_or_else(invalid_jp2_output)?;
        let component = components
            .get_mut(usize::from(record.channel))
            .ok_or_else(invalid_jp2_output)?;
        component.add_channel_definition(record.definition);
        #[cfg(verus_keep_ghost)]
        proof! {
            let channel = record.channel as int;
            let previous = previous_components[channel];
            let current = crate::image::typed_component_output_view(&components@[channel]);
            assert(current.0 == previous.0);
            assert(current.1 =~= previous.1.push(crate::image::channel_definition_view(record.definition)));
            assert(crate::image::typed_components_output_view(components@) =~= previous_components.update(
                channel, (previous.0, previous.1.push(crate::image::channel_definition_view(record.definition)))));
        }
        definition_index = definition_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    crate::image::typed_image_from_components_with_origin(
        decoded.width(),
        decoded.height(),
        decoded.x_origin(),
        decoded.y_origin(),
        components,
        limits,
    )
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures crate::image::typed_component_output(result)
            == crate::specs::part2_typed_component_copy(crate::image::typed_component_output_view(source)),
    )
)]
fn copy_typed_component(source: &TypedComponent) -> Result<TypedComponent, DecodeError> {
    let samples = match source.samples() {
        TypedSamples::Signed(source_samples) => {
            let mut copied = Vec::with_capacity(source_samples.len());
            let mut index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant index <= source_samples.len(),
                    copied@ == source_samples@.take(index as int),
                    decreases source_samples.len() - index,
                )
            )]
            while index < source_samples.len() {
                copied.push(*source_samples.get(index).ok_or_else(invalid_jp2_output)?);
                index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            #[cfg(verus_keep_ghost)]
            proof! { assert(copied@ =~= source_samples@); }
            TypedSamples::Signed(copied)
        }
        TypedSamples::Unsigned(source_samples) => {
            let mut copied = Vec::with_capacity(source_samples.len());
            let mut index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant index <= source_samples.len(),
                    copied@ == source_samples@.take(index as int),
                    decreases source_samples.len() - index,
                )
            )]
            while index < source_samples.len() {
                copied.push(*source_samples.get(index).ok_or_else(invalid_jp2_output)?);
                index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            #[cfg(verus_keep_ghost)]
            proof! { assert(copied@ =~= source_samples@); }
            TypedSamples::Unsigned(copied)
        }
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(crate::image::typed_samples_view(&samples) == crate::image::typed_component_view(source).samples);
    }
    let mut copied = crate::image::typed_component_from_parts(
        source.width(),
        source.height(),
        source.info(),
        samples,
    );
    let definition_count = source.channel_definitions().len();
    let mut definition_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant definition_index <= definition_count,
            definition_count == crate::image::typed_component_definitions(source).len(),
            crate::image::typed_component_view(&copied) == crate::image::typed_component_view(source),
            crate::image::typed_component_definitions(&copied)
                == crate::image::typed_component_definitions(source).take(definition_index as int),
            decreases definition_count - definition_index,
        )
    )]
    while definition_index < definition_count {
        copied.add_channel_definition(
            *source
                .channel_definitions()
                .get(definition_index)
                .ok_or_else(invalid_jp2_output)?,
        );
        definition_index = definition_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(crate::image::typed_component_definitions(&copied)
            =~= crate::image::typed_component_definitions(source));
    }
    Ok(copied)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures crate::image::typed_component_output(result) == crate::specs::part2_palette_component(
            crate::image::typed_component_view(source), crate::jp2::palette_view(palette), column,
        ),
    )
)]
fn map_palette_component(
    source: &TypedComponent,
    palette: &jp2::Palette,
    column: u8,
) -> Result<TypedComponent, DecodeError> {
    let descriptor = palette
        .columns()
        .get(usize::from(column))
        .ok_or_else(invalid_jp2_output)?;
    let precision = descriptor.precision;
    if precision == 0 || precision > 38 {
        return Err(invalid_jp2_output());
    }
    let sample_count = match source.samples() {
        TypedSamples::Signed(samples) => samples.len(),
        TypedSamples::Unsigned(samples) => samples.len(),
    };
    let mut signed_samples = if descriptor.signed {
        Vec::with_capacity(sample_count)
    } else {
        Vec::new()
    };
    let mut unsigned_samples = if descriptor.signed {
        Vec::new()
    } else {
        Vec::with_capacity(sample_count)
    };
    let mut sample_index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_palette_samples_empty(
            crate::image::typed_component_view(source).samples, crate::jp2::palette_view(palette), column,
        );
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant sample_index <= sample_count,
            sample_count as nat == crate::specs::part2_typed_sample_count(crate::image::typed_component_view(source).samples),
            column < crate::jp2::palette_view(palette).1.len(),
            *descriptor == crate::jp2::palette_view(palette).1[column as int],
            precision == descriptor.precision, 0 < precision <= 38,
            crate::specs::part2_palette_samples(
                crate::image::typed_component_view(source).samples, crate::jp2::palette_view(palette), column, sample_index as nat,
            ) == Ok(if descriptor.signed {
                crate::specs::Part1TypedSampleSequence::Signed(signed_samples@)
            } else { crate::specs::Part1TypedSampleSequence::Unsigned(unsigned_samples@) }),
            decreases sample_count - sample_index,
        )
    )]
    while sample_index < sample_count {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_palette_samples_step(
                crate::image::typed_component_view(source).samples, crate::jp2::palette_view(palette), column, sample_index as nat,
            );
        }
        let index_status = match source.samples() {
            TypedSamples::Signed(samples) => {
                let value = *samples.get(sample_index).ok_or_else(invalid_jp2_output)?;
                u16::try_from(value)
            }
            TypedSamples::Unsigned(samples) => {
                let value = *samples.get(sample_index).ok_or_else(invalid_jp2_output)?;
                u16::try_from(value)
            }
        };
        let palette_index = match index_status {
            Ok(index) => index,
            Err(_) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_palette_sample_error(
                        crate::image::typed_component_view(source), crate::jp2::palette_view(palette),
                        column, sample_index, crate::specs::part2_color_input_error(),
                    );
                }
                return Err(invalid_jp2_output());
            }
        };
        if palette_index >= palette.entries() {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_palette_sample_error(
                    crate::image::typed_component_view(source), crate::jp2::palette_view(palette),
                    column, sample_index, crate::specs::part2_color_input_error(),
                );
            }
            return Err(invalid_jp2_output());
        }
        let value_status = palette.value(palette_index, column);
        #[cfg(verus_keep_ghost)]
        proof! {
            if let Err(error) = value_status {
                crate::proofs::production_impl::production_palette_sample_error(
                    crate::image::typed_component_view(source), crate::jp2::palette_view(palette),
                    column, sample_index, error,
                );
            }
        }
        let value = value_status?;
        if descriptor.signed {
            let modulus = palette_modulus(precision)?;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(2 <= modulus <= 274877906944) by(bit_vector)
                    requires 0 < precision <= 38, modulus == 1u64 << precision;
            }
            let half = i64::try_from(modulus.checked_div(2).ok_or_else(arithmetic_overflow)?)
                .map_err(|_| arithmetic_overflow())?;
            let minimum = 0_i64.checked_sub(half).ok_or_else(arithmetic_overflow)?;
            let maximum = half.checked_sub(1).ok_or_else(arithmetic_overflow)?;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(half == 1i64 << ((precision - 1) as u8)) by(bit_vector)
                    requires 0 < precision <= 38, modulus == 1u64 << precision,
                        half >= 0, half as u64 == modulus / 2;
            }
            if value < minimum || value > maximum {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_palette_sample_error(
                        crate::image::typed_component_view(source), crate::jp2::palette_view(palette),
                        column, sample_index, crate::specs::part2_color_input_error(),
                    );
                }
                return Err(invalid_jp2_output());
            }
            signed_samples.push(value);
        } else {
            let modulus = palette_modulus(precision)?;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(2 <= modulus <= 274877906944) by(bit_vector)
                    requires 0 < precision <= 38, modulus == 1u64 << precision;
            }
            let maximum = modulus.checked_sub(1).ok_or_else(arithmetic_overflow)?;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(maximum < 274877906944) by(bit_vector)
                    requires 0 < precision <= 38, maximum == (1u64 << precision) - 1;
                assert(maximum as i64 == ((1i64 << precision) - 1)) by(bit_vector)
                    requires 0 < precision <= 38, maximum == (1u64 << precision) - 1;
            }
            let unsigned_value = match u64::try_from(value) {
                Ok(value) => value,
                Err(_) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        crate::proofs::production_impl::production_palette_sample_error(
                            crate::image::typed_component_view(source), crate::jp2::palette_view(palette),
                            column, sample_index, crate::specs::part2_color_input_error(),
                        );
                    }
                    return Err(invalid_jp2_output());
                }
            };
            if unsigned_value > maximum {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_palette_sample_error(
                        crate::image::typed_component_view(source), crate::jp2::palette_view(palette),
                        column, sample_index, crate::specs::part2_color_input_error(),
                    );
                }
                return Err(invalid_jp2_output());
            }
            unsigned_samples.push(unsigned_value);
        }
        sample_index = sample_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    let mut info = source.info();
    info.precision = descriptor.precision;
    info.signed = descriptor.signed;
    let samples = if descriptor.signed {
        TypedSamples::Signed(signed_samples)
    } else {
        TypedSamples::Unsigned(unsigned_samples)
    };
    Ok(crate::image::typed_component_from_parts(
        source.width(),
        source.height(),
        info,
        samples,
    ))
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result => ensures result == crate::specs::part2_palette_modulus(precision),)
)]
fn palette_modulus(precision: u8) -> Result<u64, DecodeError> {
    if precision == 0 || precision > 38 {
        return Err(invalid_jp2_output());
    }
    let mut modulus = 1_u64;
    let mut bit = 0_u8;
    #[cfg(verus_keep_ghost)]
    proof! { assert(1u64 << 0u32 == 1) by(bit_vector); }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant bit <= precision, 0 < precision <= 38,
            modulus == 1u64 << (bit as u32),
            decreases precision - bit,
        )
    )]
    while bit < precision {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(modulus <= 137438953472) by(bit_vector)
                requires bit < 38, modulus == 1u64 << (bit as u32);
            assert(modulus * 2 == 1u64 << ((bit + 1) as u32)) by(bit_vector)
                requires bit < 38, modulus == 1u64 << (bit as u32);
        }
        modulus = modulus.checked_mul(2).ok_or_else(arithmetic_overflow)?;
        bit = bit.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(modulus)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(error => ensures error == crate::specs::part2_color_input_error(),)
)]
fn invalid_jp2_output() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ContainerCodestreamMismatch,
    }
}

struct ValidatedSelection {
    tiles: Vec<u16>,
    region: [u32; 4],
    discard_levels: u8,
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn validated_selection_view(selection: ValidatedSelection) -> crate::specs::Part2ValidatedSelection {
        (selection.tiles@, selection.region@, selection.discard_levels)
    }

    closed spec fn color_selection_outcome(result: Result<(DecodeSelection, [u32; 4]), DecodeError>)
        -> Result<(crate::specs::Part2DecodeSelection, Seq<u32>), DecodeError>
    {
        match result {
            Err(error) => Err(error),
            Ok(output) => Ok((crate::options::decode_selection_value_view(output.0), output.1@)),
        }
    }

    closed spec fn selection_region_outcome(result: Result<[u32; 4], DecodeError>) -> Result<Seq<u32>, DecodeError> {
        match result { Err(error) => Err(error), Ok(region) => Ok(region@) }
    }

    closed spec fn associated_typed_outcome(result: Result<(TypedImage, Option<&jp2::Jp2Header>), DecodeError>)
        -> Result<(crate::specs::Part1DecodedTypedImage, Option<crate::specs::Part2AssociatedHeader>), DecodeError>
    {
        match result {
            Err(error) => Err(error),
            Ok(output) => Ok((crate::image::typed_image_value_view(output.0), jp2::associated_header_option(output.1))),
        }
    }

    closed spec fn selected_tiles_outcome(result: Result<Vec<u16>, DecodeError>) -> Result<Seq<u16>, DecodeError> {
        match result { Err(error) => Err(error), Ok(tiles) => Ok(tiles@) }
    }

    closed spec fn validated_selection_outcome(result: Result<ValidatedSelection, DecodeError>)
        -> Result<crate::specs::Part2ValidatedSelection, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(selection) => Ok(validated_selection_view(selection)) }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_tile_list_contains(tiles@, tile),
))]
fn tile_list_contains(tiles: &[u16], tile: u16) -> Result<bool, DecodeError> {
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= tiles.len(),
            forall |i: int| 0 <= i < index ==> tiles@[i] != tile,
            decreases tiles.len() - index,
        )
    )]
    while index < tiles.len() {
        if *tiles.get(index).ok_or_else(arithmetic_overflow)? == tile {
            return Ok(true);
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(false)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_explicit_tile_prefix(explicit_tiles@, tile_count, explicit_tiles@.len()),
))]
fn validate_explicit_tile_indices(
    explicit_tiles: &[u16],
    tile_count: u64,
) -> Result<(), DecodeError> {
    let mut index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! { crate::proofs::production_impl::production_explicit_tiles_empty(explicit_tiles@, tile_count); }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= explicit_tiles.len(),
        crate::specs::part2_explicit_tile_prefix(explicit_tiles@, tile_count, index as nat) == Ok(()),
        decreases explicit_tiles.len() - index,
    ))]
    while index < explicit_tiles.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_explicit_tiles_step(explicit_tiles@, tile_count, index as nat);
            crate::proofs::production_impl::production_explicit_tiles_error_guard(explicit_tiles@, tile_count, index as nat + 1, explicit_tiles@.len());
        }
        let tile = *explicit_tiles.get(index).ok_or_else(arithmetic_overflow)?;
        if u64::from(tile) >= tile_count {
            return Err(DecodeError::InvalidSelection {
                reason: SelectionError::TileOutOfRange,
            });
        }
        let mut earlier = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant earlier <= index, index < explicit_tiles.len(),
            explicit_tiles@[index as int] == tile, (tile as u64) < tile_count,
            crate::specs::part2_explicit_tile_prefix(explicit_tiles@, tile_count, index as nat) == Ok(()),
            forall |i: int| 0 <= i < earlier ==> explicit_tiles@[i] != tile,
            decreases index - earlier,
        ))]
        while earlier < index {
            if explicit_tiles[earlier] == tile {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(explicit_tiles@.take(index as int)[earlier as int] == tile);
                    assert(explicit_tiles@.take(index as int).contains(tile));
                    crate::proofs::production_impl::production_explicit_tiles_step(explicit_tiles@, tile_count, index as nat);
                    crate::proofs::production_impl::production_explicit_tiles_error_guard(explicit_tiles@, tile_count, index as nat + 1, explicit_tiles@.len());
                }
                return Err(DecodeError::InvalidSelection {
                    reason: SelectionError::DuplicateTile,
                });
            }
            earlier = earlier.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(!explicit_tiles@.take(index as int).contains(tile));
            crate::proofs::production_impl::production_explicit_tiles_step(explicit_tiles@, tile_count, index as nat);
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_rectangle_area(bounds),
))]
fn selection_rectangle_area(bounds: (u32, u32, u32, u32)) -> Result<u32, DecodeError> {
    let error = DecodeError::InvalidSelection {
        reason: SelectionError::NonRectangularTileSet,
    };
    let rows = bounds
        .2
        .checked_sub(bounds.0)
        .ok_or(error)?
        .checked_add(1)
        .ok_or(error)?;
    let columns = bounds
        .3
        .checked_sub(bounds.1)
        .ok_or(error)?
        .checked_add(1)
        .ok_or(error)?;
    rows.checked_mul(columns).ok_or(error)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    requires bounds.0 <= bounds.2, bounds.1 <= bounds.3,
        bounds.2 < u32::MAX, bounds.3 < u32::MAX,
    ensures result == crate::specs::part2_rectangle_rows(explicit_tiles@, tiles_across, bounds,
        (bounds.2 as nat - bounds.0 as nat + 1) as nat),
))]
fn validate_selection_rectangle_cells(
    explicit_tiles: &[u16],
    tiles_across: u32,
    bounds: (u32, u32, u32, u32),
) -> Result<(), DecodeError> {
    let mut row = bounds.0;
    #[cfg(verus_keep_ghost)]
    proof! { crate::proofs::production_impl::production_rectangle_rows_empty(explicit_tiles@, tiles_across, bounds); }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant bounds.0 <= bounds.2, bounds.1 <= bounds.3, bounds.2 < u32::MAX, bounds.3 < u32::MAX,
        bounds.0 <= row as int <= bounds.2 as int + 1,
        crate::specs::part2_rectangle_rows(explicit_tiles@, tiles_across, bounds, (row - bounds.0) as nat) == Ok(()),
        decreases bounds.2 as int + 1 - row as int,
    ))]
    while row <= bounds.2 {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_rectangle_rows_step(explicit_tiles@, tiles_across, bounds, (row - bounds.0) as nat);
            crate::proofs::production_impl::production_rectangle_rows_error(explicit_tiles@, tiles_across, bounds,
                (row as nat - bounds.0 as nat + 1) as nat, (bounds.2 as nat - bounds.0 as nat + 1) as nat);
        }
        let mut column = bounds.1;
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_rectangle_columns_empty(explicit_tiles@, tiles_across, bounds, row); }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant bounds.0 <= row <= bounds.2, bounds.1 <= bounds.3,
            bounds.2 < u32::MAX, bounds.3 < u32::MAX,
            bounds.1 <= column as int <= bounds.3 as int + 1,
            crate::specs::part2_rectangle_rows(explicit_tiles@, tiles_across, bounds, (row - bounds.0) as nat) == Ok(()),
            crate::specs::part2_rectangle_columns(explicit_tiles@, tiles_across, bounds, row, (column - bounds.1) as nat) == Ok(()),
            decreases bounds.3 as int + 1 - column as int,
        ))]
        while column <= bounds.3 {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_rectangle_columns_step(explicit_tiles@, tiles_across, bounds, row, (column - bounds.1) as nat);
                crate::proofs::production_impl::production_rectangle_columns_error(explicit_tiles@, tiles_across, bounds, row,
                    (column as nat - bounds.1 as nat + 1) as nat, (bounds.3 as nat - bounds.1 as nat + 1) as nat);
                crate::proofs::production_impl::production_rectangle_rows_step(explicit_tiles@, tiles_across, bounds, (row - bounds.0) as nat);
                crate::proofs::production_impl::production_rectangle_rows_error(explicit_tiles@, tiles_across, bounds,
                    (row as nat - bounds.0 as nat + 1) as nat, (bounds.2 as nat - bounds.0 as nat + 1) as nat);
            }
            let offset = row
                .checked_mul(tiles_across)
                .ok_or_else(arithmetic_overflow)?;
            let expected_index = offset.checked_add(column).ok_or_else(arithmetic_overflow)?;
            let expected_index = match u16::try_from(expected_index) {
                Ok(index) => index,
                Err(_) => return Err(arithmetic_overflow()),
            };
            if !tile_list_contains(explicit_tiles, expected_index)? {
                return Err(DecodeError::InvalidSelection {
                    reason: SelectionError::NonRectangularTileSet,
                });
            }
            column = column.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_rectangle_rows_step(explicit_tiles@, tiles_across, bounds, (row - bounds.0) as nat); }
        row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    requires explicit_tiles.len() > 1, tiles_across > 0,
    ensures result == crate::specs::part2_validate_tile_rectangle(explicit_tiles@, tiles_across),
))]
fn validate_selection_tile_rectangle(
    explicit_tiles: &[u16],
    tiles_across: u32,
) -> Result<(), DecodeError> {
    let first = u32::from(*explicit_tiles.first().ok_or_else(arithmetic_overflow)?);
    #[cfg(verus_keep_ghost)]
    proof! { crate::proofs::production_impl::production_unsigned_remainder_bound(first, tiles_across); }
    let mut first_row = first / tiles_across;
    let mut first_column = first % tiles_across;
    let mut last_row = first_row;
    let mut last_column = first_column;
    let mut bound_index = 1_usize;
    #[cfg(verus_keep_ghost)]
    proof! { crate::proofs::production_impl::production_tile_box_first(explicit_tiles@, tiles_across); }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant 1 <= bound_index <= explicit_tiles.len(), explicit_tiles.len() > 1, tiles_across > 0,
        first_row <= last_row, first_column <= last_column, last_row <= u16::MAX, last_column <= u16::MAX,
        crate::specs::part2_tile_rectangle_prefix(explicit_tiles@, tiles_across, bound_index as nat)
            == (first_row, first_column, last_row, last_column),
        decreases explicit_tiles.len() - bound_index,
    ))]
    while bound_index < explicit_tiles.len() {
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_tile_box_step(explicit_tiles@, tiles_across, bound_index as nat); }
        let current = u32::from(
            *explicit_tiles
                .get(bound_index)
                .ok_or_else(arithmetic_overflow)?,
        );
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_unsigned_remainder_bound(current, tiles_across); }
        let row = current / tiles_across;
        let column = current
            .checked_rem(tiles_across)
            .ok_or_else(arithmetic_overflow)?;
        first_row = first_row.min(row);
        first_column = first_column.min(column);
        last_row = last_row.max(row);
        last_column = last_column.max(column);
        bound_index = bound_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let bounds = (first_row, first_column, last_row, last_column);
    let expected = selection_rectangle_area(bounds)?;
    let expected = match usize::try_from(expected) {
        Ok(value) => value,
        Err(_) => return Err(arithmetic_overflow()),
    };
    if expected != explicit_tiles.len() {
        return Err(DecodeError::InvalidSelection {
            reason: SelectionError::NonRectangularTileSet,
        });
    }
    validate_selection_rectangle_cells(explicit_tiles, tiles_across, bounds)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures selection_region_outcome(result) == crate::specs::part2_selection_region(
        crate::codestream::selection_geometry_view(size), explicit_tiles@, requested, image_x1, image_y1,
    ),
))]
fn selection_region(
    size: &ImageSize,
    explicit_tiles: &[u16],
    requested: Option<crate::ImageRegion>,
    image_x1: u32,
    image_y1: u32,
) -> Result<[u32; 4], DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { crate::codestream::selection_geometry_known_fields(size); }
    match requested {
        Some(region) => {
            if region.width == 0 || region.height == 0 {
                return Err(DecodeError::InvalidSelection {
                    reason: SelectionError::EmptyRegion,
                });
            }
            let error = DecodeError::InvalidSelection {
                reason: SelectionError::RegionOutOfBounds,
            };
            let x1 = region.x.checked_add(region.width).ok_or(error)?;
            let y1 = region.y.checked_add(region.height).ok_or(error)?;
            if region.x < size.x_origin
                || region.y < size.y_origin
                || x1 > image_x1
                || y1 > image_y1
            {
                return Err(error);
            }
            Ok([region.x, region.y, x1, y1])
        }
        None if explicit_tiles.is_empty() => Ok([size.x_origin, size.y_origin, image_x1, image_y1]),
        None => {
            let mut x0 = image_x1;
            let mut y0 = image_y1;
            let mut x1 = size.x_origin;
            let mut y1 = size.y_origin;
            let mut tile_index = 0_usize;
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_selected_region_empty(
                    crate::codestream::selection_geometry_view(size), explicit_tiles@, image_x1, image_y1);
            }
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant tile_index <= explicit_tiles.len(), explicit_tiles.len() > 0, requested.is_none(),
                crate::specs::part2_selected_region_prefix(crate::codestream::selection_geometry_view(size),
                    explicit_tiles@, image_x1, image_y1, tile_index as nat) == Ok((x0, y0, x1, y1)),
                decreases explicit_tiles.len() - tile_index,
            ))]
            while tile_index < explicit_tiles.len() {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_selected_region_step(crate::codestream::selection_geometry_view(size),
                        explicit_tiles@, image_x1, image_y1, tile_index as nat);
                    crate::proofs::production_impl::production_selected_region_error(crate::codestream::selection_geometry_view(size),
                        explicit_tiles@, image_x1, image_y1, tile_index as nat + 1, explicit_tiles@.len());
                }
                let bounds = size.tile_bounds(
                    *explicit_tiles
                        .get(tile_index)
                        .ok_or_else(arithmetic_overflow)?,
                )?;
                x0 = x0.min(bounds.x0);
                y0 = y0.min(bounds.y0);
                x1 = x1.max(bounds.x1);
                y1 = y1.max(bounds.y1);
                tile_index = tile_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            Ok([x0, y0, x1, y1])
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures selected_tiles_outcome(result) == crate::specs::part2_select_intersecting_tiles(
        crate::codestream::selection_geometry_view(size), explicit_tiles@, region@, tile_count,
    ),
))]
fn select_intersecting_tiles(
    size: &ImageSize,
    explicit_tiles: &[u16],
    region: [u32; 4],
    tile_count: u64,
) -> Result<Vec<u16>, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { crate::codestream::selection_geometry_known_fields(size); }
    let mut tiles = Vec::new();
    let mut tile = 0_u64;
    #[cfg(verus_keep_ghost)]
    proof! { crate::proofs::production_impl::production_selected_tiles_empty(crate::codestream::selection_geometry_view(size), explicit_tiles@, region@); }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant tile <= tile_count,
        crate::specs::part2_selected_tiles_prefix(crate::codestream::selection_geometry_view(size),
            explicit_tiles@, region@, tile as nat) == Ok(tiles@),
        decreases tile_count - tile,
    ))]
    while tile < tile_count {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_selected_tiles_step(crate::codestream::selection_geometry_view(size), explicit_tiles@, region@, tile as nat);
            crate::proofs::production_impl::production_selected_tiles_error(crate::codestream::selection_geometry_view(size), explicit_tiles@, region@, tile as nat + 1, tile_count as nat);
        }
        let tile_index = match u16::try_from(tile) {
            Ok(index) => index,
            Err(_) => return Err(arithmetic_overflow()),
        };
        let bounds = size.tile_bounds(tile_index)?;
        let intersects = bounds.x0 < region[2]
            && bounds.x1 > region[0]
            && bounds.y0 < region[3]
            && bounds.y1 > region[1];
        if intersects {
            if !explicit_tiles.is_empty() && !tile_list_contains(explicit_tiles, tile_index)? {
                return Err(DecodeError::InvalidSelection {
                    reason: SelectionError::RegionOutsideSelectedTiles,
                });
            }
            tiles.push(tile_index);
        }
        tile = tile.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    if tiles.is_empty() {
        return Err(DecodeError::InvalidSelection {
            reason: SelectionError::RegionOutsideSelectedTiles,
        });
    }
    Ok(tiles)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures selected_tiles_outcome(result) == crate::specs::part2_selection_explicit(crate::options::tile_selection_view(selection)),
))]
fn explicit_tiles_from_selection(selection: &TileSelection) -> Result<Vec<u16>, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part2_selection_explicit);
        reveal(crate::options::tile_selection_view);
    }
    match selection {
        TileSelection::All => {
            let tiles = Vec::new();
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(tiles@ =~= Seq::<u16>::empty());
                assert(crate::specs::part2_selection_explicit(crate::options::tile_selection_view(selection)) == Ok(tiles@));
            }
            Ok(tiles)
        }
        TileSelection::Index(index) => {
            let tiles = vec![*index];
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(tiles@ =~= seq![*index]);
                assert(crate::specs::part2_selection_explicit(crate::options::tile_selection_view(selection)) == Ok(tiles@));
            }
            Ok(tiles)
        }
        TileSelection::Set(indices) => {
            if indices.is_empty() {
                return Err(DecodeError::InvalidSelection {
                    reason: SelectionError::EmptyTileSet,
                });
            }
            let copied = indices.clone();
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(copied@ == indices@);
                assert(crate::specs::part2_selection_explicit(crate::options::tile_selection_view(selection)) == Ok(copied@));
            }
            Ok(copied)
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures validated_selection_outcome(result) == crate::specs::part2_validate_selection(
        crate::codestream::selection_geometry_view(size), crate::options::decode_selection_view(selection),
    ),
))]
fn validate_selection(
    size: &ImageSize,
    selection: &DecodeSelection,
) -> Result<ValidatedSelection, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::codestream::selection_geometry_known_fields(size);
        reveal(crate::specs::part2_validate_selection);
    }
    let image_x1 = size
        .x_origin
        .checked_add(size.width)
        .ok_or_else(arithmetic_overflow)?;
    let image_y1 = size
        .y_origin
        .checked_add(size.height)
        .ok_or_else(arithmetic_overflow)?;
    let tile_count = size.tile_count()?;
    let tiles_across = size.tiles_across;
    if tiles_across == 0 {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidImageGeometry,
        });
    }
    if tile_count > u64::from(u16::MAX) + 1 {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidImageGeometry,
        });
    }

    let explicit_tiles = explicit_tiles_from_selection(selection.tile_selection())?;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(crate::specs::part2_selection_explicit(crate::options::decode_selection_view(selection).0) == Ok(explicit_tiles@));
    }
    validate_explicit_tile_indices(&explicit_tiles, tile_count)?;
    if explicit_tiles.len() > 1 {
        validate_selection_tile_rectangle(&explicit_tiles, tiles_across)?;
    }

    let region = selection_region(
        size,
        &explicit_tiles,
        selection.region(),
        image_x1,
        image_y1,
    )?;
    let tiles = select_intersecting_tiles(size, &explicit_tiles, region, tile_count)?;
    Ok(ValidatedSelection {
        tiles,
        region,
        discard_levels: selection.discard_levels(),
    })
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn tile_decode_metadata_view<'a, 'data>(value: &TileDecodeMetadata<'a, 'data>)
        -> crate::specs::Part2TileDecodeMetadata {
        crate::specs::Part2TileDecodeMetadata { first: codestream::tile_part_owned_view(value.first), standard: value.standard, resolution_counts: value.resolution_counts@ }
    }

    pub open spec fn tile_decode_metadata_outcome<'a, 'data>(result: Result<TileDecodeMetadata<'a, 'data>, DecodeError>)
        -> Result<crate::specs::Part2TileDecodeMetadata, DecodeError> {
        match result { Err(error) => Err(error), Ok(value) => Ok(tile_decode_metadata_view(&value)) }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_optional_mct_identity(tile_mct_option(left), tile_mct_option(right)),
))]
pub(crate) fn optional_multiple_component_transforms_equal(
    left: Option<&codestream::MultipleComponentTransform>,
    right: Option<&codestream::MultipleComponentTransform>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.equals(right),
        (None, None) => true,
        _ => false,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures resolution_counts_outcome_for_tile(result) == crate::specs::part2_tile_component_resolutions(
        styles, main, components, discard,
    ),
))]
pub(crate) fn prepare_tile_component_resolutions(
    styles: &ComponentCodingStyles,
    main: CodingStyle,
    components: u16,
    discard: Option<u8>,
) -> Result<Vec<u8>, DecodeError> {
    let discard_levels = match discard {
        None => {
            if main.multiple_component_transform {
                let count = components.min(3);
                let mut component = 0_u16;
                #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant component <= count, count == if components <= 3 { components } else { 3u16 },
                        discard.is_none(), main.multiple_component_transform,
                        crate::specs::part2_full_tile_style_admission(styles, main, components, 0)
                            == crate::specs::part2_full_tile_style_admission(styles, main, components, component),
                    decreases count - component,
                ))]
                while component < count {
                    #[cfg(verus_keep_ghost)]
                    proof! { reveal_with_fuel(crate::specs::part2_full_tile_style_admission, 1); }
                    let style = styles.style(component)?;
                    if !wavelet_transforms_equal(style.transform, main.transform) {
                        return Err(DecodeError::MalformedInput {
                            offset: 0,
                            kind: MalformedKind::InvalidCodingStyle,
                        });
                    }
                    component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
                }
                #[cfg(verus_keep_ghost)]
                proof! { reveal_with_fuel(crate::specs::part2_full_tile_style_admission, 1); }
            }
            return Ok(Vec::new());
        }
        Some(levels) => levels,
    };
    let mut counts = Vec::with_capacity(usize::from(components));
    let mut component = 0_u16;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant component <= components,
            discard == Some(discard_levels),
            crate::specs::part2_selected_tile_resolutions(styles, main, components, discard_levels, 0, Seq::empty())
                == crate::specs::part2_selected_tile_resolutions(styles, main, components, discard_levels, component, counts@),
        decreases components - component,
    ))]
    while component < components {
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part2_selected_tile_resolutions, 1); }
        let style = styles.style(component)?;
        if discard_levels > style.maximum_isotropic_reduction() {
            return Err(DecodeError::InvalidSelection {
                reason: SelectionError::ReductionTooLarge,
            });
        }
        if main.multiple_component_transform
            && component < 3
            && !wavelet_transforms_equal(style.transform, main.transform)
        {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidCodingStyle,
            });
        }
        let levels = match style.decomposition_levels.checked_add(1) {
            Some(levels) => levels,
            None => {
                return Err(DecodeError::InvalidSelection {
                    reason: SelectionError::ReductionTooLarge,
                });
            }
        };
        let levels = match levels.checked_sub(discard_levels) {
            Some(levels) => levels,
            None => {
                return Err(DecodeError::InvalidSelection {
                    reason: SelectionError::ReductionTooLarge,
                });
            }
        };
        counts.push(levels);
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part2_selected_tile_resolutions, 1); }
    Ok(counts)
}

#[cfg(verus_keep_ghost)]
verus! {
    pub open spec fn resolution_counts_outcome_for_tile(result: Result<Vec<u8>, DecodeError>) -> Result<Seq<u8>, DecodeError> {
        match result { Err(error) => Err(error), Ok(counts) => Ok(counts@) }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures tile_decode_metadata_outcome(result) == crate::specs::part2_tile_decode_metadata(parts@, components, discard),
))]
pub(crate) fn prepare_tile_decode_metadata<'a, 'data>(
    parts: &'a [TilePartPayload<'data>],
    components: u16,
    discard: Option<u8>,
) -> Result<TileDecodeMetadata<'a, 'data>, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(codestream::tile_part_owned_view); }
    let first = match parts.first() {
        Some(first) => first,
        None => {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidTilePart,
            });
        }
    };
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= parts.len(), parts.len() > 0, *first == parts@[0],
            forall |i: int| 0 <= i < index ==> tile_part_mct_view(&parts@[i]) == tile_part_mct_view(first),
        decreases parts.len() - index,
    ))]
    while index < parts.len() {
        let part = parts.get(index).ok_or_else(invalid_decode)?;
        if !optional_multiple_component_transforms_equal(
            part.multiple_component_transform.as_ref(),
            first.multiple_component_transform.as_ref(),
        ) {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(codestream::tile_parts_owned_view(parts@)[index as int].multiple_component_transform
                    != codestream::tile_parts_owned_view(parts@)[0].multiple_component_transform);
            }
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidTransform,
            });
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let main = first.coding_style;
    let irreversible = match main.transform {
        WaveletTransform::Irreversible9x7 => true,
        WaveletTransform::Reversible5x3 => false,
        WaveletTransform::Arbitrary(index) => {
            #[cfg(verus_keep_ghost)]
            proof! { codestream::owned_extensions_registry_known(&first.part2_extensions); }
            let kernel = match first.part2_extensions.arbitrary_kernel(index) {
                Some(kernel) => kernel,
                None => {
                    return Err(DecodeError::MalformedInput {
                        offset: 0,
                        kind: MalformedKind::MissingRequiredStructure,
                    });
                }
            };
            !kernel.reversible()
        }
    };
    let standard = (main.multiple_component_transform, irreversible);
    let resolution_counts = prepare_tile_component_resolutions(
        &first.component_coding_styles,
        main,
        components,
        discard,
    )?;
    Ok(TileDecodeMetadata {
        first,
        standard,
        resolution_counts,
    })
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn decoded_layout_tile_view(value: &DecodedLayoutTile) -> crate::specs::Part2DecodedLayoutTile {
        crate::specs::Part2DecodedLayoutTile {
            prepared: prepared_components_view(value.prepared@), counters: value.counters, mct_work: value.mct_work,
        }
    }

    pub open spec fn decoded_layout_tile_outcome(result: Result<DecodedLayoutTile, DecodeError>)
        -> Result<crate::specs::Part2DecodedLayoutTile, DecodeError> {
        match result { Err(error) => Err(error), Ok(value) => Ok(decoded_layout_tile_view(&value)) }
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures decoded_layout_tile_outcome(result) == crate::specs::part2_decode_tile_parts_at(
        size, tile, parts@, codestream::codestream_profile_capabilities(profile), discard, limits,
        prepared_components_view(prepared@), initial, mct_work,
    ),
))]
pub(crate) fn decode_tile_parts_at(
    size: &ImageSize,
    tile: u16,
    parts: &[TilePartPayload<'_>],
    profile: &codestream::CodestreamProfile,
    discard: Option<u8>,
    limits: &ResourceLimits,
    prepared: Vec<PreparedTileComponent>,
    initial: (u64, u64, u64),
    mct_work: u64,
) -> Result<DecodedLayoutTile, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { codestream::selection_geometry_known_fields(size); reveal(codestream::tile_part_owned_view); }
    let metadata = prepare_tile_decode_metadata(parts, size.components, discard)?;
    #[cfg(verus_keep_ghost)]
    proof! { reveal(tile_decode_metadata_view); }
    let first = metadata.first;
    let maximum_resolution_counts = match discard {
        None => None,
        Some(_) => Some(metadata.resolution_counts.as_slice()),
    };
    let plan = plan_progression_tile_at_selected(
        size,
        tile,
        first.coding_style,
        &first.component_coding_styles,
        &first.component_quantizations,
        &first.roi_shifts,
        discard.unwrap_or(0),
        profile.part15_allows_multiple_ht_sets(),
        profile.part15_magnitude_bound(),
        profile.part15_requires_all_ht_blocks(),
        limits,
    )?;
    let counters = account_planned_tile(&plan, maximum_resolution_counts, initial, limits)?;
    let decoded = decode_planned_tile_components(
        plan,
        parts,
        first,
        size,
        metadata.standard,
        maximum_resolution_counts,
        limits,
        prepared,
        mct_work,
    )?;
    #[cfg(verus_keep_ghost)]
    proof! { reveal(decoded_tile_components_view); reveal(decoded_layout_tile_view); }
    Ok(DecodedLayoutTile {
        prepared: decoded.prepared,
        counters,
        mct_work: decoded.mct_work,
    })
}

#[cfg(verus_keep_ghost)]
verus! {
    pub open spec fn collected_tile_parts_outcome(result: Result<Vec<TilePartPayload<'_>>, DecodeError>)
        -> Result<Seq<crate::specs::Part2OwnedTilePart>, DecodeError> {
        match result { Err(error) => Err(error), Ok(parts) => Ok(codestream::tile_parts_owned_view(parts@)) }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures collected_tile_parts_outcome(result) == crate::specs::part2_collect_tile_parts(
        codestream::tile_parts_owned_view(parts@), tile,
    ),
))]
pub(crate) fn collect_tile_parts<'data>(
    parts: &[TilePartPayload<'data>],
    tile: u16,
) -> Result<Vec<TilePartPayload<'data>>, DecodeError> {
    let mut output = Vec::new();
    #[cfg(verus_keep_ghost)]
    proof! { assert(codestream::tile_parts_owned_view(output@) =~= Seq::empty()); }
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= parts.len(),
            crate::specs::part2_collect_tile_parts(codestream::tile_parts_owned_view(parts@), tile)
                == Ok(crate::specs::part2_collect_tile_parts_run(
                    codestream::tile_parts_owned_view(parts@), tile, index as nat, codestream::tile_parts_owned_view(output@),
                )),
        decreases parts.len() - index,
    ))]
    while index < parts.len() {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost before = codestream::tile_parts_owned_view(output@); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part2_collect_tile_parts_run, 1); }
        let part = parts.get(index).ok_or_else(invalid_decode)?.clone();
        #[cfg(verus_keep_ghost)]
        proof! { codestream::tile_part_owned_tile_known(&part); }
        if part.tile_index == tile {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost copied = codestream::tile_part_owned_view(&part); }
            output.push(part);
            #[cfg(verus_keep_ghost)]
            proof! { assert(codestream::tile_parts_owned_view(output@) =~= before.push(copied)); }
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part2_collect_tile_parts_run, 1); }
    Ok(output)
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn layout_accumulation_view(
        prepared: Seq<PreparedTileComponent>, counters: (u64, u64, u64), work: u64,
    ) -> crate::specs::Part2DecodedLayoutTile {
        crate::specs::Part2DecodedLayoutTile { prepared: prepared_components_view(prepared), counters, mct_work: work }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures crate::image::decoded_image_outcome(result) == crate::specs::part2_decode_layout_selected(layout, requested, limits),
))]
fn decode_layout_selected(
    layout: &CodestreamLayout<'_>,
    requested: &DecodeSelection,
    limits: &ResourceLimits,
) -> Result<DecodedImage, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost goal = crate::specs::part2_decode_layout_selected(layout, requested, limits); }
    validate_part2_execution(layout)?;
    let selection = validate_selection(&layout.info.image_size, requested)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(validated_selection_view);
        reveal(validated_selection_outcome);
        assert(selection.region@.len() == 4);
        assert([selection.region@[0], selection.region@[1], selection.region@[2], selection.region@[3]] == selection.region);
    }
    let mut prepared = Vec::new();
    let mut total_packets = 0_u64;
    let mut total_precincts = 0_u64;
    let mut total_code_blocks = 0_u64;
    let mut total_mct_work = 0_u64;
    let mut selected_tile = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(prepared_components_view(prepared@) =~= Seq::empty());
        reveal(layout_accumulation_view);
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant selected_tile <= selection.tiles.len(),
                goal == crate::specs::part2_decode_layout_selected(layout, requested, limits),
                goal == crate::specs::part2_selected_layout_tiles(
                    &layout.info.image_size, codestream::tile_parts_owned_view(layout.tile_parts@),
                    codestream::codestream_profile_capabilities(&layout.info.profile), selection.tiles@,
                    selection.region, selection.discard_levels, limits, selected_tile as nat,
                    layout_accumulation_view(prepared@, (total_packets, total_precincts, total_code_blocks), total_mct_work),
                ),
            decreases selection.tiles.len() - selected_tile,
        )
    )]
    while selected_tile < selection.tiles.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(layout_accumulation_view);
            reveal_with_fuel(crate::specs::part2_selected_layout_tiles, 1);
        }
        let tile_index = *selection
            .tiles
            .get(selected_tile)
            .ok_or_else(invalid_decode)?;
        let tile_parts = collect_tile_parts(&layout.tile_parts, tile_index)?;
        let decoded = decode_tile_parts_at(
            &layout.info.image_size,
            tile_index,
            &tile_parts,
            &layout.info.profile,
            Some(selection.discard_levels),
            limits,
            prepared,
            (total_packets, total_precincts, total_code_blocks),
            total_mct_work,
        )?;
        prepared = decoded.prepared;
        total_packets = decoded.counters.0;
        total_precincts = decoded.counters.1;
        total_code_blocks = decoded.counters.2;
        total_mct_work = decoded.mct_work;
        selected_tile = selected_tile
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part2_selected_layout_tiles, 1); reveal(layout_accumulation_view); }
    assemble_prepared_components(
        &layout.info.image_size,
        &prepared,
        Some((selection.region, selection.discard_levels)),
        limits,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_prepared_standard_admission(prepared_components_view(prepared@), irreversible),
))]
fn validate_prepared_standard_components(
    prepared: &[PreparedTileComponent],
    irreversible: bool,
) -> Result<usize, DecodeError> {
    if prepared.len() < 3 {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidTransform,
        });
    }
    let (red, rest) = prepared.split_first().ok_or_else(invalid_decode)?;
    let (green, rest) = rest.split_first().ok_or_else(invalid_decode)?;
    let blue = rest.first().ok_or_else(invalid_decode)?;
    if (red.x, red.y, red.width, red.height) != (green.x, green.y, green.width, green.height)
        || (red.x, red.y, red.width, red.height) != (blue.x, blue.y, blue.width, blue.height)
        || red.samples.len() != green.samples.len()
        || red.samples.len() != blue.samples.len()
        || red.fraction_bits != green.fraction_bits
        || red.fraction_bits != blue.fraction_bits
        || (!irreversible && red.fraction_bits != 0)
        || (irreversible && red.fraction_bits != 16)
    {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidTransform,
        });
    }
    Ok(red.samples.len())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_prepared_sample(prepared_components_view(prepared@), component, index),
))]
fn prepared_sample(
    prepared: &[PreparedTileComponent],
    component: usize,
    index: usize,
) -> Result<i64, DecodeError> {
    let component = prepared.get(component).ok_or_else(invalid_decode)?;
    Ok(*component.samples.get(index).ok_or_else(invalid_decode)?)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, prepared_components_view(final(prepared)@)) == crate::specs::part1_prepared_store_sample(
        prepared_components_view(old(prepared)@), component, index, value),
))]
fn store_prepared_sample(
    prepared: &mut [PreparedTileComponent],
    component: usize,
    index: usize,
    value: i64,
) -> Result<(), DecodeError> {
    let component = prepared.get_mut(component).ok_or_else(invalid_decode)?;
    *component
        .samples
        .get_mut(index)
        .ok_or_else(invalid_decode)? = value;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, prepared_components_view(final(prepared)@)) == crate::specs::part1_prepared_standard_pixel(
        prepared_components_view(old(prepared)@), index, irreversible),
))]
fn apply_prepared_standard_pixel(
    prepared: &mut [PreparedTileComponent],
    index: usize,
    irreversible: bool,
) -> Result<(), DecodeError> {
    let y = prepared_sample(prepared, 0, index)?;
    let cb = prepared_sample(prepared, 1, index)?;
    let cr = prepared_sample(prepared, 2, index)?;
    let (red, green, blue) = crate::image::inverse_component_values(y, cb, cr, irreversible)?;
    store_prepared_sample(prepared, 0, index, red)?;
    store_prepared_sample(prepared, 1, index, green)?;
    store_prepared_sample(prepared, 2, index, blue)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, prepared_components_view(final(prepared)@), *final(work)) == crate::specs::part2_finish_prepared_tile(
        prepared_components_view(old(prepared)@), codestream::image_size_component_count(size), codestream::image_size_component_infos(size),
        tile_mct_option(mct), standard, *old(work), crate::limits::resource_work_limit(limits),
        crate::limits::resource_decoded_bytes_limit(limits)),
))]
fn finish_prepared_tile(
    prepared: &mut [PreparedTileComponent],
    size: &ImageSize,
    mct: Option<&codestream::MultipleComponentTransform>,
    standard: (bool, bool),
    limits: &ResourceLimits,
    work: &mut u64,
) -> Result<(), DecodeError> {
    if let Some(transform) = mct {
        apply_tile_multiple_component_transform(prepared, size, transform, limits, work)?;
    }
    apply_prepared_component_transform(prepared, standard)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, prepared_components_view(final(prepared)@), *final(work)) == crate::specs::part2_reconstruct_planned_tile(
        prepared_components_view(old(prepared)@), planned_components_view(plan), prepare_precincts_view(plan.precincts@),
        first.component_coding_styles, codestream::part2_kernel_registry(&first.part2_extensions),
        codestream::image_size_component_count(size), codestream::image_size_component_infos(size),
        tile_part_mct_view(first), standard, *old(work),
        crate::limits::resource_work_limit(limits), crate::limits::resource_decoded_bytes_limit(limits)),
))]
fn reconstruct_planned_tile(
    plan: &ProgressionTilePlan,
    first: &TilePartPayload<'_>,
    size: &ImageSize,
    standard: (bool, bool),
    limits: &ResourceLimits,
    prepared: &mut Vec<PreparedTileComponent>,
    work: &mut u64,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost initial = prepared_components_view(prepared@);
        let ghost plans = planned_components_view(plan);
        let ghost precincts = prepare_precincts_view(plan.precincts@);
        let ghost kernels = codestream::part2_kernel_registry(&first.part2_extensions);
        let ghost initial_work = *work;
    }
    let start = prepared.len();
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_reconstruct_components_empty(
            initial, plans, precincts, first.component_coding_styles, kernels, crate::limits::resource_decoded_bytes_limit(limits));
    }
    let mut component_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant component_index <= plan.components.len(), plans == planned_components_view(plan),
        precincts == prepare_precincts_view(plan.precincts@),
        kernels == codestream::part2_kernel_registry(&first.part2_extensions),
        initial == prepared_components_view(old(prepared)@), initial_work == *old(work), *work == initial_work,
        start == initial.len(), start <= prepared.len(),
        prepared_components_view(prepared@).take(start as int) == initial,
        crate::specs::part2_reconstruct_components_prefix(initial, plans, precincts, first.component_coding_styles,
            kernels, crate::limits::resource_decoded_bytes_limit(limits), component_index as nat)
                == (Ok(()), prepared_components_view(prepared@)),
        decreases plan.components.len() - component_index,
    ))]
    while component_index < plan.components.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_reconstruct_components_step(initial, plans, precincts,
                first.component_coding_styles, kernels, crate::limits::resource_decoded_bytes_limit(limits), component_index as nat);
            crate::proofs::production_impl::production_reconstruct_components_error(initial, plans, precincts,
                first.component_coding_styles, kernels, crate::limits::resource_decoded_bytes_limit(limits), component_index as nat + 1, plans.len());
        }
        let planned = plan
            .components
            .get(component_index)
            .ok_or_else(invalid_decode)?;
        let style = first.component_coding_styles.style(planned.component)?;
        let kernel = match style.transform {
            WaveletTransform::Arbitrary(index) => first.part2_extensions.arbitrary_kernel(index),
            _ => None,
        };
        prepared.push(prepare_tile_component_with_kernel(
            planned,
            &plan.precincts,
            style,
            kernel,
            limits,
        )?);
        component_index = component_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost folded = prepared_components_view(prepared@); }
    let suffix = prepared.get_mut(start..).ok_or_else(invalid_decode)?;
    #[cfg(verus_keep_ghost)]
    proof! { assert(prepared_components_view(suffix@) =~= folded.skip(start as int)); }
    let result = finish_prepared_tile(
        suffix,
        size,
        first.multiple_component_transform.as_ref(),
        standard,
        limits,
        work,
    );
    #[cfg(verus_keep_ghost)]
    proof! {
        let suffix_values = prepared_components_view(suffix@);
        assert(prepared_components_view(prepared@).skip(start as int) =~= suffix_values);
        let finished = crate::specs::part2_finish_prepared_tile(folded.skip(start as int),
            codestream::image_size_component_count(size), codestream::image_size_component_infos(size),
            tile_part_mct_view(first), standard, initial_work,
            crate::limits::resource_work_limit(limits), crate::limits::resource_decoded_bytes_limit(limits));
        assert((result, prepared_components_view(prepared@).skip(start as int), *work) == finished);
        assert(prepared_components_view(prepared@) =~= initial + finished.1);
    }
    result
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, prepared_components_view(final(prepared)@)) == crate::specs::part1_prepared_standard_transform(
        prepared_components_view(old(prepared)@), transform),
))]
fn apply_prepared_component_transform(
    prepared: &mut [PreparedTileComponent],
    transform: (bool, bool),
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost before = prepared_components_view(prepared@); }
    if !transform.0 {
        return Ok(());
    }
    let length = validate_prepared_standard_components(prepared, transform.1)?;
    #[cfg(verus_keep_ghost)]
    proof! { crate::proofs::production_impl::production_prepared_standard_empty(before, transform.1); }
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= length, transform.0,
            before == prepared_components_view(old(prepared)@),
            crate::specs::part1_prepared_standard_admission(before, transform.1) == Ok(length),
            crate::specs::part1_prepared_standard_prefix(before, transform.1, index as nat) == (Ok(()), prepared_components_view(prepared@)),
            decreases length - index,
        )
    )]
    while index < length {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_prepared_standard_step(before, transform.1, index as nat);
            crate::proofs::production_impl::production_prepared_standard_error(before, transform.1, index as nat + 1, length as nat);
        }
        apply_prepared_standard_pixel(prepared, index, transform.1)?;
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg(verus_keep_ghost)]
verus! {
    pub(crate) closed spec fn execution_header_view(styles: ComponentCodingStyles, extensions: &codestream::Part2Extensions)
        -> crate::specs::Part2ExecutionHeader
    {
        (styles, codestream::part2_execution_registry(extensions), codestream::part2_kernel_registry(extensions),
            codestream::part2_component_depths(extensions))
    }

    pub(crate) closed spec fn execution_tile_headers(parts: Seq<TilePartPayload<'_>>) -> Seq<crate::specs::Part2ExecutionHeader> {
        Seq::new(parts.len(), |index: int|
            execution_header_view(parts[index].component_coding_styles, &parts[index].part2_extensions))
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::part2_validate_execution_component(
            execution_header_view(*styles, extensions), component),
    )
)]
fn validate_part2_component(
    styles: &ComponentCodingStyles,
    extensions: &codestream::Part2Extensions,
    component: u16,
) -> Result<(), DecodeError> {
    validate_part2_coding_style(styles.style(component)?, extensions)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::part2_execution_components_prefix(
            execution_header_view(*styles, extensions), count as nat),
    )
)]
fn validate_part2_components(
    styles: &ComponentCodingStyles,
    extensions: &codestream::Part2Extensions,
    count: u16,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost header = execution_header_view(*styles, extensions); }
    #[cfg(verus_keep_ghost)]
    proof! { crate::proofs::production_impl::production_execution_components_empty(header); }
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= count, header == execution_header_view(*styles, extensions),
                crate::specs::part2_execution_components_prefix(header, component as nat) == Ok(()),
            decreases count - component,
        )
    )]
    while component < count {
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_execution_components_step(header, component as nat); }
        let result = validate_part2_component(styles, extensions, component);
        #[cfg(verus_keep_ghost)]
        proof! {
            if let Err(error) = result {
                assert(crate::specs::part2_execution_components_prefix(header, component as nat + 1) == Err(error));
                crate::proofs::production_impl::production_execution_components_error(header, component as nat + 1, count as nat);
            } else { crate::proofs::production_impl::production_mct_unit_result(result); }
        }
        result?;
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::part2_validate_execution_tile(
            execution_header_view(part.component_coding_styles, &part.part2_extensions),
            codestream::part2_component_depths(main), count),
    )
)]
fn validate_part2_tile_execution(
    part: &TilePartPayload<'_>,
    main: &codestream::Part2Extensions,
    count: u16,
) -> Result<(), DecodeError> {
    part.part2_extensions.validate_executable()?;
    validate_part2_components(&part.component_coding_styles, &part.part2_extensions, count)?;
    if !part.part2_extensions.component_bit_depths().is_empty()
        && !part.part2_extensions.component_bit_depths_equal(main)
    {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidMarkerOrder,
        });
    }
    Ok(())
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::part2_validate_execution_layout(
            layout.info.image_size.components,
            execution_header_view(layout.info.component_coding_styles, &layout.info.part2_extensions),
            execution_tile_headers(layout.tile_parts@)),
    )
)]
pub(crate) fn validate_part2_execution(layout: &CodestreamLayout<'_>) -> Result<(), DecodeError> {
    layout.info.part2_extensions.validate_executable()?;
    validate_part2_components(
        &layout.info.component_coding_styles,
        &layout.info.part2_extensions,
        layout.info.image_size.components,
    )?;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_execution_tiles_empty(
            execution_tile_headers(layout.tile_parts@), codestream::part2_component_depths(&layout.info.part2_extensions),
            layout.info.image_size.components);
    }
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= layout.tile_parts.len(),
                crate::specs::part2_execution_components_prefix(
                    execution_header_view(layout.info.component_coding_styles, &layout.info.part2_extensions),
                    layout.info.image_size.components as nat) == Ok(()),
                crate::specs::part2_execution_tiles_prefix(execution_tile_headers(layout.tile_parts@),
                    codestream::part2_component_depths(&layout.info.part2_extensions),
                    layout.info.image_size.components, index as nat) == Ok(()),
            decreases layout.tile_parts.len() - index,
        )
    )]
    while index < layout.tile_parts.len() {
        let tile_part = layout
            .tile_parts
            .get(index)
            .ok_or_else(arithmetic_overflow)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_execution_tiles_step(
                execution_tile_headers(layout.tile_parts@), codestream::part2_component_depths(&layout.info.part2_extensions),
                layout.info.image_size.components, index as nat);
        }
        let result = validate_part2_tile_execution(
            tile_part,
            &layout.info.part2_extensions,
            layout.info.image_size.components,
        );
        #[cfg(verus_keep_ghost)]
        proof! {
            if let Err(error) = result {
                assert(crate::specs::part2_execution_tiles_prefix(execution_tile_headers(layout.tile_parts@),
                    codestream::part2_component_depths(&layout.info.part2_extensions),
                    layout.info.image_size.components, index as nat + 1) == Err(error));
                crate::proofs::production_impl::production_execution_tiles_error(
                    execution_tile_headers(layout.tile_parts@), codestream::part2_component_depths(&layout.info.part2_extensions),
                    layout.info.image_size.components, index as nat + 1, layout.tile_parts@.len());
            } else { crate::proofs::production_impl::production_mct_unit_result(result); }
        }
        result?;
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::part2_validate_execution_style(
            style, codestream::part2_execution_registry(extensions), codestream::part2_kernel_registry(extensions)),
    )
)]
pub(crate) fn validate_part2_coding_style(
    style: CodingStyle,
    extensions: &crate::codestream::Part2Extensions,
) -> Result<(), DecodeError> {
    let missing_decomposition = match style.decomposition_structure {
        Some(index) => {
            extensions.decomposition_structure(index).is_none()
                && extensions.arbitrary_decomposition(index).is_none()
        }
        None => false,
    };
    if missing_decomposition {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::MissingRequiredStructure,
        });
    }
    let missing_kernel = match style.transform {
        WaveletTransform::Arbitrary(index) => extensions.arbitrary_kernel(index).is_none(),
        _ => false,
    };
    if missing_kernel {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::MissingRequiredStructure,
        });
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_selected_component_blocks(prepare_plan_view(plan), 0, total),
))]
pub(crate) fn count_selected_component_blocks(
    plan: &PlannedTileComponent,
    total: u64,
) -> Result<u64, DecodeError> {
    let mut count = total;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= plan.placements.len(),
            crate::specs::part2_selected_component_blocks(prepare_plan_view(plan), 0, total)
                == crate::specs::part2_selected_component_blocks(prepare_plan_view(plan), index as nat, count),
        decreases plan.placements.len() - index,
    ))]
    while index < plan.placements.len() {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(prepare_plan_view); reveal_with_fuel(crate::specs::part2_selected_component_blocks, 1); }
        let placement = plan.placements.get(index).ok_or_else(invalid_decode)?;
        let band = plan
            .subbands
            .get(placement.subband_index)
            .ok_or_else(invalid_decode)?;
        if band.resolution <= plan.output_resolution {
            count = count.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(prepare_plan_view); reveal_with_fuel(crate::specs::part2_selected_component_blocks, 1); }
    Ok(count)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_planned_tile_accounting(
        progression_plan_view(plan), crate::packet::packet_selection_view(maximum_resolution_counts), initial, limits,
    ),
))]
pub(crate) fn account_planned_tile(
    plan: &ProgressionTilePlan,
    maximum_resolution_counts: Option<&[u8]>,
    initial: (u64, u64, u64),
    limits: &ResourceLimits,
) -> Result<(u64, u64, u64), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(progression_plan_view); }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost goal = crate::specs::part2_planned_tile_accounting(
        progression_plan_view(plan), crate::packet::packet_selection_view(maximum_resolution_counts), initial, limits,
    ); }
    let selected = maximum_resolution_counts.is_some();
    let packets = match maximum_resolution_counts {
        None => plan.grid.packet_count(),
        Some(counts) => plan.grid.selected_packet_count(counts)?,
    };
    let total_packets = initial
        .0
        .checked_add(packets)
        .ok_or_else(arithmetic_overflow)?;
    if total_packets > limits.maximum_packets() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::Packets,
            limit: limits.maximum_packets(),
            actual: total_packets,
        });
    }
    let precincts = if selected {
        packets
            .checked_div(u64::from(plan.coding_style.layers))
            .ok_or_else(arithmetic_overflow)?
    } else {
        plan.grid.total_precinct_count()?
    };
    let total_precincts = initial
        .1
        .checked_add(precincts)
        .ok_or_else(arithmetic_overflow)?;
    if total_precincts > limits.maximum_precincts() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::Precincts,
            limit: limits.maximum_precincts(),
            actual: total_precincts,
        });
    }
    let mut total_blocks = initial.2;
    let mut component = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant component <= plan.components.len(),
            goal == crate::specs::part2_planned_tile_accounting(
                progression_plan_view(plan), crate::packet::packet_selection_view(maximum_resolution_counts), initial, limits,
            ),
            goal == match crate::specs::part2_account_plan_blocks(
                planning_components_view(plan.components@), selected, crate::limits::resource_code_block_limit(limits),
                component as nat, total_blocks,
            ) {
                Err(error) => Err(error),
                Ok(blocks) => Ok((total_packets, total_precincts, blocks)),
            },
        decreases plan.components.len() - component,
    ))]
    while component < plan.components.len() {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(progression_plan_view); reveal_with_fuel(crate::specs::part2_account_plan_blocks, 1); }
        let planned = plan.components.get(component).ok_or_else(invalid_decode)?;
        if selected {
            total_blocks = count_selected_component_blocks(planned, total_blocks)?;
        } else {
            total_blocks = total_blocks
                .checked_add(planned.placements.len() as u64)
                .ok_or_else(arithmetic_overflow)?;
            if total_blocks > limits.maximum_code_blocks() {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::CodeBlocks,
                    limit: limits.maximum_code_blocks(),
                    actual: total_blocks,
                });
            }
        }
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part2_account_plan_blocks, 1); }
    if selected && total_blocks > limits.maximum_code_blocks() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::CodeBlocks,
            limit: limits.maximum_code_blocks(),
            actual: total_blocks,
        });
    }
    Ok((total_packets, total_precincts, total_blocks))
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures decoded_tile_components_outcome(result) == crate::specs::part2_decode_planned_tile_components(
        progression_plan_view(&plan), tile_parts@, first, size, standard,
        crate::packet::packet_selection_view(maximum_resolution_counts), limits, prepared_components_view(prepared@), mct_work,
    ),
))]
pub(crate) fn decode_planned_tile_components(
    plan: ProgressionTilePlan,
    tile_parts: &[TilePartPayload<'_>],
    first: &TilePartPayload<'_>,
    size: &ImageSize,
    standard: (bool, bool),
    maximum_resolution_counts: Option<&[u8]>,
    limits: &ResourceLimits,
    prepared: Vec<PreparedTileComponent>,
    mct_work: u64,
) -> Result<DecodedTileComponents, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(codestream::tile_part_owned_view);
        codestream::owned_extensions_registry_known(&first.part2_extensions);
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost before = progression_plan_view(&plan);
        let ghost execution = crate::specs::part1_progression_tile_decode(
            tile_parts@, before.grid, before.precincts,
            crate::packet::packet_selection_view(maximum_resolution_counts), limits,
        );
    }
    let mut active = plan;
    #[cfg(verus_keep_ghost)]
    proof! { reveal(progression_plan_view); }
    {
        let precincts = active.precincts.as_mut_slice();
        #[cfg(verus_keep_ghost)]
        proof! { crate::packet::packet_precinct_vector_known_fields(precincts); }
        match maximum_resolution_counts {
            None => crate::packet::decode_progression_tile_packets(
                tile_parts,
                &active.grid,
                precincts,
                limits,
            ),
            Some(counts) => crate::packet::decode_progression_tile_packets_selected(
                tile_parts,
                &active.grid,
                precincts,
                Some(counts),
                limits,
            ),
        }?;
        #[cfg(verus_keep_ghost)]
        proof! { crate::packet::packet_precinct_vector_known_fields(precincts); }
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        planning_precinct_planes_known_fields(active.precincts@);
        reveal(planned_components_view);
        assert(planning_precincts_view(active.precincts@) == execution.1);
        assert(execution.0.is_ok());
        assert(planned_components_view(&active) =~= before.components);
        assert(prepare_precincts_view(active.precincts@) == crate::specs::part2_precinct_decoder_planes(execution.1));
    }
    let mut components = prepared;
    let mut work = mct_work;
    reconstruct_planned_tile(
        &active,
        first,
        size,
        standard,
        limits,
        &mut components,
        &mut work,
    )?;
    #[cfg(verus_keep_ghost)]
    proof! { reveal(decoded_tile_components_view); }
    Ok(DecodedTileComponents {
        prepared: components,
        mct_work: work,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures crate::image::decoded_image_outcome(result) == crate::specs::part2_decode_layout(layout, limits),
))]
fn decode_layout(
    layout: &CodestreamLayout<'_>,
    limits: &ResourceLimits,
) -> Result<DecodedImage, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost goal = crate::specs::part2_decode_layout(layout, limits); }
    validate_part2_execution(layout)?;
    let tile_count = layout.info.image_size.tile_count()?;
    if tile_count > u64::from(u16::MAX) + 1 {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidImageGeometry,
        });
    }
    let mut prepared = Vec::new();
    let mut total_packets = 0_u64;
    let mut total_precincts = 0_u64;
    let mut total_code_blocks = 0_u64;
    let mut total_mct_work = 0_u64;
    let mut tile = 0_u64;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(prepared_components_view(prepared@) =~= Seq::empty());
        reveal(layout_accumulation_view);
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant tile <= tile_count,
                tile_count <= u16::MAX as u64 + 1,
                goal == crate::specs::part2_decode_layout(layout, limits),
                goal == crate::specs::part2_full_layout_tiles(
                    &layout.info.image_size, codestream::tile_parts_owned_view(layout.tile_parts@),
                    codestream::codestream_profile_capabilities(&layout.info.profile), limits, tile_count, tile,
                    layout_accumulation_view(prepared@, (total_packets, total_precincts, total_code_blocks), total_mct_work),
                ),
            decreases tile_count - tile,
        )
    )]
    while tile < tile_count {
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part2_full_layout_tiles, 1); reveal(layout_accumulation_view); }
        let tile_index = match u16::try_from(tile) {
            Ok(index) => index,
            Err(_) => return Err(arithmetic_overflow()),
        };
        let tile_parts = collect_tile_parts(&layout.tile_parts, tile_index)?;
        let decoded = decode_tile_parts_at(
            &layout.info.image_size,
            tile_index,
            &tile_parts,
            &layout.info.profile,
            None,
            limits,
            prepared,
            (total_packets, total_precincts, total_code_blocks),
            total_mct_work,
        )?;
        prepared = decoded.prepared;
        total_packets = decoded.counters.0;
        total_precincts = decoded.counters.1;
        total_code_blocks = decoded.counters.2;
        total_mct_work = decoded.mct_work;
        tile = tile.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part2_full_layout_tiles, 1); reveal(layout_accumulation_view); }
    assemble_prepared_components(&layout.info.image_size, &prepared, None, limits)
}

impl DecodedCodeBlock {
    pub const fn width(&self) -> u16 {
        self.width
    }
}

impl DecodedSubband {
    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn fraction_bits(&self) -> u8 {
        self.fraction_bits
    }

    pub fn samples(&self) -> &[i64] {
        &self.samples
    }
}

pub fn decode_subband(
    width: u32,
    height: u32,
    blocks: &[PlacedCodeBlock<'_>],
    limits: &ResourceLimits,
) -> Result<DecodedSubband, DecodeError> {
    if width == 0 || height == 0 || blocks.is_empty() {
        return Err(invalid_decode());
    }
    let length = usize::try_from(width)
        .map_err(|_| arithmetic_overflow())?
        .checked_mul(usize::try_from(height).map_err(|_| arithmetic_overflow())?)
        .ok_or_else(arithmetic_overflow)?;
    let bytes = (length as u64)
        .checked_mul(8)
        .ok_or_else(arithmetic_overflow)?;
    if bytes > limits.maximum_decoded_bytes() {
        return Err(DecodeError::limit(
            crate::Resource::DecodedBytes,
            limits.maximum_decoded_bytes(),
            bytes,
        ));
    }
    let mut samples = vec![0_i64; length];
    let mut visited = vec![false; length];
    let mut fraction_bits = None;
    let mut block_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant block_index <= blocks.len(),
            decreases blocks.len() - block_index,
        )
    )]
    while block_index < blocks.len() {
        let placed = blocks[block_index];
        let block = decode_code_block(placed.request, limits)?;
        if fraction_bits.is_some() && fraction_bits != Some(block.fraction_bits()) {
            return Err(invalid_decode());
        }
        fraction_bits = Some(block.fraction_bits());
        let block_width = u32::from(block.width());
        let block_height = u32::from(block.height());
        if placed
            .x
            .checked_add(block_width)
            .ok_or_else(arithmetic_overflow)?
            > width
            || placed
                .y
                .checked_add(block_height)
                .ok_or_else(arithmetic_overflow)?
                > height
        {
            return Err(invalid_decode());
        }
        let mut y = 0_u32;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant y <= block_height,
                decreases block_height - y,
            )
        )]
        while y < block_height {
            let mut x = 0_u32;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant x <= block_width,
                    decreases block_width - x,
                )
            )]
            while x < block_width {
                let destination = checked_index(
                    width,
                    placed.x.checked_add(x).ok_or_else(arithmetic_overflow)?,
                    placed.y.checked_add(y).ok_or_else(arithmetic_overflow)?,
                )?;
                let source = checked_index(block_width, x, y)?;
                if destination >= samples.len()
                    || destination >= visited.len()
                    || source >= block.samples().len()
                    || visited[destination]
                {
                    return Err(invalid_decode());
                }
                let sample = block.samples().get(source).ok_or_else(invalid_decode)?;
                samples[destination] = *sample;
                visited[destination] = true;
                x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        block_index = block_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let mut sample_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant sample_index <= visited.len(),
            decreases visited.len() - sample_index,
        )
    )]
    while sample_index < visited.len() {
        if !visited[sample_index] {
            return Err(invalid_decode());
        }
        sample_index = sample_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    Ok(DecodedSubband {
        width,
        height,
        fraction_bits: fraction_bits.ok_or_else(invalid_decode)?,
        samples,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures crate::image::decoded_image_outcome(result) == crate::specs::part1_decode_prepared_image(
        crate::image::assembly_size_view(size), crate::image::assembly_tiles_view(tile_components@),
        component_transform, crate::limits::resource_decoded_bytes_limit(limits)),
))]
pub fn decode_prepared_image(
    size: &ImageSize,
    tile_components: &[TileComponent<'_>],
    component_transform: ComponentTransform,
    limits: &ResourceLimits,
) -> Result<DecodedImage, DecodeError> {
    let mut image = assemble_image(size, tile_components, limits)?;
    match component_transform {
        ComponentTransform::None => {}
        ComponentTransform::Reversible => {
            apply_reversible_component_transform(&mut image)?;
        }
        ComponentTransform::Irreversible => {
            apply_irreversible_component_transform(&mut image)?;
        }
    }
    Ok(image)
}

impl DecodedCodeBlock {
    pub const fn height(&self) -> u16 {
        self.height
    }

    pub const fn fraction_bits(&self) -> u8 {
        self.fraction_bits
    }

    pub fn samples(&self) -> &[i64] {
        &self.samples
    }
}

pub fn decode_code_block(
    request: CodeBlockDecodeRequest<'_>,
    limits: &ResourceLimits,
) -> Result<DecodedCodeBlock, DecodeError> {
    if request.layers.is_empty() {
        return Err(invalid_decode());
    }
    let mut decoder = Tier1CodeBlockDecoder::new(
        request.width,
        request.height,
        request.significant_bitplanes,
        request.orientation,
        request.style,
    )?;
    let mut layer_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant layer_index <= request.layers.len(),
            decreases request.layers.len() - layer_index,
        )
    )]
    while layer_index < request.layers.len() {
        let layer = request.layers[layer_index];
        decode_code_block_contribution_body(&mut decoder, layer.contribution, layer.body, limits)?;
        layer_index = layer_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    if !decoder.is_complete() {
        return Err(invalid_decode());
    }
    let coefficients = decoder.plane().coefficients();
    let expected = usize::from(request.width)
        .checked_mul(usize::from(request.height))
        .ok_or_else(arithmetic_overflow)?;
    if coefficients.len() != expected {
        return Err(invalid_decode());
    }
    let mut signed = Vec::with_capacity(coefficients.len());
    let mut coefficient_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant coefficient_index <= coefficients.len(),
            decreases coefficients.len() - coefficient_index,
        )
    )]
    while coefficient_index < coefficients.len() {
        let coefficient = coefficients[coefficient_index];
        signed.push(coefficient.signed_magnitude()?);
        coefficient_index = coefficient_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    let quantized = inverse_quantize_magnitudes(
        &signed,
        request.quantization_step,
        request.dynamic_range,
        request.reversible,
        request.roi_shift,
        limits,
    )?;
    let quantized_count = quantized.samples().len();
    let mut samples = Vec::with_capacity(quantized_count);
    let mut sample_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant sample_index <= quantized_count,
            decreases quantized_count - sample_index,
        )
    )]
    while sample_index < quantized_count {
        let sample = quantized
            .samples()
            .get(sample_index)
            .ok_or_else(invalid_decode)?;
        samples.push(*sample);
        sample_index = sample_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    Ok(DecodedCodeBlock {
        width: request.width,
        height: request.height,
        fraction_bits: quantized.fraction_bits(),
        samples,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_component_coordinate_bounds(
        codestream::selection_geometry_view(size), codestream::image_size_component_count(size),
        codestream::image_size_component_infos(size), component, tile_index,
    ),
))]
pub(crate) fn component_coordinate_bounds(
    size: &ImageSize,
    component: u16,
    tile_index: Option<u16>,
) -> Result<[u32; 4], DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        codestream::selection_geometry_known_fields(size);
    }
    if let Some(tile_index) = tile_index {
        let bounds = size.tile_component_bounds(tile_index, component)?;
        return Ok([bounds.x0, bounds.y0, bounds.x1, bounds.y1]);
    }
    let info = size.component(component)?;
    let image_x1 = size
        .x_origin
        .checked_add(size.width)
        .ok_or_else(arithmetic_overflow)?;
    let image_y1 = size
        .y_origin
        .checked_add(size.height)
        .ok_or_else(arithmetic_overflow)?;
    let x0 = ceil_div_u32(size.x_origin, u32::from(info.x_subsampling))?;
    let y0 = ceil_div_u32(size.y_origin, u32::from(info.y_subsampling))?;
    let x1 = ceil_div_u32(image_x1, u32::from(info.x_subsampling))?;
    let y1 = ceil_div_u32(image_y1, u32::from(info.y_subsampling))?;
    Ok([x0, y0, x1, y1])
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(error =>
        ensures error == crate::specs::part1_block_placement_error(false),
            error == crate::specs::color_selection_target_error(),
    )
)]
fn invalid_decode() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidCodeBlock,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_decode_wavelet_agreement(left, right),
))]
const fn wavelet_transforms_equal(left: WaveletTransform, right: WaveletTransform) -> bool {
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
fn arithmetic_overflow() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_offset(y, width, x),
))]
fn checked_index(width: u32, x: u32, y: u32) -> Result<usize, DecodeError> {
    crate::transform::transform_offset(y, width, x)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_area(width, height),
))]
fn checked_area_u32(width: u32, height: u32) -> Result<usize, DecodeError> {
    crate::transform::checked_area(width, height)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_selection_ceil(value, divisor),
))]
fn ceil_div_u32(value: u32, divisor: u32) -> Result<u32, DecodeError> {
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
fn floor_div_u32(value: u32, divisor: u32) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(invalid_decode());
    }
    Ok(value / divisor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codestream::{ProgressionOrder, parse_layout};
    use crate::packet::{
        CodeBlockPacketContext, CodeBlockPacketState, PacketBitReader, TagTree,
        decode_code_block_contribution, decode_progression_tile_packets,
    };

    fn reference_planned_block_geometry(
        grid: [u32; 4],
        clip: [u32; 4],
        block: [u32; 2],
    ) -> Result<(u32, u32, u16, u16), DecodeError> {
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let checked = |value: u128| u32::try_from(value).map_err(|_| arithmetic);
        let ix = checked(u128::from(grid[0]) + u128::from(block[0]))?;
        let nx = checked(u128::from(ix) * u128::from(grid[2]))?;
        let iy = checked(u128::from(grid[1]) + u128::from(block[1]))?;
        let ny = checked(u128::from(iy) * u128::from(grid[3]))?;
        let x = nx.max(clip[0]);
        let y = ny.max(clip[1]);
        let ex = checked(u128::from(nx) + u128::from(grid[2]))?;
        let ey = checked(u128::from(ny) + u128::from(grid[3]))?;
        let width =
            u16::try_from(i64::from(ex.min(clip[2])) - i64::from(x)).map_err(|_| arithmetic)?;
        let height =
            u16::try_from(i64::from(ey.min(clip[3])) - i64::from(y)).map_err(|_| arithmetic)?;
        Ok((x, y, width, height))
    }

    #[test]
    fn actual_planned_block_geometry_preserves_raw_lattice_clipping_and_u16_frontiers() {
        let words = [0, 1, 2, 65_535, 65_536, u32::MAX];
        let clips = [
            [0, 0, u32::MAX, u32::MAX],
            [0, 0, 0, 0],
            [1, 1, 65_535, 65_535],
            [u32::MAX, u32::MAX, 0, 0],
            [5, 7, 3, 1],
            [1, 3, 65_536, 65_537],
            [17, 31, 33, 65],
            [1, 1, u32::MAX, u32::MAX],
        ];
        let mut cases = 0;
        let mut zero_dimensions = 0;
        for first_x in words {
            for first_y in words {
                for width in words {
                    for height in words {
                        for block_x in [0, 1, 2, u32::MAX] {
                            for block_y in [0, 1, 2, u32::MAX] {
                                for clip in clips {
                                    let grid = [first_x, first_y, width, height];
                                    let block = [block_x, block_y];
                                    let expected =
                                        reference_planned_block_geometry(grid, clip, block);
                                    assert_eq!(
                                        derive_planned_codeblock_geometry(grid, clip, block),
                                        expected
                                    );
                                    if let Ok((_, _, width, height)) = expected {
                                        zero_dimensions += usize::from(width == 0 || height == 0);
                                    }
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 165_888);
        assert!(
            zero_dimensions > 100,
            "zero dimensions remain the decoder initializer's policy"
        );
    }

    #[test]
    fn actual_precinct_origins_preserve_raw_divisor_addition_and_multiplication_frontiers() {
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let words = [0, 1, 2, 65_535, 65_536, u32::MAX];
        let mut cases = 0;
        for x in words {
            for y in words {
                for across in words {
                    for precinct in words {
                        for width in words {
                            for height in words {
                                let expected = if across == 0 {
                                    Err(arithmetic)
                                } else {
                                    let checked =
                                        |value: u128| u32::try_from(value).map_err(|_| arithmetic);
                                    (|| {
                                        let x =
                                            checked(u128::from(x) + u128::from(precinct % across))?;
                                        let y =
                                            checked(u128::from(y) + u128::from(precinct / across))?;
                                        let bx = checked(u128::from(x) * u128::from(width))?;
                                        let by = checked(u128::from(y) * u128::from(height))?;
                                        Ok([bx, by])
                                    })()
                                };
                                assert_eq!(
                                    derive_planned_precinct_origin(
                                        [x, y],
                                        across,
                                        precinct,
                                        [width, height]
                                    ),
                                    expected
                                );
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 46_656);
    }

    fn raw_planned_subband(
        x0: u32,
        y0: u32,
        bits: u16,
        orientation: SubbandOrientation,
    ) -> SubbandGeometry {
        SubbandGeometry {
            resolution: u8::MAX,
            orientation,
            x0,
            y0,
            width: u32::MAX,
            height: 0,
            resolution_width: 0,
            resolution_height: u32::MAX,
            quantization_index: u16::MAX,
            quantization_step: QuantizationStep {
                exponent: u8::MAX,
                mantissa: u16::MAX,
            },
            dynamic_range: u8::MAX,
            significant_bitplanes: bits,
            roi_shift: u8::MAX,
            low_pass_first_x: true,
            low_pass_first_y: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn reference_planned_cell(
        grid: [u32; 4],
        clip: [u32; 4],
        block: [u32; 2],
        subband: SubbandGeometry,
        style: Tier1CodingStyle,
        policy: (bool, u8),
        indices: (usize, usize),
        mut state: PlannedCodeBlocks,
    ) -> Result<PlannedCodeBlocks, DecodeError> {
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let (x, y, width, height) = reference_planned_block_geometry(grid, clip, block)?;
        let mut decoder = Tier1CodeBlockDecoder::new(
            width,
            height,
            subband.significant_bitplanes,
            subband.orientation,
            style,
        )?;
        decoder.set_multiple_ht_sets_allowed(policy.0);
        decoder.set_ht_magnitude_bound(policy.1)?;
        let x = u32::try_from(i64::from(x) - i64::from(subband.x0)).map_err(|_| arithmetic)?;
        let y = u32::try_from(i64::from(y) - i64::from(subband.y0)).map_err(|_| arithmetic)?;
        let next = usize::try_from(state.decoder_index as u128 + 1).map_err(|_| arithmetic)?;
        state.placements.push(ProgressionCodeBlockPlacement {
            precinct_index: indices.0,
            decoder_index: state.decoder_index,
            subband_index: indices.1,
            x,
            y,
        });
        state.decoders.push(decoder);
        state.decoder_index = next;
        Ok(state)
    }

    #[allow(clippy::too_many_arguments)]
    fn reference_planned_codeblocks(
        grid: [u32; 4],
        clip: [u32; 4],
        blocks: [u32; 2],
        subband: SubbandGeometry,
        style: Tier1CodingStyle,
        policy: (bool, u8),
        indices: (usize, usize, usize),
        placements: Vec<ProgressionCodeBlockPlacement>,
        total: u64,
        selected: bool,
        limits: &ResourceLimits,
    ) -> Result<PlannedCodeBlocks, DecodeError> {
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let count = u64::from(blocks[0]) * u64::from(blocks[1]);
        let total = if selected {
            let next =
                u64::try_from(u128::from(total) + u128::from(count)).map_err(|_| arithmetic)?;
            if next > limits.maximum_code_blocks() {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::CodeBlocks,
                    limit: limits.maximum_code_blocks(),
                    actual: next,
                });
            }
            if next > limits.maximum_work_units() {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::WorkUnits,
                    limit: limits.maximum_work_units(),
                    actual: next,
                });
            }
            next
        } else {
            total
        };
        usize::try_from(count).map_err(|_| arithmetic)?;
        let mut state = PlannedCodeBlocks {
            decoders: Vec::new(),
            placements,
            decoder_index: indices.2,
            planned_code_blocks: total,
        };
        for index in 0..count {
            let block = [
                u32::try_from(index % u64::from(blocks[0])).unwrap(),
                u32::try_from(index / u64::from(blocks[0])).unwrap(),
            ];
            state = reference_planned_cell(
                grid,
                clip,
                block,
                subband,
                style,
                policy,
                (indices.0, indices.1),
                state,
            )?;
        }
        Ok(state)
    }

    fn assert_planned_blocks_equal(
        actual: Result<PlannedCodeBlocks, DecodeError>,
        expected: Result<PlannedCodeBlocks, DecodeError>,
    ) -> bool {
        match (actual, expected) {
            (Err(actual), Err(expected)) => {
                assert_eq!(actual, expected);
                false
            }
            (Ok(actual), Ok(expected)) => {
                assert_eq!(actual.placements, expected.placements);
                assert_eq!(actual.decoder_index, expected.decoder_index);
                assert_eq!(actual.planned_code_blocks, expected.planned_code_blocks);
                assert_eq!(
                    format!("{:?}", actual.decoders),
                    format!("{:?}", expected.decoders)
                );
                true
            }
            (Err(error), Ok(_)) => panic!("unexpected plan error: {error:?}"),
            (Ok(_), Err(error)) => panic!("plan should have failed: {error:?}"),
        }
    }

    #[test]
    fn actual_codeblock_vectors_bind_every_decoder_placement_prefix_policy_and_selected_limit() {
        let grids = [
            [0, 0, 1, 1],
            [1, 2, 2, 3],
            [0, 0, 0, 1],
            [0, 0, 65, 64],
            [u32::MAX, 0, 2, 1],
            [0, 0, 65_536, 1],
        ];
        let mut cases = 0;
        let mut successes = 0;
        for grid in grids {
            for clip in [[0, 0, 16, 16], [1, 2, 7, 9], [0, 0, u32::MAX, u32::MAX]] {
                for blocks in [[0, 0], [0, 2], [1, 1], [2, 1], [2, 3]] {
                    for (x0, y0, bits) in [
                        (0, 0, 0),
                        (0, 0, 1),
                        (1, 2, 2),
                        (u32::MAX, u32::MAX, 74),
                        (0, 0, 74),
                        (0, 0, 75),
                    ] {
                        for orientation in [SubbandOrientation::Ll, SubbandOrientation::Hh] {
                            let subband = raw_planned_subband(x0, y0, bits, orientation);
                            let style = Tier1CodingStyle {
                                force_high_throughput: true,
                                segmentation_symbols: true,
                                block_coding: if bits == 1 {
                                    crate::tier1::Tier1BlockCoding::HighThroughput
                                } else {
                                    crate::tier1::Tier1BlockCoding::Mixed
                                },
                                ..Tier1CodingStyle::default()
                            };
                            for policy in [(false, 0), (false, 74), (true, 1), (true, 75)] {
                                for selected in [false, true] {
                                    for (total, block_limit, work_limit) in
                                        [(0, 100, 100), (0, 0, 0), (1, 2, 1), (u64::MAX, 100, 100)]
                                    {
                                        let limits = ResourceLimits::HARDENED
                                            .with_maximum_code_blocks(block_limit)
                                            .unwrap()
                                            .with_maximum_work_units(work_limit)
                                            .unwrap();
                                        for indices in [
                                            (17, 31, 0),
                                            (usize::MAX, usize::MAX, 3),
                                            (4, 7, usize::MAX),
                                        ] {
                                            let prefix = vec![ProgressionCodeBlockPlacement {
                                                precinct_index: usize::MAX,
                                                decoder_index: usize::MAX,
                                                subband_index: usize::MAX,
                                                x: u32::MAX,
                                                y: u32::MAX,
                                            }];
                                            let expected = reference_planned_codeblocks(
                                                grid,
                                                clip,
                                                blocks,
                                                subband,
                                                style,
                                                policy,
                                                indices,
                                                prefix.clone(),
                                                total,
                                                selected,
                                                &limits,
                                            );
                                            let actual = plan_codeblock_decoders(
                                                grid, clip, blocks, subband, style, policy,
                                                indices, prefix, total, selected, &limits,
                                            );
                                            successes += usize::from(assert_planned_blocks_equal(
                                                actual, expected,
                                            ));
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
        assert_eq!(cases, 103_680);
        assert!(successes > 1_000);
        for blocks in [[u32::MAX, 0], [u32::MAX, u32::MAX]] {
            let subband = raw_planned_subband(0, 0, 0, SubbandOrientation::Ll);
            let limits = ResourceLimits::HARDENED
                .with_maximum_code_blocks(0)
                .unwrap();
            let expected = reference_planned_codeblocks(
                [0; 4],
                [0; 4],
                blocks,
                subband,
                Tier1CodingStyle::default(),
                (true, 0),
                (0, 0, usize::MAX),
                Vec::new(),
                0,
                true,
                &limits,
            );
            let actual = plan_codeblock_decoders(
                [0; 4],
                [0; 4],
                blocks,
                subband,
                Tier1CodingStyle::default(),
                (true, 0),
                (0, 0, usize::MAX),
                Vec::new(),
                0,
                true,
                &limits,
            );
            assert_planned_blocks_equal(actual, expected);
        }
    }

    #[test]
    fn actual_codeblock_cell_retains_existing_owned_decoder_and_placement_buffers() {
        let mut cases = 0;
        for block in [[0, 0], [1, 2], [u32::MAX, 0], [0, u32::MAX]] {
            for origin in [0, 1, u32::MAX] {
                for bits in [0, 1, 74, 75] {
                    for flattened in [0, 1, usize::MAX - 1, usize::MAX] {
                        for bound in [0, 1, 74, 75] {
                            let style = Tier1CodingStyle::default();
                            let seed = || PlannedCodeBlocks {
                                decoders: vec![
                                    Tier1CodeBlockDecoder::new(
                                        1,
                                        1,
                                        2,
                                        SubbandOrientation::Hh,
                                        style,
                                    )
                                    .unwrap(),
                                ],
                                placements: vec![ProgressionCodeBlockPlacement {
                                    precinct_index: 19,
                                    decoder_index: usize::MAX,
                                    subband_index: 37,
                                    x: u32::MAX,
                                    y: 7,
                                }],
                                decoder_index: flattened,
                                planned_code_blocks: u64::MAX,
                            };
                            let subband =
                                raw_planned_subband(origin, origin, bits, SubbandOrientation::Lh);
                            let expected = reference_planned_cell(
                                [0, 0, 1, 1],
                                [0, 0, 5, 5],
                                block,
                                subband,
                                style,
                                (true, bound),
                                (usize::MAX, 31),
                                seed(),
                            );
                            let actual = append_planned_codeblock(
                                [0, 0, 1, 1],
                                [0, 0, 5, 5],
                                block,
                                subband,
                                style,
                                (true, bound),
                                (usize::MAX, 31),
                                seed(),
                            );
                            assert_planned_blocks_equal(actual, expected);
                            cases += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 768);
    }

    fn reference_subband_lattice(
        band: [u32; 4],
        block_size: [u32; 2],
        subband: SubbandGeometry,
    ) -> Result<Option<PlannedSubbandLattice>, DecodeError> {
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodeBlock,
        };
        let checked = |value: u128| u32::try_from(value).map_err(|_| arithmetic);
        let sx = subband.x0.max(band[0]);
        let sy = subband.y0.max(band[1]);
        let sx1 = checked(u128::from(subband.x0) + u128::from(subband.width))?;
        let sy1 = checked(u128::from(subband.y0) + u128::from(subband.height))?;
        if sx >= sx1 || sy >= sy1 {
            return Ok(None);
        }
        let ex = checked(u128::from(band[0]) + u128::from(band[2]))?.min(sx1);
        let ey = checked(u128::from(band[1]) + u128::from(band[3]))?.min(sy1);
        if block_size[0] == 0 || block_size[1] == 0 {
            return Err(invalid);
        }
        let fx = sx / block_size[0];
        let fy = sy / block_size[1];
        let across = u32::try_from(i64::from(ex.div_ceil(block_size[0])) - i64::from(fx))
            .map_err(|_| arithmetic)?;
        let down = u32::try_from(i64::from(ey.div_ceil(block_size[1])) - i64::from(fy))
            .map_err(|_| arithmetic)?;
        Ok(Some(PlannedSubbandLattice {
            grid: [fx, fy, block_size[0], block_size[1]],
            clip: [sx, sy, ex, ey],
            blocks: [across, down],
        }))
    }

    #[test]
    fn actual_subband_lattice_preserves_raw_overflow_empty_intersection_and_divisor_policy() {
        let origins = [
            (0, 0),
            (1, 2),
            (17, 31),
            (u32::MAX, 0),
            (0, u32::MAX),
            (u32::MAX, u32::MAX),
        ];
        let extents = [
            (0, 0),
            (1, 2),
            (17, 31),
            (65_535, 65_536),
            (65_536, 65_535),
            (u32::MAX, u32::MAX),
        ];
        let mut cases = 0;
        let mut empty = 0;
        let mut nonempty = 0;
        for (bx, by) in origins {
            for (bw, bh) in extents {
                for block in [
                    [0, 0],
                    [0, 1],
                    [1, 0],
                    [1, 1],
                    [2, 3],
                    [64, 65],
                    [65_535, 65_536],
                    [u32::MAX, u32::MAX],
                ] {
                    for (sx, sy) in origins {
                        for (sw, sh) in extents {
                            let mut subband =
                                raw_planned_subband(sx, sy, 0, SubbandOrientation::Hh);
                            subband.width = sw;
                            subband.height = sh;
                            let band = [bx, by, bw, bh];
                            let expected = reference_subband_lattice(band, block, subband);
                            assert_eq!(
                                derive_planned_subband_lattice(band, block, subband),
                                expected
                            );
                            empty += usize::from(matches!(expected, Ok(None)));
                            nonempty += usize::from(matches!(expected, Ok(Some(_))));
                            cases += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 10_368);
        assert!(empty > 100 && nonempty > 100);
    }

    #[allow(clippy::too_many_arguments)]
    fn reference_subband_execution(
        band: [u32; 4],
        block_size: [u32; 2],
        subband: SubbandGeometry,
        style: Tier1CodingStyle,
        policy: (bool, u8),
        indices: (usize, usize, usize),
        placements: Vec<ProgressionCodeBlockPlacement>,
        total: u64,
        selected: bool,
        limits: &ResourceLimits,
    ) -> Result<PlannedSubbandExecution, DecodeError> {
        let Some(lattice) = reference_subband_lattice(band, block_size, subband)? else {
            return Ok(PlannedSubbandExecution {
                execution: None,
                placements,
                decoder_index: indices.2,
                planned_code_blocks: total,
            });
        };
        let planned = reference_planned_codeblocks(
            lattice.grid,
            lattice.clip,
            lattice.blocks,
            subband,
            style,
            policy,
            indices,
            placements,
            total,
            selected,
            limits,
        )?;
        Ok(PlannedSubbandExecution {
            execution: Some(PacketSubbandExecution {
                orientation: subband.orientation,
                code_blocks_across: lattice.blocks[0],
                code_blocks_down: lattice.blocks[1],
                maximum_zero_bitplanes: subband.significant_bitplanes,
                decoders: planned.decoders,
            }),
            placements: planned.placements,
            decoder_index: planned.decoder_index,
            planned_code_blocks: planned.planned_code_blocks,
        })
    }

    #[test]
    fn actual_subband_execution_derives_full_packet_row_and_preserves_lazy_owned_prefix() {
        let mut cases = 0;
        let mut empty = 0;
        let mut populated = 0;
        for band in [
            [0, 0, 0, 0],
            [0, 0, 2, 3],
            [1, 2, 3, 4],
            [u32::MAX, u32::MAX, 1, 1],
            [0, 0, u32::MAX, u32::MAX],
        ] {
            for (x, y, w, h) in [
                (0, 0, 0, 0),
                (0, 0, 1, 1),
                (1, 2, 3, 2),
                (2, 1, 1, 3),
                (u32::MAX, 0, 1, 1),
                (0, u32::MAX, 1, 1),
            ] {
                for block in [[0, 1], [1, 0], [1, 1], [2, 3], [65, 64]] {
                    for bits in [0, 1, 74, 75] {
                        let mut subband = raw_planned_subband(x, y, bits, SubbandOrientation::Lh);
                        subband.width = w;
                        subband.height = h;
                        let style = Tier1CodingStyle {
                            force_high_throughput: true,
                            block_coding: crate::tier1::Tier1BlockCoding::Mixed,
                            ..Tier1CodingStyle::default()
                        };
                        for policy in [(false, 0), (false, 74), (true, 1), (true, 75)] {
                            for selected in [false, true] {
                                for (total, block_limit, work_limit) in
                                    [(0, 100, 100), (0, 0, 0), (1, 2, 1), (u64::MAX, 100, 100)]
                                {
                                    let limits = ResourceLimits::HARDENED
                                        .with_maximum_code_blocks(block_limit)
                                        .unwrap()
                                        .with_maximum_work_units(work_limit)
                                        .unwrap();
                                    for indices in [
                                        (17, 31, 0),
                                        (usize::MAX, usize::MAX, 3),
                                        (4, 7, usize::MAX),
                                    ] {
                                        let prefix = vec![ProgressionCodeBlockPlacement {
                                            precinct_index: usize::MAX,
                                            decoder_index: usize::MAX,
                                            subband_index: usize::MAX,
                                            x: u32::MAX,
                                            y: u32::MAX,
                                        }];
                                        let expected = reference_subband_execution(
                                            band,
                                            block,
                                            subband,
                                            style,
                                            policy,
                                            indices,
                                            prefix.clone(),
                                            total,
                                            selected,
                                            &limits,
                                        );
                                        let actual = plan_subband_execution(
                                            band, block, subband, style, policy, indices, prefix,
                                            total, selected, &limits,
                                        );
                                        match (actual, expected) {
                                            (Err(actual), Err(expected)) => {
                                                assert_eq!(actual, expected)
                                            }
                                            (Ok(actual), Ok(expected)) => {
                                                assert_eq!(actual.placements, expected.placements);
                                                assert_eq!(
                                                    actual.decoder_index,
                                                    expected.decoder_index
                                                );
                                                assert_eq!(
                                                    actual.planned_code_blocks,
                                                    expected.planned_code_blocks
                                                );
                                                match (actual.execution, expected.execution) {
                                                    (None, None) => empty += 1,
                                                    (Some(actual), Some(expected)) => {
                                                        assert_eq!(
                                                            actual.orientation,
                                                            expected.orientation
                                                        );
                                                        assert_eq!(
                                                            actual.code_blocks_across,
                                                            expected.code_blocks_across
                                                        );
                                                        assert_eq!(
                                                            actual.code_blocks_down,
                                                            expected.code_blocks_down
                                                        );
                                                        assert_eq!(
                                                            actual.maximum_zero_bitplanes,
                                                            expected.maximum_zero_bitplanes
                                                        );
                                                        populated += usize::from(
                                                            !actual.decoders.is_empty(),
                                                        );
                                                        assert_eq!(
                                                            format!("{:?}", actual.decoders),
                                                            format!("{:?}", expected.decoders)
                                                        );
                                                    }
                                                    _ => panic!(
                                                        "wrong presence of packet subband execution"
                                                    ),
                                                }
                                            }
                                            (Err(error), Ok(_)) => {
                                                panic!("unexpected subband error: {error:?}")
                                            }
                                            (Ok(_), Err(error)) => {
                                                panic!("subband should have failed: {error:?}")
                                            }
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
        assert_eq!(cases, 57_600);
        assert!(empty > 100 && populated > 100);
    }

    #[allow(clippy::too_many_arguments)]
    fn reference_planned_precinct(
        band: [u32; 4],
        block_size: [u32; 2],
        subbands: &[SubbandGeometry],
        range: (usize, usize),
        style: Tier1CodingStyle,
        policy: (bool, u8),
        location: (u16, u8, u32, usize),
        mut placements: Vec<ProgressionCodeBlockPlacement>,
        mut total: u64,
        selected: bool,
        limits: &ResourceLimits,
    ) -> Result<PlannedPrecinct, DecodeError> {
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodeBlock,
        };
        let mut executions = Vec::new();
        let mut flattened = 0;
        for local in 0..range.1 {
            let index = usize::try_from(range.0 as u128 + local as u128).map_err(|_| arithmetic)?;
            let subband = *subbands.get(index).ok_or(invalid)?;
            let planned = reference_subband_execution(
                band,
                block_size,
                subband,
                style,
                policy,
                (location.3, index, flattened),
                placements,
                total,
                selected,
                limits,
            )?;
            placements = planned.placements;
            flattened = planned.decoder_index;
            total = planned.planned_code_blocks;
            if let Some(execution) = planned.execution {
                executions.push(execution);
            }
        }
        let precinct = PacketPrecinctDecoder::new_subbands(
            location.0, location.1, location.2, style, executions, limits,
        )?;
        Ok(PlannedPrecinct {
            precinct,
            placements,
            planned_code_blocks: total,
        })
    }

    #[test]
    fn actual_precinct_plan_derives_all_private_packet_fields_from_raw_subband_ranges() {
        let limits = [
            ResourceLimits::HARDENED,
            ResourceLimits::HARDENED
                .with_maximum_code_blocks(0)
                .unwrap(),
            ResourceLimits::HARDENED.with_maximum_work_units(0).unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_code_blocks(2)
                .unwrap()
                .with_maximum_work_units(1)
                .unwrap(),
        ];
        let mut cases = 0;
        let mut populated = 0;
        let mut empty = 0;
        for physical in [0, 1, 3, 4] {
            for variant in 0..4 {
                let subbands = (0..physical)
                    .map(|index| {
                        let mut subband = raw_planned_subband(
                            0,
                            0,
                            1,
                            [
                                SubbandOrientation::Ll,
                                SubbandOrientation::Hl,
                                SubbandOrientation::Lh,
                                SubbandOrientation::Hh,
                            ][index],
                        );
                        match variant {
                            0 => {
                                subband.width = 1;
                                subband.height = 1;
                            }
                            1 => {
                                subband.width = 0;
                                subband.height = 1;
                                subband.significant_bitplanes = 0;
                            }
                            2 => {
                                subband.x0 = u32::try_from(index % 2).unwrap();
                                subband.width = 2;
                                subband.height = 3;
                                subband.significant_bitplanes = 74;
                            }
                            _ => {
                                subband.x0 = u32::MAX;
                                subband.width = 1;
                                subband.height = 1;
                                subband.significant_bitplanes = 75;
                            }
                        }
                        subband
                    })
                    .collect::<Vec<_>>();
                let style = Tier1CodingStyle {
                    force_high_throughput: true,
                    reset_contexts: true,
                    block_coding: crate::tier1::Tier1BlockCoding::Mixed,
                    ..Tier1CodingStyle::default()
                };
                for count in [0, 1, 3, 4, 5] {
                    for first in [0, 1, usize::MAX] {
                        for band in [[0, 0, 4, 4], [17, 31, 1, 1]] {
                            for block in [[0, 1], [1, 1], [2, 3]] {
                                for policy in [(false, 0), (true, 1)] {
                                    for selected in [false, true] {
                                        for limit in &limits {
                                            for location in [
                                                (0, 0, 0, 0),
                                                (u16::MAX, u8::MAX, u32::MAX, usize::MAX),
                                            ] {
                                                for total in [0, u64::MAX] {
                                                    let prefix =
                                                        vec![ProgressionCodeBlockPlacement {
                                                            precinct_index: 19,
                                                            decoder_index: usize::MAX,
                                                            subband_index: 37,
                                                            x: u32::MAX,
                                                            y: 7,
                                                        }];
                                                    let expected = reference_planned_precinct(
                                                        band,
                                                        block,
                                                        &subbands,
                                                        (first, count),
                                                        style,
                                                        policy,
                                                        location,
                                                        prefix.clone(),
                                                        total,
                                                        selected,
                                                        limit,
                                                    );
                                                    let actual = plan_packet_precinct(
                                                        band,
                                                        block,
                                                        &subbands,
                                                        (first, count),
                                                        style,
                                                        policy,
                                                        location,
                                                        prefix,
                                                        total,
                                                        selected,
                                                        limit,
                                                    );
                                                    match (actual, expected) {
                                                        (Err(actual), Err(expected)) => {
                                                            assert_eq!(actual, expected)
                                                        }
                                                        (Ok(actual), Ok(expected)) => {
                                                            assert_eq!(
                                                                actual.placements,
                                                                expected.placements
                                                            );
                                                            assert_eq!(
                                                                actual.planned_code_blocks,
                                                                expected.planned_code_blocks
                                                            );
                                                            let is_empty = actual
                                                                .precinct
                                                                .decoders()
                                                                .is_empty();
                                                            empty += usize::from(is_empty);
                                                            populated += usize::from(!is_empty);
                                                            assert_eq!(crate::packet::exact_packet_precinct_snapshot(&actual.precinct), crate::packet::exact_packet_precinct_snapshot(&expected.precinct));
                                                        }
                                                        (Err(error), Ok(_)) => panic!(
                                                            "unexpected precinct-plan error: {error:?}"
                                                        ),
                                                        (Ok(_), Err(error)) => panic!(
                                                            "precinct plan should have failed: {error:?}"
                                                        ),
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
            }
        }
        assert_eq!(cases, 92_160);
        assert!(empty > 100 && populated > 100);
    }

    fn planning_prefix(present: bool) -> Vec<PacketPrecinctDecoder> {
        if !present {
            return Vec::new();
        }
        let style = Tier1CodingStyle::default();
        vec![
            PacketPrecinctDecoder::new_subbands(
                19,
                u8::MAX,
                u32::MAX,
                style,
                vec![PacketSubbandExecution {
                    orientation: SubbandOrientation::Hh,
                    code_blocks_across: 1,
                    code_blocks_down: 1,
                    maximum_zero_bitplanes: 2,
                    decoders: vec![
                        Tier1CodeBlockDecoder::new(1, 1, 2, SubbandOrientation::Hh, style).unwrap(),
                    ],
                }],
                &ResourceLimits::HARDENED,
            )
            .unwrap(),
        ]
    }

    fn planning_fixture_owners<'a>(
        layout: &'a CodestreamLayout<'_>,
        tile_scope: bool,
    ) -> (
        &'a ComponentCodingStyles,
        &'a ComponentQuantizations,
        &'a RoiShifts,
    ) {
        if tile_scope {
            let tile = &layout.tile_parts[0];
            (
                &tile.component_coding_styles,
                &tile.component_quantizations,
                &tile.roi_shifts,
            )
        } else {
            (
                &layout.info.component_coding_styles,
                &layout.info.component_quantizations,
                &layout.info.roi_shifts,
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn reference_component_plan(
        size: &ImageSize,
        styles: &ComponentCodingStyles,
        quantizations: &ComponentQuantizations,
        roi: &RoiShifts,
        component: u16,
        selection: (u16, u8),
        policy: (bool, u8, bool),
        mut precincts: Vec<PacketPrecinctDecoder>,
        mut total: u64,
        limits: &ResourceLimits,
    ) -> Result<PlannedComponent, DecodeError> {
        let metadata = prepare_component_plan_metadata(
            size,
            styles,
            quantizations,
            roi,
            component,
            selection,
            policy.2,
        )?;
        let mut placements = Vec::new();
        for resolution in 0..=u16::from(metadata.component_coding_style.decomposition_levels) {
            let resolution = u8::try_from(resolution).unwrap();
            let data = prepare_resolution_plan_metadata(
                size,
                metadata.component_coding_style,
                component,
                (selection.0, resolution),
                [metadata.bounds.x0, metadata.bounds.y0],
            )?;
            for precinct in 0..data.precincts_in_resolution {
                let origin = derive_planned_precinct_origin(
                    data.first_precinct,
                    data.geometry.precincts_across,
                    precinct,
                    data.band_size,
                )?;
                let planned = reference_planned_precinct(
                    [origin[0], origin[1], data.band_size[0], data.band_size[1]],
                    data.block_size,
                    &metadata.subbands,
                    (data.first_subband, data.subband_count),
                    metadata.tier1_style,
                    (policy.0, policy.1),
                    (component, resolution, precinct, precincts.len()),
                    placements,
                    total,
                    resolution <= metadata.output_resolution,
                    limits,
                )?;
                placements = planned.placements;
                total = planned.planned_code_blocks;
                precincts.push(planned.precinct);
            }
            if resolution == u8::MAX {
                return Err(arithmetic_overflow());
            }
        }
        Ok(PlannedComponent {
            component: PlannedTileComponent {
                component,
                x: metadata.component_x,
                y: metadata.component_y,
                width: metadata.component_width,
                height: metadata.component_height,
                output_resolution: metadata.output_resolution,
                subbands: metadata.subbands,
                placements,
            },
            precincts,
            planned_code_blocks: total,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn reference_tile_plan(
        size: &ImageSize,
        coding_style: CodingStyle,
        styles: &ComponentCodingStyles,
        quantizations: &ComponentQuantizations,
        roi: &RoiShifts,
        selection: (u16, u8),
        policy: (bool, u8, bool),
        limits: &ResourceLimits,
    ) -> Result<ProgressionTilePlan, DecodeError> {
        size.tile_bounds(selection.0)?;
        let grid = derive_packet_grid_for_components(
            size,
            styles,
            coding_style.layers,
            limits,
            Some(selection.0),
            selection.1,
        )?;
        let mut precincts = Vec::new();
        let mut components = Vec::new();
        let mut total = 0;
        for component in 0..size.components {
            let planned = reference_component_plan(
                size,
                styles,
                quantizations,
                roi,
                component,
                selection,
                policy,
                precincts,
                total,
                limits,
            )?;
            precincts = planned.precincts;
            total = planned.planned_code_blocks;
            components.push(planned.component);
        }
        Ok(ProgressionTilePlan {
            tile_index: selection.0,
            coding_style,
            component_coding_styles: *styles,
            grid,
            precincts,
            components,
        })
    }

    fn assert_tile_plans_equal(
        actual: Result<ProgressionTilePlan, DecodeError>,
        expected: Result<ProgressionTilePlan, DecodeError>,
    ) -> bool {
        match (actual, expected) {
            (Err(actual), Err(expected)) => {
                assert_eq!(actual, expected);
                false
            }
            (Ok(actual), Ok(expected)) => {
                assert_eq!(actual.tile_index, expected.tile_index);
                assert_eq!(actual.coding_style, expected.coding_style);
                assert_eq!(
                    actual.component_coding_styles,
                    expected.component_coding_styles
                );
                assert_eq!(actual.grid, expected.grid);
                assert_eq!(actual.components, expected.components);
                assert_eq!(
                    actual
                        .precincts
                        .iter()
                        .map(crate::packet::exact_packet_precinct_snapshot)
                        .collect::<Vec<_>>(),
                    expected
                        .precincts
                        .iter()
                        .map(crate::packet::exact_packet_precinct_snapshot)
                        .collect::<Vec<_>>()
                );
                true
            }
            (Err(error), Ok(_)) => panic!("unexpected tile-plan error: {error:?}"),
            (Ok(_), Err(error)) => panic!("tile plan should have failed: {error:?}"),
        }
    }

    #[test]
    fn actual_component_plan_derives_full_component_and_precinct_vectors_from_real_raw_owners() {
        let qcd = [0x40, 0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50];
        let fixtures = [
            prepared_codestream(5, 3, 1, &qcd, &[0, 0, 0]),
            two_tile_codestream(),
            component_override_codestream(),
        ];
        let limits = [
            ResourceLimits::HARDENED,
            ResourceLimits::HARDENED
                .with_maximum_code_blocks(0)
                .unwrap(),
            ResourceLimits::HARDENED.with_maximum_work_units(0).unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_code_blocks(2)
                .unwrap()
                .with_maximum_work_units(1)
                .unwrap(),
        ];
        let mut cases = 0;
        let mut successes = 0;
        for bytes in fixtures {
            let layout = parse_layout(&bytes, &ResourceLimits::HARDENED).unwrap();
            for tile_scope in [false, true] {
                let (styles, quantizations, roi) = planning_fixture_owners(&layout, tile_scope);
                for component in [0, 1, 64] {
                    for tile in [0, 1, u16::MAX] {
                        for discard in [0, 1, 2, 3, u8::MAX] {
                            for policy in [
                                (false, 0, false),
                                (false, 74, false),
                                (true, 1, true),
                                (true, 75, true),
                            ] {
                                for limit in &limits {
                                    for total in [0, 1, u64::MAX] {
                                        for prefix in [false, true] {
                                            let expected = reference_component_plan(
                                                &layout.info.image_size,
                                                styles,
                                                quantizations,
                                                roi,
                                                component,
                                                (tile, discard),
                                                policy,
                                                planning_prefix(prefix),
                                                total,
                                                limit,
                                            );
                                            let actual = plan_tile_component(
                                                &layout.info.image_size,
                                                styles,
                                                quantizations,
                                                roi,
                                                component,
                                                (tile, discard),
                                                policy,
                                                planning_prefix(prefix),
                                                total,
                                                limit,
                                            );
                                            match (actual, expected) {
                                                (Err(actual), Err(expected)) => {
                                                    assert_eq!(actual, expected)
                                                }
                                                (Ok(actual), Ok(expected)) => {
                                                    assert_eq!(
                                                        actual.component,
                                                        expected.component
                                                    );
                                                    assert_eq!(
                                                        actual.planned_code_blocks,
                                                        expected.planned_code_blocks
                                                    );
                                                    assert_eq!(actual.precincts.iter().map(crate::packet::exact_packet_precinct_snapshot).collect::<Vec<_>>(),
                                                        expected.precincts.iter().map(crate::packet::exact_packet_precinct_snapshot).collect::<Vec<_>>());
                                                    successes += 1;
                                                }
                                                (Err(error), Ok(_)) => panic!(
                                                    "unexpected component-plan error: {error:?}"
                                                ),
                                                (Ok(_), Err(error)) => panic!(
                                                    "component plan should have failed: {error:?}"
                                                ),
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
        assert_eq!(cases, 25_920);
        assert!(successes > 100);
    }

    #[test]
    fn actual_selected_tile_plan_and_public_owner_boundary_bind_every_field_and_first_error() {
        let qcd = [0x40, 0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50];
        let fixtures = [
            prepared_codestream(5, 3, 1, &qcd, &[0, 0, 0]),
            two_tile_codestream(),
            component_override_codestream(),
        ];
        let limits = [
            ResourceLimits::HARDENED,
            ResourceLimits::HARDENED
                .with_maximum_code_blocks(0)
                .unwrap(),
            ResourceLimits::HARDENED.with_maximum_work_units(0).unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_code_blocks(2)
                .unwrap()
                .with_maximum_work_units(1)
                .unwrap(),
        ];
        let mut cases = 0;
        let mut successes = 0;
        for bytes in fixtures {
            let layout = parse_layout(&bytes, &ResourceLimits::HARDENED).unwrap();
            for tile_scope in [false, true] {
                let (styles, quantizations, roi) = planning_fixture_owners(&layout, tile_scope);
                for tile in [0, 1, u16::MAX] {
                    for discard in [0, 1, 2, 3, u8::MAX] {
                        for policy in [
                            (false, 0, false),
                            (false, 74, false),
                            (true, 1, true),
                            (true, 75, true),
                        ] {
                            for limit in &limits {
                                let expected = reference_tile_plan(
                                    &layout.info.image_size,
                                    layout.info.coding_style,
                                    styles,
                                    quantizations,
                                    roi,
                                    (tile, discard),
                                    policy,
                                    limit,
                                );
                                let actual = plan_progression_tile_at_selected(
                                    &layout.info.image_size,
                                    tile,
                                    layout.info.coding_style,
                                    styles,
                                    quantizations,
                                    roi,
                                    discard,
                                    policy.0,
                                    policy.1,
                                    policy.2,
                                    limit,
                                );
                                successes += usize::from(assert_tile_plans_equal(actual, expected));
                                if discard == 0 && policy == (false, 74, false) {
                                    let public = plan_progression_tile_at_with_component_overrides(
                                        &layout.info.image_size,
                                        tile,
                                        layout.info.coding_style,
                                        styles,
                                        quantizations,
                                        roi,
                                        limit,
                                    );
                                    assert_tile_plans_equal(
                                        public,
                                        reference_tile_plan(
                                            &layout.info.image_size,
                                            layout.info.coding_style,
                                            styles,
                                            quantizations,
                                            roi,
                                            (tile, discard),
                                            policy,
                                            limit,
                                        ),
                                    );
                                }
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 1_440);
        assert!(successes > 10);
    }

    #[allow(clippy::too_many_arguments)]
    fn reference_packet_reconstruction(
        mut plan: ProgressionTilePlan,
        parts: &[TilePartPayload<'_>],
        first: &TilePartPayload<'_>,
        size: &ImageSize,
        standard: (bool, bool),
        counts: Option<&[u8]>,
        limits: &ResourceLimits,
        mut prepared: Vec<PreparedTileComponent>,
        mut work: u64,
    ) -> Result<DecodedTileComponents, DecodeError> {
        let (grid, precincts) = plan.packet_execution();
        match counts {
            None => crate::packet::decode_progression_tile_packets(parts, grid, precincts, limits),
            Some(counts) => crate::packet::decode_progression_tile_packets_selected(
                parts,
                grid,
                precincts,
                Some(counts),
                limits,
            ),
        }?;
        reconstruct_planned_tile(
            &plan,
            first,
            size,
            standard,
            limits,
            &mut prepared,
            &mut work,
        )?;
        Ok(DecodedTileComponents {
            prepared,
            mct_work: work,
        })
    }

    #[test]
    fn actual_packet_reconstruction_uses_real_bytes_complete_decoder_planes_and_owned_prepared_prefixes()
     {
        let qcd = [0x40, 0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50];
        let fixtures = [
            prepared_codestream(5, 3, 1, &qcd, &[0, 0, 0]),
            two_tile_codestream(),
            component_override_codestream(),
            add_single_component_mct(prepared_codestream(1, 1, 1, &qcd, &[0, 0, 0]), 2, 3),
        ];
        let limits = [
            ResourceLimits::HARDENED,
            ResourceLimits::HARDENED.with_maximum_work_units(0).unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_decoded_bytes(0)
                .unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_packet_header_bytes(0)
                .unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_code_block_bytes(0)
                .unwrap(),
        ];
        let mut cases = 0;
        let mut successes = 0;
        let mut nonzero = 0;
        for bytes in fixtures {
            let layout = parse_layout(&bytes, &ResourceLimits::HARDENED).unwrap();
            let first = &layout.tile_parts[0];
            let masks = [
                None,
                Some(Vec::new()),
                Some(vec![0; usize::from(layout.info.image_size.components)]),
                Some(vec![1; usize::from(layout.info.image_size.components)]),
                Some(vec![
                    u8::MAX;
                    usize::from(layout.info.image_size.components)
                ]),
            ];
            for tile_scope in [false, true] {
                let (styles, quantizations, roi) = planning_fixture_owners(&layout, tile_scope);
                let plan = || {
                    plan_progression_tile_at_selected(
                        &layout.info.image_size,
                        0,
                        first.coding_style,
                        styles,
                        quantizations,
                        roi,
                        0,
                        false,
                        74,
                        false,
                        &ResourceLimits::HARDENED,
                    )
                    .unwrap()
                };
                for mask in &masks {
                    for standard in [(false, false), (false, true), (true, false), (true, true)] {
                        for limit in &limits {
                            for work in [0, 1, u64::MAX] {
                                for prefix in [false, true] {
                                    let prepared = || {
                                        if prefix {
                                            vec![PreparedTileComponent {
                                                component: u16::MAX,
                                                x: u32::MAX,
                                                y: 17,
                                                width: 1,
                                                height: 1,
                                                fraction_bits: u8::MAX,
                                                samples: vec![i64::MIN, i64::MAX],
                                            }]
                                        } else {
                                            Vec::new()
                                        }
                                    };
                                    for parts in [
                                        &layout.tile_parts[..],
                                        &layout.tile_parts[..0],
                                        &layout.tile_parts[..1],
                                    ] {
                                        let expected = reference_packet_reconstruction(
                                            plan(),
                                            parts,
                                            first,
                                            &layout.info.image_size,
                                            standard,
                                            mask.as_deref(),
                                            limit,
                                            prepared(),
                                            work,
                                        );
                                        let actual = decode_planned_tile_components(
                                            plan(),
                                            parts,
                                            first,
                                            &layout.info.image_size,
                                            standard,
                                            mask.as_deref(),
                                            limit,
                                            prepared(),
                                            work,
                                        );
                                        match (actual, expected) {
                                            (Err(actual), Err(expected)) => {
                                                assert_eq!(actual, expected)
                                            }
                                            (Ok(actual), Ok(expected)) => {
                                                assert_eq!(actual.prepared, expected.prepared);
                                                assert_eq!(actual.mct_work, expected.mct_work);
                                                successes += 1;
                                                nonzero += usize::from(
                                                    actual
                                                        .prepared
                                                        .iter()
                                                        .skip(usize::from(prefix))
                                                        .any(|component| {
                                                            component
                                                                .samples
                                                                .iter()
                                                                .any(|sample| *sample != 0)
                                                        }),
                                                );
                                            }
                                            (Err(error), Ok(_)) => panic!(
                                                "unexpected packet-reconstruction error: {error:?}"
                                            ),
                                            (Ok(_), Err(error)) => panic!(
                                                "packet reconstruction should have failed: {error:?}"
                                            ),
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
        assert_eq!(cases, 14_400);
        assert!(successes > 10 && nonzero > 0);
    }

    fn reference_selected_component_blocks(
        plan: &PlannedTileComponent,
        mut total: u64,
    ) -> Result<u64, DecodeError> {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodeBlock,
        };
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        for placement in &plan.placements {
            let band = plan.subbands.get(placement.subband_index).ok_or(invalid)?;
            if band.resolution <= plan.output_resolution {
                total = u64::try_from(u128::from(total) + 1).map_err(|_| arithmetic)?;
            }
        }
        Ok(total)
    }

    #[test]
    fn actual_selected_component_accounting_preserves_raw_band_indices_resolution_and_overflow_order()
     {
        let mut cases = 0;
        for output in [0, 1, 2, u8::MAX] {
            for resolution in [0, 1, 2, 31, 32, u8::MAX] {
                for bands in 0..4 {
                    let subbands = (0..bands)
                        .map(|index| {
                            let mut band =
                                raw_planned_subband(u32::MAX, u32::MAX, 0, SubbandOrientation::Hh);
                            band.resolution = resolution.wrapping_add(u8::try_from(index).unwrap());
                            band
                        })
                        .collect::<Vec<_>>();
                    for count in 0..5 {
                        for seed in [0, 1, bands, usize::MAX] {
                            let plan = PlannedTileComponent {
                                component: u16::MAX,
                                x: u32::MAX,
                                y: 17,
                                width: 0,
                                height: u32::MAX,
                                output_resolution: output,
                                subbands: subbands.clone(),
                                placements: (0..count)
                                    .map(|index| ProgressionCodeBlockPlacement {
                                        precinct_index: usize::MAX,
                                        decoder_index: usize::MAX,
                                        subband_index: if index == 0 { seed } else { usize::MAX },
                                        x: u32::MAX,
                                        y: u32::MAX,
                                    })
                                    .collect(),
                            };
                            for total in [
                                0,
                                1,
                                2,
                                ResourceLimits::HARDENED.maximum_code_blocks(),
                                u64::MAX - 1,
                                u64::MAX,
                            ] {
                                assert_eq!(
                                    count_selected_component_blocks(&plan, total),
                                    reference_selected_component_blocks(&plan, total)
                                );
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 11_520);
    }

    fn reference_plan_accounting(
        plan: &ProgressionTilePlan,
        counts: Option<&[u8]>,
        initial: (u64, u64, u64),
        limits: &ResourceLimits,
    ) -> Result<(u64, u64, u64), DecodeError> {
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let add =
            |a: u64, b: u64| u64::try_from(u128::from(a) + u128::from(b)).map_err(|_| arithmetic);
        let packets = match counts {
            None => plan.grid.packet_count(),
            Some(counts) => plan.grid.selected_packet_count(counts)?,
        };
        let total_packets = add(initial.0, packets)?;
        if total_packets > limits.maximum_packets() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Packets,
                limit: limits.maximum_packets(),
                actual: total_packets,
            });
        }
        let precincts = if counts.is_some() {
            if plan.coding_style.layers == 0 {
                return Err(arithmetic);
            }
            packets / u64::from(plan.coding_style.layers)
        } else {
            plan.grid.total_precinct_count()?
        };
        let total_precincts = add(initial.1, precincts)?;
        if total_precincts > limits.maximum_precincts() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::Precincts,
                limit: limits.maximum_precincts(),
                actual: total_precincts,
            });
        }
        let mut blocks = initial.2;
        for component in &plan.components {
            if counts.is_some() {
                blocks = reference_selected_component_blocks(component, blocks)?;
            } else {
                blocks = add(blocks, u64::try_from(component.placements.len()).unwrap())?;
                if blocks > limits.maximum_code_blocks() {
                    return Err(DecodeError::LimitExceeded {
                        resource: Resource::CodeBlocks,
                        limit: limits.maximum_code_blocks(),
                        actual: blocks,
                    });
                }
            }
        }
        if counts.is_some() && blocks > limits.maximum_code_blocks() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::CodeBlocks,
                limit: limits.maximum_code_blocks(),
                actual: blocks,
            });
        }
        Ok((total_packets, total_precincts, blocks))
    }

    #[test]
    fn actual_layout_accounting_preserves_full_selected_global_limits_empty_and_malformed_plan_policy()
     {
        let qcd = [0x40, 0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50];
        let fixtures = [
            prepared_codestream(5, 3, 1, &qcd, &[0, 0, 0]),
            two_tile_codestream(),
            component_override_codestream(),
        ];
        let limits = [
            ResourceLimits::HARDENED,
            ResourceLimits::HARDENED.with_maximum_packets(0).unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_code_blocks(0)
                .unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_code_blocks(1)
                .unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_packets(1)
                .unwrap()
                .with_maximum_code_blocks(1)
                .unwrap(),
            ResourceLimits::HARDENED.with_maximum_work_units(0).unwrap(),
        ];
        let prior = [
            (0, 0, 0),
            (1, 2, 3),
            (u64::MAX, 0, 0),
            (0, u64::MAX, 0),
            (0, 0, u64::MAX),
            (u64::MAX, u64::MAX, u64::MAX),
            (0, ResourceLimits::HARDENED.maximum_precincts(), 0),
        ];
        let mut cases = 0;
        let mut successes = 0;
        for bytes in fixtures {
            let layout = parse_layout(&bytes, &ResourceLimits::HARDENED).unwrap();
            for tile_scope in [false, true] {
                let (styles, quantizations, roi) = planning_fixture_owners(&layout, tile_scope);
                let masks = [
                    None,
                    Some(Vec::new()),
                    Some(vec![0; usize::from(layout.info.image_size.components)]),
                    Some(vec![1; usize::from(layout.info.image_size.components)]),
                    Some(vec![
                        u8::MAX;
                        usize::from(layout.info.image_size.components)
                    ]),
                ];
                for variant in 0..6 {
                    let mut plan = plan_progression_tile_at_selected(
                        &layout.info.image_size,
                        0,
                        layout.info.coding_style,
                        styles,
                        quantizations,
                        roi,
                        0,
                        false,
                        74,
                        false,
                        &ResourceLimits::HARDENED,
                    )
                    .unwrap();
                    match variant {
                        0 => {}
                        1 => plan.components.clear(),
                        2 => plan.components[0].subbands.clear(),
                        3 => {
                            if let Some(placement) = plan.components[0].placements.last_mut() {
                                placement.subband_index = usize::MAX;
                            }
                        }
                        4 => plan.coding_style.layers = 0,
                        _ => plan.components[0].output_resolution = 0,
                    }
                    for counts in &masks {
                        for initial in prior {
                            for limit in &limits {
                                let expected = reference_plan_accounting(
                                    &plan,
                                    counts.as_deref(),
                                    initial,
                                    limit,
                                );
                                assert_eq!(
                                    account_planned_tile(&plan, counts.as_deref(), initial, limit),
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
        assert_eq!(cases, 7_560);
        assert!(successes > 100);
    }

    #[allow(clippy::too_many_arguments)]
    fn reference_raw_tile_decode(
        size: &ImageSize,
        tile: u16,
        parts: &[TilePartPayload<'_>],
        profile: &codestream::CodestreamProfile,
        discard: Option<u8>,
        limits: &ResourceLimits,
        prepared: Vec<PreparedTileComponent>,
        initial: (u64, u64, u64),
        work: u64,
    ) -> Result<DecodedLayoutTile, DecodeError> {
        let metadata = prepare_tile_decode_metadata(parts, size.components, discard)?;
        let first = metadata.first;
        let counts = discard.map(|_| metadata.resolution_counts.as_slice());
        let policy = (
            profile.part15_allows_multiple_ht_sets(),
            profile.part15_magnitude_bound(),
            profile.part15_requires_all_ht_blocks(),
        );
        let plan = reference_tile_plan(
            size,
            first.coding_style,
            &first.component_coding_styles,
            &first.component_quantizations,
            &first.roi_shifts,
            (tile, discard.unwrap_or(0)),
            policy,
            limits,
        )?;
        let counters = reference_plan_accounting(&plan, counts, initial, limits)?;
        let decoded = reference_packet_reconstruction(
            plan,
            parts,
            first,
            size,
            metadata.standard,
            counts,
            limits,
            prepared,
            work,
        )?;
        Ok(DecodedLayoutTile {
            prepared: decoded.prepared,
            counters,
            mct_work: decoded.mct_work,
        })
    }

    #[test]
    fn actual_raw_tile_decode_derives_all_samples_counters_work_and_first_errors_without_plan_results()
     {
        let qcd = [0x40, 0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50];
        let fixtures = [
            prepared_codestream(5, 3, 1, &qcd, &[0, 0, 0]),
            two_tile_codestream(),
            component_override_codestream(),
            add_single_component_mct(prepared_codestream(1, 1, 1, &qcd, &[0, 0, 0]), 2, 3),
        ];
        let limits = [
            ResourceLimits::HARDENED,
            ResourceLimits::HARDENED.with_maximum_packets(0).unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_code_blocks(0)
                .unwrap(),
            ResourceLimits::HARDENED.with_maximum_work_units(0).unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_decoded_bytes(0)
                .unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_packet_header_bytes(0)
                .unwrap(),
        ];
        let prior = [
            (0, 0, 0),
            (1, 2, 3),
            (u64::MAX, 0, 0),
            (0, u64::MAX, 0),
            (0, 0, u64::MAX),
            (u64::MAX, u64::MAX, u64::MAX),
        ];
        let mut cases = 0;
        let mut successes = 0;
        let mut nonzero = 0;
        for bytes in fixtures {
            let layout = parse_layout(&bytes, &ResourceLimits::HARDENED).unwrap();
            for tile in [0, 1, u16::MAX] {
                for discard in [None, Some(0), Some(1), Some(2), Some(u8::MAX)] {
                    for limit in &limits {
                        for initial in prior {
                            for work in [0, 1, u64::MAX] {
                                for prefix in [false, true] {
                                    let prepared = || {
                                        if prefix {
                                            vec![PreparedTileComponent {
                                                component: u16::MAX,
                                                x: u32::MAX,
                                                y: 17,
                                                width: 1,
                                                height: 1,
                                                fraction_bits: u8::MAX,
                                                samples: vec![i64::MIN, i64::MAX],
                                            }]
                                        } else {
                                            Vec::new()
                                        }
                                    };
                                    for parts in [
                                        &layout.tile_parts[..],
                                        &layout.tile_parts[..0],
                                        &layout.tile_parts[..1],
                                    ] {
                                        let expected = reference_raw_tile_decode(
                                            &layout.info.image_size,
                                            tile,
                                            parts,
                                            &layout.info.profile,
                                            discard,
                                            limit,
                                            prepared(),
                                            initial,
                                            work,
                                        );
                                        let actual = decode_tile_parts_at(
                                            &layout.info.image_size,
                                            tile,
                                            parts,
                                            &layout.info.profile,
                                            discard,
                                            limit,
                                            prepared(),
                                            initial,
                                            work,
                                        );
                                        match (actual, expected) {
                                            (Err(actual), Err(expected)) => {
                                                assert_eq!(actual, expected)
                                            }
                                            (Ok(actual), Ok(expected)) => {
                                                assert_eq!(actual.prepared, expected.prepared);
                                                assert_eq!(actual.counters, expected.counters);
                                                assert_eq!(actual.mct_work, expected.mct_work);
                                                successes += 1;
                                                nonzero += usize::from(
                                                    actual
                                                        .prepared
                                                        .iter()
                                                        .skip(usize::from(prefix))
                                                        .any(|component| {
                                                            component
                                                                .samples
                                                                .iter()
                                                                .any(|sample| *sample != 0)
                                                        }),
                                                );
                                            }
                                            (Err(error), Ok(_)) => panic!(
                                                "unexpected raw-tile decode error: {error:?}"
                                            ),
                                            (Ok(_), Err(error)) => panic!(
                                                "raw-tile decode should have failed: {error:?}"
                                            ),
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
        assert_eq!(cases, 38_880);
        assert!(successes > 10 && nonzero > 0);
    }

    #[test]
    fn actual_codec_output_is_identical_for_source_parts_and_independent_complete_owned_copies() {
        let qcd = [0x40, 0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50];
        let fixtures = [
            prepared_codestream(5, 3, 1, &qcd, &[0, 0, 0]),
            component_override_codestream(),
            add_single_component_mct(prepared_codestream(1, 1, 1, &qcd, &[0, 0, 0]), 2, 3),
        ];
        let limits = [
            ResourceLimits::HARDENED,
            ResourceLimits::HARDENED.with_maximum_packets(0).unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_code_blocks(0)
                .unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_decoded_bytes(0)
                .unwrap(),
        ];
        let mut cases = 0;
        let mut successes = 0;
        let mut nonzero = 0;
        for bytes in fixtures {
            let layout = parse_layout(&bytes, &ResourceLimits::HARDENED).unwrap();
            let copied = collect_tile_parts(&layout.tile_parts, 0).unwrap();
            assert_eq!(copied, layout.tile_parts);
            for discard in [None, Some(0), Some(1), Some(2), Some(u8::MAX)] {
                for limit in &limits {
                    for initial in [(0, 0, 0), (1, 2, 3), (u64::MAX, 0, 0), (0, 0, u64::MAX)] {
                        for work in [0, 1, u64::MAX] {
                            for prefix in [false, true] {
                                let prepared = || {
                                    if prefix {
                                        vec![PreparedTileComponent {
                                            component: u16::MAX,
                                            x: u32::MAX,
                                            y: 17,
                                            width: 1,
                                            height: 1,
                                            fraction_bits: u8::MAX,
                                            samples: vec![i64::MIN, i64::MAX],
                                        }]
                                    } else {
                                        Vec::new()
                                    }
                                };
                                let source = decode_tile_parts_at(
                                    &layout.info.image_size,
                                    0,
                                    &layout.tile_parts,
                                    &layout.info.profile,
                                    discard,
                                    limit,
                                    prepared(),
                                    initial,
                                    work,
                                );
                                let cloned = decode_tile_parts_at(
                                    &layout.info.image_size,
                                    0,
                                    &copied,
                                    &layout.info.profile,
                                    discard,
                                    limit,
                                    prepared(),
                                    initial,
                                    work,
                                );
                                match (source, cloned) {
                                    (Err(source), Err(cloned)) => assert_eq!(source, cloned),
                                    (Ok(source), Ok(cloned)) => {
                                        assert_eq!(source.prepared, cloned.prepared);
                                        assert_eq!(source.counters, cloned.counters);
                                        assert_eq!(source.mct_work, cloned.mct_work);
                                        successes += 1;
                                        nonzero += usize::from(
                                            source.prepared.iter().skip(usize::from(prefix)).any(
                                                |component| {
                                                    component
                                                        .samples
                                                        .iter()
                                                        .any(|sample| *sample != 0)
                                                },
                                            ),
                                        );
                                    }
                                    (Err(error), Ok(_)) | (Ok(_), Err(error)) => {
                                        panic!("copy changed codec outcome: {error:?}")
                                    }
                                }
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 1_440);
        assert!(successes > 10 && nonzero > 0);
    }

    fn reference_complete_layout(
        layout: &CodestreamLayout<'_>,
        requested: Option<&DecodeSelection>,
        limits: &ResourceLimits,
    ) -> Result<DecodedImage, DecodeError> {
        validate_part2_execution(layout)?;
        let (tiles, selection) = match requested {
            None => {
                let count = layout.info.image_size.tile_count()?;
                if count > u64::from(u16::MAX) + 1 {
                    return Err(DecodeError::MalformedInput {
                        offset: 0,
                        kind: MalformedKind::InvalidImageGeometry,
                    });
                }
                (
                    (0..count)
                        .map(|index| u16::try_from(index).unwrap())
                        .collect::<Vec<_>>(),
                    None,
                )
            }
            Some(requested) => {
                let selection = validate_selection(&layout.info.image_size, requested)?;
                (
                    selection.tiles,
                    Some((selection.region, selection.discard_levels)),
                )
            }
        };
        let mut prepared = Vec::new();
        let mut counters = (0, 0, 0);
        let mut work = 0;
        for tile in tiles {
            let parts = layout
                .tile_parts
                .iter()
                .filter(|part| part.tile_index == tile)
                .cloned()
                .collect::<Vec<_>>();
            let decoded = reference_raw_tile_decode(
                &layout.info.image_size,
                tile,
                &parts,
                &layout.info.profile,
                selection.map(|value| value.1),
                limits,
                prepared,
                counters,
                work,
            )?;
            prepared = decoded.prepared;
            counters = decoded.counters;
            work = decoded.mct_work;
        }
        assemble_prepared_components(&layout.info.image_size, &prepared, selection, limits)
    }

    #[test]
    fn actual_full_and_selected_layout_decode_derive_complete_images_from_real_source_parts_and_empty_state()
     {
        let qcd = [0x40, 0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50];
        let fixtures = [
            prepared_codestream(5, 3, 1, &qcd, &[0, 0, 0]),
            two_tile_codestream(),
            component_override_codestream(),
            add_single_component_mct(prepared_codestream(1, 1, 1, &qcd, &[0, 0, 0]), 2, 3),
        ];
        let requests = [
            DecodeSelection::full_image(),
            DecodeSelection::full_image().with_discard_levels(1),
            DecodeSelection::full_image().with_discard_levels(2),
            DecodeSelection::full_image().with_discard_levels(u8::MAX),
            DecodeSelection::tile(0),
            DecodeSelection::tile(1),
            DecodeSelection::tile(u16::MAX),
            DecodeSelection::tiles(Vec::new()),
            DecodeSelection::tiles(vec![0, 1]),
            DecodeSelection::tiles(vec![1, 0]),
            DecodeSelection::tiles(vec![0, 0]),
            DecodeSelection::full_image().with_region(crate::ImageRegion::new(1, 0, 1, 1)),
            DecodeSelection::full_image().with_region(crate::ImageRegion::new(0, 0, 0, 0)),
            DecodeSelection::full_image().with_region(crate::ImageRegion::new(u32::MAX, 0, 2, 1)),
        ];
        let limits = [
            ResourceLimits::HARDENED,
            ResourceLimits::HARDENED.with_maximum_packets(0).unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_code_blocks(0)
                .unwrap(),
            ResourceLimits::HARDENED.with_maximum_work_units(0).unwrap(),
            ResourceLimits::HARDENED
                .with_maximum_decoded_bytes(0)
                .unwrap(),
        ];
        let mut full_cases = 0;
        let mut selected_cases = 0;
        let mut successes = 0;
        for bytes in fixtures {
            for variant in 0..9 {
                let mut layout = parse_layout(&bytes, &ResourceLimits::HARDENED).unwrap();
                match variant {
                    0 => {}
                    1 => layout.tile_parts.clear(),
                    2 => layout.tile_parts.reverse(),
                    3 => {
                        let duplicate = layout.tile_parts[0].clone();
                        layout.tile_parts.push(duplicate);
                    }
                    4 => layout.info.image_size.tiles_across = 0,
                    5 => layout.info.image_size.tile_width = 0,
                    6 => {
                        layout.info.image_size.tiles_across = 65_537;
                        layout.info.image_size.tiles_down = 1;
                    }
                    7 => layout.info.image_size.components = 0,
                    _ => {
                        layout.info.image_size.width = u32::MAX;
                        layout.info.image_size.x_origin = 1;
                    }
                }
                for limit in &limits {
                    let expected = reference_complete_layout(&layout, None, limit);
                    assert_eq!(decode_layout(&layout, limit), expected);
                    successes += usize::from(expected.is_ok());
                    full_cases += 1;
                    for requested in &requests {
                        let expected = reference_complete_layout(&layout, Some(requested), limit);
                        assert_eq!(decode_layout_selected(&layout, requested, limit), expected);
                        successes += usize::from(expected.is_ok());
                        selected_cases += 1;
                    }
                }
            }
        }
        assert_eq!(full_cases, 180);
        assert_eq!(selected_cases, 2_520);
        assert!(successes > 10);
    }

    fn reference_standard_prepare(
        plan: &PlannedTileComponent,
        precincts: &[PacketPrecinctDecoder],
        style: CodingStyle,
        maximum: u64,
    ) -> Result<PreparedTileComponent, DecodeError> {
        use codestream::DecompositionDirection::{Horizontal, None, TwoDimensional, Vertical};
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodeBlock,
        };
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let invalid_style = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodingStyle,
        };
        if plan.subbands.is_empty() {
            return Err(invalid);
        }
        let reversible = match style.transform {
            WaveletTransform::Reversible5x3 => true,
            WaveletTransform::Irreversible9x7 => false,
            WaveletTransform::Arbitrary(_) => return Err(invalid),
        };
        if plan.output_resolution > style.decomposition_levels {
            return Err(invalid_style);
        }
        let mut directions = Vec::new();
        for resolution in 1..=u16::from(plan.output_resolution) {
            let stage = usize::from(style.decomposition_levels) - usize::from(resolution);
            directions.push(
                *style
                    .decomposition_directions
                    .get(stage)
                    .ok_or(invalid_style)?,
            );
            if resolution == 255 {
                return Err(arithmetic);
            }
        }
        let selected = 1 + directions
            .iter()
            .map(|d| match d {
                None => 0,
                Horizontal | Vertical => 1,
                TwoDimensional => 3,
            })
            .sum::<usize>();
        if selected > plan.subbands.len() {
            return Err(invalid);
        }
        let fraction = if reversible { 0 } else { 16 };
        let mut bytes = 0_u128;
        let mut bands = Vec::new();
        for geometry in &plan.subbands[..selected] {
            let count = u128::from(geometry.width) * u128::from(geometry.height);
            if count > usize::MAX as u128 {
                return Err(arithmetic);
            }
            bytes += count * 9;
            if bytes > u128::from(u64::MAX) {
                return Err(arithmetic);
            }
            if bytes > u128::from(maximum) {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::DecodedBytes,
                    limit: maximum,
                    actual: u64::try_from(bytes).unwrap(),
                });
            }
            bands.push(WorkingSubband {
                width: geometry.width,
                height: geometry.height,
                fraction_bits: fraction,
                samples: vec![0; usize::try_from(count).unwrap()],
                visited: vec![false; usize::try_from(count).unwrap()],
            });
        }
        for placement in &plan.placements {
            if placement.subband_index >= selected {
                continue;
            }
            let geometry = plan.subbands[placement.subband_index];
            let precinct = precincts.get(placement.precinct_index).ok_or(invalid)?;
            let decoder = precinct.decoders().get(placement.decoder_index).ok_or(
                DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::InvalidPacketHeader,
                },
            )?;
            let plane = decoder.plane();
            let x1 = u64::from(placement.x) + u64::from(plane.width());
            let y1 = u64::from(placement.y) + u64::from(plane.height());
            if x1 > u64::from(u32::MAX) {
                return Err(arithmetic);
            }
            if x1 > u64::from(geometry.width) {
                return Err(invalid);
            }
            if y1 > u64::from(u32::MAX) {
                return Err(arithmetic);
            }
            if y1 > u64::from(geometry.height) {
                return Err(invalid);
            }
            let mut coefficients = Vec::new();
            for y in 0..plane.height() {
                for x in 0..plane.width() {
                    let coefficient = plane
                        .coefficients()
                        .get(usize::from(y) * usize::from(plane.width()) + usize::from(x))
                        .ok_or(invalid)?;
                    let mut words = [0; 6];
                    // Fixtures finish the one-bit legacy block: any midpoint is zero.
                    if coefficient.significant {
                        words[0] = coefficient.magnitude;
                    }
                    coefficients.push((words, coefficient.significant && coefficient.negative));
                }
            }
            let quantized = crate::transform::tests::reference_reconstruction(
                &coefficients,
                geometry.quantization_step,
                geometry.dynamic_range,
                reversible,
                geometry.roi_shift,
                maximum,
            )?;
            if quantized.2 != fraction {
                return Err(invalid);
            }
            reference_quantized_block(
                &mut bands[placement.subband_index],
                &quantized.3,
                [u32::from(plane.width()), u32::from(plane.height())],
                [placement.x, placement.y],
            )?;
        }
        if bands.iter().any(|band| band.visited.iter().any(|v| !v)) {
            return Err(invalid);
        }
        let ll = bands.first().ok_or(invalid)?;
        let mut reconstructed = (ll.width, ll.height, fraction, ll.samples.clone());
        let mut first = 1;
        for direction in directions {
            if direction == None {
                continue;
            }
            let detail = bands.get(first).ok_or(invalid)?;
            if detail.fraction_bits != fraction {
                return Err(invalid);
            }
            let geometry = plan.subbands[first];
            let low = WaveletBand {
                width: reconstructed.0,
                height: reconstructed.1,
                samples: &reconstructed.3,
            };
            let high = WaveletBand {
                width: detail.width,
                height: detail.height,
                samples: &detail.samples,
            };
            reconstructed = match direction {
                Horizontal => {
                    first += 1;
                    crate::transform::tests::reference_horizontal(
                        low,
                        high,
                        geometry.resolution_width,
                        geometry.low_pass_first_x,
                        !reversible,
                        maximum,
                    )?
                }
                Vertical => {
                    first += 1;
                    crate::transform::tests::reference_vertical(
                        low,
                        high,
                        geometry.resolution_height,
                        geometry.low_pass_first_y,
                        !reversible,
                        maximum,
                    )?
                }
                TwoDimensional => {
                    let lh = bands.get(first + 1).ok_or(invalid)?;
                    let hh = bands.get(first + 2).ok_or(invalid)?;
                    if lh.fraction_bits != fraction || hh.fraction_bits != fraction {
                        return Err(invalid);
                    }
                    first += 3;
                    crate::transform::tests::reference_level(
                        WaveletLevel {
                            ll: low,
                            hl: high,
                            lh: WaveletBand {
                                width: lh.width,
                                height: lh.height,
                                samples: &lh.samples,
                            },
                            hh: WaveletBand {
                                width: hh.width,
                                height: hh.height,
                                samples: &hh.samples,
                            },
                            output_width: geometry.resolution_width,
                            output_height: geometry.resolution_height,
                            low_pass_first_x: geometry.low_pass_first_x,
                            low_pass_first_y: geometry.low_pass_first_y,
                        },
                        !reversible,
                        maximum,
                    )?
                }
                None => unreachable!(),
            };
            if reconstructed.2 != fraction {
                return Err(invalid);
            }
        }
        if reconstructed.0 != plan.width || reconstructed.1 != plan.height {
            return Err(invalid);
        }
        let count = u128::from(plan.width) * u128::from(plan.height);
        if count > usize::MAX as u128 {
            return Err(arithmetic);
        }
        if reconstructed.3.len() as u128 != count {
            return Err(invalid);
        }
        Ok(PreparedTileComponent {
            component: plan.component,
            x: plan.x,
            y: plan.y,
            width: plan.width,
            height: plan.height,
            fraction_bits: fraction,
            samples: reconstructed.3,
        })
    }

    fn standard_prepare_fixture(
        pattern: &[codestream::DecompositionDirection],
        reversible: bool,
        parity: bool,
        seed: u8,
    ) -> (
        PlannedTileComponent,
        Vec<PacketPrecinctDecoder>,
        CodingStyle,
    ) {
        use codestream::DecompositionDirection::*;
        let mut width = 1;
        let mut height = 1;
        let mut subbands = vec![SubbandGeometry {
            resolution: 0,
            orientation: SubbandOrientation::Ll,
            x0: u32::MAX,
            y0: 17,
            width,
            height,
            resolution_width: width,
            resolution_height: height,
            quantization_index: u16::MAX,
            quantization_step: QuantizationStep {
                exponent: 8,
                mantissa: 137,
            },
            dynamic_range: 8,
            significant_bitplanes: 1,
            roi_shift: 0,
            low_pass_first_x: parity,
            low_pass_first_y: parity,
        }];
        for (index, direction) in pattern.iter().enumerate() {
            let columns = width;
            let rows = height;
            if matches!(direction, Horizontal | TwoDimensional) {
                width *= 2;
            }
            if matches!(direction, Vertical | TwoDimensional) {
                height *= 2;
            }
            let count = match direction {
                None => 0,
                Horizontal | Vertical => 1,
                TwoDimensional => 3,
            };
            for detail in 0..count {
                subbands.push(SubbandGeometry {
                    resolution: u8::try_from(index + 1).unwrap(),
                    orientation: [
                        SubbandOrientation::Hl,
                        SubbandOrientation::Lh,
                        SubbandOrientation::Hh,
                    ][detail],
                    width: columns,
                    height: rows,
                    resolution_width: width,
                    resolution_height: height,
                    ..subbands[0]
                });
            }
        }
        let limits = ResourceLimits::default();
        let mut decoders = Vec::new();
        for geometry in &subbands {
            let mut decoder = Tier1CodeBlockDecoder::new(
                u16::try_from(geometry.width).unwrap(),
                u16::try_from(geometry.height).unwrap(),
                1,
                geometry.orientation,
                Tier1CodingStyle::default(),
            )
            .unwrap();
            decoder.decode_segment(&[seed; 64], 1, &limits).unwrap();
            assert!(decoder.is_complete());
            assert!(
                decoder
                    .plane()
                    .coefficients()
                    .iter()
                    .all(|c| c.magnitude <= 1)
            );
            decoders.push(decoder);
        }
        let precinct = PacketPrecinctDecoder::new(
            crate::packet::PacketPrecinctGeometry {
                component: u16::MAX,
                resolution: 0,
                precinct: 0,
                code_blocks_across: u32::try_from(decoders.len()).unwrap(),
                code_blocks_down: 1,
                maximum_zero_bitplanes: 1,
            },
            Tier1CodingStyle::default(),
            decoders,
            &limits,
        )
        .unwrap();
        let mut placements: Vec<_> = (0..subbands.len())
            .map(|index| ProgressionCodeBlockPlacement {
                precinct_index: 0,
                decoder_index: index,
                subband_index: index,
                x: 0,
                y: 0,
            })
            .collect();
        placements.reverse();
        placements.push(ProgressionCodeBlockPlacement {
            precinct_index: usize::MAX,
            decoder_index: usize::MAX,
            subband_index: usize::MAX,
            x: u32::MAX,
            y: u32::MAX,
        });
        let mut directions = [None; 32];
        for (index, direction) in pattern.iter().rev().enumerate() {
            directions[index] = *direction;
        }
        let style = CodingStyle {
            progression_order: ProgressionOrder::Lrcp,
            layers: 0,
            multiple_component_transform: false,
            decomposition_levels: u8::try_from(pattern.len()).unwrap(),
            decomposition_structure: Option::None,
            decomposition_directions: directions,
            code_block_width: 0,
            code_block_height: 0,
            code_block_style: 0,
            transform: if reversible {
                WaveletTransform::Reversible5x3
            } else {
                WaveletTransform::Irreversible9x7
            },
            precincts_defined: false,
            sop_markers: false,
            eph_markers: false,
            precinct_exponents: [(0, 0); 33],
        };
        (
            PlannedTileComponent {
                component: u16::MAX,
                x: u32::MAX,
                y: 123,
                width,
                height,
                output_resolution: u8::try_from(pattern.len()).unwrap(),
                subbands,
                placements,
            },
            vec![precinct],
            style,
        )
    }

    #[test]
    fn full_native_kernel_preparation_preserves_real_dispatch_and_lazy_validation() {
        use crate::codestream::{DecompositionDirection, KernelCoefficient};
        let limits = ResourceLimits::default();
        let invalid_block = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidCodeBlock,
        };
        let invalid_transform = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidTransform,
        };
        for pattern in [
            vec![],
            vec![DecompositionDirection::Horizontal],
            vec![DecompositionDirection::Vertical],
            vec![DecompositionDirection::TwoDimensional],
            vec![
                DecompositionDirection::Horizontal,
                DecompositionDirection::Vertical,
            ],
        ] {
            for parity in [false, true] {
                let (plan, _, mut style) = standard_prepare_fixture(&pattern, true, parity, 0);
                let decoders = plan
                    .subbands
                    .iter()
                    .map(|band| {
                        Tier1CodeBlockDecoder::new(
                            u16::try_from(band.width).unwrap(),
                            u16::try_from(band.height).unwrap(),
                            1,
                            band.orientation,
                            Tier1CodingStyle::default(),
                        )
                        .unwrap()
                    })
                    .collect::<Vec<_>>();
                let precincts = vec![
                    PacketPrecinctDecoder::new(
                        crate::packet::PacketPrecinctGeometry {
                            component: u16::MAX,
                            resolution: 0,
                            precinct: 0,
                            code_blocks_across: u32::try_from(decoders.len()).unwrap(),
                            code_blocks_down: 1,
                            maximum_zero_bitplanes: 1,
                        },
                        Tier1CodingStyle::default(),
                        decoders,
                        &limits,
                    )
                    .unwrap(),
                ];
                for reversible in [false, true] {
                    style.transform = WaveletTransform::Arbitrary(0);
                    let kernel = ArbitraryKernel::for_test(
                        reversible,
                        0,
                        false,
                        false,
                        false,
                        Some(KernelCoefficient::for_test(1, u8::MAX)),
                        vec![],
                    );
                    let expected = PreparedTileComponent {
                        component: plan.component,
                        x: plan.x,
                        y: plan.y,
                        width: plan.width,
                        height: plan.height,
                        fraction_bits: if reversible { 0 } else { 16 },
                        samples: vec![0; (plan.width * plan.height) as usize],
                    };
                    assert_eq!(
                        prepare_tile_component_with_kernel(
                            &plan,
                            &precincts,
                            style,
                            Some(&kernel),
                            &limits
                        ),
                        Ok(expected)
                    );
                    assert_eq!(
                        prepare_tile_component_with_kernel(&plan, &precincts, style, None, &limits),
                        Err(invalid_block)
                    );
                    let opposite = if reversible {
                        WaveletTransform::Irreversible9x7
                    } else {
                        WaveletTransform::Reversible5x3
                    };
                    style.transform = opposite;
                    if !pattern.is_empty() {
                        assert_eq!(
                            prepare_tile_component_with_kernel(
                                &plan,
                                &precincts,
                                style,
                                Some(&kernel),
                                &limits
                            ),
                            Err(invalid_block),
                        );
                    }
                }
                style.transform = WaveletTransform::Arbitrary(0);
                let missing =
                    ArbitraryKernel::for_test(false, u8::MAX, false, false, false, None, vec![]);
                let result = prepare_tile_component_with_kernel(
                    &plan,
                    &precincts,
                    style,
                    Some(&missing),
                    &limits,
                );
                if pattern.is_empty() {
                    let prepared = result.unwrap();
                    assert_eq!(prepared.fraction_bits, 16);
                    assert_eq!(prepared.samples, vec![0]);
                } else {
                    assert_eq!(result, Err(invalid_transform));
                }
                let tiny = limits.with_maximum_decoded_bytes(1).unwrap();
                assert_eq!(
                    prepare_tile_component_with_kernel(
                        &plan,
                        &precincts,
                        style,
                        Some(&missing),
                        &tiny
                    ),
                    Err(DecodeError::LimitExceeded {
                        resource: Resource::DecodedBytes,
                        limit: 1,
                        actual: 9
                    }),
                );
            }
        }
    }

    #[test]
    fn complete_standard_prepare_matches_independent_wide_multilevel_reference() {
        use codestream::DecompositionDirection::*;
        let patterns: &[&[_]] = &[
            &[],
            &[Horizontal],
            &[Vertical],
            &[TwoDimensional],
            &[None, Horizontal],
            &[Horizontal, None, Vertical],
            &[TwoDimensional, TwoDimensional],
            &[Horizontal, Vertical, TwoDimensional],
        ];
        let limits = ResourceLimits::default();
        let mut nonzero = false;
        for pattern in patterns {
            for reversible in [false, true] {
                for parity in [false, true] {
                    for seed in [0, 17, 64, 128, 160] {
                        let (plan, precincts, style) =
                            standard_prepare_fixture(pattern, reversible, parity, seed);
                        nonzero |= precincts[0]
                            .decoders()
                            .iter()
                            .any(|d| d.plane().coefficients().iter().any(|c| c.significant));
                        assert_eq!(
                            prepare_tile_component(&plan, &precincts, style, &limits),
                            reference_standard_prepare(
                                &plan,
                                &precincts,
                                style,
                                limits.maximum_decoded_bytes()
                            )
                        );
                    }
                }
            }
        }
        assert!(nonzero);
    }

    #[test]
    fn complete_coefficient_reconstruction_preserves_prefix_mct_and_error_frontiers() {
        use codestream::{
            MctArray, MctArrayKind, MctNumericFormat, MctStage, MultipleComponentTransform,
        };
        let bytes = prepared_codestream(
            1,
            1,
            1,
            &[0, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40],
            &[0],
        );
        let parser_limits = ResourceLimits::default();
        let layout = parse_layout(&bytes, &parser_limits).unwrap();
        for reversible in [false, true] {
            for seed in [0, 17, 128, 160] {
                for with_mct in [false, true] {
                    for bad_tail in [false, true] {
                        for maximum in [8, 9, 64] {
                            let limits = ResourceLimits::default()
                                .with_maximum_decoded_bytes(maximum)
                                .unwrap();
                            let (mut component, precincts, style) =
                                standard_prepare_fixture(&[], reversible, false, seed);
                            component.component = 0;
                            component.x = 0;
                            component.y = 0;
                            let reference =
                                reference_standard_prepare(&component, &precincts, style, maximum);
                            let mut expected = vec![PreparedTileComponent {
                                component: u16::MAX,
                                x: 9,
                                y: 9,
                                width: 1,
                                height: 1,
                                fraction_bits: u8::MAX,
                                samples: vec![123],
                            }];
                            let mut expected_work = 7;
                            let expected_result = match reference {
                                Err(error) => Err(error),
                                Ok(prepared) => {
                                    expected.push(prepared);
                                    if bad_tail {
                                        Err(DecodeError::MalformedInput {
                                            offset: 0,
                                            kind: MalformedKind::InvalidCodingStyle,
                                        })
                                    } else if with_mct {
                                        expected_work += 2;
                                        if maximum < 16 {
                                            Err(DecodeError::LimitExceeded {
                                                resource: Resource::DecodedBytes,
                                                limit: maximum,
                                                actual: 16,
                                            })
                                        } else {
                                            for value in &mut expected[1].samples {
                                                *value = if reversible {
                                                    *value * 2 + 1
                                                } else {
                                                    i64::try_from(
                                                        i128::from(*value) * 2 * (1_i128 << 16)
                                                            + (1_i128 << 32),
                                                    )
                                                    .unwrap()
                                                };
                                            }
                                            expected[1].fraction_bits =
                                                if reversible { 0 } else { 32 };
                                            Ok(())
                                        }
                                    } else {
                                        Ok(())
                                    }
                                }
                            };
                            let mut first = layout.tile_parts[0].clone();
                            first.coding_style = style;
                            first.component_coding_styles =
                                ComponentCodingStyles::uniform(1, style).unwrap();
                            if with_mct {
                                let array = |values| {
                                    MctArray::from_values(
                                        0,
                                        MctArrayKind::Offset,
                                        MctNumericFormat::Int16,
                                        0,
                                        values,
                                    )
                                };
                                first.multiple_component_transform =
                                    Some(MultipleComponentTransform::from_stages(vec![
                                        MctStage::from_collections(
                                            0,
                                            vec![codestream::MctCollection::from_arrays(
                                                reversible,
                                                vec![0],
                                                vec![0],
                                                array(vec![2]),
                                                Some(array(vec![1])),
                                            )],
                                        ),
                                    ]));
                            }
                            let mut components = vec![component];
                            if bad_tail {
                                let mut bad =
                                    standard_prepare_fixture(&[], reversible, false, seed).0;
                                bad.component = 1;
                                components.push(bad);
                            }
                            let plan = ProgressionTilePlan {
                                tile_index: 0,
                                coding_style: style,
                                component_coding_styles: first.component_coding_styles,
                                grid: PacketGrid::new(1, 1, 1, vec![1], &ResourceLimits::default())
                                    .unwrap(),
                                precincts,
                                components,
                            };
                            let mut prepared = vec![PreparedTileComponent {
                                component: u16::MAX,
                                x: 9,
                                y: 9,
                                width: 1,
                                height: 1,
                                fraction_bits: u8::MAX,
                                samples: vec![123],
                            }];
                            let mut work = 7;
                            assert_eq!(
                                reconstruct_planned_tile(
                                    &plan,
                                    &first,
                                    &layout.info.image_size,
                                    (false, false),
                                    &limits,
                                    &mut prepared,
                                    &mut work
                                ),
                                expected_result
                            );
                            assert_eq!(work, expected_work);
                            assert_eq!(prepared, expected);
                            if expected_result.is_ok() {
                                let assembled = assemble_prepared_components(
                                    &layout.info.image_size,
                                    &prepared[1..],
                                    None,
                                    &limits,
                                )
                                .unwrap();
                                assert_eq!(
                                    assembled.component(0).unwrap().samples(),
                                    expected[1].samples
                                );
                                assert_eq!(
                                    assembled.component(0).unwrap().fraction_bits(),
                                    expected[1].fraction_bits
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn complete_standard_prepare_preserves_all_ordered_rejections() {
        for variant in 0..15 {
            for maximum in [1, 8, 9, 17, 18, 35, 36, 1024] {
                let (mut plan, precincts, mut style) = standard_prepare_fixture(
                    &[codestream::DecompositionDirection::TwoDimensional],
                    true,
                    true,
                    17,
                );
                match variant {
                    0 => plan.subbands.clear(),
                    1 => style.transform = WaveletTransform::Arbitrary(255),
                    2 => plan.output_resolution = 2,
                    3 => style.decomposition_levels = 255,
                    4 => {
                        plan.subbands.pop();
                    }
                    5 => plan.subbands[0].width = u32::MAX,
                    6 => plan.subbands[0].dynamic_range = 0,
                    7 => plan.subbands[0].quantization_step.mantissa = u16::MAX,
                    8 => plan.placements[0].precinct_index = usize::MAX,
                    9 => plan.placements[0].decoder_index = usize::MAX,
                    10 => plan.placements[0].x = u32::MAX,
                    11 => {
                        plan.placements.pop();
                        plan.placements.push(plan.placements[0]);
                    }
                    12 => plan.placements.clear(),
                    13 => plan.width += 1,
                    _ => plan.subbands[1].resolution_width = 0,
                }
                let limits = ResourceLimits::default()
                    .with_maximum_decoded_bytes(maximum)
                    .unwrap();
                assert_eq!(
                    prepare_tile_component(&plan, &precincts, style, &limits),
                    reference_standard_prepare(&plan, &precincts, style, maximum),
                    "variant={variant} maximum={maximum}"
                );
            }
        }
    }

    fn copy_working_band(band: &WorkingSubband) -> WorkingSubband {
        WorkingSubband {
            width: band.width,
            height: band.height,
            fraction_bits: band.fraction_bits,
            samples: band.samples.clone(),
            visited: band.visited.clone(),
        }
    }

    fn observed_working_band(band: WorkingSubband) -> (u32, u32, u8, Vec<i64>, Vec<bool>) {
        (
            band.width,
            band.height,
            band.fraction_bits,
            band.samples,
            band.visited,
        )
    }

    fn reference_quantized_block(
        destination: &mut WorkingSubband,
        source: &[i64],
        block: [u32; 2],
        origin: [u32; 2],
    ) -> Result<(), DecodeError> {
        for y in 0..block[1] {
            for x in 0..block[0] {
                let destination_x = u64::from(origin[0]) + u64::from(x);
                let destination_y = u64::from(origin[1]) + u64::from(y);
                if destination_x > u64::from(u32::MAX) || destination_y > u64::from(u32::MAX) {
                    return Err(arithmetic_overflow());
                }
                let output = u128::from(destination_y) * u128::from(destination.width)
                    + u128::from(destination_x);
                if u128::from(destination_y) > usize::MAX as u128
                    || u128::from(destination.width) > usize::MAX as u128
                    || u128::from(destination_x) > usize::MAX as u128
                    || output > usize::MAX as u128
                {
                    return Err(arithmetic_overflow());
                }
                let input = u128::from(y) * u128::from(block[0]) + u128::from(x);
                if u128::from(y) > usize::MAX as u128
                    || u128::from(block[0]) > usize::MAX as u128
                    || u128::from(x) > usize::MAX as u128
                    || input > usize::MAX as u128
                {
                    return Err(arithmetic_overflow());
                }
                let output = output as usize;
                let input = input as usize;
                if output >= destination.samples.len()
                    || output >= destination.visited.len()
                    || input >= source.len()
                    || destination.visited[output]
                {
                    return Err(invalid_decode());
                }
                destination.samples[output] = source[input];
                destination.visited[output] = true;
            }
        }
        Ok(())
    }

    #[test]
    fn complete_quantized_block_placement_matches_independent_wide_reference() {
        for width in 0..=5_u32 {
            for height in 0..=4_u32 {
                for block_width in 0..=4_u32 {
                    for block_height in 0..=3_u32 {
                        let source: Vec<_> = (0..block_width * block_height + 1)
                            .map(|i| i64::from(i) * 13 - 29)
                            .collect();
                        for origin in [[0, 0], [1, 1], [width, height]] {
                            for source_count in [
                                (block_width * block_height) as usize,
                                (block_width * block_height).saturating_sub(1) as usize,
                                source.len(),
                            ] {
                                for previsited in [false, true] {
                                    let mut actual = WorkingSubband {
                                        width,
                                        height,
                                        fraction_bits: 13,
                                        samples: vec![9; (width * height) as usize],
                                        visited: vec![false; (width * height) as usize],
                                    };
                                    if previsited && !actual.visited.is_empty() {
                                        let index = actual.visited.len() / 2;
                                        actual.visited[index] = true;
                                    }
                                    let mut expected = copy_working_band(&actual);
                                    let expected_status = reference_quantized_block(
                                        &mut expected,
                                        &source[..source_count],
                                        [block_width, block_height],
                                        origin,
                                    );
                                    let status = write_quantized_block(
                                        &mut actual,
                                        &source[..source_count],
                                        block_width,
                                        block_height,
                                        origin[0],
                                        origin[1],
                                    );
                                    assert_eq!(
                                        (status, observed_working_band(actual)),
                                        (expected_status, observed_working_band(expected))
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn complete_quantized_block_failures_preserve_samples_visits_and_metadata() {
        for (band, source, block, origin) in [
            (
                WorkingSubband {
                    width: 2,
                    height: 3,
                    fraction_bits: 16,
                    samples: vec![9; 6],
                    visited: vec![false; 6],
                },
                vec![i64::MIN, i64::MAX, 7, 8, 9],
                [2, 3],
                [0, 0],
            ),
            (
                WorkingSubband {
                    width: 2,
                    height: 123,
                    fraction_bits: 0,
                    samples: vec![9; 6],
                    visited: vec![false; 3],
                },
                vec![1, 2, 3, 4, 5, 6],
                [2, 3],
                [0, 0],
            ),
            (
                WorkingSubband {
                    width: 0,
                    height: 0,
                    fraction_bits: 255,
                    samples: vec![9],
                    visited: vec![false],
                },
                vec![7, 8],
                [1, 2],
                [0, u32::MAX],
            ),
            (
                WorkingSubband {
                    width: 1,
                    height: 1,
                    fraction_bits: 7,
                    samples: vec![9],
                    visited: vec![true],
                },
                vec![],
                [1, 1],
                [0, 0],
            ),
            (
                WorkingSubband {
                    width: u32::MAX,
                    height: u32::MAX,
                    fraction_bits: 0,
                    samples: vec![],
                    visited: vec![],
                },
                vec![1],
                [1, 1],
                [u32::MAX, u32::MAX],
            ),
            (
                WorkingSubband {
                    width: 0,
                    height: 0,
                    fraction_bits: 19,
                    samples: vec![],
                    visited: vec![],
                },
                vec![],
                [0, 2],
                [u32::MAX, u32::MAX],
            ),
        ] {
            let mut actual = band;
            let mut expected = copy_working_band(&actual);
            let expected_status = reference_quantized_block(&mut expected, &source, block, origin);
            let status = write_quantized_block(
                &mut actual,
                &source,
                block[0],
                block[1],
                origin[0],
                origin[1],
            );
            assert_eq!(
                (status, observed_working_band(actual)),
                (expected_status, observed_working_band(expected))
            );
        }
    }

    #[test]
    fn selected_color_bridge_composes_actual_conversion_crop_and_error_order() {
        use crate::codestream::ComponentInfo;
        use crate::color::{ConvertedChannelType, ConvertedColorSpace};
        use crate::image::{TypedComponent, TypedSamples};

        fn image(width: u32, height: u32, samples: Vec<u64>) -> TypedImage {
            TypedImage::from_components_with_origin(
                width,
                height,
                7,
                11,
                vec![TypedComponent::from_parts(
                    width,
                    height,
                    ComponentInfo {
                        precision: 8,
                        signed: false,
                        x_subsampling: 1,
                        y_subsampling: 1,
                        x_registration: 0,
                        y_registration: 0,
                    },
                    TypedSamples::Unsigned(samples),
                )],
                &ResourceLimits::default(),
            )
            .unwrap()
        }
        let input = image(4, 2, vec![0, 32, 64, 96, 128, 160, 192, 255]);
        let gray = jp2::ColorSpecification::enumerated(jp2::ENUMERATED_GRAYSCALE);
        for (target, dimensions, expected) in [
            (
                [7, 11, 11, 13],
                (4, 2),
                vec![0, 8224, 16448, 24672, 32896, 41120, 49344, 65535],
            ),
            ([8, 11, 10, 13], (2, 2), vec![8224, 16448, 41120, 49344]),
            ([10, 12, 11, 13], (1, 1), vec![65535]),
        ] {
            let actual =
                convert_selected_color(&input, &gray, target, &ResourceLimits::default()).unwrap();
            assert_eq!((actual.width(), actual.height()), dimensions);
            assert_eq!(
                (actual.color_space(), actual.precision()),
                (ConvertedColorSpace::Grayscale, 16)
            );
            assert_eq!(actual.channels().len(), 1);
            assert_eq!(
                actual.channels()[0].channel_type(),
                ConvertedChannelType::Gray
            );
            assert_eq!(actual.channels()[0].samples(), expected);
        }
        for target in [
            [6, 11, 7, 12],
            [7, 10, 8, 11],
            [9, 11, 8, 12],
            [7, 12, 8, 11],
        ] {
            assert_eq!(
                convert_selected_color(&input, &gray, target, &ResourceLimits::default()),
                Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::InvalidCodeBlock
                })
            );
        }
        for target in [[7, 11, 7, 12], [7, 11, 12, 13]] {
            assert_eq!(
                convert_selected_color(&input, &gray, target, &ResourceLimits::default()),
                Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::ContainerCodestreamMismatch
                })
            );
        }
        let target = [6, 11, 7, 12];
        assert_eq!(
            convert_selected_color(
                &input,
                &jp2::ColorSpecification::enumerated(99),
                target,
                &ResourceLimits::default()
            ),
            Err(DecodeError::UnsupportedColorConversion {
                reason: crate::UnsupportedColorConversion::EnumeratedColorSpace
            })
        );
        let limits = ResourceLimits::default()
            .with_maximum_decoded_bytes(1)
            .unwrap();
        assert_eq!(
            convert_selected_color(&input, &gray, target, &limits),
            Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: 1,
                actual: 16
            })
        );
        assert_eq!(
            convert_selected_color(
                &image(1, 1, vec![256]),
                &gray,
                target,
                &ResourceLimits::default()
            ),
            Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::ContainerCodestreamMismatch
            })
        );
    }

    fn segment(marker: u16, payload: &[u8]) -> Vec<u8> {
        let length = u16::try_from(payload.len() + 2).unwrap();
        let mut bytes = marker.to_be_bytes().to_vec();
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    fn prepared_codestream(
        width: u32,
        height: u32,
        transform: u8,
        qcd: &[u8],
        packet_bytes: &[u8],
    ) -> Vec<u8> {
        let mut siz = Vec::new();
        siz.extend_from_slice(&0_u16.to_be_bytes());
        siz.extend_from_slice(&width.to_be_bytes());
        siz.extend_from_slice(&height.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&width.to_be_bytes());
        siz.extend_from_slice(&height.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&1_u16.to_be_bytes());
        siz.extend_from_slice(&[7, 1, 1]);
        let cod = [0, 0, 0, 1, 0, 2, 0, 0, 0, transform];
        let mut bytes = 0xff4f_u16.to_be_bytes().to_vec();
        bytes.extend(segment(0xff51, &siz));
        bytes.extend(segment(0xff52, &cod));
        bytes.extend(segment(0xff5c, qcd));
        let tile_start = bytes.len();
        let mut sot = Vec::new();
        sot.extend_from_slice(&0_u16.to_be_bytes());
        sot.extend_from_slice(&0_u32.to_be_bytes());
        sot.extend_from_slice(&[0, 1]);
        bytes.extend(segment(0xff90, &sot));
        bytes.extend_from_slice(&0xff93_u16.to_be_bytes());
        bytes.extend_from_slice(packet_bytes);
        let tile_length = u32::try_from(bytes.len() - tile_start).unwrap();
        bytes[tile_start + 6..tile_start + 10].copy_from_slice(&tile_length.to_be_bytes());
        bytes.extend_from_slice(&0xffd9_u16.to_be_bytes());
        bytes
    }

    fn add_plt(mut bytes: Vec<u8>, lengths: &[u8]) -> Vec<u8> {
        assert!(lengths.iter().all(|length| *length > 0 && *length < 128));
        let sod = bytes
            .windows(2)
            .position(|window| window == 0xff93_u16.to_be_bytes())
            .unwrap();
        let mut payload = vec![0];
        payload.extend_from_slice(lengths);
        bytes.splice(sod..sod, segment(0xff58, &payload));
        let sot = bytes
            .windows(2)
            .position(|window| window == 0xff90_u16.to_be_bytes())
            .unwrap();
        let eoc = bytes.len() - 2;
        let tile_length = u32::try_from(eoc - sot).unwrap();
        bytes[sot + 6..sot + 10].copy_from_slice(&tile_length.to_be_bytes());
        bytes
    }

    fn add_single_component_mct(mut bytes: Vec<u8>, matrix: i16, offset: i16) -> Vec<u8> {
        let sot = bytes
            .windows(2)
            .position(|window| window == 0xff90_u16.to_be_bytes())
            .unwrap();
        let mut markers = Vec::new();
        let mut matrix_payload = 0_u16.to_be_bytes().to_vec();
        matrix_payload.extend_from_slice(&0x0101_u16.to_be_bytes());
        matrix_payload.extend_from_slice(&0_u16.to_be_bytes());
        matrix_payload.extend_from_slice(&matrix.to_be_bytes());
        markers.extend(segment(0xff74, &matrix_payload));
        let mut offset_payload = 0_u16.to_be_bytes().to_vec();
        offset_payload.extend_from_slice(&0x0202_u16.to_be_bytes());
        offset_payload.extend_from_slice(&0_u16.to_be_bytes());
        offset_payload.extend_from_slice(&offset.to_be_bytes());
        markers.extend(segment(0xff74, &offset_payload));
        markers.extend(segment(
            0xff75,
            &[0, 0, 7, 0, 0, 0, 1, 1, 0, 1, 0, 0, 1, 0, 1, 2, 1],
        ));
        markers.extend(segment(0xff77, &[1, 7]));
        bytes.splice(sot..sot, markers);
        bytes
    }

    fn add_second_component_mct_stage(mut bytes: Vec<u8>, matrix: i16, offset: i16) -> Vec<u8> {
        let mco = bytes
            .windows(2)
            .position(|window| window == 0xff77_u16.to_be_bytes())
            .unwrap();
        let mut markers = Vec::new();
        let mut matrix_payload = 0_u16.to_be_bytes().to_vec();
        matrix_payload.extend_from_slice(&0x0103_u16.to_be_bytes());
        matrix_payload.extend_from_slice(&0_u16.to_be_bytes());
        matrix_payload.extend_from_slice(&matrix.to_be_bytes());
        markers.extend(segment(0xff74, &matrix_payload));
        let mut offset_payload = 0_u16.to_be_bytes().to_vec();
        offset_payload.extend_from_slice(&0x0204_u16.to_be_bytes());
        offset_payload.extend_from_slice(&0_u16.to_be_bytes());
        offset_payload.extend_from_slice(&offset.to_be_bytes());
        markers.extend(segment(0xff74, &offset_payload));
        markers.extend(segment(
            0xff75,
            &[0, 0, 8, 0, 0, 0, 1, 1, 0, 1, 0, 0, 1, 0, 1, 4, 3],
        ));
        markers.extend(segment(0xff77, &[2, 7, 8]));
        bytes.splice(mco..mco + 6, markers);
        bytes
    }

    fn pack_packet_header(mut bytes: Vec<u8>, ppm: bool, header: &[u8]) -> Vec<u8> {
        let sod = bytes
            .windows(2)
            .position(|window| window == 0xff93_u16.to_be_bytes())
            .unwrap();
        bytes.drain(sod + 2..sod + 2 + header.len());
        if ppm {
            let sot = bytes
                .windows(2)
                .position(|window| window == 0xff90_u16.to_be_bytes())
                .unwrap();
            let mut payload = vec![0];
            payload.extend_from_slice(&u32::try_from(header.len()).unwrap().to_be_bytes());
            payload.extend_from_slice(header);
            bytes.splice(sot..sot, segment(0xff60, &payload));
        } else {
            let sod = bytes
                .windows(2)
                .position(|window| window == 0xff93_u16.to_be_bytes())
                .unwrap();
            let mut payload = vec![0];
            payload.extend_from_slice(header);
            bytes.splice(sod..sod, segment(0xff61, &payload));
        }
        let sot = bytes
            .windows(2)
            .position(|window| window == 0xff90_u16.to_be_bytes())
            .unwrap();
        let eoc = bytes.len() - 2;
        let tile_length = u32::try_from(eoc - sot).unwrap();
        bytes[sot + 6..sot + 10].copy_from_slice(&tile_length.to_be_bytes());
        bytes
    }

    fn jp2_for_codestream(codestream: &[u8], width: u32, height: u32) -> Vec<u8> {
        let mut jp2 = box32(*b"jP  ", &[0x0d, 0x0a, 0x87, 0x0a]);
        jp2.extend(box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&height.to_be_bytes());
        image_header.extend_from_slice(&width.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut header = box32(*b"ihdr", &image_header);
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 17]));
        jp2.extend(box32(*b"jp2h", &header));
        jp2.extend(box32(*b"jp2c", codestream));
        jp2
    }

    fn jpx_for_codestreams(codestreams: &[Vec<u8>], width: u32, height: u32) -> Vec<u8> {
        let mut jpx = box32(*b"jP  ", &[0x0d, 0x0a, 0x87, 0x0a]);
        jpx.extend(box32(*b"ftyp", b"jpxb\0\0\0\0jpxb"));
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&height.to_be_bytes());
        image_header.extend_from_slice(&width.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut header = box32(*b"ihdr", &image_header);
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 17]));
        jpx.extend(box32(*b"jp2h", &header));
        for codestream in codestreams {
            jpx.extend(box32(*b"jp2c", codestream));
        }
        jpx
    }

    fn jpx_for_compositing_layers(
        codestreams: &[Vec<u8>],
        width: u32,
        height: u32,
        with_instruction: bool,
    ) -> Vec<u8> {
        let mut jpx = box32(*b"jP  ", &[0x0d, 0x0a, 0x87, 0x0a]);
        jpx.extend(box32(*b"ftyp", b"jpxb\0\0\0\0jpxb"));
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&height.to_be_bytes());
        image_header.extend_from_slice(&width.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut header = box32(*b"ihdr", &image_header);
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 17]));
        jpx.extend(box32(*b"jp2h", &header));
        for _ in codestreams {
            jpx.extend(box32(*b"jpch", &header));
        }
        for index in 0..codestreams.len() {
            let mut registration = 1_u16.to_be_bytes().to_vec();
            registration.extend_from_slice(&1_u16.to_be_bytes());
            registration.extend_from_slice(&u16::try_from(index).unwrap().to_be_bytes());
            registration.extend_from_slice(&[1, 1, 0, 0]);
            jpx.extend(box32(*b"jplh", &box32(*b"creg", &registration)));
        }
        let mut composition_options = height.to_be_bytes().to_vec();
        composition_options.extend_from_slice(
            &width
                .checked_mul(u32::try_from(codestreams.len()).unwrap())
                .unwrap()
                .to_be_bytes(),
        );
        composition_options.push(0);
        let mut composition = box32(*b"copt", &composition_options);
        if with_instruction {
            let mut instruction = 5_u16.to_be_bytes().to_vec();
            instruction.extend_from_slice(&0_u16.to_be_bytes());
            instruction.extend_from_slice(&1_u32.to_be_bytes());
            instruction.extend_from_slice(&0_u32.to_be_bytes());
            instruction.extend_from_slice(&0_u32.to_be_bytes());
            instruction.extend_from_slice(&100_u32.to_be_bytes());
            instruction.extend_from_slice(&0_u32.to_be_bytes());
            composition.extend(box32(*b"inst", &instruction));
        }
        jpx.extend(box32(*b"comp", &composition));
        for codestream in codestreams {
            jpx.extend(box32(*b"jp2c", codestream));
        }
        jpx
    }

    fn jp2_with_palette(
        codestream: &[u8],
        palette_entries: u16,
        include_channel_definitions: bool,
    ) -> Vec<u8> {
        let mut jp2 = box32(*b"jP  ", &[0x0d, 0x0a, 0x87, 0x0a]);
        jp2.extend(box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut header = box32(*b"ihdr", &image_header);
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 17]));
        let mut palette = palette_entries.to_be_bytes().to_vec();
        palette.extend_from_slice(&[2, 7, 0x84]);
        let mut entry = 0_u16;
        while entry < palette_entries {
            if entry == 128 {
                palette.extend_from_slice(&[42, 0x1d]);
            } else {
                palette.extend_from_slice(&[0, 0]);
            }
            entry += 1;
        }
        header.extend(box32(*b"pclr", &palette));
        header.extend(box32(*b"cmap", &[0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0]));
        if include_channel_definitions {
            let mut definitions = 3_u16.to_be_bytes().to_vec();
            definitions.extend_from_slice(&[0, 0, 0, 0, 0, 1]);
            definitions.extend_from_slice(&[0, 1, 0, 1, 0, 1]);
            definitions.extend_from_slice(&[0, 2, 0xff, 0xff, 0xff, 0xff]);
            header.extend(box32(*b"cdef", &definitions));
        }
        jp2.extend(box32(*b"jp2h", &header));
        jp2.extend(box32(*b"jp2c", codestream));
        jp2
    }

    #[test]
    fn whole_output_header_matches_independent_native_order_and_admission_oracle() {
        use crate::codestream::ComponentInfo;
        use crate::image::{ChannelAssociation, ChannelDefinition, ChannelType};

        type OutputComponent = (
            u32,
            u32,
            ComponentInfo,
            TypedSamples,
            Vec<ChannelDefinition>,
        );
        type OutputImage = ((u32, u32, u32, u32), Vec<OutputComponent>);

        fn malformed(kind: MalformedKind) -> DecodeError {
            DecodeError::MalformedInput { offset: 0, kind }
        }

        fn own_samples(samples: &TypedSamples) -> TypedSamples {
            match samples {
                TypedSamples::Signed(values) => TypedSamples::Signed(values.clone()),
                TypedSamples::Unsigned(values) => TypedSamples::Unsigned(values.clone()),
            }
        }

        fn observe(image: &TypedImage) -> OutputImage {
            (
                (
                    image.width(),
                    image.height(),
                    image.x_origin(),
                    image.y_origin(),
                ),
                image
                    .components()
                    .iter()
                    .map(|component| {
                        (
                            component.width(),
                            component.height(),
                            component.info(),
                            own_samples(component.samples()),
                            component.channel_definitions().to_vec(),
                        )
                    })
                    .collect(),
            )
        }

        fn source(case: usize) -> TypedImage {
            let mut components = Vec::new();
            for index in 0..2 {
                let info = ComponentInfo {
                    precision: 0,
                    signed: index != 0,
                    x_subsampling: 0,
                    y_subsampling: u8::MAX,
                    x_registration: u16::MAX,
                    y_registration: 17,
                };
                let samples = match (case, index) {
                    (3 | 6, _) => TypedSamples::Signed(vec![]),
                    (4, 0) => TypedSamples::Unsigned(vec![u64::MAX]),
                    (5, 0) => TypedSamples::Signed(vec![-1]),
                    (_, 0) => TypedSamples::Signed(vec![0, 1]),
                    (_, _) => TypedSamples::Unsigned(vec![1, 0]),
                };
                let (width, height) = match case {
                    2 if index == 0 => (3, 1),
                    3 => (0, u32::MAX),
                    4 | 5 if index == 0 => (1, 1),
                    6 => (u32::MAX, u32::MAX),
                    _ => (2, 1),
                };
                let mut component = TypedComponent::from_parts(width, height, info, samples);
                component.add_channel_definition(ChannelDefinition::new(
                    ChannelType::Unspecified,
                    ChannelAssociation::Color(u16::MAX),
                ));
                components.push(component);
            }
            TypedImage::for_test(2, 1, if case == 1 { u32::MAX } else { 3 }, 5, components)
        }

        fn oracle(
            image: &TypedImage,
            mappings: &[jp2::ComponentMapping],
            definitions: &[jp2::ChannelDefinitionRecord],
            palette: bool,
            maximum: u64,
        ) -> Result<OutputImage, DecodeError> {
            let (geometry, input) = observe(image);
            let mut output = Vec::new();
            if mappings.is_empty() {
                output = input;
            } else {
                for mapping in mappings {
                    let source = input
                        .get(usize::from(mapping.component))
                        .ok_or(malformed(MalformedKind::InvalidImageGeometry))?;
                    let samples = match mapping.mapping_type {
                        jp2::MappingType::Direct => own_samples(&source.3),
                        jp2::MappingType::Palette { column } => {
                            if !palette || column != 0 {
                                return Err(malformed(MalformedKind::ContainerCodestreamMismatch));
                            }
                            let indices = match &source.3 {
                                TypedSamples::Signed(values) => values
                                    .iter()
                                    .map(|value| i128::from(*value))
                                    .collect::<Vec<_>>(),
                                TypedSamples::Unsigned(values) => values
                                    .iter()
                                    .map(|value| i128::from(*value))
                                    .collect::<Vec<_>>(),
                            };
                            let mut values = Vec::new();
                            for index in indices {
                                if !(0..2).contains(&index) {
                                    return Err(malformed(
                                        MalformedKind::ContainerCodestreamMismatch,
                                    ));
                                }
                                values.push(if index == 0 { 7 } else { -3 });
                            }
                            TypedSamples::Signed(values)
                        }
                    };
                    let (info, definitions) = match mapping.mapping_type {
                        jp2::MappingType::Direct => (source.2, source.4.clone()),
                        jp2::MappingType::Palette { .. } => (
                            ComponentInfo {
                                precision: 8,
                                signed: true,
                                ..source.2
                            },
                            vec![],
                        ),
                    };
                    output.push((source.0, source.1, info, samples, definitions));
                }
            }
            for record in definitions {
                let component = output
                    .get_mut(usize::from(record.channel))
                    .ok_or(malformed(MalformedKind::ContainerCodestreamMismatch))?;
                component.4.push(record.definition);
            }
            if u64::from(geometry.0) + u64::from(geometry.2) > u64::from(u32::MAX)
                || u64::from(geometry.1) + u64::from(geometry.3) > u64::from(u32::MAX)
            {
                return Err(malformed(MalformedKind::ArithmeticOverflow));
            }
            if output.len() > usize::from(u16::MAX) {
                return Err(malformed(MalformedKind::ArithmeticOverflow));
            }
            if output.len() > 64 {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::Components,
                    limit: 64,
                    actual: output.len() as u64,
                });
            }
            let mut bytes = 0_u128;
            for component in &output {
                let area = u128::from(component.0) * u128::from(component.1);
                if area > usize::MAX as u128 {
                    return Err(malformed(MalformedKind::ArithmeticOverflow));
                }
                let length = match &component.3 {
                    TypedSamples::Signed(values) => values.len(),
                    TypedSamples::Unsigned(values) => values.len(),
                };
                if area != length as u128 {
                    return Err(malformed(MalformedKind::InvalidImageGeometry));
                }
                bytes += length as u128 * 8;
                if bytes > u128::from(u64::MAX) {
                    return Err(malformed(MalformedKind::ArithmeticOverflow));
                }
                if bytes > u128::from(maximum) {
                    return Err(DecodeError::LimitExceeded {
                        resource: Resource::DecodedBytes,
                        limit: maximum,
                        actual: bytes as u64,
                    });
                }
            }
            Ok((geometry, output))
        }

        let definition =
            ChannelDefinition::new(ChannelType::Opacity, ChannelAssociation::WholeImage);
        let record = |channel| jp2::ChannelDefinitionRecord {
            channel,
            definition,
        };
        let definitions = [
            vec![],
            vec![record(0)],
            vec![record(0), record(0)],
            vec![record(1), record(0)],
            vec![record(2)],
            vec![record(u16::MAX)],
        ];
        let choices = [
            jp2::ComponentMapping {
                component: 0,
                mapping_type: jp2::MappingType::Direct,
            },
            jp2::ComponentMapping {
                component: 1,
                mapping_type: jp2::MappingType::Direct,
            },
            jp2::ComponentMapping {
                component: 0,
                mapping_type: jp2::MappingType::Palette { column: 0 },
            },
            jp2::ComponentMapping {
                component: 0,
                mapping_type: jp2::MappingType::Palette { column: 1 },
            },
            jp2::ComponentMapping {
                component: u16::MAX,
                mapping_type: jp2::MappingType::Palette { column: 0 },
            },
        ];
        let mut mapping_cases = vec![vec![]];
        for first in choices {
            mapping_cases.push(vec![first]);
            for second in choices {
                mapping_cases.push(vec![first, second]);
            }
        }
        let mut contexts = 0;
        for mappings in mapping_cases {
            for palette in [false, true] {
                for records in &definitions {
                    for case in 0..7 {
                        for maximum in [1, 15, 16, 31, 32, 48] {
                            let header = jp2::Jp2Header {
                                image: jp2::ImageHeader {
                                    width: u32::MAX,
                                    height: 0,
                                    components: 0,
                                    bit_depth: jp2::ComponentBitDepth::Variable,
                                    unknown_color_space: true,
                                    intellectual_property: true,
                                },
                                color: jp2::ColorSpecification::enumerated(u32::MAX),
                                nested_box_count: u32::MAX,
                                palette_entries: u16::MAX,
                                palette_columns: u8::MAX,
                                component_mapping_entries: u16::MAX,
                                channel_definitions: u16::MAX,
                                palette: palette.then(|| {
                                    jp2::Palette::for_test(
                                        2,
                                        vec![jp2::PaletteColumn {
                                            precision: 8,
                                            signed: true,
                                        }],
                                        vec![7, -3],
                                    )
                                }),
                                component_mappings: mappings.clone(),
                                channel_definition_records: records.clone(),
                                capture_resolution: None,
                                display_resolution: None,
                            };
                            let input = source(case);
                            let expected = oracle(&input, &mappings, records, palette, maximum);
                            let limits = ResourceLimits::default()
                                .with_maximum_decoded_bytes(maximum)
                                .unwrap();
                            let actual = apply_jp2_output(input, &header, &limits)
                                .map(|image| observe(&image));
                            assert_eq!(
                                actual, expected,
                                "case={case} maximum={maximum} mappings={mappings:?} palette={palette} records={records:?}"
                            );
                            contexts += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(contexts, 15_624);
    }

    #[test]
    fn palette_component_mapping_matches_independent_complete_native_oracle() {
        use crate::codestream::ComponentInfo;
        use crate::image::{ChannelAssociation, ChannelDefinition, ChannelType};

        fn malformed(kind: MalformedKind) -> DecodeError {
            DecodeError::MalformedInput { offset: 0, kind }
        }

        fn samples(case: usize) -> TypedSamples {
            match case {
                0 => TypedSamples::Signed(vec![]),
                1 => TypedSamples::Signed(vec![-1]),
                2 => TypedSamples::Signed(vec![0]),
                3 => TypedSamples::Signed(vec![1, 0]),
                4 => TypedSamples::Signed(vec![i64::from(u16::MAX)]),
                5 => TypedSamples::Signed(vec![i64::MIN]),
                6 => TypedSamples::Signed(vec![0, 2, 1]),
                7 => TypedSamples::Signed(vec![i64::MAX]),
                8 => TypedSamples::Unsigned(vec![]),
                9 => TypedSamples::Unsigned(vec![0]),
                10 => TypedSamples::Unsigned(vec![1, 0]),
                11 => TypedSamples::Unsigned(vec![u64::from(u16::MAX)]),
                12 => TypedSamples::Unsigned(vec![u64::MAX]),
                13 => TypedSamples::Unsigned(vec![0, 2, 1]),
                _ => unreachable!(),
            }
        }

        fn oracle(
            input: &TypedSamples,
            entries: u16,
            columns: &[jp2::PaletteColumn],
            values: &[i64],
            column: u8,
        ) -> Result<TypedSamples, DecodeError> {
            let descriptor = columns
                .get(usize::from(column))
                .ok_or(malformed(MalformedKind::ContainerCodestreamMismatch))?;
            if descriptor.precision == 0 || descriptor.precision > 38 {
                return Err(malformed(MalformedKind::ContainerCodestreamMismatch));
            }
            let input = match input {
                TypedSamples::Signed(values) => values
                    .iter()
                    .map(|value| i128::from(*value))
                    .collect::<Vec<_>>(),
                TypedSamples::Unsigned(values) => values
                    .iter()
                    .map(|value| i128::from(*value))
                    .collect::<Vec<_>>(),
            };
            let modulus = 2_i128.pow(u32::from(descriptor.precision));
            let (minimum, maximum) = if descriptor.signed {
                (-(modulus / 2), modulus / 2 - 1)
            } else {
                (0, modulus - 1)
            };
            let mut signed = Vec::new();
            let mut unsigned = Vec::new();
            for index in input {
                if index < 0 || index > i128::from(u16::MAX) || index >= i128::from(entries) {
                    return Err(malformed(MalformedKind::ContainerCodestreamMismatch));
                }
                if columns.len() > usize::from(u8::MAX) {
                    return Err(malformed(MalformedKind::ArithmeticOverflow));
                }
                let address = (index as u128) * columns.len() as u128 + u128::from(column);
                if address > usize::MAX as u128 {
                    return Err(malformed(MalformedKind::ArithmeticOverflow));
                }
                let value = *values
                    .get(address as usize)
                    .ok_or(malformed(MalformedKind::InvalidBoxContents))?;
                if i128::from(value) < minimum || i128::from(value) > maximum {
                    return Err(malformed(MalformedKind::ContainerCodestreamMismatch));
                }
                if descriptor.signed {
                    signed.push(value);
                } else {
                    unsigned.push(u64::try_from(value).unwrap());
                }
            }
            Ok(if descriptor.signed {
                TypedSamples::Signed(signed)
            } else {
                TypedSamples::Unsigned(unsigned)
            })
        }

        let tables = [
            vec![],
            vec![0, -1],
            vec![127, -128, 255, 0, i64::MIN, i64::MAX],
            vec![
                (1_i64 << 37) - 1,
                -(1_i64 << 37),
                (1_i64 << 38) - 1,
                1_i64 << 38,
            ],
        ];
        let mut contexts = 0;
        for entries in [0, 1, 2, u16::MAX] {
            for column_count in [0, 1, 2, 256] {
                for column in [0, 1, u8::MAX] {
                    for precision in [0, 1, 8, 16, 38, 39, u8::MAX] {
                        for signed in [false, true] {
                            let columns =
                                vec![jp2::PaletteColumn { precision, signed }; column_count];
                            for values in &tables {
                                let palette = jp2::Palette::for_test(
                                    entries,
                                    columns.clone(),
                                    values.clone(),
                                );
                                for case in 0..14 {
                                    let info = ComponentInfo {
                                        precision: [0, 1, 38, u8::MAX][case % 4],
                                        signed: case % 2 == 0,
                                        x_subsampling: [0, 2, u8::MAX][case % 3],
                                        y_subsampling: [u8::MAX, 0, 1][case % 3],
                                        x_registration: u16::MAX,
                                        y_registration: case as u16,
                                    };
                                    let width = if case % 3 == 0 { 0 } else { 5 };
                                    let height = if case % 5 == 0 { u32::MAX } else { 1 };
                                    let mut source = TypedComponent::from_parts(
                                        width,
                                        height,
                                        info,
                                        samples(case),
                                    );
                                    source.add_channel_definition(ChannelDefinition::new(
                                        ChannelType::Unspecified,
                                        ChannelAssociation::Color(u16::MAX),
                                    ));
                                    source.add_channel_definition(ChannelDefinition::new(
                                        ChannelType::PremultipliedOpacity,
                                        ChannelAssociation::WholeImage,
                                    ));
                                    assert_eq!(copy_typed_component(&source).unwrap(), source);
                                    let expected =
                                        oracle(source.samples(), entries, &columns, values, column);
                                    match (
                                        expected,
                                        map_palette_component(&source, &palette, column),
                                    ) {
                                        (Err(expected), Err(actual)) => {
                                            assert_eq!(actual, expected)
                                        }
                                        (Ok(expected), Ok(actual)) => {
                                            assert_eq!(actual.width(), width);
                                            assert_eq!(actual.height(), height);
                                            assert_eq!(
                                                actual.info(),
                                                ComponentInfo {
                                                    precision,
                                                    signed,
                                                    ..info
                                                }
                                            );
                                            assert_eq!(actual.samples(), &expected);
                                            assert!(actual.channel_definitions().is_empty());
                                        }
                                        (expected, actual) => panic!(
                                            "entries={entries} columns={column_count} column={column} precision={precision} signed={signed} case={case}: expected={expected:?} actual={actual:?}"
                                        ),
                                    }
                                    assert_eq!(source.info(), info);
                                    assert_eq!(source.channel_definitions().len(), 2);
                                    contexts += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(contexts, 37_632);
    }

    #[test]
    fn palette_component_modulus_matches_all_native_precisions() {
        for precision in 0..=u8::MAX {
            let expected = if (1..=38).contains(&precision) {
                Ok((1_u128 << precision) as u64)
            } else {
                Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::ContainerCodestreamMismatch,
                })
            };
            assert_eq!(palette_modulus(precision), expected);
        }
    }

    #[test]
    fn jp2_palette_mapping_preserves_types_channels_and_direct_components() {
        let mut codestream = prepared_codestream(1, 1, 1, &[0, 0x40], &[0]);
        let cod = codestream
            .windows(2)
            .position(|window| window == 0xff52_u16.to_be_bytes())
            .unwrap();
        codestream[cod + 9] = 0;
        let jp2 = jp2_with_palette(&codestream, 256, true);
        let image = decode_jp2(&jp2, &DecodeOptions::default()).unwrap();
        assert_eq!(image.components().len(), 3);
        let palette_unsigned = image.component(0).unwrap();
        assert_eq!(palette_unsigned.info().precision, 8);
        assert!(!palette_unsigned.info().signed);
        assert_eq!(
            palette_unsigned.samples(),
            &TypedSamples::Unsigned(vec![42])
        );
        assert_eq!(
            palette_unsigned.channel_definitions()[0].channel_type(),
            crate::image::ChannelType::Color
        );
        assert_eq!(
            palette_unsigned.channel_definitions()[0].association(),
            crate::image::ChannelAssociation::Color(1)
        );

        let palette_signed = image.component(1).unwrap();
        assert_eq!(palette_signed.info().precision, 5);
        assert!(palette_signed.info().signed);
        assert_eq!(palette_signed.samples(), &TypedSamples::Signed(vec![-3]));
        assert_eq!(
            palette_signed.channel_definitions()[0].channel_type(),
            crate::image::ChannelType::Opacity
        );

        let direct = image.component(2).unwrap();
        assert_eq!(direct.info().precision, 8);
        assert_eq!(direct.samples(), &TypedSamples::Unsigned(vec![128]));
        assert_eq!(
            direct.channel_definitions()[0].channel_type(),
            crate::image::ChannelType::Unspecified
        );
        assert_eq!(
            direct.channel_definitions()[0].association(),
            crate::image::ChannelAssociation::Unassociated
        );
    }

    #[test]
    fn jp2_color_decode_has_explicit_bounded_grayscale_output() {
        let mut codestream = prepared_codestream(1, 1, 1, &[0, 0x40], &[0]);
        let cod = codestream
            .windows(2)
            .position(|window| window == 0xff52_u16.to_be_bytes())
            .unwrap();
        codestream[cod + 9] = 0;
        let jp2 = jp2_for_codestream(&codestream, 1, 1);
        let image = decode_jp2_color(&jp2, &DecodeOptions::default()).unwrap();
        assert_eq!(image.width(), 1);
        assert_eq!(image.height(), 1);
        assert_eq!(
            image.color_space(),
            crate::color::ConvertedColorSpace::Grayscale
        );
        assert_eq!(image.precision(), 16);
        assert_eq!(image.channel(0).unwrap().samples(), &[32_896]);
    }

    #[test]
    fn selected_color_decode_resamples_signed_subsampled_component() {
        let mut codestream = prepared_codestream(4, 1, 1, &[0, 0x40], &[0]);
        let siz = codestream
            .windows(2)
            .position(|window| window == 0xff51_u16.to_be_bytes())
            .unwrap();
        codestream[siz + 40] = 0x87;
        codestream[siz + 41] = 2;
        let cod = codestream
            .windows(2)
            .position(|window| window == 0xff52_u16.to_be_bytes())
            .unwrap();
        codestream[cod + 9] = 0;

        let mut jp2 = box32(*b"jP  ", &[0x0d, 0x0a, 0x87, 0x0a]);
        jp2.extend(box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&4_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[0x87, 7, 0, 0]);
        let mut header = box32(*b"ihdr", &image_header);
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 17]));
        jp2.extend(box32(*b"jp2h", &header));
        jp2.extend(box32(*b"jp2c", &codestream));

        let selection =
            DecodeSelection::full_image().with_region(crate::ImageRegion::new(1, 0, 3, 1));
        let image = decode_jp2_color_selected(&jp2, &DecodeOptions::default(), &selection).unwrap();
        assert_eq!((image.width(), image.height()), (3, 1));
        assert_eq!(
            image.channel(0).unwrap().samples(),
            &[32_896, 32_896, 32_896]
        );
    }

    #[test]
    fn explicitly_selects_typed_jpx_codestream_decode() {
        let mut first = prepared_codestream(1, 1, 1, &[0, 0x40], &[0]);
        let cod = first
            .windows(2)
            .position(|window| window == 0xff52_u16.to_be_bytes())
            .unwrap();
        first[cod + 9] = 0;
        let second = first.clone();
        let jpx = jpx_for_codestreams(&[first, second], 1, 1);
        let options = DecodeOptions::default();
        let image = decode_jpx_codestream(&jpx, 1, &options).unwrap();
        assert_eq!(image.width(), 1);
        assert_eq!(image.height(), 1);
        assert_eq!(
            decode_jpx_codestream_selected(&jpx, 1, &options, &DecodeSelection::full_image(),)
                .unwrap(),
            image
        );
        assert!(matches!(
            decode_jpx_codestream(&jpx, 2, &options),
            Err(DecodeError::CodestreamIndexOutOfRange {
                index: 2,
                codestreams: 2,
            })
        ));
        assert!(matches!(
            decode_jp2(&jpx, &options),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidProfile,
                ..
            })
        ));
        let color = decode_jpx_codestream_color(&jpx, 0, &options).unwrap();
        assert_eq!(
            decode_jpx_codestream_color_selected(
                &jpx,
                0,
                &options,
                &DecodeSelection::full_image(),
            )
            .unwrap(),
            color
        );
        assert_eq!(
            color.color_space(),
            crate::color::ConvertedColorSpace::Grayscale
        );
        assert_eq!(color.channel(0).unwrap().samples(), &[32_896]);
        assert_eq!(
            decode_jpx_codestream_color(&jpx, 1, &options).unwrap(),
            color
        );
        let inspection = crate::inspect_jpx_layers(&jpx, options.limits()).unwrap();
        assert_eq!(inspection.layers().len(), 2);
        assert_eq!(
            crate::render_jpx_layer(&jpx, 1, &options)
                .unwrap()
                .channel(0)
                .unwrap()
                .samples(),
            &[32_896]
        );
        assert_eq!(
            crate::render_jpx_composition(&jpx, &options)
                .unwrap()
                .channel(0)
                .unwrap()
                .samples(),
            &[32_896]
        );
        assert!(matches!(
            crate::render_jpx_frame(&jpx, 0, &options),
            Err(DecodeError::FrameIndexOutOfRange {
                index: 0,
                frames: 0,
            })
        ));
    }

    #[test]
    fn applies_part1_component_transform_per_tile() {
        let component = |component, samples| PreparedTileComponent {
            component,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            fraction_bits: 0,
            samples: vec![samples],
        };
        let mut reversible = vec![component(0, 10), component(1, 4), component(2, 8)];
        apply_prepared_component_transform(&mut reversible, (true, false)).unwrap();
        assert_eq!(
            reversible
                .iter()
                .map(|value| value.samples[0])
                .collect::<Vec<_>>(),
            [15, 7, 11]
        );

        let mut none = vec![component(0, 1), component(1, 2), component(2, 3)];
        apply_prepared_component_transform(&mut none, (false, false)).unwrap();
        assert_eq!(
            none.iter()
                .map(|value| value.samples[0])
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }

    #[test]
    fn applies_ordered_part2_mct_stages_to_raw_jp2_and_selected_jpx_decode() {
        let mut codestream = prepared_codestream(1, 1, 1, &[0, 0x40], &[0]);
        let cod = codestream
            .windows(2)
            .position(|window| window == 0xff52_u16.to_be_bytes())
            .unwrap();
        codestream[cod + 9] = 0;
        let codestream =
            add_second_component_mct_stage(add_single_component_mct(codestream, 2, 3), 3, 5);
        let options = DecodeOptions::default();
        let raw = decode_codestream(&codestream, &options).unwrap();
        assert_eq!(
            raw.component(0).unwrap().samples(),
            &TypedSamples::Unsigned(vec![141])
        );

        let jp2 = jp2_for_codestream(&codestream, 1, 1);
        let boxed = decode_jp2(&jp2, &options).unwrap();
        assert_eq!(
            boxed.component(0).unwrap().samples(),
            &TypedSamples::Unsigned(vec![141])
        );
        let jpx = jpx_for_codestreams(&[codestream.clone(), codestream], 1, 1);
        let selected = decode_jpx_codestream(&jpx, 1, &options).unwrap();
        assert_eq!(
            selected.component(0).unwrap().samples(),
            &TypedSamples::Unsigned(vec![141])
        );
    }

    #[test]
    fn inspects_selects_and_renders_static_jpx_layers() {
        let mut codestream = prepared_codestream(1, 1, 1, &[0, 0x40], &[0]);
        let cod = codestream
            .windows(2)
            .position(|window| window == 0xff52_u16.to_be_bytes())
            .unwrap();
        codestream[cod + 9] = 0;
        let jpx = jpx_for_compositing_layers(&[codestream.clone(), codestream], 1, 1, false);
        let inspection = crate::inspect_jpx_layers(&jpx, &ResourceLimits::default()).unwrap();
        assert_eq!(inspection.layers().len(), 2);
        assert_eq!(inspection.layer(1).unwrap().codestream_indices(), &[1]);
        assert_eq!(inspection.layer(1).unwrap().horizontal_offset(), 0);
        assert_eq!(inspection.composition().unwrap().width(), 2);

        let selected = crate::render_jpx_layer(&jpx, 1, &DecodeOptions::default()).unwrap();
        assert_eq!(selected.channel(0).unwrap().samples(), &[32_896]);
        let rendered = crate::render_jpx_composition(&jpx, &DecodeOptions::default()).unwrap();
        assert_eq!(rendered.width(), 2);
        assert_eq!(rendered.height(), 1);
        assert_eq!(rendered.channel(0).unwrap().samples(), &[32_896, 0]);
        assert_eq!(rendered.channel(1).unwrap().samples(), &[u16::MAX, 0]);
        let tight = ResourceLimits::default()
            .with_maximum_decoded_bytes(10)
            .unwrap();
        assert!(matches!(
            crate::render_jpx_composition(&jpx, &DecodeOptions::new(tight)),
            Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                ..
            })
        ));
    }

    #[test]
    fn renders_jpx_animation_instructions_and_exposes_timing() {
        let mut codestream = prepared_codestream(1, 1, 1, &[0, 0x40], &[0]);
        let cod = codestream
            .windows(2)
            .position(|window| window == 0xff52_u16.to_be_bytes())
            .unwrap();
        codestream[cod + 9] = 0;
        let jpx = jpx_for_compositing_layers(&[codestream], 1, 1, true);
        let inspection = crate::inspect_jpx_layers(&jpx, &ResourceLimits::default()).unwrap();
        let frames = inspection.composition().unwrap().frames();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].duration_milliseconds(), 100);
        let rendered = crate::render_jpx_frame(&jpx, 0, &DecodeOptions::default()).unwrap();
        assert_eq!(rendered.channel(0).unwrap().samples(), &[32_896]);
    }

    #[test]
    fn jp2_palette_mapping_rejects_index_range_and_output_limit() {
        let mut codestream = prepared_codestream(1, 1, 1, &[0, 0x40], &[0]);
        let cod = codestream
            .windows(2)
            .position(|window| window == 0xff52_u16.to_be_bytes())
            .unwrap();
        codestream[cod + 9] = 0;
        let out_of_range = jp2_with_palette(&codestream, 1, false);
        assert!(matches!(
            decode_jp2(&out_of_range, &DecodeOptions::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::ContainerCodestreamMismatch,
                ..
            })
        ));

        let mapped = jp2_with_palette(&codestream, 256, false);
        let limits = ResourceLimits::default()
            .with_maximum_decoded_bytes(16)
            .unwrap();
        assert!(matches!(
            decode_jp2(&mapped, &DecodeOptions::new(limits)),
            Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: 16,
                actual: 24,
            })
        ));
    }

    #[test]
    fn output_shape_contracts_reject_empty_components_and_planes() {
        assert!(crate::refinement::decode_output_shape_valid(
            1, 1, 1, 1, 2, 3
        ));
        for (input, parts, expected, actual, width, height) in [
            (0, 1, 1, 1, 2, 3),
            (1, 0, 1, 1, 2, 3),
            (1, 1, 0, 0, 2, 3),
            (1, 1, 1, 2, 2, 3),
            (1, 1, 1, 1, 0, 3),
            (1, 1, 1, 1, 2, 0),
        ] {
            assert!(!crate::refinement::decode_output_shape_valid(
                input, parts, expected, actual, width, height
            ));
        }
        assert!(crate::refinement::component_output_shape_valid(2, 3, 6));
        assert!(!crate::refinement::component_output_shape_valid(0, 3, 0));
        assert!(!crate::refinement::component_output_shape_valid(2, 0, 0));
        assert!(!crate::refinement::component_output_shape_valid(2, 3, 5));
    }

    #[test]
    fn top_level_raw_and_jp2_decode_packed_packet_headers() {
        for ppm in [true, false] {
            let mut codestream = prepared_codestream(1, 1, 1, &[0, 0x40], &[0]);
            let cod = codestream
                .windows(2)
                .position(|window| window == 0xff52_u16.to_be_bytes())
                .unwrap();
            codestream[cod + 9] = 0;
            let mut packed = pack_packet_header(codestream, ppm, &[0]);
            if ppm {
                let sot = packed
                    .windows(2)
                    .position(|window| window == 0xff90_u16.to_be_bytes())
                    .unwrap();
                packed.splice(sot..sot, segment(0xff57, &[0, 0, 0, 0, 1, 1]));
            }

            let raw = decode_codestream(&packed, &DecodeOptions::default()).unwrap();
            assert_eq!(
                raw.component(0).unwrap().samples(),
                &crate::image::TypedSamples::Unsigned(vec![128])
            );
            let jp2 = jp2_for_codestream(&packed, 1, 1);
            assert_eq!(decode_jp2(&jp2, &DecodeOptions::default()).unwrap(), raw);
        }
    }

    #[test]
    fn top_level_apis_decode_poc_with_plm_plt_and_ppm_ppt() {
        let poc = segment(0xff5f, &[0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 2, 1, 1, 0]);
        for ppm in [true, false] {
            let mut codestream = prepared_codestream(1, 1, 1, &[0, 0x40], &[0, 0]);
            let cod = codestream
                .windows(2)
                .position(|window| window == 0xff52_u16.to_be_bytes())
                .unwrap();
            codestream[cod + 7] = 2;
            codestream[cod + 9] = 0;
            let mut packed = pack_packet_header(codestream, ppm, &[0, 0]);
            let sot = packed
                .windows(2)
                .position(|window| window == 0xff90_u16.to_be_bytes())
                .unwrap();
            packed.splice(sot..sot, poc.clone());
            if ppm {
                let sot = packed
                    .windows(2)
                    .position(|window| window == 0xff90_u16.to_be_bytes())
                    .unwrap();
                packed.splice(sot..sot, segment(0xff57, &[0, 0, 0, 0, 2, 1, 1]));
            } else {
                let sod = packed
                    .windows(2)
                    .position(|window| window == 0xff93_u16.to_be_bytes())
                    .unwrap();
                packed.splice(sod..sod, segment(0xff58, &[0, 1, 1]));
                let sot = packed
                    .windows(2)
                    .position(|window| window == 0xff90_u16.to_be_bytes())
                    .unwrap();
                let eoc = packed.len() - 2;
                let tile_length = u32::try_from(eoc - sot).unwrap();
                packed[sot + 6..sot + 10].copy_from_slice(&tile_length.to_be_bytes());
            }
            let raw = decode_codestream(&packed, &DecodeOptions::default()).unwrap();
            assert_eq!(
                raw.component(0).unwrap().samples(),
                &crate::image::TypedSamples::Unsigned(vec![128])
            );
            let jp2 = jp2_for_codestream(&packed, 1, 1);
            assert_eq!(decode_jp2(&jp2, &DecodeOptions::default()).unwrap(), raw);
        }
    }

    #[test]
    fn packed_headers_honor_plt_sop_and_eph() {
        let mut codestream = prepared_codestream(
            1,
            1,
            1,
            &[0, 0x40],
            &[0, 0xff, 0x92, 0xff, 0x91, 0, 4, 0, 0],
        );
        let cod = codestream
            .windows(2)
            .position(|window| window == 0xff52_u16.to_be_bytes())
            .unwrap();
        codestream[cod + 4] = 0x06;
        codestream[cod + 9] = 0;
        let mut packed = pack_packet_header(codestream, false, &[0, 0xff, 0x92]);
        let sod = packed
            .windows(2)
            .position(|window| window == 0xff93_u16.to_be_bytes())
            .unwrap();
        packed.splice(sod..sod, segment(0xff58, &[0, 9]));
        let sot = packed
            .windows(2)
            .position(|window| window == 0xff90_u16.to_be_bytes())
            .unwrap();
        let eoc = packed.len() - 2;
        let tile_length = u32::try_from(eoc - sot).unwrap();
        packed[sot + 6..sot + 10].copy_from_slice(&tile_length.to_be_bytes());
        decode_codestream(&packed, &DecodeOptions::default()).unwrap();

        let sequence = packed
            .windows(2)
            .position(|window| window == 0xff91_u16.to_be_bytes())
            .unwrap();
        packed[sequence + 5] = 1;
        assert!(matches!(
            decode_codestream(&packed, &DecodeOptions::default()),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidPacketHeader,
                ..
            })
        ));
    }

    #[test]
    fn decodes_single_sample_with_empty_detail_resolution_packet() {
        let mut bytes = prepared_codestream(
            1,
            1,
            1,
            &[0x20, 0x48, 0x48, 0x48, 0x48],
            &[0xc0, 0x24, 0x01, 0x07, 0x74, 0x00, 0x00],
        );
        bytes[6..8].copy_from_slice(&0x4000_u16.to_be_bytes());
        let cod = bytes
            .windows(2)
            .position(|marker| marker == [0xff, 0x52])
            .unwrap();
        bytes[cod + 9] = 1;
        bytes[cod + 12] = 0x40;
        bytes.splice(cod..cod, segment(0xff50, &[0, 2, 0, 0, 0, 1]));
        let image = decode_codestream(&bytes, &DecodeOptions::default()).unwrap();
        assert_eq!(
            image.components()[0].samples(),
            &TypedSamples::Unsigned(vec![0])
        );

        let empty_packet = bytes.len() - 3;
        bytes[empty_packet] = 0x80;
        let present = decode_codestream(&bytes, &DecodeOptions::default()).unwrap();
        assert_eq!(
            present.components()[0].samples(),
            image.components()[0].samples()
        );
    }

    #[test]
    fn decodes_nonzero_samples_with_leading_zero_bitplanes() {
        // Packet bytes independently encoded from constant 8-bit PGM samples by OpenJPEG 2.4.
        for (value, packet) in [
            (1, &[0xcf, 0xb4, 0x04, 0x09][..]),
            (129, &[0xc0, 0x21, 0x03][..]),
            (200, &[0xcf, 0xb4, 0x08, 0x04, 0x4f][..]),
        ] {
            let mut bytes = prepared_codestream(1, 1, 1, &[0x40, 0x40], packet);
            let cod = bytes
                .windows(2)
                .position(|marker| marker == [0xff, 0x52])
                .unwrap();
            bytes[cod + 9] = 0;
            let image = decode_codestream(&bytes, &DecodeOptions::default()).unwrap();
            assert_eq!(
                image.components()[0].samples(),
                &TypedSamples::Unsigned(vec![value]),
                "sample {value}"
            );
        }
    }

    #[test]
    fn decodes_packet_body_through_inverse_quantization() {
        let limits = ResourceLimits::default();
        let mut inclusion = TagTree::new(1, 1, &limits).unwrap();
        let mut zero_bitplanes = TagTree::new(1, 1, &limits).unwrap();
        let mut state = CodeBlockPacketState::new();
        let mut reader = PacketBitReader::new(&[0xca], &limits).unwrap();
        let contribution = decode_code_block_contribution(
            &mut state,
            &mut inclusion,
            &mut zero_bitplanes,
            CodeBlockPacketContext {
                leaf_x: 0,
                leaf_y: 0,
                layer: 0,
                maximum_zero_bitplanes: 1,
                tier1_style: Tier1CodingStyle::default(),
            },
            &mut reader,
        )
        .unwrap()
        .unwrap();
        let body = [0; 5];
        let layers = [CodeBlockLayer {
            contribution: &contribution,
            body: &body,
        }];
        let request = CodeBlockDecodeRequest {
            width: 1,
            height: 1,
            significant_bitplanes: 1,
            orientation: SubbandOrientation::Ll,
            style: Tier1CodingStyle::default(),
            layers: &layers,
            quantization_step: QuantizationStep {
                exponent: 0,
                mantissa: 0,
            },
            dynamic_range: 8,
            reversible: true,
            roi_shift: 0,
        };
        let block = decode_code_block(request, &limits).unwrap();
        assert_eq!(block.width(), 1);
        assert_eq!(block.height(), 1);
        assert_eq!(block.samples().len(), 1);

        let subband = decode_subband(
            1,
            1,
            &[PlacedCodeBlock {
                x: 0,
                y: 0,
                request,
            }],
            &limits,
        )
        .unwrap();
        assert_eq!(subband.samples().len(), 1);
    }

    #[test]
    fn decodes_cleanup_only_ht_packet_body_through_inverse_quantization() {
        let limits = ResourceLimits::default();
        let style = Tier1CodingStyle::from_code_block_style(0x40).unwrap();
        let mut inclusion = TagTree::new(1, 1, &limits).unwrap();
        let mut zero_bitplanes = TagTree::new(1, 1, &limits).unwrap();
        let mut state = CodeBlockPacketState::new();
        let mut reader = PacketBitReader::new(&[0xc8], &limits).unwrap();
        let contribution = decode_code_block_contribution(
            &mut state,
            &mut inclusion,
            &mut zero_bitplanes,
            CodeBlockPacketContext {
                leaf_x: 0,
                leaf_y: 0,
                layer: 0,
                maximum_zero_bitplanes: 1,
                tier1_style: style,
            },
            &mut reader,
        )
        .unwrap()
        .unwrap();
        assert_eq!(contribution.new_passes, 1);
        assert_eq!(contribution.data_length, 4);
        let body = [0xfe, 0x00, 0x63, 0x00];
        let layers = [CodeBlockLayer {
            contribution: &contribution,
            body: &body,
        }];
        let block = decode_code_block(
            CodeBlockDecodeRequest {
                width: 1,
                height: 1,
                significant_bitplanes: 1,
                orientation: SubbandOrientation::Ll,
                style,
                layers: &layers,
                quantization_step: QuantizationStep {
                    exponent: 0,
                    mantissa: 0,
                },
                dynamic_range: 8,
                reversible: true,
                roi_shift: 0,
            },
            &limits,
        )
        .unwrap();
        assert_eq!(block.samples(), &[1]);
    }

    #[test]
    fn decodes_ht_refinement_packet_body_through_inverse_quantization() {
        let limits = ResourceLimits::default();
        let style = Tier1CodingStyle::from_code_block_style(0x40).unwrap();
        let mut inclusion = TagTree::new(1, 1, &limits).unwrap();
        let mut zero_bitplanes = TagTree::new(1, 1, &limits).unwrap();
        let mut state = CodeBlockPacketState::new();
        let mut reader = PacketBitReader::new(&[0xe3, 0x00], &limits).unwrap();
        let contribution = decode_code_block_contribution(
            &mut state,
            &mut inclusion,
            &mut zero_bitplanes,
            CodeBlockPacketContext {
                leaf_x: 0,
                leaf_y: 0,
                layer: 0,
                maximum_zero_bitplanes: 2,
                tier1_style: style,
            },
            &mut reader,
        )
        .unwrap()
        .unwrap();
        assert_eq!(contribution.new_passes, 2);
        assert_eq!(contribution.segment_count(), 2);
        assert_eq!(contribution.data_length, 3);
        let body = [0x00, 0x63, 0x00];
        let layers = [CodeBlockLayer {
            contribution: &contribution,
            body: &body,
        }];
        let block = decode_code_block(
            CodeBlockDecodeRequest {
                width: 1,
                height: 1,
                significant_bitplanes: 2,
                orientation: SubbandOrientation::Ll,
                style,
                layers: &layers,
                quantization_step: QuantizationStep {
                    exponent: 0,
                    mantissa: 0,
                },
                dynamic_range: 8,
                reversible: true,
                roi_shift: 0,
            },
            &limits,
        )
        .unwrap();
        assert_eq!(block.samples(), &[-3]);
    }

    #[test]
    fn top_level_jph_decodes_cleanup_only_ht_coefficients() {
        let mut codestream =
            prepared_codestream(1, 1, 1, &[0, 0x10], &[0xe4, 0xfe, 0x00, 0x63, 0x00]);
        let siz = codestream
            .windows(2)
            .position(|window| window == 0xff51_u16.to_be_bytes())
            .unwrap();
        codestream[siz + 4..siz + 6].copy_from_slice(&0x4000_u16.to_be_bytes());
        let cod = codestream
            .windows(2)
            .position(|window| window == 0xff52_u16.to_be_bytes())
            .unwrap();
        codestream.splice(cod..cod, segment(0xff50, &[0, 2, 0, 0, 0, 8]));
        let cod = codestream
            .windows(2)
            .position(|window| window == 0xff52_u16.to_be_bytes())
            .unwrap();
        codestream[cod + 9] = 0;
        codestream[cod + 12] = 0x40;

        let mut jph = jp2_for_codestream(&codestream, 1, 1);
        jph[20..24].copy_from_slice(b"jph ");
        jph[28..32].copy_from_slice(b"jph ");
        let image = decode_jph(&jph, &DecodeOptions::default()).unwrap();
        assert_eq!(
            image.component(0).unwrap().samples(),
            &crate::image::TypedSamples::Unsigned(vec![129])
        );
    }

    #[test]
    fn top_level_raw_and_jph_decode_ht_refinement() {
        let mut codestream =
            prepared_codestream(1, 1, 1, &[0, 0x18], &[0xf1, 0x80, 0x00, 0x63, 0x00]);
        let siz = codestream
            .windows(2)
            .position(|window| window == 0xff51_u16.to_be_bytes())
            .unwrap();
        codestream[siz + 4..siz + 6].copy_from_slice(&0x4000_u16.to_be_bytes());
        let cod = codestream
            .windows(2)
            .position(|window| window == 0xff52_u16.to_be_bytes())
            .unwrap();
        codestream.splice(cod..cod, segment(0xff50, &[0, 2, 0, 0, 0, 8]));
        let cod = codestream
            .windows(2)
            .position(|window| window == 0xff52_u16.to_be_bytes())
            .unwrap();
        codestream[cod + 9] = 0;
        codestream[cod + 12] = 0x40;

        let raw = decode_codestream(&codestream, &DecodeOptions::default()).unwrap();
        let mut jph = jp2_for_codestream(&codestream, 1, 1);
        jph[20..24].copy_from_slice(b"jph ");
        jph[28..32].copy_from_slice(b"jph ");
        let wrapped = decode_jph(&jph, &DecodeOptions::default()).unwrap();
        assert_eq!(wrapped, raw);
        assert_eq!(
            wrapped.component(0).unwrap().samples(),
            &crate::image::TypedSamples::Unsigned(vec![125])
        );
    }

    #[test]
    fn derives_multiresolution_subbands_and_expounded_steps() {
        let limits = ResourceLimits::default();
        let qcd = [0x40, 0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50];
        let bytes = prepared_codestream(5, 3, 1, &qcd, &[0, 0, 0]);
        let layout = parse_layout(&bytes, &limits).unwrap();
        let subbands = derive_component_subbands(
            &layout.info.image_size,
            layout.info.coding_style,
            &layout.info.quantization,
            0,
        )
        .unwrap();
        assert_eq!(subbands.len(), 7);
        assert_eq!(
            subbands
                .iter()
                .map(|band| (band.orientation, band.width, band.height))
                .collect::<Vec<_>>(),
            vec![
                (SubbandOrientation::Ll, 2, 1),
                (SubbandOrientation::Hl, 1, 1),
                (SubbandOrientation::Lh, 2, 1),
                (SubbandOrientation::Hh, 1, 1),
                (SubbandOrientation::Hl, 2, 2),
                (SubbandOrientation::Lh, 3, 1),
                (SubbandOrientation::Hh, 2, 1),
            ]
        );
        assert_eq!(subbands[0].quantization_index, 0);
        assert_eq!(subbands[6].quantization_index, 6);
        assert_eq!(subbands[6].quantization_step.exponent, 10);
    }

    #[test]
    fn derives_scalar_quantization_steps_by_resolution() {
        let limits = ResourceLimits::default();
        let encoded = (10_u16 << 11) | 5;
        let mut qcd = vec![0x41];
        qcd.extend_from_slice(&encoded.to_be_bytes());
        let bytes = prepared_codestream(5, 3, 0, &qcd, &[0, 0, 0]);
        let layout = parse_layout(&bytes, &limits).unwrap();
        let (index, step) =
            quantization_step_for_subband(&layout.info.quantization, 2, SubbandOrientation::Hh)
                .unwrap();
        assert_eq!(index, 6);
        assert_eq!(step.exponent, 9);
        assert_eq!(step.mantissa, 5);
    }

    #[test]
    fn progression_plan_routes_all_subbands_and_prepares_image() {
        let limits = ResourceLimits::default();
        let qcd = [0x40, 0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50];
        let bytes = prepared_codestream(5, 3, 1, &qcd, &[0, 0, 0]);
        let layout = parse_layout(&bytes, &limits).unwrap();
        assert_eq!(
            layout.info.coding_style.progression_order,
            ProgressionOrder::Lrcp
        );
        let mut plan = plan_progression_tile(
            &layout.info.image_size,
            layout.info.coding_style,
            &layout.info.quantization,
            &limits,
        )
        .unwrap();
        assert_eq!(plan.components()[0].subbands().len(), 7);
        assert_eq!(plan.components()[0].placements().len(), 7);
        assert_eq!(plan.precincts().len(), 3);
        assert_eq!(plan.precincts()[0].subband_count(), 1);
        assert_eq!(plan.precincts()[1].subband_count(), 3);
        let (grid, precincts) = plan.packet_execution();
        let decoded =
            decode_progression_tile_packets(&layout.tile_parts, grid, precincts, &limits).unwrap();
        assert_eq!(decoded.packets, 3);
        assert_eq!(decoded.present_packets, 0);
        let image =
            decode_progression_prepared_image(&layout.info.image_size, &plan, &limits).unwrap();
        assert_eq!(image.width(), 5);
        assert_eq!(image.height(), 3);
        assert_eq!(image.component(0).unwrap().samples(), &[0; 15]);
    }

    #[test]
    fn progression_uses_plt_boundaries_to_slice_retained_packets() {
        let limits = ResourceLimits::default();
        let qcd = [0x40, 0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50];
        let bytes = add_plt(prepared_codestream(5, 3, 1, &qcd, &[0, 0, 0]), &[1, 1, 1]);
        let layout = parse_layout(&bytes, &limits).unwrap();
        assert_eq!(layout.tile_parts[0].packet_lengths.packet_count(), 3);
        let mut plan = plan_progression_tile(
            &layout.info.image_size,
            layout.info.coding_style,
            &layout.info.quantization,
            &limits,
        )
        .unwrap();
        let (grid, precincts) = plan.packet_execution();
        let decoded =
            decode_progression_tile_packets(&layout.tile_parts, grid, precincts, &limits).unwrap();
        assert_eq!(decoded.packets, 3);
        assert_eq!(decoded.payload_bytes, 3);

        let invalid_bytes = add_plt(prepared_codestream(5, 3, 1, &qcd, &[0, 0, 0]), &[2, 1]);
        let invalid_layout = parse_layout(&invalid_bytes, &limits).unwrap();
        let mut invalid_plan = plan_progression_tile(
            &invalid_layout.info.image_size,
            invalid_layout.info.coding_style,
            &invalid_layout.info.quantization,
            &limits,
        )
        .unwrap();
        let (grid, precincts) = invalid_plan.packet_execution();
        assert!(matches!(
            decode_progression_tile_packets(&invalid_layout.tile_parts, grid, precincts, &limits,),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidPacketLength,
                ..
            })
        ));
    }

    #[test]
    fn progression_plan_uses_clipped_multi_tile_bounds() {
        let limits = ResourceLimits::default();
        let bytes = two_tile_codestream();
        let layout = parse_layout(&bytes, &limits).unwrap();
        let left = plan_progression_tile_at(
            &layout.info.image_size,
            0,
            layout.info.coding_style,
            &layout.info.quantization,
            &limits,
        )
        .unwrap();
        let right = plan_progression_tile_at(
            &layout.info.image_size,
            1,
            layout.info.coding_style,
            &layout.info.quantization,
            &limits,
        )
        .unwrap();
        assert_eq!((left.components()[0].x, left.components()[0].width), (0, 2));
        assert_eq!(
            (right.components()[0].x, right.components()[0].width),
            (2, 3)
        );
    }

    #[test]
    fn top_level_decode_assembles_clipped_tiles_and_typed_samples() {
        let image = decode_codestream(&two_tile_codestream(), &DecodeOptions::default()).unwrap();
        assert_eq!((image.width(), image.height()), (5, 1));
        let component = image.component(0).unwrap();
        assert_eq!((component.width(), component.height()), (5, 1));
        assert_eq!(
            component.info(),
            crate::codestream::ComponentInfo {
                precision: 8,
                signed: false,
                x_subsampling: 1,
                y_subsampling: 1,
                x_registration: 0,
                y_registration: 0,
            }
        );
        assert_eq!(
            component.samples(),
            &crate::image::TypedSamples::Unsigned(vec![128; 5])
        );
    }

    #[test]
    fn top_level_jp2_decode_returns_typed_image() {
        let codestream = two_tile_codestream();
        let mut bytes = box32(*b"jP  ", &[0x0d, 0x0a, 0x87, 0x0a]);
        bytes.extend(box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&1_u32.to_be_bytes());
        image_header.extend_from_slice(&5_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut header = box32(*b"ihdr", &image_header);
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 17]));
        bytes.extend(box32(*b"jp2h", &header));
        bytes.extend(box32(*b"jp2c", &codestream));
        let image = decode_jp2(&bytes, &DecodeOptions::default()).unwrap();
        assert_eq!((image.width(), image.height()), (5, 1));
        assert_eq!(
            image.component(0).unwrap().samples(),
            &crate::image::TypedSamples::Unsigned(vec![128; 5])
        );
        let selected =
            decode_jp2_selected(&bytes, &DecodeOptions::default(), &DecodeSelection::tile(1))
                .unwrap();
        assert_eq!((selected.width(), selected.height()), (3, 1));
        assert_eq!(
            selected.component(0).unwrap().samples(),
            &crate::image::TypedSamples::Unsigned(vec![128; 3])
        );
    }

    #[test]
    fn complete_native_selection_and_color_context_match_independent_spatial_reference() {
        fn malformed(kind: MalformedKind) -> DecodeError {
            DecodeError::MalformedInput { offset: 0, kind }
        }
        fn selection_error(reason: SelectionError) -> DecodeError {
            DecodeError::InvalidSelection { reason }
        }
        fn bounded(value: u128) -> Result<u32, DecodeError> {
            if value > u128::from(u32::MAX) {
                Err(malformed(MalformedKind::ArithmeticOverflow))
            } else {
                Ok(value as u32)
            }
        }
        fn tile(size: &ImageSize, index: u16) -> Result<[u32; 4], DecodeError> {
            if size.tiles_across == 0
                || u64::from(index) >= u64::from(size.tiles_across) * u64::from(size.tiles_down)
            {
                return Err(malformed(MalformedKind::InvalidImageGeometry));
            }
            let col = u32::from(index) % size.tiles_across;
            let row = u32::from(index) / size.tiles_across;
            let nx = bounded(u128::from(col) * u128::from(size.tile_width))?;
            let nx = bounded(u128::from(nx) + u128::from(size.tile_x_origin))?;
            let ny = bounded(u128::from(row) * u128::from(size.tile_height))?;
            let ny = bounded(u128::from(ny) + u128::from(size.tile_y_origin))?;
            let ix = bounded(u128::from(size.x_origin) + u128::from(size.width))?;
            let iy = bounded(u128::from(size.y_origin) + u128::from(size.height))?;
            let ex = bounded(u128::from(nx) + u128::from(size.tile_width))?;
            let ey = bounded(u128::from(ny) + u128::from(size.tile_height))?;
            let bounds = [
                nx.max(size.x_origin),
                ny.max(size.y_origin),
                ex.min(ix),
                ey.min(iy),
            ];
            if bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
                Err(malformed(MalformedKind::InvalidImageGeometry))
            } else {
                Ok(bounds)
            }
        }
        fn validate(
            size: &ImageSize,
            request: &DecodeSelection,
        ) -> Result<(Vec<u16>, [u32; 4]), DecodeError> {
            let ix = bounded(u128::from(size.x_origin) + u128::from(size.width))?;
            let iy = bounded(u128::from(size.y_origin) + u128::from(size.height))?;
            let count = u64::from(size.tiles_across) * u64::from(size.tiles_down);
            if size.tiles_across == 0 || count > 65_536 {
                return Err(malformed(MalformedKind::InvalidImageGeometry));
            }
            let explicit = match request.tile_selection() {
                TileSelection::All => vec![],
                TileSelection::Index(index) => vec![*index],
                TileSelection::Set(indices) => {
                    if indices.is_empty() {
                        return Err(selection_error(SelectionError::EmptyTileSet));
                    }
                    indices.clone()
                }
            };
            let mut seen = std::collections::BTreeSet::new();
            for index in &explicit {
                if u64::from(*index) >= count {
                    return Err(selection_error(SelectionError::TileOutOfRange));
                }
                if !seen.insert(*index) {
                    return Err(selection_error(SelectionError::DuplicateTile));
                }
            }
            if explicit.len() > 1 {
                let rows = explicit
                    .iter()
                    .map(|value| u32::from(*value) / size.tiles_across)
                    .collect::<Vec<_>>();
                let columns = explicit
                    .iter()
                    .map(|value| u32::from(*value) % size.tiles_across)
                    .collect::<Vec<_>>();
                let (r0, r1) = (*rows.iter().min().unwrap(), *rows.iter().max().unwrap());
                let (c0, c1) = (
                    *columns.iter().min().unwrap(),
                    *columns.iter().max().unwrap(),
                );
                let area = u64::from(r1 - r0 + 1) * u64::from(c1 - c0 + 1);
                if area > u64::from(u32::MAX) || area as usize != explicit.len() {
                    return Err(selection_error(SelectionError::NonRectangularTileSet));
                }
                for row in r0..=r1 {
                    for col in c0..=c1 {
                        let offset = bounded(u128::from(row) * u128::from(size.tiles_across))?;
                        let index = bounded(u128::from(offset) + u128::from(col))?;
                        if index > u32::from(u16::MAX) {
                            return Err(malformed(MalformedKind::ArithmeticOverflow));
                        }
                        if !seen.contains(&(index as u16)) {
                            return Err(selection_error(SelectionError::NonRectangularTileSet));
                        }
                    }
                }
            }
            let region = if let Some(region) = request.region() {
                if region.width == 0 || region.height == 0 {
                    return Err(selection_error(SelectionError::EmptyRegion));
                }
                let x1 = u64::from(region.x) + u64::from(region.width);
                let y1 = u64::from(region.y) + u64::from(region.height);
                if x1 > u64::from(u32::MAX)
                    || y1 > u64::from(u32::MAX)
                    || region.x < size.x_origin
                    || region.y < size.y_origin
                    || x1 > u64::from(ix)
                    || y1 > u64::from(iy)
                {
                    return Err(selection_error(SelectionError::RegionOutOfBounds));
                }
                [region.x, region.y, x1 as u32, y1 as u32]
            } else if explicit.is_empty() {
                [size.x_origin, size.y_origin, ix, iy]
            } else {
                let mut region = [ix, iy, size.x_origin, size.y_origin];
                for index in &explicit {
                    let bounds = tile(size, *index)?;
                    region[0] = region[0].min(bounds[0]);
                    region[1] = region[1].min(bounds[1]);
                    region[2] = region[2].max(bounds[2]);
                    region[3] = region[3].max(bounds[3]);
                }
                region
            };
            let mut selected = vec![];
            for index in 0..count {
                let bounds = tile(size, index as u16)?;
                if bounds[0] < region[2]
                    && bounds[2] > region[0]
                    && bounds[1] < region[3]
                    && bounds[3] > region[1]
                {
                    if !explicit.is_empty() && !seen.contains(&(index as u16)) {
                        return Err(selection_error(SelectionError::RegionOutsideSelectedTiles));
                    }
                    selected.push(index as u16);
                }
            }
            if selected.is_empty() {
                return Err(selection_error(SelectionError::RegionOutsideSelectedTiles));
            }
            Ok((selected, region))
        }
        fn color(
            size: &ImageSize,
            request: &DecodeSelection,
        ) -> Result<(crate::ImageRegion, [u32; 4], u8), DecodeError> {
            let (_, region) = validate(size, request)?;
            let discard = request.discard_levels();
            if discard >= 32 {
                return Err(selection_error(SelectionError::ReductionTooLarge));
            }
            let scale = 2_u64.pow(u32::from(discard));
            let target = region.map(|value| u64::from(value).div_ceil(scale) as u32);
            let mut horizontal = 0;
            let mut vertical = 0;
            for index in 0..size.components {
                let info = size
                    .component_info
                    .get(usize::from(index))
                    .ok_or(malformed(MalformedKind::InvalidImageGeometry))?;
                if info.x_subsampling == 0 || info.y_subsampling == 0 {
                    return Err(malformed(MalformedKind::InvalidCodeBlock));
                }
                let x = (i128::from(target[0]) * 65_536 - i128::from(info.x_registration)).max(0);
                let y = (i128::from(target[1]) * 65_536 - i128::from(info.y_registration)).max(0);
                if x % (i128::from(info.x_subsampling) * 65_536) != 0 {
                    horizontal = horizontal.max(u32::from(info.x_subsampling));
                }
                if y % (i128::from(info.y_subsampling) * 65_536) != 0 {
                    vertical = vertical.max(u32::from(info.y_subsampling));
                }
            }
            let hx = (u64::from(horizontal) * scale).min(u64::from(u32::MAX));
            let hy = (u64::from(vertical) * scale).min(u64::from(u32::MAX));
            let x0 = (i128::from(region[0]) - i128::from(hx))
                .max(0)
                .max(i128::from(size.x_origin)) as u32;
            let y0 = (i128::from(region[1]) - i128::from(hy))
                .max(0)
                .max(i128::from(size.y_origin)) as u32;
            if region[2] < x0 || region[3] < y0 {
                return Err(malformed(MalformedKind::ArithmeticOverflow));
            }
            Ok((
                crate::ImageRegion::new(x0, y0, region[2] - x0, region[3] - y0),
                target,
                discard,
            ))
        }
        let tiles = [
            TileSelection::All,
            TileSelection::Index(0),
            TileSelection::Index(8),
            TileSelection::Index(9),
            TileSelection::Set(vec![]),
            TileSelection::Set(vec![0, 0]),
            TileSelection::Set(vec![0, 4]),
            TileSelection::Set(vec![4, 0, 3, 1]),
            TileSelection::Set(vec![0, 1]),
        ];
        let regions = [
            None,
            Some(crate::ImageRegion::new(3, 5, 1, 1)),
            Some(crate::ImageRegion::new(3, 5, 13, 9)),
            Some(crate::ImageRegion::new(3, 5, 10, 8)),
            Some(crate::ImageRegion::new(3, 5, 0, 1)),
            Some(crate::ImageRegion::new(2, 5, 1, 1)),
            Some(crate::ImageRegion::new(15, 5, 2, 1)),
            Some(crate::ImageRegion::new(u32::MAX, 5, 2, 1)),
        ];
        let mut contexts = 0;
        for variant in 0..8 {
            for tile_selection in &tiles {
                for region in regions {
                    for discard in [0, 1, 31, 32] {
                        for count in [0, 1, 64, 65] {
                            for sampling in 0..3 {
                                let info = crate::codestream::ComponentInfo {
                                    precision: 0,
                                    signed: true,
                                    x_subsampling: if sampling == 2 { 0 } else { 2 },
                                    y_subsampling: 3,
                                    x_registration: u16::MAX,
                                    y_registration: 1,
                                };
                                let mut size = ImageSize {
                                    width: 13,
                                    height: 9,
                                    x_origin: 3,
                                    y_origin: 5,
                                    components: count,
                                    uniform_bit_depth: None,
                                    uniform_signed: None,
                                    tile_width: 5,
                                    tile_height: 4,
                                    tile_x_origin: 3,
                                    tile_y_origin: 5,
                                    tiles_across: 3,
                                    tiles_down: 3,
                                    component_info: [info; 64],
                                };
                                match variant {
                                    1 => size.width = 0,
                                    2 => size.x_origin = u32::MAX,
                                    3 => size.tiles_across = 0,
                                    4 => {
                                        size.tiles_across = 65_537;
                                        size.tiles_down = 1;
                                    }
                                    5 => size.tile_width = 0,
                                    6 => size.tile_x_origin = u32::MAX,
                                    7 => size.tiles_down = 0,
                                    _ => {}
                                }
                                if sampling == 1 {
                                    size.component_info[0].x_subsampling = 1;
                                    size.component_info[0].y_subsampling = 1;
                                }
                                let request = crate::options::selection_from_parts(
                                    tile_selection.clone(),
                                    region,
                                    discard,
                                );
                                let expected = validate(&size, &request);
                                let actual = validate_selection(&size, &request)
                                    .map(|value| (value.tiles, value.region));
                                assert_eq!(
                                    actual, expected,
                                    "variant={variant} tiles={tile_selection:?} region={region:?}"
                                );
                                let expected = color(&size, &request);
                                let actual = color_selection_for_size(&size, &request).map(
                                    |(selection, target)| {
                                        assert!(matches!(
                                            selection.tile_selection(),
                                            TileSelection::All
                                        ));
                                        (
                                            selection.region().unwrap(),
                                            target,
                                            selection.discard_levels(),
                                        )
                                    },
                                );
                                assert_eq!(
                                    actual, expected,
                                    "variant={variant} discard={discard} count={count} sampling={sampling}"
                                );
                                contexts += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(contexts, 27_648);
    }

    #[test]
    fn associated_native_output_composes_actual_metadata_color_crop_and_none_bypass() {
        let header = jp2::Jp2Header {
            image: jp2::ImageHeader {
                width: u32::MAX,
                height: 0,
                components: 0,
                bit_depth: jp2::ComponentBitDepth::Variable,
                unknown_color_space: true,
                intellectual_property: false,
            },
            color: jp2::ColorSpecification::enumerated(17),
            nested_box_count: u32::MAX,
            palette_entries: u16::MAX,
            palette_columns: u8::MAX,
            component_mapping_entries: u16::MAX,
            channel_definitions: u16::MAX,
            palette: None,
            component_mappings: vec![],
            channel_definition_records: vec![],
            capture_resolution: None,
            display_resolution: None,
        };
        let info = crate::codestream::ComponentInfo {
            precision: 8,
            signed: false,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let image = || {
            TypedImage::for_test(
                2,
                1,
                7,
                11,
                vec![TypedComponent::from_parts(
                    2,
                    1,
                    info,
                    TypedSamples::Unsigned(vec![0, 255]),
                )],
            )
        };
        let limits = ResourceLimits::default();
        let (mapped, selected_header) =
            apply_associated_output(image(), Some(&header), &limits).unwrap();
        assert_eq!(selected_header, Some(&header));
        assert_eq!(
            (
                mapped.width(),
                mapped.height(),
                mapped.x_origin(),
                mapped.y_origin()
            ),
            (2, 1, 7, 11)
        );
        assert_eq!(
            mapped.components()[0].samples(),
            &TypedSamples::Unsigned(vec![0, 255])
        );
        let converted = convert_associated_color(&mapped, selected_header, &limits).unwrap();
        assert_eq!((converted.width(), converted.height()), (2, 1));
        assert_eq!(converted.channels()[0].samples(), &[0, u16::MAX]);
        let cropped =
            convert_associated_selected_color(&mapped, selected_header, [8, 11, 9, 12], &limits)
                .unwrap();
        assert_eq!((cropped.width(), cropped.height()), (1, 1));
        assert_eq!(cropped.channels()[0].samples(), &[u16::MAX]);
        assert_eq!(
            finish_associated_selected_output(image(), Some(&header), [8, 11, 9, 12], &limits),
            Ok(cropped),
        );
        let missing = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::MissingRequiredStructure,
        };
        assert_eq!(
            convert_associated_color(&mapped, None, &limits),
            Err(missing)
        );
        assert_eq!(
            convert_associated_selected_color(&mapped, None, [u32::MAX; 4], &limits),
            Err(missing)
        );
        assert_eq!(
            finish_associated_selected_output(image(), None, [u32::MAX; 4], &limits),
            Err(missing)
        );
        let malformed = || {
            TypedImage::for_test(
                2,
                1,
                u32::MAX,
                u32::MAX,
                vec![TypedComponent::from_parts(
                    3,
                    0,
                    crate::codestream::ComponentInfo {
                        precision: 0,
                        ..info
                    },
                    TypedSamples::Signed(vec![i64::MIN]),
                )],
            )
        };
        let (unchanged, selected_header) =
            apply_associated_output(malformed(), None, &limits).unwrap();
        assert_eq!(unchanged, malformed());
        assert_eq!(selected_header, None);
        assert_eq!(
            apply_associated_output(malformed(), Some(&header), &limits),
            Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::ArithmeticOverflow
            })
        );
        let tiny = limits.with_maximum_decoded_bytes(1).unwrap();
        assert_eq!(
            apply_associated_output(image(), Some(&header), &tiny),
            Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: 1,
                actual: 16
            })
        );
        assert_eq!(
            finish_associated_selected_output(image(), Some(&header), [u32::MAX; 4], &tiny),
            Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: 1,
                actual: 16
            }),
        );
    }

    #[test]
    fn selected_tiles_and_regions_match_full_decode_crops() {
        let codestream = two_tile_codestream();
        let options = DecodeOptions::default();
        let full = decode_codestream(&codestream, &options).unwrap();
        let selected_tile =
            decode_codestream_selected(&codestream, &options, &DecodeSelection::tile(1)).unwrap();
        assert_eq!((selected_tile.width(), selected_tile.height()), (3, 1));
        assert_eq!(
            selected_tile.component(0).unwrap().samples(),
            &crate::image::TypedSamples::Unsigned(vec![128; 3])
        );
        let region = DecodeSelection::full_image().with_region(crate::ImageRegion::new(2, 0, 3, 1));
        let selected_region = decode_codestream_selected(&codestream, &options, &region).unwrap();
        assert_eq!((selected_region.width(), selected_region.height()), (3, 1));
        let crate::image::TypedSamples::Unsigned(full_samples) =
            full.component(0).unwrap().samples()
        else {
            panic!("expected unsigned fixture samples");
        };
        assert_eq!(
            selected_region.component(0).unwrap().samples(),
            &crate::image::TypedSamples::Unsigned(full_samples[1..4].to_vec())
        );
    }

    #[test]
    fn reduced_selection_uses_native_resolution_and_selected_limits() {
        let codestream = prepared_codestream(
            5,
            3,
            1,
            &[0x40, 0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50],
            &[0, 0, 0],
        );
        let limits = ResourceLimits::default()
            .with_maximum_packets(2)
            .unwrap()
            .with_maximum_decoded_bytes(64)
            .unwrap();
        let options = DecodeOptions::new(limits);
        assert!(matches!(
            decode_codestream(&codestream, &options),
            Err(DecodeError::LimitExceeded {
                resource: Resource::Packets,
                ..
            })
        ));
        let reduced = decode_codestream_selected(
            &codestream,
            &options,
            &DecodeSelection::full_image().with_discard_levels(1),
        )
        .unwrap();
        assert_eq!((reduced.width(), reduced.height()), (3, 2));
        assert_eq!(
            reduced.component(0).unwrap().samples(),
            &crate::image::TypedSamples::Unsigned(vec![128; 6])
        );
    }

    #[test]
    fn selected_decode_rejects_malformed_selection_deterministically() {
        let codestream = two_tile_codestream();
        let options = DecodeOptions::default();
        let cases = [
            (
                DecodeSelection::tiles(Vec::new()),
                SelectionError::EmptyTileSet,
            ),
            (
                DecodeSelection::tiles(vec![0, 0]),
                SelectionError::DuplicateTile,
            ),
            (DecodeSelection::tile(2), SelectionError::TileOutOfRange),
            (
                DecodeSelection::full_image().with_region(crate::ImageRegion::new(0, 0, 1, 1)),
                SelectionError::RegionOutOfBounds,
            ),
            (
                DecodeSelection::tile(0).with_region(crate::ImageRegion::new(3, 0, 1, 1)),
                SelectionError::RegionOutsideSelectedTiles,
            ),
            (
                DecodeSelection::full_image().with_region(crate::ImageRegion::new(1, 0, 0, 1)),
                SelectionError::EmptyRegion,
            ),
            (
                DecodeSelection::full_image().with_discard_levels(1),
                SelectionError::ReductionTooLarge,
            ),
        ];
        for (selection, reason) in cases {
            assert_eq!(
                decode_codestream_selected(&codestream, &options, &selection),
                Err(DecodeError::InvalidSelection { reason })
            );
        }
    }

    #[test]
    fn top_level_raw_and_jp2_decode_apply_retained_maxshift() {
        let mut codestream = prepared_codestream(
            5,
            3,
            1,
            &[0x40, 0x40, 0x48, 0x48, 0x50, 0x48, 0x48, 0x50],
            &[0, 0, 0],
        );
        let sot = codestream
            .windows(2)
            .position(|bytes| bytes == 0xff90_u16.to_be_bytes())
            .unwrap();
        codestream.splice(sot..sot, segment(0xff5e, &[0, 0, 4]));

        let limits = ResourceLimits::default();
        let layout = parse_layout(&codestream, &limits).unwrap();
        let plan = plan_progression_tile_at_with_roi(
            &layout.info.image_size,
            0,
            layout.info.coding_style,
            &layout.info.quantization,
            &layout.tile_parts[0].roi_shifts,
            &limits,
        )
        .unwrap();
        assert!(
            plan.components()[0]
                .subbands()
                .iter()
                .all(|subband| subband.roi_shift == 4)
        );
        let raw = decode_codestream(&codestream, &DecodeOptions::default()).unwrap();
        assert_eq!(
            raw.component(0).unwrap().samples(),
            &crate::image::TypedSamples::Unsigned(vec![128; 15])
        );

        let mut jp2 = box32(*b"jP  ", &[0x0d, 0x0a, 0x87, 0x0a]);
        jp2.extend(box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&3_u32.to_be_bytes());
        image_header.extend_from_slice(&5_u32.to_be_bytes());
        image_header.extend_from_slice(&1_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut header = box32(*b"ihdr", &image_header);
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 17]));
        jp2.extend(box32(*b"jp2h", &header));
        jp2.extend(box32(*b"jp2c", &codestream));
        assert_eq!(decode_jp2(&jp2, &DecodeOptions::default()).unwrap(), raw);
    }

    #[test]
    fn top_level_raw_and_jp2_decode_apply_component_overrides() {
        let codestream = component_override_codestream();
        let limits = ResourceLimits::default();
        let layout = parse_layout(&codestream, &limits).unwrap();
        let mut plan = plan_progression_tile_at_with_component_overrides(
            &layout.info.image_size,
            0,
            layout.info.coding_style,
            &layout.tile_parts[0].component_coding_styles,
            &layout.tile_parts[0].component_quantizations,
            &layout.tile_parts[0].roi_shifts,
            &limits,
        )
        .unwrap();
        assert_eq!(plan.components()[0].subbands().len(), 1);
        assert_eq!(plan.components()[1].subbands().len(), 4);
        assert_eq!(
            plan.components()[0].subbands()[0].quantization_step,
            QuantizationStep {
                exponent: 9,
                mantissa: 5,
            }
        );
        assert_eq!(
            plan.components()[1].subbands()[3]
                .quantization_step
                .exponent,
            10
        );
        assert_eq!(plan.grid().packet_count(), 3);
        let (grid, precincts) = plan.packet_execution();
        decode_progression_tile_packets(&layout.tile_parts, grid, precincts, &limits).unwrap();
        let prepared =
            decode_progression_prepared_image(&layout.info.image_size, &plan, &limits).unwrap();
        assert_eq!(prepared.component(0).unwrap().samples(), &[0; 16]);
        assert_eq!(prepared.component(1).unwrap().samples(), &[0; 16]);

        let raw = decode_codestream(&codestream, &DecodeOptions::default()).unwrap();
        assert_eq!(raw.components().len(), 2);
        assert_eq!(
            raw.component(0).unwrap().samples(),
            &crate::image::TypedSamples::Unsigned(vec![128; 16])
        );
        assert_eq!(
            raw.component(1).unwrap().samples(),
            &crate::image::TypedSamples::Unsigned(vec![128; 16])
        );

        let mut jp2 = box32(*b"jP  ", &[0x0d, 0x0a, 0x87, 0x0a]);
        jp2.extend(box32(*b"ftyp", b"jp2 \0\0\0\0jp2 "));
        let mut image_header = Vec::new();
        image_header.extend_from_slice(&4_u32.to_be_bytes());
        image_header.extend_from_slice(&4_u32.to_be_bytes());
        image_header.extend_from_slice(&2_u16.to_be_bytes());
        image_header.extend_from_slice(&[7, 7, 0, 0]);
        let mut header = box32(*b"ihdr", &image_header);
        header.extend(box32(*b"colr", &[1, 0, 0, 0, 0, 0, 17]));
        jp2.extend(box32(*b"jp2h", &header));
        jp2.extend(box32(*b"jp2c", &codestream));
        assert_eq!(decode_jp2(&jp2, &DecodeOptions::default()).unwrap(), raw);
    }

    fn component_override_codestream() -> Vec<u8> {
        let mut siz = Vec::new();
        siz.extend_from_slice(&0_u16.to_be_bytes());
        siz.extend_from_slice(&4_u32.to_be_bytes());
        siz.extend_from_slice(&4_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&4_u32.to_be_bytes());
        siz.extend_from_slice(&4_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&2_u16.to_be_bytes());
        siz.extend_from_slice(&[7, 1, 1, 7, 1, 1]);
        let mut bytes = 0xff4f_u16.to_be_bytes().to_vec();
        bytes.extend(segment(0xff51, &siz));
        bytes.extend(segment(0xff52, &[0, 0, 0, 1, 0, 0, 0, 0, 0, 1]));
        bytes.extend(segment(0xff53, &[1, 0, 1, 0, 0, 0, 1]));
        bytes.extend(segment(0xff5c, &[0x40, 0x40]));
        bytes.extend(segment(0xff5d, &[1, 0x40, 0x40, 0x40, 0x40, 0x40]));
        let tile_start = bytes.len();
        let mut sot = Vec::new();
        sot.extend_from_slice(&0_u16.to_be_bytes());
        sot.extend_from_slice(&0_u32.to_be_bytes());
        sot.extend_from_slice(&[0, 1]);
        bytes.extend(segment(0xff90, &sot));
        let tile_default_step = (9_u16 << 11) | 5;
        let mut tile_qcd = vec![0x41];
        tile_qcd.extend_from_slice(&tile_default_step.to_be_bytes());
        bytes.extend(segment(0xff5c, &tile_qcd));
        bytes.extend(segment(0xff5d, &[1, 0x40, 0x50, 0x50, 0x50, 0x50]));
        bytes.extend_from_slice(&0xff93_u16.to_be_bytes());
        bytes.extend_from_slice(&[0, 0, 0]);
        let length = u32::try_from(bytes.len() - tile_start).unwrap();
        bytes[tile_start + 6..tile_start + 10].copy_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(&0xffd9_u16.to_be_bytes());
        bytes
    }

    fn two_tile_codestream() -> Vec<u8> {
        let mut siz = Vec::new();
        siz.extend_from_slice(&0_u16.to_be_bytes());
        siz.extend_from_slice(&6_u32.to_be_bytes());
        siz.extend_from_slice(&1_u32.to_be_bytes());
        siz.extend_from_slice(&1_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&3_u32.to_be_bytes());
        siz.extend_from_slice(&1_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&0_u32.to_be_bytes());
        siz.extend_from_slice(&1_u16.to_be_bytes());
        siz.extend_from_slice(&[7, 1, 1]);
        let mut bytes = 0xff4f_u16.to_be_bytes().to_vec();
        bytes.extend(segment(0xff51, &siz));
        bytes.extend(segment(0xff52, &[0, 0, 0, 1, 0, 0, 0, 0, 0, 1]));
        bytes.extend(segment(0xff5c, &[0x40, 0x40]));
        for tile_index in 0_u16..2 {
            let start = bytes.len();
            let mut sot = Vec::new();
            sot.extend_from_slice(&tile_index.to_be_bytes());
            sot.extend_from_slice(&0_u32.to_be_bytes());
            sot.extend_from_slice(&[0, 1]);
            bytes.extend(segment(0xff90, &sot));
            bytes.extend_from_slice(&0xff93_u16.to_be_bytes());
            bytes.push(0);
            let length = u32::try_from(bytes.len() - start).unwrap();
            bytes[start + 6..start + 10].copy_from_slice(&length.to_be_bytes());
        }
        bytes.extend_from_slice(&0xffd9_u16.to_be_bytes());
        bytes
    }

    fn box32(box_type: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = u32::try_from(8 + payload.len())
            .unwrap()
            .to_be_bytes()
            .to_vec();
        bytes.extend_from_slice(&box_type);
        bytes.extend_from_slice(payload);
        bytes
    }
}
