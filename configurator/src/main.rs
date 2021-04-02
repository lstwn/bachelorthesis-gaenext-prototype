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
use exposurelib::primitives::{Metadata, SystemRandom};
use petgraph::dot::Dot;
use petgraph::visit::IntoNodeReferences;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

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
    if let Some(parent) = args.config_file_path.parent() {
        fs::create_dir_all(parent).context(format!(
            "Could not ensure parent folders {:?} are existing.",
            parent
        ))?;
    };
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

    let mut client_init: HashMap<&Participant, (Keys, BluetoothLayer, SocketAddr)> =
        HashMap::with_capacity(participant_count);

    let host: IpAddr = config.host.parse()?;
    let mut port: u16 = config.base_port;

    for (_, participant) in graph.node_references() {
        client_init.insert(
            participant,
            (
                Keys::new(
                    config.today,
                    config.system_params.tek_rolling_period,
                    config.system_params.infection_period,
                    &secure_random,
                )?,
                BluetoothLayer::new(),
                SocketAddr::new(host, port.clone()),
            ),
        );
        port = port + 1;
    }

    let tekrp = config.system_params.tek_rolling_period;
    for (node_index, participant) in graph.node_references() {
        for other_node_index in graph.neighbors(node_index) {
            let other_participant = graph.node_weight(other_node_index).unwrap();
            let edge_index = graph.find_edge(node_index, other_node_index).unwrap();
            let encounters = graph.edge_weight(edge_index).unwrap();
            for encounter in encounters.encounters.iter() {
                let (keys, _, endpoint) = client_init.get(&other_participant).unwrap();
                let metadata = Metadata::new(encounter.intensity, endpoint.clone());
                let (rpi, aem) = keys
                    .exposure_keyring(encounter.time.into(), tekrp)
                    .ok_or(InvalidConfigError::EncounterOutOfBounds {
                        from: participant.clone(),
                        to: other_participant.clone(),
                        at: encounter.time,
                        lower: config.today
                            - config.system_params.infection_period.as_duration(tekrp)
                            + Duration::from(tekrp),
                        upper: config.today + Duration::from(tekrp),
                    })
                    .context("Invalid config")?
                    .tek_keyring()
                    .rpi_and_aem(encounter.time.into(), metadata);
                let traced_contact = TracedContact::new(encounter.time, rpi, aem);
                let (_, bluetooth_layer, _) = client_init.get_mut(&participant).unwrap();
                bluetooth_layer.add(traced_contact, tekrp)
            }
        }
    }

    let diagnosis_server_endpoint: SocketAddr = config.diagnosis_server_endpoint.parse()?;

    let client_configs: Vec<ClientConfig> = graph
        .node_references()
        .map(|(_, participant)| {
            let participant = participant.clone();
            let (keys, bluetooth_layer, client_endpoint) =
                client_init.remove(&participant).unwrap();
            let state = ClientState::new(keys, bluetooth_layer);
            ClientConfig::new(
                participant,
                client_endpoint,
                diagnosis_server_endpoint,
                config.system_params,
                state,
            )
        })
        .collect();

    let mut client_config_output_path = args.config_output_path.clone();
    client_config_output_path.push("clients");
    if client_config_output_path.exists() {
        fs::remove_dir_all(&client_config_output_path).context(format!(
            "Error cleaning client config output path {:?}",
            client_config_output_path
        ))?;
    }
    fs::create_dir_all(&client_config_output_path).context(format!(
        "Error ensuring client config output path exists {:?}",
        client_config_output_path
    ))?;
    for client_config in client_configs.iter() {
        let yaml_client_config = serde_yaml::to_string(&client_config).context(format!(
            "Could not serialize client config {:?}",
            client_config
        ))?;
        write_config(
            &client_config_output_path,
            client_config.name(),
            "yaml",
            yaml_client_config,
        )
        .context("Error writing client config")?;
    }

    let diagnosis_server_config =
        DiagnosisServerConfig::new(diagnosis_server_endpoint, config.system_params);
    let yaml_diagnosis_server_config =
        serde_yaml::to_string(&diagnosis_server_config).context(format!(
            "Could not serialize diagnosis server config {:?}",
            diagnosis_server_config
        ))?;
    write_config(
        &args.config_output_path,
        "diagnosisserver",
        "yaml",
        yaml_diagnosis_server_config,
    )
    .context("Error writing diagnosis config")?;

    let dot_graph = format!("{}", Dot::new(&graph));
    write_config(
        &args.config_output_path,
        args.config_file_path.file_name().unwrap(),
        "dot",
        dot_graph,
    )
    .context("Error writing dot graph file")?;

    Ok(())
}

fn write_config<U: Into<PathBuf>, P: AsRef<Path>, S: AsRef<OsStr>, T: AsRef<[u8]>>(
    path: U,
    file_name: P,
    extension: S,
    config: T,
) -> Result<()> {
    let mut config_file_path: PathBuf = path.into();
    config_file_path.push(file_name);
    config_file_path.set_extension(extension);
    fs::write(&config_file_path, config)
        .context(format!("Could not write file to {:?}.", config_file_path))?;
    Ok(())
}
