use crate::{
    DecodeError, MalformedKind, Resource, ResourceLimits,
    codestream::{
        ComponentInfo, ImageSize, MCT_FIXED_BITS, MctArray, MctCollection, MctCollectionKind,
        MctWaveletType, MultipleComponentTransform, NonlinearPointTransform,
    },
    transform::{inverse_5_3, inverse_9_7},
};

#[cfg(test)]
use crate::codestream::Part2Extensions;

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

const FIXED_BITS: u8 = 16;

pub(crate) struct TransformComponentMut<'a> {
    pub component: u16,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub x_subsampling: u8,
    pub y_subsampling: u8,
    pub fraction_bits: &'a mut u8,
    pub samples: &'a mut [i64],
}

#[cfg(verus_keep_ghost)]
verus! {
    pub(crate) closed spec fn transform_component_mut_view(component: &TransformComponentMut) -> crate::specs::Part2MctComponent {
        (component.component, (component.x, component.y, component.width, component.height,
            component.x_subsampling, component.y_subsampling), *component.fraction_bits, component.samples@)
    }

    pub(crate) closed spec fn transform_components_view(components: &[TransformComponentMut]) -> Seq<crate::specs::Part2MctComponent> {
        Seq::new(components@.len(), |index: int| transform_component_mut_view(&components@[index]))
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, transform_component_mut_view(final(component))) == crate::specs::part2_mct_store_component_sample(
        transform_component_mut_view(old(component)), index, value,
    ),
))]
fn store_transform_component_sample(
    component: &mut TransformComponentMut<'_>,
    index: usize,
    value: i64,
) -> Result<(), DecodeError> {
    let sample = component
        .samples
        .get_mut(index)
        .ok_or_else(invalid_transform)?;
    *sample = value;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, transform_components_view(final(components))) == crate::specs::part2_mct_store_sample(
        transform_components_view(old(components)), destination, index, value,
    ),
))]
fn store_transform_sample(
    components: &mut [TransformComponentMut<'_>],
    destination: usize,
    index: usize,
    value: i64,
) -> Result<(), DecodeError> {
    let component = components
        .get_mut(destination)
        .ok_or_else(invalid_transform)?;
    store_transform_component_sample(component, index, value)
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct TileComponent<'a> {
    pub component: u16,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub fraction_bits: u8,
    pub samples: &'a [i64],
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct DecodedComponent {
    width: u32,
    height: u32,
    info: ComponentInfo,
    fraction_bits: u8,
    samples: Vec<i64>,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct DecodedImage {
    width: u32,
    height: u32,
    x_origin: u32,
    y_origin: u32,
    components: Vec<DecodedComponent>,
}

struct AssemblyPlane {
    width: u32,
    height: u32,
    fraction_bits: Option<u8>,
    samples: Vec<i64>,
    visited: Vec<bool>,
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

        closed spec fn view(&self) -> Self::V {
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

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum TypedSamples {
    Signed(Vec<i64>),
    Unsigned(Vec<u64>),
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum ChannelType {
    Color,
    Opacity,
    PremultipliedOpacity,
    Unspecified,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum ChannelAssociation {
    WholeImage,
    Color(u16),
    Unassociated,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ChannelDefinition {
    channel_type: ChannelType,
    association: ChannelAssociation,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct TypedComponent {
    width: u32,
    height: u32,
    info: ComponentInfo,
    samples: TypedSamples,
    channel_definitions: Vec<ChannelDefinition>,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct TypedImage {
    width: u32,
    height: u32,
    x_origin: u32,
    y_origin: u32,
    components: Vec<TypedComponent>,
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
}

pub(crate) struct NonlinearRegion<'a> {
    pub component: u16,
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
    pub transform: &'a NonlinearPointTransform,
}

impl DecodedImage {
    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn x_origin(&self) -> u32 {
        self.x_origin
    }

    pub const fn y_origin(&self) -> u32 {
        self.y_origin
    }

    pub fn components(&self) -> &[DecodedComponent] {
        &self.components
    }

    pub fn component(&self, index: u16) -> Result<&DecodedComponent, DecodeError> {
        self.components
            .get(usize::from(index))
            .ok_or_else(invalid_image)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures decoded_typed_image_outcome(result) == crate::specs::part1_decoded_image_typed(
            decoded_image_view(*self), crate::limits::resource_decoded_bytes_limit(limits),
        ),
    ))]
    pub fn typed(&self, limits: &ResourceLimits) -> Result<TypedImage, DecodeError> {
        let mut components = Vec::with_capacity(self.components.len());
        let mut decoded_bytes = 0_u64;
        let mut index = 0_usize;
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_decoded_typed_components_empty(
                decoded_components_view(self.components@), crate::limits::resource_decoded_bytes_limit(limits),
            );
            assert(decoded_typed_components_view(components@) =~= Seq::empty());
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.components.len(),
                components.len() == index,
                crate::specs::part1_decoded_typed_components(
                    decoded_components_view(self.components@), crate::limits::resource_decoded_bytes_limit(limits), index as nat,
                ) == Ok((decoded_typed_components_view(components@), decoded_bytes)),
                forall |i: int| 0 <= i < components.len() ==> typed_component_definitions(&components@[i]).len() == 0,
                decreases self.components.len() - index,
            )
        )]
        while index < self.components.len() {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_decoded_typed_components_step(
                    decoded_components_view(self.components@), crate::limits::resource_decoded_bytes_limit(limits), index as nat,
                );
            }
            let component = self.components.get(index).ok_or_else(invalid_image)?;
            match charge_assembly_bytes(
                component.samples.len(),
                &mut decoded_bytes,
                limits.maximum_decoded_bytes(),
            ) {
                Ok(()) => {}
                Err(error) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        crate::proofs::production_impl::production_decoded_typed_components_error(
                            decoded_components_view(self.components@), crate::limits::resource_decoded_bytes_limit(limits),
                            index as nat + 1, self.components.len() as nat, error,
                        );
                    }
                    return Err(error);
                }
            }
            let samples = match component.typed_samples(limits) {
                Ok(samples) => samples,
                Err(error) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        crate::proofs::production_impl::production_decoded_typed_components_error(
                            decoded_components_view(self.components@), crate::limits::resource_decoded_bytes_limit(limits),
                            index as nat + 1, self.components.len() as nat, error,
                        );
                    }
                    return Err(error);
                }
            };
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = components@; }
            components.push(TypedComponent {
                width: component.width,
                height: component.height,
                info: component.info,
                samples,
                channel_definitions: Vec::new(),
            });
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(decoded_typed_components_view(components@) =~= decoded_typed_components_view(before).push(
                    typed_component_view(&components@[index as int]),
                ));
                assert forall |i: int| 0 <= i < components.len() implies
                    typed_component_definitions(&components@[i]).len() == 0 by {};
            }
            index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        let image = TypedImage {
            width: self.width,
            height: self.height,
            x_origin: self.x_origin,
            y_origin: self.y_origin,
            components,
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            assert forall |i: int| 0 <= i < image.components.len() implies
                typed_image_definition_views(&image)[i] == Seq::empty() by {
                assert(typed_component_definitions(&image.components@[i]).len() == 0);
                assert(channel_definition_views(typed_component_definitions(&image.components@[i])) =~= Seq::empty());
            }
            assert(typed_image_definition_views(&image) =~= Seq::new(image.components.len() as nat, |i: int| Seq::empty()));
        }
        Ok(image)
    }
}

impl TypedImage {
    pub(crate) fn apply_nonlinear_regions(
        &mut self,
        regions: &[NonlinearRegion<'_>],
        limits: &ResourceLimits,
    ) -> Result<(), DecodeError> {
        let mut work = 0_u64;
        let mut component_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_index <= self.components.len(),
                decreases self.components.len() - component_index,
            )
        )]
        while component_index < self.components.len() {
            let component_number =
                u16::try_from(component_index).map_err(|_| arithmetic_overflow())?;
            let component = self
                .components
                .get_mut(component_index)
                .ok_or_else(invalid_image)?;
            let source_info = component.info;
            let sample_count = match &component.samples {
                TypedSamples::Signed(samples) => samples.len(),
                TypedSamples::Unsigned(samples) => samples.len(),
            };
            let mut output = vec![None; sample_count];
            let mut target = None;
            let mut region_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant region_index <= regions.len(),
                    decreases regions.len() - region_index,
                )
            )]
            while region_index < regions.len() {
                let region = regions.get(region_index).ok_or_else(invalid_image)?;
                if region.component == component_number {
                    if region.x0 >= region.x1
                        || region.y0 >= region.y1
                        || region.x1 > component.width
                        || region.y1 > component.height
                    {
                        return Err(invalid_image());
                    }
                    let descriptor = (region.transform.precision(), region.transform.signed());
                    if matches!(target, Some(existing) if existing != descriptor) {
                        return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
                    }
                    target = Some(descriptor);
                    let mut y = region.y0;
                    #[cfg_attr(
                        verus_keep_ghost,
                        verus_spec(
                            invariant y <= region.y1,
                            decreases region.y1 - y,
                        )
                    )]
                    while y < region.y1 {
                        let mut x = region.x0;
                        if x > region.x1 {
                            return Err(invalid_image());
                        }
                        #[cfg_attr(
                            verus_keep_ghost,
                            verus_spec(
                                invariant x <= region.x1,
                                decreases region.x1 - x,
                            )
                        )]
                        while x < region.x1 {
                            let index = (y as usize)
                                .checked_mul(component.width as usize)
                                .and_then(|row| row.checked_add(x as usize))
                                .ok_or_else(arithmetic_overflow)?;
                            let slot = output.get_mut(index).ok_or_else(invalid_image)?;
                            if slot.is_some() {
                                return Err(DecodeError::malformed(
                                    0,
                                    MalformedKind::InvalidTransform,
                                ));
                            }
                            let source = match &component.samples {
                                TypedSamples::Signed(samples) => {
                                    i128::from(*samples.get(index).ok_or_else(invalid_image)?)
                                }
                                TypedSamples::Unsigned(samples) => {
                                    i128::from(*samples.get(index).ok_or_else(invalid_image)?)
                                }
                            };
                            *slot = Some(Self::inverse_nlt_sample(
                                source,
                                source_info,
                                region.transform,
                            )?);
                            work = work.checked_add(1).ok_or_else(arithmetic_overflow)?;
                            if work > limits.maximum_work_units() {
                                return Err(DecodeError::limit(
                                    Resource::WorkUnits,
                                    limits.maximum_work_units(),
                                    work,
                                ));
                            }
                            x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
                        }
                        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
                    }
                }
                region_index = region_index
                    .checked_add(1)
                    .ok_or_else(arithmetic_overflow)?;
            }
            if let Some((precision, signed_output)) = target {
                validate_bytes(sample_count, 8, limits)?;
                if signed_output {
                    let mut samples = Vec::with_capacity(sample_count);
                    let mut index = 0_usize;
                    #[cfg_attr(
                        verus_keep_ghost,
                        verus_spec(
                            invariant index <= output.len(),
                            decreases output.len() - index,
                        )
                    )]
                    while index < output.len() {
                        samples.push(
                            i64::try_from(
                                output
                                    .get(index)
                                    .and_then(|value| *value)
                                    .ok_or_else(invalid_image)?,
                            )
                            .map_err(|_| invalid_image())?,
                        );
                        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
                    }
                    component.samples = TypedSamples::Signed(samples);
                } else {
                    let mut samples = Vec::with_capacity(sample_count);
                    let mut index = 0_usize;
                    #[cfg_attr(
                        verus_keep_ghost,
                        verus_spec(
                            invariant index <= output.len(),
                            decreases output.len() - index,
                        )
                    )]
                    while index < output.len() {
                        samples.push(
                            u64::try_from(
                                output
                                    .get(index)
                                    .and_then(|value| *value)
                                    .ok_or_else(invalid_image)?,
                            )
                            .map_err(|_| invalid_image())?,
                        );
                        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
                    }
                    component.samples = TypedSamples::Unsigned(samples);
                }
                component.info.precision = precision;
                component.info.signed = signed_output;
            }
            component_index = component_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn apply_nonlinear_transforms(
        &mut self,
        extensions: &Part2Extensions,
        limits: &ResourceLimits,
    ) -> Result<(), DecodeError> {
        let mut work = 0_u64;
        let mut component_index = 0_usize;
        while component_index < self.components.len() {
            let component_number =
                u16::try_from(component_index).map_err(|_| arithmetic_overflow())?;
            let Some(transform) = extensions.nonlinear_transform_for(component_number) else {
                component_index = component_index
                    .checked_add(1)
                    .ok_or_else(arithmetic_overflow)?;
                continue;
            };
            let component = self
                .components
                .get_mut(component_index)
                .ok_or_else(invalid_image)?;
            let source_info = component.info;
            if source_info.precision == 0
                || source_info.precision > 38
                || transform.precision() == 0
                || transform.precision() > 38
            {
                return Err(invalid_image());
            }
            let sample_count = match &component.samples {
                TypedSamples::Signed(samples) => samples.len(),
                TypedSamples::Unsigned(samples) => samples.len(),
            };
            work = work
                .checked_add(sample_count as u64)
                .ok_or_else(arithmetic_overflow)?;
            if work > limits.maximum_work_units() {
                return Err(DecodeError::limit(
                    Resource::WorkUnits,
                    limits.maximum_work_units(),
                    work,
                ));
            }
            validate_bytes(sample_count, 8, limits)?;
            let mut output = Vec::with_capacity(sample_count);
            let mut sample_index = 0_usize;
            while sample_index < sample_count {
                let source = match &component.samples {
                    TypedSamples::Signed(samples) => {
                        i128::from(*samples.get(sample_index).ok_or_else(invalid_image)?)
                    }
                    TypedSamples::Unsigned(samples) => {
                        i128::from(*samples.get(sample_index).ok_or_else(invalid_image)?)
                    }
                };
                output.push(Self::inverse_nlt_sample(source, source_info, transform)?);
                sample_index = sample_index
                    .checked_add(1)
                    .ok_or_else(arithmetic_overflow)?;
            }
            component.samples = if transform.signed() {
                let mut signed = Vec::with_capacity(output.len());
                let mut index = 0_usize;
                while index < output.len() {
                    signed.push(
                        i64::try_from(*output.get(index).ok_or_else(invalid_image)?)
                            .map_err(|_| invalid_image())?,
                    );
                    index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
                }
                TypedSamples::Signed(signed)
            } else {
                let mut unsigned = Vec::with_capacity(output.len());
                let mut index = 0_usize;
                while index < output.len() {
                    unsigned.push(
                        u64::try_from(*output.get(index).ok_or_else(invalid_image)?)
                            .map_err(|_| invalid_image())?,
                    );
                    index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
                }
                TypedSamples::Unsigned(unsigned)
            };
            component.info.precision = transform.precision();
            component.info.signed = transform.signed();
            component_index = component_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn from_components(
        width: u32,
        height: u32,
        components: Vec<TypedComponent>,
        limits: &ResourceLimits,
    ) -> Result<Self, DecodeError> {
        Self::from_components_with_origin(width, height, 0, 0, components, limits)
    }

    fn inverse_nlt_sample(
        source: i128,
        source_info: ComponentInfo,
        transform: &NonlinearPointTransform,
    ) -> Result<i128, DecodeError> {
        let target_precision = transform.precision();
        if target_precision == 0 || target_precision > 38 {
            return Err(invalid_image());
        }
        let target_half = Self::nlt_power_of_two(target_precision - 1)?;
        let target_min = if transform.signed() {
            0_i128
                .checked_sub(target_half)
                .ok_or_else(arithmetic_overflow)?
        } else {
            0
        };
        let target_max = if transform.signed() {
            target_half.checked_sub(1).ok_or_else(arithmetic_overflow)?
        } else {
            Self::nlt_power_of_two(target_precision)?
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?
        };
        let value = match transform.transform_type() {
            0 => {
                if source_info.precision != target_precision
                    || source_info.signed != transform.signed()
                {
                    return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
                }
                source
            }
            3 => {
                if source_info.precision != target_precision
                    || source_info.signed != transform.signed()
                {
                    return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
                }
                if source < 0 {
                    if source == i128::MIN || target_half < 0 || target_half == i128::MAX {
                        return Err(arithmetic_overflow());
                    }
                    crate::refinement::nlt_binary_complement(source, target_half)
                } else {
                    source
                }
            }
            2 | 4 => Self::inverse_nlt_lut_sample(source, source_info, transform)?,
            1 => Self::inverse_nlt_gamma_sample(source, source_info, transform)?,
            _ => {
                return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
            }
        };
        if value < target_min || value > target_max {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
        }
        Ok(value)
    }

    fn inverse_nlt_gamma_sample(
        source: i128,
        source_info: ComponentInfo,
        transform: &NonlinearPointTransform,
    ) -> Result<i128, DecodeError> {
        if source_info.precision == 0 || source_info.precision > 38 {
            return Err(invalid_image());
        }
        let parameters = transform
            .gamma_parameters()
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTransform))?;
        let parameter = |raw: u32| -> Result<i128, DecodeError> {
            i128::from(raw >> 16)
                .checked_mul(1_i128 << 32)
                .and_then(|integer| {
                    i128::from(raw & 0xffff)
                        .checked_mul(1_i128 << 32)
                        .and_then(|fraction| fraction.checked_div(i128::from(u16::MAX)))
                        .and_then(|fraction| integer.checked_add(fraction))
                })
                .ok_or_else(arithmetic_overflow)
        };
        let exponent = parameter(parameters[0])?;
        let slope = parameter(parameters[1])?;
        let threshold = parameter(parameters[2])?;
        let continuity_a = parameter(parameters[3])?;
        let continuity_b = parameter(parameters[4])?;
        if exponent <= 0 || slope <= 0 || continuity_a <= 0 {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
        }
        let source_scale = if source_info.signed {
            Self::nlt_power_of_two(source_info.precision - 1)?
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?
        } else {
            Self::nlt_power_of_two(source_info.precision)?
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?
        };
        let target_precision = transform.precision();
        if target_precision == 0 {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
        }
        let target_scale = if transform.signed() {
            Self::nlt_power_of_two(
                target_precision
                    .checked_sub(1)
                    .ok_or_else(arithmetic_overflow)?,
            )?
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?
        } else {
            Self::nlt_power_of_two(transform.precision())?
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?
        };
        if source_scale <= 0 || target_scale <= 0 {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
        }
        let normalized = source
            .checked_mul(1_i128 << 32)
            .and_then(|value| value.checked_div(source_scale))
            .ok_or_else(arithmetic_overflow)?;
        let negative_threshold = 0_i128
            .checked_sub(threshold)
            .ok_or_else(arithmetic_overflow)?;
        let decoded = if normalized < negative_threshold {
            let magnitude = 0_i128
                .checked_sub(normalized)
                .ok_or_else(arithmetic_overflow)?;
            let base = Self::fixed_div_q32(
                magnitude
                    .checked_add(continuity_b)
                    .ok_or_else(arithmetic_overflow)?,
                continuity_a,
            )?;
            0_i128
                .checked_sub(Self::fixed_pow_reciprocal_q32(base, exponent)?)
                .ok_or_else(arithmetic_overflow)?
        } else if normalized <= threshold {
            Self::fixed_div_q32(normalized, slope)?
        } else {
            let base = Self::fixed_div_q32(
                normalized
                    .checked_add(continuity_b)
                    .ok_or_else(arithmetic_overflow)?,
                continuity_a,
            )?;
            Self::fixed_pow_reciprocal_q32(base, exponent)?
        };
        Self::round_ratio(
            decoded
                .checked_mul(target_scale)
                .ok_or_else(arithmetic_overflow)?,
            1_i128 << 32,
        )
    }

    fn fixed_mul_q32(left: i128, right: i128) -> Result<i128, DecodeError> {
        Self::round_ratio(
            left.checked_mul(right).ok_or_else(arithmetic_overflow)?,
            1_i128 << 32,
        )
    }

    fn fixed_div_q32(left: i128, right: i128) -> Result<i128, DecodeError> {
        if right <= 0 {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
        }
        left.checked_mul(1_i128 << 32)
            .and_then(|value| value.checked_div(right))
            .ok_or_else(arithmetic_overflow)
    }

    fn fixed_pow_reciprocal_q32(base: i128, exponent: i128) -> Result<i128, DecodeError> {
        if base <= 0 || exponent <= 0 {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
        }
        Self::fixed_exp_q32(Self::fixed_div_q32(Self::fixed_ln_q32(base)?, exponent)?)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(requires value > 0,))]
    fn fixed_ln_q32(mut value: i128) -> Result<i128, DecodeError> {
        let one = 1_i128 << 32;
        let two = one.checked_mul(2).ok_or_else(arithmetic_overflow)?;
        let mut scale = 0_i32;
        let mut normalization_passes = 128_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases normalization_passes,)
        )]
        while value >= two {
            value >>= 1;
            scale = scale.checked_add(1).ok_or_else(arithmetic_overflow)?;
            normalization_passes = normalization_passes
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        normalization_passes = 128;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases normalization_passes,)
        )]
        while value < one {
            value = value.checked_mul(2).ok_or_else(arithmetic_overflow)?;
            scale = scale.checked_sub(1).ok_or_else(arithmetic_overflow)?;
            normalization_passes = normalization_passes
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        let z = Self::fixed_div_q32(
            value.checked_sub(one).ok_or_else(arithmetic_overflow)?,
            value.checked_add(one).ok_or_else(arithmetic_overflow)?,
        )?;
        let z_squared = Self::fixed_mul_q32(z, z)?;
        let mut term = z;
        let mut sum = 0_i128;
        let mut iteration = 0_u32;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant iteration <= 20,
                decreases 20 - iteration,
            )
        )]
        while iteration < 20 {
            let divisor = iteration
                .checked_mul(2)
                .and_then(|value| value.checked_add(1))
                .ok_or_else(arithmetic_overflow)?;
            if divisor == 0 {
                return Err(arithmetic_overflow());
            }
            sum = sum
                .checked_add(term / i128::from(divisor))
                .ok_or_else(arithmetic_overflow)?;
            term = Self::fixed_mul_q32(term, z_squared)?;
            iteration += 1;
        }
        const LN_2_Q32: i128 = 2_977_044_471;
        sum.checked_mul(2)
            .ok_or_else(arithmetic_overflow)?
            .checked_add(
                i128::from(scale)
                    .checked_mul(LN_2_Q32)
                    .ok_or_else(arithmetic_overflow)?,
            )
            .ok_or_else(arithmetic_overflow)
    }

    #[allow(clippy::manual_range_contains)]
    fn fixed_exp_q32(value: i128) -> Result<i128, DecodeError> {
        const LN_2_Q32: i128 = 2_977_044_471;
        let scale = value / LN_2_Q32;
        if scale < -62 || scale > 62 {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
        }
        let remainder = value - scale * LN_2_Q32;
        let mut sum = 1_i128 << 32;
        let mut term = sum;
        let mut iteration = 1_u32;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant iteration <= 25,
                decreases 25 - iteration,
            )
        )]
        while iteration <= 24 {
            term = Self::fixed_mul_q32(term, remainder)?
                .checked_div(i128::from(iteration))
                .ok_or_else(arithmetic_overflow)?;
            sum = sum.checked_add(term).ok_or_else(arithmetic_overflow)?;
            iteration += 1;
        }
        if scale >= 0 {
            let shift = u32::try_from(scale).map_err(|_| arithmetic_overflow())?;
            if sum < 0 || sum > (i128::MAX >> shift) {
                return Err(arithmetic_overflow());
            }
            Ok(sum << shift)
        } else {
            Ok(sum >> u32::try_from(-scale).map_err(|_| arithmetic_overflow())?)
        }
    }

    fn inverse_nlt_lut_sample(
        source: i128,
        source_info: ComponentInfo,
        transform: &NonlinearPointTransform,
    ) -> Result<i128, DecodeError> {
        let output_precision = transform.precision();
        if output_precision == 0 || output_precision > 38 {
            return Err(invalid_image());
        }
        let minimum = i128::from(
            transform
                .d_min()
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTransform))?,
        );
        let maximum = i128::from(
            transform
                .d_max()
                .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidTransform))?,
        );
        let points = transform.points();
        if points.len() < 2 || minimum >= maximum {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidTransform));
        }
        if source_info.precision == 0 || source_info.precision > 38 {
            return Err(invalid_image());
        }
        let source_scale = Self::nlt_power_of_two(source_info.precision)?;
        let source_offset = if source_info.signed {
            Self::nlt_power_of_two(source_info.precision - 1)?
        } else {
            0
        };
        let nominal = source
            .checked_add(source_offset)
            .ok_or_else(arithmetic_overflow)?;
        if nominal < 0 || nominal >= source_scale {
            return Err(invalid_image());
        }
        let normalized = nominal
            .checked_mul(i128::from(u32::MAX))
            .ok_or_else(arithmetic_overflow)?;
        let minimum_scaled = minimum
            .checked_mul(source_scale)
            .ok_or_else(arithmetic_overflow)?;
        let maximum_scaled = maximum
            .checked_mul(source_scale)
            .ok_or_else(arithmetic_overflow)?;
        let clamped = normalized.max(minimum_scaled).min(maximum_scaled);
        let interval_count = i128::try_from(points.len() - 1).map_err(|_| arithmetic_overflow())?;
        let denominator = maximum_scaled
            .checked_sub(minimum_scaled)
            .ok_or_else(arithmetic_overflow)?;
        let position_numerator = clamped
            .checked_sub(minimum_scaled)
            .and_then(|value| value.checked_mul(interval_count))
            .ok_or_else(arithmetic_overflow)?;
        let interval = position_numerator
            .checked_div(denominator)
            .ok_or_else(arithmetic_overflow)?;
        let remainder = position_numerator
            .checked_rem(denominator)
            .ok_or_else(arithmetic_overflow)?;
        let interval_index = usize::try_from(interval).map_err(|_| arithmetic_overflow())?;
        let raw = if interval_index >= points.len() - 1 {
            i128::from(*points.last().ok_or_else(invalid_image)?)
        } else {
            let left = i128::from(*points.get(interval_index).ok_or_else(invalid_image)?);
            let right = i128::from(
                *points
                    .get(
                        interval_index
                            .checked_add(1)
                            .ok_or_else(arithmetic_overflow)?,
                    )
                    .ok_or_else(invalid_image)?,
            );
            let numerator = left
                .checked_mul(denominator)
                .and_then(|value| {
                    right
                        .checked_sub(left)
                        .and_then(|difference| difference.checked_mul(remainder))
                        .and_then(|offset| value.checked_add(offset))
                })
                .ok_or_else(arithmetic_overflow)?;
            Self::round_ratio(numerator, denominator)?
        };
        let half = Self::nlt_power_of_two(output_precision - 1)?;
        let mut value = if transform.signed() {
            raw.checked_sub(half).ok_or_else(arithmetic_overflow)?
        } else {
            raw
        };
        if transform.transform_type() == 4 && value < 0 {
            let bias = half.checked_add(1).ok_or_else(arithmetic_overflow)?;
            value = 0_i128
                .checked_sub(value)
                .and_then(|magnitude| magnitude.checked_sub(bias))
                .ok_or_else(arithmetic_overflow)?;
        }
        Ok(value)
    }

    fn round_ratio(numerator: i128, denominator: i128) -> Result<i128, DecodeError> {
        if denominator <= 0 {
            return Err(arithmetic_overflow());
        }

        if numerator >= 0 {
            numerator
                .checked_add(denominator / 2)
                .map(|value| value / denominator)
                .ok_or_else(arithmetic_overflow)
        } else {
            0_i128
                .checked_sub(numerator)
                .and_then(|value| value.checked_add(denominator / 2))
                .and_then(|value| value.checked_div(denominator))
                .and_then(|value| 0_i128.checked_sub(value))
                .ok_or_else(arithmetic_overflow)
        }
    }

    fn nlt_power_of_two(exponent: u8) -> Result<i128, DecodeError> {
        if exponent > 38 {
            return Err(arithmetic_overflow());
        }
        let mut value = 1_i128;
        let mut bit = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant bit <= exponent,
                decreases exponent - bit,
            )
        )]
        while bit < exponent {
            value = value.checked_mul(2).ok_or_else(arithmetic_overflow)?;
            bit = bit.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        Ok(value)
    }

    pub(crate) fn from_components_with_origin(
        width: u32,
        height: u32,
        x_origin: u32,
        y_origin: u32,
        components: Vec<TypedComponent>,
        limits: &ResourceLimits,
    ) -> Result<Self, DecodeError> {
        typed_image_from_components_with_origin(
            width, height, x_origin, y_origin, components, limits,
        )
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures decoded_typed_image_outcome(result) == crate::specs::part2_typed_image_from_parts(
            width, height, x_origin, y_origin, typed_components_output_view(components@),
            crate::limits::resource_component_limit(limits),
            crate::limits::resource_decoded_bytes_limit(limits),
        ),
    ))]
pub(crate) fn typed_image_from_components_with_origin(
    width: u32,
    height: u32,
    x_origin: u32,
    y_origin: u32,
    components: Vec<TypedComponent>,
    limits: &ResourceLimits,
) -> Result<TypedImage, DecodeError> {
    x_origin
        .checked_add(width)
        .ok_or_else(arithmetic_overflow)?;
    y_origin
        .checked_add(height)
        .ok_or_else(arithmetic_overflow)?;
    let component_count = match u16::try_from(components.len()) {
        Ok(count) => count,
        Err(_) => return Err(arithmetic_overflow()),
    };
    if component_count > limits.maximum_components() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::Components,
            limit: u64::from(limits.maximum_components()),
            actual: components.len() as u64,
        });
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_typed_image_admission_empty(typed_components_output_view(components@), crate::limits::resource_decoded_bytes_limit(limits));
    }
    let mut decoded_bytes = 0_u64;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant index <= components.len(),
            x_origin as nat + width as nat <= u32::MAX,
            y_origin as nat + height as nat <= u32::MAX,
            components.len() <= u16::MAX,
            components.len() <= crate::limits::resource_component_limit(limits),
            crate::specs::part2_typed_image_admission_prefix(
                typed_components_output_view(components@), crate::limits::resource_decoded_bytes_limit(limits), index as nat,
            ) == Ok(decoded_bytes),
            decreases components.len() - index,
        ))]
    while index < components.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_typed_image_admission_step(typed_components_output_view(components@), crate::limits::resource_decoded_bytes_limit(limits), index as nat);
            crate::proofs::production_impl::production_typed_image_admission_error(typed_components_output_view(components@), crate::limits::resource_decoded_bytes_limit(limits), index as nat + 1, components.len() as nat);
        }
        let component = components.get(index).ok_or_else(invalid_image)?;
        let sample_count = match &component.samples {
            TypedSamples::Signed(samples) => samples.len(),
            TypedSamples::Unsigned(samples) => samples.len(),
        };
        let expected_samples = checked_area(component.width, component.height)?;
        if sample_count != expected_samples {
            return Err(invalid_image());
        }
        decoded_bytes = decoded_bytes
            .checked_add(
                (sample_count as u64)
                    .checked_mul(8)
                    .ok_or_else(arithmetic_overflow)?,
            )
            .ok_or_else(arithmetic_overflow)?;
        if decoded_bytes > limits.maximum_decoded_bytes() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: limits.maximum_decoded_bytes(),
                actual: decoded_bytes,
            });
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(Seq::new(components@.len(), |i: int| typed_components_output_view(components@)[i].0) =~= decoded_typed_components_view(components@));
        assert(Seq::new(components@.len(), |i: int| typed_components_output_view(components@)[i].1) =~= Seq::new(components@.len(), |i: int| channel_definition_views(typed_component_definitions(&components@[i]))));
    }
    Ok(TypedImage {
        width,
        height,
        x_origin,
        y_origin,
        components,
    })
}

impl TypedImage {
    #[cfg(test)]
    pub(crate) fn for_test(
        width: u32,
        height: u32,
        x_origin: u32,
        y_origin: u32,
        components: Vec<TypedComponent>,
    ) -> Self {
        Self {
            width,
            height,
            x_origin,
            y_origin,
            components,
        }
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == typed_image_geometry(self).0,)
    )]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == typed_image_geometry(self).1,)
    )]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == typed_image_geometry(self).2,)
    )]
    pub const fn x_origin(&self) -> u32 {
        self.x_origin
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == typed_image_geometry(self).3,)
    )]
    pub const fn y_origin(&self) -> u32 {
        self.y_origin
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(components => ensures components@ == typed_image_components(self),)
    )]
    pub fn components(&self) -> &[TypedComponent] {
        &self.components
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures typed_component_reference_output(result) == crate::specs::part2_typed_image_component(
            typed_components_output_view(typed_image_components(self)), index,
        ),
    ))]
    pub fn component(&self, index: u16) -> Result<&TypedComponent, DecodeError> {
        match self.components.get(usize::from(index)) {
            Some(component) => Ok(component),
            None => Err(invalid_image()),
        }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(component =>
        ensures typed_component_value_view(component) == crate::specs::part2_typed_component_from_parts(
            width, height, info, typed_samples_view(&samples),
        ),
        typed_component_output_view(&component) == crate::specs::part2_typed_component_from_parts(
            width, height, info, typed_samples_view(&samples),
        ),
        typed_component_definitions(&component) == Seq::<ChannelDefinition>::empty(),
    )
)]
pub(crate) fn typed_component_from_parts(
    width: u32,
    height: u32,
    info: ComponentInfo,
    samples: TypedSamples,
) -> TypedComponent {
    TypedComponent {
        width,
        height,
        info,
        samples,
        channel_definitions: Vec::new(),
    }
}

impl TypedComponent {
    pub(crate) fn from_parts(
        width: u32,
        height: u32,
        info: ComponentInfo,
        samples: TypedSamples,
    ) -> Self {
        typed_component_from_parts(width, height, info, samples)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            ensures typed_component_view(final(self)) == typed_component_view(old(self)),
            typed_component_definitions(final(self))
                == typed_component_definitions(old(self)).push(definition),
        )
    )]
    pub(crate) fn add_channel_definition(&mut self, definition: ChannelDefinition) {
        self.channel_definitions.push(definition);
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            ensures typed_component_view(final(self)) == typed_component_view(old(self)),
            typed_component_definitions(final(self)) == Seq::<ChannelDefinition>::empty(),
        )
    )]
    pub(crate) fn clear_channel_definitions(&mut self) {
        self.channel_definitions.clear();
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == typed_component_view(self).width,)
    )]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == typed_component_view(self).height,)
    )]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == typed_component_view(self).info,)
    )]
    pub const fn info(&self) -> ComponentInfo {
        self.info
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures typed_samples_view(value) == typed_component_view(self).samples,)
    )]
    pub const fn samples(&self) -> &TypedSamples {
        &self.samples
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(definitions => ensures definitions@ == typed_component_definitions(self),)
    )]
    pub fn channel_definitions(&self) -> &[ChannelDefinition] {
        &self.channel_definitions
    }
}

impl ChannelDefinition {
    pub const fn new(channel_type: ChannelType, association: ChannelAssociation) -> Self {
        Self {
            channel_type,
            association,
        }
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(kind => ensures kind == channel_definition_view(self).0,)
    )]
    pub const fn channel_type(self) -> ChannelType {
        self.channel_type
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(association => ensures association == channel_definition_view(self).1,)
    )]
    pub const fn association(self) -> ChannelAssociation {
        self.association
    }
}

impl DecodedComponent {
    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn info(&self) -> ComponentInfo {
        self.info
    }

    pub const fn fraction_bits(&self) -> u8 {
        self.fraction_bits
    }

    pub fn samples(&self) -> &[i64] {
        &self.samples
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures decoded_typed_samples_outcome(result) == crate::specs::part1_decoded_component_typed_samples(
            decoded_component_view(*self), crate::limits::resource_decoded_bytes_limit(limits),
        ),
    ))]
    pub fn typed_samples(&self, limits: &ResourceLimits) -> Result<TypedSamples, DecodeError> {
        if self.info.precision == 0 || self.info.precision > 38 {
            return Err(invalid_image());
        }
        validate_bytes(self.samples.len(), 8, limits)?;
        let precision = self.info.precision;
        let shift = self
            .info
            .precision
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?;
        let signed_power = 1_i64 << u32::from(shift);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(signed_power > 0) by(bit_vector)
                requires shift <= 37, signed_power == 1i64 << (shift as u32);
        }
        let signed_min = 0_i64
            .checked_sub(signed_power)
            .ok_or_else(arithmetic_overflow)?;
        let signed_max = signed_power
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?;
        let unsigned_power = 1_u64 << u32::from(precision);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(unsigned_power > 0) by(bit_vector)
                requires precision <= 38,
                    unsigned_power == 1u64 << (precision as u32);
        }
        let unsigned_max = unsigned_power
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?;
        if self.info.signed {
            let mut output = Vec::with_capacity(self.samples.len());
            let mut index = 0;
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_decoded_signed_empty(
                    self.samples@, self.fraction_bits, self.info.precision,
                );
            }
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant index <= self.samples.len(),
                    0 < self.info.precision <= 38,
                    self.info.signed,
                    signed_min == crate::specs::part1_output_minimum(self.info.precision, true),
                    signed_max == crate::specs::part1_output_maximum(self.info.precision, true),
                    crate::specs::part1_image_byte_budget(
                        self.samples.len() as nat, 8, crate::limits::resource_decoded_bytes_limit(limits),
                    ) == Ok(()),
                    crate::specs::part1_decoded_signed_prefix(
                        self.samples@, self.fraction_bits, self.info.precision, index as nat,
                    ) == Ok(output@),
                    decreases self.samples.len() - index,
                )
            )]
            while index < self.samples.len() {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_decoded_signed_step(
                        self.samples@, self.fraction_bits, self.info.precision, index as nat,
                    );
                }
                let value = match decoded_signed_sample(
                    self.samples[index],
                    self.fraction_bits,
                    signed_min,
                    signed_max,
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            crate::proofs::production_impl::production_decoded_signed_error(
                                self.samples@, self.fraction_bits, self.info.precision, index as nat + 1, self.samples.len() as nat, error,
                            );
                        }
                        return Err(error);
                    }
                };
                output.push(value);
                index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            Ok(TypedSamples::Signed(output))
        } else {
            let level_shift = 1_i64 << u32::from(shift);
            let mut output = Vec::with_capacity(self.samples.len());
            let mut index = 0;
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_decoded_unsigned_empty(
                    self.samples@, self.fraction_bits, self.info.precision,
                );
            }
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant index <= self.samples.len(),
                    0 < self.info.precision <= 38,
                    !self.info.signed,
                    level_shift == 1i64 << ((self.info.precision - 1) as u8),
                    unsigned_max == (1u64 << self.info.precision) - 1,
                    crate::specs::part1_image_byte_budget(
                        self.samples.len() as nat, 8, crate::limits::resource_decoded_bytes_limit(limits),
                    ) == Ok(()),
                    crate::specs::part1_decoded_unsigned_prefix(
                        self.samples@, self.fraction_bits, self.info.precision, index as nat,
                    ) == Ok(output@),
                    decreases self.samples.len() - index,
                )
            )]
            while index < self.samples.len() {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_decoded_unsigned_step(
                        self.samples@, self.fraction_bits, self.info.precision, index as nat,
                    );
                }
                let value = match decoded_unsigned_sample(
                    self.samples[index],
                    self.fraction_bits,
                    level_shift,
                    unsigned_max,
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            crate::proofs::production_impl::production_decoded_unsigned_error(
                                self.samples@, self.fraction_bits, self.info.precision, index as nat + 1, self.samples.len() as nat, error,
                            );
                        }
                        return Err(error);
                    }
                };
                output.push(value);
                index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            Ok(TypedSamples::Unsigned(output))
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures decoded_image_outcome(result) == crate::specs::part1_assemble_image(
        assembly_size_view(size), assembly_tiles_view(tiles@), crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
pub fn assemble_image(
    size: &ImageSize,
    tiles: &[TileComponent<'_>],
    limits: &ResourceLimits,
) -> Result<DecodedImage, DecodeError> {
    let components = Vec::with_capacity(usize::from(size.components));
    let image_x1 = size
        .x_origin
        .checked_add(size.width)
        .ok_or_else(arithmetic_overflow)?;
    let image_y1 = size
        .y_origin
        .checked_add(size.height)
        .ok_or_else(arithmetic_overflow)?;
    let mut image = DecodedImage {
        width: size.width,
        height: size.height,
        x_origin: size.x_origin,
        y_origin: size.y_origin,
        components,
    };
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original = image@; }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_assembly_components_empty(
            original, 0, assembly_size_view(size), assembly_tiles_view(tiles@), (image_x1, image_y1),
            None, crate::limits::resource_decoded_bytes_limit(limits),
        );
    }
    let mut decoded_bytes = 0_u64;
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= size.components,
            image_x1 as int == size.x_origin as int + size.width as int,
            image_y1 as int == size.y_origin as int + size.height as int,
            original == (size.width, size.height, size.x_origin, size.y_origin, Seq::empty()),
            crate::specs::part1_assembly_components_prefix(
                original, 0, assembly_size_view(size), assembly_tiles_view(tiles@), (image_x1, image_y1),
                None, crate::limits::resource_decoded_bytes_limit(limits), component as nat,
            ) == (Ok(()), image@, decoded_bytes),
            decreases size.components - component,
        )
    )]
    while component < size.components {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_assembly_components_step(
                original, 0, assembly_size_view(size), assembly_tiles_view(tiles@), (image_x1, image_y1),
                None, crate::limits::resource_decoded_bytes_limit(limits), component as nat,
            );
        }
        match append_assembly_component(
            &mut image,
            &mut decoded_bytes,
            size,
            tiles,
            component,
            ([image_x1, image_y1], None),
            limits.maximum_decoded_bytes(),
        ) {
            Ok(()) => {}
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_assembly_components_error_persists(
                        original, 0, assembly_size_view(size), assembly_tiles_view(tiles@), (image_x1, image_y1),
                        None, crate::limits::resource_decoded_bytes_limit(limits), component as nat + 1, size.components as nat,
                    );
                }
                return Err(error);
            }
        }
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(image)
}

type AssemblyDimensions = (ComponentInfo, u32, u32, u32, u32, u32, u32);
type AssemblySelection = Option<([u32; 4], u32)>;

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_assembly_component_dimensions(
        assembly_size_view(size), component, (extent[0], extent[1]), selection,
    ),
))]
fn assembly_component_dimensions(
    size: &ImageSize,
    component: u16,
    extent: [u32; 2],
    selection: AssemblySelection,
) -> Result<AssemblyDimensions, DecodeError> {
    let info = crate::codestream::image_component_info(size, component)?;
    match selection {
        None => {
            let x1 = registered_ceil_div(
                extent[0],
                u32::from(info.x_subsampling),
                info.x_registration,
            )?;
            let x0 = registered_ceil_div(
                size.x_origin,
                u32::from(info.x_subsampling),
                info.x_registration,
            )?;
            let width = x1.checked_sub(x0).ok_or_else(arithmetic_overflow)?;
            let y1 = registered_ceil_div(
                extent[1],
                u32::from(info.y_subsampling),
                info.y_registration,
            )?;
            let y0 = registered_ceil_div(
                size.y_origin,
                u32::from(info.y_subsampling),
                info.y_registration,
            )?;
            let height = y1.checked_sub(y0).ok_or_else(arithmetic_overflow)?;
            Ok((
                info,
                width,
                height,
                u32::from(info.x_subsampling),
                u32::from(info.y_subsampling),
                x0,
                y0,
            ))
        }
        Some((region, scale)) => {
            let x_divisor = u32::from(info.x_subsampling)
                .checked_mul(scale)
                .ok_or_else(arithmetic_overflow)?;
            let y_divisor = u32::from(info.y_subsampling)
                .checked_mul(scale)
                .ok_or_else(arithmetic_overflow)?;
            let x0 = registered_ceil_div(region[0], x_divisor, info.x_registration)?;
            let y0 = registered_ceil_div(region[1], y_divisor, info.y_registration)?;
            let width = registered_ceil_div(region[2], x_divisor, info.x_registration)?
                .checked_sub(x0)
                .ok_or_else(arithmetic_overflow)?;
            let height = registered_ceil_div(region[3], y_divisor, info.y_registration)?
                .checked_sub(y0)
                .ok_or_else(arithmetic_overflow)?;
            if width == 0 || height == 0 {
                return Err(DecodeError::InvalidSelection {
                    reason: crate::SelectionError::EmptyComponentIntersection,
                });
            }
            Ok((info, width, height, x_divisor, y_divisor, x0, y0))
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_assembly_component_crop(assembly_size_view(size), dimensions, selected),
))]
fn assembly_component_crop(
    size: &ImageSize,
    dimensions: AssemblyDimensions,
    selected: bool,
) -> Result<Option<(u32, u32, u32, u32)>, DecodeError> {
    if !selected {
        return Ok(None);
    }
    let x0 = dimensions
        .5
        .checked_sub(registered_ceil_div(
            size.x_origin,
            dimensions.3,
            dimensions.0.x_registration,
        )?)
        .ok_or_else(arithmetic_overflow)?;
    let y0 = dimensions
        .6
        .checked_sub(registered_ceil_div(
            size.y_origin,
            dimensions.4,
            dimensions.0.y_registration,
        )?)
        .ok_or_else(arithmetic_overflow)?;
    let x1 = x0
        .checked_add(dimensions.1)
        .ok_or_else(arithmetic_overflow)?;
    let y1 = y0
        .checked_add(dimensions.2)
        .ok_or_else(arithmetic_overflow)?;
    Ok(Some((x0, y0, x1, y1)))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, *final(bytes)) == crate::specs::part1_accumulate_decoded_bytes(
        count as nat, *old(bytes), maximum,
    ),
))]
fn charge_assembly_bytes(count: usize, bytes: &mut u64, maximum: u64) -> Result<(), DecodeError> {
    let component_bytes = (count as u64)
        .checked_mul(8)
        .ok_or_else(arithmetic_overflow)?;
    *bytes = bytes
        .checked_add(component_bytes)
        .ok_or_else(arithmetic_overflow)?;
    if *bytes > maximum {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::DecodedBytes,
            limit: maximum,
            actual: *bytes,
        });
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(image)@, *final(bytes)) == crate::specs::part1_assembly_append_plane(
        old(image)@, *old(bytes), assembly_size_view(size), assembly_tiles_view(tiles@), component,
        (profile.0[0], profile.0[1]), profile.1, maximum,
    ),
))]
fn append_assembly_component(
    image: &mut DecodedImage,
    bytes: &mut u64,
    size: &ImageSize,
    tiles: &[TileComponent<'_>],
    component: u16,
    profile: ([u32; 2], AssemblySelection),
    maximum: u64,
) -> Result<(), DecodeError> {
    let (extent, selection) = profile;
    let dimensions = assembly_component_dimensions(size, component, extent, selection)?;
    let count = checked_area(dimensions.1, dimensions.2)?;
    charge_assembly_bytes(count, bytes, maximum)?;
    let crop = assembly_component_crop(size, dimensions, selection.is_some())?;
    let mut plane = AssemblyPlane {
        width: dimensions.1,
        height: dimensions.2,
        fraction_bits: None,
        samples: vec![0_i64; count],
        visited: vec![false; count],
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(plane.samples@ =~= Seq::new(count as nat, |i: int| 0i64));
        assert(plane.visited@ =~= Seq::new(count as nat, |i: int| false));
    }
    collect_assembly_tiles(&mut plane, tiles, component, crop)?;
    image.components.push(DecodedComponent {
        width: dimensions.1,
        height: dimensions.2,
        info: dimensions.0,
        fraction_bits: plane.fraction_bits.ok_or_else(invalid_image)?,
        samples: plane.samples,
    });
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(decoded_components_view(image.components@) == decoded_components_view(old(image).components@).push(
            decoded_component_view(image.components@[image.components@.len() - 1]),
        )) by {
            assert forall |i: int| 0 <= i < image.components@.len() implies
                decoded_components_view(image.components@)[i] == decoded_components_view(old(image).components@).push(
                    decoded_component_view(image.components@[image.components@.len() - 1]),
                )[i] by {};
        };
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures decoded_image_outcome(result) == crate::specs::part1_assemble_selected_image(
        assembly_size_view(size), assembly_tiles_view(tiles@), region, discard_levels,
        crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
pub(crate) fn assemble_selected_image(
    size: &ImageSize,
    tiles: &[TileComponent<'_>],
    region: [u32; 4],
    discard_levels: u8,
    limits: &ResourceLimits,
) -> Result<DecodedImage, DecodeError> {
    let header = assembly_selected_header(region, discard_levels)?;
    let components = Vec::with_capacity(usize::from(size.components));
    let mut image = DecodedImage {
        width: header.3,
        height: header.4,
        x_origin: header.1,
        y_origin: header.2,
        components,
    };
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original = image@; }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_assembly_components_empty(
            original, 0, assembly_size_view(size), assembly_tiles_view(tiles@), (0, 0),
            Some((region, header.0)), crate::limits::resource_decoded_bytes_limit(limits),
        );
    }
    let mut decoded_bytes = 0_u64;
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= size.components,
            crate::specs::part1_assembly_selected_header(region, discard_levels) == Ok(header),
            original == (header.3, header.4, header.1, header.2, Seq::empty()),
            crate::specs::part1_assembly_components_prefix(
                original, 0, assembly_size_view(size), assembly_tiles_view(tiles@), (0, 0),
                Some((region, header.0)), crate::limits::resource_decoded_bytes_limit(limits), component as nat,
            ) == (Ok(()), image@, decoded_bytes),
            decreases size.components - component,
        )
    )]
    while component < size.components {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_assembly_components_step(
                original, 0, assembly_size_view(size), assembly_tiles_view(tiles@), (0, 0),
                Some((region, header.0)), crate::limits::resource_decoded_bytes_limit(limits), component as nat,
            );
        }
        match append_assembly_component(
            &mut image,
            &mut decoded_bytes,
            size,
            tiles,
            component,
            ([0, 0], Some((region, header.0))),
            limits.maximum_decoded_bytes(),
        ) {
            Ok(()) => {}
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_assembly_components_error_persists(
                        original, 0, assembly_size_view(size), assembly_tiles_view(tiles@), (0, 0),
                        Some((region, header.0)), crate::limits::resource_decoded_bytes_limit(limits),
                        component as nat + 1, size.components as nat,
                    );
                }
                return Err(error);
            }
        }
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(image)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_assembly_selected_header(region, discard_levels),
))]
fn assembly_selected_header(
    region: [u32; 4],
    discard_levels: u8,
) -> Result<(u32, u32, u32, u32, u32), DecodeError> {
    if region[0] >= region[2] || region[1] >= region[3] || discard_levels > 31 {
        return Err(invalid_image());
    }
    let scale = 1_u32 << u32::from(discard_levels);
    let x0 = ceil_div(region[0], scale)?;
    let y0 = ceil_div(region[1], scale)?;
    let width = ceil_div(region[2], scale)?
        .checked_sub(x0)
        .ok_or_else(arithmetic_overflow)?;
    let height = ceil_div(region[3], scale)?
        .checked_sub(y0)
        .ok_or_else(arithmetic_overflow)?;
    if width == 0 || height == 0 {
        return Err(invalid_image());
    }
    Ok((scale, x0, y0, width, height))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(image)@) == crate::specs::part1_apply_component_transform(old(image)@, false),
))]
pub fn apply_reversible_component_transform(image: &mut DecodedImage) -> Result<(), DecodeError> {
    apply_component_transform(image, false)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(image)@) == crate::specs::part1_apply_component_transform(old(image)@, true),
))]
pub fn apply_irreversible_component_transform(image: &mut DecodedImage) -> Result<(), DecodeError> {
    apply_component_transform(image, true)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(image)@) == crate::specs::part1_apply_component_transform(old(image)@, irreversible),
))]
fn apply_component_transform(
    image: &mut DecodedImage,
    irreversible: bool,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original = image@; }
    validate_transform_components(image, if irreversible { FIXED_BITS } else { 0 })?;
    if image.components.len() < 3 {
        return Err(invalid_image());
    }
    let length = image.components[0].samples.len();
    if image.components[1].samples.len() != length || image.components[2].samples.len() != length {
        return Err(invalid_image());
    }
    let mut index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= length,
            original == old(image)@,
            crate::specs::part1_validate_transform_components(original, if irreversible { 16u8 } else { 0u8 }) == Ok(()),
            original.4.len() >= 3,
            original.4[1].4.len() == length,
            original.4[2].4.len() == length,
            length == original.4[0].4.len(),
            crate::specs::part1_component_transform_prefix(original, irreversible, index as nat) == (Ok(()), image@),
            decreases length - index,
        )
    )]
    while index < length {
        match apply_component_pixel(image, index, irreversible) {
            Ok(()) => {}
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_component_transform_error_persists(
                        original, irreversible, index as nat + 1, length as nat,
                    );
                }
                return Err(error);
            }
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(image)@) == crate::specs::part1_component_pixel_state(old(image)@, index, irreversible),
))]
fn apply_component_pixel(
    image: &mut DecodedImage,
    index: usize,
    irreversible: bool,
) -> Result<(), DecodeError> {
    let y = component_sample(image, 0, index)?;
    let cb = component_sample(image, 1, index)?;
    let cr = component_sample(image, 2, index)?;
    let (red, green, blue) = inverse_component_values(y, cb, cr, irreversible)?;
    set_component_sample(image, 0, index, red)?;
    set_component_sample(image, 1, index, green)?;
    set_component_sample(image, 2, index, blue)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_component_values(y, cb, cr, irreversible),
))]
fn inverse_component_values(
    y: i64,
    cb: i64,
    cr: i64,
    irreversible: bool,
) -> Result<(i64, i64, i64), DecodeError> {
    if irreversible {
        let red = y
            .checked_add(fixed_mul(cr, 91_881)?)
            .ok_or_else(arithmetic_overflow)?;
        let green_first = y
            .checked_sub(fixed_mul(cb, 22_553)?)
            .ok_or_else(arithmetic_overflow)?;
        let green = green_first
            .checked_sub(fixed_mul(cr, 46_797)?)
            .ok_or_else(arithmetic_overflow)?;
        let blue = y
            .checked_add(fixed_mul(cb, 116_130)?)
            .ok_or_else(arithmetic_overflow)?;
        Ok((red, green, blue))
    } else {
        let chroma_sum = cb.checked_add(cr).ok_or_else(arithmetic_overflow)?;
        y.checked_sub(chroma_sum >> 2)
            .ok_or_else(arithmetic_overflow)?;
        let green = crate::refinement::rct_green(y, chroma_sum);
        green.checked_add(cr).ok_or_else(arithmetic_overflow)?;
        green.checked_add(cb).ok_or_else(arithmetic_overflow)?;
        let red = crate::refinement::rct_red(green, cr);
        let blue = crate::refinement::rct_blue(green, cb);
        Ok((red, green, blue))
    }
}

pub fn apply_multiple_component_transform(
    image: &mut DecodedImage,
    transform: &MultipleComponentTransform,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let mut components = Vec::with_capacity(image.components.len());
    collect_decoded_transform_components(&mut image.components, 0, &mut components)?;
    let mut work = 0_u64;
    apply_multiple_component_transform_components(&mut components, transform, limits, &mut work)
}

#[cfg_attr(verus_keep_ghost, verus_spec(decreases decoded.len(),))]
fn collect_decoded_transform_components<'a>(
    decoded: &'a mut [DecodedComponent],
    component_index: u16,
    components: &mut Vec<TransformComponentMut<'a>>,
) -> Result<(), DecodeError> {
    let Some((component, remaining)) = decoded.split_first_mut() else {
        return Ok(());
    };
    let info = component.info;
    components.push(TransformComponentMut {
        component: component_index,
        x: 0,
        y: 0,
        width: component.width,
        height: component.height,
        x_subsampling: info.x_subsampling,
        y_subsampling: info.y_subsampling,
        fraction_bits: &mut component.fraction_bits,
        samples: &mut component.samples,
    });
    collect_decoded_transform_components(
        remaining,
        component_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?,
        components,
    )
}

pub(crate) fn apply_multiple_component_transform_components(
    components: &mut [TransformComponentMut<'_>],
    transform: &MultipleComponentTransform,
    limits: &ResourceLimits,
    work: &mut u64,
) -> Result<(), DecodeError> {
    let mut stage_index = transform.stages().len();
    #[cfg_attr(verus_keep_ghost, verus_spec(decreases stage_index,))]
    while stage_index != 0 {
        stage_index = crate::refinement::mct_reverse_stage(stage_index);
        let stage = transform
            .stages()
            .get(stage_index)
            .ok_or_else(invalid_transform)?;
        let collection_count = stage.collections().len();
        let mut collection_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant collection_index <= collection_count,
                decreases collection_count - collection_index,
            )
        )]
        while collection_index < collection_count {
            apply_mct_collection(
                components,
                stage
                    .collections()
                    .get(collection_index)
                    .ok_or_else(invalid_transform)?,
                limits,
                work,
            )?;
            collection_index = collection_index
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
    }
    Ok(())
}

fn apply_mct_collection(
    components: &mut [TransformComponentMut<'_>],
    collection: &MctCollection,
    limits: &ResourceLimits,
    work: &mut u64,
) -> Result<(), DecodeError> {
    match collection.kind() {
        MctCollectionKind::Dependency => apply_mct_dependency(components, collection, limits, work),
        MctCollectionKind::Decorrelation => {
            apply_mct_decorrelation(components, collection, limits, work)
        }
        MctCollectionKind::Wavelet => apply_mct_wavelet(components, collection, limits, work),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_mct_validate_layout(
        transform_components_view(components), source_indexes@, destination_indexes@, reversible,
    ),
))]
fn validate_mct_components_layout(
    components: &[TransformComponentMut<'_>],
    source_indexes: &[usize],
    destination_indexes: &[usize],
    reversible: bool,
) -> Result<usize, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { crate::codestream::mct_fixed_precision_is_q32(); }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost before = transform_components_view(components); }
    let first_index = *source_indexes.first().ok_or_else(invalid_transform)?;
    let first = components.get(first_index).ok_or_else(invalid_transform)?;
    let sample_count = first.samples.len();
    let width = first.width;
    let height = first.height;
    let x = first.x;
    let y = first.y;
    let x_subsampling = first.x_subsampling;
    let y_subsampling = first.y_subsampling;
    let mut list = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant list <= 2,
            MCT_FIXED_BITS == 32,
            before == transform_components_view(components),
            source_indexes@.len() > 0,
            source_indexes@[0] == first_index,
            first_index < before.len(),
            before[first_index as int].1 == (x, y, width, height, x_subsampling, y_subsampling),
            before[first_index as int].3.len() == sample_count,
            list > 0 ==> forall |position: int| 0 <= position < source_indexes@.len()
                ==> crate::specs::part2_mct_layout_index_valid(before, first_index, source_indexes@[position]),
            list > 1 ==> forall |position: int| 0 <= position < destination_indexes@.len()
                ==> crate::specs::part2_mct_layout_index_valid(before, first_index, destination_indexes@[position]),
            decreases 2 - list,
        )
    )]
    while list < 2 {
        let indexes = if list == 0 {
            source_indexes
        } else {
            destination_indexes
        };
        let mut index_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index_index <= indexes.len(),
                list < 2,
                MCT_FIXED_BITS == 32,
                before == transform_components_view(components),
                source_indexes@.len() > 0,
                source_indexes@[0] == first_index,
                first_index < before.len(),
                before[first_index as int].1 == (x, y, width, height, x_subsampling, y_subsampling),
                before[first_index as int].3.len() == sample_count,
                indexes@ == if list == 0 { source_indexes@ } else { destination_indexes@ },
                list > 0 ==> forall |position: int| 0 <= position < source_indexes@.len()
                    ==> crate::specs::part2_mct_layout_index_valid(before, first_index, source_indexes@[position]),
                forall |position: int| 0 <= position < index_index
                    ==> crate::specs::part2_mct_layout_index_valid(before, first_index, indexes@[position]),
                decreases indexes.len() - index_index,
            )
        )]
        while index_index < indexes.len() {
            let index = *indexes.get(index_index).ok_or_else(invalid_transform)?;
            let component = components.get(index).ok_or_else(invalid_transform)?;
            if component.width != width
                || component.height != height
                || component.x != x
                || component.y != y
                || component.x_subsampling != x_subsampling
                || component.y_subsampling != y_subsampling
                || component.samples.len() != sample_count
                || *component.fraction_bits > MCT_FIXED_BITS
            {
                return Err(invalid_transform());
            }
            index_index = index_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        list = list.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    if reversible {
        let mut index_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index_index <= source_indexes.len(),
                reversible,
                before == transform_components_view(components),
                source_indexes@.len() > 0,
                source_indexes@[0] == first_index,
                first_index < before.len(),
                before[first_index as int].3.len() == sample_count,
                forall |position: int| 0 <= position < source_indexes@.len()
                    ==> crate::specs::part2_mct_layout_index_valid(before, first_index, source_indexes@[position]),
                forall |position: int| 0 <= position < destination_indexes@.len()
                    ==> crate::specs::part2_mct_layout_index_valid(before, first_index, destination_indexes@[position]),
                forall |position: int| 0 <= position < index_index
                    ==> before[source_indexes@[position] as int].2 == 0,
                decreases source_indexes.len() - index_index,
            )
        )]
        while index_index < source_indexes.len() {
            let index = *source_indexes
                .get(index_index)
                .ok_or_else(invalid_transform)?;
            if *components
                .get(index)
                .ok_or_else(invalid_transform)?
                .fraction_bits
                != 0
            {
                return Err(invalid_transform());
            }
            index_index = index_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
    }
    Ok(sample_count)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, *final(work)) == crate::specs::part2_mct_decorrelation_admission(
        sample_count, rows, columns, *old(work),
        crate::limits::resource_work_limit(limits), crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
fn admit_mct_decorrelation(
    sample_count: usize,
    rows: usize,
    columns: usize,
    limits: &ResourceLimits,
    work: &mut u64,
) -> Result<(), DecodeError> {
    let sample_rows = (sample_count as u64)
        .checked_mul(rows as u64)
        .ok_or_else(arithmetic_overflow)?;
    let products = sample_rows
        .checked_mul(columns as u64)
        .ok_or_else(arithmetic_overflow)?;
    let repeated_sample_rows = (sample_count as u64)
        .checked_mul(rows as u64)
        .ok_or_else(arithmetic_overflow)?;
    let operations = products
        .checked_add(repeated_sample_rows)
        .ok_or_else(arithmetic_overflow)?;
    charge_mct_work(work, operations, limits)?;
    let values = rows.checked_add(columns).ok_or_else(arithmetic_overflow)?;
    let scratch_bytes = values
        .checked_mul(core::mem::size_of::<i64>())
        .ok_or_else(arithmetic_overflow)? as u64;
    if scratch_bytes > limits.maximum_decoded_bytes() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::DecodedBytes,
            limit: limits.maximum_decoded_bytes(),
            actual: scratch_bytes,
        });
    }
    Ok(())
}

struct MctDecorrelationPreparation {
    source_indexes: Vec<usize>,
    destination_indexes: Vec<usize>,
    source_fraction_bits: Vec<u8>,
    sample_count: usize,
    rows: usize,
    columns: usize,
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (mct_decorrelation_preparation_outcome(result), *final(work)) == crate::specs::part2_mct_prepare_decorrelation(
        transform_components_view(components), crate::codestream::mct_collection_view(collection), *old(work),
        crate::limits::resource_work_limit(limits), crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
fn prepare_mct_decorrelation(
    components: &[TransformComponentMut<'_>],
    collection: &MctCollection,
    limits: &ResourceLimits,
    work: &mut u64,
) -> Result<MctDecorrelationPreparation, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost before = transform_components_view(components);
        let ghost style = crate::codestream::mct_collection_view(collection);
        let ghost initial_work = *work;
    }
    let rows = collection.input_components().len();
    let columns = collection.output_components().len();
    let matrix = collection.matrix();
    let invalid_offset_dimensions = match collection.offsets() {
        Some(offsets) => offsets.values().len() != rows,
        None => false,
    };
    if rows == 0
        || rows != columns
        || matches!(
            matrix,
            Some(matrix)
                if matrix.values().len()
                    != rows.checked_mul(columns).ok_or_else(arithmetic_overflow)?
        )
        || invalid_offset_dimensions
    {
        return Err(invalid_transform());
    }
    let mut source_indexes = Vec::with_capacity(columns);
    let mut destination_indexes = Vec::with_capacity(rows);
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_mct_indices_empty(before);
        assert(style.2.take(0) =~= Seq::empty());
        assert(source_indexes@ == Seq::empty());
    }
    let mut column = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant column <= columns,
            before == transform_components_view(components),
            style == crate::codestream::mct_collection_view(collection),
            rows == style.1.len(), columns == style.2.len(),
            crate::specs::part2_mct_decorrelation_dimensions(style) == Ok((rows, columns)),
            *work == initial_work,
            source_indexes@.len() == column,
            destination_indexes@.len() == 0,
            crate::specs::part2_mct_component_indices(before, style.2.take(column as int)) == Ok(source_indexes@),
            decreases columns - column,
        )
    )]
    while column < columns {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_mct_indices_step(before, style.2, column as nat);
            crate::proofs::production_impl::production_mct_indices_error(before, style.2, column as nat + 1, columns as nat);
            assert(style.2.take(columns as int) =~= style.2);
        }
        source_indexes.push(find_transform_component(
            components,
            *collection
                .output_components()
                .get(column)
                .ok_or_else(invalid_transform)?,
        )?);
        column = column.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(style.2.take(columns as int) =~= style.2);
        crate::proofs::production_impl::production_mct_indices_empty(before);
        assert(style.1.take(0) =~= Seq::empty());
        assert(destination_indexes@ == Seq::empty());
    }
    let mut row = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant row <= rows,
            before == transform_components_view(components),
            style == crate::codestream::mct_collection_view(collection),
            rows == style.1.len(), columns == style.2.len(),
            crate::specs::part2_mct_decorrelation_dimensions(style) == Ok((rows, columns)),
            *work == initial_work,
            source_indexes@.len() == columns,
            destination_indexes@.len() == row,
            crate::specs::part2_mct_component_indices(before, style.2) == Ok(source_indexes@),
            crate::specs::part2_mct_component_indices(before, style.1.take(row as int)) == Ok(destination_indexes@),
            decreases rows - row,
        )
    )]
    while row < rows {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_mct_indices_step(before, style.1, row as nat);
            crate::proofs::production_impl::production_mct_indices_error(before, style.1, row as nat + 1, rows as nat);
            assert(style.1.take(rows as int) =~= style.1);
        }
        destination_indexes.push(find_transform_component(
            components,
            *collection
                .input_components()
                .get(row)
                .ok_or_else(invalid_transform)?,
        )?);
        row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { assert(style.1.take(rows as int) =~= style.1); }
    let sample_count = validate_mct_components_layout(
        components,
        &source_indexes,
        &destination_indexes,
        collection.reversible(),
    )?;
    admit_mct_decorrelation(sample_count, rows, columns, limits, work)?;
    let mut source_fraction_bits = Vec::with_capacity(source_indexes.len());
    let mut index_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index_index <= source_indexes.len(),
            before == transform_components_view(components),
            style == crate::codestream::mct_collection_view(collection),
            rows == style.1.len(), columns == style.2.len(),
            crate::specs::part2_mct_decorrelation_dimensions(style) == Ok((rows, columns)),
            crate::specs::part2_mct_component_indices(before, style.2) == Ok(source_indexes@),
            crate::specs::part2_mct_component_indices(before, style.1) == Ok(destination_indexes@),
            crate::specs::part2_mct_validate_layout(before, source_indexes@, destination_indexes@, style.0) == Ok(sample_count),
            crate::specs::part2_mct_decorrelation_admission(sample_count, rows, columns, initial_work,
                crate::limits::resource_work_limit(limits), crate::limits::resource_decoded_bytes_limit(limits)) == (Ok(()), *work),
            forall |position: int| 0 <= position < source_indexes@.len()
                ==> source_indexes@[position] < before.len(),
            source_fraction_bits@ == Seq::new(index_index as nat, |position: int| before[source_indexes@[position] as int].2),
            decreases source_indexes.len() - index_index,
        )
    )]
    while index_index < source_indexes.len() {
        let index = *source_indexes
            .get(index_index)
            .ok_or_else(invalid_transform)?;
        source_fraction_bits.push(
            *components
                .get(index)
                .ok_or_else(invalid_transform)?
                .fraction_bits,
        );
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(source_fraction_bits@ =~= Seq::new(index_index as nat + 1,
                |position: int| before[source_indexes@[position] as int].2));
        }
        index_index = index_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(MctDecorrelationPreparation {
        source_indexes,
        destination_indexes,
        source_fraction_bits,
        sample_count,
        rows,
        columns,
    })
}

fn apply_mct_decorrelation(
    components: &mut [TransformComponentMut<'_>],
    collection: &MctCollection,
    limits: &ResourceLimits,
    work: &mut u64,
) -> Result<(), DecodeError> {
    let prepared = prepare_mct_decorrelation(components, collection, limits, work)?;
    let MctDecorrelationPreparation {
        source_indexes,
        destination_indexes,
        source_fraction_bits,
        sample_count,
        rows,
        columns,
    } = prepared;
    let matrix = collection.matrix();
    let mut row;
    let mut index_index;
    let mut source = Vec::with_capacity(columns);
    let mut output = Vec::with_capacity(rows);
    let mut sample_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant sample_index <= sample_count,
            decreases sample_count - sample_index,
        )
    )]
    while sample_index < sample_count {
        source.clear();
        output.clear();
        index_index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index_index <= source_indexes.len(),
                decreases source_indexes.len() - index_index,
            )
        )]
        while index_index < source_indexes.len() {
            let index = *source_indexes
                .get(index_index)
                .ok_or_else(invalid_transform)?;
            source.push(
                *components
                    .get(index)
                    .and_then(|component| component.samples.get(sample_index))
                    .ok_or_else(invalid_transform)?,
            );
            index_index = index_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        row = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant row <= rows,
                decreases rows - row,
            )
        )]
        while row < rows {
            output.push(if collection.reversible() {
                if let Some(matrix) = matrix {
                    reversible_mct_row(matrix, collection.offsets(), &source, row)?
                } else {
                    (*source.get(row).ok_or_else(invalid_transform)?)
                        .checked_add(offset_value(collection.offsets(), row)?)
                        .ok_or_else(arithmetic_overflow)?
                }
            } else if let Some(matrix) = matrix {
                fixed_mct_row_scaled(
                    matrix,
                    collection.offsets(),
                    &source,
                    &source_fraction_bits,
                    row,
                )?
            } else {
                rescale_i128(
                    i128::from(*source.get(row).ok_or_else(invalid_transform)?),
                    *source_fraction_bits
                        .get(row)
                        .ok_or_else(invalid_transform)?,
                    MCT_FIXED_BITS,
                )?
                .checked_add(offset_value_scaled(collection.offsets(), row)?)
                .and_then(|value| i64::try_from(value).ok())
                .ok_or_else(arithmetic_overflow)?
            });
            row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        row = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant row <= rows,
                decreases rows - row,
            )
        )]
        while row < rows {
            let destination = *destination_indexes.get(row).ok_or_else(invalid_transform)?;
            let value = *output.get(row).ok_or_else(invalid_transform)?;
            store_transform_sample(components, destination, sample_index, value)?;
            row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        sample_index = sample_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    let output_fraction_bits = if collection.reversible() {
        0
    } else {
        MCT_FIXED_BITS
    };
    index_index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index_index <= destination_indexes.len(),
            decreases destination_indexes.len() - index_index,
        )
    )]
    while index_index < destination_indexes.len() {
        let index = *destination_indexes
            .get(index_index)
            .ok_or_else(invalid_transform)?;
        *components
            .get_mut(index)
            .ok_or_else(invalid_transform)?
            .fraction_bits = output_fraction_bits;
        index_index = index_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

fn apply_mct_dependency(
    components: &mut [TransformComponentMut<'_>],
    collection: &MctCollection,
    limits: &ResourceLimits,
    work: &mut u64,
) -> Result<(), DecodeError> {
    let count = collection.input_components().len();
    if count == 0 || count != collection.output_components().len() {
        return Err(invalid_transform());
    }
    let expected_coefficients = if collection.reversible() {
        count
            .checked_mul(count.checked_add(1).ok_or_else(arithmetic_overflow)?)
            .and_then(|value| value.checked_div(2))
            .and_then(|value| value.checked_sub(1))
            .ok_or_else(arithmetic_overflow)?
    } else {
        count
            .checked_mul(count.saturating_sub(1))
            .and_then(|value| value.checked_div(2))
            .ok_or_else(arithmetic_overflow)?
    };
    if matches!(
        collection.matrix(),
        Some(coefficients) if coefficients.values().len() != expected_coefficients
    ) || matches!(
        collection.offsets(),
        Some(offsets) if offsets.values().len() != count
    ) {
        return Err(invalid_transform());
    }
    let mut source_indexes = Vec::with_capacity(count);
    let mut destination_indexes = Vec::with_capacity(count);
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= count,
            decreases count - index,
        )
    )]
    while index < count {
        source_indexes.push(find_transform_component(
            components,
            *collection
                .input_components()
                .get(index)
                .ok_or_else(invalid_transform)?,
        )?);
        destination_indexes.push(find_transform_component(
            components,
            *collection
                .output_components()
                .get(index)
                .ok_or_else(invalid_transform)?,
        )?);
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let first_index = *source_indexes.first().ok_or_else(invalid_transform)?;
    let first = components.get(first_index).ok_or_else(invalid_transform)?;
    let sample_count = first.samples.len();
    let geometry = (
        first.width,
        first.height,
        first.x,
        first.y,
        first.x_subsampling,
        first.y_subsampling,
    );
    let mut list = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant list <= 2,
            decreases 2 - list,
        )
    )]
    while list < 2 {
        let indexes = if list == 0 {
            &source_indexes
        } else {
            &destination_indexes
        };
        let mut component_position = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_position <= indexes.len(),
                decreases indexes.len() - component_position,
            )
        )]
        while component_position < indexes.len() {
            let component = components
                .get(
                    *indexes
                        .get(component_position)
                        .ok_or_else(invalid_transform)?,
                )
                .ok_or_else(invalid_transform)?;
            if (
                component.width,
                component.height,
                component.x,
                component.y,
                component.x_subsampling,
                component.y_subsampling,
            ) != geometry
                || component.samples.len() != sample_count
                || *component.fraction_bits > MCT_FIXED_BITS
                || (collection.reversible() && *component.fraction_bits != 0)
            {
                return Err(invalid_transform());
            }
            component_position = component_position
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        list = list.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    if collection.reversible()
        && (matches!(
            collection.matrix(),
            Some(coefficients) if coefficients.fraction_bits() != 0
        ) || matches!(
            collection.offsets(),
            Some(offsets) if offsets.fraction_bits() != 0
        ))
    {
        return Err(invalid_transform());
    }
    let operations = (sample_count as u64)
        .checked_mul(
            (expected_coefficients as u64)
                .checked_mul(2)
                .and_then(|value| value.checked_add(count as u64))
                .ok_or_else(arithmetic_overflow)?,
        )
        .ok_or_else(arithmetic_overflow)?;
    charge_mct_work(work, operations, limits)?;
    check_mct_scratch(count.saturating_mul(3), limits)?;

    let mut source = Vec::with_capacity(count);
    let mut output = Vec::with_capacity(count);
    let mut source_fraction_bits = Vec::with_capacity(count);
    index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= count,
            decreases count - index,
        )
    )]
    while index < count {
        source_fraction_bits.push(
            *components
                .get(*source_indexes.get(index).ok_or_else(invalid_transform)?)
                .ok_or_else(invalid_transform)?
                .fraction_bits,
        );
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let mut sample_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant sample_index <= sample_count,
            decreases sample_count - sample_index,
        )
    )]
    while sample_index < sample_count {
        source.clear();
        output.clear();
        index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= count,
                decreases count - index,
            )
        )]
        while index < count {
            source.push(
                *components
                    .get(*source_indexes.get(index).ok_or_else(invalid_transform)?)
                    .and_then(|component| component.samples.get(sample_index))
                    .ok_or_else(invalid_transform)?,
            );
            index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        let mut row = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant row <= count,
                decreases count - row,
            )
        )]
        while row < count {
            let value = if collection.reversible() {
                reversible_dependency_row(collection, &source, &output, row)?
            } else {
                irreversible_dependency_row(
                    collection,
                    &source,
                    &source_fraction_bits,
                    &output,
                    row,
                )?
            };
            output.push(value);
            row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= count,
                decreases count - index,
            )
        )]
        while index < count {
            let destination = *destination_indexes
                .get(index)
                .ok_or_else(invalid_transform)?;
            *components
                .get_mut(destination)
                .and_then(|component| component.samples.get_mut(sample_index))
                .ok_or_else(invalid_transform)? =
                *output.get(index).ok_or_else(invalid_transform)?;
            index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        sample_index = sample_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= destination_indexes.len(),
            decreases destination_indexes.len() - index,
        )
    )]
    while index < destination_indexes.len() {
        let destination = *destination_indexes
            .get(index)
            .ok_or_else(invalid_transform)?;
        *components
            .get_mut(destination)
            .ok_or_else(invalid_transform)?
            .fraction_bits = if collection.reversible() {
            0
        } else {
            MCT_FIXED_BITS
        };
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_mct_dependency_row(
        crate::codestream::mct_collection_view(collection).4, crate::codestream::mct_collection_view(collection).5,
        source@, None, output@, row,
    ),
))]
fn reversible_dependency_row(
    collection: &MctCollection,
    source: &[i64],
    output: &[i64],
    row: usize,
) -> Result<i64, DecodeError> {
    let base = (*source.get(row).ok_or_else(invalid_transform)?)
        .checked_add(offset_value(collection.offsets(), row)?)
        .ok_or_else(arithmetic_overflow)?;
    let Some(coefficients) = collection.matrix() else {
        return Ok(base);
    };
    if row == 0 {
        return Ok(base);
    }
    let next_row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
    let triangle = row.checked_mul(next_row).ok_or_else(arithmetic_overflow)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(triangle >= 2) by (nonlinear_arith)
            requires row > 0, next_row as int == row as int + 1,
                triangle as int == row as int * next_row as int;
    }
    let half = triangle.checked_div(2).ok_or_else(arithmetic_overflow)?;
    let row_start = half.checked_sub(1).ok_or_else(arithmetic_overflow)?;
    let scale = *coefficients
        .values()
        .get(row_start.checked_add(row).ok_or_else(arithmetic_overflow)?)
        .ok_or_else(invalid_transform)?;
    if scale <= 0 {
        return Err(invalid_transform());
    }
    let mut prediction = i128::from(scale / 2);
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost initial = prediction; }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_mct_dot_empty(initial, crate::codestream::mct_array_view(coefficients), output@, None, row_start);
    }
    let mut column = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant column <= row,
            row > 0,
            crate::specs::part2_mct_dependency_initial(crate::codestream::mct_collection_view(collection).5, source@, None, row) == Ok(base as i128),
            crate::codestream::mct_collection_view(collection).4 == Some(crate::codestream::mct_array_view(coefficients)),
            row as nat + 1 <= usize::MAX,
            row as nat * (row as nat + 1) <= usize::MAX,
            row_start as nat == row as nat * (row as nat + 1) / 2 - 1,
            (row_start as nat + row as nat) < crate::codestream::mct_array_view(coefficients).4.len(),
            row_start as nat + row as nat <= usize::MAX,
            crate::codestream::mct_array_view(coefficients).4[row_start as int + row as int] == scale,
            scale > 0,
            initial == (scale / 2) as i128,
            crate::specs::part2_mct_dot_prefix(initial, crate::codestream::mct_array_view(coefficients), output@, None, row_start, column as nat) == Ok(prediction),
            decreases row - column,
        )
    )]
    while column < row {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_mct_dot_step(initial, crate::codestream::mct_array_view(coefficients), output@, None, row_start, column as nat);
            crate::proofs::production_impl::production_mct_dot_error_guard(initial, crate::codestream::mct_array_view(coefficients), output@, None, row_start, column as nat + 1, row as nat);
        }
        let coefficient = *coefficients
            .values()
            .get(
                row_start
                    .checked_add(column)
                    .ok_or_else(arithmetic_overflow)?,
            )
            .ok_or_else(invalid_transform)?;
        let product = i128::from(coefficient)
            .checked_mul(i128::from(
                *output.get(column).ok_or_else(invalid_transform)?,
            ))
            .ok_or_else(arithmetic_overflow)?;
        prediction = prediction
            .checked_add(product)
            .ok_or_else(arithmetic_overflow)?;
        column = column.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let divisor = i128::from(scale);
    let quotient = prediction / divisor;
    let predicted = if prediction % divisor < 0 {
        quotient.checked_sub(1).ok_or_else(arithmetic_overflow)?
    } else {
        quotient
    };
    match i64::try_from(
        i128::from(base)
            .checked_add(predicted)
            .ok_or_else(arithmetic_overflow)?,
    ) {
        Ok(value) => Ok(value),
        Err(_) => Err(arithmetic_overflow()),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_mct_dependency_row(
        crate::codestream::mct_collection_view(collection).4, crate::codestream::mct_collection_view(collection).5,
        source@, Some(source_fraction_bits@), output@, row,
    ),
))]
fn irreversible_dependency_row(
    collection: &MctCollection,
    source: &[i64],
    source_fraction_bits: &[u8],
    output: &[i64],
    row: usize,
) -> Result<i64, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { crate::codestream::mct_fixed_precision_is_q32(); }
    let mut sum = rescale_i128(
        i128::from(*source.get(row).ok_or_else(invalid_transform)?),
        *source_fraction_bits
            .get(row)
            .ok_or_else(invalid_transform)?,
        MCT_FIXED_BITS,
    )?
    .checked_add(offset_value_scaled(collection.offsets(), row)?)
    .ok_or_else(arithmetic_overflow)?;
    let Some(coefficients) = collection.matrix() else {
        return match i64::try_from(sum) {
            Ok(value) => Ok(value),
            Err(_) => Err(arithmetic_overflow()),
        };
    };
    let previous = row.saturating_sub(1);
    let triangle = row.checked_mul(previous).ok_or_else(arithmetic_overflow)?;
    let row_start = triangle.checked_div(2).ok_or_else(arithmetic_overflow)?;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost initial = sum; }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_mct_dot_empty(initial, crate::codestream::mct_array_view(coefficients), output@, Some(Seq::new(output@.len(), |index: int| 32u8)), row_start);
    }
    let mut column = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant column <= row,
            MCT_FIXED_BITS == 32,
            crate::specs::part2_mct_dependency_initial(crate::codestream::mct_collection_view(collection).5, source@, Some(source_fraction_bits@), row) == Ok(initial),
            crate::codestream::mct_collection_view(collection).4 == Some(crate::codestream::mct_array_view(coefficients)),
            previous as nat == if row == 0 { 0nat } else { (row - 1) as nat },
            row as nat * previous as nat <= usize::MAX,
            row_start as nat == row as nat * previous as nat / 2,
            crate::specs::part2_mct_dot_prefix(initial, crate::codestream::mct_array_view(coefficients), output@, Some(Seq::new(output@.len(), |index: int| 32u8)), row_start, column as nat) == Ok(sum),
            decreases row - column,
        )
    )]
    while column < row {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_mct_dot_step(initial, crate::codestream::mct_array_view(coefficients), output@, Some(Seq::new(output@.len(), |index: int| 32u8)), row_start, column as nat);
            crate::proofs::production_impl::production_mct_dot_error_guard(initial, crate::codestream::mct_array_view(coefficients), output@, Some(Seq::new(output@.len(), |index: int| 32u8)), row_start, column as nat + 1, row as nat);
        }
        let coefficient = *coefficients
            .values()
            .get(
                row_start
                    .checked_add(column)
                    .ok_or_else(arithmetic_overflow)?,
            )
            .ok_or_else(invalid_transform)?;
        let product = i128::from(coefficient)
            .checked_mul(i128::from(
                *output.get(column).ok_or_else(invalid_transform)?,
            ))
            .ok_or_else(arithmetic_overflow)?;
        sum = sum
            .checked_add(rescale_i128(
                product,
                coefficients
                    .fraction_bits()
                    .checked_add(MCT_FIXED_BITS)
                    .ok_or_else(arithmetic_overflow)?,
                MCT_FIXED_BITS,
            )?)
            .ok_or_else(arithmetic_overflow)?;
        column = column.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    match i64::try_from(sum) {
        Ok(value) => Ok(value),
        Err(_) => Err(arithmetic_overflow()),
    }
}

fn apply_mct_wavelet(
    components: &mut [TransformComponentMut<'_>],
    collection: &MctCollection,
    limits: &ResourceLimits,
    work: &mut u64,
) -> Result<(), DecodeError> {
    let wavelet = collection.wavelet().ok_or_else(invalid_transform)?;
    let count = collection.input_components().len();
    if count == 0 || count != collection.output_components().len() {
        return Err(invalid_transform());
    }
    let mut source_indexes = Vec::with_capacity(count);
    let mut destination_indexes = Vec::with_capacity(count);
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= count,
            decreases count - index,
        )
    )]
    while index < count {
        source_indexes.push(find_transform_component(
            components,
            *collection
                .input_components()
                .get(index)
                .ok_or_else(invalid_transform)?,
        )?);
        destination_indexes.push(find_transform_component(
            components,
            *collection
                .output_components()
                .get(index)
                .ok_or_else(invalid_transform)?,
        )?);
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let first = components
        .get(*source_indexes.first().ok_or_else(invalid_transform)?)
        .ok_or_else(invalid_transform)?;
    let sample_count = first.samples.len();
    let geometry = (
        first.width,
        first.height,
        first.x,
        first.y,
        first.x_subsampling,
        first.y_subsampling,
    );
    let mut list = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant list <= 2,
            decreases 2 - list,
        )
    )]
    while list < 2 {
        let indexes = if list == 0 {
            &source_indexes
        } else {
            &destination_indexes
        };
        let mut component_position = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component_position <= indexes.len(),
                decreases indexes.len() - component_position,
            )
        )]
        while component_position < indexes.len() {
            let component = components
                .get(
                    *indexes
                        .get(component_position)
                        .ok_or_else(invalid_transform)?,
                )
                .ok_or_else(invalid_transform)?;
            if (
                component.width,
                component.height,
                component.x,
                component.y,
                component.x_subsampling,
                component.y_subsampling,
            ) != geometry
                || component.samples.len() != sample_count
                || *component.fraction_bits > MCT_FIXED_BITS
                || (matches!(wavelet.transform(), MctWaveletType::Reversible5x3)
                    && *component.fraction_bits != 0)
            {
                return Err(invalid_transform());
            }
            component_position = component_position
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        list = list.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let mut low_first = Vec::with_capacity(usize::from(wavelet.levels()));
    let mut high_lengths = Vec::with_capacity(usize::from(wavelet.levels()));
    let mut canvas_start = i64::from(wavelet.component_offset());
    let mut canvas_end = canvas_start
        .checked_add(i64::try_from(count).map_err(|_| arithmetic_overflow())?)
        .ok_or_else(arithmetic_overflow)?;
    let level_count = wavelet.levels();
    let mut level = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(decreases level_count - level,)
    )]
    while level < level_count {
        let next_start = ceil_divide_by_two(canvas_start)?;
        let next_end = ceil_divide_by_two(canvas_end)?;
        let low_length = usize::try_from(
            next_end
                .checked_sub(next_start)
                .ok_or_else(arithmetic_overflow)?,
        )
        .map_err(|_| invalid_transform())?;
        let current_length = usize::try_from(
            canvas_end
                .checked_sub(canvas_start)
                .ok_or_else(arithmetic_overflow)?,
        )
        .map_err(|_| invalid_transform())?;
        low_first.push(canvas_start % 2 == 0);
        high_lengths.push(
            current_length
                .checked_sub(low_length)
                .ok_or_else(arithmetic_overflow)?,
        );
        canvas_start = next_start;
        canvas_end = next_end;
        level = level.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let low_length = usize::try_from(
        canvas_end
            .checked_sub(canvas_start)
            .ok_or_else(arithmetic_overflow)?,
    )
    .map_err(|_| invalid_transform())?;
    let operations = (sample_count as u64)
        .checked_mul(count as u64)
        .and_then(|value| value.checked_mul(u64::from(wavelet.levels()).saturating_add(1)))
        .and_then(|value| value.checked_mul(8))
        .ok_or_else(arithmetic_overflow)?;
    charge_mct_work(work, operations, limits)?;
    check_mct_scratch(count.saturating_mul(3), limits)?;

    let mut source_fraction_bits = Vec::with_capacity(count);
    index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= source_indexes.len(),
            decreases source_indexes.len() - index,
        )
    )]
    while index < source_indexes.len() {
        let source_index = *source_indexes.get(index).ok_or_else(invalid_transform)?;
        source_fraction_bits.push(
            *components
                .get(source_index)
                .ok_or_else(invalid_transform)?
                .fraction_bits,
        );
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let mut source = Vec::with_capacity(count);
    let mut sample_index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant sample_index <= sample_count,
            decreases sample_count - sample_index,
        )
    )]
    while sample_index < sample_count {
        source.clear();
        index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= count,
                decreases count - index,
            )
        )]
        while index < count {
            let value = *components
                .get(*source_indexes.get(index).ok_or_else(invalid_transform)?)
                .and_then(|component| component.samples.get(sample_index))
                .ok_or_else(invalid_transform)?;
            source.push(
                if matches!(wavelet.transform(), MctWaveletType::Reversible5x3) {
                    value
                } else {
                    i64::try_from(rescale_i128(
                        i128::from(value),
                        *source_fraction_bits
                            .get(index)
                            .ok_or_else(invalid_transform)?,
                        MCT_FIXED_BITS,
                    )?)
                    .map_err(|_| arithmetic_overflow())?
                },
            );
            index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        let mut cursor = low_length;
        let mut reconstructed = Vec::with_capacity(low_length);
        reconstructed.extend_from_slice(source.get(..low_length).ok_or_else(invalid_transform)?);
        let mut level_index = high_lengths.len();
        #[cfg_attr(verus_keep_ghost, verus_spec(decreases level_index,))]
        while level_index != 0 {
            level_index = level_index.checked_sub(1).ok_or_else(arithmetic_overflow)?;
            let high_length = *high_lengths
                .get(level_index)
                .ok_or_else(invalid_transform)?;
            let high_end = cursor
                .checked_add(high_length)
                .ok_or_else(arithmetic_overflow)?;
            let high = source.get(cursor..high_end).ok_or_else(invalid_transform)?;
            reconstructed = if matches!(wavelet.transform(), MctWaveletType::Reversible5x3) {
                inverse_5_3(
                    &reconstructed,
                    high,
                    *low_first.get(level_index).ok_or_else(invalid_transform)?,
                )?
            } else {
                inverse_9_7(
                    &reconstructed,
                    high,
                    *low_first.get(level_index).ok_or_else(invalid_transform)?,
                )?
            };
            cursor = high_end;
        }
        if cursor != count || reconstructed.len() != count {
            return Err(invalid_transform());
        }
        index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= count,
                decreases count - index,
            )
        )]
        while index < count {
            *components
                .get_mut(
                    *destination_indexes
                        .get(index)
                        .ok_or_else(invalid_transform)?,
                )
                .and_then(|component| component.samples.get_mut(sample_index))
                .ok_or_else(invalid_transform)? =
                *reconstructed.get(index).ok_or_else(invalid_transform)?;
            index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        sample_index = sample_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= destination_indexes.len(),
            decreases destination_indexes.len() - index,
        )
    )]
    while index < destination_indexes.len() {
        let destination = *destination_indexes
            .get(index)
            .ok_or_else(invalid_transform)?;
        *components
            .get_mut(destination)
            .ok_or_else(invalid_transform)?
            .fraction_bits = if matches!(wavelet.transform(), MctWaveletType::Reversible5x3) {
            0
        } else {
            MCT_FIXED_BITS
        };
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_mct_offset(crate::codestream::mct_array_option(offsets), row),
))]
fn offset_value(offsets: Option<&MctArray>, row: usize) -> Result<i64, DecodeError> {
    match offsets {
        Some(offsets) if offsets.fraction_bits() == 0 => {
            Ok(*offsets.values().get(row).ok_or_else(invalid_transform)?)
        }
        Some(_) => Err(invalid_transform()),
        None => Ok(0),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_mct_offset_scaled(crate::codestream::mct_array_option(offsets), row),
))]
fn offset_value_scaled(offsets: Option<&MctArray>, row: usize) -> Result<i128, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::codestream::mct_fixed_precision_is_q32();
        reveal(crate::specs::part2_mct_offset_scaled);
    }
    match offsets {
        Some(offsets) if offsets.fraction_bits() <= MCT_FIXED_BITS => rescale_i128(
            i128::from(*offsets.values().get(row).ok_or_else(invalid_transform)?),
            offsets.fraction_bits(),
            MCT_FIXED_BITS,
        ),
        Some(_) => Err(invalid_transform()),
        None => Ok(0),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, *final(work)) == crate::specs::part2_mct_charge_work(
        *old(work), operations, crate::limits::resource_work_limit(limits),
    ),
))]
fn charge_mct_work(
    work: &mut u64,
    operations: u64,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    *work = work
        .checked_add(operations)
        .ok_or_else(arithmetic_overflow)?;
    if *work > limits.maximum_work_units() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::WorkUnits,
            limit: limits.maximum_work_units(),
            actual: *work,
        });
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_byte_budget(values as nat, 8, crate::limits::resource_decoded_bytes_limit(limits)),
))]
fn check_mct_scratch(values: usize, limits: &ResourceLimits) -> Result<(), DecodeError> {
    let bytes = (values as u64)
        .checked_mul(core::mem::size_of::<i64>() as u64)
        .ok_or_else(arithmetic_overflow)?;
    if bytes > limits.maximum_decoded_bytes() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::DecodedBytes,
            limit: limits.maximum_decoded_bytes(),
            actual: bytes,
        });
    }
    Ok(())
}

fn ceil_divide_by_two(value: i64) -> Result<i64, DecodeError> {
    let quotient = value / 2;
    quotient
        .checked_add(if value % 2 > 0 { 1 } else { 0 })
        .ok_or_else(arithmetic_overflow)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_mct_find_component(transform_components_view(components), reference),
))]
fn find_transform_component(
    components: &[TransformComponentMut<'_>],
    reference: u16,
) -> Result<usize, DecodeError> {
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= components.len(),
            forall |earlier: int| 0 <= earlier < index ==> transform_components_view(components)[earlier].0 != reference,
            decreases components.len() - index,
        )
    )]
    while index < components.len() {
        let matches = match components.get(index) {
            Some(component) => component.component == reference,
            None => false,
        };
        if matches {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_mct_component_found(transform_components_view(components), reference, index);
            }
            return Ok(index);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert forall |earlier: int| 0 <= earlier < index + 1 implies
                transform_components_view(components)[earlier].0 != reference by {
                if earlier == index { assert(transform_components_view(components)[earlier].0 != reference); }
            }
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_mct_component_missing(transform_components_view(components), reference);
    }
    Err(invalid_transform())
}

#[cfg(any())]
fn apply_multiple_component_transform_legacy(
    image: &mut DecodedImage,
    transform: &MultipleComponentTransform,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let components = transform.input_components().len();
    if components == 0
        || components != image.components.len()
        || transform.output_components().len() != components
        || transform.matrix().values().len()
            != components
                .checked_mul(components)
                .ok_or_else(arithmetic_overflow)?
    {
        return Err(invalid_transform());
    }
    let invalid_offset_dimensions = match transform.offsets() {
        Some(offsets) => offsets.values().len() != components,
        None => false,
    };
    if invalid_offset_dimensions {
        return Err(invalid_transform());
    }
    let first = image.components.first().ok_or_else(invalid_transform)?;
    let sample_count = first.samples.len();
    let expected_fraction_bits = if transform.reversible() {
        0
    } else {
        FIXED_BITS
    };
    let mut component = 0_usize;
    while component < components {
        let input_reference = *transform
            .input_components()
            .get(component)
            .ok_or_else(invalid_transform)?;
        let output_reference = *transform
            .output_components()
            .get(component)
            .ok_or_else(invalid_transform)?;
        if usize::from(input_reference) != component || usize::from(output_reference) != component {
            return Err(invalid_transform());
        }
        let current = image
            .components
            .get(component)
            .ok_or_else(invalid_transform)?;
        if current.width != first.width
            || current.height != first.height
            || current.samples.len() != sample_count
            || current.info.x_subsampling != first.info.x_subsampling
            || current.info.y_subsampling != first.info.y_subsampling
            || (transform.reversible() && current.fraction_bits != 0)
            || (!transform.reversible()
                && current.fraction_bits != 0
                && current.fraction_bits != FIXED_BITS)
        {
            return Err(invalid_transform());
        }
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let operations = (sample_count as u64)
        .checked_mul(components as u64)
        .and_then(|value| value.checked_mul(components as u64))
        .ok_or_else(arithmetic_overflow)?;
    if operations > limits.maximum_work_units() {
        return Err(DecodeError::limit(
            Resource::WorkUnits,
            limits.maximum_work_units(),
            operations,
        ));
    }

    let mut source = Vec::with_capacity(components);
    let mut output = Vec::with_capacity(components);
    let mut sample_index = 0_usize;
    while sample_index < sample_count {
        source.clear();
        output.clear();
        component = 0;
        while component < components {
            source.push(component_sample(image, component, sample_index)?);
            component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        let mut row = 0_usize;
        while row < components {
            let transformed = if transform.reversible() {
                reversible_mct_row(transform.matrix(), transform.offsets(), &source, row)?
            } else {
                fixed_mct_row(transform.matrix(), transform.offsets(), &source, image, row)?
            };
            output.push(transformed);
            row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        component = 0;
        while component < components {
            set_component_sample(
                image,
                component,
                sample_index,
                *output.get(component).ok_or_else(invalid_transform)?,
            )?;
            component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        sample_index = sample_index
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    component = 0;
    while component < components {
        image
            .components
            .get_mut(component)
            .ok_or_else(invalid_transform)?
            .fraction_bits = expected_fraction_bits;
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_mct_matrix_row(
        crate::codestream::mct_array_view(matrix), crate::codestream::mct_array_option(offsets), source@, None, row,
    ),
))]
fn reversible_mct_row(
    matrix: &MctArray,
    offsets: Option<&MctArray>,
    source: &[i64],
    row: usize,
) -> Result<i64, DecodeError> {
    let floating_offsets = match offsets {
        Some(array) => array.fraction_bits() != 0,
        None => false,
    };
    if matrix.fraction_bits() != 0 || floating_offsets {
        return Err(invalid_transform());
    }
    let mut sum = match offsets {
        Some(array) => i128::from(*array.values().get(row).ok_or_else(invalid_transform)?),
        None => 0,
    };
    let width = source.len();
    let row_start = row.checked_mul(width).ok_or_else(arithmetic_overflow)?;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost initial = sum; }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_mct_dot_empty(initial, crate::codestream::mct_array_view(matrix), source@, None, row_start);
    }
    let mut column = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant column <= width,
            width == source.len(),
            row_start as nat == row as nat * width as nat,
            crate::specs::part2_mct_row_initial(crate::codestream::mct_array_view(matrix), crate::codestream::mct_array_option(offsets), width as nat, None, row) == Ok(initial),
            crate::specs::part2_mct_dot_prefix(initial, crate::codestream::mct_array_view(matrix), source@, None, row_start, column as nat) == Ok(sum),
            decreases width - column,
        )
    )]
    while column < width {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_mct_dot_step(initial, crate::codestream::mct_array_view(matrix), source@, None, row_start, column as nat);
            crate::proofs::production_impl::production_mct_dot_error_guard(initial, crate::codestream::mct_array_view(matrix), source@, None, row_start, column as nat + 1, width as nat);
        }
        let coefficient = *matrix
            .values()
            .get(
                row_start
                    .checked_add(column)
                    .ok_or_else(arithmetic_overflow)?,
            )
            .ok_or_else(invalid_transform)?;
        let product = i128::from(coefficient)
            .checked_mul(i128::from(
                *source.get(column).ok_or_else(invalid_transform)?,
            ))
            .ok_or_else(arithmetic_overflow)?;
        sum = sum.checked_add(product).ok_or_else(arithmetic_overflow)?;
        column = column.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    match i64::try_from(sum) {
        Ok(value) => Ok(value),
        Err(_) => Err(arithmetic_overflow()),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_mct_matrix_row(
        crate::codestream::mct_array_view(matrix), crate::codestream::mct_array_option(offsets), source@, Some(source_fraction_bits@), row,
    ),
))]
fn fixed_mct_row_scaled(
    matrix: &MctArray,
    offsets: Option<&MctArray>,
    source: &[i64],
    source_fraction_bits: &[u8],
    row: usize,
) -> Result<i64, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { crate::codestream::mct_fixed_precision_is_q32(); }
    let invalid_offset_precision = match offsets {
        Some(array) => array.fraction_bits() > MCT_FIXED_BITS,
        None => false,
    };
    if source.len() != source_fraction_bits.len()
        || matrix.fraction_bits() > MCT_FIXED_BITS
        || invalid_offset_precision
    {
        return Err(invalid_transform());
    }
    let mut sum = match offsets {
        Some(array) => rescale_i128(
            i128::from(*array.values().get(row).ok_or_else(invalid_transform)?),
            array.fraction_bits(),
            MCT_FIXED_BITS,
        )?,
        None => 0,
    };
    let width = source.len();
    let row_start = row.checked_mul(width).ok_or_else(arithmetic_overflow)?;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost initial = sum; }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_mct_dot_empty(initial, crate::codestream::mct_array_view(matrix), source@, Some(source_fraction_bits@), row_start);
    }
    let mut column = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant column <= width,
            width == source.len(),
            row_start as nat == row as nat * width as nat,
            crate::specs::part2_mct_row_initial(crate::codestream::mct_array_view(matrix), crate::codestream::mct_array_option(offsets), width as nat, Some(source_fraction_bits@), row) == Ok(initial),
            crate::specs::part2_mct_dot_prefix(initial, crate::codestream::mct_array_view(matrix), source@, Some(source_fraction_bits@), row_start, column as nat) == Ok(sum),
            decreases width - column,
        )
    )]
    while column < width {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::codestream::mct_fixed_precision_is_q32();
            crate::proofs::production_impl::production_mct_dot_step(initial, crate::codestream::mct_array_view(matrix), source@, Some(source_fraction_bits@), row_start, column as nat);
            crate::proofs::production_impl::production_mct_dot_error_guard(initial, crate::codestream::mct_array_view(matrix), source@, Some(source_fraction_bits@), row_start, column as nat + 1, width as nat);
        }
        let coefficient = *matrix
            .values()
            .get(
                row_start
                    .checked_add(column)
                    .ok_or_else(arithmetic_overflow)?,
            )
            .ok_or_else(invalid_transform)?;
        let sample = *source.get(column).ok_or_else(invalid_transform)?;
        let product_fraction_bits = matrix
            .fraction_bits()
            .checked_add(
                *source_fraction_bits
                    .get(column)
                    .ok_or_else(invalid_transform)?,
            )
            .ok_or_else(arithmetic_overflow)?;
        let product = i128::from(coefficient)
            .checked_mul(i128::from(sample))
            .ok_or_else(arithmetic_overflow)?;
        sum = sum
            .checked_add(rescale_i128(
                product,
                product_fraction_bits,
                MCT_FIXED_BITS,
            )?)
            .ok_or_else(arithmetic_overflow)?;
        column = column.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    match i64::try_from(sum) {
        Ok(value) => Ok(value),
        Err(_) => Err(arithmetic_overflow()),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_mct_rescale(value, from_bits, to_bits),
))]
fn rescale_i128(value: i128, from_bits: u8, to_bits: u8) -> Result<i128, DecodeError> {
    if from_bits == to_bits {
        return Ok(value);
    }
    if from_bits < to_bits {
        let shift = to_bits
            .checked_sub(from_bits)
            .ok_or_else(arithmetic_overflow)?;
        if shift > 126 {
            return Err(arithmetic_overflow());
        }
        return value
            .checked_mul(1_i128 << u32::from(shift))
            .ok_or_else(arithmetic_overflow);
    }
    rounded_shift_i128(
        value,
        from_bits
            .checked_sub(to_bits)
            .ok_or_else(arithmetic_overflow)?,
    )
}

#[cfg(any())]
fn fixed_mct_row(
    matrix: &MctArray,
    offsets: Option<&MctArray>,
    source: &[i64],
    image: &DecodedImage,
    row: usize,
) -> Result<i64, DecodeError> {
    let offset = match offsets {
        Some(array) => fixed_array_value(array, row)?,
        None => 0,
    };
    let mut sum = i128::from(offset)
        .checked_mul(1_i128 << u32::from(FIXED_BITS))
        .ok_or_else(arithmetic_overflow)?;
    let width = source.len();
    let row_start = row.checked_mul(width).ok_or_else(arithmetic_overflow)?;
    let mut column = 0_usize;
    while column < width {
        let coefficient = fixed_array_value(
            matrix,
            row_start
                .checked_add(column)
                .ok_or_else(arithmetic_overflow)?,
        )?;
        let sample = *source.get(column).ok_or_else(invalid_transform)?;
        let fraction_bits = image
            .components
            .get(column)
            .ok_or_else(invalid_transform)?
            .fraction_bits;
        let fixed_sample = if fraction_bits == 0 {
            sample
                .checked_mul(1_i64 << u32::from(FIXED_BITS))
                .ok_or_else(arithmetic_overflow)?
        } else {
            sample
        };
        sum = sum
            .checked_add(
                i128::from(coefficient)
                    .checked_mul(i128::from(fixed_sample))
                    .ok_or_else(arithmetic_overflow)?,
            )
            .ok_or_else(arithmetic_overflow)?;
        column = column.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let rounded = rounded_shift_i128(sum, FIXED_BITS)?;
    i64::try_from(rounded).map_err(|_| arithmetic_overflow())
}

#[cfg(any())]
fn fixed_array_value(array: &MctArray, index: usize) -> Result<i64, DecodeError> {
    let value = *array.values().get(index).ok_or_else(invalid_transform)?;
    match array.fraction_bits() {
        0 => value
            .checked_mul(1_i64 << u32::from(FIXED_BITS))
            .ok_or_else(arithmetic_overflow),
        FIXED_BITS => Ok(value),
        _ => Err(invalid_transform()),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part2_mct_round_shift(value, bits),
))]
fn rounded_shift_i128(value: i128, bits: u8) -> Result<i128, DecodeError> {
    if bits == 0 {
        return Ok(value);
    }
    if bits > 126 {
        return Err(arithmetic_overflow());
    }
    let half = 1_i128 << u32::from(bits - 1);
    let divisor = 1_i128 << u32::from(bits);
    if divisor == 0 || divisor == -1 {
        return Err(arithmetic_overflow());
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert((1i128 << (bits as u32)) > 0) by (bit_vector)
            requires bits > 0, bits <= 126;
        assert((1i128 << ((bits - 1) as u32)) > 0) by (bit_vector)
            requires bits > 0, bits <= 126;
    }
    if value < 0 {
        let magnitude = 0_i128.checked_sub(value).ok_or_else(arithmetic_overflow)?;
        let magnitude = magnitude
            .checked_add(half)
            .ok_or_else(arithmetic_overflow)?;
        let rounded = magnitude / divisor;
        0_i128.checked_sub(rounded).ok_or_else(arithmetic_overflow)
    } else {
        let rounded = value.checked_add(half).ok_or_else(arithmetic_overflow)?;
        Ok(rounded / divisor)
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_validate_transform_components(image@, fraction_bits),
))]
fn validate_transform_components(
    image: &DecodedImage,
    fraction_bits: u8,
) -> Result<(), DecodeError> {
    if image.components.len() < 3 {
        return Err(invalid_image());
    }

    let first = &image.components[0];
    let mut index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= 3,
            image.components.len() >= 3,
            decoded_component_view(*first) == image@.4[0],
            forall |i: int| 0 <= i < index ==> crate::specs::part1_component_transform_compatible(
                image@.4[0], image@.4[i], fraction_bits,
            ),
            decreases 3 - index,
        )
    )]
    while index < 3 {
        if index >= image.components.len() {
            return Err(invalid_image());
        }
        let component = &image.components[index];
        if component.width != first.width
            || component.height != first.height
            || component.fraction_bits != fraction_bits
            || component.info.x_subsampling != first.info.x_subsampling
            || component.info.y_subsampling != first.info.y_subsampling
        {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(!crate::specs::part1_component_transform_compatible(
                    image@.4[0], image@.4[index as int], fraction_bits,
                ));
                assert(exists |i: int| 0 <= i < 3 && !crate::specs::part1_component_transform_compatible(
                    image@.4[0], image@.4[i], fraction_bits,
                ));
            }
            return Err(invalid_image());
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_decoded_component_sample(image@, component, index),
))]
fn component_sample(
    image: &DecodedImage,
    component: usize,
    index: usize,
) -> Result<i64, DecodeError> {
    let component = image.components.get(component).ok_or_else(invalid_image)?;
    let sample = component.samples.get(index).ok_or_else(invalid_image)?;
    Ok(*sample)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(image)@) == crate::specs::part1_set_decoded_component_sample(
        old(image)@, component, index, value,
    ),
))]
fn set_component_sample(
    image: &mut DecodedImage,
    component: usize,
    index: usize,
    value: i64,
) -> Result<(), DecodeError> {
    let component = image
        .components
        .get_mut(component)
        .ok_or_else(invalid_image)?;
    let sample = component.samples.get_mut(index).ok_or_else(invalid_image)?;
    *sample = value;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(plane)@) == crate::specs::part1_assembly_pixel(
        old(plane)@, assembly_tile_view(tile), position[0], position[1], crop,
    ),
))]
fn assembly_pixel(
    plane: &mut AssemblyPlane,
    tile: TileComponent<'_>,
    position: [u32; 2],
    crop: Option<(u32, u32)>,
) -> Result<(), DecodeError> {
    let destination_x = match crop {
        None => tile
            .x
            .checked_add(position[0])
            .ok_or_else(arithmetic_overflow)?,
        Some(origin) => position[0]
            .checked_sub(origin.0)
            .ok_or_else(arithmetic_overflow)?,
    };
    let destination_y = match crop {
        None => tile
            .y
            .checked_add(position[1])
            .ok_or_else(arithmetic_overflow)?,
        Some(origin) => position[1]
            .checked_sub(origin.1)
            .ok_or_else(arithmetic_overflow)?,
    };
    let destination = index(plane.width, destination_x, destination_y)?;
    let source_x = match crop {
        None => position[0],
        Some(_) => position[0]
            .checked_sub(tile.x)
            .ok_or_else(arithmetic_overflow)?,
    };
    let source_y = match crop {
        None => position[1],
        Some(_) => position[1]
            .checked_sub(tile.y)
            .ok_or_else(arithmetic_overflow)?,
    };
    let source = index(tile.width, source_x, source_y)?;
    if destination >= plane.samples.len()
        || destination >= plane.visited.len()
        || source >= tile.samples.len()
        || plane.visited[destination]
    {
        return Err(invalid_image());
    }
    plane.samples[destination] = tile.samples[source];
    plane.visited[destination] = true;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(plane)@) == crate::specs::part1_assembly_row_prefix(
        old(plane)@, assembly_tile_view(tile), row[0], row[2], crop,
        if row[0] >= row[1] { 0nat } else { (row[1] - row[0]) as nat },
    ),
))]
fn assembly_row(
    plane: &mut AssemblyPlane,
    tile: TileComponent<'_>,
    row: [u32; 3],
    crop: Option<(u32, u32)>,
) -> Result<(), DecodeError> {
    if row[0] >= row[1] {
        return Ok(());
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original = plane@; }
    let mut x = row[0];
    let mut passes = row[1].checked_sub(row[0]).ok_or_else(arithmetic_overflow)?;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant row[0] <= x <= row[1],
        passes == row[1] - x,
        original == old(plane)@,
        crate::specs::part1_assembly_row_prefix(
            original, assembly_tile_view(tile), row[0], row[2], crop, (x - row[0]) as nat,
        ) == (Ok(()), plane@),
        decreases passes,
    ))]
    while x < row[1] {
        match assembly_pixel(plane, tile, [x, row[2]], crop) {
            Ok(()) => {}
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_assembly_row_error_persists(
                        original, assembly_tile_view(tile), row[0], row[2], crop,
                        (x - row[0]) as nat + 1, (row[1] - row[0]) as nat,
                    );
                }
                return Err(error);
            }
        }
        x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
        passes = passes.checked_sub(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(plane)@) == crate::specs::part1_assembly_rectangle(
        old(plane)@, assembly_tile_view(tile), (rectangle[0], rectangle[1], rectangle[2], rectangle[3]), crop,
    ),
))]
fn assembly_rectangle(
    plane: &mut AssemblyPlane,
    tile: TileComponent<'_>,
    rectangle: [u32; 4],
    crop: Option<(u32, u32)>,
) -> Result<(), DecodeError> {
    if rectangle[0] >= rectangle[2] || rectangle[1] >= rectangle[3] {
        return Ok(());
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original = plane@; }
    let mut y = rectangle[1];
    let mut passes = rectangle[3]
        .checked_sub(rectangle[1])
        .ok_or_else(arithmetic_overflow)?;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant rectangle[0] < rectangle[2],
        rectangle[1] <= y <= rectangle[3],
        passes == rectangle[3] - y,
        original == old(plane)@,
        crate::specs::part1_assembly_rectangle_prefix(
            original, assembly_tile_view(tile), (rectangle[0], rectangle[1], rectangle[2], rectangle[3]),
            crop, (y - rectangle[1]) as nat,
        ) == (Ok(()), plane@),
        decreases passes,
    ))]
    while y < rectangle[3] {
        match assembly_row(plane, tile, [rectangle[0], rectangle[2], y], crop) {
            Ok(()) => {}
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_assembly_rectangle_error_persists(
                        original, assembly_tile_view(tile), (rectangle[0], rectangle[1], rectangle[2], rectangle[3]), crop,
                        (y - rectangle[1]) as nat + 1, (rectangle[3] - rectangle[1]) as nat,
                    );
                }
                return Err(error);
            }
        }
        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
        passes = passes.checked_sub(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(plane)@) == crate::specs::part1_place_assembly_tile(old(plane)@, assembly_tile_view(tile)),
))]
fn place_tile(plane: &mut AssemblyPlane, tile: TileComponent<'_>) -> Result<(), DecodeError> {
    if tile.width == 0
        || tile.height == 0
        || tile
            .x
            .checked_add(tile.width)
            .ok_or_else(arithmetic_overflow)?
            > plane.width
        || tile
            .y
            .checked_add(tile.height)
            .ok_or_else(arithmetic_overflow)?
            > plane.height
        || tile.samples.len() != checked_area(tile.width, tile.height)?
    {
        return Err(invalid_image());
    }
    assembly_rectangle(plane, tile, [0, 0, tile.width, tile.height], None)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(plane)@) == crate::specs::part1_place_selected_tile(
        old(plane)@, assembly_tile_view(tile), (crop[0], crop[1], crop[2], crop[3]),
    ),
))]
fn place_selected_tile(
    plane: &mut AssemblyPlane,
    tile: TileComponent<'_>,
    crop: [u32; 4],
) -> Result<(), DecodeError> {
    let x0 = tile.x.max(crop[0]);
    let y0 = tile.y.max(crop[1]);
    let x1 = tile
        .x
        .checked_add(tile.width)
        .ok_or_else(arithmetic_overflow)?
        .min(crop[2]);
    let y1 = tile
        .y
        .checked_add(tile.height)
        .ok_or_else(arithmetic_overflow)?
        .min(crop[3]);
    assembly_rectangle(plane, tile, [x0, y0, x1, y1], Some((crop[0], crop[1])))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(plane)@) == crate::specs::part1_assembly_tile_step(
        old(plane)@, assembly_tile_view(tile), component, crop,
    ),
))]
fn assembly_tile_step(
    plane: &mut AssemblyPlane,
    tile: TileComponent<'_>,
    component: u16,
    crop: Option<(u32, u32, u32, u32)>,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_assembly_tile_step); }
    if tile.component != component {
        return Ok(());
    }
    if plane.fraction_bits.is_some() && plane.fraction_bits != Some(tile.fraction_bits) {
        return Err(invalid_image());
    }
    plane.fraction_bits = Some(tile.fraction_bits);
    match crop {
        None => place_tile(plane, tile),
        Some(region) => place_selected_tile(plane, tile, [region.0, region.1, region.2, region.3]),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == (forall |i: int| 0 <= i < visited.len() ==> visited[i]),
))]
fn all_pixels_visited(visited: &[bool]) -> bool {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= visited.len(),
        forall |i: int| 0 <= i < index ==> visited[i],
        decreases visited.len() - index,
    ))]
    while index < visited.len() {
        if !visited[index] {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(plane)@) == crate::specs::part1_collect_assembly_tiles(
        old(plane)@, assembly_tiles_view(tiles@), component, crop,
    ),
    result.is_ok() ==> final(plane).fraction_bits.is_some(),
))]
fn collect_assembly_tiles(
    plane: &mut AssemblyPlane,
    tiles: &[TileComponent<'_>],
    component: u16,
    crop: Option<(u32, u32, u32, u32)>,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original = plane@; }
    let mut index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_assembly_tiles_empty(
            original, assembly_tiles_view(tiles@), component, crop,
        );
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= tiles.len(),
        original == old(plane)@,
        crate::specs::part1_assembly_tiles_prefix(
            original, assembly_tiles_view(tiles@), component, crop, index as nat,
        ) == (Ok(()), plane@),
        decreases tiles.len() - index,
    ))]
    while index < tiles.len() {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost before = plane@; }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(assembly_tiles_view(tiles@)[index as int] == assembly_tile_view(tiles[index as int]));
            crate::proofs::production_impl::production_assembly_tiles_step(
                original, assembly_tiles_view(tiles@), component, crop, index as nat,
            );
            assert(crate::specs::part1_assembly_tiles_prefix(
                original, assembly_tiles_view(tiles@), component, crop, index as nat + 1,
            ) == crate::specs::part1_assembly_tile_step(
                before, assembly_tile_view(tiles[index as int]), component, crop,
            ));
        }
        let tile = tiles[index];
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(assembly_tile_view(tile) == assembly_tile_view(tiles[index as int]));
        }
        let transition = assembly_tile_step(plane, tile, component, crop);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert((transition, plane@) == crate::specs::part1_assembly_tile_step(
                before, assembly_tile_view(tile), component, crop,
            ));
            assert(assembly_tile_view(tile) == assembly_tile_view(tiles[index as int]));
        }
        match transition {
            Ok(()) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(transition.is_ok());
                    assert(transition->Ok_0 == ());
                    assert(transition == Ok(()));
                    assert((transition, plane@) == crate::specs::part1_assembly_tile_step(
                        before, assembly_tile_view(tile), component, crop,
                    ));
                    assert((Ok(()), plane@) == crate::specs::part1_assembly_tile_step(
                        before, assembly_tile_view(tiles[index as int]), component, crop,
                    ));
                }
            }
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_assembly_tiles_error_persists(
                        original, assembly_tiles_view(tiles@), component, crop, index as nat + 1, tiles.len() as nat,
                    );
                }
                return Err(error);
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::part1_assembly_tiles_prefix(
                original, assembly_tiles_view(tiles@), component, crop, index as nat + 1,
            ) == (Ok(()), plane@));
        }
        index += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(crate::specs::part1_assembly_tiles_prefix(
            original, assembly_tiles_view(tiles@), component, crop, tiles.len() as nat,
        ) == (Ok(()), plane@));
        assert(assembly_tiles_view(tiles@).len() == tiles.len());
    }
    if plane.fraction_bits.is_none() || !all_pixels_visited(&plane.visited) {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(plane.fraction_bits.is_none() || !(forall |i: int|
                0 <= i < plane.visited@.len() ==> plane.visited@[i]));
            assert(plane@.2 == plane.fraction_bits);
            assert(plane@.4 == plane.visited@);
            assert(crate::specs::part1_assembly_tiles_prefix(
                original, assembly_tiles_view(tiles@), component, crop, assembly_tiles_view(tiles@).len(),
            ) == (Ok(()), plane@));
            assert(crate::specs::part1_collect_assembly_tiles(
                original, assembly_tiles_view(tiles@), component, crop,
            ) == (Err(crate::specs::part1_assembly_coverage_error(crop.is_some())), plane@));
        }
        return Err(if crop.is_some() {
            DecodeError::InvalidSelection {
                reason: crate::SelectionError::RegionOutsideSelectedTiles,
            }
        } else {
            invalid_image()
        });
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_fixed_product(value, factor),
))]
fn fixed_mul(value: i64, factor: i64) -> Result<i64, DecodeError> {
    let bias = 1_i64 << 15;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert((1i64 << 15) == 32768) by(bit_vector);
        assert(FIXED_BITS == 16) by(compute);
    }
    let product = value.checked_mul(factor).ok_or_else(arithmetic_overflow)?;
    let biased = product.checked_add(bias).ok_or_else(arithmetic_overflow)?;
    Ok(biased >> FIXED_BITS)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_decoded_signed_value(value, fraction_bits, minimum, maximum),
))]
fn decoded_signed_sample(
    value: i64,
    fraction_bits: u8,
    minimum: i64,
    maximum: i64,
) -> Result<i64, DecodeError> {
    Ok(round_fixed(value, fraction_bits)?.max(minimum).min(maximum))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_decoded_unsigned_value(value, fraction_bits, level_shift, maximum),
))]
fn decoded_unsigned_sample(
    value: i64,
    fraction_bits: u8,
    level_shift: i64,
    maximum: u64,
) -> Result<u64, DecodeError> {
    let shifted = round_fixed(value, fraction_bits)?
        .checked_add(level_shift)
        .ok_or_else(arithmetic_overflow)?;
    Ok(u64::try_from(shifted.max(0))
        .map_err(|_| arithmetic_overflow())?
        .min(maximum))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_round_fixed(value, fraction_bits),
))]
fn round_fixed(value: i64, fraction_bits: u8) -> Result<i64, DecodeError> {
    if fraction_bits == 0 {
        return Ok(value);
    }
    if fraction_bits > 62 {
        return Err(invalid_image());
    }
    Ok(value
        .checked_add(1_i64 << u32::from(fraction_bits - 1))
        .ok_or_else(arithmetic_overflow)?
        >> u32::from(fraction_bits))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_ceil_div(value, divisor),
))]
fn ceil_div(value: u32, divisor: u32) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(invalid_image());
    }

    Ok(crate::refinement::ceil_div_u32(value, divisor))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_assembly_registered_ceil(value, divisor, registration),
))]
fn registered_ceil_div(value: u32, divisor: u32, registration: u16) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(invalid_image());
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_area(width, height),
))]
fn checked_area(width: u32, height: u32) -> Result<usize, DecodeError> {
    crate::transform::checked_area(width, height)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_offset(y, width, x),
))]
fn index(width: u32, x: u32, y: u32) -> Result<usize, DecodeError> {
    crate::transform::transform_offset(y, width, x)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_byte_budget(
        samples as nat, bytes_per_sample, crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
fn validate_bytes(
    samples: usize,
    bytes_per_sample: u64,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let bytes = (samples as u64)
        .checked_mul(bytes_per_sample)
        .ok_or_else(arithmetic_overflow)?;
    if bytes > limits.maximum_decoded_bytes() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::DecodedBytes,
            limit: limits.maximum_decoded_bytes(),
            actual: bytes,
        });
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_error(false),
))]
fn invalid_image() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidImageGeometry,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_error(false),
))]
fn invalid_transform() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidTransform,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_image_error(true),
))]
fn arithmetic_overflow() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_mct_preparation_derives_real_ordered_indexes_fractions_and_late_work_errors() {
        use crate::codestream::{MctArrayKind, MctNumericFormat};
        let matrix = || {
            MctArray::from_values(
                u8::MAX,
                MctArrayKind::Offset,
                MctNumericFormat::Float64,
                u8::MAX,
                vec![1, 0, 0, 1],
            )
        };
        let collection = MctCollection::from_arrays(true, vec![17, 0], vec![0, 17], matrix(), None);
        let mut samples = [vec![1, 2], vec![3, 4]];
        let before_samples = samples.clone();
        let mut fractions = [0, 0];
        let components: Vec<_> = samples
            .iter_mut()
            .zip(fractions.iter_mut())
            .zip([0, 17])
            .map(
                |((samples, fraction_bits), component)| TransformComponentMut {
                    component,
                    x: u32::MAX,
                    y: 0,
                    width: 0,
                    height: u32::MAX,
                    x_subsampling: 0,
                    y_subsampling: u8::MAX,
                    fraction_bits,
                    samples,
                },
            )
            .collect();
        let mut work = 7;
        let prepared = prepare_mct_decorrelation(
            &components,
            &collection,
            &ResourceLimits::default(),
            &mut work,
        )
        .unwrap();
        assert_eq!(prepared.source_indexes, [0, 1]);
        assert_eq!(prepared.destination_indexes, [1, 0]);
        assert_eq!(prepared.source_fraction_bits, [0, 0]);
        assert_eq!(
            (prepared.sample_count, prepared.rows, prepared.columns, work),
            (2, 2, 2, 19)
        );
        for maximum_work in [18, 19, 20] {
            for maximum_bytes in [31, 32, 33] {
                let limits = ResourceLimits::default()
                    .with_maximum_work_units(maximum_work)
                    .unwrap()
                    .with_maximum_decoded_bytes(maximum_bytes)
                    .unwrap();
                let expected_error = if maximum_work < 19 {
                    Some(DecodeError::LimitExceeded {
                        resource: Resource::WorkUnits,
                        limit: maximum_work,
                        actual: 19,
                    })
                } else if maximum_bytes < 32 {
                    Some(DecodeError::LimitExceeded {
                        resource: Resource::DecodedBytes,
                        limit: maximum_bytes,
                        actual: 32,
                    })
                } else {
                    None
                };
                let mut work = 7;
                assert_eq!(
                    prepare_mct_decorrelation(&components, &collection, &limits, &mut work).err(),
                    expected_error
                );
                assert_eq!(work, 19);
            }
        }
        let missing = MctCollection::from_arrays(true, vec![17, 0], vec![999, 17], matrix(), None);
        let mut work = 7;
        assert_eq!(
            prepare_mct_decorrelation(&components, &missing, &ResourceLimits::default(), &mut work)
                .err(),
            Some(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidTransform
            })
        );
        assert_eq!(work, 7);
        drop(components);
        assert_eq!(samples, before_samples);
        assert_eq!(fractions, [0, 0]);
    }

    #[test]
    fn native_mct_decorrelation_admission_keeps_every_overflow_limit_and_counter_frontier() {
        let overflow = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        for maximum_work in [1, 37, 1000] {
            for maximum_bytes in [1, 56, 1000] {
                let limits = ResourceLimits::default()
                    .with_maximum_work_units(maximum_work)
                    .unwrap()
                    .with_maximum_decoded_bytes(maximum_bytes)
                    .unwrap();
                for samples in [0, 1, 2, 3, 17, usize::MAX] {
                    for rows in [0, 1, 2, 3, 17, usize::MAX] {
                        for columns in [0, 1, 2, 3, 17, usize::MAX] {
                            for before in [0, 1, 7, u64::MAX - 1, u64::MAX] {
                                let sample_rows = samples as u128 * rows as u128;
                                let operations = if sample_rows > u128::from(u64::MAX) {
                                    None
                                } else {
                                    let products = sample_rows * columns as u128;
                                    if products > u128::from(u64::MAX)
                                        || products + sample_rows > u128::from(u64::MAX)
                                    {
                                        None
                                    } else {
                                        Some(products + sample_rows)
                                    }
                                };
                                let expected = match operations {
                                    None => (Err(overflow), before),
                                    Some(operations)
                                        if operations + u128::from(before)
                                            > u128::from(u64::MAX) =>
                                    {
                                        (Err(overflow), before)
                                    }
                                    Some(operations) => {
                                        let charged =
                                            u64::try_from(operations + u128::from(before)).unwrap();
                                        if charged > maximum_work {
                                            (
                                                Err(DecodeError::LimitExceeded {
                                                    resource: Resource::WorkUnits,
                                                    limit: maximum_work,
                                                    actual: charged,
                                                }),
                                                charged,
                                            )
                                        } else {
                                            let values = rows as u128 + columns as u128;
                                            let bytes = values * 8;
                                            if values > usize::MAX as u128
                                                || bytes > usize::MAX as u128
                                            {
                                                (Err(overflow), charged)
                                            } else if bytes > u128::from(maximum_bytes) {
                                                (
                                                    Err(DecodeError::LimitExceeded {
                                                        resource: Resource::DecodedBytes,
                                                        limit: maximum_bytes,
                                                        actual: u64::try_from(bytes).unwrap(),
                                                    }),
                                                    charged,
                                                )
                                            } else {
                                                (Ok(()), charged)
                                            }
                                        }
                                    }
                                };
                                let mut work = before;
                                let result = admit_mct_decorrelation(
                                    samples, rows, columns, &limits, &mut work,
                                );
                                assert_eq!((result, work), expected);
                            }
                        }
                    }
                }
            }
        }
        for maximum_work in [36, 37, 38] {
            for maximum_bytes in [55, 56, 57] {
                let limits = ResourceLimits::default()
                    .with_maximum_work_units(maximum_work)
                    .unwrap()
                    .with_maximum_decoded_bytes(maximum_bytes)
                    .unwrap();
                let mut work = 7;
                let expected = if maximum_work < 37 {
                    Err(DecodeError::LimitExceeded {
                        resource: Resource::WorkUnits,
                        limit: maximum_work,
                        actual: 37,
                    })
                } else if maximum_bytes < 56 {
                    Err(DecodeError::LimitExceeded {
                        resource: Resource::DecodedBytes,
                        limit: maximum_bytes,
                        actual: 56,
                    })
                } else {
                    Ok(())
                };
                assert_eq!(
                    admit_mct_decorrelation(2, 3, 4, &limits, &mut work),
                    expected
                );
                assert_eq!(work, 37);
            }
        }
    }

    #[test]
    fn native_mct_layout_matches_raw_geometry_precision_and_index_reference() {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidTransform,
        };
        let base = (u32::MAX, 0_u32, 0_u32, u32::MAX, 0_u8, u8::MAX);
        let sources = [
            vec![],
            vec![0],
            vec![1],
            vec![0, 1],
            vec![1, 0],
            vec![2, 2],
            vec![3],
            vec![usize::MAX],
        ];
        let destinations = [vec![], vec![0], vec![1, 2], vec![0, 3], vec![usize::MAX]];
        for perturbation in 0..9 {
            let mut geometry = [base; 3];
            let mut lengths = [2, 2, 2];
            match perturbation {
                1 => geometry[1].0 = 0,
                2 => geometry[1].1 = u32::MAX,
                3 => geometry[1].2 = 1,
                4 => geometry[1].3 = 0,
                5 => geometry[1].4 = 1,
                6 => geometry[1].5 = 0,
                7 => lengths[1] = 1,
                8 => lengths[0] = 0,
                _ => {}
            }
            for first_bits in [0, 1, 32, 33, u8::MAX] {
                for second_bits in [0, 1, 32, 33, u8::MAX] {
                    for third_bits in [0, 32, u8::MAX] {
                        for source in &sources {
                            for destination in &destinations {
                                for reversible in [false, true] {
                                    let mut fractions = [first_bits, second_bits, third_bits];
                                    let before_fractions = fractions;
                                    let expected = source
                                        .first()
                                        .and_then(|first| {
                                            let first_geometry = geometry.get(*first)?;
                                            let valid = source
                                                .iter()
                                                .chain(destination.iter())
                                                .all(|index| {
                                                    geometry.get(*index).is_some_and(|candidate| {
                                                        candidate == first_geometry
                                                            && lengths[*index] == lengths[*first]
                                                            && fractions[*index] <= 32
                                                    })
                                                });
                                            (valid
                                                && (!reversible
                                                    || source
                                                        .iter()
                                                        .all(|index| fractions[*index] == 0)))
                                            .then_some(lengths[*first])
                                        })
                                        .ok_or(invalid);
                                    let mut samples = lengths.map(|length| vec![i64::MIN; length]);
                                    let before_samples = samples.clone();
                                    let components: Vec<_> = samples
                                        .iter_mut()
                                        .zip(fractions.iter_mut())
                                        .zip(geometry)
                                        .map(
                                            |(
                                                (samples, fraction_bits),
                                                (x, y, width, height, x_subsampling, y_subsampling),
                                            )| {
                                                TransformComponentMut {
                                                    component: u16::MAX,
                                                    x,
                                                    y,
                                                    width,
                                                    height,
                                                    x_subsampling,
                                                    y_subsampling,
                                                    fraction_bits,
                                                    samples,
                                                }
                                            },
                                        )
                                        .collect();
                                    assert_eq!(
                                        validate_mct_components_layout(
                                            &components,
                                            source,
                                            destination,
                                            reversible
                                        ),
                                        expected
                                    );
                                    drop(components);
                                    assert_eq!(samples, before_samples);
                                    assert_eq!(fractions, before_fractions);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn native_mct_array_store_frames_every_unselected_component_and_error() {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidTransform,
        };
        for destination in [0, 1, 2, 3, usize::MAX] {
            for index in [0, 1, 2, usize::MAX] {
                for value in [i64::MIN, 0, i64::MAX] {
                    let mut samples = [vec![], vec![7], vec![-3, 9]];
                    let mut expected = samples.clone();
                    let expected_result = match expected
                        .get_mut(destination)
                        .and_then(|samples| samples.get_mut(index))
                    {
                        Some(sample) => {
                            *sample = value;
                            Ok(())
                        }
                        None => Err(invalid),
                    };
                    let mut fractions = [u8::MAX, 32, 0];
                    let before_fractions = fractions;
                    let mut components: Vec<_> = samples
                        .iter_mut()
                        .zip(fractions.iter_mut())
                        .zip([0, 17, u16::MAX])
                        .map(
                            |((samples, fraction_bits), component)| TransformComponentMut {
                                component,
                                x: u32::MAX,
                                y: 1,
                                width: 0,
                                height: u32::MAX,
                                x_subsampling: 0,
                                y_subsampling: u8::MAX,
                                fraction_bits,
                                samples,
                            },
                        )
                        .collect();
                    assert_eq!(
                        store_transform_sample(&mut components, destination, index, value),
                        expected_result
                    );
                    for component in &components {
                        assert_eq!(
                            (
                                component.x,
                                component.y,
                                component.width,
                                component.height,
                                component.x_subsampling,
                                component.y_subsampling
                            ),
                            (u32::MAX, 1, 0, u32::MAX, 0, u8::MAX)
                        );
                    }
                    drop(components);
                    assert_eq!(samples, expected);
                    assert_eq!(fractions, before_fractions);
                }
            }
        }
    }

    #[test]
    fn native_mct_sample_store_preserves_all_metadata_and_failure_samples() {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidTransform,
        };
        for length in 0..=4 {
            for index in [0, 1, 3, 4, 5, usize::MAX] {
                for value in [i64::MIN, -1, 0, i64::MAX] {
                    let mut samples: Vec<_> =
                        (0..length).map(|sample| i64::from(sample) - 2).collect();
                    let mut expected = samples.clone();
                    let result = match expected.get_mut(index) {
                        Some(sample) => {
                            *sample = value;
                            Ok(())
                        }
                        None => Err(invalid),
                    };
                    let mut fraction_bits = u8::MAX;
                    let mut component = TransformComponentMut {
                        component: u16::MAX,
                        x: u32::MAX,
                        y: 1,
                        width: 0,
                        height: u32::MAX,
                        x_subsampling: 0,
                        y_subsampling: u8::MAX,
                        fraction_bits: &mut fraction_bits,
                        samples: &mut samples,
                    };
                    assert_eq!(
                        store_transform_component_sample(&mut component, index, value),
                        result
                    );
                    assert_eq!(
                        (
                            component.component,
                            component.x,
                            component.y,
                            component.width,
                            component.height,
                            component.x_subsampling,
                            component.y_subsampling,
                            *component.fraction_bits
                        ),
                        (u16::MAX, u32::MAX, 1, 0, u32::MAX, 0, u8::MAX, u8::MAX)
                    );
                    assert_eq!(component.samples, expected);
                }
            }
        }
    }

    #[test]
    fn native_mct_component_lookup_keeps_first_duplicate_and_all_raw_mutable_values() {
        let references = [0, u16::MAX, 17, 0];
        let mut samples = [vec![], vec![i64::MIN], vec![i64::MAX, 0], vec![3, -9, 1]];
        let mut fractions = [u8::MAX, 0, 32, 1];
        let before_samples = samples.clone();
        let before_fractions = fractions;
        let components: Vec<_> = samples
            .iter_mut()
            .zip(fractions.iter_mut())
            .zip(references)
            .map(
                |((samples, fraction_bits), component)| TransformComponentMut {
                    component,
                    x: u32::MAX,
                    y: 0,
                    width: 0,
                    height: u32::MAX,
                    x_subsampling: 0,
                    y_subsampling: u8::MAX,
                    fraction_bits,
                    samples,
                },
            )
            .collect();
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidTransform,
        };
        for reference in 0..=u16::MAX {
            let expected = references
                .iter()
                .position(|value| *value == reference)
                .ok_or(invalid);
            assert_eq!(find_transform_component(&components, reference), expected);
        }
        assert_eq!(find_transform_component(&[], 0), Err(invalid));
        drop(components);
        assert_eq!(samples, before_samples);
        assert_eq!(fractions, before_fractions);
    }

    #[test]
    fn native_mct_dependency_rows_match_independent_floor_and_rational_references() {
        use crate::codestream::{MctArrayKind, MctNumericFormat};
        let array = |fraction, values| {
            MctArray::from_values(
                u8::MAX,
                MctArrayKind::Offset,
                MctNumericFormat::Float64,
                fraction,
                values,
            )
        };
        let source = [10, -20, 30, -40];
        let offsets = [1, -2, 3, -4];
        for declared_reversible in [false, true] {
            for fraction in [0, 32, u8::MAX] {
                for coefficient in -2..=2 {
                    for scale in [1, 2, 3, 7] {
                        let mut matrix = vec![coefficient; 9];
                        for index in [1, 4, 8] {
                            matrix[index] = scale;
                        }
                        let collection = MctCollection::from_dependency(
                            declared_reversible,
                            vec![u16::MAX, 0, 0],
                            Some(array(fraction, matrix)),
                            Some(array(0, offsets.to_vec())),
                        );
                        for value in -5..=5 {
                            let output = [value, -value - 1, 2 * value];
                            for row in 0..4 {
                                let prediction = if row == 0 {
                                    0
                                } else {
                                    let dot: i128 = output
                                        .iter()
                                        .take(row)
                                        .map(|sample| i128::from(coefficient) * i128::from(*sample))
                                        .sum();
                                    (i128::from(scale / 2) + dot).div_euclid(i128::from(scale))
                                };
                                let expected =
                                    i128::from(source[row]) + i128::from(offsets[row]) + prediction;
                                assert_eq!(
                                    reversible_dependency_row(&collection, &source, &output, row),
                                    Ok(i64::try_from(expected).unwrap())
                                );
                            }
                        }
                    }
                }
            }
        }
        let nearest = |value: i128, bits: u8| {
            if bits == 0 {
                value
            } else {
                let divisor = 1_i128 << bits;
                let magnitude = (value.abs() + divisor / 2) / divisor;
                if value < 0 { -magnitude } else { magnitude }
            }
        };
        let output = [-7, 11, -13];
        for declared_reversible in [false, true] {
            for coefficient in -3..=3 {
                for coefficient_bits in [0, 1, 4] {
                    for offset_bits in [0, 1, 4] {
                        for source_bits in [0, 1, 4, 32] {
                            let collection = MctCollection::from_dependency(
                                declared_reversible,
                                vec![],
                                Some(array(coefficient_bits, vec![coefficient; 6])),
                                Some(array(offset_bits, offsets.to_vec())),
                            );
                            for row in 0..4 {
                                let initial = i128::from(source[row])
                                    * (1_i128 << (32 - source_bits))
                                    + i128::from(offsets[row]) * (1_i128 << (32 - offset_bits));
                                let expected = initial
                                    + output
                                        .iter()
                                        .take(row)
                                        .map(|sample| {
                                            nearest(
                                                i128::from(coefficient) * i128::from(*sample),
                                                coefficient_bits,
                                            )
                                        })
                                        .sum::<i128>();
                                assert_eq!(
                                    irreversible_dependency_row(
                                        &collection,
                                        &source,
                                        &[source_bits; 4],
                                        &output,
                                        row
                                    ),
                                    Ok(i64::try_from(expected).unwrap())
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn native_mct_dependency_rows_preserve_lazy_raw_precision_and_first_error_frontiers() {
        use crate::codestream::{MctArrayKind, MctNumericFormat};
        let array = |fraction, values| {
            MctArray::from_values(
                u8::MAX,
                MctArrayKind::Offset,
                MctNumericFormat::Float32,
                fraction,
                values,
            )
        };
        let collection =
            |matrix, offsets| MctCollection::from_dependency(false, vec![], matrix, offsets);
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidTransform,
        };
        let overflow = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        for precision in 0..=u8::MAX {
            let empty = collection(Some(array(precision, vec![])), None);
            assert_eq!(reversible_dependency_row(&empty, &[7], &[], 0), Ok(7));
            assert_eq!(
                irreversible_dependency_row(&empty, &[7], &[32], &[], 0),
                Ok(7)
            );
            let reversible = collection(Some(array(precision, vec![0, 1])), None);
            assert_eq!(
                reversible_dependency_row(&reversible, &[0, 0], &[0], 1),
                Ok(0)
            );
            let irreversible = collection(Some(array(precision, vec![0])), None);
            assert_eq!(
                irreversible_dependency_row(&irreversible, &[0, 0], &[0, 0], &[0], 1),
                if precision <= 126 {
                    Ok(0)
                } else {
                    Err(overflow)
                }
            );
            let no_matrix = collection(None, None);
            assert_eq!(
                irreversible_dependency_row(&no_matrix, &[0], &[precision], &[], 0),
                if precision <= 158 {
                    Ok(0)
                } else {
                    Err(overflow)
                }
            );
            let offsets = collection(None, Some(array(precision, vec![0])));
            assert_eq!(
                irreversible_dependency_row(&offsets, &[0], &[32], &[], 0),
                if precision <= 32 { Ok(0) } else { Err(invalid) }
            );
            assert_eq!(
                reversible_dependency_row(&offsets, &[0], &[], 0),
                if precision == 0 { Ok(0) } else { Err(invalid) }
            );
        }
        let malformed = collection(Some(array(u8::MAX, vec![])), Some(array(u8::MAX, vec![])));
        assert_eq!(
            reversible_dependency_row(&malformed, &[], &[], usize::MAX),
            Err(invalid)
        );
        assert_eq!(
            irreversible_dependency_row(&malformed, &[0], &[159], &[], 0),
            Err(overflow)
        );
        let missing = collection(None, None);
        assert_eq!(
            irreversible_dependency_row(&missing, &[0], &[], &[], 0),
            Err(invalid)
        );
        let late = collection(Some(array(0, vec![0, 1, i64::MIN, i64::MIN, 1])), None);
        assert_eq!(
            reversible_dependency_row(&late, &[0, 0, 0], &[i64::MIN; 2], 2),
            Err(overflow)
        );
        let base = collection(Some(array(0, vec![])), Some(array(0, vec![1])));
        assert_eq!(
            reversible_dependency_row(&base, &[i64::MAX], &[], 0),
            Err(overflow)
        );
        let floor = collection(Some(array(0, vec![-1, 2])), None);
        assert_eq!(reversible_dependency_row(&floor, &[0, 0], &[4], 1), Ok(-2));
        let nearest = collection(Some(array(1, vec![-1])), None);
        assert_eq!(
            irreversible_dependency_row(&nearest, &[0, 0], &[32, 32], &[1], 1),
            Ok(-1)
        );
    }

    #[test]
    fn native_mct_matrix_rows_preserve_numeric_values_raw_precision_domains_and_error_order() {
        use crate::codestream::{MctArrayKind, MctNumericFormat};
        let overflow = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidTransform,
        };
        let array = |fraction, values| {
            MctArray::from_values(
                u8::MAX,
                MctArrayKind::Offset,
                MctNumericFormat::Float64,
                fraction,
                values,
            )
        };
        let matrix = array(0, vec![2, -3, 4]);
        let offset = array(0, vec![1]);
        assert_eq!(
            reversible_mct_row(&matrix, Some(&offset), &[5, 7], 0),
            Ok(-10)
        );
        assert_eq!(
            fixed_mct_row_scaled(&matrix, Some(&offset), &[5, 7], &[0, 0], 0),
            Ok(-10_i64 << 32)
        );
        let fixed = array(32, vec![2_i64 << 32, -(3_i64 << 32)]);
        let offset = array(16, vec![1_i64 << 16]);
        assert_eq!(
            fixed_mct_row_scaled(
                &fixed,
                Some(&offset),
                &[5_i64 << 32, 7_i64 << 32],
                &[32, 32],
                0
            ),
            Ok(-10_i64 << 32)
        );
        let one = array(32, vec![1]);
        assert_eq!(fixed_mct_row_scaled(&one, None, &[1], &[1], 0), Ok(1));
        assert_eq!(fixed_mct_row_scaled(&one, None, &[-1], &[1], 0), Ok(-1));
        let empty = array(0, vec![]);
        assert_eq!(reversible_mct_row(&empty, None, &[], usize::MAX), Ok(0));
        assert_eq!(
            fixed_mct_row_scaled(&empty, None, &[], &[], usize::MAX),
            Ok(0)
        );
        assert_eq!(reversible_mct_row(&empty, None, &[1], 0), Err(invalid));
        assert_eq!(
            reversible_mct_row(&matrix, None, &[1, 2], usize::MAX),
            Err(overflow)
        );
        assert_eq!(
            reversible_mct_row(&matrix, Some(&empty), &[1, 2], usize::MAX),
            Err(invalid)
        );
        assert_eq!(
            fixed_mct_row_scaled(&one, Some(&empty), &[1], &[], usize::MAX),
            Err(invalid)
        );
        let wide = array(0, vec![i64::MAX; 3]);
        assert_eq!(
            reversible_mct_row(&wide, None, &[i64::MAX; 3], 0),
            Err(overflow)
        );
        let short = array(32, vec![]);
        assert_eq!(
            fixed_mct_row_scaled(&short, None, &[1], &[u8::MAX], 0),
            Err(invalid)
        );
        assert_eq!(
            fixed_mct_row_scaled(&one, None, &[1], &[u8::MAX], 0),
            Err(overflow)
        );
        for precision in 0..=u8::MAX {
            let zero = array(precision, vec![]);
            assert_eq!(
                reversible_mct_row(&zero, None, &[], usize::MAX),
                if precision == 0 { Ok(0) } else { Err(invalid) }
            );
            assert_eq!(
                fixed_mct_row_scaled(&zero, None, &[], &[], usize::MAX),
                if precision <= 32 { Ok(0) } else { Err(invalid) }
            );
            let zero = array(0, vec![0]);
            assert_eq!(
                fixed_mct_row_scaled(&zero, None, &[0], &[precision], 0),
                if precision <= 158 {
                    Ok(0)
                } else {
                    Err(overflow)
                }
            );
            let offsets = array(precision, vec![0]);
            assert_eq!(
                fixed_mct_row_scaled(&zero, Some(&offsets), &[0], &[0], 0),
                if precision <= 32 { Ok(0) } else { Err(invalid) }
            );
        }
    }

    #[test]
    fn native_mct_round_rescale_offsets_and_work_match_wide_reference_and_full_u8_domains() {
        let overflow = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidTransform,
        };
        let round = |value: i128, bits: u8| {
            if bits == 0 {
                return Ok(value);
            }
            if bits > 126 {
                return Err(overflow);
            }
            let magnitude = value.checked_abs().ok_or(overflow)?;
            let half = 2_i128.pow(u32::from(bits - 1));
            if magnitude > i128::MAX - half {
                return Err(overflow);
            }
            let divisor = 2_i128.pow(u32::from(bits));
            let rounded = magnitude / divisor + i128::from(magnitude % divisor >= half);
            Ok(if value < 0 { -rounded } else { rounded })
        };
        let values = [
            i128::MIN,
            i128::MIN + 1,
            -(1_i128 << 126),
            -3,
            -2,
            -1,
            0,
            1,
            2,
            3,
            1_i128 << 126,
            i128::MAX - 1,
            i128::MAX,
        ];
        for bits in 0..=u8::MAX {
            for value in values {
                assert_eq!(rounded_shift_i128(value, bits), round(value, bits));
            }
            assert_eq!(rescale_i128(i128::MIN, bits, bits), Ok(i128::MIN));
        }
        for from in 0..=u8::MAX {
            for to in 0..=u8::MAX {
                for value in [-3_i128, -1, 0, 1, 3] {
                    let expected = if from == to {
                        Ok(value)
                    } else if from > to {
                        round(value, from - to)
                    } else if to - from > 126 {
                        Err(overflow)
                    } else {
                        value
                            .checked_mul(2_i128.pow(u32::from(to - from)))
                            .ok_or(overflow)
                    };
                    assert_eq!(
                        rescale_i128(value, from, to),
                        expected,
                        "value={value} from={from} to={to}"
                    );
                }
            }
        }
        let samples = vec![i64::MIN, -3, 0, 3, i64::MAX];
        for fraction in 0..=u8::MAX {
            let array = MctArray::from_values(
                u8::MAX,
                crate::codestream::MctArrayKind::Dependency,
                crate::codestream::MctNumericFormat::Float64,
                fraction,
                samples.clone(),
            );
            for row in [0, 1, 4, 5, usize::MAX] {
                let direct = if fraction != 0 {
                    Err(invalid)
                } else {
                    samples.get(row).copied().ok_or(invalid)
                };
                assert_eq!(offset_value(Some(&array), row), direct);
                let scaled = if fraction > 32 {
                    Err(invalid)
                } else {
                    samples
                        .get(row)
                        .map(|value| i128::from(*value) * 2_i128.pow(u32::from(32 - fraction)))
                        .ok_or(invalid)
                };
                assert_eq!(offset_value_scaled(Some(&array), row), scaled);
                assert_eq!(offset_value(None, row), Ok(0));
                assert_eq!(offset_value_scaled(None, row), Ok(0));
            }
        }
        let limits = ResourceLimits::default();
        let maximum = limits.maximum_work_units();
        for before in [0, maximum, maximum + 1, u64::MAX] {
            for operations in [0, 1, 2, u64::MAX] {
                let mut work = before;
                let expected = match before.checked_add(operations) {
                    None => (Err(overflow), before),
                    Some(after) => (
                        if after > maximum {
                            Err(DecodeError::LimitExceeded {
                                resource: Resource::WorkUnits,
                                limit: maximum,
                                actual: after,
                            })
                        } else {
                            Ok(())
                        },
                        after,
                    ),
                };
                assert_eq!(
                    (charge_mct_work(&mut work, operations, &limits), work),
                    expected
                );
            }
        }
        for count in [
            0,
            1,
            limits.maximum_decoded_bytes() as usize / 8,
            limits.maximum_decoded_bytes() as usize / 8 + 1,
            usize::MAX,
        ] {
            let bytes = count as u128 * 8;
            let expected = if bytes > u128::from(u64::MAX) {
                Err(overflow)
            } else if bytes > u128::from(limits.maximum_decoded_bytes()) {
                Err(DecodeError::LimitExceeded {
                    resource: Resource::DecodedBytes,
                    limit: limits.maximum_decoded_bytes(),
                    actual: bytes as u64,
                })
            } else {
                Ok(())
            };
            assert_eq!(check_mct_scratch(count, &limits), expected);
        }
    }

    #[test]
    fn typed_image_admission_preserves_native_count_origin_shape_and_error_priority() {
        fn components(count: usize) -> Vec<TypedComponent> {
            (0..count)
                .map(|_| {
                    TypedComponent::from_parts(
                        0,
                        u32::MAX,
                        ComponentInfo {
                            precision: 0,
                            signed: true,
                            x_subsampling: 0,
                            y_subsampling: 0,
                            x_registration: u16::MAX,
                            y_registration: u16::MAX,
                        },
                        TypedSamples::Unsigned(vec![]),
                    )
                })
                .collect()
        }
        let limits = ResourceLimits::default()
            .with_maximum_decoded_bytes(1)
            .unwrap();
        let overflow = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        for count in [0, 1, 64, 65, 65_535, 65_536] {
            for (x_origin, y_origin) in [(0, 0), (u32::MAX, 0), (0, u32::MAX)] {
                let result = typed_image_from_components_with_origin(
                    1,
                    1,
                    x_origin,
                    y_origin,
                    components(count),
                    &limits,
                );
                if x_origin == u32::MAX || y_origin == u32::MAX || count > usize::from(u16::MAX) {
                    assert_eq!(result, Err(overflow));
                } else if count > usize::from(limits.maximum_components()) {
                    assert_eq!(
                        result,
                        Err(DecodeError::LimitExceeded {
                            resource: Resource::Components,
                            limit: u64::from(limits.maximum_components()),
                            actual: count as u64,
                        })
                    );
                } else {
                    let result = result.unwrap();
                    assert_eq!(result.components().len(), count);
                    for component in result.components() {
                        assert_eq!((component.width(), component.height()), (0, u32::MAX));
                        assert_eq!(component.info().precision, 0);
                        assert_eq!(component.info().x_subsampling, 0);
                        assert!(
                            matches!(component.samples(), TypedSamples::Unsigned(values) if values.is_empty())
                        );
                    }
                }
            }
        }
        let info = ComponentInfo {
            precision: 0,
            signed: false,
            x_subsampling: 0,
            y_subsampling: 0,
            x_registration: 0,
            y_registration: 0,
        };
        let malformed = || TypedComponent::from_parts(1, 1, info, TypedSamples::Signed(vec![]));
        let valid = || TypedComponent::from_parts(1, 1, info, TypedSamples::Signed(vec![7]));
        assert_eq!(
            typed_image_from_components_with_origin(0, 0, 0, 0, vec![malformed()], &limits),
            Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidImageGeometry
            }),
        );
        assert_eq!(
            TypedImage::from_components_with_origin(
                0,
                0,
                0,
                0,
                vec![valid(), malformed()],
                &limits
            ),
            Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: 1,
                actual: 8
            }),
        );
    }
    use crate::codestream::{
        MctArrayKind, MctCollection, MctNumericFormat, MctStage, MctWaveletType,
    };

    fn copy_decoded(image: &DecodedImage) -> DecodedImage {
        DecodedImage {
            width: image.width,
            height: image.height,
            x_origin: image.x_origin,
            y_origin: image.y_origin,
            components: image
                .components
                .iter()
                .map(|component| DecodedComponent {
                    width: component.width,
                    height: component.height,
                    info: component.info,
                    fraction_bits: component.fraction_bits,
                    samples: component.samples.clone(),
                })
                .collect(),
        }
    }

    fn component_reference_values(
        y: i64,
        cb: i64,
        cr: i64,
        irreversible: bool,
    ) -> Result<(i64, i64, i64), DecodeError> {
        let checked = |value: i128| {
            i64::try_from(value)
                .map(i128::from)
                .map_err(|_| arithmetic_overflow())
        };
        let fixed = |value: i64, factor: i128| {
            let product = checked(i128::from(value) * factor)?;
            Ok::<i128, DecodeError>(checked(product + 32768)?.div_euclid(65536))
        };
        let y = i128::from(y);
        if irreversible {
            let red = checked(y + fixed(cr, 91881)?)?;
            let green_first = checked(y - fixed(cb, 22553)?)?;
            let green = checked(green_first - fixed(cr, 46797)?)?;
            let blue = checked(y + fixed(cb, 116130)?)?;
            Ok((red as i64, green as i64, blue as i64))
        } else {
            let sum = checked(i128::from(cb) + i128::from(cr))?;
            let green = checked(y - sum.div_euclid(4))?;
            let red = checked(green + i128::from(cr))?;
            let blue = checked(green + i128::from(cb))?;
            Ok((red as i64, green as i64, blue as i64))
        }
    }

    fn component_reference(
        image: &mut DecodedImage,
        irreversible: bool,
    ) -> Result<(), DecodeError> {
        if image.components.len() < 3 {
            return Err(invalid_image());
        }
        let first = &image.components[0];
        for component in &image.components[..3] {
            if component.width != first.width
                || component.height != first.height
                || component.fraction_bits != if irreversible { 16 } else { 0 }
                || component.info.x_subsampling != first.info.x_subsampling
                || component.info.y_subsampling != first.info.y_subsampling
            {
                return Err(invalid_image());
            }
        }
        let length = first.samples.len();
        if image.components[1].samples.len() != length
            || image.components[2].samples.len() != length
        {
            return Err(invalid_image());
        }
        for index in 0..length {
            let (red, green, blue) = component_reference_values(
                image.components[0].samples[index],
                image.components[1].samples[index],
                image.components[2].samples[index],
                irreversible,
            )?;
            image.components[0].samples[index] = red;
            image.components[1].samples[index] = green;
            image.components[2].samples[index] = blue;
        }
        Ok(())
    }

    fn raw_component_image(samples: [Vec<i64>; 3], fraction_bits: u8) -> DecodedImage {
        let mut components: Vec<_> = samples
            .into_iter()
            .enumerate()
            .map(|(i, samples)| DecodedComponent {
                width: 5,
                height: 7,
                info: ComponentInfo {
                    precision: (8 + i) as u8,
                    signed: i.is_multiple_of(2),
                    x_subsampling: 1,
                    y_subsampling: 1,
                    x_registration: i as u16,
                    y_registration: (i + 3) as u16,
                },
                fraction_bits,
                samples,
            })
            .collect();
        components.push(DecodedComponent {
            width: 0,
            height: 19,
            info: ComponentInfo::default(),
            fraction_bits: 255,
            samples: vec![i64::MIN, i64::MAX, 7],
        });
        DecodedImage {
            width: 31,
            height: 19,
            x_origin: 9,
            y_origin: 7,
            components,
        }
    }

    #[test]
    fn complete_component_images_match_independent_wide_reference() {
        for irreversible in [false, true] {
            for y in [-7, -3, -1, 0, 1, 3, 7] {
                for cb in [-7, -3, -1, 0, 1, 3, 7] {
                    for cr in [-7, -3, -1, 0, 1, 3, 7] {
                        let mut actual = raw_component_image(
                            [
                                vec![y, y * 65536, -y * 32768],
                                vec![cb, cb * 65536, -cb * 32768],
                                vec![cr, cr * 65536, -cr * 32768],
                            ],
                            if irreversible { 16 } else { 0 },
                        );
                        let mut expected = copy_decoded(&actual);
                        let expected_status = component_reference(&mut expected, irreversible);
                        let status = if irreversible {
                            apply_irreversible_component_transform(&mut actual)
                        } else {
                            apply_reversible_component_transform(&mut actual)
                        };
                        assert_eq!((status, actual), (expected_status, expected));
                    }
                }
            }
            for samples in [
                [vec![], vec![], vec![]],
                [vec![10, i64::MIN], vec![2, i64::MAX], vec![6, 0]],
                [vec![10, i64::MAX], vec![2, -4], vec![6, 4]],
                [vec![10, i64::MAX], vec![2, 4], vec![6, -4]],
                [vec![10, 0], vec![2, i64::MAX], vec![6, i64::MIN]],
                [vec![10, i64::MIN], vec![2, 0], vec![6, 0]],
            ] {
                let mut actual = raw_component_image(samples, if irreversible { 16 } else { 0 });
                let mut expected = copy_decoded(&actual);
                let expected_status = component_reference(&mut expected, irreversible);
                let status = if irreversible {
                    apply_irreversible_component_transform(&mut actual)
                } else {
                    apply_reversible_component_transform(&mut actual)
                };
                assert_eq!((status, actual), (expected_status, expected));
            }
        }
    }

    #[test]
    fn complete_component_transform_validation_preserves_error_order_and_state() {
        for irreversible in [false, true] {
            for component_count in 0..3 {
                let mut actual = raw_component_image(
                    [vec![i64::MAX], vec![i64::MAX], vec![1]],
                    if irreversible { 16 } else { 0 },
                );
                actual.components.truncate(component_count);
                let expected = copy_decoded(&actual);
                let status = if irreversible {
                    apply_irreversible_component_transform(&mut actual)
                } else {
                    apply_reversible_component_transform(&mut actual)
                };
                assert_eq!((status, actual), (Err(invalid_image()), expected));
            }
            for variant in 0..6 {
                let mut actual = raw_component_image(
                    [vec![i64::MAX, 0], vec![i64::MAX, 0], vec![1, 0]],
                    if irreversible { 16 } else { 0 },
                );
                match variant {
                    0 => actual.components[1].width += 1,
                    1 => actual.components[2].height += 1,
                    2 => actual.components[0].fraction_bits ^= 1,
                    3 => actual.components[1].info.x_subsampling = 0,
                    4 => actual.components[2].info.y_subsampling = 0,
                    _ => {
                        actual.components[1].samples.pop();
                    }
                }
                let expected = copy_decoded(&actual);
                let status = if irreversible {
                    apply_irreversible_component_transform(&mut actual)
                } else {
                    apply_reversible_component_transform(&mut actual)
                };
                assert_eq!((status, actual), (Err(invalid_image()), expected));
            }
        }
    }

    #[test]
    fn complete_component_pixel_frames_and_rejections_match_independent_reference() {
        for irreversible in [false, true] {
            for samples in [
                [vec![10], vec![2], vec![6]],
                [vec![i64::MAX], vec![i64::MAX], vec![1]],
                [vec![10], vec![2], vec![]],
            ] {
                for index in 0..=1 {
                    let mut actual = raw_component_image(samples.clone(), 255);
                    let mut expected = copy_decoded(&actual);
                    let values = if expected.components[..3]
                        .iter()
                        .any(|component| index >= component.samples.len())
                    {
                        Err(invalid_image())
                    } else {
                        component_reference_values(
                            expected.components[0].samples[index],
                            expected.components[1].samples[index],
                            expected.components[2].samples[index],
                            irreversible,
                        )
                    };
                    let expected_status = match values {
                        Err(error) => Err(error),
                        Ok((red, green, blue)) => {
                            expected.components[0].samples[index] = red;
                            expected.components[1].samples[index] = green;
                            expected.components[2].samples[index] = blue;
                            Ok(())
                        }
                    };
                    let status = apply_component_pixel(&mut actual, index, irreversible);
                    assert_eq!((status, actual), (expected_status, expected));
                }
            }
        }
    }

    fn image_size(components: &[ComponentInfo], width: u32, height: u32) -> ImageSize {
        let mut component_info = [ComponentInfo::default(); 64];
        component_info[..components.len()].copy_from_slice(components);
        ImageSize {
            width,
            height,
            x_origin: 0,
            y_origin: 0,
            components: u16::try_from(components.len()).unwrap(),
            uniform_bit_depth: None,
            uniform_signed: None,
            tile_width: width,
            tile_height: height,
            tile_x_origin: 0,
            tile_y_origin: 0,
            tiles_across: 1,
            tiles_down: 1,
            component_info,
        }
    }

    fn reference_decoded_typed_samples(
        component: &DecodedComponent,
        maximum: u64,
    ) -> Result<TypedSamples, DecodeError> {
        let invalid = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::InvalidImageGeometry,
        };
        let arithmetic = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        let precision = component.info.precision;
        if precision == 0 || precision > 38 {
            return Err(invalid);
        }
        let bytes = component.samples.len() as u128 * 8;
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
        let signed_power = 1_i128 << (precision - 1);
        let unsigned_maximum = (1_i128 << precision) - 1;
        let mut signed = Vec::new();
        let mut unsigned = Vec::new();
        for &sample in &component.samples {
            let rounded = if component.fraction_bits == 0 {
                i128::from(sample)
            } else {
                if component.fraction_bits > 62 {
                    return Err(invalid);
                }
                let biased = i128::from(sample) + (1_i128 << (component.fraction_bits - 1));
                if biased > i128::from(i64::MAX) || biased < i128::from(i64::MIN) {
                    return Err(arithmetic);
                }
                biased >> component.fraction_bits
            };
            if component.info.signed {
                signed.push(i64::try_from(rounded.clamp(-signed_power, signed_power - 1)).unwrap());
            } else {
                let shifted = rounded + signed_power;
                if shifted > i128::from(i64::MAX) || shifted < i128::from(i64::MIN) {
                    return Err(arithmetic);
                }
                unsigned.push(u64::try_from(shifted.clamp(0, unsigned_maximum)).unwrap());
            }
        }
        Ok(if component.info.signed {
            TypedSamples::Signed(signed)
        } else {
            TypedSamples::Unsigned(unsigned)
        })
    }

    fn reference_decoded_typed_image(
        image: &DecodedImage,
        maximum: u64,
    ) -> Result<TypedImage, DecodeError> {
        let mut bytes = 0_u128;
        let mut components = Vec::new();
        for component in &image.components {
            bytes += component.samples.len() as u128 * 8;
            if bytes > u128::from(u64::MAX) {
                return Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::ArithmeticOverflow,
                });
            }
            if bytes > u128::from(maximum) {
                return Err(DecodeError::LimitExceeded {
                    resource: Resource::DecodedBytes,
                    limit: maximum,
                    actual: u64::try_from(bytes).unwrap(),
                });
            }
            components.push(TypedComponent {
                width: component.width,
                height: component.height,
                info: component.info,
                samples: reference_decoded_typed_samples(component, maximum)?,
                channel_definitions: Vec::new(),
            });
        }
        Ok(TypedImage {
            width: image.width,
            height: image.height,
            x_origin: image.x_origin,
            y_origin: image.y_origin,
            components,
        })
    }

    #[test]
    fn complete_decoded_typed_samples_match_independent_wide_reference_all_u8_parameters() {
        let payloads = [vec![], vec![i64::MIN, -65536, -1, 0, 1, i64::MAX]];
        let limits = [
            ResourceLimits::default(),
            ResourceLimits::default()
                .with_maximum_decoded_bytes(1)
                .unwrap(),
            ResourceLimits::default()
                .with_maximum_decoded_bytes(47)
                .unwrap(),
            ResourceLimits::default()
                .with_maximum_decoded_bytes(48)
                .unwrap(),
        ];
        for precision in 0..=u8::MAX {
            for fraction_bits in 0..=u8::MAX {
                for signed in [false, true] {
                    for samples in &payloads {
                        let component = DecodedComponent {
                            width: u32::MAX,
                            height: 0,
                            info: ComponentInfo {
                                precision,
                                signed,
                                x_subsampling: 0,
                                y_subsampling: 255,
                                x_registration: u16::MAX,
                                y_registration: 32768,
                            },
                            fraction_bits,
                            samples: samples.clone(),
                        };
                        for limit in &limits {
                            assert_eq!(
                                component.typed_samples(limit),
                                reference_decoded_typed_samples(
                                    &component,
                                    limit.maximum_decoded_bytes()
                                ),
                                "precision={precision} fraction={fraction_bits} signed={signed} count={} maximum={}",
                                samples.len(),
                                limit.maximum_decoded_bytes()
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn complete_decoded_typed_images_match_all_metadata_and_error_order() {
        for precision in [0, 1, 8, 38, 39, 255] {
            for fraction_bits in [0, 1, 16, 62, 63, 255] {
                for signed in [false, true] {
                    for maximum in [1, 8, 15, 16, 47, 48, 64, 128] {
                        let limits = ResourceLimits::default()
                            .with_maximum_decoded_bytes(maximum)
                            .unwrap();
                        let info = ComponentInfo {
                            precision,
                            signed,
                            x_subsampling: 0,
                            y_subsampling: 255,
                            x_registration: u16::MAX,
                            y_registration: 123,
                        };
                        let image = DecodedImage {
                            width: 0,
                            height: u32::MAX,
                            x_origin: u32::MAX,
                            y_origin: 7,
                            components: vec![
                                DecodedComponent {
                                    width: 7,
                                    height: 0,
                                    info: ComponentInfo {
                                        precision: 8,
                                        ..info
                                    },
                                    fraction_bits: 0,
                                    samples: vec![-1],
                                },
                                DecodedComponent {
                                    width: u32::MAX,
                                    height: 2,
                                    info,
                                    fraction_bits,
                                    samples: vec![0, i64::MAX],
                                },
                                DecodedComponent {
                                    width: 1,
                                    height: 19,
                                    info,
                                    fraction_bits: 255,
                                    samples: vec![],
                                },
                            ],
                        };
                        assert_eq!(
                            image.typed(&limits),
                            reference_decoded_typed_image(&image, maximum)
                        );
                    }
                }
            }
        }
        let limits = ResourceLimits::default()
            .with_maximum_decoded_bytes(1)
            .unwrap();
        let info = ComponentInfo {
            precision: 0,
            ..ComponentInfo::default()
        };
        let component = DecodedComponent {
            width: 0,
            height: 0,
            info,
            fraction_bits: 255,
            samples: vec![i64::MAX],
        };
        assert_eq!(
            component.typed_samples(&limits),
            Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidImageGeometry
            })
        );
        let image = DecodedImage {
            width: 0,
            height: 0,
            x_origin: 9,
            y_origin: 17,
            components: vec![component],
        };
        assert_eq!(
            image.typed(&limits),
            Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: 1,
                actual: 8
            })
        );
        let empty = DecodedImage {
            width: u32::MAX,
            height: 0,
            x_origin: u32::MAX,
            y_origin: 1,
            components: vec![],
        };
        assert_eq!(
            empty.typed(&limits),
            reference_decoded_typed_image(&empty, 1)
        );
    }

    #[test]
    fn decoded_byte_validation_matches_full_native_wide_products() {
        for samples in [0, 1, 8, usize::MAX / 8, usize::MAX / 8 + 1, usize::MAX] {
            for bytes_per_sample in [0, 1, 8, u64::MAX] {
                for maximum in [1, 8, 64] {
                    let limits = ResourceLimits::default()
                        .with_maximum_decoded_bytes(maximum)
                        .unwrap();
                    let bytes = samples as u128 * u128::from(bytes_per_sample);
                    let expected = if bytes > u128::from(u64::MAX) {
                        Err(DecodeError::MalformedInput {
                            offset: 0,
                            kind: MalformedKind::ArithmeticOverflow,
                        })
                    } else if bytes > u128::from(maximum) {
                        Err(DecodeError::LimitExceeded {
                            resource: Resource::DecodedBytes,
                            limit: maximum,
                            actual: u64::try_from(bytes).unwrap(),
                        })
                    } else {
                        Ok(())
                    };
                    assert_eq!(validate_bytes(samples, bytes_per_sample, &limits), expected);
                }
            }
        }
    }

    fn reference_registered(value: u32, step: u32, registration: u16) -> u32 {
        u32::try_from(
            (u128::from(value) * 65536)
                .saturating_sub(u128::from(registration))
                .div_ceil(u128::from(step) * 65536),
        )
        .unwrap()
    }

    #[test]
    fn complete_assembly_matches_independent_registered_coordinate_images() {
        let infos = [
            ComponentInfo {
                precision: 8,
                signed: false,
                x_subsampling: 1,
                y_subsampling: 1,
                x_registration: 0,
                y_registration: 0,
            },
            ComponentInfo {
                precision: 38,
                signed: true,
                x_subsampling: 2,
                y_subsampling: 3,
                x_registration: 65535,
                y_registration: 32768,
            },
            ComponentInfo {
                precision: 0,
                signed: true,
                x_subsampling: 3,
                y_subsampling: 2,
                x_registration: 12345,
                y_registration: 65535,
            },
        ];
        let limits = ResourceLimits::default();
        for origin in 0..=2 {
            for width in 1..=5 {
                for height in 1..=4 {
                    let mut size = image_size(&infos, width, height);
                    size.x_origin = origin;
                    size.y_origin = 2 - origin;
                    for discard in 0..=2 {
                        let scale = 1 << discard;
                        let mut owned = Vec::new();
                        for (component, info) in infos.iter().enumerate() {
                            let step_x = u32::from(info.x_subsampling) * scale;
                            let step_y = u32::from(info.y_subsampling) * scale;
                            let x0 =
                                reference_registered(size.x_origin, step_x, info.x_registration);
                            let y0 =
                                reference_registered(size.y_origin, step_y, info.y_registration);
                            let columns = reference_registered(
                                size.x_origin + width,
                                step_x,
                                info.x_registration,
                            ) - x0;
                            let rows = reference_registered(
                                size.y_origin + height,
                                step_y,
                                info.y_registration,
                            ) - y0;
                            for y in 0..rows {
                                for x in 0..columns {
                                    owned.push((
                                        u16::try_from(component).unwrap(),
                                        x,
                                        y,
                                        [i64::try_from(component).unwrap() * 10000
                                            + i64::from(y) * 100
                                            + i64::from(x)
                                            - 1000],
                                    ));
                                }
                            }
                        }
                        owned.reverse();
                        let tiles: Vec<_> = owned
                            .iter()
                            .map(|(component, x, y, samples)| TileComponent {
                                component: *component,
                                x: *x,
                                y: *y,
                                width: 1,
                                height: 1,
                                fraction_bits: [0, 16, 255][usize::from(*component)],
                                samples,
                            })
                            .collect();
                        for inset in 0..=1 {
                            let region = [
                                size.x_origin + inset.min(width - 1),
                                size.y_origin,
                                size.x_origin + width,
                                size.y_origin + height,
                            ];
                            let header = (
                                region[0].div_ceil(scale),
                                region[1].div_ceil(scale),
                                region[2].div_ceil(scale),
                                region[3].div_ceil(scale),
                            );
                            let mut expected = DecodedImage {
                                width: header.2 - header.0,
                                height: header.3 - header.1,
                                x_origin: header.0,
                                y_origin: header.1,
                                components: Vec::new(),
                            };
                            let mut error = if expected.width == 0 || expected.height == 0 {
                                Some(invalid_image())
                            } else {
                                None
                            };
                            for (component, info) in infos.iter().enumerate() {
                                if error.is_some() {
                                    break;
                                }
                                let step_x = u32::from(info.x_subsampling) * scale;
                                let step_y = u32::from(info.y_subsampling) * scale;
                                let x0 =
                                    reference_registered(region[0], step_x, info.x_registration);
                                let y0 =
                                    reference_registered(region[1], step_y, info.y_registration);
                                let x1 =
                                    reference_registered(region[2], step_x, info.x_registration);
                                let y1 =
                                    reference_registered(region[3], step_y, info.y_registration);
                                if x0 == x1 || y0 == y1 {
                                    error = Some(DecodeError::InvalidSelection {
                                        reason: crate::SelectionError::EmptyComponentIntersection,
                                    });
                                    break;
                                }
                                let full_x0 = reference_registered(
                                    size.x_origin,
                                    step_x,
                                    info.x_registration,
                                );
                                let full_y0 = reference_registered(
                                    size.y_origin,
                                    step_y,
                                    info.y_registration,
                                );
                                let samples = (y0..y1)
                                    .flat_map(|y| {
                                        (x0..x1).map(move |x| {
                                            i64::try_from(component).unwrap() * 10000
                                                + i64::from(y - full_y0) * 100
                                                + i64::from(x - full_x0)
                                                - 1000
                                        })
                                    })
                                    .collect();
                                expected.components.push(DecodedComponent {
                                    width: x1 - x0,
                                    height: y1 - y0,
                                    info: *info,
                                    fraction_bits: [0, 16, 255][component],
                                    samples,
                                });
                            }
                            let expected = error.map_or_else(|| Ok(expected), Err);
                            assert_eq!(
                                assemble_selected_image(&size, &tiles, region, discard, &limits),
                                expected
                            );
                        }
                        if discard == 0 {
                            let mut expected = DecodedImage {
                                width,
                                height,
                                x_origin: size.x_origin,
                                y_origin: size.y_origin,
                                components: Vec::new(),
                            };
                            let mut valid = true;
                            for (component, info) in infos.iter().enumerate() {
                                let x0 = reference_registered(
                                    size.x_origin,
                                    u32::from(info.x_subsampling),
                                    info.x_registration,
                                );
                                let y0 = reference_registered(
                                    size.y_origin,
                                    u32::from(info.y_subsampling),
                                    info.y_registration,
                                );
                                let columns = reference_registered(
                                    size.x_origin + width,
                                    u32::from(info.x_subsampling),
                                    info.x_registration,
                                ) - x0;
                                let rows = reference_registered(
                                    size.y_origin + height,
                                    u32::from(info.y_subsampling),
                                    info.y_registration,
                                ) - y0;
                                valid &= columns > 0 && rows > 0;
                                let samples = (0..rows)
                                    .flat_map(|y| {
                                        (0..columns).map(move |x| {
                                            i64::try_from(component).unwrap() * 10000
                                                + i64::from(y) * 100
                                                + i64::from(x)
                                                - 1000
                                        })
                                    })
                                    .collect();
                                expected.components.push(DecodedComponent {
                                    width: columns,
                                    height: rows,
                                    info: *info,
                                    fraction_bits: [0, 16, 255][component],
                                    samples,
                                });
                            }
                            assert_eq!(
                                assemble_image(&size, &tiles, &limits),
                                if valid {
                                    Ok(expected)
                                } else {
                                    Err(invalid_image())
                                }
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn assembly_preserves_error_order_and_selected_only_sample_coverage() {
        let info = ComponentInfo {
            precision: 8,
            signed: false,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let limits = ResourceLimits::default();
        let small = limits.with_maximum_decoded_bytes(1).unwrap();
        let size = image_size(&[info], 1, 1);
        let tile = TileComponent {
            component: 0,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            fraction_bits: 255,
            samples: &[42],
        };
        let budget = DecodeError::LimitExceeded {
            resource: Resource::DecodedBytes,
            limit: 1,
            actual: 8,
        };
        assert_eq!(
            assemble_image(&size, &[TileComponent { width: 0, ..tile }], &small),
            Err(budget)
        );
        let mut excessive_count = size;
        excessive_count.components = u16::MAX;
        assert_eq!(
            assemble_image(&excessive_count, &[tile], &small),
            Err(budget)
        );
        let mut shifted = size;
        shifted.x_origin = 1;
        assert_eq!(
            assemble_selected_image(&shifted, &[tile], [0, 0, 1, 1], 0, &small),
            Err(budget)
        );
        assert_eq!(
            assemble_selected_image(&shifted, &[tile], [0, 0, 1, 1], 0, &limits),
            Err(arithmetic_overflow())
        );
        let large_tile = TileComponent {
            width: 4,
            height: 4,
            fraction_bits: 16,
            ..tile
        };
        let selected = assemble_selected_image(
            &image_size(&[info], 4, 4),
            &[large_tile],
            [0, 0, 1, 1],
            0,
            &limits,
        )
        .unwrap();
        assert_eq!(
            selected,
            DecodedImage {
                width: 1,
                height: 1,
                x_origin: 0,
                y_origin: 0,
                components: vec![DecodedComponent {
                    width: 1,
                    height: 1,
                    info,
                    fraction_bits: 16,
                    samples: vec![42]
                }]
            }
        );
        assert_eq!(
            assemble_image(&image_size(&[info], 4, 4), &[large_tile], &limits),
            Err(invalid_image())
        );
        let outside = TileComponent {
            x: 2,
            width: 0,
            height: 0,
            samples: &[],
            ..tile
        };
        assert_eq!(
            assemble_selected_image(&size, &[tile, outside], [0, 0, 1, 1], 0, &limits)
                .unwrap()
                .components[0]
                .samples,
            [42]
        );
        assert_eq!(
            assemble_selected_image(
                &size,
                &[
                    tile,
                    TileComponent {
                        fraction_bits: 0,
                        ..outside
                    }
                ],
                [0, 0, 1, 1],
                0,
                &limits
            ),
            Err(invalid_image())
        );
        let overflowing = TileComponent {
            x: u32::MAX,
            width: 1,
            ..outside
        };
        assert_eq!(
            assemble_selected_image(&size, &[tile, overflowing], [0, 0, 1, 1], 0, &limits),
            Err(arithmetic_overflow())
        );
        assert!(
            assemble_selected_image(
                &size,
                &[
                    tile,
                    TileComponent {
                        component: 1,
                        ..overflowing
                    }
                ],
                [0, 0, 1, 1],
                0,
                &limits
            )
            .is_ok()
        );
        assert_eq!(
            assemble_selected_image(&size, &[], [0, 0, 1, 1], 0, &limits),
            Err(DecodeError::InvalidSelection {
                reason: crate::SelectionError::RegionOutsideSelectedTiles
            })
        );
        assert_eq!(assemble_image(&size, &[], &limits), Err(invalid_image()));
        let empty = image_size(&[], 0, 0);
        assert_eq!(
            assemble_image(&empty, &[], &limits),
            Ok(DecodedImage {
                width: 0,
                height: 0,
                x_origin: 0,
                y_origin: 0,
                components: vec![]
            })
        );
        let mut overflow = empty;
        overflow.width = u32::MAX;
        overflow.x_origin = 1;
        assert_eq!(
            assemble_image(&overflow, &[], &limits),
            Err(arithmetic_overflow())
        );
        assert_eq!(
            assemble_selected_image(&size, &[tile], [0, 0, 1, 1], 32, &limits),
            Err(invalid_image())
        );
    }

    #[test]
    fn assembly_byte_charge_matches_wide_integer_complete_counter_state() {
        for count in [0, 1, 7, 8, usize::MAX / 8, usize::MAX / 8 + 1, usize::MAX] {
            for before in [0, 1, u64::MAX - 8, u64::MAX - 7, u64::MAX] {
                for maximum in [0, 1, 8, u64::MAX - 1, u64::MAX] {
                    let wide = count as u128 * 8 + u128::from(before);
                    let expected = if count as u128 * 8 > u128::from(u64::MAX)
                        || wide > u128::from(u64::MAX)
                    {
                        (Err(arithmetic_overflow()), before)
                    } else {
                        let next = u64::try_from(wide).unwrap();
                        (
                            if next > maximum {
                                Err(DecodeError::LimitExceeded {
                                    resource: Resource::DecodedBytes,
                                    limit: maximum,
                                    actual: next,
                                })
                            } else {
                                Ok(())
                            },
                            next,
                        )
                    };
                    let mut actual = before;
                    assert_eq!(
                        (charge_assembly_bytes(count, &mut actual, maximum), actual),
                        expected
                    );
                }
            }
        }
    }

    #[test]
    fn applies_binary_complement_nlt_inverse() {
        let info = ComponentInfo {
            precision: 8,
            signed: true,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let component =
            TypedComponent::from_parts(4, 1, info, TypedSamples::Signed(vec![-128, -64, -1, 7]));
        let mut image =
            TypedImage::from_components(4, 1, vec![component], &ResourceLimits::default()).unwrap();
        let transform = NonlinearPointTransform::for_test(8, true, 3, None, None, Vec::new());
        let extensions = Part2Extensions::with_nonlinear_transform_for_test(transform);
        image
            .apply_nonlinear_transforms(&extensions, &ResourceLimits::default())
            .unwrap();
        assert_eq!(
            image.component(0).unwrap().samples(),
            &TypedSamples::Signed(vec![-1, -65, -128, 7])
        );
    }

    #[test]
    fn applies_lut_nlt_inverse_with_integer_interpolation() {
        let info = ComponentInfo {
            precision: 2,
            signed: false,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let component =
            TypedComponent::from_parts(4, 1, info, TypedSamples::Unsigned(vec![0, 1, 2, 3]));
        let mut image =
            TypedImage::from_components(4, 1, vec![component], &ResourceLimits::default()).unwrap();
        let transform =
            NonlinearPointTransform::for_test(4, false, 2, Some(0), Some(u32::MAX), vec![0, 15]);
        let extensions = Part2Extensions::with_nonlinear_transform_for_test(transform);
        image
            .apply_nonlinear_transforms(&extensions, &ResourceLimits::default())
            .unwrap();
        assert_eq!(
            image.component(0).unwrap().samples(),
            &TypedSamples::Unsigned(vec![0, 4, 8, 11])
        );
        assert_eq!(image.component(0).unwrap().info().precision, 4);
    }

    #[test]
    fn applies_gamma_nlt_inverse_with_fixed_point_parameters() {
        let info = ComponentInfo {
            precision: 8,
            signed: false,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let component =
            TypedComponent::from_parts(3, 1, info, TypedSamples::Unsigned(vec![0, 64, 255]));
        let mut image =
            TypedImage::from_components(3, 1, vec![component], &ResourceLimits::default()).unwrap();
        let one = 1_u32 << 16;
        let transform = NonlinearPointTransform::gamma_for_test(8, false, [one, one, one, one, 0]);
        let extensions = Part2Extensions::with_nonlinear_transform_for_test(transform);
        image
            .apply_nonlinear_transforms(&extensions, &ResourceLimits::default())
            .unwrap();
        assert_eq!(
            image.component(0).unwrap().samples(),
            &TypedSamples::Unsigned(vec![0, 64, 255])
        );
    }

    #[test]
    fn applies_tile_specific_nlt_regions_once() {
        let info = ComponentInfo {
            precision: 8,
            signed: true,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let component =
            TypedComponent::from_parts(4, 1, info, TypedSamples::Signed(vec![-128, -1, -128, -1]));
        let mut image =
            TypedImage::from_components(4, 1, vec![component], &ResourceLimits::default()).unwrap();
        let complement = NonlinearPointTransform::for_test(8, true, 3, None, None, Vec::new());
        let identity = NonlinearPointTransform::for_test(8, true, 0, None, None, Vec::new());
        image
            .apply_nonlinear_regions(
                &[
                    NonlinearRegion {
                        component: 0,
                        x0: 0,
                        y0: 0,
                        x1: 2,
                        y1: 1,
                        transform: &complement,
                    },
                    NonlinearRegion {
                        component: 0,
                        x0: 2,
                        y0: 0,
                        x1: 4,
                        y1: 1,
                        transform: &identity,
                    },
                ],
                &ResourceLimits::default(),
            )
            .unwrap();
        assert_eq!(
            image.component(0).unwrap().samples(),
            &TypedSamples::Signed(vec![-1, -128, -128, -1])
        );
    }

    #[test]
    fn assembles_subsampled_component_tiles() {
        let info = ComponentInfo {
            precision: 8,
            signed: false,
            x_subsampling: 2,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let left = [1];
        let right = [2];
        let image = assemble_image(
            &image_size(&[info], 4, 1),
            &[
                TileComponent {
                    component: 0,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: 0,
                    samples: &left,
                },
                TileComponent {
                    component: 0,
                    x: 1,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: 0,
                    samples: &right,
                },
            ],
            &ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(image.component(0).unwrap().samples(), [1, 2]);
        assert_eq!(
            image
                .component(0)
                .unwrap()
                .typed_samples(&ResourceLimits::default())
                .unwrap(),
            TypedSamples::Unsigned(vec![129, 130])
        );
    }

    #[test]
    fn applies_reversible_component_transform() {
        let info = ComponentInfo {
            precision: 8,
            signed: true,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let y = [10, 20];
        let cb = [1, 2];
        let cr = [3, 4];
        let size = image_size(&[info; 3], 2, 1);
        let mut image = assemble_image(
            &size,
            &[
                TileComponent {
                    component: 0,
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 1,
                    fraction_bits: 0,
                    samples: &y,
                },
                TileComponent {
                    component: 1,
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 1,
                    fraction_bits: 0,
                    samples: &cb,
                },
                TileComponent {
                    component: 2,
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 1,
                    fraction_bits: 0,
                    samples: &cr,
                },
            ],
            &ResourceLimits::default(),
        )
        .unwrap();
        apply_reversible_component_transform(&mut image).unwrap();
        assert_eq!(image.component(0).unwrap().samples(), [12, 23]);
        assert_eq!(image.component(1).unwrap().samples(), [9, 19]);
        assert_eq!(image.component(2).unwrap().samples(), [10, 21]);
    }

    #[test]
    fn applies_deterministic_fixed_point_multiple_component_transform() {
        let info = ComponentInfo {
            precision: 8,
            signed: true,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let first = [65_536];
        let second = [131_072];
        let size = image_size(&[info; 2], 1, 1);
        let mut image = assemble_image(
            &size,
            &[
                TileComponent {
                    component: 0,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: FIXED_BITS,
                    samples: &first,
                },
                TileComponent {
                    component: 1,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: FIXED_BITS,
                    samples: &second,
                },
            ],
            &ResourceLimits::default(),
        )
        .unwrap();
        let matrix = MctArray::from_values(
            1,
            MctArrayKind::Decorrelation,
            MctNumericFormat::Float32,
            FIXED_BITS,
            vec![98_304, 32_768, -65_536, 131_072],
        );
        let offsets = MctArray::from_values(
            2,
            MctArrayKind::Offset,
            MctNumericFormat::Float32,
            FIXED_BITS,
            vec![16_384, -32_768],
        );
        let transform = MultipleComponentTransform::from_arrays(false, 2, matrix, Some(offsets));
        apply_multiple_component_transform(&mut image, &transform, &ResourceLimits::default())
            .unwrap();
        assert_eq!(image.component(0).unwrap().samples(), &[11_811_160_064]);
        assert_eq!(image.component(1).unwrap().samples(), &[10_737_418_240]);
        assert_eq!(image.component(0).unwrap().fraction_bits(), MCT_FIXED_BITS);
    }

    #[test]
    fn applies_ordered_subset_dependency_stages_in_reverse() {
        let info = ComponentInfo {
            precision: 8,
            signed: true,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let first = [10];
        let second = [20];
        let third = [5];
        let mut image = assemble_image(
            &image_size(&[info; 3], 1, 1),
            &[
                TileComponent {
                    component: 0,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: 0,
                    samples: &first,
                },
                TileComponent {
                    component: 1,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: 0,
                    samples: &second,
                },
                TileComponent {
                    component: 2,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: 0,
                    samples: &third,
                },
            ],
            &ResourceLimits::default(),
        )
        .unwrap();
        let identity = |index| {
            MctArray::from_values(
                index,
                MctArrayKind::Decorrelation,
                MctNumericFormat::Int16,
                0,
                vec![1],
            )
        };
        let offset = |index, value| {
            MctArray::from_values(
                index,
                MctArrayKind::Offset,
                MctNumericFormat::Int16,
                0,
                vec![value],
            )
        };
        let transform = MultipleComponentTransform::from_stages(vec![
            MctStage::from_collections(
                1,
                vec![MctCollection::from_arrays(
                    true,
                    vec![0],
                    vec![1],
                    identity(1),
                    Some(offset(2, 2)),
                )],
            ),
            MctStage::from_collections(
                2,
                vec![MctCollection::from_arrays(
                    true,
                    vec![1],
                    vec![2],
                    identity(3),
                    Some(offset(4, 1)),
                )],
            ),
        ]);
        apply_multiple_component_transform(&mut image, &transform, &ResourceLimits::default())
            .unwrap();
        assert_eq!(image.component(0).unwrap().samples(), &[8]);
        assert_eq!(image.component(1).unwrap().samples(), &[6]);
        assert_eq!(image.component(2).unwrap().samples(), &[5]);
    }

    #[test]
    fn applies_reversible_and_irreversible_dependency_collections() {
        let info = ComponentInfo {
            precision: 8,
            signed: true,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let make_image = |first: &[i64], second: &[i64]| {
            assemble_image(
                &image_size(&[info; 2], 1, 1),
                &[
                    TileComponent {
                        component: 0,
                        x: 0,
                        y: 0,
                        width: 1,
                        height: 1,
                        fraction_bits: 0,
                        samples: first,
                    },
                    TileComponent {
                        component: 1,
                        x: 0,
                        y: 0,
                        width: 1,
                        height: 1,
                        fraction_bits: 0,
                        samples: second,
                    },
                ],
                &ResourceLimits::default(),
            )
            .unwrap()
        };
        let reversible_coefficients = MctArray::from_values(
            1,
            MctArrayKind::Dependency,
            MctNumericFormat::Int16,
            0,
            vec![1, 2],
        );
        let reversible = MultipleComponentTransform::from_stages(vec![MctStage::from_collections(
            1,
            vec![MctCollection::from_dependency(
                true,
                vec![0, 1],
                Some(reversible_coefficients),
                None,
            )],
        )]);
        let mut image = make_image(&[10], &[15]);
        apply_multiple_component_transform(&mut image, &reversible, &ResourceLimits::default())
            .unwrap();
        assert_eq!(image.component(0).unwrap().samples(), &[10]);
        assert_eq!(image.component(1).unwrap().samples(), &[20]);

        let irreversible_coefficients = MctArray::from_values(
            1,
            MctArrayKind::Dependency,
            MctNumericFormat::Int16,
            0,
            vec![1],
        );
        let irreversible =
            MultipleComponentTransform::from_stages(vec![MctStage::from_collections(
                1,
                vec![MctCollection::from_dependency(
                    false,
                    vec![0, 1],
                    Some(irreversible_coefficients),
                    None,
                )],
            )]);
        let mut image = make_image(&[2], &[3]);
        apply_multiple_component_transform(&mut image, &irreversible, &ResourceLimits::default())
            .unwrap();
        assert_eq!(
            image.component(0).unwrap().samples(),
            &[2_i64 << MCT_FIXED_BITS]
        );
        assert_eq!(
            image.component(1).unwrap().samples(),
            &[5_i64 << MCT_FIXED_BITS]
        );
    }

    #[test]
    fn synthesizes_reversible_component_wavelet_with_canvas_parity() {
        let info = ComponentInfo {
            precision: 8,
            signed: true,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let low0 = [3];
        let low1 = [7];
        let high = [2];
        let mut image = assemble_image(
            &image_size(&[info; 3], 1, 1),
            &[
                TileComponent {
                    component: 0,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: 0,
                    samples: &low0,
                },
                TileComponent {
                    component: 1,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: 0,
                    samples: &low1,
                },
                TileComponent {
                    component: 2,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: 0,
                    samples: &high,
                },
            ],
            &ResourceLimits::default(),
        )
        .unwrap();
        let transform = MultipleComponentTransform::from_stages(vec![MctStage::from_collections(
            1,
            vec![MctCollection::from_wavelet(
                vec![0, 1, 2],
                vec![0, 1, 2],
                MctWaveletType::Reversible5x3,
                1,
                0,
            )],
        )]);
        apply_multiple_component_transform(&mut image, &transform, &ResourceLimits::default())
            .unwrap();
        assert_eq!(image.component(0).unwrap().samples(), &[2]);
        assert_eq!(image.component(1).unwrap().samples(), &[6]);
        assert_eq!(image.component(2).unwrap().samples(), &[6]);
    }

    #[test]
    fn synthesizes_irreversible_component_wavelet_in_q32() {
        let info = ComponentInfo {
            precision: 8,
            signed: true,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let low0 = [3];
        let low1 = [7];
        let high = [2];
        let mut image = assemble_image(
            &image_size(&[info; 3], 1, 1),
            &[
                TileComponent {
                    component: 0,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: 0,
                    samples: &low0,
                },
                TileComponent {
                    component: 1,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: 0,
                    samples: &low1,
                },
                TileComponent {
                    component: 2,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: 0,
                    samples: &high,
                },
            ],
            &ResourceLimits::default(),
        )
        .unwrap();
        let transform = MultipleComponentTransform::from_stages(vec![MctStage::from_collections(
            1,
            vec![MctCollection::from_wavelet(
                vec![0, 1, 2],
                vec![0, 1, 2],
                MctWaveletType::Irreversible9x7,
                1,
                0,
            )],
        )]);
        let expected = inverse_9_7(
            &[3_i64 << MCT_FIXED_BITS, 7_i64 << MCT_FIXED_BITS],
            &[2_i64 << MCT_FIXED_BITS],
            true,
        )
        .unwrap();
        apply_multiple_component_transform(&mut image, &transform, &ResourceLimits::default())
            .unwrap();
        assert_eq!(image.component(0).unwrap().samples(), &expected[0..1]);
        assert_eq!(image.component(1).unwrap().samples(), &expected[1..2]);
        assert_eq!(image.component(2).unwrap().samples(), &expected[2..3]);
        assert_eq!(image.component(0).unwrap().fraction_bits(), MCT_FIXED_BITS);
    }

    #[test]
    fn rejects_overlapping_or_incomplete_tiles() {
        let info = ComponentInfo {
            precision: 8,
            signed: true,
            x_subsampling: 1,
            y_subsampling: 1,
            x_registration: 0,
            y_registration: 0,
        };
        let samples = [1];
        assert!(
            assemble_image(
                &image_size(&[info], 2, 1),
                &[TileComponent {
                    component: 0,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    fraction_bits: 0,
                    samples: &samples,
                }],
                &ResourceLimits::default(),
            )
            .is_err()
        );
    }
}
