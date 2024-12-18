// Withdrawal Builder Service
// This service has main method, that receives a list of bitcoin address and amount to withdraw and also L1batch proofDaReference reveal transaction id
// and then it will use client to get available utxo, and then perform utxo selection based on the total amount of the withdrawal
// and now we know the number of input and output we can estimate the fee and perform final utxo selection
// create a unsigned transaction and return it to the caller

use std::sync::Arc;

use anyhow::Result;
use bitcoin::{
    absolute, hashes::Hash, script::PushBytesBuf, transaction, Address, Amount, OutPoint,
    ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
};
use tracing::{debug, info, instrument};

use crate::{client::BitcoinClient, traits::BitcoinOps, types::BitcoinNetwork};

#[derive(Debug)]
pub struct WithdrawalBuilder {
    client: Arc<dyn BitcoinOps>,
    bridge_address: Address,
}

#[derive(Debug)]
pub struct WithdrawalRequest {
    pub address: Address,
    pub amount: Amount,
}

#[derive(Debug)]
pub struct UnsignedWithdrawalTx {
    pub tx: Transaction,
    pub txid: Txid,
    pub utxos: Vec<(OutPoint, TxOut)>,
    pub change_amount: Amount,
}

const OP_RETURN_PREFIX: &[u8] = b"VIA_PROTOCOL:WITHDRAWAL:";

impl WithdrawalBuilder {
    #[instrument(skip(rpc_url, auth), target = "bitcoin_withdrawal")]
    pub async fn new(
        rpc_url: &str,
        network: BitcoinNetwork,
        auth: bitcoincore_rpc::Auth,
        bridge_address: Address,
    ) -> Result<Self> {
        info!("Creating new WithdrawalBuilder");
        let client = Arc::new(BitcoinClient::new(rpc_url, network, auth)?);

        Ok(Self {
            client,
            bridge_address,
        })
    }

    #[instrument(skip(self, withdrawals, proof_txid), target = "bitcoin_withdrawal")]
    pub async fn create_unsigned_withdrawal_tx(
        &self,
        withdrawals: Vec<WithdrawalRequest>,
        proof_txid: Txid,
    ) -> Result<UnsignedWithdrawalTx> {
        debug!("Creating unsigned withdrawal transaction");

        // Calculate total amount needed
        let total_amount: Amount = withdrawals
            .iter()
            .try_fold(Amount::ZERO, |acc, w| acc.checked_add(w.amount))
            .ok_or_else(|| anyhow::anyhow!("Withdrawal amount overflow"))?;

        // Get available UTXOs from bridge address
        let utxos = self.get_available_utxos().await?;

        // Select UTXOs for the withdrawal
        let selected_utxos = self.select_utxos(&utxos, total_amount).await?;

        // Calculate total input amount
        let total_input_amount: Amount = selected_utxos
            .iter()
            .try_fold(Amount::ZERO, |acc, (_, txout)| acc.checked_add(txout.value))
            .ok_or_else(|| anyhow::anyhow!("Input amount overflow"))?;

        // Create OP_RETURN output with proof txid
        let op_return_data = self.create_op_return_script(proof_txid)?;
        let op_return_output = TxOut {
            value: Amount::ZERO,
            script_pubkey: op_return_data,
        };

        // Estimate fee (including OP_RETURN output)
        let fee_rate = self.client.get_fee_rate(1).await?;
        let fee_amount = self.estimate_fee(
            selected_utxos.len() as u32,
            withdrawals.len() as u32 + 1, // +1 for OP_RETURN output
            fee_rate,
        )?;

        // Verify we have enough funds
        let total_needed = total_amount
            .checked_add(fee_amount)
            .ok_or_else(|| anyhow::anyhow!("Total amount overflow"))?;

        if total_input_amount < total_needed {
            return Err(anyhow::anyhow!(
                "Insufficient funds: have {}, need {}",
                total_input_amount,
                total_needed
            ));
        }

        // Create inputs
        let inputs: Vec<TxIn> = selected_utxos
            .iter()
            .map(|(outpoint, _)| TxIn {
                previous_output: *outpoint,
                script_sig: ScriptBuf::default(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::default(),
            })
            .collect();

        // Create outputs for withdrawals
        let mut outputs: Vec<TxOut> = withdrawals
            .into_iter()
            .map(|w| TxOut {
                value: w.amount,
                script_pubkey: w.address.script_pubkey(),
            })
            .collect();

        // Add OP_RETURN output
        outputs.push(op_return_output);

        // Add change output if needed
        let change_amount = total_input_amount
            .checked_sub(total_needed)
            .ok_or_else(|| anyhow::anyhow!("Change amount calculation overflow"))?;

        if change_amount.to_sat() > 0 {
            outputs.push(TxOut {
                value: change_amount,
                script_pubkey: self.bridge_address.script_pubkey(),
            });
        }

        // Create unsigned transaction
        let unsigned_tx = Transaction {
            version: transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: inputs,
            output: outputs,
        };

        let txid = unsigned_tx.compute_txid();

        debug!("Unsigned withdrawal transaction created successfully");

        Ok(UnsignedWithdrawalTx {
            tx: unsigned_tx,
            txid,
            utxos: selected_utxos,
            change_amount,
        })
    }

    #[instrument(skip(self), target = "bitcoin_withdrawal")]
    async fn get_available_utxos(&self) -> Result<Vec<(OutPoint, TxOut)>> {
        let utxos = self.client.fetch_utxos(&self.bridge_address).await?;
        Ok(utxos)
    }

    #[instrument(skip(self, utxos), target = "bitcoin_withdrawal")]
    async fn select_utxos(
        &self,
        utxos: &[(OutPoint, TxOut)],
        target_amount: Amount,
    ) -> Result<Vec<(OutPoint, TxOut)>> {
        // Simple implementation - could be improved with better UTXO selection algorithm
        let mut selected = Vec::new();
        let mut total = Amount::ZERO;

        for utxo in utxos {
            selected.push(utxo.clone());
            total = total
                .checked_add(utxo.1.value)
                .ok_or_else(|| anyhow::anyhow!("Amount overflow during UTXO selection"))?;

            if total >= target_amount {
                break;
            }
        }

        if total < target_amount {
            return Err(anyhow::anyhow!(
                "Insufficient funds: have {}, need {}",
                total,
                target_amount
            ));
        }

        Ok(selected)
    }

    #[instrument(skip(self), target = "bitcoin_withdrawal")]
    fn estimate_fee(&self, input_count: u32, output_count: u32, fee_rate: u64) -> Result<Amount> {
        // Estimate transaction size
        let base_size = 10_u64; // version + locktime
        let input_size = 148_u64 * u64::from(input_count); // approximate size per input
        let output_size = 34_u64 * u64::from(output_count); // approximate size per output

        let total_size = base_size + input_size + output_size;
        let fee = fee_rate * total_size;

        Ok(Amount::from_sat(fee))
    }

    // Helper function to create OP_RETURN script
    fn create_op_return_script(&self, proof_txid: Txid) -> Result<ScriptBuf> {
        let mut data = Vec::with_capacity(OP_RETURN_PREFIX.len() + 32);
        data.extend_from_slice(OP_RETURN_PREFIX);
        data.extend_from_slice(&proof_txid.as_raw_hash().to_byte_array());

        let mut encoded_data = PushBytesBuf::with_capacity(data.len());
        encoded_data.extend_from_slice(&data).ok();

        Ok(ScriptBuf::new_op_return(encoded_data))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use async_trait::async_trait;
    use bitcoin::Network;
    use mockall::{mock, predicate::*};

    use super::*;
    use crate::types::BitcoinError;

    mock! {
        BitcoinOps {}
        #[async_trait]
        impl BitcoinOps for BitcoinOps {
            async fn fetch_utxos(&self, _address: &Address) -> Result<Vec<(OutPoint, TxOut)>, BitcoinError> {
                // Mock implementation
                let txid = Txid::from_str(
                    "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b"
                ).unwrap();
                let outpoint = OutPoint::new(txid, 0);
                let txout = TxOut {
                    value: Amount::from_btc(1.0).unwrap(),
                    script_pubkey: ScriptBuf::new(),
                };
                Ok(vec![(outpoint, txout)])
            }

            async fn get_fee_rate(&self, _target_blocks: u16) -> Result<u64, BitcoinError> {
                Ok(2)
            }

            async fn broadcast_signed_transaction(&self, _tx_hex: &str) -> Result<Txid, BitcoinError> {
                Ok(Txid::from_str("4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b").unwrap())
            }

            async fn check_tx_confirmation(&self, _txid: &Txid, _min_confirmations: u32) -> Result<bool, BitcoinError> {
                Ok(true)
            }

            async fn fetch_block_height(&self) -> Result<u128, BitcoinError> {
                Ok(100000)
            }

            async fn get_balance(&self, _address: &Address) -> Result<u128, BitcoinError> {
                Ok(100000000) // 1 BTC in sats
            }
            fn get_network(&self) -> bitcoin::Network {
                Network::Regtest
            }

            async fn fetch_block(&self, _height: u128) -> Result<bitcoin::Block, BitcoinError> {
                Ok(bitcoin::Block::default())
            }

            async fn get_transaction(&self, _txid: &Txid) -> Result<Transaction, BitcoinError> {
                Ok(Transaction::default())
            }

            async fn fetch_block_by_hash(&self, _hash: &bitcoin::BlockHash) -> Result<bitcoin::Block, BitcoinError> {
                Ok(bitcoin::Block::default())
            }
        }
    }

    #[tokio::test]
    async fn test_withdrawal_builder() -> Result<()> {
        let network = Network::Regtest;
        let bridge_address =
            Address::from_str("bcrt1pxqkh0g270lucjafgngmwv7vtgc8mk9j5y4j8fnrxm77yunuh398qfv8tqp")?
                .require_network(network)?;

        // Create mock and set expectations
        let mut mock_ops = MockBitcoinOps::new();
        mock_ops.expect_fetch_utxos().returning(|_| {
            let txid =
                Txid::from_str("4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b")
                    .unwrap();
            let outpoint = OutPoint::new(txid, 0);
            let txout = TxOut {
                value: Amount::from_btc(1.0).unwrap(),
                script_pubkey: ScriptBuf::new(),
            };
            Ok(vec![(outpoint, txout)])
        });

        mock_ops.expect_get_fee_rate().returning(|_| Ok(2));

        // Use mock client
        let builder = WithdrawalBuilder {
            client: Arc::new(mock_ops),
            bridge_address,
        };

        let withdrawal_address = "bcrt1pv6dtdf0vrrj6ntas926v8vw9u0j3mga29vmfnxh39zfxya83p89qz9ze3l";
        let withdrawal_amount = Amount::from_btc(0.1)?;

        let withdrawals = vec![WithdrawalRequest {
            address: Address::from_str(withdrawal_address)?.require_network(network)?,
            amount: withdrawal_amount,
        }];

        let proof_txid =
            Txid::from_str("4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b")?;

        let withdrawal_tx = builder
            .create_unsigned_withdrawal_tx(withdrawals, proof_txid)
            .await?;
        assert!(!withdrawal_tx.utxos.is_empty());

        // Verify OP_RETURN output
        let op_return_output = withdrawal_tx
            .tx
            .output
            .iter()
            .find(|output| output.script_pubkey.is_op_return())
            .expect("OP_RETURN output not found");

        assert!(op_return_output
            .script_pubkey
            .as_bytes()
            .windows(OP_RETURN_PREFIX.len())
            .any(|window| window == OP_RETURN_PREFIX));

        Ok(())
    }
}
