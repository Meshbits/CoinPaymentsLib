using System;
using Xunit;
using System.Text.Json;
using Google.Protobuf;

namespace ZcashLib.Tests
{
    public class BlockExplorerTests
    {
        [Fact]
        public async void AccountTests()
        {
            var explorer = new BlockExplorer("localhost:3001");
            var height = await explorer.GetCurrentHeight();
            Console.WriteLine("{0}", height);

            var fvk = "zxviewtestsapling1qtwl8cujyl3qzq9drtqe0374e6hlzgrm02rrkx4sds333ptuv25yuyy0ge2uvppvx6kww4fh0pf4ypmngazktt4vn9helt305mrlfhyclu7u8uyms3wqj04qhtesn2uh6xd6p8rklupcsnmthdz024uguu4zy40kajjey9gvf4w7qmkctlkpunwugl5t76ealqtlwvjx8zf2a0ftw7d8n5x0gud53hdsftvmjzf87tr250nw9kpexp0xg0v8ttef5ffnfmkg82n468sehwen9";
            var pubkey = new Zams.PubKey();
            pubkey.Fvk = fvk;
            var id = await explorer.ImportPublicKeyPackage(pubkey);
            Console.WriteLine("{0}", id);

            var account = await explorer.NewSaplingAccount(id);
            Console.WriteLine("{0}", account.Address);

            var balance = await explorer.GetAccountBalance(1, 0);
            Console.WriteLine("{0}", balance.Total);
        }

        [Fact]
        // It is recommended to load a checkpoint before this test
        public async void SyncTest() {
            var explorer = new BlockExplorer("localhost:3001");
            await explorer.Sync();
        }

        [Fact]
        public async void PrepareTxTest() {
            var explorer = new BlockExplorer("localhost:3001");
            await explorer.CancelUnsignedTx(2);
            var tx = await explorer.PrepareUnsignedTx(1, "ztestsapling1vlw97que72g9qa5na0w5a56rvm4320n2s47drxwrg5pyasaxjxkwqrv3cn9ndmv353fqwvtvjes", 1, 50000);
            var formatter = new JsonFormatter(JsonFormatter.Settings.Default);
            var txStr = formatter.Format(tx);
            await explorer.CancelUnsignedTx(tx.Id);

            Console.WriteLine(">> {0}", txStr);
        }


    }
}
