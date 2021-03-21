use crate::primitives::{
    ComputationId, InfectionPeriod, TekRollingPeriod, TemporaryExposureKey, Validity,
};
use crate::time::TimeInterval;
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

#[derive(Debug)]
pub struct DiagnosisServerState {
    black_list: DiagnosisKeys,
    grey_list: DiagnosisKeys,
}

pub type DiagnosisKey = Rc<Validity<TemporaryExposureKey>>;

#[derive(Debug)]
pub struct DiagnosisKeys {
    diagnosis_keys: HashSet<DiagnosisKey>,
    by_arrival_time: BTreeMap<DateTime<Utc>, HashSet<DiagnosisKey>>,
}

impl DiagnosisKeys {
    pub fn new() -> Self {
        Self {
            diagnosis_keys: HashSet::new(),
            by_arrival_time: BTreeMap::new(),
        }
    }
    pub fn insert(&mut self, batch: &HashSet<DiagnosisKey>) -> () {
        let arrival_time = Utc::now();
        let diagnosis_keys = &mut self.diagnosis_keys;
        let by_arrival_time = &mut self.by_arrival_time;
        batch
            .iter()
            .filter_map(|key| {
                let new = diagnosis_keys.insert(Rc::clone(&key));
                if new {
                    Some(key)
                } else {
                    None
                }
            })
            .for_each(|key| {
                by_arrival_time
                    .entry(arrival_time)
                    .or_insert(HashSet::new())
                    .insert(Rc::clone(key));
            });
    }
    pub fn get_from(&self, from: DateTime<Utc>) -> (DateTime<Utc>, Option<HashSet<DiagnosisKey>>) {
        if from > Utc::now() {
            return (Utc::now(), None);
        }
        let mut range = self.by_arrival_time.range(from..=Utc::now());
        let first = range.next();
        let (time, keys) = match first {
            Some(entry) => entry,
            None => return (from, None),
        };
        let init = if *time == from {
            (from, None)
        } else {
            (*time, Some(keys.clone()))
        };
        range.fold(init, |acc, cur| {
            let (time, cur_keys) = cur;
            let acc_keys = match acc {
                (_, Some(acc_keys)) => acc_keys,
                (_, None) => return (*time, Some(cur_keys.clone())),
            };
            let union: HashSet<DiagnosisKey> = acc_keys
                .union(cur_keys)
                .map(|element| Rc::clone(element))
                .collect();
            (*time, Some(union))
        })
    }
    pub fn prune(&mut self, _tekrp: TekRollingPeriod, _infection_period: InfectionPeriod) -> () {
        unimplemented!("");
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ListType {
    Blacklist,
    Greylist,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chunks {
    inner: Vec<Chunk>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chunk {
    covers: TimeInterval,
    data: HashMap<ComputationId, Computation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Computation {
    blacklist: HashSet<Validity<TemporaryExposureKey>>,
    greylist: HashSet<Validity<TemporaryExposureKey>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client_state::Keys;
    use crate::primitives::SystemRandom;
    use crate::time::ExposureTime;

    #[test]
    fn test_diagnosis_keys() -> Result<(), Box<dyn std::error::Error>> {
        let tekrp = TekRollingPeriod::default();
        let infection_period = InfectionPeriod::default();
        let system_random = SystemRandom::new();

        let init_from = Utc::now() - infection_period.as_duration(tekrp);

        let keys = Keys::new(Utc::now(), tekrp, infection_period, &system_random)?;
        let initial_batch: HashSet<DiagnosisKey> = keys
            .all()
            .iter()
            .map(|exposure_keyring_validity| Rc::new(exposure_keyring_validity.clone().into()))
            .collect();
        let mut blacklist = DiagnosisKeys::new();
        let (newest_from, teks) = blacklist.get_from(init_from);
        assert!(teks == None);
        assert!(newest_from == init_from);
        blacklist.insert(&initial_batch);

        let (newest_from, teks) = blacklist.get_from(init_from);
        assert!(teks == Some(initial_batch));
        let (next_newest_from, teks) = blacklist.get_from(newest_from);
        assert!(teks == None);
        assert!(newest_from == next_newest_from);

        let next_batch: HashSet<DiagnosisKey> = vec![Rc::new(Validity::new(
            ExposureTime::from(Utc::now()),
            tekrp,
            TemporaryExposureKey::new(&system_random).unwrap(),
        ))]
        .into_iter()
        .collect();
        blacklist.insert(&next_batch);

        let (next_newest_from, teks) = blacklist.get_from(newest_from);
        assert!(next_newest_from > newest_from);
        assert!(teks == Some(next_batch));

        Ok(())
    }
}
