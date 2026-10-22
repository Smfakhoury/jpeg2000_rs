use crate::{DecodeError, MalformedKind};

#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

pub const MAGNITUDE_LIMBS: usize = 6;
pub const MAGNITUDE_BITS: u16 = (MAGNITUDE_LIMBS as u16) * 64;

#[derive(Clone, Copy, Default)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, Eq, PartialEq))]
pub struct Magnitude {
    limbs: [u64; MAGNITUDE_LIMBS],
}

#[cfg(verus_keep_ghost)]
verus! {
    pub closed spec fn magnitude_limbs(value: Magnitude) -> Seq<u64> {
        value.limbs@
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures magnitude_limbs(result) == Seq::new(6, |i: int| if i == 0 { value } else { 0 }),
))]
pub(crate) const fn magnitude_from_u64(value: u64) -> Magnitude {
    let mut limbs = [0, 0, 0, 0, 0, 0];
    limbs[0] = value;
    Magnitude { limbs }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    requires bit < 384,
    ensures match result {
        Ok(value) => magnitude_limbs(value) == crate::specs::part1_magnitude_bit(bit),
        Err(_) => false,
    },
))]
pub(crate) fn magnitude_single_bit(bit: u16) -> Result<Magnitude, DecodeError> {
    let mut limbs = [0_u64, 0, 0, 0, 0, 0];
    let limb = usize::from(bit / 64);
    let offset = u32::from(bit % 64);
    limbs[limb] = 1_u64 << offset;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(limbs@ =~= crate::specs::part1_magnitude_bit(bit));
    }
    Ok(Magnitude { limbs })
}

impl Magnitude {
    pub const ZERO: Self = Self {
        limbs: [0, 0, 0, 0, 0, 0],
    };

    pub const fn from_u64(value: u64) -> Self {
        magnitude_from_u64(value)
    }

    pub const fn from_limbs(limbs: [u64; MAGNITUDE_LIMBS]) -> Self {
        Self { limbs }
    }

    pub fn single_bit(bit: u16) -> Result<Self, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { assert(MAGNITUDE_BITS == 384) by(compute); }
        if bit >= MAGNITUDE_BITS {
            return Err(arithmetic_overflow());
        }
        magnitude_single_bit(bit)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_magnitude_zero(magnitude_limbs(self)),
    ))]
    pub const fn is_zero(self) -> bool {
        let mut index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= MAGNITUDE_LIMBS,
                forall |i: int| 0 <= i < index ==> self.limbs[i] == 0,
                decreases MAGNITUDE_LIMBS - index,
            )
        )]
        while index < MAGNITUDE_LIMBS {
            if self.limbs[index] != 0 {
                return false;
            }
            index += 1;
        }
        true
    }

    pub const fn low_u64(self) -> u64 {
        self.limbs[0]
    }

    pub const fn limb(self, index: usize) -> Option<u64> {
        if index < MAGNITUDE_LIMBS {
            Some(self.limbs[index])
        } else {
            None
        }
    }

    pub fn checked_add(self, other: Self) -> Result<Self, DecodeError> {
        let mut limbs = [0_u64, 0, 0, 0, 0, 0];
        let mut carry = false;
        let mut index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= MAGNITUDE_LIMBS,
                decreases MAGNITUDE_LIMBS - index,
            )
        )]
        while index < MAGNITUDE_LIMBS {
            let sum = u128::from(self.limbs[index])
                + u128::from(other.limbs[index])
                + u128::from(u8::from(carry));
            limbs[index] = sum as u64;
            carry = sum > u128::from(u64::MAX);
            index += 1;
        }
        if carry {
            return Err(arithmetic_overflow());
        }
        Ok(Self { limbs })
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, DecodeError> {
        if self.less_than(other) {
            return Err(arithmetic_overflow());
        }
        let mut limbs = [0_u64, 0, 0, 0, 0, 0];
        let mut borrow = false;
        let mut index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= MAGNITUDE_LIMBS,
                decreases MAGNITUDE_LIMBS - index,
            )
        )]
        while index < MAGNITUDE_LIMBS {
            let subtrahend = if borrow {
                match other.limbs[index].checked_add(1) {
                    Some(value) => value,
                    None => {
                        limbs[index] = self.limbs[index];
                        index += 1;
                        continue;
                    }
                }
            } else {
                other.limbs[index]
            };
            if self.limbs[index] >= subtrahend {
                limbs[index] = self.limbs[index] - subtrahend;
                borrow = false;
            } else {
                let gap = subtrahend - self.limbs[index];
                limbs[index] = u64::MAX - (gap - 1);
                borrow = true;
            }
            index += 1;
        }
        if borrow {
            return Err(arithmetic_overflow());
        }
        Ok(Self { limbs })
    }

    pub fn checked_shl(self, shift: u16) -> Result<Self, DecodeError> {
        if shift >= MAGNITUDE_BITS {
            return if self.is_zero() {
                Ok(Self::ZERO)
            } else {
                Err(arithmetic_overflow())
            };
        }
        if shift == 0 {
            return Ok(self);
        }
        let whole = usize::from(shift / 64);
        let partial = u32::from(shift % 64);
        if whole >= MAGNITUDE_LIMBS || partial >= 64 {
            return Err(arithmetic_overflow());
        }
        let mut limbs = [0_u64, 0, 0, 0, 0, 0];
        let mut source = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant source <= MAGNITUDE_LIMBS,
                whole < MAGNITUDE_LIMBS,
                partial < 64,
                decreases MAGNITUDE_LIMBS - source,
            )
        )]
        while source < MAGNITUDE_LIMBS {
            let destination = source.checked_add(whole).ok_or_else(arithmetic_overflow)?;
            if destination >= MAGNITUDE_LIMBS {
                if self.limbs[source] != 0 {
                    return Err(arithmetic_overflow());
                }
                source += 1;
                continue;
            }
            limbs[destination] |= self.limbs[source] << partial;
            if partial != 0 {
                let next = destination.checked_add(1).ok_or_else(arithmetic_overflow)?;
                if next < MAGNITUDE_LIMBS {
                    limbs[next] |= self.limbs[source] >> (64 - partial);
                } else if self.limbs[source] >> (64 - partial) != 0 {
                    return Err(arithmetic_overflow());
                }
            }
            source += 1;
        }
        Ok(Self { limbs })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures magnitude_limbs(result) == crate::specs::part1_magnitude_shift(magnitude_limbs(self), shift),
    ))]
    pub fn shift_right(self, shift: u16) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! { assert(MAGNITUDE_BITS == 384) by(compute); }
        if shift >= MAGNITUDE_BITS {
            return magnitude_from_u64(0);
        }
        if shift == 0 {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert forall |i: int| 0 <= i < 6 implies
                    crate::specs::part1_magnitude_shift_limb(self.limbs@, shift, i) == self.limbs[i]
                by {
                    let value = self.limbs[i];
                    assert(value >> 0 == value) by(bit_vector);
                }
                assert(self.limbs@ =~= crate::specs::part1_magnitude_shift(self.limbs@, shift));
            }
            return self;
        }
        let whole = (shift / 64) as usize;
        let partial = (shift % 64) as u32;
        if whole >= MAGNITUDE_LIMBS || partial >= 64 {
            return Self::ZERO;
        }
        let mut limbs = [0_u64, 0, 0, 0, 0, 0];
        let mut destination = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant destination <= MAGNITUDE_LIMBS,
                whole < MAGNITUDE_LIMBS,
                partial < 64,
                whole == shift / 64,
                partial == shift % 64,
                shift < 384,
                forall |i: int| 0 <= i < destination ==>
                    limbs[i] == crate::specs::part1_magnitude_shift_limb(self.limbs@, shift, i),
                forall |i: int| destination <= i < 6 ==> limbs[i] == 0,
                decreases MAGNITUDE_LIMBS - destination,
            )
        )]
        while destination < MAGNITUDE_LIMBS {
            let source = destination + whole;
            if source >= MAGNITUDE_LIMBS {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert forall |i: int| destination <= i < 6 implies
                        crate::specs::part1_magnitude_shift_limb(self.limbs@, shift, i) == 0 by {}
                    assert(limbs@ =~= crate::specs::part1_magnitude_shift(self.limbs@, shift));
                }
                return Self { limbs };
            }
            limbs[destination] = self.limbs[source] >> partial;
            if partial != 0 {
                let next = source + 1;
                if next < MAGNITUDE_LIMBS {
                    limbs[destination] |= self.limbs[next] << (64 - partial);
                }
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(limbs[destination as int]
                    == crate::specs::part1_magnitude_shift_limb(self.limbs@, shift, destination as int));
            }
            destination += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(limbs@ =~= crate::specs::part1_magnitude_shift(self.limbs@, shift));
        }
        Self { limbs }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_magnitude_u64(magnitude_limbs(self)),
    ))]
    pub const fn to_u64(self) -> Option<u64> {
        let mut index = 1;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant 1 <= index,
                index <= MAGNITUDE_LIMBS,
                forall |i: int| 1 <= i < index ==> self.limbs[i] == 0,
                decreases MAGNITUDE_LIMBS - index,
            )
        )]
        while index < MAGNITUDE_LIMBS {
            if self.limbs[index] != 0 {
                return None;
            }
            index += 1;
        }
        Some(self.limbs[0])
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_magnitude_less(magnitude_limbs(self), magnitude_limbs(other)),
    ))]
    pub const fn less_than(self, other: Self) -> bool {
        let mut index = MAGNITUDE_LIMBS;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= MAGNITUDE_LIMBS,
                forall |i: int| index <= i < 6 ==> self.limbs[i] == other.limbs[i],
                decreases index,
            )
        )]
        while index > 0 {
            index -= 1;
            if self.limbs[index] < other.limbs[index] {
                return true;
            }
            if self.limbs[index] > other.limbs[index] {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert forall |i: int| 0 <= i < 6 && self.limbs[i] < other.limbs[i]
                        implies !(forall |j: int| i < j < 6 ==> self.limbs[j] == other.limbs[j])
                    by {
                        if i < index {
                            assert(self.limbs[index as int] != other.limbs[index as int]);
                        }
                    }
                }
                return false;
            }
        }
        false
    }
}

fn arithmetic_overflow() -> DecodeError {
    DecodeError::malformed(0, MalformedKind::ArithmeticOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_shifts_compares_and_adds_across_limbs() {
        let bit_255 = Magnitude::single_bit(255).unwrap();
        assert_eq!(bit_255.limb(3), Some(1_u64 << 63));
        assert_eq!(bit_255.shift_right(192), Magnitude::single_bit(63).unwrap());
        assert!(Magnitude::single_bit(254).unwrap().less_than(bit_255));
        assert_eq!(
            Magnitude::from_u64(u64::MAX)
                .checked_add(Magnitude::from_u64(1))
                .unwrap(),
            Magnitude::single_bit(64).unwrap()
        );
        assert_eq!(
            Magnitude::single_bit(320)
                .unwrap()
                .checked_sub(Magnitude::single_bit(319).unwrap())
                .unwrap(),
            Magnitude::single_bit(319).unwrap()
        );
    }
}
