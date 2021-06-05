use crate::grpc::compact_tx_streamer_client::CompactTxStreamerClient;
use crate::grpc::{BlockId, BlockRange, ChainSpec, TreeState};

use prost::bytes::BytesMut;
use prost::Message as M;
use protobuf::Message;

use tokio::runtime::Runtime;
use tonic::transport::Channel;

use zcash_client_backend::data_api::chain::{scan_cached_blocks, validate_chain};
use zcash_client_backend::data_api::{BlockSource, WalletWrite, WalletRead};

use zcash_client_backend::proto::compact_formats::CompactBlock;

use zcash_primitives::consensus::{BlockHeight, Network, Parameters, NetworkUpgrade};

use crate::error::WalletError;
use crate::wallet::PostgresWallet;
use futures::StreamExt;
use std::ops::{RangeInclusive, Range};
use crate::trp::{TrpWallet};
use crate::{ZcashdConf, db};
use postgres::Client;
use crate::db::DbPreparedStatements;
use std::rc::Rc;
use std::cell::RefCell;

const LIGHTNODE_URL: &str = "http://localhost:9067";
const MAX_CHUNK: u32 = 1000;

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
        let tip_height = get_latest_height()?;
        let to_height =
            from_height
                .saturating_add(limit.unwrap_or(u32::MAX) - 1)
                .min(tip_height);

        let blocks = r.block_on(async {
            let mut client = connect_lightnode().await.unwrap();
            let blocks = client
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
            let blocks: Vec<CompactBlock> = blocks.map(|cb| {
                let cb = cb.unwrap();
                let mut cb_bytes = BytesMut::with_capacity(cb.encoded_len());
                cb.encode_raw(&mut cb_bytes);
                CompactBlock::parse_from_bytes(&cb_bytes).unwrap()
            }).collect().await;
            blocks
        });

        for cb in blocks {
            with_row(cb).unwrap();
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

pub fn get_latest_height() -> crate::Result<u32> {
    let r = Runtime::new().unwrap();
    let tip_height = r.block_on(async {
        let mut client = connect_lightnode().await?;
        let tip_height = client
            .get_latest_block(ChainSpec {})
            .await
            .unwrap()
            .into_inner()
            .height as u32;
        Ok::<_, WalletError>(tip_height)
    })?;
    Ok(tip_height)
}

pub fn get_scan_range() -> anyhow::Result<Range<u32>, WalletError> {
    let wallet = PostgresWallet::new().unwrap();
    let sapling_activation_height: u32 = Network::TestNetwork.activation_height(NetworkUpgrade::Sapling).unwrap().into();
    let mut from_height = wallet.block_height_extrema().map(|opt| {
        opt.map(|(_, max)| u32::from(max))
            .unwrap_or(sapling_activation_height - 1)
    })? + 1;
    let r = Runtime::new().unwrap();
    let tip_height = get_latest_height()? + 1;
    let to_height = tip_height.min(from_height + MAX_CHUNK);
    Ok(from_height..to_height)
}

pub fn scan_sapling() -> anyhow::Result<(), WalletError> {
    let source = BlockLightwallet {};
    let mut data = PostgresWallet::new()?;
    scan_cached_blocks(&Network::TestNetwork, &source, &mut data, Some(MAX_CHUNK))?;
    Ok(())
}

pub fn scan_chain(c: Rc<RefCell<Client>>, config: &ZcashdConf) -> anyhow::Result<(), WalletError> {
    let mut trp_wallet = TrpWallet::new(c)?;
    loop {
        let range = get_scan_range()?;
        println!("{:?}", range);
        let len = range.end - range.start;
        if len == 0 { break }
        scan_sapling()?;
        trp_wallet.scan_transparent(range, config)?;
    }
    Ok(())
}

pub fn load_checkpoint(client: &mut Client, height: u32) -> Result<(), WalletError> {
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

    db::load_checkpoint(
        client,
        tree_state.height as u32,
        &hex::decode(tree_state.hash).map_err(|_| anyhow::anyhow!("Not hex"))?,
        tree_state.time as i32,
        &hex::decode(tree_state.tree).map_err(|_| anyhow::anyhow!("Not hex"))?,
    )?;
    Ok(())
}

pub fn rewind_to_height(height: u32) -> Result<(), WalletError> {
    let mut data = PostgresWallet::new()?;
    data.rewind_to_height(BlockHeight::from_u32(height))?;
    let trp_wallet = TrpWallet::new(data.connection.clone())?;
    trp_wallet.rewind_to_height(height)?;
    Ok(())
}
