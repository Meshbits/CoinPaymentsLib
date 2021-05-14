# Coin Payments Integration Library for Zcash

> Working Draft version 0.2

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
    public delegate void AccountUpdate(string address, bool reorg);
    public event AccountUpdate AccountUpdated;

    bool ValidateAddress(string address);
    decimal GetAddressBalance(string address, uint minConfirmations);
    string PrepareUnsignedTx(string addressFrom, string addressTo, decimal amount, decimal fee);
    string BroadcastSignedTx(string signedTx);
    decimal EstimateFee(ConfirmationSpeed speed);

    string ImportPublicKeyPackage(string pubkey);

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
it will notify the listeners registered on the `AccountUpdated` event.

Every affected address will be notified. The order is undefined. A
listener can query the new balance by calling `GetAddressBalance`.

Note: Listeners are called if the 0-confirmation balance changes.

## Getting Account Balance

Use:

```cs
decimal GetAddressBalance(string address, uint minConfirmations);
```

If `minConfirmations` is 0, it returns the balance including unconfirmed
transactions.

## Validating an address

```cs
bool ValidateAddress(string address, decimal amount);
```

`ValidateAddress` checks whether an address is well-formed and known to the ZAMS. If the address was not previously imported, `ValidateAddress` will
return false.

> Note: Do we need a more specific error result?

## Fee Estimation

```cs
decimal EstimateFee(ConfirmationSpeed speed);
```

`EstimateFee` returns an estimation of the transaction fees for a given desired confirmation speed.

Note: The confirmation speed is not guaranteed and is sorely indicative.

## Preparing a spending transaction

A spending transaction from any tracked account starts at the online ZAMS.
Some of the data needed to build a transaction depends on the 
current state of the blockchain. It is very difficult to completely create
a zcash transaction offline.

```cs
string PrepareUnsignedTx(string addressFrom, string addressTo, decimal amount, decimal fee);
```

`PrepareUnsignedTx` has the same function signature as a normal transfer but
returns an incomplete, unsigned transaction as a opaque JSON string.

The result has to be made available to the Offline ZAMS for signing 
(See next section on Offline ZAMS).

The result, complete, signed transaction is then ready for broadcasting
to the network via the Online ZAMS.

Use 
```cs
string BroadcastSignedTx(string signedTx);
```

where `signedTx` is the value obtained by signing and the return value
of `BroadcastSignedTx` is the transaction ID.

# Interface of the Offline ZAMS

```cs
  public interface IOfflinelineCoinService
  {
    KeyPackage generateAddress();

    string SignTx(string unsignedTx, string privateKey);

    void Start();
    void Stop();
  }
```

## Address Generation

```cs
KeyPackage generateAddress(string addressType);
```

`generateAddress` returns a KP. The secret key
should be stored by CP and the public key 
needs to be copied to the Online ZAMS
and imported. See [Account Monitoring](#account-monitoring).

Zcash supports several types of addresses. At this moment, they
can be "transparent" (type T) or "shielded" (type Z). In the future,
Zcash will have unified addresses (type U).

## Signing a transaction

```cs
string SignTx(string unsignedTx, string privateKey);
```

Signs a unsigned transaction created on the Online ZAMS and returns
the raw transaction for broadcasting.

See [Preparing a spending transaction](#preparing-a-spending-transaction).

# Revisions

- 0.1: Initial version
- 0.2: Make Offline ZAMS stateless (does not store secret keys)
