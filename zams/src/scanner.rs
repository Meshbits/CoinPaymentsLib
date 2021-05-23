use crate::db::AddressBook;
use crate::decrypt::decrypt_shielded_outputs;
use crate::models::{NewNote, NewTransaction, NewTransactionAndNotes, ViewingKey};
use crate::zcashdrpc::{Block, Transaction, TransactionOutput};

pub fn check_transparent_output<AB: AddressBook>(
    vout_index: i32,
    vout: &TransactionOutput,
    address_book: &AB,
) -> anyhow::Result<Vec<NewNote>> {
    let mut notes = Vec::<NewNote>::new();
    for addr in vout.scriptPubKey.addresses.iter() {
        if address_book.contains(&addr)? {
            let new_note = NewNote {
                tx_id: 0,
                vout_index,
                value: vout.valueSat as i64,
                address: addr.clone(),
                shielded: false,
                locked: false,
                spent: false,
            };
            notes.push(new_note);
        }
    }
    Ok(notes)
}

pub fn check_transparent_outputs<AB: AddressBook>(
    tx: &Transaction,
    address_book: &AB,
) -> anyhow::Result<Vec<NewNote>> {
    let mut notes = Vec::<NewNote>::new();
    for (i, vout) in tx.vout.iter().enumerate() {
        notes.append(&mut check_transparent_output(i as i32, vout, address_book)?);
    }
    Ok(notes)
}

pub fn scan_block<AB: AddressBook>(
    block: &Block,
    ivks: &[ViewingKey],
    address_book: &AB,
) -> anyhow::Result<Vec<NewTransactionAndNotes>> {
    let mut new_transactions = Vec::<NewTransactionAndNotes>::new();
    for tx in block.tx.iter() {
        let mut transparent_notes = check_transparent_outputs(tx, address_book)?;
        let mut shielded_notes = decrypt_shielded_outputs(ivks, tx)?;
        if !transparent_notes.is_empty() || !shielded_notes.is_empty() {
            transparent_notes.append(&mut shielded_notes);
            let new_tx = NewTransactionAndNotes {
                transaction: NewTransaction {
                    block_id: 0,
                    txhash: hex::decode(&tx.txid)?,
                },
                notes: transparent_notes, // contains both t & z
            };
            new_transactions.push(new_tx)
        }
    }

    Ok(new_transactions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testconfig::*;
    use crate::zcashdrpc::{get_block, get_raw_transaction, ZcashdConf};
    use std::collections::HashSet;
    use crate::db::{save_transaction_and_notes, establish_connection, save_block};

    impl AddressBook for HashSet<String> {
        fn contains(&self, address: &str) -> anyhow::Result<bool> {
            Ok(self.contains(address))
        }
    }

    #[tokio::test]
    async fn test_check_transparent_outputs() {
        let mut addresses = HashSet::<String>::new();
        addresses.insert("tmEuJYrkbLTnRSPJJtEuybJHnHxRJ56aNAz".to_string());

        let config = ZcashdConf::parse(TEST_ZCASHD_URL, TEST_DATADIR).unwrap();
        let client = reqwest::Client::new();
        let tx = get_raw_transaction(
            "5437c420e67980a929012453dd879bca3b156e900f7160553c25138bc63c5330",
            &client,
            &config,
        )
        .await
        .unwrap();
        let notes = check_transparent_outputs(&tx, &addresses).unwrap();
        assert!(!notes.is_empty());
    }

    #[tokio::test]
    async fn test_scan_block() {
        let connection = establish_connection("postgres://hanh@localhost/zamsdb");
        let mut addresses = HashSet::<String>::new();
        addresses.insert("tmEuJYrkbLTnRSPJJtEuybJHnHxRJ56aNAz".to_string());

        let config = ZcashdConf::parse(TEST_ZCASHD_URL, TEST_DATADIR).unwrap();
        let client = reqwest::Client::new();
        let block = get_block(
            "0017ad2dc36b30780e36848c4a36d1c4c8d3a3be5801754afa027ce9e539e855",
            &client,
            &config,
        )
        .await
        .unwrap();
        let block_id = save_block(&block, &connection).unwrap();
        let mut txs = scan_block(&block, &[], &addresses).unwrap();
        for tx in txs.iter_mut() {
            tx.transaction.block_id = block_id;
            save_transaction_and_notes(tx, &connection).unwrap();
        }
    }
}
