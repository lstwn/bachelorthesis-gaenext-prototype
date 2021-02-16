mod lib;
use chrono::prelude::*;
use lib::{ClientConfig, Encounter, Encounters, Intensity, Participant};
use petgraph::dot::Dot;
use petgraph::graph::Graph;
use petgraph::visit::IntoNodeReferences;
use std::collections::HashMap;

fn main() {
    let mut graph = Graph::<Participant, Encounters>::new();

    let p1 = graph.add_node(Participant::new("p1", true));
    let p2 = graph.add_node(Participant::new("p2", false));
    let p3 = graph.add_node(Participant::new("p3", false));

    graph.add_edge(
        p1,
        p2,
        Encounters::new(vec![
            Encounter::new(Utc.ymd(2021, 2, 4).and_hms(15, 44, 0), Intensity::HighRisk),
            Encounter::new(Utc.ymd(2021, 2, 4).and_hms(14, 44, 0), Intensity::LowRisk),
        ]),
    );
    graph.add_edge(
        p2,
        p3,
        Encounters::new(vec![
            Encounter::new(Utc.ymd(2021, 2, 4).and_hms(15, 44, 0), Intensity::HighRisk),
            Encounter::new(Utc.ymd(2021, 2, 4).and_hms(13, 44, 0), Intensity::LowRisk),
        ]),
    );

    let serialized_graph = serde_yaml::to_string(&graph).unwrap();

    println!("{}", serialized_graph);

    let graph: Graph<Participant, Encounters> = serde_yaml::from_str(&serialized_graph).unwrap();

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

    // println!("{}", Dot::new(&graph));
}
