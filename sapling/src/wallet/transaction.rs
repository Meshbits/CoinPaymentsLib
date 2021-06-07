use crate::error::WalletError;
use crate::grpc::RawTransaction;

use anyhow::{anyhow, Context};
use jubjub::Fr;


use zcash_client_backend::address::RecipientAddress;

use zcash_client_backend::encoding::{
    decode_extended_full_viewing_key, decode_extended_spending_key, decode_payment_address,
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

use crate::db;
use crate::db::DbPreparedStatements;
use crate::wallet::scan::connect_lightnode;


use postgres::{Client, GenericClient};
use rand::prelude::SliceRandom;
use rand::RngCore;
use serde_json::Value;

// use std::ops::DerefMut;
use std::str::FromStr;
use std::time::SystemTime;
use tokio::runtime::Runtime;
use zcash_client_backend::wallet::SpendableNote;

use zcash_primitives::legacy::Script;
use zcash_primitives::sapling::keys::OutgoingViewingKey;
use zcash_primitives::transaction::components::amount::DEFAULT_FEE;
use zcash_proofs::prover::LocalTxProver;
use crate::zams_rpc::*;

#[derive(Debug, Clone)]
pub enum Account {
    Transparent(String),
    Shielded(String, String),
}

pub struct SpendableNoteWithId {
    pub id: i32,
    pub note: SpendableNote,
}

trait NoteLike<TxIn> {
    fn id(&self) -> i32;
    fn note_value(&self) -> Amount;
    fn to_tx_input(&self, id: i32, from_address: &str) -> Result<TxIn, WalletError>;
}

impl NoteLike<SaplingTxIn> for SpendableNoteWithId {
    fn id(&self) -> i32 {
        self.id
    }
    fn note_value(&self) -> Amount {
        self.note.note_value
    }

    fn to_tx_input(&self, id: i32, from_address: &str) -> Result<SaplingTxIn, WalletError> {
        let a = u64::from(self.note.note_value);
        match self.note.rseed {
            Rseed::BeforeZip212(rcm) => {
                let mut mp = Vec::<u8>::new();
                self.note.witness.write(&mut mp).map_err(WalletError::IO)?;

                let input = SaplingTxIn {
                    id,
                    amount: a,
                    address: from_address.to_string(),
                    diversifier: hex::encode(&self.note.diversifier.0),
                    rcm: hex::encode(rcm.to_bytes()),
                    witness: hex::encode(mp),
                };
                Ok(input)
            }
            _ => unreachable!(),
        }
    }
}

impl NoteLike<Utxo> for Utxo {
    fn id(&self) -> i32 {
        self.id
    }
    fn note_value(&self) -> Amount {
        Amount::from_i64(self.amount as i64).unwrap()
    }
    fn to_tx_input(&self, _id: i32, _from_address: &str) -> Result<Utxo, WalletError> {
        Ok(self.clone())
    }
}

fn select_notes<TxIn, N: NoteLike<TxIn>, R: RngCore>(
    from_address: &str,
    spendable_notes: &mut Vec<N>,
    target_value: Amount,
    rng: &mut R,
) -> crate::Result<Vec<TxIn>> {
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
        .map(|s| s.to_tx_input(s.id(), from_address))
        .collect::<Result<Vec<_>, _>>()
}

pub fn prepare_tx<C: GenericClient, R: RngCore>(
    datetime: SystemTime,
    from_account: i32,
    to_address: &str,
    change_account: i32,
    amount: i64,
    c: &mut C,
    statements: &DbPreparedStatements,
    rng: &mut R,
) -> crate::Result<UnsignedTx> {
    let amount = Amount::from_i64(amount).map_err(|_| anyhow!("Cannot convert amount"))?;
    let target_value = amount + DEFAULT_FEE;

    let (height, anchor_height) = db::get_target_and_anchor_heights(c)?.unwrap();

    let (change_address, change_fvk) = match db::get_account(c, change_account)? {
        Account::Transparent(_) => {
            return Err(WalletError::Error(anyhow!(
                "Change account must be shielded"
            )))
        }
        Account::Shielded(change_address, change_fvk) => (change_address, change_fvk),
    };

    let mut tx = UnsignedTx {
        id: 0,
        height: u32::from(height) as i32,
        fvk: String::new(),
        trp_inputs: Vec::new(),
        sap_inputs: Vec::new(),
        output: None,
        change_address: change_address.clone(),
        change_fvk,
    };

    let mut ovk: Option<OutgoingViewingKey> = None;

    let mut notes: Vec<i32> = vec![];
    let mut utxos: Vec<i32> = vec![];

    let from_address = match db::get_account(c, from_account)? {
        Account::Shielded(from_address, extfvk) => {
            tx.fvk = extfvk.clone();
            let extfvk =
                decode_extended_full_viewing_key(HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, &extfvk)
                    .unwrap()
                    .unwrap();
            ovk = Some(extfvk.fvk.ovk);
            let mut spendable_notes = db::get_spendable_notes_by_address(
                c,
                statements,
                &from_address,
                u32::from(anchor_height),
            )?;
            let mut tx_ins = select_notes(&from_address, &mut spendable_notes, target_value, rng)?;
            tx_ins.iter().for_each(|txin| notes.push(txin.id));
            tx.sap_inputs.append(&mut tx_ins);
            from_address
        }
        Account::Transparent(from_address) => {
            let mut spendable_notes =
                db::get_spendable_transparent_notes_by_address(c, statements, &from_address)?;
            let mut tx_ins = select_notes(&from_address, &mut spendable_notes, target_value, rng)?;
            tx_ins.iter().for_each(|txin| utxos.push(txin.id));
            tx.trp_inputs.append(&mut tx_ins);
            from_address
        }
    };

    RecipientAddress::decode(&Network::TestNetwork, to_address)
        .ok_or_else(|| WalletError::Error(anyhow!("Could not decode address {}", to_address)))?;

    tx.output = Some(SaplingTxOut {
        address: to_address.to_string(),
        amount: u64::from(amount),
        ovk: ovk.map(|ovk| hex::encode(ovk.0)).unwrap_or(String::new()),
    });

    let id_payment = db::store_payment(
        c,
        datetime,
        from_account,
        &from_address,
        &to_address,
        &change_address,
        i64::from(amount),
        &notes,
        &utxos,
    )?;
    tx.id = id_payment;

    Ok(tx)
}

pub fn sign_tx(spending_key: &str, unsigned_tx: UnsignedTx) -> crate::Result<SignedTx> {
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
        tx_hash.reverse();
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
    let ovk = match output.ovk.as_str() {
        "" => None,
        o => {
            let mut ovk = [0u8; 32];
            hex::decode_to_slice(o, &mut ovk).unwrap();
            Some(OutgoingViewingKey(ovk))
        }
    };
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

    let raw_tx = hex::encode(raw_tx);
    Ok(SignedTx {
        id: unsigned_tx.id,
        raw_tx,
    })
}

pub fn broadcast_tx(c: &mut Client, signed_tx: &SignedTx) -> crate::Result<String> {
    let r = Runtime::new().unwrap();
    let res = r.block_on(async {
        let mut client = connect_lightnode().await?;
        let res = client
            .send_transaction(RawTransaction {
                data: hex::decode(&signed_tx.raw_tx)?,
                height: 0,
            })
            .await?
            .into_inner();
        Ok::<_, WalletError>(res)
    })?;
    if res.error_code == 0 {
        let tx_id: Value = serde_json::from_str(&res.error_message).unwrap();
        let tx_id = tx_id.as_str().unwrap().to_string();
        db::mark_paid(c, signed_tx.id, &tx_id)?;
        Ok(tx_id)
    } else {
        Err(WalletError::Error(anyhow!(res.error_message)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CONNECTION_STRING;
    use postgres::{Client, NoTls};
    use rand::thread_rng;
    
    
    use std::sync::{Arc, Mutex};

    fn setup() -> (Arc<Mutex<Client>>, DbPreparedStatements) {
        let client = Client::connect(CONNECTION_STRING, NoTls).unwrap();
        let c = Arc::new(Mutex::new(client));
        let mut client = c.lock().unwrap();
        client.execute("UPDATE received_notes SET payment = NULL", &[]).unwrap();
        client.execute("UPDATE utxos SET payment = NULL", &[]).unwrap();
        let statements = DbPreparedStatements::prepare(&mut client).unwrap();
        (c.clone(), statements)
    }

    #[test]
    fn test_prepare_shielded_tx() {
        let mut rng = thread_rng();
        let (c, statements) = setup();
        let tx = prepare_tx(SystemTime::UNIX_EPOCH, 1,
                            "ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn",
                            1,
                            20_000_000,
                            &mut *c.lock().unwrap(), &statements,
                            &mut rng).unwrap();
        println!("{}", serde_json::to_string(&tx).unwrap());
    }

    #[test]
    fn test_prepare_transparent_tx() {
        let mut rng = thread_rng();
        let (c, statements) = setup();
        let tx = prepare_tx(SystemTime::UNIX_EPOCH, 2,
                            "ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn",
                            1,
                            500_000,
                            &mut *c.lock().unwrap(), &statements,
                            &mut rng).unwrap();
        println!("{}", serde_json::to_string(&tx).unwrap());
    }

    #[test]
    fn test_sign_tx() {
        let tx_json = r#"{"id":7,"height":1438929,"fvk":"zxviewtestsapling1qfkvrtdpqqqqpqqr6g4fx2nwjx9788l0deqqtq9mcfmar4vk3dwtcjwfqaklemn9j4fcskzsl4fsqecxs5wx7n8sna4lcgh4lynd40hw3dv02tyc6l80xfj0wfuzmxwesw8kzvtskg6h8tzzmfxky7gslhpeacn6tl2s2c0zjzp7wak2envsy8tv9txq2tkkfa2y99rfxztza3lhvsswmz4q9p2xe05kh4yg7q3nad5s2vjj763maju3hpkpwwgavk7jpl2y8vqu5jqmglfeq","trp_inputs":[],"sap_inputs":[{"id":8,"amount":49496000,"address":"ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn","diversifier":"79b99cb8c2a4647b06906b","rcm":"7ca5ad2265311704a4764eb838dfe07cb3fce96f7a9f29b024b8fde62ce1fa01","witness":"01b402041c0990cec1a94ccc7b1891fb435ab7c5ee3d77f76ea553c54464cbe643001001aacb702d2abed6aeaf918a21b2ac81a7d094d396f4a48229765269bea18dc82b000138a4ed6a370ac246e809c0bdd8c1bb92599379c410d517e55b9065e76570cc0e0000000001cc23dbfe7d27d7ad768868d7a96b6b31260ca34e4fbf164f652eb8e651f2fd3801b4c1c846cae1423eaf52f1a8b1bfdde9ed9d43ced4d80dba9e72d862a0e03e4001ba0d7aa9e68417291c63b835fa64114f5899208238de59ee360f594c8b6c1b72018469338dcbdf2f7e54bca5bc3e1c5fad4a656f206040436d3d0433a901218b5e016d559de7a1a382349cf97fe01a2fba41a49bb5e3b306d9ff8c2bcc301c731c00000001f08f39275112dd8905b854170b7f247cf2df18454d4fa94e6e4f9320cca05f24011f8322ef806eb2430dc4a7a41c1b344bea5be946efc7b4349c1c9edb14ff9d39045453a956cdb8ac799791415d8719cd77c46242bc53e6f83bd5c43889c9f81a2c5949057dc54d4f3190e18c095c4b1b0ebc676a2efc4cc19340ce5f7e03e3e5691d2dcba385f143b0f2cca16fd2f0faafeca2ae257742c266318626965c173536d2dbdc965c08d23d09b457328de48a248105c643b6c522f6291f087dc7746c1a0101df4c68750fe1db09744cd5af904b53a4a339d34d7a6a86642cd61381a9ee8b4c017c3dd9e32ca1d0fcacaa6b211543622b7766e391919680747fef03b33bb5ca2805000001b77627db19f550fb7b42dd2ad78b7f9a70fb5438c789ba14394f09a06c7b2a4700012c2c133c9aa15ecc67f808c159b1b7b78ea51df86ef02ca993d2f7d6ba4a1043"}],"output":{"amount":20000000,"address":"ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn","ovk":"9083e776caccd9021d6c2acc052ed64f5442946930962ec7f76420ed8aa02854"},"change_address":"ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn","change_fvk":"zxviewtestsapling1qfkvrtdpqqqqpqqr6g4fx2nwjx9788l0deqqtq9mcfmar4vk3dwtcjwfqaklemn9j4fcskzsl4fsqecxs5wx7n8sna4lcgh4lynd40hw3dv02tyc6l80xfj0wfuzmxwesw8kzvtskg6h8tzzmfxky7gslhpeacn6tl2s2c0zjzp7wak2envsy8tv9txq2tkkfa2y99rfxztza3lhvsswmz4q9p2xe05kh4yg7q3nad5s2vjj763maju3hpkpwwgavk7jpl2y8vqu5jqmglfeq"}"#;
        let tx = serde_json::from_str::<UnsignedTx>(tx_json).unwrap();
        let signed_tx = sign_tx("secret-extended-key-test1qfkvrtdpqqqqpqqr6g4fx2nwjx9788l0deqqtq9mcfmar4vk3dwtcjwfqaklemn9j4em4cggyw6n8heukq963nqx6upz7ktyg4kyeanmal5l3ssely5q4nd2jcsnulytl5zpyp7zyftrfhzfyec9rdf3hyg9cm70jeg0zrs8jzp7wak2envsy8tv9txq2tkkfa2y99rfxztza3lhvsswmz4q9p2xe05kh4yg7q3nad5s2vjj763maju3hpkpwwgavk7jpl2y8vqu5jqega2yj",
                tx).unwrap();
        assert!(!signed_tx.raw_tx.is_empty());
    }

    #[test]
    fn test_sign_trp_tx() {
        let tx_json = r#"{"id":8,"height":1438929,"fvk":"","trp_inputs":[{"id":5,"amount":500000,"tx_hash":"e416d3dbc1b7f34ba62ce6474bd59021bd96cf38a51be41f7cbd59c84db6258e","output_index":0,"hex":"76a914d8ab493736da02f11ed682f88339e720fb0379d188ac","spent":false},{"id":4,"amount":500000,"tx_hash":"6f84bf20c302ffcbcc7885647da7541ef956e3ce73e0ea1c7186aa910a52b723","output_index":0,"hex":"76a914d8ab493736da02f11ed682f88339e720fb0379d188ac","spent":false}],"sap_inputs":[],"output":{"amount":500000,"address":"ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn","ovk":""},"change_address":"ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn","change_fvk":"zxviewtestsapling1qfkvrtdpqqqqpqqr6g4fx2nwjx9788l0deqqtq9mcfmar4vk3dwtcjwfqaklemn9j4fcskzsl4fsqecxs5wx7n8sna4lcgh4lynd40hw3dv02tyc6l80xfj0wfuzmxwesw8kzvtskg6h8tzzmfxky7gslhpeacn6tl2s2c0zjzp7wak2envsy8tv9txq2tkkfa2y99rfxztza3lhvsswmz4q9p2xe05kh4yg7q3nad5s2vjj763maju3hpkpwwgavk7jpl2y8vqu5jqmglfeq"}"#;
        let tx = serde_json::from_str::<UnsignedTx>(tx_json).unwrap();
        let signed_tx = sign_tx(
            "877c779ad9687164e9c2f4f0f4ff0340814392330693ce95a58fe18fd52e6e93",
            tx,
        )
        .unwrap();
        assert!(!signed_tx.raw_tx.is_empty());
    }

    #[test]
    fn test_broadcast_tx() {
        let mut connection = Client::connect(CONNECTION_STRING, NoTls).unwrap();
        let signed_tx: SignedTx = serde_json::from_str(r#"{"id":21,"raw_tx":"0400008085202f8901a0a8689597f119d02e07930c38d70c411e4b711f5d119f635bae31fe3d38d659000000006a47304402202f85a86d3716c9825c9d426b757a1c48a1cd16495f7d5a298ba55d8494f3cf33022004ffaeeec82ba9d203bf5c4bd4064d09997a0bfab698250cef27199cdb384014012103c01e7425647bdefa82b12d9bad5e3e6865bee0502694b94ca58b666abc0a5c3bffffffff000000000038e51500c862f8ffffffffff00028f7e59e53cdb8437e485bc55cb817ad953190c7f6aca539337939d57581eaf30de46ccfcc2eccd88f72e5513b05ac0b069ab9d03ea270d3b11d9da9acb467165825a342833e3b64d1ce05a24610129e60fe338491578b1512d3d8deaa7a102e1fb7cd191370313be5e6131fbe9e398fe618dfc4534b214e1b54421f52b040970827c7934eec8229f274e9b813f06d4a151ac67b9de6ddb8788d0fe3871938acddacadf481eb83094399229e0aee178ad6128478aee5608dda2147506bcfdf9feed4983be05fd0619b910fbfe613e7e4862a4bae337188b2c6996dfa6496f08623d26d0a1b4fb4b206a2793fe84aaf307b3aca05dc3f4bd3c94bc1da01dedb9e4b63b705850f7b1b76ed675eb1f39eef9e1e55d90ea1e9fc768f3cc75bb270f9778f2c85b12a2979cd25133b2a69f168647e62adb174cfd4886734b02aca4d95d7f2d1ee74c02d1b556b7e6a692bb20f08bd9f6c4bb111dd11c875578abc4350c61d36afd709cbb89252e4161dd933edb4f74d292e0a739c27a37e138022c3cd78fddbded3873c1315b9da4b6209a8378f1cd560a7ce565c1c6cbc93e71510bb64e5f7754418b198a5f00c28b1acbcddc680590c2dd33b315ab38198dadcfa437fe8c9922280ba6b47c8077501a945f63697164b122fa50515dc220ee50210a7e8a44c5af8cad4d1ba8e3b5a6a27f160eba211a3b745d089dd873f83166634013e7067e12a59da0f0d8ed0c600c01c2d05b8beb19209efd787ec7944e35228c1ad75a428ffc22841e19e4c5055d9058994ee9320a9fa8abd5a9813359287eb6dd3975f624ce878771312c734cbd4b773103f2fff076d381003b55a8b2c1ffcf3244bd4f9f7ec1f6f5e398531ee203a1f22be7b6fdeaf98efacb6f1a289ad53e91af4dbcd7f6f8718d85e528eb7e061989e4f372dc27fc49e05ed23e8bcf234d209ace316f803c3388f298b448db01c992349fa784545fc49f0d39cedd61ffe94a022e5dbc60d4cbee6e19724381bceba6e1c66dcc8d0098b3ebe5afb43df909e61e65c8515af864916292671f3c128ec72a04ba6485a6952996e73ddb21178f03d459ba604a8cdc31e4be682f8c4b65684a9ea8451c43171da9c5e23873290aa337cd7c1bae9a7f4421c7663d07334ad05d21a0c7a368ea0aacfd268096b342581a740a1b454c418621887769d7eab0fc9eebd65c0fec05403f2b8fc32f165bcc3ca28627076c5989a0f1cc8b4b87cea9ac4e99900f895c552b98e620630cf05d90db8ebe96f594cc0b432daae775d2b2be4618a154a92ca688a70650ff22f754c9294f1252015499b466df4a60f0a6efdda5a49525ab9e4f2329af25b388ef5f3368452198f05cfb5c190adb23e05ddaca4798488f5580baa494339b81908035c101da796df09692a089d800e7d73f829ce38711cdb2e7e1fcf5fb7df79e00c81cc68498da2cedc3e10d9c7dc2a6b0fec17eccd6d38d73a90fadae7d9314f516f841f8094a02be9095777317b55ef27bf8b4493d8d0293f697633f0647a89e8a94dd04ec8516b5f8adbaa01ac4d91798ad65d77724b24a6c4e3eecb8344358ff3c9c7bd0342f0f5b7275a58ab786bba4381da57d8471da071ad59504df3acc6ea7e1bbe18701c2190aeb9fa89f588198cd802c53efc942caf2bfcd620c3717d7f9444c37f33248bf546bf10c240c43e8119ed6d4769a7a78439954531c614a203c76f47ac1e124926f8f5e8b546a68c850fbe3d5c46c657c243cb4283719173df498857882dc2668b880b348ff5d370ab1cd683548c60c16c6036d25f72a017dea825d9cfdb571d14fecf19cf750e57922f9e5a9336be8d22ffcb4e554a2aedb954fbf9a275153c0355605e2e35f35d2c39cb09f02e8b26ec54ed189f4650fd51c3ff0cc677f6ae929ea9e454e0cf4bd13c45aa8337a7fd5991e3aac5465b03bb4e7e37e8571d44ea474db0bd02506b0cb6399f8eb3da141ebd50933260043d5d9abbcd9315049c83059cbd2a3f594422907176dfb286c8cb22adc4533c85171d7ceb3450588b057773455df5bceedd0e7a3d841ae912475b7f6fd61c2e897a990a17368ccd86e89ef83ee69e39097793ec2810a10ac75cb7730064dfa623b1f2c74a3a2818d62ecbf99024508d0136c0a28ac3366036f0f19a6b16a806067b239666d8cd92b74c5dbe5e8deda0c5dcead199ed254131dc8e58fe7e87261c509e13918a62a1347c3190b3ff477cf4a07f493bef0180463b0fe331b1d9a9683da27c34608b1e4aa5c3d9c9ce09926737769d94cc66edebd331e89c140ea315f6798223432c8b1ad5d4ed44de3a83effbd510c3c0c54c39b30fe5b5cc7467ad9a610305e26a879c1a194ef34e43a3ffbd02da013a476b97bcf17035246a4e6c8beca4fb2fd1770494e8db1c0bbb99c06ef8d24ecf304840290b1d5a6fc728c793c2b708f3ddcad59bd8a94edf7a7febf702745e1d7b6abdcc9a36a7bd75a922652cfea291b14114e112c9c073587add4958b64475b7ddaf82958ddb70aeb7155f9c182fd38be38de2510b6dd09ce98dae8764f5559bdd4fbcbe29546a2929106751f13d9bac6f6ddf89309e04e76d774c97495f9d98eb2d30a840e6d1f3f64d2d274834dc9e1573fff1bc2e1dbd22be6afc51c88e0efe954d114ddb109a0f2b9fb9e4f01fcd830e2af00161669bd6b3663caf3001636c35cfe16a192bac02c5aee0511896281b8b0393dd675e69167b61305ad1c4972b8a049fdc7ec56e257e571903e20b24cdbe07e00"}"#).unwrap();
        let res = broadcast_tx(&mut connection, &signed_tx);
        match res {
            Err(WalletError::Error(e)) => {
                let e = format!("{:?}", e);
                println!("{}", e);
                assert!(e.contains("tx-expiring"))
            }
            _ => {}
        }
    }
}
