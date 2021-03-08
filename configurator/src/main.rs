mod args;
mod config;
mod lib;
use anyhow::{Context, Result};
use args::{Args, EmitDefaultConfigArgs, GenerateConfigsArgs};
use config::Config;
use exposurelib::client_state::ClientState;
use exposurelib::config::{ClientConfig, Participant};
use exposurelib::primitives::SystemRandom;
use lib::{Encounter, Encounters, Intensity};
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
    let host: IpAddr = config.host.parse()?;
    let mut port: u16 = config.base_port;

    let mut client_configs: HashMap<&Participant, ClientConfig> =
        HashMap::with_capacity(graph.node_references().count());
    for (_, participant) in graph.node_references() {
        let endpoint = SocketAddr::new(host, port.clone());
        port = port + 1;
        let state = ClientState::new(
            config.today,
            config.system_params.tek_rolling_period,
            config.system_params.infection_period,
            &secure_random,
        )?;
        client_configs.insert(participant, ClientConfig {
            participant: participant.clone(),
            endpoint,
            params: config.system_params,
            state,
        });
    }
    // let client_configs: Vec<ClientConfig> = graph
    //     .node_references()
    //     .map(|(node_index, participant)| {
    //         let participant = participant.clone();
    //         let encounters: HashMap<Participant, Encounters> = graph
    //             .neighbors(node_index)
    //             .map(|other_node_index| {
    //                 let other_participant = graph.node_weight(other_node_index).unwrap();
    //                 let edge_index = graph.find_edge(node_index, other_node_index).unwrap();
    //                 let encounters = graph.edge_weight(edge_index).unwrap();
    //                 (other_participant.clone(), encounters.clone())
    //             })
    //             .collect();
    //         ClientConfig::new(participant, encounters)
    //     })
    //     .collect();

    for client_config in client_configs.values() {
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

    let mut dot_graph_file_path = PathBuf::from(&args.config_output_path);
    dot_graph_file_path.push(&args.config_file_path.file_name().unwrap());
    dot_graph_file_path.set_extension("dot");
    fs::write(&dot_graph_file_path, format!("{}", Dot::new(&graph))).context(format!(
        "Could not write dot graph file to {:?}.",
        dot_graph_file_path
    ))?;

    Ok(())
}
