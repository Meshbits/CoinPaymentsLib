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

            var fvk = "zxviewtestsapling1qtcgwxn8yl3qzqxjhr937awugvs3gl4hrpx6u7258l3pfpz0nuu8kx090rxrx33nyeayxahrsv6e9wcmhcdvsqa6dcms976s8uwflycn7xw060du2zfdhj3l7eddusgnknveua0a7gw2jxrjm2uy6vcwd7qvsmj37zyzxyxtrqqyefz83daf2gvtvtpqkrg7ahzym0m674m4xmkc739696ptmql968ecdsr5us6pcud5dl6wurc78jer56c57c5eqktp0dpvjuty2xgrz0mlt";
            var pubkey = new Zams.PubKey();
            pubkey.Fvk = fvk;
            var id = await explorer.ImportPublicKeyPackage(pubkey);
            Console.WriteLine("{0}", id);

            var accountA = await explorer.NewSaplingAccount(id);
            Console.WriteLine("{0}", accountA.Address);

            var accountB = await explorer.NewSaplingAccount(id);
            Console.WriteLine("{0}", accountB.Address);
        }

        [Fact]
        public async void GetBalanceTest() {
            var explorer = new BlockExplorer("localhost:3001");
            var balance = await explorer.GetAccountBalance(2, 10);
            Console.WriteLine("{0}", balance);
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
            await explorer.CancelUnsignedTx(1);
            var tx = await explorer.PrepareUnsignedTx(2, "ztestsapling1zhu3ppsnhhjt0p262cynnshdduucrq4eu73fp65mwyvhn0nr2phvh9n0alym9huzzvrxjvuaqgd", 2, 1000000);
            var formatter = new JsonFormatter(JsonFormatter.Settings.Default);
            var txStr = formatter.Format(tx);
            await explorer.CancelUnsignedTx(tx.Id);

            Console.WriteLine(">> {0}", txStr);
        }
    }
}
