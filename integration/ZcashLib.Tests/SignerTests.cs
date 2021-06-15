using System;
using Xunit;
using System.Text.Json;
using System.Reflection;
using System.IO;
using Google.Protobuf;

namespace ZcashLib.Tests
{
    public class SignerTests
    {
        [Fact]
        public async void KeyGeneration()
        {
            var signer = new Signer("localhost:3002");
            var entropy = new Zams.Entropy();
            entropy.SeedPhrase = "gadget chalk season manage enter amateur analyst pole dial jungle please sweet forget gap whale";
            entropy.Path = "m/0'/123431";
            var tkeys = await signer.GenerateTransparentKey(entropy);
            Console.WriteLine("{0} {1}", tkeys.Pk.Address, tkeys.Sk);

            var zkeys = await signer.GenerateSaplingKey(entropy);
            Console.WriteLine("{0} {1}", zkeys.Pk.Fvk, zkeys.Sk);

            var pk = zkeys.Pk;
            Console.WriteLine(JsonSerializer.Serialize(pk));
        }

        [Fact]
        public async void SignTest() {
            var signer = new Signer("localhost:3002");
            var assembly = Assembly.GetExecutingAssembly();
            var reader = new StreamReader(assembly.GetManifestResourceStream("ZcashLib.Tests.tx.json"));
            var txJson = reader.ReadToEnd();
            var parser = new JsonParser(JsonParser.Settings.Default);
            var tx = parser.Parse<Zams.UnsignedTx>(txJson);

            var sk = "secret-extended-key-test1qtcgwxn8yl3qzqxjhr937awugvs3gl4hrpx6u7258l3pfpz0nuu8kx090rxrx33nyecgundmrr2nz788yw9tt43dy5zlxfhkw08g84vxu7jkxenj40ysyww5gupnxgt47jeywwca7pcskyj3cqc0kwyj66ejfhsd8jaz8hsyrqqyefz83daf2gvtvtpqkrg7ahzym0m674m4xmkc739696ptmql968ecdsr5us6pcud5dl6wurc78jer56c57c5eqktp0dpvjuty2xgdyge6r";
            var signedTx = await signer.SignTx(sk, tx);

            var explorer = new BlockExplorer("localhost:3001");
            var res = await explorer.BroadcastSignedTx(signedTx);

            Console.WriteLine(res);
        }
    }
}
