use crate::grpc::compact_tx_streamer_client::CompactTxStreamerClient;
use crate::grpc::{BlockId, BlockRange};
use ff::PrimeField;
use postgres::fallible_iterator::FallibleIterator;
use postgres::types::ToSql;
use postgres::{Client, NoTls, Row, Statement};
use prost::bytes::BytesMut;
use prost::Message as M;
use protobuf::Message;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tonic::transport::Channel;
use zcash_client_backend::address::RecipientAddress;
use zcash_client_backend::data_api::chain::{scan_cached_blocks, validate_chain};
use zcash_client_backend::data_api::{
    BlockSource, PrunedBlock, ReceivedTransaction, SentTransaction, WalletRead, WalletWrite,
};
use zcash_client_backend::encoding::{
    decode_extended_full_viewing_key, decode_payment_address, encode_extended_full_viewing_key,
    encode_payment_address,
};
use zcash_client_backend::proto::compact_formats::CompactBlock;
use zcash_client_backend::wallet::{AccountId, SpendableNote, WalletShieldedOutput, WalletTx};
use zcash_client_backend::{data_api, DecryptedOutput};
use zcash_primitives::block::BlockHash;
use zcash_primitives::consensus::Network::TestNetwork;
use zcash_primitives::consensus::{BlockHeight, Network, NetworkUpgrade, Parameters};
use zcash_primitives::constants::testnet::{
    HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, HRP_SAPLING_PAYMENT_ADDRESS,
};
use zcash_primitives::memo::{Memo, MemoBytes};
use zcash_primitives::merkle_tree::{CommitmentTree, IncrementalWitness};
use zcash_primitives::sapling::{Diversifier, Node, Note, Nullifier, PaymentAddress, Rseed};
use zcash_primitives::transaction::components::Amount;
use zcash_primitives::transaction::{Transaction, TxId};
use zcash_primitives::zip32::ExtendedFullViewingKey;

const LIGHTNODE_URL: &str = "http://localhost:9067";
pub const CONNECTION_STRING: &str = "host=localhost dbname=saplingdb user=hanh";

pub struct BlockLightwallet {}

impl BlockSource for BlockLightwallet {
    type Error = MyError;

    fn with_blocks<F>(
        &self,
        from_height: BlockHeight,
        limit: Option<u32>,
        mut with_row: F,
    ) -> Result<(), Self::Error>
    where
        F: FnMut(CompactBlock) -> Result<(), Self::Error>,
    {
        let mut r = Runtime::new().unwrap();
        let from_height = u32::from(from_height) + 1;
        let to_height = from_height.saturating_add(limit.unwrap_or(u32::MAX));
        r.block_on(async {
            let mut client = connect_lightnode().await.unwrap();
            let mut blocks = client
                .get_block_range(tonic::Request::new(BlockRange {
                    start: Some(BlockId {
                        hash: Vec::new(),
                        height: from_height as u64,
                    }),
                    end: Some(BlockId {
                        hash: Vec::new(),
                        height: to_height as u64,
                    }),
                }))
                .await
                .unwrap()
                .into_inner();
            while let Some(cb) = blocks.message().await.unwrap() {
                println!("{}", cb.height);
                let mut cb_bytes = BytesMut::with_capacity(cb.encoded_len());
                cb.encode_raw(&mut cb_bytes);
                let block = CompactBlock::parse_from_bytes(&cb_bytes).unwrap();
                with_row(block).unwrap();
            }
        });
        Ok(())
    }
}

async fn connect_lightnode() -> anyhow::Result<CompactTxStreamerClient<Channel>> {
    let channel = tonic::transport::Channel::from_shared(LIGHTNODE_URL)?;
    let client = CompactTxStreamerClient::connect(channel).await?;
    Ok(client)
}

pub struct PostgresWallet {
    connection: Arc<RefCell<Client>>,
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
    pub fn new() -> Result<PostgresWallet, MyError> {
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
                "INSERT INTO received_notes (tx, output_index, account, diversifier, value, rcm, memo, nf, is_change)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    ON CONFLICT (tx, output_index) DO UPDATE
                    SET account = excluded.account,
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
    ) -> Result<(), MyError> {
        let mut client = &mut self.transaction;
        let mut encoded_tree = Vec::new();
        commitment_tree.write(&mut encoded_tree).unwrap();

        client.execute(
            &self.statements.stmt_insert_block,
            &[
                &u32::from(block_height),
                &block_hash.0.to_vec(),
                &block_time,
                &encoded_tree,
            ],
        )?;

        Ok(())
    }

    pub fn put_tx_meta(
        &mut self,
        tx: &WalletTx<Nullifier>,
        height: BlockHeight,
    ) -> Result<i32, MyError> {
        let txid = tx.txid.0.to_vec();
        let row = self.transaction.query_one(
            &self.statements.stmt_upsert_tx_meta,
            &[&txid, &(u32::from(height) as i32), &(tx.index as i32)],
        )?;
        Ok(row.get(0))
    }

    pub fn mark_spent(&mut self, tx_ref: i32, nf: &Nullifier) -> Result<(), MyError> {
        self.transaction.execute(
            &self.statements.stmt_mark_received_note_spent,
            &[&tx_ref, &&nf.0[..]],
        )?;
        Ok(())
    }

    pub fn update_expired_notes(&mut self, height: BlockHeight) -> Result<(), MyError> {
        self.transaction
            .execute(&self.statements.stmt_update_expired, &[&u32::from(height)])?;
        Ok(())
    }

    pub fn prune_witnesses(&mut self, below_height: BlockHeight) -> Result<(), MyError> {
        self.transaction.execute(
            &self.statements.stmt_prune_witnesses,
            &[&u32::from(below_height)],
        )?;
        Ok(())
    }

    pub fn insert_witness(
        &mut self,
        note_id: i32,
        witness: &IncrementalWitness<Node>,
        height: BlockHeight,
    ) -> Result<(), MyError> {
        let mut encoded = Vec::new();
        witness.write(&mut encoded).unwrap();

        self.transaction.execute(
            &self.statements.stmt_insert_witness,
            &[&note_id, &u32::from(height), &encoded],
        )?;

        Ok(())
    }

    pub fn put_received_note<T: ShieldedOutput>(
        &mut self,
        output: &T,
        tx_ref: i32,
    ) -> Result<i32, MyError> {
        let rcm = output.note().rcm().to_repr();
        let account = output.account().0 as i64;
        let diversifier = output.to().diversifier().0.to_vec();
        let value = output.note().value as i64;
        let rcm = rcm.as_ref();
        let memo = output.memo().map(|m| m.as_slice());
        let is_change = output.is_change();
        let tx = tx_ref;
        let output_index = output.index() as i64;
        let nf_bytes = output.nullifier().map(|nf| nf.0.to_vec());

        let sql_args: &[&(dyn ToSql + Sync)] = &[
            &account,
            &diversifier,
            &value,
            &rcm,
            &nf_bytes,
            &memo,
            &is_change,
            &tx,
            &output_index,
        ];

        self.transaction
            .query_one(&self.statements.stmt_upsert_received_note, sql_args)
            .map(|row| row.get(0))
            .map_err(MyError::Postgres)
    }

    pub fn put_tx_data(
        &mut self,
        tx: &Transaction,
        created_at: Option<time::OffsetDateTime>,
    ) -> Result<i32, MyError> {
        let txid = tx.txid().0.to_vec();

        let mut raw_tx = vec![];
        tx.write(&mut raw_tx).map_err(MyError::IO)?;

        self.transaction
            .query_one(
                &self.statements.stmt_upsert_tx_data,
                &[&txid, &created_at, &u32::from(tx.expiry_height), &raw_tx],
            )
            .map(|row| row.get(0))
            .map_err(MyError::Postgres)
    }

    pub fn put_sent_decrypted_note(
        &mut self,
        output: &DecryptedOutput,
        tx_ref: i32,
    ) -> Result<i32, MyError> {
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
    ) -> Result<i32, MyError> {
        let to_str = to.encode(&Network::TestNetwork);
        self.transaction
            .query_one(
                &self.statements.stmt_upsert_sent_note,
                &[
                    &account.0,
                    &to_str,
                    &i64::from(value),
                    &memo.map(|m| m.as_slice().to_vec()),
                    &tx_ref,
                    &output_index,
                ],
            )
            .map(|row| row.get(0))
            .map_err(MyError::Postgres)
    }
}

pub trait ShieldedOutput {
    fn index(&self) -> usize;
    fn account(&self) -> AccountId;
    fn to(&self) -> &PaymentAddress;
    fn note(&self) -> &Note;
    fn memo(&self) -> Option<&MemoBytes>;
    fn is_change(&self) -> Option<bool>;
    fn nullifier(&self) -> Option<Nullifier>;
}

impl ShieldedOutput for WalletShieldedOutput<Nullifier> {
    fn index(&self) -> usize {
        self.index
    }
    fn account(&self) -> AccountId {
        self.account
    }
    fn to(&self) -> &PaymentAddress {
        &self.to
    }
    fn note(&self) -> &Note {
        &self.note
    }
    fn memo(&self) -> Option<&MemoBytes> {
        None
    }
    fn is_change(&self) -> Option<bool> {
        Some(self.is_change)
    }

    fn nullifier(&self) -> Option<Nullifier> {
        Some(self.nf)
    }
}

impl ShieldedOutput for DecryptedOutput {
    fn index(&self) -> usize {
        self.index
    }
    fn account(&self) -> AccountId {
        self.account
    }
    fn to(&self) -> &PaymentAddress {
        &self.to
    }
    fn note(&self) -> &Note {
        &self.note
    }
    fn memo(&self) -> Option<&MemoBytes> {
        Some(&self.memo)
    }
    fn is_change(&self) -> Option<bool> {
        None
    }
    fn nullifier(&self) -> Option<Nullifier> {
        None
    }
}

impl WalletRead for PostgresWallet {
    type Error = MyError;
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
                "SELECT hash FROM blocks WHERE height = ?",
                &[&u32::from(block_height)],
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
                "SELECT block FROM transactions WHERE txid = ?",
                &[&txid.0.to_vec()],
            )?
            .map(|row| {
                let height: u32 = row.get(0);
                BlockHeight::from_u32(height)
            }))
    }

    fn get_address(&self, account: AccountId) -> Result<Option<PaymentAddress>, Self::Error> {
        let row = self.connection.borrow_mut().query_opt(
            "SELECT address FROM accounts WHERE account = ?",
            &[&account.0],
        )?;
        let row = row.map(|row| {
            let addr: String = row.get(0);
            decode_payment_address(HRP_SAPLING_PAYMENT_ADDRESS, &addr).map_err(MyError::Bech32)
        });
        row.transpose().map(|r| r.flatten())
    }

    fn get_extended_full_viewing_keys(
        &self,
    ) -> Result<HashMap<AccountId, ExtendedFullViewingKey>, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let mut stmt_fetch_accounts =
            client.prepare("SELECT account, extfvk FROM accounts ORDER BY account ASC")?;

        let mut rows = client.query(&stmt_fetch_accounts, &[])?;

        let mut res: HashMap<AccountId, ExtendedFullViewingKey> = HashMap::new();
        for row in rows {
            let account_id = AccountId(row.get(0));
            let efvkr =
                decode_extended_full_viewing_key(HRP_SAPLING_EXTENDED_FULL_VIEWING_KEY, row.get(1))
                    .map_err(MyError::Bech32)?;

            res.insert(account_id, efvkr.ok_or(MyError::IncorrectHrpExtFvk)?);
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
            client.prepare("SELECT * FROM accounts WHERE account = ? AND extfvk = ?")?;
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
        WHERE account = ? AND spent IS NULL AND transactions.block <= ?",
                &[&account.0, &u32::from(anchor_height)],
            )?
            .map(|row| row.get(0))
            .unwrap_or(0);

        match Amount::from_i64(balance) {
            Ok(amount) if !amount.is_negative() => Ok(amount),
            _ => Err(MyError::Error(anyhow::anyhow!(
                "Sum of values in received_notes is out of range"
            ))),
        }
    }

    fn get_memo(&self, id_note: Self::NoteRef) -> Result<Memo, Self::Error> {
        Ok(Memo::Empty)
    }

    fn get_commitment_tree(
        &self,
        block_height: BlockHeight,
    ) -> Result<Option<CommitmentTree<Node>>, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let row = client.query_opt(
            "SELECT sapling_tree FROM blocks WHERE height = ?",
            &[&u32::from(block_height)],
        )?;
        let row = row.map(|row| {
            let row_data: Vec<u8> = row.get(0);
            CommitmentTree::read(&row_data[..]).map_err(MyError::IO)
        });
        row.transpose()
    }

    fn get_witnesses(
        &self,
        block_height: BlockHeight,
    ) -> Result<Vec<(Self::NoteRef, IncrementalWitness<Node>)>, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let mut stmt_fetch_witnesses =
            client.prepare("SELECT note, witness FROM sapling_witnesses WHERE block = ?")?;
        let witnesses = client.query(&stmt_fetch_witnesses, &[&u32::from(block_height)])?;

        let witnesses: Vec<_> = witnesses
            .iter()
            .map(|row| {
                let id_note: i32 = row.get(0);
                let wdb: Vec<u8> = row.get(1);
                IncrementalWitness::<Node>::read(wdb.as_slice()).map(|witness| (id_note, witness))
            })
            .collect();
        let witnesses: Result<Vec<_>, _> = witnesses.into_iter().collect();
        witnesses.map_err(MyError::IO)
    }

    fn get_nullifiers(&self) -> Result<Vec<(AccountId, Nullifier)>, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let mut stmt_fetch_nullifiers = client.prepare(
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
                let account = AccountId(row.get(1));
                let nf_bytes: Vec<u8> = row.get(2);
                (account, Nullifier::from_slice(&nf_bytes).unwrap())
            })
            .collect();

        Ok(nullifiers)
    }

    fn get_spendable_notes(
        &self,
        account: AccountId,
        anchor_height: BlockHeight,
    ) -> Result<Vec<SpendableNote>, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let mut stmt_select_notes = client.prepare(
            "SELECT diversifier, value, rcm, witness
            FROM received_notes
            INNER JOIN transactions ON transactions.id_tx = received_notes.tx
            INNER JOIN sapling_witnesses ON sapling_witnesses.note = received_notes.id_note
            WHERE account = :account
            AND spent IS NULL
            AND transactions.block <= $1
            AND sapling_witnesses.block = $2",
        )?;

        // Select notes
        let notes = client.query(
            &stmt_select_notes,
            &[&i64::from(account.0), &u32::from(anchor_height)],
        )?;
        let notes: Vec<_> = notes.iter().map(to_spendable_note).collect();
        notes.into_iter().collect()
    }

    fn select_spendable_notes(
        &self,
        account: AccountId,
        target_value: Amount,
        anchor_height: BlockHeight,
    ) -> Result<Vec<SpendableNote>, Self::Error> {
        let mut client = self.connection.borrow_mut();
        let mut stmt_select_notes = client.prepare(
            "WITH selected AS (
            WITH eligible AS (
                SELECT id_note, diversifier, value, rcm,
                    SUM(value) OVER
                        (PARTITION BY account, spent ORDER BY id_note) AS so_far
                FROM received_notes
                INNER JOIN transactions ON transactions.id_tx = received_notes.tx
                WHERE account = $1 AND spent IS NULL AND transactions.block <= $2
            )
            SELECT * FROM eligible WHERE so_far < $3
            UNION
            SELECT * FROM (SELECT * FROM eligible WHERE so_far >= $3 LIMIT 1)
        ), witnesses AS (
            SELECT note, witness FROM sapling_witnesses
            WHERE block = $2
        )
        SELECT selected.diversifier, selected.value, selected.rcm, witnesses.witness
        FROM selected
        INNER JOIN witnesses ON selected.id_note = witnesses.note",
        )?;

        // Select notes
        let notes = client.query(
            &stmt_select_notes,
            &[
                &i64::from(account.0),
                &u32::from(anchor_height),
                &i64::from(target_value),
            ],
        )?;
        let notes: Vec<_> = notes.iter().map(to_spendable_note).collect();

        notes.into_iter().collect()
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
            .ok_or(MyError::Error(anyhow::anyhow!(
                "Cannot rewind to before sapling"
            )))?;

        let mut client = self.connection.borrow_mut();
        // Recall where we synced up to previously.
        let row = db_tx.query_opt("SELECT MAX(height) FROM blocks", &[])?;
        let last_scanned_height = row
            .map(|row| BlockHeight::from_u32(row.get(0)))
            .unwrap_or(sapling_activation_height - 1);

        // nothing to do if we're deleting back down to the max height
        let res = if block_height >= last_scanned_height {
            Ok(())
        } else {
            // Decrement witnesses.
            db_tx.execute(
                "DELETE FROM sapling_witnesses WHERE block > ?",
                &[&u32::from(block_height)],
            )?;

            // Un-mine transactions.
            db_tx.execute(
                "UPDATE transactions SET block = NULL, tx_index = NULL WHERE block > ?",
                &[&u32::from(block_height)],
            )?;

            // Now that they aren't depended on, delete scanned blocks.
            db_tx.execute(
                "DELETE FROM blocks WHERE height > ?",
                &[&u32::from(block_height)],
            )?;

            Ok(())
        };

        db_tx.commit()?;
        res
    }
}

fn to_spendable_note(row: &Row) -> Result<SpendableNote, MyError> {
    let diversifier = {
        let d: Vec<_> = row.get(0);
        if d.len() != 11 {
            return Err(MyError::Error(anyhow::anyhow!(
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
        let rcm =
            jubjub::Fr::from_repr(rcm_bytes[..].try_into().map_err(|_| MyError::InvalidNote)?)
                .ok_or(MyError::InvalidNote)?;
        Rseed::BeforeZip212(rcm)
    };

    let witness = {
        let d: Vec<_> = row.get(3);
        IncrementalWitness::read(&d[..]).map_err(MyError::IO)?
    };

    Ok(SpendableNote {
        diversifier,
        note_value,
        rseed,
        witness,
    })
}

#[derive(Debug)]
pub enum MyError {
    Bech32(bech32::Error),
    IncorrectHrpExtFvk,
    DataError(data_api::error::Error<i32>),
    IO(std::io::Error),
    InvalidNote,
    Error(anyhow::Error),
    Postgres(postgres::Error),
}

impl From<data_api::error::Error<i32>> for MyError {
    fn from(e: data_api::error::Error<i32>) -> Self {
        MyError::DataError(e)
    }
}

impl From<anyhow::Error> for MyError {
    fn from(e: anyhow::Error) -> Self {
        MyError::Error(e)
    }
}

impl From<postgres::Error> for MyError {
    fn from(e: postgres::Error) -> Self {
        MyError::Postgres(e)
    }
}

pub fn validate() -> anyhow::Result<(), MyError> {
    let source = BlockLightwallet {};
    validate_chain(&Network::TestNetwork, &source, None)?;

    Ok(())
}

pub fn scan() -> anyhow::Result<(), MyError> {
    let source = BlockLightwallet {};
    let mut data = PostgresWallet::new()?;
    scan_cached_blocks(&Network::TestNetwork, &source, &mut data, None)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    fn test_validate() {
        validate().unwrap();
    }

    #[test]
    fn test_upsert() {
        let w = PostgresWallet::new().unwrap();
        let mut client = w.connection.borrow_mut();
        let _ = client.execute(
            &w.stmt_insert_block,
            &[&1, &vec![0u8; 32], &0, &vec![0u8; 32]],
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
