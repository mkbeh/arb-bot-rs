use std::fmt::{Display, Formatter};

use strum_macros::EnumIter;

/// Order direction for symbols in a trading chain (ascending/descending).
#[derive(Clone, Debug, Copy, PartialEq, Eq, Default, EnumIter)]
pub enum SymbolOrder {
    #[default]
    Asc,
    Desc,
}

impl Display for SymbolOrder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Asc => f.write_str("ASC"),
            Self::Desc => f.write_str("DESC"),
        }
    }
}

/// Status of a trading chain.
pub enum ChainStatus {
    /// Chain newly received.
    New,
    /// Chain successfully filled.
    Filled,
    /// Chain cancelled due to error.
    Cancelled,
}

impl Display for ChainStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::New => write!(f, "new"),
            Self::Filled => write!(f, "filled"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}
