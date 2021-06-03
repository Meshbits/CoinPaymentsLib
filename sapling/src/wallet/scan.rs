use crate::grpc::compact_tx_streamer_client::CompactTxStreamerClient;
use crate::grpc::{BlockId, BlockRange, ChainSpec, TreeState};

use prost::bytes::BytesMut;
use prost::Message as M;
use protobuf::Message;

use tokio::runtime::Runtime;
use tonic::transport::Channel;

use zcash_client_backend::data_api::chain::{scan_cached_blocks, validate_chain};
use zcash_client_backend::data_api::{BlockSource, WalletWrite};

use zcash_client_backend::proto::compact_formats::CompactBlock;

use zcash_primitives::consensus::{BlockHeight, Network, Parameters, NetworkUpgrade};

use crate::error::WalletError;
use crate::wallet::PostgresWallet;
use futures::StreamExt;
use std::ops::{RangeInclusive, Range};
use crate::trp::scan_transparent;
use crate::ZcashdConf;

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
        let from_height = u32::from(from_height) + 1;
        let r = Runtime::new().unwrap();
        let to_height = r.block_on(async move {
            let mut client = connect_lightnode().await.unwrap();
            let latest_block_id = client
                .get_latest_block(ChainSpec {})
                .await
                .unwrap()
                .into_inner();
            from_height
                .saturating_add(limit.unwrap_or(u32::MAX))
                .min(latest_block_id.height as u32)
        });
        let mut s = from_height;
        while s < to_height {
            let e = (s + 999).min(to_height);

            let blocks = r.block_on(async {
                let mut client = connect_lightnode().await.unwrap();
                let blocks = client
                    .get_block_range(tonic::Request::new(BlockRange {
                        start: Some(BlockId {
                            hash: Vec::new(),
                            height: s as u64,
                        }),
                        end: Some(BlockId {
                            hash: Vec::new(),
                            height: e as u64,
                        }),
                    }))
                    .await
                    .unwrap()
                    .into_inner();
                let blocks: Vec<CompactBlock> = blocks.map(|cb| {
                    let cb = cb.unwrap();
                    let mut cb_bytes = BytesMut::with_capacity(cb.encoded_len());
                    cb.encode_raw(&mut cb_bytes);
                    CompactBlock::parse_from_bytes(&cb_bytes).unwrap()
                }).collect().await;
                blocks
            });

            for cb in blocks {
                if cb.height % 1000 == 0 {
                    println!("{}", cb.height);
                }
                with_row(cb).unwrap();
            }

            s = e + 1;
        }

        Ok(())
    }
}

pub async fn connect_lightnode() -> anyhow::Result<CompactTxStreamerClient<Channel>> {
    let channel = tonic::transport::Channel::from_shared(LIGHTNODE_URL)?;
    let client = CompactTxStreamerClient::connect(channel).await?;
    Ok(client)
}

pub fn validate() -> anyhow::Result<(), WalletError> {
    let source = BlockLightwallet {};
    validate_chain(&Network::TestNetwork, &source, None)?;

    Ok(())
}

pub fn get_scan_range(default_from_height: Option<i32>) -> anyhow::Result<RangeInclusive<i32>, WalletError> {
    let wallet = PostgresWallet::new().unwrap();
    let sapling_activation_height: u32 = Network::TestNetwork.activation_height(NetworkUpgrade::Sapling).unwrap().into();
    let from_height = wallet.get_chain_tip()?.or(default_from_height).unwrap_or(sapling_activation_height as i32);
    let r = Runtime::new().unwrap();
    let to_height = r.block_on(async {
        let mut client = connect_lightnode().await.unwrap();
        let to_height = client
            .get_latest_block(ChainSpec {})
            .await
            .unwrap()
            .into_inner()
            .height as i32;
        to_height
    });
    Ok(from_height..=to_height)
}

pub fn scan_sapling() -> anyhow::Result<(), WalletError> {
    let source = BlockLightwallet {};
    let mut data = PostgresWallet::new()?;
    scan_cached_blocks(&Network::TestNetwork, &source, &mut data, None)?;
    Ok(())
}

pub fn scan(config: &ZcashdConf) -> anyhow::Result<(), WalletError> {
    let range = get_scan_range(None)?;
    scan_sapling()?;
    scan_transparent(range, &config)?;
    Ok(())
}

pub fn load_checkpoint(height: i32) -> Result<(), WalletError> {
    let r = Runtime::new().unwrap();
    let tree_state = r.block_on(async {
        let mut client = connect_lightnode().await?;
        Ok::<_, WalletError>(client
            .get_tree_state(BlockId {
                height: height as u64,
                hash: vec![],
            })
            .await?
            .into_inner())
    })?;
    let data = PostgresWallet::new()?;

    data.load_checkpoint(
        tree_state.height as u32,
        &hex::decode(tree_state.hash).map_err(|_| anyhow::anyhow!("Not hex"))?,
        tree_state.time as i32,
        &hex::decode(tree_state.tree).map_err(|_| anyhow::anyhow!("Not hex"))?,
    )?;
    Ok(())
}

pub fn rewind_to_height(height: i32) -> Result<(), WalletError> {
    let mut data = PostgresWallet::new()?;
    data.rewind_to_height(BlockHeight::from_u32(height as u32))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testconfig::{TEST_ZCASHD_URL, TEST_DATADIR};

    #[test]
    fn test_validate() {
        validate().unwrap();
    }

    #[test]
    fn test_scan() {
        let config = ZcashdConf::parse(TEST_ZCASHD_URL, TEST_DATADIR).unwrap();
        scan(&config).unwrap();
    }

    #[test]
    fn test_scan_range() {
        let r = get_scan_range(Some(1_400_000)).unwrap();
        println!("{:?}", r);
    }
}
