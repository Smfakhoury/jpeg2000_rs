//! Placed-layer compositing and checked pixel blending.
use super::resources::{arithmetic_overflow, enforce_limit, index_of, invalid_layer};
use super::{
    ConvertedColorSpace, DecodeError, JpxInstructionInfo, OwnedLayer, Resource, ResourceLimits,
};
#[cfg(test)]
use super::{LayerChannels, MalformedKind};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verifier::spinoff_prover)]
#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures result.is_ok() ==> (result.is_ok(), crate::specs::part2_composition_planes(final(output_colors)@), final(output_alpha)@, *final(work_units) as nat)
            == crate::specs::part2_composition_layer_success(
                crate::specs::part2_composition_planes(source.colors@),
                match &source.alpha { Some(value) => Some((value.0@, value.1)), None => None },
                crate::specs::part2_composition_planes(old(output_colors)@), old(output_alpha)@,
                crate::specs::part2_composition_geometry(
                    source.width, source.height, canvas_width, canvas_height,
                    instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
                    instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
                ),
                matches!((output_space, source.color_space), (ConvertedColorSpace::Srgb, ConvertedColorSpace::Grayscale)),
                *old(work_units) as nat,
            ),
            crate::specs::part2_composition_layer_prefix_safe(
                crate::specs::part2_composition_planes(source.colors@),
                match &source.alpha { Some(value) => Some((value.0@, value.1)), None => None },
                crate::specs::part2_composition_planes(old(output_colors)@), old(output_alpha)@,
                crate::specs::part2_composition_geometry(
                    source.width, source.height, canvas_width, canvas_height,
                    instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
                    instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
                ),
                matches!((output_space, source.color_space), (ConvertedColorSpace::Srgb, ConvertedColorSpace::Grayscale)),
                *old(work_units) as nat,
                crate::specs::part2_composition_planes(final(output_colors)@), final(output_alpha)@, *final(work_units) as nat,
            ),
            (result, crate::specs::part2_composition_planes(final(output_colors)@), final(output_alpha)@, *final(work_units) as nat)
                == crate::specs::part2_composition_layer_transition(
                    crate::specs::part2_composition_planes(source.colors@),
                    match &source.alpha { Some(value) => Some((value.0@, value.1)), None => None },
                    crate::specs::part2_composition_planes(old(output_colors)@), old(output_alpha)@,
                    crate::specs::part2_composition_geometry(
                        source.width, source.height, canvas_width, canvas_height,
                        instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
                        instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
                    ),
                    matches!((output_space, source.color_space), (ConvertedColorSpace::Srgb, ConvertedColorSpace::Grayscale)),
                    *old(work_units) as nat, crate::limits::resource_work_limit(limits),
                ),
    )
)]
pub(super) fn composite_placed_layer(
    canvas_width: u32,
    canvas_height: u32,
    output_space: ConvertedColorSpace,
    source: &OwnedLayer,
    instruction: JpxInstructionInfo,
    output_colors: &mut [Vec<u16>],
    output_alpha: &mut [u16],
    work_units: &mut u64,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let expand_grayscale = matches!(
        (output_space, source.color_space),
        (ConvertedColorSpace::Srgb, ConvertedColorSpace::Grayscale)
    );
    #[allow(
        clippy::manual_map,
        reason = "Explicit cases preserve the verified slice and representation relation"
    )]
    let source_alpha = match source.alpha.as_ref() {
        Some((samples, flag)) => Some((samples.as_slice(), *flag)),
        None => None,
    };
    let maximum_work_units = limits.maximum_work_units();
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost geometry = crate::specs::part2_composition_geometry(
            source.width, source.height, canvas_width, canvas_height,
            instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
            instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
        );
        let ghost source_planes = crate::specs::part2_composition_planes(source.colors@);
        let ghost layer_alpha = match &source.alpha { Some(value) => Some((value.0@, value.1)), None => None };
        let ghost initial_colors = crate::specs::part2_composition_planes(output_colors@);
        let ghost initial_alpha = output_alpha@;
        let ghost initial_work = *work_units;
    }
    let source_x = instruction.source_x.min(source.width);
    let source_y = instruction.source_y.min(source.height);
    let source_width = if instruction.source_width == 0 {
        source.width.saturating_sub(source_x)
    } else {
        instruction
            .source_width
            .min(source.width.saturating_sub(source_x))
    };
    let source_height = if instruction.source_height == 0 {
        source.height.saturating_sub(source_y)
    } else {
        instruction
            .source_height
            .min(source.height.saturating_sub(source_y))
    };
    if source_width == 0 || source_height == 0 {
        return Ok(());
    }
    let target_width = instruction.target_width;
    let target_height = instruction.target_height;
    if target_width == 0 || target_height == 0 {
        return Err(invalid_layer());
    }
    let copy_width =
        crate::refinement::composition_clip(instruction.target_x, target_width, canvas_width);
    let copy_height =
        crate::refinement::composition_clip(instruction.target_y, target_height, canvas_height);
    if copy_width == 0 || copy_height == 0 {
        return Ok(());
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_composition_canvas_empty(
            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
        );
        crate::proofs::production_impl::production_composition_budgeted_empty(
            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
            initial_work as nat, maximum_work_units,
        );
        crate::proofs::production_impl::production_composition_prefix_observation(
            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, initial_work as nat,
            crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat, 0,
        );
    }
    let mut y = 0_u32;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant
                y <= copy_height,
                copy_width > 0 && copy_height > 0,
                geometry == crate::specs::part2_composition_geometry(
                    source.width, source.height, canvas_width, canvas_height,
                    instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
                    instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
                ),
                geometry.source_x == source_x as nat && geometry.source_y == source_y as nat,
                geometry.source_crop_width == source_width as nat && geometry.source_crop_height == source_height as nat,
                geometry.target_width == target_width as nat && geometry.target_height == target_height as nat,
                geometry.copy_width == copy_width as nat && geometry.copy_height == copy_height as nat,
                source_planes == crate::specs::part2_composition_planes(source.colors@),
                layer_alpha == match &source.alpha { Some(value) => Some((value.0@, value.1)), None => None },
                layer_alpha == match source_alpha { Some((samples, flag)) => Some((samples@, flag)), None => None },
                expand_grayscale == matches!((output_space, source.color_space), (ConvertedColorSpace::Srgb, ConvertedColorSpace::Grayscale)),
                initial_colors == crate::specs::part2_composition_planes(old(output_colors)@),
                initial_alpha == old(output_alpha)@,
                initial_work == *old(work_units),
                maximum_work_units == crate::limits::resource_work_limit(limits),
                output_colors.len() == initial_colors.len(),
                (Ok(()), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                    == crate::specs::part2_composition_budgeted_prefix(
                        source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                        initial_work as nat, maximum_work_units, (y as nat) * (copy_width as nat),
                    ),
                (true, crate::specs::part2_composition_planes(output_colors@), output_alpha@)
                    == crate::specs::part2_composition_canvas_prefix(source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, (y as nat) * (copy_width as nat)),
                *work_units as nat == initial_work as nat + (y as nat) * (copy_width as nat) * (initial_colors.len() + 1),
                crate::specs::part2_composition_layer_prefix_safe(
                    source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, initial_work as nat,
                    crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat,
                ),
                target_width > 0 && target_height > 0,
                copy_width <= target_width && copy_height <= target_height,
                source_x <= source.width && source_y <= source.height,
                source_width > 0 && source_height > 0,
                source_width <= source.width - source_x,
                source_height <= source.height - source_y,
                copy_width > 0 ==> instruction.target_x < canvas_width
                    && copy_width <= canvas_width - instruction.target_x,
                copy_height > 0 ==> instruction.target_y < canvas_height
                    && copy_height <= canvas_height - instruction.target_y,
            decreases copy_height - y,
        )
    )]
    while y < copy_height {
        let mapped_y =
            crate::refinement::composition_mapped_offset(y, source_height, target_height);
        let sy = source_y
            .checked_add(u32::try_from(mapped_y).map_err(|_| arithmetic_overflow())?)
            .ok_or_else(arithmetic_overflow)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(source_y <= sy && sy < source.height);
            assert((sy as nat) < (source_y as nat) + source_height as nat);
            assert(sy as nat == (source_y as nat)
                + crate::specs::part2_composition_mapped_offset(y, source_height, target_height));
        }
        let mut x = 0_u32;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant
                    x <= copy_width,
                    copy_width > 0 && copy_height > 0,
                    geometry == crate::specs::part2_composition_geometry(
                        source.width, source.height, canvas_width, canvas_height,
                        instruction.target_x, instruction.target_y, instruction.target_width, instruction.target_height,
                        instruction.source_x, instruction.source_y, instruction.source_width, instruction.source_height,
                    ),
                    geometry.source_x == source_x as nat && geometry.source_y == source_y as nat,
                    geometry.source_crop_width == source_width as nat && geometry.source_crop_height == source_height as nat,
                    geometry.target_width == target_width as nat && geometry.target_height == target_height as nat,
                    geometry.copy_width == copy_width as nat && geometry.copy_height == copy_height as nat,
                    source_planes == crate::specs::part2_composition_planes(source.colors@),
                    layer_alpha == match &source.alpha { Some(value) => Some((value.0@, value.1)), None => None },
                    layer_alpha == match source_alpha { Some((samples, flag)) => Some((samples@, flag)), None => None },
                    expand_grayscale == matches!((output_space, source.color_space), (ConvertedColorSpace::Srgb, ConvertedColorSpace::Grayscale)),
                    initial_colors == crate::specs::part2_composition_planes(old(output_colors)@),
                    initial_alpha == old(output_alpha)@,
                    initial_work == *old(work_units),
                    maximum_work_units == crate::limits::resource_work_limit(limits),
                    output_colors.len() == initial_colors.len(),
                    (Ok(()), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                        == crate::specs::part2_composition_budgeted_prefix(
                            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                            initial_work as nat, maximum_work_units, (y as nat) * (copy_width as nat) + x as nat,
                        ),
                    (true, crate::specs::part2_composition_planes(output_colors@), output_alpha@)
                        == crate::specs::part2_composition_canvas_prefix(source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, (y as nat) * (copy_width as nat) + x as nat),
                    *work_units as nat == initial_work as nat + ((y as nat) * (copy_width as nat) + x as nat) * (initial_colors.len() + 1),
                    crate::specs::part2_composition_layer_prefix_safe(
                        source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, initial_work as nat,
                        crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat,
                    ),
                    target_width > 0 && target_height > 0,
                    copy_width <= target_width,
                    y < copy_height && copy_height <= target_height,
                    source_height > 0,
                    source_x <= source.width && source_width > 0,
                    source_width <= source.width - source_x,
                    copy_width > 0 ==> instruction.target_x < canvas_width
                        && copy_width <= canvas_width - instruction.target_x,
                    sy < source.height,
                    source_y <= sy,
                    (sy as nat) < (source_y as nat) + source_height as nat,
                    sy as nat == (source_y as nat)
                        + crate::specs::part2_composition_mapped_offset(y, source_height, target_height),
                    (instruction.target_y as nat) + (y as nat) < canvas_height as nat,
                decreases copy_width - x,
            )
        )]
        while x < copy_width {
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost completed = (y as nat) * (copy_width as nat) + x as nat;
                let ghost previous_state = crate::specs::part2_composition_canvas_prefix(
                    source_planes, layer_alpha, initial_colors, initial_alpha,
                    geometry, expand_grayscale, completed,
                );
                let ghost previous_work = *work_units;
                let ghost previous_budgeted = crate::specs::part2_composition_budgeted_prefix(
                    source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                    initial_work as nat, maximum_work_units, completed,
                );
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(previous_state == (true, crate::specs::part2_composition_planes(output_colors@), output_alpha@));
                assert(previous_work as nat == initial_work as nat + completed * (initial_colors.len() + 1));
                assert(previous_budgeted == (Ok(()), crate::specs::part2_composition_planes(output_colors@), output_alpha@, previous_work as nat));
                crate::proofs::production_impl::production_composition_raster_prefix_bound(copy_width as nat, copy_height as nat, x as nat, y as nat);
                crate::proofs::production_impl::production_composition_budgeted_step(
                    source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                    initial_work as nat, maximum_work_units, x as nat, y as nat,
                );
            }
            let mapped_x =
                crate::refinement::composition_mapped_offset(x, source_width, target_width);
            let sx = source_x
                .checked_add(u32::try_from(mapped_x).map_err(|_| arithmetic_overflow())?)
                .ok_or_else(arithmetic_overflow)?;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(sx as nat == geometry.source_x + (x as nat) * geometry.source_crop_width / geometry.target_width);
                assert(sy as nat == geometry.source_y + (y as nat) * geometry.source_crop_height / geometry.target_height);
            }
            #[allow(
                clippy::question_mark,
                reason = "The index-error branch proves its exact unchanged state"
            )]
            let source_index = match index_of(source.width, sx, sy) {
                Ok(value) => value,
                Err(error) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert((Err(error), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                            == crate::specs::part2_composition_budgeted_prefix(
                                source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                                initial_work as nat, maximum_work_units, completed + 1,
                            ));
                        crate::proofs::production_impl::production_composition_budgeted_failure_absorbs(
                            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                            initial_work as nat, maximum_work_units, completed + 1, geometry.copy_width * geometry.copy_height,
                        );
                    }
                    return Err(error);
                }
            };
            let destination_x = instruction
                .target_x
                .checked_add(x)
                .ok_or_else(arithmetic_overflow)?;
            let destination_y = instruction
                .target_y
                .checked_add(y)
                .ok_or_else(arithmetic_overflow)?;
            #[allow(
                clippy::question_mark,
                reason = "The index-error branch proves its exact unchanged state"
            )]
            let destination_index = match index_of(canvas_width, destination_x, destination_y) {
                Ok(value) => value,
                Err(error) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert((Err(error), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                            == crate::specs::part2_composition_budgeted_prefix(
                                source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                                initial_work as nat, maximum_work_units, completed + 1,
                            ));
                        crate::proofs::production_impl::production_composition_budgeted_failure_absorbs(
                            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                            initial_work as nat, maximum_work_units, completed + 1, geometry.copy_width * geometry.copy_height,
                        );
                    }
                    return Err(error);
                }
            };
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(source_x <= sx && sx < source.width);
                assert((sx as nat) < (source_x as nat) + source_width as nat);
                assert(sx as nat == (source_x as nat)
                    + crate::specs::part2_composition_mapped_offset(x, source_width, target_width));
                assert(destination_x < canvas_width && destination_y < canvas_height);
                crate::proofs::production_impl::production_composition_index_bounds(source.width, source.height, sx, sy);
                crate::proofs::production_impl::production_composition_index_bounds(canvas_width, canvas_height, destination_x, destination_y);
                assert(source_index as nat == crate::specs::part2_row_major_index(source.width, sx, sy));
                assert(destination_index as nat == crate::specs::part2_row_major_index(canvas_width, destination_x, destination_y));
                assert((source_index as nat) < (source.width as nat) * (source.height as nat));
                assert((destination_index as nat) < (canvas_width as nat) * (canvas_height as nat));
                crate::proofs::production_impl::production_composition_canvas_step(
                    source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                    x as nat, y as nat, source_index as nat, destination_index as nat,
                );
            }
            if !crate::refinement::compose_planar_pixel(
                &source.colors,
                source_alpha,
                output_colors,
                output_alpha,
                source_index,
                destination_index,
                expand_grayscale,
            ) {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_composition_prefix_observation(
                        source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, initial_work as nat,
                        crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat, completed,
                    );
                    assert((Err(crate::specs::part2_composition_error(false)), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                        == crate::specs::part2_composition_budgeted_prefix(
                            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                            initial_work as nat, maximum_work_units, completed + 1,
                        ));
                    crate::proofs::production_impl::production_composition_budgeted_failure_absorbs(
                        source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                        initial_work as nat, maximum_work_units, completed + 1, geometry.copy_width * geometry.copy_height,
                    );
                }
                return Err(invalid_layer());
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert((true, crate::specs::part2_composition_planes(output_colors@), output_alpha@)
                    == crate::specs::part2_composition_pixel_transition(
                        source_planes, layer_alpha, previous_state.1, previous_state.2,
                        source_index as nat, destination_index as nat, expand_grayscale,
                    ));
                assert((true, crate::specs::part2_composition_planes(output_colors@), output_alpha@)
                    == crate::specs::part2_composition_canvas_prefix(source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, completed + 1));
                crate::proofs::production_impl::production_composition_prefix_observation(
                    source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, initial_work as nat,
                    crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat, completed + 1,
                );
            }
            let pixel_work = (output_colors.len() as u64)
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
            #[allow(
                clippy::question_mark,
                reason = "The overflow branch proves the exact stored-but-uncharged state"
            )]
            let next_work = match work_units.checked_add(pixel_work) {
                Some(value) => value,
                None => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert((Err(crate::specs::part2_composition_error(true)), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                            == crate::specs::part2_composition_budgeted_prefix(
                                source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                                initial_work as nat, maximum_work_units, completed + 1,
                            ));
                        crate::proofs::production_impl::production_composition_budgeted_failure_absorbs(
                            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                            initial_work as nat, maximum_work_units, completed + 1, geometry.copy_width * geometry.copy_height,
                        );
                    }
                    return Err(arithmetic_overflow());
                }
            };
            *work_units = next_work;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(pixel_work as nat == initial_colors.len() + 1);
                assert((completed + 1) * (initial_colors.len() + 1)
                    == completed * (initial_colors.len() + 1) + (initial_colors.len() + 1)) by(nonlinear_arith);
                assert(*work_units as nat == initial_work as nat + (completed + 1) * (initial_colors.len() + 1));
                crate::proofs::production_impl::production_composition_prefix_observation(
                    source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale, initial_work as nat,
                    crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat, completed + 1,
                );
            }
            #[allow(
                clippy::question_mark,
                reason = "The rejection branch proves the exact charged failure state"
            )]
            if let Err(error) = enforce_limit(Resource::WorkUnits, maximum_work_units, *work_units)
            {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert((Err(error), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                        == crate::specs::part2_composition_budgeted_prefix(
                            source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                            initial_work as nat, maximum_work_units, completed + 1,
                        ));
                    crate::proofs::production_impl::production_composition_budgeted_failure_absorbs(
                        source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                        initial_work as nat, maximum_work_units, completed + 1, geometry.copy_width * geometry.copy_height,
                    );
                }
                return Err(error);
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert((Ok(()), crate::specs::part2_composition_planes(output_colors@), output_alpha@, *work_units as nat)
                    == crate::specs::part2_composition_budgeted_prefix(
                        source_planes, layer_alpha, initial_colors, initial_alpha, geometry, expand_grayscale,
                        initial_work as nat, maximum_work_units, completed + 1,
                    ));
            }
            x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert((y as nat) * (copy_width as nat) + (copy_width as nat)
                == ((y as nat) + 1) * (copy_width as nat)) by(nonlinear_arith);
        }
        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert((copy_height as nat) * (copy_width as nat)
            == (copy_width as nat) * (copy_height as nat)) by(nonlinear_arith);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn composite_layer(
    canvas_width: u32,
    canvas_height: u32,
    offset_x: u32,
    offset_y: u32,
    source: &LayerChannels<'_>,
    output_colors: &mut [Vec<u16>],
    output_alpha: &mut [u16],
    work_units: &mut u64,
    limits: &ResourceLimits,
) -> Result<(), DecodeError> {
    let copy_width = source.width.min(canvas_width.saturating_sub(offset_x));
    let copy_height = source.height.min(canvas_height.saturating_sub(offset_y));
    let mut y = 0_u32;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant y <= copy_height,
            decreases copy_height - y,
        )
    )]
    while y < copy_height {
        let mut x = 0_u32;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant x <= copy_width,
                decreases copy_width - x,
            )
        )]
        while x < copy_width {
            let source_index = index_of(source.width, x, y)?;
            let destination_index = index_of(
                canvas_width,
                offset_x.checked_add(x).ok_or_else(arithmetic_overflow)?,
                offset_y.checked_add(y).ok_or_else(arithmetic_overflow)?,
            )?;
            let (source_alpha, premultiplied) = match source.alpha.as_ref() {
                Some((samples, premultiplied)) => (
                    *samples.get(source_index).ok_or_else(invalid_layer)?,
                    *premultiplied,
                ),
                None => (u16::MAX, false),
            };
            let destination_alpha = *output_alpha
                .get(destination_index)
                .ok_or_else(invalid_layer)?;
            let output_alpha_numerator =
                crate::refinement::source_over_alpha_numerator(source_alpha, destination_alpha);
            let mut color_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant color_index <= source.colors.len(),
                    decreases source.colors.len() - color_index,
                )
            )]
            while color_index < source.colors.len() {
                let source_sample = *source
                    .colors
                    .get(color_index)
                    .and_then(|samples| samples.get(source_index))
                    .ok_or_else(invalid_layer)?;
                if premultiplied && source_sample > source_alpha {
                    return Err(DecodeError::malformed(0, MalformedKind::InvalidBoxContents));
                }
                let destination_sample = *output_colors
                    .get(color_index)
                    .and_then(|samples| samples.get(destination_index))
                    .ok_or_else(invalid_layer)?;
                let color_numerator = crate::refinement::source_over_color_numerator(
                    source_sample,
                    destination_sample,
                    source_alpha,
                    destination_alpha,
                    premultiplied,
                );
                let output_sample = crate::refinement::source_over_rounded_ratio(
                    color_numerator,
                    output_alpha_numerator,
                );
                let destination_samples = output_colors
                    .get_mut(color_index)
                    .ok_or_else(invalid_layer)?;
                let sample = u16::try_from(output_sample).map_err(|_| arithmetic_overflow())?;
                if !crate::refinement::try_store_composition_sample(
                    destination_samples,
                    destination_index,
                    sample,
                ) {
                    return Err(invalid_layer());
                }
                color_index = color_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            let alpha = crate::refinement::source_over_rounded_ratio(
                output_alpha_numerator,
                u64::from(u16::MAX),
            );
            let sample = u16::try_from(alpha).map_err(|_| arithmetic_overflow())?;
            if !crate::refinement::try_store_composition_sample(
                output_alpha,
                destination_index,
                sample,
            ) {
                return Err(invalid_layer());
            }
            *work_units = work_units
                .checked_add(
                    (source.colors.len() as u64)
                        .checked_add(1)
                        .ok_or_else(arithmetic_overflow)?,
                )
                .ok_or_else(arithmetic_overflow)?;
            enforce_limit(
                Resource::WorkUnits,
                limits.maximum_work_units(),
                *work_units,
            )?;
            x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        y = y.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    Ok(())
}
