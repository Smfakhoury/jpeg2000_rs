use super::arbitrary::inverse_arbitrary;
use super::arithmetic::{
    arithmetic_overflow, checked_area, invalid_transform, validate_output_size,
};
use super::geometry::{band_row, collect_column, low_count, store_column, validate_level};
use super::wavelets::{inverse_5_3, inverse_9_7};
use super::{FIXED_BITS, ReconstructedPlane, WaveletBand, WaveletLevel};
#[cfg(verus_keep_ghost)]
use super::{
    arbitrary_kernel_option, arbitrary_kernel_supplied, reconstruction_outcome, sequence_outcome,
    wavelet_band_view, wavelet_level_view,
};
use crate::codestream::ArbitraryKernel;
use crate::{DecodeError, ResourceLimits};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures reconstruction_outcome(result)
        == crate::specs::part2_atk_inverse_level(wavelet_level_view(level), !crate::codestream::raw_kernel_view(*kernel).0,
            crate::codestream::raw_kernel_view(*kernel), crate::limits::resource_decoded_bytes_limit(limits)),
))]
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures reconstruction_outcome(result)
        == crate::specs::part2_atk_inverse_horizontal(wavelet_band_view(low), wavelet_band_view(high),
            output_width, low_pass_first, !crate::codestream::raw_kernel_view(*kernel).0, crate::codestream::raw_kernel_view(*kernel),
            crate::limits::resource_decoded_bytes_limit(limits)),
))]
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
    arbitrary_kernel_supplied(kernel) ==> reconstruction_outcome(result) == crate::specs::part2_atk_inverse_horizontal(
        wavelet_band_view(low), wavelet_band_view(high), output_width, low_pass_first, irreversible,
        arbitrary_kernel_option(kernel).unwrap(), crate::limits::resource_decoded_bytes_limit(limits),
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
    arbitrary_kernel_supplied(kernel) ==> sequence_outcome(result) == crate::specs::part2_atk_horizontal_prefix(
        wavelet_band_view(low), wavelet_band_view(high), low_pass_first, arbitrary_kernel_option(kernel).unwrap(), low.height as nat,
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
            arbitrary_kernel_supplied(kernel) ==> crate::specs::part2_atk_horizontal_prefix(
                wavelet_band_view(low), wavelet_band_view(high), low_pass_first, arbitrary_kernel_option(kernel).unwrap(), y as nat,
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
                    } else if arbitrary_kernel_supplied(kernel) {
                        crate::proofs::production_impl::production_atk_horizontal_error_persists(
                            wavelet_band_view(low), wavelet_band_view(high), low_pass_first, arbitrary_kernel_option(kernel).unwrap(),
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
            } else if arbitrary_kernel_supplied(kernel) {
                assert(crate::specs::part2_atk_horizontal_prefix(
                    wavelet_band_view(low), wavelet_band_view(high), low_pass_first,
                    arbitrary_kernel_option(kernel).unwrap(), y as nat + 1,
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
    arbitrary_kernel_supplied(kernel) ==> sequence_outcome(result) == crate::specs::part2_atk_horizontal_row(
        wavelet_band_view(low), wavelet_band_view(high), y, low_first, arbitrary_kernel_option(kernel).unwrap(),
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures reconstruction_outcome(result)
        == crate::specs::part2_atk_inverse_vertical(wavelet_band_view(low), wavelet_band_view(high),
            output_height, low_pass_first, !crate::codestream::raw_kernel_view(*kernel).0, crate::codestream::raw_kernel_view(*kernel),
            crate::limits::resource_decoded_bytes_limit(limits)),
))]
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
    arbitrary_kernel_supplied(kernel) ==> reconstruction_outcome(result) == crate::specs::part2_atk_inverse_vertical(
        wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first, irreversible,
        arbitrary_kernel_option(kernel).unwrap(), crate::limits::resource_decoded_bytes_limit(limits),
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
    arbitrary_kernel_supplied(kernel) ==> sequence_outcome(result) == crate::specs::part2_atk_vertical_prefix(
        wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first,
        arbitrary_kernel_option(kernel).unwrap(), output_len, low.width as nat,
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
            arbitrary_kernel_supplied(kernel) ==> crate::specs::part2_atk_vertical_prefix(
                wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first,
                arbitrary_kernel_option(kernel).unwrap(), output_len, x as nat,
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
                    } else if arbitrary_kernel_supplied(kernel) {
                        crate::proofs::production_impl::production_atk_vertical_error_persists(
                            wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first,
                            arbitrary_kernel_option(kernel).unwrap(), output_len, x as nat + 1, low.width as nat, error,
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
                    } else if arbitrary_kernel_supplied(kernel) {
                        crate::proofs::production_impl::production_atk_vertical_error_persists(
                            wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first,
                            arbitrary_kernel_option(kernel).unwrap(), output_len, x as nat + 1, low.width as nat, error,
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
            } else if arbitrary_kernel_supplied(kernel) {
                assert(crate::specs::part2_atk_vertical_prefix(
                    wavelet_band_view(low), wavelet_band_view(high), output_height, low_pass_first,
                    arbitrary_kernel_option(kernel).unwrap(), output_len, x as nat + 1,
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
    arbitrary_kernel_supplied(kernel) ==> sequence_outcome(result) == crate::specs::part2_atk_vertical_column(
        wavelet_band_view(low), wavelet_band_view(high), x, low_pass_first, arbitrary_kernel_option(kernel).unwrap(),
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
    arbitrary_kernel_supplied(kernel) ==> reconstruction_outcome(result) == crate::specs::part2_atk_inverse_level(
        wavelet_level_view(level), irreversible, arbitrary_kernel_option(kernel).unwrap(), crate::limits::resource_decoded_bytes_limit(limits),
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
