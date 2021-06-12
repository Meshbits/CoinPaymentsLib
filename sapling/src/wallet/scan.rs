use tokio::runtime::Runtime;
use zcash_client_backend::data_api::chain::scan_cached_blocks;
use zcash_client_backend::data_api::{BlockSource, WalletRead, WalletWrite};
use zcash_client_backend::proto::compact_formats::CompactBlock;
use zcash_primitives::consensus::{BlockHeight, Network, NetworkUpgrade, Parameters};
use crate::error::WalletError;
use crate::trp::TrpWallet;
use crate::wallet::PostgresWallet;
use crate::db;
use postgres::{Client, GenericClient};
use std::ops::Range;
use std::sync::{Mutex, Arc};
use crate::config::ZamsConfig;
use crate::trp::zcashdrpc::{get_block, get_latest_height, get_tree_state};

const MAX_CHUNK: u32 = 1000;

pub struct ZcashdCompactBlockSource {
    client: Arc<Mutex<Client>>,
    config: ZamsConfig,
}

impl ZcashdCompactBlockSource {
    pub fn new(client: Arc<Mutex<Client>>, config: &ZamsConfig) -> ZcashdCompactBlockSource {
        ZcashdCompactBlockSource {
            client,
            config: config.clone(),
        }
    }
}

impl BlockSource for ZcashdCompactBlockSource {
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
        // We scan [from_height+1, from_height+limit] (inclusive)
        let from_height = u32::from(from_height);
        let start_height = from_height + 1;
        let r = Runtime::new().unwrap();
        let tip_height = get_latest_height(&self.config)?;
        let end_height = from_height
            .saturating_add(limit.unwrap_or(u32::MAX))
            .min(tip_height);

        let blocks = r.block_on(async {
            let client = reqwest::Client::new();
            let mut blocks: Vec<CompactBlock> = vec![];
            for height in start_height..=end_height {
                let block = get_block(&format!("{}", height), &client, &self.config).await?;
                blocks.push(block.to_compact()?);
            }
            Ok::<_, WalletError>(blocks)
        })?;
        if blocks.is_empty() { return Ok(()) }

        let block_hash = {
            let mut client = self.client.lock().unwrap();
            db::get_block_by_height(&mut *client, from_height)
        }?;
        if let Some(block_hash) = block_hash {
            let b = blocks.first().unwrap();
            if b.prevHash != block_hash {
                return Err(WalletError::Reorg)
            }
        }

        for cb in blocks {
            with_row(cb).unwrap();
        }

        Ok(())
    }
}

pub fn get_scan_range(client: Arc<Mutex<Client>>, config: &ZamsConfig) -> anyhow::Result<Range<u32>, WalletError> {
    let wallet = PostgresWallet::new(client, config).unwrap();
    let sapling_activation_height: u32 = Network::TestNetwork
        .activation_height(NetworkUpgrade::Sapling)
        .unwrap()
        .into();
    let from_height = wallet.block_height_extrema().map(|opt| {
        opt.map(|(_, max)| u32::from(max))
            .unwrap_or(sapling_activation_height - 1)
    })? + 1;
    let _r = Runtime::new().unwrap();
    let tip_height = get_latest_height(config)? + 1;
    let to_height = tip_height.min(from_height + MAX_CHUNK);
    Ok(from_height..to_height)
}

pub fn scan_sapling(client: Arc<Mutex<Client>>, config: &ZamsConfig) -> anyhow::Result<(), WalletError> {
    let source = ZcashdCompactBlockSource::new(client.clone(), config);
    let mut data = PostgresWallet::new(client, config)?;
    scan_cached_blocks(&Network::TestNetwork, &source, &mut data, Some(MAX_CHUNK))?;
    Ok(())
}

pub fn scan_chain(client: Arc<Mutex<Client>>, config: &ZamsConfig) -> anyhow::Result<u32, WalletError> {
    let mut trp_wallet = TrpWallet::new(client.clone(), config.clone())?;
    let range = loop {
        let range = get_scan_range(client.clone(), config)?;
        log::info!("Scan {:?}", &range);
        if range.end <= range.start {
            break range;
        }
        let mut scan = || {
            scan_sapling(client.clone(), config)?;
            trp_wallet.scan_transparent(range.clone())?;
            Ok(())
        };
        let scan_result = scan();
        match scan_result {
            Err(WalletError::Reorg) => rewind_to_height(client.clone(), range.start - 10, config)?,
            _ => scan_result?,
        }
    };

    Ok(range.end)
}

pub fn load_checkpoint<C: GenericClient>(client: &mut C, height: u32, config: &ZamsConfig) -> Result<(), WalletError> {
    let tree_state = get_tree_state(height, config)?;

    db::load_checkpoint(
        client,
        height as u32,
        &hex::decode(tree_state.hash).map_err(|_| anyhow::anyhow!("Not hex"))?,
        0, // TODO: tree_state.time as i32,
        &hex::decode(tree_state.tree).map_err(|_| anyhow::anyhow!("Not hex"))?,
    )?;
    Ok(())
}

pub fn rewind_to_height(client: Arc<Mutex<Client>>, height: u32, config: &ZamsConfig) -> Result<(), WalletError> {
    log::info!("Rewind to height {}", height);
    let mut data = PostgresWallet::new(client.clone(), config)?;
    data.rewind_to_height(BlockHeight::from_u32(height))?;
    let trp_wallet = TrpWallet::new(client, config.clone())?;
    trp_wallet.rewind_to_height(height)?;
    Ok(())
}
