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

    pub(crate) open spec fn quantized_coefficients_outcome(result: Result<Vec<SignedMagnitude>, DecodeError>)
        -> Result<Seq<crate::specs::Part1QuantizedCoefficient>, DecodeError>
    {
        match result { Ok(values) => Ok(quantized_coefficients_view(values@)), Err(error) => Err(error) }
    }

    pub open spec fn wavelet_band_view(band: WaveletBand<'_>) -> crate::specs::Part1WaveletBand {
        (band.width, band.height, band.samples@)
    }

    #[cfg(verus_keep_ghost)]
    pub(crate) mod representation_proofs {
        use vstd::prelude::*;
        verus! {
            pub(crate) proof fn quantized_coefficients_fields(values: Seq<super::SignedMagnitude>)
                ensures super::quantized_coefficients_view(values).len() == values.len(),
                    forall |i: int| 0 <= i < values.len() ==> super::quantized_coefficients_view(values)[i]
                        == (crate::magnitude::magnitude_limbs(values[i].magnitude), values[i].negative),
            {
                reveal(super::quantized_coefficients_view);
            }

            pub(crate) proof fn quantized_coefficients_push(values: Seq<super::SignedMagnitude>, value: super::SignedMagnitude)
                ensures super::quantized_coefficients_view(values.push(value))
                    == super::quantized_coefficients_view(values).push((crate::magnitude::magnitude_limbs(value.magnitude), value.negative)),
            {
                reveal(super::quantized_coefficients_view);
                assert(super::quantized_coefficients_view(values.push(value)) =~=
                    super::quantized_coefficients_view(values).push((crate::magnitude::magnitude_limbs(value.magnitude), value.negative)));
            }
        }
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

    pub open spec fn arbitrary_kernel_option(value: Option<&ArbitraryKernel>) -> Option<crate::specs::Part2RawKernel> {
        match value { None => None, Some(kernel) => Some(crate::codestream::raw_kernel_view(*kernel)) }
    }

    pub open spec fn arbitrary_kernel_supplied(value: Option<&ArbitraryKernel>) -> bool {
        value.is_some()
    }
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
