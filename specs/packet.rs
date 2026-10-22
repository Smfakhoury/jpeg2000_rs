use super::{
    Tier1DecoderState, ht_reader_error, part1_legacy_contribution, part1_legacy_zero_bitplanes,
};
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

pub struct Part1LegacyPacketProgress {
    pub decoder: Tier1DecoderState,
    pub segment_index: u8,
    pub offset: usize,
    pub decoded_passes: u32,
}

pub type Part1LegacyPacketEffect = (Result<(), crate::DecodeError>, Part1LegacyPacketProgress);

#[verifier::opaque]
pub open spec fn part1_legacy_code_block_segment(
    s: Part1LegacyPacketProgress, contribution: Part1CodeBlockContribution, body: Seq<u8>,
    pass_limit: u32, byte_limit: u64, input_limit: u64, work_limit: u64,
) -> Part1LegacyPacketEffect {
    match part1_code_block_segment(contribution, s.segment_index) {
        Err(error) => (Err(error), s),
        Ok(segment) => if segment.data_length as nat > usize::MAX {
            (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s)
        } else {
            let end = s.offset as nat + segment.data_length as nat;
            if end > usize::MAX { (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s) }
            else if end > body.len() { (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), s) }
            else {
                let decoded = part1_legacy_contribution(s.decoder, body.subrange(s.offset as int, end as int), segment.passes,
                    pass_limit, byte_limit, input_limit, work_limit);
                let s = Part1LegacyPacketProgress { decoder: decoded.1, ..s };
                match decoded.0 {
                    Err(error) => (Err(error), s),
                    Ok(()) => if s.decoded_passes as nat + segment.passes as nat > u32::MAX {
                        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s)
                    } else {
                        let s = Part1LegacyPacketProgress {
                            decoded_passes: (s.decoded_passes + segment.passes) as u32,
                            offset: end as usize, ..s
                        };
                        if s.segment_index == u8::MAX {
                            (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), s)
                        } else { (Ok(()), Part1LegacyPacketProgress { segment_index: (s.segment_index + 1) as u8, ..s }) }
                    },
                }
            }
        },
    }
}

pub open spec fn part1_legacy_code_block_run(
    s: Part1LegacyPacketProgress, contribution: Part1CodeBlockContribution, body: Seq<u8>,
    remaining: nat, pass_limit: u32, byte_limit: u64, input_limit: u64, work_limit: u64,
) -> Part1LegacyPacketEffect
    decreases remaining,
{
    if remaining == 0 { (Ok(()), s) } else {
        let step = part1_legacy_code_block_segment(s, contribution, body, pass_limit, byte_limit, input_limit, work_limit);
        match step.0 {
            Err(_) => step,
            Ok(()) => part1_legacy_code_block_run(step.1, contribution, body, (remaining - 1) as nat,
                pass_limit, byte_limit, input_limit, work_limit),
        }
    }
}

pub open spec fn part1_legacy_code_block_finish(
    run: Part1LegacyPacketEffect, contribution: Part1CodeBlockContribution, body: Seq<u8>,
) -> (Result<crate::packet::CodeBlockBodyDecode, crate::DecodeError>, Tier1DecoderState) {
    let s = run.1;
    match run.0 {
        Err(error) => (Err(error), s.decoder),
        Ok(()) => if s.offset != body.len() || s.decoded_passes != contribution.new_passes as u32 {
            (Err(ht_reader_error(crate::MalformedKind::InvalidCodeBlock)), s.decoder)
        } else { (Ok(crate::packet::CodeBlockBodyDecode {
            segments: contribution.segment_count, passes: s.decoded_passes, bytes: contribution.data_length,
        }), s.decoder) },
    }
}

#[verifier::opaque]
pub open spec fn part1_legacy_code_block_body(
    decoder: Tier1DecoderState, contribution: Part1CodeBlockContribution, body: Seq<u8>,
    pass_limit: u32, byte_limit: u64, input_limit: u64, work_limit: u64,
) -> (Result<crate::packet::CodeBlockBodyDecode, crate::DecodeError>, Tier1DecoderState) {
    part1_legacy_code_block_finish(part1_legacy_code_block_run(
        Part1LegacyPacketProgress { decoder, segment_index: 0, offset: 0, decoded_passes: 0 },
        contribution, body, contribution.segment_count as nat, pass_limit, byte_limit, input_limit, work_limit),
        contribution, body)
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

#[verifier::opaque]
pub open spec fn part1_legacy_code_block_staged(
    decoder: Tier1DecoderState, contribution: Part1CodeBlockContribution, body: Seq<u8>,
    pass_limit: u32, byte_limit: u64, input_limit: u64, work_limit: u64,
) -> (Result<crate::packet::CodeBlockBodyDecode, crate::DecodeError>, Tier1DecoderState) {
    let prepared = part1_code_block_body_prepare(decoder, contribution, body);
    match prepared.0 {
        Err(error) => (Err(error), prepared.1),
        Ok(()) => part1_legacy_code_block_body(prepared.1, contribution, body,
            pass_limit, byte_limit, input_limit, work_limit),
    }
}

pub open spec fn part1_legacy_code_block_transaction(
    decoder: Tier1DecoderState, contribution: Part1CodeBlockContribution, body: Seq<u8>,
    pass_limit: u32, byte_limit: u64, input_limit: u64, work_limit: u64,
) -> (Result<crate::packet::CodeBlockBodyDecode, crate::DecodeError>, Tier1DecoderState) {
    let staged = part1_legacy_code_block_staged(decoder, contribution, body, pass_limit, byte_limit, input_limit, work_limit);
    match staged.0 {
        Err(error) => (Err(error), decoder),
        Ok(_) => staged,
    }
}

}
