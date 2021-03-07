use super::time::ExposureTime;
use crate::error::ExposurelibError;
use aes::cipher::generic_array::GenericArray;
use aes::{Aes128, BlockCipher, NewBlockCipher};
use chrono::prelude::*;
use chrono::Duration;
use ring::hkdf::Salt;
use ring::hkdf::HKDF_SHA256;
use ring::rand::SecureRandom;
pub use ring::rand::SystemRandom;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;

#[derive(Serialize, Deserialize)]
pub struct KeyForward {
    valid_from: ExposureTime,
    origin_tek: TemporaryExposureKey,
    // NOTE: omitting (origin) EPK in the prototype
    predecessor_tek: TemporaryExposureKey,
}

#[derive(Serialize, Deserialize)]
pub struct KeyUpload {
    valid_from: ExposureTime,
    tek: TemporaryExposureKey,
    // NOTE: omitting EPK in the prototype
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExposureKeys {
    valid_from: ExposureTime,
    tek: TemporaryExposureKey,
    rpik: RollingProximityIdentifierKey,
    aemk: AssociatedEncryptedMetadataKey,
    sd: Seed,
    pksk: PublicKeySymmetricKey,
}

impl ExposureKeys {
    pub fn new(
        valid_from: ExposureTime,
        secure_random: &dyn SecureRandom,
    ) -> Result<Self, ExposurelibError> {
        let tek = TemporaryExposureKey::new(secure_random)?;
        let sd = Seed::new(secure_random)?;
        Ok(Self {
            valid_from,
            tek,
            rpik: RollingProximityIdentifierKey::new(&tek)?,
            aemk: AssociatedEncryptedMetadataKey::new(&tek)?,
            sd,
            pksk: PublicKeySymmetricKey::new(&sd)?,
        })
    }
    pub fn with_timestamp(
        timestamp: DateTime<Utc>,
        tekrp: TekRollingPeriod,
        secure_random: &dyn SecureRandom,
    ) -> Result<Self, ExposurelibError> {
        let tekrp: u32 = tekrp.into();
        let valid_from = ExposureTime::en_interval_number(timestamp) / tekrp * tekrp;
        Self::new(valid_from.into(), secure_random)
    }
}

/// The TEK rolling period (TEKRP) is stated in multiples of 10 minutes.
#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct TekRollingPeriod(u16);

impl std::convert::From<TekRollingPeriod> for u32 {
    fn from(tekrp: TekRollingPeriod) -> u32 {
        tekrp.0 as u32
    }
}

impl std::convert::From<TekRollingPeriod> for Duration {
    fn from(tekrp: TekRollingPeriod) -> Duration {
        Duration::minutes((tekrp.0 * 10) as i64)
    }
}

impl std::default::Default for TekRollingPeriod {
    fn default() -> Self {
        TekRollingPeriod(144)
    }
}

/// The infection period is given in multiples of TekRollingPeriod.
#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct InfectionPeriod(u16);

impl InfectionPeriod {
    pub fn as_duration(&self, tekrp: TekRollingPeriod) -> Duration {
        let tekrp: Duration = tekrp.into();
        tekrp * (self.0 as i32)
    }
}

impl std::convert::From<InfectionPeriod> for i32 {
    fn from(infection_period: InfectionPeriod) -> i32 {
        infection_period.0 as i32
    }
}

impl std::convert::From<InfectionPeriod> for usize {
    fn from(infection_period: InfectionPeriod) -> usize {
        infection_period.0 as usize
    }
}

impl std::default::Default for InfectionPeriod {
    fn default() -> Self {
        InfectionPeriod(14)
    }
}

pub trait Key {
    const KEY_LEN: usize;
    fn get(&self) -> &[u8];
}

trait RandomKey
where
    Self: Key,
{
    fn generate(secure_random: &dyn SecureRandom) -> Result<Vec<u8>, ExposurelibError> {
        let mut key = vec![0; Self::KEY_LEN];
        match secure_random.fill(&mut key) {
            Ok(()) => Ok(key),
            Err(_) => Err(ExposurelibError::RandomKeyGenerationError),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporaryExposureKey {
    key: [u8; Self::KEY_LEN],
}

impl TemporaryExposureKey {
    pub fn new(secure_random: &dyn SecureRandom) -> Result<Self, ExposurelibError> {
        Self::generate(secure_random).and_then(|key| {
            Ok(Self {
                key: key.try_into().unwrap(),
            })
        })
    }
}

impl RandomKey for TemporaryExposureKey {}

impl Key for TemporaryExposureKey {
    const KEY_LEN: usize = 16;

    fn get(&self) -> &[u8] {
        &self.key
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seed {
    key: [u8; Self::KEY_LEN],
}

impl Seed {
    pub fn new(secure_random: &dyn SecureRandom) -> Result<Self, ExposurelibError> {
        Self::generate(secure_random).and_then(|key| {
            Ok(Self {
                key: key.try_into().unwrap(),
            })
        })
    }
}

impl RandomKey for Seed {}

impl Key for Seed {
    const KEY_LEN: usize = 10;

    fn get(&self) -> &[u8] {
        &self.key
    }
}

trait HKDFDerivedKey
where
    Self: Key,
{
    const INFO: &'static str;

    // NOTE: cannot return array here, due to const generic limitations in rustc
    fn derive<T: Key>(key_material: &T) -> Result<Vec<u8>, ExposurelibError> {
        let mut key = vec![0; Self::KEY_LEN];
        Salt::new(HKDF_SHA256, &[])
            .extract(key_material.get())
            .expand(&[Self::INFO.as_ref()], Wrapper(Self::KEY_LEN))
            .map_err(|_| ExposurelibError::KeyDerivationError)?
            .fill(&mut key)
            .map_err(|_| ExposurelibError::KeyDerivationError)?;
        Ok(key)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollingProximityIdentifierKey {
    key: [u8; Self::KEY_LEN],
}

impl RollingProximityIdentifierKey {
    pub fn new(tek: &TemporaryExposureKey) -> Result<Self, ExposurelibError> {
        Self::derive(tek).and_then(|key| {
            Ok(Self {
                key: key.try_into().unwrap(),
            })
        })
    }
}

impl Key for RollingProximityIdentifierKey {
    const KEY_LEN: usize = 16;

    fn get(&self) -> &[u8] {
        &self.key
    }
}

impl HKDFDerivedKey for RollingProximityIdentifierKey {
    const INFO: &'static str = "EN-RPIK";
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssociatedEncryptedMetadataKey {
    key: [u8; Self::KEY_LEN],
}

impl AssociatedEncryptedMetadataKey {
    pub fn new(tek: &TemporaryExposureKey) -> Result<Self, ExposurelibError> {
        Self::derive(tek).and_then(|key| {
            Ok(Self {
                key: key.try_into().unwrap(),
            })
        })
    }
}

impl Key for AssociatedEncryptedMetadataKey {
    const KEY_LEN: usize = 16;

    fn get(&self) -> &[u8] {
        &self.key
    }
}

impl HKDFDerivedKey for AssociatedEncryptedMetadataKey {
    const INFO: &'static str = "EN-AEMK";
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKeySymmetricKey {
    key: [u8; Self::KEY_LEN],
}

impl PublicKeySymmetricKey {
    pub fn new(sd: &Seed) -> Result<Self, ExposurelibError> {
        Self::derive(sd).and_then(|key| {
            Ok(Self {
                key: key.try_into().unwrap(),
            })
        })
    }
}

impl Key for PublicKeySymmetricKey {
    const KEY_LEN: usize = 16;

    fn get(&self) -> &[u8] {
        &self.key
    }
}

impl HKDFDerivedKey for PublicKeySymmetricKey {
    const INFO: &'static str = "EN-PKSK";
}

struct Wrapper<T>(T);

impl ring::hkdf::KeyType for Wrapper<usize> {
    fn len(&self) -> usize {
        self.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollingProximityIdentifier {
    key: [u8; Self::KEY_LEN],
}

impl RollingProximityIdentifier {
    const INFO: &'static str = "EN-RPI";

    pub fn new(rpik: &RollingProximityIdentifierKey, j: ExposureTime) -> Self {
        let key = GenericArray::from_slice(rpik.get());
        let cipher = Aes128::new(&key);
        let mut data = GenericArray::clone_from_slice(&Self::padded_data(j));
        cipher.encrypt_block(&mut data);
        Self { key: data.into() }
    }

    fn padded_data(j: ExposureTime) -> [u8; Self::KEY_LEN] {
        let mut padded_data = [0; Self::KEY_LEN];
        for (i, byte) in Self::INFO.as_bytes().iter().enumerate() {
            padded_data[i] = *byte;
        }
        for (i, byte) in j.as_bytes().iter().enumerate() {
            padded_data[i + 12] = *byte;
        }
        padded_data
    }
}

impl Key for RollingProximityIdentifier {
    const KEY_LEN: usize = 16;

    fn get(&self) -> &[u8] {
        &self.key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tek_rolling_period() {
        let tekrp = TekRollingPeriod::default();
        assert_eq!(Duration::hours(24), tekrp.into());
    }

    #[test]
    fn test_infection_period() {
        let tekrp = TekRollingPeriod::default();
        let infection_period = InfectionPeriod::default();
        assert_eq!(Duration::days(14), infection_period.as_duration(tekrp));
    }
}
