use crate::primitives::{
    ComputationId, TemporaryExposureKey, Validity,
};
use crate::time::TimeInterval;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum ListType {
    Blacklist,
    Greylist,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chunk {
    covers: TimeInterval,
    data: HashMap<ComputationId, ComputationState>,
}

impl Chunk {
    pub fn new(covers: TimeInterval) -> Self {
        Self {
            covers,
            data: HashMap::new(),
        }
    }
    pub fn next_chunk(&self) -> Self {
        Self {
            covers: self.covers.next_interval(),
            data: HashMap::new(),
        }
    }
    pub fn insert(
        &mut self,
        list: ListType,
        computation_id: ComputationId,
        data: &HashSet<Validity<TemporaryExposureKey>>,
    ) -> () {
        let computation = self
            .data
            .entry(computation_id)
            .or_insert(ComputationState::new());
        computation.insert(list, data);
    }
    pub fn covers(&self) -> &TimeInterval {
        &self.covers
    }
    pub fn data(&self) -> &HashMap<ComputationId, ComputationState> {
        &self.data
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComputationState {
    blacklist: HashSet<Validity<TemporaryExposureKey>>,
    greylist: HashSet<Validity<TemporaryExposureKey>>,
}

impl ComputationState {
    pub fn new() -> Self {
        Self {
            blacklist: HashSet::new(),
            greylist: HashSet::new(),
        }
    }
    pub fn insert(
        &mut self,
        list: ListType,
        data: &HashSet<Validity<TemporaryExposureKey>>,
    ) -> () {
        match list {
            ListType::Blacklist => {
                self.blacklist.extend(data);
            }
            ListType::Greylist => {
                self.greylist.extend(data);
            }
        }
    }
    pub fn blacklist(&self) -> &HashSet<Validity<TemporaryExposureKey>> {
        &self.blacklist
    }
    pub fn greylist(&self) -> &HashSet<Validity<TemporaryExposureKey>> {
        &self.greylist
    }
}
