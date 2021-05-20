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

impl ZcashdConf {
    pub fn parse(url: &str, datadir: &str) -> anyhow::Result<ZcashdConf> {
        let p = Path::new(datadir).join("zcash.conf");
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
}
