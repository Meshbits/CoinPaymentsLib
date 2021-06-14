using System;
using Xunit;
using System.Text.Json;

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
    }
}
