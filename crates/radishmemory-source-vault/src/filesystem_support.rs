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
        let identity = Identity::of(&opened)?;
        if identity != Identity::of(&before)? {
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
        if Identity::of(&metadata)? != self.identity
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
    let before = fs::symlink_metadata(path).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            SourceVaultError::new(SourceVaultErrorCode::ObjectMissing, "object is missing")
        } else {
            SourceVaultError::io("inspect encrypted object", e)
        }
    })?;
    require_file(&before)?;
    let mut file = no_follow_options()
        .read(true)
        .open(path)
        .map_err(|e| SourceVaultError::io("open encrypted object", e))?;
    let observed = Observation::of(
        &file
            .metadata()
            .map_err(|e| SourceVaultError::io("inspect encrypted object handle", e))?,
    )?;
    if observed != Observation::of(&before)? {
        return Err(changed());
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_ENVELOPE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| SourceVaultError::io("read encrypted object", e))?;
    if bytes.len() > MAX_ENVELOPE_BYTES
        || bytes.len() as u64 != observed.length
        || Observation::of(
            &file
                .metadata()
                .map_err(|e| SourceVaultError::io("recheck encrypted object handle", e))?,
        )? != observed
    {
        return Err(changed());
    }
    verify_file(path, &observed)?;
    Ok((bytes, observed))
}

pub(crate) fn verify_file(path: &Path, observed: &Observation) -> Result<(), SourceVaultError> {
    let current = fs::symlink_metadata(path)
        .map_err(|e| SourceVaultError::io("recheck encrypted object identity", e))?;
    if Observation::of(&current)? != *observed {
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

#[derive(Eq, PartialEq)]
pub(crate) struct Observation {
    identity: Identity,
    length: u64,
    modified: SystemTime,
}
impl Observation {
    pub fn of(metadata: &Metadata) -> Result<Self, SourceVaultError> {
        require_file(metadata)?;
        Ok(Self {
            identity: Identity::of(metadata)?,
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
    #[cfg(not(unix))]
    created: SystemTime,
}
impl Identity {
    fn of(metadata: &Metadata) -> Result<Self, SourceVaultError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {
                created: metadata
                    .created()
                    .map_err(|e| SourceVaultError::io("inspect creation time", e))?,
            })
        }
    }
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
