# Binance

[Binance API documentation](https://developers.binance.com/docs/binance-spot-api-docs)

## Content

1. [Get list of available tickers](#get-list-of-available-tickers)
2. [(Optional) Filter tickers by volumes over the last 24 hours](#optional-filter-tickers-by-volumes-over-the-last-24-hours)
3. [Build ticker chains](#build-ticker-chains)
4. [Receive orders by symbols from the order book and calculate possible profit according to the algorithm](#receive-orders-by-symbols-from-the-order-book-and-calculate-possible-profit-according-to-the-algorithm)
    1. [Calculate profit exclude fee](#calculate-profit-exclude-fee)
    2. [Calculate profit with sum orders volumes](#calculate-profit-with-sum-orders-volumes)
5. [Send orders](#send-orders)

## Get list of available tickers

[Exchange information API](https://developers.binance.com/docs/binance-spot-api-docs/rest-api/general-endpoints#exchange-information)

```shell
curl -X GET "https://api.binance.com/api/v3/exchangeInfo?symbolStatus=TRADING&showPermissionSets=false&permissions=\[\"SPOT\"\]" | jq | > symbols.json
```

## (Optional) Filter tickers by volumes over the last 24 hours

[24hr ticker price change statistics API](https://developers.binance.com/docs/binance-spot-api-docs/rest-api/market-data-endpoints#24hr-ticker-price-change-statistics)

```shell
curl -X GET https://api.binance.com/api/v3/ticker/24hr?type=MINI | jq | > volumes.json
```

## Build ticker chains

Ticker chains are build based on base assets.

**Example list of base assets:** BTC, ETH, LTC, BNB, USDT, USDC, FDUSD, EUR, TRY, BRL, JPY.

### Chain build algorithm

**Chain of symbols:** BTC:USDT - USDT:ETH - ETH:BTC

**Examples:**

**Example #1 (no changes):**

> **B:** BTC:USDT - USDT:ETH - ETH:BTC
>
> **A:** BTC:USDT - USDT:ETH - ETH:BTC

**Example #2 (swap 1st symbol):**

> **B:** **BTC:USDT** - BTC:ETH - ETH:USDT
>
> **A:** **USDT:BTC** - BTC:ETH - ETH:USDT

**Example #3 (swap 2nd symbol):**

> **B:** BTC:USDT - **ETH:USDT** - ETH:BTC
>
> **A:** BTC:USDT - **USD:TETH** - ETH:BTC

**Example #4 (swap 3rd symbol):**

> **B:** ETH:BTC - BTC:USDT - **ETH:USDT**
>
> **A:** ETH:BTC - BTC:USDT - **USDT:ETH**


**Example #5 (swap 1st and 2nd symbols):**

> **B:** **BTC:USDT** - **BND:BTC** - BNB:USDT
>
> **A:** **USDT:BTC** - **BTC:BNB** - BNB:USDT

**Example #6 (swap all symbols):**

> **B:** **ETH:BTC** - **RLC:ETH** - **BTC:RLC**
>
> **A:** **BTC:ETH** - **ETH:RLC** - **RLC:BTC**

**Example #7 (swap 2nd and 3rd symbols):**

> **B:** ETH:BTC - **QTUM:BTC** - **ETH:QTUM**
>
> **A:** ETH:BTC - **BTC:QTUM** - **QTUM:ETH**


**Notes:**

* Swapped 2nd symbol should not match with 1st symbol.

  > **Example:** ETH:BTC - BTC:ETH - ... - not valid
  >
  > **Example:** ETH:BTC - BTC:QTUM - ... - valid

* Exit from the 3rd symbol should be on the base asset of the 1st symbol.

  > **Example:** **ETH:BTC** - BTC:QTUM - **QTUM:USDT** - not valid
  >
  > **Example:** **ETH:BTC** - BTC:QTUM - **QTUM:ETH** - valid

## Receive orders by symbols from the order book and calculate possible profit according to the algorithm

[Order book API](https://developers.binance.com/docs/binance-spot-api-docs/rest-api/market-data-endpoints#order-book)

```shell
curl -X GET https://api.binance.com/api/v3/depth?symbol=ETHBTC&limit=100 | jq | > orders.json
```

**Example:**

> BTC:USDT (**ASC**) | ETH:USDT (**DESC**) | ETH:BTC (**ASC**)

### Calculate profit (exclude fee):

**BTC volume limit:** 0.00027 BTC = ~30$

**Fee %:** [Maker/Taker - 0.075000% / 0.075000% (BNB discount)](https://www.binance.com/ru/fee/schedule)

**Order Book:**

```json
// BTC:USDT
{
  "lastUpdateId": 72224518924,
  "bids": [
    [
      "109615.46000000",
      "7.27795000"
    ],
    [
      "109614.96000000",
      "0.00046000"
    ],
    ...
  ],
  "asks": [
    [
      "109615.47000000",
      "2.22969000"
    ],
    [
      "109615.48000000",
      "0.00028000"
    ],
    ...
  ]
}
```

```json
// ETH:USDT
{
  "lastUpdateId": 54622041690,
  "bids": [
    [
      "2585.70000000",
      "14.64600000"
    ],
    [
      "2585.69000000",
      "0.00210000"
    ],
    ...
  ],
  "asks": [
    [
      "2585.71000000",
      "19.28810000"
    ],
    [
      "2585.72000000",
      "0.40280000"
    ],
    ...
  ]
}
```

```json
// ETH:BTC
{
  "lastUpdateId": 8215337504,
  "bids": [
    [
      "0.02358000",
      "105.74550000"
    ],
    [
      "0.02357000",
      "57.30640000"
    ],
    ...
  ],
  "asks": [
    [
      "0.02359000",
      "25.63400000"
    ],
    [
      "0.02360000",
      "53.22680000"
    ],
    ...
  ]
}
```

1. **BTC:USDT [ASC] => [BID]** - sell BTC for USDT (**SELL ORDER**).

   > 0.00027 BTC * 109615.46000000 (USDT price for 1 BTC) = 29.59 USDT (volume)

2. **ETH:USDT [DESC] => [ASC]** - buy ETH for USDT (**BUY ORDER**).

   > 29.59 USDT / 2585.71000000 (USDT price for 1 ETH) = 0.01144371 ETH (volume)

3. **ETH:BTC [ASC] => [BID]** - sell ETH for BTC (**SELL ORDER**).

   > 0.01144371 ETH * 0.02358000 (ETH price for 1 BTC) = 0.00026984 BTC (volume)

4. Calculate profit

   > 0.00027 BTC (inbound volume) - 0.00026984 BTC (outbound volume) = 0,0000016 (volume) - profit is negative

### Calculate profit with sum orders volumes

Summation of volumes is used in the following cases:

* When the volume of the first order in `bids/asks` is less than the minimum volume required to complete the transaction
* When the volume of the first order in `bids/acks` is greater than the minimum volume required to execute the
  transaction, but less than the maximum limit for one transaction
    * it is necessary to take into account situations when without summing up the volume there is a profit, but with
      summing up there may not be

## Send orders

[New Order API](https://developers.binance.com/docs/binance-spot-api-docs/rest-api/trading-endpoints#new-order-trade)