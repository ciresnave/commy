use crate::algorithms::mac3;
use crate::big_digit::{BigDigit, DoubleBigDigit, BITS};
use crate::BigUint;
use crate::VEC_SIZE;
use smallvec::SmallVec;

#[inline]
pub fn mul_with_carry(a: BigDigit, b: BigDigit, acc: &mut DoubleBigDigit) -> BigDigit {
    *acc += (a as DoubleBigDigit) * (b as DoubleBigDigit);
    let lo = *acc as BigDigit;
    *acc >>= BITS;
    lo
}

pub fn mul3(x: &[BigDigit], y: &[BigDigit]) -> BigUint {
    let len = x.len() + y.len() + 1;
    let mut prod = BigUint {
        data: {
            let mut d: SmallVec<[BigDigit; VEC_SIZE]> = SmallVec::with_capacity(len);
            d.resize(len, 0);
            d
        },
    };

    mac3(&mut prod.data[..], x, y);
    prod.normalized()
}

pub fn scalar_mul(a: &mut [BigDigit], b: BigDigit) -> BigDigit {
    let mut carry = 0;
    for a in a.iter_mut() {
        *a = mul_with_carry(*a, b, &mut carry);
    }
    carry as BigDigit
}
