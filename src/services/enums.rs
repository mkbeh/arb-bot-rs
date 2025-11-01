use std::fmt::{Display, Formatter};

use strum_macros::EnumIter;

#[derive(Clone, Debug, Copy, PartialEq, Eq, Default, EnumIter)]
pub enum SymbolOrder {
    #[default]
    Asc,
    Desc,
}

impl Display for SymbolOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolOrder::Asc => f.write_str("ASC"),
            SymbolOrder::Desc => f.write_str("DESC"),
        }
    }
}

pub enum ChainStatus {
    New,
    Filled,
    Cancelled,
}

impl Display for ChainStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainStatus::New => write!(f, "new"),
            ChainStatus::Filled => write!(f, "filled"),
            ChainStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}
