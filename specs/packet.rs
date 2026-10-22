use vstd::prelude::*;

verus! {

pub struct Part1CodeBlockContribution {
    pub first_inclusion: bool,
    pub zero_bitplanes: u16,
    pub new_passes: u8,
    pub data_length: u32,
    pub high_throughput: bool,
    pub segment_count: u8,
    pub segment_passes: Seq<u8>,
    pub segment_lengths: Seq<u32>,
}

pub open spec fn part1_code_block_segment(contribution: Part1CodeBlockContribution, index: u8)
    -> Result<crate::packet::CodeBlockSegmentContribution, crate::DecodeError>
{
    if index >= contribution.segment_count {
        Err(super::part1_packet_header_error())
    } else if index as nat >= contribution.segment_passes.len() || index as nat >= contribution.segment_lengths.len() {
        Err(super::part1_packet_header_error())
    } else {
        Ok(crate::packet::CodeBlockSegmentContribution {
            passes: contribution.segment_passes[index as int],
            data_length: contribution.segment_lengths[index as int],
        })
    }
}

}
