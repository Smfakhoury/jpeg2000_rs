use super::{Tier1DecoderState, ht_reader_error, part1_legacy_zero_bitplanes};
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

pub open spec fn part1_code_block_body_prepare(
    decoder: Tier1DecoderState, contribution: Part1CodeBlockContribution, body: Seq<u8>,
) -> (Result<(), crate::DecodeError>, Tier1DecoderState) {
    if contribution.data_length as nat > usize::MAX {
        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), decoder)
    } else if body.len() != contribution.data_length as nat {
        (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), decoder)
    } else if decoder.style.force_high_throughput && !contribution.high_throughput {
        (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), decoder)
    } else {
        part1_legacy_zero_bitplanes(decoder, contribution.zero_bitplanes)
    }
}

}
