use tiny_hderive::bip32::ExtendedPrivKey;
use secp256k1::{Secp256k1, SecretKey, PublicKey, All};
use zcash_primitives::legacy::TransparentAddress;
use ripemd160::{Ripemd160, Digest};
use zcash_client_backend::encoding::{encode_transparent_address, encode_extended_full_viewing_key, decode_extended_full_viewing_key, encode_payment_address};
use zcash_primitives::constants::testnet::{B58_PUBKEY_ADDRESS_PREFIX, B58_SCRIPT_ADDRESS_PREFIX, HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, HRP_SAPLING_PAYMENT_ADDRESS};
use zcash_primitives::zip32::{ExtendedSpendingKey, ChildIndex, ExtendedFullViewingKey, DiversifierIndex};
use tiny_hderive::bip44::DerivationPath;
use anyhow::{Context, anyhow};

pub trait Signer {
    fn generate_keys(&self, path: &str) -> String;
    fn generate_address(&self, key: &str, diversifier_index: DiversifierIndex) -> anyhow::Result<(String, DiversifierIndex)>;
}

pub struct TransparentSigner {
    seed: [u8; 64],
    secp: Secp256k1<All>,
}

impl TransparentSigner {
    pub fn new(seed: &[u8; 64]) -> TransparentSigner {
        let secp = Secp256k1::new();
        TransparentSigner {
            seed: seed.clone(),
            secp,
        }
    }
}

impl Signer for TransparentSigner {
    fn generate_keys(&self, path: &str) -> String {
        let ext = ExtendedPrivKey::derive(&self.seed, path).unwrap();
        let secret_key = SecretKey::from_slice(&ext.secret()).unwrap();
        let pub_key = PublicKey::from_secret_key(&self.secp, &secret_key);
        let pub_key = pub_key.serialize();
        let pub_key = Ripemd160::digest(&pub_key);
        let address = TransparentAddress::PublicKey(pub_key.into());
        let address = encode_transparent_address(&B58_PUBKEY_ADDRESS_PREFIX, &B58_SCRIPT_ADDRESS_PREFIX, &address);
        address
    }

    fn generate_address(&self, _key: &str, _diversifier_index: DiversifierIndex) -> anyhow::Result<(String, DiversifierIndex)> {
        todo!()
    }
}

pub struct SaplingSigner {
    master: ExtendedSpendingKey,
}

impl SaplingSigner {
    pub fn new(seed: &[u8; 64]) -> SaplingSigner {
        let master = ExtendedSpendingKey::master(seed);
        SaplingSigner {
            master,
        }
    }
}

impl Signer for SaplingSigner {
    fn generate_keys(&self, path: &str) -> String {
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
        let extsk = ExtendedSpendingKey::from_path(&self.master, &path);
        let fvk = ExtendedFullViewingKey::from(&extsk);
        encode_extended_full_viewing_key(HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, &fvk)
    }

    fn generate_address(&self, key: &str, mut diversifier_index: DiversifierIndex) -> anyhow::Result<(String, DiversifierIndex)> {
        let fvk = decode_extended_full_viewing_key(HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, key)?.context("Invalid key")?;
        diversifier_index.increment().map_err(|_| anyhow::anyhow!("Out of diversifier indexes"))?;
        let (di, pa) = fvk.address(diversifier_index).map_err(|_| anyhow!("Invalid diversifier"))?;
        let address = encode_payment_address(HRP_SAPLING_PAYMENT_ADDRESS, &pa);
        Ok((address, di))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{save_viewing_key, save_account, establish_connection, make_new_account};

    #[test]
    pub fn test_generate_transparent() {
        let mut seed = [0u8; 64];
        seed.copy_from_slice(&hex::decode("fffcf9f6f3f0edeae7e4e1dedbd8d5d2cfccc9c6c3c0bdbab7b4b1aeaba8a5a29f9c999693908d8a8784817e7b7875726f6c696663605d5a5754514e4b484542").unwrap());
        let signer = TransparentSigner::new(&seed);
        let address = signer.generate_keys("m/0/2147483647'");
        assert_eq!(address, "tmEuJYrkbLTnRSPJJtEuybJHnHxRJ56aNAz");
    }

    #[test]
    pub fn test_generate_sapling() {
        let mut seed = [0u8; 64];
        seed.copy_from_slice(&hex::decode("fffcf9f6f3f0edeae7e4e1dedbd8d5d2cfccc9c6c3c0bdbab7b4b1aeaba8a5a29f9c999693908d8a8784817e7b7875726f6c696663605d5a5754514e4b484542").unwrap());
        let signer = SaplingSigner::new(&seed);
        let fvk = signer.generate_keys("m/0/2147483647'");
        assert_eq!(fvk, "zxviewtestsapling1qfa3sudalllllleyywsg65vusgex2rht985k25tcl90hruwup258elmatlv7whqqru4c6rtt8uhl428a33ak0h7uy83h9l2j7hx2qanjyr7s0sufmks6y4plnlpxm2cv38ngfpmrq7q7dkpygu6nnw6n80jg7jdtlau2vg8r68pn63ag8q6kzkdxp54g4gv0wy7wcn8sndy526tm7mwgewlulavppjx3qk8sl7av9u3rpy44k7ffyvhs5adz0cs4382rs6jwg32s4xqdcwrv0");
        let (address, _) = signer.generate_address(&fvk, DiversifierIndex::new()).unwrap();
        assert_eq!(address, "ztestsapling10g928q68yrsucpvu9jz55q5arpy756mqfmyqnyugk7q8rstnxy74n2j5xxjvz5vpq62e5vp7k5r");
    }

    #[test]
    pub fn test_populate_10_addresses() {
        let connection = establish_connection("postgres://hanh@localhost/zamsdb");
        let mut seed = [0u8; 64];
        seed.copy_from_slice(&hex::decode("fffcf9f6f3f0edeae7e4e1dedbd8d5d2cfccc9c6c3c0bdbab7b4b1aeaba8a5a29f9c999693908d8a8784817e7b7875726f6c696663605d5a5754514e4b484542").unwrap());
        let signer = SaplingSigner::new(&seed);
        let fvk = signer.generate_keys("m/0/2147483647'");
        let viewing_key_id = save_viewing_key(&fvk, &connection).unwrap();
        let mut di = DiversifierIndex::new();
        for i in 0..10 {
            let (address, di2) = signer.generate_address(&fvk, di).unwrap();
            di = di2;
            let account = make_new_account(&address, Some(viewing_key_id), Some(di2), None);
            save_account(&account, &connection).unwrap();
        }
    }
}
