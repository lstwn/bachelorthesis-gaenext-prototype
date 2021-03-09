use chrono::prelude::*;
use exposurelib::config::Participant;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InvalidConfigError {
    #[error(
        "Encounter from {from} to {to} at {at} is not within infection window [{lower}; {upper}["
    )]
    EncounterOutOfBounds {
        from: Participant,
        to: Participant,
        at: DateTime<Utc>,
        lower: DateTime<Utc>,
        upper: DateTime<Utc>,
    },
}
