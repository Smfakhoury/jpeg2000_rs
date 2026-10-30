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

    pub open spec fn converted_image_model(image: &ConvertedImage) -> crate::specs::ColorImageModel {
        let view = converted_image_view(image);
        (view.0, view.1, view.2, view.3, converted_channel_views(view.4))
    }

    pub open spec fn converted_image_result(result: Result<ConvertedImage, DecodeError>)
        -> crate::specs::Part2ConvertedImageResult
    {
        match result {
            Err(error) => Err(error),
            Ok(image) => Ok(converted_image_model(&image)),
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
