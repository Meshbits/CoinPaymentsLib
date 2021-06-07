use crate::db::{trp_rewind_to_height, DbPreparedStatements};
use crate::error::WalletError;
use crate::trp::zcashdrpc::{get_block, Block, Transaction, ZcashdConf};
use crate::wallet::scan::get_scan_range;
use crate::wallet::PostgresWallet;
use maplit::hashmap;
use postgres::{Client, Statement};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::{DerefMut, Range, RangeInclusive};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

pub mod zcashdrpc;

pub struct BlockSource {
    config: ZcashdConf,
}

impl BlockSource {
    pub fn new(config: &ZcashdConf) -> BlockSource {
        BlockSource {
            config: config.clone(),
        }
    }

    pub fn with_blocks<F>(&self, range: Range<u32>, mut with_block: F) -> Result<(), WalletError>
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
    client: Arc<Mutex<Client>>,
    statements: DbPreparedStatements,
    addresses: HashMap<String, i32>,
}

impl TrpWallet {
    pub fn new(c: Arc<Mutex<Client>>) -> crate::Result<TrpWallet> {
        let mut client = c.lock().unwrap();
        let statements = DbPreparedStatements::prepare(&mut client)?;
        Ok(TrpWallet {
            client: c.clone(),
            statements,
            addresses: HashMap::new(),
        })
    }

    fn scan_inputs(&self, tx: &Transaction, client: &mut Client) -> Result<(), WalletError> {
        for input in tx.vin.iter() {
            if let Some(ref address) = input.address {
                if let Some(account) = self.addresses.get(address.as_str()) {
                    let txid = hex::decode(input.txid.as_ref().unwrap())?;
                    let script: Vec<u8> = vec![];
                    client.execute(
                        &self.statements.upsert_spent_utxo,
                        &[
                            &txid,
                            account,
                            address,
                            &(input.vout.unwrap() as i32),
                            &(input.valueSat.unwrap() as i64),
                            &script,
                            &0,
                            &true,
                            &(tx.height.unwrap() as i32),
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
                if let Some(account) = self.addresses.get(address.as_str()) {
                    let txid = hex::decode(&tx.txid)?;
                    client.execute(
                        &self.statements.upsert_spent_utxo,
                        &[
                            &txid,
                            account,
                            address,
                            &(index as i32),
                            &(output.valueSat as i64),
                            &hex::decode(&output.scriptPubKey.hex).unwrap(),
                            &(tx.height.unwrap() as i32),
                            &false,
                            &Option::<i32>::None,
                        ],
                    )?;
                }
            }
        }
        Ok(())
    }

    pub fn load_transparent_addresses_from_db(&mut self) -> Result<(), WalletError> {
        let mut c = self.client.lock().unwrap();
        let addresses = crate::db::get_all_trp_addresses(c.deref_mut())?;
        self.addresses
            .extend(addresses.iter().map(|(id, addr)| (addr.clone(), *id)));
        Ok(())
    }

    pub fn scan_range(
        &mut self,
        range: Range<u32>,
        config: &ZcashdConf,
    ) -> Result<(), WalletError> {
        let mut c = self.client.lock().unwrap();
        let source = BlockSource::new(&config);
        source.with_blocks(range, |block| {
            for tx in block.tx.iter() {
                self.scan_inputs(tx, c.deref_mut())?;
                self.scan_outputs(tx, c.deref_mut())?;
            }
            Ok(())
        })
    }

    pub fn rewind_to_height(&self, height: u32) -> Result<(), WalletError> {
        let mut c = self.client.lock().unwrap();
        let mut db_tx = c.transaction()?;
        trp_rewind_to_height(&mut db_tx, height)?;
        db_tx.commit()?;
        Ok(())
    }

    pub fn scan_transparent(
        &mut self,
        range: Range<u32>,
        config: &ZcashdConf,
    ) -> Result<(), WalletError> {
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
    use crate::CONNECTION_STRING;
    use postgres::NoTls;

    #[test]
    fn test_with_block() {
        let config = ZcashdConf::parse(TEST_ZCASHD_URL, TEST_DATADIR).unwrap();
        let client = Client::connect(CONNECTION_STRING, NoTls).unwrap();
        let client = Arc::new(Mutex::new(client));
        let mut wallet = TrpWallet::new(client).unwrap();
        wallet.load_transparent_addresses_from_db().unwrap();
        wallet.scan_range(1_432_000..1_432_138, &config).unwrap();
    }
}
