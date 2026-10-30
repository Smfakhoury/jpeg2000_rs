#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (crate::specs::part1_packet_data_bits(previous_was_ff) as u8),
    )
)]
pub(crate) const fn packet_data_bits(previous_was_ff: bool) -> u8 {
    if previous_was_ff { 7 } else { 8 }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires remaining < 8,
        ensures returns crate::specs::part1_packet_bit(byte, remaining),
    )
)]
pub(crate) const fn packet_bit(byte: u8, remaining: u8) -> bool {
    ((byte >> remaining) & 1) != 0
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns (
            crate::specs::part1_tag_inherited_low(low as nat, inherited as nat) as u32
        ),
    )
)]
pub(crate) const fn tag_inherited_low(low: u32, inherited: u32) -> u32 {
    if low < inherited { inherited } else { low }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires !bit ==> low < u32::MAX,
        ensures returns (crate::specs::part1_tag_step_low(low as nat, bit) as u32),
    )
)]
pub(crate) const fn tag_step_low(low: u32, bit: bool) -> u32 {
    if bit { low } else { low + 1 }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires low <= value,
        ensures returns (
            crate::specs::part1_tag_step_value(low as nat, value as nat, bit) as u32
        ),
    )
)]
pub(crate) const fn tag_step_value(low: u32, value: u32, bit: bool) -> u32 {
    if bit { low } else { value }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires dimension > 0,
        ensures returns (
            crate::specs::part1_progression_digit(index as nat, dimension as nat) as u64
        ),
    )
)]
pub(crate) const fn progression_digit(index: u64, dimension: u64) -> u64 {
    index % dimension
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires dimension > 0,
        ensures returns (
            crate::specs::part1_progression_advance(index as nat, dimension as nat) as u64
        ),
    )
)]
pub(crate) const fn progression_advance(index: u64, dimension: u64) -> u64 {
    index / dimension
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_progression_emit_allowed(already_seen),
    )
)]
pub(crate) const fn progression_emit_allowed(already_seen: bool) -> bool {
    !already_seen
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_packet_next_included(old, contribution_present),
    )
)]
pub(crate) const fn packet_next_included(old: bool, contribution_present: bool) -> bool {
    old || contribution_present
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires lblock < 31,
        ensures returns crate::specs::part1_packet_next_lblock(lblock),
    )
)]
pub(crate) const fn packet_next_lblock(lblock: u8) -> u8 {
    lblock + 1
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires old <= u32::MAX - added,
        ensures returns crate::specs::part1_packet_next_passes(old, added),
    )
)]
pub(crate) const fn packet_next_passes(old: u32, added: u32) -> u32 {
    old + added
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        requires tile <= usize::MAX - packed,
        ensures returns crate::specs::part1_packet_logical_bytes(tile, packed),
    )
)]
pub(crate) const fn packet_logical_bytes(tile: usize, packed: usize) -> usize {
    tile + packed
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_sop_sequence(high, low),)
)]
pub(crate) const fn sop_sequence(high: u8, low: u8) -> u16 {
    ((high as u16) << 8) | (low as u16)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_sop_valid(marker0, marker1, length0, length1),
    )
)]
pub(crate) const fn sop_valid(marker0: u8, marker1: u8, length0: u8, length1: u8) -> bool {
    marker0 == 0xff && marker1 == 0x91 && length0 == 0 && length1 == 4
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(ensures returns crate::specs::part1_eph_valid(marker0, marker1),)
)]
pub(crate) const fn eph_valid(marker0: u8, marker1: u8) -> bool {
    marker0 == 0xff && marker1 == 0x92
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(
        ensures returns crate::specs::part1_packet_length_matches(actual, expected),
    )
)]
pub(crate) const fn packet_length_matches(actual: usize, expected: usize) -> bool {
    actual == expected
}
