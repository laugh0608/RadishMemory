use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::random::RandomSource;
use crate::{KeyEncryptionKey, ObjectMetadata, SourceVaultError};

pub(crate) struct FixedRandom(pub u8);
impl RandomSource for FixedRandom {
    fn fill(&mut self, destination: &mut [u8]) -> Result<(), SourceVaultError> {
        for byte in destination {
            *byte = self.0;
            self.0 = self.0.wrapping_add(1);
        }
        Ok(())
    }
}
pub(crate) fn key() -> KeyEncryptionKey {
    KeyEncryptionKey::new([0xa5; 32])
}
pub(crate) fn metadata(bytes: &[u8]) -> ObjectMetadata {
    ObjectMetadata::new(
        "namespace-synthetic",
        "source-synthetic",
        Sha256::digest(bytes).into(),
        bytes.len() as u64,
        "text/markdown",
    )
    .unwrap()
}

pub(crate) struct TestDirectory(pub PathBuf);
impl TestDirectory {
    pub fn new() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "radishmemory-s03b-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder
            .create(&path)
            .expect("create isolated synthetic application root");
        Self(fs::canonicalize(path).unwrap())
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove this test's isolated synthetic directory");
    }
}
