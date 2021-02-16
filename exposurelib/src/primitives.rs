use super::time::ExposureTime;
use ring::hkdf::HKDF_SHA256;
use ring::hkdf::Salt;
use ring::rand::SecureRandom;

pub struct ExposureKeys {
    tek: TemporaryExposureKey,
    rpik: DerivedKey,
    aemk: DerivedKey,
    valid_from: ExposureTime,
}

pub struct TemporaryExposureKey {
    key: [u8; Self::KEY_LEN],
}

impl TemporaryExposureKey {
    const KEY_LEN: usize = 16;

    pub fn new(secure_random: &dyn SecureRandom) -> Self {
        let mut key = [0; Self::KEY_LEN];
        match secure_random.fill(&mut key) {
            Ok(()) => TemporaryExposureKey { key },
            Err(e) => panic!("Randomness error while generating TEK: {}.", e),
        }
    }
}

pub trait Key {
    const KEY_LEN: usize = 16;
    fn get(&self) -> &[u8];
}

impl Key for TemporaryExposureKey {
    fn get(&self) -> &[u8] {
        &self.key
    }
}

/// This can be used as both a Rolling Proximity Identifier Key (RPIK)
/// and a Associated Encrypted Metadata Key (AEMK)
pub struct DerivedKey {
    key: [u8; Self::KEY_LEN],
}

// TODO:
// make info a type that has "EN-RPIK" as constant?
impl DerivedKey {
    pub fn new<T: AsRef<[u8]>>(tek: &TemporaryExposureKey, info: T) -> Self {
        let mut key = [0; Self::KEY_LEN];
        Salt::new(HKDF_SHA256, &[])
            .extract(tek.get())
            .expand(&[info.as_ref()], Wrapper(Self::KEY_LEN))
            .expect("HKDF error while expand().")
            .fill(&mut key)
            .expect("HKDF error while fill().");
        Self { key }
    }
}

impl Key for DerivedKey {
    fn get(&self) -> &[u8] {
        &self.key
    }
}

struct Wrapper<T>(T);

impl ring::hkdf::KeyType for Wrapper<usize> {
    fn len(&self) -> usize {
        self.0
    }
}

pub struct RollingProximityIdentifier {}
