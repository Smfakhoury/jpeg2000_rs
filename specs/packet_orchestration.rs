use super::{
    Part1CodeBlockContribution, Tier1DecoderState, ht_reader_error,
    part1_code_block_body_transaction,
};
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

pub struct Part1PacketSubbandState {
    pub orientation: crate::tier1::SubbandOrientation,
    pub first_decoder: usize,
    pub decoder_count: usize,
    pub code_blocks_across: u32,
    pub code_blocks_down: u32,
    pub inclusion_tree: super::Part1TagTree,
    pub zero_bitplane_tree: super::Part1TagTree,
}

pub struct Part1PrecinctHeaderState {
    pub states: Seq<super::Part1CodeBlockPacketState>,
    pub subbands: Seq<Part1PacketSubbandState>,
    pub contributions: Seq<Option<Part1CodeBlockContribution>>,
    pub reader: super::Part1PacketReaderState,
    pub subband_index: usize,
    pub local_index: usize,
    pub contribution_count: u32,
}

pub type Part1PrecinctHeaderResult = (Result<(), crate::DecodeError>, Part1PrecinctHeaderState);

pub open spec fn part1_packet_styles_equal(
    left: crate::tier1::Tier1CodingStyle, right: crate::tier1::Tier1CodingStyle,
) -> bool {
    left.block_coding == right.block_coding
        && left.selective_arithmetic_coding_bypass == right.selective_arithmetic_coding_bypass
        && left.reset_contexts == right.reset_contexts
        && left.terminate_all_passes == right.terminate_all_passes
        && left.vertical_causal_context == right.vertical_causal_context
        && left.predictable_termination == right.predictable_termination
        && left.segmentation_symbols == right.segmentation_symbols
}

#[verifier::opaque]
pub open spec fn part1_precinct_header_slot(
    state: Part1PrecinctHeaderState, contexts: Seq<crate::packet::CodeBlockPacketContext>,
    layer: u16, style: crate::tier1::Tier1CodingStyle,
) -> Part1PrecinctHeaderResult {
    if state.subband_index >= state.subbands.len() {
        (Err(ht_reader_error(crate::MalformedKind::InvalidPacketHeader)), state)
    } else {
    let subband = state.subbands[state.subband_index as int];
    if subband.first_decoder as nat + state.local_index as nat > usize::MAX {
        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
    } else {
        let index = (subband.first_decoder + state.local_index) as usize;
        if index >= contexts.len() { (Err(ht_reader_error(crate::MalformedKind::InvalidPacketHeader)), state) }
        else if state.local_index > u32::MAX || subband.code_blocks_across == 0 {
            (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
        } else {
            let context = contexts[index as int];
            if context.layer != layer || context.leaf_x != (state.local_index as u32) % subband.code_blocks_across
                || context.leaf_y != (state.local_index as u32) / subband.code_blocks_across
                || !part1_packet_styles_equal(context.tier1_style, style) {
                (Err(ht_reader_error(crate::MalformedKind::InvalidPacketHeader)), state)
            } else if index >= state.states.len() {
                (Err(ht_reader_error(crate::MalformedKind::InvalidPacketHeader)), state)
            } else {
                let result = super::part1_packet_header_decode(state.states[index as int],
                    subband.inclusion_tree, subband.zero_bitplane_tree, context, state.reader);
                let subband = Part1PacketSubbandState {
                    inclusion_tree: result.2, zero_bitplane_tree: result.3, ..subband
                };
                let state = Part1PrecinctHeaderState {
                    states: state.states.update(index as int, result.1),
                    subbands: state.subbands.update(state.subband_index as int, subband),
                    reader: result.4, ..state
                };
                match result.0 {
                    Err(error) => (Err(error), state),
                    Ok(contribution) => if contribution.is_some() && state.contribution_count == u32::MAX {
                        (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
                    } else {
                        let state = Part1PrecinctHeaderState {
                            contribution_count: if contribution.is_some() {
                                (state.contribution_count + 1) as u32
                            } else { state.contribution_count }, ..state
                        };
                        if index >= state.contributions.len() {
                            (Err(ht_reader_error(crate::MalformedKind::InvalidPacketHeader)), state)
                        } else {
                            let state = Part1PrecinctHeaderState {
                                contributions: state.contributions.update(index as int, contribution), ..state
                            };
                            if state.local_index == usize::MAX {
                                (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
                            } else {
                                (Ok(()), Part1PrecinctHeaderState { local_index: (state.local_index + 1) as usize, ..state })
                            }
                        }
                    },
                }
            }
            }
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_precinct_header_members_run(
    state: Part1PrecinctHeaderState, contexts: Seq<crate::packet::CodeBlockPacketContext>,
    layer: u16, style: crate::tier1::Tier1CodingStyle, remaining: nat,
) -> Part1PrecinctHeaderResult
    decreases remaining,
{
    if remaining == 0 { (Ok(()), state) } else {
        let step = part1_precinct_header_slot(state, contexts, layer, style);
        match step.0 {
            Err(_) => step,
            Ok(()) => part1_precinct_header_members_run(step.1, contexts, layer, style, (remaining - 1) as nat),
        }
    }
}

pub open spec fn part1_precinct_header_members(
    state: Part1PrecinctHeaderState, contexts: Seq<crate::packet::CodeBlockPacketContext>,
    layer: u16, style: crate::tier1::Tier1CodingStyle, count: usize,
) -> Part1PrecinctHeaderResult {
    part1_precinct_header_members_run(state, contexts, layer, style,
        if state.local_index >= count { 0 } else { (count - state.local_index) as nat })
}

#[verifier::opaque]
pub open spec fn part1_precinct_headers_run(
    state: Part1PrecinctHeaderState, contexts: Seq<crate::packet::CodeBlockPacketContext>,
    layer: u16, style: crate::tier1::Tier1CodingStyle, remaining: nat,
) -> Part1PrecinctHeaderResult
    decreases remaining,
{
    if remaining == 0 { (Ok(()), state) }
    else if state.subband_index >= state.subbands.len() {
        (Err(ht_reader_error(crate::MalformedKind::InvalidPacketHeader)), state)
    } else {
        let subband = state.subbands[state.subband_index as int];
        let count = subband.code_blocks_across as nat * subband.code_blocks_down as nat;
        if count > u64::MAX || count > usize::MAX {
            (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), state)
        } else if count != subband.decoder_count {
            (Err(ht_reader_error(crate::MalformedKind::InvalidPacketHeader)), state)
        } else {
            let inclusion = super::part1_tag_leaf_count(subband.inclusion_tree);
            match inclusion {
                Err(error) => (Err(error), state),
                Ok(value) => if value != count {
                    (Err(ht_reader_error(crate::MalformedKind::InvalidPacketHeader)), state)
                } else {
                    let zero = super::part1_tag_leaf_count(subband.zero_bitplane_tree);
                    match zero {
                        Err(error) => (Err(error), state),
                        Ok(value) => if value != count {
                            (Err(ht_reader_error(crate::MalformedKind::InvalidPacketHeader)), state)
                        } else {
                            let members = part1_precinct_header_members(
                                Part1PrecinctHeaderState { local_index: 0, ..state }, contexts, layer, style, count as usize);
                            match members.0 {
                                Err(_) => members,
                                Ok(()) => if members.1.subband_index == usize::MAX {
                                    (Err(ht_reader_error(crate::MalformedKind::ArithmeticOverflow)), members.1)
                                } else {
                                    part1_precinct_headers_run(Part1PrecinctHeaderState {
                                        subband_index: (members.1.subband_index + 1) as usize, ..members.1
                                    }, contexts, layer, style, (remaining - 1) as nat)
                                },
                            }
                        },
                    }
                },
            }
        }
    }
}

pub open spec fn part1_precinct_headers(
    state: Part1PrecinctHeaderState, contexts: Seq<crate::packet::CodeBlockPacketContext>,
    layer: u16, style: crate::tier1::Tier1CodingStyle,
) -> Part1PrecinctHeaderResult {
    part1_precinct_headers_run(state, contexts, layer, style,
        if state.subband_index >= state.subbands.len() { 0 }
        else { (state.subbands.len() - state.subband_index as nat) as nat })
}

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
