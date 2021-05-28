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
use crate::wallet::PostgresWallet;
use crate::error::WalletError;

const LIGHTNODE_URL: &str = "http://localhost:9067";

pub struct BlockLightwallet {}

impl BlockSource for BlockLightwallet {
    type Error = WalletError;

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

pub fn validate() -> anyhow::Result<(), WalletError> {
    let source = BlockLightwallet {};
    validate_chain(&Network::TestNetwork, &source, None)?;

    Ok(())
}

pub fn scan() -> anyhow::Result<(), WalletError> {
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
}