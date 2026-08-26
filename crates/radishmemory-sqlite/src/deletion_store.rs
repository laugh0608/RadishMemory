use std::collections::{BTreeMap, BTreeSet};

use radishmemory_core::{
    ActorRef, ActorType, CanonicalObjectType, ComponentOutcome, ComponentResult,
    ComponentResultParams, ComponentStatus, DeleteRequest, DeleteRequestParams, DeletionEvidence,
    DeletionEvidenceParams, DeletionOverallStatus, DeletionStore, DeletionTarget,
    DeletionTargetRef, Digest, EvidenceRef, EvidenceType, FrozenTargetClosure, Identifier,
    LocalDeletionExecution, M0_SCHEMA_VERSION, NonEmptyText, ObjectRef, ProducerRef, ProducerType,
    RequestedGuarantee, RequiredAction, Timestamp, validate_deletion_evidence,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};

use crate::deletion_actions::{
    execute_component_action, failed_result, failed_result_with_code, successful_result,
};
use crate::source_store::{digest, identifier, non_empty_text, timestamp};
use crate::{SqliteDatabase, SqliteError, SqliteStorageReason};

#[derive(Clone, Copy)]
struct ProfileEntry {
    key: &'static str,
    component_type: radishmemory_core::DeletionComponentType,
    action: RequiredAction,
}

const PROFILE: [ProfileEntry; 10] = [
    ProfileEntry {
        key: "source-body",
        component_type: radishmemory_core::DeletionComponentType::SourceBody,
        action: RequiredAction::Delete,
    },
    ProfileEntry {
        key: "source-metadata",
        component_type: radishmemory_core::DeletionComponentType::SourceMetadata,
        action: RequiredAction::RetainMinimal,
    },
    ProfileEntry {
        key: "source-fragment",
        component_type: radishmemory_core::DeletionComponentType::SourceFragment,
        action: RequiredAction::Delete,
    },
    ProfileEntry {
        key: "memory-proposal",
        component_type: radishmemory_core::DeletionComponentType::MemoryProposal,
        action: RequiredAction::Redact,
    },
    ProfileEntry {
        key: "memory-decision",
        component_type: radishmemory_core::DeletionComponentType::MemoryDecision,
        action: RequiredAction::RetainMinimal,
    },
    ProfileEntry {
        key: "memory-record",
        component_type: radishmemory_core::DeletionComponentType::MemoryRecord,
        action: RequiredAction::Redact,
    },
    ProfileEntry {
        key: "memory-state-event",
        component_type: radishmemory_core::DeletionComponentType::MemoryStateEvent,
        action: RequiredAction::RetainMinimal,
    },
    ProfileEntry {
        key: "full-text-index",
        component_type: radishmemory_core::DeletionComponentType::FullTextIndex,
        action: RequiredAction::Delete,
    },
    ProfileEntry {
        key: "context-cache",
        component_type: radishmemory_core::DeletionComponentType::ContextCache,
        action: RequiredAction::Delete,
    },
    ProfileEntry {
        key: "minimal-audit",
        component_type: radishmemory_core::DeletionComponentType::MinimalAudit,
        action: RequiredAction::RetainMinimal,
    },
];

const EXECUTION_ORDER: [radishmemory_core::DeletionComponentType; 10] = [
    radishmemory_core::DeletionComponentType::MemoryProposal,
    radishmemory_core::DeletionComponentType::MemoryRecord,
    radishmemory_core::DeletionComponentType::SourceFragment,
    radishmemory_core::DeletionComponentType::SourceBody,
    radishmemory_core::DeletionComponentType::SourceMetadata,
    radishmemory_core::DeletionComponentType::MemoryDecision,
    radishmemory_core::DeletionComponentType::MemoryStateEvent,
    radishmemory_core::DeletionComponentType::FullTextIndex,
    radishmemory_core::DeletionComponentType::ContextCache,
    radishmemory_core::DeletionComponentType::MinimalAudit,
];

impl DeletionStore for SqliteDatabase {
    type Error = SqliteError;

    fn store_delete_request(&mut self, request: &DeleteRequest) -> Result<(), Self::Error> {
        validate_request_profile(request)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        validate_semantic_targets(&transaction, request)?;
        let execution_closure = build_execution_closure(&transaction, request)?;
        insert_request(&transaction, request, &execution_closure)?;
        close_targets_to_recall(&transaction, request, &execution_closure)?;
        crate::derived_index::verify(&transaction)?;
        transaction.commit().map_err(SqliteError::storage)
    }

    fn execute_deletion(
        &mut self,
        namespace_id: &Identifier,
        delete_request_id: &Identifier,
        execution: &LocalDeletionExecution,
    ) -> Result<Vec<ComponentResult>, Self::Error> {
        let request =
            load_request(&self.connection, namespace_id, delete_request_id)?.ok_or_else(|| {
                SqliteError::deletion_invariant(SqliteStorageReason::MissingDeleteRequest)
            })?;
        validate_request_profile(&request)?;
        execute_request(&mut self.connection, &request, execution)
    }

    fn store_deletion_evidence(&mut self, evidence: &DeletionEvidence) -> Result<(), Self::Error> {
        store_evidence(&mut self.connection, evidence)
    }

    fn load_delete_request(
        &self,
        namespace_id: &Identifier,
        delete_request_id: &Identifier,
    ) -> Result<Option<DeleteRequest>, Self::Error> {
        load_request(&self.connection, namespace_id, delete_request_id)
    }

    fn load_deletion_evidence(
        &self,
        namespace_id: &Identifier,
        deletion_evidence_id: &Identifier,
    ) -> Result<Option<DeletionEvidence>, Self::Error> {
        load_evidence(&self.connection, namespace_id, deletion_evidence_id)
    }
}

pub(crate) fn validate_request_profile(request: &DeleteRequest) -> Result<(), SqliteError> {
    let value = request.params();
    if value.requested_guarantee != RequestedGuarantee::LocalPurge
        || value.planned_components.len() != PROFILE.len()
        || value.target_refs.iter().any(|target| {
            !matches!(
                target.object_type(),
                CanonicalObjectType::SourceArtifact | CanonicalObjectType::MemoryRecord
            )
        })
    {
        return Err(deletion_plan());
    }

    let mut sorted_targets = value.target_refs.clone();
    sorted_targets.sort();
    let expected_target_ref = if sorted_targets.len() == 1 {
        DeletionTargetRef::Object(sorted_targets[0].clone())
    } else {
        DeletionTargetRef::FrozenClosure(
            FrozenTargetClosure::freeze(sorted_targets).map_err(deletion_core)?,
        )
    };
    let target_count = u64::try_from(value.target_refs.len()).map_err(|_| deletion_plan())?;

    for (actual, expected) in value.planned_components.iter().zip(PROFILE) {
        if actual.component_key().as_str() != expected.key
            || actual.component_type() != expected.component_type
            || actual.required_action() != expected.action
            || actual.target_count() != target_count
            || actual.target_ref() != &expected_target_ref
        {
            return Err(deletion_plan());
        }
    }
    Ok(())
}

fn validate_semantic_targets(
    connection: &Connection,
    request: &DeleteRequest,
) -> Result<(), SqliteError> {
    let namespace = request.params().namespace_id.as_str();
    let memory_targets = request
        .params()
        .target_refs
        .iter()
        .filter(|target| target.object_type() == CanonicalObjectType::MemoryRecord)
        .map(|target| target.object_id().as_str())
        .collect::<BTreeSet<_>>();

    for target in &request.params().target_refs {
        let exists: bool = match target.object_type() {
            CanonicalObjectType::SourceArtifact => connection
                .query_row(
                    "SELECT EXISTS (
                         SELECT 1 FROM radishmemory_source_artifacts
                         WHERE source_id = ?1 AND namespace_id = ?2 AND deletion_state = 'active'
                     )",
                    params![target.object_id().as_str(), namespace],
                    |row| row.get(0),
                )
                .map_err(SqliteError::storage)?,
            CanonicalObjectType::MemoryRecord => connection
                .query_row(
                    "SELECT EXISTS (
                         SELECT 1 FROM radishmemory_memory_records
                         WHERE memory_id = ?1 AND namespace_id = ?2 AND deletion_state = 'active'
                     )",
                    params![target.object_id().as_str(), namespace],
                    |row| row.get(0),
                )
                .map_err(SqliteError::storage)?,
            _ => false,
        };
        if !exists {
            return Err(SqliteError::deletion_invariant(
                SqliteStorageReason::MissingDeleteTarget,
            ));
        }
    }

    for target in request
        .params()
        .target_refs
        .iter()
        .filter(|target| target.object_type() == CanonicalObjectType::SourceArtifact)
    {
        let mut statement = connection
            .prepare(
                "SELECT DISTINCT r.memory_id
                 FROM radishmemory_memory_records AS r
                 JOIN radishmemory_record_source_fragments AS rs ON rs.memory_id = r.memory_id
                 JOIN radishmemory_source_fragments AS f ON f.fragment_id = rs.fragment_id
                 WHERE f.source_id = ?1 AND r.namespace_id = ?2 AND r.deletion_state = 'active'",
            )
            .map_err(SqliteError::storage)?;
        let linked = statement
            .query_map(params![target.object_id().as_str(), namespace], |row| {
                row.get::<_, String>(0)
            })
            .map_err(SqliteError::storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SqliteError::storage)?;
        if linked
            .iter()
            .any(|memory_id| !memory_targets.contains(memory_id.as_str()))
        {
            return Err(deletion_plan());
        }
    }
    for target in request
        .params()
        .target_refs
        .iter()
        .filter(|target| target.object_type() == CanonicalObjectType::MemoryRecord)
    {
        let mut statement = connection
            .prepare(
                "SELECT r.memory_id
                 FROM radishmemory_record_supersedes AS s
                 JOIN radishmemory_memory_records AS r ON r.memory_id = s.memory_id
                 WHERE s.superseded_memory_id = ?1
                   AND r.namespace_id = ?2 AND r.deletion_state = 'active'",
            )
            .map_err(SqliteError::storage)?;
        let dependents = statement
            .query_map(params![target.object_id().as_str(), namespace], |row| {
                row.get::<_, String>(0)
            })
            .map_err(SqliteError::storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SqliteError::storage)?;
        if dependents
            .iter()
            .any(|memory_id| !memory_targets.contains(memory_id.as_str()))
        {
            return Err(deletion_plan());
        }
    }
    Ok(())
}

type ExecutionClosure = BTreeMap<&'static str, BTreeSet<ObjectRef>>;

fn build_execution_closure(
    connection: &Connection,
    request: &DeleteRequest,
) -> Result<ExecutionClosure, SqliteError> {
    let mut closure = PROFILE
        .iter()
        .map(|entry| (component_type_str(entry.component_type), BTreeSet::new()))
        .collect::<ExecutionClosure>();
    let namespace = request.params().namespace_id.as_str();
    let source_ids = request
        .params()
        .target_refs
        .iter()
        .filter(|target| target.object_type() == CanonicalObjectType::SourceArtifact)
        .map(|target| target.object_id().clone())
        .collect::<Vec<_>>();
    let memory_ids = request
        .params()
        .target_refs
        .iter()
        .filter(|target| target.object_type() == CanonicalObjectType::MemoryRecord)
        .map(|target| target.object_id().clone())
        .collect::<Vec<_>>();

    for source_id in &source_ids {
        insert_closure_ref(
            &mut closure,
            radishmemory_core::DeletionComponentType::SourceBody,
            CanonicalObjectType::SourceArtifact,
            source_id.clone(),
        );
        insert_closure_ref(
            &mut closure,
            radishmemory_core::DeletionComponentType::SourceMetadata,
            CanonicalObjectType::SourceArtifact,
            source_id.clone(),
        );
        for fragment_id in query_ids(
            connection,
            "SELECT fragment_id FROM radishmemory_source_fragments
             WHERE source_id = ?1 AND namespace_id = ?2 AND deletion_state = 'active'
             ORDER BY fragment_id",
            source_id.as_str(),
            namespace,
        )? {
            let fragment_id = identifier(fragment_id)?;
            insert_closure_ref(
                &mut closure,
                radishmemory_core::DeletionComponentType::SourceFragment,
                CanonicalObjectType::SourceFragment,
                fragment_id.clone(),
            );
            insert_closure_ref(
                &mut closure,
                radishmemory_core::DeletionComponentType::FullTextIndex,
                CanonicalObjectType::SourceFragment,
                fragment_id,
            );
        }
    }

    let fragment_ids = closure_refs(
        &closure,
        radishmemory_core::DeletionComponentType::SourceFragment,
    )?
    .iter()
    .map(|target| target.object_id().clone())
    .collect::<Vec<_>>();
    let mut proposal_ids = BTreeSet::new();
    for memory_id in &memory_ids {
        let proposal_id: String = connection
            .query_row(
                "SELECT origin_proposal_id FROM radishmemory_memory_records
                 WHERE memory_id = ?1 AND namespace_id = ?2 AND deletion_state = 'active'",
                params![memory_id.as_str(), namespace],
                |row| row.get(0),
            )
            .map_err(SqliteError::storage)?;
        proposal_ids.insert(identifier(proposal_id)?);
        insert_closure_ref(
            &mut closure,
            radishmemory_core::DeletionComponentType::MemoryRecord,
            CanonicalObjectType::MemoryRecord,
            memory_id.clone(),
        );
        insert_closure_ref(
            &mut closure,
            radishmemory_core::DeletionComponentType::FullTextIndex,
            CanonicalObjectType::MemoryRecord,
            memory_id.clone(),
        );
    }
    for fragment_id in &fragment_ids {
        for proposal_id in query_ids(
            connection,
            "SELECT DISTINCT p.proposal_id
             FROM radishmemory_memory_proposals AS p
             JOIN radishmemory_proposal_source_fragments AS ps ON ps.proposal_id = p.proposal_id
             WHERE ps.fragment_id = ?1 AND p.namespace_id = ?2 AND p.deletion_state = 'active'
             ORDER BY p.proposal_id",
            fragment_id.as_str(),
            namespace,
        )? {
            proposal_ids.insert(identifier(proposal_id)?);
        }
    }

    for proposal_id in &proposal_ids {
        insert_closure_ref(
            &mut closure,
            radishmemory_core::DeletionComponentType::MemoryProposal,
            CanonicalObjectType::MemoryProposal,
            proposal_id.clone(),
        );
        for decision_id in query_ids(
            connection,
            "SELECT decision_id FROM radishmemory_memory_decisions
             WHERE proposal_id = ?1 AND namespace_id = ?2 ORDER BY decision_id",
            proposal_id.as_str(),
            namespace,
        )? {
            insert_closure_ref(
                &mut closure,
                radishmemory_core::DeletionComponentType::MemoryDecision,
                CanonicalObjectType::MemoryDecision,
                identifier(decision_id)?,
            );
        }
    }
    for memory_id in &memory_ids {
        for event_id in query_ids(
            connection,
            "SELECT event_id FROM radishmemory_memory_state_events
             WHERE memory_id = ?1 AND namespace_id = ?2 ORDER BY event_id",
            memory_id.as_str(),
            namespace,
        )? {
            insert_closure_ref(
                &mut closure,
                radishmemory_core::DeletionComponentType::MemoryStateEvent,
                CanonicalObjectType::MemoryStateEvent,
                identifier(event_id)?,
            );
        }
    }
    insert_closure_ref(
        &mut closure,
        radishmemory_core::DeletionComponentType::MinimalAudit,
        CanonicalObjectType::DeleteRequest,
        request.params().delete_request_id.clone(),
    );
    Ok(closure)
}

fn insert_closure_ref(
    closure: &mut ExecutionClosure,
    component_type: radishmemory_core::DeletionComponentType,
    object_type: CanonicalObjectType,
    object_id: Identifier,
) {
    closure
        .entry(component_type_str(component_type))
        .or_default()
        .insert(ObjectRef::new(object_type, object_id));
}

fn closure_refs(
    closure: &ExecutionClosure,
    component_type: radishmemory_core::DeletionComponentType,
) -> Result<&BTreeSet<ObjectRef>, SqliteError> {
    closure
        .get(component_type_str(component_type))
        .ok_or_else(deletion_plan)
}

fn query_ids(
    connection: &Connection,
    sql: &str,
    first: &str,
    second: &str,
) -> Result<Vec<String>, SqliteError> {
    let mut statement = connection.prepare(sql).map_err(SqliteError::storage)?;
    statement
        .query_map(params![first, second], |row| row.get::<_, String>(0))
        .map_err(SqliteError::storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SqliteError::storage)
}

fn insert_request(
    transaction: &Transaction<'_>,
    request: &DeleteRequest,
    closure: &ExecutionClosure,
) -> Result<(), SqliteError> {
    let value = request.params();
    transaction
        .execute(
            "INSERT INTO radishmemory_delete_requests (
                 delete_request_id, canonical_schema_version, object_type, namespace_id,
                 requested_by_type, requested_by_id, requested_by_version, authorization_basis,
                 requested_guarantee, scope, device_id, reason_code, requested_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'local_device', ?10, ?11, ?12)",
            params![
                value.delete_request_id.as_str(),
                M0_SCHEMA_VERSION,
                CanonicalObjectType::DeleteRequest.as_str(),
                value.namespace_id.as_str(),
                actor_type_str(value.requested_by.actor_type()),
                value.requested_by.actor_id().as_str(),
                value.requested_by.actor_version().map(NonEmptyText::as_str),
                value.authorization_basis.as_str(),
                requested_guarantee_str(value.requested_guarantee),
                value.device_id.as_str(),
                value.reason_code.as_str(),
                value.requested_at.original(),
            ],
        )
        .map_err(SqliteError::storage)?;

    for (ordinal, target) in value.target_refs.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO radishmemory_delete_request_targets
                 (delete_request_id, ordinal, object_type, object_id) VALUES (?1, ?2, ?3, ?4)",
                params![
                    value.delete_request_id.as_str(),
                    to_i64(ordinal)?,
                    target.object_type().as_str(),
                    target.object_id().as_str(),
                ],
            )
            .map_err(SqliteError::storage)?;
    }
    for (ordinal, component) in value.planned_components.iter().enumerate() {
        let (kind, digest) = match component.target_ref() {
            DeletionTargetRef::Object(_) => ("object", None),
            DeletionTargetRef::FrozenClosure(value) => {
                ("frozen_closure", Some(value.target_refs_digest()))
            }
        };
        transaction
            .execute(
                "INSERT INTO radishmemory_delete_request_components (
                     delete_request_id, ordinal, component_key, component_type, target_ref_kind,
                     target_count, required_action, target_digest_algorithm,
                     target_digest_profile, target_digest_value
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    value.delete_request_id.as_str(),
                    to_i64(ordinal)?,
                    component.component_key().as_str(),
                    component_type_str(component.component_type()),
                    kind,
                    to_i64_u64(component.target_count())?,
                    required_action_str(component.required_action()),
                    digest.map(Digest::algorithm),
                    digest.map(|value| value.profile().as_str()),
                    digest.map(Digest::value),
                ],
            )
            .map_err(SqliteError::storage)?;
        for (target_ordinal, target) in target_refs(component.target_ref()).iter().enumerate() {
            transaction
                .execute(
                    "INSERT INTO radishmemory_delete_component_targets
                     (delete_request_id, component_key, ordinal, object_type, object_id)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        value.delete_request_id.as_str(),
                        component.component_key().as_str(),
                        to_i64(target_ordinal)?,
                        target.object_type().as_str(),
                        target.object_id().as_str(),
                    ],
                )
                .map_err(SqliteError::storage)?;
        }
    }
    for entry in PROFILE {
        for (ordinal, target) in closure_refs(closure, entry.component_type)?
            .iter()
            .enumerate()
        {
            transaction
                .execute(
                    "INSERT INTO radishmemory_delete_execution_closure
                     (delete_request_id, component_type, ordinal, object_type, object_id)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        value.delete_request_id.as_str(),
                        component_type_str(entry.component_type),
                        to_i64(ordinal)?,
                        target.object_type().as_str(),
                        target.object_id().as_str(),
                    ],
                )
                .map_err(SqliteError::storage)?;
        }
    }
    Ok(())
}

fn close_targets_to_recall(
    transaction: &Transaction<'_>,
    request: &DeleteRequest,
    closure: &ExecutionClosure,
) -> Result<(), SqliteError> {
    let namespace = request.params().namespace_id.as_str();
    for target in &request.params().target_refs {
        let changed = match target.object_type() {
            CanonicalObjectType::SourceArtifact => transaction.execute(
                "UPDATE radishmemory_source_artifacts SET deletion_state = 'pending'
                 WHERE source_id = ?1 AND namespace_id = ?2 AND deletion_state = 'active'",
                params![target.object_id().as_str(), namespace],
            ),
            CanonicalObjectType::MemoryRecord => transaction.execute(
                "UPDATE radishmemory_memory_records SET deletion_state = 'pending'
                 WHERE memory_id = ?1 AND namespace_id = ?2 AND deletion_state = 'active'",
                params![target.object_id().as_str(), namespace],
            ),
            _ => return Err(deletion_plan()),
        }
        .map_err(SqliteError::storage)?;
        if changed != 1 {
            return Err(SqliteError::deletion_invariant(
                SqliteStorageReason::MissingDeleteTarget,
            ));
        }
    }

    for target in closure_refs(
        closure,
        radishmemory_core::DeletionComponentType::SourceFragment,
    )? {
        transaction
            .execute(
                "UPDATE radishmemory_source_fragments SET deletion_state = 'pending'
                 WHERE fragment_id = ?1 AND namespace_id = ?2 AND deletion_state = 'active'",
                params![target.object_id().as_str(), namespace],
            )
            .map_err(SqliteError::storage)?;
    }
    for target in closure_refs(
        closure,
        radishmemory_core::DeletionComponentType::MemoryProposal,
    )? {
        transaction
            .execute(
                "UPDATE radishmemory_memory_proposals SET deletion_state = 'pending'
                 WHERE proposal_id = ?1 AND namespace_id = ?2 AND deletion_state = 'active'",
                params![target.object_id().as_str(), namespace],
            )
            .map_err(SqliteError::storage)?;
    }
    for target in closure_refs(
        closure,
        radishmemory_core::DeletionComponentType::FullTextIndex,
    )? {
        transaction
            .execute(
                "DELETE FROM radishmemory_recall_fts WHERE object_kind = ?1 AND object_id = ?2",
                params![
                    recall_kind(target.object_type())?,
                    target.object_id().as_str()
                ],
            )
            .map_err(SqliteError::storage)?;
    }
    for target in request
        .params()
        .target_refs
        .iter()
        .filter(|target| target.object_type() == CanonicalObjectType::MemoryRecord)
    {
        transaction
            .execute(
                "DELETE FROM radishmemory_memory_current_projection WHERE memory_id = ?1",
                params![target.object_id().as_str()],
            )
            .map_err(SqliteError::storage)?;
    }
    Ok(())
}

fn target_refs(target_ref: &DeletionTargetRef) -> Vec<&ObjectRef> {
    match target_ref {
        DeletionTargetRef::Object(value) => vec![value],
        DeletionTargetRef::FrozenClosure(value) => value.target_refs().iter().collect(),
    }
}

fn load_request(
    connection: &Connection,
    namespace_id: &Identifier,
    delete_request_id: &Identifier,
) -> Result<Option<DeleteRequest>, SqliteError> {
    let stored = connection
        .query_row(
            "SELECT canonical_schema_version, object_type, namespace_id, requested_by_type,
                    requested_by_id, requested_by_version, authorization_basis,
                    requested_guarantee, scope, device_id, reason_code, requested_at
             FROM radishmemory_delete_requests
             WHERE delete_request_id = ?1 AND namespace_id = ?2",
            params![delete_request_id.as_str(), namespace_id.as_str()],
            StoredRequest::from_row,
        )
        .optional()
        .map_err(SqliteError::storage)?;
    let Some(stored) = stored else {
        return Ok(None);
    };
    stored.into_domain(connection, delete_request_id).map(Some)
}

struct StoredRequest {
    schema_version: String,
    object_type: String,
    namespace_id: String,
    actor_type: String,
    actor_id: String,
    actor_version: Option<String>,
    authorization_basis: String,
    requested_guarantee: String,
    scope: String,
    device_id: String,
    reason_code: String,
    requested_at: String,
}

impl StoredRequest {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            schema_version: row.get(0)?,
            object_type: row.get(1)?,
            namespace_id: row.get(2)?,
            actor_type: row.get(3)?,
            actor_id: row.get(4)?,
            actor_version: row.get(5)?,
            authorization_basis: row.get(6)?,
            requested_guarantee: row.get(7)?,
            scope: row.get(8)?,
            device_id: row.get(9)?,
            reason_code: row.get(10)?,
            requested_at: row.get(11)?,
        })
    }

    fn into_domain(
        self,
        connection: &Connection,
        delete_request_id: &Identifier,
    ) -> Result<DeleteRequest, SqliteError> {
        if self.schema_version != M0_SCHEMA_VERSION
            || self.object_type != CanonicalObjectType::DeleteRequest.as_str()
            || self.scope != "local_device"
        {
            return Err(stored_deletion());
        }
        let target_refs = load_request_targets(connection, delete_request_id, None)?;
        let planned_components = load_request_components(connection, delete_request_id)?;
        DeleteRequest::new(DeleteRequestParams {
            delete_request_id: delete_request_id.clone(),
            namespace_id: identifier(self.namespace_id)?,
            requested_by: ActorRef::new(
                parse_actor_type(&self.actor_type)?,
                identifier(self.actor_id)?,
                self.actor_version.map(non_empty_text).transpose()?,
            ),
            authorization_basis: non_empty_text(self.authorization_basis)?,
            requested_guarantee: parse_requested_guarantee(&self.requested_guarantee)?,
            device_id: identifier(self.device_id)?,
            target_refs,
            planned_components,
            reason_code: non_empty_text(self.reason_code)?,
            requested_at: timestamp(&self.requested_at)?,
        })
        .map_err(deletion_core)
    }
}

fn load_request_targets(
    connection: &Connection,
    delete_request_id: &Identifier,
    component_key: Option<&Identifier>,
) -> Result<Vec<ObjectRef>, SqliteError> {
    let (sql, second) = if let Some(component_key) = component_key {
        (
            "SELECT ordinal, object_type, object_id
             FROM radishmemory_delete_component_targets
             WHERE delete_request_id = ?1 AND component_key = ?2 ORDER BY ordinal",
            Some(component_key.as_str()),
        )
    } else {
        (
            "SELECT ordinal, object_type, object_id
             FROM radishmemory_delete_request_targets
             WHERE delete_request_id = ?1 ORDER BY ordinal",
            None,
        )
    };
    let mut statement = connection.prepare(sql).map_err(SqliteError::storage)?;
    let rows = if let Some(second) = second {
        statement
            .query_map(
                params![delete_request_id.as_str(), second],
                stored_object_ref,
            )
            .map_err(SqliteError::storage)?
            .collect::<Result<Vec<_>, _>>()
    } else {
        statement
            .query_map(params![delete_request_id.as_str()], stored_object_ref)
            .map_err(SqliteError::storage)?
            .collect::<Result<Vec<_>, _>>()
    }
    .map_err(SqliteError::storage)?;
    decode_ordered_object_refs(rows)
}

pub(crate) fn stored_object_ref(row: &Row<'_>) -> rusqlite::Result<(i64, String, String)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
}

pub(crate) fn decode_ordered_object_refs(
    rows: Vec<(i64, String, String)>,
) -> Result<Vec<ObjectRef>, SqliteError> {
    let mut values = Vec::with_capacity(rows.len());
    for (expected, (ordinal, object_type, object_id)) in rows.into_iter().enumerate() {
        if ordinal != to_i64(expected)? {
            return Err(stored_deletion());
        }
        values.push(ObjectRef::new(
            parse_object_type(&object_type)?,
            identifier(object_id)?,
        ));
    }
    Ok(values)
}

fn load_request_components(
    connection: &Connection,
    delete_request_id: &Identifier,
) -> Result<Vec<DeletionTarget>, SqliteError> {
    let mut statement = connection
        .prepare(
            "SELECT ordinal, component_key, component_type, target_ref_kind, target_count,
                    required_action, target_digest_algorithm, target_digest_profile,
                    target_digest_value
             FROM radishmemory_delete_request_components
             WHERE delete_request_id = ?1 ORDER BY ordinal",
        )
        .map_err(SqliteError::storage)?;
    let rows = statement
        .query_map(params![delete_request_id.as_str()], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
            ))
        })
        .map_err(SqliteError::storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SqliteError::storage)?;
    drop(statement);

    let mut components = Vec::with_capacity(rows.len());
    for (expected, row) in rows.into_iter().enumerate() {
        let (ordinal, key, component_type, kind, target_count, action, algorithm, profile, value) =
            row;
        if ordinal != to_i64(expected)? {
            return Err(stored_deletion());
        }
        let key = identifier(key)?;
        let targets = load_request_targets(connection, delete_request_id, Some(&key))?;
        let target_ref = match (kind.as_str(), algorithm, profile, value) {
            ("object", None, None, None) if targets.len() == 1 => {
                DeletionTargetRef::Object(targets[0].clone())
            }
            ("frozen_closure", Some(algorithm), Some(profile), Some(value)) => {
                DeletionTargetRef::FrozenClosure(
                    FrozenTargetClosure::new(targets, digest(&algorithm, &profile, &value)?)
                        .map_err(deletion_core)?,
                )
            }
            _ => return Err(stored_deletion()),
        };
        components.push(
            DeletionTarget::new(
                key,
                parse_component_type(&component_type)?,
                target_ref,
                from_i64_u64(target_count)?,
                parse_required_action(&action)?,
            )
            .map_err(deletion_core)?,
        );
    }
    Ok(components)
}

fn execute_request(
    connection: &mut Connection,
    request: &DeleteRequest,
    execution: &LocalDeletionExecution,
) -> Result<Vec<ComponentResult>, SqliteError> {
    execute_request_inner(connection, request, execution, None)
}

#[cfg(feature = "fixture-runner")]
pub(crate) fn execute_request_with_fixture_failure(
    connection: &mut Connection,
    request: &DeleteRequest,
    execution: &LocalDeletionExecution,
    component_key: &Identifier,
    error_code: &NonEmptyText,
    retryable: bool,
) -> Result<Vec<ComponentResult>, SqliteError> {
    execute_request_inner(
        connection,
        request,
        execution,
        Some(InjectedFailure {
            component_key,
            error_code,
            retryable,
        }),
    )
}

struct InjectedFailure<'a> {
    component_key: &'a Identifier,
    error_code: &'a NonEmptyText,
    retryable: bool,
}

fn execute_request_inner(
    connection: &mut Connection,
    request: &DeleteRequest,
    execution: &LocalDeletionExecution,
    fixture_failure: Option<InjectedFailure<'_>>,
) -> Result<Vec<ComponentResult>, SqliteError> {
    let request_id = &request.params().delete_request_id;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(SqliteError::storage)?;
    let attempt_ordinal: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(attempt_ordinal), 0) + 1
             FROM radishmemory_deletion_execution_attempts WHERE delete_request_id = ?1",
            params![request_id.as_str()],
            |row| row.get(0),
        )
        .map_err(SqliteError::storage)?;
    transaction
        .execute(
            "INSERT INTO radishmemory_deletion_execution_attempts
             (delete_request_id, attempt_ordinal, checked_at) VALUES (?1, ?2, ?3)",
            params![
                request_id.as_str(),
                attempt_ordinal,
                execution.checked_at().original()
            ],
        )
        .map_err(SqliteError::storage)?;
    transaction.commit().map_err(SqliteError::storage)?;

    let mut has_failed = false;
    for component_type in EXECUTION_ORDER {
        let component = request
            .params()
            .planned_components
            .iter()
            .find(|component| component.component_type() == component_type)
            .ok_or_else(deletion_plan)?;

        let inject_failure = fixture_failure
            .as_ref()
            .is_some_and(|failure| failure.component_key == component.component_key());
        let successful = if inject_failure {
            None
        } else {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(SqliteError::storage)?;
            match execute_component_action(
                &transaction,
                request,
                component_type,
                attempt_ordinal,
                has_failed,
            ) {
                Ok(action) => {
                    let result = successful_result(component, execution, action)?;
                    insert_execution_result(&transaction, request_id, attempt_ordinal, &result)?;
                    transaction.commit().map_err(SqliteError::storage)?;
                    Some(result)
                }
                Err(_component_failure) => None,
            }
        };

        if successful.is_none() {
            has_failed = true;
            let result = if let Some(failure) = fixture_failure.as_ref().filter(|_| inject_failure)
            {
                failed_result_with_code(
                    component,
                    execution,
                    failure.error_code.clone(),
                    failure.retryable,
                )?
            } else {
                failed_result(component, execution)?
            };
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(SqliteError::storage)?;
            insert_execution_result(&transaction, request_id, attempt_ordinal, &result)?;
            transaction.commit().map_err(SqliteError::storage)?;
        }
    }

    load_execution_results(connection, request, attempt_ordinal)
}

fn insert_execution_result(
    transaction: &Transaction<'_>,
    request_id: &Identifier,
    attempt_ordinal: i64,
    result: &ComponentResult,
) -> Result<(), SqliteError> {
    let value = result.params();
    transaction
        .execute(
            "INSERT INTO radishmemory_deletion_execution_results (
                 delete_request_id, attempt_ordinal, component_key, processed_count, status,
                 outcome, verification_method, checked_at, error_code, retryable,
                 retention_basis_type, retention_basis_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                request_id.as_str(),
                attempt_ordinal,
                value.component_key.as_str(),
                to_i64_u64(value.processed_count)?,
                component_status_str(value.status),
                component_outcome_str(value.outcome),
                value.verification_method.as_str(),
                value.checked_at.original(),
                value.error_code.as_ref().map(NonEmptyText::as_str),
                value.retryable,
                value
                    .retention_basis
                    .as_ref()
                    .map(|basis| evidence_type_str(basis.evidence_type())),
                value
                    .retention_basis
                    .as_ref()
                    .map(|basis| basis.evidence_id().as_str()),
            ],
        )
        .map_err(SqliteError::storage)?;
    Ok(())
}

fn load_execution_results(
    connection: &Connection,
    request: &DeleteRequest,
    attempt_ordinal: i64,
) -> Result<Vec<ComponentResult>, SqliteError> {
    let mut results = Vec::with_capacity(request.params().planned_components.len());
    for component in &request.params().planned_components {
        let stored = connection
            .query_row(
                "SELECT processed_count, status, outcome, verification_method, checked_at,
                        error_code, retryable, retention_basis_type, retention_basis_id
                 FROM radishmemory_deletion_execution_results
                 WHERE delete_request_id = ?1 AND attempt_ordinal = ?2 AND component_key = ?3",
                params![
                    request.params().delete_request_id.as_str(),
                    attempt_ordinal,
                    component.component_key().as_str()
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<bool>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                    ))
                },
            )
            .optional()
            .map_err(SqliteError::storage)?
            .ok_or_else(stored_deletion)?;
        let retention_basis = match (stored.7, stored.8) {
            (Some(kind), Some(id)) => Some(EvidenceRef::new(
                parse_evidence_type(&kind)?,
                identifier(id)?,
            )),
            (None, None) => None,
            _ => return Err(stored_deletion()),
        };
        results.push(
            ComponentResult::new(ComponentResultParams {
                component_key: component.component_key().clone(),
                component_type: component.component_type(),
                target_ref: component.target_ref().clone(),
                required_action: component.required_action(),
                target_count: component.target_count(),
                processed_count: from_i64_u64(stored.0)?,
                status: parse_component_status(&stored.1)?,
                outcome: parse_component_outcome(&stored.2)?,
                verification_method: non_empty_text(stored.3)?,
                checked_at: timestamp(&stored.4)?,
                error_code: stored.5.map(non_empty_text).transpose()?,
                retryable: stored.6,
                retention_basis,
            })
            .map_err(deletion_core)?,
        );
    }
    Ok(results)
}

fn store_evidence(
    connection: &mut Connection,
    evidence: &DeletionEvidence,
) -> Result<(), SqliteError> {
    let value = evidence.params();
    let request = load_request(connection, &value.namespace_id, &value.delete_request_id)?
        .ok_or_else(|| {
            SqliteError::deletion_invariant(SqliteStorageReason::MissingDeleteRequest)
        })?;
    validate_request_profile(&request)?;
    validate_deletion_evidence(&request, evidence).map_err(deletion_core)?;

    let latest_attempt: i64 = connection
        .query_row(
            "SELECT MAX(attempt_ordinal) FROM radishmemory_deletion_execution_attempts
             WHERE delete_request_id = ?1",
            params![value.delete_request_id.as_str()],
            |row| row.get::<_, Option<i64>>(0),
        )
        .map_err(SqliteError::storage)?
        .ok_or_else(|| SqliteError::deletion_invariant(SqliteStorageReason::DeletionExecution))?;
    let persisted_results = load_execution_results(connection, &request, latest_attempt)?;
    if persisted_results != value.component_results {
        return Err(SqliteError::deletion_invariant(
            SqliteStorageReason::DeletionPlan,
        ));
    }
    let expected_status = if persisted_results
        .iter()
        .all(|result| result.params().status == ComponentStatus::Succeeded)
    {
        DeletionOverallStatus::Completed
    } else {
        DeletionOverallStatus::Failed
    };
    if value.overall_status != expected_status || value.finished_at.is_none() {
        return Err(SqliteError::deletion_invariant(
            SqliteStorageReason::DeletionExecution,
        ));
    }

    let previous_tip = connection
        .query_row(
            "SELECT deletion_evidence_id FROM radishmemory_deletion_evidence
             WHERE delete_request_id = ?1 ORDER BY execution_ordinal DESC LIMIT 1",
            params![value.delete_request_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(SqliteError::storage)?;
    if previous_tip.as_deref() != value.previous_evidence_id.as_ref().map(Identifier::as_str) {
        return Err(SqliteError::deletion_invariant(
            SqliteStorageReason::EvidenceChain,
        ));
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(SqliteError::storage)?;
    transaction
        .execute(
            "INSERT INTO radishmemory_deletion_evidence (
                 deletion_evidence_id, canonical_schema_version, object_type,
                 delete_request_id, execution_ordinal, previous_evidence_id, namespace_id,
                 scope, device_id, overall_status, started_at, finished_at, verified_by_type,
                 verified_by_id, verified_by_version, evidence_digest_algorithm,
                 evidence_digest_profile, evidence_digest_value
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, 'local_device', ?8, ?9, ?10, ?11,
                 ?12, ?13, ?14, ?15, ?16, ?17
             )",
            params![
                value.deletion_evidence_id.as_str(),
                M0_SCHEMA_VERSION,
                CanonicalObjectType::DeletionEvidence.as_str(),
                value.delete_request_id.as_str(),
                latest_attempt,
                value.previous_evidence_id.as_ref().map(Identifier::as_str),
                value.namespace_id.as_str(),
                value.device_id.as_str(),
                overall_status_str(value.overall_status),
                value.started_at.original(),
                value.finished_at.as_ref().map(Timestamp::original),
                producer_type_str(value.verified_by.producer_type()),
                value.verified_by.producer_id().as_str(),
                value.verified_by.producer_version().as_str(),
                value.evidence_digest.algorithm(),
                value.evidence_digest.profile().as_str(),
                value.evidence_digest.value(),
            ],
        )
        .map_err(SqliteError::storage)?;
    transaction.commit().map_err(SqliteError::storage)
}

fn load_evidence(
    connection: &Connection,
    namespace_id: &Identifier,
    deletion_evidence_id: &Identifier,
) -> Result<Option<DeletionEvidence>, SqliteError> {
    let stored = connection
        .query_row(
            "SELECT canonical_schema_version, object_type, delete_request_id,
                    execution_ordinal, previous_evidence_id, namespace_id, scope, device_id,
                    overall_status, started_at, finished_at, verified_by_type, verified_by_id,
                    verified_by_version, evidence_digest_algorithm, evidence_digest_profile,
                    evidence_digest_value
             FROM radishmemory_deletion_evidence
             WHERE deletion_evidence_id = ?1 AND namespace_id = ?2",
            params![deletion_evidence_id.as_str(), namespace_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                    row.get::<_, String>(14)?,
                    row.get::<_, String>(15)?,
                    row.get::<_, String>(16)?,
                ))
            },
        )
        .optional()
        .map_err(SqliteError::storage)?;
    let Some(stored) = stored else {
        return Ok(None);
    };
    if stored.0 != M0_SCHEMA_VERSION
        || stored.1 != CanonicalObjectType::DeletionEvidence.as_str()
        || stored.6 != "local_device"
    {
        return Err(stored_deletion());
    }
    let request_id = identifier(stored.2)?;
    let request =
        load_request(connection, namespace_id, &request_id)?.ok_or_else(stored_deletion)?;
    let component_results = load_execution_results(connection, &request, stored.3)?;
    let evidence = DeletionEvidence::new(DeletionEvidenceParams {
        deletion_evidence_id: deletion_evidence_id.clone(),
        delete_request_id: request_id,
        previous_evidence_id: stored.4.map(identifier).transpose()?,
        namespace_id: identifier(stored.5)?,
        device_id: identifier(stored.7)?,
        overall_status: parse_overall_status(&stored.8)?,
        component_results,
        started_at: timestamp(&stored.9)?,
        finished_at: stored.10.as_deref().map(timestamp).transpose()?,
        verified_by: ProducerRef::new(
            parse_producer_type(&stored.11)?,
            identifier(stored.12)?,
            non_empty_text(stored.13)?,
        ),
        evidence_digest: digest(&stored.14, &stored.15, &stored.16)?,
    })
    .map_err(deletion_core)?;
    validate_deletion_evidence(&request, &evidence).map_err(deletion_core)?;
    Ok(Some(evidence))
}

fn actor_type_str(value: ActorType) -> &'static str {
    match value {
        ActorType::User => "user",
        ActorType::Device => "device",
        ActorType::Rule => "rule",
        ActorType::Parser => "parser",
        ActorType::TestFixture => "test_fixture",
        ActorType::System => "system",
    }
}

fn parse_actor_type(value: &str) -> Result<ActorType, SqliteError> {
    match value {
        "user" => Ok(ActorType::User),
        "device" => Ok(ActorType::Device),
        "rule" => Ok(ActorType::Rule),
        "parser" => Ok(ActorType::Parser),
        "test_fixture" => Ok(ActorType::TestFixture),
        "system" => Ok(ActorType::System),
        _ => Err(stored_deletion()),
    }
}

fn producer_type_str(value: ProducerType) -> &'static str {
    match value {
        ProducerType::Rule => "rule",
        ProducerType::Parser => "parser",
        ProducerType::TestFixture => "test_fixture",
        ProducerType::System => "system",
    }
}

fn parse_producer_type(value: &str) -> Result<ProducerType, SqliteError> {
    match value {
        "rule" => Ok(ProducerType::Rule),
        "parser" => Ok(ProducerType::Parser),
        "test_fixture" => Ok(ProducerType::TestFixture),
        "system" => Ok(ProducerType::System),
        _ => Err(stored_deletion()),
    }
}

fn requested_guarantee_str(value: RequestedGuarantee) -> &'static str {
    match value {
        RequestedGuarantee::StopRecall => "stop_recall",
        RequestedGuarantee::LocalPurge => "local_purge",
    }
}

fn parse_requested_guarantee(value: &str) -> Result<RequestedGuarantee, SqliteError> {
    match value {
        "stop_recall" => Ok(RequestedGuarantee::StopRecall),
        "local_purge" => Ok(RequestedGuarantee::LocalPurge),
        _ => Err(stored_deletion()),
    }
}

pub(crate) fn component_type_str(value: radishmemory_core::DeletionComponentType) -> &'static str {
    match value {
        radishmemory_core::DeletionComponentType::SourceBody => "source_body",
        radishmemory_core::DeletionComponentType::SourceMetadata => "source_metadata",
        radishmemory_core::DeletionComponentType::SourceFragment => "source_fragment",
        radishmemory_core::DeletionComponentType::MemoryProposal => "memory_proposal",
        radishmemory_core::DeletionComponentType::MemoryDecision => "memory_decision",
        radishmemory_core::DeletionComponentType::MemoryRecord => "memory_record",
        radishmemory_core::DeletionComponentType::MemoryStateEvent => "memory_state_event",
        radishmemory_core::DeletionComponentType::FullTextIndex => "full_text_index",
        radishmemory_core::DeletionComponentType::ContextCache => "context_cache",
        radishmemory_core::DeletionComponentType::MinimalAudit => "minimal_audit",
    }
}

fn parse_component_type(
    value: &str,
) -> Result<radishmemory_core::DeletionComponentType, SqliteError> {
    match value {
        "source_body" => Ok(radishmemory_core::DeletionComponentType::SourceBody),
        "source_metadata" => Ok(radishmemory_core::DeletionComponentType::SourceMetadata),
        "source_fragment" => Ok(radishmemory_core::DeletionComponentType::SourceFragment),
        "memory_proposal" => Ok(radishmemory_core::DeletionComponentType::MemoryProposal),
        "memory_decision" => Ok(radishmemory_core::DeletionComponentType::MemoryDecision),
        "memory_record" => Ok(radishmemory_core::DeletionComponentType::MemoryRecord),
        "memory_state_event" => Ok(radishmemory_core::DeletionComponentType::MemoryStateEvent),
        "full_text_index" => Ok(radishmemory_core::DeletionComponentType::FullTextIndex),
        "context_cache" => Ok(radishmemory_core::DeletionComponentType::ContextCache),
        "minimal_audit" => Ok(radishmemory_core::DeletionComponentType::MinimalAudit),
        _ => Err(stored_deletion()),
    }
}

fn required_action_str(value: RequiredAction) -> &'static str {
    match value {
        RequiredAction::Delete => "delete",
        RequiredAction::Redact => "redact",
        RequiredAction::RetainMinimal => "retain_minimal",
    }
}

fn parse_required_action(value: &str) -> Result<RequiredAction, SqliteError> {
    match value {
        "delete" => Ok(RequiredAction::Delete),
        "redact" => Ok(RequiredAction::Redact),
        "retain_minimal" => Ok(RequiredAction::RetainMinimal),
        _ => Err(stored_deletion()),
    }
}

fn component_status_str(value: ComponentStatus) -> &'static str {
    match value {
        ComponentStatus::Pending => "pending",
        ComponentStatus::Succeeded => "succeeded",
        ComponentStatus::Failed => "failed",
    }
}

fn parse_component_status(value: &str) -> Result<ComponentStatus, SqliteError> {
    match value {
        "pending" => Ok(ComponentStatus::Pending),
        "succeeded" => Ok(ComponentStatus::Succeeded),
        "failed" => Ok(ComponentStatus::Failed),
        _ => Err(stored_deletion()),
    }
}

fn component_outcome_str(value: ComponentOutcome) -> &'static str {
    match value {
        ComponentOutcome::Deleted => "deleted",
        ComponentOutcome::Redacted => "redacted",
        ComponentOutcome::RetainedMinimal => "retained_minimal",
        ComponentOutcome::NotFound => "not_found",
        ComponentOutcome::NotApplicable => "not_applicable",
    }
}

fn parse_component_outcome(value: &str) -> Result<ComponentOutcome, SqliteError> {
    match value {
        "deleted" => Ok(ComponentOutcome::Deleted),
        "redacted" => Ok(ComponentOutcome::Redacted),
        "retained_minimal" => Ok(ComponentOutcome::RetainedMinimal),
        "not_found" => Ok(ComponentOutcome::NotFound),
        "not_applicable" => Ok(ComponentOutcome::NotApplicable),
        _ => Err(stored_deletion()),
    }
}

fn overall_status_str(value: DeletionOverallStatus) -> &'static str {
    match value {
        DeletionOverallStatus::Pending => "pending",
        DeletionOverallStatus::Partial => "partial",
        DeletionOverallStatus::Failed => "failed",
        DeletionOverallStatus::Completed => "completed",
    }
}

fn parse_overall_status(value: &str) -> Result<DeletionOverallStatus, SqliteError> {
    match value {
        "pending" => Ok(DeletionOverallStatus::Pending),
        "partial" => Ok(DeletionOverallStatus::Partial),
        "failed" => Ok(DeletionOverallStatus::Failed),
        "completed" => Ok(DeletionOverallStatus::Completed),
        _ => Err(stored_deletion()),
    }
}

fn evidence_type_str(value: EvidenceType) -> &'static str {
    match value {
        EvidenceType::SourceFragment => "source_fragment",
        EvidenceType::MemoryProposal => "memory_proposal",
        EvidenceType::MemoryDecision => "memory_decision",
        EvidenceType::MemoryRecord => "memory_record",
        EvidenceType::DeleteRequest => "delete_request",
        EvidenceType::PolicyBasis => "policy_basis",
    }
}

fn parse_evidence_type(value: &str) -> Result<EvidenceType, SqliteError> {
    match value {
        "source_fragment" => Ok(EvidenceType::SourceFragment),
        "memory_proposal" => Ok(EvidenceType::MemoryProposal),
        "memory_decision" => Ok(EvidenceType::MemoryDecision),
        "memory_record" => Ok(EvidenceType::MemoryRecord),
        "delete_request" => Ok(EvidenceType::DeleteRequest),
        "policy_basis" => Ok(EvidenceType::PolicyBasis),
        _ => Err(stored_deletion()),
    }
}

pub(crate) fn parse_object_type(value: &str) -> Result<CanonicalObjectType, SqliteError> {
    match value {
        "SourceArtifact" => Ok(CanonicalObjectType::SourceArtifact),
        "SourceFragment" => Ok(CanonicalObjectType::SourceFragment),
        "MemoryProposal" => Ok(CanonicalObjectType::MemoryProposal),
        "MemoryDecision" => Ok(CanonicalObjectType::MemoryDecision),
        "MemoryRecord" => Ok(CanonicalObjectType::MemoryRecord),
        "MemoryStateEvent" => Ok(CanonicalObjectType::MemoryStateEvent),
        "DeleteRequest" => Ok(CanonicalObjectType::DeleteRequest),
        _ => Err(stored_deletion()),
    }
}

pub(crate) fn recall_kind(object_type: CanonicalObjectType) -> Result<&'static str, SqliteError> {
    match object_type {
        CanonicalObjectType::SourceFragment => Ok("source_fragment"),
        CanonicalObjectType::MemoryRecord => Ok("memory_record"),
        _ => Err(SqliteError::deletion_invariant(
            SqliteStorageReason::DeletionExecution,
        )),
    }
}

fn to_i64(value: usize) -> Result<i64, SqliteError> {
    i64::try_from(value)
        .map_err(|_| SqliteError::deletion_invariant(SqliteStorageReason::NumericRange))
}

fn to_i64_u64(value: u64) -> Result<i64, SqliteError> {
    i64::try_from(value)
        .map_err(|_| SqliteError::deletion_invariant(SqliteStorageReason::NumericRange))
}

fn from_i64_u64(value: i64) -> Result<u64, SqliteError> {
    u64::try_from(value).map_err(|_| stored_deletion())
}

fn deletion_plan() -> SqliteError {
    SqliteError::deletion_invariant(SqliteStorageReason::DeletionPlan)
}

pub(crate) fn deletion_core(source: radishmemory_core::CoreError) -> SqliteError {
    SqliteError::deletion_invariant_with_core(SqliteStorageReason::DeletionPlan, source)
}

fn stored_deletion() -> SqliteError {
    SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
}
