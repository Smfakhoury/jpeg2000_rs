use crate::{
    DecodeError, MalformedKind, Resource, ResourceLimits,
    codestream::{ArbitraryKernel, KernelCoefficient},
    magnitude::Magnitude,
};

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

const FIXED_BITS: u8 = 16;
const FIXED_ONE: i64 = 1 << FIXED_BITS;

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct QuantizationStep {
    pub exponent: u8,
    pub mantissa: u16,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct WaveletBand<'a> {
    pub width: u32,
    pub height: u32,
    pub samples: &'a [i64],
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct WaveletLevel<'a> {
    pub ll: WaveletBand<'a>,
    pub hl: WaveletBand<'a>,
    pub lh: WaveletBand<'a>,
    pub hh: WaveletBand<'a>,
    pub output_width: u32,
    pub output_height: u32,
    pub low_pass_first_x: bool,
    pub low_pass_first_y: bool,
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub(crate) struct SignedMagnitude {
    pub magnitude: Magnitude,
    pub negative: bool,
}

impl SignedMagnitude {
    pub(crate) const fn new(magnitude: Magnitude, negative: bool) -> Self {
        Self {
            magnitude,
            negative,
        }
    }
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct ReconstructedPlane {
    width: u32,
    height: u32,
    fraction_bits: u8,
    samples: Vec<i64>,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn reconstructed_plane_view(plane: ReconstructedPlane) -> crate::specs::Part1ReconstructedPlane {
        (plane.width, plane.height, plane.fraction_bits, plane.samples@)
    }

    pub open spec fn reconstruction_outcome(result: Result<ReconstructedPlane, DecodeError>)
        -> crate::specs::Part1ReconstructionResult
    {
        match result {
            Ok(plane) => Ok(reconstructed_plane_view(plane)),
            Err(error) => Err(error),
        }
    }

    pub(crate) closed spec fn quantized_coefficients_view(coefficients: Seq<SignedMagnitude>)
        -> Seq<crate::specs::Part1QuantizedCoefficient>
    {
        Seq::new(coefficients.len(), |i: int|
            (crate::magnitude::magnitude_limbs(coefficients[i].magnitude), coefficients[i].negative))
    }

    pub open spec fn sequence_outcome(result: Result<Vec<i64>, DecodeError>) -> Result<Seq<i64>, DecodeError> {
        match result {
            Ok(samples) => Ok(samples@),
            Err(error) => Err(error),
        }
    }

    pub open spec fn wavelet_band_view(band: WaveletBand<'_>) -> crate::specs::Part1WaveletBand {
        (band.width, band.height, band.samples@)
    }

    pub open spec fn row_outcome(result: Result<&[i64], DecodeError>) -> Result<Seq<i64>, DecodeError> {
        match result {
            Ok(samples) => Ok(samples@),
            Err(error) => Err(error),
        }
    }

    pub open spec fn wavelet_level_view(level: WaveletLevel<'_>) -> crate::specs::Part1WaveletLevel {
        (wavelet_band_view(level.ll), wavelet_band_view(level.hl),
            wavelet_band_view(level.lh), wavelet_band_view(level.hh),
            level.output_width, level.output_height, level.low_pass_first_x, level.low_pass_first_y)
    }
}

impl ReconstructedPlane {
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == reconstructed_plane_view(*self).0,
    ))]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == reconstructed_plane_view(*self).1,
    ))]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == reconstructed_plane_view(*self).2,
    ))]
    pub const fn fraction_bits(&self) -> u8 {
        self.fraction_bits
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result@ == reconstructed_plane_view(*self).3,
    ))]
    pub fn samples(&self) -> &[i64] {
        &self.samples
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures reconstruction_outcome(result) == crate::specs::part1_inverse_integer_quantization(
        coefficients@, step.exponent, step.mantissa, dynamic_range, reversible, roi_shift,
        crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
pub fn inverse_quantize(
    coefficients: &[i64],
    step: QuantizationStep,
    dynamic_range: u8,
    reversible: bool,
    roi_shift: u8,
    limits: &ResourceLimits,
) -> Result<ReconstructedPlane, DecodeError> {
    inverse_quantize_values(
        coefficients,
        step,
        dynamic_range,
        reversible,
        roi_shift,
        limits.maximum_decoded_bytes(),
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures reconstruction_outcome(result) == crate::specs::part1_inverse_integer_quantization(
        coefficients@, step.exponent, step.mantissa, dynamic_range, reversible, roi_shift, maximum_decoded_bytes,
    ),
))]
fn inverse_quantize_values(
    coefficients: &[i64],
    step: QuantizationStep,
    dynamic_range: u8,
    reversible: bool,
    roi_shift: u8,
    maximum_decoded_bytes: u64,
) -> Result<ReconstructedPlane, DecodeError> {
    let mut wide = Vec::with_capacity(coefficients.len());
    let mut index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= coefficients.len(),
            wide.len() == index,
            forall |i: int| 0 <= i < index ==> coefficients[i] != i64::MIN,
            forall |i: int| 0 <= i < index ==>
                quantized_coefficients_view(wide@)[i] == crate::specs::part1_integer_coefficients(coefficients@)[i],
            decreases coefficients.len() - index,
        )
    )]
    while index < coefficients.len() {
        let value = coefficients[index];
        if value == i64::MIN {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(exists |i: int| 0 <= i < coefficients.len() && coefficients[i] == i64::MIN);
            }
            return Err(arithmetic_overflow());
        }
        let magnitude = checked_magnitude(value)? as u64;
        wide.push(SignedMagnitude {
            magnitude: crate::magnitude::magnitude_from_u64(magnitude),
            negative: value < 0,
        });
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(quantized_coefficients_view(wide@) =~= crate::specs::part1_integer_coefficients(coefficients@));
    }
    inverse_quantize_plane(
        &wide,
        step,
        dynamic_range,
        reversible,
        roi_shift,
        maximum_decoded_bytes,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures reconstruction_outcome(result) == crate::specs::part1_inverse_quantization(
        quantized_coefficients_view(coefficients@), step.exponent, step.mantissa,
        dynamic_range, reversible, roi_shift, crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
pub(crate) fn inverse_quantize_magnitudes(
    coefficients: &[SignedMagnitude],
    step: QuantizationStep,
    dynamic_range: u8,
    reversible: bool,
    roi_shift: u8,
    limits: &ResourceLimits,
) -> Result<ReconstructedPlane, DecodeError> {
    inverse_quantize_plane(
        coefficients,
        step,
        dynamic_range,
        reversible,
        roi_shift,
        limits.maximum_decoded_bytes(),
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures reconstruction_outcome(result) == crate::specs::part1_inverse_quantization(
        quantized_coefficients_view(coefficients@), step.exponent, step.mantissa,
        dynamic_range, reversible, roi_shift, maximum_decoded_bytes,
    ),
))]
fn inverse_quantize_plane(
    coefficients: &[SignedMagnitude],
    step: QuantizationStep,
    dynamic_range: u8,
    reversible: bool,
    roi_shift: u8,
    maximum_decoded_bytes: u64,
) -> Result<ReconstructedPlane, DecodeError> {
    if dynamic_range == 0 || dynamic_range > 38 || step.mantissa > 0x07ff {
        return Err(invalid_transform());
    }
    if step.exponent > 31 {
        return Err(invalid_transform());
    }
    validate_quantization_size(coefficients.len(), maximum_decoded_bytes)?;
    let step_size = if reversible {
        1
    } else {
        quantization_step_q16(step, dynamic_range)?
    };
    let mut samples = Vec::with_capacity(coefficients.len());
    let mut index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= coefficients.len(),
            samples.len() == index,
            0 < dynamic_range <= 38,
            step.mantissa <= 2047,
            step.exponent <= 31,
            crate::specs::part1_quantization_budget(coefficients.len() as nat, maximum_decoded_bytes) == Ok(()),
            if reversible { step_size == 1 } else {
                crate::specs::part1_quantization_step(step.exponent, step.mantissa, dynamic_range) == Ok(step_size)
            },
            crate::specs::part1_quantized_prefix(
                quantized_coefficients_view(coefficients@), roi_shift, step_size, reversible, index as nat,
            ) == Ok(samples@),
            decreases coefficients.len() - index,
        )
    )]
    while index < coefficients.len() {
        let sample = match quantize_magnitude_sample(
            coefficients[index],
            roi_shift,
            step_size,
            reversible,
        ) {
            Ok(sample) => sample,
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_quantization_error_persists(
                        quantized_coefficients_view(coefficients@), roi_shift, step_size, reversible,
                        index as nat + 1, coefficients.len() as nat, error,
                    );
                }
                return Err(error);
            }
        };
        samples.push(sample);
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    if coefficients.len() > u32::MAX as usize {
        return Err(arithmetic_overflow());
    }
    Ok(ReconstructedPlane {
        width: coefficients.len() as u32,
        height: 1,
        fraction_bits: if reversible { 0 } else { FIXED_BITS },
        samples,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_quantized_sample(
        (crate::magnitude::magnitude_limbs(value.magnitude), value.negative), shift, step_size, reversible,
    ),
))]
fn quantize_magnitude_sample(
    value: SignedMagnitude,
    shift: u8,
    step_size: i64,
    reversible: bool,
) -> Result<i64, DecodeError> {
    let coefficient = undo_maxshift_magnitude(value, shift)?;
    if reversible || coefficient == 0 {
        return Ok(crate::refinement::reversible_inverse_quantize(coefficient));
    }
    let magnitude = checked_magnitude(coefficient)?;
    if magnitude < 0 || step_size < 0 {
        return Err(arithmetic_overflow());
    }
    let centered = crate::refinement::irreversible_centered(magnitude);
    let product = centered
        .checked_mul(step_size as i128)
        .ok_or_else(arithmetic_overflow)?;
    if product < 0 {
        return Err(arithmetic_overflow());
    }
    let scaled = crate::refinement::irreversible_scale(product);
    if scaled > i64::MAX as i128 {
        return Err(arithmetic_overflow());
    }
    let scaled = scaled as i64;
    if coefficient < 0 {
        0_i64.checked_sub(scaled).ok_or_else(arithmetic_overflow)
    } else {
        Ok(scaled)
    }
}

pub fn undo_maxshift(value: i64, shift: u8) -> Result<i64, DecodeError> {
    let magnitude = u64::try_from(checked_magnitude(value)?).map_err(|_| arithmetic_overflow())?;
    let restored = crate::refinement::maxshift_restore(magnitude, shift);
    let restored = i64::try_from(restored).map_err(|_| arithmetic_overflow())?;
    if value < 0 {
        0_i64.checked_sub(restored).ok_or_else(arithmetic_overflow)
    } else {
        Ok(restored)
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_roi_coefficient(
        crate::magnitude::magnitude_limbs(value.magnitude), value.negative, shift,
    ),
))]
pub(crate) fn undo_maxshift_magnitude(
    value: SignedMagnitude,
    shift: u8,
) -> Result<i64, DecodeError> {
    if value.magnitude.is_zero() {
        return Ok(0);
    }
    let threshold = crate::magnitude::magnitude_single_bit(shift as u16)?;
    let restored = if shift == 0 || value.magnitude.less_than(threshold) {
        value.magnitude
    } else {
        value.magnitude.shift_right(shift as u16)
    };
    let magnitude = restored.to_u64().ok_or_else(arithmetic_overflow)?;
    if magnitude > i64::MAX as u64 {
        return Err(arithmetic_overflow());
    }
    let magnitude = magnitude as i64;
    if value.negative {
        0_i64.checked_sub(magnitude).ok_or_else(arithmetic_overflow)
    } else {
        Ok(magnitude)
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures sequence_outcome(result) == crate::specs::part1_inverse_five_three(low@, high@, low_pass_first),
))]
pub fn inverse_5_3(
    low: &[i64],
    high: &[i64],
    low_pass_first: bool,
) -> Result<Vec<i64>, DecodeError> {
    let mut output = interleave(low, high, low_pass_first)?;
    if output.len() == 1 && !low_pass_first {
        output[0] >>= 1;
        return Ok(output);
    }
    inverse_lift(
        &mut output,
        low_pass_first,
        InverseLiftingStep::ReversibleUpdate,
    )?;
    inverse_lift(
        &mut output,
        !low_pass_first,
        InverseLiftingStep::ReversiblePredict,
    )?;
    Ok(output)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures sequence_outcome(result) == crate::specs::part1_inverse_nine_seven(low@, high@, low_pass_first),
))]
pub fn inverse_9_7(
    low: &[i64],
    high: &[i64],
    low_pass_first: bool,
) -> Result<Vec<i64>, DecodeError> {
    let mut output = interleave(low, high, low_pass_first)?;
    if output.len() <= 1 {
        return Ok(output);
    }
    scale_parity(
        &mut output,
        low_pass_first,
        crate::refinement::nine_seven_scale(true),
    )?;
    scale_parity(
        &mut output,
        !low_pass_first,
        crate::refinement::nine_seven_scale(false),
    )?;
    inverse_nine_seven_lifts(&mut output, low_pass_first)?;
    Ok(output)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(samples)@) == crate::specs::part1_nine_seven_lifting_prefix(old(samples)@, low_first, 4),
))]
fn inverse_nine_seven_lifts(samples: &mut [i64], low_first: bool) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original = samples@; }
    let mut stage = 0_u8;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant stage <= 4,
        original == old(samples)@,
        crate::specs::part1_nine_seven_lifting_prefix(original, low_first, stage as nat) == (Ok(()), samples@),
        decreases 4 - stage,
    ))]
    while stage < 4 {
        let target_even = if stage == 0 || stage == 2 {
            low_first
        } else {
            !low_first
        };
        let factor = crate::refinement::nine_seven_lift(stage);
        match inverse_lift(samples, target_even, InverseLiftingStep::Fixed(factor)) {
            Ok(()) => {}
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_nine_seven_error_persists(
                        original, low_first, stage as nat + 1, 4,
                    );
                }
                return Err(error);
            }
        }
        stage += 1;
    }
    Ok(())
}

pub fn inverse_arbitrary(
    low: &[i64],
    high: &[i64],
    low_pass_first: bool,
    kernel: &ArbitraryKernel,
) -> Result<Vec<i64>, DecodeError> {
    let mut output = interleave(low, high, low_pass_first)?;
    if output.len() == 1 {
        if !low_pass_first {
            output[0] = output[0].checked_div(2).ok_or_else(arithmetic_overflow)?;
        }
        return Ok(output);
    }
    if !kernel.reversible() {
        let scaling = coefficient_fixed(
            kernel.scaling().ok_or_else(invalid_transform)?,
            kernel.coefficient_type(),
        )?;
        if scaling == 0 {
            return Err(invalid_transform());
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= output.len(),
                decreases output.len() - index,
            )
        )]
        while index < output.len() {
            let even = (index & 1 == 0) == low_pass_first;
            output[index] = if even {
                fixed_mul_q32(output[index], scaling)?
            } else {
                fixed_div_q32(output[index], scaling)?
            };
            index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
    }
    let mut update_odd = kernel.initial_odd();
    let mut step_index = kernel.steps().len();
    #[cfg_attr(verus_keep_ghost, verus_spec(decreases step_index,))]
    while step_index > 0 {
        step_index = crate::refinement::atk_reconstruction_step(step_index);
        let step = kernel
            .steps()
            .get(step_index)
            .ok_or_else(invalid_transform)?;
        let target_local_odd = if low_pass_first {
            update_odd
        } else {
            !update_odd
        };
        let mut target = usize::from(target_local_odd);
        let mut target_passes = output
            .len()
            .checked_sub(target)
            .and_then(|value| value.checked_add(1))
            .and_then(|value| value.checked_div(2))
            .ok_or_else(arithmetic_overflow)?;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(decreases target_passes,)
        )]
        while target < output.len() {
            if kernel.reversible() {
                let mut sum = coefficient_integer(
                    step.residue().ok_or_else(invalid_transform)?,
                    kernel.coefficient_type(),
                )?;
                let coefficient_count = step.coefficients().len();
                let mut coefficient = 0_usize;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(decreases coefficient_count - coefficient,)
                )]
                while coefficient < coefficient_count {
                    let alpha = coefficient_integer(
                        *step
                            .coefficients()
                            .get(coefficient)
                            .ok_or_else(invalid_transform)?,
                        kernel.coefficient_type(),
                    )?;
                    let contribution = lifting_source_sum(
                        &output,
                        target,
                        coefficient,
                        step.offset(),
                        kernel.whole_sample_filter(),
                        kernel.whole_sample_extension(),
                    )?;
                    sum = sum
                        .checked_add(
                            alpha
                                .checked_mul(i128::from(contribution))
                                .ok_or_else(arithmetic_overflow)?,
                        )
                        .ok_or_else(arithmetic_overflow)?;
                    coefficient = coefficient.checked_add(1).ok_or_else(arithmetic_overflow)?;
                }
                let exponent = u32::from(step.exponent());
                if exponent >= 127 {
                    return Err(invalid_transform());
                }
                let adjustment = sum >> exponent;
                let updated = i128::from(output[target])
                    .checked_sub(adjustment)
                    .ok_or_else(arithmetic_overflow)?;
                output[target] = i64::try_from(updated).map_err(|_| arithmetic_overflow())?;
            } else {
                let mut adjustment = 0_i128;
                let coefficient_count = step.coefficients().len();
                let mut coefficient = 0_usize;
                #[cfg_attr(
                    verus_keep_ghost,
                    verus_spec(decreases coefficient_count - coefficient,)
                )]
                while coefficient < coefficient_count {
                    let alpha = coefficient_fixed(
                        *step
                            .coefficients()
                            .get(coefficient)
                            .ok_or_else(invalid_transform)?,
                        kernel.coefficient_type(),
                    )?;
                    let contribution = lifting_source_sum(
                        &output,
                        target,
                        coefficient,
                        step.offset(),
                        kernel.whole_sample_filter(),
                        kernel.whole_sample_extension(),
                    )?;
                    adjustment = adjustment
                        .checked_add(
                            i128::from(alpha)
                                .checked_mul(i128::from(contribution))
                                .ok_or_else(arithmetic_overflow)?,
                        )
                        .ok_or_else(arithmetic_overflow)?;
                    coefficient = coefficient.checked_add(1).ok_or_else(arithmetic_overflow)?;
                }
                let rounded = round_shift_i128(adjustment, 32)?;
                output[target] = output[target]
                    .checked_sub(i64::try_from(rounded).map_err(|_| arithmetic_overflow())?)
                    .ok_or_else(arithmetic_overflow)?;
            }
            target = target.checked_add(2).ok_or_else(arithmetic_overflow)?;
            target_passes = target_passes
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        update_odd = crate::refinement::atk_next_parity(update_odd);
    }
    Ok(output)
}

fn lifting_source_sum(
    samples: &[i64],
    target: usize,
    coefficient: usize,
    offset: i8,
    whole_sample_filter: bool,
    whole_sample_extension: bool,
) -> Result<i64, DecodeError> {
    let target = isize::try_from(target).map_err(|_| arithmetic_overflow())?;
    let coefficient = isize::try_from(coefficient).map_err(|_| arithmetic_overflow())?;
    if whole_sample_filter {
        let distance = coefficient
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(arithmetic_overflow)?;
        let left = extended_sample(
            samples,
            target
                .checked_sub(distance)
                .ok_or_else(arithmetic_overflow)?,
            whole_sample_extension,
        )?;
        let right = extended_sample(
            samples,
            target
                .checked_add(distance)
                .ok_or_else(arithmetic_overflow)?,
            whole_sample_extension,
        )?;
        left.checked_add(right).ok_or_else(arithmetic_overflow)
    } else {
        let target_parity = target & 1;
        let delta = 1_isize
            .checked_sub(
                2_isize
                    .checked_mul(target_parity)
                    .ok_or_else(arithmetic_overflow)?,
            )
            .and_then(|value| {
                coefficient
                    .checked_add(isize::from(offset))
                    .and_then(|position| position.checked_mul(2))
                    .and_then(|position| value.checked_add(position))
            })
            .ok_or_else(arithmetic_overflow)?;
        extended_sample(
            samples,
            target.checked_add(delta).ok_or_else(arithmetic_overflow)?,
            whole_sample_extension,
        )
    }
}

fn extended_sample(
    samples: &[i64],
    mut index: isize,
    whole_sample_extension: bool,
) -> Result<i64, DecodeError> {
    if samples.len() < 2 {
        return match samples.first() {
            Some(value) => Ok(*value),
            None => Err(invalid_transform()),
        };
    }
    let length = isize::try_from(samples.len()).map_err(|_| arithmetic_overflow())?;
    if whole_sample_extension {
        if index < 0 {
            index = 0_isize.checked_sub(index).ok_or_else(arithmetic_overflow)?;
        }
        if index >= length {
            let period = 2_isize
                .checked_mul(length.checked_sub(1).ok_or_else(arithmetic_overflow)?)
                .ok_or_else(arithmetic_overflow)?;
            let wrapped = index
                .checked_rem_euclid(period)
                .ok_or_else(arithmetic_overflow)?;
            index = if wrapped < length {
                wrapped
            } else {
                period
                    .checked_sub(wrapped)
                    .ok_or_else(arithmetic_overflow)?
            };
        }
    } else {
        let parity = index & 1;
        let first = parity;
        let end = length.checked_sub(1).ok_or_else(arithmetic_overflow)?;
        let last = if end & 1 == parity {
            end
        } else {
            length.checked_sub(2).ok_or_else(arithmetic_overflow)?
        };
        index = if index < first {
            first
        } else if index > last {
            last
        } else {
            index
        };
    }
    match samples.get(usize::try_from(index).map_err(|_| arithmetic_overflow())?) {
        Some(value) => Ok(*value),
        None => Err(invalid_transform()),
    }
}

fn coefficient_integer(
    coefficient: KernelCoefficient,
    coefficient_type: u8,
) -> Result<i128, DecodeError> {
    if coefficient_type > 1 {
        return Err(invalid_transform());
    }
    Ok(bytes_to_u128(coefficient.bytes()) as i128)
}

fn coefficient_fixed(
    coefficient: KernelCoefficient,
    coefficient_type: u8,
) -> Result<i64, DecodeError> {
    let bytes = coefficient.bytes();
    match coefficient_type {
        0 | 1 => {
            let value = coefficient_integer(coefficient, coefficient_type)?;
            i64::try_from(
                value
                    .checked_mul(1_i128 << 32)
                    .ok_or_else(arithmetic_overflow)?,
            )
            .map_err(|_| arithmetic_overflow())
        }
        2 => binary_to_fixed(bytes_to_u128(bytes), 32, 8, 23, 127),
        3 => binary_to_fixed(bytes_to_u128(bytes), 64, 11, 52, 1023),
        4 => binary_to_fixed(bytes_to_u128(bytes), 128, 15, 112, 16383),
        _ => Err(invalid_transform()),
    }
}

fn bytes_to_u128(bytes: &[u8; 16]) -> u128 {
    let mut value = 0_u128;
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= bytes.len(),
            decreases bytes.len() - index,
        )
    )]
    while index < bytes.len() {
        value = (value << 8) | u128::from(bytes[index]);
        index += 1;
    }
    value
}

fn binary_to_fixed(
    bits: u128,
    total_bits: u32,
    exponent_bits: u32,
    fraction_bits: u32,
    bias: i32,
) -> Result<i64, DecodeError> {
    if total_bits == 0
        || total_bits > 128
        || exponent_bits == 0
        || exponent_bits >= 128
        || fraction_bits >= 128
    {
        return Err(invalid_transform());
    }
    let negative = bits >> (total_bits - 1) != 0;
    let exponent_mask = (1_u128 << exponent_bits)
        .checked_sub(1)
        .ok_or_else(arithmetic_overflow)?;
    let exponent = (bits >> fraction_bits) & exponent_mask;
    let fraction = bits
        & (1_u128 << fraction_bits)
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?;
    if exponent == exponent_mask {
        return Err(invalid_transform());
    }
    if exponent == 0 && fraction == 0 {
        return Ok(0);
    }
    let (significand, binary_exponent) = if exponent == 0 {
        (
            fraction,
            1_i32
                .checked_sub(bias)
                .and_then(|value| {
                    value.checked_sub(
                        i32::try_from(fraction_bits)
                            .map_err(|_| arithmetic_overflow())
                            .ok()?,
                    )
                })
                .ok_or_else(arithmetic_overflow)?,
        )
    } else {
        (
            (1_u128 << fraction_bits) | fraction,
            i32::try_from(exponent)
                .map_err(|_| arithmetic_overflow())?
                .checked_sub(bias)
                .and_then(|value| {
                    value.checked_sub(
                        i32::try_from(fraction_bits)
                            .map_err(|_| arithmetic_overflow())
                            .ok()?,
                    )
                })
                .ok_or_else(arithmetic_overflow)?,
        )
    };
    fixed_from_binary_q32(negative, significand, binary_exponent)
}

fn fixed_from_binary_q32(
    negative: bool,
    significand: u128,
    binary_exponent: i32,
) -> Result<i64, DecodeError> {
    let shift = binary_exponent
        .checked_add(32)
        .ok_or_else(arithmetic_overflow)?;
    let magnitude = if shift >= 0 {
        let shift = u32::try_from(shift).map_err(|_| arithmetic_overflow())?;
        if shift >= 128 || significand > (u128::MAX >> shift) {
            return Err(arithmetic_overflow());
        }
        significand << shift
    } else {
        let right = u32::try_from(0_i32.checked_sub(shift).ok_or_else(arithmetic_overflow)?)
            .map_err(|_| arithmetic_overflow())?;
        if right >= 128 {
            0
        } else {
            let truncated = significand >> right;
            let mask = if right == 0 {
                0
            } else {
                (1_u128 << right)
                    .checked_sub(1)
                    .ok_or_else(arithmetic_overflow)?
            };
            let remainder = significand & mask;
            let half = if right == 0 { 0 } else { 1_u128 << (right - 1) };
            truncated
                .checked_add(u128::from(
                    remainder > half || (remainder == half && truncated & 1 != 0),
                ))
                .ok_or_else(arithmetic_overflow)?
        }
    };
    let signed = i128::try_from(magnitude).map_err(|_| arithmetic_overflow())?;
    i64::try_from(if negative {
        0_i128.checked_sub(signed).ok_or_else(arithmetic_overflow)?
    } else {
        signed
    })
    .map_err(|_| arithmetic_overflow())
}

fn fixed_mul_q32(value: i64, factor: i64) -> Result<i64, DecodeError> {
    let product = i128::from(value)
        .checked_mul(i128::from(factor))
        .ok_or_else(arithmetic_overflow)?;
    i64::try_from(round_shift_i128(product, 32)?).map_err(|_| arithmetic_overflow())
}

fn fixed_div_q32(value: i64, divisor: i64) -> Result<i64, DecodeError> {
    if divisor == 0 {
        return Err(arithmetic_overflow());
    }
    let numerator = i128::from(value)
        .checked_mul(1_i128 << 32)
        .ok_or_else(arithmetic_overflow)?;
    if numerator == i128::MIN && divisor == -1 {
        return Err(arithmetic_overflow());
    }
    i64::try_from(numerator / i128::from(divisor)).map_err(|_| arithmetic_overflow())
}

fn round_shift_i128(value: i128, shift: u32) -> Result<i128, DecodeError> {
    if shift == 0 || shift >= 128 {
        return Err(arithmetic_overflow());
    }
    let half = 1_i128 << (shift - 1);
    let adjusted = if value >= 0 {
        value.checked_add(half)
    } else {
        value.checked_sub(half)
    }
    .ok_or_else(arithmetic_overflow)?;
    Ok(adjusted >> shift)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures reconstruction_outcome(result) == crate::specs::part1_inverse_level(
        wavelet_level_view(level), false, crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
pub fn inverse_level_5_3(
    level: WaveletLevel<'_>,
    limits: &ResourceLimits,
) -> Result<ReconstructedPlane, DecodeError> {
    inverse_level(level, limits, false, None)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures reconstruction_outcome(result) == crate::specs::part1_inverse_level(
        wavelet_level_view(level), true, crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
pub fn inverse_level_9_7(
    level: WaveletLevel<'_>,
    limits: &ResourceLimits,
) -> Result<ReconstructedPlane, DecodeError> {
    inverse_level(level, limits, true, None)
}

pub fn inverse_level_arbitrary(
    level: WaveletLevel<'_>,
    kernel: &ArbitraryKernel,
    limits: &ResourceLimits,
) -> Result<ReconstructedPlane, DecodeError> {
    inverse_level(level, limits, !kernel.reversible(), Some(kernel))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures reconstruction_outcome(result) == crate::specs::part1_inverse_horizontal(
        wavelet_band_view(low), wavelet_band_view(high), output_width, low_pass_first, false,
        crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
pub fn inverse_horizontal_5_3(
    low: WaveletBand<'_>,
    high: WaveletBand<'_>,
    output_width: u32,
    low_pass_first: bool,
    limits: &ResourceLimits,
) -> Result<ReconstructedPlane, DecodeError> {
    inverse_horizontal(low, high, output_width, low_pass_first, limits, false, None)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures reconstruction_outcome(result) == crate::specs::part1_inverse_horizontal(
        wavelet_band_view(low), wavelet_band_view(high), output_width, low_pass_first, true,
        crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
pub fn inverse_horizontal_9_7(
    low: WaveletBand<'_>,
    high: WaveletBand<'_>,
    output_width: u32,
    low_pass_first: bool,
    limits: &ResourceLimits,
) -> Result<ReconstructedPlane, DecodeError> {
    inverse_horizontal(low, high, output_width, low_pass_first, limits, true, None)
}

pub fn inverse_horizontal_arbitrary(
    low: WaveletBand<'_>,
    high: WaveletBand<'_>,
    output_width: u32,
    low_pass_first: bool,
    kernel: &ArbitraryKernel,
    limits: &ResourceLimits,
) -> Result<ReconstructedPlane, DecodeError> {
    inverse_horizontal(
        low,
        high,
        output_width,
        low_pass_first,
        limits,
        !kernel.reversible(),
        Some(kernel),
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures kernel.is_none() ==> reconstruction_outcome(result) == crate::specs::part1_inverse_horizontal(
        wavelet_band_view(low), wavelet_band_view(high), output_width, low_pass_first, irreversible,
        crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
fn inverse_horizontal(
    low: WaveletBand<'_>,
    high: WaveletBand<'_>,
    output_width: u32,
    low_pass_first: bool,
    limits: &ResourceLimits,
    irreversible: bool,
    kernel: Option<&ArbitraryKernel>,
) -> Result<ReconstructedPlane, DecodeError> {
    if low.height == 0
        || low.height != high.height
        || low.width != low_count(output_width, low_pass_first)
        || high.width
            != output_width
                .checked_sub(low.width)
                .ok_or_else(arithmetic_overflow)?
    {
        return Err(invalid_transform());
    }
    let output_len = checked_area(output_width, low.height)?;
    validate_output_size(output_len, limits)?;
    let samples = inverse_rows(low, high, low_pass_first, irreversible, kernel, output_len)?;
    Ok(ReconstructedPlane {
        width: output_width,
        height: low.height,
        fraction_bits: if irreversible { FIXED_BITS } else { 0 },
        samples,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures kernel.is_none() ==> sequence_outcome(result) == crate::specs::part1_horizontal_prefix(
        wavelet_band_view(low), wavelet_band_view(high), low_pass_first, irreversible, low.height as nat,
    ),
))]
fn inverse_rows(
    low: WaveletBand<'_>,
    high: WaveletBand<'_>,
    low_pass_first: bool,
    irreversible: bool,
    kernel: Option<&ArbitraryKernel>,
    capacity: usize,
) -> Result<Vec<i64>, DecodeError> {
    let mut samples = Vec::with_capacity(capacity);
    let mut y = 0_u32;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant y <= low.height,
            kernel.is_none() ==> crate::specs::part1_horizontal_prefix(
                wavelet_band_view(low), wavelet_band_view(high), low_pass_first, irreversible, y as nat,
            ) == Ok(samples@),
            decreases low.height - y,
        )
    )]
    while y < low.height {
        let row = match inverse_row_pair(low, high, y, low_pass_first, irreversible, kernel) {
            Ok(row) => row,
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    if kernel.is_none() {
                        crate::proofs::production_impl::production_horizontal_error_persists(
                            wavelet_band_view(low), wavelet_band_view(high), low_pass_first, irreversible,
                            y as nat + 1, low.height as nat, error,
                        );
                    }
                }
                return Err(error);
            }
        };
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost appended = samples@ + row@; }
        #[cfg(verus_keep_ghost)]
        proof! {
            if kernel.is_none() {
                assert(crate::specs::part1_horizontal_prefix(
                    wavelet_band_view(low), wavelet_band_view(high), low_pass_first, irreversible, y as nat + 1,
                ) == Ok(appended));
            }
        }
        samples.extend_from_slice(&row);
        #[cfg(verus_keep_ghost)]
        proof! { assert(samples@ == appended); }
        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(samples)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures kernel.is_none() ==> sequence_outcome(result) == crate::specs::part1_horizontal_row(
        wavelet_band_view(low), wavelet_band_view(high), y, low_first, irreversible,
    ),
))]
fn inverse_row_pair(
    low: WaveletBand<'_>,
    high: WaveletBand<'_>,
    y: u32,
    low_first: bool,
    irreversible: bool,
    kernel: Option<&ArbitraryKernel>,
) -> Result<Vec<i64>, DecodeError> {
    let low_row = band_row(low, y)?;
    let high_row = band_row(high, y)?;
    if let Some(kernel) = kernel {
        inverse_arbitrary(low_row, high_row, low_first, kernel)
    } else if irreversible {
        inverse_9_7(low_row, high_row, low_first)
    } else {
        inverse_5_3(low_row, high_row, low_first)
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures reconstruction_outcome(result) == crate::specs::part1_inverse_vertical(
        wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first, false,
        crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
pub fn inverse_vertical_5_3(
    low: WaveletBand<'_>,
    high: WaveletBand<'_>,
    output_height: u32,
    low_pass_first: bool,
    limits: &ResourceLimits,
) -> Result<ReconstructedPlane, DecodeError> {
    inverse_vertical(
        low,
        high,
        output_height,
        low_pass_first,
        limits,
        false,
        None,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures reconstruction_outcome(result) == crate::specs::part1_inverse_vertical(
        wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first, true,
        crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
pub fn inverse_vertical_9_7(
    low: WaveletBand<'_>,
    high: WaveletBand<'_>,
    output_height: u32,
    low_pass_first: bool,
    limits: &ResourceLimits,
) -> Result<ReconstructedPlane, DecodeError> {
    inverse_vertical(low, high, output_height, low_pass_first, limits, true, None)
}

pub fn inverse_vertical_arbitrary(
    low: WaveletBand<'_>,
    high: WaveletBand<'_>,
    output_height: u32,
    low_pass_first: bool,
    kernel: &ArbitraryKernel,
    limits: &ResourceLimits,
) -> Result<ReconstructedPlane, DecodeError> {
    inverse_vertical(
        low,
        high,
        output_height,
        low_pass_first,
        limits,
        !kernel.reversible(),
        Some(kernel),
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures kernel.is_none() ==> reconstruction_outcome(result) == crate::specs::part1_inverse_vertical(
        wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first, irreversible,
        crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
fn inverse_vertical(
    low: WaveletBand<'_>,
    high: WaveletBand<'_>,
    output_height: u32,
    low_pass_first: bool,
    limits: &ResourceLimits,
    irreversible: bool,
    kernel: Option<&ArbitraryKernel>,
) -> Result<ReconstructedPlane, DecodeError> {
    if low.width == 0
        || low.width != high.width
        || low.height != low_count(output_height, low_pass_first)
        || high.height
            != output_height
                .checked_sub(low.height)
                .ok_or_else(arithmetic_overflow)?
    {
        return Err(invalid_transform());
    }
    let output_len = checked_area(low.width, output_height)?;
    validate_output_size(output_len, limits)?;
    let samples = inverse_columns(
        low,
        high,
        output_height,
        low_pass_first,
        irreversible,
        kernel,
        output_len,
    )?;
    Ok(ReconstructedPlane {
        width: low.width,
        height: output_height,
        fraction_bits: if irreversible { FIXED_BITS } else { 0 },
        samples,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures kernel.is_none() ==> sequence_outcome(result) == crate::specs::part1_vertical_prefix(
        wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first, irreversible,
        output_len, low.width as nat,
    ),
))]
fn inverse_columns(
    low: WaveletBand<'_>,
    high: WaveletBand<'_>,
    output_height: u32,
    low_pass_first: bool,
    irreversible: bool,
    kernel: Option<&ArbitraryKernel>,
    output_len: usize,
) -> Result<Vec<i64>, DecodeError> {
    let mut samples = vec![0_i64; output_len];
    #[cfg(verus_keep_ghost)]
    proof! { assert(samples@ =~= Seq::new(output_len as nat, |i: int| 0i64)); }
    let mut x = 0_u32;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant x <= low.width,
            kernel.is_none() ==> crate::specs::part1_vertical_prefix(
                wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first, irreversible,
                output_len, x as nat,
            ) == Ok(samples@),
            decreases low.width - x,
        )
    )]
    while x < low.width {
        let column = match inverse_column_pair(low, high, x, low_pass_first, irreversible, kernel) {
            Ok(column) => column,
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    if kernel.is_none() {
                        crate::proofs::production_impl::production_vertical_error_persists(
                            wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first,
                            irreversible, output_len, x as nat + 1, low.width as nat, error,
                        );
                    }
                }
                return Err(error);
            }
        };
        match store_column(&mut samples, low.width, output_height, x, &column) {
            Ok(()) => {}
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    if kernel.is_none() {
                        crate::proofs::production_impl::production_vertical_error_persists(
                            wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first,
                            irreversible, output_len, x as nat + 1, low.width as nat, error,
                        );
                    }
                }
                return Err(error);
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            if kernel.is_none() {
                assert(crate::specs::part1_vertical_prefix(
                    wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first, irreversible,
                    output_len, x as nat + 1,
                ) == Ok(samples@));
            }
        }
        x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(samples)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures kernel.is_none() ==> sequence_outcome(result) == crate::specs::part1_vertical_column(
        wavelet_band_view(low), wavelet_band_view(high), x, low_pass_first, irreversible,
    ),
))]
fn inverse_column_pair(
    low: WaveletBand<'_>,
    high: WaveletBand<'_>,
    x: u32,
    low_pass_first: bool,
    irreversible: bool,
    kernel: Option<&ArbitraryKernel>,
) -> Result<Vec<i64>, DecodeError> {
    let low_column = collect_column(low.samples, low.width, low.height, x)?;
    let high_column = collect_column(high.samples, high.width, high.height, x)?;
    if let Some(kernel) = kernel {
        inverse_arbitrary(&low_column, &high_column, low_pass_first, kernel)
    } else if irreversible {
        inverse_9_7(&low_column, &high_column, low_pass_first)
    } else {
        inverse_5_3(&low_column, &high_column, low_pass_first)
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures kernel.is_none() ==> reconstruction_outcome(result) == crate::specs::part1_inverse_level(
        wavelet_level_view(level), irreversible, crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
fn inverse_level(
    level: WaveletLevel<'_>,
    limits: &ResourceLimits,
    irreversible: bool,
    kernel: Option<&ArbitraryKernel>,
) -> Result<ReconstructedPlane, DecodeError> {
    validate_level(level)?;
    let output_len = checked_area(level.output_width, level.output_height)?;
    if !crate::refinement::wavelet_sample_count(level.output_width, level.output_height, output_len)
    {
        return Err(invalid_transform());
    }
    validate_output_size(output_len, limits)?;
    let low_height = level.ll.height;
    let high_height = level.lh.height;
    let low_capacity = checked_area(level.output_width, low_height)?;
    let low_rows = inverse_rows(
        level.ll,
        level.hl,
        level.low_pass_first_x,
        irreversible,
        kernel,
        low_capacity,
    )?;
    let high_capacity = checked_area(level.output_width, high_height)?;
    let high_rows = inverse_rows(
        level.lh,
        level.hh,
        level.low_pass_first_x,
        irreversible,
        kernel,
        high_capacity,
    )?;
    let output = inverse_columns(
        WaveletBand {
            width: level.output_width,
            height: low_height,
            samples: &low_rows,
        },
        WaveletBand {
            width: level.output_width,
            height: high_height,
            samples: &high_rows,
        },
        level.output_height,
        level.low_pass_first_y,
        irreversible,
        kernel,
        output_len,
    )?;
    Ok(ReconstructedPlane {
        width: level.output_width,
        height: level.output_height,
        fraction_bits: if irreversible { FIXED_BITS } else { 0 },
        samples: output,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures sequence_outcome(result) == crate::specs::part1_convert_samples(
        reconstructed_plane_view(*plane), precision, signed, crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
pub fn convert_samples(
    plane: &ReconstructedPlane,
    precision: u8,
    signed: bool,
    limits: &ResourceLimits,
) -> Result<Vec<i64>, DecodeError> {
    if precision == 0 || precision > 38 || plane.fraction_bits > 62 {
        return Err(invalid_transform());
    }
    validate_output_size(plane.samples.len(), limits)?;
    let signed_shift = precision.checked_sub(1).ok_or_else(arithmetic_overflow)?;
    let maximum_shift = if signed { signed_shift } else { precision };
    let maximum_power = 1_i64 << (maximum_shift as u32);
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(maximum_power > 0) by(bit_vector)
            requires maximum_shift <= 38, maximum_power == 1i64 << (maximum_shift as u32);
    }
    let maximum = maximum_power
        .checked_sub(1)
        .ok_or_else(arithmetic_overflow)?;
    let minimum = if signed {
        0_i64
            .checked_sub(1_i64 << (signed_shift as u32))
            .ok_or_else(arithmetic_overflow)?
    } else {
        0
    };
    let level_shift = crate::refinement::level_shift(precision, signed);
    let mut converted = Vec::with_capacity(plane.samples.len());
    let mut index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= plane.samples.len(),
            0 < precision <= 38,
            plane.fraction_bits <= 62,
            crate::specs::part1_quantization_budget(
                plane.samples.len() as nat, crate::limits::resource_decoded_bytes_limit(limits),
            ) == Ok(()),
            maximum == crate::specs::part1_output_maximum(precision, signed),
            minimum == crate::specs::part1_output_minimum(precision, signed),
            level_shift == crate::specs::part1_level_shift(precision, signed),
            crate::specs::part1_conversion_prefix(
                plane.samples@, plane.fraction_bits, level_shift, minimum, maximum, index as nat,
            ) == Ok(converted@),
            decreases plane.samples.len() - index,
        )
    )]
    while index < plane.samples.len() {
        let value = match convert_sample(
            plane.samples[index],
            plane.fraction_bits,
            level_shift,
            minimum,
            maximum,
        ) {
            Ok(value) => value,
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_conversion_error_persists(
                        plane.samples@, plane.fraction_bits, level_shift, minimum, maximum,
                        index as nat + 1, plane.samples.len() as nat, error,
                    );
                }
                return Err(error);
            }
        };
        converted.push(value);
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(converted)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_convert_sample(value, fraction_bits, shift, minimum, maximum),
))]
fn convert_sample(
    value: i64,
    fraction_bits: u8,
    shift: i64,
    minimum: i64,
    maximum: i64,
) -> Result<i64, DecodeError> {
    let rounded = round_fixed(value, fraction_bits)?;
    let shifted = rounded.checked_add(shift).ok_or_else(arithmetic_overflow)?;
    Ok(crate::refinement::clip_sample(shifted, minimum, maximum))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_validate_level(wavelet_level_view(level)),
))]
fn validate_level(level: WaveletLevel<'_>) -> Result<(), DecodeError> {
    if level.output_width == 0 || level.output_height == 0 {
        return Err(invalid_transform());
    }
    let low_width = low_count(level.output_width, level.low_pass_first_x);
    let high_width = level
        .output_width
        .checked_sub(low_width)
        .ok_or_else(arithmetic_overflow)?;
    let low_height = low_count(level.output_height, level.low_pass_first_y);
    let high_height = level
        .output_height
        .checked_sub(low_height)
        .ok_or_else(arithmetic_overflow)?;
    validate_band(level.ll, low_width, low_height)?;
    validate_band(level.hl, high_width, low_height)?;
    validate_band(level.lh, low_width, high_height)?;
    validate_band(level.hh, high_width, high_height)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_validate_band(wavelet_band_view(band), width, height),
))]
fn validate_band(band: WaveletBand<'_>, width: u32, height: u32) -> Result<(), DecodeError> {
    if band.width != width
        || band.height != height
        || band.samples.len() != checked_area(width, height)?
    {
        return Err(invalid_transform());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_low_count(length, low_pass_first),
))]
fn low_count(length: u32, low_pass_first: bool) -> u32 {
    if low_pass_first {
        length / 2 + length % 2
    } else {
        length / 2
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures row_outcome(result) == crate::specs::part1_band_row(wavelet_band_view(band), y),
))]
fn band_row(band: WaveletBand<'_>, y: u32) -> Result<&[i64], DecodeError> {
    if y >= band.height {
        return Err(invalid_transform());
    }
    let start = transform_dimension(y)?
        .checked_mul(transform_dimension(band.width)?)
        .ok_or_else(arithmetic_overflow)?;
    let end = start
        .checked_add(transform_dimension(band.width)?)
        .ok_or_else(arithmetic_overflow)?;
    band.samples.get(start..end).ok_or_else(invalid_transform)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures sequence_outcome(result) == crate::specs::part1_collect_column((width, height, samples@), x),
))]
fn collect_column(
    samples: &[i64],
    width: u32,
    height: u32,
    x: u32,
) -> Result<Vec<i64>, DecodeError> {
    if x >= width {
        return Err(invalid_transform());
    }
    let mut column = Vec::with_capacity(transform_dimension(height)?);
    let mut y = 0_u32;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant y <= height,
            x < width,
            height <= usize::MAX,
            crate::specs::part1_column_prefix(samples@, width, x, y as nat) == Ok(column@),
            decreases height - y,
        )
    )]
    while y < height {
        let value = match column_sample(samples, width, x, y) {
            Ok(value) => value,
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_column_error_persists(
                        samples@, width, x, y as nat + 1, height as nat, error,
                    );
                }
                return Err(error);
            }
        };
        column.push(value);
        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(column)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_offset(y, width, x),
))]
fn transform_offset(y: u32, width: u32, x: u32) -> Result<usize, DecodeError> {
    let row = transform_dimension(y)?
        .checked_mul(transform_dimension(width)?)
        .ok_or_else(arithmetic_overflow)?;
    row.checked_add(transform_dimension(x)?)
        .ok_or_else(arithmetic_overflow)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_column_sample(samples@, width, x, y),
))]
fn column_sample(samples: &[i64], width: u32, x: u32, y: u32) -> Result<i64, DecodeError> {
    let index = transform_offset(y, width, x)?;
    if index >= samples.len() {
        return Err(invalid_transform());
    }
    Ok(samples[index])
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(output)@) == crate::specs::part1_store_column_sample(
        old(output)@, width, x, column@, y,
    ),
))]
fn store_column_sample(
    output: &mut [i64],
    width: u32,
    x: u32,
    column: &[i64],
    y: u32,
) -> Result<(), DecodeError> {
    let destination = transform_offset(y, width, x)?;
    let column_index = transform_dimension(y)?;
    let value = *column.get(column_index).ok_or_else(invalid_transform)?;
    if destination >= output.len() {
        return Err(invalid_transform());
    }
    output[destination] = value;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(output)@) == crate::specs::part1_store_column_prefix(
        old(output)@, width, x, column@, height as nat,
    ),
))]
fn store_column(
    output: &mut [i64],
    width: u32,
    height: u32,
    x: u32,
    column: &[i64],
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original = output@; }
    let mut y = 0_u32;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant y <= height,
        original == old(output)@,
        crate::specs::part1_store_column_prefix(original, width, x, column@, y as nat) == (Ok(()), output@),
        decreases height - y,
    ))]
    while y < height {
        match store_column_sample(output, width, x, column, y) {
            Ok(()) => {}
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_store_column_error_persists(
                        original, width, x, column@, y as nat + 1, height as nat,
                    );
                }
                return Err(error);
            }
        }
        y += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures sequence_outcome(result) == crate::specs::part1_interleaved(low@, high@, low_pass_first),
))]
fn interleave(low: &[i64], high: &[i64], low_pass_first: bool) -> Result<Vec<i64>, DecodeError> {
    let length = low
        .len()
        .checked_add(high.len())
        .ok_or_else(arithmetic_overflow)?;
    let expected_low = if low_pass_first {
        length / 2 + length % 2
    } else {
        length / 2
    };
    if low.len() != expected_low || high.len() != length - expected_low {
        return Err(invalid_transform());
    }
    let mut output = Vec::with_capacity(length);
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= length,
            length == low.len() + high.len(),
            low.len() == if low_pass_first { length as int / 2 + length as int % 2 } else { length as int / 2 },
            high.len() == length - low.len(),
            output.len() == index,
            forall |i: int| 0 <= i < index ==>
                output[i] == if (i % 2 == 0) == low_pass_first { low[i / 2] } else { high[i / 2] },
            decreases length - index,
        )
    )]
    while index < length {
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_index_parity(index); }
        let low_position = (index & 1 == 0) == low_pass_first;
        let source_index = index / 2;
        let value = if low_position {
            *low.get(source_index).ok_or_else(invalid_transform)?
        } else {
            *high.get(source_index).ok_or_else(invalid_transform)?
        };
        output.push(value);
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(output@ =~= Seq::new(length as nat, |i: int|
            if (i % 2 == 0) == low_pass_first { low@[i / 2] } else { high@[i / 2] }));
    }
    Ok(output)
}

#[derive(Clone, Copy)]
enum InverseLiftingStep {
    ReversibleUpdate,
    ReversiblePredict,
    Fixed(i64),
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn lifting_step_view(step: InverseLiftingStep) -> (u8, i64) {
        match step {
            InverseLiftingStep::ReversibleUpdate => (0, 0),
            InverseLiftingStep::ReversiblePredict => (1, 0),
            InverseLiftingStep::Fixed(factor) => (2, factor),
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(samples)@) == crate::specs::part1_lifting_prefix(
        old(samples)@, target_even, lifting_step_view(step).0, lifting_step_view(step).1, old(samples).len() as nat,
    ),
))]
fn inverse_lift(
    samples: &mut [i64],
    target_even: bool,
    step: InverseLiftingStep,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original = samples@; }
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= samples.len(),
            original == old(samples)@,
            samples.len() == original.len(),
            forall |i: int| 0 <= i < samples.len() && (i >= index || (i % 2 == 0) != target_even) ==>
                samples[i] == original[i],
            crate::specs::part1_lifting_prefix(
                original, target_even, lifting_step_view(step).0, lifting_step_view(step).1, index as nat,
            ) == (Ok(()), samples@),
            decreases samples.len() - index,
        )
    )]
    while index < samples.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_lifting_sample_frame(
                original, samples@, index as nat, target_even, lifting_step_view(step).0, lifting_step_view(step).1,
            );
        }
        let value = match inverse_lifting_value(samples, index, target_even, step) {
            Ok(value) => value,
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(crate::specs::part1_lifting_prefix(
                        original, target_even, lifting_step_view(step).0, lifting_step_view(step).1, index as nat + 1,
                    ) == (Err(error), samples@));
                    crate::proofs::production_impl::production_lifting_error_persists(
                        original, target_even, lifting_step_view(step).0, lifting_step_view(step).1,
                        index as nat + 1, original.len(),
                    );
                }
                return Err(error);
            }
        };
        samples[index] = value;
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    requires index < samples.len(),
    ensures result == crate::specs::part1_lifting_sample(
        samples@, index as nat, target_even, lifting_step_view(step).0, lifting_step_view(step).1,
    ),
))]
fn inverse_lifting_value(
    samples: &[i64],
    index: usize,
    target_even: bool,
    step: InverseLiftingStep,
) -> Result<i64, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { crate::proofs::production_impl::production_index_parity(index); }
    if (index & 1 == 0) != target_even || samples.len() <= 1 {
        return Ok(samples[index]);
    }
    let left = if index == 0 {
        samples[1]
    } else {
        samples[index - 1]
    };
    let right = if index + 1 < samples.len() {
        samples[index + 1]
    } else {
        samples[index - 1]
    };
    let sum = left.checked_add(right).ok_or_else(arithmetic_overflow)?;
    let adjustment = match step {
        InverseLiftingStep::Fixed(coefficient) => fixed_mul(sum, coefficient)?,
        InverseLiftingStep::ReversibleUpdate => {
            let biased_sum = sum.checked_add(2).ok_or_else(arithmetic_overflow)?;
            crate::refinement::five_three_update(biased_sum)
        }
        InverseLiftingStep::ReversiblePredict => crate::refinement::five_three_predict(sum),
    };
    if matches!(step, InverseLiftingStep::ReversiblePredict) {
        samples[index]
            .checked_add(adjustment)
            .ok_or_else(arithmetic_overflow)
    } else {
        samples[index]
            .checked_sub(adjustment)
            .ok_or_else(arithmetic_overflow)
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, final(samples)@) == crate::specs::part1_scaling_prefix(
        old(samples)@, low_parity, factor, old(samples).len() as nat,
    ),
))]
fn scale_parity(samples: &mut [i64], low_parity: bool, factor: i64) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original = samples@; }
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= samples.len(),
            original == old(samples)@,
            samples.len() == original.len(),
            forall |i: int| index <= i < samples.len() ==> samples[i] == original[i],
            crate::specs::part1_scaling_prefix(original, low_parity, factor, index as nat) == (Ok(()), samples@),
            decreases samples.len() - index,
        )
    )]
    while index < samples.len() {
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_index_parity(index); }
        if (index & 1 == 0) == low_parity {
            let value = match fixed_mul(samples[index], factor) {
                Ok(value) => value,
                Err(error) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        crate::proofs::production_impl::production_scaling_error_persists(
                            original, low_parity, factor, index as nat + 1, original.len(),
                        );
                    }
                    return Err(error);
                }
            };
            samples[index] = value;
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_fixed_product(value, factor),
))]
fn fixed_mul(value: i64, factor: i64) -> Result<i64, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { assert(FIXED_ONE == 65536) by(compute); }
    let product = value.checked_mul(factor).ok_or_else(arithmetic_overflow)?;
    let adjusted = product
        .checked_add(FIXED_ONE / 2)
        .ok_or_else(arithmetic_overflow)?;
    Ok(crate::refinement::fixed_round_q16(adjusted))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == if value == i64::MIN {
        Err(crate::specs::part1_transform_error(true))
    } else {
        Ok((if value < 0 { -(value as int) } else { value as int }) as i64)
    },
))]
fn checked_magnitude(value: i64) -> Result<i64, DecodeError> {
    if value == i64::MIN {
        return Err(arithmetic_overflow());
    }
    if value < 0 { Ok(-value) } else { Ok(value) }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_quantization_step(step.exponent, step.mantissa, dynamic_range),
))]
fn quantization_step_q16(step: QuantizationStep, dynamic_range: u8) -> Result<i64, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { assert(FIXED_ONE == 65536) by(compute); }
    let base_value = 2048_u32
        .checked_add(step.mantissa as u32)
        .ok_or_else(arithmetic_overflow)?;
    let base = (base_value as i64)
        .checked_mul(FIXED_ONE)
        .ok_or_else(arithmetic_overflow)?
        / 2048;
    if dynamic_range >= step.exponent {
        let shift = dynamic_range - step.exponent;
        if shift > 38 {
            return Err(invalid_transform());
        }
        base.checked_mul(1_i64 << (shift as u32))
            .ok_or_else(arithmetic_overflow)
    } else {
        let shift = step.exponent - dynamic_range;
        if shift > 62 {
            return Err(invalid_transform());
        }
        let divisor = 1_i64 << (shift as u32);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(divisor > 0) by(bit_vector)
                requires shift <= 62, divisor == 1i64 << (shift as u32);
        }
        if divisor == 0 {
            return Err(arithmetic_overflow());
        }
        Ok(base / divisor)
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_round_fixed(value, fraction_bits),
))]
fn round_fixed(value: i64, fraction_bits: u8) -> Result<i64, DecodeError> {
    if fraction_bits == 0 {
        return Ok(value);
    }
    if fraction_bits > 62 {
        return Err(invalid_transform());
    }
    let half_shift = fraction_bits
        .checked_sub(1)
        .ok_or_else(arithmetic_overflow)?;
    let half = 1_i64 << (half_shift as u32);
    Ok(value.checked_add(half).ok_or_else(arithmetic_overflow)? >> (fraction_bits as u32))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_dimension(value),
))]
fn transform_dimension(value: u32) -> Result<usize, DecodeError> {
    let index = value as usize;
    if index as u64 != value as u64 {
        return Err(arithmetic_overflow());
    }
    Ok(index)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_area(width, height),
))]
fn checked_area(width: u32, height: u32) -> Result<usize, DecodeError> {
    transform_dimension(width)?
        .checked_mul(transform_dimension(height)?)
        .ok_or_else(arithmetic_overflow)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_quantization_budget(
        samples as nat, crate::limits::resource_decoded_bytes_limit(limits),
    ),
))]
fn validate_output_size(samples: usize, limits: &ResourceLimits) -> Result<(), DecodeError> {
    validate_quantization_size(samples, limits.maximum_decoded_bytes())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_quantization_budget(samples as nat, maximum),
))]
fn validate_quantization_size(samples: usize, maximum: u64) -> Result<(), DecodeError> {
    let bytes = (samples as u64)
        .checked_mul(8)
        .ok_or_else(arithmetic_overflow)?;
    if bytes > maximum {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::DecodedBytes,
            limit: maximum,
            actual: bytes,
        });
    }
    Ok(())
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
    ensures result == crate::specs::part1_transform_error(true),
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

    fn reference_reconstruction(
        input: &[([u64; 6], bool)],
        step: QuantizationStep,
        range: u8,
        reversible: bool,
        shift: u8,
        maximum: u64,
    ) -> Result<(u32, u32, u8, Vec<i64>), DecodeError> {
        let overflow = DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::ArithmeticOverflow,
        };
        if !(1..=38).contains(&range) || step.exponent > 31 || step.mantissa > 2047 {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidTransform,
            });
        }
        let bytes = (input.len() as u64).checked_mul(8).ok_or(overflow)?;
        if bytes > maximum {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: maximum,
                actual: bytes,
            });
        }
        let numerator = u128::from(2048 + step.mantissa) * 32;
        let step_q16 = if reversible {
            1
        } else if range >= step.exponent {
            numerator * 2_u128.pow(u32::from(range - step.exponent))
        } else {
            numerator / 2_u128.pow(u32::from(step.exponent - range))
        };
        if step_q16 > i64::MAX as u128 {
            return Err(overflow);
        }
        let mut output = Vec::new();
        for (limbs, negative) in input {
            let bits: Vec<bool> = (0..384)
                .map(|bit| !(limbs[bit / 64] / 2_u64.pow((bit % 64) as u32)).is_multiple_of(2))
                .collect();
            let is_roi = bits[usize::from(shift)..].iter().any(|bit| *bit);
            let start = if is_roi { usize::from(shift) } else { 0 };
            if bits[start + 63..].iter().any(|bit| *bit) {
                return Err(overflow);
            }
            let magnitude: u128 = bits[start..start + 63]
                .iter()
                .enumerate()
                .map(|(bit, set)| if *set { 2_u128.pow(bit as u32) } else { 0 })
                .sum();
            let sample = if reversible || magnitude == 0 {
                magnitude
            } else {
                ((magnitude * 2 + 1) * step_q16) / 2
            };
            if sample > i64::MAX as u128 {
                return Err(overflow);
            }
            output.push(if *negative {
                -(sample as i64)
            } else {
                sample as i64
            });
        }
        Ok((
            input.len() as u32,
            1,
            if reversible { 0 } else { 16 },
            output,
        ))
    }

    fn observed_reconstruction(
        result: Result<ReconstructedPlane, DecodeError>,
    ) -> Result<(u32, u32, u8, Vec<i64>), DecodeError> {
        result.map(|plane| {
            (
                plane.width(),
                plane.height(),
                plane.fraction_bits(),
                plane.samples().to_vec(),
            )
        })
    }

    fn reference_lift(
        input: &[i64],
        even: bool,
        kind: u8,
        factor: i64,
    ) -> (Result<(), DecodeError>, Vec<i64>) {
        let mut result = input.to_vec();
        let checked = |value: i128| i64::try_from(value).map(i128::from);
        for index in 0..input.len() {
            if index.is_multiple_of(2) != even || input.len() <= 1 {
                continue;
            }
            let left = i128::from(input[if index == 0 { 1 } else { index - 1 }]);
            let right = i128::from(
                input[if index + 1 < input.len() {
                    index + 1
                } else {
                    index - 1
                }],
            );
            let sample = (|| {
                let sum = checked(left + right)?;
                let delta = if kind == 0 {
                    checked(sum + 2)?.div_euclid(4)
                } else if kind == 1 {
                    sum.div_euclid(2)
                } else {
                    checked(checked(sum * i128::from(factor))? + 32768)?.div_euclid(65536)
                };
                i64::try_from(i128::from(input[index]) + if kind == 1 { delta } else { -delta })
            })();
            match sample {
                Ok(value) => result[index] = value,
                Err(_) => return (Err(arithmetic_overflow()), result),
            }
        }
        (Ok(()), result)
    }

    fn reference_five_three(
        low: &[i64],
        high: &[i64],
        low_first: bool,
    ) -> Result<Vec<i64>, DecodeError> {
        let length = low.len() + high.len();
        let low_count = length / 2 + usize::from(low_first) * (length % 2);
        if low.len() != low_count || high.len() != length - low_count {
            return Err(invalid_transform());
        }
        let samples: Vec<_> = (0..length)
            .map(|index| {
                if index.is_multiple_of(2) == low_first {
                    low[index / 2]
                } else {
                    high[index / 2]
                }
            })
            .collect();
        if length == 1 && !low_first {
            return Ok(vec![samples[0].div_euclid(2)]);
        }
        let (status, updated) = reference_lift(&samples, low_first, 0, 0);
        status?;
        let (status, predicted) = reference_lift(&updated, !low_first, 1, 0);
        status?;
        Ok(predicted)
    }

    fn reference_scale(
        input: &[i64],
        even: bool,
        factor: i64,
    ) -> (Result<(), DecodeError>, Vec<i64>) {
        let mut output = input.to_vec();
        for (index, value) in input.iter().enumerate() {
            if index.is_multiple_of(2) != even {
                continue;
            }
            let rounded = (|| {
                let product = i64::try_from(i128::from(*value) * i128::from(factor))?;
                let biased = i64::try_from(i128::from(product) + 32768)?;
                i64::try_from(i128::from(biased).div_euclid(65536))
            })();
            match rounded {
                Ok(value) => output[index] = value,
                Err(_) => return (Err(arithmetic_overflow()), output),
            }
        }
        (Ok(()), output)
    }

    fn reference_nine_seven_lifts(
        input: &[i64],
        low_first: bool,
    ) -> (Result<(), DecodeError>, Vec<i64>) {
        let mut output = input.to_vec();
        for (stage, factor) in [29066, 57862, -3472, -103949].into_iter().enumerate() {
            let (status, next) = reference_lift(
                &output,
                if stage.is_multiple_of(2) {
                    low_first
                } else {
                    !low_first
                },
                2,
                factor,
            );
            output = next;
            if status.is_err() {
                return (status, output);
            }
        }
        (Ok(()), output)
    }

    fn reference_sequence(
        low: &[i64],
        high: &[i64],
        low_first: bool,
        irreversible: bool,
    ) -> Result<Vec<i64>, DecodeError> {
        if !irreversible {
            return reference_five_three(low, high, low_first);
        }
        let length = low.len() + high.len();
        let low_count = length / 2 + usize::from(low_first) * (length % 2);
        if low.len() != low_count || high.len() != length - low_count {
            return Err(invalid_transform());
        }
        let samples: Vec<_> = (0..length)
            .map(|index| {
                if index.is_multiple_of(2) == low_first {
                    low[index / 2]
                } else {
                    high[index / 2]
                }
            })
            .collect();
        if length <= 1 {
            return Ok(samples);
        }
        let (status, low_scaled) = reference_scale(&samples, low_first, 80621);
        status?;
        let (status, high_scaled) = reference_scale(&low_scaled, !low_first, 53274);
        status?;
        let (status, lifted) = reference_nine_seven_lifts(&high_scaled, low_first);
        status?;
        Ok(lifted)
    }

    fn reference_area(width: u32, height: u32) -> Result<usize, DecodeError> {
        let count = u128::from(width) * u128::from(height);
        if u128::from(width) > usize::MAX as u128
            || u128::from(height) > usize::MAX as u128
            || count > usize::MAX as u128
        {
            return Err(arithmetic_overflow());
        }
        Ok(count as usize)
    }

    fn reference_plane_budget(width: u32, height: u32, maximum: u64) -> Result<usize, DecodeError> {
        let count = reference_area(width, height)?;
        let bytes = (count as u128) * 8;
        if bytes > u128::from(u64::MAX) {
            return Err(arithmetic_overflow());
        }
        if bytes > u128::from(maximum) {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: maximum,
                actual: bytes as u64,
            });
        }
        Ok(count)
    }

    fn reference_band_row(band: WaveletBand<'_>, y: u32) -> Result<&[i64], DecodeError> {
        if y >= band.height {
            return Err(invalid_transform());
        }
        let start = u128::from(y) * u128::from(band.width);
        let end = start + u128::from(band.width);
        if u128::from(y) > usize::MAX as u128
            || u128::from(band.width) > usize::MAX as u128
            || end > usize::MAX as u128
        {
            return Err(arithmetic_overflow());
        }
        band.samples
            .get(start as usize..end as usize)
            .ok_or_else(invalid_transform)
    }

    fn reference_column(band: WaveletBand<'_>, x: u32) -> Result<Vec<i64>, DecodeError> {
        if x >= band.width {
            return Err(invalid_transform());
        }
        if u128::from(band.height) > usize::MAX as u128 {
            return Err(arithmetic_overflow());
        }
        let mut output = Vec::new();
        for y in 0..band.height {
            let index = u128::from(y) * u128::from(band.width) + u128::from(x);
            if u128::from(y) > usize::MAX as u128
                || u128::from(band.width) > usize::MAX as u128
                || u128::from(x) > usize::MAX as u128
                || index > usize::MAX as u128
            {
                return Err(arithmetic_overflow());
            }
            output.push(
                *band
                    .samples
                    .get(index as usize)
                    .ok_or_else(invalid_transform)?,
            );
        }
        Ok(output)
    }

    fn reference_horizontal(
        low: WaveletBand<'_>,
        high: WaveletBand<'_>,
        width: u32,
        low_first: bool,
        irreversible: bool,
        maximum: u64,
    ) -> Result<(u32, u32, u8, Vec<i64>), DecodeError> {
        let low_width = ((u64::from(width) + u64::from(low_first)) / 2) as u32;
        if low.height == 0
            || low.height != high.height
            || low.width != low_width
            || high.width != width - low_width
        {
            return Err(invalid_transform());
        }
        reference_plane_budget(width, low.height, maximum)?;
        let mut output = Vec::new();
        for y in 0..low.height {
            let low_row = reference_band_row(low, y)?;
            let high_row = reference_band_row(high, y)?;
            output.extend(reference_sequence(
                low_row,
                high_row,
                low_first,
                irreversible,
            )?);
        }
        Ok((width, low.height, if irreversible { 16 } else { 0 }, output))
    }

    fn reference_vertical(
        low: WaveletBand<'_>,
        high: WaveletBand<'_>,
        height: u32,
        low_first: bool,
        irreversible: bool,
        maximum: u64,
    ) -> Result<(u32, u32, u8, Vec<i64>), DecodeError> {
        let low_height = ((u64::from(height) + u64::from(low_first)) / 2) as u32;
        if low.width == 0
            || low.width != high.width
            || low.height != low_height
            || high.height != height - low_height
        {
            return Err(invalid_transform());
        }
        let length = reference_plane_budget(low.width, height, maximum)?;
        let mut output = vec![0; length];
        for x in 0..low.width {
            let low_column = reference_column(low, x)?;
            let high_column = reference_column(high, x)?;
            let column = reference_sequence(&low_column, &high_column, low_first, irreversible)?;
            for y in 0..height {
                let destination = u128::from(y) * u128::from(low.width) + u128::from(x);
                if destination > usize::MAX as u128 {
                    return Err(arithmetic_overflow());
                }
                *output
                    .get_mut(destination as usize)
                    .ok_or_else(invalid_transform)? =
                    *column.get(y as usize).ok_or_else(invalid_transform)?;
            }
        }
        Ok((low.width, height, if irreversible { 16 } else { 0 }, output))
    }

    fn reference_level(
        level: WaveletLevel<'_>,
        irreversible: bool,
        maximum: u64,
    ) -> Result<(u32, u32, u8, Vec<i64>), DecodeError> {
        if level.output_width == 0 || level.output_height == 0 {
            return Err(invalid_transform());
        }
        let low_width =
            ((u64::from(level.output_width) + u64::from(level.low_pass_first_x)) / 2) as u32;
        let low_height =
            ((u64::from(level.output_height) + u64::from(level.low_pass_first_y)) / 2) as u32;
        for (band, width, height) in [
            (level.ll, low_width, low_height),
            (level.hl, level.output_width - low_width, low_height),
            (level.lh, low_width, level.output_height - low_height),
            (
                level.hh,
                level.output_width - low_width,
                level.output_height - low_height,
            ),
        ] {
            if band.width != width || band.height != height {
                return Err(invalid_transform());
            }
            if band.samples.len() != reference_area(width, height)? {
                return Err(invalid_transform());
            }
        }
        reference_plane_budget(level.output_width, level.output_height, maximum)?;
        let mut low_rows = Vec::new();
        let mut high_rows = Vec::new();
        for (low, high, rows) in [
            (level.ll, level.hl, &mut low_rows),
            (level.lh, level.hh, &mut high_rows),
        ] {
            reference_area(level.output_width, low.height)?;
            for y in 0..low.height {
                let low_row = reference_band_row(low, y)?;
                let high_row = reference_band_row(high, y)?;
                rows.extend(reference_sequence(
                    low_row,
                    high_row,
                    level.low_pass_first_x,
                    irreversible,
                )?);
            }
        }
        reference_vertical(
            WaveletBand {
                width: level.output_width,
                height: level.ll.height,
                samples: &low_rows,
            },
            WaveletBand {
                width: level.output_width,
                height: level.lh.height,
                samples: &high_rows,
            },
            level.output_height,
            level.low_pass_first_y,
            irreversible,
            maximum,
        )
    }

    #[test]
    fn complete_nine_seven_sequences_match_independent_wide_reference() {
        for length in 0..=6_u32 {
            for encoding in 0..5_usize.pow(length) {
                let mut encoding = encoding;
                let samples: Vec<_> = (0..length)
                    .map(|_| {
                        let value = [-65537, -32768, 0, 32768, 65537][encoding % 5];
                        encoding /= 5;
                        value
                    })
                    .collect();
                for low_first in [false, true] {
                    let low: Vec<_> = samples
                        .iter()
                        .enumerate()
                        .filter_map(|(i, value)| {
                            (i.is_multiple_of(2) == low_first).then_some(*value)
                        })
                        .collect();
                    let high: Vec<_> = samples
                        .iter()
                        .enumerate()
                        .filter_map(|(i, value)| {
                            (i.is_multiple_of(2) != low_first).then_some(*value)
                        })
                        .collect();
                    assert_eq!(
                        inverse_9_7(&low, &high, low_first),
                        reference_sequence(&low, &high, low_first, true)
                    );
                }
            }
        }
        for low in [
            vec![],
            vec![i64::MIN],
            vec![i64::MAX],
            vec![0, i64::MAX],
            vec![1, -1, 2],
        ] {
            for high in [
                vec![],
                vec![i64::MIN],
                vec![i64::MAX],
                vec![i64::MIN, i64::MAX],
                vec![-2, 0, 2],
            ] {
                for low_first in [false, true] {
                    assert_eq!(
                        inverse_9_7(&low, &high, low_first),
                        reference_sequence(&low, &high, low_first, true)
                    );
                }
            }
        }
    }

    #[test]
    fn complete_standard_planes_match_independent_reference_for_all_small_geometries() {
        for width in 0..=7_u32 {
            for height in 0..=6_u32 {
                for low_x in [false, true] {
                    for low_y in [false, true] {
                        for irreversible in [false, true] {
                            let low_width = ((u64::from(width) + u64::from(low_x)) / 2) as u32;
                            let low_height = ((u64::from(height) + u64::from(low_y)) / 2) as u32;
                            let generated = |w: u32, h: u32, seed: i64| {
                                (0..w * h)
                                    .map(|i| {
                                        ((i64::from(i) * 7 + seed) % 17 - 8)
                                            * if irreversible { 65536 } else { 1 }
                                    })
                                    .collect::<Vec<_>>()
                            };
                            let horizontal_low = generated(low_width, height, 3);
                            let horizontal_high = generated(width - low_width, height, 5);
                            let vertical_low = generated(width, low_height, 7);
                            let vertical_high = generated(width, height - low_height, 11);
                            let ll = generated(low_width, low_height, 2);
                            let hl = generated(width - low_width, low_height, 4);
                            let lh = generated(low_width, height - low_height, 6);
                            let hh = generated(width - low_width, height - low_height, 8);
                            let h_low = WaveletBand {
                                width: low_width,
                                height,
                                samples: &horizontal_low,
                            };
                            let h_high = WaveletBand {
                                width: width - low_width,
                                height,
                                samples: &horizontal_high,
                            };
                            let v_low = WaveletBand {
                                width,
                                height: low_height,
                                samples: &vertical_low,
                            };
                            let v_high = WaveletBand {
                                width,
                                height: height - low_height,
                                samples: &vertical_high,
                            };
                            let level = WaveletLevel {
                                ll: WaveletBand {
                                    width: low_width,
                                    height: low_height,
                                    samples: &ll,
                                },
                                hl: WaveletBand {
                                    width: width - low_width,
                                    height: low_height,
                                    samples: &hl,
                                },
                                lh: WaveletBand {
                                    width: low_width,
                                    height: height - low_height,
                                    samples: &lh,
                                },
                                hh: WaveletBand {
                                    width: width - low_width,
                                    height: height - low_height,
                                    samples: &hh,
                                },
                                output_width: width,
                                output_height: height,
                                low_pass_first_x: low_x,
                                low_pass_first_y: low_y,
                            };
                            for limits in [
                                ResourceLimits::default(),
                                ResourceLimits::default()
                                    .with_maximum_decoded_bytes(8)
                                    .unwrap(),
                            ] {
                                let horizontal = if irreversible {
                                    inverse_horizontal_9_7(h_low, h_high, width, low_x, &limits)
                                } else {
                                    inverse_horizontal_5_3(h_low, h_high, width, low_x, &limits)
                                };
                                assert_eq!(
                                    observed_reconstruction(horizontal),
                                    reference_horizontal(
                                        h_low,
                                        h_high,
                                        width,
                                        low_x,
                                        irreversible,
                                        limits.maximum_decoded_bytes(),
                                    )
                                );
                                let vertical = if irreversible {
                                    inverse_vertical_9_7(v_low, v_high, height, low_y, &limits)
                                } else {
                                    inverse_vertical_5_3(v_low, v_high, height, low_y, &limits)
                                };
                                assert_eq!(
                                    observed_reconstruction(vertical),
                                    reference_vertical(
                                        v_low,
                                        v_high,
                                        height,
                                        low_y,
                                        irreversible,
                                        limits.maximum_decoded_bytes(),
                                    )
                                );
                                let reconstructed = if irreversible {
                                    inverse_level_9_7(level, &limits)
                                } else {
                                    inverse_level_5_3(level, &limits)
                                };
                                assert_eq!(
                                    observed_reconstruction(reconstructed),
                                    reference_level(
                                        level,
                                        irreversible,
                                        limits.maximum_decoded_bytes()
                                    )
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn complete_plane_rejections_preserve_late_band_checks_and_resource_order() {
        let low = WaveletBand {
            width: 2,
            height: 2,
            samples: &[65536, -65536, 32768],
        };
        let high = WaveletBand {
            width: 2,
            height: 2,
            samples: &[0, 0, 0, 0],
        };
        for limits in [
            ResourceLimits::default(),
            ResourceLimits::default()
                .with_maximum_decoded_bytes(8)
                .unwrap(),
        ] {
            for irreversible in [false, true] {
                let horizontal = if irreversible {
                    inverse_horizontal_9_7(low, high, 4, true, &limits)
                } else {
                    inverse_horizontal_5_3(low, high, 4, true, &limits)
                };
                assert_eq!(
                    observed_reconstruction(horizontal),
                    reference_horizontal(
                        low,
                        high,
                        4,
                        true,
                        irreversible,
                        limits.maximum_decoded_bytes()
                    )
                );
                let vertical = if irreversible {
                    inverse_vertical_9_7(low, high, 4, true, &limits)
                } else {
                    inverse_vertical_5_3(low, high, 4, true, &limits)
                };
                assert_eq!(
                    observed_reconstruction(vertical),
                    reference_vertical(
                        low,
                        high,
                        4,
                        true,
                        irreversible,
                        limits.maximum_decoded_bytes()
                    )
                );
            }
            let band = WaveletBand {
                width: 1,
                height: 1,
                samples: &[0],
            };
            let level = WaveletLevel {
                ll: WaveletBand {
                    samples: &[0, 1],
                    ..band
                },
                hl: band,
                lh: band,
                hh: band,
                output_width: 2,
                output_height: 2,
                low_pass_first_x: true,
                low_pass_first_y: true,
            };
            assert_eq!(
                observed_reconstruction(inverse_level_5_3(level, &limits)),
                reference_level(level, false, limits.maximum_decoded_bytes())
            );
            assert_eq!(
                observed_reconstruction(inverse_level_9_7(level, &limits)),
                reference_level(level, true, limits.maximum_decoded_bytes())
            );
        }
        let trailing_low = WaveletBand {
            width: 1,
            height: 1,
            samples: &[0, 9],
        };
        let trailing_high = WaveletBand {
            width: 1,
            height: 1,
            samples: &[0, 8],
        };
        let limits = ResourceLimits::default();
        assert_eq!(
            observed_reconstruction(inverse_horizontal_5_3(
                trailing_low,
                trailing_high,
                2,
                true,
                &limits
            )),
            Ok((2, 1, 0, vec![0, 0]))
        );
        assert_eq!(
            observed_reconstruction(inverse_vertical_5_3(
                trailing_low,
                trailing_high,
                2,
                true,
                &limits
            )),
            Ok((1, 2, 0, vec![0, 0]))
        );
        let first_overflow = WaveletBand {
            width: 2,
            height: 2,
            samples: &[0, 0],
        };
        let later_missing = WaveletBand {
            width: 1,
            height: 2,
            samples: &[i64::MAX, 0],
        };
        assert_eq!(
            observed_reconstruction(inverse_horizontal_5_3(
                first_overflow,
                later_missing,
                3,
                true,
                &limits
            )),
            Err(arithmetic_overflow())
        );
        let huge_low = WaveletBand {
            width: u32::MAX / 2 + 1,
            height: u32::MAX,
            samples: &[],
        };
        let huge_high = WaveletBand {
            width: u32::MAX / 2,
            ..huge_low
        };
        assert_eq!(
            observed_reconstruction(inverse_horizontal_5_3(
                huge_low,
                huge_high,
                u32::MAX,
                true,
                &limits
            )),
            Err(arithmetic_overflow())
        );
        let huge_low = WaveletBand {
            width: u32::MAX,
            height: u32::MAX / 2 + 1,
            samples: &[],
        };
        let huge_high = WaveletBand {
            height: u32::MAX / 2,
            ..huge_low
        };
        assert_eq!(
            observed_reconstruction(inverse_vertical_9_7(
                huge_low,
                huge_high,
                u32::MAX,
                true,
                &limits
            )),
            Err(arithmetic_overflow())
        );
    }

    #[test]
    fn complete_scaling_lifting_and_column_store_states_match_independent_arithmetic() {
        for input in [
            vec![],
            vec![i64::MIN],
            vec![65536, 0, i64::MAX],
            vec![-65537, 32768, 0, -32768, 65537],
        ] {
            for low_first in [false, true] {
                let expected = reference_nine_seven_lifts(&input, low_first);
                let mut actual = input.clone();
                let status = inverse_nine_seven_lifts(&mut actual, low_first);
                assert_eq!((status, actual), expected);
                for factor in [80621, 53274, -1, i64::MAX] {
                    let expected = reference_scale(&input, low_first, factor);
                    let mut actual = input.clone();
                    let status = scale_parity(&mut actual, low_first, factor);
                    assert_eq!((status, actual), expected);
                }
            }
        }
        for (initial, width, height, x, column) in [
            (vec![9; 6], 2_u32, 3_u32, 1_u32, vec![2, 3]),
            (vec![9; 3], 2, 3, 1, vec![2, 3, 4]),
            (vec![], 0, 0, u32::MAX, vec![]),
            (vec![9], 0, 2, 0, vec![2, 3]),
            (vec![9], 1, 1, 2, vec![7]),
        ] {
            let mut expected = initial.clone();
            let mut expected_status = Ok(());
            for y in 0..height {
                let destination = u128::from(y) * u128::from(width) + u128::from(x);
                if destination > usize::MAX as u128 {
                    expected_status = Err(arithmetic_overflow());
                    break;
                }
                if y as usize >= column.len() || destination as usize >= expected.len() {
                    expected_status = Err(invalid_transform());
                    break;
                }
                expected[destination as usize] = column[y as usize];
            }
            let mut actual = initial;
            let status = store_column(&mut actual, width, height, x, &column);
            assert_eq!((status, actual), (expected_status, expected));
        }
    }

    fn reference_converted_samples(
        plane: &ReconstructedPlane,
        precision: u8,
        signed: bool,
        maximum_bytes: u64,
    ) -> Result<Vec<i64>, DecodeError> {
        if !(1..=38).contains(&precision) || plane.fraction_bits > 62 {
            return Err(invalid_transform());
        }
        let bytes = plane.samples.len() as u128 * 8;
        if bytes > u128::from(u64::MAX) {
            return Err(arithmetic_overflow());
        }
        if bytes > u128::from(maximum_bytes) {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: maximum_bytes,
                actual: bytes as u64,
            });
        }
        let midpoint = 2_i128.pow(u32::from(precision - 1));
        let minimum = if signed { -midpoint } else { 0 };
        let maximum = if signed {
            midpoint - 1
        } else {
            2 * midpoint - 1
        };
        let shift = if signed { 0 } else { midpoint };
        let mut output = Vec::new();
        for value in &plane.samples {
            let rounded = if plane.fraction_bits == 0 {
                i128::from(*value)
            } else {
                let biased = i64::try_from(
                    i128::from(*value) + 2_i128.pow(u32::from(plane.fraction_bits - 1)),
                )
                .map_err(|_| arithmetic_overflow())?;
                i128::from(biased).div_euclid(2_i128.pow(u32::from(plane.fraction_bits)))
            };
            let shifted = i64::try_from(rounded + shift).map_err(|_| arithmetic_overflow())?;
            output.push(i128::from(shifted).clamp(minimum, maximum) as i64);
        }
        Ok(output)
    }

    #[test]
    fn complete_sample_conversion_matches_wide_reference_and_error_boundaries() {
        for samples in [
            vec![],
            vec![-32769, -32768, -1, 0, 32767, 32768, 32769],
            vec![i64::MIN],
            vec![i64::MAX],
            vec![i64::MAX, i64::MIN],
            vec![0, 1, -1],
        ] {
            for fraction_bits in [0, 1, 16, 62, 63, 255] {
                for precision in [0, 1, 8, 16, 38, 39, 63, 255] {
                    for signed in [false, true] {
                        for (width, height) in [(0, 7), (100, 0)] {
                            let plane = ReconstructedPlane {
                                width,
                                height,
                                fraction_bits,
                                samples: samples.clone(),
                            };
                            for limits in [
                                ResourceLimits::default(),
                                ResourceLimits::default()
                                    .with_maximum_decoded_bytes(8)
                                    .unwrap(),
                            ] {
                                assert_eq!(
                                    convert_samples(&plane, precision, signed, &limits),
                                    reference_converted_samples(
                                        &plane,
                                        precision,
                                        signed,
                                        limits.maximum_decoded_bytes()
                                    ),
                                    "fraction={fraction_bits} precision={precision} signed={signed}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn full_five_three_sequences_and_rejections_match_wide_reference() {
        for length in 0..=6_u32 {
            for encoding in 0..5_usize.pow(length) {
                let mut encoding = encoding;
                let samples: Vec<_> = (0..length)
                    .map(|_| {
                        let value = [-3, -1, 0, 1, 4][encoding % 5];
                        encoding /= 5;
                        value
                    })
                    .collect();
                for low_first in [false, true] {
                    let low: Vec<_> = samples
                        .iter()
                        .enumerate()
                        .filter_map(|(i, value)| {
                            (i.is_multiple_of(2) == low_first).then_some(*value)
                        })
                        .collect();
                    let high: Vec<_> = samples
                        .iter()
                        .enumerate()
                        .filter_map(|(i, value)| {
                            (i.is_multiple_of(2) != low_first).then_some(*value)
                        })
                        .collect();
                    assert_eq!(
                        inverse_5_3(&low, &high, low_first),
                        reference_five_three(&low, &high, low_first)
                    );
                }
            }
        }
        for low in [
            vec![],
            vec![i64::MIN],
            vec![i64::MAX],
            vec![0, i64::MAX],
            vec![1, -1, 2],
        ] {
            for high in [
                vec![],
                vec![i64::MIN],
                vec![i64::MAX],
                vec![i64::MIN, i64::MAX],
                vec![-2, 0, 2],
            ] {
                for low_first in [false, true] {
                    assert_eq!(
                        inverse_5_3(&low, &high, low_first),
                        reference_five_three(&low, &high, low_first)
                    );
                }
            }
        }
    }

    #[test]
    fn complete_lifting_buffers_preserve_first_error_state_and_opposite_parity() {
        for input in [
            vec![],
            vec![i64::MIN],
            vec![0, 1, 0, i64::MAX, 0],
            vec![0, i64::MAX, 0],
            vec![i64::MIN, 0, i64::MAX],
            vec![-7, 4, 3, -2, 0, 8, -1, 5],
        ] {
            for even in [false, true] {
                for (kind, factor, step) in [
                    (0, 0, InverseLiftingStep::ReversibleUpdate),
                    (1, 0, InverseLiftingStep::ReversiblePredict),
                    (2, 29066, InverseLiftingStep::Fixed(29066)),
                    (2, -103949, InverseLiftingStep::Fixed(-103949)),
                ] {
                    let expected = reference_lift(&input, even, kind, factor);
                    let mut actual = input.clone();
                    let status = inverse_lift(&mut actual, even, step);
                    assert_eq!((status, actual), expected);
                }
            }
        }
    }

    #[test]
    fn complete_quantization_planes_match_independent_bit_and_rational_reference() {
        let inputs = [
            vec![],
            vec![
                ([0; 6], true),
                ([1, 0, 0, 0, 0, 0], false),
                ([2, 0, 0, 0, 0, 0], true),
            ],
            vec![([i64::MAX as u64, 0, 0, 0, 0, 0], true)],
            vec![([1_u64 << 63, 0, 0, 0, 0, 0], true)],
            vec![([u64::MAX, 0, 0, 0, 0, 0], false)],
            vec![([1, 1, 0, 0, 0, 0], true)],
            vec![([0, 0, 0, 0, 1 << 35, 0], true)],
            vec![([u64::MAX; 6], false)],
        ];
        for input in inputs {
            let coefficients: Vec<_> = input
                .iter()
                .map(|(limbs, negative)| {
                    SignedMagnitude::new(Magnitude::from_limbs(*limbs), *negative)
                })
                .collect();
            for range in [1, 8, 16, 31, 38] {
                for exponent in [0, 8, 31] {
                    for mantissa in [0, 1, 2047] {
                        for reversible in [false, true] {
                            for shift in [0, 1, 31, 63, 64, 65, 127, 128, 191, 192, 254, 255] {
                                let step = QuantizationStep { exponent, mantissa };
                                let expected = reference_reconstruction(
                                    &input,
                                    step,
                                    range,
                                    reversible,
                                    shift,
                                    u64::MAX,
                                );
                                assert_eq!(
                                    observed_reconstruction(inverse_quantize_plane(
                                        &coefficients,
                                        step,
                                        range,
                                        reversible,
                                        shift,
                                        u64::MAX,
                                    )),
                                    expected,
                                    "range={range} step={step:?} reversible={reversible} shift={shift}"
                                );
                                assert_eq!(
                                    observed_reconstruction(inverse_quantize_magnitudes(
                                        &coefficients,
                                        step,
                                        range,
                                        reversible,
                                        shift,
                                        &ResourceLimits::default(),
                                    )),
                                    expected,
                                    "configured budget adapter range={range} step={step:?} reversible={reversible} shift={shift}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn quantization_rejects_in_production_order_and_preserves_public_adapters() {
        let step = QuantizationStep {
            exponent: 8,
            mantissa: 17,
        };
        let limits = ResourceLimits::default()
            .with_maximum_decoded_bytes(8)
            .unwrap();
        for values in [
            &[][..],
            &[0][..],
            &[1, -2, 3][..],
            &[i64::MAX][..],
            &[i64::MIN][..],
        ] {
            for reversible in [true, false] {
                let raw: Vec<_> = values
                    .iter()
                    .map(|value| ([value.unsigned_abs(), 0, 0, 0, 0, 0], *value < 0))
                    .collect();
                let expected = if values.contains(&i64::MIN) {
                    Err(arithmetic_overflow())
                } else {
                    reference_reconstruction(
                        &raw,
                        step,
                        8,
                        reversible,
                        2,
                        limits.maximum_decoded_bytes(),
                    )
                };
                assert_eq!(
                    observed_reconstruction(inverse_quantize(
                        values, step, 8, reversible, 2, &limits
                    )),
                    expected
                );
            }
        }
        let bad_step = QuantizationStep {
            exponent: 32,
            mantissa: 2048,
        };
        assert_eq!(
            observed_reconstruction(inverse_quantize(
                &[0, i64::MIN],
                bad_step,
                0,
                false,
                255,
                &limits,
            )),
            Err(arithmetic_overflow())
        );
        let wide = [SignedMagnitude::new(Magnitude::from_limbs([u64::MAX; 6]), false); 2];
        assert_eq!(
            observed_reconstruction(inverse_quantize_magnitudes(
                &wide, bad_step, 0, false, 255, &limits,
            )),
            Err(invalid_transform())
        );
        assert_eq!(
            observed_reconstruction(inverse_quantize_magnitudes(
                &wide, step, 8, false, 0, &limits,
            )),
            Err(DecodeError::LimitExceeded {
                resource: Resource::DecodedBytes,
                limit: 8,
                actual: 16
            })
        );
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(
                validate_quantization_size((u64::MAX / 8) as usize, u64::MAX),
                Ok(())
            );
            assert_eq!(
                validate_quantization_size((u64::MAX / 8 + 1) as usize, u64::MAX),
                Err(arithmetic_overflow())
            );
        }
    }

    #[test]
    fn inverse_horizontal_stage_matches_row_synthesis() {
        let low = [2, 4, 6, 8];
        let high = [1, 3, 5, 7];
        let plane = inverse_horizontal_5_3(
            WaveletBand {
                width: 2,
                height: 2,
                samples: &low,
            },
            WaveletBand {
                width: 2,
                height: 2,
                samples: &high,
            },
            4,
            true,
            &ResourceLimits::default(),
        )
        .unwrap();
        let mut expected = inverse_5_3(&low[..2], &high[..2], true).unwrap();
        expected.extend(inverse_5_3(&low[2..], &high[2..], true).unwrap());
        assert_eq!(plane.samples(), expected);
        assert_eq!((plane.width(), plane.height()), (4, 2));
    }

    #[test]
    fn inverse_vertical_stage_matches_column_synthesis() {
        let low = [2, 6, 4, 8];
        let high = [1, 5, 3, 7];
        let plane = inverse_vertical_5_3(
            WaveletBand {
                width: 2,
                height: 2,
                samples: &low,
            },
            WaveletBand {
                width: 2,
                height: 2,
                samples: &high,
            },
            4,
            true,
            &ResourceLimits::default(),
        )
        .unwrap();
        let first = inverse_5_3(&[2, 4], &[1, 3], true).unwrap();
        let second = inverse_5_3(&[6, 8], &[5, 7], true).unwrap();
        let mut expected = Vec::new();
        let mut y = 0_usize;
        while y < 4 {
            expected.push(first[y]);
            expected.push(second[y]);
            y += 1;
        }
        assert_eq!(plane.samples(), expected);
        assert_eq!((plane.width(), plane.height()), (2, 4));
    }

    #[test]
    fn reverses_five_three_lifting_for_known_samples() {
        assert_eq!(inverse_5_3(&[3, 7], &[2], true).unwrap(), [2, 6, 6]);
        assert_eq!(inverse_5_3(&[3], &[], true).unwrap(), [3]);
    }

    #[test]
    fn reconstructs_two_dimensional_reversible_level() {
        let ll = [5];
        let hl = [0];
        let lh = [0];
        let hh = [0];
        let plane = inverse_level_5_3(
            WaveletLevel {
                ll: WaveletBand {
                    width: 1,
                    height: 1,
                    samples: &ll,
                },
                hl: WaveletBand {
                    width: 1,
                    height: 1,
                    samples: &hl,
                },
                lh: WaveletBand {
                    width: 1,
                    height: 1,
                    samples: &lh,
                },
                hh: WaveletBand {
                    width: 1,
                    height: 1,
                    samples: &hh,
                },
                output_width: 2,
                output_height: 2,
                low_pass_first_x: true,
                low_pass_first_y: true,
            },
            &ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(plane.samples(), [5, 5, 5, 5]);
    }

    #[test]
    fn applies_quantization_roi_and_output_conversion() {
        assert_eq!(undo_maxshift(32, 3).unwrap(), 4);
        assert_eq!(undo_maxshift(-32, 3).unwrap(), -4);
        assert_eq!(undo_maxshift(3, 3).unwrap(), 3);

        let plane = inverse_quantize(
            &[0, 2, -2],
            QuantizationStep {
                exponent: 8,
                mantissa: 0,
            },
            8,
            false,
            0,
            &ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(plane.fraction_bits(), FIXED_BITS);
        assert_eq!(
            convert_samples(&plane, 8, true, &ResourceLimits::default()).unwrap(),
            [0, 3, -2]
        );
    }

    #[test]
    fn undoes_maximum_maxshift_before_narrowing_and_quantization() {
        let limits = ResourceLimits::default();
        let shifted = SignedMagnitude {
            magnitude: Magnitude::single_bit(291).unwrap(),
            negative: true,
        };
        assert_eq!(
            undo_maxshift_magnitude(shifted, u8::MAX).unwrap(),
            -(1_i64 << 36)
        );
        let plane = inverse_quantize_magnitudes(
            &[shifted],
            QuantizationStep {
                exponent: 0,
                mantissa: 0,
            },
            38,
            true,
            u8::MAX,
            &limits,
        )
        .unwrap();
        assert_eq!(plane.samples(), &[-(1_i64 << 36)]);
        assert_eq!(undo_maxshift(17, u8::MAX).unwrap(), 17);
    }

    #[test]
    fn bounds_output_and_rejects_bad_band_geometry() {
        let limits = ResourceLimits::default()
            .with_maximum_decoded_bytes(8)
            .unwrap();
        assert!(
            inverse_quantize(
                &[1, 2],
                QuantizationStep {
                    exponent: 0,
                    mantissa: 0,
                },
                8,
                true,
                0,
                &limits,
            )
            .is_err()
        );
    }
    #[test]
    fn inverse_lifting_separates_sample_parity_from_predict_update_roles() {
        assert_eq!(inverse_5_3(&[2], &[-1, 1], false).unwrap(), vec![1, 2, 3]);
        assert_eq!(inverse_5_3(&[], &[18], false).unwrap(), vec![9]);
        assert_eq!(inverse_5_3(&[], &[-18], false).unwrap(), vec![-9]);
    }

    #[test]
    fn inverse_nine_seven_preserves_constant_and_singleton_samples() {
        let constant = 72 * FIXED_ONE;
        let restored = inverse_9_7(&[constant, constant], &[0, 0], true).unwrap();
        assert!(
            restored
                .iter()
                .all(|sample| (*sample - constant).abs() < 256)
        );
        assert_eq!(inverse_9_7(&[constant], &[], true).unwrap(), vec![constant]);
    }
}
