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

pub fn derive_component_resolution_geometry(
    size: &ImageSize,
    coding_style: CodingStyle,
    component: u16,
    resolution: u8,
) -> Result<ComponentResolutionGeometry, DecodeError> {
    derive_component_resolution_geometry_for(size, coding_style, component, resolution, None)
}

fn derive_component_resolution_geometry_for(
    size: &ImageSize,
    coding_style: CodingStyle,
    component: u16,
    resolution: u8,
    tile_index: Option<u16>,
) -> Result<ComponentResolutionGeometry, DecodeError> {
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

fn derive_packet_grid_for_components(
    size: &ImageSize,
    coding_styles: &ComponentCodingStyles,
    layers: u16,
    limits: &ResourceLimits,
    tile_index: Option<u16>,
    discard_levels: u8,
) -> Result<PacketGrid, DecodeError> {
    let mut maximum_resolutions = 0_u8;
    let mut component_resolutions = Vec::with_capacity(usize::from(size.components));
    let mut selected_resolution_counts = Vec::with_capacity(usize::from(size.components));
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= size.components,
            decreases size.components - component,
        )
    )]
    while component < size.components {
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
    let capacity = usize::from(size.components)
        .checked_mul(usize::from(maximum_resolutions))
        .ok_or_else(arithmetic_overflow)?;
    let mut precinct_counts = Vec::with_capacity(capacity);
    let mut total_code_blocks = 0_u64;
    component = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= size.components,
            decreases size.components - component,
        )
    )]
    while component < size.components {
        let style = coding_styles.style(component)?;
        let resolutions = style
            .decomposition_levels
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        let mut resolution = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant resolution <= maximum_resolutions,
                decreases maximum_resolutions - resolution,
            )
        )]
        while resolution < maximum_resolutions {
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
                        return Err(DecodeError::limit(
                            crate::Resource::CodeBlocks,
                            limits.maximum_code_blocks(),
                            total_code_blocks,
                        ));
                    }
                }
                precinct_counts.push(if subband_count == 0 {
                    0
                } else {
                    u32::try_from(geometry.precincts).map_err(|_| arithmetic_overflow())?
                });
            } else {
                precinct_counts.push(0);
            }
            resolution = resolution.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    PacketGrid::new_with_selected_resolutions(
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

    fn optional_mct_equal(
        left: Option<&codestream::MultipleComponentTransform>,
        right: Option<&codestream::MultipleComponentTransform>,
    ) -> bool {
        match (left, right) {
            (Some(left), Some(right)) => left.equals(right),
            (None, None) => true,
            _ => false,
        }
    }

    fn apply_tile_multiple_component_transform(
        prepared: &mut [PreparedTileComponent],
        size: &ImageSize,
        transform: &codestream::MultipleComponentTransform,
        limits: &ResourceLimits,
        work: &mut u64,
    ) -> Result<(), DecodeError> {
        let mut components = Vec::with_capacity(prepared.len());
        Self::collect_prepared_transform_components(prepared, size, &mut components)?;
        apply_multiple_component_transform_components(&mut components, transform, limits, work)
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases prepared.len(),)
    )]
    fn collect_prepared_transform_components<'a>(
        prepared: &'a mut [PreparedTileComponent],
        size: &ImageSize,
        components: &mut Vec<TransformComponentMut<'a>>,
    ) -> Result<(), DecodeError> {
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
        Self::collect_prepared_transform_components(remaining, size, components)
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
        u16::from(resolution)
            .checked_sub(1)
            .and_then(|level| level.checked_mul(3))
            .and_then(|index| index.checked_add(1))
            .and_then(|index| index.checked_add(offset))
            .ok_or_else(arithmetic_overflow)?
    };
    let step = match quantization.style() {
        QuantizationStyle::None | QuantizationStyle::ScalarExpounded => {
            quantization.step(orientation_index)?
        }
        QuantizationStyle::ScalarDerived => {
            let base = quantization.step(0)?;
            let reduction = resolution.saturating_sub(1);
            QuantizationStep {
                exponent: base.exponent.saturating_sub(reduction),
                mantissa: base.mantissa,
            }
        }
    };
    Ok((orientation_index, step))
}

pub fn derive_component_subbands(
    size: &ImageSize,
    coding_style: CodingStyle,
    quantization: &Quantization,
    component: u16,
) -> Result<Vec<SubbandGeometry>, DecodeError> {
    derive_component_subbands_for(size, coding_style, quantization, component, None, 0)
}

fn derive_component_subbands_for(
    size: &ImageSize,
    coding_style: CodingStyle,
    quantization: &Quantization,
    component: u16,
    tile_index: Option<u16>,
    roi_shift: u8,
) -> Result<Vec<SubbandGeometry>, DecodeError> {
    let info = size.component(component)?;
    let component_bounds = component_coordinate_bounds(size, component, tile_index)?;
    let component_x0 = component_bounds[0];
    let component_y0 = component_bounds[1];
    let component_x1 = component_bounds[2];
    let component_y1 = component_bounds[3];
    let expected_count = usize::from(coding_style.subband_count());
    let mut subbands = Vec::with_capacity(expected_count);
    let mut resolution = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant resolution as int <= coding_style.decomposition_levels as int + 1,
            decreases coding_style.decomposition_levels as int + 1 - resolution as int,
        )
    )]
    while resolution <= coding_style.decomposition_levels {
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
                [info.precision, roi_shift],
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
                let index = u16::try_from(subbands.len()).map_err(|_| arithmetic_overflow())?;
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
                    [info.precision, roi_shift],
                    quantization,
                    index,
                )?);
            }
            if matches!(
                direction,
                crate::codestream::DecompositionDirection::TwoDimensional
                    | crate::codestream::DecompositionDirection::Vertical
            ) {
                let index = u16::try_from(subbands.len()).map_err(|_| arithmetic_overflow())?;
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
                    [info.precision, roi_shift],
                    quantization,
                    index,
                )?);
            }
            if matches!(
                direction,
                crate::codestream::DecompositionDirection::TwoDimensional
            ) {
                let index = u16::try_from(subbands.len()).map_err(|_| arithmetic_overflow())?;
                subbands.push(make_subband_geometry(
                    resolution,
                    SubbandOrientation::Hh,
                    [high_x0, high_y0, high_x1, high_y1],
                    [resolution_width, resolution_height],
                    phase,
                    [info.precision, roi_shift],
                    quantization,
                    index,
                )?);
            }
        }
        resolution = resolution.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    if subbands.len() != expected_count {
        return Err(invalid_decode());
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

#[allow(clippy::too_many_arguments)]
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
    size.tile_bounds(tile_index)?;
    let grid = derive_packet_grid_for_components(
        size,
        coding_styles,
        coding_style.layers,
        limits,
        Some(tile_index),
        discard_levels,
    )?;
    let mut precincts = Vec::new();
    let mut components = Vec::with_capacity(usize::from(size.components));
    let mut planned_code_blocks = 0_u64;
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= size.components,
            decreases size.components - component,
        )
    )]
    while component < size.components {
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
        let (scale_x, scale_y) =
            component_coding_style.resolution_downsampling(output_resolution)?;
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
        let mut placements = Vec::new();
        let mut resolution = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant resolution
                    <= component_coding_style.decomposition_levels as int + 1,
                decreases component_coding_style.decomposition_levels as int + 1
                    - resolution as int,
            )
        )]
        while resolution <= component_coding_style.decomposition_levels {
            let geometry = derive_component_resolution_geometry_for(
                size,
                component_coding_style,
                component,
                resolution,
                Some(tile_index),
            )?;
            let first_subband = if resolution == 0 {
                0_usize
            } else {
                component_coding_style.subband_count_through_resolution(
                    resolution.checked_sub(1).ok_or_else(arithmetic_overflow)?,
                )?
            };
            let subband_count = component_coding_style
                .subband_count_through_resolution(resolution)?
                .checked_sub(first_subband)
                .ok_or_else(arithmetic_overflow)?;
            let precincts_in_resolution = if subband_count == 0 {
                0
            } else {
                u32::try_from(geometry.precincts).map_err(|_| arithmetic_overflow())?
            };
            let (precinct_x_exponent, precinct_y_exponent) =
                component_coding_style.precinct_exponents(resolution)?;
            if precinct_x_exponent > 31 || precinct_y_exponent > 31 {
                return Err(invalid_decode());
            }
            let (resolution_scale_x, resolution_scale_y) =
                component_coding_style.resolution_downsampling(resolution)?;
            let resolution_x0 = ceil_div_u32(bounds.x0, resolution_scale_x)?;
            let resolution_y0 = ceil_div_u32(bounds.y0, resolution_scale_y)?;
            let precinct_width = 1_u32 << u32::from(precinct_x_exponent);
            let precinct_height = 1_u32 << u32::from(precinct_y_exponent);
            let first_precinct_x = floor_div_u32(resolution_x0, precinct_width)?;
            let first_precinct_y = floor_div_u32(resolution_y0, precinct_height)?;
            let direction = if resolution == 0 {
                crate::codestream::DecompositionDirection::None
            } else {
                component_coding_style.direction_for_resolution(resolution)?
            };
            let split_x = matches!(
                direction,
                crate::codestream::DecompositionDirection::Horizontal
                    | crate::codestream::DecompositionDirection::TwoDimensional
            );
            let split_y = matches!(
                direction,
                crate::codestream::DecompositionDirection::Vertical
                    | crate::codestream::DecompositionDirection::TwoDimensional
            );
            let band_x_exponent = if split_x {
                precinct_x_exponent
                    .checked_sub(1)
                    .ok_or_else(invalid_decode)?
            } else {
                precinct_x_exponent
            };
            let band_y_exponent = if split_y {
                precinct_y_exponent
                    .checked_sub(1)
                    .ok_or_else(invalid_decode)?
            } else {
                precinct_y_exponent
            };
            if band_x_exponent > 31 || band_y_exponent > 31 {
                return Err(invalid_decode());
            }
            let band_precinct_width = 1_u32 << u32::from(band_x_exponent);
            let band_precinct_height = 1_u32 << u32::from(band_y_exponent);
            let code_block_width =
                band_precinct_width.min(u32::from(component_coding_style.code_block_width));
            let code_block_height =
                band_precinct_height.min(u32::from(component_coding_style.code_block_height));
            if precinct_width == 0
                || precinct_height == 0
                || code_block_width == 0
                || code_block_height == 0
            {
                return Err(invalid_decode());
            }
            let mut precinct = 0_u32;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant precinct <= precincts_in_resolution,
                    decreases precincts_in_resolution - precinct,
                )
            )]
            while precinct < precincts_in_resolution {
                let precinct_x = first_precinct_x
                    .checked_add(
                        precinct
                            .checked_rem(geometry.precincts_across)
                            .ok_or_else(arithmetic_overflow)?,
                    )
                    .ok_or_else(arithmetic_overflow)?;
                let precinct_y = first_precinct_y
                    .checked_add(
                        precinct
                            .checked_div(geometry.precincts_across)
                            .ok_or_else(arithmetic_overflow)?,
                    )
                    .ok_or_else(arithmetic_overflow)?;
                let band_x = precinct_x
                    .checked_mul(band_precinct_width)
                    .ok_or_else(arithmetic_overflow)?;
                let band_y = precinct_y
                    .checked_mul(band_precinct_height)
                    .ok_or_else(arithmetic_overflow)?;
                let precinct_index = precincts.len();
                let mut executions = Vec::with_capacity(subband_count);
                let mut flattened_decoder_index = 0_usize;
                let mut local_subband = 0_usize;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant local_subband <= subband_count,
                        decreases subband_count - local_subband,
                    )
                )]
                while local_subband < subband_count {
                    let subband_index = first_subband
                        .checked_add(local_subband)
                        .ok_or_else(arithmetic_overflow)?;
                    let subband = *subbands.get(subband_index).ok_or_else(invalid_decode)?;
                    let start_x = band_x.max(subband.x0);
                    let start_y = band_y.max(subband.y0);
                    let subband_x1 = subband
                        .x0
                        .checked_add(subband.width)
                        .ok_or_else(arithmetic_overflow)?;
                    let subband_y1 = subband
                        .y0
                        .checked_add(subband.height)
                        .ok_or_else(arithmetic_overflow)?;
                    if start_x < subband_x1 && start_y < subband_y1 {
                        let end_x = band_x
                            .checked_add(band_precinct_width)
                            .ok_or_else(arithmetic_overflow)?
                            .min(subband_x1);
                        let end_y = band_y
                            .checked_add(band_precinct_height)
                            .ok_or_else(arithmetic_overflow)?
                            .min(subband_y1);
                        let first_block_x = floor_div_u32(start_x, code_block_width)?;
                        let first_block_y = floor_div_u32(start_y, code_block_height)?;
                        let blocks_across = ceil_div_u32(end_x, code_block_width)?
                            .checked_sub(first_block_x)
                            .ok_or_else(arithmetic_overflow)?;
                        let blocks_down = ceil_div_u32(end_y, code_block_height)?
                            .checked_sub(first_block_y)
                            .ok_or_else(arithmetic_overflow)?;
                        let count = u64::from(blocks_across)
                            .checked_mul(u64::from(blocks_down))
                            .ok_or_else(arithmetic_overflow)?;
                        if resolution <= output_resolution {
                            planned_code_blocks = planned_code_blocks
                                .checked_add(count)
                                .ok_or_else(arithmetic_overflow)?;
                            if planned_code_blocks > limits.maximum_code_blocks() {
                                return Err(DecodeError::limit(
                                    crate::Resource::CodeBlocks,
                                    limits.maximum_code_blocks(),
                                    planned_code_blocks,
                                ));
                            }
                            if planned_code_blocks > limits.maximum_work_units() {
                                return Err(DecodeError::limit(
                                    crate::Resource::WorkUnits,
                                    limits.maximum_work_units(),
                                    planned_code_blocks,
                                ));
                            }
                        }
                        let mut decoders = Vec::with_capacity(
                            usize::try_from(count).map_err(|_| arithmetic_overflow())?,
                        );
                        let mut block_y = 0_u32;
                        #[cfg_attr(
                            verus_keep_ghost,
                            verus_spec(
                                invariant block_y <= blocks_down,
                                decreases blocks_down - block_y,
                            )
                        )]
                        while block_y < blocks_down {
                            let mut block_x = 0_u32;
                            #[cfg_attr(
                                verus_keep_ghost,
                                verus_spec(
                                    invariant block_x <= blocks_across,
                                    decreases blocks_across - block_x,
                                )
                            )]
                            while block_x < blocks_across {
                                let nominal_x = first_block_x
                                    .checked_add(block_x)
                                    .and_then(|index| index.checked_mul(code_block_width))
                                    .ok_or_else(arithmetic_overflow)?;
                                let nominal_y = first_block_y
                                    .checked_add(block_y)
                                    .and_then(|index| index.checked_mul(code_block_height))
                                    .ok_or_else(arithmetic_overflow)?;
                                let x = nominal_x.max(start_x);
                                let y = nominal_y.max(start_y);
                                let nominal_x1 = nominal_x
                                    .checked_add(code_block_width)
                                    .ok_or_else(arithmetic_overflow)?;
                                let nominal_y1 = nominal_y
                                    .checked_add(code_block_height)
                                    .ok_or_else(arithmetic_overflow)?;
                                let block_width = u16::try_from(
                                    nominal_x1
                                        .min(end_x)
                                        .checked_sub(x)
                                        .ok_or_else(arithmetic_overflow)?,
                                )
                                .map_err(|_| arithmetic_overflow())?;
                                let block_height = u16::try_from(
                                    nominal_y1
                                        .min(end_y)
                                        .checked_sub(y)
                                        .ok_or_else(arithmetic_overflow)?,
                                )
                                .map_err(|_| arithmetic_overflow())?;
                                let mut decoder = Tier1CodeBlockDecoder::new(
                                    block_width,
                                    block_height,
                                    subband.significant_bitplanes,
                                    subband.orientation,
                                    tier1_style,
                                )?;
                                decoder.set_multiple_ht_sets_allowed(allow_multiple_ht_sets);
                                decoder.set_ht_magnitude_bound(ht_magnitude_bound)?;
                                decoders.push(decoder);
                                placements.push(ProgressionCodeBlockPlacement {
                                    precinct_index,
                                    decoder_index: flattened_decoder_index,
                                    subband_index,
                                    x: x.checked_sub(subband.x0).ok_or_else(arithmetic_overflow)?,
                                    y: y.checked_sub(subband.y0).ok_or_else(arithmetic_overflow)?,
                                });
                                flattened_decoder_index = flattened_decoder_index
                                    .checked_add(1)
                                    .ok_or_else(arithmetic_overflow)?;
                                block_x = block_x.checked_add(1).ok_or_else(arithmetic_overflow)?;
                            }
                            block_y = block_y.checked_add(1).ok_or_else(arithmetic_overflow)?;
                        }
                        executions.push(PacketSubbandExecution {
                            orientation: subband.orientation,
                            code_blocks_across: blocks_across,
                            code_blocks_down: blocks_down,
                            maximum_zero_bitplanes: subband.significant_bitplanes,
                            decoders,
                        });
                    }
                    local_subband = local_subband
                        .checked_add(1)
                        .ok_or_else(arithmetic_overflow)?;
                }
                precincts.push(PacketPrecinctDecoder::new_subbands(
                    component,
                    resolution,
                    precinct,
                    tier1_style,
                    executions,
                    limits,
                )?);
                precinct = precinct.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            resolution = resolution.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        components.push(PlannedTileComponent {
            component,
            x: component_x,
            y: component_y,
            width: component_width,
            height: component_height,
            output_resolution,
            subbands,
            placements,
        });
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
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

    pub closed spec fn prepared_component_view(value: PreparedTileComponent) -> crate::specs::Part1AssemblyTile {
        (value.component, value.x, value.y, value.width, value.height, value.fraction_bits, value.samples@)
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
fn make_subband_geometry(
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
        return Err(DecodeError::malformed(
            0,
            MalformedKind::InvalidQuantization,
        ));
    }
    let quantization_step = match quantization.style() {
        QuantizationStyle::None | QuantizationStyle::ScalarExpounded => {
            quantization.step(quantization_index)?
        }
        QuantizationStyle::ScalarDerived => {
            let base = quantization.step(0)?;
            QuantizationStep {
                exponent: base.exponent.saturating_sub(resolution.saturating_sub(1)),
                mantissa: base.mantissa,
            }
        }
    };
    let significant_bitplanes = u16::from(quantization.guard_bits())
        .checked_add(u16::from(quantization_step.exponent))
        .and_then(|value| value.checked_sub(1))
        .and_then(|value| value.checked_add(u16::from(roi_shift)))
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
    ensures kernel.is_none() ==> prepared_component_outcome(result) == crate::specs::part1_prepare_standard_component(
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
    proof! { reveal(crate::specs::part1_prepare_standard_component); }
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
    ensures kernel.is_none() ==> prepare_fold_outcome(result) == crate::specs::part1_prepare_reconstruct(
        working_bands_view(working@), prepare_plan_view(plan), prepare_style_view(coding_style), reversible,
        crate::limits::resource_decoded_bytes_limit(limits),
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
    proof! { crate::proofs::production_impl::production_prepare_fold_empty(initial, working_bands_view(working@), plan.subbands@, prepare_style_view(coding_style), reversible, crate::limits::resource_decoded_bytes_limit(limits)); }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant 1 <= resolution as int <= plan.output_resolution as int + 1,
        working.len() > 0, ll == &working@[0],
        initial == (ll.width, ll.height, ll.samples@, 1usize),
        kernel.is_none() ==> crate::specs::part1_prepare_fold(
            initial, working_bands_view(working@), plan.subbands@, prepare_style_view(coding_style), reversible,
            crate::limits::resource_decoded_bytes_limit(limits), (resolution - 1) as nat,
        ) == Ok(folded@),
        decreases plan.output_resolution as int + 1 - resolution as int,
    ))]
    while resolution <= plan.output_resolution {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_prepare_fold_step(initial, working_bands_view(working@), plan.subbands@, prepare_style_view(coding_style), reversible, crate::limits::resource_decoded_bytes_limit(limits), (resolution - 1) as nat);
            crate::proofs::production_impl::production_prepare_fold_error_guard(initial, working_bands_view(working@), plan.subbands@, prepare_style_view(coding_style), reversible, crate::limits::resource_decoded_bytes_limit(limits), resolution as nat, plan.output_resolution as nat);
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
    ensures kernel.is_none() ==> prepare_fold_outcome(result) == crate::specs::part1_prepare_resolution(
        before@, working_bands_view(working@), plan.subbands@, direction, reversible, crate::limits::resource_decoded_bytes_limit(limits),
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
    proof! { reveal(crate::specs::part1_prepare_resolution); }
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

pub fn decode_progression_prepared_image(
    size: &ImageSize,
    plan: &ProgressionTilePlan,
    limits: &ResourceLimits,
) -> Result<DecodedImage, DecodeError> {
    let mut prepared = Vec::with_capacity(plan.components.len());
    let mut component_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component_index <= plan.components.len(),
            decreases plan.components.len() - component_index,
        )
    )]
    while component_index < plan.components.len() {
        let component = plan
            .components
            .get(component_index)
            .ok_or_else(invalid_decode)?;
        prepared.push(prepare_tile_component(
            component,
            &plan.precincts,
            plan.component_coding_styles.style(component.component)?,
            limits,
        )?);
        component_index = component_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    let mut tile_components = Vec::with_capacity(prepared.len());
    component_index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component_index <= prepared.len(),
            decreases prepared.len() - component_index,
        )
    )]
    while component_index < prepared.len() {
        tile_components.push(
            prepared
                .get(component_index)
                .ok_or_else(invalid_decode)?
                .as_tile_component(),
        );
        component_index = component_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    let component_transform = if !plan.coding_style.multiple_component_transform {
        ComponentTransform::None
    } else {
        match plan.coding_style.transform {
            WaveletTransform::Reversible5x3 => ComponentTransform::Reversible,
            WaveletTransform::Irreversible9x7 => ComponentTransform::Irreversible,
            WaveletTransform::Arbitrary(_) => {
                return Err(DecodeError::malformed(
                    0,
                    MalformedKind::MissingRequiredStructure,
                ));
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
    let mapped = apply_jp2_output(decoded, inspected.file().header(), options.limits())?;
    convert_selected_color(
        &mapped,
        &inspected.file().header().color,
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
    let mapped = apply_jp2_output(decoded, inspected.file().header(), options.limits())?;
    convert_selected_color(
        &mapped,
        &inspected.file().header().color,
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
    let header =
        header.ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))?;
    crate::color::convert_jp2_color(&decoded, &header.color, options.limits())
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
    let header =
        header.ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))?;
    convert_selected_color(&decoded, &header.color, target, options.limits())
}

fn color_selection(
    input: &[u8],
    requested: &DecodeSelection,
    limits: &ResourceLimits,
) -> Result<(DecodeSelection, [u32; 4]), DecodeError> {
    let layout = codestream::parse_layout(input, limits)?;
    let size = &layout.info.image_size;
    let validated = validate_selection(size, requested)?;
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
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases size.components - component,)
    )]
    while component < size.components {
        let info = size.component(component)?;
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
    let expanded = DecodeSelection::full_image()
        .with_region(crate::ImageRegion::new(
            x0,
            y0,
            validated.region[2]
                .checked_sub(x0)
                .ok_or_else(arithmetic_overflow)?,
            validated.region[3]
                .checked_sub(y0)
                .ok_or_else(arithmetic_overflow)?,
        ))
        .with_discard_levels(discard_levels);
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
    let info = codestream::parse(selected.data(), options.limits())?;
    if let Some(header) = associated_header {
        jp2::validate_selected_header(header, &info, selected.box_header().offset())?;
    }
    let decoded = decode_codestream(selected.data(), options)?;
    let image = match associated_header {
        Some(header) => apply_jp2_output(decoded, header, options.limits())?,
        None => decoded,
    };
    Ok((image, associated_header))
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

fn apply_jp2_output(
    decoded: TypedImage,
    header: &jp2::Jp2Header,
    limits: &ResourceLimits,
) -> Result<TypedImage, DecodeError> {
    let mut components = if header.component_mappings.is_empty() {
        let mut copied = Vec::with_capacity(decoded.components().len());
        let component_count = decoded.components().len();
        let mut component_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases component_count - component_index,)
        )]
        while component_index < component_count {
            copied.push(copy_typed_component(
                decoded
                    .components()
                    .get(component_index)
                    .ok_or_else(invalid_jp2_output)?,
            )?);
            component_index = component_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        copied
    } else {
        let mut mapped = Vec::with_capacity(header.component_mappings.len());
        let mut mapping_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant mapping_index <= header.component_mappings.len(),
                decreases header.component_mappings.len() - mapping_index,
            )
        )]
        while mapping_index < header.component_mappings.len() {
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
            mapped.push(component);
            mapping_index = mapping_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        mapped
    };

    let mut definition_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant definition_index <= header.channel_definition_records.len(),
            decreases header.channel_definition_records.len() - definition_index,
        )
    )]
    while definition_index < header.channel_definition_records.len() {
        let record = header
            .channel_definition_records
            .get(definition_index)
            .ok_or_else(invalid_jp2_output)?;
        let component = components
            .get_mut(usize::from(record.channel))
            .ok_or_else(invalid_jp2_output)?;
        component.add_channel_definition(record.definition);
        definition_index = definition_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    TypedImage::from_components_with_origin(
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

fn tile_list_contains(tiles: &[u16], tile: u16) -> Result<bool, DecodeError> {
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= tiles.len(),
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

fn validate_selection(
    size: &ImageSize,
    selection: &DecodeSelection,
) -> Result<ValidatedSelection, DecodeError> {
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
        return Err(DecodeError::malformed(
            0,
            MalformedKind::InvalidImageGeometry,
        ));
    }
    if tile_count > u64::from(u16::MAX) + 1 {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::InvalidImageGeometry,
        ));
    }

    let explicit_tiles = match selection.tile_selection() {
        TileSelection::All => Vec::new(),
        TileSelection::Index(index) => vec![*index],
        TileSelection::Set(indices) => {
            if indices.is_empty() {
                return Err(DecodeError::InvalidSelection {
                    reason: SelectionError::EmptyTileSet,
                });
            }
            indices.clone()
        }
    };
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= explicit_tiles.len(),
            decreases explicit_tiles.len() - index,
        )
    )]
    while index < explicit_tiles.len() {
        let tile = *explicit_tiles.get(index).ok_or_else(arithmetic_overflow)?;
        if u64::from(tile) >= tile_count {
            return Err(DecodeError::InvalidSelection {
                reason: SelectionError::TileOutOfRange,
            });
        }
        let mut earlier = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant earlier <= index,
                decreases index - earlier,
            )
        )]
        while earlier < index {
            if explicit_tiles.get(earlier) == Some(&tile) {
                return Err(DecodeError::InvalidSelection {
                    reason: SelectionError::DuplicateTile,
                });
            }
            earlier = earlier.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    if explicit_tiles.len() > 1 {
        let first = u32::from(*explicit_tiles.first().ok_or_else(arithmetic_overflow)?);
        let mut first_row = first / tiles_across;
        let mut first_column = first % tiles_across;
        let mut last_row = first_row;
        let mut last_column = first_column;
        let mut bound_index = 1_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases explicit_tiles.len() - bound_index,)
        )]
        while bound_index < explicit_tiles.len() {
            let current = u32::from(
                *explicit_tiles
                    .get(bound_index)
                    .ok_or_else(arithmetic_overflow)?,
            );
            let row = floor_div_u32(current, tiles_across)?;
            let column = current
                .checked_rem(tiles_across)
                .ok_or_else(arithmetic_overflow)?;
            first_row = first_row.min(row);
            first_column = first_column.min(column);
            last_row = last_row.max(row);
            last_column = last_column.max(column);
            bound_index = bound_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        let expected = last_row
            .checked_sub(first_row)
            .and_then(|value| value.checked_add(1))
            .and_then(|rows| {
                last_column
                    .checked_sub(first_column)?
                    .checked_add(1)?
                    .checked_mul(rows)
            })
            .ok_or(DecodeError::InvalidSelection {
                reason: SelectionError::NonRectangularTileSet,
            })?;
        if usize::try_from(expected).map_err(|_| arithmetic_overflow())? != explicit_tiles.len() {
            return Err(DecodeError::InvalidSelection {
                reason: SelectionError::NonRectangularTileSet,
            });
        }
        let mut row = first_row;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases last_row as int + 1 - row as int,)
        )]
        while row <= last_row {
            let mut column = first_column;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(decreases last_column as int + 1 - column as int,)
            )]
            while column <= last_column {
                let expected_index = row
                    .checked_mul(tiles_across)
                    .and_then(|value| value.checked_add(column))
                    .ok_or_else(arithmetic_overflow)?;
                if !tile_list_contains(
                    &explicit_tiles,
                    u16::try_from(expected_index).map_err(|_| arithmetic_overflow())?,
                )? {
                    return Err(DecodeError::InvalidSelection {
                        reason: SelectionError::NonRectangularTileSet,
                    });
                }
                column = column.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
    }

    let region = match selection.region() {
        Some(region) => {
            if region.width == 0 || region.height == 0 {
                return Err(DecodeError::InvalidSelection {
                    reason: SelectionError::EmptyRegion,
                });
            }
            let x1 = region
                .x
                .checked_add(region.width)
                .ok_or(DecodeError::InvalidSelection {
                    reason: SelectionError::RegionOutOfBounds,
                })?;
            let y1 = region
                .y
                .checked_add(region.height)
                .ok_or(DecodeError::InvalidSelection {
                    reason: SelectionError::RegionOutOfBounds,
                })?;
            if region.x < size.x_origin
                || region.y < size.y_origin
                || x1 > image_x1
                || y1 > image_y1
            {
                return Err(DecodeError::InvalidSelection {
                    reason: SelectionError::RegionOutOfBounds,
                });
            }
            [region.x, region.y, x1, y1]
        }
        None if explicit_tiles.is_empty() => [size.x_origin, size.y_origin, image_x1, image_y1],
        None => {
            let mut x0 = image_x1;
            let mut y0 = image_y1;
            let mut x1 = size.x_origin;
            let mut y1 = size.y_origin;
            let mut tile_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant tile_index <= explicit_tiles.len(),
                    decreases explicit_tiles.len() - tile_index,
                )
            )]
            while tile_index < explicit_tiles.len() {
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
            [x0, y0, x1, y1]
        }
    };

    let mut tiles = Vec::new();
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
        let bounds = size.tile_bounds(tile_index)?;
        let intersects = bounds.x0 < region[2]
            && bounds.x1 > region[0]
            && bounds.y0 < region[3]
            && bounds.y1 > region[1];
        if intersects {
            if !explicit_tiles.is_empty() && !tile_list_contains(&explicit_tiles, tile_index)? {
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
    Ok(ValidatedSelection {
        tiles,
        region,
        discard_levels: selection.discard_levels(),
    })
}

fn decode_layout_selected(
    layout: &CodestreamLayout<'_>,
    requested: &DecodeSelection,
    limits: &ResourceLimits,
) -> Result<DecodedImage, DecodeError> {
    validate_part2_execution(layout)?;
    let selection = validate_selection(&layout.info.image_size, requested)?;
    let mut prepared = Vec::new();
    let mut total_packets = 0_u64;
    let mut total_precincts = 0_u64;
    let mut total_code_blocks = 0_u64;
    let mut total_mct_work = 0_u64;
    let mut selected_tile = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant selected_tile <= selection.tiles.len(),
            decreases selection.tiles.len() - selected_tile,
        )
    )]
    while selected_tile < selection.tiles.len() {
        let tile_index = *selection
            .tiles
            .get(selected_tile)
            .ok_or_else(invalid_decode)?;
        let mut tile_parts: Vec<TilePartPayload<'_>> = Vec::new();
        let mut part_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant part_index <= layout.tile_parts.len(),
                decreases layout.tile_parts.len() - part_index,
            )
        )]
        while part_index < layout.tile_parts.len() {
            let tile_part = layout
                .tile_parts
                .get(part_index)
                .ok_or_else(invalid_decode)?
                .clone();
            if tile_part.tile_index == tile_index {
                tile_parts.push(tile_part);
            }
            part_index = part_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        let first = tile_parts
            .first()
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
        if tile_parts.iter().any(|part| {
            !PreparedTileComponent::optional_mct_equal(
                part.multiple_component_transform.as_ref(),
                first.multiple_component_transform.as_ref(),
            )
        }) {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
        }
        let prepared_start = prepared.len();
        let coding_style = first.coding_style;
        let component_coding_styles = first.component_coding_styles;
        let component_quantizations = &first.component_quantizations;
        let transform = (
            coding_style.multiple_component_transform,
            match coding_style.transform {
                WaveletTransform::Irreversible9x7 => true,
                WaveletTransform::Reversible5x3 => false,
                WaveletTransform::Arbitrary(index) => !first
                    .part2_extensions
                    .arbitrary_kernel(index)
                    .ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::MissingRequiredStructure)
                    })?
                    .reversible(),
            },
        );
        let mut resolution_counts =
            Vec::with_capacity(usize::from(layout.info.image_size.components));
        let mut component = 0_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= layout.info.image_size.components,
                decreases layout.info.image_size.components - component,
            )
        )]
        while component < layout.info.image_size.components {
            let style = component_coding_styles.style(component)?;
            if selection.discard_levels > style.maximum_isotropic_reduction() {
                return Err(DecodeError::InvalidSelection {
                    reason: SelectionError::ReductionTooLarge,
                });
            }
            if coding_style.multiple_component_transform
                && component < 3
                && !wavelet_transforms_equal(style.transform, coding_style.transform)
            {
                return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
            }
            resolution_counts.push(
                style
                    .decomposition_levels
                    .checked_add(1)
                    .and_then(|value| value.checked_sub(selection.discard_levels))
                    .ok_or(DecodeError::InvalidSelection {
                        reason: SelectionError::ReductionTooLarge,
                    })?,
            );
            component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        let mut plan = plan_progression_tile_at_selected(
            &layout.info.image_size,
            tile_index,
            coding_style,
            &component_coding_styles,
            component_quantizations,
            &first.roi_shifts,
            selection.discard_levels,
            layout.info.profile.part15_allows_multiple_ht_sets(),
            layout.info.profile.part15_magnitude_bound(),
            layout.info.profile.part15_requires_all_ht_blocks(),
            limits,
        )?;
        let selected_packets = plan.grid.selected_packet_count(&resolution_counts)?;
        total_packets = total_packets
            .checked_add(selected_packets)
            .ok_or_else(arithmetic_overflow)?;
        if total_packets > limits.maximum_packets() {
            return Err(DecodeError::limit(
                Resource::Packets,
                limits.maximum_packets(),
                total_packets,
            ));
        }
        let selected_precincts = selected_packets
            .checked_div(u64::from(coding_style.layers))
            .ok_or_else(arithmetic_overflow)?;
        total_precincts = total_precincts
            .checked_add(selected_precincts)
            .ok_or_else(arithmetic_overflow)?;
        if total_precincts > limits.maximum_precincts() {
            return Err(DecodeError::limit(
                Resource::Precincts,
                limits.maximum_precincts(),
                total_precincts,
            ));
        }
        let mut component_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_index <= plan.components.len(),
                decreases plan.components.len() - component_index,
            )
        )]
        while component_index < plan.components.len() {
            let planned = plan
                .components
                .get(component_index)
                .ok_or_else(invalid_decode)?;
            let mut placement_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant placement_index <= planned.placements.len(),
                    decreases planned.placements.len() - placement_index,
                )
            )]
            while placement_index < planned.placements.len() {
                let placement = planned
                    .placements
                    .get(placement_index)
                    .ok_or_else(invalid_decode)?;
                if plan
                    .components
                    .get(component_index)
                    .and_then(|value| value.subbands.get(placement.subband_index))
                    .ok_or_else(invalid_decode)?
                    .resolution
                    <= planned.output_resolution
                {
                    total_code_blocks = total_code_blocks
                        .checked_add(1)
                        .ok_or_else(arithmetic_overflow)?;
                }
                placement_index = placement_index
                    .checked_add(1)
                    .ok_or_else(arithmetic_overflow)?;
            }
            component_index = component_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        if total_code_blocks > limits.maximum_code_blocks() {
            return Err(DecodeError::limit(
                Resource::CodeBlocks,
                limits.maximum_code_blocks(),
                total_code_blocks,
            ));
        }
        let (grid, precincts) = plan.packet_execution();
        crate::packet::decode_progression_tile_packets_selected(
            &tile_parts,
            grid,
            precincts,
            Some(&resolution_counts),
            limits,
        )?;
        component_index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_index <= plan.components.len(),
                decreases plan.components.len() - component_index,
            )
        )]
        while component_index < plan.components.len() {
            let planned = plan
                .components
                .get(component_index)
                .ok_or_else(invalid_decode)?;
            let style = component_coding_styles.style(planned.component)?;
            let kernel = match style.transform {
                WaveletTransform::Arbitrary(index) => {
                    first.part2_extensions.arbitrary_kernel(index)
                }
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
        if let Some(mct) = &first.multiple_component_transform {
            PreparedTileComponent::apply_tile_multiple_component_transform(
                prepared
                    .get_mut(prepared_start..)
                    .ok_or_else(invalid_decode)?,
                &layout.info.image_size,
                mct,
                limits,
                &mut total_mct_work,
            )?;
        }
        apply_prepared_component_transform(
            prepared
                .get_mut(prepared_start..)
                .ok_or_else(invalid_decode)?,
            transform,
        )?;
        selected_tile = selected_tile
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }

    let mut tile_components = Vec::with_capacity(prepared.len());
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= prepared.len(),
            decreases prepared.len() - index,
        )
    )]
    while index < prepared.len() {
        tile_components.push(
            prepared
                .get(index)
                .ok_or_else(invalid_decode)?
                .as_tile_component(),
        );
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let image = assemble_selected_image(
        &layout.info.image_size,
        &tile_components,
        selection.region,
        selection.discard_levels,
        limits,
    )?;
    Ok(image)
}

fn apply_prepared_component_transform(
    prepared: &mut [PreparedTileComponent],
    transform: (bool, bool),
) -> Result<(), DecodeError> {
    if !transform.0 {
        return Ok(());
    }
    if prepared.len() < 3 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
    }
    let (red, rest) = prepared.split_first_mut().ok_or_else(invalid_decode)?;
    let (green, rest) = rest.split_first_mut().ok_or_else(invalid_decode)?;
    let blue = rest.first_mut().ok_or_else(invalid_decode)?;
    if (red.x, red.y, red.width, red.height) != (green.x, green.y, green.width, green.height)
        || (red.x, red.y, red.width, red.height) != (blue.x, blue.y, blue.width, blue.height)
        || red.samples.len() != green.samples.len()
        || red.samples.len() != blue.samples.len()
        || red.fraction_bits != green.fraction_bits
        || red.fraction_bits != blue.fraction_bits
        || (!transform.1 && red.fraction_bits != 0)
        || (transform.1 && red.fraction_bits != 16)
    {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
    }
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= red.samples.len(),
            decreases red.samples.len() - index,
        )
    )]
    while index < red.samples.len() {
        let y = *red.samples.get(index).ok_or_else(invalid_decode)?;
        let cb = *green.samples.get(index).ok_or_else(invalid_decode)?;
        let cr = *blue.samples.get(index).ok_or_else(invalid_decode)?;
        let (r, g, b) = if transform.1 {
            let fixed_mul = |value: i64, factor: i64| -> Result<i64, DecodeError> {
                Ok(value
                    .checked_mul(factor)
                    .and_then(|product| product.checked_add(1 << 15))
                    .ok_or_else(arithmetic_overflow)?
                    >> 16)
            };
            (
                y.checked_add(fixed_mul(cr, 91_881)?)
                    .ok_or_else(arithmetic_overflow)?,
                y.checked_sub(fixed_mul(cb, 22_553)?)
                    .and_then(|value| value.checked_sub(fixed_mul(cr, 46_797).ok()?))
                    .ok_or_else(arithmetic_overflow)?,
                y.checked_add(fixed_mul(cb, 116_130)?)
                    .ok_or_else(arithmetic_overflow)?,
            )
        } else {
            let g = y
                .checked_sub(cb.checked_add(cr).ok_or_else(arithmetic_overflow)? >> 2)
                .ok_or_else(arithmetic_overflow)?;
            (
                cr.checked_add(g).ok_or_else(arithmetic_overflow)?,
                g,
                cb.checked_add(g).ok_or_else(arithmetic_overflow)?,
            )
        };
        *red.samples.get_mut(index).ok_or_else(invalid_decode)? = r;
        *green.samples.get_mut(index).ok_or_else(invalid_decode)? = g;
        *blue.samples.get_mut(index).ok_or_else(invalid_decode)? = b;
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

fn validate_part2_execution(layout: &CodestreamLayout<'_>) -> Result<(), DecodeError> {
    layout.info.part2_extensions.validate_executable()?;
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= layout.info.image_size.components,
            decreases layout.info.image_size.components - component,
        )
    )]
    while component < layout.info.image_size.components {
        validate_part2_coding_style(
            layout.info.component_coding_styles.style(component)?,
            &layout.info.part2_extensions,
        )?;
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= layout.tile_parts.len(),
            decreases layout.tile_parts.len() - index,
        )
    )]
    while index < layout.tile_parts.len() {
        let tile_part = layout
            .tile_parts
            .get(index)
            .ok_or_else(arithmetic_overflow)?;
        tile_part.part2_extensions.validate_executable()?;
        component = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= layout.info.image_size.components,
                decreases layout.info.image_size.components - component,
            )
        )]
        while component < layout.info.image_size.components {
            validate_part2_coding_style(
                tile_part.component_coding_styles.style(component)?,
                &tile_part.part2_extensions,
            )?;
            component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        if !tile_part.part2_extensions.component_bit_depths().is_empty()
            && !tile_part
                .part2_extensions
                .component_bit_depths_equal(&layout.info.part2_extensions)
        {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidMarkerOrder));
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

fn validate_part2_coding_style(
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
        return Err(DecodeError::malformed(
            0,
            MalformedKind::MissingRequiredStructure,
        ));
    }
    if let WaveletTransform::Arbitrary(index) = style.transform {
        extensions
            .arbitrary_kernel(index)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::MissingRequiredStructure))?;
    }
    Ok(())
}

fn decode_layout(
    layout: &CodestreamLayout<'_>,
    limits: &ResourceLimits,
) -> Result<DecodedImage, DecodeError> {
    validate_part2_execution(layout)?;
    let tile_count = layout.info.image_size.tile_count()?;
    if tile_count > u64::from(u16::MAX) + 1 {
        return Err(DecodeError::malformed(
            0,
            MalformedKind::InvalidImageGeometry,
        ));
    }
    let mut prepared = Vec::new();
    let mut total_packets = 0_u64;
    let mut total_precincts = 0_u64;
    let mut total_code_blocks = 0_u64;
    let mut total_mct_work = 0_u64;
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
        let mut tile_parts: Vec<TilePartPayload<'_>> = Vec::new();
        let mut part_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant part_index <= layout.tile_parts.len(),
                decreases layout.tile_parts.len() - part_index,
            )
        )]
        while part_index < layout.tile_parts.len() {
            let tile_part = layout
                .tile_parts
                .get(part_index)
                .ok_or_else(invalid_decode)?
                .clone();
            if tile_part.tile_index == tile_index {
                tile_parts.push(tile_part);
            }
            part_index = part_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        let first = tile_parts
            .first()
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?;
        if tile_parts.iter().any(|part| {
            !PreparedTileComponent::optional_mct_equal(
                part.multiple_component_transform.as_ref(),
                first.multiple_component_transform.as_ref(),
            )
        }) {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
        }
        let prepared_start = prepared.len();
        let coding_style = first.coding_style;
        let component_coding_styles = first.component_coding_styles;
        let component_quantizations = &first.component_quantizations;
        let transform = (
            coding_style.multiple_component_transform,
            match coding_style.transform {
                WaveletTransform::Irreversible9x7 => true,
                WaveletTransform::Reversible5x3 => false,
                WaveletTransform::Arbitrary(index) => !first
                    .part2_extensions
                    .arbitrary_kernel(index)
                    .ok_or_else(|| {
                        DecodeError::malformed(0, MalformedKind::MissingRequiredStructure)
                    })?
                    .reversible(),
            },
        );
        let roi_shifts = tile_parts
            .first()
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTilePart))?
            .roi_shifts;
        if coding_style.multiple_component_transform {
            let transform_component_count = layout.info.image_size.components.min(3);
            let mut component = 0_u16;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant component <= transform_component_count,
                    decreases transform_component_count - component,
                )
            )]
            while component < transform_component_count {
                let style = component_coding_styles.style(component)?;
                if !wavelet_transforms_equal(style.transform, coding_style.transform) {
                    return Err(DecodeError::malformed(0, MalformedKind::InvalidCodingStyle));
                }
                component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
        }
        let mut plan = plan_progression_tile_at_selected(
            &layout.info.image_size,
            tile_index,
            coding_style,
            &component_coding_styles,
            component_quantizations,
            &roi_shifts,
            0,
            layout.info.profile.part15_allows_multiple_ht_sets(),
            layout.info.profile.part15_magnitude_bound(),
            layout.info.profile.part15_requires_all_ht_blocks(),
            limits,
        )?;
        total_packets = total_packets
            .checked_add(plan.grid.packet_count())
            .ok_or_else(arithmetic_overflow)?;
        if total_packets > limits.maximum_packets() {
            return Err(DecodeError::limit(
                Resource::Packets,
                limits.maximum_packets(),
                total_packets,
            ));
        }
        total_precincts = total_precincts
            .checked_add(plan.grid.total_precinct_count()?)
            .ok_or_else(arithmetic_overflow)?;
        if total_precincts > limits.maximum_precincts() {
            return Err(DecodeError::limit(
                Resource::Precincts,
                limits.maximum_precincts(),
                total_precincts,
            ));
        }
        let plan_component_count = plan.components.len();
        let mut component_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_index <= plan_component_count,
                decreases plan_component_count - component_index,
            )
        )]
        while component_index < plan_component_count {
            total_code_blocks = total_code_blocks
                .checked_add(
                    plan.components
                        .get(component_index)
                        .ok_or_else(invalid_decode)?
                        .placements
                        .len() as u64,
                )
                .ok_or_else(arithmetic_overflow)?;
            if total_code_blocks > limits.maximum_code_blocks() {
                return Err(DecodeError::limit(
                    Resource::CodeBlocks,
                    limits.maximum_code_blocks(),
                    total_code_blocks,
                ));
            }
            component_index = component_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        let (grid, precincts) = plan.packet_execution();
        crate::packet::decode_progression_tile_packets(&tile_parts, grid, precincts, limits)?;
        component_index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_index <= plan_component_count,
                decreases plan_component_count - component_index,
            )
        )]
        while component_index < plan_component_count {
            let planned = plan
                .components
                .get(component_index)
                .ok_or_else(invalid_decode)?;
            let style = component_coding_styles.style(planned.component)?;
            let kernel = match style.transform {
                WaveletTransform::Arbitrary(index) => {
                    first.part2_extensions.arbitrary_kernel(index)
                }
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
        if let Some(transform) = &first.multiple_component_transform {
            PreparedTileComponent::apply_tile_multiple_component_transform(
                prepared
                    .get_mut(prepared_start..)
                    .ok_or_else(invalid_decode)?,
                &layout.info.image_size,
                transform,
                limits,
                &mut total_mct_work,
            )?;
        }
        apply_prepared_component_transform(
            prepared
                .get_mut(prepared_start..)
                .ok_or_else(invalid_decode)?,
            transform,
        )?;
        tile = tile.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }

    let mut tile_components = Vec::with_capacity(prepared.len());
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= prepared.len(),
            decreases prepared.len() - index,
        )
    )]
    while index < prepared.len() {
        tile_components.push(
            prepared
                .get(index)
                .ok_or_else(invalid_decode)?
                .as_tile_component(),
        );
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let image = decode_prepared_image(
        &layout.info.image_size,
        &tile_components,
        ComponentTransform::None,
        limits,
    )?;
    Ok(image)
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

fn component_coordinate_bounds(
    size: &ImageSize,
    component: u16,
    tile_index: Option<u16>,
) -> Result<[u32; 4], DecodeError> {
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
    Ok([
        ceil_div_u32(size.x_origin, u32::from(info.x_subsampling))?,
        ceil_div_u32(size.y_origin, u32::from(info.y_subsampling))?,
        ceil_div_u32(image_x1, u32::from(info.x_subsampling))?,
        ceil_div_u32(image_y1, u32::from(info.y_subsampling))?,
    ])
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

fn ceil_div_u32(value: u32, divisor: u32) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(invalid_decode());
    }

    let remainder = value.checked_rem(divisor).ok_or_else(arithmetic_overflow)?;
    (value / divisor)
        .checked_add(u32::from(remainder != 0))
        .ok_or_else(arithmetic_overflow)
}

fn registered_ceil_div_u32(
    value: u32,
    divisor: u32,
    registration: u16,
) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(invalid_decode());
    }
    let scale = 65_536_u64;
    let numerator = u64::from(value)
        .checked_mul(scale)
        .ok_or_else(arithmetic_overflow)?;
    if numerator <= u64::from(registration) {
        return Ok(0);
    }
    let adjusted = numerator
        .checked_sub(u64::from(registration))
        .ok_or_else(arithmetic_overflow)?;
    let denominator = u64::from(divisor)
        .checked_mul(scale)
        .ok_or_else(arithmetic_overflow)?;
    let quotient = adjusted / denominator;
    let rounded = quotient
        .checked_add(u64::from(!adjusted.is_multiple_of(denominator)))
        .ok_or_else(arithmetic_overflow)?;
    u32::try_from(rounded).map_err(|_| arithmetic_overflow())
}

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
