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
}

impl AssetBuilder {
    pub fn new(
        market_api: Market,
        base_assets: Vec<Asset>,
        min_profit_qty: Decimal,
        max_order_qty: Decimal,
    ) -> Self {
        Self {
            market_api,
            base_assets,
            min_profit_qty,
            max_order_qty,
        }
    }

    /// Get and update asset limits.
    pub async fn update_base_assets_info(&self) -> anyhow::Result<Vec<Asset>> {
        let set_asset_volumes_fn = |asset: &Asset, stat: &TickerPriceStats| -> Asset {
            let mut new_asset = asset.clone();

            if asset.symbol.clone().unwrap().starts_with("USDT") {
                new_asset.min_profit_qty = self.min_profit_qty * stat.last_price;
                new_asset.max_order_qty = self.max_order_qty * stat.last_price;
            } else {
                new_asset.min_profit_qty = self.min_profit_qty / stat.last_price;
                new_asset.max_order_qty = self.max_order_qty / stat.last_price;
            }

            new_asset
        };

        let symbols = self
            .base_assets
            .iter()
            .filter_map(|a| a.symbol.clone())
            .collect();

        let stats = self
            .market_api
            .get_ticker_price_24h(Some(symbols), TickerPriceResponseType::Mini)
            .await?;

        let mut assets = vec![];

        for asset in &self.base_assets {
            let mut found = false;
            for stat in &stats {
                if asset.symbol == Some(stat.symbol.clone()) {
                    assets.push(set_asset_volumes_fn(asset, stat));
                    found = true;
                }
            }

            if !found {
                assets.push(asset.clone());
            }
        }

        Ok(assets)
    }
}
