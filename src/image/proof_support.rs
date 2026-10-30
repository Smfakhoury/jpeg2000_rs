#[cfg(verus_keep_ghost)]
verus! {
    pub(crate) open spec fn transform_component_mut_view(component: &TransformComponentMut) -> crate::specs::Part2MctComponent {
        (component.component, (component.x, component.y, component.width, component.height,
            component.x_subsampling, component.y_subsampling), *component.fraction_bits, component.samples@)
    }

    pub(crate) open spec fn transform_components_view(components: &[TransformComponentMut]) -> Seq<crate::specs::Part2MctComponent> {
        Seq::new(components@.len(), |index: int| transform_component_mut_view(&components@[index]))
    }

    #[verifier::prophetic]
    pub(crate) open spec fn transform_component_final_samples(component: &TransformComponentMut) -> Seq<i64> {
        final(component.samples)@
    }

    #[verifier::prophetic]
    pub(crate) open spec fn transform_component_final_precision(component: &TransformComponentMut) -> u8 {
        *final(component.fraction_bits)
    }

    #[verifier::prophetic]
    pub(crate) open spec fn transform_component_borrow_frame(before: TransformComponentMut, after: TransformComponentMut) -> bool {
        transform_component_final_samples(&before) == transform_component_final_samples(&after)
            && transform_component_final_precision(&before) == transform_component_final_precision(&after)
    }

    #[verifier::prophetic]
    pub(crate) open spec fn transform_components_borrow_frame(before: Seq<TransformComponentMut>, after: Seq<TransformComponentMut>) -> bool {
        before.len() == after.len()
            && forall |index: int| 0 <= index < before.len() ==> transform_component_borrow_frame(before[index], after[index])
    }

    spec fn mct_component_pairs_outcome(result: Result<(Vec<usize>, Vec<usize>), DecodeError>)
        -> Result<(Seq<usize>, Seq<usize>), DecodeError> {
        match result { Ok((source, destination)) => Ok((source@, destination@)), Err(error) => Err(error) }
    }

    spec fn mct_wavelet_partition_outcome(result: Result<(Vec<bool>, Vec<usize>, usize), DecodeError>)
        -> Result<(Seq<bool>, Seq<usize>, usize), DecodeError> {
        match result { Ok((first, high, low)) => Ok((first@, high@, low)), Err(error) => Err(error) }
    }
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn decoded_component_view(value: DecodedComponent) -> crate::specs::Part1DecodedComponentModel {
        (value.width, value.height, value.info, value.fraction_bits, value.samples@)
    }

    pub open spec fn decoded_components_view(values: Seq<DecodedComponent>) -> Seq<crate::specs::Part1DecodedComponentModel> {
        Seq::new(values.len(), |i: int| decoded_component_view(values[i]))
    }

    pub closed spec fn decoded_image_view(value: DecodedImage) -> crate::specs::Part1DecodedImageModel {
        (value.width, value.height, value.x_origin, value.y_origin, decoded_components_view(value.components@))
    }

    impl View for DecodedImage {
        type V = crate::specs::Part1DecodedImageModel;

        open spec fn view(&self) -> Self::V {
            decoded_image_view(*self)
        }
    }

    pub closed spec fn assembly_size_view(size: &ImageSize) -> crate::specs::Part1AssemblySize {
        (size.width, size.height, size.x_origin, size.y_origin, size.components, size.component_info@)
    }

    pub open spec fn assembly_tile_view(tile: TileComponent<'_>) -> crate::specs::Part1AssemblyTile {
        (tile.component, tile.x, tile.y, tile.width, tile.height, tile.fraction_bits, tile.samples@)
    }

    pub open spec fn assembly_tiles_view(tiles: Seq<TileComponent<'_>>) -> Seq<crate::specs::Part1AssemblyTile> {
        Seq::new(tiles.len(), |i: int| assembly_tile_view(tiles[i]))
    }

    closed spec fn assembly_plane_view(plane: AssemblyPlane) -> crate::specs::Part1AssemblyPlane {
        (plane.width, plane.height, plane.fraction_bits, plane.samples@, plane.visited@)
    }

    impl View for AssemblyPlane {
        type V = crate::specs::Part1AssemblyPlane;

        closed spec fn view(&self) -> Self::V { assembly_plane_view(*self) }
    }

    pub open spec fn decoded_image_outcome(result: Result<DecodedImage, DecodeError>)
        -> Result<crate::specs::Part1DecodedImageModel, DecodeError>
    {
        match result {
            Ok(image) => Ok(decoded_image_view(image)),
            Err(error) => Err(error),
        }
    }

    pub open spec fn decoded_typed_samples_outcome(result: Result<TypedSamples, DecodeError>)
        -> Result<crate::specs::Part1TypedSampleSequence, DecodeError>
    {
        match result { Ok(samples) => Ok(typed_samples_view(&samples)), Err(error) => Err(error) }
    }

    pub open spec fn decoded_typed_components_view(components: Seq<TypedComponent>)
        -> Seq<crate::specs::Part1TypedComponentModel>
    {
        Seq::new(components.len(), |i: int| typed_component_view(&components[i]))
    }

    pub open spec fn typed_components_output_view(components: Seq<TypedComponent>)
        -> Seq<crate::specs::Part2TypedComponentOutput>
    {
        Seq::new(components.len(), |i: int| typed_component_output_view(&components[i]))
    }

    pub open spec fn decoded_typed_image_outcome(result: Result<TypedImage, DecodeError>)
        -> Result<crate::specs::Part1DecodedTypedImage, DecodeError>
    {
        match result {
            Ok(image) => Ok((typed_image_geometry(&image), typed_image_component_views(&image), typed_image_definition_views(&image))),
            Err(error) => Err(error),
        }
    }
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn typed_image_geometry(image: &TypedImage) -> (u32, u32, u32, u32) {
        (image.width, image.height, image.x_origin, image.y_origin)
    }

    pub closed spec fn typed_image_components(image: &TypedImage) -> Seq<TypedComponent> {
        image.components@
    }

    pub open spec fn typed_image_component_views(image: &TypedImage)
        -> Seq<crate::specs::Part1TypedComponentModel>
    {
        let components = typed_image_components(image);
        Seq::new(components.len(), |i: int| typed_component_view(&components[i]))
    }

    pub closed spec fn typed_component_definitions(component: &TypedComponent) -> Seq<ChannelDefinition> {
        component.channel_definitions@
    }

    pub closed spec fn channel_definition_view(definition: ChannelDefinition) -> (ChannelType, ChannelAssociation) {
        (definition.channel_type, definition.association)
    }

    pub open spec fn channel_definition_views(definitions: Seq<ChannelDefinition>)
        -> Seq<(ChannelType, ChannelAssociation)>
    {
        Seq::new(definitions.len(), |i: int| channel_definition_view(definitions[i]))
    }

    pub open spec fn typed_image_definition_views(image: &TypedImage)
        -> Seq<Seq<(ChannelType, ChannelAssociation)>>
    {
        let components = typed_image_components(image);
        Seq::new(components.len(), |i: int|
            channel_definition_views(typed_component_definitions(&components[i])))
    }

    pub open spec fn typed_samples_view(samples: &TypedSamples) -> crate::specs::Part1TypedSampleSequence {
        match samples {
            TypedSamples::Signed(values) => crate::specs::Part1TypedSampleSequence::Signed(values@),
            TypedSamples::Unsigned(values) => crate::specs::Part1TypedSampleSequence::Unsigned(values@),
        }
    }

    pub closed spec fn typed_component_view(component: &TypedComponent) -> crate::specs::Part1TypedComponentModel {
        crate::specs::Part1TypedComponentModel {
            width: component.width,
            height: component.height,
            info: component.info,
            samples: typed_samples_view(&component.samples),
        }
    }

    pub open spec fn typed_component_output_view(component: &TypedComponent)
        -> crate::specs::Part2TypedComponentOutput
    {
        (typed_component_view(component),
            channel_definition_views(typed_component_definitions(component)))
    }

    pub open spec fn typed_component_output(result: Result<TypedComponent, DecodeError>)
        -> Result<crate::specs::Part2TypedComponentOutput, DecodeError>
    {
        match result {
            Ok(component) => Ok(typed_component_output_view(&component)),
            Err(error) => Err(error),
        }
    }

    pub open spec fn typed_component_value_view(component: TypedComponent)
        -> crate::specs::Part2TypedComponentOutput
    {
        typed_component_output_view(&component)
    }

    pub open spec fn typed_component_reference_output(result: Result<&TypedComponent, DecodeError>)
        -> Result<crate::specs::Part2TypedComponentOutput, DecodeError>
    {
        match result {
            Ok(component) => Ok(typed_component_output_view(component)),
            Err(error) => Err(error),
        }
    }

    pub open spec fn typed_image_value_view(image: TypedImage) -> crate::specs::Part1DecodedTypedImage {
        (typed_image_geometry(&image), typed_image_component_views(&image), typed_image_definition_views(&image))
    }

    pub(crate) open spec fn nlt_scratch_view(output: Seq<Option<i128>>, target: Option<(u8, bool)>, work: u64)
        -> crate::specs::Part2NltScratch
    {
        crate::specs::Part2NltScratch { output, target, work }
    }

}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn mct_decorrelation_preparation_view(value: &MctDecorrelationPreparation) -> crate::specs::Part2MctDecorrelationPreparation {
        (value.source_indexes@, value.destination_indexes@, value.source_fraction_bits@,
            value.sample_count, value.rows, value.columns)
    }

    spec fn mct_decorrelation_preparation_outcome(value: Result<MctDecorrelationPreparation, DecodeError>)
        -> Result<crate::specs::Part2MctDecorrelationPreparation, DecodeError> {
        match value { Ok(prepared) => Ok(mct_decorrelation_preparation_view(&prepared)), Err(error) => Err(error) }
    }
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn mct_dependency_preparation_view(prepared: &MctDependencyPreparation)
        -> crate::specs::Part2MctDependencyPreparation {
        (prepared.source_indexes@, prepared.destination_indexes@, prepared.sample_count, prepared.count)
    }

    spec fn mct_dependency_preparation_outcome(prepared: Result<MctDependencyPreparation, DecodeError>)
        -> Result<crate::specs::Part2MctDependencyPreparation, DecodeError> {
        match prepared { Ok(value) => Ok(mct_dependency_preparation_view(&value)), Err(error) => Err(error) }
    }
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn mct_wavelet_preparation_view(value: &MctWaveletPreparation) -> crate::specs::Part2MctWaveletPreparation {
        (value.source_indexes@, value.destination_indexes@, value.low_first@, value.high_lengths@,
            value.low_length, value.sample_count, value.count, crate::codestream::mct_wavelet_view(value.wavelet),
            value.source_fraction_bits@)
    }

    spec fn mct_wavelet_preparation_outcome(result: Result<MctWaveletPreparation, DecodeError>)
        -> Result<crate::specs::Part2MctWaveletPreparation, DecodeError> {
        match result { Ok(value) => Ok(mct_wavelet_preparation_view(&value)), Err(error) => Err(error) }
    }
}
