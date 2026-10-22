use vstd::prelude::*;

verus! {

pub struct Part1PacketReaderState {
    pub input: Seq<u8>,
    pub byte_index: usize,
    pub current_byte: u8,
    pub bits_remaining: u8,
    pub previous_byte_was_ff: bool,
    pub bits_read: u64,
    pub maximum_work_units: u64,
    pub maximum_header_bytes: u64,
}

pub open spec fn part1_packet_reader_error(offset: usize, kind: crate::MalformedKind) -> crate::DecodeError {
    crate::DecodeError::MalformedInput { offset: offset as u64, kind }
}

pub open spec fn part1_packet_reader_limit(resource: crate::Resource, limit: u64, actual: u64) -> crate::DecodeError {
    crate::DecodeError::LimitExceeded { resource, limit, actual }
}

pub open spec fn part1_packet_reader_prefix(input: Seq<u8>, header_limit: u64, work_limit: u64) -> Part1PacketReaderState {
    Part1PacketReaderState {
        input, byte_index: 0, current_byte: 0, bits_remaining: 0, previous_byte_was_ff: false,
        bits_read: 0, maximum_work_units: work_limit, maximum_header_bytes: header_limit,
    }
}

pub open spec fn part1_packet_reader_initial(input: Seq<u8>, header_limit: u64, work_limit: u64)
    -> Result<Part1PacketReaderState, crate::DecodeError>
{
    if input.len() > header_limit {
        Err(part1_packet_reader_limit(crate::Resource::PacketHeaderBytes, header_limit, input.len() as u64))
    } else { Ok(part1_packet_reader_prefix(input, header_limit, work_limit)) }
}

pub open spec fn part1_packet_reader_load(s: Part1PacketReaderState)
    -> (Result<(), crate::DecodeError>, Part1PacketReaderState)
{
    if s.byte_index as u64 >= s.maximum_header_bytes {
        if s.byte_index as u64 == u64::MAX {
            (Err(part1_packet_reader_error(s.byte_index, crate::MalformedKind::ArithmeticOverflow)), s)
        } else {
            (Err(part1_packet_reader_limit(crate::Resource::PacketHeaderBytes, s.maximum_header_bytes,
                (s.byte_index as u64 + 1) as u64)), s)
        }
    } else if s.byte_index >= s.input.len() {
        (Err(part1_packet_reader_error(s.byte_index, crate::MalformedKind::Truncated)), s)
    } else {
        let byte = s.input[s.byte_index as int];
        if s.previous_byte_was_ff && byte & 0x80u8 != 0 {
            (Err(part1_packet_reader_error(s.byte_index, crate::MalformedKind::InvalidBitStuffing)), s)
        } else if s.byte_index == usize::MAX {
            (Err(part1_packet_reader_error(s.byte_index, crate::MalformedKind::ArithmeticOverflow)), s)
        } else {
            (Ok(()), Part1PacketReaderState {
                byte_index: (s.byte_index + 1) as usize, current_byte: byte,
                bits_remaining: if s.previous_byte_was_ff { 7u8 } else { 8u8 },
                previous_byte_was_ff: byte == 0xff, ..s
            })
        }
    }
}

#[verifier::opaque]
pub open spec fn part1_packet_reader_read(s: Part1PacketReaderState)
    -> (Result<bool, crate::DecodeError>, Part1PacketReaderState)
{
    let loaded = if s.bits_remaining == 0 { part1_packet_reader_load(s) } else { (Ok(()), s) };
    match loaded.0 {
        Err(error) => (Err(error), loaded.1),
        Ok(()) => {
            let s = loaded.1;
            if s.bits_remaining == 0 || s.bits_remaining > 8 {
                (Err(part1_packet_reader_error(s.byte_index, crate::MalformedKind::InvalidBitStuffing)), s)
            } else {
                let next = Part1PacketReaderState { bits_remaining: (s.bits_remaining - 1) as u8, ..s };
                if next.bits_read == u64::MAX {
                    (Err(part1_packet_reader_error(next.byte_index, crate::MalformedKind::ArithmeticOverflow)), next)
                } else {
                    let next = Part1PacketReaderState { bits_read: (next.bits_read + 1) as u64, ..next };
                    if next.bits_read > next.maximum_work_units {
                        (Err(part1_packet_reader_limit(crate::Resource::WorkUnits, next.maximum_work_units, next.bits_read)), next)
                    } else { (Ok(super::part1_packet_bit(next.current_byte, next.bits_remaining)), next) }
                }
            }
        },
    }
}

pub open spec fn part1_packet_reader_word(s: Part1PacketReaderState, remaining: nat, value: u32)
    -> (Result<u32, crate::DecodeError>, Part1PacketReaderState)
    decreases remaining,
{
    if remaining == 0 { (Ok(value), s) } else {
        let bit = part1_packet_reader_read(s);
        match bit.0 {
            Err(error) => (Err(error), bit.1),
            Ok(bit_value) => part1_packet_reader_word(bit.1, (remaining - 1) as nat,
                (value << 1) | if bit_value { 1u32 } else { 0u32 }),
        }
    }
}

pub open spec fn part1_packet_reader_read_bits(s: Part1PacketReaderState, count: u8)
    -> (Result<u32, crate::DecodeError>, Part1PacketReaderState)
{
    if count > 32 { (Err(part1_packet_reader_error(s.byte_index, crate::MalformedKind::InvalidMarkerLength)), s) }
    else { part1_packet_reader_word(s, count as nat, 0) }
}

pub struct Part1CodeBlockPacketState {
    pub included: bool,
    pub zero_bitplanes: Option<u16>,
    pub lblock: u8,
    pub total_passes: u32,
    pub high_throughput: Option<bool>,
}

pub open spec fn part1_code_block_packet_initial() -> Part1CodeBlockPacketState {
    Part1CodeBlockPacketState {
        included: false, zero_bitplanes: None, lblock: 3, total_passes: 0, high_throughput: None,
    }
}

}
