use super::{DecodeError, MalformedKind, Resource};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_packet_reader_error(offset, kind),
))]
pub(crate) fn packet_reader_error(offset: usize, kind: MalformedKind) -> DecodeError {
    DecodeError::MalformedInput {
        offset: offset as u64,
        kind,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_packet_reader_limit(resource, limit, actual),
))]
pub(super) fn packet_reader_limit(resource: Resource, limit: u64, actual: u64) -> DecodeError {
    DecodeError::LimitExceeded {
        resource,
        limit,
        actual,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_packet_header_error(),
))]
pub(super) fn invalid_packet_header() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidPacketHeader,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::ht_reader_error(MalformedKind::ArithmeticOverflow),
))]
pub(super) fn packet_arithmetic_overflow() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::ht_reader_error(MalformedKind::InvalidCodeBlock),
))]
pub(super) fn packet_invalid_code_block() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidCodeBlock,
    }
}
