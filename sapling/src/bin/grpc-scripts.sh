#!/bin/bash

grpcurl -import-path proto -proto api.proto -plaintext -d '{"from_account": 1, "to_address": "ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn", "change_account": 1, "amount": 500000, "timestamp": 0}' localhost:3001 zams.BlockExplorer.PrepareUnsignedTx

# Signer
grpcurl -import-path proto -proto api.proto -plaintext localhost:3002 zams.Signer.GetVersion

grpcurl -import-path proto -proto api.proto -plaintext \
-d '{"seed_phrase": "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong",
"path": "m/0'/2147483647'"}' \
localhost:3002 zams.Signer.GenerateSaplingKey

{
  "pk": {
    "fvk": "zxviewtestsapling1qt2j2s43llll7lastrtrju0085qvcr36tg0pfcdhe6chz9efcdzj3j37k8qwc6yskxlf5jvc495qx4x7r2z0w6v6mntm6ddfh2t68d8dhnne7vygpjnr3yyh2z2s6dwqvjzvyz8hst7dycscvyp5af7scg020a3hlzmerslt3azk69knef58rr640y7k70gtarrtyulqmj8q0s8f96rl6v5pwfvc4vwwpeuhaevgech6fpnznvy34qlng5q5tyuaa4q06ey5dcd6p5spz04qe"
  },
  "sk": "secret-extended-key-test1qt2j2s43llll7lastrtrju0085qvcr36tg0pfcdhe6chz9efcdzj3j37k8qwc6ysk9fats6y0dtrscmpwzp77pnushvsx7u3l5jcj40gyre26ujnyt5s8l6d54j7m5ztdslrx0am7aaqgh0v65st6q9cl2qyesq6mzw2erqv3azk69knef58rr640y7k70gtarrtyulqmj8q0s8f96rl6v5pwfvc4vwwpeuhaevgech6fpnznvy34qlng5q5tyuaa4q06ey5dcd6p5s2830k8"
}

grpcurl -import-path proto -proto api.proto -plaintext \
-d '{"seed_phrase": "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong",
"path": "m/0'/2147483647'"}' \
localhost:3002 zams.Signer.GenerateTransparentKey

{
  "pk": {
    "address": "tmJ3oV1rtGNEvV3BR6aHCfb4Gns5e4gE1mL"
  },
  "sk": "5a65c3bf2c82419c573bcc60f985b0e1660f6d7e3173308333484930b1b53406"
}


grpcurl -import-path proto -proto api.proto -plaintext \
-d '{"fvk": "zxviewtestsapling1qt2j2s43llll7lastrtrju0085qvcr36tg0pfcdhe6chz9efcdzj3j37k8qwc6yskxlf5jvc495qx4x7r2z0w6v6mntm6ddfh2t68d8dhnne7vygpjnr3yyh2z2s6dwqvjzvyz8hst7dycscvyp5af7scg020a3hlzmerslt3azk69knef58rr640y7k70gtarrtyulqmj8q0s8f96rl6v5pwfvc4vwwpeuhaevgech6fpnznvy34qlng5q5tyuaa4q06ey5dcd6p5spz04qe"}' \
localhost:3001 zams.BlockExplorer.ImportPublicKey

grpcurl -import-path proto -proto api.proto -plaintext \
-d '{"id_fvk": 4, "diversifier_high": 0, "diversifier_low": 5}' \
localhost:3001 zams.BlockExplorer.NewAccount

grpcurl -import-path proto -proto api.proto -plaintext \
-d '{"address": "tmJ3oV1rtGNEvV3BR6aHCfb4Gns5e4gE1mL"}' \
localhost:3001 zams.BlockExplorer.ImportPublicKey

grpcurl -import-path proto -proto api.proto -plaintext \
localhost:3001 zams.BlockExplorer.Sync

grpcurl -import-path proto -proto api.proto -plaintext \
-d '{"address": "tmJ3oV1rtGNEvV3BR6aHCfb4Gns5e4gE1mL"}' \
localhost:3001 zams.BlockExplorer.ValidateAddress

grpcurl -import-path proto -proto api.proto -plaintext \
localhost:3001 zams.BlockExplorer.EstimateFee

grpcurl -import-path proto -proto api.proto -plaintext \
localhost:3001 zams.BlockExplorer.GetCurrentHeight

grpcurl -import-path proto -proto api.proto -plaintext \
-d '{"id": 5}' \
localhost:3001 zams.BlockExplorer.CancelTx

grpcurl -import-path proto -proto api.proto -plaintext \
-d '{"from_account": 1, "to_address": "tmJ3oV1rtGNEvV3BR6aHCfb4Gns5e4gE1mL", "change_account": 1, "amount": 50000, "timestamp": 1623054216}' \
localhost:3001 zams.BlockExplorer.PrepareUnsignedTx > data/payment.json

cat data/payment.json | grpcurl -import-path proto -proto api.proto -plaintext \
-d @ \
localhost:3002 zams.Signer.SignTx

cat data/signed.json | grpcurl -import-path proto -proto api.proto -plaintext \
-d @ \
localhost:3001 zams.BlockExplorer.BroadcastSignedTx

grpcurl -import-path proto -proto api.proto -plaintext \
-d '{"account": 1, "min_confirmations": 1}' \
localhost:3001 zams.BlockExplorer.GetAccountBalance

grpcurl -import-path proto -proto api.proto -plaintext \
-d '{"id": 1}' \
localhost:3001 zams.BlockExplorer.GetPaymentInfo

grpcurl -import-path proto -proto api.proto -plaintext \
-d '{"id": 1}' \
localhost:3001 zams.BlockExplorer.ListPaymentId
