//! Local filesystem boundary for the first Phase 1 text / Markdown entry.
//!
//! This crate validates one explicitly selected file and returns a redacted,
//! exact-byte snapshot. It does not own canonical persistence, SQLite, export,
//! deletion, UI selection, or a production Capture Gateway.

mod error;

use std::fmt;
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use radishmemory_core::{
    CoreError, Digest, DigestProfile, Governance, Identifier, MediaType, NonEmptyText, ProducerRef,
    SourceArtifact, SourceArtifactParams, SourceCapture, SourceCaptureResult, SourceFragment,
    SourceFragmentParams, SourceKind, SourceOriginKind, Timestamp, Version,
    compute_exact_bytes_digest,
};

pub use error::{FileEntryError, FileEntryErrorCode, FileEntryErrorReason};

pub const FILE_ENTRY_CONTRACT_ID: &str = "radishmemory.phase1-file-entry/1";
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// One user-selected path plus the explicit roots allowed for this read.
pub struct FileReadRequest {
    selected_path: PathBuf,
    allowed_roots: Vec<PathBuf>,
}

impl FileReadRequest {
    pub fn new(
        selected_path: impl Into<PathBuf>,
        allowed_roots: Vec<PathBuf>,
    ) -> Result<Self, FileEntryError> {
        let selected_path = selected_path.into();
        if selected_path.as_os_str().is_empty()
            || allowed_roots.is_empty()
            || path_contains_nul(&selected_path)
            || allowed_roots.iter().any(|root| path_contains_nul(root))
        {
            return Err(FileEntryError::path_not_allowed());
        }
        Ok(Self {
            selected_path,
            allowed_roots,
        })
    }
}

impl fmt::Debug for FileReadRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileReadRequest")
            .field("allowed_root_count", &self.allowed_roots.len())
            .finish_non_exhaustive()
    }
}

/// Exact validated bytes and canonical source metadata, without a path.
pub struct ValidatedFileSnapshot {
    source_kind: SourceKind,
    media_type: MediaType,
    content: NonEmptyText,
    content_length: u64,
    content_digest: Digest,
    title: Option<NonEmptyText>,
}

impl ValidatedFileSnapshot {
    #[must_use]
    pub const fn source_kind(&self) -> SourceKind {
        self.source_kind
    }

    #[must_use]
    pub const fn media_type(&self) -> MediaType {
        self.media_type
    }

    #[must_use]
    pub const fn content(&self) -> &NonEmptyText {
        &self.content
    }

    #[must_use]
    pub const fn content_length(&self) -> u64 {
        self.content_length
    }

    #[must_use]
    pub const fn content_digest(&self) -> &Digest {
        &self.content_digest
    }

    #[must_use]
    pub const fn title(&self) -> Option<&NonEmptyText> {
        self.title.as_ref()
    }
}

impl fmt::Debug for ValidatedFileSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedFileSnapshot")
            .field("source_kind", &self.source_kind)
            .field("media_type", &self.media_type)
            .field("content_length", &self.content_length)
            .field("digest_profile", &self.content_digest.profile())
            .field("has_title", &self.title.is_some())
            .finish()
    }
}

pub use radishmemory_core::SourceCaptureOutcome as FileCaptureOutcome;

/// Caller-owned IDs, governance, and time facts used to bind one snapshot to canonical objects.
pub struct FileCapturePlan {
    pub namespace_id: Identifier,
    pub origin_binding_id: Identifier,
    pub source_id: Identifier,
    pub lineage_id: Identifier,
    pub version: Version,
    pub supersedes_source_ids: Vec<Identifier>,
    pub fragment_id: Identifier,
    pub observed_at: Timestamp,
    pub captured_at: Timestamp,
    pub governance: Governance,
    pub source_producer: ProducerRef,
    pub segmenter: ProducerRef,
}

impl fmt::Debug for FileCapturePlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileCapturePlan")
            .field("namespace_id", &self.namespace_id)
            .field("source_id", &self.source_id)
            .field("lineage_id", &self.lineage_id)
            .field("version", &self.version)
            .field("supersedes_count", &self.supersedes_source_ids.len())
            .field("fragment_id", &self.fragment_id)
            .finish_non_exhaustive()
    }
}

/// Minimal path-free application receipt frozen before persistence is added.
pub struct FileCaptureReceipt {
    namespace_id: Identifier,
    source_id: Identifier,
    lineage_id: Identifier,
    version: Version,
    content_digest: Digest,
    content_length: u64,
    media_type: MediaType,
    outcome: FileCaptureOutcome,
}

impl FileCaptureReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        namespace_id: Identifier,
        source_id: Identifier,
        lineage_id: Identifier,
        version: Version,
        content_digest: Digest,
        content_length: u64,
        media_type: MediaType,
        outcome: FileCaptureOutcome,
    ) -> Result<Self, FileEntryError> {
        if content_length == 0 {
            return Err(FileEntryError::empty_file());
        }
        if content_length > MAX_FILE_BYTES {
            return Err(FileEntryError::file_too_large());
        }
        if content_digest.profile() != DigestProfile::ExactBytesV1 {
            return Err(FileEntryError::integrity_mismatch());
        }
        Ok(Self {
            namespace_id,
            source_id,
            lineage_id,
            version,
            content_digest,
            content_length,
            media_type,
            outcome,
        })
    }

    pub fn from_capture_result(result: &SourceCaptureResult) -> Result<Self, FileEntryError> {
        Self::new(
            result.namespace_id().clone(),
            result.source_id().clone(),
            result.lineage_id().clone(),
            result.version(),
            result.content_digest().clone(),
            result.content_length(),
            result.media_type(),
            result.outcome(),
        )
    }

    #[must_use]
    pub const fn contract_id(&self) -> &'static str {
        FILE_ENTRY_CONTRACT_ID
    }

    #[must_use]
    pub const fn namespace_id(&self) -> &Identifier {
        &self.namespace_id
    }

    #[must_use]
    pub const fn source_id(&self) -> &Identifier {
        &self.source_id
    }

    #[must_use]
    pub const fn lineage_id(&self) -> &Identifier {
        &self.lineage_id
    }

    #[must_use]
    pub const fn version(&self) -> Version {
        self.version
    }

    #[must_use]
    pub const fn content_digest(&self) -> &Digest {
        &self.content_digest
    }

    #[must_use]
    pub const fn content_length(&self) -> u64 {
        self.content_length
    }

    #[must_use]
    pub const fn media_type(&self) -> MediaType {
        self.media_type
    }

    #[must_use]
    pub const fn outcome(&self) -> FileCaptureOutcome {
        self.outcome
    }
}

/// Maps one path-free snapshot to a complete, single-fragment canonical capture candidate.
pub fn build_source_capture(
    snapshot: ValidatedFileSnapshot,
    plan: FileCapturePlan,
) -> Result<SourceCapture, CoreError> {
    let origin_ref = NonEmptyText::new(plan.origin_binding_id.as_str().to_owned())?;
    let fragment_content = snapshot.content.clone();
    let source = SourceArtifact::new(SourceArtifactParams {
        source_id: plan.source_id,
        lineage_id: plan.lineage_id,
        version: plan.version,
        namespace_id: plan.namespace_id.clone(),
        source_kind: snapshot.source_kind,
        media_type: snapshot.media_type,
        content: snapshot.content,
        content_length: snapshot.content_length,
        content_digest: snapshot.content_digest,
        title: snapshot.title,
        origin_kind: SourceOriginKind::ExplicitUserInput,
        origin_ref: Some(origin_ref),
        observed_at: plan.observed_at,
        captured_at: plan.captured_at.clone(),
        supersedes_source_ids: plan.supersedes_source_ids,
        governance: plan.governance.clone(),
        producer: plan.source_producer,
        created_at: plan.captured_at.clone(),
    })?;
    let fragment = SourceFragment::new(SourceFragmentParams {
        fragment_id: plan.fragment_id,
        namespace_id: plan.namespace_id,
        source_id: source.params().source_id.clone(),
        ordinal: 0,
        byte_start: 0,
        byte_end: snapshot.content_length,
        heading_path: None,
        content_digest: compute_exact_bytes_digest(fragment_content.as_str().as_bytes()),
        content: fragment_content,
        segmenter: plan.segmenter,
        governance: plan.governance,
        created_at: plan.captured_at,
    })?;
    SourceCapture::new(plan.origin_binding_id, source, vec![fragment])
}

impl fmt::Debug for FileCaptureReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileCaptureReceipt")
            .field("contract_id", &FILE_ENTRY_CONTRACT_ID)
            .field("namespace_id", &self.namespace_id)
            .field("source_id", &self.source_id)
            .field("lineage_id", &self.lineage_id)
            .field("version", &self.version)
            .field("content_length", &self.content_length)
            .field("media_type", &self.media_type)
            .field("outcome", &self.outcome)
            .finish_non_exhaustive()
    }
}

/// Reads one stable, exact-byte snapshot after all path and content checks.
pub fn read_file_snapshot(
    request: &FileReadRequest,
) -> Result<ValidatedFileSnapshot, FileEntryError> {
    let resolved = resolve_selection(request)?;
    let (source_kind, media_type) = classify_file(&resolved.canonical_path)?;
    let mut file = File::open(&resolved.canonical_path).map_err(FileEntryError::io)?;
    let before_metadata = file.metadata().map_err(FileEntryError::io)?;
    if !before_metadata.is_file() {
        return Err(FileEntryError::not_regular_file());
    }
    validate_size(before_metadata.len())?;
    let before = FileObservation::from_metadata(&before_metadata);
    let path_before = fs::metadata(&resolved.canonical_path).map_err(FileEntryError::io)?;
    if before != FileObservation::from_metadata(&path_before) {
        return Err(FileEntryError::source_changed());
    }

    let capacity = usize::try_from(before_metadata.len()).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity.min(MAX_FILE_BYTES as usize));
    (&mut file)
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(FileEntryError::io)?;
    let content_length =
        u64::try_from(bytes.len()).map_err(|_| FileEntryError::file_too_large())?;
    validate_size(content_length)?;

    let after_metadata = file.metadata().map_err(FileEntryError::io)?;
    let after = FileObservation::from_metadata(&after_metadata);
    if before != after {
        return Err(FileEntryError::source_changed());
    }
    verify_selection_stable(&resolved, &after)?;

    let content = String::from_utf8(bytes).map_err(|_| FileEntryError::invalid_utf8())?;
    if content.as_bytes().contains(&0) {
        return Err(FileEntryError::nul_byte_not_allowed());
    }
    let content_digest = compute_exact_bytes_digest(content.as_bytes());
    let content = NonEmptyText::new(content.into_boxed_str())
        .map_err(|_| FileEntryError::integrity_mismatch())?;
    let title = resolved
        .canonical_path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| NonEmptyText::new(name.to_owned().into_boxed_str()).ok());
    Ok(ValidatedFileSnapshot {
        source_kind,
        media_type,
        content,
        content_length,
        content_digest,
        title,
    })
}

struct ResolvedSelection {
    selected_path: PathBuf,
    root_path: PathBuf,
    relative_path: PathBuf,
    canonical_path: PathBuf,
    canonical_root: PathBuf,
}

fn resolve_selection(request: &FileReadRequest) -> Result<ResolvedSelection, FileEntryError> {
    let selected_path = normalize_absolute(&request.selected_path)?;
    for allowed_root in &request.allowed_roots {
        let Ok(root_path) = normalize_absolute(allowed_root) else {
            continue;
        };
        if root_path.parent().is_none() {
            continue;
        }
        let Ok(relative_path) = selected_path.strip_prefix(&root_path) else {
            continue;
        };
        if relative_path.as_os_str().is_empty() {
            continue;
        }
        let relative_path = relative_path.to_path_buf();
        check_no_symlink_below_root(&root_path, &relative_path)?;
        let canonical_root = match fs::canonicalize(&root_path) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(FileEntryError::io(error)),
        };
        let root_metadata = fs::metadata(&canonical_root).map_err(FileEntryError::io)?;
        if !root_metadata.is_dir() || canonical_root.parent().is_none() {
            continue;
        }
        let canonical_path = match fs::canonicalize(&selected_path) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(FileEntryError::path_not_allowed());
            }
            Err(error) => return Err(FileEntryError::io(error)),
        };
        if canonical_path == canonical_root || !canonical_path.starts_with(&canonical_root) {
            return Err(FileEntryError::path_not_allowed());
        }
        let metadata = fs::metadata(&canonical_path).map_err(FileEntryError::io)?;
        if !metadata.is_file() {
            return Err(FileEntryError::not_regular_file());
        }
        return Ok(ResolvedSelection {
            selected_path,
            root_path,
            relative_path,
            canonical_path,
            canonical_root,
        });
    }
    Err(FileEntryError::path_not_allowed())
}

fn normalize_absolute(path: &Path) -> Result<PathBuf, FileEntryError> {
    if !path.is_absolute() {
        return Err(FileEntryError::path_not_allowed());
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(FileEntryError::path_not_allowed());
                }
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(FileEntryError::path_not_allowed());
    }
    Ok(normalized)
}

fn check_no_symlink_below_root(root: &Path, relative: &Path) -> Result<(), FileEntryError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(FileEntryError::path_not_allowed());
        }
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(FileEntryError::symlink_not_allowed());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(FileEntryError::path_not_allowed());
            }
            Err(error) => return Err(FileEntryError::io(error)),
        }
    }
    Ok(())
}

fn classify_file(path: &Path) -> Result<(SourceKind, MediaType), FileEntryError> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .ok_or_else(FileEntryError::unsupported_file_type)?;
    if extension.eq_ignore_ascii_case("txt") {
        Ok((SourceKind::Text, MediaType::TextPlain))
    } else if extension.eq_ignore_ascii_case("md") {
        Ok((SourceKind::Markdown, MediaType::TextMarkdown))
    } else {
        Err(FileEntryError::unsupported_file_type())
    }
}

fn validate_size(size: u64) -> Result<(), FileEntryError> {
    if size == 0 {
        Err(FileEntryError::empty_file())
    } else if size > MAX_FILE_BYTES {
        Err(FileEntryError::file_too_large())
    } else {
        Ok(())
    }
}

fn verify_selection_stable(
    resolved: &ResolvedSelection,
    expected: &FileObservation,
) -> Result<(), FileEntryError> {
    check_no_symlink_below_root(&resolved.root_path, &resolved.relative_path)
        .map_err(|_| FileEntryError::source_changed())?;
    let canonical_root =
        fs::canonicalize(&resolved.root_path).map_err(|_| FileEntryError::source_changed())?;
    let canonical_path =
        fs::canonicalize(&resolved.selected_path).map_err(|_| FileEntryError::source_changed())?;
    if canonical_root != resolved.canonical_root
        || canonical_path != resolved.canonical_path
        || !canonical_path.starts_with(&canonical_root)
    {
        return Err(FileEntryError::source_changed());
    }
    let metadata = fs::symlink_metadata(&resolved.selected_path)
        .map_err(|_| FileEntryError::source_changed())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(FileEntryError::source_changed());
    }
    let path_observation = FileObservation::from_metadata(&metadata);
    if &path_observation != expected {
        return Err(FileEntryError::source_changed());
    }
    Ok(())
}

fn path_contains_nul(path: &Path) -> bool {
    path.as_os_str().to_string_lossy().contains('\0')
}

#[derive(Eq, PartialEq)]
struct FileObservation {
    len: u64,
    modified: Option<SystemTime>,
    platform: PlatformObservation,
}

impl FileObservation {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            platform: PlatformObservation::from_metadata(metadata),
        }
    }
}

#[cfg(unix)]
#[derive(Eq, PartialEq)]
struct PlatformObservation {
    device: u64,
    inode: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[cfg(unix)]
impl PlatformObservation {
    fn from_metadata(metadata: &Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;

        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

#[cfg(windows)]
#[derive(Eq, PartialEq)]
struct PlatformObservation {
    attributes: u32,
    created: u64,
    last_write: u64,
    file_size: u64,
}

#[cfg(windows)]
impl PlatformObservation {
    fn from_metadata(metadata: &Metadata) -> Self {
        use std::os::windows::fs::MetadataExt;

        Self {
            attributes: metadata.file_attributes(),
            created: metadata.creation_time(),
            last_write: metadata.last_write_time(),
            file_size: metadata.file_size(),
        }
    }
}

#[cfg(not(any(unix, windows)))]
#[derive(Eq, PartialEq)]
struct PlatformObservation;

#[cfg(not(any(unix, windows)))]
impl PlatformObservation {
    const fn from_metadata(_: &Metadata) -> Self {
        Self
    }
}
