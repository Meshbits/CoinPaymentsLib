use crate::error::WalletError;
use crate::wallet::to_spendable_note;
use crate::wallet::transaction::{Account, UTXO, SpendableNoteWithId};
use anyhow::anyhow;
use postgres::{Client, GenericClient, Statement};
use std::cell::RefCell;
use std::cmp;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::time::SystemTime;
use zcash_client_backend::data_api::wallet::ANCHOR_OFFSET;
use zcash_client_backend::encoding::{decode_extended_full_viewing_key, encode_payment_address};
use zcash_client_backend::wallet::SpendableNote;
use zcash_primitives::consensus::BlockHeight;
use zcash_primitives::constants::testnet::{
    HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, HRP_SAPLING_PAYMENT_ADDRESS,
};
use zcash_primitives::zip32::DiversifierIndex;

pub struct DbPreparedStatements {
    pub stmt_select_sapling_notes: Statement,
    pub stmt_select_trp_notes: Statement,
    pub upsert_spent_utxo: Statement,
}

impl DbPreparedStatements {
    pub fn prepare(client: Rc<RefCell<Client>>) -> crate::Result<DbPreparedStatements> {
        let mut c = client.borrow_mut();
        Ok(DbPreparedStatements {
            stmt_select_sapling_notes: c.prepare(
                "SELECT id_note, diversifier, value, rcm, witness
                FROM received_notes
                INNER JOIN transactions ON transactions.id_tx = received_notes.tx
                INNER JOIN sapling_witnesses ON sapling_witnesses.note = received_notes.id_note
                WHERE address = $1
                AND spent IS NULL
                AND payment IS NULL
                AND transactions.block <= $2
                AND sapling_witnesses.block = $2",
            )?,
            stmt_select_trp_notes: c.prepare("SELECT id_utxo, tx_hash, output_index, value, script FROM utxos WHERE address = $1 AND NOT spent AND payment IS NULL")?,
            upsert_spent_utxo: c.prepare(
                "INSERT INTO utxos(tx_hash, address, output_index, value, script,
                height, spent, spent_height)
                VALUES($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (tx_hash, output_index) DO UPDATE SET
                spent = excluded.spent,
                spent_height = excluded.spent_height",
            )?,
        })
    }
}

pub fn load_checkpoint<C: GenericClient>(
    c: &mut C,
    height: u32,
    hash: &[u8],
    time: i32,
    sapling_tree: &[u8],
) -> crate::Result<()> {
    c.execute(
        "INSERT INTO blocks(height, hash, time, sapling_tree)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (height) DO UPDATE SET
            hash = excluded.hash,
            time = excluded.time,
            sapling_tree = excluded.sapling_tree",
        &[&(height as i32), &hash, &time, &sapling_tree],
    )?;
    Ok(())
}

pub fn import_fvk<C: GenericClient>(c: &mut C, fvk: &str) -> crate::Result<i32> {
    let row = c.query_one(
        "INSERT INTO fvks(extfvk) VALUES ($1)
            ON CONFLICT (extfvk) DO UPDATE SET
            extfvk = excluded.extfvk
            RETURNING id_fvk",
        &[&fvk],
    )?;
    let id_fvk: i32 = row.get(0);
    Ok(id_fvk)
}

pub fn import_address<C: GenericClient>(c: &mut C, address: &str) -> crate::Result<i32> {
    let row = c.query_one(
        "INSERT INTO accounts(fvk, address) VALUES (NULL, $1)
            ON CONFLICT (address) DO UPDATE SET
            address = excluded.address
            RETURNING account",
        &[&address],
    )?;
    let account: i32 = row.get(0);
    Ok(account)
}

pub fn generate_keys<C: GenericClient>(
    c: &mut C,
    id_fvk: i32,
    diversifier_index: u128,
) -> std::result::Result<(String, u128), WalletError> {
    let row = c.query_one("SELECT extfvk FROM fvks WHERE id_fvk = $1", &[&id_fvk])?;
    let key: String = row.get(0);
    let fvk = decode_extended_full_viewing_key(HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, &key)
        .map_err(WalletError::Bech32)?
        .ok_or(WalletError::IncorrectHrpExtFvk)?;
    let mut di = DiversifierIndex::new();
    di.0.copy_from_slice(&u128::to_le_bytes(diversifier_index)[..11]);
    di.increment()
        .map_err(|_| anyhow::anyhow!("Out of diversifier indexes"))?;
    let (di, pa) = fvk
        .address(di)
        .map_err(|_| anyhow!("Invalid diversifier"))?;
    let address = encode_payment_address(HRP_SAPLING_PAYMENT_ADDRESS, &pa);
    let mut di_bytes = [0u8; 16];
    di_bytes[..11].copy_from_slice(&di.0);
    let diversifier_index_out = u128::from_le_bytes(di_bytes);

    let row = c.query_one(
        "INSERT INTO accounts(fvk, address)
            VALUES ($1, $2)
            ON CONFLICT (address) DO UPDATE SET
            fvk = excluded.fvk RETURNING account",
        &[&id_fvk, &address],
    )?;
    let account: i32 = row.get(0);

    Ok((address, diversifier_index_out))
}

pub fn get_spendable_notes_by_address<C: GenericClient>(
    c: &mut C,
    s: &DbPreparedStatements,
    address: &str,
    anchor_height: u32,
) -> Result<Vec<SpendableNoteWithId>, WalletError> {
    // Select notes
    let notes = c.query(
        &s.stmt_select_sapling_notes,
        &[&address, &(anchor_height as i32)],
    )?;
    let notes: Vec<_> = notes.iter().map(to_spendable_note).collect();
    notes.into_iter().collect()
}

pub fn get_spendable_transparent_notes_by_address<C: GenericClient>(
    c: &mut C,
    s: &DbPreparedStatements,
    address: &str,
) -> crate::Result<Vec<UTXO>> {
    let rows = c
        .query(&s.stmt_select_trp_notes, &[&address])
        .map_err(WalletError::Postgres)?;
    let notes: Vec<_> = rows
        .iter()
        .map(|row| {
            let id: i32 = row.get(0);
            let tx_hash: Vec<u8> = row.get(1);
            let output_index: i32 = row.get(2);
            let value: i64 = row.get(3);
            let script_hex: Vec<u8> = row.get(4);
            UTXO {
                id,
                amount: value as u64,
                tx_hash: hex::encode(&tx_hash),
                output_index,
                hex: hex::encode(&script_hex),
                spent: false,
            }
        })
        .collect();
    Ok(notes)
}

pub fn get_account<C: GenericClient>(c: &mut C, id: i32) -> crate::Result<Account> {
    let row = c.query_one("SELECT a.address, f.extfvk FROM accounts a LEFT JOIN fvks f ON a.fvk = f.id_fvk WHERE a.account = $1", &[&id]).map_err(WalletError::Postgres)?;
    let address: String = row.get(0);
    let fvk: Option<String> = row.get(1);
    Ok(match fvk {
        Some(fvk) => Account::Shielded(address, fvk),
        None => Account::Transparent(address),
    })
}

pub fn get_all_trp_addresses<C: GenericClient>(c: &mut C) -> crate::Result<Vec<(i32, String)>> {
    let row = c
        .query(
            "SELECT account, address FROM accounts WHERE fvk IS NULL",
            &[],
        )
        .map_err(WalletError::Postgres)?;
    Ok(row
        .iter()
        .map(|row| {
            let id: i32 = row.get(0);
            let address: String = row.get(1);
            (id, address)
        })
        .collect())
}

pub fn get_target_and_anchor_heights<C: GenericClient>(
    c: &mut C,
) -> crate::Result<Option<(BlockHeight, BlockHeight)>> {
    block_height_extrema(c).map(|heights| {
        heights.map(|(min_height, max_height)| {
            let target_height = max_height + 1;

            // Select an anchor ANCHOR_OFFSET back from the target block,
            // unless that would be before the earliest block we have.
            let anchor_height = BlockHeight::from(cmp::max(
                u32::from(target_height).saturating_sub(ANCHOR_OFFSET),
                u32::from(min_height),
            ));

            (target_height, anchor_height)
        })
    })
}

pub fn block_height_extrema<C: GenericClient>(
    c: &mut C,
) -> crate::Result<Option<(BlockHeight, BlockHeight)>> {
    let row = c.query_one("SELECT MIN(height), MAX(height) FROM blocks", &[])?;

    let min_height: Option<i32> = row.get(0);
    let max_height: Option<i32> = row.get(1);
    let r = match (min_height, max_height) {
        (Some(min_height), Some(max_height)) => Some((
            BlockHeight::from(min_height as u32),
            BlockHeight::from(max_height as u32),
        )),
        _ => None,
    };
    Ok(r)
}

#[derive(Debug)]
pub struct Payment {
    pub datetime: SystemTime,
    pub account: i32,
    pub sender: String,
    pub recipient: String,
    pub change: String,
    pub amount: i64,
    pub paid: bool,
}

pub fn get_payment<C: GenericClient>(
    client: &mut C,
    id_payment: i32) -> crate::Result<Payment> {
    let row = client.query_one("SELECT datetime, account, sender, recipient,
        change, amount, paid FROM payments WHERE id_payment = $1", &[&id_payment])?;
    let datetime: SystemTime = row.get(0);
    let account: i32 = row.get(1);
    let sender: String = row.get(2);
    let recipient: String = row.get(3);
    let change: String = row.get(4);
    let amount: i64 = row.get(5);
    let paid: bool = row.get(6);
    Ok(Payment {
        datetime, account, sender, recipient, change, amount, paid,
    })
}

pub fn store_payment<C: GenericClient>(
    client: &mut C,
    datetime: SystemTime,
    account: i32,
    sender: &str,
    recipient: &str,
    change: &str,
    amount: i64,
    notes: &[i32],
    utxos: &[i32]
) -> crate::Result<i32> {
    let row = client.query_one("INSERT INTO payments(datetime, account, sender, recipient,
        change, amount, paid) VALUES ($1, $2, $3, $4, $5, $6, FALSE)
        RETURNING id_payment", &[
        &datetime,
        &account,
        &sender,
        &recipient,
        &change,
        &amount
    ])?;
    let id: i32 = row.get(0);
    for utxo in utxos.iter() {
        client.execute("UPDATE utxos SET payment = $1 WHERE id_utxo = $2", &[&id, &utxo])?;
    }
    for note in notes.iter() {
        client.execute("UPDATE received_notes SET payment = $1 WHERE id_note = $2", &[&id, &note])?;
    }
    Ok(id)
}

pub fn mark_paid<C: GenericClient>(client: &mut C, id_payment: i32, txid: &str) -> crate::Result<()> {
    client.execute("UPDATE payments SET paid = TRUE, txid = $2 WHERE id_payment = $1", &[&id_payment, &txid])?;
    Ok(())
}

pub fn cancel_payment<C: GenericClient>(client: &mut C, id_payment: i32) -> crate::Result<()> {
    client.execute("UPDATE payments SET paid = FALSE WHERE id_payment = $1", &[&id_payment])?;
    client.execute("UPDATE utxos SET payment = NULL WHERE payment = $1", &[&id_payment])?;
    client.execute("UPDATE received_notes SET payment = NULL WHERE payment = $1", &[&id_payment])?;
    Ok(())
}

pub fn trp_rewind_to_height<C: GenericClient>(
    client: &mut C,
    height: u32,
) -> Result<(), WalletError> {
    client.execute("DELETE FROM utxos WHERE height > $1", &[&(height as i32)])?;
    client.execute(
        "UPDATE utxos set spent = FALSE, spent_height = NULL WHERE spent_height > $1",
        &[&(height as i32)],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::CONNECTION_STRING;
    use postgres::{NoTls, Client};
    use super::*;

    #[test]
    fn test_payment() {
        let mut client = Client::connect(CONNECTION_STRING, NoTls).unwrap();
        let now = SystemTime::now();
        store_payment(&mut client, now, 1,
        "ztestsapling10xueewxz53j8kp5sdd79uk5ffsgshukkauyxduscu86zjp778xyavmqftz87pcs2zexzxyclmwn",
        "tmVTzUmRp4mNb8jSF8qUs2P39gM8oGZ4zo8",
        "tmVTzUmRp4mNb8jSF8qUs2P39gM8oGZ4zo8",
        100_000,
        &[1, 2],
        &[1]
        ).unwrap();
    }

    #[test]
    fn test_mark_paid() {
        let mut client = Client::connect(CONNECTION_STRING, NoTls).unwrap();
        mark_paid(&mut client, 2, "").unwrap();
    }

    #[test]
    fn test_cancel_paid() {
        let mut client = Client::connect(CONNECTION_STRING, NoTls).unwrap();
        cancel_payment(&mut client, 2).unwrap();
    }

    #[test]
    fn test_get_payment() {
        let mut client = Client::connect(CONNECTION_STRING, NoTls).unwrap();
        let p = get_payment(&mut client, 2).unwrap();
        println!("{:?}", p);
    }
}
