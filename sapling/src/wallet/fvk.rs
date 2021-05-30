use crate::error::WalletError;
use crate::wallet::PostgresWallet;





pub fn import_fvk(fvk: &str) -> std::result::Result<i32, WalletError> {
    let wallet = PostgresWallet::new()?;
    let id_fvk = wallet.import_fvk(fvk)?;
    Ok(id_fvk)
}

pub fn generate_keys(
    id_fvk: i32,
    diversifier_index: u128,
) -> std::result::Result<(String, u128), WalletError> {
    let wallet = PostgresWallet::new()?;
    let r = wallet.generate_keys(id_fvk, diversifier_index)?;
    Ok(r)
}
