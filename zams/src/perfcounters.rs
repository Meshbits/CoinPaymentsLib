use prometheus::{Registry, Counter, IntCounter};
use warp::{Rejection, Reply};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    pub static ref ACCOUNTS: IntCounter = IntCounter::new("account_count", "Number of Accounts").unwrap();
    pub static ref PAYMENTS: Counter = Counter::new("payments_amount", "Payment Amount Processed").unwrap();
    pub static ref PREPARED_PAYMENTS: IntCounter = IntCounter::new("payments_prepared", "Number Payments Prepared").unwrap();
    pub static ref BROADCAST_PAYMENTS: IntCounter = IntCounter::new("payments_broadcast", "Number Payments Broadcasted").unwrap();
    pub static ref REQUESTS: IntCounter = IntCounter::new("requests", "Number of Requests").unwrap();
    pub static ref RECEIVED_NOTES: IntCounter = IntCounter::new("received_notes", "Number of Notes Received").unwrap();
    pub static ref RECEIVED_AMOUNT: Counter = Counter::new("received_amount", "Amount Received").unwrap();
    pub static ref TRANSACTIONS: IntCounter = IntCounter::new("transactions_scanned", "Number of Transactions Scanned").unwrap();
}

pub fn register_custom_metrics() {
    REGISTRY.register(Box::new(ACCOUNTS.clone())).unwrap();
    REGISTRY.register(Box::new(PAYMENTS.clone())).unwrap();
    REGISTRY.register(Box::new(PREPARED_PAYMENTS.clone())).unwrap();
    REGISTRY.register(Box::new(BROADCAST_PAYMENTS.clone())).unwrap();
    REGISTRY.register(Box::new(REQUESTS.clone())).unwrap();
    REGISTRY.register(Box::new(RECEIVED_NOTES.clone())).unwrap();
    REGISTRY.register(Box::new(RECEIVED_AMOUNT.clone())).unwrap();
    REGISTRY.register(Box::new(TRANSACTIONS.clone())).unwrap();
}

pub async fn metrics_handler() -> Result<impl Reply, Rejection> {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&REGISTRY.gather(), &mut buffer) {
        eprintln!("could not encode custom metrics: {}", e);
    };
    let mut res = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("custom metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&prometheus::gather(), &mut buffer) {
        eprintln!("could not encode prometheus metrics: {}", e);
    };
    let res_custom = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("prometheus metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    res.push_str(&res_custom);
    Ok(res)
}
