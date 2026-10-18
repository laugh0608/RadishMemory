use super::{EventSink, ObservationError};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

/// A checked destination, never a credential or library path. Debug intentionally absent.
pub struct TraceFile {
    path: PathBuf,
    forbidden: Vec<PathBuf>,
    ancestors: Vec<(PathBuf, Identity)>,
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct Identity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}
fn identity(metadata: &fs::Metadata) -> Identity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Identity {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Identity {}
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct Stamp {
    modified: std::time::SystemTime,
    #[cfg(unix)]
    changed: (i64, i64),
}
fn stamp(metadata: &fs::Metadata) -> io::Result<Stamp> {
    Ok(Stamp {
        modified: metadata.modified()?,
        #[cfg(unix)]
        changed: {
            use std::os::unix::fs::MetadataExt;
            (metadata.ctime(), metadata.ctime_nsec())
        },
    })
}
fn overlap(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
fn resolve_missing(path: &Path) -> Result<PathBuf, ObservationError> {
    for ancestor in path.ancestors() {
        match fs::canonicalize(ancestor) {
            Ok(resolved) => {
                return Ok(
                    resolved.join(path.strip_prefix(ancestor).map_err(|_| ObservationError)?)
                );
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(ObservationError),
        }
    }
    Err(ObservationError)
}
impl TraceFile {
    /// Checks only; the parent must already exist, be private, and have no symlink ancestors.
    pub fn validate(path: &Path, forbidden: &[&Path]) -> Result<Self, ObservationError> {
        if !cfg!(target_os = "macos")
            || !path.is_absolute()
            || path.file_name().is_none()
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        {
            return Err(ObservationError);
        }
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            _ => return Err(ObservationError),
        }
        let parent = path.parent().ok_or(ObservationError)?;
        let mut ancestors = Vec::new();
        for ancestor in parent.ancestors() {
            let metadata = fs::symlink_metadata(ancestor).map_err(|_| ObservationError)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(ObservationError);
            }
            ancestors.push((ancestor.to_path_buf(), identity(&metadata)));
        }
        if fs::canonicalize(parent)
            .map_err(|_| ObservationError)?
            .as_os_str()
            != parent.as_os_str()
        {
            return Err(ObservationError);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(parent)
                .map_err(|_| ObservationError)?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err(ObservationError);
            }
        }
        let forbidden = forbidden
            .iter()
            .map(|root| resolve_missing(root))
            .collect::<Result<Vec<_>, _>>()?;
        if forbidden.iter().any(|root| overlap(path, root)) {
            return Err(ObservationError);
        }
        Ok(Self {
            path: path.to_path_buf(),
            forbidden,
            ancestors,
        })
    }
    fn check_ancestors(&self) -> Result<(), ObservationError> {
        for (path, original) in &self.ancestors {
            let metadata = fs::symlink_metadata(path).map_err(|_| ObservationError)?;
            if metadata.file_type().is_symlink()
                || !metadata.is_dir()
                || identity(&metadata) != *original
            {
                return Err(ObservationError);
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(self.path.parent().ok_or(ObservationError)?)
                .map_err(|_| ObservationError)?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err(ObservationError);
            }
        }
        if self.forbidden.iter().any(|root| overlap(&self.path, root)) {
            return Err(ObservationError);
        }
        Ok(())
    }
    pub(super) fn create(self) -> Result<FileSink, ObservationError> {
        self.check_ancestors()?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&self.path).map_err(|_| ObservationError)?;
        let metadata = file.metadata().map_err(|_| ObservationError)?;
        let original = identity(&metadata);
        let last_write = stamp(&metadata).map_err(|_| ObservationError)?;
        let sink = FileSink {
            config: self,
            file,
            original,
            last_write,
            bytes: 0,
        };
        sink.check().map_err(|_| ObservationError)?;
        Ok(sink)
    }
}
pub(super) struct FileSink {
    config: TraceFile,
    file: File,
    original: Identity,
    last_write: Stamp,
    bytes: u64,
}
impl FileSink {
    fn check(&self) -> io::Result<()> {
        let invalid = || io::Error::from(io::ErrorKind::InvalidData);
        self.config.check_ancestors().map_err(|_| invalid())?;
        for metadata in [
            self.file.metadata()?,
            fs::symlink_metadata(&self.config.path)?,
        ] {
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || identity(&metadata) != self.original
                || metadata.len() != self.bytes
                || stamp(&metadata)? != self.last_write
            {
                return Err(invalid());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::{MetadataExt, PermissionsExt};
                if metadata.nlink() != 1 || metadata.permissions().mode() & 0o777 != 0o600 {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }
}
impl EventSink for FileSink {
    fn emit(&mut self, event: &[u8]) -> io::Result<()> {
        self.check()?;
        self.file.write_all(event)?;
        self.file.flush()?;
        self.bytes += event.len() as u64;
        self.last_write = stamp(&self.file.metadata()?)?;
        self.check()
    }
}
