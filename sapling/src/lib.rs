mod grpc {
    tonic::include_proto!("cash.z.wallet.sdk.rpc");
}

#[path = "generated/zams.rs"]
pub mod zams_rpc;

pub type Result<T> = std::result::Result<T, WalletError>;

pub mod config;
pub mod error;

mod db;
mod trp;
mod wallet;
mod keys;

pub use crate::config::ZamsConfig;
pub use crate::error::WalletError;
pub use crate::db::{DbPreparedStatements, get_balance, import_address, generate_address, cancel_payment,
import_fvk, get_payment_info, list_pending_payments};
pub use crate::wallet::scan::{load_checkpoint, rewind_to_height, scan_chain, get_latest_height};
pub use crate::wallet::transaction::{broadcast_tx, prepare_tx, sign_tx};
pub use crate::keys::{get_bip39_seed, generate_sapling_keys, generate_transparent_address};

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
