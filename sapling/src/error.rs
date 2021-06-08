use anyhow::anyhow;
use zcash_client_backend::data_api;

#[derive(Debug)]
pub enum WalletError {
    Bech32(bech32::Error),
    IncorrectHrpExtFvk,
    DataError(data_api::error::Error<i32>),
    IO(std::io::Error),
    InvalidNote,
    Error(anyhow::Error),
    Postgres(postgres::Error),
    Tonic(tonic::Status),
    TxBuilder(zcash_primitives::transaction::builder::Error),
    Reorg,
}

impl From<data_api::error::Error<i32>> for WalletError {
    fn from(e: data_api::error::Error<i32>) -> Self {
        WalletError::DataError(e)
    }
}

impl From<anyhow::Error> for WalletError {
    fn from(e: anyhow::Error) -> Self {
        WalletError::Error(e)
    }
}

impl From<postgres::Error> for WalletError {
    fn from(e: postgres::Error) -> Self {
        WalletError::Postgres(e)
    }
}

impl From<tonic::Status> for WalletError {
    fn from(e: tonic::Status) -> Self {
        WalletError::Tonic(e)
    }
}

impl From<hex::FromHexError> for WalletError {
    fn from(_: hex::FromHexError) -> Self {
        WalletError::Error(anyhow!("Could not decode hex string"))
    }
}

impl From<zcash_primitives::transaction::builder::Error> for WalletError {
    fn from(e: zcash_primitives::transaction::builder::Error) -> Self {
        WalletError::TxBuilder(e)
    }
}

impl From<WalletError> for tonic::Status {
    fn from(e: WalletError) -> Self {
        tonic::Status::internal(format!("{:?}", e))
    }
}
