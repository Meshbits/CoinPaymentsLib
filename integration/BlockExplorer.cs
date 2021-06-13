using System.Threading.Tasks;
using Grpc.Core;
using Zams;

public enum ConfirmationSpeed {
  Slow, Medium, Fast
}

public interface IBlockExplorer {
    Task<bool> ValidateAddress(string address, ulong amount, bool tracked);
    Task<Balance> GetAccountBalance(int accountId, uint minConfirmations);
    Task<string> PrepareUnsignedTx(string addressFrom, string addressTo, ulong amount, ulong fee);
    Task<string> BroadcastSignedTx(string signedTx);
    Task<Fee> EstimateFee(ConfirmationSpeed speed, bool shielded);

    Task<uint> GetCurrentHeight();

    // TODO: Task<TxInfo> GetTxInfo(string txHash);

    Task Rewind(uint height);

    Task<uint> Sync();

    Task<int> ImportPublicKeyPackage(PubKey pubkey);

    Task<AccountAddress> NewSaplingAccount(int idFvk);
}

public class BlockExplorer : IBlockExplorer
{
  public BlockExplorer(string url) {
    var channel = new Channel(url, ChannelCredentials.Insecure);
    this.client = new Zams.BlockExplorer.BlockExplorerClient(channel);
  }

  public async Task<bool> ValidateAddress(string address, ulong amount, bool tracked)
  {
    var req = new ValidateAddressRequest();
    req.Address = address;
    req.Amount = amount;
    req.Tracked = tracked;
    var res = await client.ValidateAddressAsync(req);
    return res.Value;
  }

  public async Task<Fee> EstimateFee(ConfirmationSpeed speed, bool shielded)
  {
    var req = new EstimateFeeRequest();
    req.Shielded = shielded;
    var res = await client.EstimateFeeAsync(req);
    return res;
  }

  public async Task<Balance> GetAccountBalance(int accountId, uint minConfirmations)
  {
    var req = new GetAccountBalanceRequest();
    req.Account = accountId;
    req.MinConfirmations = minConfirmations;
    var res = await client.GetAccountBalanceAsync(req);
    return res;
  }

  public async Task<uint> GetCurrentHeight()
  {
    var res = await client.GetCurrentHeightAsync(new Empty());
    return res.Height;
  }

  public async Task<int> ImportPublicKeyPackage(PubKey pubkey)
  {
    var res = await client.ImportPublicKeyAsync(pubkey);
    return res.Id;
  }

  public async Task<AccountAddress> NewSaplingAccount(int fvkId) {
    var id = new PubKeyId();
    id.Id = fvkId;
    var res = await client.NewAccountAsync(id);
    return res;
  }

  public Task<string> PrepareUnsignedTx(string addressFrom, string addressTo, ulong amount, ulong fee)
  {
    throw new System.NotImplementedException();
  }

  public Task<string> BroadcastSignedTx(string signedTx)
  {
    throw new System.NotImplementedException();
  }

  public async Task Rewind(uint height)
  {
    var req = new BlockHeight();
    req.Height = height;
    await client.RewindAsync(req);
  }

  public async Task<uint> Sync() {
    var res = await client.SyncAsync(new Empty());
    return res.Height;
  }

  readonly Zams.BlockExplorer.BlockExplorerClient client;
}
