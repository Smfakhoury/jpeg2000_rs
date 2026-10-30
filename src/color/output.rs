use super::geometry::{arithmetic_overflow, checked_area, invalid_color_input};
use super::{ConvertedChannel, ConvertedChannelType, ConvertedColorSpace, ConvertedImage};
#[cfg(verus_keep_ghost)]
use super::{
    converted_channel_view, converted_channel_views, converted_image_result, converted_image_view,
};
use crate::DecodeError;
use crate::image::ChannelAssociation;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

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
