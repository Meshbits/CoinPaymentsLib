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
    ulong amount;
  }

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


  public interface IOnlineCoinService : IDisposable
  {
    Task<bool> ValidateAddress(string address, ulong amount, bool tracked);
    Task<ulong> GetAddressBalance(string address, uint minConfirmations);
    Task<string> PrepareUnsignedTx(string addressFrom, string addressTo, ulong amount, ulong fee);
    Task<string> BroadcastSignedTx(string signedTx);
    Task<Fee> EstimateFee(ConfirmationSpeed speed);

    uint GetCurrentHeight();
    Task<TxInfo> GetTxInfo(string txHash);

    Task<uint> Rescan(uint height);

    void ImportPublicKeyPackage(string pubkey);

    void Start();
    void Stop();
  }

  public interface IOfflinelineCoinService : IDisposable
  {
    KeyPackage generateAddress(string addressType);
    string generateDiversifiedAddress(string pkey);

    Task<string> SignTx(string unsignedTx, string privateKey);

    void Start();
    void Stop();
  }
}
