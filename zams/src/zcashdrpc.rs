use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use anyhow::bail;
use std::fs;
use std::path::Path;

pub struct ZcashdConf {
    url: String,
    rpc_username: String,
    rpc_password: String,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct TransactionInput {
    txid: Option<String>,
    vout: Option<u32>,
    valueSat: Option<u64>,
    address: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ScriptPubKey {
    addresses: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct TransactionOutput {
    valueSat: u64,
    scriptPubKey: ScriptPubKey
}

#[derive(Debug, Deserialize)]
pub struct TransactionShieldedSpend {
    cv: String,
    anchor: String,
    nullifier: String,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct TransactionShieldedOutput {
    cv: String,
    cmu: String,
    ephemeralKey: String,
    encCiphertext: String,
}

//noinspection RsFieldNaming
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct Transaction {
    txid: String,
    vin: Vec<TransactionInput>,
    vout: Vec<TransactionOutput>,
    vShieldedSpend: Vec<TransactionShieldedSpend>,
    vShieldedOutput: Vec<TransactionShieldedOutput>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct Block {
    hash: String,
    height: u32,
    anchor: String,
    bits: String,
    finalsaplingroot: String,
    merkleroot: String,
    nonce: String,
    previousblockhash: String,
    time: u64,
    tx: Vec<Transaction>,
}

impl ZcashdConf {
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
    client: &Client,
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
    let hash = res
        .as_str().unwrap()
        .to_string();
    Ok(hash)
}

pub async fn get_block(hash: &str, client: &Client, config: &ZcashdConf) -> anyhow::Result<()> {
    let res = make_json_rpc(client, "getblock", json!([hash, 2]), config).await?;
    let block: Block = serde_json::from_value(res)?;
    println!("{:?}", block);
    Ok(())
}

pub async fn get_raw_transaction(hash: &str, client: &Client, config: &ZcashdConf) -> anyhow::Result<()> {
    let res = make_json_rpc(client, "getrawtransaction", json!([hash, 1]), config).await?;
    let tx: Transaction = serde_json::from_value(res)?;
    println!("{:?}", tx);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const ZCASHD_URL: &str = "http://127.0.0.1:8232";
    const DATADIR: &str = "/home/hanh/zcash-main";

    #[tokio::test]
    async fn test_get_best_blockchain() {
        let config = ZcashdConf::parse(ZCASHD_URL, DATADIR).unwrap();
        let client = reqwest::Client::new();
        let hash = get_best_blockhash(&client, &config).await;
        assert!(hash.is_ok());
    }

    #[tokio::test]
    async fn test_get_block() {
        let config = ZcashdConf::parse(ZCASHD_URL, DATADIR).unwrap();
        let client = reqwest::Client::new();
        get_block("0000000000dc923074fea472ad53f3ebaa473f74adbd68f7d00d6409e77e17f7", &client, &config).await.unwrap();
    }

    #[tokio::test]
    async fn test_get_raw_transaction() {
        let config = ZcashdConf::parse(ZCASHD_URL, DATADIR).unwrap();
        let client = reqwest::Client::new();
        get_raw_transaction("3132d3d8006c94f3385606d3f5aa7a6f49d779a82f599eefcc16290ef448b12c", &client, &config).await.unwrap();
    }
}
