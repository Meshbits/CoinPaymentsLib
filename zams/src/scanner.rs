use crate::db::AddressBook;
use crate::models::NewNote;
use crate::zcashdrpc::{Transaction, TransactionOutput};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testconfig::*;
    use std::collections::HashSet;
    use crate::zcashdrpc::{ZcashdConf, get_raw_transaction};

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
        let tx = get_raw_transaction("5437c420e67980a929012453dd879bca3b156e900f7160553c25138bc63c5330", &client, &config).await.unwrap();
        let notes = check_transparent_outputs(&tx, &addresses).unwrap();
        assert!(!notes.is_empty());
    }
}