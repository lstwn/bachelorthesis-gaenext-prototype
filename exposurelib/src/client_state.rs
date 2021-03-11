use crate::error::ExposurelibError;
use crate::primitives::{
    AssociatedEncryptedMetadata, ExposureKeyring, InfectionPeriod, RollingProximityIdentifier,
    TekRollingPeriod, Validity,
};
use crate::time::ExposureTime;
use chrono::prelude::*;
use chrono::Duration;
use ring::rand::SecureRandom;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientState {
    // sorted after age, i.e. newest in the front, oldest in the back
    keys: Keys,
    bluetooth_layer: BluetoothLayer,
}

impl ClientState {
    pub fn new(keys: Keys, bluetooth_layer: BluetoothLayer) -> Self {
        Self {
            keys,
            bluetooth_layer,
        }
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
        self.keys()
            .iter()
            .find_map(|validity| validity.query(at, tekrp))
    }
    fn keys(&self) -> &VecDeque<Validity<ExposureKeyring>> {
        &self.0
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
    // pub fn match(
    //     &self,
    //     with: Validity<TekKeyring>
    // ) -> Option<ExposureTimeSet> {
    // }
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
