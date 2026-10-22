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

    pub fn typed(&self, limits: &ResourceLimits) -> Result<TypedImage, DecodeError> {
        let mut components = Vec::with_capacity(self.components.len());
        let mut decoded_bytes = 0_u64;
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.components.len(),
                decreases self.components.len() - index,
            )
        )]
        while index < self.components.len() {
            let component = self.components.get(index).ok_or_else(invalid_image)?;
            let bytes = (component.samples.len() as u64)
                .checked_mul(8)
                .ok_or_else(arithmetic_overflow)?;
            decoded_bytes = decoded_bytes
                .checked_add(bytes)
                .ok_or_else(arithmetic_overflow)?;
            if decoded_bytes > limits.maximum_decoded_bytes() {
                return Err(DecodeError::limit(
                    Resource::DecodedBytes,
                    limits.maximum_decoded_bytes(),
                    decoded_bytes,
                ));
            }
            components.push(TypedComponent {
                width: component.width,
                height: component.height,
                info: component.info,
                samples: component.typed_samples(limits)?,
                channel_definitions: Vec::new(),
            });
            index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        Ok(TypedImage {
            width: self.width,
            height: self.height,
            x_origin: self.x_origin,
            y_origin: self.y_origin,
            components,
        })
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
        x_origin
            .checked_add(width)
            .ok_or_else(arithmetic_overflow)?;
        y_origin
            .checked_add(height)
            .ok_or_else(arithmetic_overflow)?;
        let component_count = u16::try_from(components.len()).map_err(|_| arithmetic_overflow())?;
        if component_count > limits.maximum_components() {
            return Err(DecodeError::limit(
                Resource::Components,
                u64::from(limits.maximum_components()),
                components.len() as u64,
            ));
        }
        let mut decoded_bytes = 0_u64;
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= components.len(),
                decreases components.len() - index,
            )
        )]
        while index < components.len() {
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
                return Err(DecodeError::limit(
                    Resource::DecodedBytes,
                    limits.maximum_decoded_bytes(),
                    decoded_bytes,
                ));
            }
            index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        Ok(Self {
            width,
            height,
            x_origin,
            y_origin,
            components,
        })
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

    pub fn component(&self, index: u16) -> Result<&TypedComponent, DecodeError> {
        self.components
            .get(usize::from(index))
            .ok_or_else(invalid_image)
    }
}

impl TypedComponent {
    pub(crate) fn from_parts(
        width: u32,
        height: u32,
        info: ComponentInfo,
        samples: TypedSamples,
    ) -> Self {
        Self {
            width,
            height,
            info,
            samples,
            channel_definitions: Vec::new(),
        }
    }

    pub(crate) fn add_channel_definition(&mut self, definition: ChannelDefinition) {
        self.channel_definitions.push(definition);
    }

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

    pub fn typed_samples(&self, limits: &ResourceLimits) -> Result<TypedSamples, DecodeError> {
        if self.info.precision == 0 || self.info.precision > 38 {
            return Err(invalid_image());
        }
        validate_bytes(self.samples.len(), 8, limits)?;
        let shift = self
            .info
            .precision
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?;
        let signed_min = 0_i64
            .checked_sub(1_i64 << u32::from(shift))
            .ok_or_else(arithmetic_overflow)?;
        let signed_max = (1_i64 << u32::from(shift))
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?;
        let unsigned_max = (1_u64 << u32::from(self.info.precision))
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?;
        if self.info.signed {
            let mut output = Vec::with_capacity(self.samples.len());
            let mut index = 0;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant index <= self.samples.len(),
                    decreases self.samples.len() - index,
                )
            )]
            while index < self.samples.len() {
                let value = round_fixed(self.samples[index], self.fraction_bits)?;
                output.push(value.max(signed_min).min(signed_max));
                index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            Ok(TypedSamples::Signed(output))
        } else {
            let level_shift = 1_i64 << u32::from(shift);
            let mut output = Vec::with_capacity(self.samples.len());
            let mut index = 0;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant index <= self.samples.len(),
                    decreases self.samples.len() - index,
                )
            )]
            while index < self.samples.len() {
                let value = round_fixed(self.samples[index], self.fraction_bits)?
                    .checked_add(level_shift)
                    .ok_or_else(arithmetic_overflow)?;
                output.push(
                    u64::try_from(value.max(0))
                        .map_err(|_| arithmetic_overflow())?
                        .min(unsigned_max),
                );
                index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            Ok(TypedSamples::Unsigned(output))
        }
    }
}

pub fn assemble_image(
    size: &ImageSize,
    tiles: &[TileComponent<'_>],
    limits: &ResourceLimits,
) -> Result<DecodedImage, DecodeError> {
    let mut components = Vec::with_capacity(usize::from(size.components));
    let image_x1 = size
        .x_origin
        .checked_add(size.width)
        .ok_or_else(arithmetic_overflow)?;
    let image_y1 = size
        .y_origin
        .checked_add(size.height)
        .ok_or_else(arithmetic_overflow)?;
    let mut decoded_bytes = 0_u64;
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= size.components,
            decreases size.components - component,
        )
    )]
    while component < size.components {
        let info = size.component(component)?;
        let width =
            registered_ceil_div(image_x1, u32::from(info.x_subsampling), info.x_registration)?
                .checked_sub(registered_ceil_div(
                    size.x_origin,
                    u32::from(info.x_subsampling),
                    info.x_registration,
                )?)
                .ok_or_else(arithmetic_overflow)?;
        let height =
            registered_ceil_div(image_y1, u32::from(info.y_subsampling), info.y_registration)?
                .checked_sub(registered_ceil_div(
                    size.y_origin,
                    u32::from(info.y_subsampling),
                    info.y_registration,
                )?)
                .ok_or_else(arithmetic_overflow)?;
        let sample_count = checked_area(width, height)?;
        let component_bytes = (sample_count as u64)
            .checked_mul(8)
            .ok_or_else(arithmetic_overflow)?;
        decoded_bytes = decoded_bytes
            .checked_add(component_bytes)
            .ok_or_else(arithmetic_overflow)?;
        if decoded_bytes > limits.maximum_decoded_bytes() {
            return Err(DecodeError::limit(
                Resource::DecodedBytes,
                limits.maximum_decoded_bytes(),
                decoded_bytes,
            ));
        }

        /// Assembles a half-open image-reference-grid region from reduced tile planes.
        #[cfg(any())]
        pub(crate) fn assemble_selected_image(
            size: &ImageSize,
            tiles: &[TileComponent<'_>],
            region: [u32; 4],
            discard_levels: u8,
            limits: &ResourceLimits,
        ) -> Result<DecodedImage, DecodeError> {
            if region[0] >= region[2] || region[1] >= region[3] || discard_levels > 31 {
                return Err(invalid_image());
            }
            let scale = 1_u32 << u32::from(discard_levels);
            let output_x0 = ceil_div(region[0], scale)?;
            let output_y0 = ceil_div(region[1], scale)?;
            let output_x1 = ceil_div(region[2], scale)?;
            let output_y1 = ceil_div(region[3], scale)?;
            let output_width = output_x1
                .checked_sub(output_x0)
                .ok_or_else(arithmetic_overflow)?;
            let output_height = output_y1
                .checked_sub(output_y0)
                .ok_or_else(arithmetic_overflow)?;
            if output_width == 0 || output_height == 0 {
                return Err(invalid_image());
            }

            let mut components = Vec::with_capacity(usize::from(size.components));
            let mut decoded_bytes = 0_u64;
            let mut component = 0_u16;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant component <= size.components,
                    decreases size.components - component,
                )
            )]
            while component < size.components {
                let info = size.component(component)?;
                let x_divisor = u32::from(info.x_subsampling)
                    .checked_mul(scale)
                    .ok_or_else(arithmetic_overflow)?;
                let y_divisor = u32::from(info.y_subsampling)
                    .checked_mul(scale)
                    .ok_or_else(arithmetic_overflow)?;
                let component_x0 = ceil_div(region[0], x_divisor)?;
                let component_y0 = ceil_div(region[1], y_divisor)?;
                let component_x1 = ceil_div(region[2], x_divisor)?;
                let component_y1 = ceil_div(region[3], y_divisor)?;
                let width = component_x1
                    .checked_sub(component_x0)
                    .ok_or_else(arithmetic_overflow)?;
                let height = component_y1
                    .checked_sub(component_y0)
                    .ok_or_else(arithmetic_overflow)?;
                if width == 0 || height == 0 {
                    return Err(crate::DecodeError::InvalidSelection {
                        reason: crate::SelectionError::EmptyComponentIntersection,
                    });
                }
                let sample_count = checked_area(width, height)?;
                decoded_bytes = decoded_bytes
                    .checked_add(
                        (sample_count as u64)
                            .checked_mul(8)
                            .ok_or_else(arithmetic_overflow)?,
                    )
                    .ok_or_else(arithmetic_overflow)?;
                if decoded_bytes > limits.maximum_decoded_bytes() {
                    return Err(DecodeError::limit(
                        Resource::DecodedBytes,
                        limits.maximum_decoded_bytes(),
                        decoded_bytes,
                    ));
                }
                let full_component_x0 = ceil_div(size.x_origin, x_divisor)?;
                let full_component_y0 = ceil_div(size.y_origin, y_divisor)?;
                let crop_x = component_x0
                    .checked_sub(full_component_x0)
                    .ok_or_else(arithmetic_overflow)?;
                let crop_y = component_y0
                    .checked_sub(full_component_y0)
                    .ok_or_else(arithmetic_overflow)?;
                let crop_x1 = crop_x.checked_add(width).ok_or_else(arithmetic_overflow)?;
                let crop_y1 = crop_y.checked_add(height).ok_or_else(arithmetic_overflow)?;
                let mut samples = vec![0_i64; sample_count];
                let mut visited = vec![false; sample_count];
                let mut fraction_bits = None;
                let mut tile_index = 0_usize;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(
                        invariant tile_index <= tiles.len(),
                        decreases tiles.len() - tile_index,
                    )
                )]
                while tile_index < tiles.len() {
                    let tile = *tiles.get(tile_index).ok_or_else(invalid_image)?;
                    if tile.component == component {
                        if fraction_bits.is_some() && fraction_bits != Some(tile.fraction_bits) {
                            return Err(invalid_image());
                        }
                        fraction_bits = Some(tile.fraction_bits);
                        let tile_x1 = tile
                            .x
                            .checked_add(tile.width)
                            .ok_or_else(arithmetic_overflow)?;
                        let tile_y1 = tile
                            .y
                            .checked_add(tile.height)
                            .ok_or_else(arithmetic_overflow)?;
                        let x0 = tile.x.max(crop_x);
                        let y0 = tile.y.max(crop_y);
                        let x1 = tile_x1.min(crop_x1);
                        let y1 = tile_y1.min(crop_y1);
                        if x0 < x1 && y0 < y1 {
                            let mut y = y0;
                            while y < y1 {
                                let mut x = x0;
                                while x < x1 {
                                    let destination = index(
                                        width,
                                        x.checked_sub(crop_x).ok_or_else(arithmetic_overflow)?,
                                        y.checked_sub(crop_y).ok_or_else(arithmetic_overflow)?,
                                    )?;
                                    let source = index(
                                        tile.width,
                                        x.checked_sub(tile.x).ok_or_else(arithmetic_overflow)?,
                                        y.checked_sub(tile.y).ok_or_else(arithmetic_overflow)?,
                                    )?;
                                    if destination >= samples.len()
                                        || source >= tile.samples.len()
                                        || *visited.get(destination).ok_or_else(invalid_image)?
                                    {
                                        return Err(invalid_image());
                                    }
                                    *samples.get_mut(destination).ok_or_else(invalid_image)? =
                                        *tile.samples.get(source).ok_or_else(invalid_image)?;
                                    *visited.get_mut(destination).ok_or_else(invalid_image)? = true;
                                    x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
                                }
                                y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
                            }
                        }
                    }
                    tile_index = tile_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
                }
                if fraction_bits.is_none() || visited.iter().any(|value| !value) {
                    return Err(crate::DecodeError::InvalidSelection {
                        reason: crate::SelectionError::RegionOutsideSelectedTiles,
                    });
                }
                components.push(DecodedComponent {
                    width,
                    height,
                    info,
                    fraction_bits: fraction_bits.ok_or_else(invalid_image)?,
                    samples,
                });
                component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            Ok(DecodedImage {
                width: output_width,
                height: output_height,
                x_origin: output_x0,
                y_origin: output_y0,
                components,
            })
        }
        let mut samples = vec![0_i64; sample_count];
        let mut visited = vec![false; sample_count];
        let mut fraction_bits = None;
        let mut tile_index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant tile_index <= tiles.len(),
                decreases tiles.len() - tile_index,
            )
        )]
        while tile_index < tiles.len() {
            let tile = tiles[tile_index];
            if tile.component == component {
                if fraction_bits.is_some() && fraction_bits != Some(tile.fraction_bits) {
                    return Err(invalid_image());
                }
                fraction_bits = Some(tile.fraction_bits);
                place_tile(&mut samples, &mut visited, width, height, tile)?;
            }
            tile_index = tile_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        if fraction_bits.is_none() || visited.iter().any(|visited| !visited) {
            return Err(invalid_image());
        }
        components.push(DecodedComponent {
            width,
            height,
            info,
            fraction_bits: fraction_bits.ok_or_else(invalid_image)?,
            samples,
        });
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(DecodedImage {
        width: size.width,
        height: size.height,
        x_origin: size.x_origin,
        y_origin: size.y_origin,
        components,
    })
}

pub(crate) fn assemble_selected_image(
    size: &ImageSize,
    tiles: &[TileComponent<'_>],
    region: [u32; 4],
    discard_levels: u8,
    limits: &ResourceLimits,
) -> Result<DecodedImage, DecodeError> {
    if region[0] >= region[2] || region[1] >= region[3] || discard_levels > 31 {
        return Err(invalid_image());
    }
    let scale = 1_u32 << u32::from(discard_levels);
    let output_x0 = ceil_div(region[0], scale)?;
    let output_y0 = ceil_div(region[1], scale)?;
    let output_width = ceil_div(region[2], scale)?
        .checked_sub(output_x0)
        .ok_or_else(arithmetic_overflow)?;
    let output_height = ceil_div(region[3], scale)?
        .checked_sub(output_y0)
        .ok_or_else(arithmetic_overflow)?;
    if output_width == 0 || output_height == 0 {
        return Err(invalid_image());
    }
    let mut components = Vec::with_capacity(usize::from(size.components));
    let mut decoded_bytes = 0_u64;
    let mut component = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant component <= size.components,
            decreases size.components - component,
        )
    )]
    while component < size.components {
        let info = size.component(component)?;
        let x_divisor = u32::from(info.x_subsampling)
            .checked_mul(scale)
            .ok_or_else(arithmetic_overflow)?;
        let y_divisor = u32::from(info.y_subsampling)
            .checked_mul(scale)
            .ok_or_else(arithmetic_overflow)?;
        let component_x0 = registered_ceil_div(region[0], x_divisor, info.x_registration)?;
        let component_y0 = registered_ceil_div(region[1], y_divisor, info.y_registration)?;
        let width = registered_ceil_div(region[2], x_divisor, info.x_registration)?
            .checked_sub(component_x0)
            .ok_or_else(arithmetic_overflow)?;
        let height = registered_ceil_div(region[3], y_divisor, info.y_registration)?
            .checked_sub(component_y0)
            .ok_or_else(arithmetic_overflow)?;
        if width == 0 || height == 0 {
            return Err(crate::DecodeError::InvalidSelection {
                reason: crate::SelectionError::EmptyComponentIntersection,
            });
        }
        let sample_count = checked_area(width, height)?;
        decoded_bytes = decoded_bytes
            .checked_add(
                (sample_count as u64)
                    .checked_mul(8)
                    .ok_or_else(arithmetic_overflow)?,
            )
            .ok_or_else(arithmetic_overflow)?;
        if decoded_bytes > limits.maximum_decoded_bytes() {
            return Err(DecodeError::limit(
                Resource::DecodedBytes,
                limits.maximum_decoded_bytes(),
                decoded_bytes,
            ));
        }
        let crop_x = component_x0
            .checked_sub(registered_ceil_div(
                size.x_origin,
                x_divisor,
                info.x_registration,
            )?)
            .ok_or_else(arithmetic_overflow)?;
        let crop_y = component_y0
            .checked_sub(registered_ceil_div(
                size.y_origin,
                y_divisor,
                info.y_registration,
            )?)
            .ok_or_else(arithmetic_overflow)?;
        let crop_x1 = crop_x.checked_add(width).ok_or_else(arithmetic_overflow)?;
        let crop_y1 = crop_y.checked_add(height).ok_or_else(arithmetic_overflow)?;
        let mut samples = vec![0_i64; sample_count];
        let mut visited = vec![false; sample_count];
        let mut fraction_bits = None;
        let mut tile_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant tile_index <= tiles.len(),
                decreases tiles.len() - tile_index,
            )
        )]
        while tile_index < tiles.len() {
            let tile = *tiles.get(tile_index).ok_or_else(invalid_image)?;
            if tile.component == component {
                if fraction_bits.is_some() && fraction_bits != Some(tile.fraction_bits) {
                    return Err(invalid_image());
                }
                fraction_bits = Some(tile.fraction_bits);
                let x0 = tile.x.max(crop_x);
                let y0 = tile.y.max(crop_y);
                let x1 = tile
                    .x
                    .checked_add(tile.width)
                    .ok_or_else(arithmetic_overflow)?
                    .min(crop_x1);
                let y1 = tile
                    .y
                    .checked_add(tile.height)
                    .ok_or_else(arithmetic_overflow)?
                    .min(crop_y1);
                if x0 < x1 && y0 < y1 {
                    let mut y = y0;
                    let mut row_passes = y1.checked_sub(y0).ok_or_else(arithmetic_overflow)?;
                    #[cfg_attr(
                        verus_keep_ghost,
                        verus_spec(decreases row_passes,)
                    )]
                    while y < y1 {
                        let mut x = x0;
                        let mut column_passes =
                            x1.checked_sub(x0).ok_or_else(arithmetic_overflow)?;
                        #[cfg_attr(
                            verus_keep_ghost,
                            verus_spec(decreases column_passes,)
                        )]
                        while x < x1 {
                            let destination = index(
                                width,
                                x.checked_sub(crop_x).ok_or_else(arithmetic_overflow)?,
                                y.checked_sub(crop_y).ok_or_else(arithmetic_overflow)?,
                            )?;
                            let source = index(
                                tile.width,
                                x.checked_sub(tile.x).ok_or_else(arithmetic_overflow)?,
                                y.checked_sub(tile.y).ok_or_else(arithmetic_overflow)?,
                            )?;
                            if destination >= samples.len()
                                || source >= tile.samples.len()
                                || *visited.get(destination).ok_or_else(invalid_image)?
                            {
                                return Err(invalid_image());
                            }
                            *samples.get_mut(destination).ok_or_else(invalid_image)? =
                                *tile.samples.get(source).ok_or_else(invalid_image)?;
                            *visited.get_mut(destination).ok_or_else(invalid_image)? = true;
                            x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
                            column_passes = column_passes
                                .checked_sub(1)
                                .ok_or_else(arithmetic_overflow)?;
                        }
                        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
                        row_passes = row_passes.checked_sub(1).ok_or_else(arithmetic_overflow)?;
                    }
                }
            }
            tile_index = tile_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        if fraction_bits.is_none() || visited.iter().any(|value| !value) {
            return Err(crate::DecodeError::InvalidSelection {
                reason: crate::SelectionError::RegionOutsideSelectedTiles,
            });
        }
        components.push(DecodedComponent {
            width,
            height,
            info,
            fraction_bits: fraction_bits.ok_or_else(invalid_image)?,
            samples,
        });
        component = component.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(DecodedImage {
        width: output_width,
        height: output_height,
        x_origin: output_x0,
        y_origin: output_y0,
        components,
    })
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

fn apply_mct_decorrelation(
    components: &mut [TransformComponentMut<'_>],
    collection: &MctCollection,
    limits: &ResourceLimits,
    work: &mut u64,
) -> Result<(), DecodeError> {
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
    let mut column = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant column <= columns,
            decreases columns - column,
        )
    )]
    while column < columns {
        source_indexes.push(find_transform_component(
            components,
            *collection
                .output_components()
                .get(column)
                .ok_or_else(invalid_transform)?,
        )?);
        column = column.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let mut row = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant row <= rows,
            decreases rows - row,
        )
    )]
    while row < rows {
        destination_indexes.push(find_transform_component(
            components,
            *collection
                .input_components()
                .get(row)
                .ok_or_else(invalid_transform)?,
        )?);
        row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
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
            decreases 2 - list,
        )
    )]
    while list < 2 {
        let indexes = if list == 0 {
            &source_indexes
        } else {
            &destination_indexes
        };
        let mut index_index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index_index <= indexes.len(),
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
    if collection.reversible() {
        let mut index_index = 0_usize;
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
    let operations = (sample_count as u64)
        .checked_mul(rows as u64)
        .and_then(|value| value.checked_mul(columns as u64))
        .and_then(|value| value.checked_add((sample_count as u64).checked_mul(rows as u64)?))
        .ok_or_else(arithmetic_overflow)?;
    *work = work
        .checked_add(operations)
        .ok_or_else(arithmetic_overflow)?;
    if *work > limits.maximum_work_units() {
        return Err(DecodeError::limit(
            Resource::WorkUnits,
            limits.maximum_work_units(),
            *work,
        ));
    }
    let scratch_bytes = rows
        .checked_add(columns)
        .and_then(|value| value.checked_mul(core::mem::size_of::<i64>()))
        .ok_or_else(arithmetic_overflow)? as u64;
    if scratch_bytes > limits.maximum_decoded_bytes() {
        return Err(DecodeError::limit(
            Resource::DecodedBytes,
            limits.maximum_decoded_bytes(),
            scratch_bytes,
        ));
    }
    let mut source_fraction_bits = Vec::with_capacity(source_indexes.len());
    let mut index_index = 0_usize;
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
        source_fraction_bits.push(
            *components
                .get(index)
                .ok_or_else(invalid_transform)?
                .fraction_bits,
        );
        index_index = index_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
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
            *components
                .get_mut(destination)
                .and_then(|component| component.samples.get_mut(sample_index))
                .ok_or_else(invalid_transform)? = *output.get(row).ok_or_else(invalid_transform)?;
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
    let row_start = row
        .checked_mul(row.checked_add(1).ok_or_else(arithmetic_overflow)?)
        .and_then(|value| value.checked_div(2))
        .and_then(|value| value.checked_sub(1))
        .ok_or_else(arithmetic_overflow)?;
    let scale = *coefficients
        .values()
        .get(row_start.checked_add(row).ok_or_else(arithmetic_overflow)?)
        .ok_or_else(invalid_transform)?;
    if scale <= 0 {
        return Err(invalid_transform());
    }
    let mut prediction = i128::from(scale / 2);
    let mut column = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant column <= row,
            decreases row - column,
        )
    )]
    while column < row {
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
    i64::try_from(
        i128::from(base)
            .checked_add(predicted)
            .ok_or_else(arithmetic_overflow)?,
    )
    .map_err(|_| arithmetic_overflow())
}

fn irreversible_dependency_row(
    collection: &MctCollection,
    source: &[i64],
    source_fraction_bits: &[u8],
    output: &[i64],
    row: usize,
) -> Result<i64, DecodeError> {
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
        return i64::try_from(sum).map_err(|_| arithmetic_overflow());
    };
    let row_start = row
        .checked_mul(row.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or_else(arithmetic_overflow)?;
    let mut column = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant column <= row,
            decreases row - column,
        )
    )]
    while column < row {
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
    i64::try_from(sum).map_err(|_| arithmetic_overflow())
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

fn offset_value(offsets: Option<&MctArray>, row: usize) -> Result<i64, DecodeError> {
    match offsets {
        Some(offsets) if offsets.fraction_bits() == 0 => {
            Ok(*offsets.values().get(row).ok_or_else(invalid_transform)?)
        }
        Some(_) => Err(invalid_transform()),
        None => Ok(0),
    }
}

fn offset_value_scaled(offsets: Option<&MctArray>, row: usize) -> Result<i128, DecodeError> {
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

fn charge_mct_work(
    work: &mut u64,
    operations: u64,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    *work = work
        .checked_add(operations)
        .ok_or_else(arithmetic_overflow)?;
    if *work > limits.maximum_work_units() {
        return Err(DecodeError::limit(
            Resource::WorkUnits,
            limits.maximum_work_units(),
            *work,
        ));
    }
    Ok(())
}

fn check_mct_scratch(values: usize, limits: &ResourceLimits) -> Result<(), DecodeError> {
    let bytes = (values as u64)
        .checked_mul(core::mem::size_of::<i64>() as u64)
        .ok_or_else(arithmetic_overflow)?;
    if bytes > limits.maximum_decoded_bytes() {
        return Err(DecodeError::limit(
            Resource::DecodedBytes,
            limits.maximum_decoded_bytes(),
            bytes,
        ));
    }
    Ok(())
}

fn ceil_divide_by_two(value: i64) -> Result<i64, DecodeError> {
    let quotient = value / 2;
    quotient
        .checked_add(if value % 2 > 0 { 1 } else { 0 })
        .ok_or_else(arithmetic_overflow)
}

fn find_transform_component(
    components: &[TransformComponentMut<'_>],
    reference: u16,
) -> Result<usize, DecodeError> {
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= components.len(),
            decreases components.len() - index,
        )
    )]
    while index < components.len() {
        let matches = match components.get(index) {
            Some(component) => component.component == reference,
            None => false,
        };
        if matches {
            return Ok(index);
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
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
    let mut column = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant column <= width,
            decreases width - column,
        )
    )]
    while column < width {
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
    i64::try_from(sum).map_err(|_| arithmetic_overflow())
}

fn fixed_mct_row_scaled(
    matrix: &MctArray,
    offsets: Option<&MctArray>,
    source: &[i64],
    source_fraction_bits: &[u8],
    row: usize,
) -> Result<i64, DecodeError> {
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
    let mut column = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant column <= width,
            decreases width - column,
        )
    )]
    while column < width {
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
    i64::try_from(sum).map_err(|_| arithmetic_overflow())
}

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
    if value < 0 {
        let magnitude = 0_i128
            .checked_sub(value)
            .and_then(|magnitude| magnitude.checked_add(half))
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

fn place_tile(
    output: &mut [i64],
    visited: &mut [bool],
    width: u32,
    height: u32,
    tile: TileComponent<'_>,
) -> Result<(), DecodeError> {
    if tile.width == 0
        || tile.height == 0
        || tile
            .x
            .checked_add(tile.width)
            .ok_or_else(arithmetic_overflow)?
            > width
        || tile
            .y
            .checked_add(tile.height)
            .ok_or_else(arithmetic_overflow)?
            > height
        || tile.samples.len() != checked_area(tile.width, tile.height)?
    {
        return Err(invalid_image());
    }
    let mut y = 0_u32;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant y <= tile.height,
            decreases tile.height - y,
        )
    )]
    while y < tile.height {
        let mut x = 0_u32;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant x <= tile.width,
                decreases tile.width - x,
            )
        )]
        while x < tile.width {
            let destination = index(
                width,
                tile.x.checked_add(x).ok_or_else(arithmetic_overflow)?,
                tile.y.checked_add(y).ok_or_else(arithmetic_overflow)?,
            )?;
            let source = index(tile.width, x, y)?;
            if destination >= output.len()
                || destination >= visited.len()
                || source >= tile.samples.len()
                || visited[destination]
            {
                return Err(invalid_image());
            }
            output[destination] = tile.samples[source];
            visited[destination] = true;
            x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
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

fn ceil_div(value: u32, divisor: u32) -> Result<u32, DecodeError> {
    if divisor == 0 {
        return Err(invalid_image());
    }

    let remainder = value.checked_rem(divisor).ok_or_else(arithmetic_overflow)?;
    (value / divisor)
        .checked_add(u32::from(remainder != 0))
        .ok_or_else(arithmetic_overflow)
}

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
    let quotient = adjusted / denominator;
    let rounded = quotient
        .checked_add(u64::from(!adjusted.is_multiple_of(denominator)))
        .ok_or_else(arithmetic_overflow)?;
    u32::try_from(rounded).map_err(|_| arithmetic_overflow())
}

fn checked_area(width: u32, height: u32) -> Result<usize, DecodeError> {
    usize::try_from(width)
        .map_err(|_| arithmetic_overflow())?
        .checked_mul(usize::try_from(height).map_err(|_| arithmetic_overflow())?)
        .ok_or_else(arithmetic_overflow)
}

fn index(width: u32, x: u32, y: u32) -> Result<usize, DecodeError> {
    usize::try_from(y)
        .map_err(|_| arithmetic_overflow())?
        .checked_mul(usize::try_from(width).map_err(|_| arithmetic_overflow())?)
        .and_then(|row| row.checked_add(usize::try_from(x).ok()?))
        .ok_or_else(arithmetic_overflow)
}

fn validate_bytes(
    samples: usize,
    bytes_per_sample: u64,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let bytes = (samples as u64)
        .checked_mul(bytes_per_sample)
        .ok_or_else(arithmetic_overflow)?;
    if bytes > limits.maximum_decoded_bytes() {
        return Err(DecodeError::limit(
            Resource::DecodedBytes,
            limits.maximum_decoded_bytes(),
            bytes,
        ));
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

fn invalid_transform() -> DecodeError {
    DecodeError::malformed(0, MalformedKind::InvalidTransform)
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
