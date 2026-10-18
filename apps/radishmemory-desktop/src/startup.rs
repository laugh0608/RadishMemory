//! Explicit native acceptance location. Parsing never creates files or contacts a provider.
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::{ApplicationPaths, DesktopError, DesktopErrorCode, DesktopErrorReason, HostProfile};

#[derive(Default)]
pub(crate) enum Startup {
    #[default]
    Default,
    Acceptance(AcceptanceLocation),
}

pub(crate) struct AcceptanceLocation {
    root: PathBuf,
    namespace: String,
    device: String,
    pub(crate) trace: Option<radishmemory_source_vault::key_read_observation::TraceFile>,
}

impl Startup {
    pub(crate) fn parse(
        arguments: impl IntoIterator<Item = OsString>,
    ) -> Result<Self, DesktopError> {
        let mut arguments = arguments.into_iter().peekable();
        if arguments.peek().is_none() {
            return Ok(Self::Default);
        }
        let mut root = None;
        let mut namespace = None;
        let mut device = None;
        let mut trace = None;
        while let Some(flag) = arguments.next() {
            let value = arguments.next().ok_or_else(invalid_arguments)?;
            match flag.to_str() {
                Some("--acceptance-root") if root.is_none() => root = Some(PathBuf::from(value)),
                Some("--acceptance-namespace") if namespace.is_none() => {
                    namespace = Some(value.into_string().map_err(|_| invalid_arguments())?);
                }
                Some("--acceptance-device") if device.is_none() => {
                    device = Some(value.into_string().map_err(|_| invalid_arguments())?);
                }
                Some("--acceptance-key-read-trace")
                    if trace.is_none() && cfg!(target_os = "macos") =>
                {
                    trace = Some(PathBuf::from(value));
                }
                _ => return Err(invalid_arguments()),
            }
        }
        let mut location = AcceptanceLocation {
            root: root.ok_or_else(invalid_arguments)?,
            namespace: namespace.ok_or_else(invalid_arguments)?,
            device: device.ok_or_else(invalid_arguments)?,
            trace: None,
        };
        radishmemory_source_vault::KeySlot::new(&location.namespace, &location.device)
            .map_err(|_| invalid_arguments())?;
        location.load_profile()?;
        if let Some(path) = trace {
            let default = ApplicationPaths::resolve()?;
            location.trace = Some(
                radishmemory_source_vault::key_read_observation::TraceFile::validate(
                    &path,
                    &[&location.root, default.data_directory()],
                )
                .map_err(|_| observation_invalid())?,
            );
            location.load_profile()?;
        }
        Ok(Self::Acceptance(location))
    }

    pub(crate) fn is_acceptance(&self) -> bool {
        matches!(self, Self::Acceptance(_))
    }
}

impl AcceptanceLocation {
    pub(crate) fn paths(&self) -> Result<ApplicationPaths, DesktopError> {
        ApplicationPaths::from_data_directory(&self.root)
    }

    pub(crate) fn load_profile(&self) -> Result<HostProfile, DesktopError> {
        // Recheck before every worker command, including retry and failed-open maintenance.
        // Never create or replace a profile for this explicitly approved slot.
        validate_root(&self.root, ApplicationPaths::resolve()?.data_directory())?;
        let paths = self.paths()?;
        if !paths.database_exists()? && self.trace.is_some() {
            return Err(observation_invalid());
        }
        let profile = crate::profile::load_profile(&paths)?;
        if profile.namespace_id().as_str() != self.namespace
            || profile.device_id().as_str() != self.device
        {
            return Err(DesktopError::without_source(
                DesktopErrorCode::HostProfile,
                DesktopErrorReason::AcceptanceProfileMismatch,
                false,
            ));
        }
        Ok(profile)
    }
}

fn validate_root(root: &Path, default: &Path) -> Result<(), DesktopError> {
    if !root.is_absolute()
        || root
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        || root.starts_with(default)
        || default.starts_with(root)
    {
        return Err(invalid_root());
    }
    for ancestor in root.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).map_err(|_| invalid_root())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(invalid_root());
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(root)
            .map_err(|_| invalid_root())?
            .permissions()
            .mode()
            & 0o077
            != 0
        {
            return Err(invalid_root());
        }
    }
    // Resolve the nearest existing ancestor even if the default library does not yet exist.
    // This prevents a home-directory alias from disguising an overlapping acceptance root.
    let mut resolved_default = None;
    for ancestor in default.ancestors() {
        match fs::canonicalize(ancestor) {
            Ok(path) => {
                resolved_default =
                    Some(path.join(default.strip_prefix(ancestor).map_err(|_| invalid_root())?));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(invalid_root()),
        }
    }
    let default = resolved_default.ok_or_else(invalid_root)?;
    let root = fs::canonicalize(root).map_err(|_| invalid_root())?;
    if root.starts_with(&default) || default.starts_with(root) {
        return Err(invalid_root());
    }
    Ok(())
}

fn invalid_arguments() -> DesktopError {
    DesktopError::without_source(
        DesktopErrorCode::Runtime,
        DesktopErrorReason::StartupArgumentsInvalid,
        false,
    )
}

fn invalid_root() -> DesktopError {
    DesktopError::without_source(
        DesktopErrorCode::ApplicationDirectory,
        DesktopErrorReason::DataDirectoryInvalid,
        false,
    )
}

#[cfg(test)]
pub(crate) mod tests;

fn observation_invalid() -> DesktopError {
    DesktopError::without_source(
        DesktopErrorCode::Runtime,
        DesktopErrorReason::KeyReadObservationInvalid,
        false,
    )
}
