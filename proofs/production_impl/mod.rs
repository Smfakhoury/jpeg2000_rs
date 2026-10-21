use vstd::prelude::*;

use crate::specs::{
    ads_payload_bytes, atk_reconstruction_step, atk_update_parity, bounded_jpip_transcode,
    bounded_loop_frame, clamp_color_sample, clipped_composition_extent, color_output_bytes,
    complete_jpip_databin, complete_jpip_range, complete_mct_fragment_series,
    composition_instruction_work, composition_output_bytes, consistent_jpip_final_length,
    consistent_part15_signaling, effective_jpx_layer_count, ht_cleanup_byte_claims_valid,
    ht_completed_u, ht_kappa_extension, ht_placeholder_passes, ht_substantive_passes,
    jpip_precinct_in_class, jpip_quality_prefix_available, jpip_reconstructed_box_length,
    jpip_tile_transition_sufficient, magnitude_addition, magnitude_less, magnitude_limb_index,
    magnitude_limb_offset, magnitude_shift_fits, maxshift_restored_magnitude,
    mct_dependency_coefficients, mct_wavelet_work_units, mct_work_units, mixed_block_selects_ht,
    nested_metadata_work, palette_linear_index, part9_auxiliary_present, part9_available_bytes,
    part9_class_identifier_valid, part9_databin_identity, part9_eor_extent,
    part9_header_form_valid, part9_header_has_explicit_class, part9_header_has_explicit_codestream,
    part9_merge_end, part9_merge_start, part9_metadata_descent_allowed, part9_missing_length,
    part9_overlap_consistent, part9_packet_reframed_length, part9_precinct_flat_identity,
    part9_progression_supported, part9_reconstruction_length, part9_reframed_marker_removed,
    part9_transaction_commit, part9_vbas_first_is_canonical, part9_vbas_step,
    registered_channel_count, resampled_component_coordinate, signed_color_to_nominal,
    tile_transform_scope, valid_box_lengths, valid_cbd_scope, valid_codestream_header_associations,
    valid_ht_cleanup_partition, valid_ht_pass_group, valid_ht_quad_origin,
    valid_ht_refinement_segment, valid_ht_style_prefix, valid_jp2_primary_brand,
    valid_jph_branding, valid_jpip_codestream_layout, valid_jpip_fragment,
    valid_jpip_precinct_coordinate, valid_jpip_progressive_update,
    valid_layer_codestream_reference, valid_magnitude_bit, valid_mct_dimensions,
    valid_mct_fragment_index, valid_mct_matrix_bounds, valid_mct_stage_edge,
    valid_mct_wavelet_partition, valid_palette_lookup, valid_selection_region,
    valid_signed_color_sample,
};

verus! {

pub proof fn production_composition_canvas_empty(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: crate::specs::Part2CompositionGeometry, expand_grayscale: bool,
)
    requires geometry.copy_width > 0 && geometry.target_width > 0 && geometry.target_height > 0,
    ensures crate::specs::part2_composition_canvas_prefix(
        source, source_alpha, destination, destination_alpha, geometry, expand_grayscale, 0,
    ) == (true, destination, destination_alpha),
{
    reveal(crate::specs::part2_composition_canvas_prefix);
}

pub proof fn production_composition_canvas_step(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: crate::specs::Part2CompositionGeometry, expand_grayscale: bool,
    x: nat, y: nat, source_index: nat, destination_index: nat,
)
    requires
        geometry.copy_width > 0 && geometry.target_width > 0 && geometry.target_height > 0,
        x < geometry.copy_width,
        crate::specs::part2_composition_canvas_prefix(
            source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
            y * geometry.copy_width + x,
        ).0,
        source_index == (geometry.source_y + y * geometry.source_crop_height / geometry.target_height) * geometry.source_width
            + geometry.source_x + x * geometry.source_crop_width / geometry.target_width,
        destination_index == (geometry.target_y + y) * geometry.canvas_width + geometry.target_x + x,
    ensures
        crate::specs::part2_composition_canvas_prefix(
            source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
            y * geometry.copy_width + x + 1,
        ) == crate::specs::part2_composition_pixel_transition(
            source, source_alpha,
            crate::specs::part2_composition_canvas_prefix(
                source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
                y * geometry.copy_width + x,
            ).1,
            crate::specs::part2_composition_canvas_prefix(
                source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
                y * geometry.copy_width + x,
            ).2,
            source_index, destination_index, expand_grayscale,
        ),
{
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(
        (y * geometry.copy_width + x) as int, geometry.copy_width as int, y as int, x as int,
    );
    reveal_with_fuel(crate::specs::part2_composition_canvas_prefix, 2);
}

pub proof fn production_composition_raster_prefix_bound(width: nat, height: nat, x: nat, y: nat)
    requires width > 0 && height > 0 && x < width && y < height,
    ensures y * width + x + 1 <= width * height,
{
    assert(x + 1 <= width && y + 1 <= height);
    assert((y + 1) * width == y * width + width) by(nonlinear_arith);
    vstd::arithmetic::mul::lemma_mul_inequality((y + 1) as int, height as int, width as int);
    assert(height * width == width * height) by(nonlinear_arith);
}

pub proof fn production_composition_prefix_observation(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: crate::specs::Part2CompositionGeometry, expand_grayscale: bool, initial_work: nat,
    final_colors: Seq<Seq<u16>>, final_alpha: Seq<u16>, final_work: nat, completed: nat,
)
    requires
        geometry.source_crop_width > 0 && geometry.source_crop_height > 0,
        geometry.target_width > 0 && geometry.target_height > 0,
        geometry.copy_width > 0 && geometry.copy_height > 0,
        crate::specs::part2_composition_layer_prefix_matches(
            source, source_alpha, destination, destination_alpha, geometry, expand_grayscale, initial_work,
            final_colors, final_alpha, final_work, completed,
        ),
    ensures crate::specs::part2_composition_layer_prefix_safe(
        source, source_alpha, destination, destination_alpha, geometry, expand_grayscale, initial_work,
        final_colors, final_alpha, final_work,
    ),
{
    assert(exists |count: nat| count == completed && crate::specs::part2_composition_layer_prefix_matches(
        source, source_alpha, destination, destination_alpha, geometry, expand_grayscale, initial_work,
        final_colors, final_alpha, final_work, count,
    ));
}

pub proof fn production_composition_budgeted_empty(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: crate::specs::Part2CompositionGeometry, expand_grayscale: bool,
    initial_work: nat, work_limit: u64,
)
    requires geometry.copy_width > 0 && geometry.target_width > 0 && geometry.target_height > 0,
    ensures crate::specs::part2_composition_budgeted_prefix(
        source, source_alpha, destination, destination_alpha, geometry, expand_grayscale, initial_work, work_limit, 0,
    ) == (Ok(()), destination, destination_alpha, initial_work),
{
    reveal(crate::specs::part2_composition_budgeted_prefix);
}

pub proof fn production_composition_budgeted_step(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: crate::specs::Part2CompositionGeometry, expand_grayscale: bool,
    initial_work: nat, work_limit: u64, x: nat, y: nat,
)
    requires
        geometry.copy_width > 0 && geometry.target_width > 0 && geometry.target_height > 0,
        x < geometry.copy_width,
        crate::specs::part2_composition_budgeted_prefix(
            source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
            initial_work, work_limit, y * geometry.copy_width + x,
        ).0.is_ok(),
    ensures crate::specs::part2_composition_budgeted_prefix(
        source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
        initial_work, work_limit, y * geometry.copy_width + x + 1,
    ) == crate::specs::part2_composition_budgeted_step(
        source, source_alpha,
        crate::specs::part2_composition_budgeted_prefix(
            source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
            initial_work, work_limit, y * geometry.copy_width + x,
        ).1,
        crate::specs::part2_composition_budgeted_prefix(
            source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
            initial_work, work_limit, y * geometry.copy_width + x,
        ).2,
        geometry, expand_grayscale,
        crate::specs::part2_composition_budgeted_prefix(
            source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
            initial_work, work_limit, y * geometry.copy_width + x,
        ).3, work_limit, x, y,
    ),
{
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(
        (y * geometry.copy_width + x) as int, geometry.copy_width as int, y as int, x as int,
    );
    reveal_with_fuel(crate::specs::part2_composition_budgeted_prefix, 2);
}

pub proof fn production_composition_budgeted_failure_absorbs(
    source: Seq<Seq<u16>>, source_alpha: Option<(Seq<u16>, bool)>,
    destination: Seq<Seq<u16>>, destination_alpha: Seq<u16>,
    geometry: crate::specs::Part2CompositionGeometry, expand_grayscale: bool,
    initial_work: nat, work_limit: u64, stopped: nat, count: nat,
)
    requires
        geometry.copy_width > 0 && geometry.target_width > 0 && geometry.target_height > 0,
        stopped <= count,
        crate::specs::part2_composition_budgeted_prefix(
            source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
            initial_work, work_limit, stopped,
        ).0.is_err(),
    ensures crate::specs::part2_composition_budgeted_prefix(
        source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
        initial_work, work_limit, count,
    ) == crate::specs::part2_composition_budgeted_prefix(
        source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
        initial_work, work_limit, stopped,
    ),
    decreases count,
{
    if count > stopped {
        production_composition_budgeted_failure_absorbs(
            source, source_alpha, destination, destination_alpha, geometry, expand_grayscale,
            initial_work, work_limit, stopped, (count - 1) as nat,
        );
        reveal_with_fuel(crate::specs::part2_composition_budgeted_prefix, 2);
    }
}

pub proof fn production_ads_parser_uses_independent_field_padding(depths: nat, splits: nat)
    ensures
        ads_payload_bytes(depths, splits)
            == 3 + (depths + 3) / 4 + (splits + 3) / 4,
{
    reveal(ads_payload_bytes);
    reveal(crate::specs::packed_two_bit_bytes);
}

pub proof fn production_atk_inverse_visits_each_step_in_reverse(step: nat, steps: nat)
    requires
        step < steps,
    ensures
        atk_reconstruction_step(step, steps) < steps,
{
    reveal(atk_reconstruction_step);
}

pub proof fn production_atk_inverse_alternates_subsequences(initial_odd: bool, step: nat)
    ensures
        atk_update_parity(initial_odd, step + 1)
            != atk_update_parity(initial_odd, step),
{
    reveal(atk_update_parity);
}

pub proof fn production_cbd_rejects_tile_header_scope()
    ensures
        valid_cbd_scope(true),
        !valid_cbd_scope(false),
{
    reveal(valid_cbd_scope);
}

pub proof fn box_header_and_payload_partition(
    header_length: nat,
    total_length: nat,
)
    requires
        valid_box_lengths(header_length, total_length),
    ensures
        header_length + (total_length - header_length) == total_length,
{
}

pub proof fn checked_geometry_area_is_symmetric(width: nat, height: nat)
    ensures
        width * height == height * width,
{
}

pub proof fn production_selection_region_is_bounded(
    image_x0: nat,
    image_y0: nat,
    image_x1: nat,
    image_y1: nat,
    region_x0: nat,
    region_y0: nat,
    region_x1: nat,
    region_y1: nat,
)
    requires
        valid_selection_region(
            image_x0,
            image_y0,
            image_x1,
            image_y1,
            region_x0,
            region_y0,
            region_x1,
            region_y1,
        ),
    ensures
        image_x0 <= region_x0,
        region_x1 <= image_x1,
        image_y0 <= region_y0,
        region_y1 <= image_y1,
{
    reveal(valid_selection_region);
}

pub proof fn production_jpip_fragment_is_parent_bounded(
    offset: nat,
    length: nat,
    limit: nat,
)
    requires
        valid_jpip_fragment(offset, length, limit),
    ensures
        offset + length <= limit,
{
    reveal(valid_jpip_fragment);
}

pub proof fn production_part9_header_and_vbas_contracts_are_exact(
    form: nat,
    value: nat,
    group: nat,
    continued: bool,
)
    ensures
        part9_header_form_valid(form) == (1 <= form && form <= 3),
        part9_header_has_explicit_class(form) == (form >= 2),
        part9_header_has_explicit_codestream(form) == (form == 3),
        part9_vbas_first_is_canonical(group, continued) == (!continued || group > 0),
        part9_vbas_step(value, group) == value * 128 + group,
{
    reveal(part9_header_form_valid);
    reveal(part9_header_has_explicit_class);
    reveal(part9_header_has_explicit_codestream);
    reveal(part9_vbas_first_is_canonical);
    reveal(part9_vbas_step);
}

pub proof fn production_part9_aux_eor_and_class_contracts_are_exact(
    class_identifier: nat,
    header: nat,
    body: nat,
)
    ensures
        part9_auxiliary_present(class_identifier) == (class_identifier % 2 == 1),
        part9_eor_extent(header, body) == header + body,
        part9_class_identifier_valid(class_identifier) == (class_identifier <= 8),
{
    reveal(part9_auxiliary_present);
    reveal(part9_eor_extent);
    reveal(part9_class_identifier_valid);
}

pub proof fn production_part9_identity_contract_is_exact(
    first_class: nat,
    first_codestream: nat,
    first_in_class: nat,
    second_class: nat,
    second_codestream: nat,
    second_in_class: nat,
)
    ensures
        part9_databin_identity(
            first_class,
            first_codestream,
            first_in_class,
            second_class,
            second_codestream,
            second_in_class,
        ) == (
            first_class / 2 == second_class / 2
            && first_codestream == second_codestream
            && first_in_class == second_in_class
        ),
{
    reveal(part9_databin_identity);
}

pub proof fn production_part9_flat_precinct_identity_is_exact(
    tile: nat,
    tile_count: nat,
    component_precinct: nat,
)
    ensures
        part9_precinct_flat_identity(tile, tile_count, component_precinct)
            == tile + component_precinct * tile_count,
{
    reveal(part9_precinct_flat_identity);
}

pub proof fn production_part9_transaction_and_merge_contracts_are_exact(
    first: nat,
    second: nat,
    equal: bool,
)
    ensures
        part9_overlap_consistent(equal) == equal,
        part9_transaction_commit(equal) == equal,
        part9_merge_start(first, second) <= first,
        part9_merge_start(first, second) <= second,
        part9_merge_end(first, second) >= first,
        part9_merge_end(first, second) >= second,
{
    reveal(part9_overlap_consistent);
    reveal(part9_transaction_commit);
    reveal(part9_merge_start);
    reveal(part9_merge_end);
}

pub proof fn production_part9_range_accounting_is_exact(required: nat, missing: nat)
    requires
        missing <= required,
    ensures
        part9_missing_length(required, missing) == required - missing,
        part9_available_bytes(required, missing) == required - missing,
{
    reveal(part9_missing_length);
    reveal(part9_available_bytes);
}

pub proof fn production_part9_metadata_descent_rejects_cycle(
    depth: nat,
    maximum_depth: nat,
)
    ensures
        !part9_metadata_descent_allowed(depth, maximum_depth, true),
{
    reveal(part9_metadata_descent_allowed);
}

pub proof fn production_part9_reconstruction_and_reframing_lengths_are_exact(
    main: nat,
    tile: nat,
    packet: nat,
    eoc: nat,
    sop: bool,
    eph: bool,
)
    ensures
        part9_reconstruction_length(main, tile, packet, eoc) == main + tile + packet + eoc,
        part9_packet_reframed_length(packet, sop, eph)
            == packet + (if sop { 6 as nat } else { 0 as nat })
                + (if eph { 2 as nat } else { 0 as nat }),
{
    reveal(part9_reconstruction_length);
    reveal(part9_packet_reframed_length);
}

pub proof fn production_part9_all_progressions_and_reframed_markers_are_classified(
    order: nat,
    marker: nat,
    tile: bool,
)
    ensures
        part9_progression_supported(order) == (order <= 4),
        part9_reframed_marker_removed(marker, tile) == (
            if tile {
                marker == 0xff58 || marker == 0xff61
            } else {
                marker == 0xff55 || marker == 0xff57 || marker == 0xff60
            }
        ),
{
    reveal(part9_progression_supported);
    reveal(part9_reframed_marker_removed);
}

pub proof fn production_jpip_final_length_conflict_policy_is_deterministic(
    existing: nat,
    announced: nat,
)
    requires
        consistent_jpip_final_length(existing, announced),
    ensures
        existing == announced,
{
    reveal(consistent_jpip_final_length);
}

pub proof fn production_jpip_completeness_requires_full_prefix(
    prefix: nat,
    final_length: nat,
)
    requires
        complete_jpip_databin(prefix, final_length),
    ensures
        prefix == final_length,
{
    reveal(complete_jpip_databin);
}

pub proof fn production_jpip_packet_range_requires_all_bytes(available: nat, required: nat)
    requires
        complete_jpip_range(available, required),
    ensures
        available == required,
{
    reveal(complete_jpip_range);
}

pub proof fn production_jpip_progressive_quality_is_monotonic(
    previous_layers: nat,
    available_layers: nat,
    planned_layers: nat,
)
    requires
        valid_jpip_progressive_update(previous_layers, available_layers, planned_layers),
    ensures
        previous_layers <= available_layers,
        available_layers <= planned_layers,
{
    reveal(valid_jpip_progressive_update);
}

pub proof fn production_jpip_transcode_is_parent_bounded(
    main_header: nat,
    tile_header: nat,
    packet_bytes: nat,
    limit: nat,
)
    requires
        bounded_jpip_transcode(main_header, tile_header, packet_bytes, limit),
    ensures
        main_header + tile_header + packet_bytes + 2 <= limit,
{
    reveal(bounded_jpip_transcode);
}

pub proof fn production_jpip_precinct_identifier_matches_formula(
    tile: nat,
    tile_count: nat,
    component: nat,
    components: nat,
    precinct_index: nat,
)
    requires
        valid_jpip_precinct_coordinate(tile, tile_count, component, components),
    ensures
        jpip_precinct_in_class(tile, tile_count, component, components, precinct_index)
            == tile + tile_count * (component + components * precinct_index),
{
    reveal(jpip_precinct_in_class);
}

pub proof fn production_jpip_progressive_decode_uses_available_prefix(
    available: nat,
    requested: nat,
)
    requires
        jpip_quality_prefix_available(available, requested),
    ensures
        requested <= available,
{
    reveal(jpip_quality_prefix_available);
}

pub proof fn production_jpip_metadata_box_length_is_header_plus_payload(
    header: nat,
    payload: nat,
)
    ensures
        jpip_reconstructed_box_length(header, payload) == header + payload,
{
    reveal(jpip_reconstructed_box_length);
}

pub proof fn production_jpip_functional_transition_requires_packet_boundary(
    later_functional: bool,
    preceding_packet_count_known: bool,
    preceding_exact_consumption_known: bool,
)
    requires
        jpip_tile_transition_sufficient(
            later_functional,
            preceding_packet_count_known,
            preceding_exact_consumption_known,
        ),
        later_functional,
    ensures
        preceding_packet_count_known || preceding_exact_consumption_known,
{
    reveal(jpip_tile_transition_sufficient);
}

pub proof fn checked_palette_position_is_within_table(
    entry: nat,
    entries: nat,
    column: nat,
    columns: nat,
)
    requires
        valid_palette_lookup(entry, entries, column, columns),
    ensures
        palette_linear_index(entry, column, columns) < entries * columns,
{
    reveal(valid_palette_lookup);
    reveal(palette_linear_index);
}

pub proof fn ordinal_codestream_header_association_is_in_range(
    index: nat,
    headers: nat,
    codestreams: nat,
)
    requires
        index < headers,
        valid_codestream_header_associations(headers, codestreams),
    ensures
        index < codestreams,
{
}

pub proof fn production_jph_branding_is_canonical()
    ensures
        valid_jph_branding(true, 0, 1, true),
{
}

pub proof fn production_part15_signals_are_consistent(has_corresponding_profile: bool)
    ensures
        consistent_part15_signaling(true, true, true, has_corresponding_profile),
{
}

pub proof fn production_ht_style_prefixes_exclude_reserved_value()
    ensures
        valid_ht_style_prefix(0),
        valid_ht_style_prefix(1),
        !valid_ht_style_prefix(2),
        valid_ht_style_prefix(3),
{
}

pub proof fn ht_cleanup_partition_is_parent_bounded(cleanup_length: nat, scup: nat)
    requires
        valid_ht_cleanup_partition(cleanup_length, scup),
    ensures
        cleanup_length - scup <= cleanup_length,
        (cleanup_length - scup) + scup == cleanup_length,
{
    reveal(valid_ht_cleanup_partition);
}

pub proof fn production_ht_cleanup_fusion_is_single_byte(
    mel_byte: nat,
    vlc_byte: nat,
    terminal_byte: nat,
    value: u8,
    shared_a: nat,
    shared_b: nat,
)
    requires
        ht_cleanup_byte_claims_valid(mel_byte, vlc_byte, terminal_byte, value),
        vlc_byte <= shared_a <= mel_byte,
        vlc_byte <= shared_b <= mel_byte,
    ensures
        shared_a == shared_b,
        shared_a < terminal_byte,
        value != 0xff,
{
    crate::proofs::spec_properties::ht_cleanup_shared_byte_is_unique(
        mel_byte, vlc_byte, terminal_byte, value, shared_a, shared_b,
    );
}

pub proof fn production_ht_quad_metadata_has_bounded_origin(
    x: nat,
    y: nat,
    width: nat,
    height: nat,
)
    requires
        valid_ht_quad_origin(x, y, width, height),
    ensures
        x < width,
        y < height,
{
    reveal(valid_ht_quad_origin);
}

pub proof fn production_ht_kappa_completion_is_monotonic(
    u_base: nat,
    gamma: bool,
    exponent: nat,
)
    ensures
        ht_completed_u(u_base, gamma, exponent) == u_base
            + ht_kappa_extension(gamma, exponent),
        ht_completed_u(u_base, gamma, exponent) >= u_base,
{
    reveal(ht_completed_u);
    reveal(ht_kappa_extension);
}

pub proof fn production_ht_pass_group_has_placeholders_and_bounded_substantive_set(
    packet_passes: nat,
)
    requires
        valid_ht_pass_group(packet_passes),
    ensures
        packet_passes >= 1,
        ht_placeholder_passes(packet_passes) % 3 == 0,
        ht_substantive_passes(packet_passes) >= 1,
        ht_substantive_passes(packet_passes) <= 3,
{
    reveal(valid_ht_pass_group);
    reveal(ht_placeholder_passes);
    reveal(ht_substantive_passes);
}

pub proof fn production_ht_refinement_segment_is_bounded(bytes: nat, passes: nat)
    requires
        valid_ht_refinement_segment(bytes, passes),
    ensures
        bytes <= 2046,
        passes == 1 || passes == 2,
{
    reveal(valid_ht_refinement_segment);
}

pub proof fn production_magnitude_bit_selects_bounded_limb(bit: nat)
    requires
        valid_magnitude_bit(bit),
    ensures
        magnitude_limb_index(bit) < 6,
        magnitude_limb_offset(bit) < 64,
{
    reveal(valid_magnitude_bit);
    reveal(magnitude_limb_index);
    reveal(magnitude_limb_offset);
}

pub proof fn production_magnitude_shift_is_capacity_bounded(highest_bit: nat, shift: nat)
    requires
        magnitude_shift_fits(highest_bit, shift),
    ensures
        highest_bit + shift < 384,
{
    reveal(magnitude_shift_fits);
}

pub proof fn production_magnitude_comparison_is_transitive(
    left: nat,
    middle: nat,
    right: nat,
)
    requires
        magnitude_less(left, middle),
        magnitude_less(middle, right),
    ensures
        magnitude_less(left, right),
{
    reveal(magnitude_less);
}

pub proof fn production_magnitude_addition_is_monotonic(left: nat, right: nat)
    ensures
        magnitude_addition(left, right) >= left,
        magnitude_addition(left, right) >= right,
{
    reveal(magnitude_addition);
}

pub proof fn production_maxshift_undo_is_nonexpanding(magnitude: nat, threshold: nat)
    requires
        threshold > 0,
    ensures
        maxshift_restored_magnitude(magnitude, threshold) <= magnitude,
{
    reveal(maxshift_restored_magnitude);
}

pub proof fn production_mixed_block_selection_is_deterministic(
    lblock: nat,
    first_length_bit_is_zero: bool,
)
    ensures
        mixed_block_selects_ht(lblock, first_length_bit_is_zero)
            == (lblock > 3 && first_length_bit_is_zero),
{
    reveal(mixed_block_selects_ht);
}

pub proof fn clamped_color_sample_is_u16_bounded(value: int)
    ensures
        0 <= clamp_color_sample(value),
        clamp_color_sample(value) <= 65535,
{
}

pub proof fn production_signed_color_is_shifted_to_nominal(value: int, level: nat)
    requires
        valid_signed_color_sample(value, level),
    ensures
        0 <= signed_color_to_nominal(value, level),
        signed_color_to_nominal(value, level) < 2 * level,
{
    reveal(valid_signed_color_sample);
    reveal(signed_color_to_nominal);
}

pub proof fn production_color_resampling_is_component_bounded(
    grid_coordinate: nat,
    sampling_step: nat,
    first_component_coordinate: nat,
    component_count: nat,
)
    requires
        sampling_step > 0,
        component_count > 0,
    ensures
        first_component_coordinate <= resampled_component_coordinate(
            grid_coordinate,
            sampling_step,
            first_component_coordinate,
            component_count,
        ),
        resampled_component_coordinate(
            grid_coordinate,
            sampling_step,
            first_component_coordinate,
            component_count,
        ) < first_component_coordinate + component_count,
{
    reveal(resampled_component_coordinate);
}

pub proof fn converted_color_storage_is_two_bytes_per_sample(pixels: nat, channels: nat)
    ensures
        color_output_bytes(pixels, channels) == pixels * channels * 2,
{
}

pub proof fn production_layer_reference_is_checked(index: nat, codestreams: nat)
    requires
        valid_layer_codestream_reference(index, codestreams),
    ensures
        index < codestreams,
{
}

pub proof fn production_implicit_layers_follow_codestream_count(codestreams: nat)
    ensures
        effective_jpx_layer_count(0, codestreams) == codestreams,
{
}

pub proof fn production_rejects_contradictory_jp2_primary_brand()
    ensures
        !valid_jp2_primary_brand(true, true),
{
}

pub proof fn production_jpip_layout_has_complete_pairs(tile_pairs: nat)
    ensures
        valid_jpip_codestream_layout(3 + tile_pairs * 2),
{
}

pub proof fn production_tile_transform_does_not_cross_slice(
    start: nat,
    components: nat,
    total: nat,
)
    requires
        tile_transform_scope(start, components, total),
    ensures
        start + components <= total,
{
}

pub proof fn production_layer_crop_is_canvas_bounded(
    offset: nat,
    extent: nat,
    canvas: nat,
)
    ensures
        clipped_composition_extent(offset, extent, canvas) <= extent,
        offset < canvas ==> clipped_composition_extent(offset, extent, canvas) <= canvas - offset,
{
}

pub proof fn production_composition_storage_accounting(pixels: nat, colors: nat)
    ensures
        composition_output_bytes(pixels, colors) == pixels * (colors + 1) * 2,
{
}

pub proof fn production_u32_product_bounds(left: u32, right: u32)
    ensures
        (left as int) * (right as int) <= 18446744065119617025,
{
    vstd::arithmetic::mul::lemma_mul_upper_bound(left as int, 4294967295, right as int, 4294967295);
}

pub proof fn production_composition_mapped_bounds(position: u32, source_extent: u32, target_extent: u32)
    requires
        target_extent > 0,
    ensures
        position < target_extent && source_extent > 0 ==>
            crate::specs::part2_composition_mapped_offset(position, source_extent, target_extent) < source_extent,
{
    if position < target_extent && source_extent > 0 {
        let p = position as int;
        let s = source_extent as int;
        let t = target_extent as int;
        let n = p * s;
        vstd::arithmetic::mul::lemma_mul_strict_inequality(p, t, s);
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod(n, t);
        vstd::arithmetic::div_mod::lemma_mod_bound(n, t);
        vstd::arithmetic::mul::lemma_mul_is_commutative(n / t, t);
        vstd::arithmetic::mul::lemma_mul_is_commutative(t, s);
        vstd::arithmetic::mul::lemma_mul_strict_inequality_converse(n / t, s, t);
    }
}

pub proof fn production_composition_index_bounds(width: u32, height: u32, x: u32, y: u32)
    requires
        x < width,
        y < height,
    ensures
        crate::specs::part2_row_major_index(width, x, y) < (width as nat) * (height as nat),
{
    assert((y as int) * (width as int) + (x as int) < (width as int) * (height as int))
        by (nonlinear_arith)
        requires 0 <= x < width, 0 <= y < height,
    {}
}

pub proof fn production_source_over_term_bounds(left: u64, right: u64, other: u64)
    requires
        left <= 65535,
        right <= 65535,
        other <= 65535,
    ensures
        (left as int) * (right as int) <= 4294836225,
        (left as int) * (right as int) * (other as int) <= 281462092005375,
{
    vstd::arithmetic::mul::lemma_mul_upper_bound(left as int, 65535, right as int, 65535);
    vstd::arithmetic::mul::lemma_mul_upper_bound(
        (left as int) * (right as int), 4294836225, other as int, 65535,
    );
}

pub proof fn production_frame_materialization_bound(frames: nat, layers: nat)
    ensures
        composition_instruction_work(frames, layers)
            == frames * if layers == 0 { 1 } else { layers },
{
}

pub proof fn production_registered_channel_accounting(prior: nat, added: nat)
    ensures
        registered_channel_count(prior, added) == prior + added,
{
}

pub proof fn production_nested_metadata_accounting(boxes: nat, depth: nat)
    ensures
        nested_metadata_work(boxes, depth) == boxes + depth,
{
}

pub proof fn production_loop_frame_is_bounded(index: nat, frames: nat)
    requires
        frames > 0,
    ensures
        bounded_loop_frame(index, frames) < frames,
{
}

pub proof fn production_mct_matrix_is_square(
    components: nat,
    matrix_elements: nat,
    offset_elements: nat,
)
    requires
        valid_mct_dimensions(components, matrix_elements, offset_elements),
    ensures
        matrix_elements == components * components,
        offset_elements == 0 || offset_elements == components,
{
}

pub proof fn production_mct_work_counts_products_and_outputs(pixels: nat, components: nat)
    ensures
        mct_work_units(pixels, components) == pixels * components * (components + 1),
{
}

pub proof fn production_mct_stage_order_is_dag(parent: nat, child: nat, stages: nat)
    requires
        valid_mct_stage_edge(parent, child, stages),
    ensures
        parent < child,
        child < stages,
{
}

pub proof fn production_mct_matrix_is_component_bounded(
    rows: nat,
    columns: nat,
    elements: nat,
    component_limit: nat,
)
    requires
        valid_mct_matrix_bounds(rows, columns, elements, component_limit),
    ensures
        rows == columns,
        rows <= component_limit,
        elements == rows * columns,
{
}

pub proof fn production_mct_fragment_index_is_contiguous(
    index: nat,
    expected: nat,
    final_index: nat,
)
    requires
        valid_mct_fragment_index(index, expected, final_index),
    ensures
        index == expected,
        index <= final_index,
{
    reveal(valid_mct_fragment_index);
}

pub proof fn production_mct_fragment_completeness_reaches_final(final_index: nat)
    ensures
        complete_mct_fragment_series(final_index + 1, final_index),
{
    reveal(complete_mct_fragment_series);
}

pub proof fn production_reversible_dependency_coefficients_are_triangular(components: nat)
    requires
        components > 0,
    ensures
        mct_dependency_coefficients(components, true)
            == ((components * (components + 1) / 2 - 1) as nat),
{
    reveal(mct_dependency_coefficients);
}

pub proof fn production_irreversible_dependency_coefficients_are_strictly_triangular(
    components: nat,
)
    requires
        components > 0,
    ensures
        mct_dependency_coefficients(components, false)
            == ((components * (components - 1) / 2) as nat),
{
    reveal(mct_dependency_coefficients);
}

pub proof fn production_mct_wavelet_partition_preserves_component_count(
    low_components: nat,
    high_components: nat,
    output_components: nat,
)
    requires
        valid_mct_wavelet_partition(low_components, high_components, output_components),
    ensures
        low_components + high_components == output_components,
{
    reveal(valid_mct_wavelet_partition);
}

pub proof fn production_mct_wavelet_work_is_explicit(
    pixels: nat,
    components: nat,
    levels: nat,
)
    ensures
        mct_wavelet_work_units(pixels, components, levels)
            == pixels * components * (levels + 1) * 8,
{
    reveal(mct_wavelet_work_units);
}

}
