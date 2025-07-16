# Binance

[Binance API documentation](https://developers.binance.com/docs/binance-spot-api-docs)

## Content

1. [Получение списка доступных тикеров](#получение-списка-доступных-тикеров)

2. [(Опционально) Фильтрация тикеров по объемам за последние 24ч](#опционально-фильтрация-тикеров-по-объемам-за-последние-24ч)

3. [Формирование цепочек тикеров](#формирование-цепочек-тикеров)

4. [Получение ордеров по тикерам в стакане и расчет возможного профита по алгоритму](#получение-ордеров-по-тикерам-в-стакане-и-расчет-возможного-профита-по-алгоритму)

    1. [Расчет профита без учета комиссий](#расчет-профита-без-учета-комиссий)
    2. [Расчет профита с учетом суммирования объемов](#расчет-профита-с-учетом-суммирования-объемов)

5. [Выставление ордеров](#выставить-ордера)

## Получение списка доступных тикеров

[Exchange information API](https://developers.binance.com/docs/binance-spot-api-docs/rest-api/general-endpoints#exchange-information)

```shell
curl -X GET "https://api.binance.com/api/v3/exchangeInfo?symbolStatus=TRADING&showPermissionSets=false&permissions=\[\"SPOT\"\]" | jq | > symbols.json
```

## (Опционально) Фильтрация тикеров по объемам за последние 24ч

[24hr ticker price change statistics API](https://developers.binance.com/docs/binance-spot-api-docs/rest-api/market-data-endpoints#24hr-ticker-price-change-statistics)

```shell
curl -X GET https://api.binance.com/api/v3/ticker/24hr?type=MINI | jq | > volumes.json
```

## Формирование цепочек тикеров

Цепочки тикеры формируются на основе базовых активов.

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

### Алгоритм формирования цепочек

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

## Получение ордеров по тикерам в стакане и расчет возможного профита по алгоритму

[Order book API](https://developers.binance.com/docs/binance-spot-api-docs/rest-api/market-data-endpoints#order-book)

```shell
curl -X GET https://api.binance.com/api/v3/depth?symbol=ETHBTC&limit=100 | jq | > orders.json
```

**Пример:**

> BTC:USDT (**ASC**) | ETH:USDT (**DESC**) | ETH:BTC (**ASC**)

### Расчет профита (без учета комиссий):

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

1. **BTC:USDT [ASC] => [BID]** - продать BTC за USDT (**SELL ORDER**).

   > 0.00027 BTC * 109615.46000000 (USDT price for 1 BTC) = 29.59 USDT (volume)

2. **ETH:USDT [DESC] => [ASC]** - купить ETH за USDT (**BUY ORDER**).

   > 29.59 USDT / 2585.71000000 (USDT price for 1 ETH) = 0.01144371 ETH (volume)

3. **ETH:BTC [ASC] => [BID]** - купить BTC за ETH (**SELL ORDER**).

   > 0.01144371 ETH * 0.02358000 (ETH price for 1 BTC) = 0.00026984 BTC (volume)

4. Расчет профита

   > 0.00027 BTC (inbound volume) - 0.00026984 BTC (outbound volume) = 0,0000016 (volume) - профит отрицательный

### Расчет профита с учетом суммирования объемов

Суммирование объемов применяется в следующих случаях:

* когда объем первого ордера в `bids/acks` меньше минимально необходимого объема для осуществления сделки
* когда объем первого ордера в `bids/acks` больше минимально необходимого объема для осуществления сделки, но
  меньше максимального лимита на одну сделку
    * требуется учитывать ситуации когда без суммирования объема есть профит, а с суммированием может не быть

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
      "0.00015"
    ],
    [
      "109614.96000000",
      "0.00005"
    ],
    [
      "109614.96000000",
      "0.00030"
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

```text
// ВОПРОСЫ:

1. Если брать 1й кейс где везде объемов хватает, то как будут расчитываться оставшиеся циклы ??? - см. 4 кейс
2. Если не хватает объема во 2й паре 1й глубины и у 1й расчитанной пары порядок DESC, то как будут пересчитываться объемы для 1й пары ???
3. После пересчета объемов на меньшие как поведет себя суммирование объемов??? - по идее будет просто до лимита идти 

// DRAFT:

BTC:USDT | ETH:USD (DESC) | ETH:BTC

limit: 0.0003 BTC

// Кейсы:

// 1. Везде хватает объемов, пересчет объемов не требуется.

BTC:USDT

price: 109615.46 USDT
volume: 7.27795000 BTC

ETH:USDT

price: 2585.71 USDT
volume: 19.28810000 ETH

ETH:BTC

price: 0.02358 BTC
volume: 105.74550000 ETH

Расчет:

1. 0.0003 BTC (volume) * 109615.46 USDT (price) = 32.88 USDT (volume) 
2. 32.88 USDT (volume) / 2585.71 USDT (price) = 0.012716 ETH (volume)
3. 0.012716 ETH (volume) * 0.02358 BTC (price) = 0.0002998 BTC (volume)

compare: 0.0002998 BTC (volume) - 0.0003 BTC (volume) = -0.0000002 BTC (profit)

// 2. Не хватает объема до лимита в 1й паре, перерасчет объемов не требуется (просуммируются объемы 1й пары)

BTC:USDT

price: 109615.46 USDT
volume: 0.0002 BTC

price: 109616.46 USDT
volume: 1.2 BTC

ETH:USDT

price: 2585.71 USDT
volume: 19.28810000 ETH

price: 2586.71 USDT
volume: 10.2 ETH

ETH:BTC

price: 0.02358 BTC
volume: 105.74550000 ETH

price: 0.02359 BTC
volume: 205.7 ETH

Расчет:

# cycle #1

1. 0.0002 BTC (volume) * 109615.46 USDT (price) = 21.92 USDT (volume) 
2. 21.92 USDT (volume) / 2585.71 USDT (price) = 0.00847736 ETH (volume)
3. 0.00847736 ETH (volume) * 0.02358 BTC (price) = 0.0001998 BTC (volume)

# cycle #2

1. 0.0003 BTC (volume) * 109616.46 (last price) = 32.88 (volume)
2. 32.88 USDT (volume) / 2585.71 USDT (price) = 0.012716 ETH (volume)
3. 0.012716 ETH (volume) * 0.02358 BTC (price) = 0.0002998 BTC (volume)

// 3. Не хватает объема до лимита во 2й паре - перерасчет объем 1й пары, на втором цикле объемы проссумируются

BTC:USDT

price: 109615.46 USDT
volume: 7.27795000 BTC

ETH:USDT

price: 2585.71 USDT
volume: 0.01 ETH

price: 2586.71 USDT
volume: 20.2 ETH

ETH:BTC

price: 0.02358 BTC
volume: 105.74550000 ETH

Расчет:

cycle #1

1. 0.0003 BTC (volume) * 109615.46 USDT (price) = 32.88 USDT (volume)
2. 32.88 USDT (volume) / 2585.71 USDT (price) = 0.012716 ETH (volume) - доступно только 0.01 ETH
2.1 Пересчитываем объемы 1й пары с учетом доступного объема во 2й паре.
0.01 ETH (доступный volume) * 2585.71 USDT (price) = 25.82 USDT (новый quote volume для 1й пары)
25.82 USDT (новый quote volume для 1й пары) / 109615.46 USDT (price) = 0.00023555 BTC (новый base volume для 1й пары)
3. 0.01 ETH (volume) * 0.02358 BTC (price) = 0.0002358 BTC (volume)

cycle #2

1. 0.0003 BTC (volume) * 109615.46 USDT (price) = 32.88 USDT (volume)
2. 32.88 USDT (volume) / 2586.71 USDT (price) = 0.012711 ETH (volume) - оставляем объем если суммы текущего объема ETH и предыдущего объема ETH хватает (считаем по новой цене)
3. 0.012711 ETH (volume) * 0.02358 BTC (price) = 0.0002997 BTC (volume)

// 4. [TODO] Везде хватает объемов, пересчет объемов не требуется (несколько циклов). - по идее объемы не должны будут суммироваться

BTC:USDT

price: 109615.46 USDT
volume: 7.27795000 BTC

ETH:USDT

price: 2585.71 USDT
volume: 19.28810000 ETH

ETH:BTC

price: 0.02358 BTC
volume: 105.74550000 ETH

Расчет:

1. 0.0003 BTC (volume) * 109615.46 USDT (price) = 32.88 USDT (volume) 
2. 32.88 USDT (volume) / 2585.71 USDT (price) = 0.012716 ETH (volume)
3. 0.012716 ETH (volume) * 0.02358 BTC (price) = 0.0002998 BTC (volume)

compare: 0.0002998 BTC (volume) - 0.0003 BTC (volume) = -0.0000002 BTC (profit)
```

### FAQ

1. При расчетах не учитываются комиссии, потому что комиссия по стоимости равна пыли
   и это проще учитывать в погрешности по профиту (при сделке на ~30\$ размер комиссии составляет ~0,0022$).
2. Ограничения на минимально и максимально допустимый объем средств на 1 сделку задается в конфигурационном
   файле.
3. Размер минимально допустимого профита по алгоритму задается в конфигурационном файле.

## Выставить ордера

[New Order API](https://developers.binance.com/docs/binance-spot-api-docs/rest-api/trading-endpoints#new-order-trade)

## Головные мюсли

1. Попробовать немного подкручивать цену вверх перед применением алгоритма (актуально только для лимитных ордеров) ???
2. Рассчитывать надо по новой цене если происходит суммирование обьемов и выставлять ордера соответственно , потому что
   если есть профит по новой цене , со старой ценой будет больше профита просто