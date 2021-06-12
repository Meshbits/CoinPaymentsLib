use crate::ZamsConfig;
use postgres::{Client, NoTls};
use zcash_primitives::consensus::{MainNetwork, Parameters};
use bip39::{Mnemonic, Language, Seed};
use secp256k1::{All, SecretKey, PublicKey, Secp256k1};
use tiny_hderive::bip32::ExtendedPrivKey;
use tiny_hderive::bip44::ChildNumber;
use zcash_primitives::legacy::TransparentAddress;
use zcash_client_backend::encoding::{encode_transparent_address, encode_payment_address};
use zcash_primitives::zip32::{ExtendedSpendingKey, ExtendedFullViewingKey, DiversifierIndex};
use ripemd160::{Ripemd160, Digest};
use sha2::Sha256;

#[allow(dead_code)]
pub fn populate_taddr(count: u32) {
    let config = ZamsConfig::default();
    let mut client = Client::connect(&config.connection_string, NoTls).unwrap();
    let p = client.prepare("INSERT INTO accounts(address) VALUES ($1) ON CONFLICT(address) DO NOTHING").unwrap();
    let network = &MainNetwork;
    let entropy = [21u8; 32];
    let seed = Seed::new(&Mnemonic::from_entropy(&entropy, Language::English).unwrap(), "");
    let path = "m";
    let secp = Secp256k1::<All>::new();
    let ext = ExtendedPrivKey::derive(&seed.as_bytes(), path).unwrap();
    for i in 0..count {
        let e = ext.child(ChildNumber::non_hardened_from_u32(i)).unwrap();
        let secret_key = SecretKey::from_slice(&e.secret()).unwrap();
        let pub_key = PublicKey::from_secret_key(&secp, &secret_key);
        let pub_key = pub_key.serialize();
        let pub_key = Ripemd160::digest(&Sha256::digest(&pub_key));
        let address = TransparentAddress::PublicKey(pub_key.into());
        let address = encode_transparent_address(&network.b58_pubkey_address_prefix(), &network.b58_script_address_prefix(), &address);
        client.execute(&p, &[&address]).unwrap();
    }
}

pub fn populate_zaddr(count: u32) {
    let config = ZamsConfig::default();
    let mut client = Client::connect(&config.connection_string, NoTls).unwrap();
    let p = client.prepare("INSERT INTO accounts(address) VALUES ($1) ON CONFLICT(address) DO NOTHING").unwrap();
    let network = &MainNetwork;
    let entropy = [21u8; 32];
    let seed = Seed::new(&Mnemonic::from_entropy(&entropy, Language::English).unwrap(), "");
    let master = ExtendedSpendingKey::master(seed.as_bytes());
    let fvk = ExtendedFullViewingKey::from(&master);
    let mut di = DiversifierIndex::default();
    for _ in 0..count {
        let (di2, addr) = fvk.address(di).unwrap();
        di = di2;
        di.increment().unwrap();
        let address = encode_payment_address(network.hrp_sapling_payment_address(), &addr);
        client.execute(&p, &[&address]).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_populate() {
        populate_taddr(500_000);
        populate_zaddr(500_000);
    }
}