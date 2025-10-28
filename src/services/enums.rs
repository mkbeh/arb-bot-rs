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

pub enum OrderChainStatus {
    New,
    Filled,
    Cancelled,
}

impl Display for OrderChainStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderChainStatus::New => write!(f, "new"),
            OrderChainStatus::Filled => write!(f, "filled"),
            OrderChainStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}
