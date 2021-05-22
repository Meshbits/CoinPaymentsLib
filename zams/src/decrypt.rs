use crate::models::ViewingKey;
use crate::zcashdrpc::TransactionShieldedOutput;
use zcash_client_backend::encoding::decode_extended_full_viewing_key;
use zcash_client_backend::proto::compact_formats::CompactOutput;
use zcash_primitives::consensus::{BlockHeight, MainNetwork};
use zcash_primitives::constants::testnet::HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY;
use zcash_primitives::memo::MemoBytes;
use zcash_primitives::note_encryption::try_sapling_note_decryption;
use zcash_primitives::primitives::{Note, PaymentAddress};

pub fn try_decode(
    vk: &ViewingKey,
    output: &TransactionShieldedOutput,
    height: u32,
) -> anyhow::Result<Option<(Note, PaymentAddress, MemoBytes)>> {
    println!("{} {} {} {}", vk.key, output.cmu, output.ephemeralKey, output.encCiphertext);


    let fvk = decode_extended_full_viewing_key(HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, &vk.key)
        .unwrap()
        .unwrap();
    let ivk = fvk.fvk.vk.ivk();
    let enc_ciphertext = hex::decode(&output.encCiphertext).unwrap();
    let mut cmu = hex::decode(&output.cmu).unwrap();
    let mut epk = hex::decode(&output.ephemeralKey).unwrap();
    cmu.reverse();
    epk.reverse();

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

    let r = try_sapling_note_decryption(
        &MainNetwork,
        BlockHeight::from_u32(height),
        &ivk,
        &epk,
        &cmu,
        &enc_ciphertext,
    );

    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zcashdrpc::{ZcashdConf, get_raw_transaction};
    use crate::testconfig::*;

    #[test]
    #[allow(non_snake_case)]
    fn test_decode() {
        let FVK = "zxviewtestsapling1qfa3sudalllllleyywsg65vusgex2rht985k25tcl90hruwup258elmatlv7whqqru4c6rtt8uhl428a33ak0h7uy83h9l2j7hx2qanjyr7s0sufmks6y4plnlpxm2cv38ngfpmrq7q7dkpygu6nnw6n80jg7jdtlau2vg8r68pn63ag8q6kzkdxp54g4gv0wy7wcn8sndy526tm7mwgewlulavppjx3qk8sl7av9u3rpy44k7ffyvhs5adz0cs4382rs6jwg32s4xqdcwrv0";
        let CMU = "47ddf190fead31509672f8afd6ba0d86ba472a804d39ed53babe072ff65a90a5";
        let EPK = "8d1602c00d021a74cba4fadf4709ee26e3725dd5dfa1c81afd2a8a31b575320d";
        let CIPHER = "8dcab0ee459e39304736d741fbcb363658dcb79ff8ef7ecac979bbea6c8cef4277012954961bd54dae3380ca7f67a53b757a90cf95e4f9d68c6c4cd1b6f64e3017673191259b450f3e23322d62063247038099f4d343cfade553fb9bc95fc1c8fe5c0f323e53237a33da02e0c521473b480562759bd24e98c62c2ae993edc877dc61dbc453b244df15ebbcf06905b3a99f37da9b5381bacc4742c53dc0d41139f8d9ef8b18e93ab8129350c6a9fe093cda053d01ac52380234d75f9b4d300b72aa49a4b4a261e92eb96eee7db57ad22f98f50a390964aa7b957c804d0fd3a0e5152a85ffc288481c6a9089d8a88efe8371fd2fff9021132ef7ace1c4c2f0c62acd3ae3b91c42d71c84b7f5edcd39dab80467d94e79e5b7194fe31950d722b7730c395c2c11e8c8ffc9cc610b05a364eadea0636c13b20e1f15a3ad313d9c1370a57e5fe8705d181bb7f08a6994926282e141207ed99af8f1b706bd935a8db26b4804c09caaee4be1e3d4396b80ff6c282c00b3ff31559ca2650041403bda8c71abd5dbd4a68367840327dbd6d0626ece773e11139bc66f88030727cf716ce42dd7a09f096f667284874098b81884243ac2a540ef66b499abc5f22e26828bf066b1f44264729eacfc76c54928021d483b9e1ec2c4882361d40048efc4c4e5f661ca8ed6126bcc1721ccdb3777534d45d4be919e9f5d52086bb2b5174ddd6d629f7f052de629f11477454e8868fddb546b993cc4f4a767d2cb10490db7c7109349d152fc6681f8c42c2ee94a34839aaa609b7298307bb7e635ee1c59911213b144754ee92c";

        let vk = ViewingKey {
            id: 0,
            key: FVK.to_string(),
        };
        let output = TransactionShieldedOutput {
            cv: "".to_string(),
            cmu: CMU.to_string(),
            ephemeralKey: EPK.to_string(),
            encCiphertext: CIPHER.to_string(),
        };
        try_decode(&vk, &output, 1202824).unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_decode_transaction() {
        let config = ZcashdConf::parse(TEST_ZCASHD_URL, TEST_DATADIR).unwrap();
        let client = reqwest::Client::new();
        let tx = get_raw_transaction("6a4c85706327094bbbbbd90fc2a7386d902cd15677b4e0c2460bccd63d36f178", &client, &config).await.unwrap();
        let ivk = ViewingKey {
            id: 0,
            key: "zxviewtestsapling1qfa3sudalllllleyywsg65vusgex2rht985k25tcl90hruwup258elmatlv7whqqru4c6rtt8uhl428a33ak0h7uy83h9l2j7hx2qanjyr7s0sufmks6y4plnlpxm2cv38ngfpmrq7q7dkpygu6nnw6n80jg7jdtlau2vg8r68pn63ag8q6kzkdxp54g4gv0wy7wcn8sndy526tm7mwgewlulavppjx3qk8sl7av9u3rpy44k7ffyvhs5adz0cs4382rs6jwg32s4xqdcwrv0".to_string()
        };
        let found = tx.vShieldedOutput.iter().any(|output| try_decode(&ivk, output, tx.height.unwrap()).unwrap().is_some());
        assert!(found);
    }
}
