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

    spec fn converted_channel_result(result: Result<ConvertedChannel, DecodeError>)
        -> Result<crate::specs::ColorChannelModel, DecodeError>
    {
        match result {
            Ok(channel) => Ok(converted_channel_view(&channel)),
            Err(error) => Err(error),
        }
    }

    proof fn converted_channel_views_push(channels: Seq<ConvertedChannel>, channel: ConvertedChannel)
        ensures converted_channel_views(channels.push(channel))
            == converted_channel_views(channels).push(converted_channel_view(&channel)),
    {
        assert(converted_channel_views(channels.push(channel))
            =~= converted_channel_views(channels).push(converted_channel_view(&channel)));
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

#[cfg(verus_keep_ghost)]
verus! {
    #[verifier::inline]
    spec fn channel_layout_result(result: Result<ChannelLayout, DecodeError>) -> crate::specs::ColorLayoutResult {
        match result {
            Ok(layout) => Ok((layout.colors@, layout.alphas@)),
            Err(error) => Err(error),
        }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures converted_image_result(result) == crate::specs::color_convert_jp2(
            crate::image::typed_image_component_views(image),
            crate::image::typed_image_definition_views(image),
            crate::image::typed_image_geometry(image), crate::jp2::color_specification_method(color),
            crate::limits::resource_decoded_bytes_limit(limits), crate::limits::resource_work_limit(limits),
        ),
    )
)]
pub fn convert_jp2_color(
    image: &TypedImage,
    color: &ColorSpecification,
    limits: &ResourceLimits,
) -> Result<ConvertedImage, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost input = crate::image::typed_image_component_views(image);
        let ghost geometry = crate::image::typed_image_geometry(image);
    }
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
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost colors = layout.colors@; }

    let pixel_count = checked_area(image.width(), image.height())?;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(crate::proofs::internal_specs::color_conversion_ready(
            input, crate::image::typed_image_definition_views(image), geometry,
            crate::jp2::color_specification_method(color), (layout.colors@, layout.alphas@),
            (color_count, output_space, sycc), pixel_count,
            crate::limits::resource_decoded_bytes_limit(limits), crate::limits::resource_work_limit(limits),
        ));
    }
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
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_color_sycc_empty(input, geometry, colors, width);
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant pixel <= pixel_count,
                    sycc,
                    input == crate::image::typed_image_component_views(image),
                    geometry == crate::image::typed_image_geometry(image),
                    colors == layout.colors@,
                    width as nat == geometry.0 as nat,
                    width > 0,
                    crate::specs::part2_layer_area(geometry.0, geometry.1) == Ok(pixel_count),
                    crate::specs::color_sycc_planes_prefix(input, geometry, colors, width, pixel as nat)
                        == Ok((red@, green@, blue@)),
                    crate::proofs::internal_specs::color_conversion_ready(
                        input, crate::image::typed_image_definition_views(image), geometry,
                        crate::jp2::color_specification_method(color), (layout.colors@, layout.alphas@),
                        (color_count, output_space, sycc), pixel_count,
                        crate::limits::resource_decoded_bytes_limit(limits), crate::limits::resource_work_limit(limits),
                    ),
                decreases pixel_count - pixel,
            )
        )]
        while pixel < pixel_count {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_color_sycc_step(input, geometry, colors, width, pixel as nat);
            }
            let (r, g, b) = match convert_sycc_pixel(image, &layout.colors, pixel, width) {
                Ok(value) => value,
                Err(error) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        crate::proofs::production_impl::production_color_sycc_error(
                            input, geometry, colors, width, pixel as nat + 1, pixel_count as nat, error,
                        );
                    }
                    return Err(error);
                }
            };
            red.push(r);
            green.push(g);
            blue.push(b);
            pixel = pixel.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost sycc_planes = (red@, green@, blue@); }
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
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(converted_channel_views(channels@) =~= seq![
                (ConvertedChannelType::Red, sycc_planes.0),
                (ConvertedChannelType::Green, sycc_planes.1),
                (ConvertedChannelType::Blue, sycc_planes.2),
            ]);
        }
    } else {
        let mut color_index = 0_usize;
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_color_channels_empty(input, geometry, colors, color_count);
            assert(converted_channel_views(channels@) =~= Seq::<crate::specs::ColorChannelModel>::empty());
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant color_index <= color_count,
                    !sycc,
                    input == crate::image::typed_image_component_views(image),
                    geometry == crate::image::typed_image_geometry(image),
                    colors == layout.colors@,
                    crate::specs::color_convert_channels_prefix(input, geometry, colors, color_count, color_index as nat)
                        == Ok(converted_channel_views(channels@)),
                    crate::proofs::internal_specs::color_conversion_ready(
                        input, crate::image::typed_image_definition_views(image), geometry,
                        crate::jp2::color_specification_method(color), (layout.colors@, layout.alphas@),
                        (color_count, output_space, sycc), pixel_count,
                        crate::limits::resource_decoded_bytes_limit(limits), crate::limits::resource_work_limit(limits),
                    ),
                decreases color_count - color_index,
            )
        )]
        while color_index < color_count {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_color_channels_step(
                    input, geometry, colors, color_count, color_index as nat,
                );
            }
            let channel = match convert_color_channel(
                image,
                &layout.colors,
                color_count,
                color_index,
            ) {
                Ok(channel) => channel,
                Err(error) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        crate::proofs::production_impl::production_color_channels_error(
                            input, geometry, colors, color_count, color_index as nat + 1, color_count as nat, error,
                        );
                    }
                    return Err(error);
                }
            };
            #[cfg(verus_keep_ghost)]
            proof! { converted_channel_views_push(channels@, channel); }
            channels.push(channel);
            color_index = color_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
    }

    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost initial = converted_channel_views(channels@);
        let ghost alphas = layout.alphas@;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(crate::specs::color_convert_colors(input, geometry, colors, color_count, sycc) == Ok(initial));
        crate::proofs::production_impl::production_color_alphas_empty(input, geometry, alphas, initial);
    }
    let mut alpha_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant alpha_index <= layout.alphas.len(),
                input == crate::image::typed_image_component_views(image),
                geometry == crate::image::typed_image_geometry(image),
                colors == layout.colors@,
                alphas == layout.alphas@,
                crate::specs::color_convert_colors(input, geometry, colors, color_count, sycc) == Ok(initial),
                crate::specs::color_append_alphas_prefix(input, geometry, alphas, initial, alpha_index as nat)
                    == Ok(converted_channel_views(channels@)),
                crate::proofs::internal_specs::color_conversion_ready(
                    input, crate::image::typed_image_definition_views(image), geometry,
                    crate::jp2::color_specification_method(color), (layout.colors@, layout.alphas@),
                    (color_count, output_space, sycc), pixel_count,
                    crate::limits::resource_decoded_bytes_limit(limits), crate::limits::resource_work_limit(limits),
                ),
            decreases layout.alphas.len() - alpha_index,
        )
    )]
    while alpha_index < layout.alphas.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_color_alphas_step(input, geometry, alphas, initial, alpha_index as nat);
        }
        let alpha = *layout
            .alphas
            .get(alpha_index)
            .ok_or_else(invalid_color_input)?;
        let channel = match convert_alpha_channel(image, alpha) {
            Ok(channel) => channel,
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_color_alphas_error(
                        input, geometry, alphas, initial, alpha_index as nat + 1, alphas.len(), error,
                    );
                }
                return Err(error);
            }
        };
        #[cfg(verus_keep_ghost)]
        proof! { converted_channel_views_push(channels@, channel); }
        channels.push(channel);
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

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures converted_channel_result(result) == crate::specs::color_convert_channel(
            crate::image::typed_image_component_views(image), crate::image::typed_image_geometry(image),
            colors@, color_count, index,
        ),
    )
)]
fn convert_color_channel(
    image: &TypedImage,
    colors: &[usize],
    color_count: usize,
    index: usize,
) -> Result<ConvertedChannel, DecodeError> {
    let component_index = *colors.get(index).ok_or_else(invalid_color_input)?;
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
    let channel_type = match (color_count, index) {
        (1, 0) => ConvertedChannelType::Gray,
        (3, 0) => ConvertedChannelType::Red,
        (3, 1) => ConvertedChannelType::Green,
        (3, 2) => ConvertedChannelType::Blue,
        _ => return Err(invalid_color_input()),
    };
    Ok(ConvertedChannel {
        channel_type,
        samples,
    })
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures converted_channel_result(result) == crate::specs::color_convert_alpha(
            crate::image::typed_image_component_views(image), crate::image::typed_image_geometry(image), alpha,
        ),
    )
)]
fn convert_alpha_channel(
    image: &TypedImage,
    alpha: (usize, bool, ChannelAssociation),
) -> Result<ConvertedChannel, DecodeError> {
    let component = image
        .components()
        .get(alpha.0)
        .ok_or_else(invalid_color_input)?;
    let samples = normalize_component(
        component,
        image.width(),
        image.height(),
        image.x_origin(),
        image.y_origin(),
    )?;
    Ok(ConvertedChannel {
        channel_type: ConvertedChannelType::Alpha {
            premultiplied: alpha.1,
            association: alpha.2,
        },
        samples,
    })
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::color_sycc_pixel(
            crate::image::typed_image_component_views(image),
            crate::image::typed_image_geometry(image).2, crate::image::typed_image_geometry(image).3,
            colors@, pixel, width,
        ),
    )
)]
fn convert_sycc_pixel(
    image: &TypedImage,
    colors: &[usize],
    pixel: usize,
    width: usize,
) -> Result<(u16, u16, u16), DecodeError> {
    let (x, y) = pixel_coordinates(pixel, width)?;
    let luminance = normalized_at(
        image,
        *colors.first().ok_or_else(invalid_color_input)?,
        x,
        y,
    )?;
    let cb =
        normalized_difference_at(image, *colors.get(1).ok_or_else(invalid_color_input)?, x, y)?;
    let cr =
        normalized_difference_at(image, *colors.get(2).ok_or_else(invalid_color_input)?, x, y)?;
    sycc_to_srgb(luminance, cb, cr)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures channel_layout_result(result) == crate::specs::color_channel_layout(
            crate::image::typed_image_definition_views(image), color_count,
        ),
    )
)]
fn channel_layout(image: &TypedImage, color_count: usize) -> Result<ChannelLayout, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost input_definitions = crate::image::typed_image_definition_views(image); }
    let mut has_definitions = false;
    let component_count = image.components().len();
    let mut component_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component_index <= component_count,
                input_definitions == crate::image::typed_image_definition_views(image),
                component_count == input_definitions.len(),
                has_definitions == (exists|i: int| #![trigger input_definitions[i]]
                    0 <= i < component_index && input_definitions[i].len() > 0),
            decreases component_count - component_index,
        )
    )]
    while component_index < component_count {
        let component = image
            .components()
            .get(component_index)
            .ok_or_else(invalid_color_input)?;
        let definitions = component.channel_definitions();
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(input_definitions[component_index as int].len() == definitions.len());
            crate::proofs::production_impl::production_color_definition_presence_step(
                input_definitions, component_index as nat,
            );
        }
        if !definitions.is_empty() {
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
                    input_definitions == crate::image::typed_image_definition_views(image),
                    input_definitions.len() == color_count,
                    !(exists|i: int| #![trigger input_definitions[i]]
                        0 <= i < input_definitions.len() && input_definitions[i].len() > 0),
                    colors@ == Seq::new(component_index as nat, |i: int| i as usize),
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
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_color_layout_empty(input_definitions, color_count);
        assert(colors@ =~= Seq::new(color_count as nat, |i: int| usize::MAX));
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component_index <= component_count,
                input_definitions == crate::image::typed_image_definition_views(image),
                component_count == input_definitions.len(),
                colors.len() == color_count,
                exists|i: int| #![trigger input_definitions[i]]
                    0 <= i < input_definitions.len() && input_definitions[i].len() > 0,
                crate::specs::color_layout_components_prefix(input_definitions, color_count, component_index as nat)
                    == Ok((colors@, alphas@)),
            decreases component_count - component_index,
        )
    )]
    while component_index < component_count {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost initial = (colors@, alphas@); }
        let component = image
            .components()
            .get(component_index)
            .ok_or_else(invalid_color_input)?;
        let definitions = component.channel_definitions();
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost current_definitions = crate::image::channel_definition_views(definitions@); }
        let mut definition_index = 0_usize;
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_color_layout_step(input_definitions, color_count, component_index as nat);
            crate::proofs::production_impl::production_color_definitions_empty(initial, current_definitions, component_index, color_count);
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant definition_index <= definitions.len(),
                    input_definitions == crate::image::typed_image_definition_views(image),
                    component_index < component_count && component_count == input_definitions.len(),
                    current_definitions == crate::image::channel_definition_views(definitions@),
                    current_definitions == input_definitions[component_index as int],
                    colors.len() == color_count,
                    exists|i: int| #![trigger input_definitions[i]]
                        0 <= i < input_definitions.len() && input_definitions[i].len() > 0,
                    crate::specs::color_layout_components_prefix(input_definitions, color_count, component_index as nat) == Ok(initial),
                    crate::specs::color_layout_components_prefix(input_definitions, color_count, component_index as nat + 1)
                        == crate::specs::color_layout_definitions_prefix(
                            initial, current_definitions, component_index, color_count, current_definitions.len(),
                        ),
                    crate::specs::color_layout_definitions_prefix(initial, current_definitions,
                        component_index, color_count, definition_index as nat) == Ok((colors@, alphas@)),
                decreases definitions.len() - definition_index,
            )
        )]
        while definition_index < definitions.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_color_definitions_step(
                    initial, current_definitions, component_index, color_count, definition_index as nat,
                );
            }
            let definition = *definitions
                .get(definition_index)
                .ok_or_else(invalid_color_input)?;
            match definition.channel_type() {
                ChannelType::Color => {
                    let association = match definition.association() {
                        ChannelAssociation::Color(value) => value,
                        _ => {
                            #[cfg(verus_keep_ghost)]
                            proof! {
                                crate::proofs::production_impl::production_color_definitions_error(
                                    initial, current_definitions, component_index, color_count, definition_index as nat + 1,
                                    current_definitions.len(), crate::specs::color_layout_error(),
                                );
                                crate::proofs::production_impl::production_color_layout_error(
                                    input_definitions, color_count, component_index as nat + 1,
                                    input_definitions.len(), crate::specs::color_layout_error(),
                                );
                            }
                            return Err(invalid_channel_layout());
                        }
                    };
                    if association == 0 || usize::from(association) > color_count {
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            crate::proofs::production_impl::production_color_definitions_error(
                                initial, current_definitions, component_index, color_count, definition_index as nat + 1,
                                current_definitions.len(), crate::specs::color_layout_error(),
                            );
                            crate::proofs::production_impl::production_color_layout_error(
                                input_definitions, color_count, component_index as nat + 1,
                                input_definitions.len(), crate::specs::color_layout_error(),
                            );
                        }
                        return Err(invalid_channel_layout());
                    }
                    let slot_index = usize::from(association - 1);
                    if colors[slot_index] != usize::MAX && colors[slot_index] != component_index {
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            crate::proofs::production_impl::production_color_definitions_error(
                                initial, current_definitions, component_index, color_count, definition_index as nat + 1,
                                current_definitions.len(), crate::specs::color_layout_error(),
                            );
                            crate::proofs::production_impl::production_color_layout_error(
                                input_definitions, color_count, component_index as nat + 1,
                                input_definitions.len(), crate::specs::color_layout_error(),
                            );
                        }
                        return Err(invalid_channel_layout());
                    }
                    *colors.get_mut(slot_index).ok_or_else(invalid_color_input)? = component_index;
                }
                ChannelType::Opacity | ChannelType::PremultipliedOpacity => {
                    match validate_alpha_association(definition.association(), color_count) {
                        Ok(()) => {}
                        Err(error) => {
                            #[cfg(verus_keep_ghost)]
                            proof! {
                                crate::proofs::production_impl::production_color_definitions_error(
                                    initial, current_definitions, component_index, color_count, definition_index as nat + 1,
                                    current_definitions.len(), error,
                                );
                                crate::proofs::production_impl::production_color_layout_error(
                                    input_definitions, color_count, component_index as nat + 1,
                                    input_definitions.len(), error,
                                );
                            }
                            return Err(error);
                        }
                    }
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
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(crate::specs::color_layout_definitions_prefix(initial, current_definitions,
                    component_index, color_count, definition_index as nat + 1) == Ok((colors@, alphas@)));
            }
            definition_index = definition_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        component_index = component_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost complete = (colors@, alphas@); }
    component_index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component_index <= colors.len(),
                input_definitions == crate::image::typed_image_definition_views(image),
                complete == (colors@, alphas@),
                exists|i: int| #![trigger input_definitions[i]]
                    0 <= i < input_definitions.len() && input_definitions[i].len() > 0,
                crate::specs::color_layout_components_prefix(input_definitions, color_count, input_definitions.len()) == Ok(complete),
                forall|i: int| #![trigger complete.0[i]] 0 <= i < component_index ==> complete.0[i] != usize::MAX,
            decreases colors.len() - component_index,
        )
    )]
    while component_index < colors.len() {
        if *colors
            .get(component_index)
            .ok_or_else(invalid_color_input)?
            == usize::MAX
        {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(complete.0[component_index as int] == usize::MAX);
                assert(exists|i: int| #![trigger complete.0[i]]
                    0 <= i < complete.0.len() && complete.0[i] == usize::MAX);
            }
            return Err(invalid_channel_layout());
        }
        component_index = component_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    Ok(ChannelLayout { colors, alphas })
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result => ensures result == crate::specs::color_alpha_association(association, color_count),)
)]
fn validate_alpha_association(
    association: ChannelAssociation,
    color_count: usize,
) -> Result<(), DecodeError> {
    match association {
        ChannelAssociation::Color(value) if value == 0 || usize::from(value) > color_count => {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidAlphaAssociation,
            });
        }
        _ => {}
    }
    Ok(())
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::color_validate_components(
            crate::image::typed_image_component_views(image),
            crate::image::typed_image_geometry(image), (layout.colors@, layout.alphas@),
        ),
    )
)]
fn validate_components(image: &TypedImage, layout: &ChannelLayout) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost components = crate::image::typed_image_component_views(image);
        let ghost geometry = crate::image::typed_image_geometry(image);
        let ghost colors = layout.colors@;
        let ghost alphas = Seq::new(layout.alphas@.len(), |i: int| layout.alphas@[i].0);
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_color_validation_empty(components, geometry, colors);
        crate::proofs::production_impl::production_color_validation_empty(components, geometry, alphas);
    }
    let mut color_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant color_index <= layout.colors.len(),
                components == crate::image::typed_image_component_views(image),
                geometry == crate::image::typed_image_geometry(image),
                colors == layout.colors@,
                alphas == Seq::new(layout.alphas@.len(), |i: int| layout.alphas@[i].0),
                crate::specs::color_validate_indices_prefix(components, geometry, colors, color_index as nat) == Ok(()),
            decreases layout.colors.len() - color_index,
        )
    )]
    while color_index < layout.colors.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_color_validation_step(components, geometry, colors, color_index as nat);
        }
        let component_index = *layout
            .colors
            .get(color_index)
            .ok_or_else(invalid_color_input)?;
        let status = validate_component_index(image, component_index);
        #[cfg(verus_keep_ghost)]
        proof! {
            if let Err(error) = status {
                crate::proofs::production_impl::production_color_validation_error(
                    components, geometry, colors, color_index as nat + 1, colors.len(), error,
                );
            }
        }
        status?;
        color_index = color_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let mut alpha_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant alpha_index <= layout.alphas.len(),
                components == crate::image::typed_image_component_views(image),
                geometry == crate::image::typed_image_geometry(image),
                colors == layout.colors@,
                alphas == Seq::new(layout.alphas@.len(), |i: int| layout.alphas@[i].0),
                crate::specs::color_validate_indices_prefix(components, geometry, colors, colors.len()) == Ok(()),
                crate::specs::color_validate_indices_prefix(components, geometry, alphas, alpha_index as nat) == Ok(()),
            decreases layout.alphas.len() - alpha_index,
        )
    )]
    while alpha_index < layout.alphas.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_color_validation_step(components, geometry, alphas, alpha_index as nat);
        }
        let component_index = layout
            .alphas
            .get(alpha_index)
            .ok_or_else(invalid_color_input)?
            .0;
        let status = validate_component_index(image, component_index);
        #[cfg(verus_keep_ghost)]
        proof! {
            if let Err(error) = status {
                crate::proofs::production_impl::production_color_validation_error(
                    components, geometry, alphas, alpha_index as nat + 1, alphas.len(), error,
                );
            }
        }
        status?;
        alpha_index = alpha_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::color_validate_component_index(
            crate::image::typed_image_component_views(image),
            crate::image::typed_image_geometry(image), index,
        ),
    )
)]
fn validate_component_index(image: &TypedImage, index: usize) -> Result<(), DecodeError> {
    let component = image
        .components()
        .get(index)
        .ok_or_else(invalid_color_input)?;
    validate_component(component, image)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::color_validate_component(
            crate::image::typed_component_view(component), crate::image::typed_image_geometry(image),
        ),
    )
)]
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

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::color_validate_output_size(
            crate::image::typed_image_geometry(image).0, crate::image::typed_image_geometry(image).1,
            color_count, alpha_count, crate::limits::resource_decoded_bytes_limit(limits),
            crate::limits::resource_work_limit(limits),
        ),
    )
)]
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
        return Err(DecodeError::LimitExceeded {
            resource: Resource::DecodedBytes,
            limit: limits.maximum_decoded_bytes(),
            actual: bytes,
        });
    }
    let work = u64::try_from(samples).map_err(|_| arithmetic_overflow())?;
    if work > limits.maximum_work_units() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::WorkUnits,
            limit: limits.maximum_work_units(),
            actual: work,
        });
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

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::color_image_normalized(
            crate::image::typed_image_component_views(image),
            crate::image::typed_image_geometry(image).2,
            crate::image::typed_image_geometry(image).3, component_index, x, y,
        ),
    )
)]
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

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result == crate::specs::color_image_difference(
            crate::image::typed_image_component_views(image),
            crate::image::typed_image_geometry(image).2,
            crate::image::typed_image_geometry(image).3, component_index, x, y,
        ),
    )
)]
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
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::spec_properties::color_precision_binary_bounds(info.precision);
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
    match i32::try_from(rounded) {
        Ok(value) => Ok(value),
        Err(_) => Err(arithmetic_overflow()),
    }
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

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result => ensures result == crate::specs::color_sycc_to_srgb(y, cb, cr),)
)]
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
        .ok_or_else(arithmetic_overflow)?;
    let green = green
        .checked_sub(green_red_delta)
        .ok_or_else(arithmetic_overflow)?;
    let blue = y.checked_add(blue_delta).ok_or_else(arithmetic_overflow)?;
    Ok((clamp_u16(red), clamp_u16(green), clamp_u16(blue)))
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result => ensures result == crate::specs::color_rounded_fixed(value),)
)]
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

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result => ensures result == crate::specs::color_clamp_u16(value as int),)
)]
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

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(error => ensures error == (DecodeError::UnsupportedColorConversion { reason }),)
)]
const fn unsupported(reason: UnsupportedColorConversion) -> DecodeError {
    DecodeError::UnsupportedColorConversion { reason }
}

#[cfg_attr(verus_keep_ghost, verus_spec(error => ensures error == crate::specs::color_layout_error(),))]
const fn invalid_channel_layout() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidChannelLayout,
    }
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

    fn registered_coordinate_wide(value: u32, sampling: u8, registration: u16, ceil: bool) -> u128 {
        let numerator = (u128::from(value) * 65_536).saturating_sub(u128::from(registration));
        let denominator = u128::from(sampling) * 65_536;
        if ceil {
            numerator.div_ceil(denominator)
        } else {
            numerator / denominator
        }
    }

    fn sampled_nominal_plane_wide_reference(
        nominal: &[u64],
        component_size: (u32, u32),
        info: ComponentInfo,
        output_size: (u32, u32),
        origin: (u32, u32),
    ) -> Result<Vec<u64>, DecodeError> {
        if output_size.0 == 0 {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::ContainerCodestreamMismatch,
            });
        }
        let coordinate = registered_coordinate_wide;
        let x0 = coordinate(origin.0, info.x_subsampling, info.x_registration, true);
        let y0 = coordinate(origin.1, info.y_subsampling, info.y_registration, true);
        let mut result = Vec::new();
        for y in 0..output_size.1 {
            for x in 0..output_size.0 {
                let sx = coordinate(origin.0 + x, info.x_subsampling, info.x_registration, false)
                    .clamp(x0, x0 + u128::from(component_size.0) - 1);
                let sy = coordinate(origin.1 + y, info.y_subsampling, info.y_registration, false)
                    .clamp(y0, y0 + u128::from(component_size.1) - 1);
                let index =
                    usize::try_from((sy - y0) * u128::from(component_size.0) + sx - x0).unwrap();
                result.push(nominal[index]);
            }
        }
        Ok(result)
    }

    fn sampled_plane_wide_reference(
        nominal: &[u64],
        component_size: (u32, u32),
        info: ComponentInfo,
        output_size: (u32, u32),
        origin: (u32, u32),
    ) -> Result<Vec<u16>, DecodeError> {
        let plane = sampled_nominal_plane_wide_reference(
            nominal,
            component_size,
            info,
            output_size,
            origin,
        )?;
        let maximum = (1_u128 << info.precision) - 1;
        Ok(plane
            .into_iter()
            .map(|value| {
                u16::try_from((u128::from(value) * 65_535 + maximum / 2) / maximum).unwrap()
            })
            .collect())
    }

    fn chroma_wide_reference(value: u64, precision: u8) -> i32 {
        let midpoint = 1_u64 << (precision - 1);
        let maximum = 2 * midpoint - 1;
        let difference = i128::from(value) - i128::from(midpoint);
        let magnitude =
            (difference.unsigned_abs() * 65_535 + u128::from(maximum / 2)) / u128::from(maximum);
        i32::try_from(magnitude).unwrap() * if difference < 0 { -1 } else { 1 }
    }

    fn sycc_wide_reference(y: u16, cb: i32, cr: i32) -> (u16, u16, u16) {
        fn round(value: i64) -> i64 {
            let magnitude = (value.unsigned_abs() + 32_768) / 65_536;
            i64::try_from(magnitude).unwrap() * value.signum()
        }
        fn sample(value: i64) -> u16 {
            u16::try_from(value.clamp(0, 65_535)).unwrap()
        }
        let cb = i64::from(cb);
        let cr = i64::from(cr);
        (
            sample(i64::from(y) + round(91_881 * cr)),
            sample(i64::from(y) - round(22_554 * cb) - round(46_802 * cr)),
            sample(i64::from(y) + round(116_130 * cb)),
        )
    }

    #[test]
    fn complete_color_conversion_matches_independent_registered_planes_and_metadata() {
        let mut cases = 0;
        for precision in [1, 8, 16, 31, 38] {
            let maximum = (1_u64 << precision) - 1;
            let midpoint = 1_i64 << (precision - 1);
            for signed in [false, true] {
                for method in [ENUMERATED_GRAYSCALE, ENUMERATED_SRGB, ENUMERATED_SYCC] {
                    let color_count = if method == ENUMERATED_GRAYSCALE { 1 } else { 3 };
                    for size in [(3, 2), (5, 3)] {
                        for (origin, registration) in [
                            ((0, 0), (0, 0)),
                            ((0, 0), (32_768, 65_535)),
                            ((3, 5), (0, 0)),
                            ((3, 5), (32_768, 65_535)),
                        ] {
                            for alpha_mode in 0..3 {
                                for reverse in [false, true] {
                                    let mut components = Vec::new();
                                    let mut raw_planes = Vec::new();
                                    let mut normalized_planes = Vec::new();
                                    for role in 0..color_count + usize::from(alpha_mode != 0) {
                                        let info = ComponentInfo {
                                            precision,
                                            signed,
                                            x_subsampling: if role == 0 { 1 } else { 2 },
                                            y_subsampling: 1,
                                            x_registration: registration.0,
                                            y_registration: registration.1,
                                        };
                                        let width = u32::try_from(
                                            registered_coordinate_wide(
                                                origin.0 + size.0,
                                                info.x_subsampling,
                                                registration.0,
                                                true,
                                            ) - registered_coordinate_wide(
                                                origin.0,
                                                info.x_subsampling,
                                                registration.0,
                                                true,
                                            ),
                                        )
                                        .unwrap();
                                        let height = u32::try_from(
                                            registered_coordinate_wide(
                                                origin.1 + size.1,
                                                info.y_subsampling,
                                                registration.1,
                                                true,
                                            ) - registered_coordinate_wide(
                                                origin.1,
                                                info.y_subsampling,
                                                registration.1,
                                                true,
                                            ),
                                        )
                                        .unwrap();
                                        let nominal: Vec<u64> = (0..u64::from(width)
                                            * u64::from(height))
                                            .map(|index| {
                                                u64::try_from(
                                                    ((u128::from(index) * 977
                                                        + (role as u128) * 113
                                                        + 1)
                                                        * u128::from(maximum)
                                                        / 13)
                                                        % (u128::from(maximum) + 1),
                                                )
                                                .unwrap()
                                            })
                                            .collect();
                                        raw_planes.push(
                                            sampled_nominal_plane_wide_reference(
                                                &nominal,
                                                (width, height),
                                                info,
                                                size,
                                                origin,
                                            )
                                            .unwrap(),
                                        );
                                        normalized_planes.push(
                                            sampled_plane_wide_reference(
                                                &nominal,
                                                (width, height),
                                                info,
                                                size,
                                                origin,
                                            )
                                            .unwrap(),
                                        );
                                        let samples = if signed {
                                            TypedSamples::Signed(
                                                nominal
                                                    .iter()
                                                    .map(|value| {
                                                        i64::try_from(*value).unwrap() - midpoint
                                                    })
                                                    .collect(),
                                            )
                                        } else {
                                            TypedSamples::Unsigned(nominal)
                                        };
                                        let mut component = TypedComponent::from_parts(
                                            width, height, info, samples,
                                        );
                                        let definition = if role < color_count {
                                            ChannelDefinition::new(
                                                ChannelType::Color,
                                                ChannelAssociation::Color(
                                                    u16::try_from(role + 1).unwrap(),
                                                ),
                                            )
                                        } else if alpha_mode == 1 {
                                            ChannelDefinition::new(
                                                ChannelType::Opacity,
                                                ChannelAssociation::WholeImage,
                                            )
                                        } else {
                                            ChannelDefinition::new(
                                                ChannelType::PremultipliedOpacity,
                                                ChannelAssociation::Color(1),
                                            )
                                        };
                                        component.add_channel_definition(definition);
                                        components.push(component);
                                    }
                                    if reverse {
                                        components.reverse();
                                    }
                                    let image = TypedImage::from_components_with_origin(
                                        size.0,
                                        size.1,
                                        origin.0,
                                        origin.1,
                                        components,
                                        &ResourceLimits::default(),
                                    )
                                    .unwrap();
                                    let converted = convert_jp2_color(
                                        &image,
                                        &ColorSpecification::enumerated(method),
                                        &ResourceLimits::default(),
                                    )
                                    .unwrap();
                                    let color_planes = if method == ENUMERATED_SYCC {
                                        let mut planes = vec![Vec::new(), Vec::new(), Vec::new()];
                                        for ((y, cb), cr) in normalized_planes[0]
                                            .iter()
                                            .zip(&raw_planes[1])
                                            .zip(&raw_planes[2])
                                        {
                                            let (r, g, b) = sycc_wide_reference(
                                                *y,
                                                chroma_wide_reference(*cb, precision),
                                                chroma_wide_reference(*cr, precision),
                                            );
                                            planes[0].push(r);
                                            planes[1].push(g);
                                            planes[2].push(b);
                                        }
                                        planes
                                    } else {
                                        normalized_planes[..color_count].to_vec()
                                    };
                                    let mut expected: Vec<_> = color_planes
                                        .into_iter()
                                        .enumerate()
                                        .map(|(index, samples)| {
                                            let kind = match (color_count, index) {
                                                (1, 0) => ConvertedChannelType::Gray,
                                                (3, 0) => ConvertedChannelType::Red,
                                                (3, 1) => ConvertedChannelType::Green,
                                                (3, 2) => ConvertedChannelType::Blue,
                                                _ => unreachable!(),
                                            };
                                            (kind, samples)
                                        })
                                        .collect();
                                    if alpha_mode != 0 {
                                        expected.push((
                                            ConvertedChannelType::Alpha {
                                                premultiplied: alpha_mode == 2,
                                                association: if alpha_mode == 1 {
                                                    ChannelAssociation::WholeImage
                                                } else {
                                                    ChannelAssociation::Color(1)
                                                },
                                            },
                                            normalized_planes[color_count].clone(),
                                        ));
                                    }
                                    assert_eq!(
                                        (
                                            converted.width(),
                                            converted.height(),
                                            converted.precision()
                                        ),
                                        (size.0, size.1, 16)
                                    );
                                    assert_eq!(
                                        converted.color_space(),
                                        if color_count == 1 {
                                            ConvertedColorSpace::Grayscale
                                        } else {
                                            ConvertedColorSpace::Srgb
                                        }
                                    );
                                    assert_eq!(
                                        converted
                                            .channels()
                                            .iter()
                                            .map(|channel| (
                                                channel.channel_type(),
                                                channel.samples().to_vec()
                                            ))
                                            .collect::<Vec<_>>(),
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
        assert_eq!(cases, 1_440);
    }

    #[test]
    fn complete_color_admission_matches_wide_checked_count_and_limit_model() {
        let overflow = Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        });
        let mut cases = 0;
        for (width, height) in [(0, 0), (1, 1), (3, 2), (u32::MAX, 1), (u32::MAX, u32::MAX)] {
            let image =
                TypedImage::from_components(width, height, Vec::new(), &ResourceLimits::default())
                    .unwrap();
            for colors in [0, 1, 3, usize::MAX] {
                for alphas in [0, 1, usize::MAX] {
                    for (byte_limit, work_limit) in [(0, 0), (2, 1), (24, 12)] {
                        let limits = ResourceLimits::default()
                            .with_maximum_decoded_bytes(byte_limit)
                            .unwrap()
                            .with_maximum_work_units(work_limit)
                            .unwrap();
                        let channels = (colors as u128) + alphas as u128;
                        let area = u128::from(width) * u128::from(height);
                        let expected = if channels > usize::MAX as u128 || area > usize::MAX as u128
                        {
                            overflow
                        } else {
                            let samples = channels * area;
                            if samples > usize::MAX as u128 {
                                overflow
                            } else {
                                let bytes = samples * 2;
                                if bytes > u128::from(u64::MAX) {
                                    overflow
                                } else if bytes > u128::from(byte_limit) {
                                    Err(DecodeError::LimitExceeded {
                                        resource: Resource::DecodedBytes,
                                        limit: byte_limit,
                                        actual: u64::try_from(bytes).unwrap(),
                                    })
                                } else if samples > u128::from(work_limit) {
                                    Err(DecodeError::LimitExceeded {
                                        resource: Resource::WorkUnits,
                                        limit: work_limit,
                                        actual: u64::try_from(samples).unwrap(),
                                    })
                                } else {
                                    Ok(())
                                }
                            }
                        };
                        assert_eq!(
                            validate_output_size(&image, colors, alphas, &limits),
                            expected
                        );
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 180);
    }

    #[test]
    fn complete_color_conversion_preserves_format_metadata_budget_and_sample_error_priority() {
        let gray = ColorSpecification::enumerated(ENUMERATED_GRAYSCALE);
        let invalid = Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ContainerCodestreamMismatch,
        });
        let empty = image(0, Vec::new());
        assert_eq!(
            convert_jp2_color(
                &empty,
                &ColorSpecification::enumerated(99),
                &ResourceLimits::default()
            ),
            invalid
        );
        let bad_sample = image(1, vec![component(&[256], 8, false, (1, 1), None)]);
        let limits = ResourceLimits::default()
            .with_maximum_decoded_bytes(1)
            .unwrap()
            .with_maximum_work_units(0)
            .unwrap();
        assert_eq!(
            convert_jp2_color(&bad_sample, &gray, &limits),
            Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: 1,
                actual: 2,
            })
        );
        let limits = ResourceLimits::default()
            .with_maximum_decoded_bytes(2)
            .unwrap()
            .with_maximum_work_units(0)
            .unwrap();
        assert_eq!(
            convert_jp2_color(&bad_sample, &gray, &limits),
            Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: 0,
                actual: 1,
            })
        );
        assert_eq!(
            convert_jp2_color(&bad_sample, &gray, &ResourceLimits::default()),
            invalid
        );
        let bad_metadata = image(1, vec![component(&[0], 0, false, (1, 1), None)]);
        assert_eq!(convert_jp2_color(&bad_metadata, &gray, &limits), invalid);
        let bad_definition = image(
            1,
            vec![component(
                &[0],
                0,
                false,
                (1, 1),
                Some(ChannelDefinition::new(
                    ChannelType::Opacity,
                    ChannelAssociation::Color(0),
                )),
            )],
        );
        assert_eq!(
            convert_jp2_color(&bad_definition, &gray, &limits),
            Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidAlphaAssociation,
            })
        );
        assert_eq!(
            convert_jp2_color(
                &bad_definition,
                &ColorSpecification::enumerated(99),
                &limits
            ),
            Err(DecodeError::UnsupportedColorConversion {
                reason: UnsupportedColorConversion::EnumeratedColorSpace
            })
        );
        let mut icc = ColorSpecification::enumerated(ENUMERATED_GRAYSCALE);
        icc.method = ColorMethod::RestrictedIcc { profile_bytes: 0 };
        assert_eq!(
            convert_jp2_color(&bad_definition, &icc, &limits),
            Err(DecodeError::UnsupportedColorConversion {
                reason: UnsupportedColorConversion::IccProfile
            })
        );
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

    type LayoutMapResult =
        Result<(Vec<usize>, Vec<(usize, bool, ChannelAssociation)>), DecodeError>;

    fn layout_map_reference(
        definitions: &[Vec<(ChannelType, ChannelAssociation)>],
        count: usize,
    ) -> LayoutMapResult {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidChannelLayout,
        };
        if definitions.iter().all(Vec::is_empty) {
            return if definitions.len() == count {
                Ok(((0..count).collect(), vec![]))
            } else {
                Err(invalid)
            };
        }
        let mut colors = std::collections::BTreeMap::new();
        let mut alphas = Vec::new();
        let flattened = definitions
            .iter()
            .enumerate()
            .flat_map(|(index, entries)| entries.iter().map(move |&entry| (index, entry)))
            .collect::<Vec<_>>();
        for (component, (kind, association)) in flattened {
            match kind {
                ChannelType::Color => {
                    let ChannelAssociation::Color(color) = association else {
                        return Err(invalid);
                    };
                    if color == 0 || usize::from(color) > count {
                        return Err(invalid);
                    }
                    if let Some(previous) = colors.insert(color, component)
                        && previous != component
                    {
                        return Err(invalid);
                    }
                }
                ChannelType::Opacity | ChannelType::PremultipliedOpacity => {
                    if matches!(association, ChannelAssociation::Color(color)
                        if color == 0 || usize::from(color) > count)
                    {
                        return Err(DecodeError::MalformedInput {
                            offset: 0,
                            kind: MalformedKind::InvalidAlphaAssociation,
                        });
                    }
                    alphas.push((
                        component,
                        kind == ChannelType::PremultipliedOpacity,
                        association,
                    ));
                }
                ChannelType::Unspecified => {}
            }
        }
        let mut ordered = Vec::new();
        for color in 1..=count {
            let index = colors.get(&u16::try_from(color).unwrap()).ok_or(invalid)?;
            ordered.push(*index);
        }
        Ok((ordered, alphas))
    }

    fn layout_test_image(definitions: &[Vec<(ChannelType, ChannelAssociation)>]) -> TypedImage {
        let components = definitions
            .iter()
            .enumerate()
            .map(|(index, entries)| {
                let mut input = TypedComponent::from_parts(
                    1,
                    1,
                    ComponentInfo {
                        precision: 8,
                        signed: false,
                        x_subsampling: 1,
                        y_subsampling: 1,
                        x_registration: 0,
                        y_registration: 0,
                    },
                    TypedSamples::Unsigned(vec![u64::try_from(index).unwrap()]),
                );
                for &(kind, association) in entries {
                    input.add_channel_definition(ChannelDefinition::new(kind, association));
                }
                input
            })
            .collect();
        TypedImage::from_components(1, 1, components, &ResourceLimits::default()).unwrap()
    }

    #[test]
    fn complete_channel_layout_matches_independent_flat_map_metadata_model() {
        let options = [
            None,
            Some((ChannelType::Color, ChannelAssociation::Color(0))),
            Some((ChannelType::Color, ChannelAssociation::Color(1))),
            Some((ChannelType::Color, ChannelAssociation::Color(2))),
            Some((ChannelType::Color, ChannelAssociation::Color(3))),
            Some((ChannelType::Color, ChannelAssociation::Color(4))),
            Some((ChannelType::Color, ChannelAssociation::WholeImage)),
            Some((ChannelType::Color, ChannelAssociation::Unassociated)),
            Some((ChannelType::Opacity, ChannelAssociation::WholeImage)),
            Some((ChannelType::Opacity, ChannelAssociation::Color(0))),
            Some((ChannelType::Opacity, ChannelAssociation::Color(1))),
            Some((ChannelType::Opacity, ChannelAssociation::Color(4))),
            Some((
                ChannelType::PremultipliedOpacity,
                ChannelAssociation::WholeImage,
            )),
            Some((
                ChannelType::PremultipliedOpacity,
                ChannelAssociation::Color(2),
            )),
            Some((ChannelType::Unspecified, ChannelAssociation::Color(0))),
        ];
        let mut cases = 0;
        for components in 1..=3_u32 {
            for mut word in 0..options.len().pow(components) {
                let mut definitions = Vec::new();
                for _ in 0..components {
                    definitions.push(
                        options[word % options.len()]
                            .into_iter()
                            .collect::<Vec<_>>(),
                    );
                    word /= options.len();
                }
                let image = layout_test_image(&definitions);
                for count in 0..=3 {
                    let actual =
                        channel_layout(&image, count).map(|layout| (layout.colors, layout.alphas));
                    assert_eq!(actual, layout_map_reference(&definitions, count));
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 14_460);
    }

    #[test]
    fn channel_layout_duplicates_and_first_metadata_error_preserve_order() {
        use ChannelAssociation::{Color, Unassociated, WholeImage};
        use ChannelType::{Color as ColorKind, Opacity, PremultipliedOpacity, Unspecified};
        let cases = [
            vec![
                vec![(ColorKind, Color(1)), (ColorKind, Color(1))],
                vec![(Opacity, WholeImage)],
            ],
            vec![
                vec![(ColorKind, Color(1)), (ColorKind, Color(2))],
                vec![(ColorKind, Color(3))],
            ],
            vec![
                vec![(ColorKind, Color(1))],
                vec![(Opacity, Color(0))],
                vec![(ColorKind, Color(1))],
            ],
            vec![
                vec![(ColorKind, Color(1))],
                vec![(ColorKind, Color(1))],
                vec![(Opacity, Color(0))],
            ],
            vec![
                vec![(Unspecified, Unassociated)],
                vec![(Opacity, WholeImage)],
            ],
            vec![vec![
                (ColorKind, Color(1)),
                (PremultipliedOpacity, Color(1)),
                (PremultipliedOpacity, Color(1)),
            ]],
        ];
        for definitions in cases {
            let image = layout_test_image(&definitions);
            for count in 0..=3 {
                assert_eq!(
                    channel_layout(&image, count).map(|layout| (layout.colors, layout.alphas)),
                    layout_map_reference(&definitions, count),
                );
            }
        }
    }

    #[test]
    fn fixed_rounding_matches_unsigned_magnitude_reference_at_native_boundaries() {
        let values = [
            i128::MIN,
            i128::MIN + 32_767,
            i128::MIN + 32_768,
            i128::MIN + 32_769,
            -98_305,
            -98_304,
            -98_303,
            -65_536,
            -32_769,
            -32_768,
            -32_767,
            -1,
            0,
            1,
            32_767,
            32_768,
            32_769,
            65_536,
            98_303,
            98_304,
            98_305,
            i128::MAX - 32_769,
            i128::MAX - 32_768,
            i128::MAX - 32_767,
            i128::MAX,
        ];
        for value in values {
            let magnitude = value.unsigned_abs() + 32_768;
            let bound = if value < 0 {
                1_u128 << 127
            } else {
                i128::MAX as u128
            };
            let expected = if magnitude > bound {
                Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::ArithmeticOverflow,
                })
            } else {
                let rounded = i128::try_from(magnitude / 65_536).unwrap();
                Ok(if value < 0 { -rounded } else { rounded })
            };
            assert_eq!(rounded_fixed(value), expected, "{value}");
        }
        assert_eq!(rounded_fixed(-32_768), Ok(-1));
        assert_eq!(rounded_fixed(32_768), Ok(1));
    }

    #[test]
    fn chroma_normalization_matches_wide_signed_ratio_for_every_supported_precision() {
        let mut cases = 0;
        for precision in 1..=38 {
            let midpoint = 1_u64 << (precision - 1);
            let maximum = 2 * midpoint - 1;
            for signed in [false, true] {
                for value in [0, midpoint - 1, midpoint, maximum] {
                    let input = if signed {
                        signed_component(
                            &[i64::try_from(value).unwrap() - i64::try_from(midpoint).unwrap()],
                            precision,
                            (1, 1),
                            None,
                        )
                    } else {
                        component(&[value], precision, false, (1, 1), None)
                    };
                    let image = image(1, vec![input]);
                    let expected = chroma_wide_reference(value, precision);
                    assert_eq!(normalized_difference_at(&image, 0, 0, 0), Ok(expected));
                    let normalized = (u128::from(value) * 65_535 + u128::from(maximum / 2))
                        / u128::from(maximum);
                    assert_eq!(
                        normalized_at(&image, 0, 0, 0),
                        Ok(u16::try_from(normalized).unwrap()),
                    );
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 304);
    }

    #[test]
    fn chroma_sampling_preserves_typed_input_and_coordinate_error_order() {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ContainerCodestreamMismatch,
        };
        for precision in [0, 39, 255] {
            let input = image(1, vec![component(&[0], precision, false, (0, 0), None)]);
            assert_eq!(
                normalized_difference_at(&input, 0, u32::MAX, u32::MAX),
                Err(invalid)
            );
        }
        for input in [
            component(&[256], 8, false, (1, 1), None),
            TypedComponent::from_parts(
                1,
                1,
                ComponentInfo {
                    precision: 8,
                    signed: true,
                    x_subsampling: 1,
                    y_subsampling: 1,
                    x_registration: 0,
                    y_registration: 0,
                },
                TypedSamples::Unsigned(vec![0]),
            ),
            signed_component(&[-129], 8, (1, 1), None),
            signed_component(&[128], 8, (1, 1), None),
        ] {
            assert_eq!(
                normalized_difference_at(&image(1, vec![input]), 0, 0, 0),
                Err(invalid)
            );
        }
        let input = TypedImage::from_components_with_origin(
            1,
            1,
            u32::MAX - 1,
            0,
            vec![component(&[0], 8, false, (1, 1), None)],
            &ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(normalized_difference_at(&input, 1, 2, 0), Err(invalid));
        assert_eq!(
            normalized_difference_at(&input, 0, 2, 0),
            Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::ArithmeticOverflow,
            }),
        );
    }

    #[test]
    fn sycc_conversion_matches_independent_wide_integer_reference() {
        let differences = [
            i32::MIN,
            i32::MIN + 1,
            -65_536,
            -65_535,
            -32_768,
            -1,
            0,
            1,
            32_767,
            32_768,
            65_534,
            65_535,
            65_536,
            i32::MAX - 1,
            i32::MAX,
        ];
        let mut cases = 0;
        for y in [0, 1, 32_767, 32_768, 65_534, 65_535_u16] {
            for cb in differences {
                for cr in differences {
                    let expected = sycc_wide_reference(y, cb, cr);
                    assert_eq!(sycc_to_srgb(y, cb, cr), Ok(expected),);
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 1_350);
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
