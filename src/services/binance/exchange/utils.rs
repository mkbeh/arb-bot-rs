use rust_decimal::Decimal;

// Round Decimal to specified step.
pub fn round_to_step(value: Decimal, step: Decimal) -> Decimal {
    (value / step).round() * step
}
