#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ErrorCode {
    #[error("Not approved")]
    NotApproved,

    #[error("Tick out of range")]
    InvalidTickIndex,
    #[error("Tick upper overflow")]
    TickUpperOverflow,
    #[error("Invalid tick array account")]
    InvalidTickArray,
    #[error("Invalid tick array boundary")]
    InvalidTickArrayBoundary,

    #[error("Sqrt price limit overflow")]
    SqrtPriceLimitOverflow,
    // second inequality must be < because the price can never reach the price at the max tick
    #[error("sqrt_price_x64 out of range")]
    SqrtPriceX64,

    #[error("Liquidity sub value error")]
    LiquiditySubValueErr,
    #[error("Liquidity add value error")]
    LiquidityAddValueErr,

    #[error("Zero amount specified")]
    ZeroAmountSpecified,
    #[error("Not enough tick array account")]
    NotEnoughTickArrayAccount,

    #[error("Missing tickarray bitmap extension account")]
    MissingTickArrayBitmapExtensionAccount,
    #[error("Max token overflow")]
    MaxTokenOverflow,
    #[error("Calculate overflow")]
    CalculateOverflow,
    #[error("Insufficient liquidity for direction")]
    InsufficientLiquidityForDirection,
}
