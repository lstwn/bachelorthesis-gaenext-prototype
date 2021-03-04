mod args;
mod config;
mod lib;
use anyhow::{Context, Result};
use args::{Args, EmitDefaultConfigArgs, GenerateConfigsArgs};
use chrono::prelude::*;
use config::Config;
use lib::{ClientConfig, Encounter, Encounters, Intensity, Participant};
use petgraph::dot::Dot;
use petgraph::graph::Graph;
use petgraph::visit::IntoNodeReferences;
use std::collections::HashMap;
use std::fs;
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
    let graph = config.social_graph;

    let client_configs: Vec<ClientConfig> = graph
        .node_references()
        .map(|(node_index, participant)| {
            let participant = participant.clone();
            let encounters: HashMap<Participant, Encounters> = graph
                .neighbors(node_index)
                .map(|other_node_index| {
                    let other_participant =
                        graph.node_weight(other_node_index).expect("Cannot happen.");
                    let edge_index = graph
                        .find_edge(node_index, other_node_index)
                        .expect("Cannot happen.");
                    let encounters = graph.edge_weight(edge_index).expect("Cannot happen.");
                    (other_participant.clone(), encounters.clone())
                })
                .collect();
            ClientConfig::new(participant, encounters)
        })
        .collect();

    println!("{}", serde_yaml::to_string(&client_configs).unwrap());

    let mut dot_graph_file_path = PathBuf::from(&args.config_output_path);
    dot_graph_file_path.push(&args.config_file_path.file_name().unwrap());
    dot_graph_file_path.set_extension("dot");
    fs::write(&dot_graph_file_path, format!("{}", Dot::new(&graph))).context(format!(
        "Could not write dot graph file to {:?}.",
        dot_graph_file_path
    ))?;

    Ok(())
}
