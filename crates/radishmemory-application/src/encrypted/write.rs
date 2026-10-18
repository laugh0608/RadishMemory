//! Prepared canonical requests remain caller-owned across errors and close/reopen.
//! No implicit retry rereads a file, regenerates provenance, or widens deletion.
use super::*;
use crate::{ApplicationIdentifierKind, capture};
use radishmemory_core::{
    ComponentStatus, DeleteRequest, DeleteRequestParams, DeletionEvidence, DeletionEvidenceParams,
    DeletionOverallStatus, EvidenceRef, EvidenceType, LocalDeletionExecution, RequestedGuarantee,
    SourceCapture, SourceCatalog, build_local_purge_targets, compute_deletion_evidence_digest,
};
use radishmemory_file_entry::{FileCaptureReceipt, FileReadRequest};

impl<R: ApplicationRuntime, P: LibraryProvider> EncryptedLibrary<R, P> {
    fn require_no_pending(&self, operation: ApplicationOperation) -> Result<(), ApplicationError> {
        self.with_reader(operation, |reader| {
            let pending = reader
                .mutation_pending(&self.location.config.namespace_id)
                .map_err(|error| ApplicationError::vault(operation, error))?;
            if pending {
                return Err(ApplicationError::original_request_required(operation));
            }
            Ok(())
        })
    }

    /// Snapshot once and return the canonical request before any durable mutation.
    /// Keep it until capture succeeds. It owns plaintext; do not log or persist it
    /// in an unprotected host journal. Losing it cannot be repaired by rereading the
    /// original file: an existing pending capture then requires explicit recovery.
    pub fn prepare_import_new_source(
        &mut self,
        request: &FileReadRequest,
    ) -> Result<SourceCapture, ApplicationError> {
        self.require_no_pending(ApplicationOperation::ImportNewSource)?;
        capture::prepare(&mut self.runtime, &self.location.config, request, None)
    }

    pub fn prepare_update_source(
        &mut self,
        lineage: &Identifier,
        request: &FileReadRequest,
    ) -> Result<SourceCapture, ApplicationError> {
        let operation = ApplicationOperation::UpdateSource;
        self.require_no_pending(operation)?;
        let current = self.with_reader(operation, |reader| {
            let namespace = &self.location.config.namespace_id;
            let state = reader
                .resolve_source_lineage(namespace, lineage)
                .map_err(|error| ApplicationError::vault(operation, error))?
                .ok_or_else(|| ApplicationError::lineage_not_found(operation))?;
            let source = reader
                .load_source_artifact(namespace, state.current_source_id())
                .map_err(|error| ApplicationError::vault(operation, error))?
                .ok_or_else(|| ApplicationError::source_not_found(operation))?;
            Ok((state, source))
        })?;
        capture::prepare(
            &mut self.runtime,
            &self.location.config,
            request,
            Some(current),
        )
    }

    /// Execute or recover exactly this original canonical request. The coordinator
    /// rechecks the current lineage and pending fingerprint under its own lock.
    pub fn capture_source(
        &self,
        request: &SourceCapture,
    ) -> Result<FileCaptureReceipt, ApplicationError> {
        let operation = if request.source().params().version.get() == 1 {
            ApplicationOperation::ImportNewSource
        } else {
            ApplicationOperation::UpdateSource
        };
        if request.source().params().namespace_id != self.location.config.namespace_id {
            return Err(ApplicationError::request_profile_mismatch(operation));
        }
        let result = self
            .location
            .provider
            .capture_library_source(
                &self.location.directory,
                self.location.config.namespace_id.as_str(),
                self.location.config.deletion.device_id.as_str(),
                request,
            )
            .map_err(|error| ApplicationError::vault(operation, error))?;
        FileCaptureReceipt::from_capture_result(&result)
            .map_err(|error| ApplicationError::file_entry(operation, error))
    }

    /// Resolve the complete source lineage and dependent-memory closure once.
    /// This does not yet close recall or delete anything; retain the returned request
    /// for execute/retry. A changed closure at execution is rejected, never expanded.
    pub fn prepare_source_lineage_deletion(
        &mut self,
        lineage: &Identifier,
    ) -> Result<DeleteRequest, ApplicationError> {
        let operation = ApplicationOperation::DeleteSourceLineage;
        self.require_no_pending(operation)?;
        let target_refs = self.with_reader(operation, |reader| {
            reader
                .resolve_source_lineage_deletion_targets(
                    &self.location.config.namespace_id,
                    lineage,
                )
                .map_err(|error| ApplicationError::vault(operation, error))
        })?;
        if target_refs.is_empty() {
            return Err(ApplicationError::lineage_not_found(operation));
        }
        let planned_components = build_local_purge_targets(&target_refs)
            .map_err(|error| ApplicationError::canonical(operation, error))?;
        let config = &self.location.config;
        DeleteRequest::new(DeleteRequestParams {
            delete_request_id: capture::next_identifier(
                &mut self.runtime,
                operation,
                ApplicationIdentifierKind::DeleteRequest,
            )?,
            namespace_id: config.namespace_id.clone(),
            requested_by: config.deletion.requested_by.clone(),
            authorization_basis: config.deletion.authorization_basis.clone(),
            requested_guarantee: RequestedGuarantee::LocalPurge,
            device_id: config.deletion.device_id.clone(),
            target_refs,
            planned_components,
            reason_code: config.deletion.reason_code.clone(),
            requested_at: capture::now(&mut self.runtime, operation)?,
        })
        .map_err(|error| ApplicationError::canonical(operation, error))
    }

    /// Retrieve the persisted original authority after a process restart. Never
    /// rerun lineage planning to recover an already-started deletion.
    pub fn get_delete_request(
        &self,
        request: &Identifier,
    ) -> Result<Option<DeleteRequest>, ApplicationError> {
        let operation = ApplicationOperation::DeleteSourceLineage;
        self.with_reader(operation, |reader| {
            reader
                .load_delete_request(&self.location.config.namespace_id, request)
                .map_err(|error| ApplicationError::vault(operation, error))
        })
    }

    /// Discover durable unfinished deletion authority after restart. No new request
    /// is generated, and results are never automatically executed.
    pub fn unfinished_delete_requests(&self) -> Result<Vec<DeleteRequest>, ApplicationError> {
        let operation = ApplicationOperation::InspectRecovery;
        self.with_reader(operation, |reader| {
            reader
                .unfinished_delete_requests(&self.location.config.namespace_id)
                .map_err(|error| ApplicationError::vault(operation, error))
        })
    }

    pub fn inspect_capture_abandonment(
        &self,
    ) -> Result<Option<radishmemory_source_vault::CaptureAbandonmentTarget>, ApplicationError> {
        self.location.inspect_capture_abandonment()
    }

    /// Caller must obtain explicit approval for exactly this opaque target. This is
    /// irreversible abandonment, not cancellation of an already committed source.
    pub fn abandon_capture(
        &self,
        target: &radishmemory_source_vault::CaptureAbandonmentTarget,
    ) -> Result<radishmemory_source_vault::AbandonmentReport, ApplicationError> {
        self.location.abandon_capture(target)
    }

    pub fn get_deletion_evidence(
        &self,
        evidence: &Identifier,
    ) -> Result<Option<DeletionEvidence>, ApplicationError> {
        let operation = ApplicationOperation::GetDeletionEvidence;
        self.with_reader(operation, |reader| {
            reader
                .load_deletion_evidence(&self.location.config.namespace_id, evidence)
                .map_err(|error| ApplicationError::vault(operation, error))
        })
    }

    /// Execute/resume the exact frozen request and persist real component results.
    /// Completed evidence is returned unchanged on retry; failed attempts append
    /// evidence with the previous receipt ID. No failure implies completed deletion.
    pub fn execute_source_lineage_deletion(
        &mut self,
        request: &DeleteRequest,
    ) -> Result<DeletionEvidence, ApplicationError> {
        let operation = ApplicationOperation::DeleteSourceLineage;
        let config = &self.location.config;
        if request.params().namespace_id != config.namespace_id
            || request.params().device_id != config.deletion.device_id
        {
            return Err(ApplicationError::request_profile_mismatch(operation));
        }
        let previous = self.with_reader(operation, |reader| {
            if let Some(stored) = reader
                .load_delete_request(&config.namespace_id, &request.params().delete_request_id)
                .map_err(|error| ApplicationError::vault(operation, error))?
                && stored != *request
            {
                return Err(ApplicationError::request_profile_mismatch(operation));
            }
            reader
                .latest_deletion_evidence(&config.namespace_id, &request.params().delete_request_id)
                .map_err(|error| ApplicationError::vault(operation, error))
        })?;
        if let Some(evidence) = &previous
            && evidence.params().overall_status == DeletionOverallStatus::Completed
        {
            return Ok(evidence.clone());
        }
        let evidence_id = capture::next_identifier(
            &mut self.runtime,
            operation,
            ApplicationIdentifierKind::DeletionEvidence,
        )?;
        let started_at = capture::now(&mut self.runtime, operation)?;
        let execution = LocalDeletionExecution::new(
            started_at.clone(),
            EvidenceRef::new(
                EvidenceType::PolicyBasis,
                config.deletion.retention_policy_basis.clone(),
            ),
        )
        .map_err(|error| ApplicationError::canonical(operation, error))?;
        let results = self
            .location
            .provider
            .execute_library_deletion(&self.location.directory, request, &execution)
            .map_err(|error| ApplicationError::vault(operation, error))?;
        // Obtain the real finishing time after execution. Clock failure preserves
        // the persisted request/results for explicit recovery, never invents a time.
        let finished_at = capture::now(&mut self.runtime, operation)?;
        let overall_status = if results
            .iter()
            .all(|result| result.params().status == ComponentStatus::Succeeded)
        {
            DeletionOverallStatus::Completed
        } else {
            DeletionOverallStatus::Failed
        };
        let digest = compute_deletion_evidence_digest(
            &evidence_id,
            &request.params().delete_request_id,
            overall_status,
            &results,
        )
        .map_err(|error| ApplicationError::canonical(operation, error))?;
        let evidence = DeletionEvidence::new(DeletionEvidenceParams {
            deletion_evidence_id: evidence_id,
            delete_request_id: request.params().delete_request_id.clone(),
            previous_evidence_id: previous.map(|e| e.params().deletion_evidence_id.clone()),
            namespace_id: config.namespace_id.clone(),
            device_id: config.deletion.device_id.clone(),
            overall_status,
            component_results: results,
            started_at,
            finished_at: Some(finished_at),
            verified_by: config.deletion.verified_by.clone(),
            evidence_digest: digest,
        })
        .map_err(|error| ApplicationError::canonical(operation, error))?;
        self.location
            .provider
            .store_library_deletion_evidence(&self.location.directory, &evidence)
            .map_err(|error| ApplicationError::vault(operation, error))?;
        Ok(evidence)
    }

    pub fn verify_library(
        &self,
    ) -> Result<radishmemory_source_vault::VerificationReport, ApplicationError> {
        self.location.verify_library()
    }
    pub fn rebuild_recall(
        &self,
    ) -> Result<radishmemory_source_vault::VerificationReport, ApplicationError> {
        self.location.rebuild_recall()
    }
}
