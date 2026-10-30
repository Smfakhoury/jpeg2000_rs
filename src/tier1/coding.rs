use super::state::{arithmetic_overflow, invalid_code_block};
use super::{Tier1BlockCoding, Tier1CodingMode, Tier1CodingStyle, Tier1PassKind, Tier1PassPlan};
use crate::DecodeError;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_legacy_default_style(),
))]
pub(super) fn legacy_default_style() -> Tier1CodingStyle {
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
pub(super) fn legacy_maximum_passes(bitplane: u16) -> Result<u32, DecodeError> {
    let passes = u32::from(bitplane)
        .checked_mul(3)
        .ok_or_else(arithmetic_overflow)?;
    passes.checked_add(1).ok_or_else(arithmetic_overflow)
}
