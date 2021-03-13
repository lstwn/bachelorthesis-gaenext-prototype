use super::time::ExposureTime;
use crate::config::Intensity;
use crate::error::ExposurelibError;
use aes::cipher::generic_array::GenericArray;
use aes::{Aes128, BlockCipher, NewBlockCipher};
use chrono::Duration;
use ring::hkdf::Salt;
use ring::hkdf::HKDF_SHA256;
use ring::rand::SecureRandom;
pub use ring::rand::SystemRandom;
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use std::net::SocketAddr;

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
pub struct Validity<Keyring> {
    valid_from: ExposureTime,
    keyring: Keyring,
}

impl<Keyring> Validity<Keyring> {
    pub fn new(exposure_time: ExposureTime, tekrp: TekRollingPeriod, keyring: Keyring) -> Self {
        Self {
            valid_from: exposure_time.floor_tekrp_multiple(tekrp),
            keyring,
        }
    }
    pub fn keyring(&self) -> &Keyring {
        &self.keyring
    }
    pub fn valid_from(&self) -> ExposureTime {
        self.valid_from
    }
    pub fn query(&self, exposure_time: ExposureTime, tekrp: TekRollingPeriod) -> Option<&Keyring> {
        if exposure_time.floor_tekrp_multiple(tekrp) == self.valid_from {
            Some(self.keyring())
        } else {
            None
        }
    }
}

impl TryFrom<Validity<TemporaryExposureKey>> for Validity<TekKeyring> {
    type Error = ExposurelibError;

    fn try_from(tek_validity: Validity<TemporaryExposureKey>) -> Result<Self, Self::Error> {
        Ok(Self {
            valid_from: tek_validity.valid_from,
            keyring: TekKeyring::try_from(tek_validity.keyring)?,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TekKeyring {
    tek: TemporaryExposureKey,
    rpik: RollingProximityIdentifierKey,
    aemk: AssociatedEncryptedMetadataKey,
}

impl TekKeyring {
    pub fn rpi(&self, at: ExposureTime) -> RollingProximityIdentifier {
        RollingProximityIdentifier::new(&self.rpik, at)
    }
    pub fn aemk(&self) -> &AssociatedEncryptedMetadataKey {
        &self.aemk
    }
    fn aem(
        &self,
        rpi: &RollingProximityIdentifier,
        metadata: Metadata,
    ) -> AssociatedEncryptedMetadata {
        AssociatedEncryptedMetadata::encrypt(&self.aemk, rpi, metadata)
    }
    pub fn rpi_and_aem(
        &self,
        at: ExposureTime,
        metadata: Metadata,
    ) -> (RollingProximityIdentifier, AssociatedEncryptedMetadata) {
        let rpi = self.rpi(at);
        let aem = self.aem(&rpi, metadata);
        (rpi, aem)
    }
}

impl TryFrom<TemporaryExposureKey> for TekKeyring {
    type Error = ExposurelibError;

    fn try_from(tek: TemporaryExposureKey) -> Result<Self, Self::Error> {
        Ok(Self {
            tek,
            rpik: RollingProximityIdentifierKey::new(&tek)?,
            aemk: AssociatedEncryptedMetadataKey::new(&tek)?,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SdKeyring {
    sd: Seed,
    pksk: PublicKeySymmetricKey,
}

impl SdKeyring {
    // pub fn epk(&self, tek: &TemporaryExposureKey, pk: PublicKey) -> EncryptedPublicKey {
    // }
}

impl TryFrom<Seed> for SdKeyring {
    type Error = ExposurelibError;

    fn try_from(sd: Seed) -> Result<Self, Self::Error> {
        Ok(Self {
            sd,
            pksk: PublicKeySymmetricKey::new(&sd)?,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExposureKeyring {
    tek_keyring: TekKeyring,
    sd_keyring: SdKeyring,
}

impl ExposureKeyring {
    pub fn new(secure_random: &dyn SecureRandom) -> Result<Self, ExposurelibError> {
        Self::from_tek_and_sd(
            TemporaryExposureKey::new(secure_random)?,
            Seed::new(secure_random)?,
        )
    }
    pub fn from_tek_and_sd(tek: TemporaryExposureKey, sd: Seed) -> Result<Self, ExposurelibError> {
        Ok(Self {
            tek_keyring: TekKeyring::try_from(tek)?,
            sd_keyring: SdKeyring::try_from(sd)?,
        })
    }
    pub fn tek_keyring(&self) -> &TekKeyring {
        &self.tek_keyring
    }
    pub fn sd_keyring(&self) -> &SdKeyring {
        &self.sd_keyring
    }
    // pub fn epk(&self, pk: PublicKey) -> EncryptedPublicKey {
    //     self.sd_keyring().epk(&self.tek_keyring().tek, pk)
    // }
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
        Duration::from(tekrp) * (self.0 as i32)
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
        for (i, byte) in <[u8; std::mem::size_of::<u32>()]>::from(j)
            .iter()
            .enumerate()
        {
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

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub struct Metadata {
    intensity: Intensity,
    // actually seed instead of connection identifier
    connection_identifier: SocketAddr,
}

impl Metadata {
    pub fn new(intensity: Intensity, connection_identifier: SocketAddr) -> Self {
        Self {
            intensity,
            connection_identifier,
        }
    }
    pub fn intensity(&self) -> Intensity {
        self.intensity
    }
    pub fn connection_identifier(&self) -> SocketAddr {
        self.connection_identifier
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssociatedEncryptedMetadata {
    ciphertext: Metadata,
}

impl AssociatedEncryptedMetadata {
    pub fn encrypt(
        _aemk: &AssociatedEncryptedMetadataKey,
        _rpi: &RollingProximityIdentifier,
        metadata: Metadata,
    ) -> Self {
        Self {
            ciphertext: metadata,
        }
    }
    pub fn decrypt(
        &self,
        _aemk: &AssociatedEncryptedMetadataKey,
        _rpi: &RollingProximityIdentifier,
    ) -> Metadata {
        self.ciphertext
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tek_rolling_period() {
        let tekrp = TekRollingPeriod::default();
        assert_eq!(Duration::hours(24), Duration::from(tekrp));
    }

    #[test]
    fn test_infection_period() {
        let tekrp = TekRollingPeriod::default();
        let infection_period = InfectionPeriod::default();
        assert_eq!(Duration::days(14), infection_period.as_duration(tekrp));
    }
}
