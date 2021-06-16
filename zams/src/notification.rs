use crate::db::{list_undelivered, mark_delivered};
use crate::{ZamsConfig, WalletError};
use anyhow::Context;
use postgres::Client;
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct NotificationRecord {
    pub id: i32,
    pub eventType: String,
    pub txHash: String,
    pub account: i32,
    pub address: Option<String>,
    pub txOutputIndex: i32,
    pub amount: i64,
    pub block: u32,
}

pub fn notify_tx(client: &mut Client, config: &ZamsConfig) -> crate::Result<()> {
    let notifications = list_undelivered(client)?;
    if !notifications.is_empty() {
        let body = serde_json::to_string(&notifications).context("Cannot serialize notification")?;
        let rest_client = reqwest::Client::new();
        let r = Runtime::new().unwrap();
        r.block_on(async {
            let res = rest_client
                .post(&config.notification_url)
                .body(body)
                .header("Content-Type", "application/json")
                .send()
                .await
                .map_err(WalletError::Reqwest)?;
            res.error_for_status()?;
            Ok::<_, WalletError>(())
        })?;
        for n in notifications {
            mark_delivered(&mut *client, n.id)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notification::NotificationRecord;
    use crate::ZamsConfig;
    use postgres::{Client, NoTls};

    #[test]
    fn test_store_notification() {
        let config = ZamsConfig::default();
        let mut client = Client::connect(&config.connection_string, NoTls).unwrap();
        let record = NotificationRecord {
            id: 0,
            eventType: "outgoingTx".to_string(),
            txHash: "d04bb83a234496e033fbd480d24be47a53d38b984cfc575b05bc24580e44a42d".to_string(),
            account: 1,
            address: None, // ignored
            txOutputIndex: 5,
            amount: 1000,
            block: 1447639,
        };
        crate::db::store_notification(&mut client, &record).unwrap();
        notify_tx(&mut client, &config).unwrap();
    }
}
