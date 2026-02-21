/// Trait for mandatory DEX metadata
pub trait DexMetrics {
    /// Returns a unique string name of the DEX
    fn dex_name(&self) -> &'static str;
}
