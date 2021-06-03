pub const CONNECTION_STRING: &str = "host=localhost dbname=saplingdb user=hanh";

mod grpc {
    tonic::include_proto!("cash.z.wallet.sdk.rpc");
}

mod testconfig;

pub mod error;
mod wallet;
mod trp;

pub use crate::wallet::fvk::{generate_keys, import_fvk, import_address};
pub use crate::wallet::scan::{scan_chain, load_checkpoint, rewind_to_height};
pub use crate::wallet::transaction::{prepare_tx, sign_tx, broadcast_tx};
pub use crate::trp::zcashdrpc::ZcashdConf;
