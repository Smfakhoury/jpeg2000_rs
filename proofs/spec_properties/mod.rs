use vstd::prelude::*;

use crate::proofs::shared::{CursorView, DatabinView};
use crate::specs::{
    ads_payload_bytes, atk_reconstruction_step, atk_update_parity, bounded_jpip_transcode,
    bounded_loop_frame, clipped_composition_extent, codestream_count_within_limit,
    complete_jpip_range, component_index_width, composition_instruction_work,
    composition_output_bytes, consistent_jpip_final_length, consistent_part15_signaling,
    decomposition_graph_cardinality, decomposition_stage_subbands, disjoint_ht_refinement_claims,
    effective_component_override, effective_jpx_layer_count, ht_cleanup_byte_claims_valid,
    ht_completed_u, ht_kappa_extension, ht_placeholder_passes, ht_quad_count,
    ht_substantive_passes, jpip_packet_boundary_exact, jpip_packet_boundary_step,
    jpip_precinct_in_class, jpip_quality_prefix_available, jpip_ranges_overlap,
    jpip_reconstructed_box_length, jpip_tile_transition_sufficient, magnitude_addition,
    magnitude_less, magnitude_limb_index, magnitude_limb_offset, magnitude_shift_fits,
    maxshift_restored_magnitude, nested_metadata_work, packed_header_index_continuous,
    packed_packet_length_matches, packet_length_index_continuous, packet_length_step,
    packet_lengths_cover_payload, palette_linear_index, poc_contains, ppm_frame_fits, region_fits,
    registered_channel_count, registered_sample_numerator, resampled_component_coordinate,
    signed_color_to_nominal, tile_transform_scope, valid_cbd_scope, valid_channel_association,
    valid_codestream_header_associations, valid_color_sample, valid_component_mapping,
    valid_component_override_index, valid_decomposition_direction, valid_ht_cleanup_partition,
    valid_ht_pass_group, valid_ht_quad_origin, valid_ht_refinement_segment, valid_ht_style_prefix,
    valid_jp2_primary_brand, valid_jph_branding, valid_jpip_codestream_layout, valid_jpip_fragment,
    valid_jpip_precinct_coordinate, valid_jpip_progressive_update,
    valid_layer_codestream_reference, valid_magnitude_bit, valid_maxshift_roi,
    valid_mct_array_reference, valid_mct_collection_count, valid_mct_dimensions,
    valid_mct_matrix_bounds, valid_mct_stage_edge, valid_palette_lookup, valid_part15_block_mode,
    valid_poc_record, valid_selected_codestream_index, valid_selection_region,
    valid_signed_color_sample, work_within_limit,
};

verus! {

pub proof fn region_end_is_bounded(parent_length: nat, offset: nat, length: nat)
    requires
        region_fits(parent_length, offset, length),
    ensures
        offset + length <= parent_length,
{
}

pub proof fn selected_region_is_nonempty_and_image_bounded(
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
        region_x0 < region_x1,
        region_y0 < region_y1,
        image_x0 <= region_x0,
        region_x1 <= image_x1,
        image_y0 <= region_y0,
        region_y1 <= image_y1,
{
    reveal(valid_selection_region);
}

pub proof fn cursor_remaining_reconstructs_length(view: CursorView)
    requires
        view.invariant(),
    ensures
        view.position + view.remaining() == view.length,
{
}

pub proof fn bounded_work_increment(consumed: nat, amount: nat, limit: nat)
    requires
        work_within_limit(consumed + amount, limit),
    ensures
        work_within_limit(consumed, limit),
        consumed + amount <= limit,
{
}

pub proof fn decomposition_stage_has_bounded_band_count(direction: nat)
    requires
        valid_decomposition_direction(direction),
    ensures
        decomposition_stage_subbands(direction) <= 3,
{
    reveal(valid_decomposition_direction);
    reveal(decomposition_stage_subbands);
}

pub proof fn ads_independent_padding_has_bounded_overhead(depths: nat, splits: nat)
    ensures
        ads_payload_bytes(depths, splits)
            == 3 + (depths + 3) / 4 + (splits + 3) / 4,
{
    reveal(ads_payload_bytes);
    reveal(crate::specs::packed_two_bit_bytes);
}

pub proof fn atk_steps_execute_in_reverse_order(step: nat, steps: nat)
    requires
        step < steps,
    ensures
        atk_reconstruction_step(step, steps) < steps,
{
    reveal(atk_reconstruction_step);
}

pub proof fn atk_update_parity_alternates(initial_odd: bool, reverse_step: nat)
    ensures
        atk_update_parity(initial_odd, reverse_step + 1)
            != atk_update_parity(initial_odd, reverse_step),
{
    reveal(atk_update_parity);
}

pub proof fn cbd_is_main_header_only()
    ensures
        valid_cbd_scope(true),
        !valid_cbd_scope(false),
{
    reveal(valid_cbd_scope);
}

pub proof fn decomposition_graph_count_is_monotonic(
    no_dwt: nat,
    one_axis: nat,
    bidirectional: nat,
)
    ensures
        decomposition_graph_cardinality(no_dwt, one_axis, bidirectional) >= 1,
        decomposition_graph_cardinality(no_dwt, one_axis, bidirectional)
            == 1 + one_axis + 3 * bidirectional,
{
    reveal(decomposition_graph_cardinality);
}

pub proof fn jpip_fragment_end_is_bounded(offset: nat, length: nat, limit: nat)
    requires
        valid_jpip_fragment(offset, length, limit),
    ensures
        offset + length <= limit,
{
    reveal(valid_jpip_fragment);
}

pub proof fn repeated_jpip_final_length_is_unique(existing: nat, announced: nat)
    requires
        consistent_jpip_final_length(existing, announced),
    ensures
        existing == announced,
{
    reveal(consistent_jpip_final_length);
}

pub proof fn jpip_overlap_is_symmetric(
    first_offset: nat,
    first_length: nat,
    second_offset: nat,
    second_length: nat,
)
    ensures
        jpip_ranges_overlap(first_offset, first_length, second_offset, second_length)
            == jpip_ranges_overlap(second_offset, second_length, first_offset, first_length),
{
    reveal(jpip_ranges_overlap);
}

pub proof fn jpip_complete_view_reaches_final_length(view: DatabinView)
    requires
        view.invariant(),
        view.complete(),
    ensures
        view.contiguous_prefix == view.final_length,
{
}

pub proof fn jpip_complete_packet_range_has_no_missing_bytes(available: nat, required: nat)
    requires
        complete_jpip_range(available, required),
    ensures
        available == required,
{
    reveal(complete_jpip_range);
}

pub proof fn jpip_progressive_layer_count_is_bounded(
    previous_layers: nat,
    available_layers: nat,
    planned_layers: nat,
)
    requires
        valid_jpip_progressive_update(previous_layers, available_layers, planned_layers),
    ensures
        previous_layers <= planned_layers,
        available_layers <= planned_layers,
{
    reveal(valid_jpip_progressive_update);
}

pub proof fn jpip_transcode_length_includes_eoc(
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

pub proof fn jpip_precinct_identifier_is_deterministic(
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

pub proof fn jpip_quality_prefix_is_monotonic(available: nat, requested: nat)
    requires
        jpip_quality_prefix_available(available, requested),
    ensures
        requested <= available,
{
    reveal(jpip_quality_prefix_available);
}

pub proof fn jpip_metadata_replacement_length_is_deterministic(header: nat, payload: nat)
    ensures
        jpip_reconstructed_box_length(header, payload) == header + payload,
{
    reveal(jpip_reconstructed_box_length);
}

pub proof fn jpip_transition_status_is_precise(
    later_functional: bool,
    preceding_packet_count_known: bool,
    preceding_exact_consumption_known: bool,
)
    ensures
        jpip_tile_transition_sufficient(
            later_functional,
            preceding_packet_count_known,
            preceding_exact_consumption_known,
        ) == (
            !later_functional
            || preceding_packet_count_known
            || preceding_exact_consumption_known
        ),
{
    reveal(jpip_tile_transition_sufficient);
}

pub proof fn jpip_packet_boundary_advance_is_bounded(
    consumed: nat,
    packet_length: nat,
    tile_part_length: nat,
)
    requires
        jpip_packet_boundary_step(consumed, packet_length, tile_part_length),
    ensures
        consumed + packet_length <= tile_part_length,
        jpip_packet_boundary_exact(consumed + packet_length, tile_part_length)
            == (consumed + packet_length == tile_part_length),
{
    reveal(jpip_packet_boundary_step);
    reveal(jpip_packet_boundary_exact);
}

pub proof fn selected_codestream_index_is_bounded(index: nat, codestreams: nat)
    requires
        valid_selected_codestream_index(index, codestreams),
    ensures
        codestreams > 0,
        index < codestreams,
{
}

pub proof fn codestream_header_ordinal_target_is_bounded(
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

pub proof fn canonical_jph_brand_has_one_zero_version_compatibility()
    ensures
        valid_jph_branding(true, 0, 1, true),
{
}

pub proof fn consistent_part15_profile_binds_all_three_signals(
    rsiz_announces_ht: bool,
    cap_announces_ht: bool,
    style_uses_ht: bool,
    has_corresponding_profile: bool,
)
    requires
        consistent_part15_signaling(
            rsiz_announces_ht,
            cap_announces_ht,
            style_uses_ht,
            has_corresponding_profile,
        ),
    ensures
        rsiz_announces_ht == cap_announces_ht,
        style_uses_ht ==> cap_announces_ht,
        has_corresponding_profile ==> cap_announces_ht,
{
}

pub proof fn part15_rejects_reserved_style_prefix()
    ensures
        !valid_ht_style_prefix(2),
        valid_ht_style_prefix(0),
        valid_ht_style_prefix(1),
        valid_ht_style_prefix(3),
{
}

pub proof fn part15_rejects_reserved_capability_mode()
    ensures
        valid_part15_block_mode(0),
        !valid_part15_block_mode(1),
        valid_part15_block_mode(2),
        valid_part15_block_mode(3),
{
}

pub proof fn valid_ht_cleanup_has_bounded_disjoint_physical_partition(
    cleanup_length: nat,
    scup: nat,
)
    requires
        valid_ht_cleanup_partition(cleanup_length, scup),
    ensures
        scup >= 2,
        scup <= 4079,
        cleanup_length - scup <= cleanup_length,
        cleanup_length - scup + scup == cleanup_length,
{
    reveal(valid_ht_cleanup_partition);
}

pub proof fn ht_cleanup_shared_byte_is_unique(
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
    reveal(ht_cleanup_byte_claims_valid);
}

pub proof fn valid_ht_quad_origin_is_sample_bounded(
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
        x % 2 == 0,
        y % 2 == 0,
{
    reveal(valid_ht_quad_origin);
}

pub proof fn ht_quad_count_is_bounded_by_samples(width: nat, height: nat)
    requires
        width > 0,
        height > 0,
    ensures
        ht_quad_count(width, height) <= width * height,
{
    reveal(ht_quad_count);
    assert((width + 1) / 2 <= width) by (nonlinear_arith);
    assert((height + 1) / 2 <= height) by (nonlinear_arith);
    assert(((width + 1) / 2) * ((height + 1) / 2) <= width * height) by (nonlinear_arith);
}

pub proof fn source_over_numerators_are_bounded(
    source: u16, destination: u16, source_alpha: u16, destination_alpha: u16, premultiplied: bool,
)
    requires
        !premultiplied || source <= source_alpha,
    ensures
        crate::specs::part2_source_over_alpha_numerator(source_alpha, destination_alpha)
            <= 4294836225,
        crate::specs::part2_source_over_color_numerator(
            source, destination, source_alpha, destination_alpha, premultiplied,
        ) <= 65535 * crate::specs::part2_source_over_alpha_numerator(source_alpha, destination_alpha),
{
    let s = source as int;
    let c = destination as int;
    let a = source_alpha as int;
    let d = destination_alpha as int;
    assert(a * 65535 + d * (65535 - a) <= 4294836225) by (nonlinear_arith)
        requires 0 <= a <= 65535, 0 <= d <= 65535,
    {}
    if premultiplied {
        assert(s * 65535 * 65535 + c * d * (65535 - a)
            <= 65535 * (a * 65535 + d * (65535 - a))) by (nonlinear_arith)
            requires 0 <= s <= a <= 65535, 0 <= c <= 65535, 0 <= d <= 65535,
        {}
    } else {
        assert(s * a * 65535 + c * d * (65535 - a)
            <= 65535 * (a * 65535 + d * (65535 - a))) by (nonlinear_arith)
            requires 0 <= s <= 65535, 0 <= a <= 65535, 0 <= c <= 65535, 0 <= d <= 65535,
        {}
    }
}

pub proof fn source_over_rounding_is_nearest(numerator: u64, denominator: u64)
    ensures
        denominator == 0 ==> crate::specs::part2_source_over_rounded_ratio(numerator, denominator) == 0,
        denominator > 0 ==> 2 * ((denominator as nat)
            * crate::specs::part2_source_over_rounded_ratio(numerator, denominator))
            <= 2 * (numerator as nat) + denominator as nat,
        denominator > 0 ==> 2 * (numerator as nat) < 2 * ((denominator as nat)
            * crate::specs::part2_source_over_rounded_ratio(numerator, denominator))
            + denominator as nat,
{
    if denominator > 0 {
        let d = denominator as int;
        let biased = numerator as int + d / 2;
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod(d, 2);
        vstd::arithmetic::div_mod::lemma_mod_bound(d, 2);
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod(biased, d);
        vstd::arithmetic::div_mod::lemma_mod_bound(biased, d);
    }
}

pub proof fn source_over_rounding_preserves_sample_bound(numerator: u64, denominator: u64)
    requires
        numerator as nat <= 65535 * (denominator as nat),
    ensures
        crate::specs::part2_source_over_rounded_ratio(numerator, denominator) <= 65535,
{
    source_over_rounding_is_nearest(numerator, denominator);
    if denominator > 0 {
        let output = crate::specs::part2_source_over_rounded_ratio(numerator, denominator) as int;
        if output > 65535 {
            vstd::arithmetic::mul::lemma_mul_inequality(65536, output, denominator as int);
            vstd::arithmetic::mul::lemma_mul_is_commutative(output, denominator as int);
            assert(false);
        }
    }
}

pub proof fn ht_kappa_completion_never_reduces_base_u(
    u_base: nat,
    gamma: bool,
    exponent: nat,
)
    ensures
        ht_completed_u(u_base, gamma, exponent) >= u_base,
        !gamma ==> ht_completed_u(u_base, gamma, exponent) == u_base,
        exponent <= 2 ==> ht_completed_u(u_base, gamma, exponent) == u_base,
{
    reveal(ht_completed_u);
    reveal(ht_kappa_extension);
}

pub proof fn ht_pass_group_bounds_placeholders_and_substantive_set(packet_passes: nat)
    requires
        valid_ht_pass_group(packet_passes),
    ensures
        ht_placeholder_passes(packet_passes) % 3 == 0,
        ht_substantive_passes(packet_passes) >= 1,
        ht_substantive_passes(packet_passes) <= 3,
{
    reveal(valid_ht_pass_group);
    reveal(ht_placeholder_passes);
    reveal(ht_substantive_passes);
}

pub proof fn ht_refinement_bounds_bytes_and_passes(bytes: nat, passes: nat)
    requires
        valid_ht_refinement_segment(bytes, passes),
    ensures
        bytes <= 2046,
        passes >= 1,
        passes <= 2,
{
    reveal(valid_ht_refinement_segment);
}

pub proof fn ht_refinement_claims_cannot_alias(
    forward_end: nat,
    reverse_start: nat,
)
    requires
        disjoint_ht_refinement_claims(forward_end, reverse_start),
    ensures
        forward_end <= reverse_start,
{
    reveal(disjoint_ht_refinement_claims);
}

pub proof fn bounded_codestream_count_is_nonzero(codestreams: nat, limit: nat)
    requires
        codestream_count_within_limit(codestreams, limit),
    ensures
        codestreams > 0,
        codestreams <= limit,
{
}

pub proof fn valid_mct_reference_is_nonzero_and_bounded(index: nat, arrays: nat)
    requires
        valid_mct_array_reference(index, arrays),
    ensures
        index > 0,
        index <= arrays,
{
}

pub proof fn valid_mct_dimensions_bound_matrix_and_offsets(
    components: nat,
    matrix_elements: nat,
    offset_elements: nat,
)
    requires
        valid_mct_dimensions(components, matrix_elements, offset_elements),
    ensures
        matrix_elements == components * components,
        offset_elements <= components,
{
}

pub proof fn selected_mct_graph_has_bounded_collections_and_stages(collections: nat, stages: nat)
    requires
        valid_mct_collection_count(collections, stages),
    ensures
        collections > 0,
        collections <= 64,
        stages > 0,
        stages <= 255,
{
}

pub proof fn ordered_mct_stage_edge_is_acyclic(parent: nat, child: nat, stages: nat)
    requires
        valid_mct_stage_edge(parent, child, stages),
    ensures
        parent < child,
        child < stages,
{
}

pub proof fn valid_mct_matrix_is_bounded_and_rectangular(
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

pub proof fn valid_maxshift_has_bounded_component_and_shift(
    component: nat,
    components: nat,
    style: nat,
    shift: nat,
)
    requires
        valid_maxshift_roi(component, components, style, shift),
    ensures
        component < components,
        style == 0,
        shift <= 255,
{
}

pub proof fn magnitude_bit_has_bounded_limb_and_offset(bit: nat)
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

pub proof fn magnitude_shift_stays_in_fixed_width(highest_bit: nat, shift: nat)
    requires
        magnitude_shift_fits(highest_bit, shift),
    ensures
        highest_bit + shift < 384,
{
    reveal(magnitude_shift_fits);
}

pub proof fn magnitude_comparison_is_transitive(left: nat, middle: nat, right: nat)
    requires
        magnitude_less(left, middle),
        magnitude_less(middle, right),
    ensures
        magnitude_less(left, right),
{
    reveal(magnitude_less);
}

pub proof fn magnitude_addition_contains_both_operands(left: nat, right: nat)
    ensures
        magnitude_addition(left, right) >= left,
        magnitude_addition(left, right) >= right,
{
    reveal(magnitude_addition);
}

pub proof fn maxshift_undo_does_not_increase_magnitude(magnitude: nat, threshold: nat)
    requires
        threshold > 0,
    ensures
        maxshift_restored_magnitude(magnitude, threshold) <= magnitude,
{
    reveal(maxshift_restored_magnitude);
}

pub proof fn component_index_width_is_part_one_width(components: nat)
    requires
        components > 0,
    ensures
        component_index_width(components) == 1 || component_index_width(components) == 2,
        (components < 257 ==> component_index_width(components) == 1),
        (components >= 257 ==> component_index_width(components) == 2),
{
}

pub proof fn valid_component_override_is_in_range(component: nat, components: nat)
    requires
        valid_component_override_index(component, components),
    ensures
        components > 0,
        component < components,
{
}

pub proof fn valid_palette_lookup_has_bounded_linear_index(
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

pub proof fn valid_direct_mapping_has_zero_palette_column(
    component: nat,
    components: nat,
    palette_column: nat,
    palette_columns: nat,
)
    requires
        valid_component_mapping(component, components, 0, palette_column, palette_columns),
    ensures
        component < components,
        palette_column == 0,
{
}

pub proof fn valid_palette_mapping_has_bounded_column(
    component: nat,
    components: nat,
    palette_column: nat,
    palette_columns: nat,
)
    requires
        valid_component_mapping(component, components, 1, palette_column, palette_columns),
    ensures
        component < components,
        palette_column < palette_columns,
{
}

pub proof fn unassociated_channel_is_valid(output_channels: nat)
    ensures
        valid_channel_association(0xffff, output_channels),
{
}

pub proof fn valid_color_sample_is_bounded(value: nat, maximum: nat)
    requires
        valid_color_sample(value, maximum),
    ensures
        maximum > 0,
        value <= maximum,
{
}

pub proof fn signed_color_nominalization_is_bounded(value: int, level: nat)
    requires
        valid_signed_color_sample(value, level),
    ensures
        0 <= signed_color_to_nominal(value, level),
        signed_color_to_nominal(value, level) < 2 * level,
{
    reveal(valid_signed_color_sample);
    reveal(signed_color_to_nominal);
}

pub proof fn resampled_color_coordinate_is_component_bounded(
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

pub proof fn packet_length_step_extends_base_128(prefix: nat, group: nat)
    requires
        group < 128,
    ensures
        packet_length_step(prefix, group) == prefix * 128 + group,
        packet_length_step(prefix, group) >= prefix * 128,
{
}

pub proof fn continuous_packet_length_index_is_bounded(expected: nat, actual: nat)
    requires
        packet_length_index_continuous(expected, actual),
    ensures
        actual == expected,
        actual < 256,
{
}

pub proof fn covered_packet_payload_has_exact_boundary(declared: nat, payload: nat)
    requires
        packet_lengths_cover_payload(declared, payload),
    ensures
        declared <= payload,
        payload <= declared,
{
}

pub proof fn continuous_packed_header_index_is_bounded(expected: nat, actual: nat)
    requires
        packed_header_index_continuous(expected, actual),
    ensures
        actual == expected,
        actual < 256,
{
}

pub proof fn bounded_ppm_frame_has_bounded_end(total: nat, position: nat, length: nat)
    requires
        ppm_frame_fits(total, position, length),
    ensures
        position + length <= total,
{
}

pub proof fn packed_packet_partition_is_exact(declared: nat, tile_body: nat, packed_header: nat)
    requires
        packed_packet_length_matches(declared, tile_body, packed_header),
    ensures
        tile_body <= declared,
        packed_header <= declared,
        declared == tile_body + packed_header,
{
}

pub proof fn valid_poc_bounds_contained_coordinates(
    resolution_start: nat,
    resolution_end: nat,
    component_start: nat,
    component_end: nat,
    layer_end: nat,
    components: nat,
    layers: nat,
    resolutions: nat,
    order: nat,
    layer: nat,
    resolution: nat,
    component: nat,
)
    requires
        valid_poc_record(
            resolution_start,
            resolution_end,
            component_start,
            component_end,
            layer_end,
            components,
            layers,
            resolutions,
            order,
        ),
        poc_contains(
            layer,
            resolution,
            component,
            resolution_start,
            resolution_end,
            component_start,
            component_end,
            layer_end,
        ),
    ensures
        layer < layers,
        resolution < resolutions,
        component < components,
        order < 5,
{
}

pub proof fn tile_component_override_has_highest_precedence(
    main_default: int,
    main_component: int,
    has_main_component: bool,
    tile_default: int,
    has_tile_default: bool,
    tile_component: int,
)
    ensures
        effective_component_override(
            main_default,
            main_component,
            has_main_component,
            tile_default,
            has_tile_default,
            tile_component,
            true,
        ) == tile_component,
{
}

pub proof fn tile_default_precedes_main_component(
    main_default: int,
    main_component: int,
    has_main_component: bool,
    tile_default: int,
)
    ensures
        effective_component_override(
            main_default,
            main_component,
            has_main_component,
            tile_default,
            true,
            0,
            false,
        ) == tile_default,
{
}

pub proof fn supported_layer_reference_is_bounded(index: nat, codestreams: nat)
    requires
        valid_layer_codestream_reference(index, codestreams),
    ensures
        index < codestreams,
{
}

pub proof fn implicit_jpx_layers_are_codestream_layers(codestreams: nat)
    ensures
        effective_jpx_layer_count(0, codestreams) == codestreams,
{
}

pub proof fn jp2_primary_excludes_jpx_compatibility()
    ensures
        !valid_jp2_primary_brand(true, true),
{
}

pub proof fn jpip_pair_layout_is_odd(tile_pairs: nat)
    ensures
        valid_jpip_codestream_layout(3 + tile_pairs * 2),
{
}

pub proof fn tile_transform_slice_is_local(start: nat, components: nat, total: nat)
    requires
        tile_transform_scope(start, components, total),
    ensures
        start + components <= total,
{
}

pub proof fn clipped_layer_extent_fits_source_and_canvas(
    offset: nat,
    extent: nat,
    canvas: nat,
)
    ensures
        clipped_composition_extent(offset, extent, canvas) <= extent,
        offset < canvas ==> clipped_composition_extent(offset, extent, canvas) <= canvas - offset,
{
}

pub proof fn composited_storage_includes_alpha(pixels: nat, color_channels: nat)
    ensures
        composition_output_bytes(pixels, color_channels)
            == pixels * (color_channels + 1) * 2,
{
}

pub proof fn registered_extent_numerator_includes_offset(
    samples: nat,
    subsampling: nat,
    offset: nat,
)
    ensures
        registered_sample_numerator(samples, subsampling, offset)
            == samples * subsampling + offset,
{
}

pub proof fn instruction_materialization_is_frame_layer_bounded(frames: nat, layers: nat)
    ensures
        composition_instruction_work(frames, layers)
            == frames * if layers == 0 { 1 } else { layers },
{
}

pub proof fn multi_codestream_channel_assembly_is_additive(
    prior_channels: nat,
    codestream_channels: nat,
)
    ensures
        registered_channel_count(prior_channels, codestream_channels)
            == prior_channels + codestream_channels,
{
}

pub proof fn nested_metadata_accounting_covers_boxes_and_depth(boxes: nat, depth: nat)
    ensures
        nested_metadata_work(boxes, depth) == boxes + depth,
{
}

pub proof fn looped_frame_selection_stays_in_timeline(index: nat, frames: nat)
    requires
        frames > 0,
    ensures
        bounded_loop_frame(index, frames) < frames,
{
}

pub proof fn part15_cap_payload_tracks_ordered_ccap_words(capabilities: u8)
    ensures
        crate::specs::part15_cap_payload_length(capabilities)
            == 4 + 2 * capabilities as usize,
{
}

pub proof fn part15_classification_is_total(
    legacy: bool,
    ht: bool,
    mixed: bool,
)
    ensures
        crate::specs::part15_codestream_class(legacy, ht, mixed) <= 2,
{
    reveal(crate::specs::part15_codestream_class);
}

pub proof fn part15_magnitude_parameter_is_bounded(parameter: nat)
    requires
        parameter < 32,
    ensures
        8 <= crate::specs::part15_magnitude_bound(parameter),
        crate::specs::part15_magnitude_bound(parameter) <= 74,
{
    reveal(crate::specs::part15_magnitude_bound);
}

pub proof fn ht_group_partition_is_exact(passes: nat)
    requires
        1 <= passes,
        passes <= 164,
    ensures
        crate::specs::ht_placeholder_passes(passes)
            + crate::specs::ht_substantive_passes(passes) == passes,
{
    reveal(crate::specs::ht_placeholder_passes);
    reveal(crate::specs::ht_substantive_passes);
}

pub proof fn ht_scup_partitions_cleanup(cleanup: nat, penultimate: u8, last: u8)
    requires
        crate::specs::ht_scup(penultimate, last) as nat <= cleanup,
    ensures
        crate::specs::ht_magsgn_end(
            cleanup,
            crate::specs::ht_scup(penultimate, last) as nat,
        ) + crate::specs::ht_scup(penultimate, last) as nat == cleanup,
{
    reveal(crate::specs::ht_magsgn_end);
}

pub proof fn ht_stuffed_streams_expose_seven_data_bits()
    ensures
        crate::specs::ht_forward_data_bits(true) == 7,
        crate::specs::ht_reverse_data_bits(true, 127) == 7,
{
}

pub proof fn ht_mel_state_and_vlc_index_are_bounded(state: nat, context: nat, lookahead: nat)
    requires
        state <= 12,
        context <= 7,
        lookahead <= 127,
    ensures
        crate::specs::ht_mel_exponent(state) <= 5,
        crate::specs::ht_mel_next_state(state, true) <= 12,
        crate::specs::ht_vlc_table_index(context, lookahead) < 1024,
{
    reveal(crate::specs::ht_mel_exponent);
    reveal(crate::specs::ht_mel_next_state);
    reveal(crate::specs::ht_vlc_table_index);
}

pub proof fn ht_kappa_completion_never_reduces_u(u_base: nat, gamma: bool, exponent: nat)
    ensures
        crate::specs::ht_kappa_completion(u_base, gamma, exponent) >= u_base,
{
    reveal(crate::specs::ht_kappa_completion);
}

pub proof fn ht_bin_center_normalization_is_odd(value: nat)
    ensures
        crate::specs::ht_bin_center_odd(value) % 2 == 1,
        crate::specs::ht_bin_center_odd(value) >= value,
{
    reveal(crate::specs::ht_bin_center_odd);
}

pub proof fn ht_traversal_bounds_stripes_and_patches(height: nat, y: nat, width: nat, x: nat)
    requires
        y <= height,
        x <= width,
    ensures
        crate::specs::ht_stripe_rows(height, y) <= 4,
        crate::specs::ht_patch_end(x, width) <= width,
{
    reveal(crate::specs::ht_stripe_rows);
    reveal(crate::specs::ht_patch_end);
}

pub proof fn ht_refinement_continuation_is_additive(existing: nat, additional: nat)
    ensures
        crate::specs::ht_refinement_pass_total(existing, additional)
            == existing + additional,
{
}

}
