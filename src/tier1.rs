use crate::{
    DecodeError, MalformedKind, Resource, ResourceLimits,
    magnitude::Magnitude,
    mq::{MqContextState, MqDecoder},
    transform::SignedMagnitude,
};

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[path = "tier1_ht_tables.rs"]
mod ht_vlc_tables;

use ht_vlc_tables::{HT_VLC_INITIAL, HT_VLC_NON_INITIAL};

const RUN_LENGTH_CONTEXT: usize = 17;
const UNIFORM_CONTEXT: usize = 18;
pub const MAX_TIER1_BITPLANES: u16 = 292;
pub const MAX_TIER1_PASSES: u32 = 874;
const MAX_HT_SCUP: usize = 4_079;
const MAX_HT_CLEANUP_BYTES: usize = 65_534;
const MAX_HT_REFINEMENT_BYTES: usize = 2_046;
const MAX_HT_MAGNITUDE_BITS: u8 = 74;
const MAX_HT_MAGSGN_BITS: u8 = MAX_HT_MAGNITUDE_BITS + 1;

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct RawBitReader<'a> {
    input: &'a [u8],
    next_index: usize,
    current_byte: u8,
    bits_remaining: u8,
    marker_padding_count: u64,
    work_units: u64,
    maximum_work_units: u64,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn raw_bit_reader_view(reader: &RawBitReader<'_>) -> crate::specs::Part1RawReaderState {
        crate::specs::Part1RawReaderState {
            input: reader.input@,
            next_index: reader.next_index,
            current_byte: reader.current_byte,
            bits_remaining: reader.bits_remaining,
            marker_padding_count: reader.marker_padding_count,
            work_units: reader.work_units,
            maximum_work_units: reader.maximum_work_units,
        }
    }

    #[verifier::inline]
    spec fn raw_reader_outcome(result: Result<RawBitReader<'_>, DecodeError>)
        -> Result<crate::specs::Part1RawReaderState, DecodeError>
    {
        match result {
            Ok(reader) => Ok(raw_bit_reader_view(&reader)),
            Err(error) => Err(error),
        }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures raw_reader_outcome(result) == crate::specs::part1_raw_reader_initial(
            input@, maximum_input_bytes, maximum_work_units,
        ),
    )
)]
fn raw_reader_from_input(
    input: &[u8],
    maximum_input_bytes: u64,
    maximum_work_units: u64,
) -> Result<RawBitReader<'_>, DecodeError> {
    let input_length = input.len() as u64;
    if input_length > maximum_input_bytes {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::InputBytes,
            limit: maximum_input_bytes,
            actual: input_length,
        });
    }
    Ok(RawBitReader {
        input,
        next_index: 0,
        current_byte: 0,
        bits_remaining: 0,
        marker_padding_count: 0,
        work_units: 0,
        maximum_work_units,
    })
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(error =>
        ensures error == crate::specs::part1_raw_reader_error(offset, kind),
    )
)]
fn raw_reader_error(offset: usize, kind: MalformedKind) -> DecodeError {
    DecodeError::MalformedInput {
        offset: offset as u64,
        kind,
    }
}

impl<'a> RawBitReader<'a> {
    pub fn new(input: &'a [u8], limits: &ResourceLimits) -> Result<Self, DecodeError> {
        raw_reader_from_input(
            input,
            limits.maximum_input_bytes(),
            limits.maximum_work_units(),
        )
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, raw_bit_reader_view(final(self)))
                == crate::specs::part1_raw_reader_read(raw_bit_reader_view(old(self))),
        )
    )]
    pub fn read_bit(&mut self) -> Result<bool, DecodeError> {
        if self.bits_remaining == 0 {
            self.load_byte()?;
        }
        if self.bits_remaining == 0 || self.bits_remaining > 8 {
            return Err(raw_reader_error(
                self.next_index,
                MalformedKind::InvalidBitStuffing,
            ));
        }
        self.bits_remaining = self
            .bits_remaining
            .checked_sub(1)
            .ok_or_else(|| raw_reader_error(self.next_index, MalformedKind::ArithmeticOverflow))?;
        let bit = (self.current_byte >> self.bits_remaining) & 1;
        self.work_units = match self.work_units.checked_add(1) {
            Some(work_units) => work_units,
            None => {
                return Err(raw_reader_error(
                    self.next_index,
                    MalformedKind::ArithmeticOverflow,
                ));
            }
        };
        if self.work_units > self.maximum_work_units {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: self.maximum_work_units,
                actual: self.work_units,
            });
        }
        Ok(bit != 0)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == raw_bit_reader_view(self).next_index,)
    )]
    pub const fn bytes_consumed(&self) -> usize {
        self.next_index
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(value => ensures value == raw_bit_reader_view(self).marker_padding_count,)
    )]
    pub const fn marker_padding_count(&self) -> u64 {
        self.marker_padding_count
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures result == crate::specs::part1_raw_reader_termination(raw_bit_reader_view(self)),
        )
    )]
    pub fn validate_predictable_termination(&self) -> Result<(), DecodeError> {
        if self.next_index == 0 {
            // A pass with no coded symbols need not emit a padding byte.
            return if (self.input.is_empty() && self.work_units == 0)
                || (self.input.len() == 1 && self.input[0] == 0x55)
            {
                Ok(())
            } else {
                Err(raw_reader_error(0, MalformedKind::InvalidCodeBlock))
            };
        }
        if self.bits_remaining == 0
            && self.current_byte == 0xff
            && self.next_index < self.input.len()
            && self.next_index + 1 == self.input.len()
            && self.input[self.next_index] == 0x2a
        {
            // ERTERM appends seven padding bits after a complete final 0xff.
            return Ok(());
        }
        if self.next_index != self.input.len() || self.bits_remaining > 8 {
            return Err(raw_reader_error(0, MalformedKind::InvalidCodeBlock));
        }
        if self.bits_remaining == 0 {
            return Ok(());
        }
        let unread_mask = u8::MAX >> (8 - self.bits_remaining);
        let expected = 0x55_u8 >> (8 - self.bits_remaining);
        if self.current_byte & unread_mask != expected {
            return Err(raw_reader_error(0, MalformedKind::InvalidCodeBlock));
        }
        Ok(())
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, raw_bit_reader_view(final(self)))
                == crate::specs::part1_raw_reader_load(raw_bit_reader_view(old(self))),
        )
    )]
    fn load_byte(&mut self) -> Result<(), DecodeError> {
        let next = if self.next_index < self.input.len() {
            self.input[self.next_index]
        } else {
            0xff
        };
        let stuffed = self.current_byte == 0xff;
        if stuffed && next > 0x8f {
            self.current_byte = 0xff;
            self.bits_remaining = 8;
            self.marker_padding_count = match self.marker_padding_count.checked_add(1) {
                Some(count) => count,
                None => {
                    return Err(raw_reader_error(
                        self.next_index,
                        MalformedKind::ArithmeticOverflow,
                    ));
                }
            };
            return Ok(());
        }
        if self.next_index < self.input.len() {
            self.next_index = self.next_index.checked_add(1).ok_or_else(|| {
                raw_reader_error(self.next_index, MalformedKind::ArithmeticOverflow)
            })?;
        }
        self.current_byte = next;
        self.bits_remaining = if stuffed { 7 } else { 8 };
        Ok(())
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum Tier1PassKind {
    SignificancePropagation,
    MagnitudeRefinement,
    Cleanup,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct HtPassGroup {
    pub cleanup_passes: u8,
    pub refinement_passes: u8,
    pub placeholder_passes: u8,
    pub skipped_bitplanes: u8,
}

impl HtPassGroup {
    pub fn from_packet_passes(passes: u8) -> Result<Self, DecodeError> {
        ht_pass_group_from_packet(passes)
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result => ensures result == crate::specs::ht_packet_pass_group(passes),)
)]
fn ht_pass_group_from_packet(passes: u8) -> Result<HtPassGroup, DecodeError> {
    if passes == 0 || passes > 164 {
        return Err(invalid_code_block());
    }
    let placeholder_passes = crate::refinement::ht_placeholder_passes(passes);
    let skipped_bitplanes = placeholder_passes / 3;
    let substantive_passes = crate::refinement::ht_substantive_passes(passes);
    Ok(HtPassGroup {
        cleanup_passes: placeholder_passes
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?,
        refinement_passes: substantive_passes
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?,
        placeholder_passes,
        skipped_bitplanes,
    })
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum HtStreamDirection {
    Forward,
    Reverse,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct HtSubstreamView<'a> {
    bytes: &'a [u8],
    offset: u32,
    direction: HtStreamDirection,
}

impl<'a> HtSubstreamView<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result@ == ht_substream_view(self).bytes,))]
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == ht_substream_view(self).offset,))]
    pub const fn offset(&self) -> u32 {
        self.offset
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_direction_forward(result) == ht_substream_view(self).forward,))]
    pub const fn direction(&self) -> HtStreamDirection {
        self.direction
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct HtCleanupSegment<'a> {
    encoded: &'a [u8],
    scup: u16,
    mag_sgn: HtSubstreamView<'a>,
    mel_vlc: HtSubstreamView<'a>,
    mel: HtSubstreamView<'a>,
    vlc: HtSubstreamView<'a>,
}

impl<'a> HtCleanupSegment<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result@ == ht_cleanup_segment_view(self).encoded,))]
    pub fn encoded(&self) -> &'a [u8] {
        self.encoded
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == ht_cleanup_segment_view(self).scup,))]
    pub const fn scup(&self) -> u16 {
        self.scup
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_substream_view(&result) == ht_cleanup_segment_view(self).mag_sgn,))]
    pub const fn mag_sgn(&self) -> HtSubstreamView<'a> {
        self.mag_sgn
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_substream_view(&result) == ht_cleanup_segment_view(self).mel_vlc,))]
    pub const fn mel_vlc(&self) -> HtSubstreamView<'a> {
        self.mel_vlc
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_substream_view(&result) == ht_cleanup_segment_view(self).mel,))]
    pub const fn mel(&self) -> HtSubstreamView<'a> {
        self.mel
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_substream_view(&result) == ht_cleanup_segment_view(self).vlc,))]
    pub const fn vlc(&self) -> HtSubstreamView<'a> {
        self.vlc
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct HtBitPosition {
    pub byte: u16,
    pub bit: u8,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct HtForwardMagSgnReader<'a> {
    input: &'a [u8],
    next_byte: usize,
    value: u8,
    next_bit: u8,
    last_bit: u8,
    has_byte: bool,
    current_is_physical: bool,
    previous_was_ff: bool,
    physical_bits_read: u64,
    fill_bits_read: u64,
    work_units: u64,
    maximum_work_units: u64,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn ht_magsgn_view(reader: &HtForwardMagSgnReader<'_>) -> crate::specs::HtMagSgnState {
        crate::specs::HtMagSgnState {
            input: reader.input@, next_byte: reader.next_byte, value: reader.value,
            next_bit: reader.next_bit, last_bit: reader.last_bit, has_byte: reader.has_byte,
            current_is_physical: reader.current_is_physical, previous_was_ff: reader.previous_was_ff,
            physical_bits_read: reader.physical_bits_read, fill_bits_read: reader.fill_bits_read,
            work_units: reader.work_units, maximum_work_units: reader.maximum_work_units,
        }
    }

    #[verifier::inline]
    spec fn ht_magsgn_outcome(result: Result<HtForwardMagSgnReader<'_>, DecodeError>)
        -> Result<crate::specs::HtMagSgnState, DecodeError>
    {
        match result { Ok(reader) => Ok(ht_magsgn_view(&reader)), Err(error) => Err(error) }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures ht_magsgn_outcome(result) == crate::specs::ht_magsgn_initial(input@, maximum_work_units),
    )
)]
fn ht_magsgn_from_input(
    input: &[u8],
    maximum_work_units: u64,
) -> Result<HtForwardMagSgnReader<'_>, DecodeError> {
    if input.len() > MAX_HT_CLEANUP_BYTES {
        return Err(invalid_code_block());
    }
    Ok(HtForwardMagSgnReader {
        input,
        next_byte: 0,
        value: 0,
        next_bit: 0,
        last_bit: 0,
        has_byte: false,
        current_is_physical: false,
        previous_was_ff: false,
        physical_bits_read: 0,
        fill_bits_read: 0,
        work_units: 0,
        maximum_work_units,
    })
}

impl<'a> HtForwardMagSgnReader<'a> {
    pub fn new(input: &'a [u8], limits: &ResourceLimits) -> Result<Self, DecodeError> {
        ht_magsgn_from_input(input, limits.maximum_work_units())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == ht_magsgn_view(self).physical_bits_read,))]
    pub const fn physical_bits_read(&self) -> u64 {
        self.physical_bits_read
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == ht_magsgn_view(self).fill_bits_read,))]
    pub const fn fill_bits_read(&self) -> u64 {
        self.fill_bits_read
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_magsgn_view(final(self))) == crate::specs::ht_magsgn_read_bits(ht_magsgn_view(old(self)), count),
        )
    )]
    pub fn read_bits(&mut self, count: u8) -> Result<u64, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost initial = ht_magsgn_view(self); }
        if count > 39 {
            return Err(invalid_code_block());
        }
        let mut value = 0_u64;
        let mut bit = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant bit <= count, count <= 39,
                    initial == ht_magsgn_view(old(self)),
                    crate::specs::ht_magsgn_read_bits(initial, count)
                        == crate::specs::ht_magsgn_word_run(ht_magsgn_view(self), (count - bit) as nat, bit as nat, value),
                decreases count - bit,
            )
        )]
        while bit < count {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_ht_magsgn_word_unfold(
                    ht_magsgn_view(self), (count - bit) as nat, bit as nat, value,
                );
            }
            if bit >= 64 {
                return Err(invalid_code_block());
            }
            if self.read_bit()? {
                value |= 1_u64 << bit;
            }
            bit = bit.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_ht_magsgn_word_unfold(ht_magsgn_view(self), 0, bit as nat, value);
        }
        Ok(value)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_magsgn_view(final(self))) == crate::specs::ht_magsgn_finish(ht_magsgn_view(old(self))),
        )
    )]
    pub fn finish(&mut self) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost initial = ht_magsgn_view(self); }
        let mut remaining_bits = self
            .input
            .len()
            .checked_mul(8)
            .ok_or_else(arithmetic_overflow)?;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant initial == ht_magsgn_view(old(self)),
                    crate::specs::ht_magsgn_finish(initial)
                    == crate::specs::ht_magsgn_finish_run(ht_magsgn_view(self), remaining_bits as nat),
                decreases remaining_bits,
            )
        )]
        while self.has_pending_physical_bits() {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_ht_magsgn_finish_unfold(ht_magsgn_view(self), remaining_bits as nat);
            }
            if remaining_bits == 0 {
                return Err(invalid_code_block());
            }
            remaining_bits -= 1;
            if !self.read_bit()? {
                return Err(invalid_code_block());
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_ht_magsgn_finish_unfold(ht_magsgn_view(self), remaining_bits as nat);
        }
        Ok(())
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result => ensures result == crate::specs::ht_magsgn_pending(ht_magsgn_view(self)),)
    )]
    fn has_pending_physical_bits(&self) -> bool {
        (self.has_byte && self.current_is_physical && self.next_bit <= self.last_bit)
            || self.next_byte < self.input.len()
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_magsgn_view(final(self))) == crate::specs::ht_magsgn_read(ht_magsgn_view(old(self))),
        )
    )]
    fn read_bit(&mut self) -> Result<bool, DecodeError> {
        if !self.has_byte || self.next_bit > self.last_bit {
            self.load_byte()?;
        }
        if !self.has_byte || self.next_bit > self.last_bit {
            return Err(invalid_code_block());
        }
        if self.next_bit >= 8 {
            return Err(invalid_code_block());
        }
        let bit = self.value >> self.next_bit & 1 != 0;
        self.next_bit = self
            .next_bit
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        if self.current_is_physical {
            self.physical_bits_read = self
                .physical_bits_read
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        } else {
            self.fill_bits_read = self
                .fill_bits_read
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        self.work_units = self
            .work_units
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        if self.work_units > self.maximum_work_units {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: self.maximum_work_units,
                actual: self.work_units,
            });
        }
        Ok(bit)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_magsgn_view(final(self))) == crate::specs::ht_magsgn_load(ht_magsgn_view(old(self))),
        )
    )]
    fn load_byte(&mut self) -> Result<(), DecodeError> {
        let stuffed = self.previous_was_ff;
        if self.next_byte < self.input.len() {
            let value = *self
                .input
                .get(self.next_byte)
                .ok_or_else(invalid_code_block)?;
            if !crate::refinement::ht_forward_stuffing_valid(stuffed, value) {
                return Err(DecodeError::MalformedInput {
                    offset: self.next_byte as u64,
                    kind: MalformedKind::InvalidBitStuffing,
                });
            }
            self.value = value;
            self.current_is_physical = true;
            self.next_byte = self
                .next_byte
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        } else {
            self.value = crate::refinement::ht_magsgn_fill_value();
            self.current_is_physical = false;
        }
        self.next_bit = 0;
        self.last_bit = crate::refinement::ht_forward_data_bits(stuffed) - 1;
        self.has_byte = true;
        self.previous_was_ff = self.value == 0xff;
        Ok(())
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct HtMelBitReader<'a> {
    input: &'a [u8],
    next_byte: usize,
    current_byte: usize,
    value: u8,
    next_bit: i8,
    previous_was_ff: bool,
    bits_read: u64,
    maximum_work_units: u64,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn ht_mel_view(reader: &HtMelBitReader<'_>) -> crate::specs::HtMelState {
        crate::specs::HtMelState {
            input: reader.input@, next_byte: reader.next_byte, current_byte: reader.current_byte,
            value: reader.value, next_bit: reader.next_bit, previous_was_ff: reader.previous_was_ff,
            bits_read: reader.bits_read, maximum_work_units: reader.maximum_work_units,
        }
    }

    #[verifier::inline]
    spec fn ht_mel_outcome(result: Result<HtMelBitReader<'_>, DecodeError>)
        -> Result<crate::specs::HtMelState, DecodeError>
    {
        match result { Ok(reader) => Ok(ht_mel_view(&reader)), Err(error) => Err(error) }
    }

    pub closed spec fn ht_position_view(position: Option<HtBitPosition>) -> Option<(u16, u8)> {
        match position { Some(position) => Some((position.byte, position.bit)), None => None }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures ht_mel_outcome(result) == crate::specs::ht_mel_initial(input@, maximum_work_units),
    )
)]
fn ht_mel_from_input(
    input: &[u8],
    maximum_work_units: u64,
) -> Result<HtMelBitReader<'_>, DecodeError> {
    if input.is_empty() || input.len() > MAX_HT_SCUP - 1 {
        return Err(invalid_code_block());
    }
    let mut reader = HtMelBitReader {
        input,
        next_byte: 0,
        current_byte: 0,
        value: 0,
        next_bit: -1,
        previous_was_ff: false,
        bits_read: 0,
        maximum_work_units,
    };
    reader.load_byte()?;
    Ok(reader)
}

impl<'a> HtMelBitReader<'a> {
    pub fn new(input: &'a [u8], limits: &ResourceLimits) -> Result<Self, DecodeError> {
        ht_mel_from_input(input, limits.maximum_work_units())
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures ht_position_view(result) == crate::specs::ht_mel_position(ht_mel_view(self)),
        )
    )]
    pub fn next_position(&self) -> Option<HtBitPosition> {
        if self.next_bit < 0 {
            None
        } else {
            Some(HtBitPosition {
                byte: u16::try_from(self.current_byte).ok()?,
                bit: u8::try_from(self.next_bit).ok()?,
            })
        }
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result => ensures result == ht_mel_view(self).bits_read,)
    )]
    pub const fn bits_read(&self) -> u64 {
        self.bits_read
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_mel_view(final(self))) == crate::specs::ht_mel_read(ht_mel_view(old(self))),
        )
    )]
    pub fn read_bit(&mut self) -> Result<bool, DecodeError> {
        if self.next_bit < 0 {
            self.load_byte()?;
        }
        if self.next_bit < 0 {
            return Err(invalid_code_block());
        }
        let bit_index = u8::try_from(self.next_bit).map_err(|_| arithmetic_overflow())?;
        if bit_index >= 8 {
            return Err(invalid_code_block());
        }
        let bit = (self.value >> bit_index) & 1 != 0;
        self.next_bit -= 1;
        self.bits_read = self
            .bits_read
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        if self.bits_read > self.maximum_work_units {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: self.maximum_work_units,
                actual: self.bits_read,
            });
        }
        Ok(bit)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_mel_view(final(self))) == crate::specs::ht_mel_load(ht_mel_view(old(self))),
        )
    )]
    fn load_byte(&mut self) -> Result<(), DecodeError> {
        if self.next_byte >= self.input.len() {
            self.next_bit = -1;
            return Ok(());
        }
        let index = self.next_byte;
        let mut value = *self.input.get(index).ok_or_else(invalid_code_block)?;
        if index + 1 == self.input.len() {
            value |= 0x0f;
        }
        if !crate::refinement::ht_mel_stuffing_valid(self.previous_was_ff, value) {
            return Err(DecodeError::MalformedInput {
                offset: index as u64,
                kind: MalformedKind::InvalidBitStuffing,
            });
        }
        self.current_byte = index;
        self.value = value;
        self.next_bit = if self.previous_was_ff { 6 } else { 7 };
        self.previous_was_ff = value == 0xff;
        self.next_byte = self
            .next_byte
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct HtReverseVlcReader<'a> {
    input: &'a [u8],
    current_byte: usize,
    value: u8,
    next_bit: u8,
    last_bit: u8,
    has_byte: bool,
    later_byte_requires_unstuffing: bool,
    bits_read: u64,
    maximum_work_units: u64,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn ht_reverse_vlc_view(reader: &HtReverseVlcReader<'_>) -> crate::specs::HtReverseVlcState {
        crate::specs::HtReverseVlcState {
            input: reader.input@, current_byte: reader.current_byte, value: reader.value,
            next_bit: reader.next_bit, last_bit: reader.last_bit, has_byte: reader.has_byte,
            later_byte_requires_unstuffing: reader.later_byte_requires_unstuffing,
            bits_read: reader.bits_read, maximum_work_units: reader.maximum_work_units,
        }
    }

    #[verifier::inline]
    spec fn ht_reverse_vlc_outcome(result: Result<HtReverseVlcReader<'_>, DecodeError>)
        -> Result<crate::specs::HtReverseVlcState, DecodeError>
    {
        match result { Ok(reader) => Ok(ht_reverse_vlc_view(&reader)), Err(error) => Err(error) }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures ht_reverse_vlc_outcome(result) == crate::specs::ht_reverse_vlc_initial(input@, maximum_work_units),
    )
)]
fn ht_reverse_vlc_from_input(
    input: &[u8],
    maximum_work_units: u64,
) -> Result<HtReverseVlcReader<'_>, DecodeError> {
    if input.len() < 2 || input.len() > MAX_HT_SCUP {
        return Err(invalid_code_block());
    }
    let current_byte = input.len().checked_sub(2).ok_or_else(arithmetic_overflow)?;
    let value = *input.get(current_byte).ok_or_else(invalid_code_block)?;
    let nibble = value >> 4;
    Ok(HtReverseVlcReader {
        input,
        current_byte,
        value,
        next_bit: 4,
        last_bit: if nibble & 0x07 == 0x07 { 6 } else { 7 },
        has_byte: true,
        later_byte_requires_unstuffing: value | 0x0f > 0x8f,
        bits_read: 0,
        maximum_work_units,
    })
}

impl<'a> HtReverseVlcReader<'a> {
    pub fn new(input: &'a [u8], limits: &ResourceLimits) -> Result<Self, DecodeError> {
        ht_reverse_vlc_from_input(input, limits.maximum_work_units())
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures ht_position_view(result) == crate::specs::ht_reverse_vlc_position(ht_reverse_vlc_view(self)),
        )
    )]
    pub fn next_position(&self) -> Option<HtBitPosition> {
        if !self.has_byte {
            None
        } else {
            Some(HtBitPosition {
                byte: u16::try_from(self.current_byte).ok()?,
                bit: self.next_bit,
            })
        }
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result => ensures result == ht_reverse_vlc_view(self).bits_read,)
    )]
    pub const fn bits_read(&self) -> u64 {
        self.bits_read
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_reverse_vlc_view(final(self)))
                == crate::specs::ht_reverse_vlc_read(ht_reverse_vlc_view(old(self))),
        )
    )]
    pub fn read_bit(&mut self) -> Result<bool, DecodeError> {
        if !self.has_byte || self.next_bit >= 8 {
            return Err(invalid_code_block());
        }
        let bit = (self.value >> self.next_bit) & 1 != 0;
        self.next_bit = self
            .next_bit
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        self.bits_read = self
            .bits_read
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        if self.bits_read > self.maximum_work_units {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: self.maximum_work_units,
                actual: self.bits_read,
            });
        }
        if self.next_bit > self.last_bit {
            self.load_previous_byte()?;
        }
        Ok(bit)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_reverse_vlc_view(final(self)))
                == crate::specs::ht_reverse_vlc_load(ht_reverse_vlc_view(old(self))),
        )
    )]
    fn load_previous_byte(&mut self) -> Result<(), DecodeError> {
        if self.current_byte == 0 {
            self.has_byte = false;
            return Ok(());
        }
        self.current_byte = self
            .current_byte
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?;
        self.value = *self
            .input
            .get(self.current_byte)
            .ok_or_else(invalid_code_block)?;
        self.next_bit = 0;
        self.last_bit = crate::refinement::ht_reverse_data_bits(
            self.later_byte_requires_unstuffing,
            self.value,
        ) - 1;
        self.later_byte_requires_unstuffing = self.value > 0x8f;
        Ok(())
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct HtQuadMetadata {
    pub x: u16,
    pub y: u16,
    pub context: u8,
    pub initial_row: bool,
    pub significance: u8,
    /// Values of the embedded leading bits (e_1), one flag per sample.
    pub embedded_magnitude: u8,
    /// Flags selecting omitted leading bits (e_k), one flag per sample.
    pub embedded_bit_mask: u8,
    pub u_offset: bool,
    pub u_base: u8,
    pub codeword_bits: u8,
}

#[cfg_attr(not(verus_keep_ghost), derive(Clone, Debug, Eq, PartialEq))]
pub struct HtCleanupMetadata {
    pub width: u16,
    pub height: u16,
    pub quads: Vec<HtQuadMetadata>,
    pub significant_samples: u16,
    pub mel_bits: u32,
    pub vlc_bits: u32,
    pub mel_events: u32,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct HtRefinementSegment<'a> {
    encoded: &'a [u8],
    significance_propagation: HtSubstreamView<'a>,
    magnitude_refinement: HtSubstreamView<'a>,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct HtForwardRefinementReader<'a> {
    input: &'a [u8],
    next_byte: usize,
    current_byte: usize,
    value: u8,
    next_bit: u8,
    last_bit: u8,
    has_byte: bool,
    previous_was_ff: bool,
    physical_bits_read: u64,
    fill_bits_read: u64,
    maximum_work_units: u64,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn ht_forward_refinement_view(reader: &HtForwardRefinementReader<'_>) -> crate::specs::HtForwardRefinementState {
        crate::specs::HtForwardRefinementState {
            input: reader.input@, next_byte: reader.next_byte, current_byte: reader.current_byte,
            value: reader.value, next_bit: reader.next_bit, last_bit: reader.last_bit,
            has_byte: reader.has_byte, previous_was_ff: reader.previous_was_ff,
            physical_bits_read: reader.physical_bits_read, fill_bits_read: reader.fill_bits_read,
            maximum_work_units: reader.maximum_work_units,
        }
    }

    #[verifier::inline]
    spec fn ht_forward_refinement_outcome(result: Result<HtForwardRefinementReader<'_>, DecodeError>)
        -> Result<crate::specs::HtForwardRefinementState, DecodeError>
    {
        match result { Ok(reader) => Ok(ht_forward_refinement_view(&reader)), Err(error) => Err(error) }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures ht_forward_refinement_outcome(result)
            == crate::specs::ht_forward_refinement_initial(input@, maximum_work_units),
    )
)]
fn ht_forward_refinement_from_input(
    input: &[u8],
    maximum_work_units: u64,
) -> Result<HtForwardRefinementReader<'_>, DecodeError> {
    if input.len() > MAX_HT_REFINEMENT_BYTES {
        return Err(invalid_code_block());
    }
    Ok(HtForwardRefinementReader {
        input,
        next_byte: 0,
        current_byte: 0,
        value: 0,
        next_bit: 0,
        last_bit: 0,
        has_byte: false,
        previous_was_ff: false,
        physical_bits_read: 0,
        fill_bits_read: 0,
        maximum_work_units,
    })
}

impl<'a> HtForwardRefinementReader<'a> {
    pub fn new(input: &'a [u8], limits: &ResourceLimits) -> Result<Self, DecodeError> {
        ht_forward_refinement_from_input(input, limits.maximum_work_units())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == ht_forward_refinement_view(self).physical_bits_read,))]
    pub const fn physical_bits_read(&self) -> u64 {
        self.physical_bits_read
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == ht_forward_refinement_view(self).fill_bits_read,))]
    pub const fn fill_bits_read(&self) -> u64 {
        self.fill_bits_read
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_forward_refinement_view(final(self)))
                == crate::specs::ht_forward_refinement_read(ht_forward_refinement_view(old(self))),
        )
    )]
    pub fn read_bit(&mut self) -> Result<bool, DecodeError> {
        match self.read_positioned_bit() {
            Ok((bit, _)) => Ok(bit),
            Err(error) => Err(error),
        }
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_forward_refinement_view(final(self)))
                == crate::specs::ht_forward_refinement_positioned(ht_forward_refinement_view(old(self))),
        )
    )]
    fn read_positioned_bit(&mut self) -> Result<(bool, Option<HtBitPosition>), DecodeError> {
        if !self.has_byte || self.next_bit > self.last_bit {
            self.load_byte()?;
        }
        if !self.has_byte || self.next_bit > self.last_bit || self.next_bit >= 8 {
            return Err(invalid_code_block());
        }
        let position = if self.current_byte < self.input.len() {
            Some(HtBitPosition {
                byte: self.current_byte as u16,
                bit: self.next_bit,
            })
        } else {
            None
        };
        let bit = self.value >> self.next_bit & 1 != 0;
        self.next_bit = self
            .next_bit
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        if position.is_some() {
            self.physical_bits_read = self
                .physical_bits_read
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        } else {
            self.fill_bits_read = self
                .fill_bits_read
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        let work = self
            .physical_bits_read
            .checked_add(self.fill_bits_read)
            .ok_or_else(arithmetic_overflow)?;
        if work > self.maximum_work_units {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: self.maximum_work_units,
                actual: work,
            });
        }
        Ok((bit, position))
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_forward_refinement_view(final(self)))
                == crate::specs::ht_forward_refinement_load(ht_forward_refinement_view(old(self))),
        )
    )]
    fn load_byte(&mut self) -> Result<(), DecodeError> {
        let stuffed = self.previous_was_ff;
        if self.next_byte < self.input.len() {
            let value = *self
                .input
                .get(self.next_byte)
                .ok_or_else(invalid_code_block)?;
            if stuffed && value & 0x80 != 0 {
                return Err(DecodeError::MalformedInput {
                    offset: self.next_byte as u64,
                    kind: MalformedKind::InvalidBitStuffing,
                });
            }
            self.current_byte = self.next_byte;
            self.value = value;
            self.next_byte = self
                .next_byte
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        } else {
            self.current_byte = self.input.len();
            self.value = 0;
        }
        self.next_bit = 0;
        self.last_bit = if stuffed { 6 } else { 7 };
        self.has_byte = true;
        self.previous_was_ff = self.value == 0xff;
        Ok(())
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct HtReverseRefinementReader<'a> {
    input: &'a [u8],
    current_byte: usize,
    value: u8,
    next_bit: u8,
    last_bit: u8,
    has_byte: bool,
    later_requires_unstuffing: bool,
    physical_bits_read: u64,
    fill_bits_read: u64,
    maximum_work_units: u64,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn ht_reverse_refinement_view(reader: &HtReverseRefinementReader<'_>) -> crate::specs::HtReverseRefinementState {
        crate::specs::HtReverseRefinementState {
            input: reader.input@, current_byte: reader.current_byte, value: reader.value,
            next_bit: reader.next_bit, last_bit: reader.last_bit, has_byte: reader.has_byte,
            later_requires_unstuffing: reader.later_requires_unstuffing,
            physical_bits_read: reader.physical_bits_read, fill_bits_read: reader.fill_bits_read,
            maximum_work_units: reader.maximum_work_units,
        }
    }

    #[verifier::inline]
    spec fn ht_reverse_refinement_outcome(result: Result<HtReverseRefinementReader<'_>, DecodeError>)
        -> Result<crate::specs::HtReverseRefinementState, DecodeError>
    {
        match result { Ok(reader) => Ok(ht_reverse_refinement_view(&reader)), Err(error) => Err(error) }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures ht_reverse_refinement_outcome(result)
            == crate::specs::ht_reverse_refinement_initial(input@, maximum_work_units),
    )
)]
fn ht_reverse_refinement_from_input(
    input: &[u8],
    maximum_work_units: u64,
) -> Result<HtReverseRefinementReader<'_>, DecodeError> {
    if input.len() > MAX_HT_REFINEMENT_BYTES {
        return Err(invalid_code_block());
    }
    let mut reader = HtReverseRefinementReader {
        input,
        current_byte: input.len(),
        value: 0,
        next_bit: 0,
        last_bit: 7,
        has_byte: false,
        later_requires_unstuffing: true,
        physical_bits_read: 0,
        fill_bits_read: 0,
        maximum_work_units,
    };
    reader.load_previous_byte()?;
    Ok(reader)
}

impl<'a> HtReverseRefinementReader<'a> {
    pub fn new(input: &'a [u8], limits: &ResourceLimits) -> Result<Self, DecodeError> {
        ht_reverse_refinement_from_input(input, limits.maximum_work_units())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == ht_reverse_refinement_view(self).physical_bits_read,))]
    pub const fn physical_bits_read(&self) -> u64 {
        self.physical_bits_read
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == ht_reverse_refinement_view(self).fill_bits_read,))]
    pub const fn fill_bits_read(&self) -> u64 {
        self.fill_bits_read
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_reverse_refinement_view(final(self)))
                == crate::specs::ht_reverse_refinement_read(ht_reverse_refinement_view(old(self))),
        )
    )]
    pub fn read_bit(&mut self) -> Result<bool, DecodeError> {
        match self.read_positioned_bit() {
            Ok((bit, _)) => Ok(bit),
            Err(error) => Err(error),
        }
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_reverse_refinement_view(final(self)))
                == crate::specs::ht_reverse_refinement_positioned(ht_reverse_refinement_view(old(self))),
        )
    )]
    fn read_positioned_bit(&mut self) -> Result<(bool, Option<HtBitPosition>), DecodeError> {
        if !self.has_byte || self.next_bit > self.last_bit {
            self.load_previous_byte()?;
        }
        if self.has_byte && self.next_bit >= 8 {
            return Err(invalid_code_block());
        }
        let position = if self.has_byte {
            Some(HtBitPosition {
                byte: self.current_byte as u16,
                bit: self.next_bit,
            })
        } else {
            None
        };
        let bit = if self.has_byte {
            self.value >> self.next_bit & 1 != 0
        } else {
            false
        };
        if self.has_byte {
            self.next_bit = self
                .next_bit
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
            self.physical_bits_read = self
                .physical_bits_read
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        } else {
            self.fill_bits_read = self
                .fill_bits_read
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        let work = self
            .physical_bits_read
            .checked_add(self.fill_bits_read)
            .ok_or_else(arithmetic_overflow)?;
        if work > self.maximum_work_units {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: self.maximum_work_units,
                actual: work,
            });
        }
        Ok((bit, position))
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_reverse_refinement_view(final(self)))
                == crate::specs::ht_reverse_refinement_load(ht_reverse_refinement_view(old(self))),
        )
    )]
    fn load_previous_byte(&mut self) -> Result<(), DecodeError> {
        if self.current_byte == 0 {
            self.has_byte = false;
            self.value = 0;
            self.next_bit = 0;
            self.last_bit = 7;
            return Ok(());
        }
        self.current_byte = self
            .current_byte
            .checked_sub(1)
            .ok_or_else(arithmetic_overflow)?;
        self.value = *self
            .input
            .get(self.current_byte)
            .ok_or_else(invalid_code_block)?;
        self.next_bit = 0;
        self.last_bit = if self.later_requires_unstuffing && self.value & 0x7f == 0x7f {
            6
        } else {
            7
        };
        self.later_requires_unstuffing = self.value > 0x8f;
        self.has_byte = true;
        Ok(())
    }
}

struct HtRefinementReaders<'a> {
    forward: HtForwardRefinementReader<'a>,
    reverse: Option<HtReverseRefinementReader<'a>>,
    claimed: Vec<u8>,
    stuffed: Vec<u8>,
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn ht_refinement_readers_view(readers: &HtRefinementReaders<'_>) -> crate::specs::HtRefinementReadersState {
        crate::specs::HtRefinementReadersState {
            forward: ht_forward_refinement_view(&readers.forward),
            reverse: match readers.reverse {
                None => None, Some(reverse) => Some(ht_reverse_refinement_view(&reverse)),
            },
            claimed: readers.claimed@, stuffed: readers.stuffed@,
        }
    }

    #[verifier::inline]
    spec fn ht_refinement_readers_outcome(result: Result<HtRefinementReaders<'_>, DecodeError>)
        -> Result<crate::specs::HtRefinementReadersState, DecodeError>
    {
        match result { Ok(readers) => Ok(ht_refinement_readers_view(&readers)), Err(error) => Err(error) }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures ht_refinement_readers_outcome(result) == crate::specs::ht_refinement_readers_initial(
            forward_input@, reverse_input@, bytes, has_reverse, maximum_work_units,
        ),
    )
)]
fn ht_refinement_readers_from_inputs<'a>(
    forward_input: &'a [u8],
    reverse_input: &'a [u8],
    bytes: usize,
    has_reverse: bool,
    maximum_work_units: u64,
) -> Result<HtRefinementReaders<'a>, DecodeError> {
    let forward = ht_forward_refinement_from_input(forward_input, maximum_work_units)?;
    let reverse = if has_reverse {
        Some(ht_reverse_refinement_from_input(
            reverse_input,
            maximum_work_units,
        )?)
    } else {
        None
    };
    let claimed = vec![0; bytes];
    let stuffed = vec![0; bytes];
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(claimed@ =~= Seq::new(bytes as nat, |i: int| 0u8));
        assert(stuffed@ =~= Seq::new(bytes as nat, |i: int| 0u8));
    }
    Ok(HtRefinementReaders {
        forward,
        reverse,
        claimed,
        stuffed,
    })
}

impl<'a> HtRefinementReaders<'a> {
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_refinement_readers_view(final(self)))
                == crate::specs::ht_refinement_claim(ht_refinement_readers_view(old(self)), position),
        )
    )]
    fn claim(&mut self, position: Option<HtBitPosition>) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost initial = ht_refinement_readers_view(self); }
        let Some(position) = position else {
            return Ok(());
        };
        if position.bit >= 8 {
            return Err(invalid_code_block());
        }
        let byte = usize::from(position.byte);
        let mask = 1_u8 << position.bit;
        let result = {
            let claimed = self.claimed.get_mut(byte).ok_or_else(invalid_code_block)?;
            if !crate::refinement::ht_claim_available(*claimed & mask != 0) {
                Err(invalid_code_block())
            } else {
                *claimed |= mask;
                Ok(())
            }
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            if result.is_err() {
                assert(self.claimed@ =~= initial.claimed);
            } else {
                assert(self.claimed@ =~= initial.claimed.update(byte as int, initial.claimed[byte as int] | mask));
            }
            assert(ht_refinement_readers_view(self).forward == initial.forward);
            assert(ht_refinement_readers_view(self).reverse == initial.reverse);
            assert(ht_refinement_readers_view(self).stuffed == initial.stuffed);
        }
        result
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_refinement_readers_view(final(self)))
                == crate::specs::ht_refinement_read_forward(ht_refinement_readers_view(old(self))),
        )
    )]
    fn read_forward(&mut self) -> Result<bool, DecodeError> {
        let (bit, position) = self.forward.read_positioned_bit()?;
        self.claim(position)?;
        Ok(bit)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_refinement_readers_view(final(self)))
                == crate::specs::ht_refinement_read_reverse(ht_refinement_readers_view(old(self))),
        )
    )]
    fn read_reverse(&mut self) -> Result<bool, DecodeError> {
        let reverse = self.reverse.as_mut().ok_or_else(invalid_code_block)?;
        let (bit, position) = reverse.read_positioned_bit()?;
        if reverse.has_byte && reverse.last_bit == 6 {
            let stuffed = self
                .stuffed
                .get_mut(reverse.current_byte)
                .ok_or_else(invalid_code_block)?;
            *stuffed |= 0x80;
        }
        self.claim(position)?;
        Ok(bit)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures result == crate::specs::ht_refinement_finish(ht_refinement_readers_view(self), encoded@),
        )
    )]
    fn finish(&self, encoded: &[u8]) -> Result<(), DecodeError> {
        if self.claimed.len() != encoded.len() || self.stuffed.len() != encoded.len() {
            return Err(invalid_code_block());
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= encoded.len(),
                    self.claimed.len() == encoded.len(), self.stuffed.len() == encoded.len(),
                    forall |i: int| 0 <= i < index ==> encoded@[i] & !(self.claimed@[i] | self.stuffed@[i]) == 0,
                decreases encoded.len() - index,
            )
        )]
        while index < encoded.len() {
            let value = *encoded.get(index).ok_or_else(invalid_code_block)?;
            let claimed = *self.claimed.get(index).ok_or_else(invalid_code_block)?;
            let stuffed = *self.stuffed.get(index).ok_or_else(invalid_code_block)?;
            if value & !(claimed | stuffed) != 0 {
                return Err(invalid_code_block());
            }
            index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        Ok(())
    }
}

impl<'a> HtRefinementSegment<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result@ == ht_refinement_segment_view(self).encoded,))]
    pub fn encoded(&self) -> &'a [u8] {
        self.encoded
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_substream_view(&result) == ht_refinement_segment_view(self).significance_propagation,))]
    pub const fn significance_propagation(&self) -> HtSubstreamView<'a> {
        self.significance_propagation
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_substream_view(&result) == ht_refinement_segment_view(self).magnitude_refinement,))]
    pub const fn magnitude_refinement(&self) -> HtSubstreamView<'a> {
        self.magnitude_refinement
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct HtBlockSegments<'a> {
    cleanup: HtCleanupSegment<'a>,
    refinement: Option<HtRefinementSegment<'a>>,
    bytes: u32,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn ht_substream_view(stream: &HtSubstreamView<'_>) -> crate::specs::HtSubstreamState {
        crate::specs::HtSubstreamState {
            bytes: stream.bytes@, offset: stream.offset, forward: ht_direction_forward(stream.direction),
        }
    }

    pub closed spec fn ht_cleanup_segment_view(segment: &HtCleanupSegment<'_>) -> crate::specs::HtCleanupSegmentState {
        crate::specs::HtCleanupSegmentState {
            encoded: segment.encoded@, scup: segment.scup,
            mag_sgn: ht_substream_view(&segment.mag_sgn), mel_vlc: ht_substream_view(&segment.mel_vlc),
            mel: ht_substream_view(&segment.mel), vlc: ht_substream_view(&segment.vlc),
        }
    }

    pub closed spec fn ht_refinement_segment_view(segment: &HtRefinementSegment<'_>) -> crate::specs::HtRefinementSegmentState {
        crate::specs::HtRefinementSegmentState {
            encoded: segment.encoded@,
            significance_propagation: ht_substream_view(&segment.significance_propagation),
            magnitude_refinement: ht_substream_view(&segment.magnitude_refinement),
        }
    }

    pub closed spec fn ht_refinement_option_view(segment: Option<HtRefinementSegment<'_>>)
        -> Option<crate::specs::HtRefinementSegmentState>
    {
        match segment { None => None, Some(segment) => Some(ht_refinement_segment_view(&segment)) }
    }

    pub closed spec fn ht_block_segments_view(segments: &HtBlockSegments<'_>) -> crate::specs::HtBlockSegmentsState {
        crate::specs::HtBlockSegmentsState {
            cleanup: ht_cleanup_segment_view(&segments.cleanup),
            refinement: ht_refinement_option_view(segments.refinement), bytes: segments.bytes,
        }
    }

    pub open spec fn ht_optional_bytes_view(bytes: Option<&[u8]>) -> Option<Seq<u8>> {
        match bytes { None => None, Some(bytes) => Some(bytes@) }
    }

    pub closed spec fn ht_block_segments_outcome(result: Result<HtBlockSegments<'_>, DecodeError>)
        -> Result<crate::specs::HtBlockSegmentsState, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(segments) => Ok(ht_block_segments_view(&segments)) }
    }
}

impl<'a> HtBlockSegments<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_cleanup_segment_view(&result) == ht_block_segments_view(self).cleanup,))]
    pub const fn cleanup(&self) -> HtCleanupSegment<'a> {
        self.cleanup
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures ht_refinement_option_view(result) == ht_block_segments_view(self).refinement,))]
    pub const fn refinement(&self) -> Option<HtRefinementSegment<'a>> {
        self.refinement
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == ht_block_segments_view(self).bytes,))]
    pub const fn bytes(&self) -> u32 {
        self.bytes
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Tier1CodingStyle {
    pub block_coding: Tier1BlockCoding,
    pub force_high_throughput: bool,
    pub selective_arithmetic_coding_bypass: bool,
    pub reset_contexts: bool,
    pub terminate_all_passes: bool,
    pub vertical_causal_context: bool,
    pub predictable_termination: bool,
    pub segmentation_symbols: bool,
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum Tier1BlockCoding {
    #[default]
    Legacy,
    HighThroughput,
    Mixed,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum Tier1CodingMode {
    Mq,
    Raw,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Tier1PassPlan {
    pub kind: Tier1PassKind,
    pub coding_mode: Tier1CodingMode,
    pub terminates_segment: bool,
    pub reset_contexts_after_pass: bool,
    pub validate_predictable_termination: bool,
    pub expect_segmentation_symbol: bool,
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_tier1_coding_style_byte(value),
))]
pub(crate) fn tier1_coding_style_from_code_block_style(
    value: u8,
) -> Result<Tier1CodingStyle, DecodeError> {
    let block_coding = tier1_block_coding_from_code_block_style(value)?;
    Ok(Tier1CodingStyle {
        block_coding,
        force_high_throughput: matches!(block_coding, Tier1BlockCoding::HighThroughput),
        selective_arithmetic_coding_bypass: value & 0x01 != 0,
        reset_contexts: value & 0x02 != 0,
        terminate_all_passes: value & 0x04 != 0,
        vertical_causal_context: value & 0x08 != 0,
        predictable_termination: value & 0x10 != 0,
        segmentation_symbols: value & 0x20 != 0,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_tier1_block_coding_byte(value),
))]
pub(crate) fn tier1_block_coding_from_code_block_style(
    value: u8,
) -> Result<Tier1BlockCoding, DecodeError> {
    match value >> 6 {
        0 => Ok(Tier1BlockCoding::Legacy),
        1 if value & 0x37 == 0 => Ok(Tier1BlockCoding::HighThroughput),
        1 | 2 => Err(invalid_code_block()),
        3 if value & 0x05 == 0 => Ok(Tier1BlockCoding::Mixed),
        3 => Err(invalid_code_block()),
        _ => Err(invalid_code_block()),
    }
}

impl Tier1CodingStyle {
    pub fn from_code_block_style(value: u8) -> Result<Self, DecodeError> {
        tier1_coding_style_from_code_block_style(value)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(_result =>
        ensures ((), *final(self)) == crate::specs::part2_tier1_force_style(*old(self), required),
    ))]
    pub fn set_force_high_throughput(&mut self, required: bool) {
        self.force_high_throughput = required;
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_legacy_uses_raw(self, pass_index),
    ))]
    pub fn uses_raw_bypass(self, pass_index: u32) -> bool {
        self.selective_arithmetic_coding_bypass
            && pass_index >= 10
            && !matches!(pass_kind(pass_index), Tier1PassKind::Cleanup)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_legacy_plan(self, pass_index),
    ))]
    pub fn pass_plan(self, pass_index: u32) -> Tier1PassPlan {
        let kind = pass_kind(pass_index);
        let coding_mode = if self.uses_raw_bypass(pass_index) {
            Tier1CodingMode::Raw
        } else {
            Tier1CodingMode::Mq
        };
        let bypass_boundary = self.selective_arithmetic_coding_bypass
            && (pass_index == 9
                || (pass_index >= 10 && !matches!(kind, Tier1PassKind::SignificancePropagation)));
        let terminates_segment = self.terminate_all_passes || bypass_boundary;
        Tier1PassPlan {
            kind,
            coding_mode,
            terminates_segment,
            reset_contexts_after_pass: self.reset_contexts,
            validate_predictable_termination: self.predictable_termination
                && terminates_segment
                && matches!(coding_mode, Tier1CodingMode::Mq),
            expect_segmentation_symbol: self.segmentation_symbols
                && matches!(kind, Tier1PassKind::Cleanup),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_legacy_segment_passes(self, first_pass, available_passes),
    ))]
    pub fn segment_passes(self, first_pass: u32, available_passes: u32) -> u32 {
        if available_passes == 0 {
            return 0;
        }
        if self.terminate_all_passes {
            return 1;
        }
        if !self.selective_arithmetic_coding_bypass {
            return available_passes;
        }
        if first_pass < 10 {
            return available_passes.min(10 - first_pass);
        }
        match pass_kind(first_pass) {
            Tier1PassKind::SignificancePropagation => available_passes.min(2),
            Tier1PassKind::MagnitudeRefinement | Tier1PassKind::Cleanup => 1,
        }
    }
}

impl Tier1BlockCoding {
    pub fn from_code_block_style(value: u8) -> Result<Self, DecodeError> {
        tier1_block_coding_from_code_block_style(value)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::ht_block_coding_enabled(self),
    ))]
    pub const fn uses_high_throughput(self) -> bool {
        !matches!(self, Self::Legacy)
    }

    pub const fn selects_high_throughput(self, first_contribution_passes: u8) -> bool {
        match self {
            Self::Legacy => false,
            Self::HighThroughput => true,
            Self::Mixed => first_contribution_passes <= 3,
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_legacy_pass_kind(pass_index),
))]
pub const fn pass_kind(pass_index: u32) -> Tier1PassKind {
    match crate::refinement::tier1_pass_code(pass_index) {
        0 => Tier1PassKind::SignificancePropagation,
        1 => Tier1PassKind::MagnitudeRefinement,
        _ => Tier1PassKind::Cleanup,
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures ht_block_segments_outcome(result) == crate::specs::ht_parse_block_segments(
            cleanup@, ht_optional_bytes_view(refinement), crate::limits::resource_code_block_byte_limit(limits),
        ),
    )
)]
pub fn parse_ht_block_segments<'a>(
    cleanup: &'a [u8],
    refinement: Option<&'a [u8]>,
    limits: &ResourceLimits,
) -> Result<HtBlockSegments<'a>, DecodeError> {
    if cleanup.len() < 2 || cleanup.len() > MAX_HT_CLEANUP_BYTES {
        return Err(invalid_code_block());
    }
    let cleanup_bytes = cleanup.len() as u64;
    let refinement_bytes = match refinement {
        Some(bytes) => bytes.len() as u64,
        None => 0_u64,
    };
    if refinement_bytes > MAX_HT_REFINEMENT_BYTES as u64 {
        return Err(invalid_code_block());
    }
    let total_bytes = cleanup_bytes
        .checked_add(refinement_bytes)
        .ok_or_else(arithmetic_overflow)?;
    if total_bytes > limits.maximum_code_block_bytes() {
        return Err(DecodeError::LimitExceeded {
            resource: Resource::CodeBlockBytes,
            limit: limits.maximum_code_block_bytes(),
            actual: total_bytes,
        });
    }

    let penultimate = *cleanup
        .get(cleanup.len() - 2)
        .ok_or_else(invalid_code_block)?;
    let last = *cleanup.last().ok_or_else(invalid_code_block)?;
    let scup = crate::refinement::ht_scup(penultimate, last);
    if scup < 2 {
        return Err(invalid_code_block());
    }
    if scup > MAX_HT_SCUP || scup > cleanup.len() {
        return Err(invalid_code_block());
    }
    let mag_sgn_end = crate::refinement::ht_magsgn_end(cleanup.len(), scup);
    let mel_end = cleanup
        .len()
        .checked_sub(1)
        .ok_or_else(arithmetic_overflow)?;
    if mag_sgn_end > mel_end || mel_end > cleanup.len() {
        return Err(invalid_code_block());
    }
    let mag_sgn_bytes = cleanup.get(..mag_sgn_end).ok_or_else(invalid_code_block)?;
    let mel_vlc_bytes = cleanup.get(mag_sgn_end..).ok_or_else(invalid_code_block)?;
    let mel_bytes = cleanup
        .get(mag_sgn_end..mel_end)
        .ok_or_else(invalid_code_block)?;
    let mag_sgn_offset = 0_u32;
    let mel_vlc_offset = u32::try_from(mag_sgn_end).map_err(|_| arithmetic_overflow())?;
    let cleanup_segment = HtCleanupSegment {
        encoded: cleanup,
        scup: u16::try_from(scup).map_err(|_| arithmetic_overflow())?,
        mag_sgn: HtSubstreamView {
            bytes: mag_sgn_bytes,
            offset: mag_sgn_offset,
            direction: HtStreamDirection::Forward,
        },
        mel_vlc: HtSubstreamView {
            bytes: mel_vlc_bytes,
            offset: mel_vlc_offset,
            direction: HtStreamDirection::Forward,
        },
        mel: HtSubstreamView {
            bytes: mel_bytes,
            offset: mel_vlc_offset,
            direction: HtStreamDirection::Forward,
        },
        vlc: HtSubstreamView {
            bytes: mel_vlc_bytes,
            offset: mel_vlc_offset,
            direction: HtStreamDirection::Reverse,
        },
    };
    let refinement_segment = match refinement {
        Some(encoded) => Some(HtRefinementSegment {
            encoded,
            significance_propagation: HtSubstreamView {
                bytes: encoded,
                offset: u32::try_from(cleanup.len()).map_err(|_| arithmetic_overflow())?,
                direction: HtStreamDirection::Forward,
            },
            magnitude_refinement: HtSubstreamView {
                bytes: encoded,
                offset: u32::try_from(cleanup.len()).map_err(|_| arithmetic_overflow())?,
                direction: HtStreamDirection::Reverse,
            },
        }),
        None => None,
    };
    Ok(HtBlockSegments {
        cleanup: cleanup_segment,
        refinement: refinement_segment,
        bytes: u32::try_from(total_bytes).map_err(|_| arithmetic_overflow())?,
    })
}

struct HtCleanupReaders<'a> {
    mel: HtMelBitReader<'a>,
    vlc: HtReverseVlcReader<'a>,
    mel_last_byte: Option<u16>,
    vlc_last_byte: Option<u16>,
    terminal_byte: u16,
    mel_state: u8,
    mel_zero_events: u8,
    mel_terminating_one: bool,
    mel_events: u32,
    work_units: u64,
    maximum_work_units: u64,
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn ht_cleanup_readers_view(readers: &HtCleanupReaders<'_>) -> crate::specs::HtCleanupReaderState {
        crate::specs::HtCleanupReaderState {
            mel: ht_mel_view(&readers.mel), vlc: ht_reverse_vlc_view(&readers.vlc),
            mel_last_byte: readers.mel_last_byte, vlc_last_byte: readers.vlc_last_byte,
            terminal_byte: readers.terminal_byte, mel_state: readers.mel_state,
            mel_zero_events: readers.mel_zero_events, mel_terminating_one: readers.mel_terminating_one,
            mel_events: readers.mel_events, work_units: readers.work_units,
            maximum_work_units: readers.maximum_work_units,
        }
    }

    pub closed spec fn ht_direction_forward(direction: HtStreamDirection) -> bool {
        matches!(direction, HtStreamDirection::Forward)
    }

    pub closed spec fn ht_vlc_table_view(initial: bool) -> Seq<u16> {
        if initial { HT_VLC_INITIAL@ } else { HT_VLC_NON_INITIAL@ }
    }

    #[verifier::inline]
    spec fn ht_cleanup_readers_outcome(result: Result<HtCleanupReaders<'_>, DecodeError>)
        -> Result<crate::specs::HtCleanupReaderState, DecodeError>
    {
        match result { Ok(readers) => Ok(ht_cleanup_readers_view(&readers)), Err(error) => Err(error) }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures ht_cleanup_readers_outcome(result) == crate::specs::ht_cleanup_readers_initial(
            mel_input@, vlc_input@, maximum_work_units,
        ),
    )
)]
fn ht_cleanup_readers_from_inputs<'a>(
    mel_input: &'a [u8],
    vlc_input: &'a [u8],
    maximum_work_units: u64,
) -> Result<HtCleanupReaders<'a>, DecodeError> {
    Ok(HtCleanupReaders {
        mel: ht_mel_from_input(mel_input, maximum_work_units)?,
        vlc: ht_reverse_vlc_from_input(vlc_input, maximum_work_units)?,
        mel_last_byte: None,
        vlc_last_byte: None,
        terminal_byte: u16::try_from(
            vlc_input
                .len()
                .checked_sub(2)
                .ok_or_else(invalid_code_block)?,
        )
        .map_err(|_| arithmetic_overflow())?,
        mel_state: 0,
        mel_zero_events: 0,
        mel_terminating_one: false,
        mel_events: 0,
        work_units: 0,
        maximum_work_units,
    })
}

impl<'a> HtCleanupReaders<'a> {
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_cleanup_readers_view(final(self)))
                == crate::specs::ht_cleanup_reader_tick(ht_cleanup_readers_view(old(self))),
        )
    )]
    fn tick(&mut self) -> Result<(), DecodeError> {
        self.work_units = self
            .work_units
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        if self.work_units > self.maximum_work_units {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: self.maximum_work_units,
                actual: self.work_units,
            });
        }
        Ok(())
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures result == crate::specs::ht_cleanup_reader_can_claim(
                ht_cleanup_readers_view(self), ht_direction_forward(direction), byte,
            ),
        )
    )]
    fn can_claim_byte(&self, direction: HtStreamDirection, byte: u16) -> bool {
        let value = match self.vlc.input.get(usize::from(byte)) {
            Some(value) => *value,
            None => return false,
        };
        match direction {
            HtStreamDirection::Forward => match self.vlc_last_byte {
                Some(vlc) => crate::refinement::ht_cleanup_byte_claims_valid(
                    byte,
                    vlc,
                    self.terminal_byte,
                    value,
                ),
                None => byte <= self.terminal_byte,
            },
            HtStreamDirection::Reverse => match self.mel_last_byte {
                Some(mel) => crate::refinement::ht_cleanup_byte_claims_valid(
                    mel,
                    byte,
                    self.terminal_byte,
                    value,
                ),
                None => byte <= self.terminal_byte,
            },
        }
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_cleanup_readers_view(final(self)))
                == crate::specs::ht_cleanup_reader_read_mel(ht_cleanup_readers_view(old(self))),
        )
    )]
    fn read_mel_bit(&mut self) -> Result<bool, DecodeError> {
        if self.mel.next_position().is_none() {
            self.mel.load_byte()?;
        }
        let position = self.mel.next_position().ok_or_else(invalid_code_block)?;
        if !self.can_claim_byte(HtStreamDirection::Forward, position.byte) {
            return Err(invalid_code_block());
        }
        let bit = self.mel.read_bit()?;
        self.mel_last_byte = Some(position.byte);
        self.tick()?;
        Ok(bit)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_cleanup_readers_view(final(self)))
                == crate::specs::ht_cleanup_reader_read_vlc(ht_cleanup_readers_view(old(self))),
        )
    )]
    fn read_vlc_bit(&mut self) -> Result<bool, DecodeError> {
        let position = self.vlc.next_position().ok_or_else(invalid_code_block)?;
        if !self.can_claim_byte(HtStreamDirection::Reverse, position.byte) {
            return Err(invalid_code_block());
        }
        let bit = self.vlc.read_bit()?;
        self.vlc_last_byte = Some(position.byte);
        self.tick()?;
        Ok(bit)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures result == crate::specs::ht_cleanup_peek_vlc(ht_cleanup_readers_view(self), count),
        )
    )]
    fn peek_vlc(&self, count: u8) -> u16 {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost initial = ht_cleanup_readers_view(self); }
        let mut copy = self.vlc;
        let mut value = 0_u16;
        let mut bit = 0_u8;
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_ht_cleanup_peek_unfold(initial, ht_reverse_vlc_view(&copy), 0, 0, value);
        }
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant bit <= count,
                    initial == ht_cleanup_readers_view(self),
                invariant_except_break
                    crate::specs::ht_cleanup_peek_vlc(initial, count)
                        == crate::specs::ht_cleanup_peek_run(initial, ht_reverse_vlc_view(&copy), (count - bit) as nat, bit as nat, value),
                    crate::specs::ht_cleanup_peek_run(initial, ht_reverse_vlc_view(&copy), 0, bit as nat, value) == value,
                ensures value == crate::specs::ht_cleanup_peek_vlc(initial, count),
                decreases count - bit,
            )
        )]
        while bit < count {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_ht_cleanup_peek_unfold(
                    initial, ht_reverse_vlc_view(&copy), (count - bit) as nat, bit as nat, value,
                );
            }
            let available = match copy.next_position() {
                Some(vlc) => self.can_claim_byte(HtStreamDirection::Reverse, vlc.byte),
                None => false,
            };
            if !available {
                break;
            }
            let Ok(decoded) = copy.read_bit() else {
                break;
            };
            if decoded {
                if bit >= 16 {
                    break;
                }
                value |= 1_u16 << bit;
            }
            bit += 1;
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_ht_cleanup_peek_unfold(initial, ht_reverse_vlc_view(&copy), 0, bit as nat, value);
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            if bit == count {
                crate::proofs::production_impl::production_ht_cleanup_peek_unfold(initial, ht_reverse_vlc_view(&copy), 0, bit as nat, value);
            }
        }
        value
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_cleanup_readers_view(final(self)))
                == crate::specs::ht_cleanup_advance_vlc(ht_cleanup_readers_view(old(self)), count as nat),
        )
    )]
    fn advance_vlc(&mut self, count: u8) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost initial = ht_cleanup_readers_view(self); }
        let mut consumed = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant consumed <= count, initial == ht_cleanup_readers_view(old(self)),
                    crate::specs::ht_cleanup_advance_vlc(initial, count as nat)
                        == crate::specs::ht_cleanup_advance_vlc(ht_cleanup_readers_view(self), (count - consumed) as nat),
                decreases count - consumed,
            )
        )]
        while consumed < count {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_ht_cleanup_advance_unfold(
                    ht_cleanup_readers_view(self), (count - consumed) as nat,
                );
            }
            let _ = self.read_vlc_bit()?;
            consumed = consumed.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_ht_cleanup_advance_unfold(ht_cleanup_readers_view(self), 0);
        }
        Ok(())
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_cleanup_readers_view(final(self)))
                == crate::specs::ht_cleanup_mel_event(ht_cleanup_readers_view(old(self))),
        )
    )]
    fn mel_event(&mut self) -> Result<bool, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost initial = ht_cleanup_readers_view(self); }
        if self.mel_zero_events > 0 {
            self.mel_zero_events = self
                .mel_zero_events
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?;
            self.mel_events = self
                .mel_events
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
            return Ok(false);
        }
        if self.mel_terminating_one {
            self.mel_terminating_one = false;
            self.mel_events = self
                .mel_events
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
            return Ok(true);
        }

        if self.mel_state > 12 {
            return Err(invalid_code_block());
        }
        let exponent = crate::refinement::ht_mel_exponent(self.mel_state);
        if self.read_mel_bit()? {
            self.mel_zero_events = 1_u8 << exponent;
            self.mel_state = crate::refinement::ht_mel_next_state(self.mel_state, true);
        } else {
            let mut suffix = 0_u8;
            let mut decoded = 0_u8;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant decoded <= exponent, exponent <= 5,
                        initial == ht_cleanup_readers_view(old(self)),
                        crate::specs::ht_cleanup_mel_event(initial)
                            == crate::specs::ht_cleanup_mel_complete_suffix(
                                crate::specs::ht_cleanup_mel_suffix(ht_cleanup_readers_view(self), (exponent - decoded) as nat, suffix)),
                    decreases exponent - decoded,
                )
            )]
            while decoded < exponent {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_ht_cleanup_mel_suffix_unfold(
                        ht_cleanup_readers_view(self), (exponent - decoded) as nat, suffix,
                    );
                }
                suffix <<= 1;
                if self.read_mel_bit()? {
                    suffix |= 1;
                }
                decoded = decoded.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_ht_cleanup_mel_suffix_unfold(
                    ht_cleanup_readers_view(self), 0, suffix,
                );
            }
            self.mel_zero_events = suffix;
            self.mel_terminating_one = true;
            self.mel_state = crate::refinement::ht_mel_next_state(self.mel_state, false);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::ht_cleanup_mel_event(initial)
                == crate::specs::ht_cleanup_mel_emit(ht_cleanup_readers_view(self)));
        }
        if self.mel_zero_events > 0 {
            self.mel_zero_events = self
                .mel_zero_events
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?;
            self.mel_events = self
                .mel_events
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
            Ok(false)
        } else {
            self.mel_terminating_one = false;
            self.mel_events = self
                .mel_events
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
            Ok(true)
        }
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_cleanup_readers_view(final(self))) == crate::specs::ht_cleanup_quad_word(
                ht_cleanup_readers_view(old(self)), ht_vlc_table_view(initial), context,
            ),
        )
    )]
    fn decode_quad_word(&mut self, initial: bool, context: u8) -> Result<u16, DecodeError> {
        if context > 7 {
            return Err(invalid_code_block());
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            let word = crate::specs::ht_cleanup_peek_vlc(ht_cleanup_readers_view(self), 7);
            assert(word & 0x7fu16 <= 127) by (bit_vector);
        }
        let lookahead = usize::from(self.peek_vlc(7) & 0x7f);
        let index = crate::refinement::ht_vlc_table_index(context, lookahead as u8);
        let table = if initial {
            &HT_VLC_INITIAL
        } else {
            &HT_VLC_NON_INITIAL
        };
        let mut word = *table.get(index).ok_or_else(invalid_code_block)?;
        if context == 0 && !self.mel_event()? {
            word = 0;
        }
        let length = (word & 0x07) as u8;
        if !crate::refinement::ht_vlc_word_valid(word, length) {
            return Err(invalid_code_block());
        }
        self.advance_vlc(length)?;
        self.tick()?;
        Ok(word)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_cleanup_readers_view(final(self)))
                == crate::specs::ht_uvlc_prefix(ht_cleanup_readers_view(old(self))),
        )
    )]
    fn read_prefix(&mut self) -> Result<(u8, u8, u8), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_uvlc_prefix); }
        if self.read_vlc_bit()? {
            return Ok((1, 0, 1));
        }
        if self.read_vlc_bit()? {
            return Ok((2, 0, 2));
        }
        if self.read_vlc_bit()? {
            Ok((3, 1, 3))
        } else {
            Ok((5, 5, 3))
        }
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_cleanup_readers_view(final(self)))
                == crate::specs::ht_uvlc_suffix(ht_cleanup_readers_view(old(self)), length),
        )
    )]
    fn read_suffix(&mut self, length: u8) -> Result<u8, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost initial = ht_cleanup_readers_view(self); }
        let mut value = 0_u8;
        let mut bit = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant bit <= length, initial == ht_cleanup_readers_view(old(self)),
                    crate::specs::ht_uvlc_suffix(initial, length)
                        == crate::specs::ht_uvlc_suffix_run(ht_cleanup_readers_view(self), (length - bit) as nat, bit as nat, value),
                decreases length - bit,
            )
        )]
        while bit < length {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_ht_uvlc_suffix_unfold(
                    ht_cleanup_readers_view(self), (length - bit) as nat, bit as nat, value,
                );
            }
            if self.read_vlc_bit()? {
                if bit >= 8 {
                    return Err(invalid_code_block());
                }
                value |= 1_u8 << bit;
            }
            bit = bit.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_ht_uvlc_suffix_unfold(ht_cleanup_readers_view(self), 0, bit as nat, value);
        }
        Ok(value)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_cleanup_readers_view(final(self)))
                == crate::specs::ht_uvlc_prefix_value(ht_cleanup_readers_view(old(self))),
        )
    )]
    fn prefix_value(&mut self) -> Result<(u8, u8), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_uvlc_prefix_value); }
        let (base, suffix_length, prefix_length) = self.read_prefix()?;
        let suffix = self.read_suffix(suffix_length)?;
        let value = base.checked_add(suffix).ok_or_else(arithmetic_overflow)?;
        Ok((value, prefix_length))
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_cleanup_readers_view(final(self)))
                == crate::specs::ht_uvlc_initial(ht_cleanup_readers_view(old(self)), mode),
        )
    )]
    fn decode_initial_u(&mut self, mode: u8) -> Result<[u8; 2], DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost initial = ht_cleanup_readers_view(self); }
        let mut values = [1_u8, 1_u8];
        match mode {
            0 => {}
            1 | 2 => {
                let (value, _) = self.prefix_value()?;
                values[if mode == 1 { 0 } else { 1 }] =
                    value.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            3 => {
                let (base_0, suffix_0, prefix_0) = self.read_prefix()?;
                if prefix_0 > 2 {
                    values[1] = (if self.read_vlc_bit()? { 1_u8 } else { 0_u8 })
                        .checked_add(2)
                        .ok_or_else(arithmetic_overflow)?;
                    values[0] = base_0
                        .checked_add(self.read_suffix(suffix_0)?)
                        .ok_or_else(arithmetic_overflow)?
                        .checked_add(1)
                        .ok_or_else(arithmetic_overflow)?;
                } else {
                    let (base_1, suffix_1, _) = self.read_prefix()?;
                    values[0] = base_0
                        .checked_add(self.read_suffix(suffix_0)?)
                        .ok_or_else(arithmetic_overflow)?
                        .checked_add(1)
                        .ok_or_else(arithmetic_overflow)?;
                    values[1] = base_1
                        .checked_add(self.read_suffix(suffix_1)?)
                        .ok_or_else(arithmetic_overflow)?
                        .checked_add(1)
                        .ok_or_else(arithmetic_overflow)?;
                }
            }
            4 => {
                let (base_0, suffix_0, _) = self.read_prefix()?;
                let (base_1, suffix_1, _) = self.read_prefix()?;
                values[0] = ht_uvlc_byte(base_0, self.read_suffix(suffix_0)?, 3)?;
                values[1] = ht_uvlc_byte(base_1, self.read_suffix(suffix_1)?, 3)?;
            }
            _ => return Err(invalid_code_block()),
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            let expected = crate::specs::ht_uvlc_initial(initial, mode);
            assert(expected.0.is_ok());
            assert(expected.1 == ht_cleanup_readers_view(self));
            match expected.0 {
                Ok(expected_values) => {
                    assert(values@ =~= expected_values@);
                    vstd::array::axiom_array_ext_equal(values, expected_values);
                    assert(values =~= expected_values);
                },
                Err(_) => { assert(false); },
            }
        }
        Ok(values)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures (result, ht_cleanup_readers_view(final(self)))
                == crate::specs::ht_uvlc_non_initial(ht_cleanup_readers_view(old(self)), mode),
        )
    )]
    fn decode_non_initial_u(&mut self, mode: u8) -> Result<[u8; 2], DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost initial = ht_cleanup_readers_view(self); }
        let mut values = [1_u8, 1_u8];
        match mode {
            0 => {}
            1 | 2 => {
                let (value, _) = self.prefix_value()?;
                values[if mode == 1 { 0 } else { 1 }] =
                    value.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            3 => {
                let (base_0, suffix_0, _) = self.read_prefix()?;
                let (base_1, suffix_1, _) = self.read_prefix()?;
                values[0] = ht_uvlc_byte(base_0, self.read_suffix(suffix_0)?, 1)?;
                values[1] = ht_uvlc_byte(base_1, self.read_suffix(suffix_1)?, 1)?;
            }
            _ => return Err(invalid_code_block()),
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            let expected = crate::specs::ht_uvlc_non_initial(initial, mode);
            assert(expected.0.is_ok());
            assert(expected.1 == ht_cleanup_readers_view(self));
            match expected.0 {
                Ok(expected_values) => {
                    assert(values@ =~= expected_values@);
                    vstd::array::axiom_array_ext_equal(values, expected_values);
                    assert(values =~= expected_values);
                },
                Err(_) => { assert(false); },
            }
        }
        Ok(values)
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures result == crate::specs::ht_cleanup_readers_finish(ht_cleanup_readers_view(self)),
        )
    )]
    fn finish(&self) -> Result<(), DecodeError> {
        match (self.mel_last_byte, self.vlc_last_byte) {
            (Some(mel), Some(vlc)) => {
                let value = *self
                    .vlc
                    .input
                    .get(usize::from(mel))
                    .ok_or_else(invalid_code_block)?;
                if crate::refinement::ht_cleanup_byte_claims_valid(
                    mel,
                    vlc,
                    self.terminal_byte,
                    value,
                ) {
                    Ok(())
                } else {
                    Err(invalid_code_block())
                }
            }
            _ => Ok(()),
        }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result => ensures result == crate::specs::ht_uvlc_byte(base, suffix, bias),)
)]
fn ht_uvlc_byte(base: u8, suffix: u8, bias: u8) -> Result<u8, DecodeError> {
    match u8::try_from(crate::refinement::ht_uvlc_value(base, suffix, bias)) {
        Ok(value) => Ok(value),
        Err(_) => Err(arithmetic_overflow()),
    }
}

#[cfg(verus_keep_ghost)]
verus! {
    #[verifier::inline]
    spec fn ht_metadata_words_outcome(result: Result<([u16; 2], [u8; 2]), DecodeError>)
        -> Result<(Seq<u16>, Seq<u8>), DecodeError>
    {
        match result { Err(error) => Err(error), Ok((words, contexts)) => Ok((words@, contexts@)) }
    }

    #[verifier::inline]
    spec fn ht_metadata_quad_input(
        index: usize, row: usize, width: u16, height: u16, maximum_u: u8,
        word: u16, context: u8, significance: u8, u_value: u8, initial: bool,
    ) -> crate::specs::HtMetadataQuadInput {
        crate::specs::HtMetadataQuadInput {
            index, row, width, height, maximum_u, word, context, significance, u_value, initial,
        }
    }

    #[verifier::inline]
    spec fn ht_metadata_pair_view(
        readers: &HtCleanupReaders<'_>, quads: &Vec<HtQuadMetadata>,                 current: Seq<u8>, significant: u16, left: u8,
    ) -> crate::specs::HtMetadataPairState {
        crate::specs::HtMetadataPairState {
            readers: ht_cleanup_readers_view(readers), quads: quads@, current, significant, left,
        }
    }

    #[verifier::inline]
    spec fn ht_metadata_row_view(
        readers: &HtCleanupReaders<'_>, quads: &Vec<HtQuadMetadata>, previous: &Vec<u8>, significant: u16,
    ) -> crate::specs::HtMetadataRowState {
        crate::specs::HtMetadataRowState {
            readers: ht_cleanup_readers_view(readers), quads: quads@, previous: previous@, significant,
        }
    }

    pub closed spec fn ht_cleanup_metadata_view(metadata: &HtCleanupMetadata) -> crate::specs::HtMetadataOutput {
        crate::specs::HtMetadataOutput {
            width: metadata.width, height: metadata.height, quads: metadata.quads@,
            significant_samples: metadata.significant_samples, mel_bits: metadata.mel_bits,
            vlc_bits: metadata.vlc_bits, mel_events: metadata.mel_events,
        }
    }

    pub closed spec fn ht_cleanup_metadata_outcome(result: Result<HtCleanupMetadata, DecodeError>)
        -> Result<crate::specs::HtMetadataOutput, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(metadata) => Ok(ht_cleanup_metadata_view(&metadata)) }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        requires 0 < quads_per_row <= 32768, pair < quads_per_row,
            previous_patterns.len() == quads_per_row, old(current_patterns).len() == quads_per_row,
        ensures (ht_metadata_words_outcome(result), ht_cleanup_readers_view(final(readers)),
            final(current_patterns)@, *final(left_context))
            == crate::specs::ht_metadata_words(ht_cleanup_readers_view(old(readers)),
                old(current_patterns)@, *old(left_context), previous_patterns@, pair as nat, initial),
            final(current_patterns).len() == old(current_patterns).len(),
    )
)]
fn decode_ht_metadata_words(
    readers: &mut HtCleanupReaders<'_>,
    current_patterns: &mut [u8],
    left_context: &mut u8,
    previous_patterns: &[u8],
    quads_per_row: usize,
    pair: usize,
    initial: bool,
) -> Result<([u16; 2], [u8; 2]), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost start_readers = ht_cleanup_readers_view(readers);
        let ghost start_current = current_patterns@;
        let ghost start_left = *left_context;
    }
    let mut words = [0_u16; 2];
    let mut contexts = [0_u8; 2];
    let mut member = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(words@ =~= Seq::new(2, |i: int| 0u16));
        assert(contexts@ =~= Seq::new(2, |i: int| 0u8));
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant member <= 2, 0 < quads_per_row <= 32768, pair < quads_per_row,
                previous_patterns.len() == quads_per_row, current_patterns.len() == quads_per_row,
                old(current_patterns).len() == quads_per_row, start_current.len() == quads_per_row,
                start_readers == ht_cleanup_readers_view(old(readers)),
                start_current == old(current_patterns)@, start_left == *old(left_context),
                crate::specs::ht_metadata_words(start_readers, start_current, start_left, previous_patterns@, pair as nat, initial)
                    == crate::specs::ht_metadata_words_finish(crate::specs::ht_metadata_words_run(
                        crate::specs::HtMetadataWordsState {
                            readers: ht_cleanup_readers_view(readers), current: current_patterns@,
                            left: *left_context, words: words@, contexts: contexts@,
                        }, previous_patterns@, pair as nat, initial, member as nat)),
            ensures member == 2 || pair + member >= quads_per_row,
            decreases 2 - member,
        )
    )]
    while member < 2 {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_ht_metadata_words_unfold(
                crate::specs::HtMetadataWordsState {
                    readers: ht_cleanup_readers_view(readers), current: current_patterns@,
                    left: *left_context, words: words@, contexts: contexts@,
                }, previous_patterns@, pair as nat, initial, member as nat,
            );
        }
        let quad_index = pair.checked_add(member).ok_or_else(arithmetic_overflow)?;
        if quad_index >= quads_per_row {
            break;
        }
        let context = if initial {
            *left_context
        } else {
            let north_pattern = *previous_patterns
                .get(quad_index)
                .ok_or_else(invalid_code_block)?;
            let north_west_pattern = match quad_index.checked_sub(1) {
                Some(index) => *previous_patterns
                    .get(index)
                    .ok_or_else(invalid_code_block)?,
                None => 0,
            };
            let north_east_index = quad_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            let north_east_pattern = match previous_patterns.get(north_east_index) {
                Some(pattern) => *pattern,
                None => 0,
            };
            crate::refinement::ht_noninitial_vlc_context(
                *left_context,
                north_west_pattern,
                north_pattern,
                north_east_pattern,
            )
        };
        let word = readers.decode_quad_word(initial, context)?;
        let significance = ((word >> 4) & 0x0f) as u8;
        *words.get_mut(member).ok_or_else(invalid_code_block)? = word;
        *contexts.get_mut(member).ok_or_else(invalid_code_block)? = context;
        *current_patterns
            .get_mut(quad_index)
            .ok_or_else(invalid_code_block)? = significance;
        *left_context = if initial {
            (significance & 0x01) | ((significance & 0x0e) >> 1)
        } else if significance & 0x0c != 0 {
            2
        } else {
            0
        };
        member = member.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_ht_metadata_words_unfold(
            crate::specs::HtMetadataWordsState {
                readers: ht_cleanup_readers_view(readers), current: current_patterns@,
                left: *left_context, words: words@, contexts: contexts@,
            }, previous_patterns@, pair as nat, initial, member as nat,
        );
    }
    Ok((words, contexts))
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        requires quad_index < 32768, quad_row < 32768,
        ensures (result, ht_cleanup_readers_view(final(readers)), final(quads)@, *final(significant_samples))
            == crate::specs::ht_metadata_emit(ht_cleanup_readers_view(old(readers)), old(quads)@,
                *old(significant_samples), ht_metadata_quad_input(quad_index, quad_row, width, height,
                    maximum_u, word, context, significance, u_value, initial)),
            result.is_ok() ==> final(quads).len() == old(quads).len() + 1,
    )
)]
#[allow(
    clippy::too_many_arguments,
    reason = "Explicit codec inputs and disjoint mutable outputs keep state frames auditable."
)]
fn append_ht_metadata_quad(
    readers: &mut HtCleanupReaders<'_>,
    quads: &mut Vec<HtQuadMetadata>,
    significant_samples: &mut u16,
    quad_index: usize,
    quad_row: usize,
    width: u16,
    height: u16,
    maximum_u: u8,
    word: u16,
    context: u8,
    significance: u8,
    u_value: u8,
    initial: bool,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost start_readers = ht_cleanup_readers_view(readers);
        let ghost start_quads = quads@;
        let ghost start_significant = *significant_samples;
        let ghost input = ht_metadata_quad_input(quad_index, quad_row, width, height, maximum_u, word, context, significance, u_value, initial);
    }
    if u_value > maximum_u {
        return Err(invalid_code_block());
    }
    let x = u16::try_from(quad_index.checked_mul(2).ok_or_else(arithmetic_overflow)?)
        .map_err(|_| arithmetic_overflow())?;
    let y = u16::try_from(quad_row.checked_mul(2).ok_or_else(arithmetic_overflow)?)
        .map_err(|_| arithmetic_overflow())?;
    if !crate::refinement::ht_quad_origin_valid(x, y, width, height) {
        return Err(invalid_code_block());
    }
    let mut sample = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant sample <= 4, x == quad_index * 2, y == quad_row * 2,
                quad_index < 32768, quad_row < 32768, x <= 65534, y <= 65534,
                start_readers == ht_cleanup_readers_view(old(readers)),
                start_readers == ht_cleanup_readers_view(readers),
                start_quads == old(quads)@, start_quads == quads@,
                start_significant == *old(significant_samples),
                input == ht_metadata_quad_input(quad_index, quad_row, width, height, maximum_u, word, context, significance, u_value, initial),
                crate::specs::ht_quad_origin_valid(x as nat, y as nat, width as nat, height as nat),
                u_value <= maximum_u,
                crate::specs::ht_metadata_emit(start_readers, start_quads, start_significant, input)
                    == crate::specs::ht_metadata_emit_after_samples(
                        crate::specs::ht_metadata_samples(*significant_samples, significance, x, y, width, height, sample),
                        start_readers, start_quads, input),
            decreases 4 - sample,
        )
    )]
    while sample < 4 {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_ht_metadata_samples_unfold(*significant_samples, significance, x, y, width, height, sample);
            assert(sample >> 1u8 <= 1) by(bit_vector) requires sample < 4;
            assert(sample & 1u8 <= 1) by(bit_vector);
            assert(sample >> 1u8 == sample / 2u8) by(bit_vector);
            assert(sample & 1u8 == sample % 2u8) by(bit_vector);
        }
        if significance & (1_u8 << sample) != 0 {
            let sample_x = x
                .checked_add(u16::from(sample >> 1))
                .ok_or_else(arithmetic_overflow)?;
            let sample_y = y
                .checked_add(u16::from(sample & 1))
                .ok_or_else(arithmetic_overflow)?;
            if !crate::refinement::ht_quad_sample_in_bounds(x, y, sample, width, height)
                || sample_x >= width
                || sample_y >= height
            {
                return Err(invalid_code_block());
            }
            *significant_samples = significant_samples
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        sample = sample.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_ht_metadata_samples_unfold(*significant_samples, significance, x, y, width, height, sample);
    }
    quads.push(HtQuadMetadata {
        x,
        y,
        context,
        initial_row: initial,
        significance,
        embedded_magnitude: ((word >> 8) & 0x0f) as u8,
        embedded_bit_mask: ((word >> 12) & 0x0f) as u8,
        u_offset: word & 0x08 != 0,
        u_base: u_value,
        codeword_bits: (word & 0x07) as u8,
    });
    readers.tick()?;
    Ok(())
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        requires 0 < quads_per_row <= 32768, pair < quads_per_row, quad_row < 32768,
            previous_patterns.len() == quads_per_row, old(current_patterns).len() == quads_per_row,
        ensures (result, ht_metadata_pair_view(final(readers), final(quads), final(current_patterns)@,
                *final(significant_samples), *final(left_context)))
            == crate::specs::ht_metadata_pair(
                ht_metadata_pair_view(old(readers), old(quads), old(current_patterns)@,
                    *old(significant_samples), *old(left_context)),
                previous_patterns@, pair as nat, quad_row as nat, width, height, maximum_u, initial),
            final(current_patterns).len() == old(current_patterns).len(),
            result.is_ok() ==> final(quads).len() == old(quads).len() + if pair + 1 < quads_per_row { 2int } else { 1int },
            result == Ok(()) || result.is_err(),
    )
)]
#[allow(
    clippy::too_many_arguments,
    reason = "Explicit codec inputs and disjoint mutable outputs keep state frames auditable."
)]
fn decode_ht_metadata_pair(
    readers: &mut HtCleanupReaders<'_>,
    quads: &mut Vec<HtQuadMetadata>,
    current_patterns: &mut [u8],
    significant_samples: &mut u16,
    left_context: &mut u8,
    previous_patterns: &[u8],
    quads_per_row: usize,
    pair: usize,
    quad_row: usize,
    width: u16,
    height: u16,
    maximum_u: u8,
    initial: bool,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost start = ht_metadata_pair_view(readers, quads, current_patterns@, *significant_samples, *left_context);
    }
    let (words, contexts) = decode_ht_metadata_words(
        readers,
        current_patterns,
        left_context,
        previous_patterns,
        quads_per_row,
        pair,
        initial,
    )?;
    let mut mode = if words[0] & 0x08 != 0 { 1_u8 } else { 0_u8 };
    mode |= if words[1] & 0x08 != 0 { 2_u8 } else { 0_u8 };
    if initial && mode == 3 && readers.mel_event()? {
        mode = 4;
    }
    let u_values = if initial {
        readers.decode_initial_u(mode)?
    } else {
        readers.decode_non_initial_u(mode)?
    };
    let mut member = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant member <= 2, pair + member <= quads_per_row,
                0 < quads_per_row <= 32768, pair < quads_per_row, quad_row < 32768,
                previous_patterns.len() == quads_per_row, current_patterns.len() == quads_per_row,
                old(current_patterns).len() == quads_per_row,
                start == ht_metadata_pair_view(old(readers), old(quads), old(current_patterns)@, *old(significant_samples), *old(left_context)),
                quads.len() == start.quads.len() + member,
                crate::specs::ht_metadata_pair(start, previous_patterns@, pair as nat, quad_row as nat, width, height, maximum_u, initial)
                    == crate::specs::ht_metadata_emits_run(
                        ht_metadata_pair_view(readers, quads, current_patterns@, *significant_samples, *left_context),
                        words@, contexts@, u_values, pair as nat, quad_row as nat, width, height, maximum_u, initial, member as nat),
            ensures member == 2 || pair + member == quads_per_row,
            decreases 2 - member,
        )
    )]
    while member < 2 {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_ht_metadata_emits_unfold(
                ht_metadata_pair_view(readers, quads, current_patterns@, *significant_samples, *left_context),
                words@, contexts@, u_values, pair as nat, quad_row as nat, width, height, maximum_u, initial, member as nat,
            );
        }
        let quad_index = pair.checked_add(member).ok_or_else(arithmetic_overflow)?;
        if quad_index >= quads_per_row {
            break;
        }
        let word = *words.get(member).ok_or_else(invalid_code_block)?;
        let significance = *current_patterns
            .get(quad_index)
            .ok_or_else(invalid_code_block)?;
        let u_value = *u_values.get(member).ok_or_else(invalid_code_block)?;
        append_ht_metadata_quad(
            readers,
            quads,
            significant_samples,
            quad_index,
            quad_row,
            width,
            height,
            maximum_u,
            word,
            *contexts.get(member).ok_or_else(invalid_code_block)?,
            significance,
            u_value,
            initial,
        )?;
        member = member.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_ht_metadata_emits_unfold(
            ht_metadata_pair_view(readers, quads, current_patterns@, *significant_samples, *left_context),
            words@, contexts@, u_values, pair as nat, quad_row as nat, width, height, maximum_u, initial, member as nat,
        );
    }
    Ok(())
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        requires 0 < quads_per_row <= 32768, quad_row < 32768,
            old(previous_patterns).len() == quads_per_row,
        ensures (result, ht_metadata_row_view(final(readers), final(quads), final(previous_patterns), *final(significant_samples)))
            == crate::specs::ht_metadata_row(
                ht_metadata_row_view(old(readers), old(quads), old(previous_patterns), *old(significant_samples)),
                quad_row as nat, width, height, maximum_u),
            final(previous_patterns).len() == old(previous_patterns).len(),
            result.is_ok() ==> final(quads).len() == old(quads).len() + quads_per_row,
            result == Ok(()) || result.is_err(),
    )
)]
#[allow(
    clippy::too_many_arguments,
    reason = "Explicit geometry and disjoint mutable outputs keep state frames auditable."
)]
fn decode_ht_metadata_row(
    readers: &mut HtCleanupReaders<'_>,
    quads: &mut Vec<HtQuadMetadata>,
    previous_patterns: &mut Vec<u8>,
    significant_samples: &mut u16,
    quads_per_row: usize,
    quad_row: usize,
    width: u16,
    height: u16,
    maximum_u: u8,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost start = ht_metadata_row_view(readers, quads, previous_patterns, *significant_samples);
        let ghost start_pair = crate::specs::ht_metadata_row_initial(start);
    }
    let initial = quad_row == 0;
    let mut current_patterns = vec![0_u8; quads_per_row];
    let mut left_context = 0_u8;
    let mut pair = 0_usize;
    let mut pair_passes = quads_per_row
        .checked_add(1)
        .ok_or_else(arithmetic_overflow)?
        / 2;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(current_patterns@ =~= Seq::new(quads_per_row as nat, |i: int| 0u8));
        reveal_with_fuel(crate::specs::ht_metadata_pairs, 1);
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant 0 < quads_per_row <= 32768, quad_row < 32768,
                pair <= quads_per_row + 1, pair % 2 == 0,
                pair + pair_passes * 2 >= quads_per_row,
                pair + pair_passes * 2 <= quads_per_row + 1,
                pair / 2 <= (quads_per_row + 1) / 2,
                current_patterns.len() == quads_per_row,
                previous_patterns.len() == quads_per_row,
                old(previous_patterns).len() == quads_per_row,
                start == ht_metadata_row_view(old(readers), old(quads), old(previous_patterns), *old(significant_samples)),
                start.previous == previous_patterns@, start.previous.len() == quads_per_row,
                start_pair == crate::specs::ht_metadata_row_initial(start),
                initial == (quad_row == 0),
                quads.len() == start.quads.len() + if pair <= quads_per_row { pair as int } else { quads_per_row as int },
                crate::specs::ht_metadata_pairs(start_pair, start.previous, quad_row as nat,
                    width, height, maximum_u, (pair / 2) as nat)
                    == (Ok(()), ht_metadata_pair_view(readers, quads, current_patterns@, *significant_samples, left_context)),
            decreases pair_passes,
        )
    )]
    while pair < quads_per_row {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost before = ht_metadata_pair_view(readers, quads, current_patterns@, *significant_samples, left_context);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert((pair / 2) * 2 == pair);
            crate::proofs::production_impl::production_ht_metadata_pairs_step(start_pair, start.previous, quad_row as nat,
                width, height, maximum_u, (pair / 2) as nat);
        }
        let result = decode_ht_metadata_pair(
            readers,
            quads,
            &mut current_patterns,
            significant_samples,
            &mut left_context,
            previous_patterns,
            quads_per_row,
            pair,
            quad_row,
            width,
            height,
            maximum_u,
            initial,
        );
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::ht_metadata_pair(before, start.previous, pair as nat, quad_row as nat,
                width, height, maximum_u, initial)
                == (result, ht_metadata_pair_view(readers, quads, current_patterns@, *significant_samples, left_context)));
        }
        match result {
            Ok(()) => {}
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_ht_metadata_pairs_error(start_pair, start.previous,
                        quad_row as nat, width, height, maximum_u, (pair / 2 + 1) as nat, ((quads_per_row + 1) / 2) as nat);
                }
                return Err(error);
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(result == Ok(()));
            assert(crate::specs::ht_metadata_pairs(start_pair, start.previous, quad_row as nat,
                width, height, maximum_u, (pair / 2 + 1) as nat)
                == (Ok(()), ht_metadata_pair_view(readers, quads, current_patterns@, *significant_samples, left_context)));
        }
        pair = pair.checked_add(2).ok_or_else(arithmetic_overflow)?;
        pair_passes = pair_passes.checked_sub(1).ok_or_else(arithmetic_overflow)?;
    }
    *previous_patterns = current_patterns;
    Ok(())
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures ht_cleanup_metadata_outcome(result) == crate::specs::ht_decode_cleanup_metadata(
            cleanup@, width, height, maximum_u, crate::limits::resource_code_block_byte_limit(limits),
            crate::limits::resource_work_limit(limits),
        ),
    )
)]
pub fn decode_ht_cleanup_metadata(
    cleanup: &[u8],
    width: u16,
    height: u16,
    maximum_u: u8,
    limits: &ResourceLimits,
) -> Result<HtCleanupMetadata, DecodeError> {
    let segments = parse_ht_block_segments(cleanup, None, limits)?;
    decode_ht_cleanup_segment_metadata(segments.cleanup(), width, height, maximum_u, limits)
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures ht_cleanup_metadata_outcome(result) == crate::specs::ht_cleanup_segment_metadata(
            ht_cleanup_segment_view(&cleanup), width, height, maximum_u, crate::limits::resource_work_limit(limits),
        ),
    )
)]
fn decode_ht_cleanup_segment_metadata(
    cleanup: HtCleanupSegment<'_>,
    width: u16,
    height: u16,
    maximum_u: u8,
    limits: &ResourceLimits,
) -> Result<HtCleanupMetadata, DecodeError> {
    if width == 0 || height == 0 || maximum_u == 0 || maximum_u > MAX_HT_MAGSGN_BITS {
        return Err(invalid_code_block());
    }
    let samples = u32::from(width)
        .checked_mul(u32::from(height))
        .ok_or_else(arithmetic_overflow)?;
    if samples > 4_096 {
        return Err(invalid_code_block());
    }
    let quads_per_row = usize::from(width)
        .checked_add(1)
        .ok_or_else(arithmetic_overflow)?
        / 2;
    let quad_rows = usize::from(height)
        .checked_add(1)
        .ok_or_else(arithmetic_overflow)?
        / 2;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::spec_properties::ht_quad_count_is_bounded_by_samples(width as nat, height as nat);
        assert(quads_per_row as nat * quad_rows as nat <= 4096);
    }
    let quad_count = quads_per_row
        .checked_mul(quad_rows)
        .ok_or_else(arithmetic_overflow)?;
    let mut readers = ht_cleanup_readers_from_inputs(
        cleanup.mel().bytes(),
        cleanup.vlc().bytes(),
        limits.maximum_work_units(),
    )?;
    let mut quads = Vec::with_capacity(quad_count);
    let mut previous_patterns = vec![0_u8; quads_per_row];
    let mut significant_samples = 0_u16;
    let mut quad_row = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost start = ht_metadata_row_view(&readers, &quads, &previous_patterns, significant_samples);
        let ghost segment = ht_cleanup_segment_view(&cleanup);
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(previous_patterns@ =~= Seq::new(quads_per_row as nat, |i: int| 0u8));
        assert(quads@ =~= Seq::<HtQuadMetadata>::empty());
        reveal_with_fuel(crate::specs::ht_metadata_rows, 1);
    }
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant quad_row <= quad_rows, 0 < quads_per_row <= 32768, 0 < quad_rows <= 32768,
                width > 0, height > 0, 0 < maximum_u <= 75,
                width as nat * height as nat <= 4096,
                previous_patterns.len() == quads_per_row,
                quads_per_row as nat == (width as nat + 1) / 2,
                quad_rows as nat == (height as nat + 1) / 2,
                segment == ht_cleanup_segment_view(&cleanup),
                crate::specs::ht_cleanup_readers_initial(segment.mel.bytes,
                    segment.vlc.bytes, crate::limits::resource_work_limit(limits)) == Ok(start.readers),
                start.quads == Seq::<HtQuadMetadata>::empty(), start.significant == 0,
                start.previous == Seq::new(quads_per_row as nat, |i: int| 0u8),
                crate::specs::ht_metadata_rows(start, width, height, maximum_u, quad_row as nat)
                    == (Ok(()), ht_metadata_row_view(&readers, &quads, &previous_patterns, significant_samples)),
            decreases quad_rows - quad_row,
        )
    )]
    while quad_row < quad_rows {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost before = ht_metadata_row_view(&readers, &quads, &previous_patterns, significant_samples); }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_ht_metadata_rows_step(start, width, height, maximum_u, quad_row as nat);
        }
        let result = decode_ht_metadata_row(
            &mut readers,
            &mut quads,
            &mut previous_patterns,
            &mut significant_samples,
            quads_per_row,
            quad_row,
            width,
            height,
            maximum_u,
        );
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::ht_metadata_row(before, quad_row as nat, width, height, maximum_u)
                == (result, ht_metadata_row_view(&readers, &quads, &previous_patterns, significant_samples)));
        }
        match result {
            Ok(()) => {}
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_ht_metadata_rows_error(start, width, height, maximum_u,
                        (quad_row + 1) as nat, quad_rows as nat);
                }
                return Err(error);
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(result == Ok(()));
            assert(crate::specs::ht_metadata_rows(start, width, height, maximum_u, (quad_row + 1) as nat)
                == (Ok(()), ht_metadata_row_view(&readers, &quads, &previous_patterns, significant_samples)));
        }
        quad_row = quad_row.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    readers.finish()?;
    Ok(HtCleanupMetadata {
        width,
        height,
        quads,
        significant_samples,
        mel_bits: match u32::try_from(readers.mel.bits_read()) {
            Ok(value) => value,
            Err(_) => return Err(arithmetic_overflow()),
        },
        vlc_bits: match u32::try_from(readers.vlc.bits_read()) {
            Ok(value) => value,
            Err(_) => return Err(arithmetic_overflow()),
        },
        mel_events: readers.mel_events,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::ht_complete_kappa(
        u_base, significance, north_west_exponent, north_east_exponent, maximum_u),
))]
pub fn complete_ht_kappa(
    u_base: u8,
    significance: u8,
    north_west_exponent: u8,
    north_east_exponent: u8,
    maximum_u: u8,
) -> Result<u8, DecodeError> {
    if u_base == 0 || significance & 0xf0 != 0 || maximum_u == 0 || maximum_u > MAX_HT_MAGSGN_BITS {
        return Err(invalid_code_block());
    }
    let gamma = significance & significance.wrapping_sub(1) != 0;
    let exponent = north_west_exponent.max(north_east_exponent);
    if u_base > u8::MAX - exponent {
        return Err(arithmetic_overflow());
    }
    let completed = crate::refinement::ht_kappa_completion(u_base, gamma, exponent);
    if completed > maximum_u {
        return Err(invalid_code_block());
    }
    Ok(completed)
}

#[cfg(verus_keep_ghost)]
verus! {
    #[verifier::inline]
    spec fn ht_numeric_magnitude_outcome(result: Result<(Magnitude, bool, u8), DecodeError>)
        -> Result<(nat, bool, u8), DecodeError>
    {
        match result {
            Err(error) => Err(error),
            Ok((magnitude, negative, exponent)) => Ok((crate::magnitude::magnitude_value(magnitude), negative, exponent)),
        }
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        requires magnitude_bits <= 75, 1 <= decoded_bitplanes <= 292,
        ensures ht_numeric_magnitude_outcome(result) == crate::specs::ht_numeric_center(
            crate::magnitude::magnitude_value(bin), negative, initial_exponent,
            magnitude_bits, embedded_value, decoded_bitplanes),
            match result {
                Ok((value, _, _)) => crate::magnitude::magnitude_limbs(value)
                    == crate::specs::tier1_value_limbs(crate::magnitude::magnitude_value(value))
                    && crate::magnitude::magnitude_value(value) > 0
                    && crate::magnitude::magnitude_value(value) < crate::specs::part1_magnitude_capacity(),
                Err(_) => true,
            },
    )
)]
fn center_ht_numeric_magnitude(
    bin: Magnitude,
    negative: bool,
    initial_exponent: u8,
    magnitude_bits: u8,
    embedded_value: bool,
    decoded_bitplanes: u16,
) -> Result<(Magnitude, bool, u8), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { vstd::arithmetic::power2::lemma2_to64(); }
    let mut bin_center = bin;
    let mut exponent = initial_exponent;
    if embedded_value {
        bin_center = bin_center.checked_add(crate::magnitude::magnitude_single_bit(u16::from(
            magnitude_bits,
        ))?)?;
        exponent = magnitude_bits
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
    }
    let low = bin_center.low_u64();
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_ht_low_word_parity(crate::magnitude::magnitude_value(bin_center));
        assert((low as nat % 2) == crate::magnitude::magnitude_value(bin_center) % 2);
    }
    if crate::refinement::ht_bin_center_odd(low) != low {
        bin_center = bin_center.checked_add(crate::magnitude::magnitude_from_u64(1))?;
        exponent = exponent.max(1);
    }
    let centered = bin_center.checked_add(crate::magnitude::magnitude_from_u64(2))?;
    let shift = decoded_bitplanes - 1;
    let magnitude = centered.checked_shl(shift)?.shift_right(1);
    if magnitude.is_zero() {
        return Err(invalid_code_block());
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::magnitude::representation_proofs::magnitude_limbs_shape(magnitude);
        crate::proofs::production_impl::production_tier1_value_limbs(crate::magnitude::magnitude_limbs(magnitude));
    }
    Ok((magnitude, negative, exponent))
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures (ht_numeric_magnitude_outcome(result), ht_magsgn_view(final(reader)))
            == crate::specs::ht_reconstruct_numeric_magnitude(ht_magsgn_view(old(reader)),
                completed_u, embedded_bit, embedded_value, decoded_bitplanes),
            match result {
                Ok((value, _, _)) => crate::magnitude::magnitude_limbs(value)
                    == crate::specs::tier1_value_limbs(crate::magnitude::magnitude_value(value))
                    && crate::magnitude::magnitude_value(value) > 0
                    && crate::magnitude::magnitude_value(value) < crate::specs::part1_magnitude_capacity(),
                Err(_) => true,
            },
    )
)]
fn reconstruct_ht_magnitude(
    reader: &mut HtForwardMagSgnReader<'_>,
    completed_u: u8,
    embedded_bit: bool,
    embedded_value: bool,
    decoded_bitplanes: u16,
) -> Result<(Magnitude, bool, u8), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost start = ht_magsgn_view(reader); }
    #[cfg(verus_keep_ghost)]
    proof! {
        vstd::arithmetic::power2::lemma_pow2_pos(384);
        vstd::arithmetic::power2::lemma2_to64();
    }
    if completed_u == 0
        || completed_u > MAX_HT_MAGSGN_BITS
        || decoded_bitplanes == 0
        || decoded_bitplanes > MAX_TIER1_BITPLANES
    {
        return Err(invalid_code_block());
    }
    let magnitude_bits = crate::refinement::ht_magnitude_payload_bits(completed_u, embedded_bit);
    let mut bin_center = crate::magnitude::magnitude_zero();
    let mut negative = false;
    let mut exponent = 0_u8;
    let mut bit = 0_u8;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant bit <= magnitude_bits, magnitude_bits <= 75,
                1 <= completed_u <= 75, 1 <= decoded_bitplanes <= 292,
                start == ht_magsgn_view(old(reader)),
                magnitude_bits as nat == crate::specs::ht_magnitude_payload_bits(completed_u as nat, embedded_bit),
                crate::magnitude::magnitude_value(bin_center) < crate::specs::part1_magnitude_capacity(),
                vstd::arithmetic::power2::pow2(1) == 2,
                crate::specs::ht_reconstruct_numeric_magnitude(start, completed_u, embedded_bit, embedded_value, decoded_bitplanes)
                    == crate::specs::ht_numeric_finish(
                        crate::specs::ht_numeric_payload(ht_magsgn_view(reader), (magnitude_bits - bit) as nat,
                            bit as nat, crate::magnitude::magnitude_value(bin_center), negative, exponent),
                        magnitude_bits, embedded_value, decoded_bitplanes),
            decreases magnitude_bits - bit,
        )
    )]
    while bit < magnitude_bits {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_ht_numeric_payload_unfold(
                ht_magsgn_view(reader), (magnitude_bits - bit) as nat, bit as nat,
                crate::magnitude::magnitude_value(bin_center), negative, exponent,
            );
        }
        if reader.read_bit()? {
            if crate::refinement::ht_magnitude_sign(bit, true) {
                negative = true;
            }
            bin_center =
                bin_center.checked_add(crate::magnitude::magnitude_single_bit(u16::from(bit))?)?;
            exponent = bit.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        bit = bit.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(bit == magnitude_bits);
        crate::proofs::production_impl::production_ht_numeric_payload_unfold(
            ht_magsgn_view(reader), 0, bit as nat,
            crate::magnitude::magnitude_value(bin_center), negative, exponent,
        );
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost final_value = crate::magnitude::magnitude_value(bin_center);
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(crate::specs::ht_reconstruct_numeric_magnitude(start, completed_u, embedded_bit, embedded_value, decoded_bitplanes)
            == (crate::specs::ht_numeric_center(final_value, negative, exponent, magnitude_bits, embedded_value, decoded_bitplanes),
                ht_magsgn_view(reader)));
    }
    center_ht_numeric_magnitude(
        bin_center,
        negative,
        exponent,
        magnitude_bits,
        embedded_value,
        decoded_bitplanes,
    )
}

struct HtCleanupPlaneReconstruction<'a> {
    output: CodeBlockPlane,
    reader: HtForwardMagSgnReader<'a>,
    previous_exponents: Vec<u8>,
    current_exponents: Vec<u8>,
    statistics: Tier1PassStatistics,
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn ht_plane_view(state: &HtCleanupPlaneReconstruction<'_>) -> crate::specs::HtPlaneState {
        crate::specs::HtPlaneState {
            output: code_block_plane_view(&state.output), reader: ht_magsgn_view(&state.reader),
            previous: state.previous_exponents@, current: state.current_exponents@, statistics: state.statistics,
        }
    }

    closed spec fn ht_plane_outcome(result: Result<(CodeBlockPlane, Tier1PassStatistics), DecodeError>)
        -> Result<(crate::specs::Tier1RawPlane, Tier1PassStatistics), DecodeError> {
        match result { Err(error) => Err(error), Ok((plane, statistics)) => Ok((code_block_plane_view(&plane), statistics)) }
    }
}

impl HtCleanupPlaneReconstruction<'_> {
    #[allow(
        clippy::too_many_arguments,
        reason = "The actual sample transition consumes these independent metadata and geometry fields."
    )]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        requires sample < 4, column + 1 < old(self).current_exponents.len(),
        ensures (result, ht_plane_view(final(self))) == crate::specs::ht_plane_sample(
            ht_plane_view(old(self)), quad, width, height, column as nat, completed_u, decoded_bitplanes, sample),
            final(self).current_exponents.len() == old(self).current_exponents.len(),
            final(self).previous_exponents@ == old(self).previous_exponents@,
            result == Ok(()) || result.is_err(),
    ))]
    fn sample(
        &mut self,
        quad: HtQuadMetadata,
        width: u16,
        height: u16,
        column: usize,
        completed_u: u8,
        decoded_bitplanes: u16,
        sample: u8,
    ) -> Result<(), DecodeError> {
        let next_column = column.checked_add(1).ok_or_else(arithmetic_overflow)?;
        if sample >= 8 {
            return Err(invalid_code_block());
        }
        let x = quad
            .x
            .checked_add(u16::from(sample >> 1))
            .ok_or_else(arithmetic_overflow)?;
        let y = quad
            .y
            .checked_add(u16::from(sample & 1))
            .ok_or_else(arithmetic_overflow)?;
        if x < width && y < height {
            self.statistics.scanned(1)?;
            if quad.significance & (1_u8 << sample) != 0 {
                let embedded_bit = quad.embedded_bit_mask & (1_u8 << sample) != 0;
                let embedded_value = quad.embedded_magnitude & (1_u8 << sample) != 0;
                let (magnitude, negative, exponent) = reconstruct_ht_magnitude(
                    &mut self.reader,
                    completed_u,
                    embedded_bit,
                    embedded_value,
                    decoded_bitplanes,
                )?;
                self.output
                    .mark_significant_magnitude(x, y, magnitude, negative)?;
                self.statistics.decoded_symbols(1)?;
                self.statistics.became_significant()?;
                if sample == 1 {
                    let entry = self
                        .current_exponents
                        .get_mut(column)
                        .ok_or_else(invalid_code_block)?;
                    *entry = (*entry).max(exponent);
                } else if sample == 3 {
                    *self
                        .current_exponents
                        .get_mut(next_column)
                        .ok_or_else(invalid_code_block)? = exponent;
                }
            }
        }
        Ok(())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "The shared quad stage preserves the real caller's metadata and traversal inputs."
    )]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        requires 0 < quads_per_row <= 32768,
            old(self).current_exponents.len() == quads_per_row + 1,
            old(self).previous_exponents.len() == quads_per_row + 1,
        ensures (result, ht_plane_view(final(self))) == crate::specs::ht_plane_quad(
            ht_plane_view(old(self)), quad, width, height, maximum_u, decoded_bitplanes,
            quads_per_row as nat, quad_index as nat),
            final(self).current_exponents.len() == quads_per_row + 1,
            final(self).previous_exponents.len() == quads_per_row + 1,
            result == Ok(()) || result.is_err(),
    ))]
    fn quad(
        &mut self,
        quad: HtQuadMetadata,
        width: u16,
        height: u16,
        maximum_u: u8,
        decoded_bitplanes: u16,
        quads_per_row: usize,
        quad_index: usize,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost start = ht_plane_view(self); }
        let column = quad_index
            .checked_rem(quads_per_row)
            .ok_or_else(arithmetic_overflow)?;
        if column == 0 {
            let mut exponent_index = 0_usize;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant exponent_index <= self.current_exponents.len(),
                        self.current_exponents.len() == quads_per_row + 1,
                        start.current.len() == quads_per_row + 1,
                        start == ht_plane_view(old(self)),
                        self.previous_exponents@ == start.previous,
                        code_block_plane_view(&self.output) == start.output,
                        ht_magsgn_view(&self.reader) == start.reader, self.statistics == start.statistics,
                        forall |i: int| 0 <= i < self.current_exponents.len() ==>
                            self.current_exponents@[i] == if i < exponent_index { 0u8 } else { start.current[i] },
                    decreases self.current_exponents.len() - exponent_index,
                )
            )]
            while exponent_index < self.current_exponents.len() {
                *self
                    .current_exponents
                    .get_mut(exponent_index)
                    .ok_or_else(invalid_code_block)? = 0;
                exponent_index = exponent_index
                    .checked_add(1)
                    .ok_or_else(arithmetic_overflow)?;
            }
            #[cfg(verus_keep_ghost)]
            proof! { assert(self.current_exponents@ =~= Seq::new(start.current.len(), |i: int| 0u8)); }
        }
        let next_column = column.checked_add(1).ok_or_else(arithmetic_overflow)?;
        let completed_u = if quad.initial_row {
            if quad.u_base > maximum_u {
                return Err(invalid_code_block());
            }
            quad.u_base
        } else {
            complete_ht_kappa(
                quad.u_base,
                quad.significance,
                *self
                    .previous_exponents
                    .get(column)
                    .ok_or_else(invalid_code_block)?,
                *self
                    .previous_exponents
                    .get(next_column)
                    .ok_or_else(invalid_code_block)?,
                maximum_u,
            )?
        };
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost samples_start = ht_plane_view(self); }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_plane_samples); }
        let mut sample = 0_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant sample <= 4, column < quads_per_row, next_column == column + 1,
                    samples_start.current.len() == quads_per_row + 1,
                    self.current_exponents.len() == quads_per_row + 1,
                    self.previous_exponents.len() == quads_per_row + 1,
                    crate::specs::ht_plane_quad(ht_plane_view(old(self)), quad, width, height,
                        maximum_u, decoded_bitplanes, quads_per_row as nat, quad_index as nat)
                        == crate::specs::ht_plane_quad_finish(crate::specs::ht_plane_samples(
                            samples_start, quad, width, height, column as nat, completed_u, decoded_bitplanes, 4),
                            quads_per_row as nat, column as nat),
                    (Ok::<(), DecodeError>(()), ht_plane_view(self)) == crate::specs::ht_plane_samples(
                        samples_start, quad, width, height, column as nat, completed_u, decoded_bitplanes, sample as nat),
                decreases 4 - sample,
            )
        )]
        while sample < 4 {
            let result = self.sample(
                quad,
                width,
                height,
                column,
                completed_u,
                decoded_bitplanes,
                sample,
            );
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal(crate::specs::ht_plane_samples);
                assert((result, ht_plane_view(self)) == crate::specs::ht_plane_samples(
                    samples_start, quad, width, height, column as nat, completed_u, decoded_bitplanes, sample as nat + 1));
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                if result.is_err() {
                    crate::proofs::production_impl::production_ht_plane_samples_error(
                        samples_start, quad, width, height, column as nat, completed_u, decoded_bitplanes, sample as nat + 1, 4);
                }
            }
            result?;
            sample = sample.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        if next_column == quads_per_row {
            core::mem::swap(&mut self.previous_exponents, &mut self.current_exponents);
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, ht_plane_view(final(self))) == crate::specs::ht_plane_finish(ht_plane_view(old(self))),
    ))]
    fn finish(&mut self) -> Result<(), DecodeError> {
        self.reader.finish()
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures ht_plane_outcome(result) == crate::specs::ht_cleanup_plane(
        code_block_plane_view(plane), ht_cleanup_segment_view(&cleanup), ht_cleanup_metadata_view(metadata),
        maximum_u, decoded_bitplanes, crate::limits::resource_work_limit(limits)),
))]
fn reconstruct_ht_cleanup_plane(
    plane: &CodeBlockPlane,
    cleanup: HtCleanupSegment<'_>,
    metadata: &HtCleanupMetadata,
    maximum_u: u8,
    decoded_bitplanes: u16,
    limits: &ResourceLimits,
) -> Result<(CodeBlockPlane, Tier1PassStatistics), DecodeError> {
    if metadata.width == 0 {
        return Err(invalid_code_block());
    }
    let output = code_block_plane_from_dimensions(plane.width(), plane.height())?;
    let reader = ht_magsgn_from_input(cleanup.mag_sgn().bytes(), limits.maximum_work_units())?;
    let doubled_quads = usize::from(metadata.width)
        .checked_add(1)
        .ok_or_else(arithmetic_overflow)?;
    let quads_per_row = doubled_quads
        .checked_div(2)
        .ok_or_else(arithmetic_overflow)?;
    if quads_per_row == 0 {
        return Err(invalid_code_block());
    }
    let exponent_count = quads_per_row
        .checked_add(1)
        .ok_or_else(arithmetic_overflow)?;
    let mut state = HtCleanupPlaneReconstruction {
        output,
        reader,
        previous_exponents: vec![0_u8; exponent_count],
        current_exponents: vec![0_u8; exponent_count],
        statistics: ht_statistics_zero(),
    };
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost start = ht_plane_view(&state); }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(state.previous_exponents@ =~= Seq::new(quads_per_row as nat + 1, |i: int| 0u8));
        assert(state.current_exponents@ =~= Seq::new(quads_per_row as nat + 1, |i: int| 0u8));
        reveal(crate::specs::ht_plane_quads);
    }
    let mut quad_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant quad_index <= metadata.quads.len(), 0 < quads_per_row <= 32768,
            start.current.len() == quads_per_row + 1,
            start.previous.len() == quads_per_row + 1,
            state.current_exponents.len() == quads_per_row + 1,
            state.previous_exponents.len() == quads_per_row + 1,
            crate::specs::ht_cleanup_plane(code_block_plane_view(plane), ht_cleanup_segment_view(&cleanup),
                ht_cleanup_metadata_view(metadata), maximum_u, decoded_bitplanes, crate::limits::resource_work_limit(limits))
                == crate::specs::ht_plane_output(crate::specs::ht_plane_quads(
                    start, metadata.quads@, metadata.width, metadata.height, maximum_u,
                    decoded_bitplanes, quads_per_row as nat, metadata.quads@.len())),
            (Ok::<(), DecodeError>(()), ht_plane_view(&state)) == crate::specs::ht_plane_quads(
                start, metadata.quads@, metadata.width, metadata.height, maximum_u,
                decoded_bitplanes, quads_per_row as nat, quad_index as nat),
        decreases metadata.quads.len() - quad_index,
    ))]
    while quad_index < metadata.quads.len() {
        let quad = *metadata
            .quads
            .get(quad_index)
            .ok_or_else(invalid_code_block)?;
        let result = state.quad(
            quad,
            metadata.width,
            metadata.height,
            maximum_u,
            decoded_bitplanes,
            quads_per_row,
            quad_index,
        );
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::ht_plane_quads);
            assert((result, ht_plane_view(&state)) == crate::specs::ht_plane_quads(
                start, metadata.quads@, metadata.width, metadata.height, maximum_u,
                decoded_bitplanes, quads_per_row as nat, quad_index as nat + 1));
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            if result.is_err() {
                crate::proofs::production_impl::production_ht_plane_quads_error(
                    start, metadata.quads@, metadata.width, metadata.height, maximum_u,
                    decoded_bitplanes, quads_per_row as nat, quad_index as nat + 1, metadata.quads@.len());
            }
        }
        result?;
        quad_index = quad_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    state.finish()?;
    Ok((state.output, state.statistics))
}

#[cfg(verus_keep_ghost)]
verus! {
    spec fn ht_boolean_vector_view(value: Vec<bool>) -> Seq<bool> { value@ }

    spec fn ht_refinement_magnitude_outcome(result: Result<Magnitude, DecodeError>) -> Result<(Seq<u64>, nat), DecodeError> {
        match result { Err(error) => Err(error), Ok(value) => Ok((
            crate::magnitude::magnitude_limbs(value), crate::magnitude::magnitude_value(value))) }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures ht_refinement_magnitude_outcome(result) == crate::specs::ht_significance_magnitude(decoded_bitplanes),
))]
fn ht_significance_magnitude(decoded_bitplanes: u16) -> Result<Magnitude, DecodeError> {
    if !(2..=MAX_TIER1_BITPLANES).contains(&decoded_bitplanes) {
        return Err(invalid_code_block());
    }
    let centered = crate::magnitude::magnitude_from_u64(3).checked_shl(decoded_bitplanes - 2)?;
    #[cfg(verus_keep_ghost)]
    proof! { vstd::arithmetic::power2::lemma2_to64(); }
    Ok(centered.shift_right(1))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures ht_refinement_magnitude_outcome(result) == crate::specs::ht_refinement_delta(decoded_bitplanes),
))]
fn ht_refinement_delta(decoded_bitplanes: u16) -> Result<Magnitude, DecodeError> {
    if !(2..=MAX_TIER1_BITPLANES).contains(&decoded_bitplanes) {
        return Err(invalid_code_block());
    }
    if decoded_bitplanes == 2 {
        Ok(crate::magnitude::magnitude_from_u64(1))
    } else {
        crate::magnitude::magnitude_single_bit(decoded_bitplanes - 3)
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures ht_boolean_vector_view(result) == crate::specs::ht_cleanup_significance(coefficient_raw_views(coefficients@)),
        result@ == crate::specs::ht_cleanup_significance(coefficient_raw_views(coefficients@)),
))]
fn ht_collect_cleanup_significance(coefficients: &[CoefficientState]) -> Vec<bool> {
    let mut output = Vec::with_capacity(coefficients.len());
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= coefficients.len(), output.len() == index,
            forall |i: int| 0 <= i < index ==> output@[i] == coefficients@[i].significant,
        decreases coefficients.len() - index,
    ))]
    while index < coefficients.len() {
        output.push(coefficients[index].significant);
        index += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! { assert(output@ =~= crate::specs::ht_cleanup_significance(coefficient_raw_views(coefficients@))); }
    output
}

#[cfg(verus_keep_ghost)]
verus! {
    spec fn ht_byte_vector_outcome(result: Result<Vec<u8>, DecodeError>) -> Result<Seq<u8>, DecodeError> {
        match result { Err(error) => Err(error), Ok(value) => Ok(value@) }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures ht_byte_vector_outcome(result) == crate::specs::ht_copy_refinement_bytes(input@),
))]
fn copy_ht_refinement_bytes(input: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0_usize;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant index <= input.len(), output.len() == index,
                forall |i: int| 0 <= i < index ==> output@[i] == input@[i],
            decreases input.len() - index,
        )
    )]
    while index < input.len() {
        output.push(*input.get(index).ok_or_else(invalid_code_block)?);
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! { assert(output@ =~= input@); }
    Ok(output)
}

#[cfg(verus_keep_ghost)]
verus! {
    spec fn ht_cleanup_prepare_outcome(result: Result<(CodeBlockPlane, CodeBlockPlane, Tier1PassStatistics, u16), DecodeError>)
        -> Result<(crate::specs::Tier1RawPlane, crate::specs::Tier1RawPlane, Tier1PassStatistics, u16), DecodeError> {
        match result {
            Err(error) => Err(error),
            Ok((plane, cleanup, statistics, bitplanes)) =>
                Ok((code_block_plane_view(&plane), code_block_plane_view(&cleanup), statistics, bitplanes)),
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures ht_cleanup_prepare_outcome(result) == crate::specs::ht_cleanup_prepare(
        code_block_plane_view(plane), ht_cleanup_segment_view(&cleanup), maximum_passes,
        maximum_u, skipped_bitplanes, crate::limits::resource_work_limit(limits)),
))]
fn prepare_ht_cleanup_symbols(
    plane: &CodeBlockPlane,
    cleanup: HtCleanupSegment<'_>,
    maximum_passes: u32,
    maximum_u: u8,
    skipped_bitplanes: u16,
    limits: &ResourceLimits,
) -> Result<(CodeBlockPlane, CodeBlockPlane, Tier1PassStatistics, u16), DecodeError> {
    let metadata = decode_ht_cleanup_segment_metadata(
        cleanup,
        plane.width(),
        plane.height(),
        maximum_u,
        limits,
    )?;
    let significant_bitplanes = ht_significant_bitplanes(maximum_passes)?;
    let decoded_bitplanes = significant_bitplanes
        .checked_sub(skipped_bitplanes)
        .ok_or_else(arithmetic_overflow)?;
    let (output, statistics) = reconstruct_ht_cleanup_plane(
        plane,
        cleanup,
        &metadata,
        maximum_u,
        decoded_bitplanes,
        limits,
    )?;
    let duplicate = output.duplicate()?;
    Ok((output, duplicate, statistics, decoded_bitplanes))
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::ht_significant_bitplanes(maximum_passes),
))]
fn ht_significant_bitplanes(maximum_passes: u32) -> Result<u16, DecodeError> {
    let rounded_passes = maximum_passes
        .checked_add(2)
        .ok_or_else(arithmetic_overflow)?;
    let significant = rounded_passes
        .checked_div(3)
        .ok_or_else(arithmetic_overflow)?;
    if significant > u32::from(u16::MAX) {
        return Err(arithmetic_overflow());
    }
    Ok(significant as u16)
}

struct HtRefinementPlaneState<'a> {
    output: CodeBlockPlane,
    readers: HtRefinementReaders<'a>,
    cleanup_significant: Vec<bool>,
    statistics: Tier1PassStatistics,
}

struct HtSignificancePatchState<'a> {
    core: HtRefinementPlaneState<'a>,
    discovery: CodeBlockPlane,
    pending: Vec<(u16, u16)>,
}

struct HtSigpropPlaneState<'a> {
    core: HtRefinementPlaneState<'a>,
    discovery: CodeBlockPlane,
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn ht_refinement_core_view(state: &HtRefinementPlaneState<'_>) -> crate::specs::HtRefinementCoreState {
        crate::specs::HtRefinementCoreState {
            output: code_block_plane_view(&state.output), readers: ht_refinement_readers_view(&state.readers),
            cleanup_significant: state.cleanup_significant@, statistics: state.statistics,
        }
    }

    closed spec fn ht_sigprop_patch_view(state: &HtSignificancePatchState<'_>) -> crate::specs::HtSignificancePatchState {
        crate::specs::HtSignificancePatchState {
            core: ht_refinement_core_view(&state.core), discovery: code_block_plane_view(&state.discovery), pending: state.pending@,
        }
    }

    spec fn ht_refinement_magnitude_view(value: Magnitude) -> (Seq<u64>, nat) {
        (crate::magnitude::magnitude_limbs(value), crate::magnitude::magnitude_value(value))
    }

    closed spec fn ht_sigprop_plane_view(state: &HtSigpropPlaneState<'_>) -> crate::specs::HtSigpropPlaneState {
        crate::specs::HtSigpropPlaneState { core: ht_refinement_core_view(&state.core), discovery: code_block_plane_view(&state.discovery) }
    }

    spec fn ht_sigprop_plane_outcome(result: (Result<(), DecodeError>, HtSigpropPlaneState<'_>))
        -> crate::specs::HtSigpropPlaneResult {
        (result.0, ht_sigprop_plane_view(&result.1))
    }
}

impl HtRefinementPlaneState<'_> {
    #[allow(
        clippy::too_many_arguments,
        reason = "The real magnitude-column loop consumes independent geometry and numeric inputs."
    )]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        requires rows <= 4,
        ensures (result, ht_refinement_core_view(final(self))) == crate::specs::ht_magref_rows(
            ht_refinement_core_view(old(self)), x, stripe_y, rows as nat, 0, crate::magnitude::magnitude_value(delta), decoded_bitplanes),
            final(self).output.width == old(self).output.width, final(self).output.height == old(self).output.height,
            final(self).cleanup_significant@ == old(self).cleanup_significant@,
            result == Ok(()) || result.is_err(),
    ))]
    fn magnitude_column(
        &mut self,
        x: u16,
        stripe_y: u16,
        rows: u16,
        delta: Magnitude,
        decoded_bitplanes: u16,
    ) -> Result<(), DecodeError> {
        let mut row = 0_u16;
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_magref_rows); }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant row <= rows <= 4,
                self.output.width == old(self).output.width, self.output.height == old(self).output.height,
                self.cleanup_significant@ == old(self).cleanup_significant@,
                crate::specs::ht_magref_rows(ht_refinement_core_view(old(self)), x, stripe_y, rows as nat, 0,
                    crate::magnitude::magnitude_value(delta), decoded_bitplanes)
                    == crate::specs::ht_magref_rows(ht_refinement_core_view(self), x, stripe_y, rows as nat, row as nat,
                        crate::magnitude::magnitude_value(delta), decoded_bitplanes),
            decreases rows - row,
        ))]
        while row < rows {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::ht_magref_rows); }
            let y = stripe_y.checked_add(row).ok_or_else(arithmetic_overflow)?;
            self.magnitude_sample(x, y, delta, decoded_bitplanes)?;
            row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_magref_rows); }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        requires rows <= 4,
        ensures (result, ht_refinement_core_view(final(self))) == crate::specs::ht_magref_columns(
            ht_refinement_core_view(old(self)), width, stripe_y, rows as nat, 0, crate::magnitude::magnitude_value(delta), decoded_bitplanes),
            final(self).output.width == old(self).output.width, final(self).output.height == old(self).output.height,
            final(self).cleanup_significant@ == old(self).cleanup_significant@,
            result == Ok(()) || result.is_err(),
    ))]
    fn magnitude_stripe(
        &mut self,
        width: u16,
        stripe_y: u16,
        rows: u16,
        delta: Magnitude,
        decoded_bitplanes: u16,
    ) -> Result<(), DecodeError> {
        let mut x = 0_u16;
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_magref_columns); }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant x <= width, rows <= 4,
                self.output.width == old(self).output.width, self.output.height == old(self).output.height,
                self.cleanup_significant@ == old(self).cleanup_significant@,
                crate::specs::ht_magref_columns(ht_refinement_core_view(old(self)), width, stripe_y, rows as nat, 0,
                    crate::magnitude::magnitude_value(delta), decoded_bitplanes)
                    == crate::specs::ht_magref_columns(ht_refinement_core_view(self), width, stripe_y, rows as nat, x as nat,
                        crate::magnitude::magnitude_value(delta), decoded_bitplanes),
            decreases width - x,
        ))]
        while x < width {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::ht_magref_columns); }
            self.magnitude_column(x, stripe_y, rows, delta, decoded_bitplanes)?;
            x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_magref_columns); }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        requires width <= 4096, height <= 4096,
        ensures (result, ht_refinement_core_view(final(self))) == crate::specs::ht_magref_stripes(
            ht_refinement_core_view(old(self)), width, height, 0, crate::magnitude::magnitude_value(delta), decoded_bitplanes),
            final(self).output.width == old(self).output.width, final(self).output.height == old(self).output.height,
            final(self).cleanup_significant@ == old(self).cleanup_significant@,
            result == Ok(()) || result.is_err(),
    ))]
    fn magnitude_pass(
        &mut self,
        width: u16,
        height: u16,
        delta: Magnitude,
        decoded_bitplanes: u16,
    ) -> Result<(), DecodeError> {
        let mut stripe_y = 0_u16;
        let stripe_extent = height.checked_add(3).ok_or_else(arithmetic_overflow)?;
        let mut stripe_passes = stripe_extent
            .checked_div(4)
            .ok_or_else(arithmetic_overflow)?;
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_magref_stripes); }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant stripe_y <= height + 3, height <= 4096, width <= 4096,
                stripe_passes == if stripe_y >= height { 0int } else { (height - stripe_y + 3) / 4 },
                self.output.width == old(self).output.width, self.output.height == old(self).output.height,
                self.cleanup_significant@ == old(self).cleanup_significant@,
                crate::specs::ht_magref_stripes(ht_refinement_core_view(old(self)), width, height, 0,
                    crate::magnitude::magnitude_value(delta), decoded_bitplanes)
                    == crate::specs::ht_magref_stripes(ht_refinement_core_view(self), width, height, stripe_y as nat,
                        crate::magnitude::magnitude_value(delta), decoded_bitplanes),
            decreases stripe_passes,
        ))]
        while stripe_y < height {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::ht_magref_stripes); }
            let rows = crate::refinement::ht_stripe_rows(height, stripe_y);
            self.magnitude_stripe(width, stripe_y, rows, delta, decoded_bitplanes)?;
            stripe_y = stripe_y.checked_add(4).ok_or_else(arithmetic_overflow)?;
            stripe_passes = stripe_passes
                .checked_sub(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_magref_stripes); }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, ht_refinement_core_view(final(self))) == crate::specs::ht_magref_sample(
            ht_refinement_core_view(old(self)), x, y, crate::magnitude::magnitude_value(delta), decoded_bitplanes),
            final(self).output.width == old(self).output.width,
            final(self).output.height == old(self).output.height,
            final(self).cleanup_significant@ == old(self).cleanup_significant@,
            result == Ok(()) || result.is_err(),
    ))]
    fn magnitude_sample(
        &mut self,
        x: u16,
        y: u16,
        delta: Magnitude,
        decoded_bitplanes: u16,
    ) -> Result<(), DecodeError> {
        self.statistics.scanned(1)?;
        let index = self.output.index(x, y)?;
        if crate::refinement::ht_magref_eligible(
            *self
                .cleanup_significant
                .get(index)
                .ok_or_else(invalid_code_block)?,
        ) {
            let bit = self.readers.read_reverse()?;
            // The final upper half-bin truncates to the existing integer magnitude.
            let adjustment = if decoded_bitplanes == 2 && bit {
                crate::magnitude::magnitude_zero()
            } else {
                delta
            };
            self.output
                .apply_ht_refinement_decision(x, y, adjustment, bit)?;
            self.statistics.decoded_symbols(1)?;
            self.statistics.refined()?;
        }
        Ok(())
    }
}

impl HtSignificancePatchState<'_> {
    #[allow(
        clippy::too_many_arguments,
        reason = "The actual discovery column uses independent stripe, magnitude and causal inputs."
    )]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        requires rows <= 4,
        ensures (result, ht_sigprop_patch_view(final(self))) == crate::specs::ht_discovery_rows(
            ht_sigprop_patch_view(old(self)), x, stripe_y, rows as nat, 0, ht_refinement_magnitude_view(magnitude), causal),
            final(self).core.output.width == old(self).core.output.width,
            final(self).core.output.height == old(self).core.output.height,
            final(self).discovery.width == old(self).discovery.width,
            final(self).discovery.height == old(self).discovery.height,
            result == Ok(()) || result.is_err(),
    ))]
    fn discover_column(
        &mut self,
        x: u16,
        stripe_y: u16,
        rows: u16,
        magnitude: Magnitude,
        causal: bool,
    ) -> Result<(), DecodeError> {
        let mut row = 0_u16;
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_discovery_rows); }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant row <= rows <= 4,
                self.core.output.width == old(self).core.output.width, self.core.output.height == old(self).core.output.height,
                self.discovery.width == old(self).discovery.width, self.discovery.height == old(self).discovery.height,
                crate::specs::ht_discovery_rows(ht_sigprop_patch_view(old(self)), x, stripe_y, rows as nat, 0,
                    ht_refinement_magnitude_view(magnitude), causal) == crate::specs::ht_discovery_rows(
                        ht_sigprop_patch_view(self), x, stripe_y, rows as nat, row as nat, ht_refinement_magnitude_view(magnitude), causal),
            decreases rows - row,
        ))]
        while row < rows {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::ht_discovery_rows); }
            let y = stripe_y.checked_add(row).ok_or_else(arithmetic_overflow)?;
            self.discover_sample(x, y, magnitude, causal)?;
            row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_discovery_rows); }
        Ok(())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "The actual four-column patch preserves its source traversal parameters."
    )]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        requires start <= end, rows <= 4,
        ensures (result, ht_sigprop_patch_view(final(self))) == crate::specs::ht_discovery_columns(
            ht_sigprop_patch_view(old(self)), end, stripe_y, rows as nat, start as nat, ht_refinement_magnitude_view(magnitude), causal),
            final(self).core.output.width == old(self).core.output.width,
            final(self).core.output.height == old(self).core.output.height,
            final(self).discovery.width == old(self).discovery.width,
            final(self).discovery.height == old(self).discovery.height,
            result == Ok(()) || result.is_err(),
    ))]
    fn discover_patch(
        &mut self,
        start: u16,
        end: u16,
        stripe_y: u16,
        rows: u16,
        magnitude: Magnitude,
        causal: bool,
    ) -> Result<(), DecodeError> {
        let mut x = start;
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_discovery_columns); }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant start <= x <= end, rows <= 4,
                self.core.output.width == old(self).core.output.width, self.core.output.height == old(self).core.output.height,
                self.discovery.width == old(self).discovery.width, self.discovery.height == old(self).discovery.height,
                crate::specs::ht_discovery_columns(ht_sigprop_patch_view(old(self)), end, stripe_y, rows as nat, start as nat,
                    ht_refinement_magnitude_view(magnitude), causal) == crate::specs::ht_discovery_columns(
                        ht_sigprop_patch_view(self), end, stripe_y, rows as nat, x as nat, ht_refinement_magnitude_view(magnitude), causal),
            decreases end - x,
        ))]
        while x < end {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::ht_discovery_columns); }
            self.discover_column(x, stripe_y, rows, magnitude, causal)?;
            x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_discovery_columns); }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, ht_sigprop_patch_view(final(self))) == crate::specs::ht_pending_signs(
            ht_sigprop_patch_view(old(self)), 0, ht_refinement_magnitude_view(magnitude)),
            final(self).core.output.width == old(self).core.output.width,
            final(self).core.output.height == old(self).core.output.height,
            final(self).discovery.width == old(self).discovery.width,
            final(self).discovery.height == old(self).discovery.height,
            result == Ok(()) || result.is_err(),
    ))]
    fn signs(&mut self, magnitude: Magnitude) -> Result<(), DecodeError> {
        let mut index = 0_usize;
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_pending_signs); }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant index <= self.pending.len(), self.pending@ == old(self).pending@,
                self.core.output.width == old(self).core.output.width, self.core.output.height == old(self).core.output.height,
                self.discovery.width == old(self).discovery.width, self.discovery.height == old(self).discovery.height,
                crate::specs::ht_pending_signs(ht_sigprop_patch_view(old(self)), 0, ht_refinement_magnitude_view(magnitude))
                    == crate::specs::ht_pending_signs(ht_sigprop_patch_view(self), index as nat, ht_refinement_magnitude_view(magnitude)),
            decreases self.pending.len() - index,
        ))]
        while index < self.pending.len() {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::ht_pending_signs); }
            self.sign_sample(index, magnitude)?;
            index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_pending_signs); }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, ht_sigprop_patch_view(final(self))) == crate::specs::ht_discover_sample(
            ht_sigprop_patch_view(old(self)), x, y, ht_refinement_magnitude_view(magnitude), vertical_causal_context),
            final(self).core.output.width == old(self).core.output.width,
            final(self).core.output.height == old(self).core.output.height,
            final(self).discovery.width == old(self).discovery.width,
            final(self).discovery.height == old(self).discovery.height,
            result == Ok(()) || result.is_err(),
    ))]
    fn discover_sample(
        &mut self,
        x: u16,
        y: u16,
        magnitude: Magnitude,
        vertical_causal_context: bool,
    ) -> Result<(), DecodeError> {
        self.core.statistics.scanned(1)?;
        if !self.discovery.coefficient(x, y)?.significant {
            let summary =
                self.discovery
                    .neighbor_summary_with_causality(x, y, vertical_causal_context)?;
            if crate::refinement::ht_sigprop_eligible(false, significant_neighbor_count(summary)) {
                let significant = self.core.readers.read_forward()?;
                self.core.statistics.decoded_symbols(1)?;
                if significant {
                    self.discovery
                        .mark_significant_magnitude(x, y, magnitude, false)?;
                    self.pending.push((x, y));
                }
            }
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, ht_sigprop_patch_view(final(self))) == crate::specs::ht_patch_sign_sample(
            ht_sigprop_patch_view(old(self)), index as nat, ht_refinement_magnitude_view(magnitude)),
            final(self).core.output.width == old(self).core.output.width,
            final(self).core.output.height == old(self).core.output.height,
            final(self).pending@ == old(self).pending@,
            result == Ok(()) || result.is_err(),
    ))]
    fn sign_sample(&mut self, index: usize, magnitude: Magnitude) -> Result<(), DecodeError> {
        let (x, y) = *self.pending.get(index).ok_or_else(invalid_code_block)?;
        let negative = self.core.readers.read_forward()?;
        self.core
            .output
            .mark_significant_magnitude(x, y, magnitude, negative)?;
        self.core.statistics.decoded_symbols(1)?;
        self.core.statistics.became_significant()?;
        Ok(())
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "The shared significance stripe keeps every original geometry and causal input."
)]
#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    requires width <= 4096, stripe_y <= 4096, rows <= 4,
    ensures ht_sigprop_plane_outcome(result) == crate::specs::ht_sigprop_patches(
        ht_sigprop_plane_view(&input), width, stripe_y, rows as nat, 0, ht_refinement_magnitude_view(magnitude), causal),
        result.1.core.output.width == input.core.output.width, result.1.core.output.height == input.core.output.height,
        result.1.discovery.width == input.discovery.width, result.1.discovery.height == input.discovery.height,
        result.0 == Ok(()) || result.0.is_err(),
))]
fn run_ht_significance_stripe(
    input: HtSigpropPlaneState<'_>,
    width: u16,
    stripe_y: u16,
    rows: u16,
    magnitude: Magnitude,
    causal: bool,
) -> (Result<(), DecodeError>, HtSigpropPlaneState<'_>) {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost start = ht_sigprop_plane_view(&input); }
    let mut phase = input;
    let mut patch_x = 0_u16;
    let extent = match width.checked_add(3) {
        Some(value) => value,
        None => return (Err(arithmetic_overflow()), phase),
    };
    let mut patch_passes = match extent.checked_div(4) {
        Some(value) => value,
        None => return (Err(arithmetic_overflow()), phase),
    };
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::ht_sigprop_patches); }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant width <= 4096, stripe_y <= 4096, rows <= 4, patch_x <= width + 3,
            start == ht_sigprop_plane_view(&input),
            patch_passes == if patch_x >= width { 0int } else { (width - patch_x + 3) / 4 },
            phase.core.output.width == start.core.output.width, phase.core.output.height == start.core.output.height,
            phase.discovery.width == start.discovery.width, phase.discovery.height == start.discovery.height,
            crate::specs::ht_sigprop_patches(start, width, stripe_y, rows as nat, 0, ht_refinement_magnitude_view(magnitude), causal)
                == crate::specs::ht_sigprop_patches(ht_sigprop_plane_view(&phase), width, stripe_y, rows as nat,
                    patch_x as nat, ht_refinement_magnitude_view(magnitude), causal),
        decreases patch_passes,
    ))]
    while patch_x < width {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_sigprop_patches); }
        let patch_end = crate::refinement::ht_patch_end(patch_x, width);
        let mut patch = HtSignificancePatchState {
            core: phase.core,
            discovery: phase.discovery,
            pending: Vec::new(),
        };
        let discovered =
            patch.discover_patch(patch_x, patch_end, stripe_y, rows, magnitude, causal);
        let result = match discovered {
            Err(error) => Err(error),
            Ok(()) => patch.signs(magnitude),
        };
        phase = HtSigpropPlaneState {
            core: patch.core,
            discovery: patch.discovery,
        };
        if let Err(error) = result {
            return (Err(error), phase);
        }
        patch_x = match patch_x.checked_add(4) {
            Some(value) => value,
            None => return (Err(arithmetic_overflow()), phase),
        };
        patch_passes = match patch_passes.checked_sub(1) {
            Some(value) => value,
            None => return (Err(arithmetic_overflow()), phase),
        };
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::ht_sigprop_patches); }
    (Ok(()), phase)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    requires width <= 4096, height <= 4096,
    ensures ht_sigprop_plane_outcome(result) == crate::specs::ht_sigprop_stripes(
        ht_sigprop_plane_view(&input), width, height, 0, ht_refinement_magnitude_view(magnitude), causal),
        result.1.core.output.width == input.core.output.width, result.1.core.output.height == input.core.output.height,
        result.1.discovery.width == input.discovery.width, result.1.discovery.height == input.discovery.height,
        result.0 == Ok(()) || result.0.is_err(),
))]
fn run_ht_significance_pass(
    input: HtSigpropPlaneState<'_>,
    width: u16,
    height: u16,
    magnitude: Magnitude,
    causal: bool,
) -> (Result<(), DecodeError>, HtSigpropPlaneState<'_>) {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost start = ht_sigprop_plane_view(&input); }
    let mut phase = input;
    let mut stripe_y = 0_u16;
    let extent = match height.checked_add(3) {
        Some(value) => value,
        None => return (Err(arithmetic_overflow()), phase),
    };
    let mut stripe_passes = match extent.checked_div(4) {
        Some(value) => value,
        None => return (Err(arithmetic_overflow()), phase),
    };
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::ht_sigprop_stripes); }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant width <= 4096, height <= 4096, stripe_y <= height + 3,
            start == ht_sigprop_plane_view(&input),
            stripe_passes == if stripe_y >= height { 0int } else { (height - stripe_y + 3) / 4 },
            phase.core.output.width == start.core.output.width, phase.core.output.height == start.core.output.height,
            phase.discovery.width == start.discovery.width, phase.discovery.height == start.discovery.height,
            crate::specs::ht_sigprop_stripes(start, width, height, 0, ht_refinement_magnitude_view(magnitude), causal)
                == crate::specs::ht_sigprop_stripes(ht_sigprop_plane_view(&phase), width, height,
                    stripe_y as nat, ht_refinement_magnitude_view(magnitude), causal),
        decreases stripe_passes,
    ))]
    while stripe_y < height {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_sigprop_stripes); }
        let rows = crate::refinement::ht_stripe_rows(height, stripe_y);
        let (result, next) =
            run_ht_significance_stripe(phase, width, stripe_y, rows, magnitude, causal);
        phase = next;
        if let Err(error) = result {
            return (Err(error), phase);
        }
        stripe_y = match stripe_y.checked_add(4) {
            Some(value) => value,
            None => return (Err(arithmetic_overflow()), phase),
        };
        stripe_passes = match stripe_passes.checked_sub(1) {
            Some(value) => value,
            None => return (Err(arithmetic_overflow()), phase),
        };
    }
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::ht_sigprop_stripes); }
    (Ok(()), phase)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures ht_plane_outcome(result) == crate::specs::ht_refinement_plane(
        code_block_plane_view(cleanup_plane), ht_refinement_segment_view(&segment),
        refinement_passes, decoded_bitplanes, vertical_causal_context, crate::limits::resource_work_limit(limits)),
))]
fn decode_ht_refinement_plane(
    cleanup_plane: &CodeBlockPlane,
    segment: HtRefinementSegment<'_>,
    refinement_passes: u8,
    decoded_bitplanes: u16,
    vertical_causal_context: bool,
    limits: &ResourceLimits,
) -> Result<(CodeBlockPlane, Tier1PassStatistics), DecodeError> {
    if refinement_passes == 0 || refinement_passes > 2 || decoded_bitplanes < 2 {
        return Err(invalid_code_block());
    }
    let output = cleanup_plane.duplicate()?;
    let readers = ht_refinement_readers_from_inputs(
        segment.significance_propagation().bytes(),
        segment.magnitude_refinement().bytes(),
        segment.encoded().len(),
        refinement_passes == 2,
        limits.maximum_work_units(),
    )?;
    let cleanup_significant = ht_collect_cleanup_significance(cleanup_plane.coefficients());
    let mut state = HtRefinementPlaneState {
        output,
        readers,
        cleanup_significant,
        statistics: ht_statistics_zero(),
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(state.output.width > 0 && state.output.height > 0);
        assert(state.output.width as nat * state.output.height as nat <= 4096);
        let width = state.output.width as nat;
        let height = state.output.height as nat;
        assert(width <= 4096 && height <= 4096) by(nonlinear_arith)
            requires width > 0, height > 0, width * height <= 4096;
    }

    if refinement_passes == 2 {
        let delta = ht_refinement_delta(decoded_bitplanes)?;
        let output_height = state.output.height();
        let output_width = state.output.width();
        state.magnitude_pass(output_width, output_height, delta, decoded_bitplanes)?;
    }

    let significance_magnitude = ht_significance_magnitude(decoded_bitplanes)?;
    let significance_plane = state.output.duplicate()?;
    let output_height = state.output.height();
    let output_width = state.output.width();
    let (result, phase) = run_ht_significance_pass(
        HtSigpropPlaneState {
            core: state,
            discovery: significance_plane,
        },
        output_width,
        output_height,
        significance_magnitude,
        vertical_causal_context,
    );
    result?;
    phase.core.readers.finish(segment.encoded())?;
    Ok((phase.core.output, phase.core.statistics))
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodeBlockSegmentAccounting {
    maximum_passes: u32,
    maximum_bytes: u64,
    total_passes: u32,
    total_bytes: u64,
    segments: u32,
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CoefficientState {
    /// Decoded magnitude bits; unfinished legacy passes retain a bit prefix.
    pub magnitude: u64,
    magnitude_high: [u64; 5],
    legacy_bitplane: Option<u16>,
    pub negative: bool,
    pub significant: bool,
    pub refined: bool,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn coefficient_raw_view(value: CoefficientState) -> crate::specs::Tier1RawCoefficient {
        crate::specs::Tier1RawCoefficient {
            limbs: Seq::empty().push(value.magnitude) + value.magnitude_high@,
            legacy_bitplane: value.legacy_bitplane, negative: value.negative,
            significant: value.significant, refined: value.refined,
        }
    }

    pub open spec fn coefficient_raw_views(values: Seq<CoefficientState>) -> Seq<crate::specs::Tier1RawCoefficient> {
        Seq::new(values.len(), |i: int| coefficient_raw_view(values[i]))
    }

    pub open spec fn coefficient_state_view(value: &CoefficientState) -> crate::specs::Tier1RawCoefficient {
        coefficient_raw_view(*value)
    }

    pub open spec fn coefficient_slice_view(values: &[CoefficientState]) -> Seq<crate::specs::Tier1RawCoefficient> {
        coefficient_raw_views(values@)
    }

    pub(crate) open spec fn coefficient_signed_outcome(result: Result<SignedMagnitude, DecodeError>)
        -> Result<(Seq<u64>, nat, bool), DecodeError>
    {
        match result {
            Err(error) => Err(error),
            Ok(value) => Ok((crate::magnitude::magnitude_limbs(value.magnitude),
                crate::magnitude::magnitude_value(value.magnitude), value.negative)),
        }
    }

    pub open spec fn coefficient_raw_outcome(result: Result<CoefficientState, DecodeError>)
        -> Result<crate::specs::Tier1RawCoefficient, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(value) => Ok(coefficient_raw_view(value)) }
    }

    pub proof fn coefficient_raw_shape(value: CoefficientState)
        ensures coefficient_raw_view(value).limbs.len() == 6,
    {}
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures coefficient_raw_outcome(result) == crate::specs::tier1_coefficient_new_outcome(
        crate::magnitude::magnitude_limbs(magnitude), negative, significant, refined),
))]
fn coefficient_from_magnitude(
    magnitude: Magnitude,
    negative: bool,
    significant: bool,
    refined: bool,
) -> Result<CoefficientState, DecodeError> {
    let mut output = CoefficientState {
        magnitude: magnitude.low_u64(),
        magnitude_high: [0; 5],
        legacy_bitplane: None,
        negative,
        significant,
        refined,
    };
    output.set_magnitude(magnitude)?;
    Ok(output)
}

impl CoefficientState {
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures coefficient_signed_outcome(result)
                == crate::specs::tier1_raw_signed_coefficient(coefficient_raw_view(*self)),
        )
    )]
    pub(crate) fn signed_magnitude(&self) -> Result<SignedMagnitude, DecodeError> {
        let mut magnitude = if self.significant {
            self.wide_magnitude()
        } else {
            crate::magnitude::magnitude_zero()
        };
        if let (true, Some(bitplane)) = (self.significant, self.legacy_bitplane) {
            let midpoint = if bitplane <= 64 {
                crate::magnitude::magnitude_from_u64(crate::refinement::tier1_midpoint_delta(
                    bitplane,
                ))
            } else {
                crate::magnitude::magnitude_single_bit(bitplane - 1)?
            };
            magnitude = magnitude.checked_add(midpoint)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::magnitude::representation_proofs::magnitude_limbs_shape(magnitude);
            crate::proofs::production_impl::production_tier1_value_limbs(crate::magnitude::magnitude_limbs(magnitude));
        }
        Ok(SignedMagnitude {
            magnitude,
            negative: self.significant && self.negative,
        })
    }
    pub fn magnitude_limb(&self, index: usize) -> Option<u64> {
        if index == 0 {
            Some(self.magnitude)
        } else {
            let high_index = index.checked_sub(1)?;
            if high_index < self.magnitude_high.len() {
                Some(self.magnitude_high[high_index])
            } else {
                None
            }
        }
    }

    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(result =>
            ensures crate::magnitude::magnitude_limbs(result) == crate::specs::tier1_coefficient_limbs(coefficient_raw_view(*self)),
                crate::magnitude::magnitude_limbs(result) == coefficient_raw_view(*self).limbs,
                crate::magnitude::magnitude_value(result) == crate::specs::part1_magnitude_value(coefficient_raw_view(*self).limbs),
                crate::magnitude::magnitude_value(result) < crate::specs::part1_magnitude_capacity(),
        )
    )]
    pub fn wide_magnitude(&self) -> Magnitude {
        let mut limbs = [0_u64; 6];
        limbs[0] = self.magnitude;
        let mut index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.magnitude_high.len(),
                    limbs[0] == self.magnitude,
                    forall |i: int| 0 <= i < index ==> limbs@[i + 1] == self.magnitude_high@[i],
                decreases self.magnitude_high.len() - index,
            )
        )]
        while index < self.magnitude_high.len() {
            limbs[index + 1] = self.magnitude_high[index];
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { assert(limbs@ =~= coefficient_raw_view(*self).limbs); }
        crate::magnitude::magnitude_from_limbs(limbs)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, coefficient_state_view(final(self))) == crate::specs::tier1_set_magnitude(
                coefficient_state_view(old(self)), crate::magnitude::magnitude_limbs(magnitude)),
            result == Ok(()),
            coefficient_raw_view(*final(self)) == (crate::specs::Tier1RawCoefficient {
                limbs: crate::magnitude::magnitude_limbs(magnitude), ..coefficient_raw_view(*old(self))
            }),
    ))]
    fn set_magnitude(&mut self, magnitude: Magnitude) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { crate::magnitude::representation_proofs::magnitude_limbs_shape(magnitude); }
        self.magnitude = magnitude.low_u64();
        let mut index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.magnitude_high.len(),
                    crate::magnitude::magnitude_limbs(magnitude).len() == 6,
                    self.magnitude == crate::magnitude::magnitude_limbs(magnitude)[0],
                    self.legacy_bitplane == old(self).legacy_bitplane,
                    self.negative == old(self).negative, self.significant == old(self).significant,
                    self.refined == old(self).refined,
                    forall |i: int| 0 <= i < index ==> self.magnitude_high@[i]
                        == crate::magnitude::magnitude_limbs(magnitude)[i + 1],
                decreases self.magnitude_high.len() - index,
            )
        )]
        while index < self.magnitude_high.len() {
            self.magnitude_high[index] =
                magnitude.limb(index + 1).ok_or_else(arithmetic_overflow)?;
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { assert(coefficient_raw_view(*self).limbs =~= crate::magnitude::magnitude_limbs(magnitude)); }
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct NeighborSummary {
    pub horizontal_significant: u8,
    pub vertical_significant: u8,
    pub diagonal_significant: u8,
    pub horizontal_sign_balance: i8,
    pub vertical_sign_balance: i8,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub enum SubbandOrientation {
    Ll,
    Lh,
    Hl,
    Hh,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct SignContext {
    pub context: u8,
    pub predicted_negative: bool,
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_legacy_zero_context(summary, orientation),
))]
pub fn zero_coding_context(summary: NeighborSummary, orientation: SubbandOrientation) -> u8 {
    let (horizontal, vertical) = if matches!(orientation, SubbandOrientation::Hl) {
        (summary.vertical_significant, summary.horizontal_significant)
    } else {
        (summary.horizontal_significant, summary.vertical_significant)
    };
    let diagonal = summary.diagonal_significant;
    crate::refinement::tier1_zero_context(
        u16::from(horizontal),
        u16::from(vertical),
        u16::from(diagonal),
        matches!(orientation, SubbandOrientation::Hh),
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == if value < 0 { -1i8 } else if value > 0 { 1i8 } else { 0i8 },
))]
const fn signum_i8(value: i8) -> i8 {
    if value < 0 {
        -1
    } else if value > 0 {
        1
    } else {
        0
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_legacy_sign_context(summary),
))]
pub fn sign_context(summary: NeighborSummary) -> SignContext {
    let mut horizontal = signum_i8(summary.horizontal_sign_balance);
    let mut vertical = signum_i8(summary.vertical_sign_balance);
    if horizontal < 0 {
        horizontal = 1;
        vertical = if vertical < 0 {
            1
        } else if vertical > 0 {
            -1
        } else {
            0
        };
    }

    let offset = crate::refinement::tier1_sign_offset(horizontal, vertical);
    let predicted_negative = crate::refinement::tier1_predicted_negative(
        summary.horizontal_sign_balance,
        summary.vertical_sign_balance,
    );
    SignContext {
        context: 9 + offset,
        predicted_negative,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_legacy_refinement_context(coefficient_raw_view(coefficient), summary),
))]
pub fn magnitude_refinement_context(coefficient: CoefficientState, summary: NeighborSummary) -> u8 {
    let neighbors = u16::from(summary.horizontal_significant)
        + u16::from(summary.vertical_significant)
        + u16::from(summary.diagonal_significant);
    crate::refinement::tier1_refinement_context(coefficient.refined, neighbors)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, crate::mq::mq_decoder_view(final(decoder))) == (Ok(()),
        crate::specs::part1_legacy_initialized(crate::mq::mq_decoder_view(old(decoder)))),
))]
pub fn initialize_tier1_contexts(decoder: &mut MqDecoder<'_>) -> Result<(), DecodeError> {
    decoder.reset_contexts();
    decoder.set_context(0, 4, false)?;
    decoder.set_context(RUN_LENGTH_CONTEXT, 3, false)?;
    decoder.set_context(UNIFORM_CONTEXT, 46, false)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(crate::mq::mq_decoder_view(decoder).contexts =~=
            crate::specs::part1_legacy_initialized(crate::mq::mq_decoder_view(old(decoder))).contexts);
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, crate::mq::mq_decoder_view(final(decoder))) == (Ok(()),
        if style.reset_contexts {
            crate::specs::part1_legacy_initialized(crate::mq::mq_decoder_view(old(decoder)))
        } else { crate::mq::mq_decoder_view(old(decoder)) }),
))]
pub fn reset_tier1_contexts_after_pass(
    decoder: &mut MqDecoder<'_>,
    style: Tier1CodingStyle,
) -> Result<(), DecodeError> {
    if style.reset_contexts {
        initialize_tier1_contexts(decoder)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct SignificanceStep {
    pub zero_coding_context: u8,
    pub sign_context: Option<u8>,
    pub became_significant: bool,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Tier1BitplaneState {
    width: u16,
    height: u16,
    bitplane: u16,
    significant_before: Vec<bool>,
    significance_visited: Vec<bool>,
    significance_propagation_complete: bool,
    magnitude_refinement_complete: bool,
    cleanup_complete: bool,
}

impl Clone for Tier1BitplaneState {
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures legacy_bitplane_view(&result) == legacy_bitplane_view(self),
    ))]
    fn clone(&self) -> Self {
        Self {
            width: self.width,
            height: self.height,
            bitplane: self.bitplane,
            significant_before: self.significant_before.clone(),
            significance_visited: self.significance_visited.clone(),
            significance_propagation_complete: self.significance_propagation_complete,
            magnitude_refinement_complete: self.magnitude_refinement_complete,
            cleanup_complete: self.cleanup_complete,
        }
    }
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn legacy_bitplane_view(state: &Tier1BitplaneState) -> crate::specs::Part1LegacyBitplane {
        crate::specs::Part1LegacyBitplane {
            width: state.width, height: state.height, bitplane: state.bitplane,
            significant_before: state.significant_before@, significance_visited: state.significance_visited@,
            significance_propagation_complete: state.significance_propagation_complete,
            magnitude_refinement_complete: state.magnitude_refinement_complete,
            cleanup_complete: state.cleanup_complete,
        }
    }

    pub open spec fn legacy_bitplane_outcome(result: Result<Tier1BitplaneState, DecodeError>)
        -> Result<crate::specs::Part1LegacyBitplane, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(state) => Ok(legacy_bitplane_view(&state)) }
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Tier1PassStatistics {
    pub coefficients_scanned: u32,
    pub coding_symbols: u32,
    pub newly_significant: u32,
    pub refined_coefficients: u32,
    pub run_mode_columns: u32,
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Tier1SegmentDecode {
    pub passes: u32,
    pub bytes: u32,
    pub statistics: Tier1PassStatistics,
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug))]
struct LegacySegmentState {
    plane: CodeBlockPlane,
    next_pass: u32,
    bitplane: u16,
    bitplane_state: Option<Tier1BitplaneState>,
    mq_context_state: Option<MqContextState>,
    reset_contexts_before_mq: bool,
    decoded_segments: u32,
    decoded_bytes: u64,
    input: Vec<u8>,
    passes: u32,
}

impl Clone for LegacySegmentState {
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures legacy_segment_view(&result) == legacy_segment_view(self),
    ))]
    fn clone(&self) -> Self {
        Self {
            plane: self.plane.clone(),
            next_pass: self.next_pass,
            bitplane: self.bitplane,
            bitplane_state: self.bitplane_state.clone(),
            mq_context_state: self.mq_context_state,
            reset_contexts_before_mq: self.reset_contexts_before_mq,
            decoded_segments: self.decoded_segments,
            decoded_bytes: self.decoded_bytes,
            input: self.input.clone(),
            passes: self.passes,
        }
    }
}

impl LegacySegmentState {
    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures legacy_segment_view(final(self)) == crate::specs::part1_legacy_append(
            legacy_segment_view(old(self)), input@, passes),
    ))]
    fn append(&mut self, input: &[u8], passes: u32) {
        self.input.extend_from_slice(input);
        self.passes = passes;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(self.input@ =~= legacy_segment_view(old(self)).input + input@);
        }
    }
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug))]
pub struct Tier1CodeBlockDecoder {
    plane: CodeBlockPlane,
    orientation: SubbandOrientation,
    style: Tier1CodingStyle,
    maximum_passes: u32,
    next_pass: u32,
    bitplane: u16,
    zero_bitplanes: Option<u16>,
    bitplane_state: Option<Tier1BitplaneState>,
    mq_context_state: Option<MqContextState>,
    reset_contexts_before_mq: bool,
    decoded_segments: u32,
    decoded_bytes: u64,
    legacy_segment: Option<LegacySegmentState>,
    legacy_decode_work: u64,
    ht_cleanup_plane: Option<CodeBlockPlane>,
    ht_refinement_data: Vec<u8>,
    ht_refinement_passes: u8,
    ht_decoded_bitplanes: u16,
    ht_current_set_has_cleanup: bool,
    ht_sets_seen: u16,
    allow_multiple_ht_sets: bool,
    ht_magnitude_bound: u8,
}

impl Clone for Tier1CodeBlockDecoder {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures tier1_decoder_state(&result) == tier1_decoder_state(self),
    ))]
    fn clone(&self) -> Self {
        Self {
            plane: self.plane.clone(),
            orientation: self.orientation,
            style: self.style,
            maximum_passes: self.maximum_passes,
            next_pass: self.next_pass,
            bitplane: self.bitplane,
            zero_bitplanes: self.zero_bitplanes,
            bitplane_state: self.bitplane_state.clone(),
            mq_context_state: self.mq_context_state,
            reset_contexts_before_mq: self.reset_contexts_before_mq,
            decoded_segments: self.decoded_segments,
            decoded_bytes: self.decoded_bytes,
            legacy_segment: self.legacy_segment.clone(),
            legacy_decode_work: self.legacy_decode_work,
            ht_cleanup_plane: self.ht_cleanup_plane.clone(),
            ht_refinement_data: self.ht_refinement_data.clone(),
            ht_refinement_passes: self.ht_refinement_passes,
            ht_decoded_bitplanes: self.ht_decoded_bitplanes,
            ht_current_set_has_cleanup: self.ht_current_set_has_cleanup,
            ht_sets_seen: self.ht_sets_seen,
            allow_multiple_ht_sets: self.allow_multiple_ht_sets,
            ht_magnitude_bound: self.ht_magnitude_bound,
        }
    }
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn legacy_segment_view(legacy: &LegacySegmentState) -> crate::specs::Tier1LegacyFrame {
        crate::specs::Tier1LegacyFrame {
            plane: code_block_plane_view(&legacy.plane), next_pass: legacy.next_pass, bitplane: legacy.bitplane,
            bitplane_state: match legacy.bitplane_state {
                None => None, Some(state) => Some(legacy_bitplane_view(&state)),
            },
            mq_context_state: match legacy.mq_context_state {
                None => None, Some(state) => Some(crate::mq::mq_context_state_view(state)),
            },
            reset_contexts_before_mq: legacy.reset_contexts_before_mq,
            decoded_segments: legacy.decoded_segments, decoded_bytes: legacy.decoded_bytes,
            input: legacy.input@, passes: legacy.passes,
        }
    }

    pub closed spec fn tier1_decoder_state(decoder: &Tier1CodeBlockDecoder) -> crate::specs::Tier1DecoderState {
        crate::specs::Tier1DecoderState {
            plane: code_block_plane_view(&decoder.plane), orientation: decoder.orientation,
            style: decoder.style, maximum_passes: decoder.maximum_passes, next_pass: decoder.next_pass,
            bitplane: decoder.bitplane, zero_bitplanes: decoder.zero_bitplanes,
            bitplane_state: match decoder.bitplane_state {
                None => None, Some(state) => Some(legacy_bitplane_view(&state)),
            },
            mq_context_state: match decoder.mq_context_state {
                None => None, Some(state) => Some(crate::mq::mq_context_state_view(state)),
            },
            reset_contexts_before_mq: decoder.reset_contexts_before_mq,
            decoded_segments: decoder.decoded_segments, decoded_bytes: decoder.decoded_bytes,
            legacy_segment: match decoder.legacy_segment {
                None => None,
                Some(legacy) => Some(legacy_segment_view(&legacy)),
            },
            legacy_decode_work: decoder.legacy_decode_work,
            ht_cleanup_plane: match decoder.ht_cleanup_plane { None => None, Some(plane) => Some(code_block_plane_view(&plane)) },
            ht_refinement_data: decoder.ht_refinement_data@, ht_refinement_passes: decoder.ht_refinement_passes,
            ht_decoded_bitplanes: decoder.ht_decoded_bitplanes,
            ht_current_set_has_cleanup: decoder.ht_current_set_has_cleanup, ht_sets_seen: decoder.ht_sets_seen,
            allow_multiple_ht_sets: decoder.allow_multiple_ht_sets, ht_magnitude_bound: decoder.ht_magnitude_bound,
        }
    }

    pub closed spec fn tier1_max_ht_magnitude_bits() -> u8 { MAX_HT_MAGNITUDE_BITS }

    pub open spec fn tier1_initial_decoder_outcome(result: Result<Tier1CodeBlockDecoder, DecodeError>)
        -> Result<crate::specs::Tier1DecoderState, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(decoder) => Ok(tier1_decoder_state(&decoder)) }
    }
}

impl Tier1PassStatistics {
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, *final(self)) == crate::specs::tier1_statistics_increment(*old(self), 0, count),
    ))]
    fn scanned(&mut self, count: u32) -> Result<(), DecodeError> {
        self.coefficients_scanned = self
            .coefficients_scanned
            .checked_add(count)
            .ok_or_else(arithmetic_overflow)?;
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, *final(self)) == crate::specs::tier1_statistics_increment(*old(self), 1, count),
    ))]
    fn decoded_symbols(&mut self, count: u32) -> Result<(), DecodeError> {
        self.coding_symbols = self
            .coding_symbols
            .checked_add(count)
            .ok_or_else(arithmetic_overflow)?;
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, *final(self)) == crate::specs::tier1_statistics_increment(*old(self), 2, 1),
    ))]
    fn became_significant(&mut self) -> Result<(), DecodeError> {
        self.newly_significant = self
            .newly_significant
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, *final(self)) == crate::specs::tier1_statistics_increment(*old(self), 3, 1),
    ))]
    fn refined(&mut self) -> Result<(), DecodeError> {
        self.refined_coefficients = self
            .refined_coefficients
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, *final(self)) == crate::specs::tier1_statistics_increment(*old(self), 4, 1),
    ))]
    fn used_run_mode(&mut self) -> Result<(), DecodeError> {
        self.run_mode_columns = self
            .run_mode_columns
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, *final(self)) == crate::specs::tier1_statistics_merge(*old(self), other),
    ))]
    fn merge(&mut self, other: Self) -> Result<(), DecodeError> {
        self.scanned(other.coefficients_scanned)?;
        self.decoded_symbols(other.coding_symbols)?;
        self.newly_significant = self
            .newly_significant
            .checked_add(other.newly_significant)
            .ok_or_else(arithmetic_overflow)?;
        self.refined_coefficients = self
            .refined_coefficients
            .checked_add(other.refined_coefficients)
            .ok_or_else(arithmetic_overflow)?;
        self.run_mode_columns = self
            .run_mode_columns
            .checked_add(other.run_mode_columns)
            .ok_or_else(arithmetic_overflow)?;
        Ok(())
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures tier1_initial_decoder_outcome(result) == crate::specs::part2_tier1_initial_decoder(
        width, height, significant_bitplanes, orientation, style,
    ),
))]
pub(crate) fn tier1_decoder_from_dimensions(
    width: u16,
    height: u16,
    significant_bitplanes: u16,
    orientation: SubbandOrientation,
    style: Tier1CodingStyle,
) -> Result<Tier1CodeBlockDecoder, DecodeError> {
    if significant_bitplanes == 0 || significant_bitplanes > MAX_TIER1_BITPLANES {
        return Err(invalid_code_block());
    }
    let bitplane = significant_bitplanes
        .checked_sub(1)
        .ok_or_else(arithmetic_overflow)?;
    let maximum_passes = u32::from(significant_bitplanes)
        .checked_mul(3)
        .ok_or_else(arithmetic_overflow)?;
    let maximum_passes = maximum_passes
        .checked_sub(2)
        .ok_or_else(arithmetic_overflow)?;
    Ok(Tier1CodeBlockDecoder {
        plane: code_block_plane_from_dimensions(width, height)?,
        orientation,
        style,
        maximum_passes,
        next_pass: 0,
        bitplane,
        zero_bitplanes: None,
        bitplane_state: None,
        mq_context_state: None,
        reset_contexts_before_mq: false,
        decoded_segments: 0,
        decoded_bytes: 0,
        legacy_segment: None,
        legacy_decode_work: 0,
        ht_cleanup_plane: None,
        ht_refinement_data: Vec::new(),
        ht_refinement_passes: 0,
        ht_decoded_bitplanes: 0,
        ht_current_set_has_cleanup: false,
        ht_sets_seen: 0,
        allow_multiple_ht_sets: false,
        ht_magnitude_bound: MAX_HT_MAGNITUDE_BITS,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures tier1_initial_decoder_outcome(result) == crate::specs::part2_tier1_configured_decoder(
        width, height, significant_bitplanes, orientation, style, allow_multiple_ht_sets, ht_magnitude_bound,
    ),
))]
pub(crate) fn tier1_configured_decoder(
    width: u16,
    height: u16,
    significant_bitplanes: u16,
    orientation: SubbandOrientation,
    style: Tier1CodingStyle,
    allow_multiple_ht_sets: bool,
    ht_magnitude_bound: u8,
) -> Result<Tier1CodeBlockDecoder, DecodeError> {
    let mut decoder =
        tier1_decoder_from_dimensions(width, height, significant_bitplanes, orientation, style)?;
    decoder.set_multiple_ht_sets_allowed(allow_multiple_ht_sets);
    decoder.set_ht_magnitude_bound(ht_magnitude_bound)?;
    Ok(decoder)
}

impl Tier1CodeBlockDecoder {
    pub fn new(
        width: u16,
        height: u16,
        significant_bitplanes: u16,
        orientation: SubbandOrientation,
        style: Tier1CodingStyle,
    ) -> Result<Self, DecodeError> {
        tier1_decoder_from_dimensions(width, height, significant_bitplanes, orientation, style)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures code_block_plane_view(result) == tier1_decoder_plane_view(self),
    ))]
    pub const fn plane(&self) -> &CodeBlockPlane {
        &self.plane
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == tier1_decoder_state(self).style.block_coding,
    ))]
    pub const fn block_coding(&self) -> Tier1BlockCoding {
        self.style.block_coding
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == (!tier1_decoder_state(self).style.force_high_throughput || high_throughput),
    ))]
    pub const fn accepts_high_throughput_route(&self, high_throughput: bool) -> bool {
        !self.style.force_high_throughput || high_throughput
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == tier1_decoder_state(self).next_pass,))]
    pub const fn next_pass(&self) -> u32 {
        self.next_pass
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == tier1_decoder_state(self).maximum_passes,))]
    pub const fn maximum_passes(&self) -> u32 {
        self.maximum_passes
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == tier1_decoder_state(self).decoded_segments,))]
    pub const fn decoded_segments(&self) -> u32 {
        self.decoded_segments
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == tier1_decoder_state(self).decoded_bytes,))]
    pub const fn decoded_bytes(&self) -> u64 {
        self.decoded_bytes
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::part1_legacy_zero_bitplanes(
            tier1_decoder_state(old(self)), zero_bitplanes),
    ))]
    pub(crate) fn set_zero_bitplanes(&mut self, zero_bitplanes: u16) -> Result<(), DecodeError> {
        if let Some(previous) = self.zero_bitplanes {
            return if previous == zero_bitplanes {
                Ok(())
            } else {
                Err(invalid_code_block())
            };
        }
        if self.next_pass != 0 || self.decoded_segments != 0 || zero_bitplanes > self.bitplane {
            return Err(invalid_code_block());
        }
        let bitplane = self
            .bitplane
            .checked_sub(zero_bitplanes)
            .ok_or_else(invalid_code_block)?;
        let maximum_passes = legacy_maximum_passes(bitplane)?;
        self.bitplane = bitplane;
        self.maximum_passes = maximum_passes;
        self.zero_bitplanes = Some(zero_bitplanes);
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(_result =>
        ensures ((), tier1_decoder_state(final(self))) == crate::specs::part2_tier1_allow_sets(tier1_decoder_state(old(self)), allowed),
    ))]
    pub fn set_multiple_ht_sets_allowed(&mut self, allowed: bool) {
        self.allow_multiple_ht_sets = allowed;
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self)))
            == crate::specs::part2_tier1_magnitude_bound(tier1_decoder_state(old(self)), bound),
    ))]
    pub fn set_ht_magnitude_bound(&mut self, bound: u8) -> Result<(), DecodeError> {
        if bound == 0 || bound > MAX_HT_MAGNITUDE_BITS {
            return Err(invalid_code_block());
        }
        self.ht_magnitude_bound = bound;
        Ok(())
    }

    pub const fn is_complete(&self) -> bool {
        if self.active_high_throughput() {
            self.next_pass > 0
        } else {
            self.next_pass == self.maximum_passes
        }
    }

    pub const fn active_high_throughput(&self) -> bool {
        matches!(self.style.block_coding, Tier1BlockCoding::HighThroughput)
            || (matches!(self.style.block_coding, Tier1BlockCoding::Mixed)
                && self.ht_cleanup_plane.is_some())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::part1_legacy_contribution_admission(
            tier1_decoder_state(old(self)), input@, passes,
            crate::limits::resource_code_block_byte_limit(limits), crate::limits::resource_work_limit(limits)),
    ))]
    fn admit_legacy_contribution(
        &mut self,
        input: &[u8],
        passes: u8,
        limits: &ResourceLimits,
    ) -> Result<(u32, u32, u32), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_contribution_admission); }
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost before = tier1_decoder_state(self); }
        if passes == 0 {
            return Err(invalid_code_block());
        }
        let next_bytes = self
            .decoded_bytes
            .checked_add(input.len() as u64)
            .ok_or_else(arithmetic_overflow)?;
        if next_bytes > limits.maximum_code_block_bytes() {
            return Err(crate::mq::mq_limit(
                Resource::CodeBlockBytes,
                limits.maximum_code_block_bytes(),
                next_bytes,
            ));
        }
        let replay_passes = match &self.legacy_segment {
            Some(segment) => segment.passes,
            None => 0,
        }
        .checked_add(u32::from(passes))
        .ok_or_else(arithmetic_overflow)?;
        let work = legacy_contribution_work(
            self.plane.coefficients().len(),
            replay_passes,
            next_bytes,
            self.legacy_decode_work,
            limits.maximum_work_units(),
        )?;
        #[cfg(verus_keep_ghost)]
        proof! { assert(tier1_decoder_state(self) == before); }
        self.legacy_decode_work = work;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(tier1_decoder_state(self) == (crate::specs::Tier1DecoderState { legacy_decode_work: work, ..before }));
        }
        let next_pass = self
            .next_pass
            .checked_add(u32::from(passes))
            .ok_or_else(arithmetic_overflow)?;
        let last_pass = next_pass.checked_sub(1).ok_or_else(invalid_code_block)?;
        Ok((replay_passes, next_pass, last_pass))
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::part1_legacy_contribution(
            tier1_decoder_state(old(self)), input@, passes,
            crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
            crate::limits::resource_input_byte_limit(limits), crate::limits::resource_work_limit(limits)),
    ))]
    pub(crate) fn decode_legacy_contribution(
        &mut self,
        input: &[u8],
        passes: u8,
        limits: &ResourceLimits,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_contribution); }
        let (replay_passes, next_pass, last_pass) =
            self.admit_legacy_contribution(input, passes, limits)?;
        if self.legacy_segment.is_none()
            && (next_pass == self.maximum_passes
                || self.style.pass_plan(last_pass).terminates_segment)
        {
            self.decode_legacy_segment(input, u32::from(passes), limits)?;
            return Ok(());
        }
        self.decode_legacy_replay(input, replay_passes, limits)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::part1_legacy_replay(
            tier1_decoder_state(old(self)), input@, replay_passes,
            crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
            crate::limits::resource_input_byte_limit(limits), crate::limits::resource_work_limit(limits)),
    ))]
    fn decode_legacy_replay(
        &mut self,
        input: &[u8],
        replay_passes: u32,
        limits: &ResourceLimits,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_replay); }
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost before_replay = tier1_decoder_state(self);
            let ghost replay_model = match before_replay.legacy_segment {
                Some(segment) => segment,
                None => crate::specs::part1_legacy_snapshot(crate::specs::Tier1DecoderState { legacy_segment: None, ..before_replay }),
            };
        }
        let mut segment = match self.legacy_segment.take() {
            Some(segment) => segment,
            None => self.legacy_segment_snapshot(),
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(tier1_decoder_state(self) == (crate::specs::Tier1DecoderState { legacy_segment: None, ..before_replay }));
            assert(legacy_segment_view(&segment) == replay_model);
        }
        segment.append(input, replay_passes);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(legacy_segment_view(&segment) == (crate::specs::Tier1LegacyFrame {
                input: replay_model.input + input@, passes: replay_passes, ..replay_model
            }));
        }
        self.restore_legacy_segment(&segment);
        // Layer boundaries can truncate an unterminated arithmetic segment.
        // Replay its bounded prefix so virtual end-fill is never resumed as real input.
        self.decode_legacy_segment(&segment.input, segment.passes, limits)?;
        self.commit_legacy_replay(segment)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::part1_legacy_replay_commit(
            tier1_decoder_state(old(self)), legacy_segment_view(&segment)),
    ))]
    fn commit_legacy_replay(&mut self, segment: LegacySegmentState) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_replay_commit); }
        let last_pass = self
            .next_pass
            .checked_sub(1)
            .ok_or_else(invalid_code_block)?;
        if self.next_pass < self.maximum_passes
            && !self.style.pass_plan(last_pass).terminates_segment
        {
            self.legacy_segment = Some(segment);
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures legacy_segment_view(&result) == crate::specs::part1_legacy_snapshot(tier1_decoder_state(self)),
    ))]
    fn legacy_segment_snapshot(&self) -> LegacySegmentState {
        LegacySegmentState {
            plane: self.plane.clone(),
            next_pass: self.next_pass,
            bitplane: self.bitplane,
            bitplane_state: self.bitplane_state.clone(),
            mq_context_state: self.mq_context_state,
            reset_contexts_before_mq: self.reset_contexts_before_mq,
            decoded_segments: self.decoded_segments,
            decoded_bytes: self.decoded_bytes,
            input: Vec::new(),
            passes: 0,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures tier1_decoder_state(final(self)) == crate::specs::part1_legacy_restore(
            tier1_decoder_state(old(self)), legacy_segment_view(segment)),
    ))]
    fn restore_legacy_segment(&mut self, segment: &LegacySegmentState) {
        self.plane = segment.plane.clone();
        self.next_pass = segment.next_pass;
        self.bitplane = segment.bitplane;
        self.bitplane_state = segment.bitplane_state.clone();
        self.mq_context_state = segment.mq_context_state;
        self.reset_contexts_before_mq = segment.reset_contexts_before_mq;
        self.decoded_segments = segment.decoded_segments;
        self.decoded_bytes = segment.decoded_bytes;
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::part1_tier1_segment(
                tier1_decoder_state(old(self)), input@, passes, crate::limits::resource_code_block_pass_limit(limits),
                crate::limits::resource_code_block_byte_limit(limits), crate::limits::resource_input_byte_limit(limits),
                crate::limits::resource_work_limit(limits)),
            (passes == 0 || crate::specs::ht_group_route(tier1_decoder_state(old(self)))) ==>
            (result, tier1_decoder_state(final(self))) == crate::specs::ht_decode_staged_group(
                tier1_decoder_state(old(self)), input@, passes, crate::limits::resource_code_block_pass_limit(limits),
                crate::limits::resource_code_block_byte_limit(limits), crate::limits::resource_work_limit(limits)),
    ))]
    pub fn decode_segment(
        &mut self,
        input: &[u8],
        passes: u8,
        limits: &ResourceLimits,
    ) -> Result<Tier1SegmentDecode, DecodeError> {
        if passes == 0 {
            return Err(invalid_code_block());
        }
        if matches!(self.style.block_coding, Tier1BlockCoding::HighThroughput)
            || (matches!(self.style.block_coding, Tier1BlockCoding::Mixed)
                && self.ht_cleanup_plane.is_some())
        {
            return self.decode_ht_staged_group(input, passes, limits);
        }
        self.decode_legacy_segment(input, u32::from(passes), limits)
    }

    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::ht_placeholder_group_run(
            tier1_decoder_state(old(self)), placeholder_passes as nat, crate::limits::resource_code_block_pass_limit(limits)),
            result == Ok(()) || result.is_err(),
    ))]
    fn run_ht_placeholder_groups(
        &mut self,
        placeholder_passes: u8,
        limits: &ResourceLimits,
    ) -> Result<(), DecodeError> {
        let mut remaining = placeholder_passes;
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_placeholder_group_run); }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant remaining <= placeholder_passes,
                crate::specs::ht_placeholder_group_run(tier1_decoder_state(old(self)), placeholder_passes as nat,
                    crate::limits::resource_code_block_pass_limit(limits))
                    == crate::specs::ht_placeholder_group_run(tier1_decoder_state(self), remaining as nat,
                        crate::limits::resource_code_block_pass_limit(limits)),
            decreases remaining,
        ))]
        while remaining > 0 {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::ht_placeholder_group_run); }
            self.skip_high_throughput_segment(1, limits)?;
            self.skip_high_throughput_segment(2, limits)?;
            remaining = remaining.checked_sub(3).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::ht_placeholder_group_run); }
        Ok(())
    }

    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::ht_decode_staged_group(
            tier1_decoder_state(old(self)), input@, passes, crate::limits::resource_code_block_pass_limit(limits),
            crate::limits::resource_code_block_byte_limit(limits), crate::limits::resource_work_limit(limits)),
    ))]
    fn decode_ht_staged_group(
        &mut self,
        input: &[u8],
        passes: u8,
        limits: &ResourceLimits,
    ) -> Result<Tier1SegmentDecode, DecodeError> {
        if passes == 0 || input.is_empty() {
            return Err(invalid_code_block());
        }
        let grouping = ht_pass_group_from_packet(passes)?;
        if grouping.refinement_passes != 0 {
            return Err(invalid_code_block());
        }
        let mut staged = self.clone();
        staged.run_ht_placeholder_groups(grouping.placeholder_passes, limits)?;
        let decoded = staged.decode_high_throughput_segments(input, 1, None, 0, limits)?;
        *self = staged;
        Ok(Tier1SegmentDecode {
            passes: u32::from(passes),
            ..decoded
        })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_legacy_segment_admission(
            tier1_decoder_state(self), input@, pass_count,
            crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits)),
    ))]
    fn validate_legacy_segment(
        &self,
        input: &[u8],
        pass_count: u32,
        limits: &ResourceLimits,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_segment_admission); }
        // Later terminated passes can carry no bytes; an initial empty block cannot.
        if pass_count == 0 || (input.is_empty() && self.next_pass == 0) {
            return Err(invalid_code_block());
        }
        let next_pass = self
            .next_pass
            .checked_add(pass_count)
            .ok_or_else(arithmetic_overflow)?;
        if next_pass > limits.maximum_code_block_passes() {
            return Err(crate::mq::mq_limit(
                Resource::CodeBlockPasses,
                u64::from(limits.maximum_code_block_passes()),
                u64::from(next_pass),
            ));
        }
        let next_bytes = self
            .decoded_bytes
            .checked_add(input.len() as u64)
            .ok_or_else(arithmetic_overflow)?;
        if next_bytes > limits.maximum_code_block_bytes() {
            return Err(crate::mq::mq_limit(
                Resource::CodeBlockBytes,
                limits.maximum_code_block_bytes(),
                next_bytes,
            ));
        }
        if next_pass > self.maximum_passes
            || self.style.segment_passes(self.next_pass, pass_count) != pass_count
        {
            return Err(invalid_code_block());
        }
        if input.is_empty()
            && next_pass != self.maximum_passes
            && !self.style.pass_plan(next_pass - 1).terminates_segment
        {
            return Err(invalid_code_block());
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::part1_legacy_segment(
            tier1_decoder_state(old(self)), input@, pass_count,
            crate::limits::resource_code_block_pass_limit(limits), crate::limits::resource_code_block_byte_limit(limits),
            crate::limits::resource_input_byte_limit(limits), crate::limits::resource_work_limit(limits)),
    ))]
    fn decode_legacy_segment(
        &mut self,
        input: &[u8],
        pass_count: u32,
        limits: &ResourceLimits,
    ) -> Result<Tier1SegmentDecode, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_segment); }
        self.validate_legacy_segment(input, pass_count, limits)?;
        let coding_mode = self.style.pass_plan(self.next_pass).coding_mode;
        let statistics;
        if matches!(coding_mode, Tier1CodingMode::Mq) {
            let mut decoder = crate::mq::mq_from_input(
                input,
                limits.maximum_input_bytes(),
                limits.maximum_work_units(),
            )?;
            if self.reset_contexts_before_mq || self.mq_context_state.is_none() {
                initialize_tier1_contexts(&mut decoder)?;
                self.reset_contexts_before_mq = false;
            } else if let Some(context_state) = self.mq_context_state {
                decoder.restore_context_state(context_state);
            }
            statistics = self.decode_mq_segment_passes(&mut decoder, pass_count)?;
            if self.style.predictable_termination {
                decoder.validate_predictable_termination()?;
            }
            self.mq_context_state = Some(decoder.context_state());
        } else {
            let mut reader = raw_reader_from_input(
                input,
                limits.maximum_input_bytes(),
                limits.maximum_work_units(),
            )?;
            statistics = self.decode_raw_segment_passes(&mut reader, pass_count)?;
            if self.style.predictable_termination {
                reader.validate_predictable_termination()?;
            }
        }

        self.commit_legacy_segment(input.len(), pass_count, statistics)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::part1_legacy_segment_account(
            tier1_decoder_state(old(self)), bytes as nat, pass_count, statistics),
    ))]
    fn commit_legacy_segment(
        &mut self,
        bytes: usize,
        pass_count: u32,
        statistics: Tier1PassStatistics,
    ) -> Result<Tier1SegmentDecode, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_segment_account); }
        self.decoded_segments = self
            .decoded_segments
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        self.decoded_bytes = self
            .decoded_bytes
            .checked_add(bytes as u64)
            .ok_or_else(arithmetic_overflow)?;
        Ok(Tier1SegmentDecode {
            passes: pass_count,
            bytes: match u32::try_from(bytes) {
                Ok(bytes) => bytes,
                Err(_) => return Err(arithmetic_overflow()),
            },
            statistics,
        })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self)),
            crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(final(decoder))))
            == crate::specs::part1_legacy_segment_finish(crate::specs::part1_legacy_segment_run(
                crate::specs::Part1LegacySegmentRun {
                    decoder: tier1_decoder_state(old(self)), symbols: crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(old(decoder))),
                    statistics: crate::specs::tier1_statistics_zero(),
                }, pass_count as nat)),
    ))]
    fn decode_mq_segment_passes(
        &mut self,
        decoder: &mut MqDecoder<'_>,
        pass_count: u32,
    ) -> Result<Tier1PassStatistics, DecodeError> {
        let mut statistics = ht_statistics_zero();
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost initial = crate::specs::Part1LegacySegmentRun {
                decoder: tier1_decoder_state(self), symbols: crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(decoder)), statistics,
            };
            let ghost goal = crate::specs::part1_legacy_segment_run(initial, pass_count as nat);
        }
        let mut decoded = 0_u32;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant decoded <= pass_count,
                initial == (crate::specs::Part1LegacySegmentRun {
                    decoder: tier1_decoder_state(old(self)), symbols: crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(old(decoder))),
                    statistics: crate::specs::tier1_statistics_zero(),
                }),
                goal == crate::specs::part1_legacy_segment_run(initial, pass_count as nat),
                goal == crate::specs::part1_legacy_segment_run(crate::specs::Part1LegacySegmentRun {
                    decoder: tier1_decoder_state(self), symbols: crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(decoder)), statistics,
                }, (pass_count - decoded) as nat),
            decreases pass_count - decoded,
        ))]
        while decoded < pass_count {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal(crate::specs::part1_legacy_segment_run);
                reveal(crate::specs::part1_legacy_segment_step);
            }
            let plan = self.style.pass_plan(self.next_pass);
            if !matches!(plan.coding_mode, Tier1CodingMode::Mq) {
                return Err(invalid_code_block());
            }
            statistics.merge(self.decode_mq_pass(decoder, plan.kind)?)?;
            self.advance_pass(plan.kind)?;
            reset_tier1_contexts_after_pass(decoder, self.style)?;
            decoded = decoded.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_segment_run); }
        Ok(statistics)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self)),
            crate::specs::Part1LegacySymbols::Raw(raw_bit_reader_view(final(reader))))
            == crate::specs::part1_legacy_segment_finish(crate::specs::part1_legacy_segment_run(
                crate::specs::Part1LegacySegmentRun {
                    decoder: tier1_decoder_state(old(self)), symbols: crate::specs::Part1LegacySymbols::Raw(raw_bit_reader_view(old(reader))),
                    statistics: crate::specs::tier1_statistics_zero(),
                }, pass_count as nat)),
    ))]
    fn decode_raw_segment_passes(
        &mut self,
        reader: &mut RawBitReader<'_>,
        pass_count: u32,
    ) -> Result<Tier1PassStatistics, DecodeError> {
        let mut statistics = ht_statistics_zero();
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost initial = crate::specs::Part1LegacySegmentRun {
                decoder: tier1_decoder_state(self), symbols: crate::specs::Part1LegacySymbols::Raw(raw_bit_reader_view(reader)), statistics,
            };
            let ghost goal = crate::specs::part1_legacy_segment_run(initial, pass_count as nat);
        }
        let mut decoded = 0_u32;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant decoded <= pass_count,
                initial == (crate::specs::Part1LegacySegmentRun {
                    decoder: tier1_decoder_state(old(self)), symbols: crate::specs::Part1LegacySymbols::Raw(raw_bit_reader_view(old(reader))),
                    statistics: crate::specs::tier1_statistics_zero(),
                }),
                goal == crate::specs::part1_legacy_segment_run(initial, pass_count as nat),
                goal == crate::specs::part1_legacy_segment_run(crate::specs::Part1LegacySegmentRun {
                    decoder: tier1_decoder_state(self), symbols: crate::specs::Part1LegacySymbols::Raw(raw_bit_reader_view(reader)), statistics,
                }, (pass_count - decoded) as nat),
            decreases pass_count - decoded,
        ))]
        while decoded < pass_count {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal(crate::specs::part1_legacy_segment_run);
                reveal(crate::specs::part1_legacy_segment_step);
            }
            let plan = self.style.pass_plan(self.next_pass);
            if !matches!(plan.coding_mode, Tier1CodingMode::Raw) {
                return Err(invalid_code_block());
            }
            statistics.merge(self.decode_raw_pass(reader, plan.kind)?)?;
            self.advance_pass(plan.kind)?;
            if self.style.reset_contexts {
                self.reset_contexts_before_mq = true;
            }
            decoded = decoded.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_segment_run); }
        Ok(statistics)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::ht_decode_segments(
            tier1_decoder_state(old(self)), cleanup@, cleanup_passes, ht_optional_bytes_view(refinement),
            refinement_passes, crate::limits::resource_code_block_pass_limit(limits),
            crate::limits::resource_code_block_byte_limit(limits), crate::limits::resource_work_limit(limits)),
    ))]
    pub fn decode_high_throughput_segments(
        &mut self,
        cleanup: &[u8],
        cleanup_passes: u8,
        refinement: Option<&[u8]>,
        refinement_passes: u8,
        limits: &ResourceLimits,
    ) -> Result<Tier1SegmentDecode, DecodeError> {
        if !self.style.block_coding.uses_high_throughput()
            || !self.next_pass.is_multiple_of(3)
            || cleanup_passes != 1
            || refinement_passes > 2
            || (refinement_passes == 0) != refinement.is_none()
        {
            return Err(invalid_code_block());
        }
        if self.ht_sets_seen > 0 && !self.allow_multiple_ht_sets {
            return Err(invalid_code_block());
        }
        let pass_count = u32::from(cleanup_passes)
            .checked_add(u32::from(refinement_passes))
            .ok_or_else(arithmetic_overflow)?;
        let next_pass = self
            .next_pass
            .checked_add(pass_count)
            .ok_or_else(arithmetic_overflow)?;
        if next_pass > self.maximum_passes {
            return Err(invalid_code_block());
        }
        if next_pass > limits.maximum_code_block_passes() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::CodeBlockPasses,
                limit: u64::from(limits.maximum_code_block_passes()),
                actual: u64::from(next_pass),
            });
        }
        let significant_bitplanes = ht_significant_bitplanes(self.maximum_passes)?;
        if refinement_passes > 0 && significant_bitplanes < 2 {
            return Err(invalid_code_block());
        }
        let segments = parse_ht_block_segments(cleanup, refinement, limits)?;
        let next_bytes = self
            .decoded_bytes
            .checked_add(u64::from(segments.bytes()))
            .ok_or_else(arithmetic_overflow)?;
        if next_bytes > limits.maximum_code_block_bytes() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::CodeBlockBytes,
                limit: limits.maximum_code_block_bytes(),
                actual: next_bytes,
            });
        }
        let substantive_passes = u64::from(refinement_passes)
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        let coefficient_work = (self.plane.coefficients().len() as u64)
            .checked_mul(substantive_passes)
            .ok_or_else(arithmetic_overflow)?;
        let work_base = coefficient_work
            .checked_add(u64::from(segments.bytes()))
            .ok_or_else(arithmetic_overflow)?;
        let work = work_base
            .checked_add(u64::from(pass_count))
            .ok_or_else(arithmetic_overflow)?;
        if work > limits.maximum_work_units() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                limit: limits.maximum_work_units(),
                actual: work,
            });
        }
        let skipped = self.next_pass / 3;
        if skipped > u32::from(u16::MAX) {
            return Err(arithmetic_overflow());
        }
        let skipped_bitplanes = skipped as u16;
        let missing_msbs = self.zero_bitplanes.unwrap_or(0);
        let maximum_u = crate::refinement::part15_cleanup_u_bound(
            missing_msbs,
            skipped_bitplanes,
            self.ht_magnitude_bound,
        );
        self.decode_ht_coefficient_symbols(
            segments,
            pass_count,
            maximum_u,
            skipped_bitplanes,
            limits,
        )
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::ht_coefficient_symbols(
                tier1_decoder_state(old(self)), ht_block_segments_view(&segments),
                pass_count, maximum_u, skipped_bitplanes, crate::limits::resource_work_limit(limits)),
    ))]
    fn decode_ht_coefficient_symbols(
        &mut self,
        segments: HtBlockSegments<'_>,
        pass_count: u32,
        maximum_u: u8,
        skipped_bitplanes: u16,
        limits: &ResourceLimits,
    ) -> Result<Tier1SegmentDecode, DecodeError> {
        let (plane, cleanup_plane, statistics, decoded_bitplanes) = prepare_ht_cleanup_symbols(
            &self.plane,
            segments.cleanup(),
            self.maximum_passes,
            maximum_u,
            skipped_bitplanes,
            limits,
        )?;
        let mut output = plane;
        let mut statistics = statistics;
        self.clear_ht_refinement_state();
        if let Some(refinement) = segments.refinement() {
            let following_passes = pass_count.saturating_sub(1);
            if following_passes > u32::from(u8::MAX) {
                return Err(arithmetic_overflow());
            }
            let refinement_passes = following_passes as u8;
            let (refined, refinement_statistics) = decode_ht_refinement_plane(
                &cleanup_plane,
                refinement,
                refinement_passes,
                decoded_bitplanes,
                self.style.vertical_causal_context,
                limits,
            )?;
            output = refined;
            statistics.merge(refinement_statistics)?;
            self.ht_refinement_data = copy_ht_refinement_bytes(refinement.encoded())?;
            self.ht_refinement_passes = refinement_passes;
        }
        self.store_ht_coefficient_symbols(
            output,
            cleanup_plane,
            statistics,
            decoded_bitplanes,
            pass_count,
            segments.bytes(),
            segments.refinement().is_some(),
        )
    }

    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures tier1_decoder_state(final(self)) == crate::specs::ht_clear_refinement_state(tier1_decoder_state(old(self))),
    ))]
    fn clear_ht_refinement_state(&mut self) {
        self.ht_refinement_data.clear();
        self.ht_refinement_passes = 0;
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "This shared production commit stage preserves the actual ordered fields and accounting inputs."
    )]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::ht_store_coefficient_symbols(
            tier1_decoder_state(old(self)), code_block_plane_view(&output), code_block_plane_view(&cleanup_plane),
            statistics, decoded_bitplanes, pass_count, bytes, has_refinement),
    ))]
    fn store_ht_coefficient_symbols(
        &mut self,
        output: CodeBlockPlane,
        cleanup_plane: CodeBlockPlane,
        statistics: Tier1PassStatistics,
        decoded_bitplanes: u16,
        pass_count: u32,
        bytes: u32,
        has_refinement: bool,
    ) -> Result<Tier1SegmentDecode, DecodeError> {
        self.plane = output;
        self.ht_cleanup_plane = Some(cleanup_plane);
        self.ht_current_set_has_cleanup = true;
        self.ht_decoded_bitplanes = decoded_bitplanes;
        self.ht_sets_seen = self
            .ht_sets_seen
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        self.next_pass = self
            .next_pass
            .checked_add(pass_count)
            .ok_or_else(arithmetic_overflow)?;
        self.decoded_segments = self
            .decoded_segments
            .checked_add(if has_refinement { 2 } else { 1 })
            .ok_or_else(arithmetic_overflow)?;
        self.decoded_bytes = self
            .decoded_bytes
            .checked_add(u64::from(bytes))
            .ok_or_else(arithmetic_overflow)?;
        Ok(Tier1SegmentDecode {
            passes: pass_count,
            bytes,
            statistics,
        })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::ht_skip_segment(
            tier1_decoder_state(old(self)), passes, crate::limits::resource_code_block_pass_limit(limits)),
    ))]
    pub fn skip_high_throughput_segment(
        &mut self,
        passes: u8,
        limits: &ResourceLimits,
    ) -> Result<Tier1SegmentDecode, DecodeError> {
        if !self.style.block_coding.uses_high_throughput() || passes == 0 || passes > 2 {
            return Err(invalid_code_block());
        }
        let phase = self.next_pass % 3;
        if (phase == 0 && passes != 1)
            || (phase != 0 && u32::from(passes) > 3_u32.saturating_sub(phase))
        {
            return Err(invalid_code_block());
        }
        let next_pass = self
            .next_pass
            .checked_add(u32::from(passes))
            .ok_or_else(arithmetic_overflow)?;
        if next_pass > self.maximum_passes {
            return Err(invalid_code_block());
        }
        if next_pass > limits.maximum_code_block_passes() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::CodeBlockPasses,
                limit: u64::from(limits.maximum_code_block_passes()),
                actual: u64::from(next_pass),
            });
        }
        if phase == 0 {
            if !crate::refinement::ht_replacement_allowed(
                phase,
                self.ht_current_set_has_cleanup,
                true,
            ) {
                return Err(invalid_code_block());
            }
            if self.ht_sets_seen > 0 {
                if !crate::refinement::ht_multiple_set_allowed(
                    self.ht_sets_seen,
                    self.allow_multiple_ht_sets,
                ) {
                    return Err(invalid_code_block());
                }
                self.ht_sets_seen = self
                    .ht_sets_seen
                    .checked_add(1)
                    .ok_or_else(arithmetic_overflow)?;
            }
            self.ht_current_set_has_cleanup = false;
            self.clear_ht_refinement_state();
        } else if self.ht_current_set_has_cleanup {
            return Err(invalid_code_block());
        }
        self.next_pass = next_pass;
        self.decoded_segments = self
            .decoded_segments
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        Ok(Tier1SegmentDecode {
            passes: u32::from(passes),
            bytes: 0,
            statistics: ht_statistics_zero(),
        })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::ht_has_current_cleanup(tier1_decoder_state(self)),
    ))]
    pub const fn ht_current_set_has_cleanup(&self) -> bool {
        self.ht_current_set_has_cleanup
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::ht_decode_refinement_contribution(
            tier1_decoder_state(old(self)), input@, passes,
            crate::limits::resource_code_block_byte_limit(limits), crate::limits::resource_work_limit(limits)),
    ))]
    pub fn decode_high_throughput_refinement(
        &mut self,
        input: &[u8],
        passes: u8,
        limits: &ResourceLimits,
    ) -> Result<Tier1SegmentDecode, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost start = tier1_decoder_state(self); }
        if !self.style.block_coding.uses_high_throughput()
            || self.next_pass == 0
            || !self.ht_current_set_has_cleanup
            || self.next_pass.is_multiple_of(3)
            || passes == 0
            || match self.ht_refinement_passes.checked_add(passes) {
                Some(value) => value > 2,
                None => true,
            }
        {
            return Err(invalid_code_block());
        }
        let cleanup_plane = self
            .ht_cleanup_plane
            .as_ref()
            .ok_or_else(invalid_code_block)?;
        let next_bytes = self
            .decoded_bytes
            .checked_add(input.len() as u64)
            .ok_or_else(arithmetic_overflow)?;
        if next_bytes > limits.maximum_code_block_bytes() {
            return Err(DecodeError::LimitExceeded {
                resource: Resource::CodeBlockBytes,
                limit: limits.maximum_code_block_bytes(),
                actual: next_bytes,
            });
        }
        let mut refinement_data = self.ht_refinement_data.clone();
        #[cfg(verus_keep_ghost)]
        proof! { assert(refinement_data@ == start.ht_refinement_data); }
        refinement_data.extend_from_slice(input);
        #[cfg(verus_keep_ghost)]
        proof! { assert(refinement_data@ == start.ht_refinement_data + input@); }
        if refinement_data.len() > MAX_HT_REFINEMENT_BYTES {
            return Err(invalid_code_block());
        }
        let refinement_passes =
            crate::refinement::ht_refinement_pass_total(self.ht_refinement_passes, passes);
        if refinement_data.is_empty() {
            self.ht_refinement_passes = refinement_passes;
            self.next_pass = self
                .next_pass
                .checked_add(u32::from(passes))
                .ok_or_else(arithmetic_overflow)?;
            self.decoded_segments = self
                .decoded_segments
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
            return Ok(Tier1SegmentDecode {
                passes: u32::from(passes),
                bytes: 0,
                statistics: ht_statistics_zero(),
            });
        }
        let segment = HtRefinementSegment {
            encoded: &refinement_data,
            significance_propagation: HtSubstreamView {
                bytes: &refinement_data,
                offset: 0,
                direction: HtStreamDirection::Forward,
            },
            magnitude_refinement: HtSubstreamView {
                bytes: &refinement_data,
                offset: 0,
                direction: HtStreamDirection::Reverse,
            },
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(code_block_plane_view(cleanup_plane) == start.ht_cleanup_plane.get_Some_0());
            assert(refinement_passes as nat == start.ht_refinement_passes as nat + passes as nat);
            assert(ht_refinement_segment_view(&segment) == crate::specs::HtRefinementSegmentState {
                encoded: refinement_data@,
                significance_propagation: crate::specs::HtSubstreamState { bytes: refinement_data@, offset: 0, forward: true },
                magnitude_refinement: crate::specs::HtSubstreamState { bytes: refinement_data@, offset: 0, forward: false },
            });
        }
        let (plane, statistics) = decode_ht_refinement_plane(
            cleanup_plane,
            segment,
            refinement_passes,
            self.ht_decoded_bitplanes,
            self.style.vertical_causal_context,
            limits,
        )?;
        self.plane = plane;
        self.ht_refinement_data = refinement_data;
        self.ht_refinement_passes = refinement_passes;
        self.next_pass = self
            .next_pass
            .checked_add(u32::from(passes))
            .ok_or_else(arithmetic_overflow)?;
        self.decoded_segments = self
            .decoded_segments
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        self.decoded_bytes = next_bytes;
        Ok(Tier1SegmentDecode {
            passes: u32::from(passes),
            bytes: u32::try_from(input.len()).map_err(|_| arithmetic_overflow())?,
            statistics,
        })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self)),
            crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(final(decoder))))
            == crate::specs::part1_legacy_decode_pass(tier1_decoder_state(old(self)),
                crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(old(decoder))), kind),
    ))]
    fn decode_mq_pass(
        &mut self,
        decoder: &mut MqDecoder<'_>,
        kind: Tier1PassKind,
    ) -> Result<Tier1PassStatistics, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_decode_pass); }
        self.ensure_bitplane_state()?;
        let state = self
            .bitplane_state
            .as_mut()
            .ok_or_else(invalid_code_block)?;
        match kind {
            Tier1PassKind::SignificancePropagation => {
                decode_significance_propagation_pass_with_style(
                    &mut self.plane,
                    state,
                    decoder,
                    self.orientation,
                    self.style,
                )
            }
            Tier1PassKind::MagnitudeRefinement => decode_magnitude_refinement_pass_with_style(
                &mut self.plane,
                state,
                decoder,
                self.style,
            ),
            Tier1PassKind::Cleanup => decode_cleanup_pass_with_style(
                &mut self.plane,
                state,
                decoder,
                self.orientation,
                self.style,
            ),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self)),
            crate::specs::Part1LegacySymbols::Raw(raw_bit_reader_view(final(reader))))
            == crate::specs::part1_legacy_decode_pass(tier1_decoder_state(old(self)),
                crate::specs::Part1LegacySymbols::Raw(raw_bit_reader_view(old(reader))), kind),
    ))]
    fn decode_raw_pass(
        &mut self,
        reader: &mut RawBitReader<'_>,
        kind: Tier1PassKind,
    ) -> Result<Tier1PassStatistics, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_decode_pass); }
        self.ensure_bitplane_state()?;
        let state = self
            .bitplane_state
            .as_mut()
            .ok_or_else(invalid_code_block)?;
        match kind {
            Tier1PassKind::SignificancePropagation => decode_raw_significance_propagation_pass(
                &mut self.plane,
                state,
                reader,
                self.orientation,
                self.style,
            ),
            Tier1PassKind::MagnitudeRefinement => {
                decode_raw_magnitude_refinement_pass(&mut self.plane, state, reader, self.style)
            }
            Tier1PassKind::Cleanup => Err(invalid_code_block()),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::part1_legacy_ensure_bitplane(tier1_decoder_state(old(self))),
    ))]
    fn ensure_bitplane_state(&mut self) -> Result<(), DecodeError> {
        if self.bitplane_state.is_none() {
            self.bitplane_state = Some(legacy_bitplane_from_plane(&self.plane, self.bitplane)?);
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, tier1_decoder_state(final(self))) == crate::specs::part1_legacy_advance(tier1_decoder_state(old(self)), kind),
    ))]
    fn advance_pass(&mut self, kind: Tier1PassKind) -> Result<(), DecodeError> {
        self.next_pass = self
            .next_pass
            .checked_add(1)
            .ok_or_else(arithmetic_overflow)?;
        if matches!(kind, Tier1PassKind::Cleanup) {
            self.bitplane_state = None;
            if self.next_pass < self.maximum_passes {
                self.bitplane = self
                    .bitplane
                    .checked_sub(1)
                    .ok_or_else(invalid_code_block)?;
            }
        }
        Ok(())
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_legacy_contribution_work(samples as nat, replay, bytes, previous, maximum),
))]
fn legacy_contribution_work(
    samples: usize,
    replay: u32,
    bytes: u64,
    previous: u64,
    maximum: u64,
) -> Result<u64, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_legacy_contribution_work); }
    let base_work = match (samples as u64).checked_mul(u64::from(replay)) {
        Some(work) => work,
        None => return Err(arithmetic_overflow()),
    };
    let input_work = match base_work.checked_add(bytes) {
        Some(work) => work,
        None => return Err(arithmetic_overflow()),
    };
    let work = match input_work.checked_add(previous) {
        Some(work) => work,
        None => return Err(arithmetic_overflow()),
    };
    if work > maximum {
        return Err(crate::mq::mq_limit(Resource::WorkUnits, maximum, work));
    }
    Ok(work)
}

impl Tier1BitplaneState {
    pub fn new(plane: &CodeBlockPlane, bitplane_mask: u64) -> Result<Self, DecodeError> {
        if !is_bitplane_mask(bitplane_mask) {
            return Err(invalid_code_block());
        }
        let bitplane =
            u16::try_from(bitplane_mask.trailing_zeros()).map_err(|_| arithmetic_overflow())?;
        Self::new_at_bitplane(plane, bitplane)
    }

    pub fn new_at_bitplane(plane: &CodeBlockPlane, bitplane: u16) -> Result<Self, DecodeError> {
        legacy_bitplane_from_plane(plane, bitplane)
    }

    pub const fn bitplane_mask(&self) -> u64 {
        if self.bitplane < 64 {
            1_u64 << self.bitplane
        } else {
            0
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == legacy_bitplane_view(self).bitplane,
    ))]
    pub const fn bitplane(&self) -> u16 {
        self.bitplane
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == legacy_bitplane_view(self).significance_propagation_complete,
    ))]
    pub const fn significance_propagation_complete(&self) -> bool {
        self.significance_propagation_complete
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == legacy_bitplane_view(self).magnitude_refinement_complete,
    ))]
    pub const fn magnitude_refinement_complete(&self) -> bool {
        self.magnitude_refinement_complete
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == legacy_bitplane_view(self).cleanup_complete,
    ))]
    pub const fn cleanup_complete(&self) -> bool {
        self.cleanup_complete
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures legacy_bitplane_outcome(result) == crate::specs::part1_legacy_bitplane_initial(
            code_block_plane_view(plane), bitplane),
    ))]
fn legacy_bitplane_from_plane(
    plane: &CodeBlockPlane,
    bitplane: u16,
) -> Result<Tier1BitplaneState, DecodeError> {
    if bitplane >= MAX_TIER1_BITPLANES {
        return Err(invalid_code_block());
    }
    let mut significant_before = Vec::with_capacity(plane.coefficients.len());
    let mut index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant index <= plane.coefficients.len(), significant_before.len() == index,
                bitplane < MAX_TIER1_BITPLANES,
                forall |i: int| 0 <= i < index ==> significant_before@[i] == plane.coefficients@[i].significant,
            decreases plane.coefficients.len() - index,
        ))]
    while index < plane.coefficients.len() {
        significant_before.push(plane.coefficients[index].significant);
        index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(significant_before@ =~= code_block_plane_view(plane).coefficients.map_values(
            |c: crate::specs::Tier1RawCoefficient| c.significant));
    }
    let significance_visited = vec![false; significant_before.len()];
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(significance_visited@ =~= Seq::new(code_block_plane_view(plane).coefficients.len(), |i: int| false));
    }
    Ok(Tier1BitplaneState {
        width: plane.width,
        height: plane.height,
        bitplane,
        significance_visited,
        significant_before,
        significance_propagation_complete: false,
        magnitude_refinement_complete: false,
        cleanup_complete: false,
    })
}

impl Tier1BitplaneState {
    pub fn was_visited_by_significance_propagation(
        &self,
        plane: &CodeBlockPlane,
        x: u16,
        y: u16,
    ) -> Result<bool, DecodeError> {
        self.validate_plane(plane)?;
        let index = plane.index(x, y)?;
        if index >= self.significance_visited.len() {
            return Err(invalid_code_block());
        }
        Ok(self.significance_visited[index])
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_legacy_validate(legacy_bitplane_view(self), code_block_plane_view(plane)),
    ))]
    fn validate_plane(&self, plane: &CodeBlockPlane) -> Result<(), DecodeError> {
        if self.width != plane.width
            || self.height != plane.height
            || self.significant_before.len() != plane.coefficients.len()
            || self.significance_visited.len() != plane.coefficients.len()
        {
            return Err(invalid_code_block());
        }
        Ok(())
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_plane_view(final(plane)), legacy_bitplane_view(final(state)),
        crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(final(decoder))))
        == crate::specs::part1_legacy_pass(code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)),
            crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(old(decoder))), orientation, false, 0),
))]
pub fn decode_significance_propagation_pass(
    plane: &mut CodeBlockPlane,
    state: &mut Tier1BitplaneState,
    decoder: &mut MqDecoder<'_>,
    orientation: SubbandOrientation,
) -> Result<Tier1PassStatistics, DecodeError> {
    decode_significance_propagation_pass_with_style(
        plane,
        state,
        decoder,
        orientation,
        legacy_default_style(),
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_legacy_default_style(),
))]
fn legacy_default_style() -> Tier1CodingStyle {
    Tier1CodingStyle {
        block_coding: Tier1BlockCoding::Legacy,
        force_high_throughput: false,
        selective_arithmetic_coding_bypass: false,
        reset_contexts: false,
        terminate_all_passes: false,
        vertical_causal_context: false,
        predictable_termination: false,
        segmentation_symbols: false,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == Ok((bitplane as u32 * 3 + 1) as u32),
))]
fn legacy_maximum_passes(bitplane: u16) -> Result<u32, DecodeError> {
    let passes = u32::from(bitplane)
        .checked_mul(3)
        .ok_or_else(arithmetic_overflow)?;
    passes.checked_add(1).ok_or_else(arithmetic_overflow)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_plane_view(final(plane)), legacy_bitplane_view(final(state)),
        crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(final(decoder))))
        == crate::specs::part1_legacy_pass(code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)),
            crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(old(decoder))), orientation, style.vertical_causal_context, 0),
))]
pub fn decode_significance_propagation_pass_with_style(
    plane: &mut CodeBlockPlane,
    state: &mut Tier1BitplaneState,
    decoder: &mut MqDecoder<'_>,
    orientation: SubbandOrientation,
    style: Tier1CodingStyle,
) -> Result<Tier1PassStatistics, DecodeError> {
    decode_significance_propagation(
        plane,
        state,
        decoder,
        orientation,
        style.vertical_causal_context,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_plane_view(final(plane)), legacy_bitplane_view(final(state)),
        crate::specs::Part1LegacySymbols::Raw(raw_bit_reader_view(final(reader))))
        == crate::specs::part1_legacy_pass(code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)),
            crate::specs::Part1LegacySymbols::Raw(raw_bit_reader_view(old(reader))), orientation, style.vertical_causal_context, 0),
))]
pub fn decode_raw_significance_propagation_pass(
    plane: &mut CodeBlockPlane,
    state: &mut Tier1BitplaneState,
    reader: &mut RawBitReader<'_>,
    orientation: SubbandOrientation,
    style: Tier1CodingStyle,
) -> Result<Tier1PassStatistics, DecodeError> {
    decode_significance_propagation(
        plane,
        state,
        reader,
        orientation,
        style.vertical_causal_context,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_plane_view(final(plane)), legacy_bitplane_view(final(state)), legacy_symbols_view(final(decoder)))
        == crate::specs::part1_legacy_pass(
            code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)), legacy_symbols_view(old(decoder)),
            orientation, vertical_causal_context, 0),
))]
fn decode_significance_propagation(
    plane: &mut CodeBlockPlane,
    state: &mut Tier1BitplaneState,
    decoder: &mut impl Tier1SymbolDecoder,
    orientation: SubbandOrientation,
    vertical_causal_context: bool,
) -> Result<Tier1PassStatistics, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost goal = crate::specs::part1_legacy_pass(
            code_block_plane_view(plane), legacy_bitplane_view(state), legacy_symbols_view(decoder),
            orientation, vertical_causal_context, 0);
    }
    state.validate_plane(plane)?;
    if state.significance_propagation_complete
        || state.magnitude_refinement_complete
        || state.cleanup_complete
    {
        return Err(invalid_code_block());
    }

    let mut statistics = ht_statistics_zero();
    let height = plane.height;
    let width = plane.width;
    let mut stripe_y = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant stripe_y <= height,
                goal == crate::specs::part1_legacy_pass(
                    code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)), legacy_symbols_view(old(decoder)),
                    orientation, vertical_causal_context, 0),
                goal == crate::specs::part1_legacy_finish(crate::specs::part1_legacy_stripes(
                    legacy_frame_view(plane, state, decoder, statistics), stripe_y as nat, width, height,
                    orientation, vertical_causal_context, 0), 0),
            decreases height - stripe_y,
        )
    )]
    while stripe_y < height {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_stripes); }
        let stripe_rows = crate::refinement::tier1_stripe_rows(height, stripe_y);
        let mut x = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant x <= width,
                    stripe_y < height, stripe_rows > 0, stripe_rows <= 4,
                    stripe_y as nat + stripe_rows as nat <= height,
                    goal == crate::specs::part1_legacy_pass(
                        code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)), legacy_symbols_view(old(decoder)),
                        orientation, vertical_causal_context, 0),
                    goal == crate::specs::part1_legacy_column_tail(
                        legacy_frame_view(plane, state, decoder, statistics), x as nat, stripe_y, stripe_rows as nat,
                        width, height, orientation, vertical_causal_context, 0),
                decreases width - x,
            )
        )]
        while x < width {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part1_legacy_columns); }
            let mut row = 0;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant row <= stripe_rows,
                        x < width, stripe_y < height, stripe_rows > 0, stripe_rows <= 4,
                        stripe_y as nat + stripe_rows as nat <= height,
                        goal == crate::specs::part1_legacy_pass(
                            code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)), legacy_symbols_view(old(decoder)),
                            orientation, vertical_causal_context, 0),
                        goal == crate::specs::part1_legacy_row_tail(
                            legacy_frame_view(plane, state, decoder, statistics), x, stripe_y, row as nat, stripe_rows as nat,
                            width, height, orientation, vertical_causal_context, 0),
                    decreases stripe_rows - row,
                )
            )]
            while row < stripe_rows {
                #[cfg(verus_keep_ghost)]
                proof! { reveal(crate::specs::part1_legacy_rows); }
                let y = match stripe_y.checked_add(row) {
                    Some(y) => y,
                    None => return Err(arithmetic_overflow()),
                };
                decode_significance_coefficient(
                    plane,
                    state,
                    decoder,
                    orientation,
                    vertical_causal_context,
                    (x, y),
                    &mut statistics,
                )?;
                row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part1_legacy_rows); }
            x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_columns); }
        let next_stripe = stripe_y
            .checked_add(stripe_rows)
            .ok_or_else(arithmetic_overflow)?;
        if next_stripe <= stripe_y || next_stripe > height {
            return Err(invalid_code_block());
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_stripes); }
        stripe_y = next_stripe;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part1_legacy_stripes);
        assert(goal == crate::specs::part1_legacy_finish(
            (Ok(()), legacy_frame_view(plane, state, decoder, statistics)), 0));
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost before_finish = legacy_frame_view(plane, state, decoder, statistics); }
    state.significance_propagation_complete = crate::refinement::tier1_pass_complete(0) & 1 != 0;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(1u8 & 1u8 != 0) by(bit_vector);
        assert(state.significance_propagation_complete);
        assert(legacy_bitplane_view(state) == (crate::specs::Part1LegacyBitplane {
            significance_propagation_complete: true, ..before_finish.state
        }));
        assert(code_block_plane_view(plane) == before_finish.plane);
        assert(legacy_symbols_view(decoder) == before_finish.symbols);
        assert((Ok(statistics), code_block_plane_view(plane), legacy_bitplane_view(state), legacy_symbols_view(decoder)) == goal);
    }
    Ok(statistics)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, legacy_frame_view(final(plane), final(state), final(decoder), *final(statistics)))
        == crate::specs::part1_legacy_sig_cell(
            legacy_frame_view(old(plane), old(state), old(decoder), *old(statistics)),
            coordinate.0, coordinate.1, orientation, vertical_causal_context),
))]
fn decode_significance_coefficient(
    plane: &mut CodeBlockPlane,
    state: &mut Tier1BitplaneState,
    decoder: &mut impl Tier1SymbolDecoder,
    orientation: SubbandOrientation,
    vertical_causal_context: bool,
    coordinate: (u16, u16),
    statistics: &mut Tier1PassStatistics,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_legacy_sig_cell); }
    let (x, y) = coordinate;
    statistics.scanned(1)?;
    let index = plane.index(x, y)?;
    let coefficient = plane.coefficient(x, y)?;
    if !coefficient.significant {
        let summary = plane.neighbor_summary_with_causality(x, y, vertical_causal_context)?;
        let neighbors = significant_neighbor_count(summary);
        if crate::refinement::tier1_sigprop_eligible(coefficient.significant, neighbors) {
            if index >= state.significance_visited.len() {
                return Err(invalid_code_block());
            }
            let zero_context = zero_coding_context(summary, orientation);
            let significant = decoder.decode(usize::from(zero_context))?;
            state.significance_visited[index] = true;
            statistics.decoded_symbols(1)?;
            if significant {
                decode_sign_and_mark(plane, decoder, x, y, state.bitplane, summary)?;
                statistics.decoded_symbols(1)?;
                statistics.became_significant()?;
            }
        }
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_plane_view(final(plane)), legacy_bitplane_view(final(state)),
        crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(final(decoder))))
        == crate::specs::part1_legacy_pass(code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)),
            crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(old(decoder))), SubbandOrientation::Ll, false, 1),
))]
pub fn decode_magnitude_refinement_pass(
    plane: &mut CodeBlockPlane,
    state: &mut Tier1BitplaneState,
    decoder: &mut MqDecoder<'_>,
) -> Result<Tier1PassStatistics, DecodeError> {
    decode_magnitude_refinement_pass_with_style(plane, state, decoder, legacy_default_style())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_plane_view(final(plane)), legacy_bitplane_view(final(state)),
        crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(final(decoder))))
        == crate::specs::part1_legacy_pass(code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)),
            crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(old(decoder))), SubbandOrientation::Ll, style.vertical_causal_context, 1),
))]
pub fn decode_magnitude_refinement_pass_with_style(
    plane: &mut CodeBlockPlane,
    state: &mut Tier1BitplaneState,
    decoder: &mut MqDecoder<'_>,
    style: Tier1CodingStyle,
) -> Result<Tier1PassStatistics, DecodeError> {
    decode_magnitude_refinement(plane, state, decoder, style.vertical_causal_context)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_plane_view(final(plane)), legacy_bitplane_view(final(state)),
        crate::specs::Part1LegacySymbols::Raw(raw_bit_reader_view(final(reader))))
        == crate::specs::part1_legacy_pass(code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)),
            crate::specs::Part1LegacySymbols::Raw(raw_bit_reader_view(old(reader))), SubbandOrientation::Ll, style.vertical_causal_context, 1),
))]
pub fn decode_raw_magnitude_refinement_pass(
    plane: &mut CodeBlockPlane,
    state: &mut Tier1BitplaneState,
    reader: &mut RawBitReader<'_>,
    style: Tier1CodingStyle,
) -> Result<Tier1PassStatistics, DecodeError> {
    decode_magnitude_refinement(plane, state, reader, style.vertical_causal_context)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_plane_view(final(plane)), legacy_bitplane_view(final(state)), legacy_symbols_view(final(decoder)))
        == crate::specs::part1_legacy_pass(
            code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)), legacy_symbols_view(old(decoder)),
            SubbandOrientation::Ll, vertical_causal_context, 1),
))]
fn decode_magnitude_refinement(
    plane: &mut CodeBlockPlane,
    state: &mut Tier1BitplaneState,
    decoder: &mut impl Tier1SymbolDecoder,
    vertical_causal_context: bool,
) -> Result<Tier1PassStatistics, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost goal = crate::specs::part1_legacy_pass(
            code_block_plane_view(plane), legacy_bitplane_view(state), legacy_symbols_view(decoder),
            SubbandOrientation::Ll, vertical_causal_context, 1);
    }
    state.validate_plane(plane)?;
    if !state.significance_propagation_complete
        || state.magnitude_refinement_complete
        || state.cleanup_complete
    {
        return Err(invalid_code_block());
    }

    let mut statistics = ht_statistics_zero();
    let height = plane.height;
    let width = plane.width;
    let mut stripe_y = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant stripe_y <= height,
                goal == crate::specs::part1_legacy_pass(
                    code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)), legacy_symbols_view(old(decoder)),
                    SubbandOrientation::Ll, vertical_causal_context, 1),
                goal == crate::specs::part1_legacy_finish(crate::specs::part1_legacy_stripes(
                    legacy_frame_view(plane, state, decoder, statistics), stripe_y as nat, width, height,
                    SubbandOrientation::Ll, vertical_causal_context, 1), 1),
            decreases height - stripe_y,
        )
    )]
    while stripe_y < height {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_stripes); }
        let stripe_rows = crate::refinement::tier1_stripe_rows(height, stripe_y);
        let mut x = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant x <= width,
                    stripe_y < height, stripe_rows > 0, stripe_rows <= 4,
                    stripe_y as nat + stripe_rows as nat <= height,
                    goal == crate::specs::part1_legacy_pass(
                        code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)), legacy_symbols_view(old(decoder)),
                        SubbandOrientation::Ll, vertical_causal_context, 1),
                    goal == crate::specs::part1_legacy_column_tail(
                        legacy_frame_view(plane, state, decoder, statistics), x as nat, stripe_y, stripe_rows as nat,
                        width, height, SubbandOrientation::Ll, vertical_causal_context, 1),
                decreases width - x,
            )
        )]
        while x < width {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part1_legacy_columns); }
            let mut row = 0;
            #[cfg_attr(
                verus_keep_ghost,
                verus_spec(
                    invariant row <= stripe_rows,
                        x < width, stripe_y < height, stripe_rows > 0, stripe_rows <= 4,
                        stripe_y as nat + stripe_rows as nat <= height,
                        goal == crate::specs::part1_legacy_pass(
                            code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)), legacy_symbols_view(old(decoder)),
                            SubbandOrientation::Ll, vertical_causal_context, 1),
                        goal == crate::specs::part1_legacy_row_tail(
                            legacy_frame_view(plane, state, decoder, statistics), x, stripe_y, row as nat, stripe_rows as nat,
                            width, height, SubbandOrientation::Ll, vertical_causal_context, 1),
                    decreases stripe_rows - row,
                )
            )]
            while row < stripe_rows {
                #[cfg(verus_keep_ghost)]
                proof! { reveal(crate::specs::part1_legacy_rows); }
                let y = stripe_y.checked_add(row).ok_or_else(arithmetic_overflow)?;
                decode_refinement_coefficient(
                    plane,
                    state,
                    decoder,
                    vertical_causal_context,
                    (x, y),
                    &mut statistics,
                )?;
                row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part1_legacy_rows); }
            x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_columns); }
        let next_stripe = stripe_y
            .checked_add(stripe_rows)
            .ok_or_else(arithmetic_overflow)?;
        if next_stripe <= stripe_y || next_stripe > height {
            return Err(invalid_code_block());
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_stripes); }
        stripe_y = next_stripe;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part1_legacy_stripes);
        assert(goal == crate::specs::part1_legacy_finish(
            (Ok(()), legacy_frame_view(plane, state, decoder, statistics)), 1));
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost before_finish = legacy_frame_view(plane, state, decoder, statistics); }
    state.magnitude_refinement_complete = crate::refinement::tier1_pass_complete(1) & 2 != 0;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(2u8 & 2u8 != 0) by(bit_vector);
        assert(state.magnitude_refinement_complete);
        assert(legacy_bitplane_view(state) == (crate::specs::Part1LegacyBitplane {
            magnitude_refinement_complete: true, ..before_finish.state
        }));
        assert(code_block_plane_view(plane) == before_finish.plane);
        assert(legacy_symbols_view(decoder) == before_finish.symbols);
        assert((Ok(statistics), code_block_plane_view(plane), legacy_bitplane_view(state), legacy_symbols_view(decoder)) == goal);
    }
    Ok(statistics)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, legacy_frame_view(final(plane), state, final(decoder), *final(statistics)))
        == crate::specs::part1_legacy_mag_cell(
            legacy_frame_view(old(plane), state, old(decoder), *old(statistics)),
            coordinate.0, coordinate.1, vertical_causal_context),
))]
fn decode_refinement_coefficient(
    plane: &mut CodeBlockPlane,
    state: &Tier1BitplaneState,
    decoder: &mut impl Tier1SymbolDecoder,
    vertical_causal_context: bool,
    coordinate: (u16, u16),
    statistics: &mut Tier1PassStatistics,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_legacy_mag_cell); }
    let (x, y) = coordinate;
    statistics.scanned(1)?;
    let index = plane.index(x, y)?;
    if index >= state.significant_before.len() {
        return Err(invalid_code_block());
    }
    if crate::refinement::tier1_magref_eligible(state.significant_before[index]) {
        let coefficient = plane.coefficient(x, y)?;
        let summary = plane.neighbor_summary_with_causality(x, y, vertical_causal_context)?;
        let context = magnitude_refinement_context(coefficient, summary);
        let refinement_bit = decoder.decode(usize::from(context))?;
        statistics.decoded_symbols(1)?;
        plane.apply_refinement_bit(x, y, state.bitplane, refinement_bit)?;
        statistics.refined()?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_plane_view(final(plane)), legacy_bitplane_view(final(state)),
        crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(final(decoder))))
        == crate::specs::part1_legacy_cleanup_pass(code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)),
            crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(old(decoder))), orientation, false, false),
))]
pub fn decode_cleanup_pass(
    plane: &mut CodeBlockPlane,
    state: &mut Tier1BitplaneState,
    decoder: &mut MqDecoder<'_>,
    orientation: SubbandOrientation,
) -> Result<Tier1PassStatistics, DecodeError> {
    decode_cleanup_pass_with_style(plane, state, decoder, orientation, legacy_default_style())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_plane_view(final(plane)), legacy_bitplane_view(final(state)),
        crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(final(decoder))))
        == crate::specs::part1_legacy_cleanup_pass(code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)),
            crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(old(decoder))), orientation,
            style.vertical_causal_context, style.segmentation_symbols),
))]
pub fn decode_cleanup_pass_with_style(
    plane: &mut CodeBlockPlane,
    state: &mut Tier1BitplaneState,
    decoder: &mut MqDecoder<'_>,
    orientation: SubbandOrientation,
    style: Tier1CodingStyle,
) -> Result<Tier1PassStatistics, DecodeError> {
    decode_cleanup(
        plane,
        state,
        decoder,
        orientation,
        style.vertical_causal_context,
        style.segmentation_symbols,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_plane_view(final(plane)), legacy_bitplane_view(final(state)), legacy_symbols_view(final(decoder)))
        == crate::specs::part1_legacy_cleanup_pass(
            code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)), legacy_symbols_view(old(decoder)),
            orientation, vertical_causal_context, segmentation_symbols),
))]
fn decode_cleanup(
    plane: &mut CodeBlockPlane,
    state: &mut Tier1BitplaneState,
    decoder: &mut impl Tier1SymbolDecoder,
    orientation: SubbandOrientation,
    vertical_causal_context: bool,
    segmentation_symbols: bool,
) -> Result<Tier1PassStatistics, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost goal = crate::specs::part1_legacy_cleanup_pass(
            code_block_plane_view(plane), legacy_bitplane_view(state), legacy_symbols_view(decoder),
            orientation, vertical_causal_context, segmentation_symbols);
    }
    state.validate_plane(plane)?;
    if state.cleanup_complete
        || state.significance_propagation_complete != state.magnitude_refinement_complete
    {
        return Err(invalid_code_block());
    }

    let mut statistics = ht_statistics_zero();
    let height = plane.height;
    let width = plane.width;
    let mut stripe_y = 0_u16;
    #[cfg_attr(
        verus_keep_ghost,
        verus_spec(
            invariant stripe_y <= height,
                goal == crate::specs::part1_legacy_cleanup_pass(
                    code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)), legacy_symbols_view(old(decoder)),
                    orientation, vertical_causal_context, segmentation_symbols),
                goal == crate::specs::part1_legacy_cleanup_finish(crate::specs::part1_legacy_stripes(
                    legacy_frame_view(plane, state, decoder, statistics), stripe_y as nat, width, height,
                    orientation, vertical_causal_context, 2), segmentation_symbols),
            decreases height - stripe_y,
        )
    )]
    while stripe_y < height {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_stripes); }
        let stripe_rows = crate::refinement::tier1_stripe_rows(height, stripe_y);
        let mut x = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant x <= width,
                    stripe_y < height, stripe_rows > 0, stripe_rows <= 4,
                    stripe_y as nat + stripe_rows as nat <= height,
                    goal == crate::specs::part1_legacy_cleanup_pass(
                        code_block_plane_view(old(plane)), legacy_bitplane_view(old(state)), legacy_symbols_view(old(decoder)),
                        orientation, vertical_causal_context, segmentation_symbols),
                    goal == crate::specs::part1_legacy_cleanup_column_tail(
                        legacy_frame_view(plane, state, decoder, statistics), x as nat, stripe_y, stripe_rows as nat,
                        width, height, orientation, vertical_causal_context, segmentation_symbols),
                decreases width - x,
            )
        )]
        while x < width {
            #[cfg(verus_keep_ghost)]
            proof! { reveal(crate::specs::part1_legacy_columns); }
            decode_cleanup_column(
                plane,
                state,
                decoder,
                (orientation, vertical_causal_context),
                (x, stripe_y, stripe_rows),
                &mut statistics,
            )?;
            x = x.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_columns); }
        let next_stripe = stripe_y
            .checked_add(stripe_rows)
            .ok_or_else(arithmetic_overflow)?;
        if next_stripe <= stripe_y || next_stripe > height {
            return Err(invalid_code_block());
        }
        stripe_y = next_stripe;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part1_legacy_stripes);
    }
    if segmentation_symbols {
        decode_cleanup_segsym(plane, state, decoder, &mut statistics)?;
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost before_finish = legacy_frame_view(plane, state, decoder, statistics); }
    state.cleanup_complete = crate::refinement::tier1_pass_complete(2) & 4 != 0;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(4u8 & 4u8 != 0) by(bit_vector);
        assert((Ok(statistics), code_block_plane_view(plane), legacy_bitplane_view(state), legacy_symbols_view(decoder)) == goal);
    }
    Ok(statistics)
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, legacy_frame_view(_plane, _state, final(decoder), *final(statistics)))
        == crate::specs::part1_legacy_segsym(
            legacy_frame_view(_plane, _state, old(decoder), *old(statistics))),
))]
fn decode_cleanup_segsym(
    _plane: &CodeBlockPlane,
    _state: &Tier1BitplaneState,
    decoder: &mut impl Tier1SymbolDecoder,
    statistics: &mut Tier1PassStatistics,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_legacy_segsym); }
    let first = decoder.decode(UNIFORM_CONTEXT)?;
    let second = decoder.decode(UNIFORM_CONTEXT)?;
    let third = decoder.decode(UNIFORM_CONTEXT)?;
    let fourth = decoder.decode(UNIFORM_CONTEXT)?;
    statistics.decoded_symbols(4)?;
    if !first || second || !third || fourth {
        return Err(invalid_code_block());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, legacy_frame_view(final(plane), state, final(decoder), *final(statistics)))
        == crate::specs::part1_legacy_cleanup_column(
            legacy_frame_view(old(plane), state, old(decoder), *old(statistics)),
            coordinate.0, coordinate.1, coordinate.2, context.0, context.1),
))]
fn decode_cleanup_column(
    plane: &mut CodeBlockPlane,
    state: &Tier1BitplaneState,
    decoder: &mut impl Tier1SymbolDecoder,
    context: (SubbandOrientation, bool),
    coordinate: (u16, u16, u16),
    statistics: &mut Tier1PassStatistics,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part1_legacy_cleanup_column);
        reveal(crate::specs::part1_legacy_cleanup_run);
    }
    let (_orientation, vertical_causal_context) = context;
    let (x, stripe_y, stripe_rows) = coordinate;
    let stripe_y_1 = stripe_y.checked_add(1).ok_or_else(arithmetic_overflow)?;
    let stripe_y_2 = stripe_y.checked_add(2).ok_or_else(arithmetic_overflow)?;
    let stripe_y_3 = stripe_y.checked_add(3).ok_or_else(arithmetic_overflow)?;
    if stripe_rows == 4
        && cleanup_run_candidate(plane, state, x, stripe_y, vertical_causal_context)?
        && cleanup_run_candidate(plane, state, x, stripe_y_1, vertical_causal_context)?
        && cleanup_run_candidate(plane, state, x, stripe_y_2, vertical_causal_context)?
        && cleanup_run_candidate(plane, state, x, stripe_y_3, vertical_causal_context)?
    {
        statistics.scanned(4)?;
        statistics.used_run_mode()?;
        let interrupted = decoder.decode(RUN_LENGTH_CONTEXT)?;
        statistics.decoded_symbols(1)?;
        if interrupted {
            decode_cleanup_interrupt(plane, state, decoder, context, (x, stripe_y), statistics)?;
        }
    } else {
        decode_cleanup_rows(
            plane,
            state,
            decoder,
            context,
            (x, stripe_y, 0, stripe_rows, true),
            statistics,
        )?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, legacy_frame_view(final(plane), state, final(decoder), *final(statistics)))
        == crate::specs::part1_legacy_cleanup_interrupt(
            legacy_frame_view(old(plane), state, old(decoder), *old(statistics)),
            coordinate.0, coordinate.1, context.0, context.1),
))]
fn decode_cleanup_interrupt(
    plane: &mut CodeBlockPlane,
    state: &Tier1BitplaneState,
    decoder: &mut impl Tier1SymbolDecoder,
    context: (SubbandOrientation, bool),
    coordinate: (u16, u16),
    statistics: &mut Tier1PassStatistics,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_legacy_cleanup_interrupt); }
    let (orientation, vertical_causal_context) = context;
    let (x, stripe_y) = coordinate;
    let run_high = decoder.decode(UNIFORM_CONTEXT)?;
    let run_low = decoder.decode(UNIFORM_CONTEXT)?;
    statistics.decoded_symbols(2)?;
    let run_length = if run_high {
        if run_low { 3 } else { 2 }
    } else if run_low {
        1
    } else {
        0
    };
    let y = stripe_y
        .checked_add(run_length)
        .ok_or_else(arithmetic_overflow)?;
    let summary = plane.neighbor_summary_with_causality(x, y, vertical_causal_context)?;
    decode_sign_and_mark(plane, decoder, x, y, state.bitplane, summary)?;
    statistics.decoded_symbols(1)?;
    statistics.became_significant()?;
    let row = run_length.checked_add(1).ok_or_else(arithmetic_overflow)?;
    decode_cleanup_rows(
        plane,
        state,
        decoder,
        (orientation, vertical_causal_context),
        (x, stripe_y, row, 4, false),
        statistics,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, legacy_frame_view(final(plane), state, final(decoder), *final(statistics)))
        == if coordinate.4 {
            crate::specs::part1_legacy_rows(
                legacy_frame_view(old(plane), state, old(decoder), *old(statistics)),
                coordinate.0, coordinate.1, coordinate.2 as nat, coordinate.3 as nat, context.0, context.1, 2)
        } else {
            crate::specs::part1_legacy_cleanup_rows(
                legacy_frame_view(old(plane), state, old(decoder), *old(statistics)),
                coordinate.0, coordinate.1, coordinate.2 as nat, coordinate.3 as nat, context.0, context.1)
        },
))]
fn decode_cleanup_rows(
    plane: &mut CodeBlockPlane,
    state: &Tier1BitplaneState,
    decoder: &mut impl Tier1SymbolDecoder,
    context: (SubbandOrientation, bool),
    coordinate: (u16, u16, u16, u16, bool),
    statistics: &mut Tier1PassStatistics,
) -> Result<(), DecodeError> {
    let (orientation, vertical_causal_context) = context;
    let (x, stripe_y, mut row, rows, scan) = coordinate;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost initial = legacy_frame_view(plane, state, decoder, *statistics);
        let ghost goal = if scan {
            crate::specs::part1_legacy_rows(initial, x, stripe_y, row as nat, rows as nat, orientation, vertical_causal_context, 2)
        } else {
            crate::specs::part1_legacy_cleanup_rows(initial, x, stripe_y, row as nat, rows as nat, orientation, vertical_causal_context)
        };
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant initial == legacy_frame_view(old(plane), state, old(decoder), *old(statistics)),
            goal == if coordinate.4 {
                crate::specs::part1_legacy_rows(initial, coordinate.0, coordinate.1, coordinate.2 as nat, coordinate.3 as nat, context.0, context.1, 2)
            } else {
                crate::specs::part1_legacy_cleanup_rows(initial, coordinate.0, coordinate.1, coordinate.2 as nat, coordinate.3 as nat, context.0, context.1)
            },
            goal == if scan {
                crate::specs::part1_legacy_rows(legacy_frame_view(plane, state, decoder, *statistics),
                    x, stripe_y, row as nat, rows as nat, orientation, vertical_causal_context, 2)
            } else {
                crate::specs::part1_legacy_cleanup_rows(legacy_frame_view(plane, state, decoder, *statistics),
                    x, stripe_y, row as nat, rows as nat, orientation, vertical_causal_context)
            },
        decreases rows as int - row as int,
    ))]
    while row < rows {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::specs::part1_legacy_rows);
            reveal(crate::specs::part1_legacy_cleanup_rows);
        }
        let y = match stripe_y.checked_add(row) {
            Some(y) => y,
            None => return Err(arithmetic_overflow()),
        };
        if scan {
            statistics.scanned(1)?;
        }
        decode_cleanup_coefficient(
            plane,
            state,
            decoder,
            orientation,
            vertical_causal_context,
            (x, y),
            statistics,
        )?;
        row = row.checked_add(1).ok_or_else(arithmetic_overflow)?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::specs::part1_legacy_rows);
        reveal(crate::specs::part1_legacy_cleanup_rows);
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, legacy_frame_view(final(plane), state, final(decoder), *final(statistics)))
        == crate::specs::part1_legacy_cleanup_cell(
            legacy_frame_view(old(plane), state, old(decoder), *old(statistics)),
            coordinate.0, coordinate.1, orientation, vertical_causal_context),
))]
fn decode_cleanup_coefficient(
    plane: &mut CodeBlockPlane,
    state: &Tier1BitplaneState,
    decoder: &mut impl Tier1SymbolDecoder,
    orientation: SubbandOrientation,
    vertical_causal_context: bool,
    coordinate: (u16, u16),
    statistics: &mut Tier1PassStatistics,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_legacy_cleanup_cell); }
    let (x, y) = coordinate;
    let index = plane.index(x, y)?;
    if index >= state.significance_visited.len() {
        return Err(invalid_code_block());
    }
    let coefficient = plane.coefficient(x, y)?;
    if !crate::refinement::tier1_cleanup_eligible(
        coefficient.significant,
        state.significance_visited[index],
    ) {
        return Ok(());
    }
    let summary = plane.neighbor_summary_with_causality(x, y, vertical_causal_context)?;
    let zero_context = zero_coding_context(summary, orientation);
    let significant = decoder.decode(usize::from(zero_context))?;
    statistics.decoded_symbols(1)?;
    if significant {
        decode_sign_and_mark(plane, decoder, x, y, state.bitplane, summary)?;
        statistics.decoded_symbols(1)?;
        statistics.became_significant()?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures (result, code_block_plane_view(final(plane)), legacy_symbols_view(final(decoder)))
        == crate::specs::part1_legacy_sign_mark(
            code_block_plane_view(old(plane)), legacy_symbols_view(old(decoder)), x, y, bitplane, summary),
))]
fn decode_sign_and_mark(
    plane: &mut CodeBlockPlane,
    decoder: &mut impl Tier1SymbolDecoder,
    x: u16,
    y: u16,
    bitplane: u16,
    summary: NeighborSummary,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_legacy_sign_mark); }
    let sign = sign_context(summary);
    let raw = decoder.is_raw();
    let decoded_sign = decoder.decode(usize::from(sign.context))?;
    plane.mark_significant_at_bitplane(
        x,
        y,
        bitplane,
        crate::refinement::tier1_decoded_sign(decoded_sign, sign.predicted_negative, raw),
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_legacy_cleanup_candidate(
        code_block_plane_view(plane), legacy_bitplane_view(state), x, y, vertical_causal_context),
))]
fn cleanup_run_candidate(
    plane: &CodeBlockPlane,
    state: &Tier1BitplaneState,
    x: u16,
    y: u16,
    vertical_causal_context: bool,
) -> Result<bool, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { reveal(crate::specs::part1_legacy_cleanup_candidate); }
    let index = plane.index(x, y)?;
    if index >= state.significance_visited.len() {
        return Err(invalid_code_block());
    }
    Ok(!plane.coefficient(x, y)?.significant
        && !state.significance_visited[index]
        && significant_neighbor_count(plane.neighbor_summary_with_causality(
            x,
            y,
            vertical_causal_context,
        )?) == 0)
}

#[cfg(verus_keep_ghost)]
verus! {
    closed spec fn legacy_symbols_view<D: Tier1SymbolDecoder>(decoder: &D) -> crate::specs::Part1LegacySymbols {
        decoder.symbols_view()
    }

    closed spec fn legacy_frame_view<D: Tier1SymbolDecoder>(plane: &CodeBlockPlane, state: &Tier1BitplaneState,
        decoder: &D, statistics: Tier1PassStatistics) -> crate::specs::Part1LegacyFrame {
        crate::specs::Part1LegacyFrame {
            plane: code_block_plane_view(plane), state: legacy_bitplane_view(state),
            symbols: legacy_symbols_view(decoder), statistics,
        }
    }
}

trait Tier1SymbolDecoder {
    #[cfg(verus_keep_ghost)]
    verus! {
        spec fn symbols_view(&self) -> crate::specs::Part1LegacySymbols;
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, final(self).symbols_view()) == crate::specs::part1_legacy_symbol(
            old(self).symbols_view(), context),
    ))]
    fn decode(&mut self, context: usize) -> Result<bool, DecodeError>;

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == (self.symbols_view() is Raw),
    ))]
    fn is_raw(&self) -> bool;
}

impl Tier1SymbolDecoder for MqDecoder<'_> {
    #[cfg(verus_keep_ghost)]
    verus! {
        spec fn symbols_view(&self) -> crate::specs::Part1LegacySymbols {
            crate::specs::Part1LegacySymbols::Mq(crate::mq::mq_decoder_view(self))
        }
    }

    fn decode(&mut self, context: usize) -> Result<bool, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_symbol); }
        MqDecoder::decode(self, context)
    }

    fn is_raw(&self) -> bool {
        false
    }
}

impl Tier1SymbolDecoder for RawBitReader<'_> {
    #[cfg(verus_keep_ghost)]
    verus! {
        spec fn symbols_view(&self) -> crate::specs::Part1LegacySymbols {
            crate::specs::Part1LegacySymbols::Raw(raw_bit_reader_view(self))
        }
    }

    fn decode(&mut self, _context: usize) -> Result<bool, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(crate::specs::part1_legacy_symbol); }
        self.read_bit()
    }

    fn is_raw(&self) -> bool {
        true
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result as nat == crate::specs::ht_significant_neighbor_count(summary),
        result as nat == summary.horizontal_significant as nat
            + summary.vertical_significant as nat + summary.diagonal_significant as nat,
))]
fn significant_neighbor_count(summary: NeighborSummary) -> u16 {
    u16::from(summary.horizontal_significant)
        + u16::from(summary.vertical_significant)
        + u16::from(summary.diagonal_significant)
}

const fn is_bitplane_mask(bitplane_mask: u64) -> bool {
    bitplane_mask != 0 && bitplane_mask & (bitplane_mask - 1) == 0
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result => ensures result == crate::specs::ht_reader_error(MalformedKind::InvalidCodeBlock),)
)]
fn invalid_code_block() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::InvalidCodeBlock,
    }
}

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result => ensures result == crate::specs::ht_reader_error(MalformedKind::ArithmeticOverflow),)
)]
fn arithmetic_overflow() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}

pub fn apply_significance_decision(
    plane: &mut CodeBlockPlane,
    x: u16,
    y: u16,
    bitplane_mask: u64,
    orientation: SubbandOrientation,
    significant: bool,
    decoded_sign: bool,
) -> Result<SignificanceStep, DecodeError> {
    if bitplane_mask == 0 {
        return Err(DecodeError::malformed(0, MalformedKind::InvalidCodeBlock));
    }
    let summary = plane.neighbor_summary(x, y)?;
    let zero_context = zero_coding_context(summary, orientation);
    if !significant {
        return Ok(SignificanceStep {
            zero_coding_context: zero_context,
            sign_context: None,
            became_significant: false,
        });
    }
    let sign = sign_context(summary);
    plane.mark_significant(x, y, bitplane_mask, decoded_sign ^ sign.predicted_negative)?;
    Ok(SignificanceStep {
        zero_coding_context: zero_context,
        sign_context: Some(sign.context),
        became_significant: true,
    })
}

#[derive(Clone, Copy)]
enum NeighborKind {
    Horizontal,
    Vertical,
    Diagonal,
}

#[cfg(verus_keep_ghost)]
verus! {
    spec fn neighbor_kind_tag(kind: NeighborKind) -> u8 {
        match kind { NeighborKind::Horizontal => 0, NeighborKind::Vertical => 1, NeighborKind::Diagonal => 2 }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == crate::specs::ht_neighbor_zero(),))]
fn ht_neighbor_summary_zero() -> NeighborSummary {
    NeighborSummary {
        horizontal_significant: 0,
        vertical_significant: 0,
        diagonal_significant: 0,
        horizontal_sign_balance: 0,
        vertical_sign_balance: 0,
    }
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct CodeBlockPlane {
    width: u16,
    height: u16,
    coefficients: Vec<CoefficientState>,
}

impl Clone for CodeBlockPlane {
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures code_block_plane_view(&result) == code_block_plane_view(self),
    ))]
    fn clone(&self) -> Self {
        Self {
            width: self.width,
            height: self.height,
            coefficients: self.coefficients.clone(),
        }
    }
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn code_block_plane_view(plane: &CodeBlockPlane) -> crate::specs::Tier1RawPlane {
        crate::specs::Tier1RawPlane {
            width: plane.width, height: plane.height, coefficients: coefficient_raw_views(plane.coefficients@),
        }
    }

    pub closed spec fn tier1_decoder_plane_view(decoder: &Tier1CodeBlockDecoder) -> crate::specs::Tier1RawPlane {
        code_block_plane_view(&decoder.plane)
    }

    pub proof fn tier1_decoder_plane_matches_state(decoder: &Tier1CodeBlockDecoder)
        ensures tier1_decoder_plane_view(decoder) == tier1_decoder_state(decoder).plane,
    {
        reveal(tier1_decoder_plane_view);
        reveal(tier1_decoder_state);
    }

    pub closed spec fn code_block_plane_outcome(result: Result<CodeBlockPlane, DecodeError>)
        -> Result<crate::specs::Tier1RawPlane, DecodeError>
    {
        match result { Err(error) => Err(error), Ok(plane) => Ok(code_block_plane_view(&plane)) }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures code_block_plane_outcome(result) == crate::specs::tier1_empty_plane(width, height),
))]
fn code_block_plane_from_dimensions(
    width: u16,
    height: u16,
) -> Result<CodeBlockPlane, DecodeError> {
    if width == 0 || height == 0 {
        return Err(invalid_code_block());
    }
    let samples = usize::from(width)
        .checked_mul(usize::from(height))
        .ok_or_else(arithmetic_overflow)?;
    if samples > 4096 {
        return Err(invalid_code_block());
    }
    let empty = CoefficientState {
        magnitude: 0,
        magnitude_high: [0; 5],
        legacy_bitplane: None,
        negative: false,
        significant: false,
        refined: false,
    };
    let coefficients = vec![empty; samples];
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(coefficient_raw_view(empty).limbs =~= Seq::new(6, |i: int| 0u64));
        assert(coefficient_raw_view(empty) == crate::specs::tier1_empty_coefficient());
        assert forall |i: int| 0 <= i < samples implies coefficient_raw_view(coefficients@[i])
            == crate::specs::tier1_empty_coefficient() by {
            assert(coefficients@[i] == empty);
        }
        assert(coefficient_raw_views(coefficients@) =~= Seq::new(samples as nat, |i: int| crate::specs::tier1_empty_coefficient()));
    }
    Ok(CodeBlockPlane {
        width,
        height,
        coefficients,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(result => ensures result == crate::specs::tier1_statistics_zero(),))]
fn ht_statistics_zero() -> Tier1PassStatistics {
    Tier1PassStatistics {
        coefficients_scanned: 0,
        coding_symbols: 0,
        newly_significant: 0,
        refined_coefficients: 0,
        run_mode_columns: 0,
    }
}

impl CodeBlockPlane {
    pub fn new(width: u16, height: u16) -> Result<Self, DecodeError> {
        code_block_plane_from_dimensions(width, height)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures code_block_plane_outcome(result) == crate::specs::tier1_duplicate_plane(code_block_plane_view(self)),
    ))]
    fn duplicate(&self) -> Result<Self, DecodeError> {
        let mut output = code_block_plane_from_dimensions(self.width, self.height)?;
        if output.coefficients.len() != self.coefficients.len() {
            return Err(invalid_code_block());
        }
        let mut index = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.coefficients.len(),
                    output.width == self.width, output.height == self.height,
                    output.coefficients.len() == self.coefficients.len(),
                    forall |i: int| 0 <= i < index ==> coefficient_raw_view(output.coefficients@[i])
                        == coefficient_raw_view(self.coefficients@[i]),
                decreases self.coefficients.len() - index,
            )
        )]
        while index < self.coefficients.len() {
            *output
                .coefficients
                .get_mut(index)
                .ok_or_else(invalid_code_block)? = *self
                .coefficients
                .get(index)
                .ok_or_else(invalid_code_block)?;
            index = index.checked_add(1).ok_or_else(arithmetic_overflow)?;
        }
        #[cfg(verus_keep_ghost)]
        proof! { assert(code_block_plane_view(&output).coefficients =~= code_block_plane_view(self).coefficients); }
        Ok(output)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::tier1_raw_plane_width(code_block_plane_view(self)),
            result == code_block_plane_view(self).width,
    ))]
    pub const fn width(&self) -> u16 {
        self.width
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::tier1_raw_plane_height(code_block_plane_view(self)),
            result == code_block_plane_view(self).height,
    ))]
    pub const fn height(&self) -> u16 {
        self.height
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures coefficient_raw_outcome(result) == crate::specs::tier1_raw_plane_coefficient(code_block_plane_view(self), x, y),
    ))]
    pub fn coefficient(&self, x: u16, y: u16) -> Result<CoefficientState, DecodeError> {
        let index = self.index(x, y)?;
        if index >= self.coefficients.len() {
            return Err(invalid_code_block());
        }
        Ok(self.coefficients[index])
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures coefficient_slice_view(result) == crate::specs::tier1_raw_plane_coefficients(code_block_plane_view(self)),
            coefficient_raw_views(result@) == code_block_plane_view(self).coefficients,
    ))]
    pub fn coefficients(&self) -> &[CoefficientState] {
        &self.coefficients
    }

    /// Returns the reconstructed integer coefficient, including any legacy
    /// truncation midpoint; `coefficient` retains the decoded magnitude bits.
    pub fn signed_coefficient(&self, x: u16, y: u16) -> Result<i64, DecodeError> {
        let coefficient = self.coefficient(x, y)?;
        if !coefficient.significant {
            return Ok(0);
        }
        let magnitude = coefficient
            .signed_magnitude()?
            .magnitude
            .to_u64()
            .and_then(|value| i64::try_from(value).ok())
            .ok_or_else(arithmetic_overflow)?;
        if coefficient.negative {
            if magnitude == i64::MIN {
                Err(arithmetic_overflow())
            } else {
                Ok(-magnitude)
            }
        } else {
            Ok(magnitude)
        }
    }

    pub fn mark_significant(
        &mut self,
        x: u16,
        y: u16,
        magnitude: u64,
        negative: bool,
    ) -> Result<(), DecodeError> {
        self.mark_significant_magnitude(x, y, Magnitude::from_u64(magnitude), negative)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, code_block_plane_view(final(self))) == crate::specs::part1_legacy_mark(
            code_block_plane_view(old(self)), x, y, bitplane, negative),
            final(self).width == old(self).width, final(self).height == old(self).height,
    ))]
    fn mark_significant_at_bitplane(
        &mut self,
        x: u16,
        y: u16,
        bitplane: u16,
        negative: bool,
    ) -> Result<(), DecodeError> {
        self.mark_significant_magnitude(
            x,
            y,
            crate::magnitude::magnitude_single_bit(bitplane)?,
            negative,
        )?;
        let index = self.index(x, y)?;
        self.coefficients
            .get_mut(index)
            .ok_or_else(invalid_code_block)?
            .legacy_bitplane = Some(bitplane);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(code_block_plane_view(self).coefficients =~=
                crate::specs::part1_legacy_mark(code_block_plane_view(old(self)), x, y, bitplane, negative).1.coefficients);
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, code_block_plane_view(final(self))) == crate::specs::tier1_mark_coefficient(
            code_block_plane_view(old(self)), x, y, crate::magnitude::magnitude_limbs(magnitude),
            crate::magnitude::magnitude_value(magnitude), negative),
    ))]
    fn mark_significant_magnitude(
        &mut self,
        x: u16,
        y: u16,
        magnitude: Magnitude,
        negative: bool,
    ) -> Result<(), DecodeError> {
        if magnitude.is_zero() {
            return Err(invalid_code_block());
        }
        let index = self.index(x, y)?;
        if index >= self.coefficients.len() {
            return Err(invalid_code_block());
        }
        if self.coefficients[index].significant {
            return Err(invalid_code_block());
        }
        self.coefficients[index] = coefficient_from_magnitude(magnitude, negative, true, false)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(coefficient_raw_views(self.coefficients@) =~=
                code_block_plane_view(old(self)).coefficients.update(index as int,
                    crate::specs::tier1_new_coefficient(crate::magnitude::magnitude_limbs(magnitude), negative, true, false)));
        }
        Ok(())
    }

    pub fn refine(&mut self, x: u16, y: u16, bitplane_mask: u64) -> Result<(), DecodeError> {
        self.apply_refinement_decision(x, y, bitplane_mask, true)
    }

    pub fn apply_refinement_decision(
        &mut self,
        x: u16,
        y: u16,
        bitplane_mask: u64,
        refinement_bit: bool,
    ) -> Result<(), DecodeError> {
        if !is_bitplane_mask(bitplane_mask) {
            return Err(invalid_code_block());
        }
        let bitplane =
            u16::try_from(bitplane_mask.trailing_zeros()).map_err(|_| arithmetic_overflow())?;
        self.apply_refinement_bit(x, y, bitplane, refinement_bit)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, code_block_plane_view(final(self))) == crate::specs::part1_legacy_refine(
            code_block_plane_view(old(self)), x, y, bitplane, refinement_bit),
            final(self).width == old(self).width, final(self).height == old(self).height,
    ))]
    fn apply_refinement_bit(
        &mut self,
        x: u16,
        y: u16,
        bitplane: u16,
        refinement_bit: bool,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost start = code_block_plane_view(self); }
        if bitplane >= MAX_TIER1_BITPLANES {
            return Err(invalid_code_block());
        }
        let index = self.index(x, y)?;
        if index >= self.coefficients.len() {
            return Err(invalid_code_block());
        }
        let coefficient = self.coefficients[index];
        if !coefficient.significant {
            return Err(invalid_code_block());
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_magnitude_value_bound(coefficient_raw_view(coefficient).limbs);
        }
        if refinement_bit {
            let current = coefficient.wide_magnitude();
            let delta = crate::magnitude::magnitude_single_bit(bitplane)?;
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::magnitude::representation_proofs::magnitude_limbs_shape(current);
                crate::magnitude::representation_proofs::magnitude_limbs_shape(delta);
                crate::proofs::production_impl::production_legacy_magnitude_u64(crate::magnitude::magnitude_limbs(current));
                crate::proofs::production_impl::production_legacy_magnitude_u64(crate::magnitude::magnitude_limbs(delta));
            }
            let magnitude = match (current.to_u64(), delta.to_u64()) {
                (Some(current), Some(delta)) if current <= u64::MAX - delta => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(current as nat == crate::specs::part1_magnitude_value(coefficient_raw_view(coefficient).limbs));
                        assert(delta as nat == vstd::arithmetic::power2::pow2(bitplane as nat));
                    }
                    crate::magnitude::magnitude_from_u64(
                        crate::refinement::tier1_refined_magnitude(current, delta, true),
                    )
                }
                _ => current.checked_add(delta)?,
            };
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(crate::magnitude::magnitude_value(magnitude) ==
                    crate::specs::part1_magnitude_value(coefficient_raw_view(coefficient).limbs)
                        + vstd::arithmetic::power2::pow2(bitplane as nat));
                crate::magnitude::representation_proofs::magnitude_limbs_shape(magnitude);
                crate::proofs::production_impl::production_magnitude_value_bound(crate::magnitude::magnitude_limbs(magnitude));
                crate::proofs::production_impl::production_tier1_value_limbs(crate::magnitude::magnitude_limbs(magnitude));
            }
            self.coefficients[index].set_magnitude(magnitude)?;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(coefficient_raw_view(self.coefficients[index as int]).limbs
                    == crate::specs::tier1_value_limbs(
                        crate::specs::part1_magnitude_value(coefficient_raw_view(coefficient).limbs)
                            + vstd::arithmetic::power2::pow2(bitplane as nat)));
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(crate::specs::part1_legacy_refine(start, x, y, bitplane, refinement_bit).0 == Ok(()));
            assert(coefficient_raw_view(self.coefficients[index as int]) == (crate::specs::Tier1RawCoefficient {
                limbs: if refinement_bit {
                    crate::specs::tier1_value_limbs(crate::specs::part1_magnitude_value(coefficient_raw_view(coefficient).limbs)
                        + vstd::arithmetic::power2::pow2(bitplane as nat))
                } else { coefficient_raw_view(coefficient).limbs },
                ..coefficient_raw_view(coefficient)
            }));
        }
        self.coefficients[index].refined = true;
        if coefficient.legacy_bitplane.is_some() {
            self.coefficients[index].legacy_bitplane = Some(bitplane);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(coefficient_raw_view(self.coefficients[index as int])
                == crate::specs::part1_legacy_refine(start, x, y, bitplane, refinement_bit).1.coefficients[index as int]);
            assert(code_block_plane_view(self).coefficients =~=
                crate::specs::part1_legacy_refine(start, x, y, bitplane, refinement_bit).1.coefficients);
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, code_block_plane_view(final(self))) == crate::specs::ht_apply_refinement(
            code_block_plane_view(old(self)), x, y, crate::magnitude::magnitude_value(delta), refinement_bit),
            final(self).width == old(self).width, final(self).height == old(self).height,
    ))]
    fn apply_ht_refinement_decision(
        &mut self,
        x: u16,
        y: u16,
        delta: Magnitude,
        refinement_bit: bool,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost start = code_block_plane_view(self); }
        if delta.is_zero() && !refinement_bit {
            return Err(invalid_code_block());
        }
        let index = self.index(x, y)?;
        let coefficient = *self
            .coefficients
            .get(index)
            .ok_or_else(invalid_code_block)?;
        if !coefficient.significant {
            return Err(invalid_code_block());
        }
        let magnitude = if refinement_bit {
            coefficient.wide_magnitude().checked_add(delta)?
        } else {
            match coefficient.wide_magnitude().checked_sub(delta) {
                Ok(value) => value,
                Err(_) => return Err(invalid_code_block()),
            }
        };
        if magnitude.is_zero() {
            return Err(invalid_code_block());
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::magnitude::representation_proofs::magnitude_limbs_shape(magnitude);
            crate::proofs::production_impl::production_tier1_value_limbs(crate::magnitude::magnitude_limbs(magnitude));
        }
        let output = self
            .coefficients
            .get_mut(index)
            .ok_or_else(invalid_code_block)?;
        output.set_magnitude(magnitude)?;
        output.refined = true;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(code_block_plane_view(self).coefficients =~= start.coefficients.update(index as int,
                crate::specs::Tier1RawCoefficient {
                    limbs: crate::specs::tier1_value_limbs(crate::magnitude::magnitude_value(magnitude)), refined: true,
                    ..start.coefficients[index as int]
                }));
        }
        Ok(())
    }

    pub fn neighbor_summary(&self, x: u16, y: u16) -> Result<NeighborSummary, DecodeError> {
        self.neighbor_summary_with_causality(x, y, false)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::ht_neighbors(code_block_plane_view(self), x, y, vertical_causal_context),
    ))]
    fn neighbor_summary_with_causality(
        &self,
        x: u16,
        y: u16,
        vertical_causal_context: bool,
    ) -> Result<NeighborSummary, DecodeError> {
        self.index(x, y)?;
        let mut summary = ht_neighbor_summary_zero();
        self.accumulate_neighbor(x, y, (-1, 0), NeighborKind::Horizontal, &mut summary)?;
        self.accumulate_neighbor(x, y, (1, 0), NeighborKind::Horizontal, &mut summary)?;
        self.accumulate_neighbor(x, y, (0, -1), NeighborKind::Vertical, &mut summary)?;
        self.accumulate_neighbor(x, y, (-1, -1), NeighborKind::Diagonal, &mut summary)?;
        self.accumulate_neighbor(x, y, (1, -1), NeighborKind::Diagonal, &mut summary)?;
        if !vertical_causal_context || y % 4 != 3 {
            self.accumulate_neighbor(x, y, (0, 1), NeighborKind::Vertical, &mut summary)?;
            self.accumulate_neighbor(x, y, (-1, 1), NeighborKind::Diagonal, &mut summary)?;
            self.accumulate_neighbor(x, y, (1, 1), NeighborKind::Diagonal, &mut summary)?;
        }
        Ok(summary)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::tier1_raw_plane_index(code_block_plane_view(self), x, y),
    ))]
    fn index(&self, x: u16, y: u16) -> Result<usize, DecodeError> {
        if x >= self.width || y >= self.height {
            return Err(invalid_code_block());
        }
        let index = usize::from(y)
            .checked_mul(usize::from(self.width))
            .ok_or_else(arithmetic_overflow)?
            .checked_add(usize::from(x))
            .ok_or_else(arithmetic_overflow)?;
        if index >= self.coefficients.len() {
            return Err(invalid_code_block());
        }
        Ok(index)
    }

    // Specialize the fixed offsets and neighbor kinds in the hot context traversal.
    #[inline(always)]
    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures (result, *final(summary)) == crate::specs::ht_accumulate_neighbor(
            code_block_plane_view(self), x, y, delta.0, delta.1, neighbor_kind_tag(kind), *old(summary)),
            result == Ok(()) || result.is_err(),
    ))]
    fn accumulate_neighbor(
        &self,
        x: u16,
        y: u16,
        delta: (i8, i8),
        kind: NeighborKind,
        summary: &mut NeighborSummary,
    ) -> Result<(), DecodeError> {
        let neighbor_x = i32::from(x) + i32::from(delta.0);
        let neighbor_y = i32::from(y) + i32::from(delta.1);
        if neighbor_x < 0
            || neighbor_y < 0
            || neighbor_x >= i32::from(self.width)
            || neighbor_y >= i32::from(self.height)
        {
            return Ok(());
        }
        let coefficient = self.coefficient(
            u16::try_from(neighbor_x)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
            u16::try_from(neighbor_y)
                .map_err(|_| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?,
        )?;
        if !coefficient.significant {
            return Ok(());
        }
        let sign = if coefficient.negative { -1 } else { 1 };
        if matches!(kind, NeighborKind::Horizontal) {
            summary.horizontal_significant = summary
                .horizontal_significant
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
            summary.horizontal_sign_balance = summary
                .horizontal_sign_balance
                .checked_add(sign)
                .ok_or_else(arithmetic_overflow)?;
        } else if matches!(kind, NeighborKind::Vertical) {
            summary.vertical_significant = summary
                .vertical_significant
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
            summary.vertical_sign_balance = summary
                .vertical_sign_balance
                .checked_add(sign)
                .ok_or_else(arithmetic_overflow)?;
        } else {
            summary.diagonal_significant = summary
                .diagonal_significant
                .checked_add(1)
                .ok_or_else(arithmetic_overflow)?;
        }
        Ok(())
    }
}

impl CodeBlockSegmentAccounting {
    pub fn new(maximum_passes: u32, limits: &ResourceLimits) -> Result<Self, DecodeError> {
        if maximum_passes == 0 {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodeBlock));
        }
        Ok(Self {
            maximum_passes,
            maximum_bytes: limits.maximum_input_bytes(),
            total_passes: 0,
            total_bytes: 0,
            segments: 0,
        })
    }

    pub fn add_segment(&mut self, passes: u32, bytes: u32) -> Result<Tier1PassKind, DecodeError> {
        if passes == 0 || bytes == 0 {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodeBlock));
        }
        let next_passes = self
            .total_passes
            .checked_add(passes)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let next_bytes = self
            .total_bytes
            .checked_add(u64::from(bytes))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        let next_segments = self
            .segments
            .checked_add(1)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        if next_passes > self.maximum_passes {
            return Err(DecodeError::malformed(0, MalformedKind::InvalidCodeBlock));
        }
        if next_bytes > self.maximum_bytes {
            return Err(DecodeError::limit(
                Resource::InputBytes,
                self.maximum_bytes,
                next_bytes,
            ));
        }
        let first_pass = pass_kind(self.total_passes);
        self.total_passes = next_passes;
        self.total_bytes = next_bytes;
        self.segments = next_segments;
        Ok(first_pass)
    }

    pub const fn total_passes(&self) -> u32 {
        self.total_passes
    }

    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    pub const fn segments(&self) -> u32 {
        self.segments
    }
}

#[cfg(test)]
mod tests;
