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
