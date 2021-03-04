use crate::lib::{Encounter, Encounters, Intensity, Participant};
use chrono::prelude::*;
use petgraph::graph::Graph;
use serde::{Deserialize, Serialize};
use std::default::Default;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub base_port: u16,
    pub social_graph: Graph<Participant, Encounters>,
}

impl Default for Config {
    fn default() -> Self {
        let host = String::from("127.0.0.1");
        let base_port = 9000;

        let mut social_graph = Graph::<Participant, Encounters>::new();

        let p0 = social_graph.add_node(Participant::new("p0", true));
        let p1 = social_graph.add_node(Participant::new("p1", false));
        let p2 = social_graph.add_node(Participant::new("p2", false));

        social_graph.add_edge(
            p0,
            p1,
            Encounters::new(vec![
                Encounter::new(Utc.ymd(2021, 2, 4).and_hms(15, 44, 0), Intensity::HighRisk),
                Encounter::new(Utc.ymd(2021, 2, 4).and_hms(14, 44, 0), Intensity::LowRisk),
            ]),
        );
        social_graph.add_edge(
            p1,
            p0,
            Encounters::new(vec![
                Encounter::new(Utc.ymd(2021, 2, 4).and_hms(15, 44, 0), Intensity::HighRisk),
                Encounter::new(Utc.ymd(2021, 2, 4).and_hms(14, 44, 0), Intensity::LowRisk),
            ]),
        );
        social_graph.add_edge(
            p1,
            p2,
            Encounters::new(vec![
                Encounter::new(Utc.ymd(2021, 2, 4).and_hms(15, 44, 0), Intensity::HighRisk),
                Encounter::new(Utc.ymd(2021, 2, 4).and_hms(13, 44, 0), Intensity::LowRisk),
            ]),
        );

        Config {
            host,
            base_port,
            social_graph,
        }
    }
}
