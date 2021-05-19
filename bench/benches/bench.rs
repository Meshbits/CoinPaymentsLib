use criterion::{black_box, criterion_group, criterion_main, Criterion};
use zcash_bench::try_decrypt_note;

fn test_decrypt_benchmark(c: &mut Criterion) {
    c.bench_function("decrypt", |b| b.iter(|| {
        let height = 1202824;
        let FVK = "zxviews1q0kl7tavzyqqpq8efe0vpgzwc37zj0zr9j2quurncpsy74tdvh9c3racve9yfv6gkssvekw4sz6ueenvup6whupguzkg5rgp0kma37r4uxz9472w4zwra4jv6fm5dc2cevfpjsxdgndagslmgdwudhv4stklzfeszrlcnsqxyr2qt8tsf4yxs3he4rzllcly7xagfmnlycvvnvhhr9l9j6ad693rkueqys9f7mkc7aacxwp3tfc9hpvlckxnj4nwu6jef2x98jefhcgmpkrmn";
        let CMU = "263a4c43290ce7d644c0a3ab694bb4710a4c3b20a528e2297ac1d360b017f704";
        let EPK = "d8360fc851709bb8d53e1f7ad2bab2c28c70d2c3c570af6620599f078ab37e02";
        let CIPHER = "c9c2479a4c936b25c4848a15fc5debad377f0305f7e744cfb550bc09da12922669b6a4d82d2c8d56d9c804682bae459474467aad9417739f3eea7f6526c344b789c493c53186909b128f29dfe571cd3f9dc9d5cbaff371e5cd20a813bda9e3b2465522a3665f1bc33af61011438173dde65777627505bc79a4aea00d1437631a73538fd35faadeb44a5e781791a2008a6b079895b7f8c7f8dfe6d7b1ecdb1ca5b44980841b500f582d991ba8f68479d9927bc9e04c2cf364decc8dc5d6bc0eca67e6ea7e8fe96788944210bf6c537852655badfa64c362aa0baa2765d47623a5542a93f62d06721b05fa3129077d8f13c95304d720cc8c7241f804593c51767c9df043204c75a8e7eefe12aedb1af5cb7a907831b3e99e09649dcebfbc8b3b82726cdf67aa1e0f578f384bed3a8d037c44664589326e640fad6dd5376f26152c993fcec92e6b13f596e8133b3077da5048303cbce41f1c66f4f78f97d280b40274b8770dbe1206d62ba2df99a0e7138be3488b66d5c2b8ecfbc46c72eb052d0854a7aa4f8d92b83fa080b24c031f0aa9c889fc78ee7787ff76569bf42b1190c9c98afa33aa1e6070bcd0470da01ead7f4db1687950e483bd62d5ec848a90fade6241944b2bf459a1b85406c137d6ffa6dd6ae27afc9df1a4b762e23ad21a8971102383ffea77ad4c68d168fcc7b2d55b06d6dd94ebe4f90f22010a8f26923b66d95d77273e84026f108c8a4420959437b3166e0c1011c584870719eafdb2476d98322987e06940c471fd591a4ca954a873b7cb9fa4e41925de0cff927172d98cbd87e5f7";

        let (note, address, memo) = try_decrypt_note(
            height,
            FVK,
            hex::decode(CMU).unwrap(),
            hex::decode(EPK).unwrap(),
            hex::decode(CIPHER).unwrap(),
        )
            .unwrap();
    }));
}

criterion_group!(benches, test_decrypt_benchmark);
criterion_main!(benches);
