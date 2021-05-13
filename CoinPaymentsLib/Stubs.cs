using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace zcash.CoinPaymentsLib
{
    public class ZcashSaplingPublicKeyPackage : PublicKeyPackage
    {
        public string Address { get; private set; }

        public ZcashSaplingPublicKeyPackage(string address)
        {
            this.Address = address;
        }
    }

    public class ZcashOnlineServiceStub : IOnlineCoinService
    {
        public string BroadcastSignedTx(string signedTx)
        {
            throw new NotImplementedException();
        }

        public decimal EstimateFee(ConfirmationSpeed speed)
        {
            throw new NotImplementedException();
        }

        public decimal GetAddressBalance(string address, uint minConfirmations)
        {
            throw new NotImplementedException();
        }

        public void ImportPublicKeyPackage(PublicKeyPackage pubkey)
        {
            throw new NotImplementedException();
        }

        public string PrepareUnsignedTx(string addressFrom, string addressTo, decimal amount, decimal fee)
        {
            throw new NotImplementedException();
        }

        public void RegisterAccountUpdateListener(IOnlineCoinService.AccountUpdateDelegate listener)
        {
            AccountUpdateListeners += listener;
        }

        public void ScanTransaction(byte[] rawTx)
        {
            throw new NotImplementedException();
        }

        public bool ValidateAddress(string address)
        {
            throw new NotImplementedException();
        }

        public void Start()
        {
            throw new NotImplementedException();
        }

        public void Stop()
        {
            throw new NotImplementedException();
        }

        private IOnlineCoinService.AccountUpdateDelegate AccountUpdateListeners;
    }

    public class ZcashOfflineServiceStub : IOfflinelineCoinService
    {
        public PublicKeyPackage generateAddress()
        {
            throw new NotImplementedException();
        }

        public string SignTx(string unsignedTx)
        {
            throw new NotImplementedException();
        }
    }
}
