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

    pub closed spec fn magnitude_value(value: Magnitude) -> nat {
        crate::specs::part1_magnitude_value(value.limbs@)
    }
}

#[cfg(verus_keep_ghost)]
pub mod representation_proofs {
    use vstd::prelude::*;

    verus! {
        pub proof fn magnitude_limbs_shape(value: super::Magnitude)
            ensures super::magnitude_limbs(value).len() == 6,
        {
            reveal(super::magnitude_limbs);
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == Magnitude::ZERO,
    magnitude_limbs(result) == Seq::new(6, |i: int| 0u64),
    magnitude_value(result) == 0,
))]
pub(crate) const fn magnitude_zero() -> Magnitude {
    let value = Magnitude::ZERO;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(value.limbs@ =~= Seq::new(6, |i: int| 0u64));
        crate::proofs::production_impl::production_magnitude_zero_value(value.limbs@);
    }
    value
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures magnitude_limbs(result) == Seq::new(6, |i: int| if i == 0 { value } else { 0 }),
    magnitude_value(result) == value as nat,
))]
pub(crate) const fn magnitude_from_u64(value: u64) -> Magnitude {
    let mut limbs = [0, 0, 0, 0, 0, 0];
    limbs[0] = value;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::proofs::production_impl::production_magnitude_sparse_prefix(limbs@, 0, value, 6);
        vstd::arithmetic::power2::lemma2_to64();
        vstd::arithmetic::mul::lemma_mul_basics(value as int);
    }
    Magnitude { limbs }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures magnitude_limbs(result) == limbs@,
    magnitude_value(result) == crate::specs::part1_magnitude_value(limbs@),
    magnitude_value(result) < crate::specs::part1_magnitude_capacity(),
))]
pub(crate) const fn magnitude_from_limbs(limbs: [u64; MAGNITUDE_LIMBS]) -> Magnitude {
    #[cfg(verus_keep_ghost)]
    proof! { crate::proofs::production_impl::production_magnitude_value_bound(limbs@); }
    Magnitude { limbs }
}

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures match result {
        Ok(value) => bit < 384 && magnitude_limbs(value) == crate::specs::part1_magnitude_bit(bit)
            && magnitude_value(value) == vstd::arithmetic::power2::pow2(bit as nat),
        Err(error) => bit >= 384 && error == crate::specs::part1_transform_error(true),
    },
))]
pub(crate) fn magnitude_single_bit(bit: u16) -> Result<Magnitude, DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! { assert(MAGNITUDE_BITS == 384) by(compute); }
    if bit >= MAGNITUDE_BITS {
        return Err(arithmetic_overflow());
    }
    let mut limbs = [0_u64, 0, 0, 0, 0, 0];
    let limb = (bit / 64) as usize;
    let offset = (bit % 64) as u32;
    limbs[limb] = 1_u64 << offset;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(limbs@ =~= crate::specs::part1_magnitude_bit(bit));
        crate::proofs::production_impl::production_magnitude_sparse_prefix(
            limbs@, limb as nat, 1u64 << offset, 6,
        );
        vstd::bits::lemma_u64_pow2_no_overflow(offset as nat);
        vstd::bits::lemma_u64_shl_is_mul(1, offset as u64);
        assert((1u64 << offset) == (1u64 << (offset as u64))) by(bit_vector)
            requires offset < 64;
        vstd::arithmetic::power2::lemma_pow2_adds(offset as nat, 64 * (limb as nat));
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
        magnitude_from_limbs(limbs)
    }

    pub fn single_bit(bit: u16) -> Result<Self, DecodeError> {
        magnitude_single_bit(bit)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_magnitude_zero(magnitude_limbs(self)),
        magnitude_value(self) == crate::specs::part1_magnitude_value(magnitude_limbs(self)),
        result == (magnitude_value(self) == 0),
        magnitude_value(self) < crate::specs::part1_magnitude_capacity(),
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
                #[cfg(verus_keep_ghost)]
                proof! { crate::proofs::production_impl::production_magnitude_prefix_properties(self.limbs@, 6); }
                return false;
            }
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_magnitude_prefix_properties(self.limbs@, 6); }
        true
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result as nat == crate::specs::part1_magnitude_low_value(magnitude_limbs(self)),
        result as nat == magnitude_value(self) % 18446744073709551616,
        magnitude_value(self) == crate::specs::part1_magnitude_value(magnitude_limbs(self)),
        result == magnitude_limbs(self)[0],
        magnitude_value(self) < crate::specs::part1_magnitude_capacity(),
    ))]
    pub const fn low_u64(self) -> u64 {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_magnitude_low_word(self.limbs@);
            crate::proofs::production_impl::production_magnitude_value_bound(self.limbs@);
            vstd::arithmetic::power2::lemma2_to64();
        }
        self.limbs[0]
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures result == crate::specs::part1_magnitude_limb(magnitude_limbs(self), index),
        magnitude_limbs(self).len() == 6,
        result == if index < 6 { Some(magnitude_limbs(self)[index as int]) } else { None },
    ))]
    pub const fn limb(self, index: usize) -> Option<u64> {
        if index < MAGNITUDE_LIMBS {
            Some(self.limbs[index])
        } else {
            None
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures magnitude_value(self) < crate::specs::part1_magnitude_capacity(),
        magnitude_value(self) == crate::specs::part1_magnitude_value(magnitude_limbs(self)),
        magnitude_value(other) == crate::specs::part1_magnitude_value(magnitude_limbs(other)),
        magnitude_value(other) < crate::specs::part1_magnitude_capacity(),
        match result {
            Ok(value) => magnitude_value(value) == crate::specs::magnitude_addition(magnitude_value(self), magnitude_value(other))
                && magnitude_value(value) == crate::specs::part1_magnitude_value(magnitude_limbs(value))
                && magnitude_value(value) < crate::specs::part1_magnitude_capacity(),
            Err(error) => error == crate::specs::part1_transform_error(true)
                && magnitude_value(self) + magnitude_value(other) >= crate::specs::part1_magnitude_capacity(),
        },
    ))]
    pub fn checked_add(self, other: Self) -> Result<Self, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_magnitude_value_bound(self.limbs@);
            crate::proofs::production_impl::production_magnitude_value_bound(other.limbs@);
        }
        let mut limbs = [0_u64, 0, 0, 0, 0, 0];
        let mut carry = false;
        let mut index = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant index <= MAGNITUDE_LIMBS,
                magnitude_value(self) < crate::specs::part1_magnitude_capacity(),
                magnitude_value(other) < crate::specs::part1_magnitude_capacity(),
                crate::specs::part1_magnitude_prefix_value(self.limbs@, index as nat)
                    + crate::specs::part1_magnitude_prefix_value(other.limbs@, index as nat)
                    == crate::specs::part1_magnitude_prefix_value(limbs@, index as nat)
                        + if carry { vstd::arithmetic::power2::pow2(64 * (index as nat)) } else { 0nat },
                decreases MAGNITUDE_LIMBS - index,
            )
        )]
        while index < MAGNITUDE_LIMBS {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = limbs@; let ghost previous_carry = carry; }
            let sum = (self.limbs[index] as u128)
                + (other.limbs[index] as u128)
                + if carry { 1u128 } else { 0u128 };
            limbs[index] = sum as u64;
            carry = sum > u64::MAX as u128;
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_magnitude_add_limb(sum);
                crate::proofs::production_impl::production_magnitude_prefix_update(before, index as nat, sum as u64);
                vstd::arithmetic::power2::lemma2_to64();
                vstd::arithmetic::power2::lemma_pow2_adds(64 * (index as nat), 64);
                crate::proofs::production_impl::production_magnitude_add_prefix_step(
                    crate::specs::part1_magnitude_prefix_value(self.limbs@, index as nat),
                    crate::specs::part1_magnitude_prefix_value(other.limbs@, index as nat),
                    crate::specs::part1_magnitude_prefix_value(before, index as nat),
                    self.limbs[index as int], other.limbs[index as int], sum as u64, previous_carry, carry,
                    vstd::arithmetic::power2::pow2(64 * (index as nat)),
                    vstd::arithmetic::power2::pow2(64 * (index as nat) + 64),
                );
            }
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_magnitude_value_bound(limbs@); }
        if carry {
            return Err(arithmetic_overflow());
        }
        Ok(Self { limbs })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures magnitude_value(self) == crate::specs::part1_magnitude_value(magnitude_limbs(self)),
        magnitude_value(other) == crate::specs::part1_magnitude_value(magnitude_limbs(other)),
        magnitude_value(self) < crate::specs::part1_magnitude_capacity(),
        magnitude_value(other) < crate::specs::part1_magnitude_capacity(),
        magnitude_limbs(self).len() == 6,
        magnitude_limbs(other).len() == 6,
        result.is_ok() == (magnitude_value(self) >= magnitude_value(other)),
        match result {
            Ok(value) => magnitude_value(value) == (magnitude_value(self) - magnitude_value(other)) as nat
                && crate::specs::part1_magnitude_subtraction(magnitude_value(self), magnitude_value(other)) == Ok(magnitude_value(value))
                && magnitude_value(value) == crate::specs::part1_magnitude_value(magnitude_limbs(value))
                && magnitude_value(value) < crate::specs::part1_magnitude_capacity()
                && magnitude_limbs(value).len() == 6,
            Err(error) => magnitude_value(self) < magnitude_value(other)
                && error == crate::specs::part1_transform_error(true),
        },
    ))]
    pub fn checked_sub(self, other: Self) -> Result<Self, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_magnitude_prefix_order(self.limbs@, other.limbs@, 6);
            crate::proofs::production_impl::production_magnitude_value_bound(self.limbs@);
            crate::proofs::production_impl::production_magnitude_value_bound(other.limbs@);
            assert(crate::specs::part1_magnitude_prefix_less(self.limbs@, other.limbs@, 6)
                == crate::specs::part1_magnitude_less(self.limbs@, other.limbs@));
        }
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
                magnitude_value(self) >= magnitude_value(other),
                magnitude_value(self) < crate::specs::part1_magnitude_capacity(),
                magnitude_value(other) < crate::specs::part1_magnitude_capacity(),
                crate::specs::part1_magnitude_prefix_value(self.limbs@, index as nat)
                    + (if borrow { vstd::arithmetic::power2::pow2(64 * index as nat) } else { 0nat })
                    == crate::specs::part1_magnitude_prefix_value(other.limbs@, index as nat)
                        + crate::specs::part1_magnitude_prefix_value(limbs@, index as nat),
                decreases MAGNITUDE_LIMBS - index,
            )
        )]
        while index < MAGNITUDE_LIMBS {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = limbs@; let ghost previous_borrow = borrow; }
            let subtrahend = if borrow {
                match other.limbs[index].checked_add(1) {
                    Some(value) => value,
                    None => {
                        limbs[index] = self.limbs[index];
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            assert((self.limbs[index as int] as nat) + 18446744073709551616nat
                                == (other.limbs[index as int] as nat) + 1 + limbs[index as int] as nat);
                            crate::proofs::production_impl::production_magnitude_sub_prefix_step(
                                self.limbs@, other.limbs@, before, index as nat, limbs[index as int], previous_borrow, borrow,
                            );
                        }
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
            #[cfg(verus_keep_ghost)]
            proof! {
                assert((self.limbs[index as int] as nat) + (if borrow { 18446744073709551616nat } else { 0nat })
                    == (other.limbs[index as int] as nat) + (if previous_borrow { 1nat } else { 0nat }) + limbs[index as int] as nat);
                crate::proofs::production_impl::production_magnitude_sub_prefix_step(
                    self.limbs@, other.limbs@, before, index as nat, limbs[index as int], previous_borrow, borrow,
                );
            }
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { crate::proofs::production_impl::production_magnitude_value_bound(limbs@); }
        if borrow {
            return Err(arithmetic_overflow());
        }
        Ok(Self { limbs })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures magnitude_value(self) < crate::specs::part1_magnitude_capacity(),
        magnitude_value(self) == crate::specs::part1_magnitude_value(magnitude_limbs(self)),
        match result {
            Ok(value) => magnitude_limbs(value) == crate::specs::part1_magnitude_left(magnitude_limbs(self), shift)
                && magnitude_value(value) == crate::specs::part1_magnitude_value(magnitude_limbs(value))
                && magnitude_value(value) == magnitude_value(self) * vstd::arithmetic::power2::pow2(shift as nat)
                && magnitude_value(value) < crate::specs::part1_magnitude_capacity(),
            Err(error) => error == crate::specs::part1_transform_error(true)
                && magnitude_value(self) * vstd::arithmetic::power2::pow2(shift as nat) >= crate::specs::part1_magnitude_capacity(),
        },
    ))]
    pub fn checked_shl(self, shift: u16) -> Result<Self, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(MAGNITUDE_BITS == 384) by(compute);
            crate::proofs::production_impl::production_magnitude_value_bound(self.limbs@);
            crate::proofs::production_impl::production_magnitude_left_fits_value(self.limbs@, shift);
            vstd::arithmetic::power2::lemma2_to64();
            vstd::arithmetic::mul::lemma_mul_basics(magnitude_value(self) as int);
            vstd::arithmetic::mul::lemma_mul_basics(vstd::arithmetic::power2::pow2(shift as nat) as int);
        }
        if shift >= MAGNITUDE_BITS {
            return if self.is_zero() {
                Ok(magnitude_zero())
            } else {
                Err(arithmetic_overflow())
            };
        }
        if shift == 0 {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert forall |i: int| 0 <= i < 6 implies
                    crate::specs::part1_magnitude_left(self.limbs@, shift)[i] == self.limbs[i]
                by { crate::proofs::production_impl::production_magnitude_left_projection(self.limbs@, shift, i); }
                assert(self.limbs@ =~= crate::specs::part1_magnitude_left(self.limbs@, shift));
            }
            return Ok(self);
        }
        let whole = (shift / 64) as usize;
        let partial = (shift % 64) as u32;
        if whole >= MAGNITUDE_LIMBS || partial >= 64 {
            return Err(arithmetic_overflow());
        }
        let mut limbs = [0_u64, 0, 0, 0, 0, 0];
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_magnitude_or_zero(0);
            assert(limbs@ =~= crate::proofs::internal_specs::magnitude_left_prefix(self.limbs@, shift, 0));
        }
        let mut source = 0;
        #[cfg_attr(
            verus_keep_ghost,
            verus_spec(
                invariant source <= MAGNITUDE_LIMBS,
                whole < MAGNITUDE_LIMBS,
                partial < 64,
                whole == shift / 64,
                partial == shift % 64,
                shift < 384,
                magnitude_value(self) < crate::specs::part1_magnitude_capacity(),
                limbs@ == crate::proofs::internal_specs::magnitude_left_prefix(self.limbs@, shift, source as nat),
                forall |i: int| 0 <= i < source ==>
                    if i + whole >= 6 { self.limbs[i] == 0 }
                    else if partial > 0 && i + whole == 5 {
                        self.limbs[i] >> ((64 - partial) as u32) == 0
                    } else { true },
                decreases MAGNITUDE_LIMBS - source,
            )
        )]
        while source < MAGNITUDE_LIMBS {
            let destination = source.checked_add(whole).ok_or_else(arithmetic_overflow)?;
            if destination >= MAGNITUDE_LIMBS {
                if self.limbs[source] != 0 {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(!crate::specs::part1_magnitude_left_fits(self.limbs@, shift));
                        crate::proofs::production_impl::production_magnitude_left_fits_value(self.limbs@, shift);
                    }
                    return Err(arithmetic_overflow());
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::proofs::production_impl::production_magnitude_left_prefix_discard(self.limbs@, shift, source as nat);
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
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(!crate::specs::part1_magnitude_left_fits(self.limbs@, shift));
                        crate::proofs::production_impl::production_magnitude_left_fits_value(self.limbs@, shift);
                    }
                    return Err(arithmetic_overflow());
                }
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::proofs::production_impl::production_magnitude_left_prefix_step(self.limbs@, shift, source as nat);
            }
            source += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::proofs::production_impl::production_magnitude_left_prefix_complete(self.limbs@, shift);
            assert(limbs@ == crate::specs::part1_magnitude_left(self.limbs@, shift));
            assert(crate::specs::part1_magnitude_left_fits(self.limbs@, shift));
            crate::proofs::production_impl::production_magnitude_left_fits_value(self.limbs@, shift);
            crate::proofs::production_impl::production_magnitude_left_shift_value(self.limbs@, shift);
            vstd::arithmetic::div_mod::lemma_small_mod(
                magnitude_value(self) * vstd::arithmetic::power2::pow2(shift as nat),
                crate::specs::part1_magnitude_capacity(),
            );
            crate::proofs::production_impl::production_magnitude_value_bound(limbs@);
        }
        Ok(Self { limbs })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(result =>
        ensures magnitude_limbs(result) == crate::specs::part1_magnitude_shift(magnitude_limbs(self), shift),
        magnitude_value(self) == crate::specs::part1_magnitude_value(magnitude_limbs(self)),
        magnitude_value(result) == crate::specs::part1_magnitude_value(magnitude_limbs(result)),
        magnitude_value(result) == magnitude_value(self) / vstd::arithmetic::power2::pow2(shift as nat),
        magnitude_value(self) < crate::specs::part1_magnitude_capacity(),
        magnitude_value(result) < crate::specs::part1_magnitude_capacity(),
    ))]
    pub fn shift_right(self, shift: u16) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(MAGNITUDE_BITS == 384) by(compute);
            crate::proofs::production_impl::production_magnitude_right_shift_value(self.limbs@, shift);
            crate::proofs::production_impl::production_magnitude_value_bound(self.limbs@);
        }
        if shift >= MAGNITUDE_BITS {
            #[cfg(verus_keep_ghost)]
            proof! {
                let shifted = crate::specs::part1_magnitude_shift(self.limbs@, shift);
                assert(crate::specs::part1_magnitude_zero(shifted));
                crate::proofs::production_impl::production_magnitude_zero_value(shifted);
            }
            return magnitude_zero();
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
            return magnitude_zero();
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
                magnitude_value(self) < crate::specs::part1_magnitude_capacity(),
                crate::specs::part1_magnitude_value(crate::specs::part1_magnitude_shift(self.limbs@, shift))
                    == magnitude_value(self) / vstd::arithmetic::power2::pow2(shift as nat),
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
                return magnitude_from_limbs(limbs);
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
        magnitude_from_limbs(limbs)
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

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures result == crate::specs::part1_transform_error(true),
))]
fn arithmetic_overflow() -> DecodeError {
    DecodeError::MalformedInput {
        offset: 0,
        kind: MalformedKind::ArithmeticOverflow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed_limbs(value: Magnitude) -> [u64; 6] {
        std::array::from_fn(|i| value.limb(i).unwrap())
    }

    fn reference_bits(limbs: [u64; 6]) -> [bool; 384] {
        std::array::from_fn(|i| !(limbs[i / 64] / 2_u64.pow((i % 64) as u32)).is_multiple_of(2))
    }

    fn reference_words(bits: &[bool; 384]) -> [u64; 6] {
        let mut words = [0_u64; 6];
        for (i, set) in bits.iter().enumerate() {
            if *set {
                words[i / 64] += 2_u64.pow((i % 64) as u32);
            }
        }
        words
    }

    #[test]
    fn full_native_limb_getter_matches_independent_array_and_extreme_indices() {
        let mut vectors = vec![
            [0; 6],
            [u64::MAX; 6],
            [1, 2, 3, 4, 5, 6],
            [0, u64::MAX, 0, u64::MAX, 0, u64::MAX],
        ];
        for word in 0..6 {
            let mut limbs = [0; 6];
            limbs[word] = 1_u64 << 63;
            vectors.push(limbs);
        }
        for limbs in vectors {
            let value = Magnitude::from_limbs(limbs);
            for index in (0..=12).chain([usize::MAX / 2, usize::MAX - 1, usize::MAX]) {
                assert_eq!(value.limb(index), limbs.get(index).copied());
            }
        }
    }

    #[test]
    fn full_native_subtraction_matches_independent_bit_borrow_and_exact_underflow() {
        let mut vectors = vec![
            [0; 6],
            [u64::MAX; 6],
            [1, 2, 3, 4, 5, 6],
            [u64::MAX, 0, u64::MAX, 0, u64::MAX, 0],
        ];
        for word in 0..6 {
            let mut power = [0; 6];
            power[word] = 1;
            vectors.push(power);
            let mut below = [0; 6];
            below[..word].fill(u64::MAX);
            vectors.push(below);
        }
        let mut seed = 0xd1342543de82ef95_u64;
        for _ in 0..64 {
            let words = core::array::from_fn(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                seed
            });
            vectors.push(words);
        }
        let inputs: Vec<_> = vectors
            .into_iter()
            .map(|words| (words, reference_bits(words)))
            .collect();
        for (left, a) in &inputs {
            for (right, b) in &inputs {
                let mut difference = [false; 384];
                let mut borrow = false;
                for bit in 0..384 {
                    let digit = i16::from(a[bit]) - i16::from(b[bit]) - i16::from(borrow);
                    difference[bit] = digit.rem_euclid(2) != 0;
                    borrow = digit < 0;
                }
                let expected = if borrow {
                    Err(DecodeError::MalformedInput {
                        offset: 0,
                        kind: MalformedKind::ArithmeticOverflow,
                    })
                } else {
                    Ok(reference_words(&difference))
                };
                let actual = Magnitude::from_limbs(*left)
                    .checked_sub(Magnitude::from_limbs(*right))
                    .map(observed_limbs);
                assert_eq!(actual, expected, "left={left:?} right={right:?}");
            }
        }
    }

    #[test]
    fn full_native_bit_producers_preserve_all_limbs_and_exact_errors() {
        assert_eq!(magnitude_zero(), Magnitude::ZERO);
        assert_eq!(observed_limbs(magnitude_zero()), [0; 6]);
        for value in [0, 1, 1 << 63, u64::MAX] {
            assert_eq!(
                observed_limbs(magnitude_from_u64(value)),
                [value, 0, 0, 0, 0, 0]
            );
            assert_eq!(Magnitude::from_u64(value), magnitude_from_u64(value));
        }
        for bit in 0..=u16::MAX {
            let expected = if bit < 384 {
                let mut bits = [false; 384];
                bits[usize::from(bit)] = true;
                Ok(reference_words(&bits))
            } else {
                Err(DecodeError::MalformedInput {
                    offset: 0,
                    kind: MalformedKind::ArithmeticOverflow,
                })
            };
            assert_eq!(magnitude_single_bit(bit).map(observed_limbs), expected);
            assert_eq!(Magnitude::single_bit(bit).map(observed_limbs), expected);
        }
    }

    #[test]
    fn full_u16_shifts_and_modulo_getters_match_independent_384_bit_reference() {
        for words in [
            [0; 6],
            [1, 0, 0, 0, 0, 0],
            [u64::MAX, 0, 0, 0, 0, 0],
            [0, 1, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 1],
            [0, 0, 0, 0, 0, 1 << 63],
            [u64::MAX; 6],
            [
                0x0123456789abcdef,
                0xfedcba9876543210,
                7,
                u64::MAX,
                3,
                0x55aa55aa55aa55aa,
            ],
        ] {
            let input = magnitude_from_limbs(words);
            let bits = reference_bits(words);
            let zero = !bits.iter().any(|bit| *bit);
            assert_eq!(observed_limbs(input), words);
            assert_eq!(input.low_u64(), words[0]);
            assert_eq!(input.is_zero(), zero);
            for shift in 0..=u16::MAX {
                let shift = usize::from(shift);
                let (left, right) = if shift >= 384 {
                    (
                        if zero {
                            Ok([0; 6])
                        } else {
                            Err(arithmetic_overflow())
                        },
                        [0; 6],
                    )
                } else {
                    let right_bits = std::array::from_fn(|i| i + shift < 384 && bits[i + shift]);
                    let left = if bits[384 - shift..].iter().any(|bit| *bit) {
                        Err(arithmetic_overflow())
                    } else {
                        let left_bits = std::array::from_fn(|i| i >= shift && bits[i - shift]);
                        Ok(reference_words(&left_bits))
                    };
                    (left, reference_words(&right_bits))
                };
                assert_eq!(input.checked_shl(shift as u16).map(observed_limbs), left);
                assert_eq!(observed_limbs(input.shift_right(shift as u16)), right);
            }
        }
    }

    #[test]
    fn exact_full_capacity_addition_matches_bit_carry_reference() {
        let cases = [
            [0; 6],
            [1, 0, 0, 0, 0, 0],
            [u64::MAX; 6],
            [u64::MAX, u64::MAX, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 1 << 63],
            [7, 11, 13, 17, 19, 23],
        ];
        for left in cases {
            for right in cases {
                let left_bits = reference_bits(left);
                let right_bits = reference_bits(right);
                let mut output = [false; 384];
                let mut carry = 0_u8;
                for i in 0..384 {
                    let sum = u8::from(left_bits[i]) + u8::from(right_bits[i]) + carry;
                    output[i] = !sum.is_multiple_of(2);
                    carry = u8::from(sum >= 2);
                }
                let expected = if carry != 0 {
                    Err(arithmetic_overflow())
                } else {
                    Ok(reference_words(&output))
                };
                assert_eq!(
                    magnitude_from_limbs(left)
                        .checked_add(magnitude_from_limbs(right))
                        .map(observed_limbs),
                    expected
                );
            }
        }
        for trailing_ones in 0..=384 {
            let mut bits = [false; 384];
            bits[..trailing_ones].fill(true);
            let left = magnitude_from_limbs(reference_words(&bits));
            let expected = if trailing_ones == 384 {
                Err(arithmetic_overflow())
            } else {
                bits.fill(false);
                bits[trailing_ones] = true;
                Ok(reference_words(&bits))
            };
            assert_eq!(
                left.checked_add(magnitude_from_u64(1)).map(observed_limbs),
                expected
            );
        }
    }

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
