use crate::schema::{blocks, transactions, accounts, viewing_keys};

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

#[derive(Insertable)]
#[table_name="transactions"]
pub struct NewTransaction {
    pub block_id: i32,
    pub txid: Vec<u8>,
}

#[derive(Queryable, Debug)]
pub struct Transaction {
    pub id: i32,
    pub txid: Vec<u8>,
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
    pub diversifier_index: Option<i32>,
    pub user_id: Option<i32>,
}

#[derive(Queryable, Debug)]
pub struct Account {
    pub id: i32,
    pub address: String,
    pub viewing_key_id: Option<i32>,
    pub diversifier_index: Option<i32>,
    pub user_id: Option<i32>,
}
