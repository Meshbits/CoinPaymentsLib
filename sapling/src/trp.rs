use crate::error::WalletError;
use crate::trp::zcashdrpc::{get_block, Block, Transaction, ZcashdConf};
use crate::wallet::scan::get_scan_range;
use crate::wallet::PostgresWallet;
use maplit::hashmap;
use postgres::{Client, Statement};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::{RangeInclusive, Range, DerefMut};
use std::sync::Arc;
use tokio::runtime::Runtime;
use std::rc::Rc;
use crate::db::DbPreparedStatements;

pub mod zcashdrpc;
pub mod db;

pub struct BlockSource {
    config: ZcashdConf,
}

impl BlockSource {
    pub fn new(config: &ZcashdConf) -> BlockSource {
        BlockSource {
            config: config.clone(),
        }
    }

    pub fn with_blocks<F>(
        &self,
        range: Range<u32>,
        mut with_block: F,
    ) -> Result<(), WalletError>
    where
        F: FnMut(Block) -> Result<(), WalletError>,
    {
        let r = Runtime::new().unwrap();
        let client = reqwest::Client::new();
        for height in range {
            let block = r.block_on(get_block(&height.to_string(), &client, &self.config))?;
            with_block(block)?;
        }
        Ok(())
    }
}

pub struct TrpWallet {
    statements: DbPreparedStatements,
    addresses: HashMap<String, i32>,
}

impl TrpWallet {
    pub fn new(c: Rc<RefCell<Client>>) -> crate::Result<TrpWallet> {
        let statements = DbPreparedStatements::prepare(c)?;
        Ok(TrpWallet {
            statements,
            addresses: HashMap::new(),
        })
    }

    fn scan_inputs(&self, tx: &Transaction, client: &mut Client) -> Result<(), WalletError> {
        for input in tx.vin.iter() {
            if let Some(ref address) = input.address {
                if self.addresses.contains_key(address.as_str()) {
                    let txid = hex::decode(input.txid.as_ref().unwrap())?;
                    let script: Option<String> = None;
                    client.execute(
                        &self.statements.upsert_spent_utxo,
                        &[
                            &txid,
                            address,
                            &(input.vout.unwrap() as i32),
                            &(input.valueSat.unwrap() as i64),
                            &script,
                            &0,
                            &true,
                            &(tx.height.unwrap() as i32)
                        ],
                    )?;
                }
            }
        }
        Ok(())
    }

    fn scan_outputs(&self, tx: &Transaction, client: &mut Client) -> Result<(), WalletError> {
        for (index, output) in tx.vout.iter().enumerate() {
            for address in output.scriptPubKey.addresses.iter() {
                if self.addresses.contains_key(address.as_str()) {
                    let txid = hex::decode(&tx.txid)?;
                    client.execute(
                        &self.statements.upsert_spent_utxo,
                        &[
                            &txid,
                            address,
                            &(index as i32),
                            &(output.valueSat as i64),
                            &Some(hex::decode(&output.scriptPubKey.hex).unwrap()),
                            &(tx.height.unwrap() as i32),
                            &false,
                            &Option::<i32>::None
                        ],
                    )?;
                }
            }
        }
        Ok(())
    }

    pub fn load_transparent_addresses_from_db(&mut self) -> Result<(), WalletError> {
        let mut c = self.statements.client.borrow_mut();
        let addresses = crate::db::get_all_trp_addresses(c.deref_mut())?;
        self
        .addresses
        .extend(addresses.iter().map(|(id, addr)| (addr.clone(), *id)));
        Ok(())
    }

    pub fn scan_range(&mut self, range: Range<u32>, config: &ZcashdConf) -> Result<(), WalletError> {
        let mut c = self.statements.client.borrow_mut();
        let source = BlockSource::new(&config);
        source
            .with_blocks(range, |block| {
                for tx in block.tx.iter() {
                    self.scan_inputs(tx, c.deref_mut())?;
                    self.scan_outputs(tx, c.deref_mut())?;
                }
                Ok(())
            })
    }

    pub fn rewind_to_height(&self, height: u32) -> Result<(), WalletError> {
        let mut c = self.statements.client.borrow_mut();
        let mut db_tx = c.transaction()?;
        db::trp_rewind_to_height(&mut db_tx, height)?;
        db_tx.commit()?;
        Ok(())
    }

    pub fn scan_transparent(&mut self, range: Range<u32>, config: &ZcashdConf) -> Result<(), WalletError> {
        self.load_transparent_addresses_from_db()?;
        self.scan_range(range, &config)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testconfig::{TEST_DATADIR, TEST_ZCASHD_URL};
    use crate::wallet::PostgresWallet;

    #[test]
    fn test_with_block() {
        let config = ZcashdConf::parse(TEST_ZCASHD_URL, TEST_DATADIR).unwrap();
        let sapling_wallet = PostgresWallet::new().unwrap();
        let mut wallet = TrpWallet::new(sapling_wallet.connection).unwrap();
        wallet.load_transparent_addresses_from_db().unwrap();
        wallet.scan_range(1_432_000..1_432_138, &config).unwrap();
    }
}
