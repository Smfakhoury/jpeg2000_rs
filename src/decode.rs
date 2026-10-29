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
include!("decode/proof_support.rs");

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
    decode_typed_layout(input.len(), &layout, options.limits(), None)
}

pub fn decode_codestream_selected(
    input: &[u8],
    options: &DecodeOptions,
    selection: &DecodeSelection,
) -> Result<TypedImage, DecodeError> {
    let layout = codestream::parse_layout(input, options.limits())?;
    decode_typed_layout(input.len(), &layout, options.limits(), Some(selection))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures crate::image::decoded_typed_image_outcome(result) == crate::specs::part2_decode_typed_layout(
        input_bytes, layout, limits, requested,
    ),
))]
fn decode_typed_layout(
    input_bytes: usize,
    layout: &CodestreamLayout<'_>,
    limits: &ResourceLimits,
    requested: Option<&DecodeSelection>,
) -> Result<TypedImage, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part2_decode_typed_layout); }
    let decoded = match requested {
        None => decode_layout(layout, limits)?,
        Some(requested) => decode_layout_selected(layout, requested, limits)?,
    };
    let image = typed_layout(layout, decoded, limits)?;
    validate_part1_output_shape(input_bytes, layout, &image)?;
    Ok(image)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_validate_typed_output_shape(
        input_bytes, layout.tile_parts.len(), layout.info.image_size.components,
        crate::image::typed_image_value_view(*image),
    ),
))]
fn validate_part1_output_shape(
    input_bytes: usize,
    layout: &CodestreamLayout<'_>,
    image: &TypedImage,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost components = crate::image::typed_image_component_views(image);
        let ghost goal = crate::specs::part1_validate_typed_output_shape(
            input_bytes, layout.tile_parts.len(), layout.info.image_size.components,
            crate::image::typed_image_value_view(*image),
        );
    }
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
            component_count == components.len(),
            components == crate::image::typed_image_component_views(image),
            goal == crate::specs::part1_validate_typed_output_shape(
                input_bytes, layout.tile_parts.len(), layout.info.image_size.components,
                crate::image::typed_image_value_view(*image),
            ),
            goal == crate::specs::part1_validate_typed_output_components(components, component as nat),
            decreases component_count - component,
        )
    )]
    while component < component_count {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal_with_fuel(crate::specs::part1_validate_typed_output_components, 1);
        }
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
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal_with_fuel(crate::specs::part1_validate_typed_output_components, 1);
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures crate::image::decoded_typed_image_outcome(result) == crate::specs::part2_typed_layout(
        layout, crate::image::decoded_image_view(decoded), limits,
    ),
))]
fn typed_layout(
    layout: &CodestreamLayout<'_>,
    decoded: DecodedImage,
    limits: &ResourceLimits,
) -> Result<TypedImage, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part2_typed_layout); }
    let mut image = decoded.typed(limits)?;
    let regions = collect_nonlinear_regions(layout, &image)?;
    if !regions.is_empty() {
        image.apply_nonlinear_regions(&regions, limits)?;
    }
    Ok(image)
}

type NonlinearComponentExtent = ((u32, u32, u32, u32), u64);

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_nonlinear_component_extent(
        size, crate::image::typed_image_value_view(*image), component,
    ),
))]
fn nonlinear_component_extent(
    size: &codestream::ImageSize,
    image: &TypedImage,
    component: u16,
) -> Result<NonlinearComponentExtent, DecodeError> {
    let info = size.component(component)?;
    let output = image.component(component)?;
    let x0 = registered_ceil_div_u32(
        image.x_origin(),
        u32::from(info.x_subsampling),
        info.x_registration,
    )?;
    let y0 = registered_ceil_div_u32(
        image.y_origin(),
        u32::from(info.y_subsampling),
        info.y_registration,
    )?;
    let x1 = x0
        .checked_add(output.width())
        .ok_or_else(arithmetic_overflow)?;
    let y1 = y0
        .checked_add(output.height())
        .ok_or_else(arithmetic_overflow)?;
    let count = size.tile_count()?;
    Ok(((x0, y0, x1, y1), count))
}

#[cfg(verus_keep_ghost)]
verus! {
    pub(crate) open spec fn nonlinear_region_view(region: &NonlinearRegion<'_>) -> crate::specs::Part2NonlinearRegion {
        crate::specs::Part2NonlinearRegion {
            component: region.component, x0: region.x0, y0: region.y0, x1: region.x1, y1: region.y1,
            transform: codestream::owned_nonlinear_view(region.transform),
        }
    }

    pub(crate) open spec fn nonlinear_regions_view(regions: Seq<NonlinearRegion<'_>>) -> Seq<crate::specs::Part2NonlinearRegion> {
        regions.map_values(|region: NonlinearRegion<'_>| nonlinear_region_view(&region))
    }

    pub(crate) open spec fn nonlinear_regions_outcome(result: Result<Vec<NonlinearRegion<'_>>, DecodeError>)
        -> Result<Seq<crate::specs::Part2NonlinearRegion>, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(regions) => Ok(nonlinear_regions_view(regions@)) }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures nonlinear_regions_outcome(result) == crate::specs::part2_collect_nonlinear_regions(
        layout, crate::image::typed_image_value_view(*image),
    ),
))]
fn collect_nonlinear_regions<'a>(
    layout: &'a CodestreamLayout<'_>,
    image: &TypedImage,
) -> Result<Vec<NonlinearRegion<'a>>, DecodeError> {
    let mut regions = Vec::new();
    let mut component = 0_u16;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost image_view = crate::image::typed_image_value_view(*image);
        let ghost goal = crate::specs::part2_collect_nonlinear_regions(layout, image_view);
    }
    #[cfg(verus_keep_ghost)]
    proof! { assert(nonlinear_regions_view(regions@) =~= Seq::empty()); }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= layout.info.image_size.components,
            image_view == crate::image::typed_image_value_view(*image),
            goal == crate::specs::part2_collect_nonlinear_regions(layout, image_view),
            goal == crate::specs::part2_nonlinear_component_regions(layout, image_view, component as nat, nonlinear_regions_view(regions@)),
            decreases layout.info.image_size.components - component,
        )
    )]
    while component < layout.info.image_size.components {
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part2_nonlinear_component_regions, 1); }
        let (extent, tile_count) =
            nonlinear_component_extent(&layout.info.image_size, image, component)?;
        let (output_x0, output_y0, output_x1, output_y1) = extent;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost tile_goal = crate::specs::part2_nonlinear_tile_regions(
                layout, component, extent, tile_count, 0, nonlinear_regions_view(regions@),
            );
        }
        let mut tile = 0_u64;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant tile <= tile_count,
                image_view == crate::image::typed_image_value_view(*image),
                component < layout.info.image_size.components,
                goal == crate::specs::part2_collect_nonlinear_regions(layout, image_view),
                goal == match tile_goal {
                    Err(error) => Err(error),
                    Ok(regions) => crate::specs::part2_nonlinear_component_regions(layout, image_view, component as nat + 1, regions),
                },
                tile_goal == crate::specs::part2_nonlinear_tile_regions(layout, component, extent, tile_count, tile as nat, nonlinear_regions_view(regions@)),
                extent == (output_x0, output_y0, output_x1, output_y1),
                decreases tile_count - tile,
            )
        )]
        while tile < tile_count {
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(crate::specs::part2_nonlinear_tile_regions, 1); }
            let tile_index = match u16::try_from(tile) {
                Ok(index) => index,
                Err(_) => return Err(arithmetic_overflow()),
            };
            let mut extensions = &layout.info.part2_extensions;
            let mut part = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant part <= layout.tile_parts.len(),
                    codestream::owned_extensions_view(extensions) == crate::specs::part2_nonlinear_tile_extensions(
                        codestream::owned_extensions_view(&layout.info.part2_extensions),
                        codestream::tile_parts_owned_view(layout.tile_parts@), tile_index, part as nat,
                    ),
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
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost before = regions@;
                let ghost region = crate::specs::part2_nonlinear_tile_region(layout, component, tile_index, extent);
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
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(nonlinear_regions_view(regions@) =~= match region {
                    Err(_) => nonlinear_regions_view(before),
                    Ok(None) => nonlinear_regions_view(before),
                    Ok(Some(region)) => nonlinear_regions_view(before).push(region),
                });
            }
            tile = tile.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(crate::specs::part2_nonlinear_tile_regions, 1); }
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal_with_fuel(crate::specs::part2_nonlinear_component_regions, 1); }
    Ok(regions)
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
mod tests;
