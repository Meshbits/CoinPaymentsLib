using System;
using System.Threading.Tasks;

namespace zcash.CoinPaymentsLib
{
  public enum ConfirmationSpeed
  {
    Slow, 
    Normal, 
    Fast
  }

  public interface KeyPackage
  {
    string PublicKey { get; }
    string PrivateKey { get; }
  }

  public enum FeeType {
    Fixed,
    PerKb,
  }

  public struct Fee {
    FeeType type;
    decimal amount;
  }

  public struct TxIn {
    string txHash;
    decimal amount;
    UInt32 voutIndex;
    string address;
  }

  public struct TxOut {
    decimal amount;
    string address;
    string memoHex;
  }

  public struct TxInfo {
    string hash;
    UInt32 height;
    TxIn[] inputs;
    TxOut[] outputs;
    decimal fee;
  }


  public interface IOnlineCoinService : IDisposable
  {
    Task<bool> ValidateAddress(string address, decimal amount, bool tracked);
    Task<decimal> GetAddressBalance(string address, uint minConfirmations);
    Task<string> PrepareUnsignedTx(string addressFrom, string addressTo, decimal amount, decimal fee);
    Task<string> BroadcastSignedTx(string signedTx);
    Task<Fee> EstimateFee(ConfirmationSpeed speed);

    UInt32 GetCurrentHeight();
    Task<TxInfo> GetTxInfo(string txHash);

    Task<UInt32> Rescan(UInt32 height);

    void ImportPublicKeyPackage(string pubkey);

    void Start();
    void Stop();
  }

  public interface IOfflinelineCoinService : IDisposable
  {
    KeyPackage generateAddress(string addressType);

    Task<string> SignTx(string unsignedTx, string privateKey);

    void Start();
    void Stop();
  }
}
