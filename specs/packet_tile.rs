use super::{Part1PacketGrid, Part1PacketPrecinctState, ht_reader_error};
use vstd::prelude::*;

verus! {

pub struct Part1TilePacketPreparation {
    pub tile_index:u16,
    pub index:usize,
    pub payload_bytes:u64,
    pub packed_payload_bytes:u64,
    pub declared_packets:u64,
    pub all_lengths:bool,
    pub packed:Seq<Seq<u8>>,
}

#[verifier::opaque]
pub open spec fn part1_progression_resolution_levels(
    first:&crate::codestream::TilePartPayload<'_>,components:u16,index:u16,maximum:u8,
) -> Result<u8,crate::DecodeError>
    decreases if index>=components {0nat} else {(components-index) as nat},
{
    if index>=components {Ok(maximum)}
    else {
        match crate::codestream::component_coding_style_result(&first.component_coding_styles,index) {
            Err(error)=>Err(error),
            Ok(style)=>if style.decomposition_levels==u8::MAX || index==u16::MAX {
                Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
            } else {
                let count=(style.decomposition_levels+1) as u8;
                part1_progression_resolution_levels(first,components,(index+1) as u16,if count>maximum {count}else{maximum})
            },
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_progression_selection_check(
    selected:Seq<u8>,available:Seq<u8>,index:usize,
) -> Result<(),crate::DecodeError>
    decreases selected.len()-index as nat,
{
    if index>=selected.len() {Ok(())}
    else if index>=available.len() || selected[index as int]==0 || selected[index as int]>available[index as int] {
        Err(ht_reader_error(crate::MalformedKind::InvalidCodingStyle))
    } else if index==usize::MAX {Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))}
    else {part1_progression_selection_check(selected,available,(index+1) as usize)}
}

pub open spec fn part1_progression_layout(
    parts:Seq<crate::codestream::TilePartPayload<'_>>,grid:Part1PacketGrid,
    selection:Option<Seq<u8>>,limits:&crate::ResourceLimits,
) -> Result<(),crate::DecodeError> {
    if parts.len()==0 {Err(ht_reader_error(crate::MalformedKind::InvalidTilePart))}
    else if parts.len() as u64>crate::limits::resource_tile_part_limit(limits) as u64 {
        Err(super::part1_packet_reader_limit(crate::Resource::TileParts,
            crate::limits::resource_tile_part_limit(limits) as u64,parts.len() as u64))
    } else {
        match part1_progression_resolution_levels(&parts[0],grid.components,0,0) {
            Err(error)=>Err(error),
            Ok(maximum)=>if parts[0].coding_style.layers!=grid.layers || maximum!=grid.resolutions {
                Err(ht_reader_error(crate::MalformedKind::InvalidCodingStyle))
            } else {
                match selection {
                    None=>Ok(()),
                    Some(selected)=>if selected.len()!=grid.components as nat {
                        Err(ht_reader_error(crate::MalformedKind::InvalidCodingStyle))
                    } else {part1_progression_selection_check(selected,grid.component_resolutions,0)},
                }
            },
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_progression_payloads(
    parts:Seq<crate::codestream::TilePartPayload<'_>>,s:Part1TilePacketPreparation,maximum_header:u64,
) -> Result<Part1TilePacketPreparation,crate::DecodeError>
    decreases parts.len()-s.index as nat,
{
    if s.index>=parts.len() {Ok(s)}
    else {
        let part=parts[s.index as int];
        if s.index>u8::MAX || part.tile_index!=s.tile_index || part.tile_part_index!=s.index
            || (part.tile_part_count!=0 && part.tile_part_count as nat!=parts.len()) {
            Err(ht_reader_error(crate::MalformedKind::InvalidTilePart))
        } else if s.payload_bytes as nat+(part.data@.len() as u64) as nat>u64::MAX {
            Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
        } else {
            match super::part1_packed_header_reassembly(
                crate::codestream::packed_packet_headers_view(&part.packed_packet_headers),maximum_header) {
                Err(error)=>Err(error),
                Ok(packed)=>if s.packed_payload_bytes as nat+(packed.len() as u64) as nat>u64::MAX {
                    Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
                } else if s.packed_payload_bytes as nat+(packed.len() as u64) as nat>maximum_header {
                    Err(super::part1_packet_reader_limit(crate::Resource::PacketHeaderBytes,
                        maximum_header,(s.packed_payload_bytes+packed.len() as u64) as u64))
                } else {
                    let lengths=crate::codestream::packet_lengths_view(&part.packet_lengths);
                    let present=!matches!(lengths.source,super::Part1PacketLengthSource::None);
                    if present && s.declared_packets as nat+lengths.packet_count as nat>u64::MAX {
                        Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))
                    } else if s.index==usize::MAX {Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))}
                    else {part1_progression_payloads(parts,Part1TilePacketPreparation {
                        tile_index:s.tile_index,index:(s.index+1) as usize,
                        payload_bytes:(s.payload_bytes+part.data@.len() as u64) as u64,
                        packed_payload_bytes:(s.packed_payload_bytes+packed.len() as u64) as u64,
                        declared_packets:if present {(s.declared_packets+lengths.packet_count) as u64}else{s.declared_packets},
                        all_lengths:s.all_lengths&&present,packed:s.packed.push(packed),
                    },maximum_header)}
                },
            }
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_progression_precinct_indices(
    grid:Part1PacketGrid,precincts:Seq<Part1PacketPrecinctState>,index:usize,
) -> Result<(),crate::DecodeError>
    decreases precincts.len()-index as nat,
{
    if index>=precincts.len() {Ok(())}
    else {
        let precinct=precincts[index as int];
        match super::part1_packet_grid_precinct_index(grid,precinct.component,precinct.resolution,precinct.precinct) {
            Err(error)=>Err(error),
            Ok(expected)=>if expected!=index {Err(ht_reader_error(crate::MalformedKind::InvalidPacketHeader))}
                else if index==usize::MAX {Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))}
                else {part1_progression_precinct_indices(grid,precincts,(index+1) as usize)},
        }
    }
}

pub open spec fn part1_progression_precinct_layout(
    grid:Part1PacketGrid,precincts:Seq<Part1PacketPrecinctState>,
) -> Result<(),crate::DecodeError> {
    match super::part1_packet_grid_total_precincts(grid) {
        Err(error)=>Err(error),
        Ok(count)=>if count as nat>usize::MAX {Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow))}
            else if precincts.len()!=count as nat {Err(ht_reader_error(crate::MalformedKind::InvalidPacketHeader))}
            else {part1_progression_precinct_indices(grid,precincts,0)},
    }
}

pub open spec fn part1_progression_tile_input(
    parts:Seq<crate::codestream::TilePartPayload<'_>>,grid:Part1PacketGrid,
    precincts:Seq<Part1PacketPrecinctState>,selection:Option<Seq<u8>>,limits:&crate::ResourceLimits,
) -> Result<Part1TilePacketPreparation,crate::DecodeError> {
    match part1_progression_layout(parts,grid,selection,limits) {
        Err(error)=>Err(error),
        Ok(())=>match part1_progression_payloads(parts,Part1TilePacketPreparation {
            tile_index:parts[0].tile_index,index:0,payload_bytes:0,packed_payload_bytes:0,
            declared_packets:0,all_lengths:true,packed:Seq::empty(),
        },crate::limits::resource_packet_header_byte_limit(limits)) {
            Err(error)=>Err(error),
            Ok(input)=>if input.all_lengths && input.declared_packets!=grid.packet_count {
                Err(ht_reader_error(crate::MalformedKind::InvalidPacketLength))
            } else {match part1_progression_precinct_layout(grid,precincts) {
                Err(error)=>Err(error),Ok(())=>Ok(input),
            }},
        },
    }
}

}
