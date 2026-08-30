use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};

use radishmemory_application::{ApplicationIdentifierKind, ApplicationRuntime, Identifier};

use crate::{ApplicationPaths, DesktopError, DesktopErrorCode, DesktopErrorReason};

pub const HOST_PROFILE_CONTRACT_ID: &str = "radishmemory.phase1-host-profile/1";
const PROFILE_MAX_BYTES: u64 = 512;

#[derive(Clone, Eq, PartialEq)]
pub struct HostProfile {
    namespace_id: Identifier,
    device_id: Identifier,
}

impl HostProfile {
    #[must_use]
    pub const fn namespace_id(&self) -> &Identifier {
        &self.namespace_id
    }

    #[must_use]
    pub const fn device_id(&self) -> &Identifier {
        &self.device_id
    }
}

impl fmt::Debug for HostProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostProfile")
            .field("contract_id", &HOST_PROFILE_CONTRACT_ID)
            .finish_non_exhaustive()
    }
}

pub fn load_or_create_host_profile<R>(
    paths: &ApplicationPaths,
    runtime: &mut R,
) -> Result<HostProfile, DesktopError>
where
    R: ApplicationRuntime,
{
    paths.prepare()?;
    let database_exists = paths.database_exists()?;
    match fs::symlink_metadata(paths.profile_path()) {
        Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_file() => {
            load_profile(paths)
        }
        Ok(_) => Err(DesktopError::without_source(
            DesktopErrorCode::HostProfile,
            DesktopErrorReason::ProfileInvalid,
            false,
        )),
        Err(source) if source.kind() == io::ErrorKind::NotFound && database_exists => {
            Err(DesktopError::without_source(
                DesktopErrorCode::HostProfile,
                DesktopErrorReason::ProfileMissingForExistingDatabase,
                false,
            ))
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => create_profile(paths, runtime),
        Err(source) => Err(DesktopError::io(
            DesktopErrorCode::HostProfile,
            DesktopErrorReason::ProfileReadFailed,
            source.kind() == io::ErrorKind::Interrupted,
            &source,
        )),
    }
}

fn create_profile<R>(paths: &ApplicationPaths, runtime: &mut R) -> Result<HostProfile, DesktopError>
where
    R: ApplicationRuntime,
{
    let profile = HostProfile {
        namespace_id: runtime
            .next_identifier(ApplicationIdentifierKind::Namespace)
            .map_err(|_| identity_generation_failed())?,
        device_id: runtime
            .next_identifier(ApplicationIdentifierKind::Device)
            .map_err(|_| identity_generation_failed())?,
    };
    if !valid_profile_identifier(profile.namespace_id(), "namespace-")
        || !valid_profile_identifier(profile.device_id(), "device-")
    {
        return Err(identity_generation_failed());
    }
    let bytes = encode_profile(&profile);
    let temporary_path = paths.data_directory().join(format!(
        ".host-profile-{}.tmp",
        profile.namespace_id().as_str()
    ));
    let publish_result = publish_profile(paths, &temporary_path, &bytes);
    if let Err(source) = fs::remove_file(&temporary_path)
        && source.kind() != io::ErrorKind::NotFound
        && publish_result.is_ok()
    {
        return Err(DesktopError::io(
            DesktopErrorCode::HostProfile,
            DesktopErrorReason::ProfileWriteFailed,
            source.kind() == io::ErrorKind::Interrupted,
            &source,
        ));
    }
    match publish_result {
        Ok(()) => Ok(profile),
        Err(PublishError::AlreadyExists) => load_profile(paths),
        Err(PublishError::Io(source)) => Err(DesktopError::io(
            DesktopErrorCode::HostProfile,
            DesktopErrorReason::ProfileWriteFailed,
            source.kind() == io::ErrorKind::Interrupted,
            &source,
        )),
        Err(PublishError::Verification) => Err(DesktopError::without_source(
            DesktopErrorCode::HostProfile,
            DesktopErrorReason::ProfileWriteFailed,
            false,
        )),
    }
}

enum PublishError {
    AlreadyExists,
    Io(io::Error),
    Verification,
}

fn publish_profile(
    paths: &ApplicationPaths,
    temporary_path: &std::path::Path,
    bytes: &[u8],
) -> Result<(), PublishError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary_path)
        .map_err(PublishError::Io)?;
    set_owner_only_file(&file).map_err(PublishError::Io)?;
    file.write_all(bytes).map_err(PublishError::Io)?;
    file.flush().map_err(PublishError::Io)?;
    file.sync_all().map_err(PublishError::Io)?;
    drop(file);
    let written = read_bounded_file(temporary_path).map_err(PublishError::Io)?;
    if written != bytes {
        return Err(PublishError::Verification);
    }
    match fs::hard_link(temporary_path, paths.profile_path()) {
        Ok(()) => {}
        Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
            return Err(PublishError::AlreadyExists);
        }
        Err(source) => return Err(PublishError::Io(source)),
    }
    sync_parent(paths.data_directory()).map_err(PublishError::Io)?;
    let published = read_bounded_file(paths.profile_path()).map_err(PublishError::Io)?;
    if published != bytes {
        return Err(PublishError::Verification);
    }
    Ok(())
}

fn load_profile(paths: &ApplicationPaths) -> Result<HostProfile, DesktopError> {
    let bytes = read_bounded_file(paths.profile_path()).map_err(|source| {
        DesktopError::io(
            DesktopErrorCode::HostProfile,
            DesktopErrorReason::ProfileReadFailed,
            source.kind() == io::ErrorKind::Interrupted,
            &source,
        )
    })?;
    parse_profile(&bytes).ok_or_else(|| {
        DesktopError::without_source(
            DesktopErrorCode::HostProfile,
            DesktopErrorReason::ProfileInvalid,
            false,
        )
    })
}

fn read_bounded_file(path: &std::path::Path) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > PROFILE_MAX_BYTES
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "host profile file type or size is invalid",
        ));
    }
    let mut file = File::open(path)?;
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    Read::by_ref(&mut file)
        .take(PROFILE_MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > PROFILE_MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "host profile exceeds size limit",
        ));
    }
    Ok(bytes)
}

fn encode_profile(profile: &HostProfile) -> Vec<u8> {
    format!(
        "contract_id={HOST_PROFILE_CONTRACT_ID}\nnamespace_id={}\ndevice_id={}\n",
        profile.namespace_id().as_str(),
        profile.device_id().as_str()
    )
    .into_bytes()
}

fn parse_profile(bytes: &[u8]) -> Option<HostProfile> {
    let value = std::str::from_utf8(bytes).ok()?;
    let lines = value.split('\n').collect::<Vec<_>>();
    if lines.len() != 4 || !lines[3].is_empty() {
        return None;
    }
    let contract = lines[0].strip_prefix("contract_id=")?;
    let namespace = lines[1].strip_prefix("namespace_id=")?;
    let device = lines[2].strip_prefix("device_id=")?;
    if contract != HOST_PROFILE_CONTRACT_ID
        || !valid_profile_identifier_value(namespace, "namespace-")
        || !valid_profile_identifier_value(device, "device-")
    {
        return None;
    }
    Some(HostProfile {
        namespace_id: Identifier::new(namespace).ok()?,
        device_id: Identifier::new(device).ok()?,
    })
}

fn valid_profile_identifier(identifier: &Identifier, prefix: &str) -> bool {
    valid_profile_identifier_value(identifier.as_str(), prefix)
}

fn valid_profile_identifier_value(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|suffix| {
        suffix.len() == 32
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn identity_generation_failed() -> DesktopError {
    DesktopError::without_source(
        DesktopErrorCode::Runtime,
        DesktopErrorReason::IdentityGenerationFailed,
        true,
    )
}

#[cfg(unix)]
fn set_owner_only_file(file: &File) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    file.set_permissions(fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_owner_only_file(_file: &File) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn sync_parent(path: &std::path::Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent(_path: &std::path::Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use radishmemory_application::{ApplicationIdentifierKind, Timestamp};

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(std::path::PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "radishmemory-desktop-profile-{}-{label}-{sequence}",
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

    #[derive(Default)]
    struct TestRuntime(u64);

    impl ApplicationRuntime for TestRuntime {
        type Error = io::Error;

        fn next_identifier(
            &mut self,
            kind: ApplicationIdentifierKind,
        ) -> Result<Identifier, Self::Error> {
            self.0 += 1;
            let prefix = match kind {
                ApplicationIdentifierKind::Namespace => "namespace-",
                ApplicationIdentifierKind::Device => "device-",
                _ => "other-",
            };
            Identifier::new(format!("{prefix}{:032x}", self.0))
                .map_err(|_| io::Error::other("invalid test identifier"))
        }

        fn now(&mut self) -> Result<Timestamp, Self::Error> {
            Timestamp::parse("2026-08-30T00:00:00Z")
                .map_err(|_| io::Error::other("invalid test timestamp"))
        }
    }

    #[test]
    fn profile_is_atomically_created_and_stable_across_reopen() {
        let directory = TestDirectory::new("stable");
        let paths = ApplicationPaths::from_data_directory(&directory.0).unwrap();
        let first = load_or_create_host_profile(&paths, &mut TestRuntime::default()).unwrap();
        let second = load_or_create_host_profile(&paths, &mut TestRuntime(99)).unwrap();
        assert_eq!(first, second);
        let text = fs::read_to_string(paths.profile_path()).unwrap();
        assert!(text.starts_with("contract_id=radishmemory.phase1-host-profile/1\n"));
        assert!(!text.contains(directory.0.to_str().unwrap()));
        assert!(!format!("{first:?}").contains(first.namespace_id().as_str()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                fs::metadata(paths.profile_path())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn existing_database_without_profile_fails_closed() {
        let directory = TestDirectory::new("missing");
        let paths = ApplicationPaths::from_data_directory(&directory.0).unwrap();
        fs::write(paths.database_path(), b"synthetic-database-marker").unwrap();
        let error = load_or_create_host_profile(&paths, &mut TestRuntime::default()).unwrap_err();
        assert_eq!(
            error.reason(),
            DesktopErrorReason::ProfileMissingForExistingDatabase
        );
        assert!(!paths.profile_path().exists());
    }

    #[test]
    fn malformed_profile_is_not_replaced_or_echoed() {
        let directory = TestDirectory::new("invalid");
        let paths = ApplicationPaths::from_data_directory(&directory.0).unwrap();
        let marker = "synthetic-secret-profile-marker";
        fs::write(paths.profile_path(), marker).unwrap();
        let error = load_or_create_host_profile(&paths, &mut TestRuntime::default()).unwrap_err();
        assert_eq!(error.reason(), DesktopErrorReason::ProfileInvalid);
        assert!(!format!("{error:?} {error}").contains(marker));
        assert_eq!(fs::read_to_string(paths.profile_path()).unwrap(), marker);
    }
}
