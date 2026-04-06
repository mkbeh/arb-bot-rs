/// Trait for mandatory protocol metadata
pub trait ProtocolMetrics {
    /// Returns a unique string name of the DEX
    fn name(&self) -> &'static str;
}
