// Copyright 2018 Stichting Organism
//
// Copyright 2018 Friedel Ziegelmayer
//
// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A Big integer (signed version: `BigInt`, unsigned version: `BigUint`).
//!
//! A `BigUint` is represented as a vector of `BigDigit`s.
//! A `BigInt` is a combination of `BigUint` and `Sign`.
//!
// Copyright 2018 Stichting Organism

// Copyright 2018 Friedel Ziegelmayer

// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.

// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A Big integer (signed version: `BigInt`, unsigned version: `BigUint`).
//!
//! A `BigUint` is represented as a vector of `BigDigit`s.
//! A `BigInt` is a combination of `BigUint` and `Sign`.
//!
//! Common numerical operations are overloaded, so we can treat them
//! the same way we treat other numbers.
//!
//! ## Example
//!
//! ```rust
//! extern crate num_bigint_dig as num_bigint;
//! extern crate num_traits;
//!
#![...existing code...]
#[cfg(feature = "std")]
impl Error for ParseBigIntError {
    fn description(&self) -> &str {
        self.__description()
    }
}

pub use crate::biguint::BigUint;
pub use crate::biguint::IntoBigUint;
pub use crate::biguint::ToBigUint;

pub use crate::bigint::negate_sign;
pub use crate::bigint::BigInt;
pub use crate::bigint::IntoBigInt;
pub use crate::bigint::Sign;
pub use crate::bigint::ToBigInt;

#[cfg(feature = "rand")]
pub use crate::bigrand::{RandBigInt, RandomBits, UniformBigInt, UniformBigUint};

#[cfg(feature = "prime")]
pub use bigrand::RandPrime;

#[cfg(not(feature = "u64_digit"))]
pub const VEC_SIZE: usize = 8;

#[cfg(feature = "u64_digit")]
pub const VEC_SIZE: usize = 4;

mod big_digit {
    /// A `BigDigit` is a `BigUint`'s composing element.
    #[cfg(not(feature = "u64_digit"))]
    pub type BigDigit = u32;
    #[cfg(feature = "u64_digit")]
    pub type BigDigit = u64;

    /// A `DoubleBigDigit` is the internal type used to do the computations.  Its
    /// size is the double of the size of `BigDigit`.
    #[cfg(not(feature = "u64_digit"))]
    pub type DoubleBigDigit = u64;
    #[cfg(feature = "u64_digit")]
    pub type DoubleBigDigit = u128;

    /// A `SignedDoubleBigDigit` is the signed version of `DoubleBigDigit`.
    #[cfg(not(feature = "u64_digit"))]
    pub type SignedDoubleBigDigit = i64;
    #[cfg(feature = "u64_digit")]
    pub type SignedDoubleBigDigit = i128;

    // `DoubleBigDigit` size dependent
    #[cfg(not(feature = "u64_digit"))]
    pub const BITS: usize = 32;
    #[cfg(feature = "u64_digit")]
    pub const BITS: usize = 64;

    #[cfg(not(feature = "u64_digit"))]
    #[allow(dead_code)]
    const LO_MASK: DoubleBigDigit = (-1i32 as DoubleBigDigit) >> BITS;
    #[cfg(feature = "u64_digit")]
    #[allow(dead_code)]
    const LO_MASK: DoubleBigDigit = (-1i64 as DoubleBigDigit) >> BITS;

    #[inline]
    #[allow(dead_code)]
    fn get_hi(n: DoubleBigDigit) -> BigDigit {
        (n >> BITS) as BigDigit
    }
    #[inline]
    #[allow(dead_code)]
    fn get_lo(n: DoubleBigDigit) -> BigDigit {
        (n & LO_MASK) as BigDigit
    }

    /// Split one `DoubleBigDigit` into two `BigDigit`s.
    #[inline]
    #[allow(dead_code)]
    pub fn from_doublebigdigit(n: DoubleBigDigit) -> (BigDigit, BigDigit) {
        (get_hi(n), get_lo(n))
    }

    /// Join two `BigDigit`s into one `DoubleBigDigit`
    #[inline]
    pub fn to_doublebigdigit(hi: BigDigit, lo: BigDigit) -> DoubleBigDigit {
        (DoubleBigDigit::from(lo)) | ((DoubleBigDigit::from(hi)) << BITS)
    }
}
