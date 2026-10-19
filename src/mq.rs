use crate::{DecodeError, MalformedKind, Resource, ResourceLimits};

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

const CONTEXT_COUNT: usize = 19;

#[derive(Clone, Copy)]
struct ProbabilityState {
    q_e: u16,
    next_mps: u8,
    next_lps: u8,
    switch_mps: bool,
}

const STATES: [ProbabilityState; 47] = [
    ProbabilityState {
        q_e: 0x5601,
        next_mps: 1,
        next_lps: 1,
        switch_mps: true,
    },
    ProbabilityState {
        q_e: 0x3401,
        next_mps: 2,
        next_lps: 6,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x1801,
        next_mps: 3,
        next_lps: 9,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0ac1,
        next_mps: 4,
        next_lps: 12,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0521,
        next_mps: 5,
        next_lps: 29,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0221,
        next_mps: 38,
        next_lps: 33,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x5601,
        next_mps: 7,
        next_lps: 6,
        switch_mps: true,
    },
    ProbabilityState {
        q_e: 0x5401,
        next_mps: 8,
        next_lps: 14,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x4801,
        next_mps: 9,
        next_lps: 14,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x3801,
        next_mps: 10,
        next_lps: 14,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x3001,
        next_mps: 11,
        next_lps: 17,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x2401,
        next_mps: 12,
        next_lps: 18,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x1c01,
        next_mps: 13,
        next_lps: 20,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x1601,
        next_mps: 29,
        next_lps: 21,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x5601,
        next_mps: 15,
        next_lps: 14,
        switch_mps: true,
    },
    ProbabilityState {
        q_e: 0x5401,
        next_mps: 16,
        next_lps: 14,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x5101,
        next_mps: 17,
        next_lps: 15,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x4801,
        next_mps: 18,
        next_lps: 16,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x3801,
        next_mps: 19,
        next_lps: 17,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x3401,
        next_mps: 20,
        next_lps: 18,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x3001,
        next_mps: 21,
        next_lps: 19,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x2801,
        next_mps: 22,
        next_lps: 19,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x2401,
        next_mps: 23,
        next_lps: 20,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x2201,
        next_mps: 24,
        next_lps: 21,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x1c01,
        next_mps: 25,
        next_lps: 22,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x1801,
        next_mps: 26,
        next_lps: 23,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x1601,
        next_mps: 27,
        next_lps: 24,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x1401,
        next_mps: 28,
        next_lps: 25,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x1201,
        next_mps: 29,
        next_lps: 26,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x1101,
        next_mps: 30,
        next_lps: 27,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0ac1,
        next_mps: 31,
        next_lps: 28,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x09c1,
        next_mps: 32,
        next_lps: 29,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x08a1,
        next_mps: 33,
        next_lps: 30,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0521,
        next_mps: 34,
        next_lps: 31,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0441,
        next_mps: 35,
        next_lps: 32,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x02a1,
        next_mps: 36,
        next_lps: 33,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0221,
        next_mps: 37,
        next_lps: 34,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0141,
        next_mps: 38,
        next_lps: 35,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0111,
        next_mps: 39,
        next_lps: 36,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0085,
        next_mps: 40,
        next_lps: 37,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0049,
        next_mps: 41,
        next_lps: 38,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0025,
        next_mps: 42,
        next_lps: 39,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0015,
        next_mps: 43,
        next_lps: 40,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0009,
        next_mps: 44,
        next_lps: 41,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0005,
        next_mps: 45,
        next_lps: 42,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x0001,
        next_mps: 45,
        next_lps: 43,
        switch_mps: false,
    },
    ProbabilityState {
        q_e: 0x5601,
        next_mps: 46,
        next_lps: 46,
        switch_mps: false,
    },
];

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct MqContext {
    state: u8,
    mps: bool,
}

#[derive(Clone, Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct MqContextState {
    contexts: [MqContext; CONTEXT_COUNT],
}

#[cfg_attr(not(verus_keep_ghost), derive(Debug))]
pub struct MqDecoder<'a> {
    input: &'a [u8],
    byte_index: usize,
    a: u32,
    c: u32,
    ct: u8,
    contexts: [MqContext; CONTEXT_COUNT],
    work_units: u64,
    maximum_work_units: u64,
    marker_padding_count: u64,
}

impl<'a> MqDecoder<'a> {
    pub fn new(input: &'a [u8], limits: &ResourceLimits) -> Result<Self, DecodeError> {
        let input_length = input.len() as u64;
        if input_length > limits.maximum_input_bytes() {
            return Err(DecodeError::limit(
                Resource::InputBytes,
                limits.maximum_input_bytes(),
                input_length,
            ));
        }
        let mut decoder = Self {
            input,
            byte_index: 0,
            a: 0x8000,
            c: if input.is_empty() {
                0x00ff_0000
            } else {
                u32::from(input[0]) << 16
            },
            ct: 0,
            contexts: [MqContext {
                state: 0,
                mps: false,
            }; CONTEXT_COUNT],
            work_units: 0,
            maximum_work_units: limits.maximum_work_units(),
            marker_padding_count: 0,
        };
        decoder.byte_in()?;
        decoder.c = decoder
            .c
            .checked_mul(128)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::ArithmeticOverflow))?;
        decoder.ct = decoder
            .ct
            .checked_sub(7)
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidMqState))?;
        Ok(decoder)
    }

    pub fn set_context(&mut self, context: usize, state: u8, mps: bool) -> Result<(), DecodeError> {
        if context >= self.contexts.len() || usize::from(state) >= STATES.len() {
            return Err(DecodeError::malformed(
                self.byte_index as u64,
                MalformedKind::InvalidMqState,
            ));
        }
        self.contexts[context] = MqContext { state, mps };
        Ok(())
    }

    pub fn reset_contexts(&mut self) {
        self.contexts = [MqContext {
            state: 0,
            mps: false,
        }; CONTEXT_COUNT];
    }

    pub const fn context_state(&self) -> MqContextState {
        MqContextState {
            contexts: self.contexts,
        }
    }

    pub fn restore_context_state(&mut self, state: MqContextState) {
        self.contexts = state.contexts;
    }

    pub fn context(&self, context: usize) -> Result<MqContext, DecodeError> {
        if context >= self.contexts.len() {
            return Err(DecodeError::malformed(
                self.byte_index as u64,
                MalformedKind::InvalidMqState,
            ));
        }
        Ok(self.contexts[context])
    }

    pub const fn bytes_consumed(&self) -> usize {
        if self.byte_index >= self.input.len() {
            self.input.len()
        } else {
            self.byte_index + 1
        }
    }

    pub const fn marker_padding_count(&self) -> u64 {
        self.marker_padding_count
    }

    pub fn validate_predictable_termination(&self) -> Result<(), DecodeError> {
        let remaining = self.input.len().saturating_sub(self.bytes_consumed());
        if remaining > 2 || self.marker_padding_count > 2 {
            return Err(DecodeError::malformed(
                self.byte_index as u64,
                MalformedKind::InvalidCodeBlock,
            ));
        }
        Ok(())
    }

    pub fn decode(&mut self, context_index: usize) -> Result<bool, DecodeError> {
        if context_index >= self.contexts.len() {
            return Err(DecodeError::malformed(
                self.byte_index as u64,
                MalformedKind::InvalidMqState,
            ));
        }
        self.charge_work()?;
        let context = self.contexts[context_index];
        let state_index = usize::from(context.state);
        if state_index >= STATES.len() {
            return Err(DecodeError::malformed(
                self.byte_index as u64,
                MalformedKind::InvalidMqState,
            ));
        }
        let probability = STATES[state_index];
        if u32::from(probability.q_e) > self.a {
            return Err(DecodeError::malformed(
                self.byte_index as u64,
                MalformedKind::InvalidMqState,
            ));
        }
        self.a = crate::refinement::mq_interval_subtract(self.a, probability.q_e);

        let decision;
        if (self.c >> 16) < u32::from(probability.q_e) {
            if self.a < u32::from(probability.q_e) {
                self.a = u32::from(probability.q_e);
                decision = crate::refinement::mq_decision(context.mps, false);
                self.transition(context_index, probability.next_mps, false)?;
            } else {
                self.a = u32::from(probability.q_e);
                decision = crate::refinement::mq_decision(context.mps, true);
                self.transition(context_index, probability.next_lps, probability.switch_mps)?;
            }
            self.renormalize()?;
        } else {
            let qe_code = u64::from(probability.q_e) * 65_536;
            if qe_code > u64::from(self.c) {
                return Err(DecodeError::malformed(
                    self.byte_index as u64,
                    MalformedKind::InvalidMqState,
                ));
            }
            self.c = crate::refinement::mq_code_subtract(self.c, probability.q_e);
            if self.a & 0x8000 == 0 {
                if self.a < u32::from(probability.q_e) {
                    decision = crate::refinement::mq_decision(context.mps, true);
                    self.transition(context_index, probability.next_lps, probability.switch_mps)?;
                } else {
                    decision = crate::refinement::mq_decision(context.mps, false);
                    self.transition(context_index, probability.next_mps, false)?;
                }
                self.renormalize()?;
            } else {
                decision = crate::refinement::mq_decision(context.mps, false);
            }
        }
        Ok(decision)
    }

    fn renormalize(&mut self) -> Result<(), DecodeError> {
        let mut remaining_shifts = 16_u8;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant remaining_shifts <= 16,
                decreases remaining_shifts,
            )
        )]
        while self.a < 0x8000 {
            if remaining_shifts == 0 {
                return Err(DecodeError::malformed(
                    self.byte_index as u64,
                    MalformedKind::InvalidMqState,
                ));
            }
            remaining_shifts -= 1;
            if self.ct == 0 {
                self.byte_in()?;
            }
            if self.a > u32::MAX / 2 || self.c > u32::MAX / 2 || self.ct == 0 {
                return Err(DecodeError::malformed(
                    self.byte_index as u64,
                    MalformedKind::ArithmeticOverflow,
                ));
            }
            self.a = crate::refinement::mq_renorm_double(self.a);
            self.c = crate::refinement::mq_renorm_double(self.c);
            self.ct = crate::refinement::mq_renorm_ct(self.ct);
            self.charge_work()?;
        }
        Ok(())
    }

    fn byte_in(&mut self) -> Result<(), DecodeError> {
        let current = self.current_byte();
        let next = self.next_byte();
        let class = crate::refinement::mq_byte_class(current, next);
        if crate::refinement::mq_byte_advances(current, next) {
            self.advance_byte()?;
        }
        self.c = self
            .c
            .checked_add(crate::refinement::mq_byte_addend(current, next))
            .ok_or_else(|| {
                DecodeError::malformed(self.byte_index as u64, MalformedKind::ArithmeticOverflow)
            })?;
        self.ct = crate::refinement::mq_byte_ct(current, next);
        if class == 0 {
            self.marker_padding_count =
                self.marker_padding_count.checked_add(1).ok_or_else(|| {
                    DecodeError::malformed(
                        self.byte_index as u64,
                        MalformedKind::ArithmeticOverflow,
                    )
                })?;
        }
        Ok(())
    }

    fn current_byte(&self) -> u8 {
        if self.byte_index < self.input.len() {
            self.input[self.byte_index]
        } else {
            0xff
        }
    }

    fn next_byte(&self) -> u8 {
        match self.byte_index.checked_add(1) {
            Some(next_index) if next_index < self.input.len() => self.input[next_index],
            _ => 0xff,
        }
    }

    fn advance_byte(&mut self) -> Result<(), DecodeError> {
        if self.byte_index < self.input.len() {
            self.byte_index = self.byte_index.checked_add(1).ok_or_else(|| {
                DecodeError::malformed(self.byte_index as u64, MalformedKind::ArithmeticOverflow)
            })?;
        }
        Ok(())
    }

    fn transition(
        &mut self,
        context_index: usize,
        next_state: u8,
        switch_mps: bool,
    ) -> Result<(), DecodeError> {
        if context_index >= self.contexts.len() || usize::from(next_state) >= STATES.len() {
            return Err(DecodeError::malformed(
                self.byte_index as u64,
                MalformedKind::InvalidMqState,
            ));
        }
        let mut context = self.contexts[context_index];
        context.state = next_state;
        context.mps = crate::refinement::mq_next_mps(context.mps, switch_mps);
        self.contexts[context_index] = context;
        Ok(())
    }

    fn charge_work(&mut self) -> Result<(), DecodeError> {
        self.work_units = self.work_units.checked_add(1).ok_or_else(|| {
            DecodeError::malformed(self.byte_index as u64, MalformedKind::ArithmeticOverflow)
        })?;
        if self.work_units > self.maximum_work_units {
            return Err(DecodeError::limit(
                Resource::WorkUnits,
                self.maximum_work_units,
                self.work_units,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_table_transitions_are_bounded() {
        for state in STATES {
            assert!(usize::from(state.next_mps) < STATES.len());
            assert!(usize::from(state.next_lps) < STATES.len());
            assert!(state.q_e > 0);
        }
    }

    #[test]
    fn decodes_deterministic_symbol_sequence() {
        let limits = ResourceLimits::default();
        let mut first = MqDecoder::new(&[0x00, 0x00, 0x00, 0x00], &limits).unwrap();
        let mut second = MqDecoder::new(&[0x00, 0x00, 0x00, 0x00], &limits).unwrap();
        let mut first_symbols = Vec::new();
        let mut second_symbols = Vec::new();
        for _ in 0..32 {
            first_symbols.push(first.decode(0).unwrap());
            second_symbols.push(second.decode(0).unwrap());
        }
        assert_eq!(first_symbols, second_symbols);
        assert_eq!(
            first_symbols,
            [
                false, true, true, false, true, false, true, false, true, false, true, false, true,
                false, true, false, true, false, true, false, true, false, true, false, true,
                false, true, false, true, false, true, true,
            ]
        );
        assert!(first.bytes_consumed() <= 4);
    }

    #[test]
    fn validates_context_state_and_work_limits() {
        let limits = ResourceLimits::default()
            .with_maximum_work_units(1)
            .unwrap();
        let mut decoder = MqDecoder::new(&[0], &limits).unwrap();
        assert!(decoder.set_context(0, 47, false).is_err());
        assert!(decoder.context(CONTEXT_COUNT).is_err());
        let _ = decoder.decode(0);
        assert!(matches!(
            decoder.decode(0),
            Err(DecodeError::LimitExceeded {
                resource: Resource::WorkUnits,
                ..
            })
        ));
    }

    #[test]
    fn preserves_probability_contexts_across_segments() {
        let limits = ResourceLimits::default();
        let mut first = MqDecoder::new(&[0, 0], &limits).unwrap();
        first.set_context(0, 4, false).unwrap();
        let _ = first.decode(0).unwrap();
        let state = first.context_state();

        let mut second = MqDecoder::new(&[0, 0], &limits).unwrap();
        second.restore_context_state(state);
        assert_eq!(second.context(0).unwrap(), first.context(0).unwrap());
    }

    #[test]
    fn handles_marker_padding_without_out_of_bounds_reads() {
        let mut decoder = MqDecoder::new(&[0xff, 0x90], &ResourceLimits::default()).unwrap();
        for _ in 0..16 {
            let _ = decoder.decode(0);
        }
        assert!(decoder.marker_padding_count() > 0);
        assert!(decoder.bytes_consumed() <= 2);
    }

    #[test]
    fn validates_predictable_termination_bounds() {
        let limits = ResourceLimits::default();
        let short = MqDecoder::new(&[0, 0], &limits).unwrap();
        short.validate_predictable_termination().unwrap();

        let long = MqDecoder::new(&[0; 8], &limits).unwrap();
        assert!(long.validate_predictable_termination().is_err());
    }
}
