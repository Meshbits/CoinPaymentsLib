use crate::schema::{blocks, transactions, accounts, viewing_keys, notes};

#[derive(Insertable)]
#[table_name="blocks"]
pub struct NewBlock {
    pub height: i32,
    pub anchor: Vec<u8>,
    pub hash: Vec<u8>,
    pub prevhash: Vec<u8>,
}

#[derive(Queryable, Debug)]
pub struct Block {
    pub id: i32,
    pub height: i32,
    pub anchor: Vec<u8>,
    pub hash: Vec<u8>,
    pub prevhash: Vec<u8>,
}

#[derive(Insertable, Debug)]
#[table_name="transactions"]
pub struct NewTransaction {
    pub block_id: i32,
    pub txhash: Vec<u8>,
}

#[derive(Queryable, Debug)]
pub struct Transaction {
    pub id: i32,
    pub txhash: Vec<u8>,
    pub block_id: i32,
}

#[derive(Insertable)]
#[table_name="viewing_keys"]
pub struct NewViewingKey {
    pub key: String,
}

#[derive(Queryable, Debug)]
pub struct ViewingKey {
    pub id: i32,
    pub key: String,
}

#[derive(Insertable)]
#[table_name="accounts"]
pub struct NewAccount {
    pub address: String,
    pub viewing_key_id: Option<i32>,
    pub diversifier_index_high: Option<i64>,
    pub diversifier_index_low: Option<i64>,
    pub user_id: Option<i32>,
}

#[derive(Queryable, Debug)]
pub struct Account {
    pub id: i32,
    pub address: String,
    pub viewing_key_id: Option<i32>,
    pub diversifier_index_high: Option<i64>,
    pub diversifier_index_low: Option<i64>,
    pub user_id: Option<i32>,
}

#[derive(Insertable, Debug)]
#[table_name="notes"]
pub struct NewNote {
    pub tx_id: i32,
    pub vout_index: i32,
    pub value: i64,
    pub address: String,
    pub shielded: bool,
    pub locked: bool,
    pub spent: bool,
}

#[derive(Queryable, Debug)]
pub struct Note {
    pub id: i32,
    pub tx_id: i32,
    pub vout_index: i32,
    pub value: i64,
    pub address: String,
    pub shielded: bool,
    pub locked: bool,
    pub spent: bool,
}

#[derive(Debug)]
pub struct NewTransactionAndNotes {
    pub transaction: NewTransaction,
    pub notes: Vec<NewNote>,
}
