#![no_main]

use jpeg2000_rs::{
    codestream::{CodingStyle, ProgressionOrder, TilePartPayload, WaveletTransform},
    packet::{
        CodeBlockPacketContext, CodeBlockPacketState, PacketBitReader, PacketCodeBlockExecution,
        PacketGrid, PacketIterator, PacketPrecinctDecoder, PacketPrecinctGeometry, TagTree,
        decode_code_block_contribution, decode_code_block_contribution_body,
        decode_multi_code_block_packet, decode_progression_tile_packets,
    },
    tier1::{SubbandOrientation, Tier1CodeBlockDecoder, Tier1CodingStyle},
};
use jpeg2000_rs_fuzz::resource_limits;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let limits = resource_limits();
    let code_block_style = if data.get(7).copied().unwrap_or(0) & 0x40 != 0 {
        0x40
    } else {
        data.get(7).copied().unwrap_or(0) & 0x3f
    };
    let width = u32::from(data.first().copied().unwrap_or(0) % 32) + 1;
    let height = u32::from(data.get(1).copied().unwrap_or(0) % 32) + 1;
    let threshold = u32::from(data.get(2).copied().unwrap_or(0));
    let packet_data = data.get(3..).unwrap_or_default();
    let Ok(mut tree) = TagTree::new(width, height, &limits) else {
        return;
    };
    let Ok(mut reader) = PacketBitReader::new(packet_data, &limits) else {
        return;
    };
    let x = u32::from(data.get(3).copied().unwrap_or(0)) % width;
    let y = u32::from(data.get(4).copied().unwrap_or(0)) % height;
    let _ = tree.decode(x, y, threshold, &mut reader);

    let mut inclusion = TagTree::new(1, 1, &limits).unwrap();
    let mut zero_bitplanes = TagTree::new(1, 1, &limits).unwrap();
    let mut code_block = CodeBlockPacketState::new();
    let mut contribution_reader = PacketBitReader::new(packet_data, &limits).unwrap();
    let contribution = decode_code_block_contribution(
        &mut code_block,
        &mut inclusion,
        &mut zero_bitplanes,
        CodeBlockPacketContext {
            leaf_x: 0,
            leaf_y: 0,
            layer: u16::from(data.get(5).copied().unwrap_or(0)),
            maximum_zero_bitplanes: u16::from(data.get(6).copied().unwrap_or(0)) + 1,
            tier1_style: Tier1CodingStyle::from_code_block_style(code_block_style).unwrap(),
        },
        &mut contribution_reader,
    );
    if let Ok(Some(contribution)) = contribution {
        let significant_bitplanes = 256_u16.saturating_sub(contribution.zero_bitplanes);
        if let Ok(mut decoder) = Tier1CodeBlockDecoder::new(
            1,
            1,
            significant_bitplanes,
            SubbandOrientation::Ll,
            Tier1CodingStyle::from_code_block_style(code_block_style).unwrap(),
        ) {
            let _ = decode_code_block_contribution_body(
                &mut decoder,
                &contribution,
                packet_data,
                &limits,
            );
        }
    }

    let tier1_style = Tier1CodingStyle::from_code_block_style(code_block_style).unwrap();
    let tile_coding_style = CodingStyle {
        progression_order: ProgressionOrder::Lrcp,
        layers: 2,
        multiple_component_transform: false,
        decomposition_levels: 0,
        decomposition_structure: None,
        decomposition_directions: [jpeg2000_rs::codestream::DecompositionDirection::TwoDimensional;
            32],
        code_block_width: 1,
        code_block_height: 1,
        code_block_style,
        transform: WaveletTransform::Reversible5x3,
        precincts_defined: false,
        sop_markers: false,
        eph_markers: false,
        precinct_exponents: [(15, 15); 33],
    };
    let tile_part = TilePartPayload::uniform_for_packet_decoding(
        0,
        0,
        1,
        1,
        tile_coding_style,
        jpeg2000_rs::codestream::RoiShifts::none(),
        packet_data,
    )
    .unwrap();
    let mut packet_states = [CodeBlockPacketState::new(), CodeBlockPacketState::new()];
    let mut packet_inclusion = TagTree::new(2, 1, &limits).unwrap();
    let mut packet_zero_bitplanes = TagTree::new(2, 1, &limits).unwrap();
    let packet_contexts = [
        CodeBlockPacketContext {
            leaf_x: 0,
            leaf_y: 0,
            layer: 0,
            maximum_zero_bitplanes: 1,
            tier1_style,
        },
        CodeBlockPacketContext {
            leaf_x: 1,
            leaf_y: 0,
            layer: 0,
            maximum_zero_bitplanes: 1,
            tier1_style,
        },
    ];
    let first_decoder =
        Tier1CodeBlockDecoder::new(1, 1, 1, SubbandOrientation::Ll, tier1_style).unwrap();
    let second_decoder =
        Tier1CodeBlockDecoder::new(1, 1, 1, SubbandOrientation::Ll, tier1_style).unwrap();
    let mut packet_decoders = [first_decoder, second_decoder];
    let _ = decode_multi_code_block_packet(
        &tile_part,
        0,
        PacketCodeBlockExecution {
            states: &mut packet_states,
            inclusion_tree: &mut packet_inclusion,
            zero_bitplane_tree: &mut packet_zero_bitplanes,
            contexts: &packet_contexts,
            decoders: &mut packet_decoders,
        },
        &limits,
    );

    let layers = u16::from(data.get(5).copied().unwrap_or(0) % 4) + 1;
    let components = u16::from(data.get(6).copied().unwrap_or(0) % 4) + 1;
    let resolutions = data.get(7).copied().unwrap_or(0) % 4 + 1;
    let count_len = usize::from(components) * usize::from(resolutions);
    let precinct_counts = (0..count_len)
        .map(|index| u32::from(data.get(8 + index).copied().unwrap_or(0) % 8) + 1)
        .collect();
    let Ok(grid) = PacketGrid::new(layers, components, resolutions, precinct_counts, &limits)
    else {
        return;
    };
    let order = match data.get(8 + count_len).copied().unwrap_or(0) % 5 {
        0 => ProgressionOrder::Lrcp,
        1 => ProgressionOrder::Rlcp,
        2 => ProgressionOrder::Rpcl,
        3 => ProgressionOrder::Pcrl,
        _ => ProgressionOrder::Cprl,
    };
    let mut iterator = PacketIterator::new(&grid, order);
    while let Ok(Some(_)) = iterator.next_packet() {}

    let progression_grid = PacketGrid::new(2, 1, 1, vec![1], &limits).unwrap();
    let progression_decoder =
        Tier1CodeBlockDecoder::new(1, 1, 1, SubbandOrientation::Ll, tier1_style).unwrap();
    let progression_precinct = PacketPrecinctDecoder::new(
        PacketPrecinctGeometry {
            component: 0,
            resolution: 0,
            precinct: 0,
            code_blocks_across: 1,
            code_blocks_down: 1,
            maximum_zero_bitplanes: 1,
        },
        tier1_style,
        vec![progression_decoder],
        &limits,
    )
    .unwrap();
    let mut progression_precincts = [progression_precinct];
    let _ = decode_progression_tile_packets(
        &[tile_part],
        &progression_grid,
        &mut progression_precincts,
        &limits,
    );
});
