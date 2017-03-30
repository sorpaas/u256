// Rust Bitcoin Library
// Written in 2014 by
//     Andrew Poelstra <apoelstra@wpsoftware.net>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the CC0 Public Domain Dedication
// along with this software.
// If not, see <http://creativecommons.org/publicdomain/zero/1.0/>.
//

//! # Big unsigned integer types
//!
//! Implementation of a various large-but-fixed sized unsigned integer types.
//! The functions here are designed to be fast.
//!

#![no_std]

use core::convert::{From, Into};
use core::ops::{Add, Sub, Not, Mul, Div, Shr, Shl};
use core::cmp::Ordering;

#[repr(C)]
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub struct U256([u64; 4]);

impl U256 {
    pub fn zero() -> U256 { U256([0; 4]) }

    pub fn low_u32(&self) -> u32 {
        let &U256(ref arr) = self;
        arr[0] as u32
    }

    pub fn mul_u32(self, other: u32) -> U256 {
        let U256(ref arr) = self;
        let mut carry = [0u64; 4];
        let mut ret = [0u64; 4];
        for i in 0..4 {
            let upper = other as u64 * (arr[i] >> 32);
            let lower = other as u64 * (arr[i] & 0xFFFFFFFF);
            if i < 3 {
                carry[i + 1] += upper >> 32;
            }
            ret[i] = lower + (upper << 32);
        }
        U256(ret) + U256(carry)
    }

    pub fn bits(&self) -> usize {
        let &U256(ref arr) = self;
        for i in 1..4 {
            if arr[4 - i] > 0 { return (0x40 * (4 - i + 1)) - arr[4 - i].leading_zeros() as usize; }
        }
        0x40 - arr[0].leading_zeros() as usize
    }
}

impl From<u64> for U256 {
    fn from(val: u64) -> U256 {
        U256([0, 0, 0, val])
    }
}

impl Into<u64> for U256 {
    fn into(self) -> u64 {
        assert!(self.0[0] == 0 && self.0[1] == 0 && self.0[2] == 0);
        self.0[3]
    }
}

impl From<usize> for U256 {
    fn from(val: usize) -> U256 {
        (val as u64).into()
    }
}

impl Into<usize> for U256 {
    fn into(self) -> usize {
        let v64: u64 = self.into();
        v64 as usize
    }
}

impl From<i32> for U256 {
    fn from(val: i32) -> U256 {
        (val as u64).into()
    }
}

impl<'a> From<&'a [u8]> for U256 {
    fn from(val: &'a [u8]) -> U256 {
        assert!(val.len() <= 256 / 8);
        let mut u256 = U256::zero();

        for i in 0..val.len() {
            let rev = val.len() - 1 - i;
            let pos = rev / 8;
            u256.0[pos] += (val[i] as u64) << ((rev % 8) * 8);
        }

        u256
    }
}

impl Not for U256 {
    type Output = U256;

    fn not(self) -> U256 {
        let U256(ref arr) = self;
        let mut ret = [0u64; 4];
        for i in 0..4 {
            ret[i] = !arr[i];
        }
        U256(ret)
    }
}

impl Add for U256 {
    type Output = U256;

    fn add(self, other: U256) -> U256 {
        let U256(ref me) = self;
        let U256(ref you) = other;

        let mut ret = [0u64; 4];
        let mut carry = false;
        for i in 0..4 {
            let (v, o) = me[i].overflowing_add(you[i]);
            ret[i] = v + (if carry { 1 } else { 0 });
            carry = o;
        }

        assert!(carry == false);
        U256(ret)
    }
}

impl Sub for U256 {
    type Output = U256;

    #[inline]
    fn sub(self, other: U256) -> U256 {
        self + !other
    }
}

impl Mul for U256 {
    type Output = U256;

    fn mul(self, other: U256) -> U256 {
        let mut me = self;
        // TODO: be more efficient about this
        for i in 0..(2 * 4) {
            me = (me + me.mul_u32((other >> (32 * i)).low_u32())) << (32 * i);
        }
        me
    }
}

impl Div for U256 {
    type Output = U256;

    fn div(self, other: U256) -> U256 {
        let mut sub_copy = self;
        let mut shift_copy = other;
        let mut ret = [0u64; 4];

        let my_bits = self.bits();
        let your_bits = other.bits();

        // Check for division by 0
        assert!(your_bits != 0);

        // Early return in case we are dividing by a larger number than us
        if my_bits < your_bits {
            return U256(ret);
        }

        // Bitwise long division
        let mut shift = my_bits - your_bits;
        shift_copy = shift_copy << shift;
        loop {
            if sub_copy >= shift_copy {
                ret[shift / 64] |= 1 << (shift % 64);
                sub_copy = sub_copy - shift_copy;
            }
            shift_copy = shift_copy >> 1;
            if shift == 0 { break; }
            shift -= 1;
        }

        U256(ret)
    }
}

impl Ord for U256 {
    fn cmp(&self, other: &U256) -> Ordering {
	let &U256(ref me) = self;
	let &U256(ref you) = other;
	let mut i = 4;
	while i > 0 {
	    i -= 1;
	    if me[i] < you[i] { return Ordering::Less; }
	    if me[i] > you[i] { return Ordering::Greater; }
	}
	Ordering::Equal
    }
}

impl PartialOrd for U256 {
    fn partial_cmp(&self, other: &U256) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Shl<usize> for U256 {
    type Output = U256;

    fn shl(self, shift: usize) -> U256 {
        let U256(ref original) = self;
        let mut ret = [0u64; 4];
        let word_shift = shift / 64;
        let bit_shift = shift % 64;
        for i in 0..4 {
            // Shift
            if bit_shift < 64 && i + word_shift < 4 {
                ret[i + word_shift] += original[i] << bit_shift;
            }
            // Carry
            if bit_shift > 0 && i + word_shift + 1 < 4 {
                ret[i + word_shift + 1] += original[i] >> (64 - bit_shift);
            }
        }
        U256(ret)
    }
}

impl Shr<usize> for U256 {
    type Output = U256;

    fn shr(self, shift: usize) -> U256 {
        let U256(ref original) = self;
        let mut ret = [0u64; 4];
        let word_shift = shift / 64;
        let bit_shift = shift % 64;
        for i in word_shift..4 {
            // Shift
            ret[i - word_shift] += original[i] >> bit_shift;
            // Carry
            if bit_shift > 0 && i < 4 - 1 {
                ret[i - word_shift] += original[i + 1] << (64 - bit_shift);
            }
        }
        U256(ret)
    }
}

#[cfg(test)]
mod tests {
    use U256;

    #[test]
    fn u256_add() {
        assert_eq!(
            U256([0xffffffffffffffffu64, 0u64, 0u64, 0u64]) +
                U256([0xffffffffffffffffu64, 0u64, 0u64, 0u64]),
            U256([0xfffffffffffffffeu64, 1u64, 0u64, 0u64])
        );
    }
}
