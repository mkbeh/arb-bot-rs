# Arbitrage algorithm

Arbitrage is a trading strategy in which an asset is bought in one market and simultaneously sold in another market. The
goal is to profit from small differences in price across the two different markets.

Triangular arbitrage is a type of foreign exchange (forex) trading that involves exchanging one currency for a second,
then trading it for a third, and then finally exchanging it back into the original currency. The goal of this trading
pattern is to profit from discrepancies among three foreign currencies when their exchange rates across markets don't
match up.

A trader using triangular arbitrage, for example, could make a series of exchanges—U. S. dollar (USD) to euros (EUR) to
the British pound (GBP) to USD using the EUR/USD, EUR/GBP, and USD/GBP rates. If the transaction costs are low, the
trader could net a profit from this exchange.

## Mechanics and Execution

The mechanics and execution of triangular arbitrage hinge on the swift identification and exploitation of inefficiencies
in currency exchange rates.

### Identifying Arbitrage Opportunities

The process begins by identifying potential arbitrage opportunities where the actual cross-exchange rates in the market
do not align with the expected cross rates. For instance, an arbitrage opportunity is present if the product of the
exchange rates from USD to EUR and EUR to GBP does not equal the direct exchange rate from USD to GBP.

### Executing the Trades

* **First Trade**: The trader exchanges the initial currency for a second currency at the prevailing market rate. This
  step is critical as it sets the foundation for the arbitrage cycle.

* **Second Trade**: The second currency is then traded for a third currency, again using the current exchange rate. This
  step exploits the first identified discrepancy.

* **Final Trade**: Finally, the third currency is exchanged back into the initial currency. The rate at which this
  exchange occurs is vital as it determines whether the overall transaction results in a profit.

### Detailed Example of Triangular Arbitrage

Consider the following real-world scenario to illustrate the execution of triangular arbitrage:

* **Exchange Rates**:

    * USD/EUR = 0.85

    * EUR/GBP = 0.70

    * GBP/USD = 1.50

* **Implied Rate Calculation**: An arbitrageur calculates the implied USD/GBP rate by multiplying the USD/EUR and
  EUR/GBP
  rates (0.85 * 0.70 = 0.595). However, the market’s direct exchange rate for GBP/USD, which implies a USD/GBP rate of
  0.6667 (1/1.50), presents a discrepancy from the calculated rate.

### Arbitrage Execution Steps

* **Step 1**: The arbitrageur starts with $100,000 USD and converts it to Euros at the USD/EUR rate of 0.85, receiving
  85,000 Euros.

* **Step 2**: These 85,000 Euros are then converted to British Pounds at the EUR/GBP rate of 0.70, yielding 59,500 GBP.

* **Step 3**: Finally, the 59,500 GBP are converted back into USD at the GBP/USD rate of 1.50, resulting in $119,000
  USD.

### Outcome

The arbitrage cycle starts with $100,000 and ends with $119,000, thus securing a profit of $19,000 from the arbitrage
transactions. This example highlights the profit potential when discrepancies between the implied and actual exchange
rates are efficiently and quickly exploited.

### Exchanges

* [Binance](#binance)

## Binance

### 🔗 Building Ticker Chains

Ticker chains are constructed based on base assets to identify triangular arbitrage opportunities.

**Example Base Assets:**

* **Major assets:** BTC, ETH, BNB, USDT, USDC, FDUSD

* **Fiat currencies:** EUR, TRY, BRL, JPY

* **Other cryptocurrencies:** LTC, ADA, DOT, XRP, and other major pairs

#### Chain Construction Algorithm

**Symbol Pattern:** BTC/USDT → USDT/ETH → ETH/BTC

 **Examples:**

**Case #1 - Standard Chain**

```text
BTC/USDT → USDT/ETH → ETH/BTC
```

**Case #2 - First Symbol Reversed**

```text
USDT/BTC → BTC/ETH → ETH/USDT
```

**Case #3 - Second Symbol Reversed**

```text
BTC/USDT → USDT/ETH → ETH/BTC
```

**Case #4 - Third Symbol Reversed**

```text
**ETH/BTC → BTC/USDT → USDT/ETH**
```

**Case #5 - First & Second Symbols Reversed**

```text
USDT/BTC → BTC/BNB → BNB/USDT
```

**Case #6 - Second & Third Symbols Reversed**

```text
ETH/BTC → BTC/QTUM → QTUM/ETH
```

**Case #7 - All Symbols Reversed**

```text
BTC/ETH → ETH/RLC → RLC/BTC
```

#### Validation Rules

1. **Non-matching Constraint:** A reversed symbol should not create a circular reference
    * ❌ **Invalid:** ETH/BTC → BTC/ETH → ...
    * ✅ **Valid:** ETH/BTC → BTC/QTUM → ...
2. **Base Asset Consistency:** The exit from the third symbol must return to the base asset of the first symbol
    * ❌ **Invalid:** ETH/BTC → BTC/QTUM → QTUM/USDT
    * ✅ **Valid:** ETH/BTC → BTC/QTUM → QTUM/ETH

### 📊 Profit Calculation Algorithm

**Example Calculation Scenario:**

**Trading Pair Chain:** BTC/USDT (ASC) → ETH/USDT (DESC) → ETH/BTC (ASC)

**Volume Limit:** 0.00027 BTC (~$30)

**Fee Structure:** Maker/Taker 0.075% (with BNB discount)

#### Order Book Data Snippets

```json
// BTC/USDT Order Book
{
  "lastUpdateId": 72224518924,
  "bids": [
    [
      "109615.46",
      "7.27795"
    ],
    [
      "109614.96",
      "0.00046"
    ]
  ],
  "asks": [
    [
      "109615.47",
      "2.22969"
    ],
    [
      "109615.48",
      "0.00028"
    ]
  ]
}
```

```json
// ETH/USDT Order Book
{
  "lastUpdateId": 54622041690,
  "bids": [
    [
      "2585.70",
      "14.64600"
    ],
    [
      "2585.69",
      "0.00210"
    ]
  ],
  "asks": [
    [
      "2585.71",
      "19.28810"
    ],
    [
      "2585.72",
      "0.40280"
    ]
  ]
}
```

```json
// ETH/BTC Order Book
{
  "lastUpdateId": 8215337504,
  "bids": [
    [
      "0.02358",
      "105.74550"
    ],
    [
      "0.02357",
      "57.30640"
    ]
  ],
  "asks": [
    [
      "0.02359",
      "25.63400"
    ],
    [
      "0.02360",
      "53.22680"
    ]
  ]
}
```

#### Step-by-Step Calculation

[TODO]

