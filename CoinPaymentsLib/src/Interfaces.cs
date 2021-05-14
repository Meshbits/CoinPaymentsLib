using System;

namespace zcash.CoinPaymentsLib
{
  public enum ConfirmationSpeed
  {
    Slow, 
    Normal, 
    Fast
  }

  public interface PublicKeyPackage
  {
    string Address { get; }
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

    void ImportPublicKeyPackage(PublicKeyPackage pubkey);

    void Start();
    void Stop();
  }

  public interface IOfflinelineCoinService : IDisposable
  {
    PublicKeyPackage generateAddress(string addressType);

    string SignTx(string unsignedTx);

    void Start();
    void Stop();
  }
}
