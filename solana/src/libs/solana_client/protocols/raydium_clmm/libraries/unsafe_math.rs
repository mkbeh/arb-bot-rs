use super::U256;

pub trait UnsafeMathTrait {
    /// Returns ceil (x / y)
    /// Division by 0 throws a panic, and must be checked externally
    ///
    /// In Solidity dividing by 0 results in 0, not an exception.
    fn div_rounding_up(x: Self, y: Self) -> Self;
}

impl UnsafeMathTrait for U256 {
    fn div_rounding_up(x: Self, y: Self) -> Self {
        x / y + Self::from((x % y > Self::default()) as u8)
    }
}
