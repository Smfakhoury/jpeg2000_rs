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

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn converted_channel_view(channel: &ConvertedChannel) -> (ConvertedChannelType, Seq<u16>) {
        (channel.channel_type, channel.samples@)
    }

    pub closed spec fn converted_image_view(image: &ConvertedImage)
        -> (u32, u32, ConvertedColorSpace, u8, Seq<ConvertedChannel>)
    {
        (image.width, image.height, image.color_space, image.precision, image.channels@)
    }

    pub open spec fn converted_channel_views(channels: Seq<ConvertedChannel>)
        -> Seq<(ConvertedChannelType, Seq<u16>)>
    {
        Seq::new(channels.len(), |i: int| converted_channel_view(&channels[i]))
    }

    pub open spec fn converted_image_result(result: Result<ConvertedImage, DecodeError>)
        -> crate::specs::Part2ConvertedImageResult
    {
        match result {
            Err(error) => Err(error),
            Ok(image) => {
                let view = converted_image_view(&image);
                Ok((view.0, view.1, view.2, view.3, converted_channel_views(view.4)))
            },
        }
    }

    #[verifier::inline]
    pub(crate) open spec fn normalized_plane_result(result: Result<Vec<u16>, DecodeError>)
        -> Result<Seq<u16>, DecodeError>
    {
        match result {
            Ok(samples) => Ok(samples@),
            Err(error) => Err(error),
        }
    }
}

impl ConvertedChannel {
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(kind => ensures kind == converted_channel_view(self).0,)
    )]
    pub const fn channel_type(&self) -> ConvertedChannelType {
        self.channel_type
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(samples => ensures samples@ == converted_channel_view(self).1,)
    )]
    pub fn samples(&self) -> &[u16] {
        &self.samples
    }
}

impl ConvertedImage {
    pub(crate) fn crop(self, x: u32, y: u32, width: u32, height: u32) -> Result<Self, DecodeError> {
        if width == 0 || height == 0 {
            return Err(invalid_color_input());
        }
        let x1 = x.checked_add(width).ok_or_else(arithmetic_overflow)?;
        let y1 = y.checked_add(height).ok_or_else(arithmetic_overflow)?;
        if x1 > self.width || y1 > self.height {
            return Err(invalid_color_input());
        }
        if x == 0 && y == 0 && width == self.width && height == self.height {
            return Ok(self);
        }
        let count = checked_area(width, height)?;
        let mut channels = Vec::with_capacity(self.channels.len());
        let mut channel_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases self.channels.len() - channel_index,)
        )]
        while channel_index < self.channels.len() {
            let channel = self
                .channels
                .get(channel_index)
                .ok_or_else(invalid_color_input)?;
            let mut samples = Vec::with_capacity(count);
            let mut row = y;
            #[cfg_attr(verus_keep_ghost, verus_spec(decreases y1 - row,))]
            while row < y1 {
                let mut column = x;
                #[cfg_attr(verus_keep_ghost, verus_spec(decreases x1 - column,))]
                while column < x1 {
                    let index = (row as usize)
                        .checked_mul(self.width as usize)
                        .and_then(|value| value.checked_add(column as usize))
                        .ok_or_else(arithmetic_overflow)?;
                    samples.push(*channel.samples.get(index).ok_or_else(invalid_color_input)?);
                    column = column.checked_add(1).ok_or_else(arithmetic_overflow)?;
                }
                row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            channels.push(ConvertedChannel {
                channel_type: channel.channel_type,
                samples,
            });
            channel_index = channel_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        Ok(Self {
            width,
            height,
            color_space: self.color_space,
            precision: self.precision,
            channels,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_test_channels(
        width: u32,
        height: u32,
        color_space: ConvertedColorSpace,
        channels: Vec<(ConvertedChannelType, Vec<u16>)>,
    ) -> Self {
        Self {
            width,
            height,
            color_space,
            precision: 16,
            channels: channels
                .into_iter()
                .map(|(channel_type, samples)| ConvertedChannel {
                    channel_type,
                    samples,
                })
                .collect(),
        }
    }
}

#[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures converted_image_result(result) == crate::specs::part2_image_from_planes(
                width, height, color_space, crate::specs::part2_composition_planes(color_samples@), alpha_samples@, false,
            ),
        )
    )]
pub(crate) fn from_composited(
    width: u32,
    height: u32,
    color_space: ConvertedColorSpace,
    color_samples: Vec<Vec<u16>>,
    alpha_samples: Vec<u16>,
) -> Result<ConvertedImage, DecodeError> {
    from_layer(
        width,
        height,
        color_space,
        color_samples,
        alpha_samples,
        false,
    )
}

#[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures converted_image_result(result) == crate::specs::part2_image_from_planes(
                width, height, color_space, crate::specs::part2_composition_planes(color_samples@), alpha_samples@, premultiplied,
            ),
        )
    )]
pub(crate) fn from_layer(
    width: u32,
    height: u32,
    color_space: ConvertedColorSpace,
    color_samples: Vec<Vec<u16>>,
    alpha_samples: Vec<u16>,
    premultiplied: bool,
) -> Result<ConvertedImage, DecodeError> {
    let expected_colors = match color_space {
        ConvertedColorSpace::Srgb => 3_usize,
        ConvertedColorSpace::Grayscale => 1_usize,
    };
    let expected_samples = checked_area(width, height)?;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost input = crate::specs::part2_composition_planes(color_samples@);
        let ghost input_alpha = alpha_samples@;
    }
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
                    color_samples.len() == expected_colors,
                    expected_colors as nat == crate::specs::part2_layer_color_count(color_space),
                    crate::specs::part2_layer_area(width, height) == Ok(expected_samples),
                    alpha_samples.len() == expected_samples,
                    channels.len() == index,
                    input == crate::specs::part2_composition_planes(color_samples@),
                    input_alpha == alpha_samples@,
                    forall|i: int| 0 <= i < index ==> #[trigger] input[i].len() == expected_samples,
                    converted_channel_views(channels@) == Seq::new(index as nat, |i: int|
                        (crate::specs::part2_output_channel(color_space, i), input[i])),
                decreases color_samples.len() - index,
            )
        )]
    while index < color_samples.len() {
        let samples = color_samples.get(index).ok_or_else(invalid_color_input)?;
        if samples.len() != expected_samples {
            #[cfg(verus_keep_ghost)]
            proof! { assert(input[index as int].len() != expected_samples); }
            return Err(invalid_color_input());
        }
        let channel_type = match (color_space, index) {
            (ConvertedColorSpace::Grayscale, 0) => ConvertedChannelType::Gray,
            (ConvertedColorSpace::Srgb, 0) => ConvertedChannelType::Red,
            (ConvertedColorSpace::Srgb, 1) => ConvertedChannelType::Green,
            (ConvertedColorSpace::Srgb, 2) => ConvertedChannelType::Blue,
            _ => return Err(invalid_color_input()),
        };
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost previous = channels@; }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(channel_type == crate::specs::part2_output_channel(color_space, index as int));
            assert(samples@ == input[index as int]);
        }
        channels.push(ConvertedChannel {
            channel_type,
            samples: copy_channel_samples(samples),
        });
        #[cfg(verus_keep_ghost)]
        proof! {
            assert forall|i: int| 0 <= i < channels.len() implies
                #[trigger] converted_channel_view(&channels@[i])
                    == (crate::specs::part2_output_channel(color_space, i), input[i]) by {
                if i < index {
                    assert(channels@[i] == previous[i]);
                    assert(converted_channel_views(previous)[i]
                        == (crate::specs::part2_output_channel(color_space, i), input[i]));
                } else {
                    assert(converted_channel_view(&channels@[i]).0 == channel_type);
                    assert(converted_channel_view(&channels@[i]).1 == input[i]);
                }
            }
            assert(converted_channel_views(channels@) =~= Seq::new((index + 1) as nat, |i: int|
                (crate::specs::part2_output_channel(color_space, i), input[i])));
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost previous = channels@; }
    channels.push(ConvertedChannel {
        channel_type: ConvertedChannelType::Alpha {
            premultiplied,
            association: ChannelAssociation::WholeImage,
        },
        samples: alpha_samples,
    });
    #[cfg(verus_keep_ghost)]
    proof! {
        assert forall|i: int| 0 <= i < previous.len() implies
            #[trigger] converted_channel_view(&channels@[i]) == converted_channel_view(&previous[i]) by {}
        assert(converted_channel_views(channels@) =~=
            Seq::new(color_samples.len() as nat, |i: int|
                (crate::specs::part2_output_channel(color_space, i), input[i]))
            .push((ConvertedChannelType::Alpha { premultiplied, association: ChannelAssociation::WholeImage }, input_alpha)));
    }
    Ok(ConvertedImage {
        width,
        height,
        color_space,
        precision: 16,
        channels,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(output => ensures output@ == input@,))]
fn copy_channel_samples(input: &[u16]) -> Vec<u16> {
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0_usize;
    #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= input.len(), output@ == input@.take(index as int),
                decreases input.len() - index,
            )
        )]
    while index < input.len() {
        output.push(input[index]);
        index += 1;
    }
    output
}

impl ConvertedImage {
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(width => ensures width == converted_image_view(self).0,)
    )]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(height => ensures height == converted_image_view(self).1,)
    )]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(space => ensures space == converted_image_view(self).2,)
    )]
    pub const fn color_space(&self) -> ConvertedColorSpace {
        self.color_space
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(precision => ensures precision == converted_image_view(self).3,)
    )]
    pub const fn precision(&self) -> u8 {
        self.precision
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(channels => ensures channels@ == converted_image_view(self).4,)
    )]
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

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures normalized_plane_result(result) == crate::specs::color_normalize_component(
            crate::image::typed_component_view(component), width, height, x_origin, y_origin,
        ),
    )
)]
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
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost input = crate::image::typed_component_view(component);
    }
    let mut output = Vec::with_capacity(count);
    let mut pixel = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_color_plane_empty(input, width, x_origin, y_origin);
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant pixel <= count,
                input == crate::image::typed_component_view(component),
                width > 0,
                width as nat <= usize::MAX as nat,
                row_width as nat == width as nat,
                crate::specs::part2_layer_area(width, height) == Ok(count),
                crate::specs::color_normalize_component_prefix(input, width, x_origin, y_origin, pixel as nat)
                    == Ok(output@),
            decreases count - pixel,
        )
    )]
    while pixel < count {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_color_plane_step(
                input, width, x_origin, y_origin, pixel as nat,
            );
        }
        let (x, y) = match pixel_coordinates(pixel, row_width) {
            Ok(coordinates) => coordinates,
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_color_plane_error(
                        input, width, x_origin, y_origin, pixel as nat + 1, count as nat, error,
                    );
                }
                return Err(error);
            }
        };
        let value = match normalized_component_at(component, x_origin, y_origin, x, y) {
            Ok(value) => value,
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_color_plane_error(
                        input, width, x_origin, y_origin, pixel as nat + 1, count as nat, error,
                    );
                }
                return Err(error);
            }
        };
        output.push(value);
        pixel = pixel.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(output)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result => ensures result == crate::specs::color_pixel_coordinates(pixel, width),)
)]
fn pixel_coordinates(pixel: usize, width: usize) -> Result<(u32, u32), DecodeError> {
    if width == 0 {
        return Err(invalid_color_input());
    }
    let x = match u32::try_from(pixel % width) {
        Ok(value) => value,
        Err(_) => return Err(arithmetic_overflow()),
    };
    let y = match u32::try_from(pixel / width) {
        Ok(value) => value,
        Err(_) => return Err(arithmetic_overflow()),
    };
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

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::color_component_normalized(
            crate::image::typed_component_view(component), image_x_origin, image_y_origin, x, y,
        ),
    )
)]
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

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::color_component_nominal(
            crate::image::typed_component_view(component), image_x_origin, image_y_origin, x, y,
        ),
    )
)]
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
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::spec_properties::color_precision_binary_bounds(info.precision);
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
    let row = (sample_y as usize)
        .checked_mul(component.width() as usize)
        .ok_or_else(arithmetic_overflow)?;
    let index = row
        .checked_add(sample_x as usize)
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

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result => ensures result == crate::specs::color_normalize_unsigned(value, precision),)
)]
fn normalize_unsigned(value: u64, precision: u8) -> Result<u16, DecodeError> {
    if precision == 0 || precision > 38 {
        return Err(invalid_color_input());
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::spec_properties::color_precision_binary_bounds(precision);
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
    let scaled = value
        .checked_mul(OUTPUT_MAXIMUM)
        .ok_or_else(arithmetic_overflow)?;
    let numerator = scaled
        .checked_add(maximum / 2)
        .ok_or_else(arithmetic_overflow)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(scaled as nat <= 65535 * (maximum as nat));
        crate::proofs::spec_properties::source_over_rounding_preserves_sample_bound(scaled, maximum);
        assert(numerator / maximum <= 65535);
    }
    match u16::try_from(numerator / maximum) {
        Ok(value) => Ok(value),
        Err(_) => Err(arithmetic_overflow()),
    }
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

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::color_registered_coordinate(value, divisor, registration, true),
    )
)]
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
    let rounded = crate::refinement::registered_extent(adjusted, denominator);
    match u32::try_from(rounded) {
        Ok(value) => Ok(value),
        Err(_) => Err(arithmetic_overflow()),
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::color_registered_coordinate(value, divisor, registration, false),
    )
)]
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
    match u32::try_from(adjusted / denominator) {
        Ok(value) => Ok(value),
        Err(_) => Err(arithmetic_overflow()),
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(area => ensures area == crate::specs::part2_layer_area(width, height),)
)]
fn checked_area(width: u32, height: u32) -> Result<usize, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part2_layer_area);
        assert((width as int) * (height as int) <= 18446744065119617025) by(nonlinear_arith);
    }
    let area = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(arithmetic_overflow)?;
    match usize::try_from(area) {
        Ok(value) => Ok(value),
        Err(_) => Err(arithmetic_overflow()),
    }
}

const fn unsupported(reason: UnsupportedColorConversion) -> DecodeError {
    DecodeError::UnsupportedColorConversion { reason }
}

const fn invalid_channel_layout() -> DecodeError {
    DecodeError::malformed(0, MalformedKind::InvalidChannelLayout)
}

#[cfg_attr(verus_keep_ghost, verus_spec(error => ensures error == crate::specs::part2_color_input_error(),))]
fn invalid_color_input() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ContainerCodestreamMismatch,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(error => ensures error == crate::specs::part2_composition_error(true),))]
fn arithmetic_overflow() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        codestream::ComponentInfo,
        image::{ChannelDefinition, ChannelType, TypedComponent, TypedImage, TypedSamples},
    };

    fn sampled_plane_wide_reference(
        nominal: &[u64],
        component_size: (u32, u32),
        info: ComponentInfo,
        output_size: (u32, u32),
        origin: (u32, u32),
    ) -> Result<Vec<u16>, DecodeError> {
        if output_size.0 == 0 {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::ContainerCodestreamMismatch,
            });
        }
        let coordinate = |value: u32, sampling: u8, registration: u16, ceil: bool| {
            let numerator = (u128::from(value) * 65_536).saturating_sub(u128::from(registration));
            let denominator = u128::from(sampling) * 65_536;
            if ceil {
                numerator.div_ceil(denominator)
            } else {
                numerator / denominator
            }
        };
        let x0 = coordinate(origin.0, info.x_subsampling, info.x_registration, true);
        let y0 = coordinate(origin.1, info.y_subsampling, info.y_registration, true);
        let maximum = (1_u128 << info.precision) - 1;
        let mut result = Vec::new();
        for y in 0..output_size.1 {
            for x in 0..output_size.0 {
                let sx = coordinate(origin.0 + x, info.x_subsampling, info.x_registration, false)
                    .clamp(x0, x0 + u128::from(component_size.0) - 1);
                let sy = coordinate(origin.1 + y, info.y_subsampling, info.y_registration, false)
                    .clamp(y0, y0 + u128::from(component_size.1) - 1);
                let index =
                    usize::try_from((sy - y0) * u128::from(component_size.0) + sx - x0).unwrap();
                let value = u128::from(nominal[index]);
                result.push(u16::try_from((value * 65_535 + maximum / 2) / maximum).unwrap());
            }
        }
        Ok(result)
    }

    #[test]
    fn complete_normalized_component_planes_match_wide_registered_reference() {
        let mut cases = 0;
        for precision in [1, 2, 7, 8, 12, 16, 24, 31, 38] {
            let maximum = (1_u64 << precision) - 1;
            for signed in [false, true] {
                for sampling in [(1, 1), (2, 1), (1, 2), (2, 3)] {
                    for origin in [(0, 0), (1, 3), (3, 5), (7, 2)] {
                        for registration in
                            [(0, 0), (1, 65_535), (32_768, 12_345), (65_535, 32_768)]
                        {
                            for size in [(1_u32, 1_u32), (3, 2)] {
                                let nominal = (0..size.0 * size.1)
                                    .map(|i| match i {
                                        0 => 0,
                                        1 => maximum,
                                        2 => maximum / 2,
                                        _ => (u64::from(i) * 17 + 1) % (maximum + 1),
                                    })
                                    .collect::<Vec<_>>();
                                let info = ComponentInfo {
                                    precision,
                                    signed,
                                    x_subsampling: sampling.0,
                                    y_subsampling: sampling.1,
                                    x_registration: registration.0,
                                    y_registration: registration.1,
                                };
                                let samples = if signed {
                                    let level = 1_i64 << (precision - 1);
                                    TypedSamples::Signed(
                                        nominal
                                            .iter()
                                            .map(|&value| i64::try_from(value).unwrap() - level)
                                            .collect(),
                                    )
                                } else {
                                    TypedSamples::Unsigned(nominal.clone())
                                };
                                let input =
                                    TypedComponent::from_parts(size.0, size.1, info, samples);
                                for output in [(1, 1), (5, 4), (0, 3), (4, 0)] {
                                    let expected = sampled_plane_wide_reference(
                                        &nominal, size, info, output, origin,
                                    );
                                    assert_eq!(
                                        normalize_component(
                                            &input, output.0, output.1, origin.0, origin.1
                                        ),
                                        expected,
                                        "precision={precision} signed={signed} sampling={sampling:?} origin={origin:?} registration={registration:?} size={size:?} output={output:?}",
                                    );
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 9_216);
    }

    #[test]
    fn registered_coordinates_match_wide_full_domain_boundaries() {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ContainerCodestreamMismatch,
        };
        for value in [0, 1, 2, u32::MAX / 2, u32::MAX] {
            for divisor in [0, 1, 2, 255, 65_536, u32::MAX] {
                for registration in [0, 1, 32_768, u16::MAX] {
                    let reference = |ceil: bool| {
                        if divisor == 0 {
                            return Err(invalid);
                        }
                        let numerator =
                            (u128::from(value) * 65_536).saturating_sub(u128::from(registration));
                        let denominator = u128::from(divisor) * 65_536;
                        let quotient = if ceil {
                            numerator.div_ceil(denominator)
                        } else {
                            numerator / denominator
                        };
                        Ok(u32::try_from(quotient).unwrap())
                    };
                    assert_eq!(
                        registered_ceil_div(value, divisor, registration),
                        reference(true)
                    );
                    assert_eq!(
                        registered_floor_div(value, divisor, registration),
                        reference(false)
                    );
                }
            }
        }
    }

    #[test]
    fn normalized_component_rejects_every_invalid_stage_input_in_order() {
        let invalid = Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ContainerCodestreamMismatch,
        });
        let arithmetic = Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        });
        let info = ComponentInfo {
            precision: 8,
            signed: false,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        for bad_info in [
            ComponentInfo {
                precision: 0,
                ..info
            },
            ComponentInfo {
                precision: 39,
                ..info
            },
            ComponentInfo {
                precision: u8::MAX,
                ..info
            },
            ComponentInfo {
                x_subsampling: 0,
                ..info
            },
            ComponentInfo {
                y_subsampling: 0,
                ..info
            },
        ] {
            let input = TypedComponent::from_parts(1, 1, bad_info, TypedSamples::Unsigned(vec![0]));
            assert_eq!(
                normalize_component(&input, 1, 1, u32::MAX, u32::MAX),
                invalid
            );
        }
        for size in [(0, 1), (1, 0)] {
            let input =
                TypedComponent::from_parts(size.0, size.1, info, TypedSamples::Unsigned(vec![]));
            assert_eq!(normalize_component(&input, 1, 1, 0, 0), invalid);
        }
        let wrong_tag = TypedComponent::from_parts(1, 1, info, TypedSamples::Signed(vec![0]));
        assert_eq!(normalize_component(&wrong_tag, 1, 1, 0, 0), invalid);
        let wrong_signed = TypedComponent::from_parts(
            1,
            1,
            ComponentInfo {
                signed: true,
                ..info
            },
            TypedSamples::Unsigned(vec![0]),
        );
        assert_eq!(normalize_component(&wrong_signed, 1, 1, 0, 0), invalid);
        for samples in [
            TypedSamples::Unsigned(vec![256]),
            TypedSamples::Unsigned(vec![]),
        ] {
            let input = TypedComponent::from_parts(1, 1, info, samples);
            assert_eq!(normalize_component(&input, 1, 1, 0, 0), invalid);
        }
        for value in [-129, 128] {
            let input = TypedComponent::from_parts(
                1,
                1,
                ComponentInfo {
                    signed: true,
                    ..info
                },
                TypedSamples::Signed(vec![value]),
            );
            assert_eq!(normalize_component(&input, 1, 1, 0, 0), invalid);
        }
        let short = TypedComponent::from_parts(3, 2, info, TypedSamples::Unsigned(vec![0; 5]));
        assert_eq!(normalize_component(&short, 3, 2, 0, 0), invalid);
        let extra = TypedComponent::from_parts(3, 2, info, TypedSamples::Unsigned(vec![0; 7]));
        assert_eq!(normalize_component(&extra, 3, 2, 0, 0), Ok(vec![0; 6]));
        let outside = TypedComponent::from_parts(1, 1, info, TypedSamples::Unsigned(vec![0]));
        assert_eq!(normalize_component(&outside, 1, 1, u32::MAX, 0), arithmetic);
        let late_origin_overflow = TypedComponent::from_parts(
            1,
            1,
            ComponentInfo {
                x_subsampling: 255,
                ..info
            },
            TypedSamples::Unsigned(vec![0]),
        );
        assert_eq!(
            normalize_component(&late_origin_overflow, 2, 1, u32::MAX, 0),
            arithmetic
        );
        let empty = TypedComponent::from_parts(
            0,
            0,
            ComponentInfo {
                precision: 0,
                x_subsampling: 0,
                ..info
            },
            TypedSamples::Unsigned(vec![]),
        );
        assert_eq!(normalize_component(&empty, 0, 0, 0, 0), invalid);
        assert_eq!(
            normalize_component(&empty, 1, 0, u32::MAX, u32::MAX),
            Ok(vec![])
        );
    }

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
    fn normalized_plane_constructor_preserves_all_metadata_and_rejects_each_shape_fault() {
        for space in [ConvertedColorSpace::Grayscale, ConvertedColorSpace::Srgb] {
            let kinds: &[ConvertedChannelType] = match space {
                ConvertedColorSpace::Grayscale => &[ConvertedChannelType::Gray],
                ConvertedColorSpace::Srgb => &[
                    ConvertedChannelType::Red,
                    ConvertedChannelType::Green,
                    ConvertedChannelType::Blue,
                ],
            };
            for premultiplied in [false, true] {
                for (width, height) in [(0, 2), (2, 0), (2, 1)] {
                    let pixels = width as usize * height as usize;
                    let colors: Vec<Vec<u16>> = (0..kinds.len())
                        .map(|channel| {
                            (0..pixels)
                                .map(|pixel| (100 * channel + pixel) as u16)
                                .collect()
                        })
                        .collect();
                    let alpha: Vec<u16> = (0..pixels).map(|pixel| 32_768 + pixel as u16).collect();
                    let image = from_layer(
                        width,
                        height,
                        space,
                        colors.clone(),
                        alpha.clone(),
                        premultiplied,
                    )
                    .unwrap();
                    assert_eq!(
                        (
                            image.width(),
                            image.height(),
                            image.color_space(),
                            image.precision()
                        ),
                        (width, height, space, 16)
                    );
                    assert_eq!(image.channels().len(), kinds.len() + 1);
                    for (index, kind) in kinds.iter().enumerate() {
                        assert_eq!(image.channel(index as u16).unwrap().channel_type(), *kind);
                        assert_eq!(
                            image.channel(index as u16).unwrap().samples(),
                            colors[index]
                        );
                    }
                    assert_eq!(
                        image.channel(kinds.len() as u16).unwrap().channel_type(),
                        ConvertedChannelType::Alpha {
                            premultiplied,
                            association: ChannelAssociation::WholeImage,
                        }
                    );
                    assert_eq!(image.channel(kinds.len() as u16).unwrap().samples(), alpha);
                    let mut wrong_colors = colors.clone();
                    wrong_colors.push(vec![0; pixels]);
                    assert_eq!(
                        from_layer(
                            width,
                            height,
                            space,
                            wrong_colors,
                            alpha.clone(),
                            premultiplied
                        )
                        .err(),
                        Some(invalid_color_input())
                    );
                    assert_eq!(
                        from_layer(
                            width,
                            height,
                            space,
                            colors.clone(),
                            vec![0; pixels + 1],
                            premultiplied
                        )
                        .err(),
                        Some(invalid_color_input())
                    );
                    for index in 0..kinds.len() {
                        let mut wrong_colors = colors.clone();
                        wrong_colors[index].push(0);
                        assert_eq!(
                            from_layer(
                                width,
                                height,
                                space,
                                wrong_colors,
                                alpha.clone(),
                                premultiplied
                            )
                            .err(),
                            Some(invalid_color_input())
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn converted_crop_preserves_color_and_alpha_metadata_and_checks_bounds() {
        let make_image = || {
            from_composited(
                3,
                2,
                ConvertedColorSpace::Srgb,
                vec![
                    vec![0, 1, 2, 3, 4, 5],
                    vec![10, 11, 12, 13, 14, 15],
                    vec![20, 21, 22, 23, 24, 25],
                ],
                vec![0, 100, 200, 300, 400, 500],
            )
            .unwrap()
        };
        let cropped = make_image().crop(1, 0, 2, 2).unwrap();
        assert_eq!(
            (cropped.width(), cropped.height(), cropped.precision()),
            (2, 2, 16)
        );
        assert_eq!(cropped.color_space(), ConvertedColorSpace::Srgb);
        assert_eq!(cropped.channel(0).unwrap().samples(), &[1, 2, 4, 5]);
        assert_eq!(cropped.channel(3).unwrap().samples(), &[100, 200, 400, 500]);
        assert_eq!(
            cropped.channel(0).unwrap().channel_type(),
            ConvertedChannelType::Red
        );
        assert_eq!(
            cropped.channel(3).unwrap().channel_type(),
            ConvertedChannelType::Alpha {
                premultiplied: false,
                association: ChannelAssociation::WholeImage,
            }
        );
        assert_eq!(make_image().crop(0, 0, 3, 2).unwrap(), make_image());
        for (x, y, width, height) in [
            (0, 0, 0, 1),
            (3, 0, 1, 1),
            (0, 2, 1, 1),
            (u32::MAX, 0, 1, 1),
        ] {
            assert!(make_image().crop(x, y, width, height).is_err());
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
