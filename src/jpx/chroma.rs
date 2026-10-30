//! Chroma-key normalization and transparent-pixel policy.
#[cfg(verus_keep_ghost)]
use super::owned_layer_view;
use super::resources::{arithmetic_overflow, checked_area, invalid_layer};
use super::{ComponentBitDepth, DecodeError, MalformedKind, OwnedLayer};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures (result, final(keys)@, *final(offset)) == crate::specs::part2_append_component_key(
            values@, descriptor, old(keys)@, *old(offset)),
    )
)]
pub(super) fn append_component_chroma_key(
    values: &[u8],
    descriptor: (u8, bool),
    keys: &mut Vec<u16>,
    offset: &mut usize,
) -> Result<(), DecodeError> {
    let (bits, signed) = descriptor;
    let bytes = usize::from(bits.saturating_add(7) / 8);
    let end = offset.checked_add(bytes).ok_or_else(arithmetic_overflow)?;
    let key = normalized_chroma_keys(
        values.get(*offset..end).ok_or_else(invalid_layer)?,
        1,
        ComponentBitDepth::Uniform { bits, signed },
    )?;
    keys.push(*key.first().ok_or_else(invalid_layer)?);
    *offset = end;
    Ok(())
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures crate::color::normalized_plane_result(result)
            == crate::specs::part2_normalized_component_chroma_keys(values@, descriptors@),
    )
)]
pub(super) fn normalized_chroma_keys_for_components(
    values: &[u8],
    descriptors: &[(u8, bool)],
) -> Result<Vec<u16>, DecodeError> {
    let mut expected = 0_usize;
    let mut index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! { crate::proofs::production_impl::production_chroma_descriptors_empty(descriptors@); }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= descriptors.len(),
                crate::specs::part2_chroma_descriptor_bytes(descriptors@, index as nat) == Ok(expected),
            decreases descriptors.len() - index,
        )
    )]
    while index < descriptors.len() {
        let bits = descriptors.get(index).ok_or_else(invalid_layer)?.0;
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_chroma_descriptors_step(descriptors@, index as nat);
            if bits == 0 || bits > 38 {
                crate::proofs::production_impl::production_chroma_descriptors_error(
                    descriptors@, index as nat + 1, descriptors@.len());
            } else if expected as nat + (bits as nat + 7) / 8 > usize::MAX as nat {
                crate::proofs::production_impl::production_chroma_descriptors_error(
                    descriptors@, index as nat + 1, descriptors@.len());
            }
        }
        if bits == 0 || bits > 38 {
            return Err(invalid_layer());
        }
        expected = expected
            .checked_add(usize::from(bits.saturating_add(7) / 8))
            .ok_or_else(arithmetic_overflow)?;
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    if values.len() != expected {
        return Err(invalid_layer());
    }
    let mut keys = Vec::with_capacity(descriptors.len());
    let mut offset = 0_usize;
    index = 0;
    #[cfg(verus_keep_ghost)]
    proof! { crate::proofs::production_impl::production_component_keys_empty(values@, descriptors@); }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= descriptors.len(),
                crate::specs::part2_chroma_descriptor_bytes(descriptors@, descriptors@.len()) == Ok(expected),
                values.len() == expected,
                crate::specs::part2_component_keys_prefix(values@, descriptors@, index as nat)
                    == (Ok(()), keys@, offset),
            decreases descriptors.len() - index,
        )
    )]
    while index < descriptors.len() {
        let descriptor = *descriptors.get(index).ok_or_else(invalid_layer)?;
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_component_keys_step(values@, descriptors@, index as nat); }
        let result = append_component_chroma_key(values, descriptor, &mut keys, &mut offset);
        #[cfg(verus_keep_ghost)]
        proof! {
            if let Err(error) = result {
                assert(crate::specs::part2_component_keys_prefix(values@, descriptors@, index as nat + 1)
                    == (Err(error), keys@, offset));
                crate::proofs::production_impl::production_component_keys_error(
                    values@, descriptors@, index as nat + 1, descriptors@.len());
            } else {
                crate::proofs::production_impl::production_mct_unit_result(result);
            }
        }
        result?;
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(keys)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures (result, owned_layer_view(final(owned)))
            == crate::specs::part2_apply_chroma_key(owned_layer_view(old(owned)), keys@),
    )
)]
pub(super) fn apply_chroma_key(owned: &mut OwnedLayer, keys: &[u16]) -> Result<(), DecodeError> {
    apply_chroma_key_with_policy(owned, keys, false)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures (result, owned_layer_view(final(owned)))
            == crate::specs::part2_apply_chroma_key_policy(owned_layer_view(old(owned)), keys@, reject_missing),
    )
)]
pub(super) fn apply_chroma_key_with_policy(
    owned: &mut OwnedLayer,
    keys: &[u16],
    reject_missing: bool,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost input = owned_layer_view(owned); }
    if keys.len() != owned.colors.len() {
        return Err(invalid_layer());
    }
    let pixels = checked_area(owned.width, owned.height)?;
    let mut alpha = vec![u16::MAX; pixels];
    let mut pixel = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant pixel <= pixels, alpha.len() == pixels,
                owned_layer_view(owned) == input,
                input == owned_layer_view(old(owned)),
                keys.len() == input.3.len(),
                crate::specs::part2_layer_area(input.0, input.1) == Ok(pixels),
                reject_missing ==> crate::specs::part2_chroma_sources_cover(input.3, pixel as nat),
                forall|index: int| 0 <= index < pixel ==>
                    alpha@[index] == crate::specs::part2_chroma_key_pixel(input.3, keys@, index),
            decreases pixels - pixel,
        )
    )]
    while pixel < pixels {
        let mut matches = true;
        let mut color = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant color <= owned.colors.len(),
                    owned_layer_view(owned) == input,
                    input == owned_layer_view(old(owned)),
                    keys.len() == input.3.len(),
                    pixel < pixels,
                    crate::specs::part2_layer_area(input.0, input.1) == Ok(pixels),
                    reject_missing ==> crate::proofs::internal_specs::part2_chroma_cover_prefix(
                        input.3, pixel as nat + 1, color as nat),
                    matches == crate::proofs::internal_specs::part2_chroma_key_prefix_matches(
                        input.3, keys@, pixel as int, color as nat),
                decreases owned.colors.len() - color,
            )
        )]
        while color < owned.colors.len() {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost previous = matches; }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(color < input.3.len());
                assert(owned.colors@[color as int]@ == input.3[color as int]);
            }
            let sample = match owned.colors.get(color) {
                Some(samples) => samples.get(pixel),
                None => None,
            };
            let key = keys.get(color);
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(key.is_some());
                assert(sample.is_some() == (pixel < input.3[color as int].len()));
            }
            if reject_missing && sample.is_none() {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(!crate::specs::part2_chroma_plane_has_samples(input.3[color as int], pixels as nat));
                    if crate::specs::part2_chroma_sources_cover(input.3, pixels as nat) {
                        assert(crate::specs::part2_chroma_plane_has_samples(input.3[color as int], pixels as nat));
                        assert(false);
                    }
                }
                return Err(invalid_layer());
            }
            let matches_key = match (sample, key) {
                (Some(sample), Some(key)) => *sample == *key,
                (None, None) => true,
                _ => false,
            };
            if !matches_key {
                matches = false;
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(key.is_some());
                if let Some(value) = key { assert(*value == keys@[color as int]); }
                assert(sample.is_some() == (pixel < input.3[color as int].len()));
                if let Some(value) = sample { assert(*value == input.3[color as int][pixel as int]); }
                assert(matches_key == (pixel < input.3[color as int].len()
                    && input.3[color as int][pixel as int] == keys@[color as int]));
                assert(matches == (previous && matches_key));
                crate::proofs::production_impl::production_chroma_key_prefix_step(
                    input.3, keys@, pixel as int, color as nat);
                if reject_missing {
                    assert(crate::specs::part2_chroma_plane_has_samples(input.3[color as int], pixel as nat + 1));
                    crate::proofs::production_impl::production_chroma_cover_step(input.3, pixel as nat + 1, color as nat);
                }
            }
            color = color.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_chroma_key_prefix_complete(input.3, keys@, pixel as int);
            if reject_missing {
                crate::proofs::production_impl::production_chroma_cover_complete(input.3, pixel as nat + 1);
            }
            assert(crate::specs::part2_chroma_key_pixel(input.3, keys@, pixel as int)
                == crate::specs::part2_chroma_key_alpha(matches));
        }
        *alpha.get_mut(pixel).ok_or_else(invalid_layer)? =
            crate::refinement::chroma_key_alpha(matches);
        pixel = pixel.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(alpha@ =~= Seq::new(pixels as nat, |index: int|
            crate::specs::part2_chroma_key_pixel(input.3, keys@, index)));
    }
    owned.alpha = Some((alpha, false));
    Ok(())
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures crate::color::normalized_plane_result(result)
            == crate::specs::part2_normalized_chroma_keys(values@, channels, bit_depth),
    )
)]
pub(super) fn normalized_chroma_keys(
    values: &[u8],
    channels: u8,
    bit_depth: ComponentBitDepth,
) -> Result<Vec<u16>, DecodeError> {
    let ComponentBitDepth::Uniform { bits, signed } = bit_depth else {
        return Err(DecodeError::MalformedInput {
            offset: 0,
            kind: MalformedKind::MissingRequiredStructure,
        });
    };
    if bits == 0 || bits > 38 {
        return Err(invalid_layer());
    }
    let bytes = usize::from(bits.saturating_add(7) / 8);
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(1 <= bytes <= 5);
        assert(bytes as nat * channels as nat <= 1275) by(nonlinear_arith)
            requires bytes <= 5, channels <= 255;
    }
    if values.len()
        != bytes
            .checked_mul(usize::from(channels))
            .ok_or_else(arithmetic_overflow)?
    {
        return Err(invalid_layer());
    }
    let mut keys = Vec::with_capacity(usize::from(channels));
    let mut channel = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(1 <= bytes <= 5);
        reveal_with_fuel(crate::specs::part2_chroma_key_bytes, 1);
        reveal_with_fuel(crate::proofs::internal_specs::part2_chroma_byte_bound, 1);
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant channel <= channels as usize, keys.len() == channel,
                bit_depth == (ComponentBitDepth::Uniform { bits, signed }),
                0 < bits <= 38, 1 <= bytes <= 5,
                bytes as nat == (bits as nat + 7) / 8,
                values.len() == bytes as nat * channels as nat,
                forall|index: int| 0 <= index < channel ==> keys@[index]
                    == crate::specs::part2_chroma_key_normalize(crate::specs::part2_chroma_key_bytes(
                        values@, (index * bytes) as nat, bytes as nat), bits, signed),
            decreases channels as usize - channel,
        )
    )]
    while channel < usize::from(channels) {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(channel as nat * bytes as nat <= 1275) by(nonlinear_arith)
                requires channel < channels as usize, channels <= 255, bytes <= 5;
        }
        let start = channel.checked_mul(bytes).ok_or_else(arithmetic_overflow)?;
        let mut raw = 0_u64;
        let mut byte = 0_usize;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(start as nat + bytes as nat <= values.len()) by(nonlinear_arith)
                requires start as nat == channel as nat * bytes as nat,
                    channel < channels as usize, bytes > 0,
                    values.len() == bytes as nat * channels as nat;
            reveal_with_fuel(crate::specs::part2_chroma_key_bytes, 1);
            reveal_with_fuel(crate::proofs::internal_specs::part2_chroma_byte_bound, 1);
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant byte <= bytes, 1 <= bytes <= 5,
                    0 < bits <= 38, bit_depth == (ComponentBitDepth::Uniform { bits, signed }),
                    bytes as nat == (bits as nat + 7) / 8,
                    channel < channels as usize, keys.len() == channel,
                    values.len() == bytes as nat * channels as nat,
                    start as nat == channel as nat * bytes as nat,
                    start as nat + bytes as nat <= values.len(),
                    forall|index: int| 0 <= index < channel ==> keys@[index]
                        == crate::specs::part2_chroma_key_normalize(crate::specs::part2_chroma_key_bytes(
                            values@, (index * bytes) as nat, bytes as nat), bits, signed),
                    raw as nat == crate::specs::part2_chroma_key_bytes(values@, start as nat, byte as nat),
                    raw as nat <= crate::proofs::internal_specs::part2_chroma_byte_bound(byte as nat),
                decreases bytes - byte,
            )
        )]
        while byte < bytes {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_chroma_byte_bound(byte as nat);
                crate::proofs::production_impl::production_chroma_bytes_step(values@, start as nat, byte as nat);
            }
            raw = raw.checked_mul(256).ok_or_else(arithmetic_overflow)?;
            let position = start.checked_add(byte).ok_or_else(arithmetic_overflow)?;
            let sample = *values.get(position).ok_or_else(arithmetic_overflow)?;
            raw = raw
                .checked_add(u64::from(sample))
                .ok_or_else(arithmetic_overflow)?;
            byte = byte.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::spec_properties::color_precision_binary_bounds(bits); }
        let mask = (1_u64 << bits)
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?;
        if mask == 0 {
            return Err(invalid_layer());
        }
        let masked = raw & mask;
        let biased = if signed {
            let sign_bit = mask.checked_add(1).ok_or_else(arithmetic_overflow)? / 2;
            masked ^ sign_bit
        } else {
            masked
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_chroma_normalization(raw, bits, signed);
        }
        keys.push(crate::color::normalize_unsigned(biased, bits)?);
        channel = channel.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(keys@ =~= Seq::new(channels as nat, |index: int|
            crate::specs::part2_chroma_key_normalize(crate::specs::part2_chroma_key_bytes(
                values@, (index * bytes) as nat, bytes as nat), bits, signed)));
    }
    Ok(keys)
}
