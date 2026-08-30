use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::{DesktopError, DesktopErrorCode, DesktopErrorReason};

const QUALIFIER: &str = "io.github";
const ORGANIZATION: &str = "laugh0608";
const APPLICATION: &str = "RadishMemory";

pub struct ApplicationPaths {
    data_directory: PathBuf,
    profile_path: PathBuf,
    database_path: PathBuf,
}

impl ApplicationPaths {
    pub fn resolve() -> Result<Self, DesktopError> {
        let project = ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION).ok_or_else(|| {
            DesktopError::without_source(
                DesktopErrorCode::ApplicationDirectory,
                DesktopErrorReason::ProjectDirectoryUnavailable,
                false,
            )
        })?;
        Self::from_data_directory(project.data_local_dir())
    }

    pub fn from_data_directory(path: impl Into<PathBuf>) -> Result<Self, DesktopError> {
        let data_directory = path.into();
        if !data_directory.is_absolute() || data_directory.as_os_str().is_empty() {
            return Err(DesktopError::without_source(
                DesktopErrorCode::ApplicationDirectory,
                DesktopErrorReason::DataDirectoryInvalid,
                false,
            ));
        }
        Ok(Self {
            profile_path: data_directory.join("host-profile-v1.txt"),
            database_path: data_directory.join("library.sqlite3"),
            data_directory,
        })
    }

    pub fn prepare(&self) -> Result<(), DesktopError> {
        match fs::symlink_metadata(&self.data_directory) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(DesktopError::without_source(
                        DesktopErrorCode::ApplicationDirectory,
                        DesktopErrorReason::DataDirectoryInvalid,
                        false,
                    ));
                }
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                fs::create_dir_all(&self.data_directory).map_err(|source| {
                    DesktopError::io(
                        DesktopErrorCode::ApplicationDirectory,
                        DesktopErrorReason::DataDirectoryCreateFailed,
                        source.kind() == io::ErrorKind::Interrupted,
                        &source,
                    )
                })?;
            }
            Err(source) => {
                return Err(DesktopError::io(
                    DesktopErrorCode::ApplicationDirectory,
                    DesktopErrorReason::DataDirectoryInvalid,
                    source.kind() == io::ErrorKind::Interrupted,
                    &source,
                ));
            }
        }
        let metadata = fs::symlink_metadata(&self.data_directory).map_err(|source| {
            DesktopError::io(
                DesktopErrorCode::ApplicationDirectory,
                DesktopErrorReason::DataDirectoryInvalid,
                source.kind() == io::ErrorKind::Interrupted,
                &source,
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(DesktopError::without_source(
                DesktopErrorCode::ApplicationDirectory,
                DesktopErrorReason::DataDirectoryInvalid,
                false,
            ));
        }
        set_owner_only_directory(&self.data_directory)?;
        self.database_exists().map(|_| ())
    }

    pub fn database_exists(&self) -> Result<bool, DesktopError> {
        match fs::symlink_metadata(&self.database_path) {
            Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_file() => Ok(true),
            Ok(_) => Err(DesktopError::without_source(
                DesktopErrorCode::ApplicationDirectory,
                DesktopErrorReason::DatabasePathInvalid,
                false,
            )),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(source) => Err(DesktopError::io(
                DesktopErrorCode::ApplicationDirectory,
                DesktopErrorReason::DatabasePathInvalid,
                source.kind() == io::ErrorKind::Interrupted,
                &source,
            )),
        }
    }

    #[must_use]
    pub(crate) fn data_directory(&self) -> &Path {
        &self.data_directory
    }

    #[must_use]
    pub(crate) fn profile_path(&self) -> &Path {
        &self.profile_path
    }

    #[must_use]
    pub(crate) fn database_path(&self) -> &Path {
        &self.database_path
    }
}

impl std::fmt::Debug for ApplicationPaths {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ApplicationPaths")
            .field("resolved", &true)
            .finish_non_exhaustive()
    }
}

#[cfg(unix)]
fn set_owner_only_directory(path: &Path) -> Result<(), DesktopError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|source| {
        DesktopError::io(
            DesktopErrorCode::ApplicationDirectory,
            DesktopErrorReason::DataDirectoryCreateFailed,
            source.kind() == io::ErrorKind::Interrupted,
            &source,
        )
    })
}

#[cfg(not(unix))]
fn set_owner_only_directory(_path: &Path) -> Result<(), DesktopError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "radishmemory-desktop-paths-{}-{label}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn relative_application_directory_is_rejected() {
        let error = ApplicationPaths::from_data_directory("relative-data").unwrap_err();
        assert_eq!(error.reason(), DesktopErrorReason::DataDirectoryInvalid);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_application_directory_is_rejected() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new("symlink");
        let target = directory.0.join("target");
        let link = directory.0.join("data-link");
        fs::create_dir(&target).unwrap();
        symlink(&target, &link).unwrap();
        let paths = ApplicationPaths::from_data_directory(&link).unwrap();
        let error = paths.prepare().unwrap_err();
        assert_eq!(error.reason(), DesktopErrorReason::DataDirectoryInvalid);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_database_is_rejected() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new("database-symlink");
        let data = directory.0.join("data");
        let target = directory.0.join("other.sqlite3");
        fs::create_dir(&data).unwrap();
        fs::write(&target, b"synthetic-marker").unwrap();
        symlink(&target, data.join("library.sqlite3")).unwrap();
        let paths = ApplicationPaths::from_data_directory(&data).unwrap();
        let error = paths.prepare().unwrap_err();
        assert_eq!(error.reason(), DesktopErrorReason::DatabasePathInvalid);
    }
}
