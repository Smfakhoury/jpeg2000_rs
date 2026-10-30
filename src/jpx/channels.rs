//! Converted-image channel selection and alpha-plane folding.
use super::resources::{arithmetic_overflow, checked_area, copy_samples, invalid_layer};
use super::{
    ConvertedChannelType, ConvertedColorSpace, ConvertedImage, DecodeError, LayerChannels,
};
#[cfg(verus_keep_ghost)]
use super::{
    layer_alpha_planes, layer_borrowed_planes, layer_borrowed_planes_push, layer_channels_outcome,
    layer_color_slots, layer_owned_alpha,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[allow(clippy::collapsible_if)]
#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures layer_channels_outcome(result) == crate::specs::part2_layer_input(
            crate::color::converted_image_view(image).0, crate::color::converted_image_view(image).1,
            crate::color::converted_image_view(image).2,
            crate::color::converted_channel_views(crate::color::converted_image_view(image).4),
        ),
    )
)]
pub(super) fn layer_channels(image: &ConvertedImage) -> Result<LayerChannels<'_>, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost input = crate::color::converted_channel_views(crate::color::converted_image_view(image).4);
    }
    let color_count = match image.color_space() {
        ConvertedColorSpace::Srgb => 3_usize,
        ConvertedColorSpace::Grayscale => 1_usize,
    };
    let mut colors: Vec<Option<&[u16]>> = vec![None; color_count];
    let mut alphas = Vec::new();
    let channel_count = image.channels().len();
    let mut index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_layer_scan_empty(input, color_count as nat);
        assert(layer_color_slots(colors@) =~= Seq::new(color_count as nat, |i: int| None));
        assert(layer_alpha_planes(alphas@) =~= Seq::empty());
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= channel_count,
                input == crate::color::converted_channel_views(crate::color::converted_image_view(image).4),
                channel_count == input.len(),
                color_count as nat == crate::specs::part2_layer_color_count(crate::color::converted_image_view(image).2),
                colors.len() == color_count,
                crate::specs::part2_layer_channel_scan(input, color_count as nat, index as nat)
                    == Ok((layer_color_slots(colors@), layer_alpha_planes(alphas@))),
            decreases channel_count - index,
        )
    )]
    while index < channel_count {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost scan_colors = layer_color_slots(colors@);
            let ghost scan_alphas = layer_alpha_planes(alphas@);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_layer_scan_step(input, color_count as nat, index as nat);
        }
        let channel = image.channels().get(index).ok_or_else(invalid_layer)?;
        let color_index = match channel.channel_type() {
            ConvertedChannelType::Red => Some(0),
            ConvertedChannelType::Green => Some(1),
            ConvertedChannelType::Blue => Some(2),
            ConvertedChannelType::Gray => Some(0),
            ConvertedChannelType::Alpha {
                premultiplied,
                association,
            } => {
                let _ = association;
                alphas.push((channel.samples(), premultiplied));
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(layer_alpha_planes(alphas@) =~= scan_alphas.push((crate::color::converted_channel_view(channel).1, premultiplied)));
                }
                None
            }
        };
        if let Some(color_index) = color_index {
            if color_index >= colors.len() {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_layer_scan_error(
                        input, color_count as nat, index as nat + 1, input.len(), crate::specs::part2_composition_error(false),
                    );
                }
                return Err(invalid_layer());
            }
            if colors[color_index].is_some() {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_layer_scan_error(
                        input, color_count as nat, index as nat + 1, input.len(), crate::specs::part2_composition_error(false),
                    );
                }
                return Err(invalid_layer());
            }
            *colors.get_mut(color_index).ok_or_else(invalid_layer)? = Some(channel.samples());
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(layer_color_slots(colors@) =~= scan_colors.update(color_index as int, Some(crate::color::converted_channel_view(channel).1)));
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::part2_layer_channel_scan(input, color_count as nat, index as nat + 1)
                == Ok((layer_color_slots(colors@), layer_alpha_planes(alphas@))));
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost scanned_colors = layer_color_slots(colors@);
        let ghost scanned_alphas = layer_alpha_planes(alphas@);
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part2_layer_area);
    }
    let expected = checked_area(image.width(), image.height())?;
    let mut resolved = Vec::with_capacity(color_count);
    index = 0;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= colors.len(),
                resolved.len() == index,
                input == crate::color::converted_channel_views(crate::color::converted_image_view(image).4),
                colors.len() == color_count,
                color_count as nat == crate::specs::part2_layer_color_count(crate::color::converted_image_view(image).2),
                expected as nat == (crate::color::converted_image_view(image).0 as nat) * (crate::color::converted_image_view(image).1 as nat),
                crate::specs::part2_layer_channel_scan(input, color_count as nat, input.len()) == Ok((scanned_colors, scanned_alphas)),
                scanned_colors == layer_color_slots(colors@),
                scanned_alphas == layer_alpha_planes(alphas@),
                forall |i: int| #![trigger scanned_colors[i]] 0 <= i < index ==> scanned_colors[i].is_some() && scanned_colors[i].unwrap().len() == expected,
                layer_borrowed_planes(resolved@) == Seq::new(index as nat, |i: int| scanned_colors[i].unwrap()),
            decreases colors.len() - index,
        )
    )]
    while index < colors.len() {
        let samples = match *colors.get(index).ok_or_else(invalid_layer)? {
            Some(samples) => samples,
            None => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(scanned_colors[index as int].is_none());
                }
                return Err(invalid_layer());
            }
        };
        if samples.len() != expected {
            return Err(invalid_layer());
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(samples@ == scanned_colors[index as int].unwrap());
            layer_borrowed_planes_push(resolved@, samples);
        }
        resolved.push(samples);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(layer_borrowed_planes(resolved@) =~= Seq::new(index as nat + 1, |i: int| scanned_colors[i].unwrap()));
        }
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    let mut resolved_alpha = None;
    let mut alpha_index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(layer_borrowed_planes(resolved@) =~= Seq::new(scanned_colors.len(), |i: int| scanned_colors[i].unwrap()));
        crate::proofs::production_impl::production_layer_alpha_empty(scanned_alphas, expected as nat);
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant alpha_index <= alphas.len(),
                input == crate::color::converted_channel_views(crate::color::converted_image_view(image).4),
                color_count as nat == crate::specs::part2_layer_color_count(crate::color::converted_image_view(image).2),
                expected as nat == (crate::color::converted_image_view(image).0 as nat) * (crate::color::converted_image_view(image).1 as nat),
                crate::specs::part2_layer_channel_scan(input, color_count as nat, input.len()) == Ok((scanned_colors, scanned_alphas)),
                scanned_alphas == layer_alpha_planes(alphas@),
                crate::specs::part2_layer_resolve_colors(scanned_colors, expected as nat) == Ok(layer_borrowed_planes(resolved@)),
                crate::specs::part2_layer_alpha_fold(scanned_alphas, expected as nat, alpha_index as nat) == Ok(layer_owned_alpha(resolved_alpha)),
                alpha_index == 0 ==> resolved_alpha.is_none(),
                alpha_index > 0 ==> resolved_alpha.is_some() && resolved_alpha.unwrap().0.len() == expected,
            decreases alphas.len() - alpha_index,
        )
    )]
    while alpha_index < alphas.len() {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost previous_alpha = layer_owned_alpha(resolved_alpha);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_layer_alpha_step(scanned_alphas, expected as nat, alpha_index as nat);
        }
        let (samples, premultiplied) = *alphas.get(alpha_index).ok_or_else(invalid_layer)?;
        if samples.len() != expected {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_layer_alpha_error(
                    scanned_alphas, expected as nat, alpha_index as nat + 1, scanned_alphas.len(), crate::specs::part2_composition_error(false),
                );
            }
            return Err(invalid_layer());
        }
        if alpha_index == 0 {
            resolved_alpha = Some((copy_samples(samples)?, premultiplied));
        } else {
            let (combined, combined_premultiplied) =
                resolved_alpha.as_mut().ok_or_else(invalid_layer)?;
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost initial_combined = combined@;
            }
            let mut pixel = 0_usize;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(combined@ =~= crate::proofs::internal_specs::part2_layer_alpha_product_prefix(initial_combined, samples@, 0));
            }
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant pixel <= combined.len(),
                        combined.len() == expected,
                        samples.len() == expected,
                        initial_combined.len() == expected,
                        combined@ == crate::proofs::internal_specs::part2_layer_alpha_product_prefix(initial_combined, samples@, pixel as nat),
                    decreases combined.len() - pixel,
                )
            )]
            while pixel < combined.len() {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_layer_alpha_product_step(initial_combined, samples@, pixel as nat);
                }
                let combined_sample = *combined.get(pixel).ok_or_else(invalid_layer)?;
                let alpha_sample = *samples.get(pixel).ok_or_else(invalid_layer)?;
                *combined.get_mut(pixel).ok_or_else(invalid_layer)? =
                    crate::refinement::normalized_sample_product(combined_sample, alpha_sample)
                        .ok_or_else(arithmetic_overflow)?;
                pixel = pixel.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(combined@ =~= Seq::new(expected as nat, |i: int| crate::specs::part2_normalized_sample_product(initial_combined[i], samples@[i]) as u16));
            }
            *combined_premultiplied = false;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::part2_layer_alpha_fold(scanned_alphas, expected as nat, alpha_index as nat + 1)
                == Ok(layer_owned_alpha(resolved_alpha)));
        }
        alpha_index = alpha_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(LayerChannels {
        width: image.width(),
        height: image.height(),
        colors: resolved,
        alpha: resolved_alpha,
    })
}
