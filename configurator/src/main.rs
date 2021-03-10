mod args;
mod config;
mod error;
use crate::error::InvalidConfigError;
use anyhow::{Context, Result};
use args::{Args, EmitDefaultConfigArgs, GenerateConfigsArgs};
use chrono::Duration;
use config::Config;
use exposurelib::client_state::{BluetoothLayer, ClientState, Keys, TracedContact};
use exposurelib::config::{ClientConfig, DiagnosisServerConfig, Participant};
use exposurelib::primitives::SystemRandom;
use petgraph::dot::Dot;
use petgraph::visit::IntoNodeReferences;
use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

fn main() -> Result<()> {
    let args = Args::new();

    match args {
        Args::EmitDefaultConfig(args) => handle_emit_default_config(args),
        Args::GenerateConfigs(args) => handle_generate_configs(args),
    }
}

fn handle_emit_default_config(args: EmitDefaultConfigArgs) -> Result<()> {
    let config = Config::default();
    let config = serde_yaml::to_string(&config).unwrap();
    fs::write(&args.config_file_path, config).context(format!(
        "Could not write config to {:?}.",
        args.config_file_path
    ))?;
    Ok(())
}

fn handle_generate_configs(args: GenerateConfigsArgs) -> Result<()> {
    let config = fs::read_to_string(&args.config_file_path).context(format!(
        "Could not read config from {:?}.",
        args.config_file_path
    ))?;
    let config: Config = serde_yaml::from_str(&config).context(format!(
        "Could not deserialize config from {:?}. Check config file.",
        args.config_file_path
    ))?;
    let graph = &config.social_graph;

    let secure_random = SystemRandom::new();

    let participant_count = graph.node_references().count();

    let mut client_keys: HashMap<&Participant, Keys> = HashMap::with_capacity(participant_count);
    let mut client_bluetooth_layers: HashMap<&Participant, BluetoothLayer> =
        HashMap::with_capacity(participant_count);
    for (_, participant) in graph.node_references() {
        client_keys.insert(
            participant,
            Keys::new(
                config.today,
                config.system_params.tek_rolling_period,
                config.system_params.infection_period,
                &secure_random,
            )?,
        );
        client_bluetooth_layers.insert(participant, BluetoothLayer::new());
    }

    let tekrp = config.system_params.tek_rolling_period;
    for (node_index, participant) in graph.node_references() {
        for other_node_index in graph.neighbors(node_index) {
            let other_participant = graph.node_weight(other_node_index).unwrap();
            let edge_index = graph.find_edge(node_index, other_node_index).unwrap();
            let encounters = graph.edge_weight(edge_index).unwrap();
            for encounter in encounters.encounters.iter() {
                let rpi = client_keys
                    .get(&participant)
                    .unwrap()
                    .rpi(encounter.time.into(), tekrp)
                    .ok_or(InvalidConfigError::EncounterOutOfBounds {
                        from: participant.clone(),
                        to: other_participant.clone(),
                        at: encounter.time,
                        lower: config.today
                            - config.system_params.infection_period.as_duration(tekrp)
                            + Duration::from(tekrp),
                        upper: config.today + Duration::from(tekrp),
                    })
                    .context("Invalid config")?;
                let traced_contact = TracedContact::new(encounter.time, rpi);
                client_bluetooth_layers
                    .get_mut(&other_participant)
                    .unwrap()
                    .add(traced_contact, tekrp)
            }
        }
    }

    let diagnosis_server_endpoint: SocketAddr = config.diagnosis_server_endpoint.parse()?;
    let host: IpAddr = config.host.parse()?;
    let mut port: u16 = config.base_port;

    let client_configs: Vec<ClientConfig> = graph
        .node_references()
        .map(|(_, participant)| {
            let participant = participant.clone();
            let client_endpoint = SocketAddr::new(host, port.clone());
            port = port + 1;
            let state = ClientState::new(
                client_keys.remove(&participant).unwrap(),
                client_bluetooth_layers.remove(&participant).unwrap(),
            );
            ClientConfig::new(
                participant,
                client_endpoint,
                diagnosis_server_endpoint,
                config.system_params,
                state,
            )
        })
        .collect();

    for client_config in client_configs.iter() {
        let mut client_config_file_path = PathBuf::from(&args.config_output_path);
        client_config_file_path.push(client_config.name());
        client_config_file_path.set_extension("yaml");
        let yaml_client_config = serde_yaml::to_string(&client_config).context(format!(
            "Could not serialize client config {:?}",
            client_config
        ))?;
        fs::write(&client_config_file_path, yaml_client_config).context(format!(
            "Could not write client config to {:?}.",
            client_config_file_path
        ))?;
    }

    let diagnosis_server_config =
        DiagnosisServerConfig::new(diagnosis_server_endpoint, config.system_params);
    let mut diagnosis_server_config_file_path = PathBuf::from(&args.config_output_path);
    diagnosis_server_config_file_path.push("diagnosisserver");
    diagnosis_server_config_file_path.set_extension("yaml");
    let yaml_diagnosis_server_config = serde_yaml::to_string(&diagnosis_server_config).context(
        format!("Could not serialize diagnosis server config {:?}", diagnosis_server_config),
    )?;
    fs::write(&diagnosis_server_config_file_path, yaml_diagnosis_server_config).context(format!(
        "Could not write client config to {:?}.",
        diagnosis_server_config_file_path
    ))?;

    let mut dot_graph_file_path = PathBuf::from(&args.config_output_path);
    dot_graph_file_path.push(&args.config_file_path.file_name().unwrap());
    dot_graph_file_path.set_extension("dot");
    fs::write(&dot_graph_file_path, format!("{}", Dot::new(&graph))).context(format!(
        "Could not write dot graph file to {:?}.",
        dot_graph_file_path
    ))?;

    Ok(())
}
