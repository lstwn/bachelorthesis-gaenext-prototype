use chrono::prelude::*;
use exposurelib::config::{Encounter, Encounters, Intensity, Participant, SystemParams};
use petgraph::graph::{Graph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::default::Default;

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

        let today = Utc.ymd(2021, 3, 14).and_hms(0, 0, 0);

        let p0 = social_graph.add_node(Participant::new("p0", true));
        let p1 = social_graph.add_node(Participant::new("p1", false));
        let p2 = social_graph.add_node(Participant::new("p2", false));
        let p3 = social_graph.add_node(Participant::new("p3", false));
        let p4 = social_graph.add_node(Participant::new("p4", false));

        // NOTE: an edge from p0 to p1 means that p0 registered p1
        // at the given time with the given risk
        // DISCLAIMER: the graph library (petgraph) allows for duplicate edges,
        // this is not supported and can result in undefined behavior :D
        // therefore it is recommended to rely on the convenience wrappers below
        // however, they do *not* perform encounter deduplication, this is the
        // users responsibility

        let t1 = Utc.ymd(2021, 3, 1).and_hms(13, 44, 0);
        let t2 = Utc.ymd(2021, 3, 1).and_hms(15, 44, 0);

        social_graph.add_mutual_registration(
            p0,
            p1,
            Encounters::new(vec![Encounter::new(t2, Intensity::HighRisk)]),
        );
        social_graph.add_mutual_registration(
            p1,
            p2,
            Encounters::new(vec![Encounter::new(t2, Intensity::HighRisk)]),
        );
        social_graph.add_one_way_registration(
            p1,
            p2,
            Encounters::new(vec![Encounter::new(t1, Intensity::LowRisk)]),
        );
        social_graph.add_one_way_registration(
            p2,
            p1,
            Encounters::new(vec![Encounter::new(t1, Intensity::HighRisk)]),
        );
        social_graph.add_mutual_registration(
            p2,
            p3,
            Encounters::new(vec![Encounter::new(t1, Intensity::HighRisk)]),
        );
        social_graph.add_mutual_registration(
            p2,
            p4,
            Encounters::new(vec![Encounter::new(t1, Intensity::HighRisk)]),
        );
        social_graph.add_mutual_registration(
            p0,
            p4,
            Encounters::new(vec![Encounter::new(t1, Intensity::HighRisk)]),
        );

        Config {
            host: String::from("127.0.0.1"),
            base_port: 10000,
            diagnosis_server_endpoint: String::from("127.0.0.1:9999"),
            system_params: SystemParams::default(),
            today,
            social_graph,
        }
    }
}

trait ConvenienceWrapper {
    fn add_mutual_registration(
        &mut self,
        between: NodeIndex<u32>,
        and: NodeIndex<u32>,
        encounters: Encounters,
    ) -> ();
    fn add_one_way_registration(
        &mut self,
        of: NodeIndex<u32>,
        by: NodeIndex<u32>,
        encounters: Encounters,
    ) -> ();
}

impl ConvenienceWrapper for Graph<Participant, Encounters> {
    fn add_mutual_registration(
        &mut self,
        between: NodeIndex<u32>,
        and: NodeIndex<u32>,
        encounters: Encounters,
    ) -> () {
        self.add_one_way_registration(between, and, encounters.clone());
        self.add_one_way_registration(and, between, encounters);
    }
    fn add_one_way_registration(
        &mut self,
        by: NodeIndex<u32>,
        of: NodeIndex<u32>,
        encounters: Encounters,
    ) -> () {
        if let Some(edge) = self.find_edge(by, of) {
            self.edge_weight_mut(edge)
                .unwrap()
                .encounters
                .extend(encounters.encounters.iter());
        } else {
            self.add_edge(by, of, encounters);
        }
    }
}
