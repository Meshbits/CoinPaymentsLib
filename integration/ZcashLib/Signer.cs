using System.Threading.Tasks;
using Grpc.Core;
using Zams;

public interface ISigner
{
  Task<Keys> GenerateTransparentKey(Entropy entropy);
  Task<Keys> GenerateSaplingKey(Entropy entropy);

  Task<SignedTx> SignTx(string sk, UnsignedTx tx);
}

sealed public class Signer : ISigner
{
  public Signer(string url) {
    var channel = new Channel(url, ChannelCredentials.Insecure);
    this.client = new Zams.Signer.SignerClient(channel);
  }

  public async Task<Keys> GenerateTransparentKey(Entropy entropy) {
    var keys = await client.GenerateTransparentKeyAsync(entropy);
    return keys;
  }

  public async Task<Keys> GenerateSaplingKey(Entropy entropy)
  {
    var keys = await client.GenerateSaplingKeyAsync(entropy);
    return keys;
  }

  public async Task<SignedTx> SignTx(string sk, UnsignedTx tx) {
    var req = new SignTxRequest();
    req.SecretKey = sk;
    req.UnsignedTx = tx;
    var res = await client.SignTxAsync(req);
    return res;
  }

  readonly Zams.Signer.SignerClient client;
}
