use zksync_db_connection::{
    connection::Connection,
    error::DalResult,
    instrument::{InstrumentExt, Instrumented},
};
use zksync_types::{
    via_verifier_btc_inscription_operations::ViaVerifierBtcInscriptionRequestType, L1BatchNumber,
};

pub use crate::models::storage_block::{L1BatchMetadataError, L1BatchWithOptionalMetadata};
use crate::Core;

#[derive(Debug)]
pub struct ViaVerifierBlocksDal<'a, 'c> {
    pub(crate) storage: &'a mut Connection<'c, Core>,
}

impl ViaVerifierBlocksDal<'_, '_> {
    pub async fn insert_vote_l1_batch_inscription_request_id(
        &mut self,
        batch_number: L1BatchNumber,
        inscription_request_id: i64,
        inscription_request: ViaVerifierBtcInscriptionRequestType,
    ) -> DalResult<()> {
        match inscription_request {
            ViaVerifierBtcInscriptionRequestType::VoteOnchain => {
                let instrumentation = Instrumented::new("set_inscription_request_tx_id#commit")
                    .with_arg("batch_number", &batch_number)
                    .with_arg("inscription_request_id", &inscription_request_id);

                let query = sqlx::query!(
                    r#"
                    INSERT INTO
                        via_l1_batch_vote_inscription_request (l1_batch_number, vote_l1_batch_inscription_id, created_at, updated_at)
                    VALUES
                        ($1, $2, NOW(), NOW())
                    ON CONFLICT DO NOTHING
                    "#,
                    i64::from(batch_number.0),
                    inscription_request_id as i32,
                );
                let result = instrumentation
                    .clone()
                    .with(query)
                    .execute(self.storage)
                    .await?;

                if result.rows_affected() == 0 {
                    let err = instrumentation.constraint_error(anyhow::anyhow!(
                        "Update commit_l1_batch_inscription_id that is is not null is not allowed"
                    ));
                    return Err(err);
                }
                Ok(())
            }
        }
    }

    pub async fn check_vote_l1_batch_inscription_request_if_exists(
        &mut self,
        batch_number: i64,
    ) -> DalResult<bool> {
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM via_l1_batch_vote_inscription_request
                WHERE l1_batch_number = $1
            )
            "#,
            batch_number
        )
        .instrument("check_vote_l1_batch_inscription_request_id_exists")
        .fetch_one(self.storage)
        .await?;

        Ok(exists.unwrap_or(false))
    }
}
