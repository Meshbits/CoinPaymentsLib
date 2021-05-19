use zcash_client_backend::encoding::decode_extended_full_viewing_key;
use zcash_client_backend::proto::compact_formats::CompactOutput;
use zcash_primitives::consensus::BlockHeight;
use zcash_primitives::consensus::Network::MainNetwork;
use zcash_primitives::constants::mainnet::{
    HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, HRP_SAPLING_PAYMENT_ADDRESS,
};
use zcash_primitives::memo::MemoBytes;
use zcash_primitives::note_encryption::try_sapling_note_decryption;
use zcash_primitives::sapling::{Note, PaymentAddress, SaplingIvk};

pub fn try_decrypt_note(
    height: u32,
    fvk: &str,
    mut cmu: Vec<u8>,
    mut epk: Vec<u8>,
    ciphertext: Vec<u8>,
) -> Option<(Note, PaymentAddress, MemoBytes)> {
    cmu.reverse(); // bytes are reversed in decode raw tx
    epk.reverse();

    let fvk = decode_extended_full_viewing_key(HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, fvk)
        .unwrap()
        .unwrap(); // two unwraps because the result is Result<Option<T>>
    let ivk = fvk.fvk.vk.ivk();
    let output = CompactOutput {
        // Use this class to help us with decoding cmu, epk
        cmu,
        epk,
        ciphertext: vec![], // the content does not matter
        unknown_fields: Default::default(),
        cached_size: Default::default(),
    };

    let cmu = output.cmu().unwrap();
    let epk = output.epk().unwrap();

    try_sapling_note_decryption(
        &MainNetwork,
        BlockHeight::from_u32(height),
        &ivk,
        &epk,
        &cmu,
        &ciphertext,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zcash_client_backend::encoding::encode_payment_address;

    #[test]
    fn test_vector1() -> anyhow::Result<()> {
        let height = 1202824;
        let FVK = "zxviews1q0kl7tavzyqqpq8efe0vpgzwc37zj0zr9j2quurncpsy74tdvh9c3racve9yfv6gkssvekw4sz6ueenvup6whupguzkg5rgp0kma37r4uxz9472w4zwra4jv6fm5dc2cevfpjsxdgndagslmgdwudhv4stklzfeszrlcnsqxyr2qt8tsf4yxs3he4rzllcly7xagfmnlycvvnvhhr9l9j6ad693rkueqys9f7mkc7aacxwp3tfc9hpvlckxnj4nwu6jef2x98jefhcgmpkrmn";
        let CMU = "337fcf043e1f7e4ddb0d37d533e1995ddef860bf218484624143c5d75f70a5e0";
        let EPK = "a29816cd86810601c9a9eb61c1e2f146816848b1513b9b7c4ff8b9e8f4695b43";
        let CIPHER = "18ef22d92037b418241aa3ab234785431766cc98fc92c97ced405883d5f7287fe030cfc4e1c000d56a5b8a56e9fe3191b32e5bf8f660bd7708868a3fa39ca4133dfa8beace75fe1fe9e85f2a14c7654778e864e80e08a5e37b2d4bfd74ded6e01ced184cf42abae1d50b297d6658d2eab1f73256dcfb0037ce599a88a811fffa4ec300ef993868a906468fc0ea984753be5ab2642be0e829ca9e883a608717471b531aba487ea08e3376b0927b931bbae207640258e76c08ea1a613fa6a4a068951ec86b8c085f002ac62b446935377fbae8006b8c3477f9de28eefa34193c3591df6ec30ce800f228700af4ba57c533abdb9a8530631f8414c1548b8cc28eaace5270c4f2716f2014641b2cc7aa0e19b4eb0e7e197d077fc8f61027a33c804601a4feb2e093374e1983e4370219e1236d3fedbfd8b7a54a4f6f22742f7b064883c35de02543b258eb716bef757b5551a8fce938f8640e922161a00c38c39de49f21e9d4561aa234010a0e98557bd8596ebd7c029f088814fbd4300e4b36c183905efa040a85446282752dd30edb39c44c4dfb06f6680daf6d5cfbd5ddb938ebdd9e57d7b8ca22e0402b10fb28a727c8a9dd0b50576a593846d38efe104ee4d51d97e94f33bc880913150bcd66d2159757aaf80b96752481aef12127d41aba1e614f0764286a3930a5dc8c48cb6ce7c99951f0efe35e199400f6230e60c6cca088e9dd4eb5139978bf6ba7df2022132fd08def2c466313e9c987e834b97acda2d820cc00aeee50d078d7433dc4f1c9e7590e7a920923a35366ef93657ae6ff06f01e526f";

        // this one does not decrypt
        assert!(try_decrypt_note(
            height,
            FVK,
            hex::decode(CMU)?,
            hex::decode(EPK)?,
            hex::decode(CIPHER)?,
        )
        .is_none());

        Ok(())
    }

    #[test]
    fn test_vector2() -> anyhow::Result<()> {
        let height = 1202824;
        let FVK = "zxviews1q0kl7tavzyqqpq8efe0vpgzwc37zj0zr9j2quurncpsy74tdvh9c3racve9yfv6gkssvekw4sz6ueenvup6whupguzkg5rgp0kma37r4uxz9472w4zwra4jv6fm5dc2cevfpjsxdgndagslmgdwudhv4stklzfeszrlcnsqxyr2qt8tsf4yxs3he4rzllcly7xagfmnlycvvnvhhr9l9j6ad693rkueqys9f7mkc7aacxwp3tfc9hpvlckxnj4nwu6jef2x98jefhcgmpkrmn";
        let CMU = "263a4c43290ce7d644c0a3ab694bb4710a4c3b20a528e2297ac1d360b017f704";
        let EPK = "d8360fc851709bb8d53e1f7ad2bab2c28c70d2c3c570af6620599f078ab37e02";
        let CIPHER = "c9c2479a4c936b25c4848a15fc5debad377f0305f7e744cfb550bc09da12922669b6a4d82d2c8d56d9c804682bae459474467aad9417739f3eea7f6526c344b789c493c53186909b128f29dfe571cd3f9dc9d5cbaff371e5cd20a813bda9e3b2465522a3665f1bc33af61011438173dde65777627505bc79a4aea00d1437631a73538fd35faadeb44a5e781791a2008a6b079895b7f8c7f8dfe6d7b1ecdb1ca5b44980841b500f582d991ba8f68479d9927bc9e04c2cf364decc8dc5d6bc0eca67e6ea7e8fe96788944210bf6c537852655badfa64c362aa0baa2765d47623a5542a93f62d06721b05fa3129077d8f13c95304d720cc8c7241f804593c51767c9df043204c75a8e7eefe12aedb1af5cb7a907831b3e99e09649dcebfbc8b3b82726cdf67aa1e0f578f384bed3a8d037c44664589326e640fad6dd5376f26152c993fcec92e6b13f596e8133b3077da5048303cbce41f1c66f4f78f97d280b40274b8770dbe1206d62ba2df99a0e7138be3488b66d5c2b8ecfbc46c72eb052d0854a7aa4f8d92b83fa080b24c031f0aa9c889fc78ee7787ff76569bf42b1190c9c98afa33aa1e6070bcd0470da01ead7f4db1687950e483bd62d5ec848a90fade6241944b2bf459a1b85406c137d6ffa6dd6ae27afc9df1a4b762e23ad21a8971102383ffea77ad4c68d168fcc7b2d55b06d6dd94ebe4f90f22010a8f26923b66d95d77273e84026f108c8a4420959437b3166e0c1011c584870719eafdb2476d98322987e06940c471fd591a4ca954a873b7cb9fa4e41925de0cff927172d98cbd87e5f7";

        for _ in 1..100000 {
            let (note, address, memo) = try_decrypt_note(
                height,
                FVK,
                hex::decode(CMU)?,
                hex::decode(EPK)?,
                hex::decode(CIPHER)?,
            )
            .unwrap();

            assert_eq!(note.value, 100000);
            assert_eq!(
                encode_payment_address(HRP_SAPLING_PAYMENT_ADDRESS, &address),
                "zs1a7qnkg8hr74ujj08jhjcdfs7s62yathqlyn5vd2e8ww96ln28m3t2jkxun5fp7hxjntcg8ccuvs"
            );
            assert_eq!(
                std::str::from_utf8(memo.as_array())?
                    .to_string()
                    .trim_end_matches(char::from(0)),
                "Hello world!!!"
            );
        }

        Ok(())
    }
}
