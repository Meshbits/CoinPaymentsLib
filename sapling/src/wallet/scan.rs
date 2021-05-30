use crate::grpc::compact_tx_streamer_client::CompactTxStreamerClient;
use crate::grpc::{BlockId, BlockRange, ChainSpec};

use prost::bytes::BytesMut;
use prost::Message as M;
use protobuf::Message;

use tokio::runtime::Runtime;
use tonic::transport::Channel;

use zcash_client_backend::data_api::chain::{scan_cached_blocks, validate_chain};
use zcash_client_backend::data_api::{BlockSource, WalletWrite};

use zcash_client_backend::proto::compact_formats::CompactBlock;

use zcash_primitives::consensus::{BlockHeight, Network};

use crate::error::WalletError;
use crate::wallet::PostgresWallet;

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
        r.block_on(async {
            let mut client = connect_lightnode().await.unwrap();
            let latest_block_id = client
                .get_latest_block(ChainSpec {})
                .await
                .unwrap()
                .into_inner();
            let to_height = from_height
                .saturating_add(limit.unwrap_or(u32::MAX))
                .min(latest_block_id.height as u32);
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
                if cb.height % 1000 == 0 {
                    println!("{}", cb.height);
                }
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

pub fn load_checkpoint(height: i32) -> Result<(), WalletError> {
    let mut r = Runtime::new().map_err(WalletError::IO)?;
    r.block_on(async {
        let mut client = connect_lightnode().await?;
        let tree_state = client
            .get_tree_state(BlockId {
                height: height as u64,
                hash: vec![],
            })
            .await?
            .into_inner();
        let data = PostgresWallet::new()?;
        data.load_checkpoint(
            tree_state.height as i32,
            &hex::decode(tree_state.hash).map_err(|_| anyhow::anyhow!("Not hex"))?,
            tree_state.time as i32,
            &hex::decode(tree_state.tree).map_err(|_| anyhow::anyhow!("Not hex"))?,
        )?;
        Ok::<(), WalletError>(())
    })?;
    Ok(())
}

pub fn rewind_to_height(height: i32) -> Result<(), WalletError> {
    let mut data = PostgresWallet::new()?;
    data.rewind_to_height(BlockHeight::from_u32(height as u32))?;

    Ok(())
}

// pub fn load_checkpoint(&self, height: i32, hash: &[u8], time: i64, sapling_tree: &[u8]) -> Result<(), WalletError> {

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    fn test_validate() {
        validate().unwrap();
    }

    #[test]
    fn test_scan() {
        scan().unwrap();
    }
}
