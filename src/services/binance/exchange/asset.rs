use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::{
    config::Asset,
    libs::binance_api::{Market, TickerPriceResponseType, TickerPriceStats},
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

    /// Get and update asset limits.
    pub async fn update_base_assets_info(&self) -> anyhow::Result<Vec<Asset>> {
        let symbols: Vec<_> = self
            .base_assets
            .iter()
            .filter_map(|a| a.symbol.clone())
            .collect();

        let stats = if symbols.is_empty() {
            vec![]
        } else {
            self.market_api
                .get_ticker_price_24h(Some(symbols), TickerPriceResponseType::Mini)
                .await?
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
                    Some(stat) => self.set_asset_volumes(asset, stat),
                    None => asset.clone(),
                },
            )
            .collect();

        Ok(assets)
    }

    fn set_asset_volumes(&self, asset: &Asset, stat: &TickerPriceStats) -> Asset {
        let mut new_asset = asset.clone();

        if asset.symbol.clone().unwrap().starts_with("USDT") {
            new_asset.min_profit_qty = self.min_profit_qty * stat.last_price;
            new_asset.max_order_qty = self.max_order_qty * stat.last_price;
            new_asset.min_ticker_qty_24h = self.min_ticker_qty_24h * stat.last_price;
        } else {
            new_asset.min_profit_qty = self.min_profit_qty / stat.last_price;
            new_asset.max_order_qty = self.max_order_qty / stat.last_price;
            new_asset.min_ticker_qty_24h = self.min_ticker_qty_24h / stat.last_price;
        }

        new_asset
    }
}
