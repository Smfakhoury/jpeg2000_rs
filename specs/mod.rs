use vstd::arithmetic::power2::pow2;
use vstd::prelude::*;

verus! {

pub open spec fn region_fits(parent_length: nat, offset: nat, length: nat) -> bool {
    offset <= parent_length && length <= parent_length - offset
}

pub open spec fn valid_box_lengths(header_length: nat, total_length: nat) -> bool {
    header_length <= total_length
}

pub open spec fn work_within_limit(consumed: nat, limit: nat) -> bool {
    consumed <= limit
}

pub open spec fn valid_selection_region(
    image_x0: nat,
    image_y0: nat,
    image_x1: nat,
    image_y1: nat,
    region_x0: nat,
    region_y0: nat,
    region_x1: nat,
    region_y1: nat,
) -> bool {
    image_x0 <= region_x0
        && image_y0 <= region_y0
        && region_x0 < region_x1
        && region_y0 < region_y1
        && region_x1 <= image_x1
        && region_y1 <= image_y1
}

pub open spec fn reduced_extent(start: nat, end: nat, scale: nat) -> nat
    recommends
        scale > 0,
        start <= end,
{
    (
        (((end + scale - 1) as int) / (scale as int))
            - (((start + scale - 1) as int) / (scale as int))
    ) as nat
}

pub open spec fn selected_packet_work(selected_packets: nat, packet_limit: nat) -> bool {
    selected_packets <= packet_limit
}

pub open spec fn valid_selected_codestream_index(index: nat, codestreams: nat) -> bool {
    codestreams > 0 && index < codestreams
}

pub open spec fn valid_codestream_header_associations(headers: nat, codestreams: nat) -> bool {
    headers <= codestreams
}

pub open spec fn valid_jph_branding(
    primary_is_jph: bool,
    minor_version: nat,
    compatible_brands: nat,
    sole_compatible_is_jph: bool,
) -> bool {
    primary_is_jph
        && minor_version == 0
        && compatible_brands == 1
        && sole_compatible_is_jph
}

pub open spec fn consistent_part15_signaling(
    rsiz_announces_ht: bool,
    cap_announces_ht: bool,
    style_uses_ht: bool,
    has_corresponding_profile: bool,
) -> bool {
    rsiz_announces_ht == cap_announces_ht
        && (!style_uses_ht || cap_announces_ht)
        && (!has_corresponding_profile || cap_announces_ht)
}

pub open spec fn valid_part15_block_mode(mode: nat) -> bool {
    mode == 0 || mode == 2 || mode == 3
}

pub open spec fn valid_part15_capability_word(ccap: nat) -> bool {
    ccap < 65536
        && valid_part15_block_mode(ccap / 16384)
        && ccap % 2048 < 64
}

pub open spec fn valid_ht_style_prefix(prefix: nat) -> bool {
    prefix == 0 || prefix == 1 || prefix == 3
}

pub open spec fn valid_ht_cleanup_partition(cleanup_length: nat, scup: nat) -> bool {
    2 <= scup && scup <= cleanup_length && scup <= 4079
}

pub open spec fn ht_cleanup_byte_claims_valid(
    mel_byte: nat,
    vlc_byte: nat,
    terminal_byte: nat,
    value: u8,
) -> bool {
    mel_byte <= terminal_byte
        && vlc_byte <= terminal_byte
        && (mel_byte < vlc_byte || (mel_byte == vlc_byte && mel_byte < terminal_byte && value != 0xff))
}

pub open spec fn ht_noninitial_vlc_context(
    west_context: u8, north_west: u8, north: u8, north_east: u8,
) -> u8 {
    ((if west_context & 2 != 0 { 2int } else { 0int })
        + (if north_west & 8 != 0 || north & 2 != 0 { 1int } else { 0int })
        + (if north & 8 != 0 || north_east & 2 != 0 { 4int } else { 0int })) as u8
}

pub open spec fn valid_ht_quad_origin(x: nat, y: nat, width: nat, height: nat) -> bool {
    width > 0 && height > 0 && x < width && y < height && x % 2 == 0 && y % 2 == 0
}

pub open spec fn ht_quad_count(width: nat, height: nat) -> nat {
    ((width + 1) / 2) * ((height + 1) / 2)
}

pub open spec fn ht_kappa_extension(gamma: bool, exponent: nat) -> nat {
    if gamma && exponent > 2 {
        (exponent - 2) as nat
    } else {
        0
    }
}

pub open spec fn ht_completed_u(u_base: nat, gamma: bool, exponent: nat) -> nat {
    u_base + ht_kappa_extension(gamma, exponent)
}

pub open spec fn valid_ht_magnitude_width(completed_u: nat) -> bool {
    completed_u > 0 && completed_u <= 75
}

pub open spec fn ht_placeholder_passes(packet_passes: nat) -> nat {
    if packet_passes == 0 {
        0
    } else {
        (((packet_passes - 1) / 3) * 3) as nat
    }
}

pub open spec fn ht_cleanup_passes(packet_passes: nat) -> nat {
    if packet_passes == 0 {
        0
    } else {
        ht_placeholder_passes(packet_passes) + 1
    }
}

pub open spec fn ht_refinement_passes(packet_passes: nat) -> nat {
    (packet_passes - ht_cleanup_passes(packet_passes)) as nat
}

pub open spec fn valid_ht_pass_group(packet_passes: nat) -> bool {
    1 <= packet_passes && packet_passes <= 164
}

pub open spec fn valid_ht_refinement_segment(bytes: nat, passes: nat) -> bool {
    bytes <= 2046 && 1 <= passes && passes <= 2
}

pub open spec fn disjoint_ht_refinement_claims(forward_end: nat, reverse_start: nat) -> bool {
    forward_end <= reverse_start
}

pub open spec fn ht_substantive_passes(packet_passes: nat) -> nat {
    (packet_passes - ht_placeholder_passes(packet_passes)) as nat
}

pub open spec fn mixed_block_selects_ht(lblock: nat, first_length_bit_is_zero: bool) -> bool {
    lblock > 3 && first_length_bit_is_zero
}

pub open spec fn part15_cap_payload_length(capabilities: u8) -> usize {
    (4 + 2 * capabilities as int) as usize
}

pub open spec fn part15_signaling_valid(
    rsiz_ht: bool,
    pcap_ht: bool,
    styles_ht: bool,
    corresponding_profile: bool,
) -> bool {
    rsiz_ht == pcap_ht
        && (!styles_ht || pcap_ht)
        && (!corresponding_profile || pcap_ht)
}

pub open spec fn part15_codestream_class(
    has_legacy: bool,
    has_ht: bool,
    has_mixed: bool,
) -> nat {
    if has_mixed || (has_legacy && has_ht) {
        2
    } else if has_ht {
        1
    } else {
        0
    }
}

pub open spec fn part15_mode_allows(
    mode: nat,
    has_legacy: bool,
    has_mixed: bool,
) -> bool {
    valid_part15_block_mode(mode)
        && (mode != 0 || !has_legacy)
        && (mode != 2 || !has_mixed)
}

pub open spec fn part15_magnitude_bound(parameter: nat) -> nat {
    if parameter == 0 {
        8
    } else if parameter <= 19 {
        parameter + 8
    } else if parameter <= 30 {
        (4 * (parameter - 19) + 27) as nat
    } else {
        74
    }
}

pub open spec fn part15_cleanup_u_bound(
    missing_msbs: nat, placeholder_bitplanes: nat, capability_bound: nat,
) -> nat {
    let bound = missing_msbs + placeholder_bitplanes + 2;
    let capability_limit = if capability_bound < 255 { capability_bound + 1 } else { 255 };
    if bound <= capability_limit { bound } else { capability_limit }
}

pub open spec fn part15_component_allowed(
    roi_allowed: bool,
    has_roi: bool,
    roi_shift: nat,
    irreversible_allowed: bool,
    uses_ht: bool,
    irreversible: bool,
) -> bool {
    (!has_roi || (roi_allowed && roi_shift <= 37))
        && (irreversible_allowed || !uses_ht || !irreversible)
}

pub open spec fn part15_capability_fields_valid(
    mode: nat,
    reserved: nat,
    magnitude_parameter: nat,
) -> bool {
    valid_part15_block_mode(mode) && reserved == 0 && magnitude_parameter < 32
}

pub open spec fn part15_headers_allowed(
    heterogeneous_allowed: bool,
    has_functional_tile_header: bool,
) -> bool {
    heterogeneous_allowed || !has_functional_tile_header
}

pub open spec fn ht_segment_passes(phase: nat, remaining: nat) -> nat {
    if phase == 0 {
        1
    } else if remaining < 3 - phase {
        remaining
    } else {
        (3 - phase) as nat
    }
}

pub open spec fn ht_mixed_selects_legacy(
    segment_length: nat,
    legacy_length: nat,
    cleanup_phase: bool,
    lblock_gt_three: bool,
    leading_zero: bool,
) -> bool {
    (segment_length == 0 && legacy_length != 0)
        || (segment_length != 0 && !(cleanup_phase && lblock_gt_three && leading_zero))
}

pub open spec fn ht_scup(penultimate: u8, last: u8) -> usize {
    (last as int * 16 + penultimate as int % 16) as usize
}

pub open spec fn ht_magsgn_end(cleanup_length: nat, scup: nat) -> nat
    recommends
        scup <= cleanup_length,
{
    (cleanup_length - scup) as nat
}

pub open spec fn ht_forward_data_bits(stuffed: bool) -> nat {
    if stuffed { 7 } else { 8 }
}

pub open spec fn ht_forward_stuffing_valid(stuffed: bool, value: u8) -> bool {
    !stuffed || value < 128
}

pub open spec fn ht_mel_stuffing_valid(previous_ff: bool, value: nat) -> bool {
    !previous_ff || value <= 143
}

pub open spec fn ht_reverse_data_bits(needs_unstuffing: bool, value: u8) -> nat {
    if needs_unstuffing && value % 128 == 127 { 7 } else { 8 }
}

pub open spec fn ht_mel_exponent(state: nat) -> nat {
    if state <= 2 {
        0
    } else if state <= 5 {
        1
    } else if state <= 8 {
        2
    } else if state <= 10 {
        3
    } else if state == 11 {
        4
    } else {
        5
    }
}

pub open spec fn ht_mel_next_state(state: nat, run: bool) -> nat {
    if run {
        if state < 12 { state + 1 } else { 12 }
    } else if state > 0 {
        (state - 1) as nat
    } else {
        0
    }
}

pub open spec fn ht_vlc_table_index(context: nat, lookahead: nat) -> nat {
    context * 128 + lookahead
}

pub open spec fn ht_vlc_word_valid(word: nat, length: nat) -> bool {
    length <= 7 && (word == 0 || length > 0)
}

pub open spec fn ht_uvlc_value(base: nat, suffix: nat, bias: nat) -> nat {
    base + suffix + bias
}

pub open spec fn ht_quad_sample_in_bounds(
    x: nat,
    y: nat,
    sample: nat,
    width: nat,
    height: nat,
) -> bool {
    sample < 4 && x + sample / 2 < width && y + sample % 2 < height
}

pub open spec fn ht_quad_origin_valid(x: nat, y: nat, width: nat, height: nat) -> bool {
    x < width && y < height && x % 2 == 0 && y % 2 == 0
}

pub open spec fn ht_kappa_completion(u_base: nat, gamma: bool, exponent: nat) -> nat {
    u_base + if gamma && exponent > 2 { (exponent - 2) as nat } else { 0 }
}

pub open spec fn ht_magsgn_fill_value() -> nat {
    255
}

pub open spec fn ht_magnitude_payload_bits(completed_u: nat, embedded: bool) -> nat {
    if embedded { (completed_u - 1) as nat } else { completed_u }
}

pub open spec fn ht_magnitude_sign(bit: nat, decoded: bool) -> bool {
    bit == 0 && decoded
}

pub open spec fn ht_bin_center_odd(value: nat) -> nat {
    if value % 2 == 0 { value + 1 } else { value }
}

pub open spec fn ht_claim_available(already_claimed: bool) -> bool {
    !already_claimed
}

pub open spec fn ht_stripe_rows(height: nat, stripe_y: nat) -> nat
    recommends
        stripe_y <= height,
{
    if height - stripe_y < 4 { (height - stripe_y) as nat } else { 4 }
}

pub open spec fn ht_patch_end(patch_x: nat, width: nat) -> nat {
    if patch_x + 4 < width { patch_x + 4 } else { width }
}

pub open spec fn ht_sigprop_eligible(significant: bool, neighbors: nat) -> bool {
    !significant && neighbors > 0
}

pub open spec fn ht_magref_eligible(significant_at_cleanup: bool) -> bool {
    significant_at_cleanup
}

pub open spec fn ht_refinement_pass_total(existing: nat, additional: nat) -> nat {
    existing + additional
}

pub open spec fn ht_multiple_set_allowed(sets_seen: nat, multiple_allowed: bool) -> bool {
    sets_seen == 0 || multiple_allowed
}

pub open spec fn ht_replacement_allowed(
    phase: nat,
    has_cleanup: bool,
    empty_segment: bool,
) -> bool {
    (phase == 0 && empty_segment) || (phase != 0 && has_cleanup)
}

pub open spec fn codestream_count_within_limit(codestreams: nat, limit: nat) -> bool {
    codestreams > 0 && codestreams <= limit
}

pub open spec fn valid_layer_codestream_reference(index: nat, codestreams: nat) -> bool {
    codestreams > 0 && index < codestreams
}

pub open spec fn effective_jpx_layer_count(explicit_layers: nat, codestreams: nat) -> nat {
    if explicit_layers == 0 { codestreams } else { explicit_layers }
}

pub open spec fn valid_jp2_primary_brand(primary_is_jp2: bool, jpx_compatible: bool) -> bool {
    !primary_is_jp2 || !jpx_compatible
}

pub open spec fn valid_jpip_codestream_layout(entries: nat) -> bool {
    entries >= 3 && entries % 2 == 1
}

pub open spec fn tile_transform_scope(start: nat, components: nat, total: nat) -> bool {
    start <= total && components <= total - start
}

pub open spec fn clipped_composition_extent(offset: nat, extent: nat, canvas: nat) -> nat {
    if offset >= canvas {
        0
    } else if extent <= canvas - offset {
        extent
    } else {
        (canvas - offset) as nat
    }
}

pub open spec fn composition_output_bytes(pixels: nat, color_channels: nat) -> nat {
    pixels * (color_channels + 1) * 2
}

pub open spec fn registered_sample_numerator(
    samples: nat,
    subsampling: nat,
    offset: nat,
) -> nat {
    samples * subsampling + offset
}

pub open spec fn composition_instruction_work(frames: nat, layers: nat) -> nat {
    frames * if layers == 0 { 1 } else { layers }
}

pub open spec fn registered_channel_count(
    prior_channels: nat,
    codestream_channels: nat,
) -> nat {
    prior_channels + codestream_channels
}

pub open spec fn nested_metadata_work(boxes: nat, depth: nat) -> nat {
    boxes + depth
}

pub open spec fn bounded_loop_frame(index: nat, frames: nat) -> nat
    recommends frames > 0
{
    index % frames
}

pub open spec fn valid_mct_array_reference(index: nat, arrays: nat) -> bool {
    index > 0 && index <= arrays
}

pub open spec fn valid_mct_dimensions(
    components: nat,
    matrix_elements: nat,
    offset_elements: nat,
) -> bool {
    components > 0
        && matrix_elements == components * components
        && (offset_elements == 0 || offset_elements == components)
}

pub open spec fn mct_work_units(pixels: nat, components: nat) -> nat {
    pixels * components * (components + 1)
}

pub open spec fn valid_mct_collection_count(collections: nat, stages: nat) -> bool {
    collections > 0 && collections <= 64 && stages > 0 && stages <= 255
}

pub open spec fn valid_mct_stage_edge(parent: nat, child: nat, stages: nat) -> bool {
    parent < child && child < stages
}

pub open spec fn valid_mct_matrix_bounds(
    rows: nat,
    columns: nat,
    elements: nat,
    component_limit: nat,
) -> bool {
    rows > 0
        && rows == columns
        && rows <= component_limit
        && elements == rows * columns
}

pub open spec fn valid_mct_fragment_index(index: nat, expected: nat, final_index: nat) -> bool {
    index == expected && index <= final_index
}

pub open spec fn complete_mct_fragment_series(next_index: nat, final_index: nat) -> bool {
    next_index == final_index + 1
}

pub open spec fn mct_dependency_coefficients(components: nat, reversible: bool) -> nat {
    if components == 0 {
        0
    } else if reversible {
        ((components * (components + 1) / 2 - 1) as nat)
    } else {
        ((components * (components - 1) / 2) as nat)
    }
}

pub open spec fn valid_mct_wavelet_partition(
    low_components: nat,
    high_components: nat,
    output_components: nat,
) -> bool {
    low_components + high_components == output_components
}

pub open spec fn mct_wavelet_work_units(
    pixels: nat,
    components: nat,
    levels: nat,
) -> nat {
    pixels * components * (levels + 1) * 8
}

pub open spec fn valid_maxshift_roi(component: nat, components: nat, style: nat, shift: nat) -> bool {
    component < components && style == 0 && shift <= 255
}

pub open spec fn valid_magnitude_bit(bit: nat) -> bool {
    bit < 384
}

pub open spec fn magnitude_limb_index(bit: nat) -> nat {
    bit / 64
}

pub open spec fn magnitude_limb_offset(bit: nat) -> nat {
    bit % 64
}

pub open spec fn magnitude_shift_fits(highest_bit: nat, shift: nat) -> bool {
    highest_bit < 384 && shift < 384 && highest_bit + shift < 384
}

pub open spec fn magnitude_less(left: nat, right: nat) -> bool {
    left < right
}

pub open spec fn magnitude_addition(left: nat, right: nat) -> nat {
    left + right
}

pub open spec fn maxshift_restored_magnitude(magnitude: nat, threshold: nat) -> nat
    recommends
        threshold > 0,
{
    if magnitude < threshold {
        magnitude
    } else {
        magnitude / threshold
    }
}

pub open spec fn component_index_width(components: nat) -> nat {
    if components < 257 { 1 } else { 2 }
}

pub open spec fn valid_component_override_index(component: nat, components: nat) -> bool {
    components > 0 && component < components
}

pub open spec fn palette_linear_index(entry: nat, column: nat, columns: nat) -> nat {
    entry * columns + column
}

pub open spec fn valid_palette_lookup(entry: nat, entries: nat, column: nat, columns: nat) -> bool {
    entries > 0
        && columns > 0
        && entry < entries
        && column < columns
        && palette_linear_index(entry, column, columns) < entries * columns
}

pub open spec fn valid_component_mapping(
    component: nat,
    components: nat,
    mapping_type: nat,
    palette_column: nat,
    palette_columns: nat,
) -> bool {
    component < components
        && ((mapping_type == 0 && palette_column == 0)
            || (mapping_type == 1 && palette_column < palette_columns))
}

pub open spec fn valid_channel_association(association: nat, output_channels: nat) -> bool {
    association <= output_channels || association == 0xffff
}

pub open spec fn valid_color_sample(value: nat, maximum: nat) -> bool {
    maximum > 0 && value <= maximum
}

pub open spec fn color_output_bytes(pixels: nat, channels: nat) -> nat {
    pixels * channels * 2
}

pub open spec fn clamp_color_sample(value: int) -> int {
    if value < 0 {
        0
    } else if value > 65535 {
        65535
    } else {
        value
    }
}

pub open spec fn valid_signed_color_sample(value: int, level: nat) -> bool {
    level > 0 && -(level as int) <= value && value < level
}

pub open spec fn signed_color_to_nominal(value: int, level: nat) -> int {
    value + level as int
}

pub open spec fn resampled_component_coordinate(
    grid_coordinate: nat,
    sampling_step: nat,
    first_component_coordinate: nat,
    component_count: nat,
) -> nat
    recommends
        sampling_step > 0,
        component_count > 0,
{
    let last = (first_component_coordinate + component_count - 1) as nat;
    let nearest = grid_coordinate / sampling_step;
    if nearest < first_component_coordinate {
        first_component_coordinate
    } else if nearest > last {
        last
    } else {
        nearest
    }
}

pub open spec fn packet_length_step(prefix: nat, group: nat) -> nat {
    prefix * 128 + group
}

pub open spec fn packet_length_index_continuous(expected: nat, actual: nat) -> bool {
    expected == actual && actual < 256
}

pub open spec fn packet_lengths_cover_payload(declared: nat, payload: nat) -> bool {
    declared == payload
}

pub open spec fn part9_header_form_valid(form: nat) -> bool {
    1 <= form && form <= 3
}

pub open spec fn part9_header_has_explicit_class(form: nat) -> bool {
    form >= 2
}

pub open spec fn part9_header_has_explicit_codestream(form: nat) -> bool {
    form == 3
}

pub open spec fn part9_vbas_first_is_canonical(payload: nat, continued: bool) -> bool {
    !continued || payload > 0
}

pub open spec fn part9_vbas_step(value: nat, group: nat) -> nat {
    value * 128 + group
}

pub open spec fn part9_auxiliary_present(class_identifier: nat) -> bool {
    class_identifier % 2 == 1
}

pub open spec fn part9_eor_extent(header_length: nat, body_length: nat) -> nat {
    header_length + body_length
}

pub open spec fn part9_class_identifier_valid(class_identifier: nat) -> bool {
    class_identifier <= 8
}

pub open spec fn part9_databin_identity(
    first_class: nat,
    first_codestream: nat,
    first_in_class: nat,
    second_class: nat,
    second_codestream: nat,
    second_in_class: nat,
) -> bool {
    first_class / 2 == second_class / 2
        && first_codestream == second_codestream
        && first_in_class == second_in_class
}

pub open spec fn valid_jpip_fragment(offset: nat, length: nat, limit: nat) -> bool {
    offset <= limit && length <= limit - offset
}

pub open spec fn consistent_jpip_final_length(existing: nat, announced: nat) -> bool {
    existing == announced
}

pub open spec fn jpip_ranges_overlap(
    first_offset: nat,
    first_length: nat,
    second_offset: nat,
    second_length: nat,
) -> bool {
    first_offset < second_offset + second_length
        && second_offset < first_offset + first_length
}

pub open spec fn part9_overlap_consistent(bytes_equal: bool) -> bool {
    bytes_equal
}

pub open spec fn part9_transaction_commit(validation_complete: bool) -> bool {
    validation_complete
}

pub open spec fn part9_merge_start(first: nat, second: nat) -> nat {
    if first < second { first } else { second }
}

pub open spec fn part9_merge_end(first: nat, second: nat) -> nat {
    if first > second { first } else { second }
}

pub open spec fn complete_jpip_databin(contiguous_prefix: nat, final_length: nat) -> bool {
    contiguous_prefix == final_length
}

pub open spec fn complete_jpip_range(available: nat, required: nat) -> bool {
    available == required
}

pub open spec fn part9_missing_length(required_end: nat, available_end: nat) -> nat
    recommends
        available_end <= required_end,
{
    (required_end - available_end) as nat
}

pub open spec fn part9_available_bytes(required: nat, missing: nat) -> nat
    recommends
        missing <= required,
{
    (required - missing) as nat
}

pub open spec fn valid_jpip_progressive_update(
    previous_layers: nat,
    available_layers: nat,
    planned_layers: nat,
) -> bool {
    previous_layers <= available_layers && available_layers <= planned_layers
}

pub open spec fn bounded_jpip_transcode(
    main_header: nat,
    tile_header: nat,
    packet_bytes: nat,
    limit: nat,
) -> bool {
    main_header + tile_header + packet_bytes + 2 <= limit
}

pub open spec fn jpip_precinct_in_class(
    tile: nat,
    tile_count: nat,
    component: nat,
    components: nat,
    precinct_index: nat,
) -> nat {
    tile + tile_count * (component + components * precinct_index)
}

pub open spec fn valid_jpip_precinct_coordinate(
    tile: nat,
    tile_count: nat,
    component: nat,
    components: nat,
) -> bool {
    tile < tile_count && component < components && tile_count > 0 && components > 0
}

pub open spec fn jpip_quality_prefix_available(available: nat, requested: nat) -> bool {
    requested <= available
}

pub open spec fn jpip_reconstructed_box_length(header: nat, payload: nat) -> nat {
    header + payload
}

pub open spec fn part9_metadata_descent_allowed(
    depth: nat,
    maximum_depth: nat,
    cycle: bool,
) -> bool {
    depth <= maximum_depth && !cycle
}

pub open spec fn part9_reconstruction_length(
    main_header: nat,
    tile_headers: nat,
    packet_data: nat,
    eoc: nat,
) -> nat {
    main_header + tile_headers + packet_data + eoc
}

pub open spec fn part9_precinct_flat_identity(
    tile: nat,
    tile_count: nat,
    component_precinct: nat,
) -> nat {
    tile + component_precinct * tile_count
}

pub open spec fn part9_reframed_marker_removed(marker: nat, tile_header: bool) -> bool {
    if tile_header {
        marker == 0xff58 || marker == 0xff61
    } else {
        marker == 0xff55 || marker == 0xff57 || marker == 0xff60
    }
}

pub open spec fn part9_progression_supported(order: nat) -> bool {
    order <= 4
}

pub open spec fn part9_packet_reframed_length(payload: nat, sop: bool, eph: bool) -> nat {
    payload + (if sop { 6 as nat } else { 0 as nat })
        + (if eph { 2 as nat } else { 0 as nat })
}

pub open spec fn jpip_tile_transition_sufficient(
    later_functional: bool,
    preceding_packet_count_known: bool,
    preceding_exact_consumption_known: bool,
) -> bool {
    !later_functional || preceding_packet_count_known || preceding_exact_consumption_known
}

pub open spec fn jpip_packet_boundary_step(
    consumed: nat,
    packet_length: nat,
    tile_part_length: nat,
) -> bool {
    consumed + packet_length <= tile_part_length
}

pub open spec fn jpip_packet_boundary_exact(consumed: nat, tile_part_length: nat) -> bool {
    consumed == tile_part_length
}

pub open spec fn packed_header_index_continuous(expected: nat, actual: nat) -> bool {
    expected == actual && actual < 256
}

pub open spec fn ppm_frame_fits(total: nat, position: nat, length: nat) -> bool {
    position <= total && length <= total - position
}

pub open spec fn packed_packet_length_matches(
    declared: nat,
    tile_body: nat,
    packed_header: nat,
) -> bool {
    declared == tile_body + packed_header
}

pub open spec fn valid_poc_record(
    resolution_start: nat,
    resolution_end: nat,
    component_start: nat,
    component_end: nat,
    layer_end: nat,
    components: nat,
    layers: nat,
    resolutions: nat,
    order: nat,
) -> bool {
    resolution_start < resolution_end
        && resolution_end <= resolutions
        && component_start < component_end
        && component_end <= components
        && 0 < layer_end
        && layer_end <= layers
        && order < 5
}

pub open spec fn poc_contains(
    layer: nat,
    resolution: nat,
    component: nat,
    resolution_start: nat,
    resolution_end: nat,
    component_start: nat,
    component_end: nat,
    layer_end: nat,
) -> bool {
    layer < layer_end
        && resolution_start <= resolution
        && resolution < resolution_end
        && component_start <= component
        && component < component_end
}

pub open spec fn effective_component_override(
    main_default: int,
    main_component: int,
    has_main_component: bool,
    tile_default: int,
    has_tile_default: bool,
    tile_component: int,
    has_tile_component: bool,
) -> int {
    if has_tile_component {
        tile_component
    } else if has_tile_default {
        tile_default
    } else if has_main_component {
        main_component
    } else {
        main_default
    }
}

pub open spec fn decomposition_stage_subbands(direction: nat) -> nat {
    if direction == 0 {
        0
    } else if direction == 1 {
        3
    } else {
        1
    }
}

pub open spec fn valid_decomposition_direction(direction: nat) -> bool {
    direction < 4
}

pub open spec fn decomposition_graph_cardinality(
    no_dwt_stages: nat,
    one_axis_stages: nat,
    bidirectional_stages: nat,
) -> nat {
    1 + one_axis_stages + 3 * bidirectional_stages
}

pub open spec fn decomposition_axis_scale(horizontal_splits: nat) -> nat {
    pow2(horizontal_splits)
}

pub open spec fn valid_decomposition_dimensions(
    parent_width: nat,
    parent_height: nat,
    low_width: nat,
    low_height: nat,
    high_width: nat,
    high_height: nat,
    split_horizontal: bool,
    split_vertical: bool,
) -> bool {
    (!split_horizontal
        || (low_width + high_width == parent_width && low_height == parent_height))
        && (!split_vertical
            || (low_height + high_height == parent_height && low_width == parent_width))
}

pub open spec fn inverse_stage_order(stage: nat, stages: nat) -> nat
    recommends
        stage < stages,
{
    (stages - stage - 1) as nat
}

pub open spec fn packed_two_bit_bytes(elements: nat) -> nat {
    (elements + 3) / 4
}

pub open spec fn ads_payload_bytes(depth_elements: nat, split_elements: nat) -> nat {
    3 + packed_two_bit_bytes(depth_elements) + packed_two_bit_bytes(split_elements)
}

pub open spec fn atk_reconstruction_step(step: nat, steps: nat) -> nat
    recommends
        step < steps,
{
    (steps - step - 1) as nat
}

pub open spec fn atk_update_parity(initial_odd: bool, reverse_step: nat) -> bool {
    if reverse_step % 2 == 0 {
        initial_odd
    } else {
        !initial_odd
    }
}

pub open spec fn valid_cbd_scope(in_main_header: bool) -> bool {
    in_main_header
}

// Selected Part 2 and JPX functional semantics used by executable production contracts.
pub open spec fn part2_tlm_tile_index_bytes(style: u8) -> usize {
    (((style >> 4) & 3) as int) as usize
}

pub open spec fn part2_tlm_length_bytes(style: u8) -> usize {
    if style & 0x40 == 0 { 2 } else { 4 }
}

pub open spec fn part2_crg_payload_bytes(components: u16) -> usize {
    ((components as int) * 4) as usize
}

pub open spec fn part2_packed_two_bit_bytes(elements: usize) -> usize {
    ((elements + 3) / 4) as usize
}

pub open spec fn part2_dfs_direction_code(byte: u8, slot: u8) -> u8
    recommends slot < 4,
{
    (byte >> (6 - 2 * slot)) & 3
}

pub open spec fn part2_cbd_precision(encoded: u8) -> u8 {
    (((encoded & 0x7f) as int) + 1) as u8
}

pub open spec fn part2_cbd_signed(encoded: u8) -> bool {
    encoded & 0x80 != 0
}

pub open spec fn part2_nlt_binary_complement(source: i128, half: i128) -> i128 {
    if source < 0 {
        (-(source as int) - ((half as int) + 1)) as i128
    } else {
        source
    }
}

pub open spec fn part2_atk_reconstruction_step(remaining: usize) -> usize
    recommends remaining > 0,
{
    (remaining - 1) as usize
}

pub open spec fn part2_atk_next_parity(current: bool) -> bool {
    !current
}

pub open spec fn part2_fragment_next(index: u16) -> u16
    recommends index < u16::MAX,
{
    ((index as int) + 1) as u16
}

pub open spec fn part2_fragment_complete(next: u16, final_index: u16) -> bool {
    next as int == final_index as int + 1
}

pub open spec fn part2_mct_reverse_stage(remaining: usize) -> usize
    recommends remaining > 0,
{
    (remaining - 1) as usize
}

pub open spec fn part2_binary_fixed_shift(binary_exponent: i32) -> i32
    recommends
        i32::MIN + 32 <= binary_exponent,
        binary_exponent <= i32::MAX - 32,
{
    ((binary_exponent as int) + 32) as i32
}

pub open spec fn part2_palette_coordinates_valid(
    entry: u16,
    entries: u16,
    column: u8,
    columns: u8,
) -> bool {
    entry < entries && column < columns
}

pub open spec fn part2_component_mapping_valid(
    component: u16,
    components: u16,
    mapping_type: u8,
    palette_column: u8,
    palette_columns: u8,
) -> bool {
    component < components
        && ((mapping_type == 0 && palette_column == 0)
            || (mapping_type == 1
                && palette_columns > 0
                && palette_column < palette_columns))
}

pub open spec fn part2_channel_definition_valid(
    channel: u16,
    channel_type: u16,
    association: u16,
    output_channels: u16,
) -> bool {
    channel < output_channels
        && (channel_type == 0
            || channel_type == 1
            || channel_type == 2
            || channel_type == 0xffff)
        && (association <= output_channels || association == 0xffff)
}

pub open spec fn part2_first_override_allowed(already_overridden: bool) -> bool {
    !already_overridden
}

pub open spec fn part2_signed_to_nominal(value: i64, level: i64) -> i64 {
    ((value as int) + (level as int)) as i64
}

pub open spec fn part2_effective_layer_count(
    explicit_layers: usize,
    codestreams: usize,
) -> usize {
    if explicit_layers == 0 { codestreams } else { explicit_layers }
}

pub open spec fn part2_registered_extent(numerator: u64, grid: u64) -> u64
    recommends grid > 0,
{
    if numerator == 0 {
        0
    } else {
        (1 + ((numerator - 1) as int) / (grid as int)) as u64
    }
}

pub open spec fn part2_registered_source_coordinate(
    numerator: u32,
    offset: u32,
    subsampling: u32,
) -> u32
    recommends
        subsampling > 0,
        offset <= numerator,
{
    (((numerator as int) - (offset as int)) / (subsampling as int)) as u32
}

pub open spec fn part2_composition_clip(offset: u32, extent: u32, canvas: u32) -> u32 {
    if offset >= canvas {
        0
    } else if extent <= canvas - offset {
        extent
    } else {
        ((canvas as int) - (offset as int)) as u32
    }
}

pub open spec fn part2_loop_frame(index: usize, frames: usize) -> usize
    recommends frames > 0,
{
    (index % frames) as usize
}

pub open spec fn part2_inverse_alpha(source_alpha: u16) -> u16 {
    (65535 - source_alpha as int) as u16
}

pub open spec fn part2_chroma_key_alpha(matches: bool) -> u16 {
    if matches { 0 } else { u16::MAX }
}

pub open spec fn part2_metadata_count(parent: u32, child: u32) -> u32
    recommends parent <= u32::MAX - child,
{
    ((parent as int) + (child as int)) as u32
}

// Part 1 functional semantics used by executable production contracts.
pub open spec fn part1_ceil_div(value: nat, divisor: nat) -> nat
    recommends divisor > 0,
{
    if value == 0 {
        0
    } else {
        ((((value - 1) as int) / (divisor as int)) + 1int) as nat
    }
}

pub open spec fn part1_clip_start(nominal: nat, image_start: nat) -> nat {
    if nominal < image_start { image_start } else { nominal }
}

pub open spec fn part1_clip_end(nominal_end: nat, image_end: nat) -> nat {
    if nominal_end < image_end { nominal_end } else { image_end }
}

pub open spec fn part1_packet_data_bits(previous_was_ff: bool) -> nat {
    if previous_was_ff { 7 } else { 8 }
}

pub open spec fn part1_packet_bit(byte: u8, remaining: u8) -> bool
    recommends remaining < 8,
{
    ((byte >> remaining) & 1) != 0
}

pub open spec fn part1_tag_inherited_low(low: nat, inherited: nat) -> nat {
    if low < inherited { inherited } else { low }
}

pub open spec fn part1_tag_step_low(low: nat, bit: bool) -> nat {
    if bit { low } else { low + 1 }
}

pub open spec fn part1_tag_step_value(low: nat, value: nat, bit: bool) -> nat {
    if bit { low } else { value }
}

pub open spec fn part1_progression_digit(index: nat, dimension: nat) -> nat
    recommends dimension > 0,
{
    (index % dimension) as nat
}

pub open spec fn part1_progression_advance(index: nat, dimension: nat) -> nat
    recommends dimension > 0,
{
    (index / dimension) as nat
}

pub open spec fn part1_progression_emit_allowed(already_seen: bool) -> bool {
    !already_seen
}

pub open spec fn part1_mq_decision(mps: bool, lps: bool) -> bool {
    if lps { !mps } else { mps }
}

pub open spec fn part1_mq_next_mps(mps: bool, switch_mps: bool) -> bool {
    if switch_mps { !mps } else { mps }
}

pub open spec fn part1_tier1_pass_code(pass_index: nat) -> nat {
    if pass_index == 0 { 2 } else { ((pass_index - 1) % 3) as nat }
}

pub open spec fn part1_tier1_refinement_context(refined: bool, neighbors: nat) -> nat {
    if refined { 16 } else if neighbors > 0 { 15 } else { 14 }
}

pub open spec fn part1_tier1_refined_magnitude(
    magnitude: nat,
    delta: nat,
    refinement_bit: bool,
) -> nat {
    if refinement_bit { magnitude + delta } else { magnitude }
}

pub open spec fn part1_maxshift_restore(magnitude: u64, shift: u8) -> u64 {
    if shift >= 64 {
        magnitude
    } else {
        let threshold = 1_u64 << shift;
        if magnitude < threshold { magnitude } else { magnitude >> shift }
    }
}

pub open spec fn part1_tier1_midpoint_delta(bitplane: u16) -> u64 {
    if bitplane == 0 || bitplane > 64 { 0 } else { 1_u64 << (bitplane - 1) }
}

pub open spec fn part1_reversible_inverse_quantize(coefficient: int) -> int {
    coefficient
}

pub open spec fn part1_five_three_predict(sum: i64) -> i64 {
    sum >> 1
}

pub open spec fn part1_five_three_update(biased_sum: i64) -> i64 {
    biased_sum >> 2
}

pub open spec fn part1_rct_green(y: i64, chroma_sum: i64) -> i64 {
    ((y as int) - ((chroma_sum >> 2) as int)) as i64
}

pub open spec fn part1_rct_red(green: int, cr: int) -> int {
    green + cr
}

pub open spec fn part1_rct_blue(green: int, cb: int) -> int {
    green + cb
}

pub open spec fn part1_level_shift(precision: u8, signed: bool) -> i64
    recommends precision > 0 && precision <= 63,
{
    if signed { 0 } else { 1_i64 << (precision - 1) }
}

pub open spec fn part1_clip_sample(value: i64, minimum: i64, maximum: i64) -> i64 {
    if value < minimum { minimum } else if value > maximum { maximum } else { value }
}

pub open spec fn part1_marker_value(high: u8, low: u8) -> u16 {
    ((high as u16) << 8) | (low as u16)
}

pub open spec fn part1_parser_progress(previous: usize, current: usize, limit: usize) -> bool {
    previous < current && current <= limit
}

pub open spec fn part1_main_header_boundary(marker: u16) -> bool {
    marker == 0xff4f || marker == 0xff93 || marker == 0xffd9
}

pub open spec fn part1_tile_header_boundary(marker: u16) -> bool {
    marker == 0xff4f || marker == 0xff51 || marker == 0xff90 || marker == 0xffd9
}

pub open spec fn part1_packet_next_included(old: bool, contribution_present: bool) -> bool {
    old || contribution_present
}

pub open spec fn part1_packet_next_lblock(lblock: u8) -> u8
    recommends lblock < 31,
{
    ((lblock as int) + 1) as u8
}

pub open spec fn part1_packet_next_passes(old: u32, added: u32) -> u32
    recommends old <= u32::MAX - added,
{
    ((old as int) + (added as int)) as u32
}

pub open spec fn part1_exact_consumption(consumed: usize, expected: usize) -> bool {
    consumed == expected
}

pub open spec fn part1_packet_logical_bytes(tile: usize, packed: usize) -> usize
    recommends tile <= usize::MAX - packed,
{
    ((tile as int) + (packed as int)) as usize
}

pub open spec fn part1_mq_interval_subtract(interval: u32, qe: u16) -> u32
    recommends (qe as u32) <= interval,
{
    ((interval as int) - (qe as int)) as u32
}

pub open spec fn part1_mq_code_subtract(code: u32, qe: u16) -> u32
    recommends (qe as u64) * 65536 <= code as u64,
{
    ((code as int) - (qe as int) * 65536) as u32
}

pub open spec fn part1_mq_byte_class(current: u8, next: u8) -> u8 {
    if current == 0xff && next > 0x8f {
        0
    } else if current == 0xff {
        1
    } else {
        2
    }
}

pub open spec fn part1_mq_byte_addend(current: u8, next: u8) -> u32 {
    if current == 0xff && next > 0x8f {
        0xff00
    } else if current == 0xff {
        (next as u32) << 9
    } else {
        (next as u32) << 8
    }
}

pub open spec fn part1_mq_byte_ct(current: u8, next: u8) -> u8 {
    if current == 0xff && next <= 0x8f { 7 } else { 8 }
}

pub open spec fn part1_mq_byte_advances(current: u8, next: u8) -> bool {
    !(current == 0xff && next > 0x8f)
}

pub open spec fn part1_mq_renorm_double(value: u32) -> u32
    recommends value <= u32::MAX / 2,
{
    ((value as int) * 2) as u32
}

pub open spec fn part1_mq_renorm_ct(ct: u8) -> u8
    recommends ct > 0,
{
    ((ct as int) - 1) as u8
}

pub open spec fn part1_tier1_stripe_rows(height: u16, y: u16) -> u16
    recommends y < height,
{
    if (height as int) - (y as int) < 4 {
        ((height as int) - (y as int)) as u16
    } else {
        4
    }
}

pub open spec fn part1_tier1_sigprop_eligible(significant: bool, neighbors: u16) -> bool {
    !significant && neighbors > 0
}

pub open spec fn part1_tier1_magref_eligible(significant_before: bool) -> bool {
    significant_before
}

pub open spec fn part1_tier1_cleanup_eligible(
    significant: bool,
    visited: bool,
) -> bool {
    !significant && !visited
}

pub open spec fn part1_tier1_pass_complete(kind: u8) -> u8 {
    if kind == 0 { 1 } else if kind == 1 { 2 } else { 4 }
}

pub open spec fn part1_tier1_zero_context(
    horizontal: u16,
    vertical: u16,
    diagonal: u16,
    high_high: bool,
) -> u8 {
    if high_high {
        let direct = (horizontal as int) + (vertical as int);
        if diagonal == 0 {
            if direct == 0 { 0 } else if direct == 1 { 1 } else { 2 }
        } else if diagonal == 1 {
            if direct == 0 { 3 } else if direct == 1 { 4 } else { 5 }
        } else if diagonal == 2 {
            if direct == 0 { 6 } else { 7 }
        } else {
            8
        }
    } else if horizontal == 0 {
        if vertical == 0 {
            if diagonal == 0 { 0 } else if diagonal == 1 { 1 } else { 2 }
        } else if vertical == 1 {
            3
        } else {
            4
        }
    } else if horizontal == 1 {
        if vertical == 0 {
            if diagonal == 0 { 5 } else { 6 }
        } else {
            7
        }
    } else {
        8
    }
}

pub open spec fn part1_tier1_sign_offset(horizontal: i8, vertical: i8) -> u8 {
    if horizontal == 0 {
        if vertical == 0 { 0 } else { 1 }
    } else if vertical < 0 {
        2
    } else if vertical == 0 {
        3
    } else {
        4
    }
}

pub open spec fn part1_tier1_predicted_negative(horizontal: i8, vertical: i8) -> bool {
    horizontal < 0 || (horizontal == 0 && vertical < 0)
}

pub open spec fn part1_tier1_decoded_sign(symbol: bool, predicted: bool, raw: bool) -> bool {
    if raw { symbol } else { symbol != predicted }
}

pub open spec fn part1_irreversible_centered(magnitude: i64) -> i128 {
    ((magnitude as int) * 2 + 1) as i128
}

pub open spec fn part1_irreversible_scale(product: i128) -> i128
    recommends product >= 0,
{
    ((product as int) / 2) as i128
}

pub open spec fn part1_fixed_round_q16(adjusted: i64) -> i64 {
    adjusted >> 16
}

pub open spec fn part1_nine_seven_scale(low: bool) -> i64 {
    if low { 80621 } else { 53274 }
}

pub open spec fn part1_nine_seven_lift(stage: u8) -> i64 {
    if stage == 0 {
        29066
    } else if stage == 1 {
        57862
    } else if stage == 2 {
        (-3472int) as i64
    } else {
        (-103949int) as i64
    }
}

pub open spec fn part1_wavelet_detail_advance(direction: u8) -> usize {
    if direction == 3 { 3 } else if direction == 0 { 0 } else { 1 }
}

pub open spec fn part1_wavelet_next_resolution(resolution: u8) -> u8
    recommends resolution < u8::MAX,
{
    ((resolution as int) + 1) as u8
}

pub open spec fn part1_wavelet_sample_count(
    width: u32,
    height: u32,
    samples: usize,
) -> bool {
    if width == 0 {
        samples == 0
    } else {
        samples % width as usize == 0
            && samples / width as usize == height as usize
    }
}

pub open spec fn part1_sop_sequence(high: u8, low: u8) -> u16 {
    ((high as u16) << 8) | (low as u16)
}

pub open spec fn part1_sop_valid(
    marker0: u8,
    marker1: u8,
    length0: u8,
    length1: u8,
) -> bool {
    marker0 == 0xff && marker1 == 0x91 && length0 == 0 && length1 == 4
}

pub open spec fn part1_eph_valid(marker0: u8, marker1: u8) -> bool {
    marker0 == 0xff && marker1 == 0x92
}

pub open spec fn part1_packet_length_matches(actual: usize, expected: usize) -> bool {
    actual == expected
}

pub open spec fn part1_decode_output_shape_valid(
    input_bytes: usize,
    tile_parts: usize,
    expected_components: u16,
    output_components: usize,
    width: u32,
    height: u32,
) -> bool {
    input_bytes > 0
        && tile_parts > 0
        && expected_components > 0
        && expected_components as usize == output_components
        && width > 0
        && height > 0
}

pub open spec fn part1_component_output_shape_valid(
    width: u32,
    height: u32,
    samples: usize,
) -> bool {
    width > 0 && height > 0 && part1_wavelet_sample_count(width, height, samples)
}

}
