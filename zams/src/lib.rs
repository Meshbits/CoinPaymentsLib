#[path = "generated/zams.rs"]
pub mod zams_rpc;

pub type Result<T> = std::result::Result<T, WalletError>;

pub mod config;
pub mod error;

mod db;
mod keys;
mod perfcounters;
mod trp;
mod wallet;
mod notification;
mod utils;

pub use crate::config::ZamsConfig;
pub use crate::db::{
    cancel_payment, generate_address, get_balance, get_payment_info, import_address, import_fvk,
    list_pending_payments, DbPreparedStatements,
};
pub use crate::error::WalletError;
pub use crate::keys::{generate_sapling_keys, generate_transparent_address, get_bip39_seed};
pub use crate::perfcounters::{metrics_handler, register_custom_metrics, REGISTRY, REQUESTS};
pub use crate::trp::zcashdrpc::get_latest_height;
pub use crate::trp::TrpWallet;
pub use crate::utils::{populate_taddr, populate_zaddr};
pub use crate::wallet::scan::{load_checkpoint, rewind_to_height, scan_chain};
pub use crate::wallet::transaction::{broadcast_tx, prepare_tx, sign_tx};

pub const ZATPERZEC: f64 = 1e8;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
