use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub struct SyntheticDatabase {
    path: PathBuf,
}

impl SyntheticDatabase {
    pub fn new(label: &str) -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "radishmemory-synthetic-{label}-{}-{sequence}.sqlite3",
            std::process::id()
        ));
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SyntheticDatabase {
    fn drop(&mut self) {
        for suffix in ["", "-journal", "-shm", "-wal"] {
            let mut candidate = self.path.as_os_str().to_owned();
            candidate.push(suffix);
            if let Err(error) = std::fs::remove_file(&candidate)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                panic!("synthetic SQLite test cleanup failed: {error}");
            }
        }
    }
}
