//! Quantization tables, component overrides and region-of-interest shifts.

use super::{
    CodingStyle, ComponentQuantizations, DecodeError, MAX_COMPONENTS, MAX_QUANTIZATION_STEPS,
    MalformedKind, Quantization, QuantizationStep, QuantizationStyle, RoiShifts,
};
#[cfg(verus_keep_ghost)]
use super::{quantization_owner_view, quantization_view, roi_shifts_view};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

impl ComponentQuantizations {
    pub(super) fn new(components: u16, default: Quantization) -> Result<Self, DecodeError> {
        if components == 0 || usize::from(components) > MAX_COMPONENTS {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidImageGeometry,
            ));
        }

        Ok(Self {
            components,
            quantizations: vec![default; usize::from(components)],
        })
    }

    pub(crate) fn uniform(components: u16, default: Quantization) -> Result<Self, DecodeError> {
        Self::new(components, default)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_component_quantization(self, component),
    ))]
    pub fn quantization(&self, component: u16) -> Result<Quantization, DecodeError> {
        if component >= self.components {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidQuantization,
            });
        }
        match self.quantizations.get(usize::from(component)) {
            Some(quantization) => Ok(*quantization),
            None => Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidQuantization,
            }),
        }
    }

    pub(super) fn set(
        &mut self,
        component: u16,
        quantization: Quantization,
    ) -> Result<(), DecodeError> {
        if component >= self.components {
            return Err(DecodeError::malformed(
                0,
                MalformedKind::InvalidQuantization,
            ));
        }
        let slot = self
            .quantizations
            .get_mut(usize::from(component))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidQuantization))?;
        *slot = quantization;
        Ok(())
    }

    pub fn equals(&self, other: &Self) -> bool {
        if self.components != other.components {
            return false;
        }
        let mut component = 0_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= self.components,
                decreases self.components - component,
            )
        )]
        while component < self.components {
            let left = match self.quantization(component) {
                Ok(value) => value,
                Err(_) => return false,
            };
            let right = match other.quantization(component) {
                Ok(value) => value,
                Err(_) => return false,
            };
            if !left.equals(&right) {
                return false;
            }
            component += 1;
        }
        true
    }
}
impl RoiShifts {
    pub const fn none() -> Self {
        Self {
            shifts: [0; MAX_COMPONENTS],
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_roi_shift(roi_shifts_view(self), component),
    ))]
    pub fn shift(&self, component: u16) -> Result<u8, DecodeError> {
        match self.shifts.get(usize::from(component)) {
            Some(shift) => Ok(*shift),
            None => Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidRegionOfInterest,
            }),
        }
    }

    pub fn equals(&self, other: &Self) -> bool {
        let mut component = 0_usize;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant component <= self.shifts.len(),
                decreases self.shifts.len() - component,
            )
        )]
        while component < self.shifts.len() {
            if self.shifts[component] != other.shifts[component] {
                return false;
            }
            component += 1;
        }
        true
    }

    pub(super) fn set(&mut self, component: u16, shift: u8) -> Result<(), DecodeError> {
        let value = self
            .shifts
            .get_mut(usize::from(component))
            .ok_or_else(|| DecodeError::malformed(0, MalformedKind::InvalidRegionOfInterest))?;
        *value = shift;
        Ok(())
    }
}
impl Default for RoiShifts {
    fn default() -> Self {
        Self::none()
    }
}
impl Quantization {
    pub(crate) fn reversible_default(coding_style: CodingStyle) -> Self {
        Self {
            style: QuantizationStyle::None,
            guard_bits: 0,
            step_count: coding_style.subband_count(),
            steps: [QuantizationStep {
                exponent: 0,
                mantissa: 0,
            }; MAX_QUANTIZATION_STEPS],
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == quantization_view(self).0,
    ))]
    pub const fn style(&self) -> QuantizationStyle {
        self.style
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == quantization_view(self).1,
    ))]
    pub const fn guard_bits(&self) -> u8 {
        self.guard_bits
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part2_quantization_lookup(quantization_view(self), index),
    ))]
    pub fn step(&self, index: u16) -> Result<QuantizationStep, DecodeError> {
        if index >= self.step_count {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidQuantization,
            });
        }
        let index = usize::from(index);
        if index >= self.steps.len() {
            return Err(DecodeError::MalformedInput {
                offset: 0,
                kind: MalformedKind::InvalidQuantization,
            });
        }
        Ok(self.steps[index])
    }

    pub fn equals(&self, other: &Self) -> bool {
        if !quantization_styles_equal(self.style, other.style)
            || self.guard_bits != other.guard_bits
            || self.step_count != other.step_count
        {
            return false;
        }
        let mut index = 0_u16;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= self.step_count,
                decreases self.step_count - index,
            )
        )]
        while index < self.step_count {
            let left = match self.step(index) {
                Ok(value) => value,
                Err(_) => return false,
            };
            let right = match other.step(index) {
                Ok(value) => value,
                Err(_) => return false,
            };
            if left.exponent != right.exponent || left.mantissa != right.mantissa {
                return false;
            }
            index += 1;
        }
        true
    }

    pub fn is_compatible_with(self, coding_style: CodingStyle) -> bool {
        match self.style {
            QuantizationStyle::None | QuantizationStyle::ScalarExpounded => {
                self.step_count == coding_style.subband_count()
            }
            QuantizationStyle::ScalarDerived => self.step_count == 1,
        }
    }
}
impl Clone for ComponentQuantizations {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(result => ensures quantization_owner_view(&result) == quantization_owner_view(self),))]
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(quantization_owner_view); }
        Self {
            components: self.components,
            quantizations: self.quantizations.clone(),
        }
    }
}
const fn quantization_styles_equal(left: QuantizationStyle, right: QuantizationStyle) -> bool {
    matches!(
        (left, right),
        (QuantizationStyle::None, QuantizationStyle::None)
            | (
                QuantizationStyle::ScalarDerived,
                QuantizationStyle::ScalarDerived
            )
            | (
                QuantizationStyle::ScalarExpounded,
                QuantizationStyle::ScalarExpounded
            )
    )
}
