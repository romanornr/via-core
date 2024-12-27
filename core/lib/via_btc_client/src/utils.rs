use bitcoin::{hashes::Hash, Txid};
use tokio::time::Duration;

use crate::types;

pub(crate) async fn with_retry<F, T, E>(
    f: F,
    max_retries: u8,
    retry_delay_ms: u64,
    operation_name: &str,
) -> Result<T, E>
where
    F: Fn() -> Result<T, E> + Send + Sync,
    E: std::fmt::Debug,
{
    let mut retries = 0;
    loop {
        match f() {
            Ok(result) => return Ok(result),
            Err(e) if retries < max_retries => {
                tracing::warn!(
                    error = ?e,
                    retries,
                    "{} failed, retrying",
                    operation_name
                );
                retries += 1;
                tokio::time::sleep(Duration::from_millis(retry_delay_ms)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

pub fn bytes_to_txid(bytes: &[u8]) -> Result<Txid, types::IndexerError> {
    let txid = Txid::from_slice(bytes).map_err(|e| types::IndexerError::TxIdParsingError(e))?;
    Ok(txid)
}
