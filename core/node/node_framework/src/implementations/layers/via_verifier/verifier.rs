use std::{str::FromStr, sync::Arc};

use anyhow::Context;
use via_btc_client::{client::BitcoinClient, types::NodeAuth};
use via_btc_watch::BitcoinNetwork;
use via_withdrawal_service::verifier::ViaWithdrawalVerifier;
use zksync_config::{ViaBtcSenderConfig, ViaVerifierConfig};

use crate::{
    implementations::resources::pools::{MasterPool, PoolResource},
    service::StopReceiver,
    task::{Task, TaskId},
    wiring_layer::{WiringError, WiringLayer},
    FromContext, IntoContext,
};

/// Wiring layer for verifier task
#[derive(Debug)]
pub struct ViaWithdrawalVerifierLayer {
    pub config: ViaVerifierConfig,
    pub btc_sender_config: ViaBtcSenderConfig,
}

#[derive(Debug, FromContext)]
#[context(crate = crate)]
pub struct Input {
    pub master_pool: PoolResource<MasterPool>,
}

#[derive(IntoContext)]
#[context(crate = crate)]
pub struct Output {
    #[context(task)]
    pub via_withdrawal_verifier_task: ViaWithdrawalVerifier,
}

#[async_trait::async_trait]
impl WiringLayer for ViaWithdrawalVerifierLayer {
    type Input = Input;
    type Output = Output;

    fn layer_name(&self) -> &'static str {
        "via_withdrawal_verifier_layer"
    }

    async fn wire(self, input: Self::Input) -> Result<Self::Output, WiringError> {
        let master_pool = input.master_pool.get().await?;
        let auth = NodeAuth::UserPass(
            self.btc_sender_config.rpc_user().to_string(),
            self.btc_sender_config.rpc_password().to_string(),
        );
        let network = BitcoinNetwork::from_str(self.btc_sender_config.network()).unwrap();

        let btc_client = Arc::new(
            BitcoinClient::new(self.btc_sender_config.rpc_url(), network, auth)
                .context("Error to init the btc client for verifier task")?,
        );

        let via_withdrawal_verifier_task =
            ViaWithdrawalVerifier::new(master_pool, btc_client, self.config)
                .await
                .context("Error to init the via withdrawal verifier")?;

        Ok(Output {
            via_withdrawal_verifier_task,
        })
    }
}

#[async_trait::async_trait]
impl Task for ViaWithdrawalVerifier {
    fn id(&self) -> TaskId {
        "via_withdrawal_verifier".into()
    }

    async fn run(self: Box<Self>, stop_receiver: StopReceiver) -> anyhow::Result<()> {
        (*self).run(stop_receiver.0).await
    }
}
