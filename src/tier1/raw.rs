use super::RawBitReader;
#[cfg(verus_keep_ghost)]
use super::{raw_bit_reader_view, raw_reader_outcome};
use crate::{DecodeError, MalformedKind, Resource, ResourceLimits};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(
    verus_keep_ghost,
    verus_spec(result =>
        ensures raw_reader_outcome(result) == crate::specs::part1_raw_reader_initial(
            input@, maximum_input_bytes, maximum_work_units,
        ),
    )
)]
pub(super) fn raw_reader_from_input(
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
