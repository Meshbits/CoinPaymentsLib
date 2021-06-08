pub const CONNECTION_STRING: &str = "host=localhost dbname=saplingdb user=hanh";

mod grpc {
    tonic::include_proto!("cash.z.wallet.sdk.rpc");
}

#[path = "generated/zams.rs"]
pub mod zams_rpc;

mod testconfig;

pub type Result<T> = std::result::Result<T, WalletError>;

pub mod db;
pub mod error;
mod trp;
mod wallet;
mod keys;

use crate::error::WalletError;
pub use crate::trp::zcashdrpc::ZcashdConf;
pub use crate::wallet::scan::{load_checkpoint, rewind_to_height, scan_chain, get_latest_height};
pub use crate::wallet::transaction::{broadcast_tx, prepare_tx, sign_tx};
pub use crate::keys::*;

#[cfg(not(feature = "mainnet"))]
pub mod constants {
    use zcash_primitives::consensus::Network::{self, TestNetwork};
    pub const NETWORK: Network = TestNetwork;
}

#[cfg(feature = "mainnet")]
pub mod constants {
    use zcash_primitives::consensus::Network::{self, MainNetwork};
    pub const NETWORK: Network = MainNetwork;
}
