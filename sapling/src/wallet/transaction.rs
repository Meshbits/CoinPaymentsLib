use crate::error::WalletError;
use crate::grpc::RawTransaction;
use crate::wallet::PostgresWallet;
use anyhow::{anyhow, Context};
use jubjub::Fr;

use serde::{Deserialize, Serialize};
use zcash_client_backend::address::RecipientAddress;
use zcash_client_backend::data_api::WalletRead;
use zcash_client_backend::encoding::{
    decode_extended_full_viewing_key, decode_extended_spending_key, decode_payment_address,
    encode_extended_full_viewing_key,
};
use zcash_primitives::consensus::{BlockHeight, BranchId, Network};
use zcash_primitives::constants::testnet::{
    HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, HRP_SAPLING_EXTENDED_SPENDING_KEY,
    HRP_SAPLING_PAYMENT_ADDRESS,
};
use zcash_primitives::merkle_tree::IncrementalWitness;
use zcash_primitives::sapling::{Diversifier, Node, Rseed};
use zcash_primitives::transaction::builder::Builder;
use zcash_primitives::transaction::components::{Amount, OutPoint, TxOut};

use crate::wallet::scan::connect_lightnode;
use bytes::Bytes;
use rand::prelude::SliceRandom;
use rand::RngCore;
use serde_json::Value;
use std::str::FromStr;
use tokio::runtime::Runtime;
use zcash_client_backend::wallet::SpendableNote;
use zcash_primitives::consensus;
use zcash_primitives::legacy::Script;
use zcash_primitives::sapling::keys::OutgoingViewingKey;
use zcash_proofs::prover::LocalTxProver;
use zcash_primitives::transaction::components::amount::DEFAULT_FEE;

#[derive(Debug, Clone)]
pub enum Account {
    Transparent(String),
    Shielded(String, String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnsignedTx {
    pub height: i64,
    pub fvk: String,
    pub trp_inputs: Vec<UTXO>,
    pub sap_inputs: Vec<SaplingTxIn>,
    pub output: Option<SaplingTxOut>,
    pub change_address: String,
    pub change_fvk: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaplingTxIn {
    pub amount: u64,
    pub address: String,
    pub diversifier: String,
    pub rcm: String,
    pub witness: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaplingTxOut {
    pub amount: u64,
    pub address: String,
    pub ovk: Option<String>,
}

trait NoteLike<TxIn> {
    fn note_value(&self) -> Amount;
    fn to_tx_input(&self, from_address: &str) -> Result<TxIn, WalletError>;
}

impl NoteLike<SaplingTxIn> for SpendableNote {
    fn note_value(&self) -> Amount {
        self.note_value
    }

    fn to_tx_input(&self, from_address: &str) -> Result<SaplingTxIn, WalletError> {
        let a = u64::from(self.note_value);
        match self.rseed {
            Rseed::BeforeZip212(rcm) => {
                let mut mp = Vec::<u8>::new();
                self.witness.write(&mut mp).map_err(WalletError::IO)?;

                let input = SaplingTxIn {
                    amount: a,
                    address: from_address.to_string(),
                    diversifier: hex::encode(&self.diversifier.0),
                    rcm: hex::encode(rcm.to_bytes()),
                    witness: hex::encode(mp),
                };
                Ok(input)
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UTXO {
    pub amount: u64,
    pub tx_hash: String,
    pub output_index: i32,
    pub hex: String,
    pub spent: bool,
}

impl NoteLike<UTXO> for UTXO {
    fn note_value(&self) -> Amount {
        Amount::from_i64(self.amount as i64).unwrap()
    }

    fn to_tx_input(&self, _from_address: &str) -> Result<UTXO, WalletError> {
        Ok(self.clone())
    }
}

fn select_notes<TxIn, N: NoteLike<TxIn>, R: RngCore>(
    from_address: &str,
    spendable_notes: &mut Vec<N>,
    target_value: Amount,
    rng: &mut R,
) -> Result<Vec<TxIn>, WalletError> {
    spendable_notes.shuffle(rng);
    let mut partial_sum = Amount::zero();
    let mut index = 0usize;
    for s in spendable_notes.iter() {
        partial_sum += s.note_value();
        index += 1;
        if partial_sum >= target_value {
            break;
        }
    }
    let (selected_notes, _) = spendable_notes.split_at(index);
    let selected_value: Amount = selected_notes.iter().map(|n| n.note_value()).sum();
    if selected_value < target_value {
        return Err(WalletError::Error(anyhow!(
            "Not enough funds: needed={:?}, available={:?}",
            target_value,
            selected_value
        )));
    }

    selected_notes
        .iter()
        .map(|s| s.to_tx_input(from_address))
        .collect::<Result<Vec<_>, _>>()
}

pub fn prepare_tx<R: RngCore>(
    from_account: i32,
    to_address: &str,
    change_account: i32,
    amount: i64,
    rng: &mut R,
) -> Result<UnsignedTx, WalletError> {
    let amount = Amount::from_i64(amount).map_err(|_| anyhow!("Cannot convert amount"))?;
    let target_value = amount + DEFAULT_FEE;

    let wallet = PostgresWallet::new()?;
    let (height, anchor_height) = wallet.get_target_and_anchor_heights()?.unwrap();

    let (change_address, change_fvk) = match wallet.get_account(change_account)? {
        Account::Transparent(_) => {
            return Err(WalletError::Error(anyhow!(
                "Change account must be shielded"
            )))
        }
        Account::Shielded(change_address, change_fvk) => (change_address, change_fvk),
    };
    let wallet = PostgresWallet::new()?;

    let mut tx = UnsignedTx {
        height: i64::from(height),
        fvk: String::new(),
        trp_inputs: Vec::new(),
        sap_inputs: Vec::new(),
        output: None,
        change_address,
        change_fvk,
    };

    let mut ovk: Option<OutgoingViewingKey> = None;

    match wallet.get_account(from_account)? {
        Account::Shielded(from_address, extfvk) => {
            tx.fvk = extfvk.clone();
            let extfvk =
                decode_extended_full_viewing_key(HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, &extfvk)
                    .unwrap()
                    .unwrap();
            ovk = Some(extfvk.fvk.ovk);
            let mut spendable_notes =
                wallet.get_spendable_notes_by_address(&from_address, anchor_height)?;
            let mut tx_ins = select_notes(&from_address, &mut spendable_notes, target_value, rng)?;
            tx.sap_inputs.append(&mut tx_ins);
        }
        Account::Transparent(from_address) => {
            let mut spendable_notes =
                wallet.get_spendable_transparent_notes_by_address(&from_address)?;
            let mut tx_ins = select_notes(&from_address, &mut spendable_notes, target_value, rng)?;
            tx.trp_inputs.append(&mut tx_ins);
        }
    }

    RecipientAddress::decode(&Network::TestNetwork, to_address)
        .ok_or_else(|| WalletError::Error(anyhow!("Could not decode address {}", to_address)))?;

    tx.output = Some(SaplingTxOut {
        address: to_address.to_string(),
        amount: u64::from(amount),
        ovk: ovk.map(|ovk| hex::encode(ovk.0)),
    });

    Ok(tx)
}

pub fn sign_tx(spending_key: &str, unsigned_tx: UnsignedTx) -> Result<Bytes, WalletError> {
    let prover = LocalTxProver::with_default_location()
        .ok_or_else(|| WalletError::Error(anyhow!("Could not build local prover")))?;
    let height = BlockHeight::from_u32(unsigned_tx.height as u32);
    let consensus_branch_id = BranchId::for_height(&Network::TestNetwork, height);
    let mut builder = Builder::new(Network::TestNetwork, height);

    for input in unsigned_tx.trp_inputs.iter() {
        let seckey =
            secp256k1::SecretKey::from_str(spending_key).context("Cannot parse secret key")?;
        let mut tx_hash = [0u8; 32];
        hex::decode_to_slice(&input.tx_hash, &mut tx_hash)?;
        let utxo = OutPoint::new(tx_hash, input.output_index as u32);
        let hex = hex::decode(&input.hex).unwrap();
        let script = Script(hex);
        let txout = TxOut {
            value: Amount::from_u64(input.amount).unwrap(),
            script_pubkey: script,
        };
        builder.add_transparent_input(seckey, utxo, txout)?;
    }

    for input in unsigned_tx.sap_inputs.iter() {
        let extsk = decode_extended_spending_key(HRP_SAPLING_EXTENDED_SPENDING_KEY, spending_key)
            .map_err(WalletError::Bech32)?
            .unwrap();
        let mut d = [0u8; 11];
        hex::decode_to_slice(&input.diversifier, &mut d)?;
        let diversifier = Diversifier(d);
        let from = decode_payment_address(HRP_SAPLING_PAYMENT_ADDRESS, &input.address)
            .map_err(WalletError::Bech32)?
            .unwrap();
        let mut rcm = [0u8; 32];
        hex::decode_to_slice(&input.rcm, &mut rcm)?;
        let rseed = Rseed::BeforeZip212(Fr::from_bytes(&rcm).unwrap());
        let note = from.create_note(input.amount, rseed).unwrap();
        let w = hex::decode(&input.witness)?;
        let witness = IncrementalWitness::<Node>::read(&w[..]).map_err(WalletError::IO)?;
        let merkle_path = witness.path().unwrap();
        builder
            .add_sapling_spend(extsk.clone(), diversifier, note, merkle_path)
            .map_err(WalletError::TxBuilder)?;
    }

    let output = unsigned_tx.output.unwrap();
    let recipient = RecipientAddress::decode(&Network::TestNetwork, &output.address)
        .ok_or_else(|| WalletError::Error(anyhow!("Invalid recipient address")))?;
    let ovk = output.ovk.map(|o| {
        let mut ovk = [0u8; 32];
        hex::decode_to_slice(&o, &mut ovk).unwrap();
        OutgoingViewingKey(ovk)
    });
    match recipient {
        RecipientAddress::Shielded(pa) => {
            builder.add_sapling_output(ovk, pa, Amount::from_u64(output.amount).unwrap(), None)?;
        }
        RecipientAddress::Transparent(ta) => {
            builder.add_transparent_output(&ta, Amount::from_u64(output.amount).unwrap())?;
        }
    }

    let change_recipient =
        RecipientAddress::decode(&Network::TestNetwork, &unsigned_tx.change_address)
            .ok_or_else(|| WalletError::Error(anyhow!("Invalid recipient address")))?;
    let change_pa = match change_recipient {
        RecipientAddress::Shielded(pa) => pa,
        RecipientAddress::Transparent(_) => {
            return Err(WalletError::Error(anyhow!(
                "Change address must be shielded"
            )))
        }
    };
    let change_fvk = decode_extended_full_viewing_key(
        HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY,
        &unsigned_tx.change_fvk,
    )
    .map_err(WalletError::Bech32)?
    .unwrap();
    let change_ovk = change_fvk.fvk.ovk;
    builder.send_change_to(change_ovk, change_pa);
    let (tx, _) = builder.build(consensus_branch_id, &prover)?;
    let mut raw_tx = vec![];
    tx.write(&mut raw_tx).map_err(WalletError::IO)?;

    Ok(Bytes::from(raw_tx))
}

pub fn broadcast_tx(tx: &Bytes) -> Result<String, WalletError> {
    let r = Runtime::new().unwrap();
    r.block_on(async {
        let mut client = connect_lightnode().await?;
        let res = client
            .send_transaction(RawTransaction {
                data: tx.to_vec(),
                height: 0,
            })
            .await?
            .into_inner();
        if res.error_code == 0 {
            let tx_id: Value = serde_json::from_str(&res.error_message).unwrap();
            Ok(tx_id.as_str().unwrap().to_string())
        } else {
            Err(WalletError::Error(anyhow!(res.error_message)))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn test_prepare_shielded_tx() {
        let mut rng = thread_rng();
        let tx = prepare_tx(1,
                            "ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn",
                            1,
                            20_000_000,
                            &mut rng).unwrap();
        println!("{}", serde_json::to_string(&tx).unwrap());
    }

    #[test]
    fn test_prepare_transparent_tx() {
        let mut rng = thread_rng();
        let tx = prepare_tx(2,
                            "ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn",
                            1,
                            500_000,
                            &mut rng).unwrap();
        println!("{}", serde_json::to_string(&tx).unwrap());
    }

    #[test]
    fn test_sign_tx() {
        let tx_json = r#"{"height":1432389,"fvk":"zxviewtestsapling1qfkvrtdpqqqqpqqr6g4fx2nwjx9788l0deqqtq9mcfmar4vk3dwtcjwfqaklemn9j4fcskzsl4fsqecxs5wx7n8sna4lcgh4lynd40hw3dv02tyc6l80xfj0wfuzmxwesw8kzvtskg6h8tzzmfxky7gslhpeacn6tl2s2c0zjzp7wak2envsy8tv9txq2tkkfa2y99rfxztza3lhvsswmz4q9p2xe05kh4yg7q3nad5s2vjj763maju3hpkpwwgavk7jpl2y8vqu5jqmglfeq","trp_inputs":[],"sap_inputs":[{"amount":49999000,"address":"ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn","diversifier":"79b99cb8c2a4647b06906b","rcm":"f014e4cc9fe42a4a46bab3a7932ae2fcaf5b93f810eb07d1ffcac422f94e1008","witness":"017062d4f6d73664a730b96979e24068f3ce7b4788a1bcec8c5e518303ec3e0e5a0169f43decade5a86c736f9683b467376f7a755946eaa43eb44e56de8247f534181000011c972bb37f512e3874341e5e4e5d52f98069cd864502ec9b8ae16bb1c0acde1100000154daf736c7f68b0f22072be3e6b59434618b514c0c32d044c187048e2600c60b01f98b75b62bf721db663a442cbfa411242ec07ccb70aee42ea3618ca7b157270a014d03c61befc68d02710784399567067db98f24eda340a1fd4a3ecc549d0fd0660001b4c1c846cae1423eaf52f1a8b1bfdde9ed9d43ced4d80dba9e72d862a0e03e4001ba0d7aa9e68417291c63b835fa64114f5899208238de59ee360f594c8b6c1b72018469338dcbdf2f7e54bca5bc3e1c5fad4a656f206040436d3d0433a901218b5e016d559de7a1a382349cf97fe01a2fba41a49bb5e3b306d9ff8c2bcc301c731c00000001f08f39275112dd8905b854170b7f247cf2df18454d4fa94e6e4f9320cca05f24011f8322ef806eb2430dc4a7a41c1b344bea5be946efc7b4349c1c9edb14ff9d3903c422c2dd975315e782dea13817c340e2ca2dfc6f82a81a3afb0d8c190b63b121245c9e78d7b573d2d32ccbd99a7902dcdf23ad4ae8ade853f3d2afc499bac8622097e0e5876229b41d449795e0b83d56174e9c35ef3b6bd7b5268746423fbe1500"}],"output":{"amount":20000000,"address":"ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn","ovk":"9083e776caccd9021d6c2acc052ed64f5442946930962ec7f76420ed8aa02854"},"change_address":"ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn","change_fvk":"zxviewtestsapling1qfkvrtdpqqqqpqqr6g4fx2nwjx9788l0deqqtq9mcfmar4vk3dwtcjwfqaklemn9j4fcskzsl4fsqecxs5wx7n8sna4lcgh4lynd40hw3dv02tyc6l80xfj0wfuzmxwesw8kzvtskg6h8tzzmfxky7gslhpeacn6tl2s2c0zjzp7wak2envsy8tv9txq2tkkfa2y99rfxztza3lhvsswmz4q9p2xe05kh4yg7q3nad5s2vjj763maju3hpkpwwgavk7jpl2y8vqu5jqmglfeq"}"#;
        let tx = serde_json::from_str::<UnsignedTx>(tx_json).unwrap();
        let signed_tx = sign_tx("secret-extended-key-test1qfkvrtdpqqqqpqqr6g4fx2nwjx9788l0deqqtq9mcfmar4vk3dwtcjwfqaklemn9j4em4cggyw6n8heukq963nqx6upz7ktyg4kyeanmal5l3ssely5q4nd2jcsnulytl5zpyp7zyftrfhzfyec9rdf3hyg9cm70jeg0zrs8jzp7wak2envsy8tv9txq2tkkfa2y99rfxztza3lhvsswmz4q9p2xe05kh4yg7q3nad5s2vjj763maju3hpkpwwgavk7jpl2y8vqu5jqega2yj",
                tx).unwrap();
        assert!(!signed_tx.is_empty());
    }

    #[test]
    fn test_sign_trp_tx() {
        let tx_json = r#"{"height":1432389,"fvk":"","trp_inputs":[{"amount":1000000,"tx_hash":"9786e9b81c0c3f39d5c800ff9fe72ac0593221ae7b9980ba5454c7db63a8b674","output_index":0,"hex":"76a914d8ab493736da02f11ed682f88339e720fb0379d188ac","spent":false}],"sap_inputs":[],"output":{"amount":500000,"address":"ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn","ovk":null},"change_address":"ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn","change_fvk":"zxviewtestsapling1qfkvrtdpqqqqpqqr6g4fx2nwjx9788l0deqqtq9mcfmar4vk3dwtcjwfqaklemn9j4fcskzsl4fsqecxs5wx7n8sna4lcgh4lynd40hw3dv02tyc6l80xfj0wfuzmxwesw8kzvtskg6h8tzzmfxky7gslhpeacn6tl2s2c0zjzp7wak2envsy8tv9txq2tkkfa2y99rfxztza3lhvsswmz4q9p2xe05kh4yg7q3nad5s2vjj763maju3hpkpwwgavk7jpl2y8vqu5jqmglfeq"}"#;
        let tx = serde_json::from_str::<UnsignedTx>(tx_json).unwrap();
        let signed_tx = sign_tx(
            "877c779ad9687164e9c2f4f0f4ff0340814392330693ce95a58fe18fd52e6e93",
            tx,
        )
        .unwrap();
        assert!(!signed_tx.is_empty());
    }
}
