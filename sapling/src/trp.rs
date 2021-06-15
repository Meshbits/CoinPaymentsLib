use crate::db::{trp_rewind_to_height, DbPreparedStatements, store_notification};
use crate::error::WalletError;
use crate::trp::zcashdrpc::{get_block, Block, Transaction};

use postgres::Client;

use std::collections::HashMap;
use std::ops::Range;

use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use crate::{db, ZATPERZEC, ZamsConfig};
use crate::notification::NotificationRecord;

pub mod zcashdrpc;

pub struct BlockSource {
    config: ZamsConfig,
    client: Arc<Mutex<Client>>,
}

impl BlockSource {
    pub fn new(client: Arc<Mutex<Client>>, config: &ZamsConfig) -> BlockSource {
        BlockSource {
            config: config.clone(),
            client,
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
            let prev_block_hash = {
                let mut client = self.client.lock().unwrap();
                db::get_block_by_height(&mut *client, height - 1)?
            };
            if let (Some(ph), Some(ph2)) = (prev_block_hash, block.previousblockhash.as_ref()) {
                if hex::encode(ph) != *ph2 {
                    return Err(WalletError::Reorg)
                }
            }
            with_block(block)?;
        }
        Ok(())
    }
}

pub struct TrpWallet {
    config: ZamsConfig,
    client: Arc<Mutex<Client>>,
    statements: DbPreparedStatements,
    addresses: HashMap<String, i32>,
}

impl TrpWallet {
    pub fn new(c: Arc<Mutex<Client>>, config: ZamsConfig) -> crate::Result<TrpWallet> {
        let mut client = c.lock().unwrap();
        let statements = DbPreparedStatements::prepare(&mut client)?;
        Ok(TrpWallet {
            config,
            client: c.clone(),
            statements,
            addresses: HashMap::new(),
        })
    }

    fn scan_inputs(&self, tx: &Transaction, notifications: &mut Vec<NotificationRecord>, client: &mut Client) -> Result<(), WalletError> {
        for input in tx.vin.iter() {
            if let Some(ref address) = input.address {
                if let Some(account) = self.addresses.get(address.as_str()) {
                    crate::perfcounters::RECEIVED_NOTES.inc();
                    crate::perfcounters::RECEIVED_AMOUNT.inc_by((input.valueSat.unwrap() as f64) / ZATPERZEC);
                    let tx_hash = input.txid.as_ref().unwrap().clone();
                    let txid = hex::decode(&tx_hash)?;
                    let script: Vec<u8> = vec![];
                    let amount = input.valueSat.unwrap() as i64;
                    client.execute(
                        &self.statements.upsert_spent_utxo,
                        &[
                            &txid,
                            account,
                            address,
                            &(input.vout.unwrap() as i32),
                            &amount,
                            &script,
                            &0,
                            &true,
                            &(tx.height.unwrap() as i32),
                        ],
                    )?;
                    let notification = NotificationRecord {
                        id: 0, // ignored
                        eventType: "outgoingTx".to_string(),
                        txHash: tx_hash,
                        account: *account,
                        address: None,
                        txOutputIndex: input.vout.unwrap() as i32,
                        amount,
                        block: tx.height.unwrap(),
                    };
                    notifications.push(notification);
                }
            }
        }
        Ok(())
    }

    fn scan_outputs(&self, tx: &Transaction, notifications: &mut Vec<NotificationRecord>, client: &mut Client) -> Result<(), WalletError> {
        for (index, output) in tx.vout.iter().enumerate() {
            for address in output.scriptPubKey.addresses.iter() {
                if let Some(account) = self.addresses.get(address.as_str()) {
                    let txid = hex::decode(&tx.txid)?;
                    let amount = output.valueSat as i64;
                    client.execute(
                        &self.statements.upsert_spent_utxo,
                        &[
                            &txid,
                            account,
                            address,
                            &(index as i32),
                            &amount,
                            &hex::decode(&output.scriptPubKey.hex).unwrap(),
                            &(tx.height.unwrap() as i32),
                            &false,
                            &Option::<i32>::None,
                        ],
                    )?;
                    let notification = NotificationRecord {
                        id: 0, // ignored
                        eventType: "incomingTx".to_string(),
                        txHash: tx.txid.clone(),
                        account: *account,
                        address: None,
                        txOutputIndex: index as i32,
                        amount,
                        block: tx.height.unwrap(),
                    };
                    notifications.push(notification);
                }
            }
        }
        Ok(())
    }

    pub fn load_transparent_addresses_from_db(&mut self) -> Result<(), WalletError> {
        let mut c = self.client.lock().unwrap();
        let addresses = crate::db::get_all_trp_addresses(&mut *c)?;
        self.addresses
            .extend(addresses.iter().map(|(id, addr)| (addr.clone(), *id)));
        Ok(())
    }

    pub fn scan_range(
        &mut self,
        range: Range<u32>
    ) -> Result<(), WalletError> {
        let mut notifications: Vec<NotificationRecord> = Vec::new();
        let source = BlockSource::new(self.client.clone(), &self.config);
        source.with_blocks(range, |block| {
            let mut c = self.client.lock().unwrap();
            for tx in block.tx.iter() {
                self.scan_inputs(tx, &mut notifications, &mut *c)?;
                self.scan_outputs(tx, &mut notifications, &mut *c)?;
                crate::perfcounters::TRANSACTIONS.inc();
            }
            Ok(())
        })?;
        let mut c = self.client.lock().unwrap();
        for n in notifications.iter() {
            store_notification(&mut *c, n)?;
        }
        Ok(())
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
        range: Range<u32>
    ) -> Result<(), WalletError> {
        self.load_transparent_addresses_from_db()?;
        self.scan_range(range)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use postgres::NoTls;
    use crate::config::ZamsConfig;

    #[test]
    fn test_with_block() {
        let config = ZamsConfig::default();
        let client = Client::connect(&config.connection_string, NoTls).unwrap();
        let client = Arc::new(Mutex::new(client));
        let mut wallet = TrpWallet::new(client, config.clone()).unwrap();
        wallet.load_transparent_addresses_from_db().unwrap();
        wallet.scan_range(1_432_000..1_432_138).unwrap();
    }
}
