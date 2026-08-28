//! Local filesystem boundary for the first Phase 1 text / Markdown entry.
//!
//! This crate validates one explicitly selected file, returns a redacted exact-byte
//! snapshot, and publishes one verified managed source to an explicitly allowed
//! destination without overwriting. It does not own canonical persistence, SQLite,
//! deletion, UI selection, or a production Capture Gateway.

mod error;

use std::fmt;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use radishmemory_core::{
    CoreError, DeletionState, Digest, DigestProfile, Governance, Identifier, MediaType,
    NonEmptyText, ProducerRef, SourceArtifact, SourceArtifactParams, SourceCapture,
    SourceCaptureResult, SourceFragment, SourceFragmentParams, SourceKind, SourceOriginKind,
    Timestamp, Version, compute_exact_bytes_digest,
};

pub use error::{FileEntryError, FileEntryErrorCode, FileEntryErrorReason};

pub const FILE_ENTRY_CONTRACT_ID: &str = "radishmemory.phase1-file-entry/1";
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

static NEXT_EXPORT_TEMP: AtomicU64 = AtomicU64::new(0);
const EXPORT_TEMP_ATTEMPTS: usize = 16;

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

/// One user-selected export target plus the explicit roots allowed for this write.
pub struct FileExportRequest {
    target_path: PathBuf,
    allowed_roots: Vec<PathBuf>,
}

impl FileExportRequest {
    pub fn new(
        target_path: impl Into<PathBuf>,
        allowed_roots: Vec<PathBuf>,
    ) -> Result<Self, FileEntryError> {
        let target_path = target_path.into();
        if target_path.as_os_str().is_empty()
            || !target_path.is_absolute()
            || allowed_roots.is_empty()
            || path_contains_nul(&target_path)
            || allowed_roots.iter().any(|root| path_contains_nul(root))
        {
            return Err(FileEntryError::destination_not_allowed());
        }
        Ok(Self {
            target_path,
            allowed_roots,
        })
    }
}

impl fmt::Debug for FileExportRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileExportRequest")
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

/// Path-free proof that one managed source was published byte-for-byte.
pub struct FileExportReceipt {
    namespace_id: Identifier,
    source_id: Identifier,
    lineage_id: Identifier,
    version: Version,
    content_digest: Digest,
    content_length: u64,
    media_type: MediaType,
}

impl FileExportReceipt {
    fn from_source(source: &SourceArtifact) -> Self {
        let params = source.params();
        Self {
            namespace_id: params.namespace_id.clone(),
            source_id: params.source_id.clone(),
            lineage_id: params.lineage_id.clone(),
            version: params.version,
            content_digest: params.content_digest.clone(),
            content_length: params.content_length,
            media_type: params.media_type,
        }
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

impl fmt::Debug for FileExportReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileExportReceipt")
            .field("contract_id", &FILE_ENTRY_CONTRACT_ID)
            .field("namespace_id", &self.namespace_id)
            .field("source_id", &self.source_id)
            .field("lineage_id", &self.lineage_id)
            .field("version", &self.version)
            .field("content_length", &self.content_length)
            .field("media_type", &self.media_type)
            .finish_non_exhaustive()
    }
}

/// Publishes a caller-selected, already loaded canonical source without overwriting a target.
///
/// The caller remains responsible for resolving the exact namespace and source ID through the
/// Source Vault. This boundary revalidates the managed bytes and active deletion state before it
/// performs any filesystem write.
pub fn export_managed_source(
    source: &SourceArtifact,
    request: &FileExportRequest,
) -> Result<FileExportReceipt, FileEntryError> {
    export_managed_source_with_operations(source, request, write_export_bytes, |_| {})
}

fn export_managed_source_with_operations<WriteTemporary, BeforePublish>(
    source: &SourceArtifact,
    request: &FileExportRequest,
    write_temporary: WriteTemporary,
    before_publish: BeforePublish,
) -> Result<FileExportReceipt, FileEntryError>
where
    WriteTemporary: FnOnce(&mut File, &[u8]) -> std::io::Result<()>,
    BeforePublish: FnOnce(&Path),
{
    let params = source.params();
    if params.governance.deletion_state() != DeletionState::Active {
        return Err(FileEntryError::canonical_conflict());
    }
    validate_size(params.content_length)?;
    let expected_bytes = params.content.as_str().as_bytes();
    if params.content_digest.profile() != DigestProfile::ExactBytesV1
        || usize::try_from(params.content_length) != Ok(expected_bytes.len())
        || compute_exact_bytes_digest(expected_bytes) != params.content_digest
    {
        return Err(FileEntryError::integrity_mismatch());
    }

    let destination = resolve_export_destination(request)?;
    let (mut temporary, temporary_path) = create_export_temporary(&destination.canonical_parent)?;
    let write_result = write_temporary(&mut temporary, expected_bytes)
        .and_then(|()| temporary.flush())
        .and_then(|()| temporary.sync_all())
        .map_err(FileEntryError::io);
    let written_observation = temporary
        .metadata()
        .ok()
        .map(|metadata| FileObservation::from_metadata(&metadata));
    drop(temporary);
    if let Err(error) = write_result {
        return cleanup_temporary_after_failure(
            &temporary_path,
            written_observation.as_ref(),
            error,
        );
    }

    let (verified_temporary, temporary_observation) =
        match open_and_verify_exact_file(&temporary_path, expected_bytes, &params.content_digest) {
            Ok(verified) => verified,
            Err(error) => {
                return cleanup_temporary_after_failure(
                    &temporary_path,
                    written_observation.as_ref(),
                    error,
                );
            }
        };
    if let Err(error) = verify_export_destination_stable(&destination) {
        drop(verified_temporary);
        return cleanup_temporary_after_failure(
            &temporary_path,
            Some(&temporary_observation),
            error,
        );
    }
    if let Err(error) = verify_path_matches_open_file(&temporary_path, &temporary_observation) {
        drop(verified_temporary);
        return cleanup_temporary_after_failure(
            &temporary_path,
            Some(&temporary_observation),
            error,
        );
    }
    before_publish(&destination.target_path);

    let publish_result =
        fs::hard_link(&temporary_path, &destination.target_path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                FileEntryError::destination_exists()
            } else {
                FileEntryError::io(error)
            }
        });
    if let Err(error) = publish_result {
        drop(verified_temporary);
        return cleanup_temporary_after_failure(
            &temporary_path,
            Some(&temporary_observation),
            error,
        );
    }

    let published_temporary_observation = verified_temporary
        .metadata()
        .map(|metadata| FileObservation::from_metadata(&metadata))
        .map_err(FileEntryError::io);
    let published_temporary_observation = match published_temporary_observation {
        Ok(observation) => observation,
        Err(error) => {
            drop(verified_temporary);
            return cleanup_temporary_after_failure(
                &temporary_path,
                Some(&temporary_observation),
                error,
            );
        }
    };

    let target_verification = open_and_verify_exact_file(
        &destination.target_path,
        expected_bytes,
        &params.content_digest,
    )
    .and_then(|(target, target_observation)| {
        drop(target);
        if target_observation == published_temporary_observation {
            Ok(())
        } else {
            Err(FileEntryError::integrity_mismatch())
        }
    });
    drop(verified_temporary);
    if let Err(error) = target_verification {
        return cleanup_temporary_after_failure(
            &temporary_path,
            Some(&published_temporary_observation),
            error,
        );
    }
    remove_owned_temporary(&temporary_path, &published_temporary_observation)?;
    Ok(FileExportReceipt::from_source(source))
}

fn write_export_bytes(file: &mut File, bytes: &[u8]) -> std::io::Result<()> {
    file.write_all(bytes)
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

struct ResolvedExportDestination {
    root_path: PathBuf,
    relative_parent: PathBuf,
    requested_parent: PathBuf,
    canonical_root: PathBuf,
    canonical_parent: PathBuf,
    target_path: PathBuf,
}

fn resolve_export_destination(
    request: &FileExportRequest,
) -> Result<ResolvedExportDestination, FileEntryError> {
    let selected_target = normalize_absolute(&request.target_path)
        .map_err(|_| FileEntryError::destination_not_allowed())?;
    for allowed_root in &request.allowed_roots {
        let Ok(root_path) = normalize_absolute(allowed_root) else {
            continue;
        };
        if root_path.parent().is_none() {
            continue;
        }
        let Ok(relative_target) = selected_target.strip_prefix(&root_path) else {
            continue;
        };
        if relative_target.as_os_str().is_empty()
            || relative_target
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            continue;
        }
        let Some(target_name) = relative_target.file_name() else {
            continue;
        };
        let relative_parent = relative_target.parent().unwrap_or_else(|| Path::new(""));
        check_export_parent_components(&root_path, relative_parent)?;

        let canonical_root = match fs::canonicalize(&root_path) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(FileEntryError::io(error)),
        };
        let root_metadata = fs::metadata(&canonical_root).map_err(FileEntryError::io)?;
        if !root_metadata.is_dir() || canonical_root.parent().is_none() {
            continue;
        }
        let requested_parent = selected_target
            .parent()
            .ok_or_else(FileEntryError::destination_not_allowed)?
            .to_path_buf();
        let canonical_parent = match fs::canonicalize(&requested_parent) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(FileEntryError::destination_not_allowed());
            }
            Err(error) => return Err(FileEntryError::io(error)),
        };
        let parent_metadata = fs::metadata(&canonical_parent).map_err(FileEntryError::io)?;
        if !parent_metadata.is_dir()
            || (canonical_parent != canonical_root
                && !canonical_parent.starts_with(&canonical_root))
        {
            return Err(FileEntryError::destination_not_allowed());
        }
        let target_path = canonical_parent.join(target_name);
        reject_existing_export_target(&target_path)?;
        return Ok(ResolvedExportDestination {
            root_path,
            relative_parent: relative_parent.to_path_buf(),
            requested_parent,
            canonical_root,
            canonical_parent,
            target_path,
        });
    }
    Err(FileEntryError::destination_not_allowed())
}

fn check_export_parent_components(root: &Path, relative: &Path) -> Result<(), FileEntryError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(FileEntryError::destination_not_allowed());
        }
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(FileEntryError::destination_not_allowed());
            }
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => return Err(FileEntryError::destination_not_allowed()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(FileEntryError::destination_not_allowed());
            }
            Err(error) => return Err(FileEntryError::io(error)),
        }
    }
    Ok(())
}

fn reject_existing_export_target(target: &Path) -> Result<(), FileEntryError> {
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(FileEntryError::destination_not_allowed())
        }
        Ok(_) => Err(FileEntryError::destination_exists()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(FileEntryError::io(error)),
    }
}

fn verify_export_destination_stable(
    destination: &ResolvedExportDestination,
) -> Result<(), FileEntryError> {
    check_export_parent_components(&destination.root_path, &destination.relative_parent)?;
    let canonical_root = fs::canonicalize(&destination.root_path)
        .map_err(|_| FileEntryError::destination_not_allowed())?;
    let canonical_parent = fs::canonicalize(&destination.requested_parent)
        .map_err(|_| FileEntryError::destination_not_allowed())?;
    if canonical_root != destination.canonical_root
        || canonical_parent != destination.canonical_parent
        || (canonical_parent != canonical_root && !canonical_parent.starts_with(&canonical_root))
    {
        return Err(FileEntryError::destination_not_allowed());
    }
    reject_existing_export_target(&destination.target_path)
}

fn create_export_temporary(parent: &Path) -> Result<(File, PathBuf), FileEntryError> {
    let mut last_collision = None;
    for _ in 0..EXPORT_TEMP_ATTEMPTS {
        let sequence = NEXT_EXPORT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".radishmemory-export-{}-{sequence}.tmp",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                last_collision = Some(error);
            }
            Err(error) => return Err(FileEntryError::io(error)),
        }
    }
    Err(FileEntryError::io(last_collision.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "temporary export name collision",
        )
    })))
}

fn open_and_verify_exact_file(
    path: &Path,
    expected_bytes: &[u8],
    expected_digest: &Digest,
) -> Result<(File, FileObservation), FileEntryError> {
    let path_metadata = fs::symlink_metadata(path).map_err(FileEntryError::io)?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(FileEntryError::integrity_mismatch());
    }
    let mut file = File::open(path).map_err(FileEntryError::io)?;
    let before_metadata = file.metadata().map_err(FileEntryError::io)?;
    if !before_metadata.is_file()
        || usize::try_from(before_metadata.len()) != Ok(expected_bytes.len())
    {
        return Err(FileEntryError::integrity_mismatch());
    }
    let before = FileObservation::from_metadata(&before_metadata);
    let mut actual_bytes = Vec::with_capacity(expected_bytes.len());
    (&mut file)
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut actual_bytes)
        .map_err(FileEntryError::io)?;
    let after_metadata = file.metadata().map_err(FileEntryError::io)?;
    let after = FileObservation::from_metadata(&after_metadata);
    if before != after
        || actual_bytes != expected_bytes
        || compute_exact_bytes_digest(&actual_bytes) != *expected_digest
    {
        return Err(FileEntryError::integrity_mismatch());
    }
    verify_path_matches_open_file(path, &after)?;
    Ok((file, after))
}

fn verify_path_matches_open_file(
    path: &Path,
    expected: &FileObservation,
) -> Result<(), FileEntryError> {
    let metadata = fs::symlink_metadata(path).map_err(FileEntryError::io)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || FileObservation::from_metadata(&metadata) != *expected
    {
        return Err(FileEntryError::integrity_mismatch());
    }
    Ok(())
}

fn cleanup_temporary_after_failure<T>(
    path: &Path,
    expected: Option<&FileObservation>,
    primary: FileEntryError,
) -> Result<T, FileEntryError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(primary),
        Err(error) => Err(FileEntryError::io(error)),
        Ok(_) if expected.is_none() => Err(FileEntryError::integrity_mismatch()),
        Ok(metadata)
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || expected
                    .is_some_and(|value| FileObservation::from_metadata(&metadata) != *value) =>
        {
            Err(FileEntryError::integrity_mismatch())
        }
        Ok(_) => match fs::remove_file(path) {
            Ok(()) => Err(primary),
            Err(error) => Err(FileEntryError::io(error)),
        },
    }
}

fn remove_owned_temporary(path: &Path, expected: &FileObservation) -> Result<(), FileEntryError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(FileEntryError::io(error)),
        Ok(metadata)
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || FileObservation::from_metadata(&metadata) != *expected =>
        {
            Err(FileEntryError::integrity_mismatch())
        }
        Ok(_) => fs::remove_file(path).map_err(FileEntryError::io),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use radishmemory_core::{
        EgressPolicy, ProducerType, RetentionMode, RetentionRule, Sensitivity,
    };

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "radishmemory-export-unit-{label}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("synthetic export directory must be created");
            Self(path)
        }

        fn target(&self) -> PathBuf {
            self.0.join("exported.txt")
        }

        fn request(&self) -> FileExportRequest {
            FileExportRequest::new(self.target(), vec![self.0.clone()])
                .expect("synthetic export request must be valid")
        }

        fn assert_no_temporary(&self) {
            let names = fs::read_dir(&self.0)
                .expect("synthetic directory must be readable")
                .map(|entry| {
                    entry
                        .expect("synthetic directory entry must be readable")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect::<Vec<_>>();
            assert!(
                names
                    .iter()
                    .all(|name| !name.starts_with(".radishmemory-export-"))
            );
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("synthetic export directory must be removed");
        }
    }

    fn identifier(value: &str) -> Identifier {
        Identifier::new(value).expect("synthetic identifier must be valid")
    }

    fn nonempty(value: &str) -> NonEmptyText {
        NonEmptyText::new(value).expect("synthetic text must be nonempty")
    }

    fn synthetic_source(deletion_state: DeletionState) -> SourceArtifact {
        let content = nonempty("Synthetic managed export bytes.\r\n");
        SourceArtifact::new(SourceArtifactParams {
            source_id: identifier("source-export-1"),
            lineage_id: identifier("lineage-export-1"),
            version: Version::new(1).expect("synthetic version must be valid"),
            namespace_id: identifier("namespace-export-1"),
            source_kind: SourceKind::Text,
            media_type: MediaType::TextPlain,
            content_length: content.utf8_len() as u64,
            content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
            content,
            title: None,
            origin_kind: SourceOriginKind::ExplicitUserInput,
            origin_ref: Some(nonempty("origin-binding-export-1")),
            observed_at: Timestamp::parse("2026-08-28T10:00:00Z")
                .expect("synthetic timestamp must be valid"),
            captured_at: Timestamp::parse("2026-08-28T10:00:01Z")
                .expect("synthetic timestamp must be valid"),
            supersedes_source_ids: vec![],
            governance: Governance::new(
                Sensitivity::Personal,
                EgressPolicy::LocalOnly,
                RetentionRule::new(RetentionMode::UntilDeleted, None, None)
                    .expect("synthetic retention must be valid"),
                deletion_state,
                identifier("policy-local-only"),
            )
            .expect("synthetic governance must be valid"),
            producer: ProducerRef::new(
                ProducerType::Parser,
                identifier("file-entry-parser"),
                nonempty("1"),
            ),
            created_at: Timestamp::parse("2026-08-28T10:00:01Z")
                .expect("synthetic timestamp must be valid"),
        })
        .expect("synthetic source must be valid")
    }

    #[test]
    fn temporary_write_failure_is_explicit_and_cleans_task_file() {
        let directory = TestDirectory::new("write-failure");
        let source = synthetic_source(DeletionState::Active);
        let error = export_managed_source_with_operations(
            &source,
            &directory.request(),
            |_, _| Err(std::io::Error::other("synthetic write failure")),
            |_| {},
        )
        .expect_err("temporary write failure must reject export");

        assert_eq!(error.reason(), FileEntryErrorReason::IoFailure);
        assert!(!directory.target().exists());
        directory.assert_no_temporary();
    }

    #[test]
    fn publication_race_never_overwrites_target_and_cleans_task_file() {
        let directory = TestDirectory::new("publish-race");
        let source = synthetic_source(DeletionState::Active);
        let error = export_managed_source_with_operations(
            &source,
            &directory.request(),
            write_export_bytes,
            |target| fs::write(target, b"concurrent owner bytes").expect("race target must exist"),
        )
        .expect_err("concurrent target must reject publication");

        assert_eq!(error.reason(), FileEntryErrorReason::DestinationExists);
        assert_eq!(
            fs::read(directory.target()).expect("race target must remain readable"),
            b"concurrent owner bytes"
        );
        directory.assert_no_temporary();
    }

    #[test]
    fn non_active_source_is_rejected_before_any_filesystem_write() {
        let directory = TestDirectory::new("non-active");
        let source = synthetic_source(DeletionState::Pending);
        let error = export_managed_source(&source, &directory.request())
            .expect_err("pending source must not export");

        assert_eq!(error.reason(), FileEntryErrorReason::CanonicalConflict);
        assert!(!directory.target().exists());
        directory.assert_no_temporary();
    }

    #[test]
    fn destination_outside_allowed_root_is_rejected_before_write() {
        let allowed = TestDirectory::new("allowed-root");
        let outside = TestDirectory::new("outside-root");
        let source = synthetic_source(DeletionState::Active);
        let request = FileExportRequest::new(outside.target(), vec![allowed.0.clone()])
            .expect("absolute export request shape must be valid");
        let error = export_managed_source(&source, &request)
            .expect_err("outside destination must not export");

        assert_eq!(error.reason(), FileEntryErrorReason::DestinationNotAllowed);
        assert!(!outside.target().exists());
        allowed.assert_no_temporary();
        outside.assert_no_temporary();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_parent_below_allowed_root_is_rejected_before_write() {
        use std::os::unix::fs::symlink;

        let allowed = TestDirectory::new("parent-symlink-allowed");
        let outside = TestDirectory::new("parent-symlink-outside");
        let linked_parent = allowed.0.join("linked-parent");
        symlink(&outside.0, &linked_parent).expect("synthetic parent symlink must be created");
        let target = linked_parent.join("exported.txt");
        let source = synthetic_source(DeletionState::Active);
        let request = FileExportRequest::new(&target, vec![allowed.0.clone()])
            .expect("absolute export request must be valid");
        let error =
            export_managed_source(&source, &request).expect_err("symlink parent must not export");

        assert_eq!(error.reason(), FileEntryErrorReason::DestinationNotAllowed);
        assert!(!outside.target().exists());
        allowed.assert_no_temporary();
        outside.assert_no_temporary();
    }
}
