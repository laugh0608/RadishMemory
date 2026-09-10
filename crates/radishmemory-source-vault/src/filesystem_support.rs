use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use crate::envelope::MAX_ENVELOPE_BYTES;
use crate::{SourceVaultError, SourceVaultErrorCode};

pub(crate) struct Directory {
    pub path: PathBuf,
    handle: File,
    identity: Identity,
}

impl Directory {
    pub fn open(path: PathBuf) -> Result<Self, SourceVaultError> {
        let before = fs::symlink_metadata(&path)
            .map_err(|e| SourceVaultError::io("inspect directory", e))?;
        require_directory(&before)?;
        let before_handle = reference_handle(&path)?;
        require_directory(
            &before_handle
                .metadata()
                .map_err(|e| SourceVaultError::io("inspect directory reference", e))?,
        )?;
        let before_identity = Identity::of(&before_handle)?;
        let mut options = no_follow_options();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // BACKUP_SEMANTICS | OPEN_REPARSE_POINT. Pin directory against rename/delete.
            options
                .write(true)
                .custom_flags(0x02000000 | 0x00200000)
                .share_mode(3);
        }
        let handle = options
            .open(&path)
            .map_err(|e| SourceVaultError::io("open directory capability", e))?;
        let opened = handle
            .metadata()
            .map_err(|e| SourceVaultError::io("inspect directory handle", e))?;
        require_directory(&opened)?;
        let identity = Identity::of(&handle)?;
        if identity != before_identity {
            return Err(changed());
        }
        let directory = Self {
            path,
            handle,
            identity,
        };
        directory.verify()?;
        Ok(directory)
    }

    pub fn child(&self, name: &str) -> Result<Self, SourceVaultError> {
        self.verify()?;
        let path = self.path.join(name);
        let builder = DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        match builder.create(&path) {
            Ok(()) => (),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(SourceVaultError::io("create object directory", e)),
        }
        self.verify()?;
        let child = Self::open(path)?;
        self.sync()?;
        child.sync()?;
        Ok(child)
    }

    pub fn verify(&self) -> Result<(), SourceVaultError> {
        let metadata = fs::symlink_metadata(&self.path)
            .map_err(|e| SourceVaultError::io("recheck directory capability", e))?;
        require_directory(&metadata)?;
        let current = reference_handle(&self.path)?;
        require_directory(
            &current
                .metadata()
                .map_err(|e| SourceVaultError::io("inspect directory reference", e))?,
        )?;
        if Identity::of(&current)? != self.identity
            || fs::canonicalize(&self.path)
                .map_err(|e| SourceVaultError::io("resolve directory capability", e))?
                != self.path
        {
            return Err(changed());
        }
        Ok(())
    }

    pub fn sync(&self) -> Result<(), SourceVaultError> {
        self.verify()?;
        // Unsupported directory durability is an error, never a successful no-op.
        self.handle
            .sync_all()
            .map_err(|e| SourceVaultError::io("sync directory", e))?;
        self.verify()
    }
}

pub(crate) fn application_root(path: &Path) -> Result<PathBuf, SourceVaultError> {
    if !cfg!(any(target_os = "macos", target_os = "linux", windows)) {
        return Err(SourceVaultError::io(
            "unsupported filesystem platform",
            io::ErrorKind::Unsupported.into(),
        ));
    }
    if !path.is_absolute()
        || path.parent().is_none()
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(invalid_directory());
    }
    require_directory(
        &fs::symlink_metadata(path)
            .map_err(|e| SourceVaultError::io("inspect application root", e))?,
    )?;
    let resolved =
        fs::canonicalize(path).map_err(|e| SourceVaultError::io("resolve application root", e))?;
    // A trusted platform caller must supply a dedicated application directory, never an ambient root.
    let mut ambient = vec![std::env::temp_dir()];
    if let Ok(cwd) = std::env::current_dir() {
        ambient.push(cwd);
    }
    for key in ["HOME", "USERPROFILE"] {
        if let Some(value) = std::env::var_os(key) {
            ambient.push(PathBuf::from(value));
        }
    }
    if resolved.parent().is_none()
        || ambient
            .iter()
            .filter_map(|p| fs::canonicalize(p).ok())
            .any(|p| p == resolved)
    {
        return Err(invalid_directory());
    }
    Ok(resolved)
}

pub(crate) fn create_new(path: &Path) -> Result<File, SourceVaultError> {
    let mut options = no_follow_options();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|e| {
        if e.kind() == io::ErrorKind::AlreadyExists {
            exists()
        } else {
            SourceVaultError::io("create encrypted staging object", e)
        }
    })
}

pub(crate) fn read_file(path: &Path) -> Result<(Vec<u8>, Observation), SourceVaultError> {
    let observed = Observation::open(path)?;
    let mut file = no_follow_options()
        .read(true)
        .open(path)
        .map_err(|e| SourceVaultError::io("open encrypted object", e))?;
    observed.verify_handle(&file)?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_ENVELOPE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| SourceVaultError::io("read encrypted object", e))?;
    if bytes.len() > MAX_ENVELOPE_BYTES || bytes.len() as u64 != observed.snapshot.length {
        return Err(changed());
    }
    observed.verify_handle(&file)?;
    verify_file(path, &observed)?;
    Ok((bytes, observed))
}

pub(crate) fn verify_file(path: &Path, observed: &Observation) -> Result<(), SourceVaultError> {
    if Observation::open(path)? != *observed {
        return Err(changed());
    }
    Ok(())
}

pub(crate) fn present(path: &Path) -> Result<bool, SourceVaultError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            require_file(&metadata)?;
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(SourceVaultError::io("inspect attempt object", e)),
    }
}

pub(crate) struct Observation {
    snapshot: Snapshot,
    // Keep the original file alive until the observation expires, preventing file-ID reuse.
    // Windows uses a metadata-only handle with read/write/delete sharing, so this does not prevent
    // the caller's hard-link publication or cleanup, and replacement tests still exercise rejection.
    _handle: File,
}
impl PartialEq for Observation {
    fn eq(&self, other: &Self) -> bool {
        self.snapshot == other.snapshot
    }
}
impl Eq for Observation {}

impl Observation {
    pub fn of(file: &File, path: &Path) -> Result<Self, SourceVaultError> {
        let observed = Self::open(path)?;
        // A path opened for retention must refer to the file actually written/read by the caller.
        observed.verify_handle(file)?;
        Ok(observed)
    }

    fn open(path: &Path) -> Result<Self, SourceVaultError> {
        let before = fs::symlink_metadata(path).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                SourceVaultError::new(SourceVaultErrorCode::ObjectMissing, "object is missing")
            } else {
                SourceVaultError::io("inspect encrypted object", e)
            }
        })?;
        require_file(&before)?;
        let handle = reference_handle(path)?;
        let snapshot = Snapshot::of(&handle)?;
        Ok(Self {
            snapshot,
            _handle: handle,
        })
    }

    fn verify_handle(&self, file: &File) -> Result<(), SourceVaultError> {
        if Snapshot::of(file)? != self.snapshot {
            return Err(changed());
        }
        Ok(())
    }
}

#[derive(Eq, PartialEq)]
struct Snapshot {
    identity: Identity,
    length: u64,
    modified: SystemTime,
}
impl Snapshot {
    fn of(file: &File) -> Result<Self, SourceVaultError> {
        let metadata = file
            .metadata()
            .map_err(|e| SourceVaultError::io("inspect object handle", e))?;
        require_file(&metadata)?;
        Ok(Self {
            identity: Identity::of(file)?,
            length: metadata.len(),
            modified: metadata
                .modified()
                .map_err(|e| SourceVaultError::io("inspect modification time", e))?,
        })
    }
}

#[derive(Eq, PartialEq)]
struct Identity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    native: radishmemory_windows_filesystem::FileIdentity,
}
impl Identity {
    fn of(file: &File) -> Result<Self, SourceVaultError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = file
                .metadata()
                .map_err(|e| SourceVaultError::io("inspect filesystem identity", e))?;
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }
        #[cfg(windows)]
        {
            Ok(Self {
                native: radishmemory_windows_filesystem::file_identity(file)
                    .map_err(|e| SourceVaultError::io("inspect filesystem identity", e))?,
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = file;
            Err(SourceVaultError::io(
                "unsupported filesystem identity",
                io::ErrorKind::Unsupported.into(),
            ))
        }
    }
}

fn reference_handle(path: &Path) -> Result<File, SourceVaultError> {
    let mut options = no_follow_options();
    #[cfg(not(windows))]
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // No data access; allow mutation of names while retaining the file's identity. Open the
        // reparse point itself (never its target); BACKUP_SEMANTICS also permits directory handles.
        options
            .access_mode(0)
            .share_mode(7)
            .custom_flags(0x02000000 | 0x00200000);
    }
    options
        .open(path)
        .map_err(|e| SourceVaultError::io("open filesystem identity reference", e))
}

fn no_follow_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Darwin sys/fcntl.h: O_NOFOLLOW | O_NONBLOCK (also avoids blocking on a substituted FIFO).
        options.custom_flags(0x100 | 0x4);
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Linux asm-generic/fcntl.h on the reviewed ARM64/x86_64 targets.
        options.custom_flags(0x20000 | 0x800);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // OPEN_REPARSE_POINT; reads exclude write/delete sharing while the handle is held.
        options.custom_flags(0x00200000).share_mode(1);
    }
    options
}

fn is_link(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
fn require_directory(metadata: &Metadata) -> Result<(), SourceVaultError> {
    if is_link(metadata) || !metadata.is_dir() {
        return Err(invalid_directory());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(invalid_directory());
        }
    }
    Ok(())
}
fn require_file(metadata: &Metadata) -> Result<(), SourceVaultError> {
    if is_link(metadata) || !metadata.is_file() || metadata.len() > MAX_ENVELOPE_BYTES as u64 {
        return Err(SourceVaultError::new(
            SourceVaultErrorCode::InvalidFile,
            "object must be a bounded regular file",
        ));
    }
    Ok(())
}
fn invalid_directory() -> SourceVaultError {
    SourceVaultError::new(
        SourceVaultErrorCode::InvalidDirectory,
        "dedicated private application directory required",
    )
}
pub(crate) fn changed() -> SourceVaultError {
    SourceVaultError::new(
        SourceVaultErrorCode::FilesystemChanged,
        "filesystem identity or content changed",
    )
}
pub(crate) fn exists() -> SourceVaultError {
    SourceVaultError::new(
        SourceVaultErrorCode::ObjectExists,
        "object or staging target already exists",
    )
}
