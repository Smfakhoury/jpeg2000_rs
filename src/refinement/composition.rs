use super::CompositionPixelInputs;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part2_effective_layer_count(
            explicit_layers,
            codestreams,
        ),
    )
)]
pub(crate) const fn effective_layer_count(explicit_layers: usize, codestreams: usize) -> usize {
    if explicit_layers == 0 {
        codestreams
    } else {
        explicit_layers
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(clipped =>
        ensures clipped == crate::specs::part2_composition_clip(offset, extent, canvas),
            clipped <= extent,
            offset < canvas ==> clipped <= canvas - offset,
            offset >= canvas ==> clipped == 0,
    )
)]
pub(crate) const fn composition_clip(offset: u32, extent: u32, canvas: u32) -> u32 {
    if offset >= canvas {
        0
    } else if extent <= canvas - offset {
        extent
    } else {
        canvas - offset
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(mapped =>
        requires target_extent > 0,
        ensures mapped as nat == crate::specs::part2_composition_mapped_offset(position, source_extent, target_extent),
            position < target_extent && source_extent > 0 ==> mapped < source_extent,
    )
)]
pub(crate) const fn composition_mapped_offset(
    position: u32,
    source_extent: u32,
    target_extent: u32,
) -> u64 {
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_u32_product_bounds(position, source_extent);
        crate::proofs::production_impl::production_composition_mapped_bounds(position, source_extent, target_extent);
    }
    (position as u64) * (source_extent as u64) / (target_extent as u64)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(index =>
        ensures index == crate::specs::part2_composition_pixel_index(width, x, y),
    )
)]
pub(crate) fn composition_pixel_index(width: u32, x: u32, y: u32) -> Option<usize> {
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_u32_product_bounds(y, width);
    }
    let row = (y as u64) * (width as u64);
    let index = row + (x as u64);
    usize::try_from(index).ok()
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires frames > 0,
        ensures returns crate::specs::part2_loop_frame(index, frames),
    )
)]
pub(crate) const fn loop_frame(index: usize, frames: usize) -> usize {
    index % frames
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part2_inverse_alpha(source_alpha),)
)]
pub(crate) const fn inverse_alpha(source_alpha: u16) -> u16 {
    u16::MAX - source_alpha
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(output =>
        ensures output as nat == crate::specs::part2_source_over_alpha_numerator(source_alpha, destination_alpha),
            output <= 4294836225,
    )
)]
pub(crate) const fn source_over_alpha_numerator(source_alpha: u16, destination_alpha: u16) -> u64 {
    let source = source_alpha as u64;
    let destination = destination_alpha as u64;
    let inverse = inverse_alpha(source_alpha) as u64;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_source_over_term_bounds(source, 65535, 0);
        crate::proofs::production_impl::production_source_over_term_bounds(destination, inverse, 0);
        crate::proofs::spec_properties::source_over_numerators_are_bounded(
            0, 0, source_alpha, destination_alpha, false,
        );
    }
    source * 65_535 + destination * inverse
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(output =>
        ensures output as nat == crate::specs::part2_source_over_color_numerator(
                source, destination, source_alpha, destination_alpha, premultiplied,
            ),
            (!premultiplied || source <= source_alpha) ==> output as nat
                <= 65535 * crate::specs::part2_source_over_alpha_numerator(source_alpha, destination_alpha),
    )
)]
pub(crate) const fn source_over_color_numerator(
    source: u16,
    destination: u16,
    source_alpha: u16,
    destination_alpha: u16,
    premultiplied: bool,
) -> u64 {
    #[cfg(verus_keep_ghost)]
    proof! {
        if !premultiplied || source <= source_alpha {
            crate::proofs::spec_properties::source_over_numerators_are_bounded(
                source, destination, source_alpha, destination_alpha, premultiplied,
            );
        }
    }
    let source = source as u64;
    let destination = destination as u64;
    let inverse = inverse_alpha(source_alpha) as u64;
    let source_alpha = source_alpha as u64;
    let destination_alpha = destination_alpha as u64;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_source_over_term_bounds(source, 65535, 65535);
        crate::proofs::production_impl::production_source_over_term_bounds(source, source_alpha, 65535);
        crate::proofs::production_impl::production_source_over_term_bounds(destination, destination_alpha, inverse);
    }
    let source_term = if premultiplied {
        source * 65_535 * 65_535
    } else {
        source * source_alpha * 65_535
    };
    source_term + destination * destination_alpha * inverse
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(output =>
        ensures output as nat == crate::specs::part2_source_over_rounded_ratio(numerator, denominator),
            denominator == 0 ==> output == 0,
            denominator > 0 ==> 2 * ((denominator as nat) * (output as nat))
                <= 2 * (numerator as nat) + denominator as nat,
            denominator > 0 ==> 2 * (numerator as nat)
                < 2 * ((denominator as nat) * (output as nat)) + denominator as nat,
            numerator as nat <= 65535 * (denominator as nat) ==> output <= 65535,
    )
)]
pub(crate) const fn source_over_rounded_ratio(numerator: u64, denominator: u64) -> u128 {
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::spec_properties::source_over_rounding_is_nearest(numerator, denominator);
        if numerator as nat <= 65535 * (denominator as nat) {
            crate::proofs::spec_properties::source_over_rounding_preserves_sample_bound(numerator, denominator);
        }
    }
    if denominator == 0 {
        0
    } else {
        ((numerator as u128) + (denominator as u128) / 2) / (denominator as u128)
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(product =>
        ensures product == Some(crate::specs::part2_normalized_sample_product(first, second) as u16),
            crate::specs::part2_normalized_sample_product(first, second) <= 65535,
    )
)]
pub(crate) fn normalized_sample_product(first: u16, second: u16) -> Option<u16> {
    #[cfg(verus_keep_ghost)]
    proof! {
        vstd::arithmetic::mul::lemma_mul_inequality(first as int, 65535, second as int);
        vstd::arithmetic::mul::lemma_mul_inequality(second as int, 65535, 65535);
        assert((first as int) * (second as int) <= 4294836225) by(nonlinear_arith);
    }
    let numerator = u64::from(first) * u64::from(second);
    u16::try_from(source_over_rounded_ratio(numerator, u64::from(u16::MAX))).ok()
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(stored =>
        ensures (stored, final(samples)@) == crate::specs::part2_composition_store(old(samples)@, index as nat, value),
            final(samples)@.len() == old(samples)@.len(),
    )
)]
pub(crate) fn try_store_composition_sample(samples: &mut [u16], index: usize, value: u16) -> bool {
    if index < samples.len() {
        samples[index] = value;
        true
    } else {
        false
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(pixel =>
        ensures pixel == crate::specs::part2_source_over_pixel(
            source, destination, source_alpha, destination_alpha, premultiplied, channels,
        ),
            pixel.is_some() <==> crate::specs::part2_source_over_pixel_valid(source, source_alpha, premultiplied, channels),
            pixel.is_some() ==> pixel.unwrap().1 as nat == crate::specs::part2_source_over_alpha_sample(source_alpha, destination_alpha),
            pixel.is_some() && channels > 0 ==> pixel.unwrap().0[0] as nat == crate::specs::part2_source_over_color_sample(source[0], destination[0], source_alpha, destination_alpha, premultiplied),
            pixel.is_some() && channels > 1 ==> pixel.unwrap().0[1] as nat == crate::specs::part2_source_over_color_sample(source[1], destination[1], source_alpha, destination_alpha, premultiplied),
            pixel.is_some() && channels > 2 ==> pixel.unwrap().0[2] as nat == crate::specs::part2_source_over_color_sample(source[2], destination[2], source_alpha, destination_alpha, premultiplied),
    )
)]
pub(crate) fn source_over_pixel(
    source: [u16; 3],
    destination: [u16; 3],
    source_alpha: u16,
    destination_alpha: u16,
    premultiplied: bool,
    channels: usize,
) -> Option<([u16; 3], u16)> {
    if channels > 3 {
        return None;
    }
    let alpha_numerator = source_over_alpha_numerator(source_alpha, destination_alpha);
    let mut colors = destination;
    let mut i = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant
                i <= channels && channels <= 3,
                alpha_numerator as nat == crate::specs::part2_source_over_alpha_numerator(source_alpha, destination_alpha),
                alpha_numerator <= 4294836225,
                i > 0 ==> (!premultiplied || source[0] <= source_alpha)
                    && colors[0] as nat == crate::specs::part2_source_over_color_sample(source[0], destination[0], source_alpha, destination_alpha, premultiplied),
                i > 1 ==> (!premultiplied || source[1] <= source_alpha)
                    && colors[1] as nat == crate::specs::part2_source_over_color_sample(source[1], destination[1], source_alpha, destination_alpha, premultiplied),
                i > 2 ==> (!premultiplied || source[2] <= source_alpha)
                    && colors[2] as nat == crate::specs::part2_source_over_color_sample(source[2], destination[2], source_alpha, destination_alpha, premultiplied),
                i == 0 ==> colors[0] == destination[0],
                i <= 1 ==> colors[1] == destination[1],
                i <= 2 ==> colors[2] == destination[2],
            decreases channels - i,
        )
    )]
    while i < channels {
        if premultiplied && source[i] > source_alpha {
            return None;
        }
        let numerator = source_over_color_numerator(
            source[i],
            destination[i],
            source_alpha,
            destination_alpha,
            premultiplied,
        );
        let rounded = source_over_rounded_ratio(numerator, alpha_numerator);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(rounded <= 65535);
            assert(rounded as nat == crate::specs::part2_source_over_color_sample(
                source[i as int], destination[i as int], source_alpha, destination_alpha, premultiplied,
            ));
        }
        colors[i] = u16::try_from(rounded).ok()?;
        i += 1;
    }
    let alpha = source_over_rounded_ratio(alpha_numerator, u16::MAX as u64);
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(alpha <= 65535);
        assert(alpha as nat == crate::specs::part2_source_over_alpha_sample(source_alpha, destination_alpha));
    }
    Some((colors, u16::try_from(alpha).ok()?))
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(stored =>
        ensures (stored, crate::specs::part2_composition_planes(final(output_colors)@), final(output_alpha)@)
            == crate::specs::part2_composition_scatter_pixel(
                crate::specs::part2_composition_planes(old(output_colors)@),
                old(output_alpha)@, index as nat, pixel, alpha,
            ),
            final(output_colors)@.len() == old(output_colors)@.len(),
            final(output_alpha)@.len() == old(output_alpha)@.len(),
    )
)]
pub(crate) fn scatter_composition_pixel(
    output_colors: &mut [Vec<u16>],
    output_alpha: &mut [u16],
    index: usize,
    pixel: [u16; 3],
    alpha: u16,
) -> bool {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost initial_colors = crate::specs::part2_composition_planes(output_colors@);
        let ghost initial_alpha = output_alpha@;
    }
    let channels = output_colors.len();
    if channels > 3 {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::part2_composition_color_prefix(initial_colors, index as nat, pixel, 0) =~= initial_colors);
        }
        return false;
    }
    let mut channel = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant
                channel <= channels && channels <= 3,
                channels == output_colors.len(),
                channels == old(output_colors)@.len(),
                initial_colors.len() == channels,
                initial_colors == crate::specs::part2_composition_planes(old(output_colors)@),
                initial_alpha == old(output_alpha)@,
                output_alpha@ == initial_alpha,
                channel <= crate::specs::part2_composition_scatter_count(initial_colors, index as nat),
                crate::specs::part2_composition_planes(output_colors@)
                    == crate::specs::part2_composition_color_prefix(initial_colors, index as nat, pixel, channel as nat),
            decreases channels - channel,
        )
    )]
    while channel < channels {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost before_colors = output_colors@;
            let ghost before_planes = crate::specs::part2_composition_planes(output_colors@);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(before_planes[channel as int] == before_colors[channel as int]@);
            assert(before_colors[channel as int]@ == initial_colors[channel as int]);
        }
        let stored = {
            let samples = output_colors[channel].as_mut_slice();
            try_store_composition_sample(samples, index, pixel[channel])
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(output_colors@[channel as int]@ ==
                if stored { initial_colors[channel as int].update(index as int, pixel[channel as int]) }
                else { initial_colors[channel as int] });
        }
        if !stored {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(index >= initial_colors[channel as int].len());
                assert(crate::specs::part2_composition_scatter_count(initial_colors, index as nat) == channel);
            }
            return false;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert forall |other: int| 0 <= other < channels implies
                #[trigger] crate::specs::part2_composition_planes(output_colors@)[other]
                    == crate::specs::part2_composition_color_prefix(initial_colors, index as nat, pixel, (channel + 1) as nat)[other] by {
                if other != channel {
                    assert(output_colors@[other] == before_colors[other]);
                    assert(before_planes[other]
                        == crate::specs::part2_composition_color_prefix(initial_colors, index as nat, pixel, channel as nat)[other]);
                }
            }
            assert(crate::specs::part2_composition_planes(output_colors@)
                =~= crate::specs::part2_composition_color_prefix(initial_colors, index as nat, pixel, (channel + 1) as nat));
        }
        channel += 1;
    }
    try_store_composition_sample(output_alpha, index, alpha)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(pixel =>
        ensures pixel == crate::specs::part2_composition_gather_pixel(
            crate::specs::part2_composition_planes(source_colors@),
            match source_alpha {
                Some((samples, flag)) => Some((samples@, flag)),
                None => None,
            },
            crate::specs::part2_composition_planes(output_colors@), output_alpha@, source_index as nat, destination_index as nat, expand_grayscale,
        ),
    )
)]
pub(crate) fn gather_composition_pixel(
    source_colors: &[Vec<u16>],
    source_alpha: Option<(&[u16], bool)>,
    output_colors: &[Vec<u16>],
    output_alpha: &[u16],
    source_index: usize,
    destination_index: usize,
    expand_grayscale: bool,
) -> Option<CompositionPixelInputs> {
    let (source_alpha_sample, premultiplied) = match source_alpha {
        Some((samples, flag)) => (*samples.get(source_index)?, flag),
        None => (u16::MAX, false),
    };
    let destination_alpha = *output_alpha.get(destination_index)?;
    let channels = output_colors.len();
    if channels > 3 {
        return None;
    }
    let mut source_pixel = [0_u16; 3];
    let mut destination_pixel = [0_u16; 3];
    let mut channel = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant
                channel <= channels && channels <= 3,
                channels == output_colors.len(),
                source_alpha.is_some() ==> source_index < source_alpha.unwrap().0@.len()
                    && source_alpha_sample == source_alpha.unwrap().0@[source_index as int]
                    && premultiplied == source_alpha.unwrap().1,
                source_alpha.is_none() ==> source_alpha_sample == 65535 && !premultiplied,
                destination_index < output_alpha.len(),
                destination_alpha == output_alpha@[destination_index as int],
                channel > 0 ==> crate::specs::part2_composition_gather_channel(crate::specs::part2_composition_planes(source_colors@), crate::specs::part2_composition_planes(output_colors@), source_index as nat, destination_index as nat, expand_grayscale, 0) == Some((source_pixel[0], destination_pixel[0])),
                channel > 1 ==> crate::specs::part2_composition_gather_channel(crate::specs::part2_composition_planes(source_colors@), crate::specs::part2_composition_planes(output_colors@), source_index as nat, destination_index as nat, expand_grayscale, 1) == Some((source_pixel[1], destination_pixel[1])),
                channel > 2 ==> crate::specs::part2_composition_gather_channel(crate::specs::part2_composition_planes(source_colors@), crate::specs::part2_composition_planes(output_colors@), source_index as nat, destination_index as nat, expand_grayscale, 2) == Some((source_pixel[2], destination_pixel[2])),
                channel == 0 ==> source_pixel[0] == 0 && destination_pixel[0] == 0,
                channel <= 1 ==> source_pixel[1] == 0 && destination_pixel[1] == 0,
                channel <= 2 ==> source_pixel[2] == 0 && destination_pixel[2] == 0,
            decreases channels - channel,
        )
    )]
    while channel < channels {
        let source_channel = if expand_grayscale { 0 } else { channel };
        source_pixel[channel] = *source_colors.get(source_channel)?.get(source_index)?;
        destination_pixel[channel] = *output_colors.get(channel)?.get(destination_index)?;
        channel += 1;
    }
    Some((
        source_pixel,
        destination_pixel,
        source_alpha_sample,
        destination_alpha,
        premultiplied,
    ))
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(composed =>
        ensures (composed, crate::specs::part2_composition_planes(final(output_colors)@), final(output_alpha)@)
            == crate::specs::part2_composition_pixel_transition(
                crate::specs::part2_composition_planes(source_colors@),
                match source_alpha {
                    Some((samples, flag)) => Some((samples@, flag)),
                    None => None,
                },
                crate::specs::part2_composition_planes(old(output_colors)@), old(output_alpha)@,
                source_index as nat, destination_index as nat, expand_grayscale,
            ),
            final(output_colors)@.len() == old(output_colors)@.len(),
            final(output_alpha)@.len() == old(output_alpha)@.len(),
            !composed ==> crate::specs::part2_composition_planes(final(output_colors)@)
                == crate::specs::part2_composition_planes(old(output_colors)@)
                && final(output_alpha)@ == old(output_alpha)@,
    )
)]
pub(crate) fn compose_planar_pixel(
    source_colors: &[Vec<u16>],
    source_alpha: Option<(&[u16], bool)>,
    output_colors: &mut [Vec<u16>],
    output_alpha: &mut [u16],
    source_index: usize,
    destination_index: usize,
    expand_grayscale: bool,
) -> bool {
    let (s, d, sa, da, premultiplied) = match gather_composition_pixel(
        source_colors,
        source_alpha,
        output_colors,
        output_alpha,
        source_index,
        destination_index,
        expand_grayscale,
    ) {
        Some(value) => value,
        None => return false,
    };
    let (pixel, alpha) = match source_over_pixel(s, d, sa, da, premultiplied, output_colors.len()) {
        Some(value) => value,
        None => return false,
    };
    scatter_composition_pixel(output_colors, output_alpha, destination_index, pixel, alpha)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part2_chroma_key_alpha(matches),)
)]
pub(crate) const fn chroma_key_alpha(matches: bool) -> u16 {
    if matches { 0 } else { u16::MAX }
}
