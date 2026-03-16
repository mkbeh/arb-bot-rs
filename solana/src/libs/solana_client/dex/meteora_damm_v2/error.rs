use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum PoolError {
    #[error("Math operation overflow")]
    MathOverflow,

    #[error("Invalid fee setup")]
    InvalidFee,

    #[error("Exceeded max fee bps")]
    ExceedMaxFeeBps,

    #[error("Type cast error")]
    TypeCastFailed,

    #[error("Trade is over price range")]
    PriceRangeViolation,

    #[error("Invalid pool version")]
    InvalidPoolVersion,

    #[error("Undetermined error")]
    UndeterminedError,

    #[error("Invalid base fee mode")]
    InvalidBaseFeeMode,

    #[error("Invalid fee market cap scheduler")]
    InvalidFeeMarketCapScheduler,

    #[error("Invalid fee rate limiter")]
    InvalidFeeRateLimiter,

    #[error("Invalid fee scheduler")]
    InvalidFeeTimeScheduler,

    #[error("Invalid collect fee mode")]
    InvalidCollectFeeMode,
}
