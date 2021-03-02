mod lib;
mod args;
mod config;
use chrono::prelude::*;
use lib::{ClientConfig, Encounter, Encounters, Intensity, Participant};
use petgraph::dot::Dot;
use petgraph::graph::Graph;
use petgraph::visit::IntoNodeReferences;
use std::collections::HashMap;
use args::{Args, GenerateConfigsArgs, EmitDefaultConfigArgs};
use config::Config;
use std::fs;

type IoResult = std::io::Result<()>;

fn main() -> IoResult {
    let args = Args::new();

    match args {
        Args::EmitDefaultConfig(args) => handle_emit_default_config(args),
        Args::GenerateConfigs(args) => handle_generate_configs(args),
    }
}

fn handle_emit_default_config(args: EmitDefaultConfigArgs) -> IoResult {
    let config = Config::default();
    let config = serde_yaml::to_string(&config).unwrap();
    fs::write(args.config_file_path, config)?;
    Ok(())
}

fn handle_generate_configs(args: GenerateConfigsArgs) -> IoResult {
    let config = fs::read_to_string(args.config_file_path)?;
    let config: Config = serde_yaml::from_str(&config).unwrap();
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
    Ok(())

    // println!("{}", Dot::new(&graph));
}
