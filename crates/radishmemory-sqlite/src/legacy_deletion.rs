//! Adopt an existing plaintext deletion without inventing objects or authority.
//! The frozen legacy closure is checked against surviving provenance; missing
//! rows require the original executor's persisted component result.
use std::collections::{BTreeMap, BTreeSet};

use radishmemory_core::{
    CanonicalObjectType as ObjectType, DeleteRequest, DeletionComponentType as ComponentType,
    ObjectRef, Timestamp, compute_nfc_text_digest,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    SqliteError, SqliteStorageReason, deletion_actions::load_execution_closure, deletion_store,
    source_store,
};
type Result<T> = std::result::Result<T, SqliteError>;
fn invalid() -> SqliteError {
    SqliteError::deletion_invariant(SqliteStorageReason::DeletionExecution)
}
fn require(value: bool) -> Result<()> {
    if value { Ok(()) } else { Err(invalid()) }
}
fn query_ids(connection: &Connection, sql: &str, value: &str) -> Result<BTreeSet<String>> {
    connection
        .prepare(sql)
        .map_err(SqliteError::storage)?
        .query_map([value], |request| request.get(0))
        .map_err(SqliteError::storage)?
        .collect::<std::result::Result<_, _>>()
        .map_err(SqliteError::storage)
}
fn closure_ids(
    connection: &Connection,
    request: &DeleteRequest,
    component: ComponentType,
    kind: ObjectType,
) -> Result<BTreeSet<String>> {
    let rows = load_execution_closure(connection, &request.params().delete_request_id, component)?;
    require(rows.iter().all(|v| v.object_type() == kind))?;
    let values: BTreeSet<_> = rows
        .iter()
        .map(|v| v.object_id().as_str().to_owned())
        .collect();
    require(values.len() == rows.len())?;
    Ok(values)
}
fn require_closed_target(
    connection: &Connection,
    table: &str,
    key: &str,
    value: &str,
    namespace: &str,
) -> Result<()> {
    let valid: bool = connection.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE {key}=?1 AND namespace_id=?2 AND deletion_state IN ('pending','failed','deleted'))"),params![value,namespace], |request| request.get(0)).map_err(SqliteError::storage)?;
    require(valid)
}

/// Select a real partial or complete legacy attempt. Later attempts need not
/// reproduce it; receipts pin this ordinal rather than trusting the latest row.
fn legacy_component_proof(
    connection: &Connection,
    request: &DeleteRequest,
    component: ComponentType,
    ordinal: Option<i64>,
) -> Result<i64> {
    let definition = request
        .params()
        .planned_components
        .iter()
        .find(|v| v.component_type() == component)
        .ok_or_else(invalid)?;
    let (method, outcome) = match component {
        ComponentType::SourceBody | ComponentType::SourceFragment => {
            ("sqlite-row-absence-v1", "deleted")
        }
        ComponentType::MemoryProposal => ("sqlite-content-redaction-v1", "redacted"),
        _ => return Err(invalid()),
    };
    let mut stmt=connection.prepare("SELECT request.attempt_ordinal,request.checked_at,a.checked_at FROM radishmemory_deletion_execution_results request JOIN radishmemory_deletion_execution_attempts a USING(delete_request_id,attempt_ordinal) WHERE request.delete_request_id=?1 AND request.component_key=?2 AND (?3 IS NULL OR request.attempt_ordinal=?3) AND request.status='succeeded' AND request.outcome IN (?4,'not_found') AND request.verification_method=?5 AND request.processed_count=?6 AND request.error_code IS NULL AND request.retryable IS NULL AND request.retention_basis_type IS NULL AND request.retention_basis_id IS NULL ORDER BY request.attempt_ordinal").map_err(SqliteError::storage)?;
    let rows = stmt
        .query_map(
            params![
                request.params().delete_request_id.as_str(),
                definition.component_key().as_str(),
                ordinal,
                outcome,
                method,
                i64::try_from(definition.target_count()).map_err(|_| invalid())?
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(SqliteError::storage)?;
    if let Some(row) = rows.into_iter().next() {
        let (attempt, checked, started) = row.map_err(SqliteError::storage)?;
        let checked = Timestamp::parse(&checked).map_err(|_| invalid())?;
        let started = Timestamp::parse(&started).map_err(|_| invalid())?;
        require(checked == started && checked >= request.params().requested_at)?;
        return Ok(attempt);
    }
    Err(invalid())
}

pub(crate) fn validate_resume(
    connection: &Connection,
    request: &DeleteRequest,
) -> Result<BTreeMap<String, Option<i64>>> {
    deletion_store::validate_request_profile(request)?;
    let namespace = request.params().namespace_id.as_str();
    let roots = |kind| {
        request
            .params()
            .target_refs
            .iter()
            .filter(|t| t.object_type() == kind)
            .map(|t| t.object_id().as_str().to_owned())
            .collect::<BTreeSet<_>>()
    };
    let sources = roots(ObjectType::SourceArtifact);
    let memories = roots(ObjectType::MemoryRecord);
    require(
        closure_ids(
            connection,
            request,
            ComponentType::SourceBody,
            ObjectType::SourceArtifact,
        )? == sources,
    )?;
    require(
        closure_ids(
            connection,
            request,
            ComponentType::SourceMetadata,
            ObjectType::SourceArtifact,
        )? == sources,
    )?;
    require(
        closure_ids(
            connection,
            request,
            ComponentType::MemoryRecord,
            ObjectType::MemoryRecord,
        )? == memories,
    )?;
    require(
        load_execution_closure(
            connection,
            &request.params().delete_request_id,
            ComponentType::ContextCache,
        )?
        .is_empty(),
    )?;
    let mut audit: BTreeSet<_> = request
        .params()
        .target_refs
        .iter()
        .filter(|t| t.object_type() == ObjectType::SourceArtifact)
        .cloned()
        .collect();
    audit.insert(ObjectRef::new(
        ObjectType::DeleteRequest,
        request.params().delete_request_id.clone(),
    ));
    require(
        load_execution_closure(
            connection,
            &request.params().delete_request_id,
            ComponentType::MinimalAudit,
        )?
        .into_iter()
        .collect::<BTreeSet<_>>()
            == audit,
    )?;
    let fragments = closure_ids(
        connection,
        request,
        ComponentType::SourceFragment,
        ObjectType::SourceFragment,
    )?;
    let proposals = closure_ids(
        connection,
        request,
        ComponentType::MemoryProposal,
        ObjectType::MemoryProposal,
    )?;
    let decisions = closure_ids(
        connection,
        request,
        ComponentType::MemoryDecision,
        ObjectType::MemoryDecision,
    )?;
    let events = closure_ids(
        connection,
        request,
        ComponentType::MemoryStateEvent,
        ObjectType::MemoryStateEvent,
    )?;
    let mut fts = BTreeSet::new();
    for (kind, values) in [
        (ObjectType::SourceFragment, &fragments),
        (ObjectType::MemoryRecord, &memories),
    ] {
        for value in values {
            fts.insert(ObjectRef::new(
                kind,
                source_store::identifier(value.clone())?,
            ));
        }
    }
    require(
        load_execution_closure(
            connection,
            &request.params().delete_request_id,
            ComponentType::FullTextIndex,
        )?
        .into_iter()
        .collect::<BTreeSet<_>>()
            == fts,
    )?;
    let mut bodies = BTreeMap::new();
    for source in &sources {
        require_closed_target(
            connection,
            "radishmemory_source_artifacts",
            "source_id",
            source,
            namespace,
        )?;
        require(query_ids(connection,"SELECT b.source_id FROM radishmemory_source_artifacts a JOIN radishmemory_source_artifacts b ON a.namespace_id=b.namespace_id AND a.lineage_id=b.lineage_id WHERE a.source_id=?1 AND b.deletion_state!='deleted'",source)?.is_subset(&sources))?;
        require(
            query_ids(
                connection,
                "SELECT fragment_id FROM radishmemory_source_fragments WHERE source_id=?1",
                source,
            )?
            .is_subset(&fragments),
        )?;
        require(
            query_ids(
                connection,
                "SELECT source_id FROM radishmemory_source_bodies WHERE source_id=?1",
                source,
            )?
            .is_empty(),
        )?;
        let objects = query_ids(
            connection,
            "SELECT source_id FROM radishmemory_source_vault_attempts WHERE source_id=?1",
            source,
        )?;
        if objects.is_empty() {
            bodies.insert(
                source.clone(),
                Some(legacy_component_proof(
                    connection,
                    request,
                    ComponentType::SourceBody,
                    None,
                )?),
            );
        } else {
            let valid:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM radishmemory_source_vault_attempts a JOIN radishmemory_source_vault_references request USING(source_id) WHERE a.source_id=?1 AND a.state IN ('retired','committed'))",[source],|row|row.get(0)).map_err(SqliteError::storage)?;
            require(valid)?;
            bodies.insert(source.clone(), None);
        }
    }
    let mut dependent_proposals = BTreeSet::new();
    for fragment in &fragments {
        let parent: Option<String> = connection
            .query_row(
                "SELECT source_id FROM radishmemory_source_fragments WHERE fragment_id=?1",
                [fragment],
                |row| row.get(0),
            )
            .optional()
            .map_err(SqliteError::storage)?;
        if let Some(parent) = parent {
            require(sources.contains(&parent))?;
            require_closed_target(
                connection,
                "radishmemory_source_fragments",
                "fragment_id",
                fragment,
                namespace,
            )?;
        } else {
            legacy_component_proof(connection, request, ComponentType::SourceFragment, None)?;
        }
        dependent_proposals.extend(query_ids(
            connection,
            "SELECT proposal_id FROM radishmemory_proposal_source_fragments WHERE fragment_id=?1",
            fragment,
        )?);
        require(
            query_ids(
                connection,
                "SELECT memory_id FROM radishmemory_record_source_fragments WHERE fragment_id=?1",
                fragment,
            )?
            .is_subset(&memories),
        )?;
    }
    let mut expected_events = BTreeSet::new();
    for memory in &memories {
        require_closed_target(
            connection,
            "radishmemory_memory_records",
            "memory_id",
            memory,
            namespace,
        )?;
        require(query_ids(connection,"SELECT s.memory_id FROM radishmemory_record_supersedes s JOIN radishmemory_memory_records request ON request.memory_id=s.memory_id WHERE s.superseded_memory_id=?1 AND request.deletion_state!='deleted'",memory)?.is_subset(&memories))?;
        dependent_proposals.extend(query_ids(
            connection,
            "SELECT origin_proposal_id FROM radishmemory_memory_records WHERE memory_id=?1",
            memory,
        )?);
        expected_events.extend(query_ids(
            connection,
            "SELECT event_id FROM radishmemory_memory_state_events WHERE memory_id=?1",
            memory,
        )?);
    }
    require(dependent_proposals.is_subset(&proposals))?;
    let mut expected_decisions = BTreeSet::new();
    for proposal in &proposals {
        require_closed_target(
            connection,
            "radishmemory_memory_proposals",
            "proposal_id",
            proposal,
            namespace,
        )?;
        if !dependent_proposals.contains(proposal) {
            // A completed redaction removes fragment links. Only that exact
            // marker plus its real old result can account for lost provenance.
            legacy_component_proof(connection, request, ComponentType::MemoryProposal, None)?;
            let marker = "[redacted:local-deletion]";
            let valid:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM radishmemory_memory_proposals p WHERE proposal_id=?1 AND content_text=?2 AND content_digest_value=?3 AND NOT EXISTS(SELECT 1 FROM radishmemory_proposal_source_fragments s WHERE s.proposal_id=p.proposal_id))",params![proposal,marker,compute_nfc_text_digest(marker).value()],|row|row.get(0)).map_err(SqliteError::storage)?;
            require(valid)?;
        }
        expected_decisions.extend(query_ids(
            connection,
            "SELECT decision_id FROM radishmemory_memory_decisions WHERE proposal_id=?1",
            proposal,
        )?);
    }
    require(expected_decisions == decisions && expected_events == events)?;
    for (table, key, values) in [
        ("radishmemory_memory_decisions", "decision_id", decisions),
        ("radishmemory_memory_state_events", "event_id", events),
    ] {
        for value in values {
            let row_namespace: String = connection
                .query_row(
                    &format!("SELECT namespace_id FROM {table} WHERE {key}=?1"),
                    [value],
                    |row| row.get(0),
                )
                .map_err(SqliteError::storage)?;
            require(row_namespace == namespace)?;
        }
    }
    Ok(bodies)
}

pub(crate) fn verify_recorded_body_absence(
    connection: &Connection,
    request: &DeleteRequest,
    source: &str,
) -> Result<()> {
    require(request.params().target_refs.iter().any(|t| {
        t.object_type() == ObjectType::SourceArtifact && t.object_id().as_str() == source
    }))?;
    require_closed_target(
        connection,
        "radishmemory_source_artifacts",
        "source_id",
        source,
        request.params().namespace_id.as_str(),
    )?;
    let ordinal:i64=connection.query_row("SELECT attempt_ordinal FROM radishmemory_legacy_body_retirements WHERE source_id=?1 AND delete_request_id=?2",params![source,request.params().delete_request_id.as_str()],|row|row.get(0)).map_err(SqliteError::storage)?;
    legacy_component_proof(
        connection,
        request,
        ComponentType::SourceBody,
        Some(ordinal),
    )?;
    for table in [
        "radishmemory_source_bodies",
        "radishmemory_source_vault_attempts",
        "radishmemory_source_vault_references",
        "radishmemory_source_vault_deletions",
    ] {
        require(
            query_ids(
                connection,
                &format!("SELECT source_id FROM {table} WHERE source_id=?1"),
                source,
            )?
            .is_empty(),
        )?;
    }
    Ok(())
}

pub(crate) fn verify_receipts(connection: &Connection, namespace: &str) -> Result<()> {
    let mut stmt = connection
        .prepare("SELECT source_id,delete_request_id FROM radishmemory_legacy_body_retirements")
        .map_err(SqliteError::storage)?;
    let rows = stmt
        .query_map([], |request| {
            Ok((request.get::<_, String>(0)?, request.get::<_, String>(1)?))
        })
        .map_err(SqliteError::storage)?;
    for row in rows {
        let (source, request) = row.map_err(SqliteError::storage)?;
        let request = deletion_store::load_request(
            connection,
            &source_store::identifier(namespace.to_owned())?,
            &source_store::identifier(request)?,
        )?
        .ok_or_else(invalid)?;
        verify_recorded_body_absence(connection, &request, &source)?;
    }
    Ok(())
}
