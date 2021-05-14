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
    public event IOnlineCoinService.AccountUpdate AccountUpdated;

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

        public void ScanTransaction(byte[] rawTx)
        {
            throw new NotImplementedException();
        }

        public bool ValidateAddress(string address, decimal amount)
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

    public void Dispose()
    {
      throw new NotImplementedException();
    }
  }

    public class ZcashOfflineServiceStub : IOfflinelineCoinService
    {
    public void Dispose()
    {
      throw new NotImplementedException();
    }

    public PublicKeyPackage generateAddress(string addressType)
    {
      throw new NotImplementedException();
    }

    public string SignTx(string unsignedTx)
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
  }
}
