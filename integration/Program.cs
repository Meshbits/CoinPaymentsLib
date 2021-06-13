using System;
using System.Threading.Tasks;
using System.Text.Json;

namespace CoinPaymentsLib
{
    class Program
    {
        async static Task TestGenerateKeys()
        {
            var signer = new Signer("localhost:3002");
            var entropy = new Zams.Entropy();
            entropy.SeedPhrase = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
            entropy.Path = "m/0'/123431";
            var tkeys = await signer.GenerateTransparentKey(entropy);
            Console.WriteLine("{0} {1}", tkeys.Pk.Address, tkeys.Sk);

            var zkeys = await signer.GenerateSaplingKey(entropy);
            Console.WriteLine("{0} {1}", zkeys.Pk.Fvk, zkeys.Sk);

            var pk = zkeys.Pk;
            Console.WriteLine(JsonSerializer.Serialize(pk));
        }

        async static Task TestBlockExplorer() {
            var explorer = new BlockExplorer("localhost:3001");
            var height = await explorer.GetCurrentHeight();
            Console.WriteLine("{0}", height);

            var fvk = "zxviews1qtwl8cujyl3qzq9drtqe0374e6hlzgrm02rrkx4sds333ptuv25yuyy0ge2uvppvx6kww4fh0pf4ypmngazktt4vn9helt305mrlfhyclu7u8uyms3wqj04qhtesn2uh6xd6p8rklupcsnmthdz024uguu4zy40kajjey9gvf4w7qmkctlkpunwugl5t76ealqtlwvjx8zf2a0ftw7d8n5x0gud53hdsftvmjzf87tr250nw9kpexp0xg0v8ttef5ffnfmkg82n468sc4lr4w";
            var pubkey = new Zams.PubKey();
            pubkey.Fvk = fvk;
            var id = await explorer.ImportPublicKeyPackage(pubkey);
            Console.WriteLine("{0}", id);

            var account = await explorer.NewSaplingAccount(id);
            Console.WriteLine("{0}", account.Address);

            var balance = await explorer.GetAccountBalance(1, 0);
            Console.WriteLine("{0}", balance.Total);
        }

        static void Main(string[] args) {
            TestGenerateKeys().Wait();
            TestBlockExplorer().Wait();
        }
    }
}
