use zcash_primitives::zip32::{ExtendedSpendingKey, ExtendedFullViewingKey, ChildIndex};
use zcash_client_backend::encoding::{encode_extended_spending_key, encode_extended_full_viewing_key, encode_transparent_address};
use zcash_primitives::constants::testnet::{HRP_SAPLING_EXTENDED_SPENDING_KEY, HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, B58_PUBKEY_ADDRESS_PREFIX, B58_SCRIPT_ADDRESS_PREFIX};
use tiny_hderive::bip44::DerivationPath;
use bip39::{Language, Mnemonic, Seed};
use ripemd160::{Ripemd160, Digest};
use sha2::{Sha256};
use crate::zams_rpc::Entropy;
use crate::zams_rpc::entropy::TypeOfEntropy;
use crate::error::WalletError;
use anyhow::Context;
use tiny_hderive::bip32::ExtendedPrivKey;
use secp256k1::{SecretKey, PublicKey, Secp256k1, All};
use zcash_primitives::legacy::TransparentAddress;

pub fn get_bip39_seed(entropy: Entropy) -> crate::Result<Seed> {
    let mnemonic = match entropy.type_of_entropy.context("Missing entropy")? {
        TypeOfEntropy::SeedPhrase(seed) => {
            Mnemonic::from_phrase(&seed, Language::English)?
        }
        TypeOfEntropy::Hex(hex) => {
            Mnemonic::from_entropy(&hex::decode(hex)?, Language::English)?
        }
    };
    Ok(Seed::new(&mnemonic, ""))
}

pub fn generate_transparent_address(seed: Seed, path: &str) -> (String, String) {
    let secp = Secp256k1::<All>::new();
    let ext = ExtendedPrivKey::derive(&seed.as_bytes(), path).unwrap();
    let secret_key = SecretKey::from_slice(&ext.secret()).unwrap();
    let pub_key = PublicKey::from_secret_key(&secp, &secret_key);
    let pub_key = pub_key.serialize();
    let pub_key = Ripemd160::digest(&Sha256::digest(&pub_key));
    let address = TransparentAddress::PublicKey(pub_key.into());
    let address = encode_transparent_address(&B58_PUBKEY_ADDRESS_PREFIX, &B58_SCRIPT_ADDRESS_PREFIX, &address);
    let seckey = secret_key.to_string();
    (seckey, address)
}

pub fn generate_sapling_keys(seed: Seed, path: &str) -> (String, String) {
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
    let sk = encode_extended_spending_key(HRP_SAPLING_EXTENDED_SPENDING_KEY, &extsk);
    let fvk = encode_extended_full_viewing_key(HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, &fvk);
    (sk, fvk)
}
