use exposurelib::config::{Encounters, Intensity, Participant};
use exposurelib::time::ExposureTime;
use petgraph::graph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::visit::IntoNodeReferences;
use std::collections::{HashSet, VecDeque};

const DIVIDER: &'static str = "-------------------------------------------";

pub fn mark_ssev_group(graph: &mut Graph<Participant, Encounters>) -> () {
    let positively_tested: Vec<_> = graph
        .node_references()
        .filter(|(_node_index, participant)| participant.positively_tested())
        .map(|(infected, _participant)| {
            let mut ssev_times = HashSet::new();
            for traced in graph.neighbors(infected) {
                let encounters = graph.find_edge(infected, traced).unwrap();
                let encounters = graph.edge_weight(encounters).unwrap();
                for encounter in encounters.encounters.iter() {
                    if encounter.intensity == Intensity::HighRisk {
                        let exposure_time = ExposureTime::from(encounter.time);
                        ssev_times.insert((exposure_time, encounter.time));
                    }
                }
            }
            (infected, ssev_times)
        })
        .collect();
    println!(
        "Calculating the participants belonging to an SSEV\n{}",
        DIVIDER
    );
    for (infected, ssev_times) in positively_tested {
        for (exposure_time, human_time) in ssev_times {
            let participant = graph.node_weight(infected).unwrap();
            println!(
                "Searching for SSEV group belonging to SSEV \
                at {:?} (or {:?}) originating from {}:",
                human_time, exposure_time, participant.name
            );
            explore_and_mark(
                infected,
                exposure_time,
                graph,
                &mut VecDeque::new(),
                &mut HashSet::new(),
            );
        }
    }
}

fn explore_and_mark(
    from: NodeIndex<u32>,
    ssev_time: ExposureTime,
    graph: &mut Graph<Participant, Encounters>,
    queue: &mut VecDeque<NodeIndex<u32>>,
    explored: &mut HashSet<NodeIndex<u32>>,
) -> () {
    graph.node_weight_mut(from).unwrap().set_to_be_warned();
    explored.insert(from);
    let participant = graph.node_weight(from).unwrap();
    println!("Added participant to SSEV group: {}", participant.name);
    for neighbor in graph.neighbors(from) {
        if mutual_high_risk_registration_at(from, neighbor, ssev_time, graph)
            && !explored.contains(&neighbor)
        {
            queue.push_front(neighbor);
        }
    }
    while let Some(queued) = queue.pop_back() {
        explore_and_mark(queued, ssev_time, graph, queue, explored);
    }
}

fn mutual_high_risk_registration_at(
    from: NodeIndex<u32>,
    to: NodeIndex<u32>,
    at: ExposureTime,
    graph: &Graph<Participant, Encounters>,
) -> bool {
    let from_to = match graph.find_edge(from, to) {
        Some(from_to) => graph.edge_weight(from_to).unwrap(),
        None => return false,
    };
    let to_from = match graph.find_edge(to, from) {
        Some(to_from) => graph.edge_weight(to_from).unwrap(),
        None => return false,
    };
    if let None = from_to.encounters.iter().find(|encounter| {
        ExposureTime::from(encounter.time) == at && encounter.intensity == Intensity::HighRisk
    }) {
        return false;
    }
    if let None = to_from.encounters.iter().find(|encounter| {
        ExposureTime::from(encounter.time) == at && encounter.intensity == Intensity::HighRisk
    }) {
        return false;
    }
    true
}
