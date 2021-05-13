namespace zcash.CoinPaymentsLib
{
  public enum ConfirmationSpeed
    {
        Slow, Normal, Fast
    }

    public interface PublicKeyPackage
    {
        string Address { get; }
    }

    public interface IOnlineCoinService
    {
        public delegate void AccountUpdateDelegate(string address, bool reorg);

        bool ValidateAddress(string address);
        decimal GetAddressBalance(string address, uint minConfirmations);
        string PrepareUnsignedTx(string addressFrom, string addressTo, decimal amount, decimal fee);
        string BroadcastSignedTx(string signedTx);
        decimal EstimateFee(ConfirmationSpeed speed);

        public void RegisterAccountUpdateListener(AccountUpdateDelegate listener);
        void ImportPublicKeyPackage(PublicKeyPackage pubkey);

        void Start();
        void Stop();
    }

    public interface IOfflinelineCoinService
    {
        PublicKeyPackage generateAddress();

        string SignTx(string unsignedTx);
    }
}
