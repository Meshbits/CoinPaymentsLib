---
title: Acceptance Test
date: "2021-06-16T12:53:00+08:00"
---

{{ page.date | date_to_long_string }}

# Acceptance criteria and demo

From Coin Payements Integration [documentation](https://coinpaymentsnet.github.io/CoinIntegration/):

> After all the operations are implemented following the provided requirements they should all be combined in a demo showing the following user story

- Addresses A and B are generated
- Address A receives an external deposit from a 3rd party wallet (transaction is parsed and A is detected as a receiver)
- Using the private key for A the transaction is built sending the received coins to the address B.
- The built transaction is sent to the network
- Address B receives an external deposit from address A (transaction is parsed and A is detected as a sender while B is detected as a receiver)

This document describes how to perform the acceptance test with &#x24E9;

# Setup

- Run `zcashd` on testnet
- Check that `zams.ini` is configured for testnet
- Create database `zamsdb` and run setup script `up.sql`
- Load a checkpoint: `cli load-checkpoint <height>`
- Sync: `cli scan`
- Run `zams` & `signer`
- Run mock notification listener

# Tests

## Account Generation

```
dotnet test --filter DisplayName~KeyGeneration
dotnet test --filter DisplayName~AccountTests
```

Optional: Check `fvks` and `accounts` table in database

## Receive payment on account A

Use a testnet faucet to send some funds to the first address

Verify that the notification listener receives a message when the transaction is confirmed.

```js
[
  {
    id: 1,
    eventType: 'incomingTx',
    txHash: 'ad7268bb01dc4283013d622d0c4f0507336d64c7992bcbcce5beddfed978a9eb',
    account: 1,
    address: 'ztestsapling1m8te40smdz78tfgm03737nfkq27ysmf8lreccy4u43mhavh4p9jy47lwaqn03zr8da6c2g4nert',
    txOutputIndex: 0,
    amount: 100000000,
    block: 1450082
  }
]
```

## Prepare a payment from A to B

**Wait for 10 confirmations**

Optional: Edit `BalanceTest` to check for account 1 with a minimal of 10 confirmations

```cs
var balance = await explorer.GetAccountBalance(1, 10);
```

When ready, run the `PrepareTxTest` and copy the output into `tx.json`

```
dotnet test --filter DisplayName~PrepareTxTest
```

The output should be enclosed in {}, i.e. do not copy the leading >>

Optional: Review the JSON. It has a payment ID, the list of inputs
and the recipient, but no signature.

## Sign and Broadcast

Run the `SignTest`, it will pass the unsigned tx to the `signer`
and broadcast the result via `zams`.

```
dotnet test --filter DisplayName~SignTest
```

This will give the transaction ID. 

Optional: Review the transaction on a block explorer and verify that it 
has shielded inputs/outputs.

Once confirmed, check that the mock notification listener received:

- one outgoing tx: we spent one output
- one incoming tx to account A: that's the change
- one incoming tx to account B: that's the amount paid out


For example,

```js
[
  {
    id: 2,
    eventType: 'outgoingTx',
    txHash: 'cb4620db7187a7995073f4d24084e42f6f4d99f25b50083250d229127029c34e',
    account: 1,
    address: 'ztestsapling1m8te40smdz78tfgm03737nfkq27ysmf8lreccy4u43mhavh4p9jy47lwaqn03zr8da6c2g4nert',
    txOutputIndex: 0,
    amount: 100000000,
    block: 1450103
  },
  {
    id: 3,
    eventType: 'incomingTx',
    txHash: 'cb4620db7187a7995073f4d24084e42f6f4d99f25b50083250d229127029c34e',
    account: 2,
    address: 'ztestsapling1zhu3ppsnhhjt0p262cynnshdduucrq4eu73fp65mwyvhn0nr2phvh9n0alym9huzzvrxjvuaqgd',
    txOutputIndex: 0,
    amount: 1000000,
    block: 1450103
  },
  {
    id: 4,
    eventType: 'incomingTx',
    txHash: 'cb4620db7187a7995073f4d24084e42f6f4d99f25b50083250d229127029c34e',
    account: 1,
    address: 'ztestsapling1m8te40smdz78tfgm03737nfkq27ysmf8lreccy4u43mhavh4p9jy47lwaqn03zr8da6c2g4nert',
    txOutputIndex: 1,
    amount: 98999000,
    block: 1450103
  }
]
```
