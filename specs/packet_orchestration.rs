use super::{Part1CodeBlockContribution, Tier1DecoderState, ht_reader_error, part1_code_block_body_transaction};
use vstd::prelude::*;

verus! {

pub struct Part1PacketBodyPreflight {
    pub index: usize,
    pub end: usize,
}

pub type Part1PacketBodyPreflightResult = (Result<(), crate::DecodeError>, Part1PacketBodyPreflight);

#[verifier::opaque]
pub open spec fn part1_packet_body_preflight(
    state: Part1PacketBodyPreflight, contributions: Seq<Option<Part1CodeBlockContribution>>,
    data: Seq<u8>, error_offset: usize,
) -> Part1PacketBodyPreflightResult
    decreases contributions.len() - state.index as nat,
{
    if state.index >= contributions.len() { (Ok(()), state) } else {
        let checked = match contributions[state.index as int] {
            None => (Ok(()), state),
            Some(contribution) => if contribution.data_length as nat > usize::MAX {
                (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
            } else if state.end as nat + contribution.data_length as nat > usize::MAX {
                (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
            } else {
                let state = Part1PacketBodyPreflight {
                    end: (state.end + contribution.data_length) as usize, ..state
                };
                if state.end > data.len() {
                    (Err(crate::DecodeError::MalformedInput {
                        offset: error_offset as u64, kind: crate::MalformedKind::Truncated,
                    }), state)
                } else { (Ok(()), state) }
            },
        };
        match checked.0 {
            Err(_) => checked,
            Ok(()) => if checked.1.index == usize::MAX {
                (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), checked.1)
            } else {
                part1_packet_body_preflight(Part1PacketBodyPreflight {
                    index: (checked.1.index + 1) as usize, ..checked.1
                }, contributions, data, error_offset)
            },
        }
    }
}

pub struct Part1PacketBodyExecution {
    pub decoders: Seq<Tier1DecoderState>,
    pub index: usize,
    pub offset: usize,
    pub segments: u32,
    pub passes: u32,
}

pub type Part1PacketBodyExecutionResult = (Result<(), crate::DecodeError>, Part1PacketBodyExecution);

pub open spec fn part1_packet_body_execute_slot(
    state: Part1PacketBodyExecution, contribution: Part1CodeBlockContribution, data: Seq<u8>,
    error_offset: usize, decode_tier1: bool,
    pass_limit: u32, byte_limit: u64, input_limit: u64, work_limit: u64,
) -> Part1PacketBodyExecutionResult {
    if contribution.data_length as nat > usize::MAX
        || state.offset as nat + contribution.data_length as nat > usize::MAX {
        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
    } else {
        let end = (state.offset + contribution.data_length) as usize;
        if state.offset > end || end > data.len() {
            (Err(crate::DecodeError::MalformedInput {
                offset: error_offset as u64, kind: crate::MalformedKind::Truncated,
            }), state)
        } else {
            let decoded = if !decode_tier1 { (Ok(()), state) }
            else if state.index >= state.decoders.len() {
                (Err(ht_reader_error(crate::MalformedKind::InvalidPacketHeader)), state)
            } else {
                let result = part1_code_block_body_transaction(state.decoders[state.index as int],
                    contribution, data.subrange(state.offset as int, end as int),
                    pass_limit, byte_limit, input_limit, work_limit);
                let state = Part1PacketBodyExecution {
                    decoders: state.decoders.update(state.index as int, result.1), ..state
                };
                match result.0 {
                    Err(error) => (Err(error), state),
                    Ok(value) => if state.segments as nat + value.segments as nat > u32::MAX {
                        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
                    } else {
                        let state = Part1PacketBodyExecution {
                            segments: (state.segments + value.segments) as u32, ..state
                        };
                        if state.passes as nat + value.passes as nat > u32::MAX {
                            (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
                        } else {
                            (Ok(()), Part1PacketBodyExecution {
                                passes: (state.passes + value.passes) as u32, ..state
                            })
                        }
                    },
                }
            };
            match decoded.0 {
                Err(_) => decoded,
                Ok(()) => (Ok(()), Part1PacketBodyExecution { offset: end, ..decoded.1 }),
            }
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_packet_body_execute(
    state: Part1PacketBodyExecution, contributions: Seq<Option<Part1CodeBlockContribution>>,
    data: Seq<u8>, error_offset: usize, decode_tier1: bool,
    pass_limit: u32, byte_limit: u64, input_limit: u64, work_limit: u64,
) -> Part1PacketBodyExecutionResult
    decreases contributions.len() - state.index as nat,
{
    if state.index >= contributions.len() { (Ok(()), state) } else {
        let step = match contributions[state.index as int] {
            None => (Ok(()), state),
            Some(contribution) => part1_packet_body_execute_slot(state, contribution, data,
                error_offset, decode_tier1, pass_limit, byte_limit, input_limit, work_limit),
        };
        match step.0 {
            Err(_) => step,
            Ok(()) => if step.1.index == usize::MAX {
                (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), step.1)
            } else {
                part1_packet_body_execute(Part1PacketBodyExecution {
                    index: (step.1.index + 1) as usize, ..step.1
                }, contributions, data, error_offset, decode_tier1,
                    pass_limit, byte_limit, input_limit, work_limit)
            },
        }
    }
}

}
