use std::{collections::HashMap, sync::LazyLock};

use solana_sdk::pubkey::Pubkey;
use zerocopy::{FromBytes, Unaligned};

use crate::libs::solana_client::dex::{
    meteora_dlmm::prelude::*,
    model::{PoolState, TxEvent},
};

pub static DEX_REGISTRY: LazyLock<HashMap<Pubkey, RegistryItem>> = LazyLock::new(init_dex_registry);

pub type ParserFn<T> = fn(&[u8]) -> Option<T>;

pub struct RegistryItem {
    pub name: &'static str,
    pub pool_size: u64,
    pub parser: DexParsers,
}

pub struct DexParsers {
    pub tx: ParserFn<TxEvent>,
    pub pool: ParserFn<PoolState>,
}

fn init_dex_registry() -> HashMap<Pubkey, RegistryItem> {
    let mut m = HashMap::new();

    m.insert(
        METEORA_DLMM,
        RegistryItem {
            name: "meteora_dlmm",
            pool_size: METEORA_DLMM_POOL_SIZE,
            parser: DexParsers {
                tx: |d| generic_parser(d, METEORA_DLMM_SWAP_DISCR, TxEvent::MeteoraDLMM),
                pool: |d| {
                    generic_parser::<MeteoraPoolDLMM, _>(d, METEORA_DLMM_ACCOUNT_DISCR, |data| {
                        PoolState::MeteoraDLMM(Box::new(data))
                    })
                },
            },
        },
    );

    m
}

fn generic_parser<Data, Event>(
    data: &[u8],
    discriminator: [u8; 8],
    wrap: fn(Data) -> Event,
) -> Option<Event>
where
    Data: FromBytes + Unaligned + Copy,
{
    if data.len() < 8 || data[0..8] != discriminator {
        return None;
    }

    let payload = data.get(8..)?;
    let (val, _) = Data::read_from_prefix(payload).ok()?;
    Some(wrap(val))
}
