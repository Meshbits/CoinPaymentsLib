use crate::db::DbPreparedStatements;
use crate::CONNECTION_STRING;
use anyhow::bail;
use postgres::NoTls;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use crate::testconfig::{TEST_ZCASHD_URL, TEST_DATADIR};

#[derive(Debug, Clone)]
pub struct ZcashdConf {
    url: String,
    rpc_username: String,
    rpc_password: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct TransactionInput {
    pub txid: Option<String>,
    pub vout: Option<u32>,
    pub valueSat: Option<u64>,
    pub address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScriptPubKey {
    #[serde(default)]
    pub addresses: Vec<String>,
    pub hex: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct TransactionOutput {
    pub valueSat: u64,
    pub scriptPubKey: ScriptPubKey,
}

//noinspection RsFieldNaming
#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Transaction {
    pub txid: String,
    pub height: Option<u32>,
    pub vin: Vec<TransactionInput>,
    pub vout: Vec<TransactionOutput>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Block {
    pub hash: String,
    pub height: u32,
    pub anchor: String,
    pub previousblockhash: Option<String>,
    pub nextblockhash: Option<String>,
    pub time: u64,
    pub tx: Vec<Transaction>,
}

impl ZcashdConf {
    pub fn new() -> ZcashdConf {
        ZcashdConf::parse(TEST_ZCASHD_URL, TEST_DATADIR).unwrap()
    }
    pub fn parse(url: &str, datadir: &str) -> anyhow::Result<ZcashdConf> {
        let p = Path::new(datadir).join("zams.toml");
        let conf_str = fs::read_to_string(p)?;
        let conf: toml::Value = toml::from_str(&conf_str)?;
        let table = conf.as_table().unwrap();
        let rpc_username = table.get("rpcuser").unwrap().as_str().unwrap();
        let rpc_password = table.get("rpcpassword").unwrap().as_str().unwrap();

        Ok(ZcashdConf {
            url: url.to_owned(),
            rpc_username: rpc_username.to_owned(),
            rpc_password: rpc_password.to_owned(),
        })
    }
}

#[derive(Serialize, Deserialize)]
struct JsonRpcBody<'a> {
    jsonrpc: &'static str,
    id: &'static str,
    method: &'a str,
    params: Value,
}

pub async fn make_json_rpc(
    client: &reqwest::Client,
    method: &str,
    params: Value,
    config: &ZcashdConf,
) -> anyhow::Result<Value> {
    let body = JsonRpcBody {
        jsonrpc: &"1.0",
        id: &"zams",
        method,
        params,
    };
    let body = serde_json::to_string(&body)?;
    let res = client
        .post(&config.url)
        .header("Content-Type", "application/json")
        .basic_auth(&config.rpc_username, Some(&config.rpc_password))
        .body(body)
        .send()
        .await?;
    let res = res.error_for_status()?;
    let res: Map<String, Value> = res.json().await?;
    let res = match res.get("error") {
        Some(Value::Null) => res.get("result").unwrap().clone(),
        Some(Value::String(s)) => bail!(s.clone()),
        _ => bail!("unknown error"),
    };
    Ok(res)
}

pub async fn get_best_blockhash(client: &Client, config: &ZcashdConf) -> anyhow::Result<String> {
    let res = make_json_rpc(client, "getbestblockhash", json!([]), config).await?;
    let hash = res.as_str().unwrap().to_string();
    Ok(hash)
}

pub async fn get_block(
    hash_height: &str,
    client: &Client,
    config: &ZcashdConf,
) -> anyhow::Result<Block> {
    let res = make_json_rpc(client, "getblock", json!([hash_height, 2]), config).await?;
    let mut block: Block = serde_json::from_value(res).unwrap();
    for tx in block.tx.iter_mut() {
        tx.height = Some(block.height);
    }
    Ok(block)
}

pub async fn get_raw_transaction(
    hash: &str,
    client: &Client,
    config: &ZcashdConf,
) -> anyhow::Result<Transaction> {
    let res = make_json_rpc(client, "getrawtransaction", json!([hash, 1]), config).await?;
    let tx: Transaction = serde_json::from_value(res)?;
    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testconfig::*;

    #[tokio::test]
    async fn test_get_best_blockchain() {
        let config = ZcashdConf::parse(TEST_ZCASHD_URL, TEST_DATADIR).unwrap();
        let client = reqwest::Client::new();
        let hash = get_best_blockhash(&client, &config).await;
        assert!(hash.is_ok());
    }

    #[tokio::test]
    async fn test_make_json_rpc() {
        let client = reqwest::Client::new();
        let config = ZcashdConf::parse(TEST_ZCASHD_URL, TEST_DATADIR).unwrap();
        let hash = "00030a5790262b189b710903915059257c241a9d21a6dba8c88c3beac3e02b9c";
        let res = make_json_rpc(&client, "getblock", json!([hash, 2]), &config)
            .await
            .unwrap();
        println!("{}", serde_json::to_string(&res).unwrap());
    }

    #[tokio::test]
    async fn test_get_block() {
        let config = ZcashdConf::parse(TEST_ZCASHD_URL, TEST_DATADIR).unwrap();
        let client = reqwest::Client::new();
        get_block(
            "00030a5790262b189b710903915059257c241a9d21a6dba8c88c3beac3e02b9c",
            &client,
            &config,
        )
        .await
        .unwrap();
        // assert!(get_block("0000000000000000000000000000000000000000000000000000000000000000", &client, &config).await.is_ok());
    }

    #[tokio::test]
    async fn test_get_raw_transaction() {
        let config = ZcashdConf::parse(TEST_ZCASHD_URL, TEST_DATADIR).unwrap();
        let client = reqwest::Client::new();
        let tx = get_raw_transaction(
            "a0a8689597f119d02e07930c38d70c411e4b711f5d119f635bae31fe3d38d659",
            &client,
            &config,
        )
        .await
        .unwrap();
        println!("{:?}", tx);
    }
}
