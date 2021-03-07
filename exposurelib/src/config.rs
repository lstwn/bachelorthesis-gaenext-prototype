use serde::{Serialize, Deserialize};
use std::net::SocketAddr;
use std::fmt;
use std::hash;
use std::cmp;
use chrono::prelude::*;
use crate::client_state::ClientState;
use crate::primitives::*;

#[derive(Serialize, Deserialize)]
pub struct DiagnosisServerConfig {
    endpoint: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Participant {
    pub name: String,
    #[serde(default = "Participant::default_positively_tested")]
    pub positively_tested: bool,
}

impl Participant {
    pub fn new<T: Into<String>>(name: T, positively_tested: bool) -> Self {
        Participant {
            name: name.into(),
            positively_tested,
        }
    }
    fn default_positively_tested() -> bool {
        false
    }
}

impl hash::Hash for Participant {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl cmp::PartialEq for Participant {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl cmp::Eq for Participant {}

impl fmt::Display for Participant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Encounters {
    pub encounters: Vec<Encounter>,
}

impl Encounters {
    pub fn new(encounters: Vec<Encounter>) -> Self {
        Encounters { encounters }
    }
}

impl fmt::Display for Encounters {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut string = String::new();
        for encounter in self.encounters.iter() {
            string.push_str(&format!("{} | ", encounter));
        }
        let string = string.trim_end_matches(" | ");
        write!(f, "{}", string)
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct Encounter {
    pub time: DateTime<Utc>,
    pub intensity: Intensity,
}

impl Encounter {
    pub fn new(timestamp: DateTime<Utc>, intensity: Intensity) -> Self {
        Encounter {
            time: timestamp,
            intensity,
        }
    }
}

impl fmt::Display for Encounter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.time, self.intensity)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Copy, Clone)]
pub enum Intensity {
    LowRisk,
    HighRisk,
}

impl fmt::Display for Intensity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Copy, Clone)]
pub struct SystemParams {
    pub tek_rolling_period: TekRollingPeriod,
    pub infection_period: InfectionPeriod,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientConfig {
    #[serde(flatten)]
    pub participant: Participant,
    pub endpoint: SocketAddr,
    #[serde(flatten)]
    pub params: SystemParams,
    pub state: ClientState,
    // TODO: bluetooth
}

impl ClientConfig {
    pub fn name(&self) -> &str {
        &self.participant.name
    }
}
