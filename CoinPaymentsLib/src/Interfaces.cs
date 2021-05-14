using System;

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

  public interface IOnlineCoinService : IDisposable
  {
    public delegate void AccountUpdate(string address);
    public event AccountUpdate AccountUpdated;

    bool ValidateAddress(string address, decimal amount);
    decimal GetAddressBalance(string address, uint minConfirmations);
    string PrepareUnsignedTx(string addressFrom, string addressTo, decimal amount, decimal fee);
    string BroadcastSignedTx(string signedTx);
    decimal EstimateFee(ConfirmationSpeed speed);

    void ImportPublicKeyPackage(string pubkey);

    void Start();
    void Stop();
  }

  public interface IOfflinelineCoinService : IDisposable
  {
    KeyPackage generateAddress(string addressType);

    string SignTx(string unsignedTx, string privateKey);

    void Start();
    void Stop();
  }
}
