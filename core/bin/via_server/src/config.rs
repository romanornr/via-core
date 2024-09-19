use anyhow::Context as _;
use zksync_config::{
    configs::{
        consensus::{ConsensusConfig, ConsensusSecrets},
        ObservabilityConfig,
    },
    PostgresConfig, ViaBtcSenderConfig, ViaBtcWatchConfig, ViaCelestiaConfig,
};
use zksync_core_leftovers::temp_config_store::{decode_yaml_repr, TempConfigStore};
use zksync_env_config::FromEnv;
use zksync_protobuf_config::proto;

pub(crate) fn read_consensus_secrets() -> anyhow::Result<Option<ConsensusSecrets>> {
    // Read public config.
    let Ok(path) = std::env::var("CONSENSUS_SECRETS_PATH") else {
        return Ok(None);
    };
    let secrets = std::fs::read_to_string(&path).context(path)?;
    Ok(Some(
        decode_yaml_repr::<proto::secrets::ConsensusSecrets>(&secrets)
            .context("failed decoding YAML")?,
    ))
}
//
// pub(crate) fn read_consensus_config() -> anyhow::Result<Option<ConsensusConfig>> {
//     // Read public config.
//     let Ok(path) = std::env::var("CONSENSUS_CONFIG_PATH") else {
//         return Ok(None);
//     };
//     let cfg = std::fs::read_to_string(&path).context(path)?;
//     Ok(Some(
//         decode_yaml_repr::<proto::consensus::Config>(&cfg).context("failed decoding YAML")?,
//     ))
// }

pub(crate) fn load_env_config() -> anyhow::Result<TempConfigStore> {
    Ok(TempConfigStore {
        postgres_config: PostgresConfig::from_env().ok(),
        health_check_config: None,
        merkle_tree_api_config: None,
        web3_json_rpc_config: None,
        circuit_breaker_config: None,
        mempool_config: None,
        network_config: None,
        contract_verifier: None,
        operations_manager_config: None,
        state_keeper_config: None,
        house_keeper_config: None,
        fri_proof_compressor_config: None,
        fri_prover_config: None,
        fri_prover_group_config: None,
        fri_prover_gateway_config: None,
        fri_witness_vector_generator: None,
        fri_witness_generator_config: None,
        prometheus_config: None,
        proof_data_handler_config: None,
        api_config: None,
        db_config: None,
        eth_sender_config: None,
        eth_watch_config: None,
        gas_adjuster_config: None,
        observability: ObservabilityConfig::from_env().ok(),
        snapshot_creator: None,
        da_dispatcher_config: None,
        protective_reads_writer_config: None,
        basic_witness_input_producer_config: None,
        core_object_store: None,
        base_token_adjuster_config: None,
        commitment_generator: None,
        pruning: None,
        snapshot_recovery: None,
        external_price_api_client_config: None,
        external_proof_integration_api_config: None,
        experimental_vm_config: None,
        prover_job_monitor_config: None,
    })
}

// TODO: temporary solution, should be removed after the config is refactored
pub(crate) fn via_load_env_config(
) -> anyhow::Result<(ViaBtcWatchConfig, ViaBtcSenderConfig, ViaCelestiaConfig)> {
    let btc_watch_config =
        ViaBtcWatchConfig::from_env().context("Failed to load BTC watch config")?;
    let btc_sender_config =
        ViaBtcSenderConfig::from_env().context("Failed to load celestia config")?;
    let celestia_config =
        ViaCelestiaConfig::from_env().context("Failed to load celestia config")?;

    Ok((btc_watch_config, btc_sender_config, celestia_config))
}
