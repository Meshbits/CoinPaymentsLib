# Coin Payments Integration Library for Zcash

> Working Draft version 0.4

The CoinPaymentsLib (CPLib) .NET library provides the bridge between
the Coin Payments (CP) server code and the Zcash account
management service (ZAMS).

The ZAMS runs as a server locally with a CP server or in the same
private network.

We expect two types of ZAMS deployements: Online ZAMS and Offline ZAMS.

ZAMS are services that we will provide binaries and source code for.
The CPLib encapsulates the communication with these services in C#.

## Online ZAMS 

The Online ZAMS communicates with `zcashd` (zcash full node client) in block explorer mode. 

- It tracks account balances and incoming payments. Therefore, this ZAMS is watch-only. 
- It can also build *unsigned* spending transactions but *does not* have
the ability to sign them. The Online ZAMS requires connectivity to a `zcashd` instance that has Internet access. 
- It can broadcast a fully signed transaction
- It can estimate the current network fees

## Offline ZAMS

The Offline ZAMS runs without Internet access (or even from an air-gapped
computer). It holds all the account signing keys and therefore can
sign transactions. 

## Communication between with CPLib and ZAMS

We expect the communication to be based on GRPC. The implementation
is encapsulated by the CPLib and should not be visible
from users of CPLib.

# Interface for Online ZAMS

```cs
  public interface IOnlineCoinService : IDisposable
  {
    Task<bool> ValidateAddress(string address, ulong amount, bool tracked);
    Task<ulong> GetAddressBalance(string address, uint minConfirmations);
    Task<string> PrepareUnsignedTx(string addressFrom, string addressTo, ulong amount, ulong fee);
    Task<string> BroadcastSignedTx(string signedTx);
    Task<Fee> EstimateFee(ConfirmationSpeed speed);

    uint GetCurrentHeight();
    Task<TxInfo> GetTxInfo();

    Task<uint> Rescan(uint height);

    void ImportPublicKeyPackage(string pubkey);

    void Start();
    void Stop();
  }
```

## Start/Stop

Once constructed, use `Start` to start the service. Before tear-down, call
`Stop` or `Dispose()`

## Account Monitoring

Account Creation must be done from the Offline ZAMS. The result is a 
Key package (KP) that contains both private key and public key.

Note: For shielded addresses, the KP also contains a viewing key.
This key does not give the ability to spend. The exact contents of
a KP is implementation dependent but it will always have
an address.

The public key of a KP must be imported to the online ZAMS.

```cs
string ImportPublicKeyPackage(string pubkey);
```

This function returns the address.

Once imported, the online ZAMS will start tracking incoming payments.

Note: **Past** Incoming payments made to an address may not be visible.

> Is this an issue? If so, we can expose a transaction rescan method.
But it will be SLOW.

## Incoming payments

If the ZAMS detects a change in the balance of any of the tracked accounts,
it will make a REST POST call to a URL specified by a command line option.

Example:

```
{
"eventType"   : "incomingTx",
"txHash" : "5f322ed7628b9603f128f07856e97aaaeaaf58dda569753265f3af0af9a81311",
"toAddress" : "t1bKe2mUcRSiY4XJdREzyD1WXLHADfncUrq",
"txOutputIndex" : 0,
"block" : 1252523
}
```

For payments made from CP, `eventType` should be `outgoingTx` when
the spending transaction is confirmed.

> If the target endpoint does not return 200 OK, the ZAMS must
retry.

## Getting Account Balance

Use:

```cs
Task<ulong> GetAddressBalance(string address, uint minConfirmations);
```

If `minConfirmations` is 0, it returns the balance including unconfirmed
transactions.

Amounts are given in zatoshis. 100 million (10^8) zatoshis = 1 ZEC

## Validating an address

```cs
bool ValidateAddress(string address, ulong amount, bool tracked);
```

`ValidateAddress` checks whether an address is well-formed and known to the ZAMS if `tracked` is true.

> Note: Do we need a more specific error result?

## Fee Estimation

```cs
Task<Fee> EstimateFee(ConfirmationSpeed speed);
```

`EstimateFee` returns an estimation of the transaction fees for a given desired confirmation speed.

Note: The confirmation speed is not guaranteed and is sorely indicative.

Shielded addresses have a fixed fee independent from the transaction size.

## Blockchain Info
```cs
uint GetCurrentHeight();
Task<uint> Rescan(uint height);
```

`GetCurrentHeight` returns the height of the latest block scanned.
`Rescan` triggers a rescan of the blockchain from a particular height
and returns the latest block scanned.

> `Rescan` is an expensive operation. While it runs, other operations
will be suspended.

## Transaction Info

```cs
public struct TxIn {
  string txHash;
  ulong amount;
  uint voutIndex;
  string address;
}

public struct TxOut {
  ulong amount;
  string address;
  string memoHex;
}

public struct TxInfo {
  string hash;
  uint height;
  TxIn[] inputs;
  TxOut[] outputs;
  ulong fee;
}

Task<TxInfo> GetTxInfo(string txHash);
```

`GetTxInfo` retrieves transaction information for a past transaction.
Shielded transactions will only have the data that can be decrypted.
Other entries will be null.

## Preparing a spending transaction

A spending transaction from any tracked account starts at the online ZAMS.
Some of the data needed to build a transaction depends on the 
current state of the blockchain. It is very difficult to completely create
a zcash transaction offline.

```cs
Task<string> PrepareUnsignedTx(string addressFrom, string addressTo, ulong amount, ulong fee);
```

`PrepareUnsignedTx` has the same function signature as a normal transfer but
returns an incomplete, unsigned transaction as a opaque JSON string.

The result has to be made available to the Offline ZAMS for signing 
(See next section on Offline ZAMS).

The result, complete, signed transaction is then ready for broadcasting
to the network via the Online ZAMS.

Use 
```cs
Task<string> BroadcastSignedTx(string signedTx);
```

where `signedTx` is the value obtained by signing and the return value
of `BroadcastSignedTx` is the transaction ID.

# Interface of the Offline ZAMS

```cs
  public interface IOfflinelineCoinService
  {
    KeyPackage generateAddress();

    Task<string> SignTx(string unsignedTx, string privateKey);

    void Start();
    void Stop();
  }
```

## Address Generation

```cs
KeyPackage generateAddress(string addressType);
string generateDiversifiedAddress(string pkey);
```

`generateAddress` returns a KP. The secret key
should be stored by CP and the public key 
needs to be copied to the Online ZAMS
and imported. See [Account Monitoring](#account-monitoring).

Zcash supports several types of addresses. At this moment, they
can be "transparent" (type T) or "shielded" (type Z). In the future,
Zcash will have unified addresses (type U).

### Transparent Addresses

Transparent addresses behave much like Bitcoin addresses. They are associated with a secret key and a public key.

### Shielded (Sapling) Addresses

Shielded addresses have several key components. The public key (full viewing key) allows the owner
to recognize his incoming payments by trial decryption. However this process takes ~1 ms due to the compute cost of
elliptical curve cryptography. If a large number of keys are used, the total decryption time becomes prohibitive.
However, shielded addresses can share the same decryption key while being different and unlinkable.
With this method, we would only need to trial decrypt once with the shared incoming viewing key and 
be able to recognize a payment made to any of the generated addresses. If the decryption is successful, we
can further identify to which address the payment was made.

To use this feature, call `generateDiversifiedAddress` with the public key from the Key Package 
returned by `generateAddress`.
Each call will return a different address which has the same underlying public key. 

## Signing a transaction

```cs
Task<string> SignTx(string unsignedTx, string privateKey);
```

Signs a unsigned transaction created on the Online ZAMS and returns
the raw transaction for broadcasting.

See [Preparing a spending transaction](#preparing-a-spending-transaction).

# Revisions

- 0.1: Initial version
- 0.2: Make Offline ZAMS stateless (does not store secret keys)
- 0.3: Removed account notification event, added REST webhook,
added GetTx, GetHeight, Rescan methods, async versions,
tweaked GetFee
- 0.4: Use diversified addresses, change amount to zatoshis
