use crate::{
    DecodeError, MalformedKind, Resource, ResourceLimits, UnsupportedColorConversion,
    image::{ChannelAssociation, ChannelType, TypedComponent, TypedImage, TypedSamples},
    jp2::{
        ColorMethod, ColorSpecification, ENUMERATED_GRAYSCALE, ENUMERATED_SRGB, ENUMERATED_SYCC,
    },
};

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

const OUTPUT_MAXIMUM: u64 = u16::MAX as u64;
const FIXED_ONE: i128 = 65_536;

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum ConvertedColorSpace {
    Srgb,
    Grayscale,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum ConvertedChannelType {
    Red,
    Green,
    Blue,
    Gray,
    Alpha {
        premultiplied: bool,
        association: ChannelAssociation,
    },
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ConvertedChannel {
    channel_type: ConvertedChannelType,
    samples: Vec<u16>,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ConvertedImage {
    width: u32,
    height: u32,
    color_space: ConvertedColorSpace,
    precision: u8,
    channels: Vec<ConvertedChannel>,
}

impl ConvertedChannel {
    pub const fn channel_type(&self) -> ConvertedChannelType {
        self.channel_type
    }

    pub fn samples(&self) -> &[u16] {
        &self.samples
    }
}

impl ConvertedImage {
    pub(crate) fn from_composited(
        width: u32,
        height: u32,
        color_space: ConvertedColorSpace,
        color_samples: Vec<Vec<u16>>,
        alpha_samples: Vec<u16>,
    ) -> Result<Self, DecodeError> {
        let expected_colors = match color_space {
            ConvertedColorSpace::Srgb => 3_usize,
            ConvertedColorSpace::Grayscale => 1_usize,
        };
        let expected_samples = checked_area(width, height)?;
        if color_samples.len() != expected_colors || alpha_samples.len() != expected_samples {
            return Err(invalid_color_input());
        }
        let mut channels = Vec::with_capacity(
            expected_colors
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?,
        );
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= color_samples.len(),
                decreases color_samples.len() - index,
            )
        )]
        while index < color_samples.len() {
            let samples = color_samples.get(index).ok_or_else(invalid_color_input)?;
            if samples.len() != expected_samples {
                return Err(invalid_color_input());
            }
            let channel_type = match (color_space, index) {
                (ConvertedColorSpace::Grayscale, 0) => ConvertedChannelType::Gray,
                (ConvertedColorSpace::Srgb, 0) => ConvertedChannelType::Red,
                (ConvertedColorSpace::Srgb, 1) => ConvertedChannelType::Green,
                (ConvertedColorSpace::Srgb, 2) => ConvertedChannelType::Blue,
                _ => return Err(invalid_color_input()),
            };
            channels.push(ConvertedChannel {
                channel_type,
                samples: samples.clone(),
            });
            index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        channels.push(ConvertedChannel {
            channel_type: ConvertedChannelType::Alpha {
                premultiplied: false,
                association: ChannelAssociation::WholeImage,
            },
            samples: alpha_samples,
        });
        Ok(Self {
            width,
            height,
            color_space,
            precision: 16,
            channels,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn color_space(&self) -> ConvertedColorSpace {
        self.color_space
    }

    pub const fn precision(&self) -> u8 {
        self.precision
    }

    pub fn channels(&self) -> &[ConvertedChannel] {
        &self.channels
    }

    pub fn channel(&self, index: u16) -> Result<&ConvertedChannel, DecodeError> {
        self.channels
            .get(usize::from(index))
            .ok_or_else(invalid_color_input)
    }
}

struct ChannelLayout {
    colors: Vec<usize>,
    alphas: Vec<(usize, bool, ChannelAssociation)>,
}

pub fn convert_jp2_color(
    image: &TypedImage,
    color: &ColorSpecification,
    limits: &ResourceLimits,
) -> Result<ConvertedImage, DecodeError> {
    if image.width() == 0 || image.height() == 0 {
        return Err(invalid_color_input());
    }
    let (color_count, output_space, sycc) = match color.method() {
        ColorMethod::Enumerated {
            color_space: ENUMERATED_SRGB,
        } => (3_usize, ConvertedColorSpace::Srgb, false),
        ColorMethod::Enumerated {
            color_space: ENUMERATED_GRAYSCALE,
        } => (1_usize, ConvertedColorSpace::Grayscale, false),
        ColorMethod::Enumerated {
            color_space: ENUMERATED_SYCC,
        } => (3_usize, ConvertedColorSpace::Srgb, true),
        ColorMethod::Enumerated { .. } => {
            return Err(unsupported(
                UnsupportedColorConversion::EnumeratedColorSpace,
            ));
        }
        ColorMethod::RestrictedIcc { .. } | ColorMethod::AnyIcc { .. } => {
            return Err(unsupported(UnsupportedColorConversion::IccProfile));
        }
    };
    let layout = channel_layout(image, color_count)?;
    validate_components(image, &layout)?;
    validate_output_size(image, color_count, layout.alphas.len(), limits)?;

    let pixel_count = checked_area(image.width(), image.height())?;
    let mut channels = Vec::with_capacity(
        color_count
            .checked_add(layout.alphas.len())
            .ok_or_else(arithmetic_overflow)?,
    );
    if sycc {
        let width = usize::try_from(image.width()).map_err(|_| arithmetic_overflow())?;
        if width == 0 {
            return Err(invalid_color_input());
        }
        let mut red = Vec::with_capacity(pixel_count);
        let mut green = Vec::with_capacity(pixel_count);
        let mut blue = Vec::with_capacity(pixel_count);
        let mut pixel = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant pixel <= pixel_count,
                decreases pixel_count - pixel,
            )
        )]
        while pixel < pixel_count {
            let (x, y) = pixel_coordinates(pixel, width)?;
            let luminance = normalized_at(
                image,
                *layout.colors.first().ok_or_else(invalid_color_input)?,
                x,
                y,
            )?;
            let blue_difference = normalized_difference_at(
                image,
                *layout.colors.get(1).ok_or_else(invalid_color_input)?,
                x,
                y,
            )?;
            let red_difference = normalized_difference_at(
                image,
                *layout.colors.get(2).ok_or_else(invalid_color_input)?,
                x,
                y,
            )?;
            let (r, g, b) = sycc_to_srgb(luminance, blue_difference, red_difference)?;
            red.push(r);
            green.push(g);
            blue.push(b);
            pixel = pixel.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        channels.push(ConvertedChannel {
            channel_type: ConvertedChannelType::Red,
            samples: red,
        });
        channels.push(ConvertedChannel {
            channel_type: ConvertedChannelType::Green,
            samples: green,
        });
        channels.push(ConvertedChannel {
            channel_type: ConvertedChannelType::Blue,
            samples: blue,
        });
    } else {
        let mut color_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant color_index <= color_count,
                decreases color_count - color_index,
            )
        )]
        while color_index < color_count {
            let component_index = *layout
                .colors
                .get(color_index)
                .ok_or_else(invalid_color_input)?;
            let component = image
                .components()
                .get(component_index)
                .ok_or_else(invalid_color_input)?;
            let samples = normalize_component(
                component,
                image.width(),
                image.height(),
                image.x_origin(),
                image.y_origin(),
            )?;
            let channel_type = match (color_count, color_index) {
                (1, 0) => ConvertedChannelType::Gray,
                (3, 0) => ConvertedChannelType::Red,
                (3, 1) => ConvertedChannelType::Green,
                (3, 2) => ConvertedChannelType::Blue,
                _ => return Err(invalid_color_input()),
            };
            channels.push(ConvertedChannel {
                channel_type,
                samples,
            });
            color_index = color_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
    }

    let mut alpha_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant alpha_index <= layout.alphas.len(),
            decreases layout.alphas.len() - alpha_index,
        )
    )]
    while alpha_index < layout.alphas.len() {
        let (component_index, premultiplied, association) = *layout
            .alphas
            .get(alpha_index)
            .ok_or_else(invalid_color_input)?;
        let component = image
            .components()
            .get(component_index)
            .ok_or_else(invalid_color_input)?;
        channels.push(ConvertedChannel {
            channel_type: ConvertedChannelType::Alpha {
                premultiplied,
                association,
            },
            samples: normalize_component(
                component,
                image.width(),
                image.height(),
                image.x_origin(),
                image.y_origin(),
            )?,
        });
        alpha_index = alpha_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }

    Ok(ConvertedImage {
        width: image.width(),
        height: image.height(),
        color_space: output_space,
        precision: 16,
        channels,
    })
}

fn channel_layout(image: &TypedImage, color_count: usize) -> Result<ChannelLayout, DecodeError> {
    let mut has_definitions = false;
    let component_count = image.components().len();
    let mut component_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component_index <= component_count,
            decreases component_count - component_index,
        )
    )]
    while component_index < component_count {
        let component = image
            .components()
            .get(component_index)
            .ok_or_else(invalid_color_input)?;
        if !component.channel_definitions().is_empty() {
            has_definitions = true;
        }
        component_index = component_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    if !has_definitions {
        if component_count != color_count {
            return Err(invalid_channel_layout());
        }
        let mut colors = Vec::with_capacity(color_count);
        component_index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_index <= color_count,
                decreases color_count - component_index,
            )
        )]
        while component_index < color_count {
            colors.push(component_index);
            component_index = component_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        return Ok(ChannelLayout {
            colors,
            alphas: Vec::new(),
        });
    }

    let mut colors = vec![usize::MAX; color_count];
    let mut alphas = Vec::new();
    component_index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component_index <= component_count,
            decreases component_count - component_index,
        )
    )]
    while component_index < component_count {
        let component = image
            .components()
            .get(component_index)
            .ok_or_else(invalid_color_input)?;
        let definitions = component.channel_definitions();
        let mut definition_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant definition_index <= definitions.len(),
                decreases definitions.len() - definition_index,
            )
        )]
        while definition_index < definitions.len() {
            let definition = *definitions
                .get(definition_index)
                .ok_or_else(invalid_color_input)?;
            match definition.channel_type() {
                ChannelType::Color => {
                    let association = match definition.association() {
                        ChannelAssociation::Color(value) => value,
                        _ => {
                            return Err(invalid_channel_layout());
                        }
                    };
                    if association == 0 || usize::from(association) > color_count {
                        return Err(invalid_channel_layout());
                    }
                    let slot = colors
                        .get_mut(usize::from(association - 1))
                        .ok_or_else(invalid_color_input)?;
                    if *slot != usize::MAX && *slot != component_index {
                        return Err(invalid_channel_layout());
                    }
                    *slot = component_index;
                }
                ChannelType::Opacity | ChannelType::PremultipliedOpacity => {
                    validate_alpha_association(definition.association(), color_count)?;
                    alphas.push((
                        component_index,
                        matches!(definition.channel_type(), ChannelType::PremultipliedOpacity),
                        definition.association(),
                    ));
                }
                ChannelType::Unspecified => {
                    // Unspecified channels are intentionally not part of colour conversion.
                }
            }
            definition_index = definition_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        component_index = component_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    component_index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component_index <= colors.len(),
            decreases colors.len() - component_index,
        )
    )]
    while component_index < colors.len() {
        if *colors
            .get(component_index)
            .ok_or_else(invalid_color_input)?
            == usize::MAX
        {
            return Err(invalid_channel_layout());
        }
        component_index = component_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    Ok(ChannelLayout { colors, alphas })
}

fn validate_alpha_association(
    association: ChannelAssociation,
    color_count: usize,
) -> Result<(), DecodeError> {
    match association {
        ChannelAssociation::Color(value) if value == 0 || usize::from(value) > color_count => {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidAlphaAssociation,
            ));
        }
        _ => {}
    }
    Ok(())
}

fn validate_components(image: &TypedImage, layout: &ChannelLayout) -> Result<(), DecodeError> {
    let mut color_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant color_index <= layout.colors.len(),
            decreases layout.colors.len() - color_index,
        )
    )]
    while color_index < layout.colors.len() {
        let component = image
            .components()
            .get(
                *layout
                    .colors
                    .get(color_index)
                    .ok_or_else(invalid_color_input)?,
            )
            .ok_or_else(invalid_color_input)?;
        validate_component(component, image)?;
        color_index = color_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let mut alpha_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant alpha_index <= layout.alphas.len(),
            decreases layout.alphas.len() - alpha_index,
        )
    )]
    while alpha_index < layout.alphas.len() {
        let component = image
            .components()
            .get(
                layout
                    .alphas
                    .get(alpha_index)
                    .ok_or_else(invalid_color_input)?
                    .0,
            )
            .ok_or_else(invalid_color_input)?;
        validate_component(component, image)?;
        alpha_index = alpha_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

fn validate_component(component: &TypedComponent, image: &TypedImage) -> Result<(), DecodeError> {
    let info = component.info();
    if info.precision == 0 || info.precision > 38 {
        return Err(invalid_color_input());
    }
    if info.signed != matches!(component.samples(), TypedSamples::Signed(_)) {
        return Err(invalid_color_input());
    }
    if info.x_subsampling == 0 || info.y_subsampling == 0 {
        return Err(invalid_color_input());
    }
    let image_x1 = image
        .x_origin()
        .checked_add(image.width())
        .ok_or_else(arithmetic_overflow)?;
    let image_y1 = image
        .y_origin()
        .checked_add(image.height())
        .ok_or_else(arithmetic_overflow)?;
    let expected_width =
        registered_ceil_div(image_x1, u32::from(info.x_subsampling), info.x_registration)?
            .checked_sub(registered_ceil_div(
                image.x_origin(),
                u32::from(info.x_subsampling),
                info.x_registration,
            )?)
            .ok_or_else(arithmetic_overflow)?;
    let expected_height =
        registered_ceil_div(image_y1, u32::from(info.y_subsampling), info.y_registration)?
            .checked_sub(registered_ceil_div(
                image.y_origin(),
                u32::from(info.y_subsampling),
                info.y_registration,
            )?)
            .ok_or_else(arithmetic_overflow)?;
    if component.width() != expected_width || component.height() != expected_height {
        return Err(invalid_color_input());
    }
    let expected_samples = checked_area(expected_width, expected_height)?;
    let actual_samples = match component.samples() {
        TypedSamples::Signed(samples) => samples.len(),
        TypedSamples::Unsigned(samples) => samples.len(),
    };
    if actual_samples != expected_samples {
        return Err(invalid_color_input());
    }
    Ok(())
}

fn validate_output_size(
    image: &TypedImage,
    color_count: usize,
    alpha_count: usize,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let channels = color_count
        .checked_add(alpha_count)
        .ok_or_else(arithmetic_overflow)?;
    let samples = checked_area(image.width(), image.height())?
        .checked_mul(channels)
        .ok_or_else(arithmetic_overflow)?;
    let bytes = (samples as u64)
        .checked_mul(2)
        .ok_or_else(arithmetic_overflow)?;
    if bytes > limits.maximum_decoded_bytes() {
        return Err(DecodeError::limit(
            Resource::DecodedBytes,
            limits.maximum_decoded_bytes(),
            bytes,
        ));
    }
    let work = u64::try_from(samples).map_err(|_| arithmetic_overflow())?;
    if work > limits.maximum_work_units() {
        return Err(DecodeError::limit(
            Resource::WorkUnits,
            limits.maximum_work_units(),
            work,
        ));
    }
    Ok(())
}

pub(crate) fn normalize_component(
    component: &TypedComponent,
    width: u32,
    height: u32,
    x_origin: u32,
    y_origin: u32,
) -> Result<Vec<u16>, DecodeError> {
    if width == 0 {
        return Err(invalid_color_input());
    }
    let row_width = usize::try_from(width).map_err(|_| arithmetic_overflow())?;
    if row_width == 0 {
        return Err(invalid_color_input());
    }
    let count = checked_area(width, height)?;
    let mut output = Vec::with_capacity(count);
    let mut pixel = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant pixel <= count,
            decreases count - pixel,
        )
    )]
    while pixel < count {
        let (x, y) = pixel_coordinates(pixel, row_width)?;
        output.push(normalized_component_at(
            component, x_origin, y_origin, x, y,
        )?);
        pixel = pixel.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(output)
}

fn pixel_coordinates(pixel: usize, width: usize) -> Result<(u32, u32), DecodeError> {
    if width == 0 {
        return Err(invalid_color_input());
    }
    let x = u32::try_from(pixel % width).map_err(|_| arithmetic_overflow())?;
    let y = u32::try_from(pixel / width).map_err(|_| arithmetic_overflow())?;
    Ok((x, y))
}

fn normalized_at(
    image: &TypedImage,
    component_index: usize,
    x: u32,
    y: u32,
) -> Result<u16, DecodeError> {
    let component = image
        .components()
        .get(component_index)
        .ok_or_else(invalid_color_input)?;
    normalized_component_at(component, image.x_origin(), image.y_origin(), x, y)
}

fn normalized_difference_at(
    image: &TypedImage,
    component_index: usize,
    x: u32,
    y: u32,
) -> Result<i32, DecodeError> {
    let component = image
        .components()
        .get(component_index)
        .ok_or_else(invalid_color_input)?;
    let info = component.info();
    if info.precision == 0 || info.precision > 38 {
        return Err(invalid_color_input());
    }
    let value = nominal_component_at(component, image.x_origin(), image.y_origin(), x, y)?;
    let maximum = (1_u64 << info.precision)
        .checked_sub(1)
        .ok_or_else(arithmetic_overflow)?;
    if maximum == 0 {
        return Err(invalid_color_input());
    }
    if value > maximum {
        return Err(invalid_color_input());
    }
    let midpoint = 1_u64 << (info.precision - 1);
    let difference = i128::from(value) - i128::from(midpoint);
    let scaled = difference
        .checked_mul(i128::from(OUTPUT_MAXIMUM))
        .ok_or_else(arithmetic_overflow)?;
    let rounded = if scaled < 0 {
        (scaled - i128::from(maximum / 2)) / i128::from(maximum)
    } else {
        (scaled + i128::from(maximum / 2)) / i128::from(maximum)
    };
    i32::try_from(rounded).map_err(|_| arithmetic_overflow())
}

fn normalized_component_at(
    component: &TypedComponent,
    image_x_origin: u32,
    image_y_origin: u32,
    x: u32,
    y: u32,
) -> Result<u16, DecodeError> {
    let info = component.info();
    let value = nominal_component_at(component, image_x_origin, image_y_origin, x, y)?;
    normalize_unsigned(value, info.precision)
}

fn nominal_component_at(
    component: &TypedComponent,
    image_x_origin: u32,
    image_y_origin: u32,
    x: u32,
    y: u32,
) -> Result<u64, DecodeError> {
    let info = component.info();
    if info.precision == 0
        || info.precision > 38
        || info.x_subsampling == 0
        || info.y_subsampling == 0
    {
        return Err(invalid_color_input());
    }
    if component.width() == 0 || component.height() == 0 {
        return Err(invalid_color_input());
    }
    let absolute_x = image_x_origin
        .checked_add(x)
        .ok_or_else(arithmetic_overflow)?;
    let absolute_y = image_y_origin
        .checked_add(y)
        .ok_or_else(arithmetic_overflow)?;
    let x_step = u32::from(info.x_subsampling);
    let y_step = u32::from(info.y_subsampling);
    let component_x0 = registered_ceil_div(image_x_origin, x_step, info.x_registration)?;
    let component_y0 = registered_ceil_div(image_y_origin, y_step, info.y_registration)?;
    let component_x1 = component_x0
        .checked_add(component.width())
        .ok_or_else(arithmetic_overflow)?;
    let component_y1 = component_y0
        .checked_add(component.height())
        .ok_or_else(arithmetic_overflow)?;
    let absolute_sample_x = registered_floor_div(absolute_x, x_step, info.x_registration)?
        .max(component_x0)
        .min(
            component_x1
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?,
        );
    let absolute_sample_y = registered_floor_div(absolute_y, y_step, info.y_registration)?
        .max(component_y0)
        .min(
            component_y1
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?,
        );
    let sample_x = absolute_sample_x
        .checked_sub(component_x0)
        .ok_or_else(arithmetic_overflow)?;
    let sample_y = absolute_sample_y
        .checked_sub(component_y0)
        .ok_or_else(arithmetic_overflow)?;
    let index = (sample_y as usize)
        .checked_mul(component.width() as usize)
        .and_then(|row| row.checked_add(sample_x as usize))
        .ok_or_else(arithmetic_overflow)?;
    match component.samples() {
        TypedSamples::Unsigned(samples) if !info.signed => {
            let value = *samples.get(index).ok_or_else(invalid_color_input)?;
            let maximum = (1_u64 << info.precision)
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?;
            if value > maximum {
                return Err(invalid_color_input());
            }
            Ok(value)
        }
        TypedSamples::Signed(samples) if info.signed => {
            let value = *samples.get(index).ok_or_else(invalid_color_input)?;
            let shift = info
                .precision
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?;
            let level = 1_i64 << u32::from(shift);
            let minimum = 0_i64.checked_sub(level).ok_or_else(arithmetic_overflow)?;
            let maximum = level.checked_sub(1).ok_or_else(arithmetic_overflow)?;
            if value < minimum || value > maximum {
                return Err(invalid_color_input());
            }
            if value > i64::MAX - level {
                return Err(arithmetic_overflow());
            }
            u64::try_from(crate::refinement::signed_to_nominal(value, level))
                .map_err(|_| arithmetic_overflow())
        }
        _ => Err(invalid_color_input()),
    }
}

fn normalize_unsigned(value: u64, precision: u8) -> Result<u16, DecodeError> {
    if precision == 0 || precision > 38 {
        return Err(invalid_color_input());
    }
    let maximum = (1_u64 << precision)
        .checked_sub(1)
        .ok_or_else(arithmetic_overflow)?;
    if maximum == 0 {
        return Err(invalid_color_input());
    }
    if value > maximum {
        return Err(invalid_color_input());
    }
    let numerator = value
        .checked_mul(OUTPUT_MAXIMUM)
        .and_then(|scaled| scaled.checked_add(maximum / 2))
        .ok_or_else(arithmetic_overflow)?;
    u16::try_from(numerator / maximum).map_err(|_| arithmetic_overflow())
}

fn sycc_to_srgb(y: u16, cb: i32, cr: i32) -> Result<(u16, u16, u16), DecodeError> {
    let y = i128::from(y);
    let cb = i128::from(cb);
    let cr = i128::from(cr);
    let red_delta = rounded_fixed(
        91_881_i128
            .checked_mul(cr)
            .ok_or_else(arithmetic_overflow)?,
    )?;
    let green_blue_delta = rounded_fixed(
        22_554_i128
            .checked_mul(cb)
            .ok_or_else(arithmetic_overflow)?,
    )?;
    let green_red_delta = rounded_fixed(
        46_802_i128
            .checked_mul(cr)
            .ok_or_else(arithmetic_overflow)?,
    )?;
    let blue_delta = rounded_fixed(
        116_130_i128
            .checked_mul(cb)
            .ok_or_else(arithmetic_overflow)?,
    )?;
    let red = y.checked_add(red_delta).ok_or_else(arithmetic_overflow)?;
    let green = y
        .checked_sub(green_blue_delta)
        .and_then(|value| value.checked_sub(green_red_delta))
        .ok_or_else(arithmetic_overflow)?;
    let blue = y.checked_add(blue_delta).ok_or_else(arithmetic_overflow)?;
    Ok((clamp_u16(red), clamp_u16(green), clamp_u16(blue)))
}

fn rounded_fixed(value: i128) -> Result<i128, DecodeError> {
    if value < 0 {
        Ok(value
            .checked_sub(FIXED_ONE / 2)
            .ok_or_else(arithmetic_overflow)?
            / FIXED_ONE)
    } else {
        Ok(value
            .checked_add(FIXED_ONE / 2)
            .ok_or_else(arithmetic_overflow)?
            / FIXED_ONE)
    }
}

fn clamp_u16(value: i128) -> u16 {
    if value <= 0 {
        0
    } else if value >= i128::from(u16::MAX) {
        u16::MAX
    } else {
        value as u16
    }
}

fn registered_ceil_div(value: u32, divisor: u32, registration: u16) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(invalid_color_input());
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

fn registered_floor_div(value: u32, divisor: u32, registration: u16) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(invalid_color_input());
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
    u32::try_from(adjusted / denominator).map_err(|_| arithmetic_overflow())
}

fn checked_area(width: u32, height: u32) -> Result<usize, DecodeError> {
    let area = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(arithmetic_overflow)?;
    usize::try_from(area).map_err(|_| arithmetic_overflow())
}

const fn unsupported(reason: UnsupportedColorConversion) -> DecodeError {
    DecodeError::UnsupportedColorConversion { reason }
}

const fn invalid_channel_layout() -> DecodeError {
    DecodeError::malformed(0, MalformedKind::InvalidChannelLayout)
}

fn invalid_color_input() -> DecodeError {
    DecodeError::malformed(0, MalformedKind::ContainerCodestreamMismatch)
}

fn arithmetic_overflow() -> DecodeError {
    DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        codestream::ComponentInfo,
        image::{ChannelDefinition, ChannelType, TypedComponent, TypedImage, TypedSamples},
    };

    fn component(
        samples: &[u64],
        precision: u8,
        signed: bool,
        subsampling: (u8, u8),
        definition: Option<ChannelDefinition>,
    ) -> TypedComponent {
        let mut component = TypedComponent::from_parts(
            u32::try_from(samples.len()).unwrap(),
            1,
            ComponentInfo {
                precision,
                signed,
                x_subsampling: subsampling.0,
                y_subsampling: subsampling.1,
                x_registration: 0,
                y_registration: 0,
            },
            if signed {
                TypedSamples::Signed(samples.iter().map(|value| *value as i64).collect())
            } else {
                TypedSamples::Unsigned(samples.to_vec())
            },
        );
        if let Some(definition) = definition {
            component.add_channel_definition(definition);
        }
        component
    }

    fn color(association: u16) -> ChannelDefinition {
        ChannelDefinition::new(ChannelType::Color, ChannelAssociation::Color(association))
    }

    fn image(width: u32, components: Vec<TypedComponent>) -> TypedImage {
        TypedImage::from_components(width, 1, components, &ResourceLimits::default()).unwrap()
    }

    fn signed_component(
        samples: &[i64],
        precision: u8,
        subsampling: (u8, u8),
        definition: Option<ChannelDefinition>,
    ) -> TypedComponent {
        let mut component = TypedComponent::from_parts(
            u32::try_from(samples.len()).unwrap(),
            1,
            ComponentInfo {
                precision,
                signed: true,
                x_subsampling: subsampling.0,
                y_subsampling: subsampling.1,
                x_registration: 0,
                y_registration: 0,
            },
            TypedSamples::Signed(samples.to_vec()),
        );
        if let Some(definition) = definition {
            component.add_channel_definition(definition);
        }
        component
    }

    #[test]
    fn registration_offset_changes_component_sample_location() {
        let registered = TypedComponent::from_parts(
            2,
            1,
            ComponentInfo {
                precision: 8,
                signed: false,
                x_subsampling: 2,
                y_subsampling: 1,
                x_registration: 32_768,
                y_registration: 0,
            },
            TypedSamples::Unsigned(vec![10, 20]),
        );
        assert_eq!(nominal_component_at(&registered, 0, 0, 2, 0).unwrap(), 10);
        assert_eq!(nominal_component_at(&registered, 0, 0, 3, 0).unwrap(), 20);
    }

    #[test]
    fn srgb_reorders_associated_channels_and_preserves_alpha() {
        let alpha = component(
            &[0, 255],
            8,
            false,
            (1, 1),
            Some(ChannelDefinition::new(
                ChannelType::PremultipliedOpacity,
                ChannelAssociation::WholeImage,
            )),
        );
        let blue = component(&[255, 0], 8, false, (1, 1), Some(color(3)));
        let red = component(&[0, 255], 8, false, (1, 1), Some(color(1)));
        let green = component(&[128, 64], 8, false, (1, 1), Some(color(2)));
        let converted = convert_jp2_color(
            &image(2, vec![alpha, blue, red, green]),
            &ColorSpecification::enumerated(ENUMERATED_SRGB),
            &ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(converted.color_space(), ConvertedColorSpace::Srgb);
        assert_eq!(converted.precision(), 16);
        assert_eq!(converted.channel(0).unwrap().samples(), &[0, 65_535]);
        assert_eq!(converted.channel(1).unwrap().samples(), &[32_896, 16_448]);
        assert_eq!(converted.channel(2).unwrap().samples(), &[65_535, 0]);
        assert_eq!(converted.channel(3).unwrap().samples(), &[0, 65_535]);
        assert_eq!(
            converted.channel(3).unwrap().channel_type(),
            ConvertedChannelType::Alpha {
                premultiplied: true,
                association: ChannelAssociation::WholeImage,
            }
        );
    }

    #[test]
    fn grayscale_scales_full_declared_range() {
        let converted = convert_jp2_color(
            &image(2, vec![component(&[0, 1], 1, false, (1, 1), None)]),
            &ColorSpecification::enumerated(ENUMERATED_GRAYSCALE),
            &ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(converted.color_space(), ConvertedColorSpace::Grayscale);
        assert_eq!(converted.channel(0).unwrap().samples(), &[0, 65_535]);
    }

    #[test]
    fn sycc_neutral_chroma_is_deterministic_srgb() {
        let converted = convert_jp2_color(
            &image(
                3,
                vec![
                    component(&[0, 128, 255], 8, false, (1, 1), None),
                    component(&[128, 128, 128], 8, false, (1, 1), None),
                    component(&[128, 128, 128], 8, false, (1, 1), None),
                ],
            ),
            &ColorSpecification::enumerated(ENUMERATED_SYCC),
            &ResourceLimits::default(),
        )
        .unwrap();
        for channel in converted.channels() {
            assert_eq!(channel.samples(), &[0, 32_896, 65_535]);
        }
    }

    #[test]
    fn signed_subsampled_sycc_and_alpha_resample_to_image_grid() {
        let alpha = signed_component(
            &[-128, 127],
            8,
            (2, 1),
            Some(ChannelDefinition::new(
                ChannelType::Opacity,
                ChannelAssociation::WholeImage,
            )),
        );
        let converted = convert_jp2_color(
            &image(
                4,
                vec![
                    signed_component(&[-128, -43, 42, 127], 8, (1, 1), Some(color(1))),
                    signed_component(&[-128, 127], 8, (2, 1), Some(color(2))),
                    signed_component(&[0, 0], 8, (2, 1), Some(color(3))),
                    alpha,
                ],
            ),
            &ColorSpecification::enumerated(ENUMERATED_SYCC),
            &ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(converted.channels().len(), 4);
        assert_eq!(converted.channel(0).unwrap().samples().len(), 4);
        assert_eq!(converted.channel(1).unwrap().samples().len(), 4);
        assert_eq!(converted.channel(2).unwrap().samples().len(), 4);
        assert_eq!(
            converted.channel(3).unwrap().samples(),
            &[0, 0, 65_535, 65_535]
        );
    }

    #[test]
    fn subsampling_respects_nonzero_image_origin() {
        let image = TypedImage::from_components_with_origin(
            4,
            1,
            1,
            0,
            vec![component(&[0, 255], 8, false, (2, 1), None)],
            &ResourceLimits::default(),
        )
        .unwrap();
        let converted = convert_jp2_color(
            &image,
            &ColorSpecification::enumerated(ENUMERATED_GRAYSCALE),
            &ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(converted.channel(0).unwrap().samples(), &[0, 0, 0, 65_535]);
    }

    #[test]
    fn conversion_validates_alpha_association_samples_and_output_bound() {
        let invalid_alpha = component(
            &[255],
            8,
            false,
            (1, 1),
            Some(ChannelDefinition::new(
                ChannelType::Opacity,
                ChannelAssociation::Color(2),
            )),
        );
        let gray = component(&[0], 8, false, (1, 1), Some(color(1)));
        assert!(matches!(
            convert_jp2_color(
                &image(1, vec![gray, invalid_alpha]),
                &ColorSpecification::enumerated(ENUMERATED_GRAYSCALE),
                &ResourceLimits::default()
            ),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::InvalidAlphaAssociation,
                ..
            })
        ));

        let out_of_range = image(1, vec![component(&[256], 8, false, (1, 1), None)]);
        assert!(matches!(
            convert_jp2_color(
                &out_of_range,
                &ColorSpecification::enumerated(ENUMERATED_GRAYSCALE),
                &ResourceLimits::default()
            ),
            Err(DecodeError::MalformedInput {
                kind: MalformedKind::ContainerCodestreamMismatch,
                ..
            })
        ));

        let limit = ResourceLimits::default()
            .with_maximum_decoded_bytes(1)
            .unwrap();
        assert!(matches!(
            convert_jp2_color(
                &image(1, vec![component(&[0], 8, false, (1, 1), None)]),
                &ColorSpecification::enumerated(ENUMERATED_GRAYSCALE),
                &limit
            ),
            Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: 1,
                actual: 2
            })
        ));
    }

    #[test]
    fn unknown_enumerated_space_is_typed_unsupported() {
        assert!(matches!(
            convert_jp2_color(
                &image(1, vec![component(&[0], 8, false, (1, 1), None)]),
                &ColorSpecification::enumerated(4_294_967_295),
                &ResourceLimits::default()
            ),
            Err(DecodeError::UnsupportedColorConversion {
                reason: UnsupportedColorConversion::EnumeratedColorSpace
            })
        ));
    }
}
