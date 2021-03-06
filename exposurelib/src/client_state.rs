use crate::config::Intensity;
use crate::error::ExposurelibError;
use crate::primitives::{
    AssociatedEncryptedMetadata, ExposureKeyring, InfectionPeriod, RollingProximityIdentifier,
    TekKeyring, TekRollingPeriod, TemporaryExposureKey, Validity,
};
use crate::time::{ExposureTime, ExposureTimeSet};
use chrono::prelude::*;
use chrono::Duration;
use ring::rand::SecureRandom;
use serde::{Deserialize, Serialize};
use std::cmp::{Eq, PartialEq};
use std::collections::{btree_set::Union, BTreeMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientState {
    // sorted after age, i.e. newest in the front, oldest in the back
    pub keys: Keys,
    pub bluetooth_layer: BluetoothLayer,
}

impl ClientState {
    pub fn new(keys: Keys, bluetooth_layer: BluetoothLayer) -> Self {
        Self {
            keys,
            bluetooth_layer,
        }
    }
    pub fn keys(&self) -> &Keys {
        &self.keys
    }
    pub fn bluetooth_layer(&self) -> &BluetoothLayer {
        &self.bluetooth_layer
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Keys(VecDeque<Validity<ExposureKeyring>>);

impl Keys {
    pub fn new(
        from: DateTime<Utc>,
        tekrp: TekRollingPeriod,
        infection_period: InfectionPeriod,
        secure_random: &dyn SecureRandom,
    ) -> Result<Self, ExposurelibError> {
        let mut keys = VecDeque::with_capacity(usize::from(infection_period));
        let infection_period = i32::from(infection_period);
        let tekrp_duration = Duration::from(tekrp);
        for i in 0..infection_period {
            let distance: Duration = tekrp_duration * i;
            let date = from - distance;
            let exposure_keyring = ExposureKeyring::new(secure_random)?;
            keys.push_back(Validity::new(
                ExposureTime::from(date),
                tekrp,
                exposure_keyring,
            ));
        }
        Ok(Self(keys))
    }
    pub fn exposure_keyring(
        &self,
        at: ExposureTime,
        tekrp: TekRollingPeriod,
    ) -> Option<&ExposureKeyring> {
        self.all()
            .iter()
            .find_map(|validity| validity.query(at, tekrp))
    }
    pub fn all(&self) -> &VecDeque<Validity<ExposureKeyring>> {
        &self.0
    }
    pub fn all_teks(&self) -> HashSet<Validity<TemporaryExposureKey>> {
        self.all()
            .iter()
            .cloned()
            .map(|exposure_keyring| Validity::<TemporaryExposureKey>::from(exposure_keyring))
            .collect()
    }
    pub fn is_own_tek(&self, tek: &Validity<TemporaryExposureKey>) -> bool {
        self.all()
            .iter()
            .find(|exposure_keyring| **exposure_keyring == *tek)
            .is_some()
    }
    pub fn prune(&mut self, _tekrp: TekRollingPeriod, _infection_period: InfectionPeriod) -> () {
        unimplemented!("A retention period could be implemented here");
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BluetoothLayer {
    traced_contacts: BTreeMap<ExposureTime, BTreeMap<ExposureTime, Vec<TracedContact>>>,
}

impl BluetoothLayer {
    pub fn new() -> Self {
        Self {
            traced_contacts: BTreeMap::new(),
        }
    }
    pub fn add(&mut self, traced_contact: TracedContact, tekrp: TekRollingPeriod) -> () {
        let encounters_at_tekrp_multiple = self
            .traced_contacts
            .entry(traced_contact.exposure_time.floor_tekrp_multiple(tekrp))
            .or_insert(BTreeMap::new());
        let encounters_at_exposure_time = encounters_at_tekrp_multiple
            .entry(traced_contact.exposure_time)
            .or_insert(Vec::new());
        encounters_at_exposure_time.push(traced_contact);
    }
    pub fn match_with(&self, with: Validity<TekKeyring>) -> Option<Match> {
        // assert!(with.valid_from() == with.valid_from().floor_tekrp_multiple(tekrp));
        let encounters_at_tekrp_multiple = match self.traced_contacts.get(&with.valid_from()) {
            Some(encounters_at_exposure_time) => encounters_at_exposure_time,
            None => return None,
        };

        let mut high_risk = ExposureTimeSet::new();
        let mut low_risk = ExposureTimeSet::new();
        let mut socket_addr = None;

        for encounters_at_exposure_time in encounters_at_tekrp_multiple.iter() {
            let (exposure_time, traced_contacts) = encounters_at_exposure_time;
            let derived_rpi = with.keyring().rpi(exposure_time.clone());
            for traced_contact in traced_contacts {
                let observed_rpi = &traced_contact.rpi;
                if *observed_rpi == derived_rpi {
                    let metadata = traced_contact
                        .aem
                        .decrypt(with.keyring().aemk(), &derived_rpi);
                    if metadata.intensity() == Intensity::HighRisk {
                        high_risk.insert(exposure_time.clone());
                    } else {
                        low_risk.insert(exposure_time.clone());
                    }
                    // NOTE: just for debugging; latest CI can actually win..
                    if socket_addr.is_some()
                        && socket_addr.unwrap() != metadata.connection_identifier()
                    {
                        panic!("One TEK yielded different socket addresses/connection identifiers");
                    } else {
                        socket_addr = Some(metadata.connection_identifier());
                    }
                }
            }
        }

        if socket_addr.is_some() {
            Some(Match::new(
                socket_addr.unwrap(),
                Validity::<TemporaryExposureKey>::from(with),
                high_risk,
                low_risk,
            ))
        } else {
            None
        }
    }
    pub fn prune(&mut self, _tekrp: TekRollingPeriod, _infection_period: InfectionPeriod) -> () {
        unimplemented!("");
    }
}

#[derive(Debug, Eq)]
pub struct Match {
    socket_addr: SocketAddr,
    tek: Validity<TemporaryExposureKey>,
    high_risk: ExposureTimeSet,
    low_risk: ExposureTimeSet,
}

impl Match {
    pub fn new(
        socket_addr: SocketAddr,
        tek: Validity<TemporaryExposureKey>,
        high_risk: ExposureTimeSet,
        low_risk: ExposureTimeSet,
    ) -> Self {
        Self {
            socket_addr,
            tek,
            high_risk,
            low_risk,
        }
    }
    pub fn connection_identifier(&self) -> SocketAddr {
        self.socket_addr
    }
    pub fn tek(&self) -> &Validity<TemporaryExposureKey> {
        &self.tek
    }
    pub fn high_risk(&self) -> &ExposureTimeSet {
        &self.high_risk
    }
    pub fn low_risk(&self) -> &ExposureTimeSet {
        &self.low_risk
    }
    pub fn any_risk(&self) -> Union<ExposureTime> {
        self.high_risk.union(self.low_risk())
    }
}

impl PartialEq for Match {
    fn eq(&self, other: &Self) -> bool {
        self.tek == other.tek
    }
}

impl Hash for Match {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tek.hash(state);
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TracedContact {
    timestamp: DateTime<Utc>,
    exposure_time: ExposureTime,
    rpi: RollingProximityIdentifier,
    aem: AssociatedEncryptedMetadata,
}

impl TracedContact {
    pub fn new(
        timestamp: DateTime<Utc>,
        rpi: RollingProximityIdentifier,
        aem: AssociatedEncryptedMetadata,
    ) -> Self {
        Self {
            timestamp,
            exposure_time: ExposureTime::from(timestamp),
            rpi,
            aem,
        }
    }
}
