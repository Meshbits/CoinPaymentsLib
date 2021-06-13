using System.Threading.Tasks;
using Grpc.Core;
using Zams;

public interface ISigner
{
  Task<Keys> GenerateTransparentKey(Entropy entropy);
  Task<Keys> GenerateSaplingKey(Entropy entropy);
}

sealed class Signer : ISigner
{
  public Signer(string url) {
    var channel = new Channel(url, ChannelCredentials.Insecure);
    this.client = new Zams.Signer.SignerClient(channel);
  }

  async public Task<Keys> GenerateTransparentKey(Entropy entropy) {
    var keys = await this.client.GenerateTransparentKeyAsync(entropy);
    return keys;
  }

  async public Task<Keys> GenerateSaplingKey(Entropy entropy)
  {
    var keys = await this.client.GenerateSaplingKeyAsync(entropy);
    return keys;
  }

  readonly Zams.Signer.SignerClient client;
}
