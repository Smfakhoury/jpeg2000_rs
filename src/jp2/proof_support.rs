#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn color_specification_method(color: &ColorSpecification) -> ColorMethod {
        color.method
    }
}


#[cfg(verus_keep_ghost)]
verus! {
    pub open spec fn associated_header_view(header: &Jp2Header) -> crate::specs::Part2AssociatedHeader {
        (output_header_view(header), header.image, color_specification_method(&header.color))
    }

    pub closed spec fn codestream_header_association_view(stream: &CodestreamBox)
        -> Option<(u64, Option<crate::specs::Part2AssociatedHeader>)>
    {
        match &stream.codestream_header {
            None => None,
            Some(association) => Some((association.header.offset,
                match &association.parsed { None => None, Some(header) => Some(associated_header_view(header)) })),
        }
    }

    pub open spec fn associated_header_outcome(result: Result<&Jp2Header, DecodeError>)
        -> Result<crate::specs::Part2AssociatedHeader, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(header) => Ok(associated_header_view(header)) }
    }

    pub open spec fn associated_header_option(header: Option<&Jp2Header>) -> Option<crate::specs::Part2AssociatedHeader> {
        match header { None => None, Some(header) => Some(associated_header_view(header)) }
    }
}


#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn palette_view(palette: &Palette) -> crate::specs::Part2PaletteModel {
        (palette.entries, palette.columns@, palette.values@)
    }

    pub open spec fn output_header_view(header: &Jp2Header) -> crate::specs::Part2OutputHeader {
        (
            match &header.palette { Some(palette) => Some(palette_view(palette)), None => None },
            Seq::new(header.component_mappings@.len(), |i: int|
                (header.component_mappings@[i].component, header.component_mappings@[i].mapping_type)),
            Seq::new(header.channel_definition_records@.len(), |i: int|
                (header.channel_definition_records@[i].channel,
                    crate::image::channel_definition_view(header.channel_definition_records@[i].definition))),
        )
    }
}
