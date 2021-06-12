using System;
using Grpc.Core;

namespace CoinPaymentsLib
{
    public class ZamsClient {
        public ZamsClient(Zams.BlockExplorer.BlockExplorerClient client) {
            this.client = client;
        }

        public string GetVersion() {
            var version = client.GetVersion(new Zams.Empty());
            return version.Version;
        }

        readonly Zams.BlockExplorer.BlockExplorerClient client;
    }


    class Program
    {
        static void Main(string[] args)
        {
            var channel = new Channel("127.0.0.1:3001", ChannelCredentials.Insecure);
            var client = new ZamsClient(new Zams.BlockExplorer.BlockExplorerClient(channel));
            var version = client.GetVersion();
            Console.WriteLine(version);
        }
    }
}
