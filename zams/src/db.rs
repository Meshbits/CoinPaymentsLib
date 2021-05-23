use crate::models::{
    Account, NewAccount, NewBlock, NewNote, NewTransaction, NewViewingKey, ViewingKey,
};
use crate::schema::viewing_keys::dsl::viewing_keys;
use crate::zcashdrpc::{Block as RpcBlock, Transaction as RpcTx};
use anyhow::Context;
use diesel::dsl::*;
use diesel::pg::upsert::excluded;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use hex::decode;
use zcash_primitives::zip32::DiversifierIndex;

pub fn establish_connection(database_url: &str) -> PgConnection {
    PgConnection::establish(&database_url).expect(&format!("Error connecting to {}", database_url))
}

pub fn save_block(block: &RpcBlock, connection: &PgConnection) -> anyhow::Result<i32> {
    use crate::schema::blocks::columns::{hash, id};
    let block = NewBlock {
        height: block.height as i32,
        anchor: decode(&block.anchor)?,
        hash: decode(&block.hash)?,
        prevhash: decode(&block.previousblockhash)?,
    };

    let block_id: i32 = diesel::insert_into(crate::schema::blocks::table)
        .values(&block)
        .on_conflict(hash)
        .do_update()
        .set(hash.eq(excluded(hash)))
        .returning(id)
        .get_result(connection)
        .expect("Error saving block");
    Ok(block_id)
}

pub fn save_transaction(
    tx: &RpcTx,
    block_id: i32,
    connection: &PgConnection,
) -> anyhow::Result<i32> {
    use crate::schema::transactions::columns::{txhash, id};
    let tx = NewTransaction {
        block_id,
        txhash: decode(&tx.txid)?,
    };

    let tx_id = diesel::insert_into(crate::schema::transactions::table)
        .values(&tx)
        .on_conflict(txhash)
        .do_update()
        .set(txhash.eq(excluded(txhash)))
        .returning(id)
        .get_result(connection)
        .expect("Error saving tx");
    Ok(tx_id)
}

pub fn save_viewing_key(viewing_key: &str, connection: &PgConnection) -> anyhow::Result<i32> {
    use crate::schema::viewing_keys::columns::{id, key};

    let ivk = NewViewingKey {
        key: viewing_key.to_string(),
    };
    let viewing_key_id = diesel::insert_into(crate::schema::viewing_keys::table)
        .values(&ivk)
        .on_conflict(key)
        .do_update()
        .set(key.eq(excluded(key)))
        .returning(id)
        .get_result(connection)
        .expect("Error saving viewing key");
    Ok(viewing_key_id)
}

pub fn save_account(account: &NewAccount, connection: &PgConnection) -> anyhow::Result<()> {
    diesel::insert_into(crate::schema::accounts::table)
        .values(account)
        .on_conflict_do_nothing()
        .execute(connection)
        .expect("Error saving account/address");
    Ok(())
}

pub fn save_note(note: &NewNote, connection: &PgConnection) -> anyhow::Result<()> {
    diesel::insert_into(crate::schema::notes::table)
        .values(note)
        .on_conflict_do_nothing()
        .execute(connection)
        .expect("Error saving note");
    Ok(())
}

pub fn read_ivks(connection: &PgConnection) -> anyhow::Result<Vec<ViewingKey>> {
    let results: Vec<ViewingKey> = viewing_keys.load::<ViewingKey>(connection)?;
    Ok(results)
}

pub fn make_new_account(
    address: &str,
    viewing_key_id: Option<i32>,
    diversifier: Option<DiversifierIndex>,
    user_id: Option<i32>,
) -> NewAccount {
    let di = diversifier.map(|d| {
        let mut bytes = [0u8; 16];
        bytes[..11].copy_from_slice(&d.0);
        let di = u128::from_le_bytes(bytes);
        let hi = (di << 64) as i64;
        let lo = di as i64; // truncate
        (hi, lo)
    });
    NewAccount {
        address: address.to_string(),
        viewing_key_id,
        diversifier_index_high: di.map(|d| d.0),
        diversifier_index_low: di.map(|d| d.1),
        user_id,
    }
}

pub trait NoteAdaptable {
    fn put(&self, note: &NewNote) -> anyhow::Result<i32>;
}

pub struct NoteAdapter {
    connection: PgConnection,
}

impl NoteAdapter {
    pub fn new(database_url: &str) -> NoteAdapter {
        let connection = establish_connection(database_url);
        NoteAdapter { connection }
    }
}

impl NoteAdaptable for NoteAdapter {
    fn put(&self, note: &NewNote) -> anyhow::Result<i32> {
        use crate::schema::notes::columns::id;
        let note_id = diesel::insert_into(crate::schema::notes::table)
            .values(note)
            .on_conflict_do_nothing()
            .returning(id)
            .get_result(&self.connection)?;
        Ok(note_id)
    }
}

pub trait AddressBook {
    fn contains(&self, address: &str) -> anyhow::Result<bool>;
}

struct TransparentAddressBook {
    connection: PgConnection,
}

impl TransparentAddressBook {
    pub fn new(connection: PgConnection) -> TransparentAddressBook {
        TransparentAddressBook { connection }
    }
}

impl AddressBook for TransparentAddressBook {
    fn contains(&self, addr: &str) -> anyhow::Result<bool> {
        use crate::schema::accounts::dsl::*;

        select(exists(accounts.filter(address.eq(addr))))
            .get_result(&self.connection)
            .context("db error")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_save_block_tx_note() {
        let connection = establish_connection("postgres://hanh@localhost/zamsdb");
        let block = RpcBlock {
            hash: "0000000001c55378be4d0cc4f74ef6ff1bdc558f95f00bd9677d2ed49867bc98".to_string(),
            height: 1255004,
            anchor: "ad0c5cf26bb4d94571387f95fb7c1eab535bc2adbac4b3aad2496505515dc2f6".to_string(),
            previousblockhash: "000000000073a2e6f7b9c730e8293ea00786dc73b5bafb438708091fe6625b30"
                .to_string(),
            time: 1621434320,
            tx: vec![],
        };
        let id = save_block(&block, &connection).unwrap();

        let tx = RpcTx {
            txid: "3132d3d8006c94f3385606d3f5aa7a6f49d779a82f599eefcc16290ef448b12c".to_string(),
            height: Some(1255004),
            vin: vec![],
            vout: vec![],
            vShieldedSpend: vec![],
            vShieldedOutput: vec![],
        };
        let tx_id = save_transaction(&tx, id, &connection).unwrap();

        let note = NewNote {
            tx_id,
            vout_index: 0,
            value: 1000000,
            address: "tmEuJYrkbLTnRSPJJtEuybJHnHxRJ56aNAz".to_string(),
            shielded: false,
            locked: false,
            spent: false,
        };

        save_note(&note, &connection).unwrap();
    }

    #[test]
    pub fn test_save_account() {
        let connection = establish_connection("postgres://hanh@localhost/zamsdb");
        let account = NewAccount {
            address: "ztestsapling1uleg00fxnx67pyf5jjfhx2t5f025rll6se4vutwr7qxav4894xv623vrwf3z6x2kt5d4wn7ywjc".to_string(),
            viewing_key_id: None,
            diversifier_index_high: None,
            diversifier_index_low: None,
            user_id: None
        };
        save_account(&account, &connection).unwrap();

        let viewing_key_id = save_viewing_key("zxviewtestsapling1qw0h9kqpqqqqpqrv5epemex9kkduyzyqqasg8jdfskqfyrdttg4shvuzkcdwld2g5e6vlmx5jvcjctfentdjsfhmrj82ku7t874n3n4tc2g92j4gx3yy3udzu42vywl89rgnkhflqtqn4emtxayskst7aputd3hls4qrf9s3vt5a7qa6g5k4msg5nypkvq2cpc9f8nxau987syuqkygm3v9v0umra4hvjzlzzvtr23lha7ftsrcr66wh2jtuuh5w4jf8x2ppf7j9suqfaumqy", &connection).unwrap();
        let account = NewAccount {
            address: "ztestsapling12hqwav6cu8zs4pd7gdwpxyccv6kuydvsxy92dzs28nzv2ccnctxh7fhdf8xmz0ky98lmcmflj9g".to_string(),
            viewing_key_id: Some(viewing_key_id),
            diversifier_index_high: Some(0),
            diversifier_index_low: Some(2),
            user_id: Some(14)
        };
        save_account(&account, &connection).unwrap();
    }
}
