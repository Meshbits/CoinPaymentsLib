use anyhow::bail;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::config::ZamsConfig;
use zcash_client_backend::proto::compact_formats::{CompactBlock, CompactTx, CompactSpend, CompactOutput};
use tokio::runtime::Runtime;
use crate::WalletError;

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

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Transaction {
    pub txid: String,
    pub height: Option<u32>,
    #[serde(default)]
    pub vin: Vec<TransactionInput>,
    #[serde(default)]
    pub vout: Vec<TransactionOutput>,
    #[serde(default)]
    pub vShieldedSpend: Vec<ShieldedSpend>,
    #[serde(default)]
    pub vShieldedOutput: Vec<ShieldedOutput>
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct ShieldedSpend {
    pub cv: String,
    pub anchor: String,
    pub nullifier: String,
    pub rk: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct ShieldedOutput {
    pub cv: String,
    pub cmu: String,
    pub ephemeralKey: String,
    pub encCiphertext: String,
    pub outCiphertext: String,
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

impl Block {
    const COMPACT_NOTE_SIZE: usize = 52;

    pub fn to_compact(&self) -> crate::Result<CompactBlock> {
        let mut cb = CompactBlock::new();
        cb.prevHash = hex::decode(self.previousblockhash.as_ref().unwrap())?;
        cb.hash = hex::decode(&self.hash)?;
        cb.height = self.height as u64;
        // TODO: do we need cb.header ?
        cb.time = self.time as u32;
        for (i, tx) in self.tx.iter().enumerate() {
            let mut ctx = CompactTx::new();
            ctx.index = i as u64;
            ctx.hash = hex::decode(&tx.txid)?;
            ctx.fee = 0;
            for tx_spend in tx.vShieldedSpend.iter() {
                let mut spend = CompactSpend::new();
                spend.nf = hex::decode(&tx_spend.nullifier)?;
                spend.nf.reverse();
                ctx.spends.push(spend);
            }
            for tx_out in tx.vShieldedOutput.iter() {
                let mut output = CompactOutput::new();
                output.cmu = hex::decode(&tx_out.cmu)?;
                output.cmu.reverse();
                output.epk = hex::decode(&tx_out.ephemeralKey)?;
                output.epk.reverse();
                output.ciphertext = hex::decode(&tx_out.encCiphertext)?;
                output.ciphertext.truncate(Block::COMPACT_NOTE_SIZE);
                ctx.outputs.push(output);
            }
            cb.vtx.push(ctx);
        }
        Ok(cb)
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
    config: &ZamsConfig,
) -> anyhow::Result<Value> {
    let body = JsonRpcBody {
        jsonrpc: &"1.0",
        id: &"zams",
        method,
        params,
    };
    let body = serde_json::to_string(&body)?;
    let res = client
        .post(&config.zcashd)
        .header("Content-Type", "application/json")
        .basic_auth(&config.rpc_user, Some(&config.rpc_password))
        .body(body)
        .send()
        .await?;
    let res: Map<String, Value> = res.json().await?;
    if let Some(error) = res.get("error") {
        if let Some(message) = error.get("message") {
            bail!(message.clone())
        }
    }
    Ok(res["result"].clone())
}

pub fn get_latest_height(config: &ZamsConfig) -> crate::Result<u32> {
    let r = Runtime::new().unwrap();
    let height = r.block_on(async {
        let client = reqwest::Client::new();
        let res = make_json_rpc(&client, "getblockcount", json!([]), config).await?;
        let height = res.as_u64().unwrap() as u32;
        Ok::<_, WalletError>(height)
    })?;
    Ok(height)
}

pub struct TreeState {
    pub hash: String,
    pub tree: String,
}

pub fn get_tree_state(height: u32, config: &ZamsConfig) -> crate::Result<TreeState> {
    let r = Runtime::new().unwrap();
    let tree_state = r.block_on(async {
        let client = reqwest::Client::new();
        let height = height.to_string();
        let res = make_json_rpc(&client, "z_gettreestate", json!([&height]), config).await?;
        let hash = res[&"hash"].as_str().unwrap().to_string();
        let tree = res[&"sapling"][&"commitments"][&"finalState"].as_str().unwrap().to_string();
        Ok::<_, WalletError>(TreeState {
            hash,
            tree,
        })
    })?;
    Ok(tree_state)
}

pub fn send_raw_tx(raw_tx: &str, config: &ZamsConfig) -> crate::Result<String> {
    let r = Runtime::new().unwrap();
    let txid = r.block_on(async {
        let client = reqwest::Client::new();
        let res = make_json_rpc(&client, "sendrawtransaction", json!([&raw_tx]), config).await?;
        let txid = res.as_str().unwrap().to_string();
        Ok::<_, WalletError>(txid)
    })?;
    Ok(txid)
}

pub async fn get_block(
    hash_height: &str,
    client: &Client,
    config: &ZamsConfig,
) -> anyhow::Result<Block> {
    let res = make_json_rpc(client, "getblock", json!([hash_height, 2]), config).await?;
    let mut block: Block = serde_json::from_value(res).unwrap();
    for tx in block.tx.iter_mut() {
        tx.height = Some(block.height);
    }
    Ok(block)
}

#[allow(dead_code)]
pub async fn get_raw_transaction(
    hash: &str,
    client: &Client,
    config: &ZamsConfig,
) -> anyhow::Result<Transaction> {
    let res = make_json_rpc(client, "getrawtransaction", json!([hash, 1]), config).await?;
    let tx: Transaction = serde_json::from_value(res)?;
    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ZamsConfig;

    #[tokio::test]
    async fn test_make_json_rpc() {
        let config = ZamsConfig::default();
        let client = reqwest::Client::new();
        let hash = "00030a5790262b189b710903915059257c241a9d21a6dba8c88c3beac3e02b9c";
        let res = make_json_rpc(&client, "getblock", json!([hash, 2]), &config)
            .await
            .unwrap();
        println!("{}", serde_json::to_string(&res).unwrap());
    }

    #[tokio::test]
    async fn test_get_block() {
        let config = ZamsConfig::default();
        let client = reqwest::Client::new();
        get_block(
            "002d1b37e1a0ecc0a90c735cae30f42db0f5384d95c96215d302bc9da8788428",
            &client,
            &config,
        )
        .await
        .unwrap();
        // assert!(get_block("0000000000000000000000000000000000000000000000000000000000000000", &client, &config).await.is_ok());
    }

    #[tokio::test]
    async fn test_get_raw_transaction() {
        let config = ZamsConfig::default();
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
