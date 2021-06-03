use crate::error::WalletError;
use postgres::GenericClient;

pub fn trp_rewind_to_height<C: GenericClient>(client: &mut C, height: u32) -> Result<(), WalletError> {
    client.execute("DELETE FROM utxos WHERE height > $1", &[&(height as i32)])?;
    client.execute("UPDATE utxos set spent = FALSE, spent_height = NULL WHERE spent_height > $1", &[&(height as i32)])?;
    Ok(())
}
