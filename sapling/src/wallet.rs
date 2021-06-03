use crate::error::WalletError;
use crate::wallet::shielded_output::ShieldedOutput;
use crate::CONNECTION_STRING;
use anyhow::anyhow;
use ff::PrimeField;
use postgres::types::ToSql;
use postgres::{Client, NoTls, Row, Statement, GenericClient};
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;
use zcash_client_backend::address::RecipientAddress;
use zcash_client_backend::data_api::{
    PrunedBlock, ReceivedTransaction, SentTransaction, WalletRead, WalletWrite,
};
use zcash_client_backend::encoding::{
    decode_extended_full_viewing_key, decode_payment_address, encode_extended_full_viewing_key,
    encode_payment_address,
};
use zcash_client_backend::wallet::{AccountId, SpendableNote, WalletTx};
use zcash_client_backend::DecryptedOutput;
use zcash_primitives::block::BlockHash;
use zcash_primitives::consensus::{BlockHeight, Network, NetworkUpgrade, Parameters};
use zcash_primitives::constants::testnet::{
    HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, HRP_SAPLING_PAYMENT_ADDRESS,
};
use zcash_primitives::memo::{Memo, MemoBytes};
use zcash_primitives::merkle_tree::{CommitmentTree, IncrementalWitness};
use zcash_primitives::sapling::{Diversifier, Node, Nullifier, PaymentAddress, Rseed};
use zcash_primitives::transaction::components::Amount;
use zcash_primitives::transaction::{Transaction, TxId};
use zcash_primitives::zip32::{DiversifierIndex, ExtendedFullViewingKey};
use zcash_primitives::consensus;
use crate::wallet::transaction::{UTXO, Account};
use std::ops::DerefMut;

pub mod fvk;
pub mod scan;
pub mod shielded_output;
pub mod transaction;

pub struct PostgresWallet {
    pub connection: Arc<RefCell<Client>>,
    stmt_insert_block: Statement,

    stmt_upsert_tx_meta: Statement,
    stmt_upsert_tx_data: Statement,

    stmt_mark_received_note_spent: Statement,
    stmt_upsert_received_note: Statement,

    stmt_upsert_sent_note: Statement,

    stmt_insert_witness: Statement,
    stmt_prune_witnesses: Statement,
    stmt_update_expired: Statement,
}

impl PostgresWallet {
    pub fn new() -> Result<PostgresWallet, WalletError> {
        let connection = Client::connect(CONNECTION_STRING, NoTls).unwrap();
        let c = Arc::new(RefCell::new(connection));
        let mut connection = c.borrow_mut();
        Ok(PostgresWallet {
            connection: c.clone(),
            stmt_insert_block: connection.prepare(
                "INSERT INTO blocks (height, hash, time, sapling_tree)
                    VALUES ($1, $2, $3, $4)",
            )?,
            stmt_upsert_tx_meta: connection.prepare(
                "INSERT INTO transactions (txid, block, tx_index)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (txid) DO UPDATE SET block = excluded.block, tx_index = excluded.tx_index
                    RETURNING id_tx",
            )?,
            stmt_upsert_tx_data: connection.prepare(
                "INSERT INTO transactions (txid, created, expiry_height, raw)
                    VALUES ($1, $2, $3, $4)
                    ON CONFLICT (txid) DO UPDATE SET created = excluded.created, expiry_height = excluded.expiry_height,
                    raw = excluded.raw RETURNING id_tx",
            )?,
            stmt_mark_received_note_spent: connection.prepare(
                "UPDATE received_notes SET spent = $1 WHERE nf = $2"
            )?,
            stmt_upsert_received_note: connection.prepare(
                "INSERT INTO received_notes (tx, output_index, account, address, diversifier, value, rcm, memo, nf, is_change)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                    ON CONFLICT (tx, output_index) DO UPDATE
                    SET account = excluded.account,
                        address = excluded.address,
                        diversifier = excluded.diversifier,
                        value = excluded.value,
                        rcm = excluded.rcm,
                        nf = coalesce(excluded.nf, received_notes.nf),
                        memo = coalesce(excluded.memo, received_notes.memo),
                        is_change = coalesce(excluded.is_change, received_notes.is_change)
                    RETURNING id_note",
            )?,
            stmt_upsert_sent_note: connection.prepare(
                "INSERT INTO sent_notes (tx, output_index, from_account, address, value, memo)
                    VALUES ($1, $2, $3, $4, $5, $6)
                    ON CONFLICT (tx, output_index) DO UPDATE SET
                    from_account = excluded.from_account,
                    address = excluded.address,
                    value = excluded.value,
                    memo = excluded.memo RETURNING id_note",
            )?,
            stmt_insert_witness: connection.prepare(
                "INSERT INTO sapling_witnesses (note, block, witness)
                    VALUES ($1, $2, $3)",
            )?,
            stmt_prune_witnesses: connection.prepare(
                "DELETE FROM sapling_witnesses WHERE block < $1"
            )?,
            stmt_update_expired: connection.prepare(
                "UPDATE received_notes SET spent = NULL WHERE EXISTS (
                        SELECT id_tx FROM transactions
                        WHERE id_tx = received_notes.spent AND block IS NULL AND expiry_height < $1
                    )",
            )?,
        })
    }

    pub fn load_checkpoint(
        &self,
        height: u32,
        hash: &[u8],
        time: i32,
        sapling_tree: &[u8],
    ) -> Result<(), WalletError> {
        let mut client = self.connection.borrow_mut();
        let mut db_tx = client.transaction()?;
        db_tx.execute(
            "INSERT INTO blocks(height, hash, time, sapling_tree)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (height) DO UPDATE SET
            hash = excluded.hash,
            time = excluded.time,
            sapling_tree = excluded.sapling_tree",
            &[&(height as i32), &hash, &time, &sapling_tree],
        )?;
        update_chain_tip(&mut db_tx, height)?;
        db_tx.commit()?;
        Ok(())
    }

    pub fn import_fvk(&self, fvk: &str) -> Result<i32, WalletError> {
        let row = self.connection.borrow_mut().query_one(
            "INSERT INTO fvks(extfvk) VALUES ($1)
            ON CONFLICT (extfvk) DO UPDATE SET
            extfvk = excluded.extfvk
            RETURNING id_fvk",
            &[&fvk],
        )?;
        Ok(row.get(0))
    }

    pub fn import_address(&self, address: &str) -> Result<i32, WalletError> {
        let row = self.connection.borrow_mut().query_one(
            "INSERT INTO accounts(fvk, address) VALUES (NULL, $1)
            ON CONFLICT (address) DO UPDATE SET
            address = excluded.address
            RETURNING account",
            &[&address],
        )?;
        Ok(row.get(0))
    }

    pub fn generate_keys(
        &self,
        id_fvk: i32,
        diversifier_index: u128,
    ) -> std::result::Result<(String, u128), WalletError> {
        let row = self
            .connection
            .borrow_mut()
            .query_one("SELECT extfvk FROM fvks WHERE id_fvk = $1", &[&id_fvk])?;
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

        let row = self.connection.borrow_mut().query_one(
            "INSERT INTO accounts(fvk, address)
            VALUES ($1, $2)
            ON CONFLICT (address) DO UPDATE SET
            fvk = excluded.fvk RETURNING account",
            &[&id_fvk, &address],
        )?;
        let account: i32 = row.get(0);

        Ok((address, diversifier_index_out))
    }

    pub fn get_spendable_notes_by_address(
        &self,
        address: &str,
        anchor_height: BlockHeight,
    ) -> Result<Vec<SpendableNote>, WalletError> {
        let mut client = self.connection.borrow_mut();
        let stmt_select_notes = client.prepare(
            "SELECT diversifier, value, rcm, witness
            FROM received_notes
            INNER JOIN transactions ON transactions.id_tx = received_notes.tx
            INNER JOIN sapling_witnesses ON sapling_witnesses.note = received_notes.id_note
            WHERE address = $1
            AND spent IS NULL
            AND transactions.block <= $2
            AND sapling_witnesses.block = $2",
        )?;

        // Select notes
        let notes = client.query(
            &stmt_select_notes,
            &[&address, &(u32::from(anchor_height) as i32)],
        )?;
        let notes: Vec<_> = notes.iter().map(to_spendable_note).collect();
        notes.into_iter().collect()
    }

    pub fn get_spendable_transparent_notes_by_address(&self, address: &str) -> Result<Vec<UTXO>, WalletError> {
        let mut client = self.connection.borrow_mut();
        let rows = client.query("SELECT tx_hash, output_index, value, script FROM utxos WHERE address = $1 AND not spent", &[&address]).map_err(WalletError::Postgres)?;
        let notes: Vec<_> = rows.iter().map(|row| {
            let tx_hash: Vec<u8> = row.get(0);
            let output_index: i32 = row.get(1);
            let value: i64 = row.get(2);
            let script_hex: Vec<u8> = row.get(3);
            UTXO {
                amount: value as u64,
                tx_hash: hex::encode(&tx_hash),
                output_index,
                hex: hex::encode(&script_hex),
                spent: false
            }
        }).collect();
        Ok(notes)
    }

    pub fn get_chain_tip(&self) -> Result<Option<i32>, WalletError> {
        let mut client = self.connection.borrow_mut();
        let row = client.query_opt("SELECT height FROM chaintip WHERE id = 0", &[]).map_err(WalletError::Postgres)?;

        let height = row.map(|row| row.get::<_, i32>(0));
        Ok(height)
    }

    pub fn get_account(&self, id: i32) -> Result<Account, WalletError> {
        let mut client = self.connection.borrow_mut();
        let row = client.query_one("SELECT a.address, f.extfvk FROM accounts a LEFT JOIN fvks f ON a.fvk = f.id_fvk WHERE a.account = $1", &[&id]).map_err(WalletError::Postgres)?;
        let address: String = row.get(0);
        let fvk: Option<String> = row.get(1);
        Ok(match fvk {
            Some(fvk) => Account::Shielded(address, fvk),
            None => Account::Transparent(address),
        })
    }

    pub fn get_all_trp_addresses(&self) -> Result<Vec<(i32, String)>, WalletError> {
        let mut client = self.connection.borrow_mut();
        let row = client.query("SELECT account, address FROM accounts WHERE fvk IS NULL", &[]).map_err(WalletError::Postgres)?;
        Ok(row.iter().map(|row| {
            let id: i32 = row.get(0);
            let address: String = row.get(1);
            (id, address)
        }).collect())
    }
}

pub fn update_chain_tip<C: GenericClient>(client: &mut C, height: u32) -> Result<(), WalletError> {
    client.execute("INSERT INTO chaintip(id, height) VALUES(0, $1)
        ON CONFLICT (id) DO UPDATE SET height = excluded.height", &[&(height as i32)]).map_err(WalletError::Postgres)?;
    Ok(())
}

struct WalletDbTransaction<'a> {
    statements: &'a PostgresWallet,
    transaction: postgres::Transaction<'a>,
}

impl<'a> WalletDbTransaction<'a> {
    pub fn insert_block(
        &mut self,
        block_height: BlockHeight,
        block_hash: BlockHash,
        block_time: u32,
        commitment_tree: &CommitmentTree<Node>,
    ) -> Result<(), WalletError> {
        let client = &mut self.transaction;
        let mut encoded_tree = Vec::new();
        commitment_tree.write(&mut encoded_tree).unwrap();

        client.execute(
            &self.statements.stmt_insert_block,
            &[
                &(u32::from(block_height) as i32),
                &block_hash.0.to_vec(),
                &(block_time as i32),
                &encoded_tree,
            ],
        )?;

        Ok(())
    }

    pub fn put_tx_meta(
        &mut self,
        tx: &WalletTx<Nullifier>,
        height: BlockHeight,
    ) -> Result<i32, WalletError> {
        let txid = tx.txid.0.to_vec();
        let row = self.transaction.query_one(
            &self.statements.stmt_upsert_tx_meta,
            &[&txid, &(u32::from(height) as i32), &(tx.index as i32)],
        )?;
        Ok(row.get(0))
    }

    pub fn mark_spent(&mut self, tx_ref: i32, nf: &Nullifier) -> Result<(), WalletError> {
        self.transaction.execute(
            &self.statements.stmt_mark_received_note_spent,
            &[&tx_ref, &&nf.0[..]],
        )?;
        Ok(())
    }

    pub fn update_expired_notes(&mut self, height: BlockHeight) -> Result<(), WalletError> {
        self.transaction.execute(
            &self.statements.stmt_update_expired,
            &[&(u32::from(height) as i32)],
        )?;
        Ok(())
    }

    pub fn prune_witnesses(&mut self, below_height: BlockHeight) -> Result<(), WalletError> {
        self.transaction.execute(
            &self.statements.stmt_prune_witnesses,
            &[&(u32::from(below_height) as i32)],
        )?;
        Ok(())
    }

    pub fn insert_witness(
        &mut self,
        note_id: i32,
        witness: &IncrementalWitness<Node>,
        height: BlockHeight,
    ) -> Result<(), WalletError> {
        let mut encoded = Vec::new();
        witness.write(&mut encoded).unwrap();

        self.transaction.execute(
            &self.statements.stmt_insert_witness,
            &[&note_id, &(u32::from(height) as i32), &encoded],
        )?;

        Ok(())
    }

    pub fn put_received_note<T: ShieldedOutput>(
        &mut self,
        output: &T,
        tx_ref: i32,
    ) -> Result<i32, WalletError> {
        let rcm = output.note().rcm().to_repr();
        let account = output.account().0 as i32; // account is in fact id_fvk
        let diversifier = output.to().diversifier().0.to_vec();
        let value = output.note().value as i64;
        let rcm = rcm.as_ref();
        let memo = output.memo().map(|m| m.as_slice());
        let is_change = output.is_change();
        let tx = tx_ref;
        let output_index = output.index() as i32;
        let nf_bytes = output.nullifier().map(|nf| nf.0.to_vec());
        let address = encode_payment_address(HRP_SAPLING_PAYMENT_ADDRESS, output.to());
        let row = self.transaction.query_one(
            "SELECT account FROM accounts WHERE address = $1 AND fvk = $2",
            &[&address, &account],
        )?;
        let account: i32 = row.get(0);

        let sql_args: &[&(dyn ToSql + Sync)] = &[
            &tx,
            &output_index,
            &account,
            &address,
            &diversifier,
            &value,
            &rcm,
            &memo,
            &nf_bytes,
            &is_change,
        ];

        self.transaction
            .query_one(&self.statements.stmt_upsert_received_note, sql_args)
            .map(|row| row.get(0))
            .map_err(WalletError::Postgres)
    }

    pub fn put_tx_data(
        &mut self,
        tx: &Transaction,
        created_at: Option<time::OffsetDateTime>,
    ) -> Result<i32, WalletError> {
        let txid = tx.txid().0.to_vec();

        let mut raw_tx = vec![];
        tx.write(&mut raw_tx).map_err(WalletError::IO)?;

        self.transaction
            .query_one(
                &self.statements.stmt_upsert_tx_data,
                &[
                    &txid,
                    &created_at,
                    &(u32::from(tx.expiry_height) as i32),
                    &raw_tx,
                ],
            )
            .map(|row| row.get(0))
            .map_err(WalletError::Postgres)
    }

    pub fn put_sent_decrypted_note(
        &mut self,
        output: &DecryptedOutput,
        tx_ref: i32,
    ) -> Result<i32, WalletError> {
        let output_index = output.index as i32;
        let account = output.account;
        let value = Amount::from_i64(output.note.value as i64).unwrap();
        let memo = &output.memo;

        self.put_sent_note(
            tx_ref,
            output_index,
            account,
            &RecipientAddress::Shielded(output.to.clone()),
            value,
            Some(memo),
        )
    }

    pub fn put_sent_note(
        &mut self,
        tx_ref: i32,
        output_index: i32,
        account: AccountId,
        to: &RecipientAddress,
        value: Amount,
        memo: Option<&MemoBytes>,
    ) -> Result<i32, WalletError> {
        let to_str = to.encode(&Network::TestNetwork);
        let row = self.transaction.query_one(
            "SELECT account FROM accounts WHERE address = $1 AND fvk = $2",
            &[&to_str, &(account.0 as i32)],
        )?;
        let account: i32 = row.get(0);
        self.transaction
            .query_one(
                &self.statements.stmt_upsert_sent_note,
                &[
                    &account,
                    &to_str,
                    &i64::from(value),
                    &memo.map(|m| m.as_slice().to_vec()),
                    &tx_ref,
                    &output_index,
                ],
            )
            .map(|row| row.get(0))
            .map_err(WalletError::Postgres)
    }
}

impl WalletRead for PostgresWallet {
    type Error = WalletError;
    type NoteRef = i32;
    type TxRef = i32;

    fn block_height_extrema(&self) -> Result<Option<(BlockHeight, BlockHeight)>, Self::Error> {
        let row = self
            .connection
            .borrow_mut()
            .query_one("SELECT MIN(height), MAX(height) FROM blocks", &[])?;

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

    fn get_block_hash(&self, block_height: BlockHeight) -> Result<Option<BlockHash>, Self::Error> {
        Ok(self
            .connection
            .borrow_mut()
            .query_opt(
                "SELECT hash FROM blocks WHERE height = $1",
                &[&(u32::from(block_height) as i32)],
            )?
            .map(|row| {
                let row_data = row.get::<_, Vec<_>>(0);
                BlockHash::from_slice(&row_data)
            }))
    }

    fn get_tx_height(&self, txid: TxId) -> Result<Option<BlockHeight>, Self::Error> {
        Ok(self
            .connection
            .borrow_mut()
            .query_opt(
                "SELECT block FROM transactions WHERE txid = $1",
                &[&txid.0.to_vec()],
            )?
            .map(|row| {
                let height: u32 = row.get(0);
                BlockHeight::from_u32(height)
            }))
    }

    fn get_address(&self, account: AccountId) -> Result<Option<PaymentAddress>, Self::Error> {
        let row = self.connection.borrow_mut().query_opt(
            "SELECT address FROM accounts WHERE account = $1",
            &[&account.0],
        )?;
        let row = row.map(|row| {
            let addr: String = row.get(0);
            decode_payment_address(HRP_SAPLING_PAYMENT_ADDRESS, &addr).map_err(WalletError::Bech32)
        });
        row.transpose().map(|r| r.flatten())
    }

    fn get_extended_full_viewing_keys(
        &self,
    ) -> Result<HashMap<AccountId, ExtendedFullViewingKey>, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let stmt_fetch_accounts =
            client.prepare("SELECT id_fvk, extfvk FROM fvks ORDER BY id_fvk ASC")?;

        let rows = client.query(&stmt_fetch_accounts, &[])?;

        let mut res: HashMap<AccountId, ExtendedFullViewingKey> = HashMap::new();
        for row in rows {
            let id_fvk: i32 = row.get(0);
            let account_id = AccountId(id_fvk as u32);
            let efvkr =
                decode_extended_full_viewing_key(HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, row.get(1))
                    .map_err(WalletError::Bech32)?;

            res.insert(account_id, efvkr.ok_or(WalletError::IncorrectHrpExtFvk)?);
        }

        Ok(res)
    }

    fn is_valid_account_extfvk(
        &self,
        account: AccountId,
        extfvk: &ExtendedFullViewingKey,
    ) -> Result<bool, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let statement =
            client.prepare("SELECT * FROM accounts WHERE account = $1 AND extfvk = $2")?;
        let extfvk =
            encode_extended_full_viewing_key(HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, extfvk);

        let res = client.query(&statement, &[&account.0, &extfvk])?;
        Ok(!res.is_empty())
    }

    fn get_balance_at(
        &self,
        account: AccountId,
        anchor_height: BlockHeight,
    ) -> Result<Amount, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let balance = client
            .query_opt(
                "SELECT SUM(value) FROM received_notes
        INNER JOIN transactions ON transactions.id_tx = received_notes.tx
        WHERE account = $1 AND spent IS NULL AND transactions.block <= $2",
                &[&account.0, &(u32::from(anchor_height) as i32)],
            )?
            .map(|row| row.get(0))
            .unwrap_or(0);

        match Amount::from_i64(balance) {
            Ok(amount) if !amount.is_negative() => Ok(amount),
            _ => Err(WalletError::Error(anyhow::anyhow!(
                "Sum of values in received_notes is out of range"
            ))),
        }
    }

    fn get_memo(&self, _id_note: Self::NoteRef) -> Result<Memo, Self::Error> {
        Ok(Memo::Empty)
    }

    fn get_commitment_tree(
        &self,
        block_height: BlockHeight,
    ) -> Result<Option<CommitmentTree<Node>>, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let row = client.query_opt(
            "SELECT sapling_tree FROM blocks WHERE height = $1",
            &[&(u32::from(block_height) as i32)],
        )?;
        let row = row.map(|row| {
            let row_data: Vec<u8> = row.get(0);
            CommitmentTree::read(&row_data[..]).map_err(WalletError::IO)
        });
        row.transpose()
    }

    fn get_witnesses(
        &self,
        block_height: BlockHeight,
    ) -> Result<Vec<(Self::NoteRef, IncrementalWitness<Node>)>, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let stmt_fetch_witnesses =
            client.prepare("SELECT note, witness FROM sapling_witnesses WHERE block = $1")?;
        let witnesses =
            client.query(&stmt_fetch_witnesses, &[&(u32::from(block_height) as i32)])?;

        let witnesses: Vec<_> = witnesses
            .iter()
            .map(|row| {
                let id_note: i32 = row.get(0);
                let wdb: Vec<u8> = row.get(1);
                IncrementalWitness::<Node>::read(wdb.as_slice()).map(|witness| (id_note, witness))
            })
            .collect();
        let witnesses: Result<Vec<_>, _> = witnesses.into_iter().collect();
        witnesses.map_err(WalletError::IO)
    }

    fn get_nullifiers(&self) -> Result<Vec<(AccountId, Nullifier)>, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let stmt_fetch_nullifiers = client.prepare(
            "SELECT rn.id_note, rn.account, rn.nf, tx.block as block
            FROM received_notes rn
            LEFT OUTER JOIN transactions tx
            ON tx.id_tx = rn.spent
            WHERE block IS NULL",
        )?;
        let nullifiers = client.query(&stmt_fetch_nullifiers, &[])?;
        let nullifiers: Vec<_> = nullifiers
            .iter()
            .map(|row| {
                let id: i32 = row.get(1);
                let account = AccountId(id as u32);
                let nf_bytes: Vec<u8> = row.get(2);
                (account, Nullifier::from_slice(&nf_bytes).unwrap())
            })
            .collect();

        Ok(nullifiers)
    }

    fn get_spendable_notes(
        &self,
        _account: AccountId,
        _anchor_height: BlockHeight,
    ) -> Result<Vec<SpendableNote>, Self::Error> {
        unimplemented!();
    }

    fn select_spendable_notes(
        &self,
        _account: AccountId,
        _target_value: Amount,
        _anchor_height: BlockHeight,
    ) -> Result<Vec<SpendableNote>, Self::Error> {
        // unused
        unimplemented!();
    }
}

impl WalletWrite for PostgresWallet {
    fn advance_by_block(
        &mut self,
        block: &PrunedBlock,
        updated_witnesses: &[(Self::NoteRef, IncrementalWitness<Node>)],
    ) -> Result<Vec<(Self::NoteRef, IncrementalWitness<Node>)>, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let mut db_tx = WalletDbTransaction {
            statements: self,
            transaction: client.transaction()?,
        };

        // Insert the block into the database.
        db_tx.insert_block(
            block.block_height,
            block.block_hash,
            block.block_time,
            &block.commitment_tree,
        )?;

        let mut new_witnesses = vec![];
        for tx in block.transactions {
            let tx_row = db_tx.put_tx_meta(&tx, block.block_height)?;

            // Mark notes as spent and remove them from the scanning cache
            for spend in &tx.shielded_spends {
                db_tx.mark_spent(tx_row, &spend.nf)?;
            }

            for output in &tx.shielded_outputs {
                let received_note_id = db_tx.put_received_note(output, tx_row)?;

                // Save witness for note.
                new_witnesses.push((received_note_id, output.witness.clone()));
            }
        }

        // Insert current new_witnesses into the database.
        for (received_note_id, witness) in updated_witnesses.iter().chain(new_witnesses.iter()) {
            let rnid = *received_note_id;
            db_tx.insert_witness(rnid, witness, block.block_height)?;
        }

        // Prune the stored witnesses (we only expect rollbacks of at most 100 blocks).
        db_tx.prune_witnesses(block.block_height - 100)?;

        // Update now-expired transactions that didn't get mined.
        db_tx.update_expired_notes(block.block_height)?;

        update_chain_tip(&mut db_tx.transaction, u32::from(block.block_height))?;

        db_tx.transaction.commit()?;
        Ok(new_witnesses)
    }

    fn store_received_tx(
        &mut self,
        received_tx: &ReceivedTransaction,
    ) -> Result<Self::TxRef, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let mut db_tx = WalletDbTransaction {
            statements: self,
            transaction: client.transaction()?,
        };

        let tx_ref = db_tx.put_tx_data(received_tx.tx, None)?;

        for output in received_tx.outputs {
            if output.outgoing {
                db_tx.put_sent_decrypted_note(output, tx_ref)?;
            } else {
                db_tx.put_received_note(output, tx_ref)?;
            }
        }

        db_tx.transaction.commit()?;
        Ok(tx_ref)
    }

    fn store_sent_tx(&mut self, sent_tx: &SentTransaction) -> Result<Self::TxRef, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let mut db_tx = WalletDbTransaction {
            statements: self,
            transaction: client.transaction()?,
        };

        let tx_ref = db_tx.put_tx_data(&sent_tx.tx, Some(sent_tx.created))?;

        // Mark notes as spent.
        //
        // This locks the notes so they aren't selected again by a subsequent call to
        // create_spend_to_address() before this transaction has been mined (at which point the notes
        // get re-marked as spent).
        //
        // Assumes that create_spend_to_address() will never be called in parallel, which is a
        // reasonable assumption for a light client such as a mobile phone.
        for spend in &sent_tx.tx.shielded_spends {
            db_tx.mark_spent(tx_ref, &spend.nullifier)?;
        }

        db_tx.put_sent_note(
            tx_ref,
            sent_tx.output_index as i32,
            sent_tx.account,
            sent_tx.recipient_address,
            sent_tx.value,
            sent_tx.memo.as_ref(),
        )?;

        db_tx.transaction.commit()?;
        // Return the row number of the transaction, so the caller can fetch it for sending.
        Ok(tx_ref)
    }

    fn rewind_to_height(&mut self, block_height: BlockHeight) -> Result<(), Self::Error> {
        let mut client = self.connection.borrow_mut();
        let mut db_tx = client.transaction()?;

        let sapling_activation_height = Network::TestNetwork
            .activation_height(NetworkUpgrade::Sapling)
            .ok_or_else(|| {
                WalletError::Error(anyhow::anyhow!("Cannot rewind to before sapling"))
            })?;

        // Recall where we synced up to previously.
        let row = db_tx.query_opt("SELECT MAX(height) FROM blocks", &[])?;
        let last_scanned_height = row
            .map(|row| {
                let height: i32 = row.get(0);
                BlockHeight::from_u32(height as u32)
            })
            .unwrap_or(sapling_activation_height - 1);

        // nothing to do if we're deleting back down to the max height
        let res = if block_height >= last_scanned_height {
            Ok(())
        } else {
            // Decrement witnesses.
            db_tx.execute(
                "DELETE FROM sapling_witnesses WHERE block > $1",
                &[&(u32::from(block_height) as i32)],
            )?;

            // Un-mine transactions.
            db_tx.execute(
                "UPDATE transactions SET block = NULL, tx_index = NULL WHERE block > $1",
                &[&(u32::from(block_height) as i32)],
            )?;

            // Now that they aren't depended on, delete scanned blocks.
            db_tx.execute(
                "DELETE FROM blocks WHERE height > $1",
                &[&(u32::from(block_height) as i32)],
            )?;

            Ok(())
        };

        db_tx.commit()?;
        res
    }
}

fn to_spendable_note(row: &Row) -> Result<SpendableNote, WalletError> {
    let diversifier = {
        let d: Vec<_> = row.get(0);
        if d.len() != 11 {
            return Err(WalletError::Error(anyhow::anyhow!(
                "Invalid diversifier length",
            )));
        }
        let mut tmp = [0; 11];
        tmp.copy_from_slice(&d);
        Diversifier(tmp)
    };

    let note_value = Amount::from_i64(row.get(1)).unwrap();

    let rseed = {
        let rcm_bytes: Vec<_> = row.get(2);

        // We store rcm directly in the data DB, regardless of whether the note
        // used a v1 or v2 note plaintext, so for the purposes of spending let's
        // pretend this is a pre-ZIP 212 note.
        let rcm = jubjub::Fr::from_repr(
            rcm_bytes[..]
                .try_into()
                .map_err(|_| WalletError::InvalidNote)?,
        )
        .ok_or(WalletError::InvalidNote)?;
        Rseed::BeforeZip212(rcm)
    };

    let witness = {
        let d: Vec<_> = row.get(3);
        IncrementalWitness::read(&d[..]).map_err(WalletError::IO)?
    };

    Ok(SpendableNote {
        diversifier,
        note_value,
        rseed,
        witness,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upsert() {
        let w = PostgresWallet::new().unwrap();
        let mut client = w.connection.borrow_mut();
        let _ = client.execute(
            &w.stmt_insert_block,
            &[&100000, &vec![0u8; 32], &0, &vec![0u8; 32]],
        );

        let mut db_tx = WalletDbTransaction {
            statements: &w,
            transaction: client.transaction().unwrap(),
        };

        let tx = WalletTx {
            txid: TxId([0; 32]),
            index: 0,
            num_spends: 0,
            num_outputs: 0,
            shielded_spends: vec![],
            shielded_outputs: vec![],
        };
        db_tx.put_tx_meta(&tx, BlockHeight::from_u32(1)).unwrap();

        let tx = WalletTx {
            txid: TxId([0; 32]),
            index: 1,
            num_spends: 0,
            num_outputs: 0,
            shielded_spends: vec![],
            shielded_outputs: vec![],
        };
        db_tx.put_tx_meta(&tx, BlockHeight::from_u32(1)).unwrap();

        db_tx.transaction.commit().unwrap();
    }
}