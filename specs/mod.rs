use vstd::arithmetic::power2::pow2;
use vstd::prelude::*;

pub use crate::proofs::internal_specs::{
    part2_composition_layer_prefix_matches, part2_composition_layer_prefix_safe,
};

verus! {

pub type Part2RawKernelCoefficient = (Seq<u8>, u8);
pub type Part2RawKernelStep = (i8, u8, Option<Part2RawKernelCoefficient>, Seq<Part2RawKernelCoefficient>);
pub type Part2RawKernel = (bool, u8, bool, bool, bool, Option<Part2RawKernelCoefficient>, Seq<Part2RawKernelStep>);

pub type Part1DecodedComponentModel = (u32, u32, crate::codestream::ComponentInfo, u8, Seq<i64>);
pub type Part1DecodedImageModel = (u32, u32, u32, u32, Seq<Part1DecodedComponentModel>);
pub type Part1DecodedImageState = (Result<(), crate::DecodeError>, Part1DecodedImageModel);
pub type Part1WorkingSubbandModel = (u32, u32, u8, Seq<i64>, Seq<bool>);
pub type Part1WorkingSubbandState = (Result<(), crate::DecodeError>, Part1WorkingSubbandModel);
pub type Part1PrepareWorking = (Seq<Part1WorkingSubbandModel>, u64);
pub type Part1PrepareWorkingState = (Result<(), crate::DecodeError>, Part1PrepareWorking);
pub type Part1PrepareStyle = (crate::codestream::WaveletTransform, u8, Seq<crate::codestream::DecompositionDirection>);
pub type Part1PrepareFold = (u32, u32, Seq<i64>, usize);
pub open spec fn part1_prepared_as_tile(prepared: Part1AssemblyTile) -> Part1AssemblyTile { prepared }
pub struct Part1PreparePlan {
    pub component: u16,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub resolution: u8,
    pub subbands: Seq<crate::decode::SubbandGeometry>,
    pub placements: Seq<crate::decode::ProgressionCodeBlockPlacement>,
}

pub open spec fn part1_prepare_fraction(reversible: bool) -> u8 {
    if reversible { 0u8 } else { 16u8 }
}

pub open spec fn part1_prepare_append_band(
    before: Part1PrepareWorking, bands: Seq<crate::decode::SubbandGeometry>, index: usize, fraction: u8, maximum: u64,
) -> Part1PrepareWorkingState {
    if index >= bands.len() { (Err(part1_block_placement_error(false)), before) } else {
        let geometry = bands[index as int];
        match part1_transform_area(geometry.width, geometry.height) {
            Err(error) => (Err(error), before),
            Ok(length) => {
                let bytes = length as nat * 9;
                if bytes > u64::MAX || before.1 as nat + bytes > u64::MAX { (Err(part1_block_placement_error(true)), before) }
                else {
                    let total = (before.1 as nat + bytes) as u64;
                    if total > maximum {
                        (Err(crate::DecodeError::LimitExceeded { resource: crate::Resource::DecodedBytes, limit: maximum, actual: total }), (before.0, total))
                    } else {
                        (Ok(()), (before.0.push((geometry.width, geometry.height, fraction,
                            Seq::new(length as nat, |i: int| 0i64), Seq::new(length as nat, |i: int| false))), total))
                    }
                }
            },
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_prepare_init_prefix(
    bands: Seq<crate::decode::SubbandGeometry>, fraction: u8, maximum: u64, count: nat,
) -> Result<Part1PrepareWorking, crate::DecodeError>
    decreases count,
{
    if count == 0 { Ok((Seq::empty(), 0)) } else {
        match part1_prepare_init_prefix(bands, fraction, maximum, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(previous) => if count > bands.len() { Err(part1_block_placement_error(false)) } else {
                let geometry = bands[count - 1];
                match part1_transform_area(geometry.width, geometry.height) {
                    Err(error) => Err(error),
                    Ok(length) => {
                        let bytes = length as nat * 9;
                        if bytes > u64::MAX || previous.1 as nat + bytes > u64::MAX {
                            Err(part1_block_placement_error(true))
                        } else {
                            let total = (previous.1 as nat + bytes) as u64;
                            if total > maximum {
                                Err(crate::DecodeError::LimitExceeded { resource: crate::Resource::DecodedBytes, limit: maximum, actual: total })
                            } else {
                                Ok((previous.0.push((geometry.width, geometry.height, fraction,
                                    Seq::new(length as nat, |i: int| 0i64), Seq::new(length as nat, |i: int| false))), total))
                            }
                        }
                    },
                }
            },
        }
    }
}

pub open spec fn part1_prepare_signed_cell(plane: Tier1RawPlane, x: u16, y: u16)
    -> Result<Part1QuantizedCoefficient, crate::DecodeError>
{
    match tier1_raw_plane_coefficient(plane, x, y) {
        Err(error) => Err(error),
        Ok(coefficient) => match tier1_raw_signed_coefficient(coefficient) {
            Err(error) => Err(error), Ok(signed) => Ok((signed.0, signed.2)),
        },
    }
}

#[verifier::opaque]
pub open spec fn part1_prepare_coefficient_row(
    plane: Tier1RawPlane, y: u16, initial: Seq<Part1QuantizedCoefficient>, count: nat,
) -> Result<Seq<Part1QuantizedCoefficient>, crate::DecodeError>
    decreases count,
{
    if count == 0 { Ok(initial) } else {
        match part1_prepare_coefficient_row(plane, y, initial, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(values) => match part1_prepare_signed_cell(plane, (count - 1) as u16, y) {
                Err(error) => Err(error), Ok(value) => Ok(values.push(value)),
            },
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_prepare_coefficient_rows(plane: Tier1RawPlane, count: nat)
    -> Result<Seq<Part1QuantizedCoefficient>, crate::DecodeError>
    decreases count,
{
    if count == 0 { Ok(Seq::empty()) } else {
        match part1_prepare_coefficient_rows(plane, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(values) => part1_prepare_coefficient_row(plane, (count - 1) as u16, values, plane.width as nat),
        }
    }
}

pub open spec fn part1_prepare_collect_coefficients(plane: Tier1RawPlane)
    -> Result<Seq<Part1QuantizedCoefficient>, crate::DecodeError>
{
    if plane.width as nat * plane.height as nat > usize::MAX { Err(part1_block_placement_error(true)) }
    else { part1_prepare_coefficient_rows(plane, plane.height as nat) }
}

pub open spec fn part1_prepare_place_plane(
    working: Part1PrepareWorking, geometry: crate::decode::SubbandGeometry,
    placement: crate::decode::ProgressionCodeBlockPlacement, plane: Tier1RawPlane, reversible: bool, maximum: u64,
) -> Part1PrepareWorkingState {
    match part1_image_checked_u32(placement.x as int + plane.width as int) {
        Err(error) => (Err(error), working),
        Ok(x1) => if x1 > geometry.width { (Err(part1_block_placement_error(false)), working) } else {
            match part1_image_checked_u32(placement.y as int + plane.height as int) {
                Err(error) => (Err(error), working),
                Ok(y1) => if y1 > geometry.height { (Err(part1_block_placement_error(false)), working) } else {
                    match part1_prepare_collect_coefficients(plane) {
                        Err(error) => (Err(error), working),
                        Ok(coefficients) => match part1_inverse_quantization(
                            coefficients, geometry.quantization_step.exponent, geometry.quantization_step.mantissa,
                            geometry.dynamic_range, reversible, geometry.roi_shift, maximum,
                        ) {
                            Err(error) => (Err(error), working),
                            Ok(quantized) => if quantized.2 != part1_prepare_fraction(reversible) {
                                (Err(part1_block_placement_error(false)), working)
                            } else if placement.subband_index >= working.0.len() {
                                (Err(part1_block_placement_error(false)), working)
                            } else {
                                let written = part1_quantized_block_rows(
                                    working.0[placement.subband_index as int], quantized.3, plane.width as u32,
                                    placement.x, placement.y, plane.height as nat,
                                );
                                (written.0, (working.0.update(placement.subband_index as int, written.1), working.1))
                            },
                        },
                    }
                },
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn part1_prepare_placement(
    working: Part1PrepareWorking, bands: Seq<crate::decode::SubbandGeometry>,
    precincts: Seq<Seq<Tier1RawPlane>>, placement: crate::decode::ProgressionCodeBlockPlacement,
    reversible: bool, maximum: u64,
) -> Part1PrepareWorkingState {
    if placement.subband_index >= working.0.len() { (Ok(()), working) }
    else if placement.subband_index >= bands.len() || placement.precinct_index >= precincts.len() {
        (Err(part1_block_placement_error(false)), working)
    } else if placement.decoder_index >= precincts[placement.precinct_index as int].len() {
        (Err(part1_packet_header_error()), working)
    } else {
        part1_prepare_place_plane(working, bands[placement.subband_index as int], placement,
            precincts[placement.precinct_index as int][placement.decoder_index as int], reversible, maximum)
    }
}

#[verifier::opaque]
pub open spec fn part1_prepare_placements(
    initial: Part1PrepareWorking, plan: Part1PreparePlan, precincts: Seq<Seq<Tier1RawPlane>>,
    reversible: bool, maximum: u64, count: nat,
) -> Part1PrepareWorkingState
    decreases count,
{
    if count == 0 { (Ok(()), initial) } else {
        let previous = part1_prepare_placements(initial, plan, precincts, reversible, maximum, (count - 1) as nat);
        match previous.0 {
            Err(_) => previous,
            Ok(()) => part1_prepare_placement(previous.1, plan.subbands, precincts, plan.placements[count - 1], reversible, maximum),
        }
    }
}

pub open spec fn part1_prepare_coverage(bands: Seq<Part1WorkingSubbandModel>) -> Result<(), crate::DecodeError> {
    if forall |b: int, i: int| 0 <= b < bands.len() && 0 <= i < bands[b].4.len() ==> bands[b].4[i] {
        Ok(())
    } else { Err(part1_block_placement_error(false)) }
}

pub open spec fn part1_prepare_band(band: Part1WorkingSubbandModel) -> Part1WaveletBand {
    (band.0, band.1, band.3)
}

#[verifier::opaque]
pub open spec fn part1_prepare_resolution(
    before: Part1PrepareFold, bands: Seq<Part1WorkingSubbandModel>, geometry: Seq<crate::decode::SubbandGeometry>,
    direction: crate::codestream::DecompositionDirection, reversible: bool, maximum: u64,
) -> Result<Part1PrepareFold, crate::DecodeError> {
    use crate::codestream::DecompositionDirection::*;
    let first = before.3 as nat;
    if first >= bands.len() { Err(part1_block_placement_error(false)) }
    else if bands[first as int].2 != part1_prepare_fraction(reversible) || first >= geometry.len() {
        Err(part1_block_placement_error(false))
    } else {
        let detail = bands[first as int];
        let shape = geometry[first as int];
        let low = (before.0, before.1, before.2);
        let transformed = match direction {
            None => Err(part1_block_placement_error(false)),
            Horizontal | Vertical => if first + 1 > usize::MAX { Err(part1_block_placement_error(true)) } else {
                if matches!(direction, Horizontal) {
                    part1_inverse_horizontal(low, part1_prepare_band(detail), shape.resolution_width, shape.low_pass_first_x, !reversible, maximum)
                } else {
                    part1_inverse_vertical(low, part1_prepare_band(detail), shape.resolution_height, shape.low_pass_first_y, !reversible, maximum)
                }
            },
            TwoDimensional => if first + 1 > usize::MAX { Err(part1_block_placement_error(true)) }
                else if first + 1 >= bands.len() { Err(part1_block_placement_error(false)) }
                else if first + 2 > usize::MAX { Err(part1_block_placement_error(true)) }
                else if first + 2 >= bands.len() { Err(part1_block_placement_error(false)) }
                else if bands[(first + 1) as int].2 != part1_prepare_fraction(reversible)
                    || bands[(first + 2) as int].2 != part1_prepare_fraction(reversible) { Err(part1_block_placement_error(false)) }
                else if first + 3 > usize::MAX { Err(part1_block_placement_error(true)) }
                else {
                    part1_inverse_level((low, part1_prepare_band(detail),
                        part1_prepare_band(bands[(first + 1) as int]), part1_prepare_band(bands[(first + 2) as int]),
                        shape.resolution_width, shape.resolution_height, shape.low_pass_first_x, shape.low_pass_first_y), !reversible, maximum)
                },
        };
        match transformed {
            Err(error) => Err(error),
            Ok(plane) => if plane.2 != part1_prepare_fraction(reversible) { Err(part1_block_placement_error(false)) }
                else { Ok((plane.0, plane.1, plane.3, (first + if matches!(direction, TwoDimensional) { 3nat } else { 1nat }) as usize)) },
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_prepare_fold(
    initial: Part1PrepareFold, bands: Seq<Part1WorkingSubbandModel>, geometry: Seq<crate::decode::SubbandGeometry>,
    style: Part1PrepareStyle, reversible: bool, maximum: u64, count: nat,
) -> Result<Part1PrepareFold, crate::DecodeError>
    decreases count,
{
    if count == 0 { Ok(initial) } else {
        match part1_prepare_fold(initial, bands, geometry, style, reversible, maximum, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(previous) => match part1_direction_for_resolution(style.1, style.2, count as u8) {
                Err(error) => Err(error),
                Ok(direction) => {
                    let transformed = if matches!(direction, crate::codestream::DecompositionDirection::None) { Ok(previous) }
                        else { part1_prepare_resolution(previous, bands, geometry, direction, reversible, maximum) };
                    match transformed {
                        Err(error) => Err(error),
                        Ok(after) => if count >= 255 { Err(part1_block_placement_error(true)) } else { Ok(after) },
                    }
                },
            },
        }
    }
}

pub open spec fn part1_prepare_copy_plane(plane: Part1ReconstructedPlane, first_detail: usize) -> Result<Part1PrepareFold, crate::DecodeError> {
    Ok((plane.0, plane.1, plane.3, first_detail))
}

pub open spec fn part1_prepare_reconstruct(
    bands: Seq<Part1WorkingSubbandModel>, plan: Part1PreparePlan, style: Part1PrepareStyle, reversible: bool, maximum: u64,
) -> Result<Part1PrepareFold, crate::DecodeError> {
    if bands.len() == 0 { Err(part1_block_placement_error(false)) } else {
        let ll = bands[0];
        part1_prepare_fold((ll.0, ll.1, ll.3, 1usize), bands, plan.subbands, style, reversible, maximum, plan.resolution as nat)
    }
}

pub open spec fn part1_prepare_finish(plan: Part1PreparePlan, fraction: u8, folded: Part1PrepareFold)
    -> Result<Part1AssemblyTile, crate::DecodeError>
{
    if folded.0 != plan.width || folded.1 != plan.height { Err(part1_block_placement_error(false)) }
    else {
        match part1_transform_area(plan.width, plan.height) {
            Err(error) => Err(error),
            Ok(length) => if folded.2.len() != length { Err(part1_block_placement_error(false)) }
                else { Ok((plan.component, plan.x, plan.y, plan.width, plan.height, fraction, folded.2)) },
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_prepare_standard_component(
    plan: Part1PreparePlan, precincts: Seq<Seq<Tier1RawPlane>>, style: Part1PrepareStyle, maximum: u64,
) -> Result<Part1AssemblyTile, crate::DecodeError> {
    if plan.subbands.len() == 0 { Err(part1_block_placement_error(false)) } else {
        match style.0 {
            crate::codestream::WaveletTransform::Arbitrary(_) => Err(part1_block_placement_error(false)),
            _ => {
                let reversible = matches!(style.0, crate::codestream::WaveletTransform::Reversible5x3);
                match part1_subband_count_through_resolution(style.1, style.2, plan.resolution) {
                    Err(error) => Err(error),
                    Ok(selected) => if selected > plan.subbands.len() { Err(part1_block_placement_error(false)) } else {
                        match part1_prepare_init_prefix(plan.subbands, part1_prepare_fraction(reversible), maximum, selected as nat) {
                            Err(error) => Err(error),
                            Ok(initial) => {
                                let placed = part1_prepare_placements(initial, plan, precincts, reversible, maximum, plan.placements.len());
                                match placed.0 {
                                    Err(error) => Err(error),
                                    Ok(()) => match part1_prepare_coverage(placed.1.0) {
                                        Err(error) => Err(error),
                                        Ok(()) => if placed.1.0.len() == 0 { Err(part1_block_placement_error(false)) } else {
                                            let ll = placed.1.0[0];
                                            match part1_prepare_fold((ll.0, ll.1, ll.3, 1usize), placed.1.0, plan.subbands,
                                                style, reversible, maximum, plan.resolution as nat) {
                                                Err(error) => Err(error),
                                                Ok(folded) => part1_prepare_finish(plan, part1_prepare_fraction(reversible), folded),
                                            }
                                        },
                                    },
                                }
                            },
                        }
                    },
                }
            },
        }
    }
}
pub type Part1AssemblySize = (u32, u32, u32, u32, u16, Seq<crate::codestream::ComponentInfo>);
pub type Part1AssemblyTile = (u16, u32, u32, u32, u32, u8, Seq<i64>);
pub type Part1AssemblyPlane = (u32, u32, Option<u8>, Seq<i64>, Seq<bool>);
pub type Part1AssemblyPlaneState = (Result<(), crate::DecodeError>, Part1AssemblyPlane);
pub type Part1AssemblyState = (Result<(), crate::DecodeError>, Part1DecodedImageModel, u64);
pub type Part1AssemblyDimensions = (crate::codestream::ComponentInfo, u32, u32, u32, u32, u32, u32);
pub type Part1AssemblySelection = Option<([u32; 4], u32)>;

pub const PART2_NORMALIZED_SAMPLE_BITS: u32 = 16;
pub const PART2_NORMALIZED_SAMPLE_MAX: u16 = u16::MAX;
pub const PART2_NORMALIZED_SAMPLE_HALF: u16 = PART2_NORMALIZED_SAMPLE_MAX / 2;

pub const COLOR_FIXED_SCALE: u32 = 65_536;
pub const COLOR_FIXED_HALF: u32 = COLOR_FIXED_SCALE / 2;

pub open spec fn color_fixed_round(value: int) -> int {
    if value < 0 { -((-value + COLOR_FIXED_HALF) / (COLOR_FIXED_SCALE as int)) }
    else { (value + COLOR_FIXED_HALF) / (COLOR_FIXED_SCALE as int) }
}

pub open spec fn color_rounded_fixed(value: i128) -> Result<i128, crate::DecodeError> {
    if (value as int) < (i128::MIN as int) + COLOR_FIXED_HALF
        || (value as int) > (i128::MAX as int) - COLOR_FIXED_HALF {
        Err(part2_composition_error(true))
    } else { Ok(color_fixed_round(value as int) as i128) }
}

pub open spec fn color_clamp_u16(value: int) -> u16 {
    if value <= 0 { 0 } else if value >= PART2_NORMALIZED_SAMPLE_MAX as int {
        PART2_NORMALIZED_SAMPLE_MAX
    } else { value as u16 }
}

pub open spec fn color_sycc_to_srgb(y: u16, cb: i32, cr: i32)
    -> Result<(u16, u16, u16), crate::DecodeError>
{
    Ok((
        color_clamp_u16((y as int) + color_fixed_round(91881 * (cr as int))),
        color_clamp_u16((y as int) - color_fixed_round(22554 * (cb as int))
            - color_fixed_round(46802 * (cr as int))),
        color_clamp_u16((y as int) + color_fixed_round(116130 * (cb as int))),
    ))
}

pub enum Part1TypedSampleSequence {
    Signed(Seq<i64>),
    Unsigned(Seq<u64>),
}

pub struct Part1TypedComponentModel {
    pub width: u32,
    pub height: u32,
    pub info: crate::codestream::ComponentInfo,
    pub samples: Part1TypedSampleSequence,
}

pub type Part1DecodedTypedImage = (
    ColorGeometry, Seq<Part1TypedComponentModel>,
    Seq<Seq<(crate::image::ChannelType, crate::image::ChannelAssociation)>>,
);

pub type Part2TypedComponentOutput = (Part1TypedComponentModel, Seq<ColorDefinition>);

pub open spec fn part2_typed_sample_count(samples: Part1TypedSampleSequence) -> nat {
    match samples {
        Part1TypedSampleSequence::Signed(values) => values.len(),
        Part1TypedSampleSequence::Unsigned(values) => values.len(),
    }
}

pub open spec fn part2_typed_component_from_parts(
    width: u32, height: u32, info: crate::codestream::ComponentInfo, samples: Part1TypedSampleSequence,
) -> Part2TypedComponentOutput {
    (Part1TypedComponentModel { width, height, info, samples }, Seq::empty())
}

pub open spec fn part2_typed_component_copy(source: Part2TypedComponentOutput)
    -> Result<Part2TypedComponentOutput, crate::DecodeError>
{
    Ok(source)
}

pub open spec fn part2_palette_modulus(precision: u8) -> Result<u64, crate::DecodeError> {
    if precision == 0 || precision > 38 { Err(part2_color_input_error()) }
    else { Ok(1u64 << precision) }
}

pub open spec fn part2_palette_source_index(samples: Part1TypedSampleSequence, index: usize)
    -> Result<u16, crate::DecodeError>
{
    let value = match samples {
        Part1TypedSampleSequence::Signed(values) =>
            if index < values.len() { Some(values[index as int] as int) } else { None },
        Part1TypedSampleSequence::Unsigned(values) =>
            if index < values.len() { Some(values[index as int] as int) } else { None },
    };
    match value {
        None => Err(part2_color_input_error()),
        Some(value) => if value < 0 || value > u16::MAX { Err(part2_color_input_error()) }
            else { Ok(value as u16) },
    }
}

pub open spec fn part2_palette_sample(
    samples: Part1TypedSampleSequence, palette: Part2PaletteModel, column: u8, index: usize,
) -> Result<i64, crate::DecodeError> {
    if column >= palette.1.len() { Err(part2_color_input_error()) }
    else {
        let descriptor = palette.1[column as int];
        if descriptor.precision == 0 || descriptor.precision > 38 { Err(part2_color_input_error()) }
        else {
            match part2_palette_source_index(samples, index) {
                Err(error) => Err(error),
                Ok(entry) => if entry >= palette.0 { Err(part2_color_input_error()) } else {
                    match part2_palette_value(palette, entry, column) {
                        Err(error) => Err(error),
                        Ok(value) =>
                            if value < part1_output_minimum(descriptor.precision, descriptor.signed)
                                || value > part1_output_maximum(descriptor.precision, descriptor.signed) {
                                Err(part2_color_input_error())
                            } else { Ok(value) },
                    }
                },
            }
        }
    }
}

#[verifier::opaque]
pub open spec fn part2_palette_samples(
    samples: Part1TypedSampleSequence, palette: Part2PaletteModel, column: u8, count: nat,
) -> Result<Part1TypedSampleSequence, crate::DecodeError>
    decreases count,
{
    if column >= palette.1.len() { Err(part2_color_input_error()) }
    else {
        let descriptor = palette.1[column as int];
        if descriptor.precision == 0 || descriptor.precision > 38 { Err(part2_color_input_error()) }
        else if count == 0 {
            Ok(if descriptor.signed { Part1TypedSampleSequence::Signed(Seq::empty()) }
                else { Part1TypedSampleSequence::Unsigned(Seq::empty()) })
        } else {
            match part2_palette_samples(samples, palette, column, (count - 1) as nat) {
                Err(error) => Err(error),
                Ok(previous) => match part2_palette_sample(samples, palette, column, (count - 1) as usize) {
                    Err(error) => Err(error),
                    Ok(value) => Ok(match previous {
                        Part1TypedSampleSequence::Signed(values) =>
                            Part1TypedSampleSequence::Signed(values.push(value)),
                        Part1TypedSampleSequence::Unsigned(values) =>
                            Part1TypedSampleSequence::Unsigned(values.push(value as u64)),
                    }),
                },
            }
        }
    }
}

pub open spec fn part2_palette_component(
    source: Part1TypedComponentModel, palette: Part2PaletteModel, column: u8,
) -> Result<Part2TypedComponentOutput, crate::DecodeError> {
    if column >= palette.1.len() { Err(part2_color_input_error()) }
    else {
        let descriptor = palette.1[column as int];
        if descriptor.precision == 0 || descriptor.precision > 38 { Err(part2_color_input_error()) }
        else {
            match part2_palette_samples(source.samples, palette, column, part2_typed_sample_count(source.samples)) {
                Err(error) => Err(error),
                Ok(samples) => Ok(part2_typed_component_from_parts(source.width, source.height,
                    crate::codestream::ComponentInfo {
                        precision: descriptor.precision, signed: descriptor.signed, ..source.info
                    }, samples)),
            }
        }
    }
}

pub open spec fn part1_packet_header_error() -> crate::DecodeError {
    crate::DecodeError::MalformedInput { offset: 0, kind: crate::MalformedKind::InvalidPacketHeader }
}

pub open spec fn part1_packet_decoder_count(decoders: Seq<crate::tier1::Tier1CodeBlockDecoder>) -> nat {
    decoders.len()
}

pub open spec fn part1_packet_precinct_decoder(
    decoders: Seq<crate::tier1::Tier1CodeBlockDecoder>, index: usize,
) -> Result<crate::tier1::Tier1CodeBlockDecoder, crate::DecodeError> {
    if index < decoders.len() { Ok(decoders[index as int]) } else { Err(part1_packet_header_error()) }
}

pub open spec fn part1_image_round_fixed(value: i64, fraction_bits: u8) -> Result<i64, crate::DecodeError> {
    if fraction_bits > 62 { Err(part1_image_error(false)) }
    else { part1_round_fixed(value, fraction_bits) }
}

pub open spec fn part1_image_byte_budget(samples: nat, bytes_per_sample: u64, maximum: u64)
    -> Result<(), crate::DecodeError>
{
    let bytes = samples * bytes_per_sample;
    if bytes > u64::MAX { Err(part1_image_error(true)) }
    else if bytes > maximum {
        Err(crate::DecodeError::LimitExceeded {
            resource: crate::Resource::DecodedBytes, limit: maximum, actual: bytes as u64,
        })
    } else { Ok(()) }
}

pub open spec fn part1_decoded_signed_value(value: i64, fraction_bits: u8, minimum: i64, maximum: i64)
    -> Result<i64, crate::DecodeError>
{
    match part1_image_round_fixed(value, fraction_bits) {
        Err(error) => Err(error),
        Ok(rounded) => {
            let raised = if rounded < minimum { minimum } else { rounded };
            Ok(if raised > maximum { maximum } else { raised })
        },
    }
}

pub open spec fn part1_decoded_unsigned_value(value: i64, fraction_bits: u8, level_shift: i64, maximum: u64)
    -> Result<u64, crate::DecodeError>
{
    match part1_image_round_fixed(value, fraction_bits) {
        Err(error) => Err(error),
        Ok(rounded) => match part1_checked_sample((rounded as int) + (level_shift as int)) {
            Err(error) => Err(error),
            Ok(shifted) => {
                let nonnegative = if shifted < 0 { 0u64 } else { shifted as u64 };
                Ok(if nonnegative < maximum { nonnegative } else { maximum })
            },
        },
    }
}

pub open spec fn part1_decoded_signed_sample(value: i64, fraction_bits: u8, precision: u8)
    -> Result<i64, crate::DecodeError>
{
    if precision == 0 || precision > 38 { Err(part1_image_error(false)) } else {
        part1_decoded_signed_value(value, fraction_bits, part1_output_minimum(precision, true), part1_output_maximum(precision, true))
    }
}

pub open spec fn part1_decoded_unsigned_sample(value: i64, fraction_bits: u8, precision: u8)
    -> Result<u64, crate::DecodeError>
{
    if precision == 0 || precision > 38 { Err(part1_image_error(false)) } else {
        part1_decoded_unsigned_value(value, fraction_bits, 1i64 << ((precision - 1) as u8), ((1u64 << precision) - 1) as u64)
    }
}

#[verifier::opaque]
pub open spec fn part1_decoded_signed_prefix(samples: Seq<i64>, fraction_bits: u8, precision: u8, count: nat)
    -> Result<Seq<i64>, crate::DecodeError>
    decreases count,
{
    if count == 0 { Ok(Seq::empty()) } else {
        match part1_decoded_signed_prefix(samples, fraction_bits, precision, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(output) => match part1_decoded_signed_sample(samples[count - 1], fraction_bits, precision) {
                Err(error) => Err(error),
                Ok(value) => Ok(output.push(value)),
            },
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_decoded_unsigned_prefix(samples: Seq<i64>, fraction_bits: u8, precision: u8, count: nat)
    -> Result<Seq<u64>, crate::DecodeError>
    decreases count,
{
    if count == 0 { Ok(Seq::empty()) } else {
        match part1_decoded_unsigned_prefix(samples, fraction_bits, precision, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(output) => match part1_decoded_unsigned_sample(samples[count - 1], fraction_bits, precision) {
                Err(error) => Err(error),
                Ok(value) => Ok(output.push(value)),
            },
        }
    }
}

pub open spec fn part1_decoded_component_typed_samples(component: Part1DecodedComponentModel, maximum: u64)
    -> Result<Part1TypedSampleSequence, crate::DecodeError>
{
    if component.2.precision == 0 || component.2.precision > 38 { Err(part1_image_error(false)) } else {
        match part1_image_byte_budget(component.4.len(), 8, maximum) {
            Err(error) => Err(error),
            Ok(()) => if component.2.signed {
                match part1_decoded_signed_prefix(component.4, component.3, component.2.precision, component.4.len()) {
                    Err(error) => Err(error), Ok(values) => Ok(Part1TypedSampleSequence::Signed(values)),
                }
            } else {
                match part1_decoded_unsigned_prefix(component.4, component.3, component.2.precision, component.4.len()) {
                    Err(error) => Err(error), Ok(values) => Ok(Part1TypedSampleSequence::Unsigned(values)),
                }
            },
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_decoded_typed_components(
    components: Seq<Part1DecodedComponentModel>, maximum: u64, count: nat,
) -> Result<(Seq<Part1TypedComponentModel>, u64), crate::DecodeError>
    decreases count,
{
    if count == 0 { Ok((Seq::empty(), 0)) } else {
        match part1_decoded_typed_components(components, maximum, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(previous) => {
                let component = components[count - 1];
                let charged = part1_accumulate_decoded_bytes(component.4.len(), previous.1, maximum);
                match charged.0 {
                    Err(error) => Err(error),
                    Ok(()) => match part1_decoded_component_typed_samples(component, maximum) {
                        Err(error) => Err(error),
                        Ok(samples) => Ok((previous.0.push(Part1TypedComponentModel {
                            width: component.0, height: component.1, info: component.2, samples,
                        }), charged.1)),
                    },
                }
            },
        }
    }
}

pub open spec fn part1_decoded_image_typed(image: Part1DecodedImageModel, maximum: u64)
    -> Result<Part1DecodedTypedImage, crate::DecodeError>
{
    match part1_decoded_typed_components(image.4, maximum, image.4.len()) {
        Err(error) => Err(error),
        Ok(converted) => Ok(((image.0, image.1, image.2, image.3), converted.0,
            Seq::new(converted.0.len(), |i: int| Seq::empty()))),
    }
}

pub open spec fn color_registered_coordinate(value: u32, divisor: u32, registration: u16, ceil: bool)
    -> Result<u32, crate::DecodeError>
{
    if divisor == 0 {
        Err(part2_color_input_error())
    } else {
        let numerator = (value as nat) * 65536;
        if numerator <= registration as nat {
            Ok(0)
        } else {
            let adjusted = (numerator - registration as nat) as nat;
            let denominator = (divisor as nat) * 65536;
            let coordinate = if ceil { part1_ceil_div(adjusted, denominator) }
                else { adjusted / denominator };
            if coordinate > u32::MAX as nat {
                Err(part2_composition_error(true))
            } else { Ok(coordinate as u32) }
        }
    }
}

pub open spec fn color_normalize_unsigned(value: u64, precision: u8)
    -> Result<u16, crate::DecodeError>
{
    if precision == 0 || precision > 38 {
        Err(part2_color_input_error())
    } else {
        let maximum = ((1u64 << precision) - 1) as nat;
        if value as nat > maximum {
            Err(part2_color_input_error())
        } else {
            Ok((((value as nat) * (PART2_NORMALIZED_SAMPLE_MAX as nat) + maximum / 2) / maximum) as u16)
        }
    }
}

pub open spec fn color_component_index(
    component: Part1TypedComponentModel, x_origin: u32, y_origin: u32, x: u32, y: u32,
) -> Result<usize, crate::DecodeError> {
    let info = component.info;
    if info.precision == 0 || info.precision > 38 || info.x_subsampling == 0
        || info.y_subsampling == 0 || component.width == 0 || component.height == 0 {
        Err(part2_color_input_error())
    } else if (x_origin as nat) + x as nat > u32::MAX as nat
        || (y_origin as nat) + y as nat > u32::MAX as nat {
        Err(part2_composition_error(true))
    } else {
        match (
            color_registered_coordinate(x_origin, info.x_subsampling as u32, info.x_registration, true),
            color_registered_coordinate(y_origin, info.y_subsampling as u32, info.y_registration, true),
        ) {
            (Err(error), _) => Err(error),
            (_, Err(error)) => Err(error),
            (Ok(x0), Ok(y0)) => {
                if (x0 as nat) + component.width as nat > u32::MAX as nat
                    || (y0 as nat) + component.height as nat > u32::MAX as nat {
                    Err(part2_composition_error(true))
                } else {
                    match (
                        color_registered_coordinate((x_origin + x) as u32, info.x_subsampling as u32, info.x_registration, false),
                        color_registered_coordinate((y_origin + y) as u32, info.y_subsampling as u32, info.y_registration, false),
                    ) {
                        (Err(error), _) => Err(error),
                        (_, Err(error)) => Err(error),
                        (Ok(raw_x), Ok(raw_y)) => {
                            let x1 = (x0 as nat) + component.width as nat;
                            let y1 = (y0 as nat) + component.height as nat;
                            let cx = if raw_x < x0 { x0 as nat } else if raw_x as nat >= x1 { (x1 - 1) as nat } else { raw_x as nat };
                            let cy = if raw_y < y0 { y0 as nat } else if raw_y as nat >= y1 { (y1 - 1) as nat } else { raw_y as nat };
                            let index = ((cy - y0 as nat) as nat) * (component.width as nat) + (cx - x0 as nat);
                            if index > usize::MAX as nat { Err(part2_composition_error(true)) }
                            else { Ok(index as usize) }
                        },
                    }
                }
            },
        }
    }
}

pub open spec fn color_component_nominal(
    component: Part1TypedComponentModel, x_origin: u32, y_origin: u32, x: u32, y: u32,
) -> Result<u64, crate::DecodeError> {
    match color_component_index(component, x_origin, y_origin, x, y) {
        Err(error) => Err(error),
        Ok(index) => match component.samples {
            Part1TypedSampleSequence::Unsigned(samples) => {
                let maximum = ((1u64 << component.info.precision) - 1) as nat;
                if component.info.signed || index >= samples.len() || samples[index as int] as nat > maximum {
                    Err(part2_color_input_error())
                } else { Ok(samples[index as int]) }
            },
            Part1TypedSampleSequence::Signed(samples) => {
                let level = 1i64 << ((component.info.precision - 1) as u8);
                if !component.info.signed || index >= samples.len()
                    || samples[index as int] < -(level as int)
                    || samples[index as int] >= level {
                    Err(part2_color_input_error())
                } else if samples[index as int] > (i64::MAX - level) as int {
                    Err(part2_composition_error(true))
                } else { Ok((samples[index as int] + level) as u64) }
            },
        },
    }
}

pub open spec fn color_component_normalized(
    component: Part1TypedComponentModel, x_origin: u32, y_origin: u32, x: u32, y: u32,
) -> Result<u16, crate::DecodeError> {
    match color_component_nominal(component, x_origin, y_origin, x, y) {
        Err(error) => Err(error),
        Ok(value) => color_normalize_unsigned(value, component.info.precision),
    }
}

pub open spec fn color_normalize_difference(value: u64, precision: u8)
    -> Result<i32, crate::DecodeError>
{
    if precision == 0 || precision > 38 {
        Err(part2_color_input_error())
    } else {
        let maximum = ((1u64 << precision) - 1) as nat;
        if value as nat > maximum { Err(part2_color_input_error()) }
        else {
            let midpoint = 1u64 << ((precision - 1) as u8);
            let scaled = ((value as int) - midpoint as int) * (PART2_NORMALIZED_SAMPLE_MAX as int);
            let rounded = if scaled < 0 {
                -((-scaled + maximum / 2) / (maximum as int))
            } else { (scaled + maximum / 2) / (maximum as int) };
            if rounded < i32::MIN || rounded > i32::MAX {
                Err(part2_composition_error(true))
            } else { Ok(rounded as i32) }
        }
    }
}

pub open spec fn color_image_normalized(
    components: Seq<Part1TypedComponentModel>, x_origin: u32, y_origin: u32,
    component_index: usize, x: u32, y: u32,
) -> Result<u16, crate::DecodeError> {
    if component_index >= components.len() { Err(part2_color_input_error()) }
    else { color_component_normalized(components[component_index as int], x_origin, y_origin, x, y) }
}

pub open spec fn color_image_difference(
    components: Seq<Part1TypedComponentModel>, x_origin: u32, y_origin: u32,
    component_index: usize, x: u32, y: u32,
) -> Result<i32, crate::DecodeError> {
    if component_index >= components.len() { Err(part2_color_input_error()) }
    else {
        let component = components[component_index as int];
        if component.info.precision == 0 || component.info.precision > 38 {
            Err(part2_color_input_error())
        } else {
            match color_component_nominal(component, x_origin, y_origin, x, y) {
                Err(error) => Err(error),
                Ok(value) => color_normalize_difference(value, component.info.precision),
            }
        }
    }
}

pub type ColorGeometry = (u32, u32, u32, u32);

pub open spec fn color_registered_span(start: u32, end: u32, step: u8, registration: u16)
    -> Result<u32, crate::DecodeError>
{
    match (
        color_registered_coordinate(end, step as u32, registration, true),
        color_registered_coordinate(start, step as u32, registration, true),
    ) {
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(error),
        (Ok(last), Ok(first)) => if last < first { Err(part2_composition_error(true)) }
            else { Ok((last - first) as u32) },
    }
}

pub open spec fn color_validate_component(component: Part1TypedComponentModel, geometry: ColorGeometry)
    -> Result<(), crate::DecodeError>
{
    let info = component.info;
    let signed = match component.samples {
        Part1TypedSampleSequence::Signed(_) => true,
        Part1TypedSampleSequence::Unsigned(_) => false,
    };
    if info.precision == 0 || info.precision > 38 || info.signed != signed
        || info.x_subsampling == 0 || info.y_subsampling == 0 {
        Err(part2_color_input_error())
    } else if (geometry.2 as nat) + geometry.0 as nat > u32::MAX as nat
        || (geometry.3 as nat) + geometry.1 as nat > u32::MAX as nat {
        Err(part2_composition_error(true))
    } else {
        match (
            color_registered_span(geometry.2, (geometry.2 + geometry.0) as u32,
                info.x_subsampling, info.x_registration),
            color_registered_span(geometry.3, (geometry.3 + geometry.1) as u32,
                info.y_subsampling, info.y_registration),
        ) {
            (Err(error), _) => Err(error),
            (_, Err(error)) => Err(error),
            (Ok(width), Ok(height)) => {
                if component.width != width || component.height != height {
                    Err(part2_color_input_error())
                } else {
                    match part2_layer_area(width, height) {
                        Err(error) => Err(error),
                        Ok(area) => {
                            let samples = match component.samples {
                                Part1TypedSampleSequence::Signed(values) => values.len(),
                                Part1TypedSampleSequence::Unsigned(values) => values.len(),
                            };
                            if samples != area { Err(part2_color_input_error()) } else { Ok(()) }
                        },
                    }
                }
            },
        }
    }
}

pub open spec fn color_validate_component_index(
    components: Seq<Part1TypedComponentModel>, geometry: ColorGeometry, index: usize,
) -> Result<(), crate::DecodeError> {
    if index >= components.len() { Err(part2_color_input_error()) }
    else { color_validate_component(components[index as int], geometry) }
}

#[verifier::opaque]
pub open spec fn color_validate_indices_prefix(
    components: Seq<Part1TypedComponentModel>, geometry: ColorGeometry, indices: Seq<usize>, count: nat,
) -> Result<(), crate::DecodeError>
    recommends count <= indices.len(),
    decreases count,
{
    if count == 0 { Ok(()) } else {
        match color_validate_indices_prefix(components, geometry, indices, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(()) => color_validate_component_index(components, geometry, indices[count - 1]),
        }
    }
}

pub open spec fn color_validate_components(
    components: Seq<Part1TypedComponentModel>, geometry: ColorGeometry, layout: ColorLayoutState,
) -> Result<(), crate::DecodeError> {
    match color_validate_indices_prefix(components, geometry, layout.0, layout.0.len()) {
        Err(error) => Err(error),
        Ok(()) => {
            let indices = Seq::new(layout.1.len(), |i: int| layout.1[i].0);
            color_validate_indices_prefix(components, geometry, indices, indices.len())
        },
    }
}

pub open spec fn color_validate_output_size(
    width: u32, height: u32, colors: usize, alphas: usize, byte_limit: u64, work_limit: u64,
) -> Result<(), crate::DecodeError> {
    if (colors as nat) + alphas as nat > usize::MAX as nat {
        Err(part2_composition_error(true))
    } else {
        match part2_layer_area(width, height) {
            Err(error) => Err(error),
            Ok(area) => {
                let samples = (area as nat) * ((colors as nat) + alphas as nat);
                let bytes = ((samples as u64) as nat) * 2;
                if samples > usize::MAX as nat || bytes > u64::MAX as nat {
                    Err(part2_composition_error(true))
                } else if bytes > byte_limit as nat {
                    Err(crate::DecodeError::LimitExceeded {
                        resource: crate::Resource::DecodedBytes, limit: byte_limit, actual: bytes as u64,
                    })
                } else if samples > u64::MAX as nat {
                    Err(part2_composition_error(true))
                } else if samples > work_limit as nat {
                    Err(crate::DecodeError::LimitExceeded {
                        resource: crate::Resource::WorkUnits, limit: work_limit, actual: samples as u64,
                    })
                } else { Ok(()) }
            },
        }
    }
}

pub type ColorChannelModel = (crate::color::ConvertedChannelType, Seq<u16>);
pub type ColorSyccPlanes = (Seq<u16>, Seq<u16>, Seq<u16>);
pub type ColorImageModel = (u32, u32, crate::color::ConvertedColorSpace, u8, Seq<ColorChannelModel>);

pub open spec fn color_convert_channel(
    components: Seq<Part1TypedComponentModel>, geometry: ColorGeometry,
    colors: Seq<usize>, color_count: usize, index: usize,
) -> Result<ColorChannelModel, crate::DecodeError> {
    if index >= colors.len() || colors[index as int] >= components.len() {
        Err(part2_color_input_error())
    } else {
        match color_normalize_component(components[colors[index as int] as int],
            geometry.0, geometry.1, geometry.2, geometry.3) {
            Err(error) => Err(error),
            Ok(samples) => match (color_count, index) {
                (1, 0) => Ok((crate::color::ConvertedChannelType::Gray, samples)),
                (3, 0) => Ok((crate::color::ConvertedChannelType::Red, samples)),
                (3, 1) => Ok((crate::color::ConvertedChannelType::Green, samples)),
                (3, 2) => Ok((crate::color::ConvertedChannelType::Blue, samples)),
                _ => Err(part2_color_input_error()),
            },
        }
    }
}

#[verifier::opaque]
pub open spec fn color_convert_channels_prefix(
    components: Seq<Part1TypedComponentModel>, geometry: ColorGeometry,
    colors: Seq<usize>, color_count: usize, count: nat,
) -> Result<Seq<ColorChannelModel>, crate::DecodeError>
    decreases count,
{
    if count == 0 { Ok(Seq::empty()) } else {
        match color_convert_channels_prefix(components, geometry, colors, color_count, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(channels) => if count - 1 > usize::MAX {
                Err(part2_composition_error(true))
            } else {
                match color_convert_channel(components, geometry, colors, color_count, (count - 1) as usize) {
                    Err(error) => Err(error),
                    Ok(channel) => Ok(channels.push(channel)),
                }
            },
        }
    }
}

pub open spec fn color_convert_alpha(
    components: Seq<Part1TypedComponentModel>, geometry: ColorGeometry,
    alpha: (usize, bool, crate::image::ChannelAssociation),
) -> Result<ColorChannelModel, crate::DecodeError> {
    if alpha.0 >= components.len() { Err(part2_color_input_error()) } else {
        match color_normalize_component(components[alpha.0 as int],
            geometry.0, geometry.1, geometry.2, geometry.3) {
            Err(error) => Err(error),
            Ok(samples) => Ok((crate::color::ConvertedChannelType::Alpha {
                premultiplied: alpha.1, association: alpha.2,
            }, samples)),
        }
    }
}

#[verifier::opaque]
pub open spec fn color_append_alphas_prefix(
    components: Seq<Part1TypedComponentModel>, geometry: ColorGeometry,
    alphas: Seq<(usize, bool, crate::image::ChannelAssociation)>,
    initial: Seq<ColorChannelModel>, count: nat,
) -> Result<Seq<ColorChannelModel>, crate::DecodeError>
    recommends count <= alphas.len(),
    decreases count,
{
    if count == 0 { Ok(initial) } else {
        match color_append_alphas_prefix(components, geometry, alphas, initial, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(channels) => match color_convert_alpha(components, geometry, alphas[count - 1]) {
                Err(error) => Err(error),
                Ok(channel) => Ok(channels.push(channel)),
            },
        }
    }
}

pub open spec fn color_sycc_pixel(
    components: Seq<Part1TypedComponentModel>, x_origin: u32, y_origin: u32,
    colors: Seq<usize>, pixel: usize, width: usize,
) -> Result<(u16, u16, u16), crate::DecodeError> {
    match color_pixel_coordinates(pixel, width) {
        Err(error) => Err(error),
        Ok((x, y)) => {
            if colors.len() == 0 { Err(part2_color_input_error()) } else {
                match color_image_normalized(components, x_origin, y_origin, colors[0], x, y) {
                    Err(error) => Err(error),
                    Ok(y_sample) => if colors.len() <= 1 { Err(part2_color_input_error()) } else {
                        match color_image_difference(components, x_origin, y_origin, colors[1], x, y) {
                            Err(error) => Err(error),
                            Ok(cb) => if colors.len() <= 2 { Err(part2_color_input_error()) } else {
                                match color_image_difference(components, x_origin, y_origin, colors[2], x, y) {
                                    Err(error) => Err(error),
                                    Ok(cr) => color_sycc_to_srgb(y_sample, cb, cr),
                                }
                            },
                        }
                    },
                }
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn color_sycc_planes_prefix(
    components: Seq<Part1TypedComponentModel>, geometry: ColorGeometry,
    colors: Seq<usize>, width: usize, count: nat,
) -> Result<ColorSyccPlanes, crate::DecodeError>
    decreases count,
{
    if count == 0 { Ok((Seq::empty(), Seq::empty(), Seq::empty())) } else {
        match color_sycc_planes_prefix(components, geometry, colors, width, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(planes) => if count - 1 > usize::MAX {
                Err(part2_composition_error(true))
            } else {
                match color_sycc_pixel(components, geometry.2, geometry.3, colors, (count - 1) as usize, width) {
                    Err(error) => Err(error),
                    Ok((r, g, b)) => Ok((planes.0.push(r), planes.1.push(g), planes.2.push(b))),
                }
            },
        }
    }
}

pub open spec fn color_convert_colors(
    components: Seq<Part1TypedComponentModel>, geometry: ColorGeometry,
    colors: Seq<usize>, color_count: usize, sycc: bool,
) -> Result<Seq<ColorChannelModel>, crate::DecodeError> {
    if sycc {
        if geometry.0 as nat > usize::MAX as nat { Err(part2_composition_error(true)) }
        else if geometry.0 == 0 { Err(part2_color_input_error()) }
        else {
            match part2_layer_area(geometry.0, geometry.1) {
                Err(error) => Err(error),
                Ok(area) => match color_sycc_planes_prefix(components, geometry, colors, geometry.0 as usize, area as nat) {
                    Err(error) => Err(error),
                    Ok(planes) => Ok(seq![
                        (crate::color::ConvertedChannelType::Red, planes.0),
                        (crate::color::ConvertedChannelType::Green, planes.1),
                        (crate::color::ConvertedChannelType::Blue, planes.2),
                    ]),
                },
            }
        }
    } else { color_convert_channels_prefix(components, geometry, colors, color_count, color_count as nat) }
}

pub open spec fn color_output_format(method: crate::jp2::ColorMethod)
    -> Result<(usize, crate::color::ConvertedColorSpace, bool), crate::DecodeError>
{
    match method {
        crate::jp2::ColorMethod::Enumerated { color_space: crate::jp2::ENUMERATED_SRGB } =>
            Ok((3, crate::color::ConvertedColorSpace::Srgb, false)),
        crate::jp2::ColorMethod::Enumerated { color_space: crate::jp2::ENUMERATED_GRAYSCALE } =>
            Ok((1, crate::color::ConvertedColorSpace::Grayscale, false)),
        crate::jp2::ColorMethod::Enumerated { color_space: crate::jp2::ENUMERATED_SYCC } =>
            Ok((3, crate::color::ConvertedColorSpace::Srgb, true)),
        crate::jp2::ColorMethod::Enumerated { .. } => Err(crate::DecodeError::UnsupportedColorConversion {
            reason: crate::UnsupportedColorConversion::EnumeratedColorSpace,
        }),
        _ => Err(crate::DecodeError::UnsupportedColorConversion {
            reason: crate::UnsupportedColorConversion::IccProfile,
        }),
    }
}

pub open spec fn color_convert_jp2(
    components: Seq<Part1TypedComponentModel>, definitions: Seq<Seq<ColorDefinition>>,
    geometry: ColorGeometry, method: crate::jp2::ColorMethod, byte_limit: u64, work_limit: u64,
) -> Part2ConvertedImageResult {
    if geometry.0 == 0 || geometry.1 == 0 { Err(part2_color_input_error()) }
    else {
        match color_output_format(method) {
            Err(error) => Err(error),
            Ok((color_count, space, sycc)) => match color_channel_layout(definitions, color_count) {
                Err(error) => Err(error),
                Ok(layout) => match color_validate_components(components, geometry, layout) {
                    Err(error) => Err(error),
                    Ok(()) => match color_validate_output_size(
                        geometry.0, geometry.1, color_count, layout.1.len() as usize, byte_limit, work_limit,
                    ) {
                        Err(error) => Err(error),
                        Ok(()) => match part2_layer_area(geometry.0, geometry.1) {
                            Err(error) => Err(error),
                            Ok(_) => match color_convert_colors(components, geometry, layout.0, color_count, sycc) {
                                Err(error) => Err(error),
                                Ok(channels) => match color_append_alphas_prefix(
                                    components, geometry, layout.1, channels, layout.1.len(),
                                ) {
                                    Err(error) => Err(error),
                                    Ok(output) => Ok((geometry.0, geometry.1, space, 16, output)),
                                },
                            },
                        },
                    },
                },
            },
        }
    }
}

pub open spec fn color_crop_sample(samples: Seq<u16>, width: u32, row: u32, column: u32)
    -> Result<u16, crate::DecodeError>
{
    let index = (row as nat) * (width as nat) + column as nat;
    if index > usize::MAX as nat { Err(part2_composition_error(true)) }
    else if index >= samples.len() { Err(part2_color_input_error()) }
    else { Ok(samples[index as int]) }
}

#[verifier::opaque]
pub open spec fn color_crop_columns(
    samples: Seq<u16>, source_width: u32, row: u32, x: u32, initial: Seq<u16>, count: nat,
) -> Result<Seq<u16>, crate::DecodeError>
    decreases count,
{
    if count == 0 { Ok(initial) } else {
        match color_crop_columns(samples, source_width, row, x, initial, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(output) => {
                let column = (x as nat) + count - 1;
                if column > u32::MAX as nat { Err(part2_composition_error(true)) }
                else {
                    match color_crop_sample(samples, source_width, row, column as u32) {
                        Err(error) => Err(error),
                        Ok(value) => Ok(output.push(value)),
                    }
                }
            },
        }
    }
}

#[verifier::opaque]
pub open spec fn color_crop_rows(
    samples: Seq<u16>, source_width: u32, x: u32, y: u32, width: u32, count: nat,
) -> Result<Seq<u16>, crate::DecodeError>
    decreases count,
{
    if count == 0 { Ok(Seq::empty()) } else {
        match color_crop_rows(samples, source_width, x, y, width, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(output) => {
                let row = (y as nat) + count - 1;
                if row > u32::MAX as nat { Err(part2_composition_error(true)) }
                else { color_crop_columns(samples, source_width, row as u32, x, output, width as nat) }
            },
        }
    }
}

pub open spec fn color_crop_plane(
    samples: Seq<u16>, source_width: u32, x: u32, y: u32, x1: u32, y1: u32,
) -> Result<Seq<u16>, crate::DecodeError> {
    if x > x1 || y > y1 { Err(part2_color_input_error()) }
    else { color_crop_rows(samples, source_width, x, y, (x1 - x) as u32, (y1 - y) as nat) }
}

#[verifier::opaque]
pub open spec fn color_crop_channels(
    channels: Seq<ColorChannelModel>, source_width: u32, x: u32, y: u32, x1: u32, y1: u32, count: nat,
) -> Result<Seq<ColorChannelModel>, crate::DecodeError>
    recommends count <= channels.len(),
    decreases count,
{
    if count == 0 { Ok(Seq::empty()) } else {
        match color_crop_channels(channels, source_width, x, y, x1, y1, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(output) => {
                let channel = channels[count - 1];
                match color_crop_plane(channel.1, source_width, x, y, x1, y1) {
                    Err(error) => Err(error),
                    Ok(samples) => Ok(output.push((channel.0, samples))),
                }
            },
        }
    }
}

pub open spec fn color_crop_image(image: ColorImageModel, x: u32, y: u32, width: u32, height: u32)
    -> Part2ConvertedImageResult
{
    if width == 0 || height == 0 { Err(part2_color_input_error()) }
    else if (x as nat) + width as nat > u32::MAX as nat
        || (y as nat) + height as nat > u32::MAX as nat {
        Err(part2_composition_error(true))
    } else {
        let x1 = (x + width) as u32;
        let y1 = (y + height) as u32;
        if x1 > image.0 || y1 > image.1 { Err(part2_color_input_error()) }
        else if x == 0 && y == 0 && width == image.0 && height == image.1 { Ok(image) }
        else {
            match part2_layer_area(width, height) {
                Err(error) => Err(error),
                Ok(_) => match color_crop_channels(image.4, image.0, x, y, x1, y1, image.4.len()) {
                    Err(error) => Err(error),
                    Ok(channels) => Ok((width, height, image.2, image.3, channels)),
                },
            }
        }
    }
}

pub open spec fn color_selection_target_error() -> crate::DecodeError {
    crate::DecodeError::MalformedInput { offset: 0, kind: crate::MalformedKind::InvalidCodeBlock }
}

pub open spec fn color_convert_selected(
    components: Seq<Part1TypedComponentModel>, definitions: Seq<Seq<ColorDefinition>>,
    geometry: ColorGeometry, method: crate::jp2::ColorMethod, target: [u32; 4],
    byte_limit: u64, work_limit: u64,
) -> Part2ConvertedImageResult {
    match color_convert_jp2(components, definitions, geometry, method, byte_limit, work_limit) {
        Err(error) => Err(error),
        Ok(image) => {
            if target[0] < geometry.2 || target[1] < geometry.3
                || target[2] < target[0] || target[3] < target[1] {
                Err(color_selection_target_error())
            } else {
                color_crop_image(image, (target[0] - geometry.2) as u32, (target[1] - geometry.3) as u32,
                    (target[2] - target[0]) as u32, (target[3] - target[1]) as u32)
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn color_normalize_component_prefix(
    component: Part1TypedComponentModel, width: u32, x_origin: u32, y_origin: u32, count: nat,
) -> Result<Seq<u16>, crate::DecodeError>
    recommends width > 0,
    decreases count,
{
    if count == 0 { Ok(Seq::empty()) } else {
        match color_normalize_component_prefix(component, width, x_origin, y_origin, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(samples) => {
                let pixel = (count - 1) as nat;
                if pixel > usize::MAX as nat {
                    Err(part2_composition_error(true))
                } else {
                    match color_pixel_coordinates(pixel as usize, width as usize) {
                        Err(error) => Err(error),
                        Ok((x, y)) => match color_component_normalized(component, x_origin, y_origin, x, y) {
                            Err(error) => Err(error),
                            Ok(value) => Ok(samples.push(value)),
                        },
                    }
                }
            },
        }
    }
}

pub open spec fn color_normalize_component(
    component: Part1TypedComponentModel, width: u32, height: u32, x_origin: u32, y_origin: u32,
) -> Result<Seq<u16>, crate::DecodeError> {
    if width == 0 { Err(part2_color_input_error()) }
    else if width as nat > usize::MAX as nat { Err(part2_composition_error(true)) }
    else {
        match part2_layer_area(width, height) {
            Err(error) => Err(error),
            Ok(area) => color_normalize_component_prefix(component, width, x_origin, y_origin, area as nat),
        }
    }
}

pub open spec fn color_pixel_coordinates(pixel: usize, width: usize)
    -> Result<(u32, u32), crate::DecodeError> {
    if width == 0 { Err(part2_color_input_error()) }
    else if (pixel % width) as nat > u32::MAX as nat || (pixel / width) as nat > u32::MAX as nat {
        Err(part2_composition_error(true))
    } else { Ok(((pixel % width) as u32, (pixel / width) as u32)) }
}

pub type ColorDefinition = (crate::image::ChannelType, crate::image::ChannelAssociation);
pub type ColorLayoutState = (Seq<usize>, Seq<(usize, bool, crate::image::ChannelAssociation)>);
pub type ColorLayoutResult = Result<ColorLayoutState, crate::DecodeError>;

pub open spec fn color_layout_error() -> crate::DecodeError {
    crate::DecodeError::MalformedInput { offset: 0, kind: crate::MalformedKind::InvalidChannelLayout }
}

pub open spec fn color_alpha_association(association: crate::image::ChannelAssociation, count: usize)
    -> Result<(), crate::DecodeError>
{
    match association {
        crate::image::ChannelAssociation::Color(value) if value == 0 || value as nat > count as nat =>
            Err(crate::DecodeError::MalformedInput { offset: 0, kind: crate::MalformedKind::InvalidAlphaAssociation }),
        _ => Ok(()),
    }
}

pub open spec fn color_layout_definition_step(
    state: ColorLayoutState, definition: ColorDefinition, component: usize, color_count: usize,
) -> ColorLayoutResult {
    match definition.0 {
        crate::image::ChannelType::Color => match definition.1 {
            crate::image::ChannelAssociation::Color(value) => {
                if value == 0 || value as nat > color_count as nat { Err(color_layout_error()) }
                else {
                    let slot = (value - 1) as int;
                    if slot >= state.0.len() { Err(part2_color_input_error()) }
                    else if state.0[slot] != usize::MAX && state.0[slot] != component {
                        Err(color_layout_error())
                    } else {
                        Ok((state.0.update(slot, component), state.1))
                    }
                }
            },
            _ => Err(color_layout_error()),
        },
        crate::image::ChannelType::Opacity | crate::image::ChannelType::PremultipliedOpacity =>
            match color_alpha_association(definition.1, color_count) {
                Err(error) => Err(error),
                Ok(()) => Ok((state.0, state.1.push((component,
                    matches!(definition.0, crate::image::ChannelType::PremultipliedOpacity), definition.1)))),
            },
        crate::image::ChannelType::Unspecified => Ok(state),
    }
}

#[verifier::opaque]
pub open spec fn color_layout_definitions_prefix(
    initial: ColorLayoutState, definitions: Seq<ColorDefinition>,
    component: usize, color_count: usize, count: nat,
) -> ColorLayoutResult
    recommends count <= definitions.len(),
    decreases count,
{
    if count == 0 { Ok(initial) } else {
        match color_layout_definitions_prefix(initial, definitions, component, color_count, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(state) => color_layout_definition_step(state, definitions[(count - 1) as int], component, color_count),
        }
    }
}

#[verifier::opaque]
pub open spec fn color_layout_components_prefix(
    definitions: Seq<Seq<ColorDefinition>>, color_count: usize, count: nat,
) -> ColorLayoutResult
    recommends count <= definitions.len(),
    decreases count,
{
    if count == 0 { Ok((Seq::new(color_count as nat, |i: int| usize::MAX), Seq::empty())) }
    else {
        match color_layout_components_prefix(definitions, color_count, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(state) => color_layout_definitions_prefix(state, definitions[(count - 1) as int],
                (count - 1) as usize, color_count, definitions[(count - 1) as int].len()),
        }
    }
}

pub open spec fn color_channel_layout(definitions: Seq<Seq<ColorDefinition>>, color_count: usize)
    -> ColorLayoutResult
{
    if !(exists|i: int| #![trigger definitions[i]] 0 <= i < definitions.len() && definitions[i].len() > 0) {
        if definitions.len() != color_count as nat { Err(color_layout_error()) }
        else { Ok((Seq::new(color_count as nat, |i: int| i as usize), Seq::empty())) }
    } else {
        match color_layout_components_prefix(definitions, color_count, definitions.len()) {
            Err(error) => Err(error),
            Ok(state) => if exists|i: int| #![trigger state.0[i]] 0 <= i < state.0.len() && state.0[i] == usize::MAX {
                Err(color_layout_error())
            } else { Ok(state) },
        }
    }
}

pub open spec fn region_fits(parent_length: nat, offset: nat, length: nat) -> bool {
    offset <= parent_length && length <= parent_length - offset
}

pub open spec fn valid_box_lengths(header_length: nat, total_length: nat) -> bool {
    header_length <= total_length
}

pub open spec fn work_within_limit(consumed: nat, limit: nat) -> bool {
    consumed <= limit
}

pub open spec fn valid_selection_region(
    image_x0: nat,
    image_y0: nat,
    image_x1: nat,
    image_y1: nat,
    region_x0: nat,
    region_y0: nat,
    region_x1: nat,
    region_y1: nat,
) -> bool {
    image_x0 <= region_x0
        && image_y0 <= region_y0
        && region_x0 < region_x1
        && region_y0 < region_y1
        && region_x1 <= image_x1
        && region_y1 <= image_y1
}

pub open spec fn reduced_extent(start: nat, end: nat, scale: nat) -> nat
    recommends
        scale > 0,
        start <= end,
{
    (
        (((end + scale - 1) as int) / (scale as int))
            - (((start + scale - 1) as int) / (scale as int))
    ) as nat
}

pub open spec fn selected_packet_work(selected_packets: nat, packet_limit: nat) -> bool {
    selected_packets <= packet_limit
}

pub open spec fn valid_selected_codestream_index(index: nat, codestreams: nat) -> bool {
    codestreams > 0 && index < codestreams
}

pub open spec fn valid_codestream_header_associations(headers: nat, codestreams: nat) -> bool {
    headers <= codestreams
}

pub open spec fn valid_jph_branding(
    primary_is_jph: bool,
    minor_version: nat,
    compatible_brands: nat,
    sole_compatible_is_jph: bool,
) -> bool {
    primary_is_jph
        && minor_version == 0
        && compatible_brands == 1
        && sole_compatible_is_jph
}

pub open spec fn consistent_part15_signaling(
    rsiz_announces_ht: bool,
    cap_announces_ht: bool,
    style_uses_ht: bool,
    has_corresponding_profile: bool,
) -> bool {
    rsiz_announces_ht == cap_announces_ht
        && (!style_uses_ht || cap_announces_ht)
        && (!has_corresponding_profile || cap_announces_ht)
}

pub open spec fn valid_part15_block_mode(mode: nat) -> bool {
    mode == 0 || mode == 2 || mode == 3
}

pub open spec fn valid_part15_capability_word(ccap: nat) -> bool {
    ccap < 65536
        && valid_part15_block_mode(ccap / 16384)
        && ccap % 2048 < 64
}

pub open spec fn valid_ht_style_prefix(prefix: nat) -> bool {
    prefix == 0 || prefix == 1 || prefix == 3
}

pub open spec fn valid_ht_cleanup_partition(cleanup_length: nat, scup: nat) -> bool {
    2 <= scup && scup <= cleanup_length && scup <= 4079
}

pub open spec fn ht_cleanup_byte_claims_valid(
    mel_byte: nat,
    vlc_byte: nat,
    terminal_byte: nat,
    value: u8,
) -> bool {
    mel_byte <= terminal_byte
        && vlc_byte <= terminal_byte
        && (mel_byte < vlc_byte || (mel_byte == vlc_byte && mel_byte < terminal_byte && value != 0xff))
}

pub open spec fn ht_noninitial_vlc_context(
    west_context: u8, north_west: u8, north: u8, north_east: u8,
) -> u8 {
    ((if west_context & 2 != 0 { 2int } else { 0int })
        + (if north_west & 8 != 0 || north & 2 != 0 { 1int } else { 0int })
        + (if north & 8 != 0 || north_east & 2 != 0 { 4int } else { 0int })) as u8
}

pub open spec fn valid_ht_quad_origin(x: nat, y: nat, width: nat, height: nat) -> bool {
    width > 0 && height > 0 && x < width && y < height && x % 2 == 0 && y % 2 == 0
}

pub open spec fn ht_quad_count(width: nat, height: nat) -> nat {
    ((width + 1) / 2) * ((height + 1) / 2)
}

pub open spec fn ht_kappa_extension(gamma: bool, exponent: nat) -> nat {
    if gamma && exponent > 2 {
        (exponent - 2) as nat
    } else {
        0
    }
}

pub open spec fn ht_completed_u(u_base: nat, gamma: bool, exponent: nat) -> nat {
    u_base + ht_kappa_extension(gamma, exponent)
}

pub open spec fn valid_ht_magnitude_width(completed_u: nat) -> bool {
    completed_u > 0 && completed_u <= 75
}

pub open spec fn ht_placeholder_passes(packet_passes: nat) -> nat {
    if packet_passes == 0 {
        0
    } else {
        (((packet_passes - 1) / 3) * 3) as nat
    }
}

pub open spec fn ht_cleanup_passes(packet_passes: nat) -> nat {
    if packet_passes == 0 {
        0
    } else {
        ht_placeholder_passes(packet_passes) + 1
    }
}

pub open spec fn ht_refinement_passes(packet_passes: nat) -> nat {
    (packet_passes - ht_cleanup_passes(packet_passes)) as nat
}

pub open spec fn valid_ht_pass_group(packet_passes: nat) -> bool {
    1 <= packet_passes && packet_passes <= 164
}

pub open spec fn valid_ht_refinement_segment(bytes: nat, passes: nat) -> bool {
    bytes <= 2046 && 1 <= passes && passes <= 2
}

pub open spec fn disjoint_ht_refinement_claims(forward_end: nat, reverse_start: nat) -> bool {
    forward_end <= reverse_start
}

pub open spec fn ht_substantive_passes(packet_passes: nat) -> nat {
    (packet_passes - ht_placeholder_passes(packet_passes)) as nat
}

pub open spec fn mixed_block_selects_ht(lblock: nat, first_length_bit_is_zero: bool) -> bool {
    lblock > 3 && first_length_bit_is_zero
}

pub open spec fn part15_cap_payload_length(capabilities: u8) -> usize {
    (4 + 2 * capabilities as int) as usize
}

pub open spec fn part15_signaling_valid(
    rsiz_ht: bool,
    pcap_ht: bool,
    styles_ht: bool,
    corresponding_profile: bool,
) -> bool {
    rsiz_ht == pcap_ht
        && (!styles_ht || pcap_ht)
        && (!corresponding_profile || pcap_ht)
}

pub open spec fn part15_codestream_class(
    has_legacy: bool,
    has_ht: bool,
    has_mixed: bool,
) -> nat {
    if has_mixed || (has_legacy && has_ht) {
        2
    } else if has_ht {
        1
    } else {
        0
    }
}

pub open spec fn part15_mode_allows(
    mode: nat,
    has_legacy: bool,
    has_mixed: bool,
) -> bool {
    valid_part15_block_mode(mode)
        && (mode != 0 || !has_legacy)
        && (mode != 2 || !has_mixed)
}

pub open spec fn part15_magnitude_bound(parameter: nat) -> nat {
    if parameter == 0 {
        8
    } else if parameter <= 19 {
        parameter + 8
    } else if parameter <= 30 {
        (4 * (parameter - 19) + 27) as nat
    } else {
        74
    }
}

pub open spec fn part15_cleanup_u_bound(
    missing_msbs: nat, placeholder_bitplanes: nat, capability_bound: nat,
) -> nat {
    let bound = missing_msbs + placeholder_bitplanes + 2;
    let capability_limit = if capability_bound < 255 { capability_bound + 1 } else { 255 };
    if bound <= capability_limit { bound } else { capability_limit }
}

pub open spec fn part15_component_allowed(
    roi_allowed: bool,
    has_roi: bool,
    roi_shift: nat,
    irreversible_allowed: bool,
    uses_ht: bool,
    irreversible: bool,
) -> bool {
    (!has_roi || (roi_allowed && roi_shift <= 37))
        && (irreversible_allowed || !uses_ht || !irreversible)
}

pub open spec fn part15_capability_fields_valid(
    mode: nat,
    reserved: nat,
    magnitude_parameter: nat,
) -> bool {
    valid_part15_block_mode(mode) && reserved == 0 && magnitude_parameter < 32
}

pub open spec fn part15_headers_allowed(
    heterogeneous_allowed: bool,
    has_functional_tile_header: bool,
) -> bool {
    heterogeneous_allowed || !has_functional_tile_header
}

pub open spec fn ht_segment_passes(phase: nat, remaining: nat) -> nat {
    if phase == 0 {
        1
    } else if remaining < 3 - phase {
        remaining
    } else {
        (3 - phase) as nat
    }
}

pub open spec fn ht_mixed_selects_legacy(
    segment_length: nat,
    legacy_length: nat,
    cleanup_phase: bool,
    lblock_gt_three: bool,
    leading_zero: bool,
) -> bool {
    (segment_length == 0 && legacy_length != 0)
        || (segment_length != 0 && !(cleanup_phase && lblock_gt_three && leading_zero))
}

pub open spec fn ht_scup(penultimate: u8, last: u8) -> usize {
    (last as int * 16 + penultimate as int % 16) as usize
}

pub open spec fn ht_magsgn_end(cleanup_length: nat, scup: nat) -> nat
    recommends
        scup <= cleanup_length,
{
    (cleanup_length - scup) as nat
}

pub open spec fn ht_forward_data_bits(stuffed: bool) -> nat {
    if stuffed { 7 } else { 8 }
}

pub open spec fn ht_forward_stuffing_valid(stuffed: bool, value: u8) -> bool {
    !stuffed || value < 128
}

pub open spec fn ht_mel_stuffing_valid(previous_ff: bool, value: nat) -> bool {
    !previous_ff || value <= 143
}

pub open spec fn ht_reverse_data_bits(needs_unstuffing: bool, value: u8) -> nat {
    if needs_unstuffing && value % 128 == 127 { 7 } else { 8 }
}

pub open spec fn ht_mel_exponent(state: nat) -> nat {
    if state <= 2 {
        0
    } else if state <= 5 {
        1
    } else if state <= 8 {
        2
    } else if state <= 10 {
        3
    } else if state == 11 {
        4
    } else {
        5
    }
}

pub open spec fn ht_mel_next_state(state: nat, run: bool) -> nat {
    if run {
        if state < 12 { state + 1 } else { 12 }
    } else if state > 0 {
        (state - 1) as nat
    } else {
        0
    }
}

pub open spec fn ht_vlc_table_index(context: nat, lookahead: nat) -> nat {
    context * 128 + lookahead
}

pub open spec fn ht_vlc_word_valid(word: nat, length: nat) -> bool {
    length <= 7 && (word == 0 || length > 0)
}

pub open spec fn ht_uvlc_value(base: nat, suffix: nat, bias: nat) -> nat {
    base + suffix + bias
}

pub open spec fn ht_quad_sample_in_bounds(
    x: nat,
    y: nat,
    sample: nat,
    width: nat,
    height: nat,
) -> bool {
    sample < 4 && x + sample / 2 < width && y + sample % 2 < height
}

pub open spec fn ht_quad_origin_valid(x: nat, y: nat, width: nat, height: nat) -> bool {
    x < width && y < height && x % 2 == 0 && y % 2 == 0
}

pub open spec fn ht_kappa_completion(u_base: nat, gamma: bool, exponent: nat) -> nat {
    u_base + if gamma && exponent > 2 { (exponent - 2) as nat } else { 0 }
}

pub open spec fn ht_magsgn_fill_value() -> nat {
    255
}

pub open spec fn ht_magnitude_payload_bits(completed_u: nat, embedded: bool) -> nat {
    if embedded { (completed_u - 1) as nat } else { completed_u }
}

pub open spec fn ht_magnitude_sign(bit: nat, decoded: bool) -> bool {
    bit == 0 && decoded
}

pub open spec fn ht_bin_center_odd(value: nat) -> nat {
    if value % 2 == 0 { value + 1 } else { value }
}

pub open spec fn ht_claim_available(already_claimed: bool) -> bool {
    !already_claimed
}

pub open spec fn ht_stripe_rows(height: nat, stripe_y: nat) -> nat
    recommends
        stripe_y <= height,
{
    if height - stripe_y < 4 { (height - stripe_y) as nat } else { 4 }
}

pub open spec fn ht_patch_end(patch_x: nat, width: nat) -> nat {
    if patch_x + 4 < width { patch_x + 4 } else { width }
}

pub open spec fn ht_sigprop_eligible(significant: bool, neighbors: nat) -> bool {
    !significant && neighbors > 0
}

pub open spec fn ht_magref_eligible(significant_at_cleanup: bool) -> bool {
    significant_at_cleanup
}

pub open spec fn ht_refinement_pass_total(existing: nat, additional: nat) -> nat {
    existing + additional
}

pub open spec fn ht_multiple_set_allowed(sets_seen: nat, multiple_allowed: bool) -> bool {
    sets_seen == 0 || multiple_allowed
}

pub open spec fn ht_replacement_allowed(
    phase: nat,
    has_cleanup: bool,
    empty_segment: bool,
) -> bool {
    (phase == 0 && empty_segment) || (phase != 0 && has_cleanup)
}

pub open spec fn codestream_count_within_limit(codestreams: nat, limit: nat) -> bool {
    codestreams > 0 && codestreams <= limit
}

pub open spec fn valid_layer_codestream_reference(index: nat, codestreams: nat) -> bool {
    codestreams > 0 && index < codestreams
}

pub open spec fn effective_jpx_layer_count(explicit_layers: nat, codestreams: nat) -> nat {
    if explicit_layers == 0 { codestreams } else { explicit_layers }
}

pub open spec fn valid_jp2_primary_brand(primary_is_jp2: bool, jpx_compatible: bool) -> bool {
    !primary_is_jp2 || !jpx_compatible
}

pub open spec fn valid_jpip_codestream_layout(entries: nat) -> bool {
    entries >= 3 && entries % 2 == 1
}

pub open spec fn tile_transform_scope(start: nat, components: nat, total: nat) -> bool {
    start <= total && components <= total - start
}

pub open spec fn clipped_composition_extent(offset: nat, extent: nat, canvas: nat) -> nat {
    if offset >= canvas {
        0
    } else if extent <= canvas - offset {
        extent
    } else {
        (canvas - offset) as nat
    }
}

pub open spec fn composition_output_bytes(pixels: nat, color_channels: nat) -> nat {
    pixels * (color_channels + 1) * 2
}

pub open spec fn registered_sample_numerator(
    samples: nat,
    subsampling: nat,
    offset: nat,
) -> nat {
    samples * subsampling + offset
}

pub open spec fn composition_instruction_work(frames: nat, layers: nat) -> nat {
    frames * if layers == 0 { 1 } else { layers }
}

pub open spec fn registered_channel_count(
    prior_channels: nat,
    codestream_channels: nat,
) -> nat {
    prior_channels + codestream_channels
}

pub open spec fn nested_metadata_work(boxes: nat, depth: nat) -> nat {
    boxes + depth
}

pub open spec fn bounded_loop_frame(index: nat, frames: nat) -> nat
    recommends frames > 0
{
    index % frames
}

pub open spec fn valid_mct_array_reference(index: nat, arrays: nat) -> bool {
    index > 0 && index <= arrays
}

pub open spec fn valid_mct_dimensions(
    components: nat,
    matrix_elements: nat,
    offset_elements: nat,
) -> bool {
    components > 0
        && matrix_elements == components * components
        && (offset_elements == 0 || offset_elements == components)
}

pub open spec fn mct_work_units(pixels: nat, components: nat) -> nat {
    pixels * components * (components + 1)
}

pub open spec fn valid_mct_collection_count(collections: nat, stages: nat) -> bool {
    collections > 0 && collections <= 64 && stages > 0 && stages <= 255
}

pub open spec fn valid_mct_stage_edge(parent: nat, child: nat, stages: nat) -> bool {
    parent < child && child < stages
}

pub open spec fn valid_mct_matrix_bounds(
    rows: nat,
    columns: nat,
    elements: nat,
    component_limit: nat,
) -> bool {
    rows > 0
        && rows == columns
        && rows <= component_limit
        && elements == rows * columns
}

pub open spec fn valid_mct_fragment_index(index: nat, expected: nat, final_index: nat) -> bool {
    index == expected && index <= final_index
}

pub open spec fn complete_mct_fragment_series(next_index: nat, final_index: nat) -> bool {
    next_index == final_index + 1
}

pub open spec fn mct_dependency_coefficients(components: nat, reversible: bool) -> nat {
    if components == 0 {
        0
    } else if reversible {
        ((components * (components + 1) / 2 - 1) as nat)
    } else {
        ((components * (components - 1) / 2) as nat)
    }
}

pub open spec fn valid_mct_wavelet_partition(
    low_components: nat,
    high_components: nat,
    output_components: nat,
) -> bool {
    low_components + high_components == output_components
}

pub open spec fn mct_wavelet_work_units(
    pixels: nat,
    components: nat,
    levels: nat,
) -> nat {
    pixels * components * (levels + 1) * 8
}

pub open spec fn valid_maxshift_roi(component: nat, components: nat, style: nat, shift: nat) -> bool {
    component < components && style == 0 && shift <= 255
}

pub open spec fn valid_magnitude_bit(bit: nat) -> bool {
    bit < 384
}

pub open spec fn magnitude_limb_index(bit: nat) -> nat {
    bit / 64
}

pub open spec fn magnitude_limb_offset(bit: nat) -> nat {
    bit % 64
}

pub open spec fn magnitude_shift_fits(highest_bit: nat, shift: nat) -> bool {
    highest_bit < 384 && shift < 384 && highest_bit + shift < 384
}

pub open spec fn magnitude_less(left: nat, right: nat) -> bool {
    left < right
}

pub open spec fn magnitude_addition(left: nat, right: nat) -> nat {
    left + right
}

pub open spec fn maxshift_restored_magnitude(magnitude: nat, threshold: nat) -> nat
    recommends
        threshold > 0,
{
    if magnitude < threshold {
        magnitude
    } else {
        magnitude / threshold
    }
}

pub open spec fn component_index_width(components: nat) -> nat {
    if components < 257 { 1 } else { 2 }
}

pub open spec fn valid_component_override_index(component: nat, components: nat) -> bool {
    components > 0 && component < components
}

pub open spec fn palette_linear_index(entry: nat, column: nat, columns: nat) -> nat {
    entry * columns + column
}

pub open spec fn valid_palette_lookup(entry: nat, entries: nat, column: nat, columns: nat) -> bool {
    entries > 0
        && columns > 0
        && entry < entries
        && column < columns
        && palette_linear_index(entry, column, columns) < entries * columns
}

pub open spec fn valid_component_mapping(
    component: nat,
    components: nat,
    mapping_type: nat,
    palette_column: nat,
    palette_columns: nat,
) -> bool {
    component < components
        && ((mapping_type == 0 && palette_column == 0)
            || (mapping_type == 1 && palette_column < palette_columns))
}

pub open spec fn valid_channel_association(association: nat, output_channels: nat) -> bool {
    association <= output_channels || association == 0xffff
}

pub open spec fn valid_color_sample(value: nat, maximum: nat) -> bool {
    maximum > 0 && value <= maximum
}

pub open spec fn color_output_bytes(pixels: nat, channels: nat) -> nat {
    pixels * channels * 2
}

pub open spec fn clamp_color_sample(value: int) -> int {
    if value < 0 {
        0
    } else if value > 65535 {
        65535
    } else {
        value
    }
}

pub open spec fn valid_signed_color_sample(value: int, level: nat) -> bool {
    level > 0 && -(level as int) <= value && value < level
}

pub open spec fn signed_color_to_nominal(value: int, level: nat) -> int {
    value + level as int
}

pub open spec fn resampled_component_coordinate(
    grid_coordinate: nat,
    sampling_step: nat,
    first_component_coordinate: nat,
    component_count: nat,
) -> nat
    recommends
        sampling_step > 0,
        component_count > 0,
{
    let last = (first_component_coordinate + component_count - 1) as nat;
    let nearest = grid_coordinate / sampling_step;
    if nearest < first_component_coordinate {
        first_component_coordinate
    } else if nearest > last {
        last
    } else {
        nearest
    }
}

pub open spec fn packet_length_step(prefix: nat, group: nat) -> nat {
    prefix * 128 + group
}

pub open spec fn packet_length_index_continuous(expected: nat, actual: nat) -> bool {
    expected == actual && actual < 256
}

pub open spec fn packet_lengths_cover_payload(declared: nat, payload: nat) -> bool {
    declared == payload
}

pub open spec fn part9_header_form_valid(form: nat) -> bool {
    1 <= form && form <= 3
}

pub open spec fn part9_header_has_explicit_class(form: nat) -> bool {
    form >= 2
}

pub open spec fn part9_header_has_explicit_codestream(form: nat) -> bool {
    form == 3
}

pub open spec fn part9_vbas_first_is_canonical(payload: nat, continued: bool) -> bool {
    !continued || payload > 0
}

pub open spec fn part9_vbas_step(value: nat, group: nat) -> nat {
    value * 128 + group
}

pub open spec fn part9_auxiliary_present(class_identifier: nat) -> bool {
    class_identifier % 2 == 1
}

pub open spec fn part9_eor_extent(header_length: nat, body_length: nat) -> nat {
    header_length + body_length
}

pub open spec fn part9_class_identifier_valid(class_identifier: nat) -> bool {
    class_identifier <= 8
}

pub open spec fn part9_databin_identity(
    first_class: nat,
    first_codestream: nat,
    first_in_class: nat,
    second_class: nat,
    second_codestream: nat,
    second_in_class: nat,
) -> bool {
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
}

pub open spec fn valid_jpip_fragment(offset: nat, length: nat, limit: nat) -> bool {
    offset <= limit && length <= limit - offset
}

pub open spec fn consistent_jpip_final_length(existing: nat, announced: nat) -> bool {
    existing == announced
}

pub open spec fn jpip_ranges_overlap(
    first_offset: nat,
    first_length: nat,
    second_offset: nat,
    second_length: nat,
) -> bool {
    first_offset < second_offset + second_length
        && second_offset < first_offset + first_length
}

pub open spec fn part9_overlap_consistent(bytes_equal: bool) -> bool {
    bytes_equal
}

pub open spec fn part9_transaction_commit(validation_complete: bool) -> bool {
    validation_complete
}

pub open spec fn part9_merge_start(first: nat, second: nat) -> nat {
    if first < second { first } else { second }
}

pub open spec fn part9_merge_end(first: nat, second: nat) -> nat {
    if first > second { first } else { second }
}

pub open spec fn complete_jpip_databin(contiguous_prefix: nat, final_length: nat) -> bool {
    contiguous_prefix == final_length
}

pub open spec fn complete_jpip_range(available: nat, required: nat) -> bool {
    available == required
}

pub open spec fn part9_missing_length(required_end: nat, available_end: nat) -> nat
    recommends
        available_end <= required_end,
{
    (required_end - available_end) as nat
}

pub open spec fn part9_available_bytes(required: nat, missing: nat) -> nat
    recommends
        missing <= required,
{
    (required - missing) as nat
}

pub open spec fn valid_jpip_progressive_update(
    previous_layers: nat,
    available_layers: nat,
    planned_layers: nat,
) -> bool {
    previous_layers <= available_layers && available_layers <= planned_layers
}

pub open spec fn bounded_jpip_transcode(
    main_header: nat,
    tile_header: nat,
    packet_bytes: nat,
    limit: nat,
) -> bool {
    main_header + tile_header + packet_bytes + 2 <= limit
}

pub open spec fn jpip_precinct_in_class(
    tile: nat,
    tile_count: nat,
    component: nat,
    components: nat,
    precinct_index: nat,
) -> nat {
    tile + tile_count * (component + components * precinct_index)
}

pub open spec fn valid_jpip_precinct_coordinate(
    tile: nat,
    tile_count: nat,
    component: nat,
    components: nat,
) -> bool {
    tile < tile_count && component < components && tile_count > 0 && components > 0
}

pub open spec fn jpip_quality_prefix_available(available: nat, requested: nat) -> bool {
    requested <= available
}

pub open spec fn jpip_reconstructed_box_length(header: nat, payload: nat) -> nat {
    header + payload
}

pub open spec fn part9_metadata_descent_allowed(
    depth: nat,
    maximum_depth: nat,
    cycle: bool,
) -> bool {
    depth <= maximum_depth && !cycle
}

pub open spec fn part9_reconstruction_length(
    main_header: nat,
    tile_headers: nat,
    packet_data: nat,
    eoc: nat,
) -> nat {
    main_header + tile_headers + packet_data + eoc
}

pub open spec fn part9_precinct_flat_identity(
    tile: nat,
    tile_count: nat,
    component_precinct: nat,
) -> nat {
    tile + component_precinct * tile_count
}

pub open spec fn part9_reframed_marker_removed(marker: nat, tile_header: bool) -> bool {
    if tile_header {
        marker == 0xff58 || marker == 0xff61
    } else {
        marker == 0xff55 || marker == 0xff57 || marker == 0xff60
    }
}

pub open spec fn part9_progression_supported(order: nat) -> bool {
    order <= 4
}

pub open spec fn part9_packet_reframed_length(payload: nat, sop: bool, eph: bool) -> nat {
    payload + (if sop { 6 as nat } else { 0 as nat })
        + (if eph { 2 as nat } else { 0 as nat })
}

pub open spec fn jpip_tile_transition_sufficient(
    later_functional: bool,
    preceding_packet_count_known: bool,
    preceding_exact_consumption_known: bool,
) -> bool {
    !later_functional || preceding_packet_count_known || preceding_exact_consumption_known
}

pub open spec fn jpip_packet_boundary_step(
    consumed: nat,
    packet_length: nat,
    tile_part_length: nat,
) -> bool {
    consumed + packet_length <= tile_part_length
}

pub open spec fn jpip_packet_boundary_exact(consumed: nat, tile_part_length: nat) -> bool {
    consumed == tile_part_length
}

pub open spec fn packed_header_index_continuous(expected: nat, actual: nat) -> bool {
    expected == actual && actual < 256
}

pub open spec fn ppm_frame_fits(total: nat, position: nat, length: nat) -> bool {
    position <= total && length <= total - position
}

pub open spec fn packed_packet_length_matches(
    declared: nat,
    tile_body: nat,
    packed_header: nat,
) -> bool {
    declared == tile_body + packed_header
}

pub open spec fn valid_poc_record(
    resolution_start: nat,
    resolution_end: nat,
    component_start: nat,
    component_end: nat,
    layer_end: nat,
    components: nat,
    layers: nat,
    resolutions: nat,
    order: nat,
) -> bool {
    resolution_start < resolution_end
        && resolution_end <= resolutions
        && component_start < component_end
        && component_end <= components
        && 0 < layer_end
        && layer_end <= layers
        && order < 5
}

pub open spec fn poc_contains(
    layer: nat,
    resolution: nat,
    component: nat,
    resolution_start: nat,
    resolution_end: nat,
    component_start: nat,
    component_end: nat,
    layer_end: nat,
) -> bool {
    layer < layer_end
        && resolution_start <= resolution
        && resolution < resolution_end
        && component_start <= component
        && component < component_end
}

pub open spec fn effective_component_override(
    main_default: int,
    main_component: int,
    has_main_component: bool,
    tile_default: int,
    has_tile_default: bool,
    tile_component: int,
    has_tile_component: bool,
) -> int {
    if has_tile_component {
        tile_component
    } else if has_tile_default {
        tile_default
    } else if has_main_component {
        main_component
    } else {
        main_default
    }
}

pub open spec fn decomposition_stage_subbands(direction: nat) -> nat {
    if direction == 0 {
        0
    } else if direction == 1 {
        3
    } else {
        1
    }
}

pub open spec fn valid_decomposition_direction(direction: nat) -> bool {
    direction < 4
}

pub open spec fn decomposition_graph_cardinality(
    no_dwt_stages: nat,
    one_axis_stages: nat,
    bidirectional_stages: nat,
) -> nat {
    1 + one_axis_stages + 3 * bidirectional_stages
}

pub open spec fn decomposition_axis_scale(horizontal_splits: nat) -> nat {
    pow2(horizontal_splits)
}

pub open spec fn valid_decomposition_dimensions(
    parent_width: nat,
    parent_height: nat,
    low_width: nat,
    low_height: nat,
    high_width: nat,
    high_height: nat,
    split_horizontal: bool,
    split_vertical: bool,
) -> bool {
    (!split_horizontal
        || (low_width + high_width == parent_width && low_height == parent_height))
        && (!split_vertical
            || (low_height + high_height == parent_height && low_width == parent_width))
}

pub open spec fn inverse_stage_order(stage: nat, stages: nat) -> nat
    recommends
        stage < stages,
{
    (stages - stage - 1) as nat
}

pub open spec fn packed_two_bit_bytes(elements: nat) -> nat {
    (elements + 3) / 4
}

pub open spec fn ads_payload_bytes(depth_elements: nat, split_elements: nat) -> nat {
    3 + packed_two_bit_bytes(depth_elements) + packed_two_bit_bytes(split_elements)
}

pub open spec fn atk_reconstruction_step(step: nat, steps: nat) -> nat
    recommends
        step < steps,
{
    (steps - step - 1) as nat
}

pub open spec fn atk_update_parity(initial_odd: bool, reverse_step: nat) -> bool {
    if reverse_step % 2 == 0 {
        initial_odd
    } else {
        !initial_odd
    }
}

pub open spec fn valid_cbd_scope(in_main_header: bool) -> bool {
    in_main_header
}

// Selected Part 2 and JPX functional semantics used by executable production contracts.
pub open spec fn part2_tlm_tile_index_bytes(style: u8) -> usize {
    (((style >> 4) & 3) as int) as usize
}

pub open spec fn part2_tlm_length_bytes(style: u8) -> usize {
    if style & 0x40 == 0 { 2 } else { 4 }
}

pub open spec fn part2_crg_payload_bytes(components: u16) -> usize {
    ((components as int) * 4) as usize
}

pub open spec fn part2_packed_two_bit_bytes(elements: usize) -> usize {
    ((elements + 3) / 4) as usize
}

pub open spec fn part2_dfs_direction_code(byte: u8, slot: u8) -> u8
    recommends slot < 4,
{
    (byte >> (6 - 2 * slot)) & 3
}

pub open spec fn part2_cbd_precision(encoded: u8) -> u8 {
    (((encoded & 0x7f) as int) + 1) as u8
}

pub open spec fn part2_cbd_signed(encoded: u8) -> bool {
    encoded & 0x80 != 0
}

pub open spec fn part2_nlt_binary_complement(source: i128, half: i128) -> i128 {
    if source < 0 {
        (-(source as int) - ((half as int) + 1)) as i128
    } else {
        source
    }
}

pub open spec fn part2_atk_reconstruction_step(remaining: usize) -> usize
    recommends remaining > 0,
{
    (remaining - 1) as usize
}

pub open spec fn part2_atk_next_parity(current: bool) -> bool {
    !current
}

pub open spec fn part2_fragment_next(index: u16) -> u16
    recommends index < u16::MAX,
{
    ((index as int) + 1) as u16
}

pub open spec fn part2_fragment_complete(next: u16, final_index: u16) -> bool {
    next as int == final_index as int + 1
}

pub open spec fn part2_mct_reverse_stage(remaining: usize) -> usize
    recommends remaining > 0,
{
    (remaining - 1) as usize
}

pub open spec fn part2_binary_fixed_shift(binary_exponent: i32) -> i32
    recommends
        i32::MIN + 32 <= binary_exponent,
        binary_exponent <= i32::MAX - 32,
{
    ((binary_exponent as int) + 32) as i32
}

pub open spec fn part2_palette_coordinates_valid(
    entry: u16,
    entries: u16,
    column: u8,
    columns: u8,
) -> bool {
    entry < entries && column < columns
}

pub type Part2PaletteModel = (u16, Seq<crate::jp2::PaletteColumn>, Seq<i64>);

pub open spec fn part2_palette_value(
    palette: Part2PaletteModel, entry: u16, column: u8,
) -> Result<i64, crate::DecodeError> {
    if palette.1.len() > u8::MAX {
        Err(part2_composition_error(true))
    } else if !part2_palette_coordinates_valid(
        entry, palette.0, column, palette.1.len() as u8,
    ) {
        Err(part2_composition_error(false))
    } else {
        let index = (entry as nat) * palette.1.len() + (column as nat);
        if index > usize::MAX {
            Err(part2_composition_error(true))
        } else if index >= palette.2.len() {
            Err(part2_composition_error(false))
        } else { Ok(palette.2[index as int]) }
    }
}

pub open spec fn part2_component_mapping_valid(
    component: u16,
    components: u16,
    mapping_type: u8,
    palette_column: u8,
    palette_columns: u8,
) -> bool {
    component < components
        && ((mapping_type == 0 && palette_column == 0)
            || (mapping_type == 1
                && palette_columns > 0
                && palette_column < palette_columns))
}

pub open spec fn part2_channel_definition_valid(
    channel: u16,
    channel_type: u16,
    association: u16,
    output_channels: u16,
) -> bool {
    channel < output_channels
        && (channel_type == 0
            || channel_type == 1
            || channel_type == 2
            || channel_type == 0xffff)
        && (association <= output_channels || association == 0xffff)
}

pub open spec fn part2_first_override_allowed(already_overridden: bool) -> bool {
    !already_overridden
}

pub open spec fn part2_signed_to_nominal(value: i64, level: i64) -> i64 {
    ((value as int) + (level as int)) as i64
}

pub open spec fn part2_effective_layer_count(
    explicit_layers: usize,
    codestreams: usize,
) -> usize {
    if explicit_layers == 0 { codestreams } else { explicit_layers }
}

pub open spec fn part2_registered_extent(numerator: u64, grid: u64) -> u64
    recommends grid > 0,
{
    if numerator == 0 {
        0
    } else {
        (1 + ((numerator - 1) as int) / (grid as int)) as u64
    }
}

pub open spec fn part2_registered_source_coordinate(
    numerator: u32,
    offset: u32,
    subsampling: u32,
) -> u32
    recommends
        subsampling > 0,
        offset <= numerator,
{
    (((numerator as int) - (offset as int)) / (subsampling as int)) as u32
}

pub open spec fn part2_composition_clip(offset: u32, extent: u32, canvas: u32) -> u32 {
    if offset >= canvas {
        0
    } else if extent <= canvas - offset {
        extent
    } else {
        ((canvas as int) - (offset as int)) as u32
    }
}

pub open spec fn part2_composition_mapped_offset(position: u32, source_extent: u32, target_extent: u32) -> nat
    recommends target_extent > 0,
{
    (position as nat) * (source_extent as nat) / (target_extent as nat)
}

pub open spec fn part2_row_major_index(width: u32, x: u32, y: u32) -> nat {
    (y as nat) * (width as nat) + x as nat
}

pub open spec fn part2_composition_pixel_index(width: u32, x: u32, y: u32) -> Option<usize> {
    let index = part2_row_major_index(width, x, y);
    if index <= usize::MAX as nat { Some(index as usize) } else { None }
}

pub open spec fn part2_loop_frame(index: usize, frames: usize) -> usize
    recommends frames > 0,
{
    (index % frames) as usize
}

pub open spec fn part2_inverse_alpha(source_alpha: u16) -> u16 {
    ((PART2_NORMALIZED_SAMPLE_MAX as int) - source_alpha as int) as u16
}

pub type Part2LayerChannel = (crate::color::ConvertedChannelType, Seq<u16>);
pub type Part2LayerAlpha = (Seq<u16>, bool);
pub type Part2LayerInputResult = Result<(u32, u32, Seq<Seq<u16>>, Option<Part2LayerAlpha>), crate::DecodeError>;
pub type Part2OwnedLayerResult = Result<(u32, u32, crate::color::ConvertedColorSpace, Seq<Seq<u16>>, Option<Part2LayerAlpha>), crate::DecodeError>;
pub type Part2OwnedLayer = (u32, u32, crate::color::ConvertedColorSpace, Seq<Seq<u16>>, Option<Part2LayerAlpha>);
pub type Part2ConvertedImage = (u32, u32, crate::color::ConvertedColorSpace, u8, Seq<Part2LayerChannel>);
pub type Part2ConvertedImageResult = Result<Part2ConvertedImage, crate::DecodeError>;
pub type Part2Instruction = (usize, u32, u32, u32, u32, u32, u32, u32, u32, bool, u32);
pub type Part2Placement = (Part2OwnedLayer, Part2Instruction);
pub type Part2Frame = (usize, u64, u32, bool, Seq<Part2Instruction>);
pub type Part2TimelineInstruction = (Part2Instruction, u32);
pub type Part2TimelineSet = (u16, Seq<Part2TimelineInstruction>);
pub type Part2TimelineState = (Seq<Part2Frame>, Seq<Option<usize>>, usize, usize, u64);
pub type Part2TimelineExecution = Result<(bool, Part2TimelineState), crate::DecodeError>;
pub type Part2SuppliedLayer = (usize, Option<Part2OwnedLayerResult>);
pub type Part2GatheredLayers = (Result<(), crate::DecodeError>, Seq<Part2Placement>, Seq<Part2SuppliedLayer>);
pub type Part2RenderState = (Result<(), crate::DecodeError>, Seq<Seq<u16>>, Seq<u16>, nat, nat);
pub type Part2LayerScanResult = Result<(Seq<Option<Seq<u16>>>, Seq<Part2LayerAlpha>), crate::DecodeError>;

pub open spec fn part2_color_input_error() -> crate::DecodeError {
    crate::DecodeError::MalformedInput { offset: 0, kind: crate::MalformedKind::ContainerCodestreamMismatch }
}

pub open spec fn part2_output_channel(space: crate::color::ConvertedColorSpace, index: int)
    -> crate::color::ConvertedChannelType
{
    match space {
        crate::color::ConvertedColorSpace::Grayscale => crate::color::ConvertedChannelType::Gray,
        crate::color::ConvertedColorSpace::Srgb => if index == 0 { crate::color::ConvertedChannelType::Red }
            else if index == 1 { crate::color::ConvertedChannelType::Green }
            else { crate::color::ConvertedChannelType::Blue },
    }
}

pub open spec fn part2_image_from_planes(
    width: u32, height: u32, space: crate::color::ConvertedColorSpace,
    colors: Seq<Seq<u16>>, alpha: Seq<u16>, premultiplied: bool,
) -> Part2ConvertedImageResult {
    match part2_layer_area(width, height) {
        Err(error) => Err(error),
        Ok(area) => if colors.len() != part2_layer_color_count(space) || alpha.len() != area
            || exists|i: int| 0 <= i < colors.len() && colors[i].len() != area {
            Err(part2_color_input_error())
        } else {
            Ok((width, height, space, 16,
                Seq::new(colors.len(), |i: int| (part2_output_channel(space, i), colors[i]))
                    .push((crate::color::ConvertedChannelType::Alpha {
                        premultiplied, association: crate::image::ChannelAssociation::WholeImage,
                    }, alpha))))
        },
    }
}

#[verifier::opaque]
pub open spec fn part2_layer_area(width: u32, height: u32) -> Result<usize, crate::DecodeError> {
    if (width as nat) * (height as nat) <= usize::MAX as nat {
        Ok(((width as nat) * (height as nat)) as usize)
    } else {
        Err(part2_composition_error(true))
    }
}

pub open spec fn part2_layer_color_count(space: crate::color::ConvertedColorSpace) -> nat {
    match space {
        crate::color::ConvertedColorSpace::Srgb => 3,
        crate::color::ConvertedColorSpace::Grayscale => 1,
    }
}

pub open spec fn part2_layer_channel_index(kind: crate::color::ConvertedChannelType) -> Option<nat> {
    match kind {
        crate::color::ConvertedChannelType::Red | crate::color::ConvertedChannelType::Gray => Some(0),
        crate::color::ConvertedChannelType::Green => Some(1),
        crate::color::ConvertedChannelType::Blue => Some(2),
        crate::color::ConvertedChannelType::Alpha { .. } => None,
    }
}

pub open spec fn part2_layer_channel_scan_step(
    colors: Seq<Option<Seq<u16>>>, alphas: Seq<Part2LayerAlpha>, channel: Part2LayerChannel,
) -> Part2LayerScanResult {
    match channel.0 {
        crate::color::ConvertedChannelType::Alpha { premultiplied, .. } =>
            Ok((colors, alphas.push((channel.1, premultiplied)))),
        _ => {
            let slot = part2_layer_channel_index(channel.0).unwrap();
            if slot >= colors.len() || colors[slot as int].is_some() {
                Err(part2_composition_error(false))
            } else {
                Ok((colors.update(slot as int, Some(channel.1)), alphas))
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn part2_layer_channel_scan(
    channels: Seq<Part2LayerChannel>, color_count: nat, count: nat,
) -> Part2LayerScanResult
    recommends count <= channels.len(),
    decreases count,
{
    if count == 0 {
        Ok((Seq::new(color_count, |i: int| None), Seq::empty()))
    } else {
        match part2_layer_channel_scan(channels, color_count, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok((colors, alphas)) => part2_layer_channel_scan_step(colors, alphas, channels[(count - 1) as int]),
        }
    }
}

pub open spec fn part2_layer_resolve_colors(colors: Seq<Option<Seq<u16>>>, area: nat)
    -> Result<Seq<Seq<u16>>, crate::DecodeError>
{
    if exists |i: int| #![trigger colors[i]] 0 <= i < colors.len() && (colors[i].is_none() || colors[i].unwrap().len() != area) {
        Err(part2_composition_error(false))
    } else {
        Ok(Seq::new(colors.len(), |i: int| colors[i].unwrap()))
    }
}

pub open spec fn part2_normalized_sample_product(first: u16, second: u16) -> nat {
    ((first as nat) * (second as nat) + PART2_NORMALIZED_SAMPLE_HALF as nat)
        / PART2_NORMALIZED_SAMPLE_MAX as nat
}

pub open spec fn part2_layer_alpha_merge(
    previous: Option<Part2LayerAlpha>, next: Part2LayerAlpha, area: nat,
) -> Result<Option<Part2LayerAlpha>, crate::DecodeError> {
    if next.0.len() != area {
        Err(part2_composition_error(false))
    } else {
        match previous {
            None => Ok(Some(next)),
            Some((samples, _)) => Ok(Some((
                Seq::new(area, |i: int| part2_normalized_sample_product(samples[i], next.0[i]) as u16),
                false,
            ))),
        }
    }
}

#[verifier::opaque]
pub open spec fn part2_layer_alpha_fold(alphas: Seq<Part2LayerAlpha>, area: nat, count: nat)
    -> Result<Option<Part2LayerAlpha>, crate::DecodeError>
    recommends count <= alphas.len(),
    decreases count,
{
    if count == 0 {
        Ok(None)
    } else {
        match part2_layer_alpha_fold(alphas, area, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(previous) => part2_layer_alpha_merge(previous, alphas[(count - 1) as int], area),
        }
    }
}

pub open spec fn part2_layer_input(
    width: u32, height: u32, space: crate::color::ConvertedColorSpace, channels: Seq<Part2LayerChannel>,
) -> Part2LayerInputResult {
    match part2_layer_channel_scan(channels, part2_layer_color_count(space), channels.len()) {
        Err(error) => Err(error),
        Ok((colors, alphas)) => {
            let area = (width as nat) * (height as nat);
            if area > usize::MAX as nat {
                Err(part2_composition_error(true))
            } else {
                match part2_layer_resolve_colors(colors, area) {
                    Err(error) => Err(error),
                    Ok(resolved) => match part2_layer_alpha_fold(alphas, area, alphas.len()) {
                        Err(error) => Err(error),
                        Ok(alpha) => Ok((width, height, resolved, alpha)),
                    },
                }
            }
        },
    }
}

pub open spec fn part2_owned_layer_input(
    width: u32, height: u32, space: crate::color::ConvertedColorSpace, channels: Seq<Part2LayerChannel>,
) -> Part2OwnedLayerResult {
    match part2_layer_input(width, height, space, channels) {
        Err(error) => Err(error),
        Ok((width, height, colors, alpha)) => Ok((width, height, space, colors, alpha)),
    }
}

pub open spec fn part2_source_over_alpha_numerator(source_alpha: u16, destination_alpha: u16) -> nat {
    let maximum = PART2_NORMALIZED_SAMPLE_MAX as nat;
    let inverse = ((maximum as int) - source_alpha as int) as nat;
    (source_alpha as nat) * maximum + (destination_alpha as nat) * inverse
}

pub open spec fn part2_source_over_color_numerator(
    source: u16, destination: u16, source_alpha: u16, destination_alpha: u16, premultiplied: bool,
) -> nat {
    let maximum = PART2_NORMALIZED_SAMPLE_MAX as nat;
    let source_term = if premultiplied {
        (source as nat) * maximum * maximum
    } else {
        (source as nat) * (source_alpha as nat) * maximum
    };
    let inverse = ((maximum as int) - source_alpha as int) as nat;
    source_term + (destination as nat) * (destination_alpha as nat) * inverse
}

pub open spec fn part2_source_over_rounded_ratio(numerator: u64, denominator: u64) -> nat {
    if denominator == 0 { 0 } else {
        ((numerator as nat) + (denominator as nat) / 2) / (denominator as nat)
    }
}

pub open spec fn part2_source_over_color_sample(
    source: u16, destination: u16, source_alpha: u16, destination_alpha: u16, premultiplied: bool,
) -> nat {
    let alpha = part2_source_over_alpha_numerator(source_alpha, destination_alpha);
    if alpha == 0 { 0 } else {
        (part2_source_over_color_numerator(
            source, destination, source_alpha, destination_alpha, premultiplied,
        ) + alpha / 2) / alpha
    }
}

pub open spec fn part2_source_over_alpha_sample(source_alpha: u16, destination_alpha: u16) -> nat {
    (part2_source_over_alpha_numerator(source_alpha, destination_alpha)
        + PART2_NORMALIZED_SAMPLE_HALF as nat) / PART2_NORMALIZED_SAMPLE_MAX as nat
}

pub open spec fn part2_composition_store(samples: Seq<u16>, index: nat, value: u16) -> (bool, Seq<u16>) {
    if index < samples.len() { (true, samples.update(index as int, value)) } else { (false, samples) }
}

pub open spec fn part2_composition_gather_channel(
    source: Seq<Seq<u16>>, destination: Seq<Seq<u16>>,
    source_index: nat, destination_index: nat, expand_grayscale: bool, channel: nat,
) -> Option<(u16, u16)> {
    let source_channel = if expand_grayscale { 0nat } else { channel };
    if source_channel >= source.len() || channel >= destination.len() {
        None
    } else if source_index >= source[source_channel as int].len()
        || destination_index >= destination[channel as int].len() {
        None
    } else {
        Some((source[source_channel as int][source_index as int],
            destination[channel as int][destination_index as int]))
    }
}

pub open spec fn part2_composition_gather_pixel(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    source_index: nat, destination_index: nat, expand_grayscale: bool,
) -> Option<([u16; 3], [u16; 3], u16, u16, bool)> {
    let channels = destination.len();
    if channels > 3 || destination_index >= destination_alpha.len()
        || (source_alpha.is_some() && source_index >= source_alpha.unwrap().0.len()) {
        None
    } else {
        let first = part2_composition_gather_channel(source, destination, source_index, destination_index, expand_grayscale, 0);
        let second = part2_composition_gather_channel(source, destination, source_index, destination_index, expand_grayscale, 1);
        let third = part2_composition_gather_channel(source, destination, source_index, destination_index, expand_grayscale, 2);
        if (channels > 0 && first.is_none()) || (channels > 1 && second.is_none())
            || (channels > 2 && third.is_none()) {
            None
        } else {
            Some(([
                if channels > 0 { first.unwrap().0 } else { 0 },
                if channels > 1 { second.unwrap().0 } else { 0 },
                if channels > 2 { third.unwrap().0 } else { 0 },
            ], [
                if channels > 0 { first.unwrap().1 } else { 0 },
                if channels > 1 { second.unwrap().1 } else { 0 },
                if channels > 2 { third.unwrap().1 } else { 0 },
            ], if source_alpha.is_some() { source_alpha.unwrap().0[source_index as int] } else { PART2_NORMALIZED_SAMPLE_MAX },
                destination_alpha[destination_index as int],
                source_alpha.is_some() && source_alpha.unwrap().1))
        }
    }
}

pub open spec fn part2_source_over_pixel_valid(source: [u16; 3], source_alpha: u16, premultiplied: bool, channels: usize) -> bool {
    channels <= 3 && (!premultiplied
        || ((channels == 0 || source[0] <= source_alpha)
            && (channels <= 1 || source[1] <= source_alpha)
            && (channels <= 2 || source[2] <= source_alpha)))
}

pub open spec fn part2_composition_planes(colors: Seq<Vec<u16>>) -> Seq<Seq<u16>> {
    Seq::new(colors.len(), |channel: int| colors[channel]@)
}

pub open spec fn part2_composition_scatter_count(colors: Seq<Seq<u16>>, index: nat) -> nat {
    if colors.len() > 3 || (colors.len() > 0 && index >= colors[0].len()) {
        0
    } else if colors.len() > 1 && index >= colors[1].len() {
        1
    } else if colors.len() > 2 && index >= colors[2].len() {
        2
    } else {
        colors.len()
    }
}

pub open spec fn part2_composition_color_prefix(
    colors: Seq<Seq<u16>>, index: nat, pixel: [u16; 3], count: nat,
) -> Seq<Seq<u16>> {
    Seq::new(colors.len(), |channel: int|
        if channel < count && channel < 3 && index < colors[channel].len() {
            colors[channel].update(index as int, pixel[channel])
        } else {
            colors[channel]
        })
}

pub open spec fn part2_composition_scatter_pixel(
    colors: Seq<Seq<u16>>, alpha: Seq<u16>, index: nat, pixel: [u16; 3], value: u16,
) -> (bool, Seq<Seq<u16>>, Seq<u16>) {
    let count = part2_composition_scatter_count(colors, index);
    let stored = colors.len() <= 3 && count == colors.len() && index < alpha.len();
    (stored, part2_composition_color_prefix(colors, index, pixel, count),
        if stored { alpha.update(index as int, value) } else { alpha })
}

pub open spec fn part2_source_over_pixel(
    source: [u16; 3], destination: [u16; 3], source_alpha: u16, destination_alpha: u16,
    premultiplied: bool, channels: usize,
) -> Option<([u16; 3], u16)> {
    if !part2_source_over_pixel_valid(source, source_alpha, premultiplied, channels) {
        None
    } else {
        Some(([
            if channels > 0 { part2_source_over_color_sample(source[0], destination[0], source_alpha, destination_alpha, premultiplied) as u16 } else { destination[0] },
            if channels > 1 { part2_source_over_color_sample(source[1], destination[1], source_alpha, destination_alpha, premultiplied) as u16 } else { destination[1] },
            if channels > 2 { part2_source_over_color_sample(source[2], destination[2], source_alpha, destination_alpha, premultiplied) as u16 } else { destination[2] },
        ], part2_source_over_alpha_sample(source_alpha, destination_alpha) as u16))
    }
}

pub open spec fn part2_composition_pixel_transition(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    source_index: nat, destination_index: nat, expand_grayscale: bool,
) -> (bool, Seq<Seq<u16>>, Seq<u16>) {
    let input = part2_composition_gather_pixel(
        source, source_alpha, destination, destination_alpha,
        source_index, destination_index, expand_grayscale,
    );
    if input.is_none() {
        (false, destination, destination_alpha)
    } else {
        let (s, d, sa, da, premultiplied) = input.unwrap();
        let pixel = part2_source_over_pixel(s, d, sa, da, premultiplied, destination.len() as usize);
        if pixel.is_none() {
            (false, destination, destination_alpha)
        } else {
            part2_composition_scatter_pixel(
                destination, destination_alpha,
                destination_index, pixel.unwrap().0, pixel.unwrap().1,
            )
        }
    }
}

pub struct Part2CompositionGeometry {
    pub source_width: nat,
    pub source_x: nat,
    pub source_y: nat,
    pub source_crop_width: nat,
    pub source_crop_height: nat,
    pub target_x: nat,
    pub target_y: nat,
    pub target_width: nat,
    pub target_height: nat,
    pub canvas_width: nat,
    pub copy_width: nat,
    pub copy_height: nat,
}

pub open spec fn part2_composition_geometry(
    source_width: u32, source_height: u32, canvas_width: u32, canvas_height: u32,
    target_x: u32, target_y: u32, target_width: u32, target_height: u32,
    source_x: u32, source_y: u32, source_crop_width: u32, source_crop_height: u32,
) -> Part2CompositionGeometry {
    let sx = if source_x < source_width { source_x } else { source_width };
    let sy = if source_y < source_height { source_y } else { source_height };
    let remaining_width = ((source_width as int) - sx as int) as nat;
    let remaining_height = ((source_height as int) - sy as int) as nat;
    Part2CompositionGeometry {
        source_width: source_width as nat,
        source_x: sx as nat,
        source_y: sy as nat,
        source_crop_width: if source_crop_width == 0 || source_crop_width as nat > remaining_width {
            remaining_width
        } else { source_crop_width as nat },
        source_crop_height: if source_crop_height == 0 || source_crop_height as nat > remaining_height {
            remaining_height
        } else { source_crop_height as nat },
        target_x: target_x as nat,
        target_y: target_y as nat,
        target_width: target_width as nat,
        target_height: target_height as nat,
        canvas_width: canvas_width as nat,
        copy_width: part2_composition_clip(target_x, target_width, canvas_width) as nat,
        copy_height: part2_composition_clip(target_y, target_height, canvas_height) as nat,
    }
}

#[verifier::opaque]
pub open spec fn part2_composition_canvas_prefix(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: Part2CompositionGeometry, expand_grayscale: bool, count: nat,
) -> (bool, Seq<Seq<u16>>, Seq<u16>)
    recommends geometry.copy_width > 0 && geometry.target_width > 0 && geometry.target_height > 0,
    decreases count,
{
    if count == 0 {
        (true, destination, destination_alpha)
    } else {
        let previous = part2_composition_canvas_prefix(
            source, source_alpha, destination, destination_alpha,
            geometry, expand_grayscale, (count - 1) as nat,
        );
        if !previous.0 {
            previous
        } else {
            let x = ((count - 1) as nat) % geometry.copy_width;
            let y = ((count - 1) as nat) / geometry.copy_width;
            let sx = geometry.source_x + x * geometry.source_crop_width / geometry.target_width;
            let sy = geometry.source_y + y * geometry.source_crop_height / geometry.target_height;
            let source_index = sy * geometry.source_width + sx;
            let destination_index = (geometry.target_y + y) * geometry.canvas_width + geometry.target_x + x;
            part2_composition_pixel_transition(
                source, source_alpha, previous.1, previous.2,
                source_index, destination_index, expand_grayscale,
            )
        }
    }
}

pub open spec fn part2_composition_layer_result(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: Part2CompositionGeometry, expand_grayscale: bool,
) -> (bool, Seq<Seq<u16>>, Seq<u16>) {
    if geometry.source_crop_width == 0 || geometry.source_crop_height == 0 {
        (true, destination, destination_alpha)
    } else if geometry.target_width == 0 || geometry.target_height == 0 {
        (false, destination, destination_alpha)
    } else if geometry.copy_width == 0 || geometry.copy_height == 0 {
        (true, destination, destination_alpha)
    } else {
        part2_composition_canvas_prefix(
            source, source_alpha, destination, destination_alpha,
            geometry, expand_grayscale, geometry.copy_width * geometry.copy_height,
        )
    }
}

pub open spec fn part2_composition_layer_work(geometry: Part2CompositionGeometry, channels: nat) -> nat {
    if geometry.source_crop_width == 0 || geometry.source_crop_height == 0
        || geometry.target_width == 0 || geometry.target_height == 0
        || geometry.copy_width == 0 || geometry.copy_height == 0 {
        0
    } else {
        geometry.copy_width * geometry.copy_height * (channels + 1)
    }
}

pub open spec fn part2_composition_layer_success(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: Part2CompositionGeometry, expand_grayscale: bool, initial_work: nat,
) -> (bool, Seq<Seq<u16>>, Seq<u16>, nat) {
    let result = part2_composition_layer_result(
        source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
    );
    (result.0, result.1, result.2, initial_work + part2_composition_layer_work(geometry, destination.len()))
}

pub type Part2CompositionBudgetedState = (Result<(), crate::DecodeError>, Seq<Seq<u16>>, Seq<u16>, nat);

pub open spec fn part2_composition_error(arithmetic: bool) -> crate::DecodeError {
    crate::DecodeError::MalformedInput {
        offset: 0,
        kind: if arithmetic { crate::MalformedKind::ArithmeticOverflow } else { crate::MalformedKind::InvalidBoxContents },
    }
}

pub open spec fn part2_composition_budgeted_step(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: Part2CompositionGeometry, expand_grayscale: bool,
    work: nat, work_limit: u64, x: nat, y: nat,
) -> Part2CompositionBudgetedState
    recommends geometry.target_width > 0 && geometry.target_height > 0,
{
    let sx = geometry.source_x + x * geometry.source_crop_width / geometry.target_width;
    let sy = geometry.source_y + y * geometry.source_crop_height / geometry.target_height;
    let si = sy * geometry.source_width + sx;
    let di = (geometry.target_y + y) * geometry.canvas_width + geometry.target_x + x;
    if si > usize::MAX as nat || di > usize::MAX as nat {
        (Err(part2_composition_error(true)), destination, destination_alpha, work)
    } else {
        let pixel = part2_composition_pixel_transition(
            source, source_alpha, destination, destination_alpha, si, di, expand_grayscale,
        );
        if !pixel.0 {
            (Err(part2_composition_error(false)), pixel.1, pixel.2, work)
        } else {
            let next = work + destination.len() + 1;
            if next > u64::MAX as nat {
                (Err(part2_composition_error(true)), pixel.1, pixel.2, work)
            } else if next > work_limit as nat {
                (Err(crate::DecodeError::LimitExceeded {
                    resource: crate::Resource::WorkUnits, limit: work_limit, actual: next as u64,
                }), pixel.1, pixel.2, next)
            } else {
                (Ok(()), pixel.1, pixel.2, next)
            }
        }
    }
}

#[verifier::opaque]
pub open spec fn part2_composition_budgeted_prefix(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: Part2CompositionGeometry, expand_grayscale: bool,
    initial_work: nat, work_limit: u64, count: nat,
) -> Part2CompositionBudgetedState
    recommends geometry.copy_width > 0 && geometry.target_width > 0 && geometry.target_height > 0,
    decreases count,
{
    if count == 0 {
        (Ok(()), destination, destination_alpha, initial_work)
    } else {
        let previous = part2_composition_budgeted_prefix(
            source, source_alpha, destination, destination_alpha,
            geometry, expand_grayscale, initial_work, work_limit, (count - 1) as nat,
        );
        if previous.0.is_err() {
            previous
        } else {
            part2_composition_budgeted_step(
                source, source_alpha, previous.1, previous.2, geometry, expand_grayscale,
                previous.3, work_limit, ((count - 1) as nat) % geometry.copy_width,
                ((count - 1) as nat) / geometry.copy_width,
            )
        }
    }
}

pub open spec fn part2_composition_layer_transition(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: Part2CompositionGeometry, expand_grayscale: bool, initial_work: nat, work_limit: u64,
) -> Part2CompositionBudgetedState {
    if geometry.source_crop_width == 0 || geometry.source_crop_height == 0 {
        (Ok(()), destination, destination_alpha, initial_work)
    } else if geometry.target_width == 0 || geometry.target_height == 0 {
        (Err(part2_composition_error(false)), destination, destination_alpha, initial_work)
    } else if geometry.copy_width == 0 || geometry.copy_height == 0 {
        (Ok(()), destination, destination_alpha, initial_work)
    } else {
        part2_composition_budgeted_prefix(
            source, source_alpha, destination, destination_alpha,
            geometry, expand_grayscale, initial_work, work_limit, geometry.copy_width * geometry.copy_height,
        )
    }
}

pub open spec fn part2_render_space(layers: Seq<Part2Placement>) -> crate::color::ConvertedColorSpace {
    if exists|i: int| 0 <= i < layers.len() && #[trigger] layers[i].0.2 == crate::color::ConvertedColorSpace::Srgb {
        crate::color::ConvertedColorSpace::Srgb
    } else {
        crate::color::ConvertedColorSpace::Grayscale
    }
}

pub open spec fn part2_render_layer_bytes(layer: Part2OwnedLayer) -> Result<nat, crate::DecodeError> {
    let channels = (layer.3.len() + 1) * 2;
    if layer.3.len() + 1 > u64::MAX as nat || channels > u64::MAX as nat {
        Err(part2_composition_error(true))
    } else {
        match part2_layer_area(layer.0, layer.1) {
            Err(error) => Err(error),
            Ok(area) => if area as nat * channels > u64::MAX as nat {
                Err(part2_composition_error(true))
            } else { Ok(area as nat * channels) },
        }
    }
}

pub open spec fn part2_charge_layer_bytes(layer: Part2OwnedLayer, previous: nat, byte_limit: u64)
    -> (Result<(), crate::DecodeError>, nat)
{
    match part2_render_layer_bytes(layer) {
        Err(error) => (Err(error), previous),
        Ok(bytes) => if previous + bytes > u64::MAX as nat {
            (Err(part2_composition_error(true)), previous)
        } else if previous + bytes > byte_limit as nat {
            (Err(crate::DecodeError::LimitExceeded {
                resource: crate::Resource::DecodedBytes, limit: byte_limit, actual: (previous + bytes) as u64,
            }), previous + bytes)
        } else { (Ok(()), previous + bytes) },
    }
}

pub open spec fn part2_render_step(
    width: u32, height: u32, space: crate::color::ConvertedColorSpace,
    placement: Part2Placement, previous: Part2RenderState, byte_limit: u64, work_limit: u64,
) -> Part2RenderState {
    if previous.0.is_err() { previous } else {
        let charge = part2_charge_layer_bytes(placement.0, previous.4, byte_limit);
        match charge.0 {
            Err(error) => (Err(error), previous.1, previous.2, previous.3, charge.1),
            Ok(()) => {
                let layer = placement.0;
                let instruction = placement.1;
                let state = part2_composition_layer_transition(
                    layer.3, layer.4, previous.1, previous.2,
                    part2_composition_geometry(layer.0, layer.1, width, height,
                        instruction.1, instruction.2, instruction.3, instruction.4,
                        instruction.5, instruction.6, instruction.7, instruction.8),
                    matches!((space, layer.2), (crate::color::ConvertedColorSpace::Srgb, crate::color::ConvertedColorSpace::Grayscale)),
                    previous.3, work_limit,
                );
                (state.0, state.1, state.2, state.3, charge.1)
            },
        }
    }
}

#[verifier::opaque]
pub open spec fn part2_render_prefix(
    width: u32, height: u32, layers: Seq<Part2Placement>, initial: Part2RenderState,
    byte_limit: u64, work_limit: u64, count: nat,
) -> Part2RenderState
    recommends count <= layers.len(),
    decreases count,
{
    if count == 0 { initial } else {
        part2_render_step(width, height, part2_render_space(layers), layers[count as int - 1],
            part2_render_prefix(width, height, layers, initial, byte_limit, work_limit, (count - 1) as nat),
            byte_limit, work_limit)
    }
}

pub open spec fn part2_render_initial(width: u32, height: u32, layers: Seq<Part2Placement>) -> Part2RenderState {
    match part2_layer_area(width, height) {
        Err(error) => (Err(error), Seq::empty(), Seq::empty(), 0, 0),
        Ok(area) => {
            let colors = part2_layer_color_count(part2_render_space(layers));
            let bytes = area as nat * ((colors + 1) * 2);
            if bytes > u64::MAX as nat {
                (Err(part2_composition_error(true)), Seq::empty(), Seq::empty(), 0, 0)
            } else {
                (Ok(()), Seq::new(colors, |i: int| Seq::new(area as nat, |j: int| 0u16)),
                    Seq::new(area as nat, |j: int| 0u16), area as nat, bytes)
            }
        },
    }
}

pub open spec fn part2_render_layers(
    width: u32, height: u32, layers: Seq<Part2Placement>, byte_limit: u64, work_limit: u64,
) -> Part2ConvertedImageResult {
    if layers.len() == 0 { Err(part2_composition_error(false)) } else {
        let state = part2_render_prefix(width, height, layers,
            part2_render_initial(width, height, layers), byte_limit, work_limit, layers.len());
        match state.0 {
            Err(error) => Err(error),
            Ok(()) => part2_image_from_planes(width, height, part2_render_space(layers), state.1, state.2, false),
        }
    }
}

pub open spec fn part2_frame_index(requested: usize, frames: nat, loop_count: u8) -> Result<usize, crate::DecodeError> {
    if frames == 0 {
        Err(crate::DecodeError::FrameIndexOutOfRange { index: requested, frames: 0 })
    } else if loop_count == 0 {
        Ok((requested as nat % frames) as usize)
    } else if frames * loop_count as nat > usize::MAX as nat {
        Err(part2_composition_error(true))
    } else if requested as nat >= frames * loop_count as nat {
        Err(crate::DecodeError::FrameIndexOutOfRange { index: requested, frames: (frames * loop_count as nat) as usize })
    } else {
        Ok((requested as nat % frames) as usize)
    }
}

#[verifier::opaque]
pub open spec fn part2_frame_instruction_sequence(frames: Seq<Part2Frame>, selected: nat, count: nat) -> Seq<Part2Instruction>
    recommends selected < frames.len(), count <= selected + 1,
    decreases count,
{
    if count == 0 { Seq::empty() } else {
        let prior = part2_frame_instruction_sequence(frames, selected, (count - 1) as nat);
        if count - 1 == selected || frames[count as int - 1].3 {
            prior + frames[count as int - 1].4
        } else { prior }
    }
}

pub open spec fn part2_select_frame(frames: Seq<Part2Frame>, requested: usize, loop_count: u8)
    -> Result<Seq<Part2Instruction>, crate::DecodeError>
{
    match part2_frame_index(requested, frames.len(), loop_count) {
        Err(error) => Err(error),
        Ok(selected) => Ok(part2_frame_instruction_sequence(frames, selected as nat, selected as nat + 1)),
    }
}

#[verifier::opaque]
pub open spec fn part2_supply_search(supplied: Seq<Part2SuppliedLayer>, reference: usize, start: nat) -> Option<nat>
    recommends start <= supplied.len(),
    decreases supplied.len() - start,
{
    if start >= supplied.len() { None }
    else if supplied[start as int].0 == reference && supplied[start as int].1.is_some() { Some(start) }
    else { part2_supply_search(supplied, reference, start + 1) }
}

pub open spec fn part2_take_supplied_layer(supplied: Seq<Part2SuppliedLayer>, reference: usize)
    -> (Part2OwnedLayerResult, Seq<Part2SuppliedLayer>)
{
    match part2_supply_search(supplied, reference, 0) {
        None => (Err(part2_composition_error(false)), supplied),
        Some(index) => (supplied[index as int].1.unwrap(), supplied.update(index as int, (reference, None))),
    }
}

pub open spec fn part2_gather_layer_step(instruction: Part2Instruction, prior: Part2GatheredLayers) -> Part2GatheredLayers {
    if prior.0.is_err() { prior } else {
        let taken = part2_take_supplied_layer(prior.2, instruction.0);
        match taken.0 {
            Err(error) => (Err(error), prior.1, taken.1),
            Ok(layer) => (Ok(()), prior.1.push((layer, instruction)), taken.1),
        }
    }
}

#[verifier::opaque]
pub open spec fn part2_gather_layers(
    instructions: Seq<Part2Instruction>, supplied: Seq<Part2SuppliedLayer>, count: nat,
) -> Part2GatheredLayers
    recommends count <= instructions.len(),
    decreases count,
{
    if count == 0 { (Ok(()), Seq::empty(), supplied) } else {
        part2_gather_layer_step(instructions[count as int - 1], part2_gather_layers(instructions, supplied, (count - 1) as nat))
    }
}

pub open spec fn part2_collect_supplied_layers(instructions: Seq<Part2Instruction>, supplied: Seq<Part2SuppliedLayer>)
    -> Result<Seq<Part2Placement>, crate::DecodeError>
{
    let result = part2_gather_layers(instructions, supplied, instructions.len());
    match result.0 { Err(error) => Err(error), Ok(()) => Ok(result.1) }
}

pub open spec fn part2_render_normalized_frame(
    width: u32, height: u32, frames: Seq<Part2Frame>, loop_count: u8, requested: usize,
    supplied: Seq<Part2SuppliedLayer>, byte_limit: u64, work_limit: u64,
) -> Part2ConvertedImageResult {
    match part2_select_frame(frames, requested, loop_count) {
        Err(error) => Err(error),
        Ok(instructions) => match part2_collect_supplied_layers(instructions, supplied) {
            Err(error) => Err(error),
            Ok(placements) => part2_render_layers(width, height, placements, byte_limit, work_limit),
        },
    }
}

pub open spec fn part2_timeline_limit(layer_count: usize, maximum_frames: u32) -> Result<usize, crate::DecodeError> {
    let layers = if layer_count == 0 { 1nat } else { layer_count as nat };
    let limit = layers * maximum_frames as nat;
    if limit > usize::MAX as nat { Err(part2_composition_error(true)) } else { Ok(limit as usize) }
}

pub open spec fn part2_timeline_work_error(limit: usize, index: nat) -> crate::DecodeError {
    if index + 1 > u64::MAX as nat { part2_composition_error(true) } else {
        crate::DecodeError::LimitExceeded { resource: crate::Resource::WorkUnits, limit: limit as u64, actual: (index + 1) as u64 }
    }
}

pub open spec fn part2_timeline_assign(layer: usize, instruction: Part2TimelineInstruction) -> Part2Instruction {
    let i = instruction.0;
    (layer, i.1, i.2, i.3, i.4, i.5, i.6, i.7, i.8, i.9, i.10)
}

pub open spec fn part2_timeline_frame(
    frames: Seq<Part2Frame>, time: u64, instruction: Part2Instruction, life: u32, maximum_frames: u32,
) -> Result<(Seq<Part2Frame>, u64), crate::DecodeError> {
    let fresh = frames.len() == 0 || frames.last().2 != 0;
    if fresh && frames.len() >= maximum_frames as nat {
        Err(crate::DecodeError::LimitExceeded { resource: crate::Resource::Frames,
            limit: maximum_frames as u64, actual: (frames.len() + 1) as u64 })
    } else if time as nat + life as nat > u64::MAX as nat {
        Err(part2_composition_error(true))
    } else {
        let next = if fresh {
            frames.push((frames.len() as usize, time, life, instruction.9, seq![instruction]))
        } else {
            let last = frames.last();
            frames.update(frames.len() as int - 1, (last.0, last.1, life, instruction.9, last.4.push(instruction)))
        };
        Ok((next, (time as nat + life as nat) as u64))
    }
}

pub open spec fn part2_timeline_step(
    state: Part2TimelineState, instruction: Part2TimelineInstruction,
    layer_count: usize, maximum_frames: u32, maximum_instructions: usize,
) -> Part2TimelineExecution {
    let (frames, old_assignments, next_layer, index, time) = state;
    if index >= maximum_instructions {
        Err(part2_timeline_work_error(maximum_instructions, index as nat))
    } else {
        let assignments = if index as nat >= old_assignments.len() { old_assignments.push(None) } else { old_assignments };
        let assigned = if (index as nat) < assignments.len() { assignments[index as int] } else { None };
        if assigned.is_none() && next_layer >= layer_count {
            Ok((false, (frames, assignments, next_layer, index, time)))
        } else {
            let layer = if assigned.is_some() { assigned.unwrap() } else { next_layer };
            let next = if assigned.is_some() { next_layer as nat } else { next_layer as nat + 1 };
            if next > usize::MAX as nat { Err(part2_composition_error(true)) } else {
                match part2_timeline_frame(frames, time, part2_timeline_assign(layer, instruction), instruction.1, maximum_frames) {
                    Err(error) => Err(error),
                    Ok((new_frames, new_time)) => {
                        let target = index as nat + instruction.0.10 as nat;
                        if instruction.0.10 != 0 && target > usize::MAX as nat {
                            Err(part2_composition_error(true))
                        } else if instruction.0.10 != 0 && target >= maximum_instructions as nat {
                            Err(part2_timeline_work_error(maximum_instructions, target))
                        } else if instruction.0.10 != 0 && target < assignments.len()
                            && assignments[target as int].is_some() && assignments[target as int].unwrap() != layer {
                            Err(part2_composition_error(false))
                        } else if index as nat + 1 > usize::MAX as nat {
                            Err(part2_composition_error(true))
                        } else {
                            let extended = if target < assignments.len() { assignments } else {
                                assignments + Seq::new((target + 1 - assignments.len()) as nat, |i: int| None)
                            };
                            let new_assignments = if instruction.0.10 == 0 { assignments }
                                else { extended.update(target as int, Some(layer)) };
                            Ok((true, (new_frames, new_assignments, next as usize, (index + 1) as usize, new_time)))
                        }
                    },
                }
            }
        }
    }
}

pub open spec fn part2_timeline_finished(execution: Part2TimelineExecution) -> bool {
    execution.is_err() || !execution.ok().unwrap().0
}

#[verifier::opaque]
pub open spec fn part2_timeline_instructions(
    instructions: Seq<Part2TimelineInstruction>, initial: Part2TimelineExecution, count: nat,
    layers: usize, maximum_frames: u32, maximum_instructions: usize,
) -> Part2TimelineExecution
    recommends count <= instructions.len(),
    decreases count,
{
    if count == 0 { initial } else {
        let prior = part2_timeline_instructions(instructions, initial, (count - 1) as nat, layers, maximum_frames, maximum_instructions);
        if part2_timeline_finished(prior) { prior } else {
            part2_timeline_step(prior.ok().unwrap().1, instructions[count as int - 1], layers, maximum_frames, maximum_instructions)
        }
    }
}

#[verifier::opaque]
pub open spec fn part2_timeline_repetitions(
    instructions: Seq<Part2TimelineInstruction>, initial: Part2TimelineExecution, count: nat,
    layers: usize, maximum_frames: u32, maximum_instructions: usize,
) -> Part2TimelineExecution
    decreases count,
{
    if count == 0 { initial } else {
        part2_timeline_instructions(instructions,
            part2_timeline_repetitions(instructions, initial, (count - 1) as nat, layers, maximum_frames, maximum_instructions),
            instructions.len(), layers, maximum_frames, maximum_instructions)
    }
}

#[verifier::opaque]
pub open spec fn part2_timeline_sets(
    sets: Seq<Part2TimelineSet>, initial: Part2TimelineExecution, count: nat,
    layers: usize, maximum_frames: u32, maximum_instructions: usize,
) -> Part2TimelineExecution
    recommends count <= sets.len(),
    decreases count,
{
    if count == 0 { initial } else {
        let set = sets[count as int - 1];
        part2_timeline_repetitions(set.1,
            part2_timeline_sets(sets, initial, (count - 1) as nat, layers, maximum_frames, maximum_instructions),
            set.0 as nat + 1, layers, maximum_frames, maximum_instructions)
    }
}

pub open spec fn part2_timeline_frames(execution: Part2TimelineExecution) -> Result<Seq<Part2Frame>, crate::DecodeError> {
    match execution { Err(error) => Err(error), Ok((_, state)) => Ok(state.0) }
}

pub open spec fn part2_build_timeline(sets: Seq<Part2TimelineSet>, layers: usize, maximum_frames: u32)
    -> Result<Seq<Part2Frame>, crate::DecodeError>
{
    match part2_timeline_limit(layers, maximum_frames) {
        Err(error) => Err(error),
        Ok(limit) => part2_timeline_frames(part2_timeline_sets(sets,
            Ok((true, (Seq::empty(), Seq::empty(), 0, 0, 0))), sets.len(), layers, maximum_frames, limit)),
    }
}

pub open spec fn part2_chroma_key_alpha(matches: bool) -> u16 {
    if matches { 0 } else { u16::MAX }
}

pub open spec fn part2_metadata_count(parent: u32, child: u32) -> u32
    recommends parent <= u32::MAX - child,
{
    ((parent as int) + (child as int)) as u32
}

// Part 1 functional semantics used by executable production contracts.
pub open spec fn part1_ceil_div(value: nat, divisor: nat) -> nat
    recommends divisor > 0,
{
    if value == 0 {
        0
    } else {
        ((((value - 1) as int) / (divisor as int)) + 1int) as nat
    }
}

pub open spec fn part1_clip_start(nominal: nat, image_start: nat) -> nat {
    if nominal < image_start { image_start } else { nominal }
}

pub open spec fn part1_clip_end(nominal_end: nat, image_end: nat) -> nat {
    if nominal_end < image_end { nominal_end } else { image_end }
}

pub open spec fn part1_packet_data_bits(previous_was_ff: bool) -> nat {
    if previous_was_ff { 7 } else { 8 }
}

pub open spec fn part1_packet_bit(byte: u8, remaining: u8) -> bool
    recommends remaining < 8,
{
    ((byte >> remaining) & 1) != 0
}

pub struct Tier1RawCoefficient {
    pub limbs: Seq<u64>,
    pub legacy_bitplane: Option<u16>,
    pub negative: bool,
    pub significant: bool,
    pub refined: bool,
}

pub struct Tier1RawPlane {
    pub width: u16,
    pub height: u16,
    pub coefficients: Seq<Tier1RawCoefficient>,
}

pub open spec fn tier1_raw_plane_index(plane: Tier1RawPlane, x: u16, y: u16) -> Result<usize, crate::DecodeError> {
    let index = y as nat * plane.width as nat + x as nat;
    if x >= plane.width || y >= plane.height {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
    } else if index > usize::MAX {
        Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
    } else if index >= plane.coefficients.len() {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
    } else { Ok(index as usize) }
}

pub open spec fn tier1_raw_plane_coefficient(plane: Tier1RawPlane, x: u16, y: u16)
    -> Result<Tier1RawCoefficient, crate::DecodeError>
{
    match tier1_raw_plane_index(plane, x, y) {
        Err(error) => Err(error),
        Ok(index) => Ok(plane.coefficients[index as int]),
    }
}

pub open spec fn tier1_value_limbs(value: nat) -> Seq<u64> {
    Seq::new(6, |i: int| ((value / pow2((64 * i) as nat)) % 18446744073709551616) as u64)
}

pub open spec fn tier1_empty_coefficient() -> Tier1RawCoefficient {
    Tier1RawCoefficient {
        limbs: Seq::new(6, |i: int| 0u64), legacy_bitplane: None,
        negative: false, significant: false, refined: false,
    }
}

pub open spec fn tier1_empty_plane(width: u16, height: u16) -> Result<Tier1RawPlane, crate::DecodeError> {
    let samples = width as nat * height as nat;
    if width == 0 || height == 0 { Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)) }
    else if samples > usize::MAX { Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)) }
    else if samples > 4096 { Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)) }
    else { Ok(Tier1RawPlane {
        width, height, coefficients: Seq::new(samples, |i: int| tier1_empty_coefficient()),
    }) }
}

pub open spec fn tier1_new_coefficient(limbs: Seq<u64>, negative: bool, significant: bool, refined: bool)
    -> Tier1RawCoefficient {
    Tier1RawCoefficient { limbs, legacy_bitplane: None, negative, significant, refined }
}

pub open spec fn tier1_coefficient_new_outcome(
    limbs: Seq<u64>, negative: bool, significant: bool, refined: bool,
) -> Result<Tier1RawCoefficient, crate::DecodeError> {
    Ok(tier1_new_coefficient(limbs, negative, significant, refined))
}

pub open spec fn tier1_set_magnitude(coefficient: Tier1RawCoefficient, limbs: Seq<u64>)
    -> (Result<(), crate::DecodeError>, Tier1RawCoefficient)
{
    (Ok(()), Tier1RawCoefficient { limbs, ..coefficient })
}

pub open spec fn tier1_raw_plane_width(plane: Tier1RawPlane) -> u16 { plane.width }
pub open spec fn tier1_raw_plane_height(plane: Tier1RawPlane) -> u16 { plane.height }
pub open spec fn tier1_raw_plane_coefficients(plane: Tier1RawPlane) -> Seq<Tier1RawCoefficient> {
    plane.coefficients
}
pub open spec fn tier1_coefficient_limbs(coefficient: Tier1RawCoefficient) -> Seq<u64> {
    coefficient.limbs
}

pub open spec fn tier1_mark_coefficient(
    plane: Tier1RawPlane, x: u16, y: u16, limbs: Seq<u64>, value: nat, negative: bool,
) -> (Result<(), crate::DecodeError>, Tier1RawPlane) {
    if value == 0 { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), plane) }
    else {
        match tier1_raw_plane_index(plane, x, y) {
            Err(error) => (Err(error), plane),
            Ok(index) => if plane.coefficients[index as int].significant {
                (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), plane)
            } else {
                (Ok(()), Tier1RawPlane { coefficients: plane.coefficients.update(index as int,
                    tier1_new_coefficient(limbs, negative, true, false)), ..plane })
            },
        }
    }
}

pub open spec fn tier1_statistics_zero() -> crate::tier1::Tier1PassStatistics {
    crate::tier1::Tier1PassStatistics {
        coefficients_scanned: 0, coding_symbols: 0, newly_significant: 0, refined_coefficients: 0, run_mode_columns: 0,
    }
}

pub open spec fn tier1_statistics_increment(
    state: crate::tier1::Tier1PassStatistics, field: u8, count: u32,
) -> (Result<(), crate::DecodeError>, crate::tier1::Tier1PassStatistics) {
    let value = if field == 0 { state.coefficients_scanned } else if field == 1 { state.coding_symbols }
        else if field == 2 { state.newly_significant } else if field == 3 { state.refined_coefficients }
        else { state.run_mode_columns };
    if value + count > u32::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state) }
    else {
        let value = (value + count) as u32;
        (Ok(()), if field == 0 { crate::tier1::Tier1PassStatistics { coefficients_scanned: value, ..state } }
            else if field == 1 { crate::tier1::Tier1PassStatistics { coding_symbols: value, ..state } }
            else if field == 2 { crate::tier1::Tier1PassStatistics { newly_significant: value, ..state } }
            else if field == 3 { crate::tier1::Tier1PassStatistics { refined_coefficients: value, ..state } }
            else { crate::tier1::Tier1PassStatistics { run_mode_columns: value, ..state } })
    }
}

pub open spec fn tier1_statistics_merge_step(
    prior: (Result<(), crate::DecodeError>, crate::tier1::Tier1PassStatistics), field: u8, count: u32,
) -> (Result<(), crate::DecodeError>, crate::tier1::Tier1PassStatistics) {
    match prior.0 { Err(_) => prior, Ok(()) => tier1_statistics_increment(prior.1, field, count) }
}

pub open spec fn tier1_statistics_merge(state: crate::tier1::Tier1PassStatistics, other: crate::tier1::Tier1PassStatistics)
    -> (Result<(), crate::DecodeError>, crate::tier1::Tier1PassStatistics) {
    let result = tier1_statistics_increment(state, 0, other.coefficients_scanned);
    let result = tier1_statistics_merge_step(result, 1, other.coding_symbols);
    let result = tier1_statistics_merge_step(result, 2, other.newly_significant);
    let result = tier1_statistics_merge_step(result, 3, other.refined_coefficients);
    tier1_statistics_merge_step(result, 4, other.run_mode_columns)
}

pub open spec fn ht_copy_refinement_bytes(input: Seq<u8>) -> Result<Seq<u8>, crate::DecodeError> { Ok(input) }

pub open spec fn tier1_raw_signed_coefficient(coefficient: Tier1RawCoefficient)
    -> Result<(Seq<u64>, nat, bool), crate::DecodeError>
    recommends coefficient.limbs.len() == 6,
{
    if !coefficient.significant {
        Ok((Seq::new(6, |i: int| 0u64), 0, false))
    } else {
        let base = part1_magnitude_value(coefficient.limbs);
        let delta = match coefficient.legacy_bitplane {
            None => 0nat,
            Some(bitplane) => if bitplane <= 64 { part1_tier1_midpoint_delta(bitplane) as nat }
                else { pow2((bitplane - 1) as nat) },
        };
        if (match coefficient.legacy_bitplane { Some(bitplane) => bitplane > 384, None => false })
            || base + delta >= part1_magnitude_capacity() {
            Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
        } else { Ok((tier1_value_limbs(base + delta), base + delta, coefficient.negative)) }
    }
}

pub type HtNumericMagnitudeResult = (Result<(nat, bool, u8), crate::DecodeError>, HtMagSgnState);

pub open spec fn ht_complete_kappa(base: u8, significance: u8, north_west: u8, north_east: u8, maximum: u8)
    -> Result<u8, crate::DecodeError> {
    let exponent = if north_west > north_east { north_west } else { north_east };
    let gamma = significance & significance.wrapping_sub(1) != 0;
    if base == 0 || significance & 0xf0u8 != 0 || maximum == 0 || maximum > 75 {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
    } else if base as nat + exponent as nat > 255 {
        Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
    } else {
        let completed = ht_kappa_completion(base as nat, gamma, exponent as nat);
        if completed > maximum { Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)) }
        else { Ok(completed as u8) }
    }
}

pub struct HtPlaneState {
    pub output: Tier1RawPlane,
    pub reader: HtMagSgnState,
    pub previous: Seq<u8>,
    pub current: Seq<u8>,
    pub statistics: crate::tier1::Tier1PassStatistics,
}

pub struct Tier1LegacyFrame {
    pub plane: Tier1RawPlane,
    pub next_pass: u32,
    pub bitplane: u16,
    pub bitplane_state: Option<crate::tier1::Tier1BitplaneState>,
    pub mq_context_state: Option<crate::mq::MqContextState>,
    pub reset_contexts_before_mq: bool,
    pub decoded_segments: u32,
    pub decoded_bytes: u64,
    pub input: Seq<u8>,
    pub passes: u32,
}

pub open spec fn ht_neighbor_zero() -> crate::tier1::NeighborSummary {
    crate::tier1::NeighborSummary {
        horizontal_significant: 0, vertical_significant: 0, diagonal_significant: 0,
        horizontal_sign_balance: 0, vertical_sign_balance: 0,
    }
}

pub open spec fn ht_significant_neighbor_count(summary: crate::tier1::NeighborSummary) -> nat {
    summary.horizontal_significant as nat + summary.vertical_significant as nat + summary.diagonal_significant as nat
}

pub open spec fn ht_significance_magnitude(decoded: u16) -> Result<(Seq<u64>, nat), crate::DecodeError> {
    if decoded < 2 || decoded > 292 { Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)) }
    else {
        let scaled = 3nat * pow2((decoded - 2) as nat);
        if scaled >= part1_magnitude_capacity() { Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)) }
        else { Ok((part1_magnitude_shift(part1_magnitude_left(
            Seq::new(6, |i: int| if i == 0 { 3u64 } else { 0u64 }), (decoded - 2) as u16), 1), (scaled / 2) as nat)) }
    }
}

pub open spec fn ht_refinement_delta(decoded: u16) -> Result<(Seq<u64>, nat), crate::DecodeError> {
    if decoded < 2 || decoded > 292 { Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)) }
    else if decoded == 2 { Ok((Seq::new(6, |i: int| if i == 0 { 1u64 } else { 0u64 }), 1)) }
    else { Ok((part1_magnitude_bit((decoded - 3) as u16), pow2((decoded - 3) as nat))) }
}

pub open spec fn ht_cleanup_significance(coefficients: Seq<Tier1RawCoefficient>) -> Seq<bool> {
    Seq::new(coefficients.len(), |i: int| coefficients[i].significant)
}

pub open spec fn ht_apply_refinement(
    plane: Tier1RawPlane, x: u16, y: u16, delta: nat, bit: bool,
) -> (Result<(), crate::DecodeError>, Tier1RawPlane)
    recommends forall |i: int| 0 <= i < plane.coefficients.len() ==> plane.coefficients[i].limbs.len() == 6,
{
    if delta == 0 && !bit { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), plane) }
    else {
        match tier1_raw_plane_index(plane, x, y) {
            Err(error) => (Err(error), plane),
            Ok(index) => {
                let coefficient = plane.coefficients[index as int];
                let value = part1_magnitude_value(coefficient.limbs);
                let changed = if bit { value + delta } else { (value - delta) as nat };
                if !coefficient.significant || (!bit && value < delta) || changed == 0 {
                    (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), plane)
                } else if changed >= part1_magnitude_capacity() {
                    (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), plane)
                } else {
                    (Ok(()), Tier1RawPlane { coefficients: plane.coefficients.update(index as int,
                        Tier1RawCoefficient { limbs: tier1_value_limbs(changed), refined: true, ..coefficient }), ..plane })
                }
            },
        }
    }
}

pub struct HtRefinementCoreState {
    pub output: Tier1RawPlane,
    pub readers: HtRefinementReadersState,
    pub cleanup_significant: Seq<bool>,
    pub statistics: crate::tier1::Tier1PassStatistics,
}

pub type HtRefinementCoreResult = (Result<(), crate::DecodeError>, HtRefinementCoreState);

pub open spec fn ht_refinement_count(state: HtRefinementCoreState, field: u8) -> HtRefinementCoreResult {
    let count = tier1_statistics_increment(state.statistics, field, 1);
    (count.0, HtRefinementCoreState { statistics: count.1, ..state })
}

pub open spec fn ht_magref_sample(state: HtRefinementCoreState, x: u16, y: u16, delta: nat, bitplanes: u16)
    -> HtRefinementCoreResult
    recommends forall |i: int| 0 <= i < state.output.coefficients.len() ==> state.output.coefficients[i].limbs.len() == 6,
{
    let scanned = ht_refinement_count(state, 0);
    match scanned.0 {
        Err(_) => scanned,
        Ok(()) => match tier1_raw_plane_index(scanned.1.output, x, y) {
            Err(error) => (Err(error), scanned.1),
            Ok(index) => if index >= scanned.1.cleanup_significant.len() {
                (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), scanned.1)
            } else if !scanned.1.cleanup_significant[index as int] { scanned }
            else {
                let read = ht_refinement_read_reverse(scanned.1.readers);
                let state = HtRefinementCoreState { readers: read.1, ..scanned.1 };
                match read.0 {
                    Err(error) => (Err(error), state),
                    Ok(bit) => {
                        let store = ht_apply_refinement(state.output, x, y,
                            if bitplanes == 2 && bit { 0nat } else { delta }, bit);
                        let state = HtRefinementCoreState { output: store.1, ..state };
                        match store.0 {
                            Err(error) => (Err(error), state),
                            Ok(()) => {
                                let counted = ht_refinement_count(state, 1);
                                match counted.0 { Err(_) => counted, Ok(()) => ht_refinement_count(counted.1, 3) }
                            },
                        }
                    },
                }
            },
        },
    }
}

pub type HtDiscoveryResult = (Result<(), crate::DecodeError>, HtRefinementCoreState, Tier1RawPlane, Seq<(u16, u16)>);

pub open spec fn ht_discover_sample_fields(
    state: HtRefinementCoreState, discovery: Tier1RawPlane, pending: Seq<(u16, u16)>,
    x: u16, y: u16, magnitude: (Seq<u64>, nat), causal: bool,
) -> HtDiscoveryResult {
    let scanned = ht_refinement_count(state, 0);
    match scanned.0 {
        Err(error) => (Err(error), scanned.1, discovery, pending),
        Ok(()) => match tier1_raw_plane_coefficient(discovery, x, y) {
            Err(error) => (Err(error), scanned.1, discovery, pending),
            Ok(coefficient) => if coefficient.significant { (Ok(()), scanned.1, discovery, pending) }
            else {
                match ht_neighbors(discovery, x, y, causal) {
                    Err(error) => (Err(error), scanned.1, discovery, pending),
                    Ok(summary) => if ht_significant_neighbor_count(summary) == 0 {
                        (Ok(()), scanned.1, discovery, pending)
                    } else {
                        let read = ht_refinement_read_forward(scanned.1.readers);
                        let state = HtRefinementCoreState { readers: read.1, ..scanned.1 };
                        match read.0 {
                            Err(error) => (Err(error), state, discovery, pending),
                            Ok(bit) => {
                                let counted = ht_refinement_count(state, 1);
                                match counted.0 {
                                    Err(error) => (Err(error), counted.1, discovery, pending),
                                    Ok(()) => if !bit { (Ok(()), counted.1, discovery, pending) } else {
                                        let store = tier1_mark_coefficient(discovery, x, y, magnitude.0, magnitude.1, false);
                                        (store.0, counted.1, store.1, if store.0.is_err() { pending } else { pending.push((x, y)) })
                                    },
                                }
                            },
                        }
                    },
                }
            },
        },
    }
}

pub open spec fn ht_sign_sample(
    state: HtRefinementCoreState, pending: Seq<(u16, u16)>, index: nat, magnitude: (Seq<u64>, nat),
) -> HtRefinementCoreResult {
    if index >= pending.len() { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state) }
    else {
        let read = ht_refinement_read_forward(state.readers);
        let state = HtRefinementCoreState { readers: read.1, ..state };
        match read.0 {
            Err(error) => (Err(error), state),
            Ok(negative) => {
                let store = tier1_mark_coefficient(state.output, pending[index as int].0, pending[index as int].1,
                    magnitude.0, magnitude.1, negative);
                let state = HtRefinementCoreState { output: store.1, ..state };
                match store.0 {
                    Err(error) => (Err(error), state),
                    Ok(()) => {
                        let counted = ht_refinement_count(state, 1);
                        match counted.0 { Err(_) => counted, Ok(()) => ht_refinement_count(counted.1, 2) }
                    },
                }
            },
        }
    }
}

pub struct HtSignificancePatchState {
    pub core: HtRefinementCoreState,
    pub discovery: Tier1RawPlane,
    pub pending: Seq<(u16, u16)>,
}

pub type HtSignificancePatchResult = (Result<(), crate::DecodeError>, HtSignificancePatchState);

pub open spec fn ht_discover_sample(
    state: HtSignificancePatchState, x: u16, y: u16, magnitude: (Seq<u64>, nat), causal: bool,
) -> HtSignificancePatchResult {
    let result = ht_discover_sample_fields(state.core, state.discovery, state.pending, x, y, magnitude, causal);
    (result.0, HtSignificancePatchState { core: result.1, discovery: result.2, pending: result.3 })
}

pub open spec fn ht_patch_sign_sample(
    state: HtSignificancePatchState, index: nat, magnitude: (Seq<u64>, nat),
) -> HtSignificancePatchResult {
    let result = ht_sign_sample(state.core, state.pending, index, magnitude);
    (result.0, HtSignificancePatchState { core: result.1, ..state })
}

#[verifier::opaque]
pub open spec fn ht_magref_rows(
    state: HtRefinementCoreState, x: u16, stripe_y: u16, rows: nat, row: nat, delta: nat, bitplanes: u16,
) -> HtRefinementCoreResult
    recommends row <= rows <= 4,
    decreases rows - row,
{
    if row >= rows { (Ok(()), state) }
    else if stripe_y as nat + row > u16::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state) }
    else {
        let step = ht_magref_sample(state, x, (stripe_y as nat + row) as u16, delta, bitplanes);
        match step.0 { Err(_) => step, Ok(()) => ht_magref_rows(step.1, x, stripe_y, rows, row + 1, delta, bitplanes) }
    }
}

#[verifier::opaque]
pub open spec fn ht_magref_columns(
    state: HtRefinementCoreState, width: u16, stripe_y: u16, rows: nat, x: nat, delta: nat, bitplanes: u16,
) -> HtRefinementCoreResult
    recommends x <= width, rows <= 4,
    decreases width as nat - x,
{
    if x >= width { (Ok(()), state) } else {
        let step = ht_magref_rows(state, x as u16, stripe_y, rows, 0, delta, bitplanes);
        match step.0 { Err(_) => step, Ok(()) => ht_magref_columns(step.1, width, stripe_y, rows, x + 1, delta, bitplanes) }
    }
}

#[verifier::opaque]
pub open spec fn ht_magref_stripes(
    state: HtRefinementCoreState, width: u16, height: u16, stripe_y: nat, delta: nat, bitplanes: u16,
) -> HtRefinementCoreResult
    recommends width <= 4096, height <= 4096, stripe_y <= height as nat + 3,
    decreases (height as nat + 3 - stripe_y) / 4,
{
    if stripe_y >= height { (Ok(()), state) } else {
        let rows = ht_stripe_rows(height as nat, stripe_y);
        let step = ht_magref_columns(state, width, stripe_y as u16, rows, 0, delta, bitplanes);
        match step.0 { Err(_) => step, Ok(()) => ht_magref_stripes(step.1, width, height, stripe_y + 4, delta, bitplanes) }
    }
}

#[verifier::opaque]
pub open spec fn ht_discovery_rows(
    state: HtSignificancePatchState, x: u16, stripe_y: u16, rows: nat, row: nat, magnitude: (Seq<u64>, nat), causal: bool,
) -> HtSignificancePatchResult
    recommends row <= rows <= 4,
    decreases rows - row,
{
    if row >= rows { (Ok(()), state) }
    else if stripe_y as nat + row > u16::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state) }
    else {
        let step = ht_discover_sample(state, x, (stripe_y as nat + row) as u16, magnitude, causal);
        match step.0 { Err(_) => step, Ok(()) => ht_discovery_rows(step.1, x, stripe_y, rows, row + 1, magnitude, causal) }
    }
}

#[verifier::opaque]
pub open spec fn ht_discovery_columns(
    state: HtSignificancePatchState, end: u16, stripe_y: u16, rows: nat, x: nat, magnitude: (Seq<u64>, nat), causal: bool,
) -> HtSignificancePatchResult
    recommends x <= end, rows <= 4,
    decreases end as nat - x,
{
    if x >= end { (Ok(()), state) } else {
        let step = ht_discovery_rows(state, x as u16, stripe_y, rows, 0, magnitude, causal);
        match step.0 { Err(_) => step, Ok(()) => ht_discovery_columns(step.1, end, stripe_y, rows, x + 1, magnitude, causal) }
    }
}

#[verifier::opaque]
pub open spec fn ht_pending_signs(state: HtSignificancePatchState, index: nat, magnitude: (Seq<u64>, nat))
    -> HtSignificancePatchResult
    recommends index <= state.pending.len(),
    decreases state.pending.len() - index,
{
    if index >= state.pending.len() { (Ok(()), state) } else {
        let step = ht_patch_sign_sample(state, index, magnitude);
        match step.0 { Err(_) => step, Ok(()) => ht_pending_signs(step.1, index + 1, magnitude) }
    }
}

pub struct HtSigpropPlaneState {
    pub core: HtRefinementCoreState,
    pub discovery: Tier1RawPlane,
}

pub type HtSigpropPlaneResult = (Result<(), crate::DecodeError>, HtSigpropPlaneState);

pub open spec fn ht_sigprop_patch(
    state: HtSigpropPlaneState, start: u16, end: u16, stripe_y: u16, rows: nat, magnitude: (Seq<u64>, nat), causal: bool,
) -> HtSigpropPlaneResult
    recommends start <= end, rows <= 4,
{
    let patch = HtSignificancePatchState { core: state.core, discovery: state.discovery, pending: Seq::empty() };
    let discovered = ht_discovery_columns(patch, end, stripe_y, rows, start as nat, magnitude, causal);
    let signed = match discovered.0 { Err(_) => discovered, Ok(()) => ht_pending_signs(discovered.1, 0, magnitude) };
    (signed.0, HtSigpropPlaneState { core: signed.1.core, discovery: signed.1.discovery })
}

#[verifier::opaque]
pub open spec fn ht_sigprop_patches(
    state: HtSigpropPlaneState, width: u16, stripe_y: u16, rows: nat, patch_x: nat, magnitude: (Seq<u64>, nat), causal: bool,
) -> HtSigpropPlaneResult
    recommends width <= 4096, patch_x <= width as nat + 3, rows <= 4,
    decreases (width as nat + 3 - patch_x) / 4,
{
    if patch_x >= width { (Ok(()), state) } else {
        let end = ht_patch_end(patch_x, width as nat) as u16;
        let step = ht_sigprop_patch(state, patch_x as u16, end, stripe_y, rows, magnitude, causal);
        match step.0 { Err(_) => step, Ok(()) => ht_sigprop_patches(step.1, width, stripe_y, rows, patch_x + 4, magnitude, causal) }
    }
}

#[verifier::opaque]
pub open spec fn ht_sigprop_stripes(
    state: HtSigpropPlaneState, width: u16, height: u16, stripe_y: nat, magnitude: (Seq<u64>, nat), causal: bool,
) -> HtSigpropPlaneResult
    recommends width <= 4096, height <= 4096, stripe_y <= height as nat + 3,
    decreases (height as nat + 3 - stripe_y) / 4,
{
    if stripe_y >= height { (Ok(()), state) } else {
        let step = ht_sigprop_patches(state, width, stripe_y as u16, ht_stripe_rows(height as nat, stripe_y), 0, magnitude, causal);
        match step.0 { Err(_) => step, Ok(()) => ht_sigprop_stripes(step.1, width, height, stripe_y + 4, magnitude, causal) }
    }
}

pub open spec fn ht_refinement_plane_finish(result: HtSigpropPlaneResult, encoded: Seq<u8>)
    -> Result<(Tier1RawPlane, crate::tier1::Tier1PassStatistics), crate::DecodeError> {
    match result.0 {
        Err(error) => Err(error),
        Ok(()) => match ht_refinement_finish(result.1.core.readers, encoded) {
            Err(error) => Err(error), Ok(()) => Ok((result.1.core.output, result.1.core.statistics)),
        },
    }
}

pub open spec fn ht_refinement_significance(
    state: HtRefinementCoreState, encoded: Seq<u8>, bitplanes: u16, causal: bool,
) -> Result<(Tier1RawPlane, crate::tier1::Tier1PassStatistics), crate::DecodeError> {
    match ht_significance_magnitude(bitplanes) {
        Err(error) => Err(error),
        Ok(magnitude) => match tier1_duplicate_plane(state.output) {
            Err(error) => Err(error),
            Ok(discovery) => ht_refinement_plane_finish(ht_sigprop_stripes(
                HtSigpropPlaneState { core: state, discovery },
                state.output.width, state.output.height, 0, magnitude, causal), encoded),
        },
    }
}

pub open spec fn ht_refinement_plane(
    cleanup: Tier1RawPlane, segment: HtRefinementSegmentState, passes: u8, bitplanes: u16, causal: bool, maximum_work: u64,
) -> Result<(Tier1RawPlane, crate::tier1::Tier1PassStatistics), crate::DecodeError>
    recommends segment.encoded.len() <= usize::MAX,
{
    if passes == 0 || passes > 2 || bitplanes < 2 { Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)) }
    else {
        match tier1_duplicate_plane(cleanup) {
            Err(error) => Err(error),
            Ok(output) => match ht_refinement_readers_initial(segment.significance_propagation.bytes,
                segment.magnitude_refinement.bytes, segment.encoded.len() as usize, passes == 2, maximum_work) {
                Err(error) => Err(error),
                Ok(readers) => {
                    let state = HtRefinementCoreState { output, readers,
                        cleanup_significant: ht_cleanup_significance(cleanup.coefficients), statistics: tier1_statistics_zero() };
                    if passes == 2 {
                        match ht_refinement_delta(bitplanes) {
                            Err(error) => Err(error),
                            Ok(delta) => {
                                let refined = ht_magref_stripes(state, output.width, output.height, 0, delta.1, bitplanes);
                                match refined.0 { Err(error) => Err(error),
                                    Ok(()) => ht_refinement_significance(refined.1, segment.encoded, bitplanes, causal) }
                            },
                        }
                    } else { ht_refinement_significance(state, segment.encoded, bitplanes, causal) }
                },
            },
        }
    }
}

pub open spec fn ht_neighbor_update(
    state: crate::tier1::NeighborSummary, kind: u8, negative: bool,
) -> (Result<(), crate::DecodeError>, crate::tier1::NeighborSummary) {
    let sign = if negative { -1int } else { 1int };
    if kind == 0 {
        if state.horizontal_significant == u8::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state) }
        else {
            let state = crate::tier1::NeighborSummary { horizontal_significant: (state.horizontal_significant + 1) as u8, ..state };
            let balance = state.horizontal_sign_balance as int + sign;
            if balance < i8::MIN || balance > i8::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state) }
            else { (Ok(()), crate::tier1::NeighborSummary { horizontal_sign_balance: balance as i8, ..state }) }
        }
    } else if kind == 1 {
        if state.vertical_significant == u8::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state) }
        else {
            let state = crate::tier1::NeighborSummary { vertical_significant: (state.vertical_significant + 1) as u8, ..state };
            let balance = state.vertical_sign_balance as int + sign;
            if balance < i8::MIN || balance > i8::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state) }
            else { (Ok(()), crate::tier1::NeighborSummary { vertical_sign_balance: balance as i8, ..state }) }
        }
    } else if state.diagonal_significant == u8::MAX {
        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
    } else { (Ok(()), crate::tier1::NeighborSummary { diagonal_significant: (state.diagonal_significant + 1) as u8, ..state }) }
}

pub open spec fn ht_accumulate_neighbor(
    plane: Tier1RawPlane, x: u16, y: u16, dx: i8, dy: i8, kind: u8, summary: crate::tier1::NeighborSummary,
) -> (Result<(), crate::DecodeError>, crate::tier1::NeighborSummary) {
    let nx = x as int + dx as int;
    let ny = y as int + dy as int;
    if nx < 0 || ny < 0 || nx >= plane.width || ny >= plane.height { (Ok(()), summary) }
    else {
        match tier1_raw_plane_coefficient(plane, nx as u16, ny as u16) {
            Err(error) => (Err(error), summary),
            Ok(coefficient) => if coefficient.significant { ht_neighbor_update(summary, kind, coefficient.negative) }
                else { (Ok(()), summary) },
        }
    }
}

pub open spec fn ht_neighbor_then(
    previous: (Result<(), crate::DecodeError>, crate::tier1::NeighborSummary),
    plane: Tier1RawPlane, x: u16, y: u16, dx: i8, dy: i8, kind: u8,
) -> (Result<(), crate::DecodeError>, crate::tier1::NeighborSummary) {
    match previous.0 { Err(_) => previous, Ok(()) => ht_accumulate_neighbor(plane, x, y, dx, dy, kind, previous.1) }
}

pub open spec fn ht_neighbors(plane: Tier1RawPlane, x: u16, y: u16, vertical_causal: bool)
    -> Result<crate::tier1::NeighborSummary, crate::DecodeError> {
    match tier1_raw_plane_index(plane, x, y) {
        Err(error) => Err(error),
        Ok(_) => {
            let state = ht_accumulate_neighbor(plane, x, y, (-1int) as i8, 0, 0, ht_neighbor_zero());
            let state = ht_neighbor_then(state, plane, x, y, 1, 0, 0);
            let state = ht_neighbor_then(state, plane, x, y, 0, (-1int) as i8, 1);
            let state = ht_neighbor_then(state, plane, x, y, (-1int) as i8, (-1int) as i8, 2);
            let state = ht_neighbor_then(state, plane, x, y, 1, (-1int) as i8, 2);
            let state = if vertical_causal && y % 4 == 3 { state } else {
                let state = ht_neighbor_then(state, plane, x, y, 0, 1, 1);
                let state = ht_neighbor_then(state, plane, x, y, (-1int) as i8, 1, 2);
                ht_neighbor_then(state, plane, x, y, 1, 1, 2)
            };
            match state.0 { Err(error) => Err(error), Ok(()) => Ok(state.1) }
        },
    }
}

pub struct Tier1DecoderState {
    pub plane: Tier1RawPlane,
    pub orientation: crate::tier1::SubbandOrientation,
    pub style: crate::tier1::Tier1CodingStyle,
    pub maximum_passes: u32,
    pub next_pass: u32,
    pub bitplane: u16,
    pub zero_bitplanes: Option<u16>,
    pub bitplane_state: Option<crate::tier1::Tier1BitplaneState>,
    pub mq_context_state: Option<crate::mq::MqContextState>,
    pub reset_contexts_before_mq: bool,
    pub decoded_segments: u32,
    pub decoded_bytes: u64,
    pub legacy_segment: Option<Tier1LegacyFrame>,
    pub legacy_decode_work: u64,
    pub ht_cleanup_plane: Option<Tier1RawPlane>,
    pub ht_refinement_data: Seq<u8>,
    pub ht_refinement_passes: u8,
    pub ht_decoded_bitplanes: u16,
    pub ht_current_set_has_cleanup: bool,
    pub ht_sets_seen: u16,
    pub allow_multiple_ht_sets: bool,
    pub ht_magnitude_bound: u8,
}

pub open spec fn tier1_duplicate_plane(plane: Tier1RawPlane) -> Result<Tier1RawPlane, crate::DecodeError> {
    match tier1_empty_plane(plane.width, plane.height) {
        Err(error) => Err(error),
        Ok(blank) => if blank.coefficients.len() != plane.coefficients.len() {
            Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
        } else { Ok(plane) },
    }
}

pub open spec fn ht_cleanup_prepare(
    plane: Tier1RawPlane, cleanup: HtCleanupSegmentState, maximum_passes: u32,
    maximum_u: u8, skipped_bitplanes: u16, maximum_work: u64,
) -> Result<(Tier1RawPlane, Tier1RawPlane, crate::tier1::Tier1PassStatistics, u16), crate::DecodeError> {
    match ht_cleanup_segment_metadata(cleanup, plane.width, plane.height, maximum_u, maximum_work) {
        Err(error) => Err(error),
        Ok(metadata) => {
            let significant = (maximum_passes as nat + 2) / 3;
            if maximum_passes as nat + 2 > u32::MAX || significant > u16::MAX
                || significant < skipped_bitplanes {
                Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
            } else {
                let bitplanes = (significant - skipped_bitplanes) as u16;
                match ht_cleanup_plane(plane, cleanup, metadata, maximum_u, bitplanes, maximum_work) {
                    Err(error) => Err(error),
                    Ok((output, statistics)) => match tier1_duplicate_plane(output) {
                        Err(error) => Err(error),
                        Ok(duplicate) => Ok((output, duplicate, statistics, bitplanes)),
                    },
                }
            }
        },
    }
}

pub type HtDecoderResult = (Result<crate::tier1::Tier1SegmentDecode, crate::DecodeError>, Tier1DecoderState);

pub open spec fn ht_store_coefficient_symbols(
    state: Tier1DecoderState, output: Tier1RawPlane, cleanup: Tier1RawPlane,
    statistics: crate::tier1::Tier1PassStatistics, bitplanes: u16,
    passes: u32, bytes: u32, has_refinement: bool,
) -> HtDecoderResult {
    let state = Tier1DecoderState {
        plane: output, ht_cleanup_plane: Some(cleanup),
        ht_current_set_has_cleanup: true, ht_decoded_bitplanes: bitplanes, ..state
    };
    if state.ht_sets_seen == u16::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state) }
    else {
        let state = Tier1DecoderState { ht_sets_seen: (state.ht_sets_seen + 1) as u16, ..state };
        if state.next_pass as nat + passes as nat > u32::MAX {
            (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
        } else {
            let state = Tier1DecoderState { next_pass: (state.next_pass + passes) as u32, ..state };
            let segments = if has_refinement { 2nat } else { 1nat };
            if state.decoded_segments as nat + segments > u32::MAX {
                (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
            } else {
                let state = Tier1DecoderState { decoded_segments: (state.decoded_segments + segments) as u32, ..state };
                if state.decoded_bytes as nat + bytes as nat > u64::MAX {
                    (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
                } else {
                    let state = Tier1DecoderState { decoded_bytes: (state.decoded_bytes + bytes) as u64, ..state };
                    (Ok(crate::tier1::Tier1SegmentDecode { passes, bytes, statistics }), state)
                }
            }
        }
    }
}

pub open spec fn ht_cleanup_symbols_only(
    state: Tier1DecoderState, segments: HtBlockSegmentsState, passes: u32,
    maximum_u: u8, skipped_bitplanes: u16, maximum_work: u64,
) -> HtDecoderResult {
    match ht_cleanup_prepare(state.plane, segments.cleanup, state.maximum_passes, maximum_u, skipped_bitplanes, maximum_work) {
        Err(error) => (Err(error), state),
        Ok((output, cleanup, statistics, bitplanes)) => ht_store_coefficient_symbols(
            Tier1DecoderState { ht_refinement_data: Seq::empty(), ht_refinement_passes: 0, ..state },
            output, cleanup, statistics, bitplanes, passes, segments.bytes, false),
    }
}

pub open spec fn ht_coefficient_symbols(
    state: Tier1DecoderState, segments: HtBlockSegmentsState, passes: u32,
    maximum_u: u8, skipped_bitplanes: u16, maximum_work: u64,
) -> HtDecoderResult {
    match ht_cleanup_prepare(state.plane, segments.cleanup, state.maximum_passes, maximum_u, skipped_bitplanes, maximum_work) {
        Err(error) => (Err(error), state),
        Ok((output, cleanup, statistics, bitplanes)) => {
            let state = Tier1DecoderState { ht_refinement_data: Seq::empty(), ht_refinement_passes: 0, ..state };
            match segments.refinement {
                None => ht_store_coefficient_symbols(state, output, cleanup, statistics, bitplanes, passes, segments.bytes, false),
                Some(segment) => {
                    let refinement_passes = if passes == 0 { 0nat } else { (passes - 1) as nat };
                    if refinement_passes > u8::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state) }
                    else {
                        match ht_refinement_plane(cleanup, segment, refinement_passes as u8, bitplanes,
                            state.style.vertical_causal_context, maximum_work) {
                            Err(error) => (Err(error), state),
                            Ok((refined, refinement_statistics)) => {
                                let merged = tier1_statistics_merge(statistics, refinement_statistics);
                                match merged.0 {
                                    Err(error) => (Err(error), state),
                                    Ok(()) => match ht_copy_refinement_bytes(segment.encoded) {
                                        Err(error) => (Err(error), state),
                                        Ok(data) => ht_store_coefficient_symbols(Tier1DecoderState {
                                            ht_refinement_data: data, ht_refinement_passes: refinement_passes as u8, ..state },
                                            refined, cleanup, merged.1, bitplanes, passes, segments.bytes, true),
                                    },
                                }
                            },
                        }
                    }
                },
            }
        },
    }
}

pub open spec fn ht_significant_bitplanes(maximum_passes: u32) -> Result<u16, crate::DecodeError> {
    let planes = (maximum_passes as nat + 2) / 3;
    if maximum_passes as nat + 2 > u32::MAX || planes > u16::MAX {
        Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
    } else { Ok(planes as u16) }
}

pub open spec fn ht_block_coding_enabled(coding: crate::tier1::Tier1BlockCoding) -> bool {
    !matches!(coding, crate::tier1::Tier1BlockCoding::Legacy)
}

pub open spec fn ht_segments_after_parse(
    state: Tier1DecoderState, segments: HtBlockSegmentsState, passes: u32, refinement_passes: u8,
    maximum_bytes: u64, maximum_work: u64,
) -> HtDecoderResult {
    let next_bytes = state.decoded_bytes as nat + segments.bytes as nat;
    let coefficient_work = (state.plane.coefficients.len() as u64) as nat * (refinement_passes as nat + 1);
    let work = coefficient_work + segments.bytes as nat + passes as nat;
    if next_bytes > u64::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state) }
    else if next_bytes > maximum_bytes {
        (Err(crate::DecodeError::LimitExceeded { resource: crate::Resource::CodeBlockBytes,
            limit: maximum_bytes, actual: next_bytes as u64 }), state)
    } else if work > u64::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state) }
    else if work > maximum_work {
        (Err(crate::DecodeError::LimitExceeded { resource: crate::Resource::WorkUnits,
            limit: maximum_work, actual: work as u64 }), state)
    } else {
        let skipped = state.next_pass / 3;
        if skipped > u16::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state) }
        else {
            let missing = match state.zero_bitplanes { None => 0nat, Some(value) => value as nat };
            let maximum_u = part15_cleanup_u_bound(missing, skipped as nat, state.ht_magnitude_bound as nat) as u8;
            ht_coefficient_symbols(state, segments, passes, maximum_u, skipped as u16, maximum_work)
        }
    }
}

pub open spec fn ht_decode_segments(
    state: Tier1DecoderState, cleanup: Seq<u8>, cleanup_passes: u8, refinement: Option<Seq<u8>>,
    refinement_passes: u8, maximum_passes: u32, maximum_bytes: u64, maximum_work: u64,
) -> HtDecoderResult {
    if !ht_block_coding_enabled(state.style.block_coding) || state.next_pass % 3 != 0
        || cleanup_passes != 1 || refinement_passes > 2 || (refinement_passes == 0) != refinement.is_none() {
        (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state)
    } else if state.ht_sets_seen > 0 && !state.allow_multiple_ht_sets {
        (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state)
    } else {
        let passes = cleanup_passes as nat + refinement_passes as nat;
        let next = state.next_pass as nat + passes;
        if next > u32::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state) }
        else if next > state.maximum_passes { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state) }
        else if next > maximum_passes {
            (Err(crate::DecodeError::LimitExceeded { resource: crate::Resource::CodeBlockPasses,
                limit: maximum_passes as u64, actual: next as u64 }), state)
        } else {
            match ht_significant_bitplanes(state.maximum_passes) {
                Err(error) => (Err(error), state),
                Ok(bitplanes) => if refinement_passes > 0 && bitplanes < 2 {
                    (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state)
                } else {
                    match ht_parse_block_segments(cleanup, refinement, maximum_bytes) {
                        Err(error) => (Err(error), state),
                        Ok(segments) => ht_segments_after_parse(state, segments, passes as u32,
                            refinement_passes, maximum_bytes, maximum_work),
                    }
                },
            }
        }
    }
}

pub type HtPlaneResult = (Result<(), crate::DecodeError>, HtPlaneState);

pub open spec fn ht_plane_sample_store(
    state: HtPlaneState, x: u16, y: u16, sample: u8, column: nat,
    value: nat, negative: bool, exponent: u8,
) -> HtPlaneResult
    recommends column + 1 < state.current.len(),
{
    let store = tier1_mark_coefficient(state.output, x, y, tier1_value_limbs(value), value, negative);
    let state = HtPlaneState { output: store.1, ..state };
    match store.0 {
        Err(error) => (Err(error), state),
        Ok(()) => {
            let count = tier1_statistics_increment(state.statistics, 1, 1);
            let state = HtPlaneState { statistics: count.1, ..state };
            match count.0 {
                Err(error) => (Err(error), state),
                Ok(()) => {
                    let count = tier1_statistics_increment(state.statistics, 2, 1);
                    let state = HtPlaneState { statistics: count.1, ..state };
                    match count.0 {
                        Err(error) => (Err(error), state),
                        Ok(()) => (Ok(()), HtPlaneState {
                            current: if sample == 1 {
                                state.current.update(column as int,
                                    if state.current[column as int] > exponent { state.current[column as int] } else { exponent })
                            } else if sample == 3 { state.current.update((column + 1) as int, exponent) }
                            else { state.current }, ..state
                        }),
                    }
                },
            }
        },
    }
}

pub open spec fn ht_plane_sample(
    state: HtPlaneState, quad: crate::tier1::HtQuadMetadata, width: u16, height: u16,
    column: nat, completed_u: u8, bitplanes: u16, sample: u8,
) -> HtPlaneResult
    recommends sample < 4, column + 1 < state.current.len(),
{
    let x = quad.x as nat + (sample >> 1) as nat;
    let y = quad.y as nat + (sample & 1u8) as nat;
    if x > u16::MAX || y > u16::MAX {
        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
    } else if x >= width || y >= height { (Ok(()), state) }
    else {
        let scanned = tier1_statistics_increment(state.statistics, 0, 1);
        let state = HtPlaneState { statistics: scanned.1, ..state };
        match scanned.0 {
            Err(error) => (Err(error), state),
            Ok(()) => if quad.significance & (1u8 << sample) == 0 { (Ok(()), state) } else {
                let decoded = ht_reconstruct_numeric_magnitude(state.reader, completed_u,
                    quad.embedded_bit_mask & (1u8 << sample) != 0,
                    quad.embedded_magnitude & (1u8 << sample) != 0, bitplanes);
                let state = HtPlaneState { reader: decoded.1, ..state };
                match decoded.0 {
                    Err(error) => (Err(error), state),
                    Ok((value, negative, exponent)) =>
                        ht_plane_sample_store(state, x as u16, y as u16, sample, column, value, negative, exponent),
                }
            },
        }
    }
}

#[verifier::opaque]
pub open spec fn ht_plane_samples(
    state: HtPlaneState, quad: crate::tier1::HtQuadMetadata, width: u16, height: u16,
    column: nat, completed_u: u8, bitplanes: u16, count: nat,
) -> HtPlaneResult
    recommends count <= 4, column + 1 < state.current.len(),
    decreases count,
{
    if count == 0 { (Ok(()), state) } else {
        let prefix = ht_plane_samples(state, quad, width, height, column, completed_u, bitplanes, (count - 1) as nat);
        match prefix.0 { Err(_) => prefix, Ok(()) =>
            ht_plane_sample(prefix.1, quad, width, height, column, completed_u, bitplanes, (count - 1) as u8) }
    }
}

pub open spec fn ht_plane_quad_finish(result: HtPlaneResult, quads_per_row: nat, column: nat) -> HtPlaneResult {
    match result.0 {
        Err(_) => result,
        Ok(()) => (Ok(()), if column + 1 == quads_per_row {
            HtPlaneState { previous: result.1.current, current: result.1.previous, ..result.1 }
        } else { result.1 }),
    }
}

pub open spec fn ht_plane_quad(
    state: HtPlaneState, quad: crate::tier1::HtQuadMetadata, width: u16, height: u16,
    maximum_u: u8, bitplanes: u16, quads_per_row: nat, index: nat,
) -> HtPlaneResult
    recommends 0 < quads_per_row <= 32768, state.current.len() == quads_per_row + 1,
        state.previous.len() == quads_per_row + 1,
{
    let column = index % quads_per_row;
    let state = HtPlaneState { current: if column == 0 { Seq::new(state.current.len(), |i: int| 0u8) }
        else { state.current }, ..state };
    let completed = if quad.initial_row {
        if quad.u_base > maximum_u { Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)) } else { Ok(quad.u_base) }
    } else { ht_complete_kappa(quad.u_base, quad.significance, state.previous[column as int],
        state.previous[(column + 1) as int], maximum_u) };
    match completed {
        Err(error) => (Err(error), state),
        Ok(value) => {
            let result = ht_plane_samples(state, quad, width, height, column, value, bitplanes, 4);
            ht_plane_quad_finish(result, quads_per_row, column)
        },
    }
}

#[verifier::opaque]
pub open spec fn ht_plane_quads(
    state: HtPlaneState, quads: Seq<crate::tier1::HtQuadMetadata>, width: u16, height: u16,
    maximum_u: u8, bitplanes: u16, quads_per_row: nat, count: nat,
) -> HtPlaneResult
    recommends count <= quads.len(), 0 < quads_per_row <= 32768,
        state.current.len() == quads_per_row + 1, state.previous.len() == quads_per_row + 1,
    decreases count,
{
    if count == 0 { (Ok(()), state) } else {
        let prefix = ht_plane_quads(state, quads, width, height, maximum_u, bitplanes, quads_per_row, (count - 1) as nat);
        match prefix.0 { Err(_) => prefix, Ok(()) =>
            ht_plane_quad(prefix.1, quads[(count - 1) as int], width, height, maximum_u, bitplanes, quads_per_row, (count - 1) as nat) }
    }
}

pub open spec fn ht_plane_finish(state: HtPlaneState) -> HtPlaneResult {
    let finished = ht_magsgn_finish(state.reader);
    (finished.0, HtPlaneState { reader: finished.1, ..state })
}

pub open spec fn ht_plane_output(decoded: HtPlaneResult)
    -> Result<(Tier1RawPlane, crate::tier1::Tier1PassStatistics), crate::DecodeError> {
    match decoded.0 {
        Err(error) => Err(error),
        Ok(()) => {
            let finished = ht_plane_finish(decoded.1);
            match finished.0 { Err(error) => Err(error),
                Ok(()) => Ok((finished.1.output, finished.1.statistics)) }
        },
    }
}

pub open spec fn ht_cleanup_plane(
    plane: Tier1RawPlane, cleanup: HtCleanupSegmentState, metadata: HtMetadataOutput,
    maximum_u: u8, bitplanes: u16, maximum_work: u64,
) -> Result<(Tier1RawPlane, crate::tier1::Tier1PassStatistics), crate::DecodeError> {
    if metadata.width == 0 { Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)) }
    else {
        match tier1_empty_plane(plane.width, plane.height) {
            Err(error) => Err(error),
            Ok(output) => match ht_magsgn_initial(cleanup.mag_sgn.bytes, maximum_work) {
                Err(error) => Err(error),
                Ok(reader) => {
                    let quads_per_row = (metadata.width as nat + 1) / 2;
                    if metadata.width as nat + 1 > usize::MAX || quads_per_row + 1 > usize::MAX {
                        Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
                    } else if quads_per_row == 0 {
                        Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
                    } else {
                    let state = HtPlaneState { output, reader,
                        previous: Seq::new(quads_per_row + 1, |i: int| 0u8),
                        current: Seq::new(quads_per_row + 1, |i: int| 0u8),
                        statistics: tier1_statistics_zero(),
                    };
                    let decoded = ht_plane_quads(state, metadata.quads, metadata.width, metadata.height,
                        maximum_u, bitplanes, quads_per_row, metadata.quads.len());
                    ht_plane_output(decoded)
                    }
                },
            },
        }
    }
}

#[verifier::opaque]
pub open spec fn ht_numeric_payload(
    reader: HtMagSgnState, remaining: nat, index: nat, value: nat, negative: bool, exponent: u8,
) -> HtNumericMagnitudeResult
    recommends index + remaining <= 75, value < part1_magnitude_capacity(),
    decreases remaining,
{
    if remaining == 0 { (Ok((value, negative, exponent)), reader) } else {
        let read = ht_magsgn_read(reader);
        match read.0 {
            Err(error) => (Err(error), read.1),
            Ok(bit) => if bit {
                let changed = value + pow2(index);
                if changed >= part1_magnitude_capacity() {
                    (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), read.1)
                } else {
                    ht_numeric_payload(read.1, (remaining - 1) as nat, index + 1, changed,
                        negative || index == 0, (index + 1) as u8)
                }
            } else {
                ht_numeric_payload(read.1, (remaining - 1) as nat, index + 1, value, negative, exponent)
            },
        }
    }
}

pub open spec fn ht_numeric_center(
    base: nat, negative: bool, exponent: u8, magnitude_bits: u8, embedded_value: bool, decoded_bitplanes: u16,
) -> Result<(nat, bool, u8), crate::DecodeError>
    recommends magnitude_bits <= 75, 1 <= decoded_bitplanes <= 292,
{
    let embedded = if embedded_value { base + pow2(magnitude_bits as nat) } else { base };
    let exp = if embedded_value { (magnitude_bits + 1) as u8 } else { exponent };
    if embedded >= part1_magnitude_capacity() {
        Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
    } else {
        let odd = if embedded % 2 == 0 { embedded + 1 } else { embedded };
        let exp = if embedded % 2 == 0 && exp < 1 { 1u8 } else { exp };
        let centered = odd + 2;
        let scaled = centered * pow2((decoded_bitplanes - 1) as nat);
        if odd >= part1_magnitude_capacity() || centered >= part1_magnitude_capacity()
            || scaled >= part1_magnitude_capacity() {
            Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
        } else if scaled / 2 == 0 {
            Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
        } else { Ok(((scaled / 2) as nat, negative, exp)) }
    }
}

pub open spec fn ht_numeric_finish(
    payload: HtNumericMagnitudeResult, magnitude_bits: u8, embedded_value: bool, decoded_bitplanes: u16,
) -> HtNumericMagnitudeResult
    recommends magnitude_bits <= 75, 1 <= decoded_bitplanes <= 292,
{
    (match payload.0 {
        Err(error) => Err(error),
        Ok((base, negative, exponent)) => ht_numeric_center(base, negative, exponent, magnitude_bits, embedded_value, decoded_bitplanes),
    }, payload.1)
}

pub open spec fn ht_reconstruct_numeric_magnitude(
    reader: HtMagSgnState, completed_u: u8, embedded_bit: bool, embedded_value: bool, decoded_bitplanes: u16,
) -> HtNumericMagnitudeResult {
    if completed_u == 0 || completed_u > 75 || decoded_bitplanes == 0 || decoded_bitplanes > 292 {
        (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), reader)
    } else {
        let bits = ht_magnitude_payload_bits(completed_u as nat, embedded_bit) as u8;
        ht_numeric_finish(ht_numeric_payload(reader, bits as nat, 0, 0, false, 0),
            bits, embedded_value, decoded_bitplanes)
    }
}

pub struct HtMetadataWordsState {
    pub readers: HtCleanupReaderState,
    pub current: Seq<u8>,
    pub left: u8,
    pub words: Seq<u16>,
    pub contexts: Seq<u8>,
}

pub type HtMetadataWordsResult = (
    Result<(Seq<u16>, Seq<u8>), crate::DecodeError>, HtCleanupReaderState, Seq<u8>, u8,
);

pub open spec fn ht_metadata_context(previous: Seq<u8>, left: u8, index: nat, initial: bool) -> u8
    recommends index < previous.len(),
{
    if initial { left } else {
        ht_noninitial_vlc_context(left,
            if index > 0 { previous[(index - 1) as int] } else { 0 },
            previous[index as int],
            if index + 1 < previous.len() { previous[(index + 1) as int] } else { 0 })
    }
}

pub open spec fn ht_metadata_left(significance: u8, initial: bool) -> u8 {
    if initial { (significance & 1u8) | ((significance & 0x0eu8) >> 1u8) }
    else if significance & 0x0cu8 != 0 { 2 } else { 0 }
}

#[verifier::opaque]
pub open spec fn ht_metadata_words_run(
    state: HtMetadataWordsState, previous: Seq<u8>, pair: nat, initial: bool, member: nat,
) -> (Result<(), crate::DecodeError>, HtMetadataWordsState)
    recommends member <= 2, state.words.len() == 2, state.contexts.len() == 2,
        state.current.len() == previous.len(),
    decreases 2nat - member,
{
    if member >= 2 || pair + member >= previous.len() { (Ok(()), state) } else {
        let index = pair + member;
        let context = ht_metadata_context(previous, state.left, index, initial);
        let read = ht_cleanup_quad_word(state.readers, crate::tier1::ht_vlc_table_view(initial), context);
        let consumed = HtMetadataWordsState { readers: read.1, ..state };
        match read.0 {
            Err(error) => (Err(error), consumed),
            Ok(word) => {
                let significance = ((word >> 4u16) & 0x0fu16) as u8;
                ht_metadata_words_run(HtMetadataWordsState {
                    current: state.current.update(index as int, significance),
                    left: ht_metadata_left(significance, initial),
                    words: state.words.update(member as int, word),
                    contexts: state.contexts.update(member as int, context), ..consumed
                }, previous, pair, initial, member + 1)
            },
        }
    }
}

pub open spec fn ht_metadata_words_finish(result: (Result<(), crate::DecodeError>, HtMetadataWordsState))
    -> HtMetadataWordsResult
{
    (match result.0 { Err(error) => Err(error), Ok(()) => Ok((result.1.words, result.1.contexts)) },
        result.1.readers, result.1.current, result.1.left)
}

pub open spec fn ht_metadata_words(
    readers: HtCleanupReaderState, current: Seq<u8>, left: u8,
    previous: Seq<u8>, pair: nat, initial: bool,
) -> HtMetadataWordsResult
    recommends current.len() == previous.len(),
{
    ht_metadata_words_finish(ht_metadata_words_run(HtMetadataWordsState {
        readers, current, left, words: Seq::new(2, |i: int| 0u16), contexts: Seq::new(2, |i: int| 0u8),
    }, previous, pair, initial, 0))
}

pub struct HtMetadataQuadInput {
    pub index: usize,
    pub row: usize,
    pub width: u16,
    pub height: u16,
    pub maximum_u: u8,
    pub word: u16,
    pub context: u8,
    pub significance: u8,
    pub u_value: u8,
    pub initial: bool,
}

#[verifier::opaque]
pub open spec fn ht_metadata_samples(
    significant: u16, significance: u8, x: u16, y: u16, width: u16, height: u16, sample: u8,
) -> (Result<(), crate::DecodeError>, u16)
    recommends sample <= 4, x <= 65534, y <= 65534,
    decreases 4nat - sample,
{
    if sample >= 4 { (Ok(()), significant) }
    else if significance & (1u8 << sample) == 0 {
        ht_metadata_samples(significant, significance, x, y, width, height, (sample + 1) as u8)
    } else if x as nat + (sample >> 1u8) as nat >= width
        || y as nat + (sample & 1u8) as nat >= height {
        (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), significant)
    } else if significant == u16::MAX {
        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), significant)
    } else {
        ht_metadata_samples((significant + 1) as u16, significance, x, y, width, height, (sample + 1) as u8)
    }
}

pub type HtMetadataEmitResult = (
    Result<(), crate::DecodeError>, HtCleanupReaderState, Seq<crate::tier1::HtQuadMetadata>, u16,
);

pub open spec fn ht_metadata_emit_after_samples(
    result: (Result<(), crate::DecodeError>, u16),
    readers: HtCleanupReaderState, quads: Seq<crate::tier1::HtQuadMetadata>, input: HtMetadataQuadInput,
) -> HtMetadataEmitResult {
    match result.0 {
        Err(error) => (Err(error), readers, quads, result.1),
        Ok(()) => {
            let quad = crate::tier1::HtQuadMetadata {
                x: (input.index * 2) as u16, y: (input.row * 2) as u16,
                context: input.context, initial_row: input.initial,
                significance: input.significance,
                embedded_magnitude: ((input.word >> 8u16) & 0x0fu16) as u8,
                embedded_bit_mask: ((input.word >> 12u16) & 0x0fu16) as u8,
                u_offset: input.word & 8u16 != 0, u_base: input.u_value,
                codeword_bits: (input.word & 7u16) as u8,
            };
            let tick = ht_cleanup_reader_tick(readers);
            (tick.0, tick.1, quads.push(quad), result.1)
        },
    }
}

pub open spec fn ht_metadata_emit(
    readers: HtCleanupReaderState, quads: Seq<crate::tier1::HtQuadMetadata>,
    significant: u16, input: HtMetadataQuadInput,
) -> HtMetadataEmitResult
    recommends input.index < 32768, input.row < 32768,
{
    let x = (input.index * 2) as u16;
    let y = (input.row * 2) as u16;
    if input.u_value > input.maximum_u
        || !ht_quad_origin_valid(x as nat, y as nat, input.width as nat, input.height as nat) {
        (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), readers, quads, significant)
    } else {
        ht_metadata_emit_after_samples(ht_metadata_samples(significant, input.significance, x, y,
            input.width, input.height, 0), readers, quads, input)
    }
}

pub struct HtMetadataPairState {
    pub readers: HtCleanupReaderState,
    pub quads: Seq<crate::tier1::HtQuadMetadata>,
    pub current: Seq<u8>,
    pub significant: u16,
    pub left: u8,
}

pub open spec fn ht_metadata_u_values(readers: HtCleanupReaderState, words: Seq<u16>, initial: bool)
    -> (Result<[u8; 2], crate::DecodeError>, HtCleanupReaderState)
    recommends words.len() == 2,
{
    let mode = (if words[0] & 8u16 != 0 { 1u8 } else { 0u8 })
        | (if words[1] & 8u16 != 0 { 2u8 } else { 0u8 });
    let event = if initial && mode == 3 { ht_cleanup_mel_event(readers) } else { (Ok(false), readers) };
    match event.0 {
        Err(error) => (Err(error), event.1),
        Ok(four) => {
            let selected = if four { 4u8 } else { mode };
            if initial { ht_uvlc_initial(event.1, selected) } else { ht_uvlc_non_initial(event.1, selected) }
        },
    }
}

#[verifier::opaque]
pub open spec fn ht_metadata_emits_run(
    state: HtMetadataPairState, words: Seq<u16>, contexts: Seq<u8>, values: [u8; 2],
    pair: nat, row: nat, width: u16, height: u16, maximum_u: u8, initial: bool, member: nat,
) -> (Result<(), crate::DecodeError>, HtMetadataPairState)
    recommends member <= 2, words.len() == 2, contexts.len() == 2, pair < 32768, row < 32768,
        state.current.len() <= 32768,
    decreases 2nat - member,
{
    if member >= 2 || pair + member >= state.current.len() { (Ok(()), state) } else {
        let index = pair + member;
        let emit = ht_metadata_emit(state.readers, state.quads, state.significant, HtMetadataQuadInput {
            index: index as usize, row: row as usize, width, height, maximum_u,
            word: words[member as int], context: contexts[member as int],
            significance: state.current[index as int], u_value: values[member as int], initial,
        });
        let changed = HtMetadataPairState {
            readers: emit.1, quads: emit.2, significant: emit.3, ..state
        };
        match emit.0 {
            Err(error) => (Err(error), changed),
            Ok(()) => ht_metadata_emits_run(changed, words, contexts, values,
                pair, row, width, height, maximum_u, initial, member + 1),
        }
    }
}

pub open spec fn ht_metadata_pair_after_words(
    words: HtMetadataWordsResult, quads: Seq<crate::tier1::HtQuadMetadata>, significant: u16,
    pair: nat, row: nat, width: u16, height: u16, maximum_u: u8, initial: bool,
) -> (Result<(), crate::DecodeError>, HtMetadataPairState)
    recommends pair < 32768, row < 32768, words.2.len() <= 32768,
{
    let changed = HtMetadataPairState { readers: words.1, quads, significant, current: words.2, left: words.3 };
    match words.0 {
        Err(error) => (Err(error), changed),
        Ok((cache, contexts)) => {
            let values = ht_metadata_u_values(changed.readers, cache, initial);
            let consumed = HtMetadataPairState { readers: values.1, ..changed };
            match values.0 {
                Err(error) => (Err(error), consumed),
                Ok(values) => ht_metadata_emits_run(consumed, cache, contexts, values,
                    pair, row, width, height, maximum_u, initial, 0),
            }
        },
    }
}

pub open spec fn ht_metadata_pair(
    state: HtMetadataPairState, previous: Seq<u8>,
    pair: nat, row: nat, width: u16, height: u16, maximum_u: u8, initial: bool,
) -> (Result<(), crate::DecodeError>, HtMetadataPairState)
    recommends pair < 32768, row < 32768, state.current.len() == previous.len() <= 32768,
{
    ht_metadata_pair_after_words(
        ht_metadata_words(state.readers, state.current, state.left, previous, pair, initial),
        state.quads, state.significant, pair, row, width, height, maximum_u, initial)
}

pub struct HtMetadataRowState {
    pub readers: HtCleanupReaderState,
    pub quads: Seq<crate::tier1::HtQuadMetadata>,
    pub previous: Seq<u8>,
    pub significant: u16,
}

#[verifier::opaque]
pub open spec fn ht_metadata_pairs(
    state: HtMetadataPairState, previous: Seq<u8>, row: nat,
    width: u16, height: u16, maximum_u: u8, count: nat,
) -> (Result<(), crate::DecodeError>, HtMetadataPairState)
    recommends 0 < previous.len() <= 32768, state.current.len() == previous.len(),
        row < 32768, count <= (previous.len() + 1) / 2,
    decreases count,
{
    if count == 0 { (Ok(()), state) } else {
        let prefix = ht_metadata_pairs(state, previous, row, width, height, maximum_u, (count - 1) as nat);
        match prefix.0 {
            Err(error) => (Err(error), prefix.1),
            Ok(()) => ht_metadata_pair(prefix.1, previous, 2 * ((count - 1) as nat), row, width, height, maximum_u, row == 0),
        }
    }
}

pub open spec fn ht_metadata_row_initial(state: HtMetadataRowState) -> HtMetadataPairState {
    HtMetadataPairState {
        readers: state.readers, quads: state.quads, significant: state.significant,
        current: Seq::new(state.previous.len(), |i: int| 0u8), left: 0,
    }
}

pub open spec fn ht_metadata_row_finish(
    result: (Result<(), crate::DecodeError>, HtMetadataPairState), previous: Seq<u8>,
) -> (Result<(), crate::DecodeError>, HtMetadataRowState) {
    (result.0, HtMetadataRowState {
        readers: result.1.readers, quads: result.1.quads, significant: result.1.significant,
        previous: if result.0.is_ok() { result.1.current } else { previous },
    })
}

pub open spec fn ht_metadata_row(
    state: HtMetadataRowState, row: nat, width: u16, height: u16, maximum_u: u8,
) -> (Result<(), crate::DecodeError>, HtMetadataRowState)
    recommends 0 < state.previous.len() <= 32768, row < 32768,
{
    ht_metadata_row_finish(ht_metadata_pairs(ht_metadata_row_initial(state), state.previous, row,
        width, height, maximum_u, (state.previous.len() + 1) / 2), state.previous)
}

#[verifier::opaque]
pub open spec fn ht_metadata_rows(
    state: HtMetadataRowState, width: u16, height: u16, maximum_u: u8, count: nat,
) -> (Result<(), crate::DecodeError>, HtMetadataRowState)
    recommends 0 < state.previous.len() <= 32768, count <= 32768,
    decreases count,
{
    if count == 0 { (Ok(()), state) } else {
        let prefix = ht_metadata_rows(state, width, height, maximum_u, (count - 1) as nat);
        match prefix.0 {
            Err(error) => (Err(error), prefix.1),
            Ok(()) => ht_metadata_row(prefix.1, (count - 1) as nat, width, height, maximum_u),
        }
    }
}

pub struct HtMetadataOutput {
    pub width: u16,
    pub height: u16,
    pub quads: Seq<crate::tier1::HtQuadMetadata>,
    pub significant_samples: u16,
    pub mel_bits: u32,
    pub vlc_bits: u32,
    pub mel_events: u32,
}

pub open spec fn ht_metadata_finish(
    result: (Result<(), crate::DecodeError>, HtMetadataRowState), width: u16, height: u16,
) -> Result<HtMetadataOutput, crate::DecodeError> {
    match result.0 {
        Err(error) => Err(error),
        Ok(()) => {
            let state = result.1;
            match ht_cleanup_readers_finish(state.readers) {
                Err(error) => Err(error),
                Ok(()) => if state.readers.mel.bits_read > u32::MAX || state.readers.vlc.bits_read > u32::MAX {
                    Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
                } else {
                    Ok(HtMetadataOutput {
                        width, height, quads: state.quads, significant_samples: state.significant,
                        mel_bits: state.readers.mel.bits_read as u32, vlc_bits: state.readers.vlc.bits_read as u32,
                        mel_events: state.readers.mel_events,
                    })
                },
            }
        },
    }
}

pub open spec fn ht_cleanup_segment_metadata(
    cleanup: HtCleanupSegmentState, width: u16, height: u16, maximum_u: u8, maximum_work: u64,
) -> Result<HtMetadataOutput, crate::DecodeError> {
    if width == 0 || height == 0 || maximum_u == 0 || maximum_u > 75 {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
    } else if width as nat * height as nat > u32::MAX {
        Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
    } else if width as nat * height as nat > 4096 {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
    } else {
        match ht_cleanup_readers_initial(cleanup.mel.bytes, cleanup.vlc.bytes, maximum_work) {
            Err(error) => Err(error),
            Ok(readers) => ht_metadata_finish(ht_metadata_rows(HtMetadataRowState {
                readers, quads: Seq::empty(), significant: 0,
                previous: Seq::new((width as nat + 1) / 2, |i: int| 0u8),
            }, width, height, maximum_u, (height as nat + 1) / 2), width, height),
        }
    }
}

pub open spec fn ht_decode_cleanup_metadata(
    cleanup: Seq<u8>, width: u16, height: u16, maximum_u: u8, maximum_bytes: u64, maximum_work: u64,
) -> Result<HtMetadataOutput, crate::DecodeError> {
    match ht_parse_block_segments(cleanup, None, maximum_bytes) {
        Err(error) => Err(error),
        Ok(segments) => ht_cleanup_segment_metadata(segments.cleanup, width, height, maximum_u, maximum_work),
    }
}

pub struct HtSubstreamState {
    pub bytes: Seq<u8>,
    pub offset: u32,
    pub forward: bool,
}

pub struct HtCleanupSegmentState {
    pub encoded: Seq<u8>,
    pub scup: u16,
    pub mag_sgn: HtSubstreamState,
    pub mel_vlc: HtSubstreamState,
    pub mel: HtSubstreamState,
    pub vlc: HtSubstreamState,
}

pub struct HtRefinementSegmentState {
    pub encoded: Seq<u8>,
    pub significance_propagation: HtSubstreamState,
    pub magnitude_refinement: HtSubstreamState,
}

pub struct HtBlockSegmentsState {
    pub cleanup: HtCleanupSegmentState,
    pub refinement: Option<HtRefinementSegmentState>,
    pub bytes: u32,
}

pub open spec fn ht_parse_block_segments(
    cleanup: Seq<u8>, refinement: Option<Seq<u8>>, maximum_bytes: u64,
) -> Result<HtBlockSegmentsState, crate::DecodeError> {
    let refinement_length = match refinement { None => 0nat, Some(bytes) => bytes.len() };
    if cleanup.len() < 2 || cleanup.len() > 65534 || refinement_length > 2046 {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
    } else {
        let total = cleanup.len() + refinement_length;
        if total > maximum_bytes {
            Err(crate::DecodeError::LimitExceeded {
                resource: crate::Resource::CodeBlockBytes, limit: maximum_bytes, actual: total as u64,
            })
        } else {
            let scup = ht_scup(cleanup[(cleanup.len() - 2) as int], cleanup[(cleanup.len() - 1) as int]);
            if scup < 2 || scup > 4079 || scup > cleanup.len() {
                Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
            } else {
                let start = (cleanup.len() - scup) as int;
                let offset = start as u32;
                let suffix = cleanup.subrange(start, cleanup.len() as int);
                Ok(HtBlockSegmentsState {
                    cleanup: HtCleanupSegmentState {
                        encoded: cleanup, scup: scup as u16,
                        mag_sgn: HtSubstreamState { bytes: cleanup.subrange(0, start), offset: 0, forward: true },
                        mel_vlc: HtSubstreamState { bytes: suffix, offset, forward: true },
                        mel: HtSubstreamState { bytes: cleanup.subrange(start, (cleanup.len() - 1) as int), offset, forward: true },
                        vlc: HtSubstreamState { bytes: suffix, offset, forward: false },
                    },
                    refinement: match refinement {
                        None => None,
                        Some(bytes) => Some(HtRefinementSegmentState {
                            encoded: bytes,
                            significance_propagation: HtSubstreamState { bytes, offset: cleanup.len() as u32, forward: true },
                            magnitude_refinement: HtSubstreamState { bytes, offset: cleanup.len() as u32, forward: false },
                        }),
                    },
                    bytes: total as u32,
                })
            }
        }
    }
}

pub open spec fn ht_packet_pass_group(passes: u8) -> Result<crate::tier1::HtPassGroup, crate::DecodeError> {
    if passes == 0 || passes > 164 {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
    } else {
        let placeholders = ht_placeholder_passes(passes as nat);
        Ok(crate::tier1::HtPassGroup {
            cleanup_passes: (placeholders + 1) as u8,
            refinement_passes: (ht_substantive_passes(passes as nat) - 1) as u8,
            placeholder_passes: placeholders as u8,
            skipped_bitplanes: (placeholders / 3) as u8,
        })
    }
}

pub struct HtMagSgnState {
    pub input: Seq<u8>,
    pub next_byte: usize,
    pub value: u8,
    pub next_bit: u8,
    pub last_bit: u8,
    pub has_byte: bool,
    pub current_is_physical: bool,
    pub previous_was_ff: bool,
    pub physical_bits_read: u64,
    pub fill_bits_read: u64,
    pub work_units: u64,
    pub maximum_work_units: u64,
}

pub open spec fn ht_magsgn_initial(input: Seq<u8>, maximum_work_units: u64)
    -> Result<HtMagSgnState, crate::DecodeError>
{
    if input.len() > 65534 {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
    } else {
        Ok(HtMagSgnState {
            input, next_byte: 0, value: 0, next_bit: 0, last_bit: 0, has_byte: false,
            current_is_physical: false, previous_was_ff: false,
            physical_bits_read: 0, fill_bits_read: 0, work_units: 0, maximum_work_units,
        })
    }
}

pub open spec fn ht_magsgn_load(state: HtMagSgnState)
    -> (Result<(), crate::DecodeError>, HtMagSgnState)
{
    let physical = state.next_byte < state.input.len();
    let value = if physical { state.input[state.next_byte as int] } else { 0xffu8 };
    if physical && !ht_forward_stuffing_valid(state.previous_was_ff, value) {
        (Err(crate::DecodeError::MalformedInput {
            offset: state.next_byte as u64, kind: crate::MalformedKind::InvalidBitStuffing,
        }), state)
    } else {
        let changed = HtMagSgnState { value, current_is_physical: physical, ..state };
        if physical && state.next_byte == usize::MAX {
            (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), changed)
        } else {
            (Ok(()), HtMagSgnState {
                next_byte: if physical { (state.next_byte + 1) as usize } else { state.next_byte },
                next_bit: 0, last_bit: (ht_forward_data_bits(state.previous_was_ff) - 1) as u8,
                has_byte: true, previous_was_ff: value == 0xff, ..changed
            })
        }
    }
}

pub open spec fn ht_magsgn_read(state: HtMagSgnState)
    -> (Result<bool, crate::DecodeError>, HtMagSgnState)
{
    let loaded = if !state.has_byte || state.next_bit > state.last_bit {
        ht_magsgn_load(state)
    } else { (Ok(()), state) };
    match loaded.0 {
        Err(error) => (Err(error), loaded.1),
        Ok(()) => {
            let ready = loaded.1;
            if !ready.has_byte || ready.next_bit > ready.last_bit || ready.next_bit >= 8 {
                (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), ready)
            } else {
                let advanced = HtMagSgnState { next_bit: (ready.next_bit + 1) as u8, ..ready };
                let counter = if ready.current_is_physical { ready.physical_bits_read } else { ready.fill_bits_read };
                if counter == u64::MAX {
                    (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), advanced)
                } else {
                    let consumed = HtMagSgnState {
                        physical_bits_read: if ready.current_is_physical { (counter + 1) as u64 } else { ready.physical_bits_read },
                        fill_bits_read: if ready.current_is_physical { ready.fill_bits_read } else { (counter + 1) as u64 },
                        ..advanced
                    };
                    if ready.work_units == u64::MAX {
                        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), consumed)
                    } else {
                        let charged = HtMagSgnState { work_units: (ready.work_units + 1) as u64, ..consumed };
                        if charged.work_units > ready.maximum_work_units {
                            (Err(ht_reader_work_error(ready.maximum_work_units, charged.work_units)), charged)
                        } else {
                            (Ok(ht_reader_bit(ready.value, ready.next_bit)), charged)
                        }
                    }
                }
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn ht_magsgn_word_run(state: HtMagSgnState, remaining: nat, index: nat, value: u64)
    -> (Result<u64, crate::DecodeError>, HtMagSgnState)
    recommends index + remaining <= 39,
    decreases remaining,
{
    if remaining == 0 { (Ok(value), state) } else {
        let read = ht_magsgn_read(state);
        match read.0 {
            Err(error) => (Err(error), read.1),
            Ok(bit) => ht_magsgn_word_run(read.1, (remaining - 1) as nat, index + 1,
                if bit { value | (1u64 << index as u8) } else { value }),
        }
    }
}

pub open spec fn ht_magsgn_read_bits(state: HtMagSgnState, count: u8)
    -> (Result<u64, crate::DecodeError>, HtMagSgnState)
{
    if count > 39 { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state) }
    else { ht_magsgn_word_run(state, count as nat, 0, 0) }
}

pub open spec fn ht_magsgn_pending(state: HtMagSgnState) -> bool {
    (state.has_byte && state.current_is_physical && state.next_bit <= state.last_bit)
        || state.next_byte < state.input.len()
}

#[verifier::opaque]
pub open spec fn ht_magsgn_finish_run(state: HtMagSgnState, remaining: nat)
    -> (Result<(), crate::DecodeError>, HtMagSgnState)
    decreases remaining,
{
    if !ht_magsgn_pending(state) { (Ok(()), state) }
    else if remaining == 0 { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state) }
    else {
        let read = ht_magsgn_read(state);
        match read.0 {
            Err(error) => (Err(error), read.1),
            Ok(false) => (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), read.1),
            Ok(true) => ht_magsgn_finish_run(read.1, (remaining - 1) as nat),
        }
    }
}

pub open spec fn ht_magsgn_finish(state: HtMagSgnState)
    -> (Result<(), crate::DecodeError>, HtMagSgnState)
{
    if state.input.len() * 8 > usize::MAX {
        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
    } else { ht_magsgn_finish_run(state, state.input.len() * 8) }
}

pub struct HtForwardRefinementState {
    pub input: Seq<u8>,
    pub next_byte: usize,
    pub current_byte: usize,
    pub value: u8,
    pub next_bit: u8,
    pub last_bit: u8,
    pub has_byte: bool,
    pub previous_was_ff: bool,
    pub physical_bits_read: u64,
    pub fill_bits_read: u64,
    pub maximum_work_units: u64,
}

pub struct HtReverseRefinementState {
    pub input: Seq<u8>,
    pub current_byte: usize,
    pub value: u8,
    pub next_bit: u8,
    pub last_bit: u8,
    pub has_byte: bool,
    pub later_requires_unstuffing: bool,
    pub physical_bits_read: u64,
    pub fill_bits_read: u64,
    pub maximum_work_units: u64,
}

pub type HtPositionedRead = Result<(bool, Option<crate::tier1::HtBitPosition>), crate::DecodeError>;

pub open spec fn ht_refinement_work(physical: u64, fill: u64, maximum: u64) -> Result<(), crate::DecodeError> {
    if physical + fill > u64::MAX { Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)) }
    else if physical + fill > maximum { Err(ht_reader_work_error(maximum, (physical + fill) as u64)) }
    else { Ok(()) }
}

pub open spec fn ht_forward_refinement_initial(input: Seq<u8>, maximum_work_units: u64)
    -> Result<HtForwardRefinementState, crate::DecodeError>
{
    if input.len() > 2046 { Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)) }
    else { Ok(HtForwardRefinementState {
        input, next_byte: 0, current_byte: 0, value: 0, next_bit: 0, last_bit: 0,
        has_byte: false, previous_was_ff: false, physical_bits_read: 0, fill_bits_read: 0, maximum_work_units,
    }) }
}

pub open spec fn ht_forward_refinement_load(state: HtForwardRefinementState)
    -> (Result<(), crate::DecodeError>, HtForwardRefinementState)
{
    let physical = state.next_byte < state.input.len();
    let value = if physical { state.input[state.next_byte as int] } else { 0u8 };
    if physical && state.previous_was_ff && value & 0x80u8 != 0 {
        (Err(crate::DecodeError::MalformedInput {
            offset: state.next_byte as u64, kind: crate::MalformedKind::InvalidBitStuffing,
        }), state)
    } else {
        let changed = HtForwardRefinementState {
            current_byte: if physical { state.next_byte } else { state.input.len() as usize },
            value, ..state
        };
        if physical && state.next_byte == usize::MAX {
            (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), changed)
        } else {
            (Ok(()), HtForwardRefinementState {
                next_byte: if physical { (state.next_byte + 1) as usize } else { state.next_byte },
                next_bit: 0, last_bit: if state.previous_was_ff { 6 } else { 7 },
                has_byte: true, previous_was_ff: value == 0xff, ..changed
            })
        }
    }
}

pub open spec fn ht_forward_refinement_positioned(state: HtForwardRefinementState)
    -> (HtPositionedRead, HtForwardRefinementState)
{
    let loaded = if !state.has_byte || state.next_bit > state.last_bit {
        ht_forward_refinement_load(state)
    } else { (Ok(()), state) };
    match loaded.0 {
        Err(error) => (Err(error), loaded.1),
        Ok(()) => {
            let ready = loaded.1;
            if !ready.has_byte || ready.next_bit > ready.last_bit || ready.next_bit >= 8 {
                (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), ready)
            } else {
                let physical = ready.current_byte < ready.input.len();
                let position = if physical {
                    Some(crate::tier1::HtBitPosition { byte: ready.current_byte as u16, bit: ready.next_bit })
                } else { None };
                let advanced = HtForwardRefinementState { next_bit: (ready.next_bit + 1) as u8, ..ready };
                let counter = if physical { ready.physical_bits_read } else { ready.fill_bits_read };
                if counter == u64::MAX {
                    (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), advanced)
                } else {
                    let consumed = HtForwardRefinementState {
                        physical_bits_read: if physical { (counter + 1) as u64 } else { ready.physical_bits_read },
                        fill_bits_read: if physical { ready.fill_bits_read } else { (counter + 1) as u64 }, ..advanced
                    };
                    match ht_refinement_work(consumed.physical_bits_read, consumed.fill_bits_read, consumed.maximum_work_units) {
                        Err(error) => (Err(error), consumed),
                        Ok(()) => (Ok((ht_reader_bit(ready.value, ready.next_bit), position)), consumed),
                    }
                }
            }
        },
    }
}

pub open spec fn ht_forward_refinement_read(state: HtForwardRefinementState)
    -> (Result<bool, crate::DecodeError>, HtForwardRefinementState)
{
    let read = ht_forward_refinement_positioned(state);
    (match read.0 { Err(error) => Err(error), Ok((bit, _)) => Ok(bit) }, read.1)
}

pub open spec fn ht_reverse_refinement_load(state: HtReverseRefinementState)
    -> (Result<(), crate::DecodeError>, HtReverseRefinementState)
{
    if state.current_byte == 0 {
        (Ok(()), HtReverseRefinementState { has_byte: false, value: 0, next_bit: 0, last_bit: 7, ..state })
    } else {
        let index = (state.current_byte - 1) as usize;
        let moved = HtReverseRefinementState { current_byte: index, ..state };
        if index >= state.input.len() {
            (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), moved)
        } else {
            let value = state.input[index as int];
            (Ok(()), HtReverseRefinementState {
                value, next_bit: 0,
                last_bit: if state.later_requires_unstuffing && value & 0x7fu8 == 0x7f { 6 } else { 7 },
                later_requires_unstuffing: value > 0x8f, has_byte: true, ..moved
            })
        }
    }
}

pub open spec fn ht_reverse_refinement_initial(input: Seq<u8>, maximum_work_units: u64)
    -> Result<HtReverseRefinementState, crate::DecodeError>
{
    if input.len() > 2046 { Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)) }
    else {
        let loaded = ht_reverse_refinement_load(HtReverseRefinementState {
            current_byte: input.len() as usize, input, value: 0, next_bit: 0, last_bit: 7,
            has_byte: false, later_requires_unstuffing: true, physical_bits_read: 0, fill_bits_read: 0, maximum_work_units,
        });
        match loaded.0 { Err(error) => Err(error), Ok(()) => Ok(loaded.1) }
    }
}

pub open spec fn ht_reverse_refinement_positioned(state: HtReverseRefinementState)
    -> (HtPositionedRead, HtReverseRefinementState)
{
    let loaded = if !state.has_byte || state.next_bit > state.last_bit {
        ht_reverse_refinement_load(state)
    } else { (Ok(()), state) };
    match loaded.0 {
        Err(error) => (Err(error), loaded.1),
        Ok(()) => {
            let ready = loaded.1;
            if ready.has_byte && ready.next_bit >= 8 {
                (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), ready)
            } else {
                let position = if ready.has_byte {
                    Some(crate::tier1::HtBitPosition { byte: ready.current_byte as u16, bit: ready.next_bit })
                } else { None };
                let advanced = HtReverseRefinementState {
                    next_bit: if ready.has_byte { (ready.next_bit + 1) as u8 } else { ready.next_bit }, ..ready
                };
                let counter = if ready.has_byte { ready.physical_bits_read } else { ready.fill_bits_read };
                if counter == u64::MAX {
                    (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), advanced)
                } else {
                    let consumed = HtReverseRefinementState {
                        physical_bits_read: if ready.has_byte { (counter + 1) as u64 } else { ready.physical_bits_read },
                        fill_bits_read: if ready.has_byte { ready.fill_bits_read } else { (counter + 1) as u64 }, ..advanced
                    };
                    match ht_refinement_work(consumed.physical_bits_read, consumed.fill_bits_read, consumed.maximum_work_units) {
                        Err(error) => (Err(error), consumed),
                        Ok(()) => (Ok((if ready.has_byte { ht_reader_bit(ready.value, ready.next_bit) } else { false }, position)), consumed),
                    }
                }
            }
        },
    }
}

pub open spec fn ht_reverse_refinement_read(state: HtReverseRefinementState)
    -> (Result<bool, crate::DecodeError>, HtReverseRefinementState)
{
    let read = ht_reverse_refinement_positioned(state);
    (match read.0 { Err(error) => Err(error), Ok((bit, _)) => Ok(bit) }, read.1)
}

pub struct HtRefinementReadersState {
    pub forward: HtForwardRefinementState,
    pub reverse: Option<HtReverseRefinementState>,
    pub claimed: Seq<u8>,
    pub stuffed: Seq<u8>,
}

pub open spec fn ht_refinement_readers_initial(
    forward_input: Seq<u8>, reverse_input: Seq<u8>, bytes: usize, has_reverse: bool, maximum_work_units: u64,
) -> Result<HtRefinementReadersState, crate::DecodeError> {
    match ht_forward_refinement_initial(forward_input, maximum_work_units) {
        Err(error) => Err(error),
        Ok(forward) => {
            let reverse = if has_reverse {
                match ht_reverse_refinement_initial(reverse_input, maximum_work_units) {
                    Err(error) => Err(error), Ok(reverse) => Ok(Some(reverse)),
                }
            } else { Ok(None) };
            match reverse {
                Err(error) => Err(error),
                Ok(reverse) => Ok(HtRefinementReadersState {
                    forward, reverse, claimed: Seq::new(bytes as nat, |i: int| 0u8), stuffed: Seq::new(bytes as nat, |i: int| 0u8),
                }),
            }
        },
    }
}

pub open spec fn ht_refinement_claim(state: HtRefinementReadersState, position: Option<crate::tier1::HtBitPosition>)
    -> (Result<(), crate::DecodeError>, HtRefinementReadersState)
{
    match position {
        None => (Ok(()), state),
        Some(position) => if position.bit >= 8 || position.byte >= state.claimed.len() {
            (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state)
        } else {
            let mask = 1u8 << position.bit;
            let value = state.claimed[position.byte as int];
            if value & mask != 0 {
                (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state)
            } else {
                (Ok(()), HtRefinementReadersState { claimed: state.claimed.update(position.byte as int, value | mask), ..state })
            }
        },
    }
}

pub open spec fn ht_refinement_read_forward(state: HtRefinementReadersState)
    -> (Result<bool, crate::DecodeError>, HtRefinementReadersState)
{
    let read = ht_forward_refinement_positioned(state.forward);
    let consumed = HtRefinementReadersState { forward: read.1, ..state };
    match read.0 {
        Err(error) => (Err(error), consumed),
        Ok((bit, position)) => {
            let claim = ht_refinement_claim(consumed, position);
            (match claim.0 { Err(error) => Err(error), Ok(()) => Ok(bit) }, claim.1)
        },
    }
}

pub open spec fn ht_refinement_read_reverse(state: HtRefinementReadersState)
    -> (Result<bool, crate::DecodeError>, HtRefinementReadersState)
{
    match state.reverse {
        None => (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state),
        Some(reverse) => {
            let read = ht_reverse_refinement_positioned(reverse);
            let consumed = HtRefinementReadersState { reverse: Some(read.1), ..state };
            match read.0 {
                Err(error) => (Err(error), consumed),
                Ok((bit, position)) => {
                    if read.1.has_byte && read.1.last_bit == 6 && read.1.current_byte >= consumed.stuffed.len() {
                        (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), consumed)
                    } else {
                        let marked = if read.1.has_byte && read.1.last_bit == 6 {
                            HtRefinementReadersState { stuffed: consumed.stuffed.update(
                                read.1.current_byte as int, consumed.stuffed[read.1.current_byte as int] | 0x80u8), ..consumed }
                        } else { consumed };
                        let claim = ht_refinement_claim(marked, position);
                        (match claim.0 { Err(error) => Err(error), Ok(()) => Ok(bit) }, claim.1)
                    }
                },
            }
        },
    }
}

pub open spec fn ht_refinement_finish(state: HtRefinementReadersState, encoded: Seq<u8>) -> Result<(), crate::DecodeError> {
    if state.claimed.len() != encoded.len() || state.stuffed.len() != encoded.len()
        || !(forall |i: int| 0 <= i < encoded.len() ==> encoded[i] & !(state.claimed[i] | state.stuffed[i]) == 0) {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
    } else { Ok(()) }
}

pub struct HtReverseVlcState {
    pub input: Seq<u8>,
    pub current_byte: usize,
    pub value: u8,
    pub next_bit: u8,
    pub last_bit: u8,
    pub has_byte: bool,
    pub later_byte_requires_unstuffing: bool,
    pub bits_read: u64,
    pub maximum_work_units: u64,
}

pub struct HtMelState {
    pub input: Seq<u8>,
    pub next_byte: usize,
    pub current_byte: usize,
    pub value: u8,
    pub next_bit: i8,
    pub previous_was_ff: bool,
    pub bits_read: u64,
    pub maximum_work_units: u64,
}

pub open spec fn ht_reader_error(kind: crate::MalformedKind) -> crate::DecodeError {
    crate::DecodeError::MalformedInput { offset: 0, kind }
}

pub open spec fn ht_reader_work_error(limit: u64, actual: u64) -> crate::DecodeError {
    crate::DecodeError::LimitExceeded { resource: crate::Resource::WorkUnits, limit, actual }
}

pub open spec fn ht_reader_bit(value: u8, index: u8) -> bool
    recommends index < 8,
{
    (value >> index) & 1 != 0
}

pub open spec fn ht_reverse_vlc_initial(input: Seq<u8>, maximum_work_units: u64)
    -> Result<HtReverseVlcState, crate::DecodeError>
{
    if input.len() < 2 || input.len() > 4079 {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
    } else {
        let index = (input.len() - 2) as usize;
        let value = input[index as int];
        Ok(HtReverseVlcState {
            input, current_byte: index, value, next_bit: 4,
            last_bit: if (value >> 4) & 7 == 7 { 6 } else { 7 },
            has_byte: true, later_byte_requires_unstuffing: value | 0x0fu8 > 0x8f,
            bits_read: 0, maximum_work_units,
        })
    }
}

pub open spec fn ht_reverse_vlc_load(state: HtReverseVlcState)
    -> (Result<(), crate::DecodeError>, HtReverseVlcState)
{
    if state.current_byte == 0 {
        (Ok(()), HtReverseVlcState { has_byte: false, ..state })
    } else {
        let index = (state.current_byte - 1) as usize;
        let moved = HtReverseVlcState { current_byte: index, ..state };
        if index >= state.input.len() {
            (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), moved)
        } else {
            let value = state.input[index as int];
            (Ok(()), HtReverseVlcState {
                value, next_bit: 0,
                last_bit: (ht_reverse_data_bits(state.later_byte_requires_unstuffing, value) - 1) as u8,
                later_byte_requires_unstuffing: value > 0x8f, ..moved
            })
        }
    }
}

pub open spec fn ht_reverse_vlc_read(state: HtReverseVlcState)
    -> (Result<bool, crate::DecodeError>, HtReverseVlcState)
{
    if !state.has_byte || state.next_bit >= 8 {
        (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state)
    } else {
        let advanced = HtReverseVlcState { next_bit: (state.next_bit + 1) as u8, ..state };
        if state.bits_read == u64::MAX {
            (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), advanced)
        } else {
            let charged = HtReverseVlcState { bits_read: (state.bits_read + 1) as u64, ..advanced };
            if charged.bits_read > state.maximum_work_units {
                (Err(ht_reader_work_error(state.maximum_work_units, charged.bits_read)), charged)
            } else {
                let loaded = if charged.next_bit > state.last_bit {
                    ht_reverse_vlc_load(charged)
                } else { (Ok(()), charged) };
                match loaded.0 {
                    Err(error) => (Err(error), loaded.1),
                    Ok(()) => (Ok(ht_reader_bit(state.value, state.next_bit)), loaded.1),
                }
            }
        }
    }
}

pub open spec fn ht_reverse_vlc_position(state: HtReverseVlcState) -> Option<(u16, u8)> {
    if !state.has_byte || state.current_byte > u16::MAX { None }
    else { Some((state.current_byte as u16, state.next_bit)) }
}

pub open spec fn ht_mel_load(state: HtMelState)
    -> (Result<(), crate::DecodeError>, HtMelState)
{
    if state.next_byte >= state.input.len() {
        (Ok(()), HtMelState { next_bit: -1i8, ..state })
    } else {
        let index = state.next_byte;
        let value = if index + 1 == state.input.len() {
            state.input[index as int] | 0x0fu8
        } else { state.input[index as int] };
        if !ht_mel_stuffing_valid(state.previous_was_ff, value as nat) {
            (Err(crate::DecodeError::MalformedInput {
                offset: index as u64, kind: crate::MalformedKind::InvalidBitStuffing,
            }), state)
        } else {
            (Ok(()), HtMelState {
                next_byte: (index + 1) as usize, current_byte: index, value,
                next_bit: if state.previous_was_ff { 6 } else { 7 },
                previous_was_ff: value == 0xff, ..state
            })
        }
    }
}

pub open spec fn ht_mel_initial(input: Seq<u8>, maximum_work_units: u64)
    -> Result<HtMelState, crate::DecodeError>
{
    if input.len() == 0 || input.len() > 4078 {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
    } else {
        let loaded = ht_mel_load(HtMelState {
            input, next_byte: 0, current_byte: 0, value: 0, next_bit: -1i8,
            previous_was_ff: false, bits_read: 0, maximum_work_units,
        });
        match loaded.0 { Err(error) => Err(error), Ok(()) => Ok(loaded.1) }
    }
}

pub open spec fn ht_mel_read(state: HtMelState)
    -> (Result<bool, crate::DecodeError>, HtMelState)
{
    let loaded = if state.next_bit < 0 { ht_mel_load(state) } else { (Ok(()), state) };
    match loaded.0 {
        Err(error) => (Err(error), loaded.1),
        Ok(()) => {
            let ready = loaded.1;
            if ready.next_bit < 0 || ready.next_bit >= 8 {
                (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), ready)
            } else {
                let advanced = HtMelState { next_bit: (ready.next_bit - 1) as i8, ..ready };
                if ready.bits_read == u64::MAX {
                    (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), advanced)
                } else {
                    let charged = HtMelState { bits_read: (ready.bits_read + 1) as u64, ..advanced };
                    if charged.bits_read > ready.maximum_work_units {
                        (Err(ht_reader_work_error(ready.maximum_work_units, charged.bits_read)), charged)
                    } else {
                        (Ok(ht_reader_bit(ready.value, ready.next_bit as u8)), charged)
                    }
                }
            }
        },
    }
}

pub open spec fn ht_mel_position(state: HtMelState) -> Option<(u16, u8)> {
    if state.next_bit < 0 || state.current_byte > u16::MAX { None }
    else { Some((state.current_byte as u16, state.next_bit as u8)) }
}

pub open spec fn ht_reverse_vlc_prefix(state: HtReverseVlcState, count: nat)
    -> (Result<Seq<bool>, crate::DecodeError>, HtReverseVlcState)
    decreases count,
{
    if count == 0 { (Ok(Seq::empty()), state) }
    else {
        let prefix = ht_reverse_vlc_prefix(state, (count - 1) as nat);
        match prefix.0 {
            Err(error) => (Err(error), prefix.1),
            Ok(bits) => {
                let step = ht_reverse_vlc_read(prefix.1);
                match step.0 {
                    Err(error) => (Err(error), step.1),
                    Ok(bit) => (Ok(bits.push(bit)), step.1),
                }
            },
        }
    }
}

pub open spec fn ht_mel_prefix(state: HtMelState, count: nat)
    -> (Result<Seq<bool>, crate::DecodeError>, HtMelState)
    decreases count,
{
    if count == 0 { (Ok(Seq::empty()), state) }
    else {
        let prefix = ht_mel_prefix(state, (count - 1) as nat);
        match prefix.0 {
            Err(error) => (Err(error), prefix.1),
            Ok(bits) => {
                let step = ht_mel_read(prefix.1);
                match step.0 {
                    Err(error) => (Err(error), step.1),
                    Ok(bit) => (Ok(bits.push(bit)), step.1),
                }
            },
        }
    }
}

pub struct HtCleanupReaderState {
    pub mel: HtMelState,
    pub vlc: HtReverseVlcState,
    pub mel_last_byte: Option<u16>,
    pub vlc_last_byte: Option<u16>,
    pub terminal_byte: u16,
    pub mel_state: u8,
    pub mel_zero_events: u8,
    pub mel_terminating_one: bool,
    pub mel_events: u32,
    pub work_units: u64,
    pub maximum_work_units: u64,
}

pub open spec fn ht_cleanup_readers_initial(mel_input: Seq<u8>, vlc_input: Seq<u8>, maximum_work_units: u64)
    -> Result<HtCleanupReaderState, crate::DecodeError>
{
    match ht_mel_initial(mel_input, maximum_work_units) {
        Err(error) => Err(error),
        Ok(mel) => match ht_reverse_vlc_initial(vlc_input, maximum_work_units) {
            Err(error) => Err(error),
            Ok(vlc) => Ok(HtCleanupReaderState {
                mel, vlc, mel_last_byte: None, vlc_last_byte: None,
                terminal_byte: (vlc_input.len() - 2) as u16,
                mel_state: 0, mel_zero_events: 0, mel_terminating_one: false,
                mel_events: 0, work_units: 0, maximum_work_units,
            }),
        },
    }
}

pub open spec fn ht_cleanup_reader_tick(state: HtCleanupReaderState)
    -> (Result<(), crate::DecodeError>, HtCleanupReaderState)
{
    if state.work_units == u64::MAX {
        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
    } else {
        let changed = HtCleanupReaderState { work_units: (state.work_units + 1) as u64, ..state };
        if changed.work_units > state.maximum_work_units {
            (Err(ht_reader_work_error(state.maximum_work_units, changed.work_units)), changed)
        } else { (Ok(()), changed) }
    }
}

pub open spec fn ht_cleanup_reader_can_claim(state: HtCleanupReaderState, forward: bool, byte: u16) -> bool {
    if byte >= state.vlc.input.len() { false } else {
        let value = state.vlc.input[byte as int];
        let opposite = if forward { state.vlc_last_byte } else { state.mel_last_byte };
        match opposite {
            None => byte <= state.terminal_byte,
            Some(other) => if forward {
                ht_cleanup_byte_claims_valid(byte as nat, other as nat, state.terminal_byte as nat, value)
            } else {
                ht_cleanup_byte_claims_valid(other as nat, byte as nat, state.terminal_byte as nat, value)
            },
        }
    }
}

pub open spec fn ht_cleanup_reader_read_mel(state: HtCleanupReaderState)
    -> (Result<bool, crate::DecodeError>, HtCleanupReaderState)
{
    let loaded = if ht_mel_position(state.mel).is_none() { ht_mel_load(state.mel) }
        else { (Ok(()), state.mel) };
    let ready = HtCleanupReaderState { mel: loaded.1, ..state };
    match loaded.0 {
        Err(error) => (Err(error), ready),
        Ok(()) => match ht_mel_position(ready.mel) {
            None => (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), ready),
            Some(position) => if !ht_cleanup_reader_can_claim(ready, true, position.0) {
                (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), ready)
            } else {
                let read = ht_mel_read(ready.mel);
                let consumed = HtCleanupReaderState { mel: read.1, ..ready };
                match read.0 {
                    Err(error) => (Err(error), consumed),
                    Ok(bit) => {
                        let claimed = HtCleanupReaderState { mel_last_byte: Some(position.0), ..consumed };
                        let charged = ht_cleanup_reader_tick(claimed);
                        match charged.0 { Err(error) => (Err(error), charged.1), Ok(()) => (Ok(bit), charged.1) }
                    },
                }
            },
        },
    }
}

pub open spec fn ht_cleanup_reader_read_vlc(state: HtCleanupReaderState)
    -> (Result<bool, crate::DecodeError>, HtCleanupReaderState)
{
    match ht_reverse_vlc_position(state.vlc) {
        None => (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state),
        Some(position) => if !ht_cleanup_reader_can_claim(state, false, position.0) {
            (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state)
        } else {
            let read = ht_reverse_vlc_read(state.vlc);
            let consumed = HtCleanupReaderState { vlc: read.1, ..state };
            match read.0 {
                Err(error) => (Err(error), consumed),
                Ok(bit) => {
                    let claimed = HtCleanupReaderState { vlc_last_byte: Some(position.0), ..consumed };
                    let charged = ht_cleanup_reader_tick(claimed);
                    match charged.0 { Err(error) => (Err(error), charged.1), Ok(()) => (Ok(bit), charged.1) }
                },
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn ht_cleanup_peek_run(
    owner: HtCleanupReaderState, copy: HtReverseVlcState, remaining: nat, index: nat, value: u16,
) -> u16
    recommends index + remaining <= 255,
    decreases remaining,
{
    if remaining == 0 { value } else {
        let available = match ht_reverse_vlc_position(copy) {
            None => false, Some(position) => ht_cleanup_reader_can_claim(owner, false, position.0),
        };
        if !available { value } else {
            let read = ht_reverse_vlc_read(copy);
            match read.0 {
                Err(_) => value,
                Ok(bit) => if bit && index >= 16 { value } else {
                    ht_cleanup_peek_run(owner, read.1, (remaining - 1) as nat, index + 1,
                        if bit { value | (1u16 << index as u8) } else { value })
                },
            }
        }
    }
}

pub open spec fn ht_cleanup_peek_vlc(state: HtCleanupReaderState, count: u8) -> u16 {
    ht_cleanup_peek_run(state, state.vlc, count as nat, 0, 0)
}

pub open spec fn ht_cleanup_readers_finish(state: HtCleanupReaderState) -> Result<(), crate::DecodeError> {
    match (state.mel_last_byte, state.vlc_last_byte) {
        (Some(mel), Some(vlc)) => {
            if mel >= state.vlc.input.len()
                || !ht_cleanup_byte_claims_valid(mel as nat, vlc as nat, state.terminal_byte as nat, state.vlc.input[mel as int]) {
                Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock))
            } else { Ok(()) }
        },
        _ => Ok(()),
    }
}

#[verifier::opaque]
pub open spec fn ht_cleanup_advance_vlc(state: HtCleanupReaderState, count: nat)
    -> (Result<(), crate::DecodeError>, HtCleanupReaderState)
    decreases count,
{
    if count == 0 { (Ok(()), state) } else {
        let read = ht_cleanup_reader_read_vlc(state);
        match read.0 {
            Err(error) => (Err(error), read.1),
            Ok(_) => ht_cleanup_advance_vlc(read.1, (count - 1) as nat),
        }
    }
}

pub open spec fn ht_cleanup_mel_emit(state: HtCleanupReaderState)
    -> (Result<bool, crate::DecodeError>, HtCleanupReaderState)
{
    let changed = if state.mel_zero_events > 0 {
        HtCleanupReaderState { mel_zero_events: (state.mel_zero_events - 1) as u8, ..state }
    } else { HtCleanupReaderState { mel_terminating_one: false, ..state } };
    if state.mel_events == u32::MAX {
        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), changed)
    } else {
        (Ok(state.mel_zero_events == 0), HtCleanupReaderState { mel_events: (state.mel_events + 1) as u32, ..changed })
    }
}

#[verifier::opaque]
pub open spec fn ht_cleanup_mel_suffix(state: HtCleanupReaderState, remaining: nat, suffix: u8)
    -> (Result<u8, crate::DecodeError>, HtCleanupReaderState)
    recommends remaining <= 5,
    decreases remaining,
{
    if remaining == 0 { (Ok(suffix), state) } else {
        let shifted = suffix << 1u8;
        let read = ht_cleanup_reader_read_mel(state);
        match read.0 {
            Err(error) => (Err(error), read.1),
            Ok(bit) => ht_cleanup_mel_suffix(read.1, (remaining - 1) as nat,
                if bit { shifted | 1u8 } else { shifted }),
        }
    }
}

pub open spec fn ht_cleanup_mel_complete_suffix(result: (Result<u8, crate::DecodeError>, HtCleanupReaderState))
    -> (Result<bool, crate::DecodeError>, HtCleanupReaderState)
{
    match result.0 {
        Err(error) => (Err(error), result.1),
        Ok(suffix) => ht_cleanup_mel_emit(HtCleanupReaderState {
            mel_zero_events: suffix, mel_terminating_one: true,
            mel_state: ht_mel_next_state(result.1.mel_state as nat, false) as u8, ..result.1
        }),
    }
}

pub open spec fn ht_cleanup_mel_event(state: HtCleanupReaderState)
    -> (Result<bool, crate::DecodeError>, HtCleanupReaderState)
{
    if state.mel_zero_events > 0 || state.mel_terminating_one { ht_cleanup_mel_emit(state) }
    else if state.mel_state > 12 { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state) }
    else {
        let read = ht_cleanup_reader_read_mel(state);
        match read.0 {
            Err(error) => (Err(error), read.1),
            Ok(true) => ht_cleanup_mel_emit(HtCleanupReaderState {
                mel_zero_events: 1u8 << ht_mel_exponent(state.mel_state as nat) as u8,
                mel_state: ht_mel_next_state(state.mel_state as nat, true) as u8, ..read.1
            }),
            Ok(false) => ht_cleanup_mel_complete_suffix(
                ht_cleanup_mel_suffix(read.1, ht_mel_exponent(state.mel_state as nat), 0)),
        }
    }
}

#[verifier::opaque]
pub open spec fn ht_uvlc_prefix(state: HtCleanupReaderState)
    -> (Result<(u8, u8, u8), crate::DecodeError>, HtCleanupReaderState)
{
    let first = ht_cleanup_reader_read_vlc(state);
    match first.0 {
        Err(error) => (Err(error), first.1),
        Ok(true) => (Ok((1, 0, 1)), first.1),
        Ok(false) => {
            let second = ht_cleanup_reader_read_vlc(first.1);
            match second.0 {
                Err(error) => (Err(error), second.1),
                Ok(true) => (Ok((2, 0, 2)), second.1),
                Ok(false) => {
                    let third = ht_cleanup_reader_read_vlc(second.1);
                    match third.0 {
                        Err(error) => (Err(error), third.1),
                        Ok(true) => (Ok((3, 1, 3)), third.1),
                        Ok(false) => (Ok((5, 5, 3)), third.1),
                    }
                },
            }
        },
    }
}

#[verifier::opaque]
pub open spec fn ht_uvlc_suffix_run(state: HtCleanupReaderState, remaining: nat, index: nat, value: u8)
    -> (Result<u8, crate::DecodeError>, HtCleanupReaderState)
    recommends index + remaining <= 255,
    decreases remaining,
{
    if remaining == 0 { (Ok(value), state) } else {
        let read = ht_cleanup_reader_read_vlc(state);
        match read.0 {
            Err(error) => (Err(error), read.1),
            Ok(bit) => if bit && index >= 8 {
                (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), read.1)
            } else {
                ht_uvlc_suffix_run(read.1, (remaining - 1) as nat, index + 1,
                    if bit { value | (1u8 << index as u8) } else { value })
            },
        }
    }
}

pub open spec fn ht_uvlc_suffix(state: HtCleanupReaderState, length: u8)
    -> (Result<u8, crate::DecodeError>, HtCleanupReaderState)
{
    ht_uvlc_suffix_run(state, length as nat, 0, 0)
}

#[verifier::opaque]
pub open spec fn ht_uvlc_prefix_value(state: HtCleanupReaderState)
    -> (Result<(u8, u8), crate::DecodeError>, HtCleanupReaderState)
{
    let prefix = ht_uvlc_prefix(state);
    match prefix.0 {
        Err(error) => (Err(error), prefix.1),
        Ok((base, length, prefix_length)) => {
            let suffix = ht_uvlc_suffix(prefix.1, length);
            match suffix.0 {
                Err(error) => (Err(error), suffix.1),
                Ok(value) => if base + value > u8::MAX {
                    (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), suffix.1)
                } else { (Ok(((base + value) as u8, prefix_length)), suffix.1) },
            }
        },
    }
}

pub type HtUvlcPairResult = (Result<[u8; 2], crate::DecodeError>, HtCleanupReaderState);

pub open spec fn ht_uvlc_byte(base: u8, suffix: u8, bias: u8) -> Result<u8, crate::DecodeError> {
    let value = base + suffix + bias;
    if value > u8::MAX { Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)) }
    else { Ok(value as u8) }
}

pub open spec fn ht_uvlc_single(state: HtCleanupReaderState, first: bool) -> HtUvlcPairResult {
    let prefix = ht_uvlc_prefix_value(state);
    match prefix.0 {
        Err(error) => (Err(error), prefix.1),
        Ok((value, _)) => if value == u8::MAX {
            (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), prefix.1)
        } else {
            (Ok(if first { [(value + 1) as u8, 1u8] } else { [1u8, (value + 1) as u8] }), prefix.1)
        },
    }
}

pub open spec fn ht_uvlc_pair_after_first(
    state: HtCleanupReaderState, first: (u8, u8, u8), bias: u8,
) -> HtUvlcPairResult {
    let second = ht_uvlc_prefix(state);
    match second.0 {
        Err(error) => (Err(error), second.1),
        Ok(second_prefix) => {
            let first_suffix = ht_uvlc_suffix(second.1, first.1);
            match first_suffix.0 {
                Err(error) => (Err(error), first_suffix.1),
                Ok(value) => {
                    let first_value = first.0 + value + bias;
                    if first_value > u8::MAX {
                        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), first_suffix.1)
                    } else {
                        let second_suffix = ht_uvlc_suffix(first_suffix.1, second_prefix.1);
                        match second_suffix.0 {
                            Err(error) => (Err(error), second_suffix.1),
                            Ok(value) => {
                                let second_value = second_prefix.0 + value + bias;
                                if second_value > u8::MAX {
                                    (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), second_suffix.1)
                                } else { (Ok([first_value as u8, second_value as u8]), second_suffix.1) }
                            },
                        }
                    }
                },
            }
        },
    }
}

pub open spec fn ht_uvlc_initial(state: HtCleanupReaderState, mode: u8) -> HtUvlcPairResult {
    if mode == 0 { (Ok([1u8, 1u8]), state) }
    else if mode == 1 || mode == 2 { ht_uvlc_single(state, mode == 1) }
    else if mode == 3 || mode == 4 {
        let prefix = ht_uvlc_prefix(state);
        match prefix.0 {
            Err(error) => (Err(error), prefix.1),
            Ok(first) => if mode == 4 || first.2 <= 2 {
                ht_uvlc_pair_after_first(prefix.1, first, if mode == 4 { 3 } else { 1 })
            } else {
                let bit = ht_cleanup_reader_read_vlc(prefix.1);
                match bit.0 {
                    Err(error) => (Err(error), bit.1),
                    Ok(bit_value) => {
                        let suffix = ht_uvlc_suffix(bit.1, first.1);
                        match suffix.0 {
                            Err(error) => (Err(error), suffix.1),
                            Ok(value) => if first.0 + value + 1 > u8::MAX {
                                (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), suffix.1)
                            } else {
                                (Ok([(first.0 + value + 1) as u8, if bit_value { 3u8 } else { 2u8 }]), suffix.1)
                            },
                        }
                    },
                }
            },
        }
    } else { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state) }
}

pub open spec fn ht_uvlc_non_initial(state: HtCleanupReaderState, mode: u8) -> HtUvlcPairResult {
    if mode == 0 { (Ok([1u8, 1u8]), state) }
    else if mode == 1 || mode == 2 { ht_uvlc_single(state, mode == 1) }
    else if mode == 3 {
        let prefix = ht_uvlc_prefix(state);
        match prefix.0 {
            Err(error) => (Err(error), prefix.1),
            Ok(first) => ht_uvlc_pair_after_first(prefix.1, first, 1),
        }
    } else { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state) }
}

pub open spec fn ht_cleanup_quad_word(state: HtCleanupReaderState, table: Seq<u16>, context: u8)
    -> (Result<u16, crate::DecodeError>, HtCleanupReaderState)
{
    if context > 7 { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state) }
    else {
        let index = context as nat * 128 + (ht_cleanup_peek_vlc(state, 7) & 0x7fu16) as nat;
        if index >= table.len() { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), state) }
        else {
            let event = if context == 0 { ht_cleanup_mel_event(state) } else { (Ok(true), state) };
            match event.0 {
                Err(error) => (Err(error), event.1),
                Ok(keep) => {
                    let word = if keep { table[index as int] } else { 0u16 };
                    let length = (word & 7u16) as u8;
                    if !ht_vlc_word_valid(word as nat, length as nat) {
                        (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), event.1)
                    } else {
                        let advanced = ht_cleanup_advance_vlc(event.1, length as nat);
                        match advanced.0 {
                            Err(error) => (Err(error), advanced.1),
                            Ok(()) => {
                                let charged = ht_cleanup_reader_tick(advanced.1);
                                (match charged.0 { Err(error) => Err(error), Ok(()) => Ok(word) }, charged.1)
                            },
                        }
                    }
                },
            }
        }
    }
}

pub struct Part1RawReaderState {
    pub input: Seq<u8>,
    pub next_index: usize,
    pub current_byte: u8,
    pub bits_remaining: u8,
    pub marker_padding_count: u64,
    pub work_units: u64,
    pub maximum_work_units: u64,
}

pub open spec fn part1_raw_reader_error(offset: usize, kind: crate::MalformedKind) -> crate::DecodeError {
    crate::DecodeError::MalformedInput { offset: offset as u64, kind }
}

pub open spec fn part1_raw_reader_initial(
    input: Seq<u8>, maximum_input_bytes: u64, maximum_work_units: u64,
) -> Result<Part1RawReaderState, crate::DecodeError>
    recommends input.len() <= usize::MAX,
{
    if input.len() as u64 > maximum_input_bytes {
        Err(crate::DecodeError::LimitExceeded {
            resource: crate::Resource::InputBytes, limit: maximum_input_bytes, actual: input.len() as u64,
        })
    } else {
        Ok(Part1RawReaderState {
            input, next_index: 0, current_byte: 0, bits_remaining: 0,
            marker_padding_count: 0, work_units: 0, maximum_work_units,
        })
    }
}

pub open spec fn part1_raw_reader_load(state: Part1RawReaderState)
    -> (Result<(), crate::DecodeError>, Part1RawReaderState)
{
    let next = if state.next_index < state.input.len() { state.input[state.next_index as int] } else { 0xffu8 };
    let stuffed = state.current_byte == 0xff;
    if stuffed && next > 0x8f {
        let changed = Part1RawReaderState { current_byte: 0xff, bits_remaining: 8, ..state };
        if state.marker_padding_count == u64::MAX {
            (Err(part1_raw_reader_error(state.next_index, crate::MalformedKind::ArithmeticOverflow)), changed)
        } else {
            (Ok(()), Part1RawReaderState { marker_padding_count: (state.marker_padding_count + 1) as u64, ..changed })
        }
    } else if state.next_index < state.input.len() && state.next_index == usize::MAX {
        (Err(part1_raw_reader_error(state.next_index, crate::MalformedKind::ArithmeticOverflow)), state)
    } else {
        (Ok(()), Part1RawReaderState {
            next_index: if state.next_index < state.input.len() { (state.next_index + 1) as usize } else { state.next_index },
            current_byte: next,
            bits_remaining: if stuffed { 7 } else { 8 },
            ..state
        })
    }
}

pub open spec fn part1_raw_reader_read(state: Part1RawReaderState)
    -> (Result<bool, crate::DecodeError>, Part1RawReaderState)
{
    let loaded = if state.bits_remaining == 0 { part1_raw_reader_load(state) } else { (Ok(()), state) };
    match loaded.0 {
        Err(error) => (Err(error), loaded.1),
        Ok(()) => {
            let ready = loaded.1;
            if ready.bits_remaining == 0 || ready.bits_remaining > 8 {
                (Err(part1_raw_reader_error(ready.next_index, crate::MalformedKind::InvalidBitStuffing)), ready)
            } else {
                let consumed = Part1RawReaderState { bits_remaining: (ready.bits_remaining - 1) as u8, ..ready };
                if ready.work_units == u64::MAX {
                    (Err(part1_raw_reader_error(ready.next_index, crate::MalformedKind::ArithmeticOverflow)), consumed)
                } else {
                    let charged = Part1RawReaderState { work_units: (ready.work_units + 1) as u64, ..consumed };
                    if charged.work_units > charged.maximum_work_units {
                        (Err(crate::DecodeError::LimitExceeded {
                            resource: crate::Resource::WorkUnits,
                            limit: charged.maximum_work_units,
                            actual: charged.work_units,
                        }), charged)
                    } else {
                        (Ok(part1_packet_bit(charged.current_byte, charged.bits_remaining)), charged)
                    }
                }
            }
        },
    }
}

pub open spec fn part1_raw_reader_termination(state: Part1RawReaderState) -> Result<(), crate::DecodeError> {
    let valid = if state.next_index == 0 {
        (state.input.len() == 0 && state.work_units == 0)
            || (state.input.len() == 1 && state.input[0] == 0x55)
    } else if state.bits_remaining == 0 && state.current_byte == 0xff
        && state.next_index < state.input.len()
        && state.next_index + 1 == state.input.len()
        && state.input[state.next_index as int] == 0x2a {
        true
    } else if state.next_index != state.input.len() || state.bits_remaining > 8 {
        false
    } else if state.bits_remaining == 0 {
        true
    } else {
        let shift = (8 - state.bits_remaining) as u8;
        state.current_byte & (0xffu8 >> shift) == 0x55u8 >> shift
    };
    if valid { Ok(()) } else {
        Err(part1_raw_reader_error(0, crate::MalformedKind::InvalidCodeBlock))
    }
}

pub open spec fn part1_tag_inherited_low(low: nat, inherited: nat) -> nat {
    if low < inherited { inherited } else { low }
}

pub open spec fn part1_tag_step_low(low: nat, bit: bool) -> nat {
    if bit { low } else { low + 1 }
}

pub open spec fn part1_tag_step_value(low: nat, value: nat, bit: bool) -> nat {
    if bit { low } else { value }
}

pub open spec fn part1_progression_digit(index: nat, dimension: nat) -> nat
    recommends dimension > 0,
{
    (index % dimension) as nat
}

pub open spec fn part1_progression_advance(index: nat, dimension: nat) -> nat
    recommends dimension > 0,
{
    (index / dimension) as nat
}

pub open spec fn part1_progression_emit_allowed(already_seen: bool) -> bool {
    !already_seen
}

pub open spec fn part1_mq_decision(mps: bool, lps: bool) -> bool {
    if lps { !mps } else { mps }
}

pub open spec fn part1_mq_next_mps(mps: bool, switch_mps: bool) -> bool {
    if switch_mps { !mps } else { mps }
}

pub open spec fn part1_tier1_pass_code(pass_index: nat) -> nat {
    if pass_index == 0 { 2 } else { ((pass_index - 1) % 3) as nat }
}

pub open spec fn part1_tier1_refinement_context(refined: bool, neighbors: nat) -> nat {
    if refined { 16 } else if neighbors > 0 { 15 } else { 14 }
}

pub open spec fn part1_tier1_refined_magnitude(
    magnitude: nat,
    delta: nat,
    refinement_bit: bool,
) -> nat {
    if refinement_bit { magnitude + delta } else { magnitude }
}

pub open spec fn part1_maxshift_restore(magnitude: u64, shift: u8) -> u64 {
    if shift >= 64 {
        magnitude
    } else {
        let threshold = 1_u64 << shift;
        if magnitude < threshold { magnitude } else { magnitude >> shift }
    }
}

pub open spec fn part1_tier1_midpoint_delta(bitplane: u16) -> u64 {
    if bitplane == 0 || bitplane > 64 { 0 } else { 1_u64 << (bitplane - 1) }
}

pub open spec fn part1_reversible_inverse_quantize(coefficient: int) -> int {
    coefficient
}

pub open spec fn part1_five_three_predict(sum: i64) -> i64 {
    sum >> 1
}

pub open spec fn part1_five_three_update(biased_sum: i64) -> i64 {
    biased_sum >> 2
}

pub open spec fn part1_rct_green(y: i64, chroma_sum: i64) -> i64 {
    ((y as int) - ((chroma_sum >> 2) as int)) as i64
}

pub open spec fn part1_rct_red(green: int, cr: int) -> int {
    green + cr
}

pub open spec fn part1_rct_blue(green: int, cb: int) -> int {
    green + cb
}

pub open spec fn part1_level_shift(precision: u8, signed: bool) -> i64
    recommends precision > 0 && precision <= 63,
{
    if signed { 0 } else { 1_i64 << (precision - 1) }
}

pub open spec fn part1_clip_sample(value: i64, minimum: i64, maximum: i64) -> i64 {
    if value < minimum { minimum } else if value > maximum { maximum } else { value }
}

pub open spec fn part1_marker_value(high: u8, low: u8) -> u16 {
    ((high as u16) << 8) | (low as u16)
}

pub open spec fn part1_parser_progress(previous: usize, current: usize, limit: usize) -> bool {
    previous < current && current <= limit
}

pub open spec fn part1_main_header_boundary(marker: u16) -> bool {
    marker == 0xff4f || marker == 0xff93 || marker == 0xffd9
}

pub open spec fn part1_tile_header_boundary(marker: u16) -> bool {
    marker == 0xff4f || marker == 0xff51 || marker == 0xff90 || marker == 0xffd9
}

pub open spec fn part1_packet_next_included(old: bool, contribution_present: bool) -> bool {
    old || contribution_present
}

pub open spec fn part1_packet_next_lblock(lblock: u8) -> u8
    recommends lblock < 31,
{
    ((lblock as int) + 1) as u8
}

pub open spec fn part1_packet_next_passes(old: u32, added: u32) -> u32
    recommends old <= u32::MAX - added,
{
    ((old as int) + (added as int)) as u32
}

pub open spec fn part1_exact_consumption(consumed: usize, expected: usize) -> bool {
    consumed == expected
}

pub open spec fn part1_packet_logical_bytes(tile: usize, packed: usize) -> usize
    recommends tile <= usize::MAX - packed,
{
    ((tile as int) + (packed as int)) as usize
}

pub open spec fn part1_mq_interval_subtract(interval: u32, qe: u16) -> u32
    recommends (qe as u32) <= interval,
{
    ((interval as int) - (qe as int)) as u32
}

pub open spec fn part1_mq_code_subtract(code: u32, qe: u16) -> u32
    recommends (qe as u64) * 65536 <= code as u64,
{
    ((code as int) - (qe as int) * 65536) as u32
}

pub open spec fn part1_mq_byte_class(current: u8, next: u8) -> u8 {
    if current == 0xff && next > 0x8f {
        0
    } else if current == 0xff {
        1
    } else {
        2
    }
}

pub open spec fn part1_mq_byte_addend(current: u8, next: u8) -> u32 {
    if current == 0xff && next > 0x8f {
        0xff00
    } else if current == 0xff {
        (next as u32) << 9
    } else {
        (next as u32) << 8
    }
}

pub open spec fn part1_mq_byte_ct(current: u8, next: u8) -> u8 {
    if current == 0xff && next <= 0x8f { 7 } else { 8 }
}

pub open spec fn part1_mq_byte_advances(current: u8, next: u8) -> bool {
    !(current == 0xff && next > 0x8f)
}

pub open spec fn part1_mq_renorm_double(value: u32) -> u32
    recommends value <= u32::MAX / 2,
{
    ((value as int) * 2) as u32
}

pub open spec fn part1_mq_renorm_ct(ct: u8) -> u8
    recommends ct > 0,
{
    ((ct as int) - 1) as u8
}

pub open spec fn part1_tier1_stripe_rows(height: u16, y: u16) -> u16
    recommends y < height,
{
    if (height as int) - (y as int) < 4 {
        ((height as int) - (y as int)) as u16
    } else {
        4
    }
}

pub open spec fn part1_tier1_sigprop_eligible(significant: bool, neighbors: u16) -> bool {
    !significant && neighbors > 0
}

pub open spec fn part1_tier1_magref_eligible(significant_before: bool) -> bool {
    significant_before
}

pub open spec fn part1_tier1_cleanup_eligible(
    significant: bool,
    visited: bool,
) -> bool {
    !significant && !visited
}

pub open spec fn part1_tier1_pass_complete(kind: u8) -> u8 {
    if kind == 0 { 1 } else if kind == 1 { 2 } else { 4 }
}

pub open spec fn part1_tier1_zero_context(
    horizontal: u16,
    vertical: u16,
    diagonal: u16,
    high_high: bool,
) -> u8 {
    if high_high {
        let direct = (horizontal as int) + (vertical as int);
        if diagonal == 0 {
            if direct == 0 { 0 } else if direct == 1 { 1 } else { 2 }
        } else if diagonal == 1 {
            if direct == 0 { 3 } else if direct == 1 { 4 } else { 5 }
        } else if diagonal == 2 {
            if direct == 0 { 6 } else { 7 }
        } else {
            8
        }
    } else if horizontal == 0 {
        if vertical == 0 {
            if diagonal == 0 { 0 } else if diagonal == 1 { 1 } else { 2 }
        } else if vertical == 1 {
            3
        } else {
            4
        }
    } else if horizontal == 1 {
        if vertical == 0 {
            if diagonal == 0 { 5 } else { 6 }
        } else {
            7
        }
    } else {
        8
    }
}

pub open spec fn part1_tier1_sign_offset(horizontal: i8, vertical: i8) -> u8 {
    if horizontal == 0 {
        if vertical == 0 { 0 } else { 1 }
    } else if vertical < 0 {
        2
    } else if vertical == 0 {
        3
    } else {
        4
    }
}

pub open spec fn part1_tier1_predicted_negative(horizontal: i8, vertical: i8) -> bool {
    horizontal < 0 || (horizontal == 0 && vertical < 0)
}

pub open spec fn part1_tier1_decoded_sign(symbol: bool, predicted: bool, raw: bool) -> bool {
    if raw { symbol } else { symbol != predicted }
}

pub open spec fn part1_irreversible_centered(magnitude: i64) -> i128 {
    ((magnitude as int) * 2 + 1) as i128
}

pub open spec fn part1_irreversible_scale(product: i128) -> i128
    recommends product >= 0,
{
    ((product as int) / 2) as i128
}

pub open spec fn part1_fixed_round_q16(adjusted: i64) -> i64 {
    adjusted >> 16
}

pub open spec fn part1_nine_seven_scale(low: bool) -> i64 {
    if low { 80621 } else { 53274 }
}

pub open spec fn part1_nine_seven_lift(stage: u8) -> i64 {
    if stage == 0 {
        29066
    } else if stage == 1 {
        57862
    } else if stage == 2 {
        (-3472int) as i64
    } else {
        (-103949int) as i64
    }
}

pub open spec fn part1_wavelet_detail_advance(direction: u8) -> usize {
    if direction == 3 { 3 } else if direction == 0 { 0 } else { 1 }
}

pub open spec fn part1_wavelet_next_resolution(resolution: u8) -> u8
    recommends resolution < u8::MAX,
{
    ((resolution as int) + 1) as u8
}

pub open spec fn part1_wavelet_sample_count(
    width: u32,
    height: u32,
    samples: usize,
) -> bool {
    if width == 0 {
        samples == 0
    } else {
        samples % width as usize == 0
            && samples / width as usize == height as usize
    }
}

pub open spec fn part1_sop_sequence(high: u8, low: u8) -> u16 {
    ((high as u16) << 8) | (low as u16)
}

pub open spec fn part1_sop_valid(
    marker0: u8,
    marker1: u8,
    length0: u8,
    length1: u8,
) -> bool {
    marker0 == 0xff && marker1 == 0x91 && length0 == 0 && length1 == 4
}

pub open spec fn part1_eph_valid(marker0: u8, marker1: u8) -> bool {
    marker0 == 0xff && marker1 == 0x92
}

pub open spec fn part1_packet_length_matches(actual: usize, expected: usize) -> bool {
    actual == expected
}

pub open spec fn part1_decode_output_shape_valid(
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

pub open spec fn part1_component_output_shape_valid(
    width: u32,
    height: u32,
    samples: usize,
) -> bool {
    width > 0 && height > 0 && part1_wavelet_sample_count(width, height, samples)
}

pub open spec fn part1_magnitude_limb(limbs: Seq<u64>, index: usize) -> Option<u64> {
    if index < limbs.len() { Some(limbs[index as int]) } else { None }
}

pub open spec fn part1_magnitude_zero(limbs: Seq<u64>) -> bool {
    forall |i: int| 0 <= i < limbs.len() ==> limbs[i] == 0
}

pub open spec fn part1_magnitude_prefix_value(limbs: Seq<u64>, count: nat) -> nat
    decreases count,
{
    if count == 0 { 0 }
    else {
        part1_magnitude_prefix_value(limbs, (count - 1) as nat)
            + (limbs[count - 1] as nat) * pow2((64 * (count - 1)) as nat)
    }
}

pub open spec fn part1_magnitude_value(limbs: Seq<u64>) -> nat {
    part1_magnitude_prefix_value(limbs, 6)
}

pub open spec fn part1_magnitude_capacity() -> nat {
    pow2(384)
}

pub open spec fn part1_magnitude_low_value(limbs: Seq<u64>) -> nat {
    part1_magnitude_value(limbs) % pow2(64)
}

pub open spec fn part1_magnitude_limb_or_zero(limbs: Seq<u64>, index: int) -> u64 {
    if 0 <= index < 6 { limbs[index] } else { 0 }
}

pub open spec fn part1_magnitude_right_word(low: u64, high: u64, partial: u32) -> u64 {
    if partial == 0 { low } else { (low >> partial) | (high << ((64 - partial) as u32)) }
}

pub open spec fn part1_magnitude_left_word(current: u64, previous: u64, partial: u32) -> u64 {
    if partial == 0 { current } else { (current << partial) | (previous >> ((64 - partial) as u32)) }
}

pub open spec fn part1_magnitude_left_limb(limbs: Seq<u64>, shift: u16, index: int) -> u64 {
    if shift >= 384 { 0 } else {
        let source = index - shift / 64;
        part1_magnitude_left_word(
            part1_magnitude_limb_or_zero(limbs, source),
            part1_magnitude_limb_or_zero(limbs, source - 1),
            (shift % 64) as u32,
        )
    }
}

pub open spec fn part1_magnitude_left(limbs: Seq<u64>, shift: u16) -> Seq<u64> {
    Seq::new(6, |i: int| part1_magnitude_left_limb(limbs, shift, i))
}

pub open spec fn part1_magnitude_left_fits(limbs: Seq<u64>, shift: u16) -> bool {
    if shift >= 384 {
        part1_magnitude_zero(limbs)
    } else {
        forall |i: int| 0 <= i < 6 ==>
            if i + shift / 64 >= 6 { limbs[i] == 0 }
            else if shift % 64 != 0 && i + shift / 64 == 5 {
                limbs[i] >> ((64 - shift % 64) as u32) == 0
            } else { true }
    }
}

pub open spec fn part1_magnitude_u64(limbs: Seq<u64>) -> Option<u64> {
    if forall |i: int| 1 <= i < limbs.len() ==> limbs[i] == 0 {
        Some(limbs[0])
    } else {
        None
    }
}

pub open spec fn part1_magnitude_less(left: Seq<u64>, right: Seq<u64>) -> bool {
    exists |i: int| 0 <= i < left.len() && left[i] < right[i]
        && forall |j: int| i < j < left.len() ==> left[j] == right[j]
}

pub open spec fn part1_magnitude_prefix_less(left: Seq<u64>, right: Seq<u64>, count: nat) -> bool {
    exists |i: int| 0 <= i < count && left[i] < right[i]
        && forall |j: int| i < j < count ==> left[j] == right[j]
}

pub open spec fn part1_magnitude_subtraction(left: nat, right: nat) -> Result<nat, crate::DecodeError> {
    if left < right { Err(part1_transform_error(true)) } else { Ok((left - right) as nat) }
}

pub open spec fn part1_magnitude_bit(bit: u16) -> Seq<u64> {
    Seq::new(6, |i: int| if i == bit / 64 { 1u64 << (bit % 64) } else { 0 })
}

pub open spec fn part1_magnitude_shift_limb(limbs: Seq<u64>, shift: u16, i: int) -> u64 {
    let source = i + shift / 64;
    let partial = (shift % 64) as u32;
    if shift >= 384 || source >= 6 {
        0
    } else if partial == 0 || source + 1 >= 6 {
        limbs[source] >> partial
    } else {
        (limbs[source] >> partial) | (limbs[source + 1] << (64 - partial))
    }
}

pub open spec fn part1_magnitude_shift(limbs: Seq<u64>, shift: u16) -> Seq<u64> {
    Seq::new(6, |i: int| part1_magnitude_shift_limb(limbs, shift, i))
}

pub open spec fn part1_transform_error(arithmetic: bool) -> crate::DecodeError {
    crate::DecodeError::MalformedInput {
        offset: 0,
        kind: if arithmetic { crate::MalformedKind::ArithmeticOverflow }
            else { crate::MalformedKind::InvalidTransform },
    }
}

pub open spec fn part1_checked_sample(value: int) -> Result<i64, crate::DecodeError> {
    if i64::MIN <= value <= i64::MAX {
        Ok(value as i64)
    } else {
        Err(part1_transform_error(true))
    }
}

pub open spec fn part1_roi_coefficient(
    limbs: Seq<u64>, negative: bool, shift: u8,
) -> Result<i64, crate::DecodeError> {
    if part1_magnitude_zero(limbs) {
        Ok(0)
    } else {
        let restored = if shift == 0 || part1_magnitude_less(limbs, part1_magnitude_bit(shift as u16)) {
            limbs
        } else {
            part1_magnitude_shift(limbs, shift as u16)
        };
        match part1_magnitude_u64(restored) {
            Some(value) => if value <= i64::MAX {
                Ok((if negative { -(value as int) } else { value as int }) as i64)
            } else {
                Err(part1_transform_error(true))
            },
            None => Err(part1_transform_error(true)),
        }
    }
}

pub open spec fn part1_quantization_step(
    exponent: u8, mantissa: u16, dynamic_range: u8,
) -> Result<i64, crate::DecodeError> {
    let base = ((2048 + mantissa as int) * 65536 / 2048) as i64;
    if dynamic_range >= exponent {
        let shift = (dynamic_range - exponent) as u8;
        if shift > 38 {
            Err(part1_transform_error(false))
        } else {
            part1_checked_sample((base as int) * ((1i64 << shift) as int))
        }
    } else {
        let shift = (exponent - dynamic_range) as u8;
        if shift > 62 {
            Err(part1_transform_error(false))
        } else {
            Ok((base / (1i64 << shift)) as i64)
        }
    }
}

pub open spec fn part1_dequantized_sample(
    coefficient: i64, step: i64, reversible: bool,
) -> Result<i64, crate::DecodeError> {
    if reversible || coefficient == 0 {
        Ok(part1_reversible_inverse_quantize(coefficient as int) as i64)
    } else {
        let magnitude = if coefficient < 0 { -(coefficient as int) } else { coefficient as int };
        let scaled = (2 * magnitude + 1) * (step as int) / 2;
        if coefficient == i64::MIN || step < 0 || scaled > i64::MAX {
            Err(part1_transform_error(true))
        } else {
            Ok((if coefficient < 0 { -scaled } else { scaled }) as i64)
        }
    }
}

pub type Part1QuantizedCoefficient = (Seq<u64>, bool);
pub type Part1ReconstructedPlane = (u32, u32, u8, Seq<i64>);
pub type Part1ReconstructionResult = Result<Part1ReconstructedPlane, crate::DecodeError>;

pub open spec fn part1_quantized_sample(
    coefficient: Part1QuantizedCoefficient, shift: u8, step: i64, reversible: bool,
) -> Result<i64, crate::DecodeError> {
    match part1_roi_coefficient(coefficient.0, coefficient.1, shift) {
        Err(error) => Err(error),
        Ok(value) => part1_dequantized_sample(value, step, reversible),
    }
}

pub open spec fn part1_quantized_prefix(
    coefficients: Seq<Part1QuantizedCoefficient>, shift: u8, step: i64, reversible: bool, count: nat,
) -> Result<Seq<i64>, crate::DecodeError>
    decreases count,
{
    if count == 0 {
        Ok(Seq::empty())
    } else {
        match part1_quantized_prefix(coefficients, shift, step, reversible, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(prefix) => match part1_quantized_sample(coefficients[count - 1], shift, step, reversible) {
                Err(error) => Err(error),
                Ok(sample) => Ok(prefix.push(sample)),
            },
        }
    }
}

pub open spec fn part1_quantization_budget(count: nat, maximum: u64) -> Result<(), crate::DecodeError> {
    if count * 8 > u64::MAX {
        Err(part1_transform_error(true))
    } else if count * 8 > maximum {
        Err(crate::DecodeError::LimitExceeded {
            resource: crate::Resource::DecodedBytes, limit: maximum, actual: (count * 8) as u64,
        })
    } else {
        Ok(())
    }
}

pub open spec fn part1_inverse_quantization(
    coefficients: Seq<Part1QuantizedCoefficient>, exponent: u8, mantissa: u16,
    dynamic_range: u8, reversible: bool, shift: u8, maximum: u64,
) -> Part1ReconstructionResult {
    if dynamic_range == 0 || dynamic_range > 38 || mantissa > 2047 || exponent > 31 {
        Err(part1_transform_error(false))
    } else {
        match part1_quantization_budget(coefficients.len(), maximum) {
            Err(error) => Err(error),
            Ok(()) => match if reversible { Ok(1i64) }
                else { part1_quantization_step(exponent, mantissa, dynamic_range) } {
                Err(error) => Err(error),
                Ok(step) => match part1_quantized_prefix(coefficients, shift, step, reversible, coefficients.len()) {
                    Err(error) => Err(error),
                    Ok(samples) => if coefficients.len() > u32::MAX {
                        Err(part1_transform_error(true))
                    } else {
                        Ok((coefficients.len() as u32, 1u32, if reversible { 0u8 } else { 16u8 }, samples))
                    },
                },
            },
        }
    }
}

pub open spec fn part1_integer_coefficients(coefficients: Seq<i64>) -> Seq<Part1QuantizedCoefficient> {
    Seq::new(coefficients.len(), |i: int| {
        let value = coefficients[i];
        let magnitude = (if value < 0 { -(value as int) } else { value as int }) as u64;
        (Seq::new(6, |j: int| if j == 0 { magnitude } else { 0u64 }), value < 0)
    })
}

pub open spec fn part1_inverse_integer_quantization(
    coefficients: Seq<i64>, exponent: u8, mantissa: u16,
    dynamic_range: u8, reversible: bool, shift: u8, maximum: u64,
) -> Part1ReconstructionResult {
    if exists |i: int| 0 <= i < coefficients.len() && coefficients[i] == i64::MIN {
        Err(part1_transform_error(true))
    } else {
        part1_inverse_quantization(
            part1_integer_coefficients(coefficients), exponent, mantissa,
            dynamic_range, reversible, shift, maximum,
        )
    }
}

pub open spec fn part1_interleaved(
    low: Seq<i64>, high: Seq<i64>, low_first: bool,
) -> Result<Seq<i64>, crate::DecodeError> {
    let length = low.len() + high.len();
    let expected = if low_first { length / 2 + length % 2 } else { length / 2 };
    if length > usize::MAX {
        Err(part1_transform_error(true))
    } else if low.len() != expected || high.len() != length - expected {
        Err(part1_transform_error(false))
    } else {
        Ok(Seq::new(length, |i: int| if (i % 2 == 0) == low_first { low[i / 2] } else { high[i / 2] }))
    }
}

pub open spec fn part1_fixed_product(value: i64, factor: i64) -> Result<i64, crate::DecodeError> {
    let product = (value as int) * (factor as int);
    if product < i64::MIN || product > i64::MAX
        || product + 32768 < i64::MIN || product + 32768 > i64::MAX {
        Err(part1_transform_error(true))
    } else {
        Ok(((product + 32768) as i64) >> 16)
    }
}

// Kinds 0/1 are the reversible update/predict; kind 2 is the fixed-point lift.
pub open spec fn part1_lifting_sample(
    samples: Seq<i64>, index: nat, target_even: bool, kind: u8, factor: i64,
) -> Result<i64, crate::DecodeError> {
    if (index % 2 == 0) != target_even || samples.len() <= 1 {
        Ok(samples[index as int])
    } else {
        let left = samples[if index == 0 { 1 } else { index - 1 }];
        let right = samples[if index + 1 < samples.len() { index as int + 1 } else { index - 1 }];
        match part1_checked_sample((left as int) + (right as int)) {
            Err(error) => Err(error),
            Ok(sum) => {
                let adjustment = if kind == 2 {
                    part1_fixed_product(sum, factor)
                } else if kind == 0 {
                    match part1_checked_sample((sum as int) + 2) {
                        Err(error) => Err(error),
                        Ok(biased) => Ok(biased >> 2),
                    }
                } else {
                    Ok(sum >> 1)
                };
                match adjustment {
                    Err(error) => Err(error),
                    Ok(delta) => part1_checked_sample(
                        (samples[index as int] as int) + if kind == 1 { delta as int } else { -(delta as int) },
                    ),
                }
            },
        }
    }
}

pub type Part1LiftingState = (Result<(), crate::DecodeError>, Seq<i64>);

pub open spec fn part1_lifting_prefix(
    samples: Seq<i64>, target_even: bool, kind: u8, factor: i64, count: nat,
) -> Part1LiftingState
    decreases count,
{
    if count == 0 {
        (Ok(()), samples)
    } else {
        let previous = part1_lifting_prefix(samples, target_even, kind, factor, (count - 1) as nat);
        match previous.0 {
            Err(_) => previous,
            Ok(()) => match part1_lifting_sample(samples, (count - 1) as nat, target_even, kind, factor) {
                Err(error) => (Err(error), previous.1),
                Ok(value) => (Ok(()), previous.1.update(count - 1, value)),
            },
        }
    }
}

pub open spec fn part1_inverse_five_three(
    low: Seq<i64>, high: Seq<i64>, low_first: bool,
) -> Result<Seq<i64>, crate::DecodeError> {
    match part1_interleaved(low, high, low_first) {
        Err(error) => Err(error),
        Ok(samples) => if samples.len() == 1 && !low_first {
            Ok(samples.update(0, samples[0] >> 1))
        } else {
            let update = part1_lifting_prefix(samples, low_first, 0, 0, samples.len());
            match update.0 {
                Err(error) => Err(error),
                Ok(()) => {
                    let predict = part1_lifting_prefix(update.1, !low_first, 1, 0, update.1.len());
                    match predict.0 {
                        Err(error) => Err(error),
                        Ok(()) => Ok(predict.1),
                    }
                },
            }
        },
    }
}

pub open spec fn part1_scaling_prefix(
    samples: Seq<i64>, target_even: bool, factor: i64, count: nat,
) -> Part1LiftingState
    decreases count,
{
    if count == 0 {
        (Ok(()), samples)
    } else {
        let previous = part1_scaling_prefix(samples, target_even, factor, (count - 1) as nat);
        match previous.0 {
            Err(_) => previous,
            Ok(()) => {
                let index = count - 1;
                if (index % 2 == 0) != target_even {
                    previous
                } else {
                    match part1_fixed_product(samples[index], factor) {
                        Err(error) => (Err(error), previous.1),
                        Ok(value) => (Ok(()), previous.1.update(index, value)),
                    }
                }
            },
        }
    }
}

pub open spec fn part1_nine_seven_lifting_prefix(
    samples: Seq<i64>, low_first: bool, count: nat,
) -> Part1LiftingState
    decreases count,
{
    if count == 0 {
        (Ok(()), samples)
    } else {
        let previous = part1_nine_seven_lifting_prefix(samples, low_first, (count - 1) as nat);
        match previous.0 {
            Err(_) => previous,
            Ok(()) => part1_lifting_prefix(
                previous.1, if count % 2 == 1 { low_first } else { !low_first },
                2, part1_nine_seven_lift((count - 1) as u8), previous.1.len(),
            ),
        }
    }
}

pub open spec fn part1_inverse_nine_seven(
    low: Seq<i64>, high: Seq<i64>, low_first: bool,
) -> Result<Seq<i64>, crate::DecodeError> {
    match part1_interleaved(low, high, low_first) {
        Err(error) => Err(error),
        Ok(samples) => if samples.len() <= 1 {
            Ok(samples)
        } else {
            let low_scaled = part1_scaling_prefix(samples, low_first, part1_nine_seven_scale(true), samples.len());
            match low_scaled.0 {
                Err(error) => Err(error),
                Ok(()) => {
                    let high_scaled = part1_scaling_prefix(
                        low_scaled.1, !low_first, part1_nine_seven_scale(false), low_scaled.1.len(),
                    );
                    match high_scaled.0 {
                        Err(error) => Err(error),
                        Ok(()) => {
                            let lifted = part1_nine_seven_lifting_prefix(high_scaled.1, low_first, 4);
                            match lifted.0 {
                                Err(error) => Err(error),
                                Ok(()) => Ok(lifted.1),
                            }
                        },
                    }
                },
            }
        },
    }
}

pub type Part1WaveletBand = (u32, u32, Seq<i64>);

pub open spec fn part1_transform_dimension(value: u32) -> Result<usize, crate::DecodeError> {
    if value > usize::MAX {
        Err(part1_transform_error(true))
    } else {
        Ok(value as usize)
    }
}

pub open spec fn part1_transform_area(width: u32, height: u32) -> Result<usize, crate::DecodeError> {
    if width > usize::MAX || height > usize::MAX || (width as int) * (height as int) > usize::MAX {
        Err(part1_transform_error(true))
    } else {
        Ok(((width as int) * (height as int)) as usize)
    }
}

pub open spec fn part1_low_count(length: u32, low_first: bool) -> u32 {
    ((length as int) / 2 + if low_first { (length as int) % 2 } else { 0 }) as u32
}

pub open spec fn part1_band_row(band: Part1WaveletBand, y: u32) -> Result<Seq<i64>, crate::DecodeError> {
    let start = (y as int) * (band.0 as int);
    let end = start + band.0;
    if y >= band.1 {
        Err(part1_transform_error(false))
    } else if y > usize::MAX || band.0 > usize::MAX || start > usize::MAX || end > usize::MAX {
        Err(part1_transform_error(true))
    } else if end > band.2.len() {
        Err(part1_transform_error(false))
    } else {
        Ok(band.2.subrange(start, end))
    }
}

pub open spec fn part1_inverse_sequence(
    low: Seq<i64>, high: Seq<i64>, low_first: bool, irreversible: bool,
) -> Result<Seq<i64>, crate::DecodeError> {
    if irreversible {
        part1_inverse_nine_seven(low, high, low_first)
    } else {
        part1_inverse_five_three(low, high, low_first)
    }
}

pub open spec fn part1_horizontal_row(
    low: Part1WaveletBand, high: Part1WaveletBand, y: u32, low_first: bool, irreversible: bool,
) -> Result<Seq<i64>, crate::DecodeError> {
    match part1_band_row(low, y) {
        Err(error) => Err(error),
        Ok(low_row) => match part1_band_row(high, y) {
            Err(error) => Err(error),
            Ok(high_row) => part1_inverse_sequence(low_row, high_row, low_first, irreversible),
        },
    }
}

pub open spec fn part1_horizontal_prefix(
    low: Part1WaveletBand, high: Part1WaveletBand, low_first: bool, irreversible: bool, count: nat,
) -> Result<Seq<i64>, crate::DecodeError>
    decreases count,
{
    if count == 0 {
        Ok(Seq::empty())
    } else {
        match part1_horizontal_prefix(low, high, low_first, irreversible, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(previous) => match part1_horizontal_row(low, high, (count - 1) as u32, low_first, irreversible) {
                Err(error) => Err(error),
                Ok(row) => Ok(previous + row),
            },
        }
    }
}

pub open spec fn part1_horizontal_geometry(
    low: Part1WaveletBand, high: Part1WaveletBand, width: u32, low_first: bool,
) -> Result<(), crate::DecodeError> {
    if low.1 == 0 || low.1 != high.1 || low.0 != part1_low_count(width, low_first) {
        Err(part1_transform_error(false))
    } else if low.0 > width {
        Err(part1_transform_error(true))
    } else if high.0 != (width - low.0) as u32 {
        Err(part1_transform_error(false))
    } else {
        Ok(())
    }
}

pub open spec fn part1_inverse_horizontal(
    low: Part1WaveletBand, high: Part1WaveletBand, width: u32,
    low_first: bool, irreversible: bool, maximum: u64,
) -> Part1ReconstructionResult {
    match part1_horizontal_geometry(low, high, width, low_first) {
        Err(error) => Err(error),
        Ok(()) => match part1_transform_area(width, low.1) {
            Err(error) => Err(error),
            Ok(length) => match part1_quantization_budget(length as nat, maximum) {
                Err(error) => Err(error),
                Ok(()) => match part1_horizontal_prefix(low, high, low_first, irreversible, low.1 as nat) {
                    Err(error) => Err(error),
                    Ok(samples) => Ok((width, low.1, if irreversible { 16u8 } else { 0u8 }, samples)),
                },
            },
        },
    }
}

pub open spec fn part1_transform_offset(y: u32, width: u32, x: u32) -> Result<usize, crate::DecodeError> {
    let row = (y as int) * (width as int);
    if y > usize::MAX || width > usize::MAX || x > usize::MAX || row > usize::MAX || row + x > usize::MAX {
        Err(part1_transform_error(true))
    } else {
        Ok((row + x) as usize)
    }
}

pub open spec fn part1_column_sample(
    samples: Seq<i64>, width: u32, x: u32, y: u32,
) -> Result<i64, crate::DecodeError> {
    match part1_transform_offset(y, width, x) {
        Err(error) => Err(error),
        Ok(index) => if index >= samples.len() {
            Err(part1_transform_error(false))
        } else {
            Ok(samples[index as int])
        },
    }
}

pub open spec fn part1_column_prefix(
    samples: Seq<i64>, width: u32, x: u32, count: nat,
) -> Result<Seq<i64>, crate::DecodeError>
    decreases count,
{
    if count == 0 {
        Ok(Seq::empty())
    } else {
        match part1_column_prefix(samples, width, x, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(previous) => match part1_column_sample(samples, width, x, (count - 1) as u32) {
                Err(error) => Err(error),
                Ok(value) => Ok(previous.push(value)),
            },
        }
    }
}

pub open spec fn part1_collect_column(band: Part1WaveletBand, x: u32) -> Result<Seq<i64>, crate::DecodeError> {
    if x >= band.0 {
        Err(part1_transform_error(false))
    } else if band.1 > usize::MAX {
        Err(part1_transform_error(true))
    } else {
        part1_column_prefix(band.2, band.0, x, band.1 as nat)
    }
}

pub open spec fn part1_store_column_sample(
    output: Seq<i64>, width: u32, x: u32, column: Seq<i64>, y: u32,
) -> Part1LiftingState {
    match part1_transform_offset(y, width, x) {
        Err(error) => (Err(error), output),
        Ok(index) => if y >= column.len() || index >= output.len() {
            (Err(part1_transform_error(false)), output)
        } else {
            (Ok(()), output.update(index as int, column[y as int]))
        },
    }
}

pub open spec fn part1_store_column_prefix(
    output: Seq<i64>, width: u32, x: u32, column: Seq<i64>, count: nat,
) -> Part1LiftingState
    decreases count,
{
    if count == 0 {
        (Ok(()), output)
    } else {
        let previous = part1_store_column_prefix(output, width, x, column, (count - 1) as nat);
        match previous.0 {
            Err(_) => previous,
            Ok(()) => part1_store_column_sample(previous.1, width, x, column, (count - 1) as u32),
        }
    }
}

pub open spec fn part1_vertical_column(
    low: Part1WaveletBand, high: Part1WaveletBand, x: u32, low_first: bool, irreversible: bool,
) -> Result<Seq<i64>, crate::DecodeError> {
    match part1_collect_column(low, x) {
        Err(error) => Err(error),
        Ok(low_column) => match part1_collect_column(high, x) {
            Err(error) => Err(error),
            Ok(high_column) => part1_inverse_sequence(low_column, high_column, low_first, irreversible),
        },
    }
}

pub open spec fn part1_vertical_prefix(
    low: Part1WaveletBand, high: Part1WaveletBand, height: u32, low_first: bool,
    irreversible: bool, length: usize, count: nat,
) -> Result<Seq<i64>, crate::DecodeError>
    decreases count,
{
    if count == 0 {
        Ok(Seq::new(length as nat, |i: int| 0i64))
    } else {
        match part1_vertical_prefix(low, high, height, low_first, irreversible, length, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(previous) => match part1_vertical_column(low, high, (count - 1) as u32, low_first, irreversible) {
                Err(error) => Err(error),
                Ok(column) => {
                    let stored = part1_store_column_prefix(previous, low.0, (count - 1) as u32, column, height as nat);
                    match stored.0 {
                        Err(error) => Err(error),
                        Ok(()) => Ok(stored.1),
                    }
                },
            },
        }
    }
}

pub open spec fn part1_vertical_geometry(
    low: Part1WaveletBand, high: Part1WaveletBand, height: u32, low_first: bool,
) -> Result<(), crate::DecodeError> {
    if low.0 == 0 || low.0 != high.0 || low.1 != part1_low_count(height, low_first) {
        Err(part1_transform_error(false))
    } else if low.1 > height {
        Err(part1_transform_error(true))
    } else if high.1 != (height - low.1) as u32 {
        Err(part1_transform_error(false))
    } else {
        Ok(())
    }
}

pub open spec fn part1_inverse_vertical(
    low: Part1WaveletBand, high: Part1WaveletBand, height: u32,
    low_first: bool, irreversible: bool, maximum: u64,
) -> Part1ReconstructionResult {
    match part1_vertical_geometry(low, high, height, low_first) {
        Err(error) => Err(error),
        Ok(()) => match part1_transform_area(low.0, height) {
            Err(error) => Err(error),
            Ok(length) => match part1_quantization_budget(length as nat, maximum) {
                Err(error) => Err(error),
                Ok(()) => match part1_vertical_prefix(low, high, height, low_first, irreversible, length, low.0 as nat) {
                    Err(error) => Err(error),
                    Ok(samples) => Ok((low.0, height, if irreversible { 16u8 } else { 0u8 }, samples)),
                },
            },
        },
    }
}

pub type Part1WaveletLevel = (Part1WaveletBand, Part1WaveletBand, Part1WaveletBand, Part1WaveletBand, u32, u32, bool, bool);

pub open spec fn part1_validate_band(band: Part1WaveletBand, width: u32, height: u32) -> Result<(), crate::DecodeError> {
    if band.0 != width || band.1 != height {
        Err(part1_transform_error(false))
    } else {
        match part1_transform_area(width, height) {
            Err(error) => Err(error),
            Ok(length) => if band.2.len() != length {
                Err(part1_transform_error(false))
            } else {
                Ok(())
            },
        }
    }
}

pub open spec fn part1_validate_level(level: Part1WaveletLevel) -> Result<(), crate::DecodeError> {
    let low_width = part1_low_count(level.4, level.6);
    let high_width = (level.4 - low_width) as u32;
    let low_height = part1_low_count(level.5, level.7);
    let high_height = (level.5 - low_height) as u32;
    if level.4 == 0 || level.5 == 0 {
        Err(part1_transform_error(false))
    } else {
        match part1_validate_band(level.0, low_width, low_height) {
            Err(error) => Err(error),
            Ok(()) => match part1_validate_band(level.1, high_width, low_height) {
                Err(error) => Err(error),
                Ok(()) => match part1_validate_band(level.2, low_width, high_height) {
                    Err(error) => Err(error),
                    Ok(()) => part1_validate_band(level.3, high_width, high_height),
                },
            },
        }
    }
}

pub open spec fn part1_level_rows_and_columns(
    level: Part1WaveletLevel, irreversible: bool, length: usize,
) -> Part1ReconstructionResult {
    match part1_transform_area(level.4, level.0.1) {
        Err(error) => Err(error),
        Ok(_) => match part1_horizontal_prefix(level.0, level.1, level.6, irreversible, level.0.1 as nat) {
            Err(error) => Err(error),
            Ok(low_rows) => match part1_transform_area(level.4, level.2.1) {
                Err(error) => Err(error),
                Ok(_) => match part1_horizontal_prefix(level.2, level.3, level.6, irreversible, level.2.1 as nat) {
                    Err(error) => Err(error),
                    Ok(high_rows) => match part1_vertical_prefix(
                        (level.4, level.0.1, low_rows), (level.4, level.2.1, high_rows),
                        level.5, level.7, irreversible, length, level.4 as nat,
                    ) {
                        Err(error) => Err(error),
                        Ok(samples) => Ok((level.4, level.5, if irreversible { 16u8 } else { 0u8 }, samples)),
                    },
                },
            },
        },
    }
}

pub open spec fn part1_inverse_level(
    level: Part1WaveletLevel, irreversible: bool, maximum: u64,
) -> Part1ReconstructionResult {
    match part1_validate_level(level) {
        Err(error) => Err(error),
        Ok(()) => match part1_transform_area(level.4, level.5) {
            Err(error) => Err(error),
            Ok(length) => if !part1_wavelet_sample_count(level.4, level.5, length) {
                Err(part1_transform_error(false))
            } else {
                match part1_quantization_budget(length as nat, maximum) {
                    Err(error) => Err(error),
                    Ok(()) => part1_level_rows_and_columns(level, irreversible, length),
                }
            },
        },
    }
}

pub open spec fn part1_round_fixed(value: i64, fraction_bits: u8) -> Result<i64, crate::DecodeError> {
    if fraction_bits == 0 {
        Ok(value)
    } else if fraction_bits > 62 {
        Err(part1_transform_error(false))
    } else {
        let half = 1i64 << ((fraction_bits - 1) as u8);
        match part1_checked_sample((value as int) + (half as int)) {
            Err(error) => Err(error),
            Ok(biased) => Ok(biased >> fraction_bits),
        }
    }
}

pub open spec fn part1_convert_sample(
    value: i64, fraction_bits: u8, shift: i64, minimum: i64, maximum: i64,
) -> Result<i64, crate::DecodeError> {
    match part1_round_fixed(value, fraction_bits) {
        Err(error) => Err(error),
        Ok(rounded) => match part1_checked_sample((rounded as int) + (shift as int)) {
            Err(error) => Err(error),
            Ok(shifted) => Ok(part1_clip_sample(shifted, minimum, maximum)),
        },
    }
}

pub open spec fn part1_conversion_prefix(
    samples: Seq<i64>, fraction_bits: u8, shift: i64, minimum: i64, maximum: i64, count: nat,
) -> Result<Seq<i64>, crate::DecodeError>
    decreases count,
{
    if count == 0 {
        Ok(Seq::empty())
    } else {
        match part1_conversion_prefix(samples, fraction_bits, shift, minimum, maximum, (count - 1) as nat) {
            Err(error) => Err(error),
            Ok(previous) => match part1_convert_sample(samples[count - 1], fraction_bits, shift, minimum, maximum) {
                Err(error) => Err(error),
                Ok(value) => Ok(previous.push(value)),
            },
        }
    }
}

pub open spec fn part1_output_maximum(precision: u8, signed: bool) -> i64 {
    (((if signed { 1i64 << ((precision - 1) as u8) } else { 1i64 << precision }) as int) - 1) as i64
}

pub open spec fn part1_output_minimum(precision: u8, signed: bool) -> i64 {
    if signed { (-((1i64 << ((precision - 1) as u8)) as int)) as i64 } else { 0 }
}

pub open spec fn part1_convert_samples(
    plane: Part1ReconstructedPlane, precision: u8, signed: bool, maximum_bytes: u64,
) -> Result<Seq<i64>, crate::DecodeError> {
    if precision == 0 || precision > 38 || plane.2 > 62 {
        Err(part1_transform_error(false))
    } else {
        match part1_quantization_budget(plane.3.len(), maximum_bytes) {
            Err(error) => Err(error),
            Ok(()) => part1_conversion_prefix(
                plane.3, plane.2, part1_level_shift(precision, signed),
                part1_output_minimum(precision, signed), part1_output_maximum(precision, signed), plane.3.len(),
            ),
        }
    }
}

pub open spec fn part1_image_error(arithmetic: bool) -> crate::DecodeError {
    crate::DecodeError::MalformedInput {
        offset: 0,
        kind: if arithmetic { crate::MalformedKind::ArithmeticOverflow }
            else { crate::MalformedKind::InvalidImageGeometry },
    }
}

pub open spec fn part1_image_size_component(
    components: u16, information: Seq<crate::codestream::ComponentInfo>, index: u16,
) -> Result<crate::codestream::ComponentInfo, crate::DecodeError> {
    if index >= components || index >= information.len() {
        Err(part1_image_error(false))
    } else {
        Ok(information[index as int])
    }
}

pub open spec fn part1_image_checked_u32(value: int) -> Result<u32, crate::DecodeError> {
    if value < 0 || value > u32::MAX { Err(part1_image_error(true)) } else { Ok(value as u32) }
}

pub open spec fn part1_image_ceil_div(value: u32, divisor: u32) -> Result<u32, crate::DecodeError> {
    if divisor == 0 { Err(part1_image_error(false)) }
    else { Ok(part1_ceil_div(value as nat, divisor as nat) as u32) }
}

pub open spec fn part1_assembly_registered_ceil(value: u32, divisor: u32, registration: u16)
    -> Result<u32, crate::DecodeError>
{
    if divisor == 0 { Err(part1_image_error(false)) }
    else { color_registered_coordinate(value, divisor, registration, true) }
}

pub open spec fn part1_accumulate_decoded_bytes(samples: nat, before: u64, maximum: u64)
    -> (Result<(), crate::DecodeError>, u64)
{
    let bytes = samples * 8;
    if bytes > u64::MAX || before as nat + bytes > u64::MAX {
        (Err(part1_image_error(true)), before)
    } else {
        let next = (before as nat + bytes) as u64;
        if next > maximum {
            (Err(crate::DecodeError::LimitExceeded {
                resource: crate::Resource::DecodedBytes, limit: maximum, actual: next,
            }), next)
        } else { (Ok(()), next) }
    }
}

pub open spec fn part1_assembly_write_indices(
    plane: Part1AssemblyPlane, tile: Part1AssemblyTile, destination: usize, source: usize,
) -> Part1AssemblyPlaneState {
    if destination >= plane.3.len() || destination >= plane.4.len()
        || source >= tile.6.len() || plane.4[destination as int] {
        (Err(part1_image_error(false)), plane)
    } else {
        (Ok(()), (plane.0, plane.1, plane.2,
            plane.3.update(destination as int, tile.6[source as int]), plane.4.update(destination as int, true)))
    }
}

pub open spec fn part1_assembly_pixel(
    plane: Part1AssemblyPlane, tile: Part1AssemblyTile, x: u32, y: u32, crop: Option<(u32, u32)>,
) -> Part1AssemblyPlaneState {
    let destination_x = match crop {
        Some(origin) => part1_image_checked_u32((x as int) - origin.0),
        None => part1_image_checked_u32((tile.1 as int) + x),
    };
    let destination_y = match crop {
        Some(origin) => part1_image_checked_u32((y as int) - origin.1),
        None => part1_image_checked_u32((tile.2 as int) + y),
    };
    match destination_x {
        Err(error) => (Err(error), plane),
        Ok(dx) => match destination_y {
            Err(error) => (Err(error), plane),
            Ok(dy) => match part1_transform_offset(dy, plane.0, dx) {
                Err(error) => (Err(error), plane),
                Ok(destination) => {
                    let source_x = match crop {
                        Some(_) => part1_image_checked_u32((x as int) - tile.1),
                        None => Ok(x),
                    };
                    let source_y = match crop {
                        Some(_) => part1_image_checked_u32((y as int) - tile.2),
                        None => Ok(y),
                    };
                    match source_x {
                        Err(error) => (Err(error), plane),
                        Ok(sx) => match source_y {
                            Err(error) => (Err(error), plane),
                            Ok(sy) => match part1_transform_offset(sy, tile.3, sx) {
                                Err(error) => (Err(error), plane),
                                Ok(source) => part1_assembly_write_indices(plane, tile, destination, source),
                            },
                        },
                    }
                },
            },
        },
    }
}

pub open spec fn part1_assembly_row_prefix(
    plane: Part1AssemblyPlane, tile: Part1AssemblyTile, x_start: u32, y: u32,
    crop: Option<(u32, u32)>, completed: nat,
) -> Part1AssemblyPlaneState
    decreases completed,
{
    if completed == 0 { (Ok(()), plane) } else {
        let previous = part1_assembly_row_prefix(plane, tile, x_start, y, crop, (completed - 1) as nat);
        match previous.0 {
            Err(_) => previous,
            Ok(()) => part1_assembly_pixel(previous.1, tile, (x_start as nat + completed - 1) as u32, y, crop),
        }
    }
}

pub open spec fn part1_assembly_rectangle_prefix(
    plane: Part1AssemblyPlane, tile: Part1AssemblyTile, rectangle: (u32, u32, u32, u32),
    crop: Option<(u32, u32)>, completed: nat,
) -> Part1AssemblyPlaneState
    decreases completed,
{
    if completed == 0 { (Ok(()), plane) } else {
        let previous = part1_assembly_rectangle_prefix(plane, tile, rectangle, crop, (completed - 1) as nat);
        match previous.0 {
            Err(_) => previous,
            Ok(()) => part1_assembly_row_prefix(
                previous.1, tile, rectangle.0, (rectangle.1 as nat + completed - 1) as u32, crop,
                (rectangle.2 as nat - rectangle.0 as nat) as nat,
            ),
        }
    }
}

pub open spec fn part1_assembly_rectangle(
    plane: Part1AssemblyPlane, tile: Part1AssemblyTile, rectangle: (u32, u32, u32, u32),
    crop: Option<(u32, u32)>,
) -> Part1AssemblyPlaneState {
    if rectangle.0 >= rectangle.2 || rectangle.1 >= rectangle.3 {
        (Ok(()), plane)
    } else {
        part1_assembly_rectangle_prefix(plane, tile, rectangle, crop, (rectangle.3 - rectangle.1) as nat)
    }
}

pub open spec fn part1_validate_assembly_tile(plane: Part1AssemblyPlane, tile: Part1AssemblyTile)
    -> Result<(), crate::DecodeError>
{
    if tile.3 == 0 || tile.4 == 0 { Err(part1_image_error(false)) } else {
        match part1_image_checked_u32((tile.1 as int) + tile.3) {
            Err(error) => Err(error),
            Ok(x1) => if x1 > plane.0 { Err(part1_image_error(false)) } else {
                match part1_image_checked_u32((tile.2 as int) + tile.4) {
                    Err(error) => Err(error),
                    Ok(y1) => if y1 > plane.1 { Err(part1_image_error(false)) } else {
                        match part1_transform_area(tile.3, tile.4) {
                            Err(error) => Err(error),
                            Ok(count) => if tile.6.len() != count { Err(part1_image_error(false)) } else { Ok(()) },
                        }
                    },
                }
            },
        }
    }
}

pub open spec fn part1_place_assembly_tile(plane: Part1AssemblyPlane, tile: Part1AssemblyTile)
    -> Part1AssemblyPlaneState
{
    match part1_validate_assembly_tile(plane, tile) {
        Err(error) => (Err(error), plane),
        Ok(()) => part1_assembly_rectangle_prefix(plane, tile, (0, 0, tile.3, tile.4), None, tile.4 as nat),
    }
}

pub open spec fn part1_place_selected_tile(
    plane: Part1AssemblyPlane, tile: Part1AssemblyTile, crop: (u32, u32, u32, u32),
) -> Part1AssemblyPlaneState {
    let x0 = if tile.1 > crop.0 { tile.1 } else { crop.0 };
    let y0 = if tile.2 > crop.1 { tile.2 } else { crop.1 };
    match part1_image_checked_u32((tile.1 as int) + tile.3) {
        Err(error) => (Err(error), plane),
        Ok(tile_x1) => match part1_image_checked_u32((tile.2 as int) + tile.4) {
            Err(error) => (Err(error), plane),
            Ok(tile_y1) => {
                let x1 = if tile_x1 < crop.2 { tile_x1 } else { crop.2 };
                let y1 = if tile_y1 < crop.3 { tile_y1 } else { crop.3 };
                if x0 < x1 && y0 < y1 {
                    part1_assembly_rectangle_prefix(
                        plane, tile, (x0, y0, x1, y1), Some((crop.0, crop.1)), (y1 - y0) as nat,
                    )
                } else { (Ok(()), plane) }
            },
        },
    }
}

#[verifier::opaque]
pub open spec fn part1_assembly_tile_step(
    plane: Part1AssemblyPlane, tile: Part1AssemblyTile, component: u16, crop: Option<(u32, u32, u32, u32)>,
) -> Part1AssemblyPlaneState {
    if tile.0 != component {
        (Ok(()), plane)
    } else if plane.2.is_some() && plane.2 != Some(tile.5) {
        (Err(part1_image_error(false)), plane)
    } else {
        let changed = (plane.0, plane.1, Some(tile.5), plane.3, plane.4);
        match crop {
            None => part1_place_assembly_tile(changed, tile),
            Some(region) => part1_place_selected_tile(changed, tile, region),
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_assembly_tiles_prefix(
    plane: Part1AssemblyPlane, tiles: Seq<Part1AssemblyTile>, component: u16,
    crop: Option<(u32, u32, u32, u32)>, completed: nat,
) -> Part1AssemblyPlaneState
    decreases completed,
{
    if completed == 0 { (Ok(()), plane) } else {
        let previous = part1_assembly_tiles_prefix(plane, tiles, component, crop, (completed - 1) as nat);
        match previous.0 {
            Err(_) => previous,
            Ok(()) => part1_assembly_tile_step(previous.1, tiles[completed - 1], component, crop),
        }
    }
}

pub open spec fn part1_assembly_coverage_error(selected: bool) -> crate::DecodeError {
    if selected {
        crate::DecodeError::InvalidSelection { reason: crate::SelectionError::RegionOutsideSelectedTiles }
    } else { part1_image_error(false) }
}

pub open spec fn part1_collect_assembly_tiles(
    plane: Part1AssemblyPlane, tiles: Seq<Part1AssemblyTile>, component: u16,
    crop: Option<(u32, u32, u32, u32)>,
) -> Part1AssemblyPlaneState {
    let result = part1_assembly_tiles_prefix(plane, tiles, component, crop, tiles.len());
    match result.0 {
        Err(_) => result,
        Ok(()) => if result.1.2.is_none() || !(forall |i: int| 0 <= i < result.1.4.len() ==> result.1.4[i]) {
            (Err(part1_assembly_coverage_error(crop.is_some())), result.1)
        } else { result },
    }
}

pub open spec fn part1_assembly_base_dimensions(
    size: Part1AssemblySize, info: crate::codestream::ComponentInfo, extent: (u32, u32),
) -> Result<Part1AssemblyDimensions, crate::DecodeError> {
    match part1_assembly_registered_ceil(extent.0, info.x_subsampling as u32, info.x_registration) {
        Err(error) => Err(error),
        Ok(x1) => match part1_assembly_registered_ceil(size.2, info.x_subsampling as u32, info.x_registration) {
            Err(error) => Err(error),
            Ok(x0) => match part1_image_checked_u32((x1 as int) - x0) {
                Err(error) => Err(error),
                Ok(width) => match part1_assembly_registered_ceil(extent.1, info.y_subsampling as u32, info.y_registration) {
                    Err(error) => Err(error),
                    Ok(y1) => match part1_assembly_registered_ceil(size.3, info.y_subsampling as u32, info.y_registration) {
                        Err(error) => Err(error),
                        Ok(y0) => match part1_image_checked_u32((y1 as int) - y0) {
                            Err(error) => Err(error),
                            Ok(height) => Ok((info, width, height, info.x_subsampling as u32, info.y_subsampling as u32, x0, y0)),
                        },
                    },
                },
            },
        },
    }
}

pub open spec fn part1_assembly_selected_dimensions(
    info: crate::codestream::ComponentInfo, region: [u32; 4], scale: u32,
) -> Result<Part1AssemblyDimensions, crate::DecodeError> {
    match part1_image_checked_u32((info.x_subsampling as int) * (scale as int)) {
        Err(error) => Err(error),
        Ok(x_step) => match part1_image_checked_u32((info.y_subsampling as int) * (scale as int)) {
            Err(error) => Err(error),
            Ok(y_step) => match part1_assembly_registered_ceil(region[0], x_step, info.x_registration) {
                Err(error) => Err(error),
                Ok(x0) => match part1_assembly_registered_ceil(region[1], y_step, info.y_registration) {
                    Err(error) => Err(error),
                    Ok(y0) => match part1_assembly_registered_ceil(region[2], x_step, info.x_registration) {
                        Err(error) => Err(error),
                        Ok(x1) => match part1_image_checked_u32((x1 as int) - x0) {
                            Err(error) => Err(error),
                            Ok(width) => match part1_assembly_registered_ceil(region[3], y_step, info.y_registration) {
                                Err(error) => Err(error),
                                Ok(y1) => match part1_image_checked_u32((y1 as int) - y0) {
                                    Err(error) => Err(error),
                                    Ok(height) => if width == 0 || height == 0 {
                                        Err(crate::DecodeError::InvalidSelection { reason: crate::SelectionError::EmptyComponentIntersection })
                                    } else { Ok((info, width, height, x_step, y_step, x0, y0)) },
                                },
                            },
                        },
                    },
                },
            },
        },
    }
}

pub open spec fn part1_assembly_component_dimensions(
    size: Part1AssemblySize, component: u16, extent: (u32, u32), selection: Part1AssemblySelection,
) -> Result<Part1AssemblyDimensions, crate::DecodeError> {
    match part1_image_size_component(size.4, size.5, component) {
        Err(error) => Err(error),
        Ok(info) => match selection {
            None => part1_assembly_base_dimensions(size, info, extent),
            Some((region, scale)) => part1_assembly_selected_dimensions(info, region, scale),
        },
    }
}

pub open spec fn part1_assembly_component_crop(
    size: Part1AssemblySize, dimensions: Part1AssemblyDimensions, selected: bool,
) -> Result<Option<(u32, u32, u32, u32)>, crate::DecodeError> {
    if !selected { Ok(None) } else {
        match part1_assembly_registered_ceil(size.2, dimensions.3, dimensions.0.x_registration) {
            Err(error) => Err(error),
            Ok(full_x0) => match part1_image_checked_u32((dimensions.5 as int) - full_x0) {
                Err(error) => Err(error),
                Ok(x0) => match part1_assembly_registered_ceil(size.3, dimensions.4, dimensions.0.y_registration) {
                    Err(error) => Err(error),
                    Ok(full_y0) => match part1_image_checked_u32((dimensions.6 as int) - full_y0) {
                        Err(error) => Err(error),
                        Ok(y0) => match part1_image_checked_u32((x0 as int) + dimensions.1) {
                            Err(error) => Err(error),
                            Ok(x1) => match part1_image_checked_u32((y0 as int) + dimensions.2) {
                                Err(error) => Err(error),
                                Ok(y1) => Ok(Some((x0, y0, x1, y1))),
                            },
                        },
                    },
                },
            },
        }
    }
}

pub open spec fn part1_assembly_append_plane(
    image: Part1DecodedImageModel, bytes: u64, size: Part1AssemblySize, tiles: Seq<Part1AssemblyTile>,
    component: u16, extent: (u32, u32), selection: Part1AssemblySelection, maximum: u64,
) -> Part1AssemblyState {
    match part1_assembly_component_dimensions(size, component, extent, selection) {
        Err(error) => (Err(error), image, bytes),
        Ok(dimensions) => match part1_transform_area(dimensions.1, dimensions.2) {
            Err(error) => (Err(error), image, bytes),
            Ok(count) => {
                let charged = part1_accumulate_decoded_bytes(count as nat, bytes, maximum);
                match charged.0 {
                    Err(error) => (Err(error), image, charged.1),
                    Ok(()) => match part1_assembly_component_crop(size, dimensions, selection.is_some()) {
                        Err(error) => (Err(error), image, charged.1),
                        Ok(crop) => {
                            let initial = (dimensions.1, dimensions.2, None,
                                Seq::new(count as nat, |i: int| 0i64), Seq::new(count as nat, |i: int| false));
                            let collected = part1_collect_assembly_tiles(initial, tiles, component, crop);
                            match collected.0 {
                                Err(error) => (Err(error), image, charged.1),
                                Ok(()) => match collected.1.2 {
                                    None => (Err(part1_image_error(false)), image, charged.1),
                                    Some(fraction_bits) => (Ok(()),
                                        (image.0, image.1, image.2, image.3, image.4.push((
                                            dimensions.1, dimensions.2, dimensions.0, fraction_bits, collected.1.3,
                                        ))), charged.1),
                                },
                            }
                        },
                    },
                }
            },
        },
    }
}

#[verifier::opaque]
pub open spec fn part1_assembly_components_prefix(
    image: Part1DecodedImageModel, bytes: u64, size: Part1AssemblySize, tiles: Seq<Part1AssemblyTile>,
    extent: (u32, u32), selection: Part1AssemblySelection, maximum: u64, completed: nat,
) -> Part1AssemblyState
    decreases completed,
{
    if completed == 0 { (Ok(()), image, bytes) } else {
        let previous = part1_assembly_components_prefix(image, bytes, size, tiles, extent, selection, maximum, (completed - 1) as nat);
        match previous.0 {
            Err(_) => previous,
            Ok(()) => part1_assembly_append_plane(previous.1, previous.2, size, tiles, (completed - 1) as u16, extent, selection, maximum),
        }
    }
}

pub open spec fn part1_assemble_image(size: Part1AssemblySize, tiles: Seq<Part1AssemblyTile>, maximum: u64)
    -> Result<Part1DecodedImageModel, crate::DecodeError>
{
    match part1_image_checked_u32((size.2 as int) + size.0) {
        Err(error) => Err(error),
        Ok(x1) => match part1_image_checked_u32((size.3 as int) + size.1) {
            Err(error) => Err(error),
            Ok(y1) => {
                let initial = (size.0, size.1, size.2, size.3, Seq::empty());
                let result = part1_assembly_components_prefix(initial, 0, size, tiles, (x1, y1), None, maximum, size.4 as nat);
                match result.0 { Err(error) => Err(error), Ok(()) => Ok(result.1) }
            },
        },
    }
}

pub open spec fn part1_assembly_selected_header(region: [u32; 4], discard: u8)
    -> Result<(u32, u32, u32, u32, u32), crate::DecodeError>
{
    if region[0] >= region[2] || region[1] >= region[3] || discard > 31 {
        Err(part1_image_error(false))
    } else {
        let scale = 1u32 << discard;
        match part1_image_ceil_div(region[0], scale) {
            Err(error) => Err(error),
            Ok(x0) => match part1_image_ceil_div(region[1], scale) {
                Err(error) => Err(error),
                Ok(y0) => match part1_image_ceil_div(region[2], scale) {
                    Err(error) => Err(error),
                    Ok(x1) => match part1_image_checked_u32((x1 as int) - x0) {
                        Err(error) => Err(error),
                        Ok(width) => match part1_image_ceil_div(region[3], scale) {
                            Err(error) => Err(error),
                            Ok(y1) => match part1_image_checked_u32((y1 as int) - y0) {
                                Err(error) => Err(error),
                                Ok(height) => if width == 0 || height == 0 { Err(part1_image_error(false)) }
                                    else { Ok((scale, x0, y0, width, height)) },
                            },
                        },
                    },
                },
            },
        }
    }
}

pub open spec fn part1_assemble_selected_image(
    size: Part1AssemblySize, tiles: Seq<Part1AssemblyTile>, region: [u32; 4], discard: u8, maximum: u64,
) -> Result<Part1DecodedImageModel, crate::DecodeError> {
    match part1_assembly_selected_header(region, discard) {
        Err(error) => Err(error),
        Ok(header) => {
            let initial = (header.3, header.4, header.1, header.2, Seq::empty());
            let result = part1_assembly_components_prefix(initial, 0, size, tiles, (0, 0), Some((region, header.0)), maximum, size.4 as nat);
            match result.0 { Err(error) => Err(error), Ok(()) => Ok(result.1) }
        },
    }
}

pub open spec fn part1_selector_error(arithmetic: bool) -> crate::DecodeError {
    crate::DecodeError::MalformedInput {
        offset: 0,
        kind: if arithmetic { crate::MalformedKind::ArithmeticOverflow }
            else { crate::MalformedKind::InvalidCodingStyle },
    }
}

pub open spec fn part1_decomposition_direction(
    levels: u8, directions: Seq<crate::codestream::DecompositionDirection>, stage: u8,
) -> Result<crate::codestream::DecompositionDirection, crate::DecodeError> {
    if stage >= levels || stage >= directions.len() {
        Err(part1_selector_error(false))
    } else {
        Ok(directions[stage as int])
    }
}

pub open spec fn part1_direction_for_resolution(
    levels: u8, directions: Seq<crate::codestream::DecompositionDirection>, resolution: u8,
) -> Result<crate::codestream::DecompositionDirection, crate::DecodeError> {
    if resolution == 0 || resolution > levels {
        Err(part1_selector_error(false))
    } else {
        part1_decomposition_direction(levels, directions, (levels - resolution) as u8)
    }
}

pub open spec fn part1_direction_detail_count(direction: crate::codestream::DecompositionDirection) -> nat {
    match direction {
        crate::codestream::DecompositionDirection::None => 0,
        crate::codestream::DecompositionDirection::Horizontal => 1,
        crate::codestream::DecompositionDirection::Vertical => 1,
        crate::codestream::DecompositionDirection::TwoDimensional => 3,
    }
}

pub open spec fn part1_subband_count_prefix(
    levels: u8, directions: Seq<crate::codestream::DecompositionDirection>, completed: nat,
) -> Result<usize, crate::DecodeError>
    decreases completed,
{
    if completed == 0 {
        Ok(1)
    } else {
        match part1_subband_count_prefix(levels, directions, (completed - 1) as nat) {
            Err(error) => Err(error),
            Ok(previous) => match part1_direction_for_resolution(levels, directions, completed as u8) {
                Err(error) => Err(error),
                Ok(direction) => {
                    let next = previous as nat + part1_direction_detail_count(direction);
                    if next > usize::MAX || completed as u8 == u8::MAX {
                        Err(part1_selector_error(true))
                    } else {
                        Ok(next as usize)
                    }
                },
            },
        }
    }
}

pub open spec fn part1_subband_count_through_resolution(
    levels: u8, directions: Seq<crate::codestream::DecompositionDirection>, resolution: u8,
) -> Result<usize, crate::DecodeError> {
    if resolution > levels {
        Err(part1_selector_error(false))
    } else {
        part1_subband_count_prefix(levels, directions, resolution as nat)
    }
}

pub open spec fn part1_decoded_component_sample(
    image: Part1DecodedImageModel, component: usize, index: usize,
) -> Result<i64, crate::DecodeError> {
    if component >= image.4.len() {
        Err(part1_image_error(false))
    } else if index >= image.4[component as int].4.len() {
        Err(part1_image_error(false))
    } else {
        Ok(image.4[component as int].4[index as int])
    }
}

pub open spec fn part1_set_decoded_component_sample(
    image: Part1DecodedImageModel, component: usize, index: usize, value: i64,
) -> Part1DecodedImageState {
    if component >= image.4.len() {
        (Err(part1_image_error(false)), image)
    } else {
        let original = image.4[component as int];
        if index >= original.4.len() {
            (Err(part1_image_error(false)), image)
        } else {
            let changed = (original.0, original.1, original.2, original.3, original.4.update(index as int, value));
            (Ok(()), (image.0, image.1, image.2, image.3, image.4.update(component as int, changed)))
        }
    }
}

pub open spec fn part1_component_transform_compatible(
    first: Part1DecodedComponentModel, component: Part1DecodedComponentModel, fraction_bits: u8,
) -> bool {
    component.0 == first.0 && component.1 == first.1 && component.3 == fraction_bits
        && component.2.x_subsampling == first.2.x_subsampling
        && component.2.y_subsampling == first.2.y_subsampling
}

pub open spec fn part1_validate_transform_components(
    image: Part1DecodedImageModel, fraction_bits: u8,
) -> Result<(), crate::DecodeError> {
    if image.4.len() >= 3 && forall |i: int| 0 <= i < 3 ==>
        part1_component_transform_compatible(image.4[0], image.4[i], fraction_bits)
    {
        Ok(())
    } else {
        Err(part1_image_error(false))
    }
}

pub open spec fn part1_reversible_component_values(y: i64, cb: i64, cr: i64)
    -> Result<(i64, i64, i64), crate::DecodeError>
{
    match part1_checked_sample((cb as int) + (cr as int)) {
        Err(error) => Err(error),
        Ok(sum) => match part1_checked_sample((y as int) - ((sum >> 2) as int)) {
            Err(error) => Err(error),
            Ok(green) => match part1_checked_sample((green as int) + (cr as int)) {
                Err(error) => Err(error),
                Ok(red) => match part1_checked_sample((green as int) + (cb as int)) {
                    Err(error) => Err(error),
                    Ok(blue) => Ok((red, green, blue)),
                },
            },
        },
    }
}

pub open spec fn part1_irreversible_component_values(y: i64, cb: i64, cr: i64)
    -> Result<(i64, i64, i64), crate::DecodeError>
{
    match part1_fixed_product(cr, 91881) {
        Err(error) => Err(error),
        Ok(red_delta) => match part1_checked_sample((y as int) + (red_delta as int)) {
            Err(error) => Err(error),
            Ok(red) => match part1_fixed_product(cb, 22553) {
                Err(error) => Err(error),
                Ok(green_cb) => match part1_checked_sample((y as int) - (green_cb as int)) {
                    Err(error) => Err(error),
                    Ok(green_first) => match part1_fixed_product(cr, 46797) {
                        Err(error) => Err(error),
                        Ok(green_cr) => match part1_checked_sample((green_first as int) - (green_cr as int)) {
                            Err(error) => Err(error),
                            Ok(green) => match part1_fixed_product(cb, 116130) {
                                Err(error) => Err(error),
                                Ok(blue_delta) => match part1_checked_sample((y as int) + (blue_delta as int)) {
                                    Err(error) => Err(error),
                                    Ok(blue) => Ok((red, green, blue)),
                                },
                            },
                        },
                    },
                },
            },
        },
    }
}

pub open spec fn part1_component_values(y: i64, cb: i64, cr: i64, irreversible: bool)
    -> Result<(i64, i64, i64), crate::DecodeError>
{
    if irreversible { part1_irreversible_component_values(y, cb, cr) }
    else { part1_reversible_component_values(y, cb, cr) }
}

pub open spec fn part1_component_pixel_state(
    image: Part1DecodedImageModel, index: usize, irreversible: bool,
) -> Part1DecodedImageState {
    match part1_decoded_component_sample(image, 0, index) {
        Err(error) => (Err(error), image),
        Ok(y) => match part1_decoded_component_sample(image, 1, index) {
            Err(error) => (Err(error), image),
            Ok(cb) => match part1_decoded_component_sample(image, 2, index) {
                Err(error) => (Err(error), image),
                Ok(cr) => match part1_component_values(y, cb, cr, irreversible) {
                    Err(error) => (Err(error), image),
                    Ok((red, green, blue)) => {
                        let red_state = part1_set_decoded_component_sample(image, 0, index, red);
                        match red_state.0 {
                            Err(_) => red_state,
                            Ok(()) => {
                                let green_state = part1_set_decoded_component_sample(red_state.1, 1, index, green);
                                match green_state.0 {
                                    Err(_) => green_state,
                                    Ok(()) => part1_set_decoded_component_sample(green_state.1, 2, index, blue),
                                }
                            },
                        }
                    },
                },
            },
        },
    }
}

pub open spec fn part1_component_transform_prefix(
    image: Part1DecodedImageModel, irreversible: bool, count: nat,
) -> Part1DecodedImageState
    decreases count,
{
    if count == 0 {
        (Ok(()), image)
    } else {
        let previous = part1_component_transform_prefix(image, irreversible, (count - 1) as nat);
        match previous.0 {
            Err(_) => previous,
            Ok(()) => part1_component_pixel_state(previous.1, (count - 1) as usize, irreversible),
        }
    }
}

pub open spec fn part1_apply_component_transform(
    image: Part1DecodedImageModel, irreversible: bool,
) -> Part1DecodedImageState {
    match part1_validate_transform_components(image, if irreversible { 16u8 } else { 0u8 }) {
        Err(error) => (Err(error), image),
        Ok(()) => if image.4.len() < 3
            || image.4[1].4.len() != image.4[0].4.len()
            || image.4[2].4.len() != image.4[0].4.len()
        {
            (Err(part1_image_error(false)), image)
        } else {
            part1_component_transform_prefix(image, irreversible, image.4[0].4.len())
        },
    }
}

pub open spec fn part1_component_transform_frame(
    before: Part1DecodedImageModel, after: Part1DecodedImageModel,
) -> bool {
    before.0 == after.0 && before.1 == after.1 && before.2 == after.2 && before.3 == after.3
        && before.4.len() == after.4.len()
        && forall |i: int| 0 <= i < before.4.len() ==>
            before.4[i].0 == after.4[i].0
                && before.4[i].1 == after.4[i].1
                && before.4[i].2 == after.4[i].2
                && before.4[i].3 == after.4[i].3
                && before.4[i].4.len() == after.4[i].4.len()
                && (i >= 3 ==> before.4[i] == after.4[i])
}

pub open spec fn part1_block_placement_error(arithmetic: bool) -> crate::DecodeError {
    crate::DecodeError::MalformedInput {
        offset: 0,
        kind: if arithmetic { crate::MalformedKind::ArithmeticOverflow }
            else { crate::MalformedKind::InvalidCodeBlock },
    }
}

pub open spec fn part1_quantized_block_pixel(
    band: Part1WorkingSubbandModel, source: Seq<i64>, block_width: u32,
    placement_x: u32, placement_y: u32, x: u32, y: u32,
) -> Part1WorkingSubbandState {
    let destination_x = (placement_x as int) + (x as int);
    let destination_y = (placement_y as int) + (y as int);
    if destination_x > u32::MAX || destination_y > u32::MAX {
        (Err(part1_block_placement_error(true)), band)
    } else {
        match part1_transform_offset(destination_y as u32, band.0, destination_x as u32) {
            Err(error) => (Err(error), band),
            Ok(destination) => match part1_transform_offset(y, block_width, x) {
                Err(error) => (Err(error), band),
                Ok(input) => if destination >= band.3.len() || destination >= band.4.len()
                    || input >= source.len() || band.4[destination as int]
                {
                    (Err(part1_block_placement_error(false)), band)
                } else {
                    (Ok(()), (band.0, band.1, band.2,
                        band.3.update(destination as int, source[input as int]),
                        band.4.update(destination as int, true)))
                },
            },
        }
    }
}

pub open spec fn part1_quantized_block_row(
    band: Part1WorkingSubbandModel, source: Seq<i64>, block_width: u32,
    placement_x: u32, placement_y: u32, y: u32, count: nat,
) -> Part1WorkingSubbandState
    decreases count,
{
    if count == 0 {
        (Ok(()), band)
    } else {
        let previous = part1_quantized_block_row(band, source, block_width, placement_x, placement_y, y, (count - 1) as nat);
        match previous.0 {
            Err(_) => previous,
            Ok(()) => part1_quantized_block_pixel(previous.1, source, block_width, placement_x, placement_y, (count - 1) as u32, y),
        }
    }
}

pub open spec fn part1_quantized_block_rows(
    band: Part1WorkingSubbandModel, source: Seq<i64>, block_width: u32,
    placement_x: u32, placement_y: u32, count: nat,
) -> Part1WorkingSubbandState
    decreases count,
{
    if count == 0 {
        (Ok(()), band)
    } else {
        let previous = part1_quantized_block_rows(band, source, block_width, placement_x, placement_y, (count - 1) as nat);
        match previous.0 {
            Err(_) => previous,
            Ok(()) => part1_quantized_block_row(previous.1, source, block_width, placement_x, placement_y, (count - 1) as u32, block_width as nat),
        }
    }
}

pub open spec fn part1_working_subband_frame(
    before: Part1WorkingSubbandModel, after: Part1WorkingSubbandModel,
) -> bool {
    before.0 == after.0 && before.1 == after.1 && before.2 == after.2
        && before.3.len() == after.3.len() && before.4.len() == after.4.len()
        && (forall |i: int| 0 <= i < before.4.len() && before.4[i] ==> after.4[i])
        && (forall |i: int| 0 <= i < before.4.len() && i < before.3.len() && before.4[i] ==>
            before.3[i] == after.3[i])
}

}
