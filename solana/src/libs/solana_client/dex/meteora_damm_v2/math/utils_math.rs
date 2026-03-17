use num_traits::cast::FromPrimitive;
use ruint::aliases::U256;

use super::u128x128_math::Rounding;
use crate::libs::solana_client::dex::meteora_damm_v2::{
    error::PoolError, math::safe_math::SafeMath,
};

#[inline]
pub fn safe_mul_div_cast_u64<T: FromPrimitive>(
    x: u64,
    y: u64,
    denominator: u64,
    rounding: Rounding,
) -> anyhow::Result<T> {
    let prod = u128::from(x).safe_mul(y.into())?;
    let denominator: u128 = denominator.into();

    let result = match rounding {
        Rounding::Up => prod
            .safe_add(denominator)?
            .safe_sub(1u128)?
            .safe_div(denominator)?,
        Rounding::Down => prod.safe_div(denominator)?,
    };

    T::from_u128(result).ok_or_else(|| PoolError::TypeCastFailed.into())
}

pub fn safe_mul_div_cast_u128(
    x: u128,
    y: u128,
    denominator: u128,
    rounding: Rounding,
) -> anyhow::Result<u128> {
    let prod = U256::from(x).safe_mul(U256::from(y))?;
    let denominator = U256::from(denominator);

    let result = match rounding {
        Rounding::Up => prod.div_ceil(denominator),
        Rounding::Down => prod.safe_div(denominator)?,
    };

    result
        .try_into()
        .map_err(|_| PoolError::TypeCastFailed.into())
}

// Ref: https://github.com/MeteoraAg/dynamic-bonding-curve/blob/dd40b7d4d53bf2254f395b9f52eb7f6850d24236/programs/dynamic-bonding-curve/src/math/utils_math.rs#L74
pub fn sqrt_u256(radicand: U256) -> Option<U256> {
    if radicand == U256::ZERO {
        return Some(U256::ZERO);
    }
    // Compute bit, the largest power of 4 <= n
    let max_shift = U256::ZERO.leading_zeros() - 1;
    let shift = (max_shift - radicand.leading_zeros()) & !1;
    let mut bit = U256::ONE.checked_shl(shift)?;

    let mut n = radicand;
    let mut result = U256::ZERO;
    while bit != U256::ZERO {
        let result_with_bit = result.checked_add(bit)?;
        if n >= result_with_bit {
            n = n.checked_sub(result_with_bit)?;
            result = result.checked_shr(1)?.checked_add(bit)?;
        } else {
            result = result.checked_shr(1)?;
        }
        (bit, _) = bit.overflowing_shr(2);
    }
    Some(result)
}
