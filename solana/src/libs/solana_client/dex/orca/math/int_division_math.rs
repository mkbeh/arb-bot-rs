#[must_use]
pub fn floor_division(dividend: i32, divisor: i32) -> i32 {
    assert!(divisor > 0, "Divisor must be positive.");
    if dividend % divisor == 0 || dividend.signum() == divisor.signum() {
        dividend / divisor
    } else {
        dividend / divisor - 1
    }
}
