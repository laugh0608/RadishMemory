use crate::error::{SourceVaultError, SourceVaultErrorCode};

pub(crate) trait RandomSource {
    fn fill(&mut self, destination: &mut [u8]) -> Result<(), SourceVaultError>;
}

pub(crate) struct SystemRandom;

impl RandomSource for SystemRandom {
    fn fill(&mut self, destination: &mut [u8]) -> Result<(), SourceVaultError> {
        getrandom::fill(destination).map_err(|_| {
            SourceVaultError::new(
                SourceVaultErrorCode::RandomSourceUnavailable,
                "system random source unavailable",
            )
        })
    }
}
