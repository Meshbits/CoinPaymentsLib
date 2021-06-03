pub const CONNECTION_STRING: &str = "host=localhost dbname=saplingdb user=hanh";

mod grpc {
    tonic::include_proto!("cash.z.wallet.sdk.rpc");
}

mod testconfig;

pub type Result<T> = std::result::Result::<T, WalletError>;

pub mod error;
pub mod db;
mod wallet;
mod trp;

pub use crate::wallet::scan::{scan_chain, load_checkpoint, rewind_to_height};
pub use crate::wallet::transaction::{prepare_tx, sign_tx, broadcast_tx};
pub use crate::trp::zcashdrpc::ZcashdConf;
use crate::error::WalletError;
