use std::collections::HashMap;

use anyhow::bail;
use rust_decimal::{Decimal, prelude::Zero};

use crate::{
    config::Asset,
    libs::kucoin_api::{Market, models::Ticker},
};

pub struct AssetBuilder {
    market_api: Market,
    base_assets: Vec<Asset>,
    min_profit_qty: Decimal,
    max_order_qty: Decimal,
    min_ticker_qty_24h: Decimal,
}

impl AssetBuilder {
    pub fn new(
        market_api: Market,
        base_assets: Vec<Asset>,
        min_profit_qty: Decimal,
        max_order_qty: Decimal,
        min_ticker_qty_24h: Decimal,
    ) -> Self {
        Self {
            market_api,
            base_assets,
            min_profit_qty,
            max_order_qty,
            min_ticker_qty_24h,
        }
    }

    pub async fn update_base_assets_info(&self) -> anyhow::Result<Vec<Asset>> {
        let symbols: Vec<_> = self
            .base_assets
            .iter()
            .filter_map(|a| a.symbol.clone())
            .collect();

        let stats = if symbols.is_empty() {
            vec![]
        } else {
            let resp = self.market_api.get_all_tickers().await?;
            resp.data.ticker
        };

        let stats_map: HashMap<_, _> = stats
            .iter()
            .map(|stat| (stat.symbol.clone(), stat))
            .collect();

        let assets = self
            .base_assets
            .iter()
            .map(
                |asset| match asset.symbol.as_ref().and_then(|s| stats_map.get(s)) {
                    Some(stat) => self.set_asset_volumes(asset, stat).unwrap(),
                    None => asset.clone(),
                },
            )
            .collect();

        Ok(assets)
    }

    fn set_asset_volumes(&self, asset: &Asset, stat: &Ticker) -> anyhow::Result<Asset> {
        let mut new_asset = asset.clone();

        if stat.high == Decimal::zero() {
            bail!("Price for asset {} is zero", asset.symbol.clone().unwrap());
        }

        if asset.symbol.clone().unwrap().starts_with("USDT") {
            new_asset.min_profit_qty = self.min_profit_qty * stat.high;
            new_asset.max_order_qty = self.max_order_qty * stat.high;
            new_asset.min_ticker_qty_24h = self.min_ticker_qty_24h * stat.high;
        } else {
            new_asset.min_profit_qty = self.min_profit_qty / stat.high;
            new_asset.max_order_qty = self.max_order_qty / stat.high;
            new_asset.min_ticker_qty_24h = self.min_ticker_qty_24h / stat.high;
        }

        Ok(new_asset)
    }
}
