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
            entropy.SeedPhrase = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
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

            var sk = "secret-extended-key-test1qtwl8cujyl3qzq9drtqe0374e6hlzgrm02rrkx4sds333ptuv25yuyy0ge2uvppvxm45z8zhextlt6dqdxxhc5z2l4k9n4qyfktcd7p26pf2nxvaj4msnfjgrc7n7her5lynvusrmnr92y0e4q0lvkawsxzjr4j6kglyjrszf4w7qmkctlkpunwugl5t76ealqtlwvjx8zf2a0ftw7d8n5x0gud53hdsftvmjzf87tr250nw9kpexp0xg0v8ttef5ffnfmkg82n468s9vjp30";
            var signedTx = await signer.SignTx(sk, tx);

            var explorer = new BlockExplorer("localhost:3001");
            var res = await explorer.BroadcastSignedTx(signedTx);

            Console.WriteLine(res);
        }
    }
}
