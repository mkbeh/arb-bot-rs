use std::fmt::Display;

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
            SymbolOrder::Asc => f.write_str("asc"),
            SymbolOrder::Desc => f.write_str("desc"),
        }
    }
}
