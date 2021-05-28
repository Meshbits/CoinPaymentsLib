use std::collections::HashMap;
use zcash_client_backend::data_api::{
    PrunedBlock, ReceivedTransaction, SentTransaction, WalletRead, WalletWrite,
};
use zcash_client_backend::wallet::{AccountId, SpendableNote};
use zcash_primitives::block::BlockHash;
use zcash_primitives::consensus::BlockHeight;
use zcash_primitives::memo::Memo;
use zcash_primitives::merkle_tree::{CommitmentTree, IncrementalWitness};
use zcash_primitives::sapling::{Node, Nullifier, PaymentAddress};
use zcash_primitives::transaction::components::Amount;
use zcash_primitives::transaction::TxId;
use zcash_primitives::zip32::ExtendedFullViewingKey;
use diesel::PgConnection;
use crate::db::establish_connection;

pub struct DbWallet {
    connection: PgConnection,
}

impl DbWallet {
    pub fn new(database_url: &str) -> DbWallet {
        let connection = establish_connection(database_url);
        DbWallet {
            connection,
        }
    }
}

impl WalletRead for DbWallet {
    type Error = anyhow::Error;
    type NoteRef = i32;
    type TxRef = i32;

    fn block_height_extrema(&self) -> Result<Option<(BlockHeight, BlockHeight)>, Self::Error> {
        todo!()
    }

    fn get_block_hash(&self, block_height: BlockHeight) -> Result<Option<BlockHash>, Self::Error> {
        todo!()
    }

    fn get_tx_height(&self, txid: TxId) -> Result<Option<BlockHeight>, Self::Error> {
        todo!()
    }

    fn get_address(&self, account: AccountId) -> Result<Option<PaymentAddress>, Self::Error> {
        todo!()
    }

    fn get_extended_full_viewing_keys(
        &self,
    ) -> Result<HashMap<AccountId, ExtendedFullViewingKey>, Self::Error> {
        todo!()
    }

    fn is_valid_account_extfvk(
        &self,
        account: AccountId,
        extfvk: &ExtendedFullViewingKey,
    ) -> Result<bool, Self::Error> {
        todo!()
    }

    fn get_balance_at(
        &self,
        account: AccountId,
        anchor_height: BlockHeight,
    ) -> Result<Amount, Self::Error> {
        todo!()
    }

    fn get_memo(&self, id_note: Self::NoteRef) -> Result<Memo, Self::Error> {
        todo!()
    }

    fn get_commitment_tree(
        &self,
        block_height: BlockHeight,
    ) -> Result<Option<CommitmentTree<Node>>, Self::Error> {
        todo!()
    }

    fn get_witnesses(
        &self,
        block_height: BlockHeight,
    ) -> Result<Vec<(Self::NoteRef, IncrementalWitness<Node>)>, Self::Error> {
        todo!()
    }

    fn get_nullifiers(&self) -> Result<Vec<(AccountId, Nullifier)>, Self::Error> {
        todo!()
    }

    fn get_spendable_notes(
        &self,
        account: AccountId,
        anchor_height: BlockHeight,
    ) -> Result<Vec<SpendableNote>, Self::Error> {
        todo!()
    }

    fn select_spendable_notes(
        &self,
        account: AccountId,
        target_value: Amount,
        anchor_height: BlockHeight,
    ) -> Result<Vec<SpendableNote>, Self::Error> {
        todo!()
    }
}

impl WalletWrite for DbWallet {
    fn advance_by_block(
        &mut self,
        block: &PrunedBlock,
        updated_witnesses: &[(Self::NoteRef, IncrementalWitness<Node>)],
    ) -> Result<Vec<(Self::NoteRef, IncrementalWitness<Node>)>, Self::Error> {
        todo!()
    }

    fn store_received_tx(
        &mut self,
        received_tx: &ReceivedTransaction,
    ) -> Result<Self::TxRef, Self::Error> {
        todo!()
    }

    fn store_sent_tx(&mut self, sent_tx: &SentTransaction) -> Result<Self::TxRef, Self::Error> {
        todo!()
    }

    fn rewind_to_height(&mut self, block_height: BlockHeight) -> Result<(), Self::Error> {
        todo!()
    }
}
