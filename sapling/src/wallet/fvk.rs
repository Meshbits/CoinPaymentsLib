use crate::error::WalletError;
use crate::wallet::PostgresWallet;

pub fn import_fvk(fvk: &str) -> Result<i32, WalletError> {
    let wallet = PostgresWallet::new()?;
    let id_fvk = wallet.import_fvk(fvk)?;
    Ok(id_fvk)
}

pub fn import_address(address: &str) ->Result<i32, WalletError> {
    let wallet = PostgresWallet::new()?;
    let id_account = wallet.import_address(address)?;
    Ok(id_account)
}

pub fn generate_keys(
    id_fvk: i32,
    diversifier_index: u128,
) -> std::result::Result<(String, u128), WalletError> {
    let wallet = PostgresWallet::new()?;
    let r = wallet.generate_keys(id_fvk, diversifier_index)?;
    Ok(r)
}
