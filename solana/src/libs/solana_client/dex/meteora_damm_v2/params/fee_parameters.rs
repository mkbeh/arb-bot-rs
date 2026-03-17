use crate::libs::solana_client::dex::meteora_damm_v2::{
    error::PoolError, fee::*, math::safe_math::SafeMath,
};

pub fn validate_fee_fraction(numerator: u64, denominator: u64) -> anyhow::Result<()> {
    if denominator == 0 || numerator >= denominator {
        Err(PoolError::InvalidFee.into())
    } else {
        Ok(())
    }
}

pub fn to_numerator(bps: u128, denominator: u128) -> anyhow::Result<u64> {
    let numerator = bps
        .safe_mul(denominator)?
        .safe_div(MAX_BASIS_POINT.into())?;
    Ok(u64::try_from(numerator).map_err(|_| PoolError::TypeCastFailed)?)
}
