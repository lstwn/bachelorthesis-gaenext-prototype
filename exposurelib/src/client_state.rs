use crate::error::ExposurelibError;
use crate::primitives::{ExposureKeys, InfectionPeriod, TekRollingPeriod};
use chrono::prelude::*;
use chrono::Duration;
use ring::rand::SecureRandom;
use std::collections::VecDeque;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientState {
    // sorted after age, i.e. newest in the front, oldest in the back
    keys: VecDeque<ExposureKeys>,
}

impl ClientState {
    pub fn new(
        from: DateTime<Utc>,
        tekrp: TekRollingPeriod,
        infection_period: InfectionPeriod,
        secure_random: &dyn SecureRandom,
    ) -> Result<Self, ExposurelibError> {
        let mut keys = VecDeque::with_capacity(infection_period.into());
        let infection_period: i32 = infection_period.into();
        let tekrp_duration: Duration = tekrp.into();
        for i in 0..infection_period {
            let distance: Duration = tekrp_duration * i;
            let date = from - distance;
            keys.push_back(ExposureKeys::with_timestamp(date, tekrp, secure_random)?);
        }
        Ok(ClientState { keys })
    }
}
