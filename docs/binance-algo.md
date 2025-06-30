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

**Список базовых активов:**

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

#### Алгоритм формирования цепочек

**Цепочка из тикеров:** BTS:CNY | CNY:USD | USD:BTS

**Краткое описание алгоритма:** необходимо продать BTS за CNY, купить за CNY USD и продать USD за BTS.

**Примеры:**

**Пример №0 (ничего не раскручивать):**

> **Оригинал:** BTC:USDT - USDT:ETH - ETH:BTC

> **Требуется:** BTC:USDT - USDT:ETH - ETH:BTC

**Пример №1 (раскручивать только 1 пару):**

> **Оригинал:** **BTC:USDT** - BTC:ETH - ETH:USDT

> **Требуется:** **USDT:BTC** - BTC:ETH - ETH:USDT

**Пример №2 (раскручивать только 2 пару):**

> **Оригинал:** BTC:USDT - **ETH:USDT** - ETH:BTC

> **Требуется:** BTC:USDT - **USD:TETH** - ETH:BTC

**Пример №3 (раскручивать только 3 пару):**

> **Оригинал:** ETH:BTC - BTC:USDT - **ETH:USDT**

> **Требуется:** ETH:BTC - BTC:USDT - **USDT:ETH**


**Пример №4 (раскручивать 1 и 2 пары):**

> **Оригинал:** **BTC:USDT** - **BND:BTC** - BNB:USDT

> **Требуется:** **USDT:BTC** - **BTC:BNB** - BNB:USDT

**Пример №5 (раскручивать все 3 пары):**

> **Оригинал:** **ETH:BTC** - **RLC:ETH** - **BTC:RLC**

> **Требуется:** **BTC:ETH** - **ETH:RLC** - **RLC:BTC**

**Пример №6 (раскручивать 2 и 3 пары):**

> **Оригинал:** ETH:BTC - **QTUM:BTC** - **ETH:QTUM**

> **Требуется:** ETH:BTC - **BTC:QTUM** - **QTUM:ETH**


**Примечание:**

1. Раскручивать 2 пару нужно так чтобы она не совпала с 1 парой.

   > **Пример:** ETH:BTC - BTC:ETH - ... - некорректно

   > **Пример:** ETH:BTC - BTC:QTUM - ... - корректно

2. Выход из 3 пары должен быть на базовый актив 1 пары.

   > **Пример:** **ETH:BTC** - BTC:QTUM - **QTUM:USDT** - некорректно

   > **Пример:** **ETH:BTC** - BTC:QTUM - **QTUM:ETH** - корректно


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
