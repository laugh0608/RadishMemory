use std::collections::BTreeSet;

use radishmemory_core::{
    CanonicalObjectType, ComponentOutcome, ComponentResult, ComponentResultParams, ComponentStatus,
    DeleteRequest, DeletionTarget, Identifier, LocalDeletionExecution, ObjectRef,
    compute_nfc_text_digest,
};
use rusqlite::{Connection, Transaction, params};

use crate::deletion_store::{
    component_type_str, decode_ordered_object_refs, deletion_core, recall_kind, stored_object_ref,
};
use crate::source_store::non_empty_text;
use crate::{SqliteError, SqliteStorageReason};

const REDACTION_MARKER: &str = "[redacted:local-deletion]";
const COMPONENT_ERROR_CODE: &str = "sqlite-deletion-component-failed";

pub(crate) struct ActionResult {
    outcome: ComponentOutcome,
    verification_method: &'static str,
}

pub(crate) fn successful_result(
    component: &DeletionTarget,
    execution: &LocalDeletionExecution,
    action: ActionResult,
) -> Result<ComponentResult, SqliteError> {
    let retained = action.outcome == ComponentOutcome::RetainedMinimal;
    ComponentResult::new(ComponentResultParams {
        component_key: component.component_key().clone(),
        component_type: component.component_type(),
        target_ref: component.target_ref().clone(),
        required_action: component.required_action(),
        target_count: component.target_count(),
        processed_count: component.target_count(),
        status: ComponentStatus::Succeeded,
        outcome: action.outcome,
        verification_method: non_empty_text(action.verification_method.to_owned())?,
        checked_at: execution.checked_at().clone(),
        error_code: None,
        retryable: None,
        retention_basis: retained.then(|| execution.retention_basis().clone()),
    })
    .map_err(deletion_core)
}

pub(crate) fn failed_result(
    component: &DeletionTarget,
    execution: &LocalDeletionExecution,
) -> Result<ComponentResult, SqliteError> {
    failed_result_with_code(
        component,
        execution,
        non_empty_text(COMPONENT_ERROR_CODE.to_owned())?,
        true,
    )
}

pub(crate) fn failed_result_with_code(
    component: &DeletionTarget,
    execution: &LocalDeletionExecution,
    error_code: radishmemory_core::NonEmptyText,
    retryable: bool,
) -> Result<ComponentResult, SqliteError> {
    ComponentResult::new(ComponentResultParams {
        component_key: component.component_key().clone(),
        component_type: component.component_type(),
        target_ref: component.target_ref().clone(),
        required_action: component.required_action(),
        target_count: component.target_count(),
        processed_count: 0,
        status: ComponentStatus::Failed,
        outcome: ComponentOutcome::NotApplicable,
        verification_method: non_empty_text("sqlite-component-transaction-v1".to_owned())?,
        checked_at: execution.checked_at().clone(),
        error_code: Some(error_code),
        retryable: Some(retryable),
        retention_basis: None,
    })
    .map_err(deletion_core)
}

pub(crate) fn execute_component_action(
    transaction: &Transaction<'_>,
    request: &DeleteRequest,
    component_type: radishmemory_core::DeletionComponentType,
    attempt_ordinal: i64,
    has_failed: bool,
) -> Result<ActionResult, SqliteError> {
    let targets = load_execution_closure(
        transaction,
        &request.params().delete_request_id,
        component_type,
    )?;
    match component_type {
        radishmemory_core::DeletionComponentType::SourceBody => {
            delete_source_bodies(transaction, &targets)
        }
        radishmemory_core::DeletionComponentType::SourceMetadata => {
            redact_source_metadata(transaction, &targets)
        }
        radishmemory_core::DeletionComponentType::SourceFragment => {
            delete_source_fragments(transaction, &targets)
        }
        radishmemory_core::DeletionComponentType::MemoryProposal => {
            redact_memory_proposals(transaction, &targets)
        }
        radishmemory_core::DeletionComponentType::MemoryDecision => {
            retain_memory_decisions(transaction, &targets)
        }
        radishmemory_core::DeletionComponentType::MemoryRecord => {
            redact_memory_records(transaction, &targets)
        }
        radishmemory_core::DeletionComponentType::MemoryStateEvent => {
            retain_memory_events(transaction, &targets)
        }
        radishmemory_core::DeletionComponentType::FullTextIndex => {
            delete_full_text_rows(transaction, &targets)
        }
        radishmemory_core::DeletionComponentType::ContextCache => {
            verify_context_cache_absent(&targets)
        }
        radishmemory_core::DeletionComponentType::MinimalAudit => {
            retain_minimal_audit(transaction, request, &targets, attempt_ordinal, has_failed)
        }
    }
}

fn load_execution_closure(
    connection: &Connection,
    request_id: &Identifier,
    component_type: radishmemory_core::DeletionComponentType,
) -> Result<Vec<ObjectRef>, SqliteError> {
    let mut statement = connection
        .prepare(
            "SELECT ordinal, object_type, object_id
             FROM radishmemory_delete_execution_closure
             WHERE delete_request_id = ?1 AND component_type = ?2 ORDER BY ordinal",
        )
        .map_err(SqliteError::storage)?;
    let rows = statement
        .query_map(
            params![request_id.as_str(), component_type_str(component_type)],
            stored_object_ref,
        )
        .map_err(SqliteError::storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SqliteError::storage)?;
    decode_ordered_object_refs(rows)
}

fn delete_source_bodies(
    transaction: &Transaction<'_>,
    targets: &[ObjectRef],
) -> Result<ActionResult, SqliteError> {
    require_closure_type(targets, CanonicalObjectType::SourceArtifact)?;
    let mut removed = 0;
    for target in targets {
        removed += transaction
            .execute(
                "DELETE FROM radishmemory_source_bodies WHERE source_id = ?1",
                params![target.object_id().as_str()],
            )
            .map_err(SqliteError::storage)?;
    }
    Ok(ActionResult {
        outcome: if removed == 0 {
            ComponentOutcome::NotFound
        } else {
            ComponentOutcome::Deleted
        },
        verification_method: "sqlite-row-absence-v1",
    })
}

fn redact_source_metadata(
    transaction: &Transaction<'_>,
    targets: &[ObjectRef],
) -> Result<ActionResult, SqliteError> {
    require_closure_type(targets, CanonicalObjectType::SourceArtifact)?;
    let mut retained = 0;
    for target in targets {
        retained += transaction
            .execute(
                "UPDATE radishmemory_source_artifacts SET title = NULL, origin_ref = NULL
                 WHERE source_id = ?1",
                params![target.object_id().as_str()],
            )
            .map_err(SqliteError::storage)?;
    }
    Ok(ActionResult {
        outcome: if retained == 0 {
            ComponentOutcome::NotFound
        } else {
            ComponentOutcome::RetainedMinimal
        },
        verification_method: "sqlite-minimal-metadata-v1",
    })
}

fn delete_source_fragments(
    transaction: &Transaction<'_>,
    targets: &[ObjectRef],
) -> Result<ActionResult, SqliteError> {
    require_closure_type(targets, CanonicalObjectType::SourceFragment)?;
    let mut removed = 0;
    for target in targets {
        transaction
            .execute(
                "DELETE FROM radishmemory_fragment_heading_path WHERE fragment_id = ?1",
                params![target.object_id().as_str()],
            )
            .map_err(SqliteError::storage)?;
        removed += transaction
            .execute(
                "DELETE FROM radishmemory_source_fragments WHERE fragment_id = ?1",
                params![target.object_id().as_str()],
            )
            .map_err(SqliteError::storage)?;
    }
    Ok(ActionResult {
        outcome: if removed == 0 {
            ComponentOutcome::NotFound
        } else {
            ComponentOutcome::Deleted
        },
        verification_method: "sqlite-row-absence-v1",
    })
}

fn redact_memory_proposals(
    transaction: &Transaction<'_>,
    targets: &[ObjectRef],
) -> Result<ActionResult, SqliteError> {
    require_closure_type(targets, CanonicalObjectType::MemoryProposal)?;
    let marker_digest = compute_nfc_text_digest(REDACTION_MARKER);
    let mut redacted = 0;
    for target in targets {
        transaction
            .execute(
                "DELETE FROM radishmemory_proposal_source_fragments WHERE proposal_id = ?1",
                params![target.object_id().as_str()],
            )
            .map_err(SqliteError::storage)?;
        redacted += transaction
            .execute(
                "UPDATE radishmemory_memory_proposals
                 SET content_text = ?2, content_digest_value = ?3
                 WHERE proposal_id = ?1",
                params![
                    target.object_id().as_str(),
                    REDACTION_MARKER,
                    marker_digest.value()
                ],
            )
            .map_err(SqliteError::storage)?;
    }
    Ok(ActionResult {
        outcome: if redacted == 0 {
            ComponentOutcome::NotFound
        } else {
            ComponentOutcome::Redacted
        },
        verification_method: "sqlite-content-redaction-v1",
    })
}

fn retain_memory_decisions(
    transaction: &Transaction<'_>,
    targets: &[ObjectRef],
) -> Result<ActionResult, SqliteError> {
    require_closure_type(targets, CanonicalObjectType::MemoryDecision)?;
    let mut retained = 0;
    for target in targets {
        retained += transaction
            .execute(
                "UPDATE radishmemory_memory_decisions SET reason_text = NULL WHERE decision_id = ?1",
                params![target.object_id().as_str()],
            )
            .map_err(SqliteError::storage)?;
    }
    Ok(ActionResult {
        outcome: if retained == 0 {
            ComponentOutcome::NotFound
        } else {
            ComponentOutcome::RetainedMinimal
        },
        verification_method: "sqlite-minimal-decision-v1",
    })
}

fn redact_memory_records(
    transaction: &Transaction<'_>,
    targets: &[ObjectRef],
) -> Result<ActionResult, SqliteError> {
    require_closure_type(targets, CanonicalObjectType::MemoryRecord)?;
    let marker_digest = compute_nfc_text_digest(REDACTION_MARKER);
    let mut redacted = 0;
    for target in targets {
        transaction
            .execute(
                "DELETE FROM radishmemory_record_source_fragments WHERE memory_id = ?1",
                params![target.object_id().as_str()],
            )
            .map_err(SqliteError::storage)?;
        redacted += transaction
            .execute(
                "UPDATE radishmemory_memory_records
                 SET content_text = ?2, content_digest_value = ?3
                 WHERE memory_id = ?1",
                params![
                    target.object_id().as_str(),
                    REDACTION_MARKER,
                    marker_digest.value()
                ],
            )
            .map_err(SqliteError::storage)?;
    }
    Ok(ActionResult {
        outcome: if redacted == 0 {
            ComponentOutcome::NotFound
        } else {
            ComponentOutcome::Redacted
        },
        verification_method: "sqlite-content-redaction-v1",
    })
}

fn retain_memory_events(
    transaction: &Transaction<'_>,
    targets: &[ObjectRef],
) -> Result<ActionResult, SqliteError> {
    require_closure_type(targets, CanonicalObjectType::MemoryStateEvent)?;
    let mut retained = 0;
    for target in targets {
        let exists: bool = transaction
            .query_row(
                "SELECT EXISTS (
                     SELECT 1 FROM radishmemory_memory_state_events WHERE event_id = ?1
                 )",
                params![target.object_id().as_str()],
                |row| row.get(0),
            )
            .map_err(SqliteError::storage)?;
        retained += usize::from(exists);
    }
    Ok(ActionResult {
        outcome: if retained == 0 {
            ComponentOutcome::NotFound
        } else {
            ComponentOutcome::RetainedMinimal
        },
        verification_method: "sqlite-minimal-state-event-v1",
    })
}

fn delete_full_text_rows(
    transaction: &Transaction<'_>,
    targets: &[ObjectRef],
) -> Result<ActionResult, SqliteError> {
    let mut removed = 0;
    for target in targets {
        removed += transaction
            .execute(
                "DELETE FROM radishmemory_recall_fts WHERE object_kind = ?1 AND object_id = ?2",
                params![
                    recall_kind(target.object_type())?,
                    target.object_id().as_str()
                ],
            )
            .map_err(SqliteError::storage)?;
        let remains: bool = transaction
            .query_row(
                "SELECT EXISTS (
                     SELECT 1 FROM radishmemory_recall_fts
                     WHERE object_kind = ?1 AND object_id = ?2
                 )",
                params![
                    recall_kind(target.object_type())?,
                    target.object_id().as_str()
                ],
                |row| row.get(0),
            )
            .map_err(SqliteError::storage)?;
        if remains {
            return Err(SqliteError::deletion_invariant(
                SqliteStorageReason::DeletionExecution,
            ));
        }
    }
    Ok(ActionResult {
        outcome: if removed == 0 {
            ComponentOutcome::NotFound
        } else {
            ComponentOutcome::Deleted
        },
        verification_method: "sqlite-fts-row-absence-v1",
    })
}

fn verify_context_cache_absent(targets: &[ObjectRef]) -> Result<ActionResult, SqliteError> {
    if !targets.is_empty() {
        return Err(SqliteError::deletion_invariant(
            SqliteStorageReason::DeletionExecution,
        ));
    }
    Ok(ActionResult {
        outcome: ComponentOutcome::NotFound,
        verification_method: "m0-context-cache-not-persisted-v1",
    })
}

fn retain_minimal_audit(
    transaction: &Transaction<'_>,
    request: &DeleteRequest,
    targets: &[ObjectRef],
    attempt_ordinal: i64,
    has_failed: bool,
) -> Result<ActionResult, SqliteError> {
    let request_targets = targets
        .iter()
        .filter(|target| target.object_type() == CanonicalObjectType::DeleteRequest)
        .collect::<Vec<_>>();
    let source_targets = targets
        .iter()
        .filter(|target| target.object_type() == CanonicalObjectType::SourceArtifact)
        .collect::<Vec<_>>();
    if request_targets.len() != 1
        || request_targets[0].object_id() != &request.params().delete_request_id
        || request_targets.len() + source_targets.len() != targets.len()
    {
        return Err(SqliteError::deletion_invariant(
            SqliteStorageReason::DeletionExecution,
        ));
    }
    let result_count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM radishmemory_deletion_execution_results
             WHERE delete_request_id = ?1 AND attempt_ordinal = ?2",
            params![request.params().delete_request_id.as_str(), attempt_ordinal],
            |row| row.get(0),
        )
        .map_err(SqliteError::storage)?;
    if result_count != 9 {
        return Err(SqliteError::deletion_invariant(
            SqliteStorageReason::DeletionExecution,
        ));
    }
    purge_source_entry_state(transaction, request, &source_targets)?;
    let final_state = if has_failed { "failed" } else { "deleted" };
    finalize_governance_state(
        transaction,
        &request.params().delete_request_id,
        radishmemory_core::DeletionComponentType::SourceMetadata,
        "radishmemory_source_artifacts",
        "source_id",
        final_state,
    )?;
    finalize_governance_state(
        transaction,
        &request.params().delete_request_id,
        radishmemory_core::DeletionComponentType::MemoryProposal,
        "radishmemory_memory_proposals",
        "proposal_id",
        final_state,
    )?;
    finalize_governance_state(
        transaction,
        &request.params().delete_request_id,
        radishmemory_core::DeletionComponentType::MemoryRecord,
        "radishmemory_memory_records",
        "memory_id",
        final_state,
    )?;
    Ok(ActionResult {
        outcome: ComponentOutcome::RetainedMinimal,
        verification_method: "sqlite-minimal-audit-chain-v1",
    })
}

fn purge_source_entry_state(
    transaction: &Transaction<'_>,
    request: &DeleteRequest,
    source_targets: &[&ObjectRef],
) -> Result<(), SqliteError> {
    let namespace_id = request.params().namespace_id.as_str();
    let mut lineages = BTreeSet::new();
    for target in source_targets {
        let lineage_id: String = transaction
            .query_row(
                "SELECT lineage_id FROM radishmemory_source_artifacts
                 WHERE source_id = ?1 AND namespace_id = ?2",
                params![target.object_id().as_str(), namespace_id],
                |row| row.get(0),
            )
            .map_err(SqliteError::storage)?;
        lineages.insert(lineage_id);
    }

    for lineage_id in lineages {
        let tip_exists: bool = transaction
            .query_row(
                "SELECT EXISTS (
                     SELECT 1 FROM radishmemory_source_lineage_tips
                     WHERE namespace_id = ?1 AND lineage_id = ?2
                 )",
                params![namespace_id, lineage_id],
                |row| row.get(0),
            )
            .map_err(SqliteError::storage)?;
        if tip_exists {
            return Err(SqliteError::deletion_invariant(
                SqliteStorageReason::DeletionExecution,
            ));
        }
        transaction
            .execute(
                "DELETE FROM radishmemory_source_capture_audit
                 WHERE namespace_id = ?1 AND source_id IN (
                     SELECT source_id FROM radishmemory_source_artifacts
                     WHERE namespace_id = ?1 AND lineage_id = ?2
                 )",
                params![namespace_id, lineage_id],
            )
            .map_err(SqliteError::storage)?;
        transaction
            .execute(
                "DELETE FROM radishmemory_source_origin_bindings
                 WHERE namespace_id = ?1 AND lineage_id = ?2",
                params![namespace_id, lineage_id],
            )
            .map_err(SqliteError::storage)?;
    }
    Ok(())
}

fn finalize_governance_state(
    transaction: &Transaction<'_>,
    request_id: &Identifier,
    component_type: radishmemory_core::DeletionComponentType,
    table: &str,
    id_column: &str,
    final_state: &str,
) -> Result<(), SqliteError> {
    let targets = load_execution_closure(transaction, request_id, component_type)?;
    let sql = format!("UPDATE {table} SET deletion_state = ?2 WHERE {id_column} = ?1");
    for target in targets {
        transaction
            .execute(&sql, params![target.object_id().as_str(), final_state])
            .map_err(SqliteError::storage)?;
    }
    Ok(())
}

fn require_closure_type(
    targets: &[ObjectRef],
    expected: CanonicalObjectType,
) -> Result<(), SqliteError> {
    if targets
        .iter()
        .any(|target| target.object_type() != expected)
    {
        return Err(SqliteError::deletion_invariant(
            SqliteStorageReason::DeletionExecution,
        ));
    }
    Ok(())
}
