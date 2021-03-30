use chrono::prelude::*;
use petgraph::graph::Graph;
use serde::{Deserialize, Serialize};
use std::default::Default;
use exposurelib::config::{Encounter, Encounters, Intensity, Participant, SystemParams};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub base_port: u16,
    pub diagnosis_server_endpoint: String,
    pub system_params: SystemParams,
    pub today: DateTime<Utc>,
    /// All dates specified in the graph sould be within
    /// [today - tek_rolling_period * infection_period; today + tek_rolling_period[
    pub social_graph: Graph<Participant, Encounters>,
}

impl Default for Config {
    fn default() -> Self {
        let mut social_graph = Graph::<Participant, Encounters>::new();

        let p0 = social_graph.add_node(Participant::new("p0", true));
        let p1 = social_graph.add_node(Participant::new("p1", false));
        let p2 = social_graph.add_node(Participant::new("p2", false));

        social_graph.add_edge(
            p0,
            p1,
            Encounters::new(vec![
                Encounter::new(Utc.ymd(2021, 3, 1).and_hms(15, 44, 0), Intensity::HighRisk),
                Encounter::new(Utc.ymd(2021, 3, 1).and_hms(14, 44, 0), Intensity::LowRisk),
            ]),
        );
        social_graph.add_edge(
            p1,
            p0,
            Encounters::new(vec![
                Encounter::new(Utc.ymd(2021, 3, 1).and_hms(15, 44, 0), Intensity::HighRisk),
                Encounter::new(Utc.ymd(2021, 3, 1).and_hms(14, 44, 0), Intensity::LowRisk),
            ]),
        );
        social_graph.add_edge(
            p1,
            p2,
            Encounters::new(vec![
                Encounter::new(Utc.ymd(2021, 3, 1).and_hms(15, 44, 0), Intensity::HighRisk),
                Encounter::new(Utc.ymd(2021, 3, 1).and_hms(13, 44, 0), Intensity::LowRisk),
            ]),
        );
        social_graph.add_edge(
            p2,
            p1,
            Encounters::new(vec![
                Encounter::new(Utc.ymd(2021, 3, 1).and_hms(15, 44, 0), Intensity::HighRisk),
                Encounter::new(Utc.ymd(2021, 3, 1).and_hms(13, 44, 0), Intensity::LowRisk),
            ]),
        );

        Config {
            host: String::from("127.0.0.1"),
            base_port: 9000,
            diagnosis_server_endpoint: String::from("127.0.0.1:8999"),
            system_params: SystemParams::default(),
            today: Utc.ymd(2021, 3, 14).and_hms(0, 0, 0),
            social_graph,
        }
    }
}
