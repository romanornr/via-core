use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use bitcoin::{hashes::Hash, TapSighashType, Txid, Witness};
use musig2::{CompactSignature, PartialSignature};
use reqwest::{header, Client, StatusCode};
use tokio::sync::watch;
use via_btc_client::{
    traits::{BitcoinOps, Serializable},
    withdrawal_builder::UnsignedWithdrawalTx,
};
use via_musig2::{verify_signature, Signer};
use via_verifier_dal::{ConnectionPool, Verifier, VerifierDal};
use zksync_config::configs::via_verifier::{VerifierMode, ViaVerifierConfig};
use zksync_types::H256;

use crate::{
    types::{NoncePair, PartialSignaturePair, SigningSessionResponse},
    utils::{decode_nonce, decode_signature, encode_nonce, encode_signature, get_signer},
};

pub struct ViaWithdrawalVerifier {
    master_connection_pool: ConnectionPool<Verifier>,
    btc_client: Arc<dyn BitcoinOps>,
    config: ViaVerifierConfig,
    client: Client,
    signer: Signer,
    final_sig: Option<CompactSignature>,
}

impl ViaWithdrawalVerifier {
    pub async fn new(
        master_connection_pool: ConnectionPool<Verifier>,
        btc_client: Arc<dyn BitcoinOps>,
        config: ViaVerifierConfig,
    ) -> anyhow::Result<Self> {
        let signer = get_signer(
            &config.private_key.clone(),
            config.verifiers_pub_keys_str.clone(),
        )?;

        Ok(Self {
            master_connection_pool,
            btc_client,
            signer,
            client: Client::new(),
            config,
            final_sig: None,
        })
    }

    pub async fn run(mut self, mut stop_receiver: watch::Receiver<bool>) -> anyhow::Result<()> {
        let mut timer = tokio::time::interval(self.config.polling_interval());

        while !*stop_receiver.borrow_and_update() {
            tokio::select! {
                _ = timer.tick() => { /* continue iterations */ }
                _ = stop_receiver.changed() => break,
            }

            match self.loop_iteration().await {
                Ok(()) => {}
                Err(err) => {
                    tracing::error!("Failed to process verifier withdrawal task: {err}");
                }
            }
        }

        tracing::info!("Stop signal received, verifier withdrawal is shutting down");
        Ok(())
    }

    async fn loop_iteration(&mut self) -> Result<(), anyhow::Error> {
        let mut session_info = self.get_session().await?;

        if self.config.verifier_mode == VerifierMode::COORDINATOR {
            tracing::info!("create a new session");

            if session_info.l1_block_number != 0 {
                let withdrawal_txid = self
                    .master_connection_pool
                    .connection_tagged("coordinator task")
                    .await?
                    .via_votes_dal()
                    .get_vote_transaction_withdrawal_tx(session_info.l1_block_number)
                    .await?;

                // TODO: refactore the transaction confirmation for the musig2, and implement utxo manager like in the inscriber
                // Check if the previous batch musig2 transaction was minted before start a new session.
                if let Some(tx) = withdrawal_txid {
                    let tx_id = Txid::from_slice(&tx)?;
                    let is_confirmed = self.btc_client.check_tx_confirmation(&tx_id, 1).await?;
                    if !is_confirmed {
                        return Ok(());
                    }
                }
            }

            self.create_new_session().await?;
        }

        session_info = self.get_session().await?;
        if session_info.l1_block_number == 0 {
            tracing::info!("Empty session, nothing to process");
            return Ok(());
        }

        if self.config.verifier_mode == VerifierMode::COORDINATOR
            && self
                .build_and_broadcast_final_transaction(&session_info)
                .await?
        {
            return Ok(());
        }

        let session_signature = self.get_session_signatures().await?;
        let session_nonces = self.get_session_nonces().await?;
        let verifier_index = self.signer.signer_index();

        if session_signature.contains_key(&verifier_index)
            && session_nonces.contains_key(&verifier_index)
        {
            return Ok(());
        }

        // Reinit the signer, when a new session is created by the coordinator.
        if !session_signature.contains_key(&verifier_index)
            && !session_nonces.contains_key(&verifier_index)
            && (self.signer.has_created_partial_sig() || self.signer.has_submitted_nonce())
        {
            self.reinit_signer()?;
            return Ok(());
        }

        if session_info.received_nonces < session_info.required_signers {
            let message = hex::decode(&session_info.message_to_sign)?;

            if self.signer.has_not_started() {
                self.signer.start_signing_session(message)?;
            }

            if !session_nonces.contains_key(&verifier_index) {
                self.submit_nonce().await?;
            }
        } else if session_info.received_nonces >= session_info.required_signers {
            if self.signer.has_created_partial_sig() {
                return Ok(());
            }
            self.submit_partial_signature(session_nonces).await?;
        }

        Ok(())
    }

    fn create_request_headers(
        &self,
        body: Option<&impl serde::Serialize>,
    ) -> anyhow::Result<header::HeaderMap> {
        let mut headers = header::HeaderMap::new();
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let verifier_index = self.signer.signer_index().to_string();

        let private_key = bitcoin::PrivateKey::from_wif(&self.config.private_key)?;
        let secret_key = private_key.inner;

        // Create signature based on whether there's a body
        let signature = if let Some(data) = body {
            // Sign the request body directly
            crate::auth::sign_request(data, &secret_key)?
        } else {
            // Sign timestamp + verifier_index as a JSON object
            let payload = serde_json::json!({
                "timestamp": timestamp,
                "verifier_index": verifier_index,
            });
            crate::auth::sign_request(&payload, &secret_key)?
        };

        headers.insert("X-Timestamp", header::HeaderValue::from_str(&timestamp)?);
        headers.insert(
            "X-Verifier-Index",
            header::HeaderValue::from_str(&verifier_index)?,
        );
        headers.insert("X-Signature", header::HeaderValue::from_str(&signature)?);

        Ok(headers)
    }

    async fn get_session(&self) -> anyhow::Result<SigningSessionResponse> {
        let url = format!("{}/session", self.config.url);
        let headers = self.create_request_headers(None::<&()>)?;
        let resp = self
            .client
            .get(&url)
            .headers(headers.clone())
            .send()
            .await?;
        if resp.status().as_u16() != StatusCode::OK.as_u16() {
            anyhow::bail!(
                "Error to fetch the session, status: {}, url: {}, headers: {:?}, resp: {:?}",
                resp.status(),
                url,
                headers,
                resp.text().await?
            );
        }
        let session_info: SigningSessionResponse = resp.json().await?;
        Ok(session_info)
    }

    async fn get_session_nonces(&self) -> anyhow::Result<HashMap<usize, String>> {
        let nonces_url = format!("{}/session/nonce", self.config.url);
        let headers = self.create_request_headers(None::<&()>)?;
        let resp = self.client.get(&nonces_url).headers(headers).send().await?;
        let nonces: HashMap<usize, String> = resp.json().await?;
        Ok(nonces)
    }

    async fn submit_nonce(&mut self) -> anyhow::Result<()> {
        let nonce = self
            .signer
            .our_nonce()
            .ok_or_else(|| anyhow::anyhow!("No nonce available"))?;

        let nonce_pair = encode_nonce(self.signer.signer_index(), nonce).unwrap();
        let url = format!("{}/session/nonce", self.config.url);
        let headers = self.create_request_headers(Some(&nonce_pair))?;
        let res = self
            .client
            .post(&url)
            .headers(headers)
            .json(&nonce_pair)
            .send()
            .await?;

        if res.status().is_success() {
            self.signer.mark_nonce_submitted();
            return Ok(());
        }
        Ok(())
    }

    async fn get_session_signatures(&self) -> anyhow::Result<HashMap<usize, PartialSignature>> {
        let url = format!("{}/session/signature", self.config.url);
        let headers = self.create_request_headers(None::<&()>)?;
        let resp = self.client.get(&url).headers(headers).send().await?;
        let signatures: HashMap<usize, PartialSignaturePair> = resp.json().await?;
        let mut partial_sigs: HashMap<usize, PartialSignature> = HashMap::new();
        for (idx, sig) in signatures {
            partial_sigs.insert(idx, decode_signature(sig.signature).unwrap());
        }
        Ok(partial_sigs)
    }

    async fn submit_partial_signature(
        &mut self,
        session_nonces: HashMap<usize, String>,
    ) -> anyhow::Result<()> {
        // Process each nonce
        for (idx, nonce_b64) in session_nonces {
            if idx != self.signer.signer_index() {
                let nonce = decode_nonce(NoncePair {
                    signer_index: idx,
                    nonce: nonce_b64,
                })?;
                self.signer
                    .receive_nonce(idx, nonce.clone())
                    .map_err(|e| anyhow::anyhow!("Failed to receive nonce: {}", e))?;
            }
        }

        let partial_sig = self.signer.create_partial_signature()?;
        let sig_pair = encode_signature(self.signer.signer_index(), partial_sig)?;

        let url = format!("{}/session/signature", self.config.url);
        let headers = self.create_request_headers(Some(&sig_pair))?;
        let resp = self
            .client
            .post(&url)
            .headers(headers)
            .json(&sig_pair)
            .send()
            .await?;
        if resp.status().is_success() {
            self.signer.mark_partial_sig_submitted();
        }
        Ok(())
    }

    fn reinit_signer(&mut self) -> anyhow::Result<()> {
        let signer = get_signer(
            &self.config.private_key.clone(),
            self.config.verifiers_pub_keys_str.clone(),
        )?;
        self.signer = signer;
        self.final_sig = None;
        Ok(())
    }

    async fn create_new_session(&mut self) -> anyhow::Result<()> {
        let url = format!("{}/session/new", self.config.url);
        let headers = self.create_request_headers(None::<&()>)?;
        let resp = self
            .client
            .post(&url)
            .headers(headers)
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        if !resp.status().is_success() {
            self.reinit_signer()?;
        }
        Ok(())
    }

    async fn create_final_signature(
        &mut self,
        session_info: &SigningSessionResponse,
    ) -> anyhow::Result<()> {
        if self.final_sig.is_some() {
            return Ok(());
        }

        if session_info.received_partial_signatures >= session_info.required_signers {
            let signatures = self.get_session_signatures().await?;
            for (&i, sig) in &signatures {
                if self.signer.signer_index() != i {
                    self.signer.receive_partial_signature(i, *sig)?;
                }
            }

            let final_sig = self.signer.create_final_signature()?;
            let agg_pub = self.signer.aggregated_pubkey();
            verify_signature(
                agg_pub,
                final_sig,
                &hex::decode(&session_info.message_to_sign)?,
            )?;
            self.final_sig = Some(final_sig);

            return Ok(());
        }
        Ok(())
    }

    fn sign_transaction(
        &self,
        unsigned_tx: UnsignedWithdrawalTx,
        musig2_signature: CompactSignature,
    ) -> String {
        let mut unsigned_tx = unsigned_tx;
        let mut final_sig_with_hashtype = musig2_signature.serialize().to_vec();
        let sighash_type = TapSighashType::All;
        final_sig_with_hashtype.push(sighash_type as u8);
        for tx in &mut unsigned_tx.tx.input {
            tx.witness = Witness::from(vec![final_sig_with_hashtype.clone()]);
        }
        bitcoin::consensus::encode::serialize_hex(&unsigned_tx.tx)
    }

    async fn build_and_broadcast_final_transaction(
        &mut self,
        session_info: &SigningSessionResponse,
    ) -> anyhow::Result<bool> {
        self.create_final_signature(session_info)
            .await
            .context("Error create final signature")?;

        if let Some(musig2_signature) = self.final_sig {
            let withdrawal_txid = self
                .master_connection_pool
                .connection_tagged("coordinator task")
                .await?
                .via_votes_dal()
                .get_vote_transaction_withdrawal_tx(session_info.l1_block_number)
                .await?;

            if withdrawal_txid.is_some() {
                return Ok(false);
            }

            let unsigned_tx = UnsignedWithdrawalTx::from_bytes(&session_info.unsigned_tx);
            let signed_tx = self.sign_transaction(unsigned_tx.clone(), musig2_signature);

            let txid = self
                .btc_client
                .broadcast_signed_transaction(&signed_tx)
                .await?;

            self.master_connection_pool
                .connection_tagged("coordinator task")
                .await?
                .via_votes_dal()
                .mark_vote_transaction_as_processed_withdrawals(
                    H256::from_slice(&txid.as_raw_hash().to_byte_array()),
                    session_info.l1_block_number,
                )
                .await?;

            tracing::info!(
                "New withdrawal transaction processed, l1 batch {} musig2 tx_id {}",
                session_info.l1_block_number,
                txid
            );

            self.reinit_signer()?;

            return Ok(true);
        }
        Ok(false)
    }
}
