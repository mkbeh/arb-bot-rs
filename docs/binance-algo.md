# Binance

[Binance API documentation](https://developers.binance.com/docs/binance-spot-api-docs)

## Алгоритм

* [Получить список доступных тикеров](#получить-список-доступных-тикеров)

* [**(Опционально)** Отфильтровать тикеры по объемам за последние 24ч](#опционально-отфильтровать-тикеры-по-объемам-за-последние-24ч)

* [Сформировать цепочки тикеров на основе списка базовых валют](#сформировать-цепочки-тикеров-на-основе-списка-базовых-валют)

* [Получить список bids/ascs по тикерам в стакане](#получить-список-bidsascs-по-тикерам-в-стакане)

* [Рассчитать по алгоритму возможный профит по цепочкам тикеров](#рассчитать-по-алгоритму-возможный-профит-по-цепочкам-тикеров)

* [Выставить ордера](#выставить-ордера)

### Получить список доступных тикеров

[Exchange information API](https://developers.binance.com/docs/binance-spot-api-docs/rest-api/general-endpoints#exchange-information)

```shell
curl -X GET "https://api.binance.com/api/v3/exchangeInfo?symbolStatus=TRADING&showPermissionSets=false&permissions=\[\"SPOT\"\]" | jq | > symbols.json
```

### (Опционально) Отфильтровать тикеры по объемам за последние 24ч

[24hr ticker price change statistics API](https://developers.binance.com/docs/binance-spot-api-docs/rest-api/market-data-endpoints#24hr-ticker-price-change-statistics)

```shell
curl -X GET https://api.binance.com/api/v3/ticker/24hr?type=MINI | jq | > volumes.json
```

### Сформировать цепочки тикеров на основе списка базовых валют

**Список базовых валют:**

* BTC
* ETH
* LTC
* BNB
* USDT
* USDC
* FDUSD
* EUR
* TRY
* BRL
* JPY

### Получить список bids/ascs по тикерам в стакане

[Order book API](https://developers.binance.com/docs/binance-spot-api-docs/rest-api/market-data-endpoints#order-book)

```shell
curl -X GET https://api.binance.com/api/v3/depth?symbol=ETHBTC&limit=100 | jq | > orders.json
```

### Рассчитать по алгоритму возможный профит по цепочкам тикеров

**Дополнительно:**

* учесть комиссии за сделки
* учесть погрешность по профиту

[TODO desc]

### Выставить ордера

[TODO desc]
