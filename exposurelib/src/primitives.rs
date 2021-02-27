use super::time::ExposureTime;
use aes::cipher::generic_array::GenericArray;
use aes::{Aes128, BlockCipher, NewBlockCipher};
use ring::hkdf::Salt;
use ring::hkdf::HKDF_SHA256;
use ring::rand::SecureRandom;
use serde::{Deserialize, Serialize};
use std::convert::From;
use std::convert::TryInto;

#[derive(Serialize, Deserialize)]
pub struct KeyForward {
    valid_from: ExposureTime,
    origin_tek: TemporaryExposureKey,
    // NOTE: omitting (origin) EPK in the prototype
    predecessor_tek: TemporaryExposureKey,
}

#[derive(Serialize, Deserialize)]
#[serde(rename = "TEK")]
pub struct KeyUpload {
    valid_from: ExposureTime,
    tek: TemporaryExposureKey,
    // NOTE: omitting EPK in the prototype
}

impl From<ExposureKeys> for KeyUpload {
    fn from(exposure_keys: ExposureKeys) -> Self {
        Self {
            valid_from: exposure_keys.valid_from,
            tek: exposure_keys.tek,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(into = "KeyUpload", from = "KeyUpload")]
pub struct ExposureKeys {
    valid_from: ExposureTime,
    tek: TemporaryExposureKey,
    // NOTE: omitting SD, PKSK, EPK in the prototype
    rpik: RollingProximityIdentifierKey,
    aemk: AssociatedEncryptedMetadataKey,
}

impl From<KeyUpload> for ExposureKeys {
    fn from(key_upload: KeyUpload) -> Self {
        let rpik = RollingProximityIdentifierKey::new(&key_upload.tek);
        let aemk = AssociatedEncryptedMetadataKey::new(&key_upload.tek);
        Self {
            valid_from: key_upload.valid_from,
            tek: key_upload.tek,
            rpik,
            aemk,
        }
    }
}

pub trait Key {
    const KEY_LEN: usize;
    fn get(&self) -> &[u8];
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporaryExposureKey {
    key: [u8; Self::KEY_LEN],
}

impl TemporaryExposureKey {
    pub fn new(secure_random: &dyn SecureRandom) -> Self {
        let mut key = [0; Self::KEY_LEN];
        match secure_random.fill(&mut key) {
            Ok(()) => TemporaryExposureKey { key },
            Err(e) => panic!("Randomness error while generating TEK: {}.", e),
        }
    }
}

impl Key for TemporaryExposureKey {
    const KEY_LEN: usize = 16;

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
    fn derive<T: Key>(key_material: &T) -> Vec<u8> {
        let mut key = vec![0; Self::KEY_LEN];
        Salt::new(HKDF_SHA256, &[])
            .extract(key_material.get())
            .expand(&[Self::INFO.as_ref()], Wrapper(Self::KEY_LEN))
            .expect("HKDF error while expand().")
            .fill(&mut key)
            .expect("HKDF error while fill().");
        key
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RollingProximityIdentifierKey {
    key: [u8; Self::KEY_LEN],
}

impl RollingProximityIdentifierKey {
    pub fn new(tek: &TemporaryExposureKey) -> Self {
        Self {
            key: Self::derive(tek).try_into().unwrap(),
        }
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct AssociatedEncryptedMetadataKey {
    key: [u8; Self::KEY_LEN],
}

impl AssociatedEncryptedMetadataKey {
    pub fn new(tek: &TemporaryExposureKey) -> Self {
        Self {
            key: Self::derive(tek).try_into().unwrap(),
        }
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

struct Wrapper<T>(T);

impl ring::hkdf::KeyType for Wrapper<usize> {
    fn len(&self) -> usize {
        self.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
