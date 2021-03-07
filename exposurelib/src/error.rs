use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExposurelibError {
    #[error("CSRNG error")]
    RandomKeyGenerationError,

    #[error("HKDF error")]
    KeyDerivationError,
}
