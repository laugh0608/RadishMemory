//! Production application boundary for the Phase 1 local library host.
//!
//! This crate composes the canonical core, the explicit local-file boundary,
//! and the SQLite adapter. It does not own a desktop toolkit, platform picker,
//! persistent bookmark, network listener, model, or synchronization runtime.

mod error;

use std::error::Error;
use std::fmt;
use std::path::Path;

use radishmemory_core::{
    ActorRef, ActorType, ComponentStatus, DeleteRequest, DeleteRequestParams,
    DeletionEvidenceParams, DeletionState, DeletionStore, EgressPolicy, EvidenceRef, EvidenceType,
    Governance, LocalDeletionExecution, LocalSearch, LocalSearchHit, LocalSearchRequest,
    ProducerRef, ProducerType, RequestedGuarantee, RetentionMode, RetentionRule,
    SourceCaptureStore, SourceCatalog, SourceCatalogRequest, SourceFragment, SourceVault, Version,
    build_local_purge_targets, compute_deletion_evidence_digest, source_origin_binding_id_is_valid,
};
pub use radishmemory_core::{
    DeletionEvidence, DeletionOverallStatus, Identifier, NonEmptyText, Sensitivity, SourceArtifact,
    SourceLineageSummary, SourceVersionSummary, Timestamp,
};
pub use radishmemory_file_entry::{
    FileCaptureOutcome, FileCaptureReceipt, FileExportReceipt, FileExportRequest, FileReadRequest,
};
use radishmemory_file_entry::{
    FileCapturePlan, build_source_capture, export_managed_source, read_file_snapshot,
};
use radishmemory_sqlite::SqliteDatabase;

pub use error::{
    ApplicationError, ApplicationErrorCode, ApplicationErrorReason, ApplicationOperation,
};

pub const LOCAL_LIBRARY_HOST_CONTRACT_ID: &str = "radishmemory.phase1-local-library-host/1";

/// Identifier roles requested from the production host runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationIdentifierKind {
    Namespace,
    Device,
    OriginBinding,
    Source,
    Lineage,
    Fragment,
    DeleteRequest,
    DeletionEvidence,
}

/// Host-owned nondeterministic capabilities, replaceable by deterministic tests.
pub trait ApplicationRuntime {
    type Error: Error + Send + Sync + 'static;

    fn next_identifier(
        &mut self,
        kind: ApplicationIdentifierKind,
    ) -> Result<Identifier, Self::Error>;

    fn now(&mut self) -> Result<Timestamp, Self::Error>;
}

/// Stable local-library defaults supplied once by the trusted desktop host.
#[derive(Clone)]
pub struct LocalLibraryConfig {
    namespace_id: Identifier,
    governance: Governance,
    source_producer: ProducerRef,
    segmenter: ProducerRef,
    deletion: LocalDeletionConfig,
}

/// Trusted local identity and policy inputs for canonical deletion evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalDeletionConfig {
    requested_by: ActorRef,
    authorization_basis: NonEmptyText,
    device_id: Identifier,
    reason_code: NonEmptyText,
    retention_policy_basis: Identifier,
    verified_by: ProducerRef,
}

impl LocalDeletionConfig {
    #[must_use]
    pub const fn new(
        requested_by: ActorRef,
        authorization_basis: NonEmptyText,
        device_id: Identifier,
        reason_code: NonEmptyText,
        retention_policy_basis: Identifier,
        verified_by: ProducerRef,
    ) -> Self {
        Self {
            requested_by,
            authorization_basis,
            device_id,
            reason_code,
            retention_policy_basis,
            verified_by,
        }
    }
}

impl LocalLibraryConfig {
    /// Builds the frozen single-user, single-device Phase 1 local policy profile.
    pub fn phase1_local(
        namespace_id: Identifier,
        device_id: Identifier,
    ) -> Result<Self, ApplicationError> {
        let operation = ApplicationOperation::OpenLibrary;
        let text = |value: &'static str| {
            NonEmptyText::new(value)
                .map_err(|source| ApplicationError::canonical(operation, source))
        };
        let id = |value: &'static str| {
            Identifier::new(value).map_err(|source| ApplicationError::canonical(operation, source))
        };
        let governance = Governance::new(
            Sensitivity::Personal,
            EgressPolicy::LocalOnly,
            RetentionRule::new(RetentionMode::UntilDeleted, None, None)
                .map_err(|source| ApplicationError::canonical(operation, source))?,
            DeletionState::Active,
            id("policy-phase1-local")?,
        )
        .map_err(|source| ApplicationError::canonical(operation, source))?;
        Self::new(
            namespace_id,
            governance,
            ProducerRef::new(
                ProducerType::System,
                id("producer-phase1-desktop")?,
                text("1.0.0")?,
            ),
            ProducerRef::new(
                ProducerType::Rule,
                id("segmenter-whole-file")?,
                text("1.0.0")?,
            ),
            LocalDeletionConfig::new(
                ActorRef::new(ActorType::User, id("user-local")?, None),
                text("explicit-user-lineage-deletion")?,
                device_id,
                text("user-requested-local-purge")?,
                id("policy-local-deletion")?,
                ProducerRef::new(
                    ProducerType::System,
                    id("deletion-verifier")?,
                    text("1.0.0")?,
                ),
            ),
        )
    }

    pub fn new(
        namespace_id: Identifier,
        governance: Governance,
        source_producer: ProducerRef,
        segmenter: ProducerRef,
        deletion: LocalDeletionConfig,
    ) -> Result<Self, ApplicationError> {
        if governance.deletion_state() != DeletionState::Active {
            return Err(ApplicationError::invalid_configuration());
        }
        Ok(Self {
            namespace_id,
            governance,
            source_producer,
            segmenter,
            deletion,
        })
    }

    #[must_use]
    pub const fn namespace_id(&self) -> &Identifier {
        &self.namespace_id
    }
}

impl fmt::Debug for LocalLibraryConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalLibraryConfig")
            .field("namespace_id", &self.namespace_id)
            .field("governance", &self.governance)
            .field("source_producer", &self.source_producer)
            .field("segmenter", &self.segmenter)
            .field("deletion", &self.deletion)
            .finish()
    }
}

/// One source-only search hit with a resolvable citation and redacted Debug.
#[derive(Clone, Eq, PartialEq)]
pub struct SourceSearchResult {
    source_id: Identifier,
    lineage_id: Identifier,
    version: Version,
    fragment_id: Identifier,
    byte_start: u64,
    byte_end: u64,
    title: Option<NonEmptyText>,
    content: NonEmptyText,
}

impl SourceSearchResult {
    fn from_resolved(fragment: &SourceFragment, source: &SourceArtifact) -> Self {
        let fragment = fragment.params();
        let source = source.params();
        Self {
            source_id: source.source_id.clone(),
            lineage_id: source.lineage_id.clone(),
            version: source.version,
            fragment_id: fragment.fragment_id.clone(),
            byte_start: fragment.byte_start,
            byte_end: fragment.byte_end,
            title: source.title.clone(),
            content: fragment.content.clone(),
        }
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
    pub const fn fragment_id(&self) -> &Identifier {
        &self.fragment_id
    }

    #[must_use]
    pub const fn byte_start(&self) -> u64 {
        self.byte_start
    }

    #[must_use]
    pub const fn byte_end(&self) -> u64 {
        self.byte_end
    }

    #[must_use]
    pub fn title(&self) -> Option<&NonEmptyText> {
        self.title.as_ref()
    }

    #[must_use]
    pub const fn content(&self) -> &NonEmptyText {
        &self.content
    }
}

impl fmt::Debug for SourceSearchResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceSearchResult")
            .field("source_id", &self.source_id)
            .field("lineage_id", &self.lineage_id)
            .field("version", &self.version)
            .field("fragment_id", &self.fragment_id)
            .field("byte_start", &self.byte_start)
            .field("byte_end", &self.byte_end)
            .field("has_title", &self.title.is_some())
            .field("content_length", &self.content.utf8_len())
            .finish()
    }
}

/// File-backed local library composed behind one UI-facing service.
pub struct LocalLibrary<R> {
    database: SqliteDatabase,
    runtime: R,
    config: LocalLibraryConfig,
}

impl<R> LocalLibrary<R>
where
    R: ApplicationRuntime,
{
    pub fn open(
        database_path: impl AsRef<Path>,
        runtime: R,
        config: LocalLibraryConfig,
    ) -> Result<Self, ApplicationError> {
        let database = SqliteDatabase::open(database_path).map_err(|source| {
            ApplicationError::storage(ApplicationOperation::OpenLibrary, source)
        })?;
        Ok(Self {
            database,
            runtime,
            config,
        })
    }

    pub fn import_new_source(
        &mut self,
        request: &FileReadRequest,
    ) -> Result<FileCaptureReceipt, ApplicationError> {
        let operation = ApplicationOperation::ImportNewSource;
        let observed_at = self.now(operation)?;
        let snapshot = read_file_snapshot(request)
            .map_err(|source| ApplicationError::file_entry(operation, source))?;
        let origin_binding_id =
            self.next_identifier(operation, ApplicationIdentifierKind::OriginBinding)?;
        if !source_origin_binding_id_is_valid(origin_binding_id.as_str()) {
            return Err(ApplicationError::invalid_runtime_identifier(operation));
        }
        let plan = FileCapturePlan {
            namespace_id: self.config.namespace_id.clone(),
            origin_binding_id,
            source_id: self.next_identifier(operation, ApplicationIdentifierKind::Source)?,
            lineage_id: self.next_identifier(operation, ApplicationIdentifierKind::Lineage)?,
            version: Version::new(1)
                .map_err(|source| ApplicationError::canonical(operation, source))?,
            supersedes_source_ids: Vec::new(),
            fragment_id: self.next_identifier(operation, ApplicationIdentifierKind::Fragment)?,
            observed_at,
            captured_at: self.now(operation)?,
            governance: self.config.governance.clone(),
            source_producer: self.config.source_producer.clone(),
            segmenter: self.config.segmenter.clone(),
        };
        self.capture(snapshot, plan, operation)
    }

    pub fn update_source(
        &mut self,
        lineage_id: &Identifier,
        request: &FileReadRequest,
    ) -> Result<FileCaptureReceipt, ApplicationError> {
        let operation = ApplicationOperation::UpdateSource;
        let state = self
            .database
            .resolve_source_lineage(&self.config.namespace_id, lineage_id)
            .map_err(|source| ApplicationError::storage(operation, source))?
            .ok_or_else(|| ApplicationError::lineage_not_found(operation))?;
        let current = self
            .database
            .load_source_artifact(&self.config.namespace_id, state.current_source_id())
            .map_err(|source| ApplicationError::storage(operation, source))?
            .ok_or_else(|| ApplicationError::source_not_found(operation))?;
        let observed_at = self.now(operation)?;
        let snapshot = read_file_snapshot(request)
            .map_err(|source| ApplicationError::file_entry(operation, source))?;
        let next_version = state
            .current_version()
            .get()
            .checked_add(1)
            .ok_or_else(|| ApplicationError::invalid_runtime_identifier(operation))?;
        let plan = FileCapturePlan {
            namespace_id: self.config.namespace_id.clone(),
            origin_binding_id: state.origin_binding_id().clone(),
            source_id: self.next_identifier(operation, ApplicationIdentifierKind::Source)?,
            lineage_id: state.lineage_id().clone(),
            version: Version::new(next_version)
                .map_err(|source| ApplicationError::canonical(operation, source))?,
            supersedes_source_ids: vec![state.current_source_id().clone()],
            fragment_id: self.next_identifier(operation, ApplicationIdentifierKind::Fragment)?,
            observed_at,
            captured_at: self.now(operation)?,
            governance: current.params().governance.clone(),
            source_producer: self.config.source_producer.clone(),
            segmenter: self.config.segmenter.clone(),
        };
        self.capture(snapshot, plan, operation)
    }

    pub fn list_sources(
        &self,
        offset: u64,
        limit: usize,
    ) -> Result<Vec<SourceLineageSummary>, ApplicationError> {
        let operation = ApplicationOperation::ListSources;
        let request = SourceCatalogRequest::new(self.config.namespace_id.clone(), offset, limit)
            .map_err(|source| ApplicationError::canonical(operation, source))?;
        self.database
            .list_source_lineages(&request)
            .map_err(|source| ApplicationError::storage(operation, source))
    }

    pub fn list_source_versions(
        &self,
        lineage_id: &Identifier,
    ) -> Result<Vec<SourceVersionSummary>, ApplicationError> {
        let operation = ApplicationOperation::ListSources;
        self.database
            .list_source_versions(&self.config.namespace_id, lineage_id)
            .map_err(|source| ApplicationError::storage(operation, source))
    }

    pub fn get_source(
        &self,
        source_id: &Identifier,
    ) -> Result<Option<SourceArtifact>, ApplicationError> {
        let operation = ApplicationOperation::GetSource;
        self.database
            .load_source_artifact(&self.config.namespace_id, source_id)
            .map_err(|source| ApplicationError::storage(operation, source))
    }

    pub fn search_sources(
        &mut self,
        query: NonEmptyText,
        top_k: usize,
        allowed_sensitivities: impl IntoIterator<Item = Sensitivity>,
    ) -> Result<Vec<SourceSearchResult>, ApplicationError> {
        let operation = ApplicationOperation::SearchSources;
        let request = LocalSearchRequest::new(
            self.config.namespace_id.clone(),
            query,
            self.now(operation)?,
            top_k,
            allowed_sensitivities,
        )
        .map_err(|source| ApplicationError::canonical(operation, source))?;
        let hits = self
            .database
            .search(&request)
            .map_err(|source| ApplicationError::storage(operation, source))?;
        let mut results = Vec::new();
        for hit in hits {
            let LocalSearchHit::SourceFragment(fragment) = hit else {
                continue;
            };
            let source = self
                .database
                .load_source_artifact(&self.config.namespace_id, &fragment.params().source_id)
                .map_err(|source| ApplicationError::storage(operation, source))?
                .ok_or_else(|| ApplicationError::source_not_found(operation))?;
            results.push(SourceSearchResult::from_resolved(&fragment, &source));
        }
        Ok(results)
    }

    pub fn export_source(
        &self,
        source_id: &Identifier,
        request: &FileExportRequest,
    ) -> Result<FileExportReceipt, ApplicationError> {
        let operation = ApplicationOperation::ExportSource;
        let source = self
            .database
            .load_source_artifact(&self.config.namespace_id, source_id)
            .map_err(|source| ApplicationError::storage(operation, source))?
            .ok_or_else(|| ApplicationError::source_not_found(operation))?;
        export_managed_source(&source, request)
            .map_err(|source| ApplicationError::file_entry(operation, source))
    }

    /// Deletes every active source version in one lineage and persists canonical evidence.
    pub fn delete_source_lineage(
        &mut self,
        lineage_id: &Identifier,
    ) -> Result<DeletionEvidence, ApplicationError> {
        let operation = ApplicationOperation::DeleteSourceLineage;
        let target_refs = self
            .database
            .resolve_source_lineage_deletion_targets(&self.config.namespace_id, lineage_id)
            .map_err(|source| ApplicationError::storage(operation, source))?;
        if target_refs.is_empty() {
            return Err(ApplicationError::lineage_not_found(operation));
        }
        let planned_components = build_local_purge_targets(&target_refs)
            .map_err(|source| ApplicationError::canonical(operation, source))?;
        let requested_at = self.now(operation)?;
        let started_at = self.now(operation)?;
        let finished_at = self.now(operation)?;
        let delete_request_id =
            self.next_identifier(operation, ApplicationIdentifierKind::DeleteRequest)?;
        let deletion_evidence_id =
            self.next_identifier(operation, ApplicationIdentifierKind::DeletionEvidence)?;
        let request = DeleteRequest::new(DeleteRequestParams {
            delete_request_id,
            namespace_id: self.config.namespace_id.clone(),
            requested_by: self.config.deletion.requested_by.clone(),
            authorization_basis: self.config.deletion.authorization_basis.clone(),
            requested_guarantee: RequestedGuarantee::LocalPurge,
            device_id: self.config.deletion.device_id.clone(),
            target_refs,
            planned_components,
            reason_code: self.config.deletion.reason_code.clone(),
            requested_at,
        })
        .map_err(|source| ApplicationError::canonical(operation, source))?;
        self.database
            .store_delete_request(&request)
            .map_err(|source| ApplicationError::storage(operation, source))?;

        let execution = LocalDeletionExecution::new(
            started_at.clone(),
            EvidenceRef::new(
                EvidenceType::PolicyBasis,
                self.config.deletion.retention_policy_basis.clone(),
            ),
        )
        .map_err(|source| ApplicationError::canonical(operation, source))?;
        let component_results = self
            .database
            .execute_deletion(
                &self.config.namespace_id,
                &request.params().delete_request_id,
                &execution,
            )
            .map_err(|source| ApplicationError::storage(operation, source))?;
        let overall_status = if component_results
            .iter()
            .all(|result| result.params().status == ComponentStatus::Succeeded)
        {
            DeletionOverallStatus::Completed
        } else {
            DeletionOverallStatus::Failed
        };
        let evidence_digest = compute_deletion_evidence_digest(
            &deletion_evidence_id,
            &request.params().delete_request_id,
            overall_status,
            &component_results,
        )
        .map_err(|source| ApplicationError::canonical(operation, source))?;
        let evidence = DeletionEvidence::new(DeletionEvidenceParams {
            deletion_evidence_id,
            delete_request_id: request.params().delete_request_id.clone(),
            previous_evidence_id: None,
            namespace_id: self.config.namespace_id.clone(),
            device_id: self.config.deletion.device_id.clone(),
            overall_status,
            component_results,
            started_at,
            finished_at: Some(finished_at),
            verified_by: self.config.deletion.verified_by.clone(),
            evidence_digest,
        })
        .map_err(|source| ApplicationError::canonical(operation, source))?;
        self.database
            .store_deletion_evidence(&evidence)
            .map_err(|source| ApplicationError::storage(operation, source))?;
        Ok(evidence)
    }

    pub fn get_deletion_evidence(
        &self,
        deletion_evidence_id: &Identifier,
    ) -> Result<Option<DeletionEvidence>, ApplicationError> {
        let operation = ApplicationOperation::GetDeletionEvidence;
        self.database
            .load_deletion_evidence(&self.config.namespace_id, deletion_evidence_id)
            .map_err(|source| ApplicationError::storage(operation, source))
    }

    pub fn verify_library(&self) -> Result<(), ApplicationError> {
        self.database.verify_recall_derivations().map_err(|source| {
            ApplicationError::storage(ApplicationOperation::VerifyLibrary, source)
        })
    }

    pub fn rebuild_recall(&mut self) -> Result<(), ApplicationError> {
        self.database
            .rebuild_recall_derivations()
            .map_err(|source| {
                ApplicationError::storage(ApplicationOperation::RebuildRecall, source)
            })
    }

    fn capture(
        &mut self,
        snapshot: radishmemory_file_entry::ValidatedFileSnapshot,
        plan: FileCapturePlan,
        operation: ApplicationOperation,
    ) -> Result<FileCaptureReceipt, ApplicationError> {
        let capture = build_source_capture(snapshot, plan)
            .map_err(|source| ApplicationError::canonical(operation, source))?;
        let result = self
            .database
            .capture_source(&capture)
            .map_err(|source| ApplicationError::storage(operation, source))?;
        FileCaptureReceipt::from_capture_result(&result)
            .map_err(|source| ApplicationError::file_entry(operation, source))
    }

    fn next_identifier(
        &mut self,
        operation: ApplicationOperation,
        kind: ApplicationIdentifierKind,
    ) -> Result<Identifier, ApplicationError> {
        self.runtime
            .next_identifier(kind)
            .map_err(|source| ApplicationError::identifier_generation(operation, source))
    }

    fn now(&mut self, operation: ApplicationOperation) -> Result<Timestamp, ApplicationError> {
        self.runtime
            .now()
            .map_err(|source| ApplicationError::clock(operation, source))
    }
}

impl<R> fmt::Debug for LocalLibrary<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalLibrary")
            .field("database", &self.database)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
