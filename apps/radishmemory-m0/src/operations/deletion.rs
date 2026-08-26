use radishmemory_core::{
    CanonicalObjectType, ComponentStatus, DeleteRequest, DeleteRequestParams,
    DeletionComponentType, DeletionEvidence, DeletionEvidenceParams, DeletionOverallStatus,
    DeletionStore, DeletionTarget, DeletionTargetRef, EvidenceRef, EvidenceType,
    FrozenTargetClosure, LocalDeletionExecution, MemoryStore, ObjectRef, RequestedGuarantee,
    RequiredAction, SourceVault, compute_digest,
};
use radishmemory_sqlite::FixtureDeletionFailure;
use serde_json::Value;

use super::OperationOutcome;
use crate::error::{RunnerError, RunnerErrorCode, RunnerResult};
use crate::fixture::{bool_value, object, string, strings};
use crate::state::{
    ExecutionSnapshot, ScenarioState, actor, fixture_producer, id, text, timestamp,
};

const PROFILE: [(&str, DeletionComponentType, RequiredAction); 10] = [
    (
        "source-body",
        DeletionComponentType::SourceBody,
        RequiredAction::Delete,
    ),
    (
        "source-metadata",
        DeletionComponentType::SourceMetadata,
        RequiredAction::RetainMinimal,
    ),
    (
        "source-fragment",
        DeletionComponentType::SourceFragment,
        RequiredAction::Delete,
    ),
    (
        "memory-proposal",
        DeletionComponentType::MemoryProposal,
        RequiredAction::Redact,
    ),
    (
        "memory-decision",
        DeletionComponentType::MemoryDecision,
        RequiredAction::RetainMinimal,
    ),
    (
        "memory-record",
        DeletionComponentType::MemoryRecord,
        RequiredAction::Redact,
    ),
    (
        "memory-state-event",
        DeletionComponentType::MemoryStateEvent,
        RequiredAction::RetainMinimal,
    ),
    (
        "full-text-index",
        DeletionComponentType::FullTextIndex,
        RequiredAction::Delete,
    ),
    (
        "context-cache",
        DeletionComponentType::ContextCache,
        RequiredAction::Delete,
    ),
    (
        "minimal-audit",
        DeletionComponentType::MinimalAudit,
        RequiredAction::RetainMinimal,
    ),
];

pub fn plan_delete(state: &mut ScenarioState, input: &Value) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    if string(input, "requested_guarantee")? != "local_purge"
        || string(input, "component_profile")? != "m0-local-purge"
    {
        return Err(invalid_operation("deletion-profile-unsupported"));
    }
    let logical_key = string(input, "logical_key")?;
    let target_keys = strings(input, "target_keys")?;
    let target_refs = target_keys
        .iter()
        .map(|key| {
            if let Some(source) = state.sources.get(key) {
                Ok(ObjectRef::new(
                    CanonicalObjectType::SourceArtifact,
                    source.params().source_id.clone(),
                ))
            } else if let Some(record) = state.records.get(key) {
                Ok(ObjectRef::new(
                    CanonicalObjectType::MemoryRecord,
                    record.params().memory_id.clone(),
                ))
            } else {
                Err(invalid_operation("delete-target-unresolved"))
            }
        })
        .collect::<RunnerResult<Vec<_>>>()?;
    let mut sorted = target_refs.clone();
    sorted.sort();
    let target_ref = if sorted.len() == 1 {
        DeletionTargetRef::Object(sorted[0].clone())
    } else {
        DeletionTargetRef::FrozenClosure(
            FrozenTargetClosure::freeze(sorted).map_err(core("delete-closure-invalid"))?,
        )
    };
    let target_count = target_refs.len() as u64;
    let components = PROFILE
        .iter()
        .map(|(key, component_type, action)| {
            DeletionTarget::new(
                id(key)?,
                *component_type,
                target_ref.clone(),
                target_count,
                *action,
            )
            .map_err(core("deletion-component-invalid"))
        })
        .collect::<RunnerResult<Vec<_>>>()?;
    let request_id = state.stable_id("DeleteRequest", logical_key)?;
    let request = DeleteRequest::new(DeleteRequestParams {
        delete_request_id: request_id.clone(),
        namespace_id: state.namespace_id.clone(),
        requested_by: actor("user:sample")?,
        authorization_basis: text("explicit-fixture-deletion-authorization")?,
        requested_guarantee: RequestedGuarantee::LocalPurge,
        device_id: state.device_id.clone(),
        target_refs,
        planned_components: components,
        reason_code: text("fixture-local-purge")?,
        requested_at: timestamp(string(input, "requested_at")?)?,
    })
    .map_err(core("delete-request-invalid"))?;
    state
        .storage
        .database
        .store_delete_request(&request)
        .map_err(storage("delete-plan-store-failed"))?;
    let loaded = state
        .storage
        .database
        .load_delete_request(&state.namespace_id, &request_id)
        .map_err(storage("delete-plan-load-failed"))?;
    let closed = target_keys.iter().all(|key| {
        if let Some(source) = state.sources.get(key) {
            state
                .storage
                .database
                .load_source_artifact(&state.namespace_id, &source.params().source_id)
                .is_ok_and(|value| value.is_none())
        } else if let Some(record) = state.records.get(key) {
            state
                .storage
                .database
                .load_memory_record(&state.namespace_id, &record.params().memory_id)
                .is_ok_and(|value| value.is_none())
        } else {
            false
        }
    });
    state
        .delete_requests
        .insert(logical_key.to_owned(), request);
    state.emit(logical_key, &request_id, None);
    Ok(OperationOutcome::succeeded([
        (
            "planned-component-count-10",
            loaded
                .as_ref()
                .is_some_and(|value| value.params().planned_components.len() == 10),
        ),
        ("targets-enter-pending-before-delete", closed),
    ]))
}

pub fn execute_deletion(
    state: &mut ScenarioState,
    input: &Value,
) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    let request_key = string(input, "delete_request_key")?;
    let request = state
        .delete_requests
        .get(request_key)
        .cloned()
        .ok_or_else(|| invalid_operation("delete-request-unresolved"))?;
    let execution = LocalDeletionExecution::new(
        request.params().requested_at.clone(),
        EvidenceRef::new(
            EvidenceType::PolicyBasis,
            id("policy:m0:local-deletion-v1")?,
        ),
    )
    .map_err(core("deletion-execution-input-invalid"))?;
    let profile = string(input, "component_outcome_profile")?;
    let (results, expected_error_code) = match profile {
        "all-required-actions-succeeded" => (
            state
                .storage
                .database
                .execute_deletion(
                    &state.namespace_id,
                    &request.params().delete_request_id,
                    &execution,
                )
                .map_err(storage("deletion-execution-failed"))?,
            None,
        ),
        "fail-one-component" => {
            let error_code = string(input, "error_code")?.to_owned();
            let failure = FixtureDeletionFailure::new(
                id(string(input, "failed_component_key")?)?,
                text(&error_code)?,
                bool_value(input, "retryable")?,
            );
            (
                state
                    .storage
                    .database
                    .execute_deletion_with_fixture_failure(
                        &state.namespace_id,
                        &request.params().delete_request_id,
                        &execution,
                        &failure,
                    )
                    .map_err(storage("fixture-deletion-execution-failed"))?,
                Some(error_code),
            )
        }
        _ => return Err(invalid_operation("deletion-outcome-profile-unsupported")),
    };
    let component_keys = results
        .iter()
        .map(|result| result.params().component_key.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let coverage = component_keys.len() == request.params().planned_components.len();
    state.metrics.deletion_numerator += component_keys.len() as u64;
    state.metrics.deletion_denominator += request.params().planned_components.len() as u64;
    let failed = results
        .iter()
        .any(|result| result.params().status == ComponentStatus::Failed);
    let closed = state.storage.database.verify_recall_derivations().is_ok();
    let content_absent =
        results.iter().all(|result| {
            let verification = result.params().verification_method.as_str();
            state.sources.values().all(|source| {
                !verification.contains(source.params().content.as_str())
                    && result.params().error_code.as_ref().is_none_or(|code| {
                        !code.as_str().contains(source.params().content.as_str())
                    })
            })
        });
    let first_error = results
        .iter()
        .find_map(|result| result.params().error_code.as_ref())
        .map(|value| value.as_str().to_owned());
    let retryable = results.iter().find_map(|result| result.params().retryable);
    state.executions.insert(
        request_key.to_owned(),
        ExecutionSnapshot {
            results,
            expected_error_code: expected_error_code.clone(),
        },
    );
    let outcome = OperationOutcome::succeeded([
        ("all-planned-components-have-results", coverage),
        ("deleted-content-not-in-evidence", content_absent),
        ("failed-component-keeps-target-closed", failed && closed),
    ])
    .with_status(if failed { "failed" } else { "succeeded" });
    Ok(match (first_error, retryable) {
        (Some(code), Some(retryable)) => outcome.with_error(code, retryable),
        _ => outcome,
    })
}

pub fn emit_deletion_evidence(
    state: &mut ScenarioState,
    input: &Value,
) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    let logical_key = string(input, "logical_key")?;
    let request_key = string(input, "delete_request_key")?;
    let request = state
        .delete_requests
        .get(request_key)
        .cloned()
        .ok_or_else(|| invalid_operation("evidence-request-unresolved"))?;
    let execution = state
        .executions
        .get(request_key)
        .ok_or_else(|| invalid_operation("evidence-execution-unresolved"))?;
    let overall_status = if execution
        .results
        .iter()
        .all(|result| result.params().status == ComponentStatus::Succeeded)
    {
        DeletionOverallStatus::Completed
    } else {
        DeletionOverallStatus::Failed
    };
    let expected_status = match string(input, "expected_overall_status")? {
        "completed" => DeletionOverallStatus::Completed,
        "failed" => DeletionOverallStatus::Failed,
        _ => return Err(invalid_operation("evidence-status-unsupported")),
    };
    let evidence_id = state.stable_id("DeletionEvidence", logical_key)?;
    let digest_value = serde_json::json!({
        "component_results": execution.results.iter().map(|result| serde_json::json!({
            "component_key": result.params().component_key.as_str(),
            "processed_count": result.params().processed_count,
            "status": component_status(result.params().status),
        })).collect::<Vec<_>>(),
        "deletion_evidence_id": evidence_id.as_str(),
        "delete_request_id": request.params().delete_request_id.as_str(),
        "overall_status": overall_status_str(overall_status),
    });
    let digest = compute_digest("deletion-evidence-v1", &digest_value.to_string())
        .map_err(core("deletion-evidence-digest-invalid"))?;
    let evidence = DeletionEvidence::new(DeletionEvidenceParams {
        deletion_evidence_id: evidence_id.clone(),
        delete_request_id: request.params().delete_request_id.clone(),
        previous_evidence_id: None,
        namespace_id: state.namespace_id.clone(),
        device_id: state.device_id.clone(),
        overall_status,
        component_results: execution.results.clone(),
        started_at: request.params().requested_at.clone(),
        finished_at: Some(request.params().requested_at.clone()),
        verified_by: fixture_producer()?,
        evidence_digest: digest.clone(),
    })
    .map_err(core("deletion-evidence-invalid"))?;
    state
        .storage
        .database
        .store_deletion_evidence(&evidence)
        .map_err(storage("deletion-evidence-store-failed"))?;
    let loaded = state
        .storage
        .database
        .load_deletion_evidence(&state.namespace_id, &evidence_id)
        .map_err(storage("deletion-evidence-load-failed"))?;
    let component_exact = loaded.as_ref().is_some_and(|loaded| {
        loaded.params().component_results.len() == request.params().planned_components.len()
    });
    let error_preserved = execution
        .expected_error_code
        .as_ref()
        .is_none_or(|expected| {
            evidence.params().component_results.iter().any(|result| {
                result
                    .params()
                    .error_code
                    .as_ref()
                    .is_some_and(|actual| actual.as_str() == expected)
            })
        });
    let completed_after_success = overall_status != DeletionOverallStatus::Completed
        || evidence
            .params()
            .component_results
            .iter()
            .all(|result| result.params().status == ComponentStatus::Succeeded);
    state.evidences.insert(logical_key.to_owned(), evidence);
    state.emit(
        logical_key,
        &evidence_id,
        Some((digest.profile().as_str(), digest.value())),
    );
    Ok(OperationOutcome::succeeded([
        (
            "completed-only-after-all-success",
            overall_status == expected_status && completed_after_success,
        ),
        ("evidence-component-set-exact", component_exact),
        (
            "failed-evidence-not-completed",
            expected_status != DeletionOverallStatus::Failed
                || overall_status == DeletionOverallStatus::Failed,
        ),
        ("error-code-preserved", error_preserved),
    ]))
}

fn component_status(status: ComponentStatus) -> &'static str {
    match status {
        ComponentStatus::Pending => "pending",
        ComponentStatus::Succeeded => "succeeded",
        ComponentStatus::Failed => "failed",
    }
}

fn overall_status_str(status: DeletionOverallStatus) -> &'static str {
    match status {
        DeletionOverallStatus::Pending => "pending",
        DeletionOverallStatus::Partial => "partial",
        DeletionOverallStatus::Failed => "failed",
        DeletionOverallStatus::Completed => "completed",
    }
}

fn core(detail: &'static str) -> impl FnOnce(radishmemory_core::CoreError) -> RunnerError {
    move |source| RunnerError::with_source(RunnerErrorCode::OperationFailed, detail, source)
}

fn storage(detail: &'static str) -> impl FnOnce(radishmemory_sqlite::SqliteError) -> RunnerError {
    move |source| RunnerError::with_source(RunnerErrorCode::Storage, detail, source)
}

fn invalid_operation(detail: &'static str) -> RunnerError {
    RunnerError::new(RunnerErrorCode::OperationFailed, detail)
}
