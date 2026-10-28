use std::{
    fs, io,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use radishmemory_application::{ApplicationIdentifierKind, ApplicationRuntime, LocalLibraryConfig};
use radishmemory_core::{Identifier, Timestamp};

pub struct Directory(pub PathBuf);

impl Directory {
    pub fn new() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "radishmemory-q01-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        // This path was exclusively created above, never supplied by a caller.
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[derive(Default)]
pub struct Runtime(u64);

impl ApplicationRuntime for Runtime {
    type Error = io::Error;

    fn next_identifier(
        &mut self,
        kind: ApplicationIdentifierKind,
    ) -> Result<Identifier, io::Error> {
        self.0 += 1;
        let prefix = if kind == ApplicationIdentifierKind::OriginBinding {
            "origin-binding-q01"
        } else {
            "synthetic-q01"
        };
        Ok(Identifier::new(format!("{prefix}-{}", self.0)).unwrap())
    }

    fn now(&mut self) -> Result<Timestamp, io::Error> {
        Ok(Timestamp::parse("2026-10-28T00:00:00Z").unwrap())
    }
}

pub fn config() -> LocalLibraryConfig {
    LocalLibraryConfig::phase1_local(
        Identifier::new("namespace-0123456789abcdef0123456789abcdef").unwrap(),
        Identifier::new("device-fedcba9876543210fedcba9876543210").unwrap(),
    )
    .unwrap()
}
