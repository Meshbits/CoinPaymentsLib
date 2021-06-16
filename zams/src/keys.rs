use zcash_primitives::zip32::{ExtendedSpendingKey, ExtendedFullViewingKey, ChildIndex};
use zcash_client_backend::encoding::{encode_extended_spending_key, encode_extended_full_viewing_key, encode_transparent_address};
use tiny_hderive::bip44::DerivationPath;
use bip39::{Language, Mnemonic, Seed};
use ripemd160::{Ripemd160, Digest};
use sha2::Sha256;
use crate::zams_rpc as grpc;

use anyhow::Context;
use tiny_hderive::bip32::ExtendedPrivKey;
use secp256k1::{SecretKey, PublicKey, Secp256k1, All};
use zcash_primitives::legacy::TransparentAddress;
use zcash_primitives::consensus::Parameters;

pub fn get_bip39_seed(entropy: grpc::Entropy) -> crate::Result<Seed> {
    let mnemonic = match entropy.type_of_entropy.context("Missing entropy")? {
        grpc::entropy::TypeOfEntropy::SeedPhrase(seed) => {
            Mnemonic::from_phrase(&seed, Language::English)?
        }
        grpc::entropy::TypeOfEntropy::Hex(hex) => {
            Mnemonic::from_entropy(&hex::decode(hex)?, Language::English)?
        }
    };
    Ok(Seed::new(&mnemonic, ""))
}

pub fn generate_transparent_address<P: Parameters>(network: &P, seed: Seed, path: &str) -> (String, String) {
    let secp = Secp256k1::<All>::new();
    let ext = ExtendedPrivKey::derive(&seed.as_bytes(), path).unwrap();
    let secret_key = SecretKey::from_slice(&ext.secret()).unwrap();
    let pub_key = PublicKey::from_secret_key(&secp, &secret_key);
    let pub_key = pub_key.serialize();
    let pub_key = Ripemd160::digest(&Sha256::digest(&pub_key));
    let address = TransparentAddress::PublicKey(pub_key.into());
    let address = encode_transparent_address(&network.b58_pubkey_address_prefix(), &network.b58_script_address_prefix(), &address);
    let seckey = secret_key.to_string();
    (seckey, address)
}

pub fn generate_sapling_keys<P: Parameters>(network: &P, seed: Seed, path: &str) -> (String, String) {
    let master = ExtendedSpendingKey::master(seed.as_bytes());
    let path: DerivationPath = path.parse().unwrap();
    let path: Vec<ChildIndex> = path.iter().map(|child| {
        let c = u32::from_be_bytes(child.to_bytes());
        let c = c & !(1 << 31);
        if child.is_hardened() {
            ChildIndex::Hardened(c)
        }
        else {
            ChildIndex::NonHardened(c)
        }
    }).collect();
    let extsk = ExtendedSpendingKey::from_path(&master, &path);
    let fvk = ExtendedFullViewingKey::from(&extsk);
    let sk = encode_extended_spending_key(network.hrp_sapling_extended_spending_key(), &extsk);
    let fvk = encode_extended_full_viewing_key(network.hrp_sapling_extended_full_viewing_key(), &fvk);
    (sk, fvk)
}
