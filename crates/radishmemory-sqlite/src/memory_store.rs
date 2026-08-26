use std::collections::{BTreeMap, BTreeSet};

use radishmemory_core::{
    ActorRef, ActorType, CanonicalObjectType, Decision, EvidenceRef, EvidenceType, Identifier,
    M0_SCHEMA_VERSION, MemoryDecision, MemoryDecisionParams, MemoryEventType, MemoryProposal,
    MemoryProposalParams, MemoryRecord, MemoryRecordParams, MemoryState, MemoryStateEvent,
    MemoryStateEventParams, MemoryStore, MemoryType, MemoryValue, MemoryValueKind, NonEmptyText,
    ProposalOperation, ResolvedSource, SupersededTarget, TimePrecision, Timestamp, UnitInterval,
    ValidTime, ValidTimeMode, validate_memory_event_chain, validate_memory_materialization,
    validate_memory_proposal_sources, validate_memory_supersession,
};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};

use crate::source_store::{
    StoredGovernance, StoredProducer, deletion_state_str, digest, egress_policy_str, from_i64,
    identifier, invalid_core, load_resolved_source_fragment, non_empty_text, optional_text,
    producer_type_str, retention_mode_str, sensitivity_str, timestamp, version,
};
use crate::{SqliteDatabase, SqliteError, SqliteStorageReason};

impl MemoryStore for SqliteDatabase {
    type Error = SqliteError;

    fn store_memory_proposal(&mut self, proposal: &MemoryProposal) -> Result<(), Self::Error> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        validate_proposal_for_store(&transaction, proposal)?;
        ensure_proposal_is_not_duplicate(&transaction, proposal)?;
        insert_proposal(&transaction, proposal)?;
        transaction.commit().map_err(SqliteError::storage)
    }

    fn store_memory_decision(&mut self, decision: &MemoryDecision) -> Result<(), Self::Error> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        let proposal = load_memory_proposal(
            &transaction,
            &decision.params().namespace_id,
            &decision.params().proposal_id,
        )?
        .ok_or_else(|| SqliteError::memory_invariant(SqliteStorageReason::MissingProposal))?;
        let decisions = load_decision_chain(&transaction, &proposal)?;
        validate_decision_extension(&proposal, &decisions, decision)?;
        if let Some(result_memory_id) = &decision.params().result_memory_id
            && memory_exists_in_namespace(
                &transaction,
                &decision.params().namespace_id,
                result_memory_id,
            )?
        {
            return Err(SqliteError::memory_invariant(
                SqliteStorageReason::MemoryReference,
            ));
        }
        insert_decision(&transaction, decision)?;
        transaction.commit().map_err(SqliteError::storage)
    }

    fn materialize_accepted_memory(
        &mut self,
        record: &MemoryRecord,
        initial_event: &MemoryStateEvent,
        superseded_events: &[MemoryStateEvent],
    ) -> Result<(), Self::Error> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        let params = record.params();
        let proposal = load_memory_proposal(
            &transaction,
            &params.namespace_id,
            &params.origin_proposal_id,
        )?
        .ok_or_else(|| SqliteError::memory_invariant(SqliteStorageReason::MissingProposal))?;
        let decision = load_memory_decision(
            &transaction,
            &params.namespace_id,
            &params.accepted_by_decision_id,
        )?
        .ok_or_else(|| SqliteError::memory_invariant(SqliteStorageReason::MissingDecision))?;
        validate_memory_materialization(&proposal, &decision, record, initial_event).map_err(
            |source| {
                SqliteError::memory_invariant_with_core(
                    SqliteStorageReason::Materialization,
                    source,
                )
            },
        )?;
        validate_event_references(&transaction, record, initial_event, false, Some(record))?;
        validate_record_references(&transaction, record)?;
        let superseded =
            validate_supersession_for_store(&transaction, &proposal, record, superseded_events)?;

        insert_record(&transaction, record)?;
        insert_event(&transaction, initial_event)?;
        for event in superseded_events {
            insert_event(&transaction, event)?;
        }
        crate::derived_index::insert_memory_record(&transaction, record)?;
        for (target, _) in &superseded {
            crate::derived_index::update_memory_record(&transaction, target)?;
        }

        let event_refs = [initial_event];
        validate_memory_event_chain(record, &event_refs).map_err(|source| {
            SqliteError::memory_invariant_with_core(SqliteStorageReason::EventChain, source)
        })?;
        if !superseded.is_empty() {
            let targets = superseded
                .iter()
                .map(|(target, event)| SupersededTarget {
                    record: target,
                    superseded_event: event,
                })
                .collect::<Vec<_>>();
            validate_memory_supersession(&proposal, record, &targets).map_err(|source| {
                SqliteError::memory_invariant_with_core(
                    SqliteStorageReason::Materialization,
                    source,
                )
            })?;
        }
        transaction.commit().map_err(SqliteError::storage)
    }

    fn append_memory_state_event(&mut self, event: &MemoryStateEvent) -> Result<(), Self::Error> {
        if event.params().previous_event_id.is_none()
            || event.params().event_type == MemoryEventType::Superseded
        {
            return Err(SqliteError::memory_invariant(
                SqliteStorageReason::Materialization,
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        let (record, mut events) = load_memory_record_closure(
            &transaction,
            &event.params().namespace_id,
            &event.params().memory_id,
        )?
        .ok_or_else(|| SqliteError::memory_invariant(SqliteStorageReason::MissingMemory))?;
        validate_event_extension(&transaction, &record, &events, event, None)?;
        events.push(event.clone());
        let projected = project_record(&record, event.to_state(), &event.params().event_id)?;
        let refs = events.iter().collect::<Vec<_>>();
        validate_memory_event_chain(&projected, &refs).map_err(|source| {
            SqliteError::memory_invariant_with_core(SqliteStorageReason::EventChain, source)
        })?;
        insert_event(&transaction, event)?;
        crate::derived_index::update_memory_record(&transaction, &projected)?;
        transaction.commit().map_err(SqliteError::storage)
    }

    fn load_memory_proposal(
        &self,
        namespace_id: &Identifier,
        proposal_id: &Identifier,
    ) -> Result<Option<MemoryProposal>, Self::Error> {
        load_memory_proposal(&self.connection, namespace_id, proposal_id)
    }

    fn load_memory_decision(
        &self,
        namespace_id: &Identifier,
        decision_id: &Identifier,
    ) -> Result<Option<MemoryDecision>, Self::Error> {
        load_memory_decision(&self.connection, namespace_id, decision_id)
    }

    fn load_memory_record(
        &self,
        namespace_id: &Identifier,
        memory_id: &Identifier,
    ) -> Result<Option<MemoryRecord>, Self::Error> {
        load_memory_record_closure(&self.connection, namespace_id, memory_id)
            .map(|closure| closure.map(|(record, _)| record))
    }

    fn load_memory_state_events(
        &self,
        namespace_id: &Identifier,
        memory_id: &Identifier,
    ) -> Result<Option<Vec<MemoryStateEvent>>, Self::Error> {
        load_memory_record_closure(&self.connection, namespace_id, memory_id)
            .map(|closure| closure.map(|(_, events)| events))
    }
}

fn validate_proposal_for_store(
    connection: &Connection,
    proposal: &MemoryProposal,
) -> Result<(), SqliteError> {
    validate_proposal_sources(connection, proposal, false)?;
    for target_id in &proposal.params().target_memory_ids {
        let target =
            load_memory_record_closure(connection, &proposal.params().namespace_id, target_id)?
                .ok_or_else(|| SqliteError::memory_invariant(SqliteStorageReason::MissingMemory))?
                .0;
        if target.params().current_state != MemoryState::Confirmed {
            return Err(SqliteError::memory_invariant(
                SqliteStorageReason::MemoryReference,
            ));
        }
    }
    Ok(())
}

fn validate_proposal_sources(
    connection: &Connection,
    proposal: &MemoryProposal,
    stored: bool,
) -> Result<(), SqliteError> {
    let mut resolved_pairs = Vec::with_capacity(proposal.params().source_fragment_refs.len());
    for fragment_id in &proposal.params().source_fragment_refs {
        let resolved = load_resolved_source_fragment(
            connection,
            &proposal.params().namespace_id,
            fragment_id,
        )?
        .ok_or_else(|| {
            if stored {
                SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
            } else {
                SqliteError::memory_invariant(SqliteStorageReason::MissingFragment)
            }
        })?;
        resolved_pairs.push(resolved);
    }
    let resolved = resolved_pairs
        .iter()
        .map(|(fragment, source)| ResolvedSource { fragment, source })
        .collect::<Vec<_>>();
    validate_memory_proposal_sources(proposal, &resolved).map_err(|source| {
        if stored {
            SqliteError::invalid_stored_with_source(
                SqliteStorageReason::StoredIntegrityMismatch,
                source,
            )
        } else {
            SqliteError::memory_invariant_with_core(
                SqliteStorageReason::ProposalSourceResolution,
                source,
            )
        }
    })
}

fn ensure_proposal_is_not_duplicate(
    connection: &Connection,
    proposal: &MemoryProposal,
) -> Result<(), SqliteError> {
    let params = proposal.params();
    let mut statement = connection
        .prepare(
            "SELECT proposal_id FROM radishmemory_memory_proposals
             WHERE namespace_id = ?1 AND operation = ?2
               AND content_digest_algorithm = ?3 AND content_digest_profile = ?4
               AND content_digest_value = ?5",
        )
        .map_err(SqliteError::storage)?;
    let candidates = statement
        .query_map(
            params![
                params.namespace_id.as_str(),
                proposal_operation_str(params.operation),
                params.proposed_content.content_digest().algorithm(),
                params.proposed_content.content_digest().profile().as_str(),
                params.proposed_content.content_digest().value(),
            ],
            |row| row.get::<_, String>(0),
        )
        .map_err(SqliteError::storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SqliteError::storage)?;
    let expected_sources = params
        .source_fragment_refs
        .iter()
        .map(|identifier| identifier.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    let expected_targets = params
        .target_memory_ids
        .iter()
        .map(|identifier| identifier.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    for candidate in candidates {
        let sources = load_string_set(
            connection,
            "SELECT fragment_id FROM radishmemory_proposal_source_fragments WHERE proposal_id = ?1",
            &candidate,
        )?;
        let targets = load_string_set(
            connection,
            "SELECT target_memory_id FROM radishmemory_proposal_targets WHERE proposal_id = ?1",
            &candidate,
        )?;
        if sources == expected_sources && targets == expected_targets {
            return Err(SqliteError::conflict(
                SqliteStorageReason::DuplicateProposal,
            ));
        }
    }
    Ok(())
}

fn load_string_set(
    connection: &Connection,
    sql: &str,
    parent_id: &str,
) -> Result<BTreeSet<String>, SqliteError> {
    let mut statement = connection.prepare(sql).map_err(SqliteError::storage)?;
    statement
        .query_map(params![parent_id], |row| row.get::<_, String>(0))
        .map_err(SqliteError::storage)?
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(SqliteError::storage)
}

fn insert_proposal(connection: &Connection, proposal: &MemoryProposal) -> Result<(), SqliteError> {
    let value = proposal.params();
    let content = &value.proposed_content;
    let retention = value.governance.retention();
    let valid_time = &value.valid_time;
    connection
        .execute(
            "INSERT INTO radishmemory_memory_proposals (
                 proposal_id, canonical_schema_version, object_type, namespace_id, operation,
                 memory_type, subject_ref, content_kind, content_text,
                 content_digest_algorithm, content_digest_profile, content_digest_value,
                 observed_at, valid_time_mode, valid_time_start_at, valid_time_end_at,
                 valid_time_precision, confidence, importance, sensitivity, egress_policy,
                 retention_mode, retention_expires_at, retention_policy_id, deletion_state,
                 policy_basis, producer_type, producer_id, producer_version, reason_code,
                 proposed_at
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                 ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
                 ?29, ?30, ?31
             )",
            params![
                value.proposal_id.as_str(),
                M0_SCHEMA_VERSION,
                CanonicalObjectType::MemoryProposal.as_str(),
                value.namespace_id.as_str(),
                proposal_operation_str(value.operation),
                memory_type_str(value.memory_type),
                value.subject_ref.as_str(),
                memory_value_kind_str(content.kind()),
                content.text().as_str(),
                content.content_digest().algorithm(),
                content.content_digest().profile().as_str(),
                content.content_digest().value(),
                value.observed_at.original(),
                valid_time_mode_str(valid_time.mode()),
                valid_time.start_at().map(Timestamp::original),
                valid_time.end_at().map(Timestamp::original),
                time_precision_str(valid_time.precision()),
                value.confidence.get(),
                value.importance.get(),
                sensitivity_str(value.governance.sensitivity()),
                egress_policy_str(value.governance.egress_policy()),
                retention_mode_str(retention.mode()),
                retention.expires_at().map(Timestamp::original),
                retention.policy_id().map(Identifier::as_str),
                deletion_state_str(value.governance.deletion_state()),
                value.governance.policy_basis().as_str(),
                producer_type_str(value.producer.producer_type()),
                value.producer.producer_id().as_str(),
                value.producer.producer_version().as_str(),
                value.reason_code.as_str(),
                value.proposed_at.original(),
            ],
        )
        .map_err(SqliteError::storage)?;
    insert_identifier_list(
        connection,
        "INSERT INTO radishmemory_proposal_source_fragments (proposal_id, ordinal, fragment_id) VALUES (?1, ?2, ?3)",
        &value.proposal_id,
        &value.source_fragment_refs,
    )?;
    insert_identifier_list(
        connection,
        "INSERT INTO radishmemory_proposal_targets (proposal_id, ordinal, target_memory_id) VALUES (?1, ?2, ?3)",
        &value.proposal_id,
        &value.target_memory_ids,
    )
}

fn load_memory_proposal(
    connection: &Connection,
    namespace_id: &Identifier,
    proposal_id: &Identifier,
) -> Result<Option<MemoryProposal>, SqliteError> {
    let stored = connection
        .query_row(
            "SELECT proposal_id, canonical_schema_version, object_type, namespace_id,
                    operation, memory_type, subject_ref, content_kind, content_text,
                    content_digest_algorithm, content_digest_profile, content_digest_value,
                    observed_at, valid_time_mode, valid_time_start_at, valid_time_end_at,
                    valid_time_precision, confidence, importance, sensitivity, egress_policy,
                    retention_mode, retention_expires_at, retention_policy_id, deletion_state,
                    policy_basis, producer_type, producer_id, producer_version, reason_code,
                    proposed_at
             FROM radishmemory_memory_proposals
             WHERE namespace_id = ?1 AND proposal_id = ?2
               AND deletion_state = 'active'",
            params![namespace_id.as_str(), proposal_id.as_str()],
            StoredProposal::from_row,
        )
        .optional()
        .map_err(SqliteError::storage)?;
    let Some(stored) = stored else {
        return Ok(None);
    };
    let sources = load_identifier_list(
        connection,
        "SELECT ordinal, fragment_id FROM radishmemory_proposal_source_fragments WHERE proposal_id = ?1 ORDER BY ordinal",
        proposal_id,
    )?;
    let targets = load_identifier_list(
        connection,
        "SELECT ordinal, target_memory_id FROM radishmemory_proposal_targets WHERE proposal_id = ?1 ORDER BY ordinal",
        proposal_id,
    )?;
    let proposal = stored.into_domain(sources, targets)?;
    validate_proposal_sources(connection, &proposal, true)?;
    for target_id in &proposal.params().target_memory_ids {
        if !memory_exists_in_namespace(connection, namespace_id, target_id)? {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::StoredIntegrityMismatch,
            ));
        }
    }
    Ok(Some(proposal))
}

fn validate_decision_extension(
    proposal: &MemoryProposal,
    decisions: &[MemoryDecision],
    decision: &MemoryDecision,
) -> Result<(), SqliteError> {
    let value = decision.params();
    if value.namespace_id != proposal.params().namespace_id
        || value.proposal_id != proposal.params().proposal_id
    {
        return Err(SqliteError::memory_invariant(
            SqliteStorageReason::DecisionChain,
        ));
    }
    let tip = decision_chain_tip(decisions, false)?;
    match tip {
        None if value.previous_decision_id.is_none() => Ok(()),
        Some(tip)
            if tip.params().decision == Decision::Defer
                && value.previous_decision_id.as_ref() == Some(&tip.params().decision_id) =>
        {
            Ok(())
        }
        Some(tip) if tip.params().decision != Decision::Defer => Err(
            SqliteError::memory_invariant(SqliteStorageReason::TerminalDecision),
        ),
        _ => Err(SqliteError::memory_invariant(
            SqliteStorageReason::DecisionChain,
        )),
    }
}

fn insert_decision(connection: &Connection, decision: &MemoryDecision) -> Result<(), SqliteError> {
    let value = decision.params();
    connection
        .execute(
            "INSERT INTO radishmemory_memory_decisions (
                 decision_id, canonical_schema_version, object_type, namespace_id, proposal_id,
                 previous_decision_id, decision, actor_type, actor_id, actor_version,
                 authorization_basis, reason_code, reason_text, result_memory_id, decided_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                value.decision_id.as_str(),
                M0_SCHEMA_VERSION,
                CanonicalObjectType::MemoryDecision.as_str(),
                value.namespace_id.as_str(),
                value.proposal_id.as_str(),
                value.previous_decision_id.as_ref().map(Identifier::as_str),
                decision_str(value.decision),
                actor_type_str(value.decided_by.actor_type()),
                value.decided_by.actor_id().as_str(),
                value.decided_by.actor_version().map(NonEmptyText::as_str),
                value.authorization_basis.as_str(),
                value.reason_code.as_str(),
                value.reason_text.as_ref().map(NonEmptyText::as_str),
                value.result_memory_id.as_ref().map(Identifier::as_str),
                value.decided_at.original(),
            ],
        )
        .map_err(SqliteError::storage)?;
    Ok(())
}

fn load_memory_decision(
    connection: &Connection,
    namespace_id: &Identifier,
    decision_id: &Identifier,
) -> Result<Option<MemoryDecision>, SqliteError> {
    let proposal_id = connection
        .query_row(
            "SELECT proposal_id FROM radishmemory_memory_decisions
             WHERE namespace_id = ?1 AND decision_id = ?2",
            params![namespace_id.as_str(), decision_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(SqliteError::storage)?;
    let Some(proposal_id) = proposal_id else {
        return Ok(None);
    };
    let proposal_id = identifier(proposal_id)?;
    let proposal = load_memory_proposal(connection, namespace_id, &proposal_id)?
        .ok_or_else(|| SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch))?;
    let decisions = load_decision_chain(connection, &proposal)?;
    Ok(decisions
        .into_iter()
        .find(|decision| &decision.params().decision_id == decision_id))
}

fn load_decision_chain(
    connection: &Connection,
    proposal: &MemoryProposal,
) -> Result<Vec<MemoryDecision>, SqliteError> {
    let mut statement = connection
        .prepare(
            "SELECT decision_id, canonical_schema_version, object_type, namespace_id,
                    proposal_id, previous_decision_id, decision, actor_type, actor_id,
                    actor_version, authorization_basis, reason_code, reason_text,
                    result_memory_id, decided_at
             FROM radishmemory_memory_decisions WHERE proposal_id = ?1",
        )
        .map_err(SqliteError::storage)?;
    let stored = statement
        .query_map(
            params![proposal.params().proposal_id.as_str()],
            StoredDecision::from_row,
        )
        .map_err(SqliteError::storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SqliteError::storage)?;
    let mut decisions = Vec::with_capacity(stored.len());
    for stored in stored {
        let decision = stored.into_domain()?;
        if decision.params().namespace_id != proposal.params().namespace_id
            || decision.params().proposal_id != proposal.params().proposal_id
        {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::StoredIntegrityMismatch,
            ));
        }
        decisions.push(decision);
    }
    decision_chain_tip(&decisions, true)?;
    order_decision_chain(decisions)
}

fn decision_chain_tip(
    decisions: &[MemoryDecision],
    stored: bool,
) -> Result<Option<&MemoryDecision>, SqliteError> {
    if decisions.is_empty() {
        return Ok(None);
    }
    let roots = decisions
        .iter()
        .filter(|decision| decision.params().previous_decision_id.is_none())
        .collect::<Vec<_>>();
    if roots.len() != 1 {
        return Err(chain_error(stored));
    }
    let mut current = roots[0];
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(&current.params().decision_id) {
            return Err(chain_error(stored));
        }
        let children = decisions
            .iter()
            .filter(|candidate| {
                candidate.params().previous_decision_id.as_ref()
                    == Some(&current.params().decision_id)
            })
            .collect::<Vec<_>>();
        if children.len() > 1
            || (current.params().decision != Decision::Defer && !children.is_empty())
        {
            return Err(chain_error(stored));
        }
        let Some(next) = children.first().copied() else {
            break;
        };
        current = next;
    }
    if visited.len() != decisions.len() {
        return Err(chain_error(stored));
    }
    Ok(Some(current))
}

fn order_decision_chain(
    decisions: Vec<MemoryDecision>,
) -> Result<Vec<MemoryDecision>, SqliteError> {
    if decisions.is_empty() {
        return Ok(decisions);
    }
    let mut by_previous = BTreeMap::new();
    let mut root = None;
    for decision in decisions {
        if let Some(previous) = decision.params().previous_decision_id.clone() {
            if by_previous.insert(previous, decision).is_some() {
                return Err(chain_error(true));
            }
        } else if root.replace(decision).is_some() {
            return Err(chain_error(true));
        }
    }
    let mut ordered = Vec::new();
    let mut current = root.ok_or_else(|| chain_error(true))?;
    loop {
        let current_id = current.params().decision_id.clone();
        ordered.push(current);
        let Some(next) = by_previous.remove(&current_id) else {
            break;
        };
        current = next;
    }
    if !by_previous.is_empty() {
        return Err(chain_error(true));
    }
    Ok(ordered)
}

fn chain_error(stored: bool) -> SqliteError {
    if stored {
        SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
    } else {
        SqliteError::memory_invariant(SqliteStorageReason::DecisionChain)
    }
}

fn validate_record_references(
    connection: &Connection,
    record: &MemoryRecord,
) -> Result<(), SqliteError> {
    let value = record.params();
    for fragment_id in &value.source_fragment_refs {
        if load_resolved_source_fragment(connection, &value.namespace_id, fragment_id)?.is_none() {
            return Err(SqliteError::memory_invariant(
                SqliteStorageReason::MissingFragment,
            ));
        }
    }
    for target_id in value
        .supersedes_memory_ids
        .iter()
        .chain(&value.contradicts_memory_ids)
    {
        if !memory_exists_in_namespace(connection, &value.namespace_id, target_id)? {
            return Err(SqliteError::memory_invariant(
                SqliteStorageReason::MemoryReference,
            ));
        }
    }
    Ok(())
}

fn validate_supersession_for_store(
    connection: &Connection,
    proposal: &MemoryProposal,
    record: &MemoryRecord,
    superseded_events: &[MemoryStateEvent],
) -> Result<Vec<(MemoryRecord, MemoryStateEvent)>, SqliteError> {
    if proposal.params().operation == ProposalOperation::Create {
        if superseded_events.is_empty() {
            return Ok(Vec::new());
        }
        return Err(SqliteError::memory_invariant(
            SqliteStorageReason::Materialization,
        ));
    }
    let event_by_memory = superseded_events
        .iter()
        .map(|event| (&event.params().memory_id, event))
        .collect::<BTreeMap<_, _>>();
    if event_by_memory.len() != superseded_events.len()
        || event_by_memory.len() != proposal.params().target_memory_ids.len()
    {
        return Err(SqliteError::memory_invariant(
            SqliteStorageReason::Materialization,
        ));
    }

    let mut projected_targets = Vec::with_capacity(event_by_memory.len());
    for target_id in &proposal.params().target_memory_ids {
        let event = event_by_memory
            .get(target_id)
            .copied()
            .ok_or_else(|| SqliteError::memory_invariant(SqliteStorageReason::Materialization))?;
        let (target, mut events) =
            load_memory_record_closure(connection, &proposal.params().namespace_id, target_id)?
                .ok_or_else(|| SqliteError::memory_invariant(SqliteStorageReason::MissingMemory))?;
        validate_event_extension(connection, &target, &events, event, Some(record))?;
        events.push(event.clone());
        let projected = project_record(&target, event.to_state(), &event.params().event_id)?;
        let refs = events.iter().collect::<Vec<_>>();
        validate_memory_event_chain(&projected, &refs).map_err(|source| {
            SqliteError::memory_invariant_with_core(SqliteStorageReason::EventChain, source)
        })?;
        projected_targets.push((projected, event.clone()));
    }
    let targets = projected_targets
        .iter()
        .map(|(target, event)| SupersededTarget {
            record: target,
            superseded_event: event,
        })
        .collect::<Vec<_>>();
    validate_memory_supersession(proposal, record, &targets).map_err(|source| {
        SqliteError::memory_invariant_with_core(SqliteStorageReason::Materialization, source)
    })?;
    Ok(projected_targets)
}

fn insert_record(connection: &Connection, record: &MemoryRecord) -> Result<(), SqliteError> {
    let value = record.params();
    let content = &value.content;
    let retention = value.governance.retention();
    let valid_time = &value.valid_time;
    connection
        .execute(
            "INSERT INTO radishmemory_memory_records (
                 memory_id, canonical_schema_version, object_type, lineage_id, version,
                 namespace_id, memory_type, subject_ref, content_kind, content_text,
                 content_digest_algorithm, content_digest_profile, content_digest_value,
                 origin_proposal_id, accepted_by_decision_id, observed_at, valid_time_mode,
                 valid_time_start_at, valid_time_end_at, valid_time_precision, confidence,
                 importance, sensitivity, egress_policy, retention_mode,
                 retention_expires_at, retention_policy_id, deletion_state, policy_basis,
                 created_at
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                 ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
                 ?29, ?30
             )",
            params![
                value.memory_id.as_str(),
                M0_SCHEMA_VERSION,
                CanonicalObjectType::MemoryRecord.as_str(),
                value.lineage_id.as_str(),
                memory_to_i64(value.version.get())?,
                value.namespace_id.as_str(),
                memory_type_str(value.memory_type),
                value.subject_ref.as_str(),
                memory_value_kind_str(content.kind()),
                content.text().as_str(),
                value.content_digest.algorithm(),
                value.content_digest.profile().as_str(),
                value.content_digest.value(),
                value.origin_proposal_id.as_str(),
                value.accepted_by_decision_id.as_str(),
                value.observed_at.original(),
                valid_time_mode_str(valid_time.mode()),
                valid_time.start_at().map(Timestamp::original),
                valid_time.end_at().map(Timestamp::original),
                time_precision_str(valid_time.precision()),
                value.confidence.get(),
                value.importance.get(),
                sensitivity_str(value.governance.sensitivity()),
                egress_policy_str(value.governance.egress_policy()),
                retention_mode_str(retention.mode()),
                retention.expires_at().map(Timestamp::original),
                retention.policy_id().map(Identifier::as_str),
                deletion_state_str(value.governance.deletion_state()),
                value.governance.policy_basis().as_str(),
                value.created_at.original(),
            ],
        )
        .map_err(SqliteError::storage)?;
    insert_identifier_list(
        connection,
        "INSERT INTO radishmemory_record_source_fragments (memory_id, ordinal, fragment_id) VALUES (?1, ?2, ?3)",
        &value.memory_id,
        &value.source_fragment_refs,
    )?;
    insert_identifier_list(
        connection,
        "INSERT INTO radishmemory_record_supersedes (memory_id, ordinal, superseded_memory_id) VALUES (?1, ?2, ?3)",
        &value.memory_id,
        &value.supersedes_memory_ids,
    )?;
    insert_identifier_list(
        connection,
        "INSERT INTO radishmemory_record_contradicts (memory_id, ordinal, contradicted_memory_id) VALUES (?1, ?2, ?3)",
        &value.memory_id,
        &value.contradicts_memory_ids,
    )
}

fn insert_event(connection: &Connection, event: &MemoryStateEvent) -> Result<(), SqliteError> {
    let value = event.params();
    connection
        .execute(
            "INSERT INTO radishmemory_memory_state_events (
                 event_id, canonical_schema_version, object_type, namespace_id, memory_id,
                 previous_event_id, event_type, from_state, cause_type, cause_id, actor_type,
                 actor_id, actor_version, reason_code, effective_at, occurred_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                value.event_id.as_str(),
                M0_SCHEMA_VERSION,
                CanonicalObjectType::MemoryStateEvent.as_str(),
                value.namespace_id.as_str(),
                value.memory_id.as_str(),
                value.previous_event_id.as_ref().map(Identifier::as_str),
                memory_event_type_str(value.event_type),
                value.from_state.map(memory_state_str),
                evidence_type_str(value.cause_ref.evidence_type()),
                value.cause_ref.evidence_id().as_str(),
                actor_type_str(value.actor.actor_type()),
                value.actor.actor_id().as_str(),
                value.actor.actor_version().map(NonEmptyText::as_str),
                value.reason_code.as_str(),
                value.effective_at.as_ref().map(Timestamp::original),
                value.occurred_at.original(),
            ],
        )
        .map_err(SqliteError::storage)?;
    insert_identifier_list(
        connection,
        "INSERT INTO radishmemory_event_related_memories (event_id, ordinal, related_memory_id) VALUES (?1, ?2, ?3)",
        &value.event_id,
        &value.related_memory_ids,
    )
}

fn validate_event_extension(
    connection: &Connection,
    record: &MemoryRecord,
    events: &[MemoryStateEvent],
    event: &MemoryStateEvent,
    pending_record: Option<&MemoryRecord>,
) -> Result<(), SqliteError> {
    let value = event.params();
    if value.namespace_id != record.params().namespace_id
        || value.memory_id != record.params().memory_id
        || value.previous_event_id.as_ref() != Some(&record.params().last_state_event_id)
        || value.from_state != Some(record.params().current_state)
        || record.params().current_state != MemoryState::Confirmed
        || events.is_empty()
    {
        return Err(SqliteError::memory_invariant(
            SqliteStorageReason::EventChain,
        ));
    }
    validate_event_references(connection, record, event, false, pending_record)
}

fn validate_event_references(
    connection: &Connection,
    record: &MemoryRecord,
    event: &MemoryStateEvent,
    stored: bool,
    pending_record: Option<&MemoryRecord>,
) -> Result<(), SqliteError> {
    let namespace_id = &record.params().namespace_id;
    let cause_exists = match event.params().cause_ref.evidence_type() {
        EvidenceType::MemoryDecision => decision_exists_in_namespace(
            connection,
            namespace_id,
            event.params().cause_ref.evidence_id(),
        )?,
        EvidenceType::MemoryRecord => {
            pending_record.is_some_and(|pending| {
                pending.params().namespace_id == *namespace_id
                    && pending.params().memory_id == *event.params().cause_ref.evidence_id()
            }) || memory_exists_in_namespace(
                connection,
                namespace_id,
                event.params().cause_ref.evidence_id(),
            )?
        }
        EvidenceType::PolicyBasis => true,
        EvidenceType::DeleteRequest => {
            if stored {
                false
            } else {
                return Err(SqliteError::memory_invariant(
                    SqliteStorageReason::UnsupportedCause,
                ));
            }
        }
        EvidenceType::SourceFragment | EvidenceType::MemoryProposal => false,
    };
    if !cause_exists {
        return Err(if stored {
            SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
        } else {
            SqliteError::memory_invariant(SqliteStorageReason::MemoryReference)
        });
    }
    for related_id in &event.params().related_memory_ids {
        let is_pending = pending_record.is_some_and(|pending| {
            pending.params().namespace_id == *namespace_id
                && pending.params().memory_id == *related_id
        });
        if !is_pending && !memory_exists_in_namespace(connection, namespace_id, related_id)? {
            return Err(if stored {
                SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
            } else {
                SqliteError::memory_invariant(SqliteStorageReason::MemoryReference)
            });
        }
    }
    Ok(())
}

pub(crate) fn load_memory_record_closure(
    connection: &Connection,
    namespace_id: &Identifier,
    memory_id: &Identifier,
) -> Result<Option<(MemoryRecord, Vec<MemoryStateEvent>)>, SqliteError> {
    load_memory_record_closure_inner(connection, namespace_id, memory_id, &mut BTreeSet::new())
}

fn load_memory_record_closure_inner(
    connection: &Connection,
    namespace_id: &Identifier,
    memory_id: &Identifier,
    visiting: &mut BTreeSet<Identifier>,
) -> Result<Option<(MemoryRecord, Vec<MemoryStateEvent>)>, SqliteError> {
    let stored = connection
        .query_row(
            "SELECT memory_id, canonical_schema_version, object_type, lineage_id, version,
                    namespace_id, memory_type, subject_ref, content_kind, content_text,
                    content_digest_algorithm, content_digest_profile, content_digest_value,
                    origin_proposal_id, accepted_by_decision_id, observed_at, valid_time_mode,
                    valid_time_start_at, valid_time_end_at, valid_time_precision, confidence,
                    importance, sensitivity, egress_policy, retention_mode,
                    retention_expires_at, retention_policy_id, deletion_state, policy_basis,
                    created_at
             FROM radishmemory_memory_records
             WHERE namespace_id = ?1 AND memory_id = ?2 AND deletion_state = 'active'",
            params![namespace_id.as_str(), memory_id.as_str()],
            StoredRecord::from_row,
        )
        .optional()
        .map_err(SqliteError::storage)?;
    let Some(stored) = stored else {
        return Ok(None);
    };
    if !visiting.insert(memory_id.clone()) {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::StoredIntegrityMismatch,
        ));
    }

    let result = (|| {
        let sources = load_identifier_list(
            connection,
            "SELECT ordinal, fragment_id FROM radishmemory_record_source_fragments WHERE memory_id = ?1 ORDER BY ordinal",
            memory_id,
        )?;
        let supersedes = load_identifier_list(
            connection,
            "SELECT ordinal, superseded_memory_id FROM radishmemory_record_supersedes WHERE memory_id = ?1 ORDER BY ordinal",
            memory_id,
        )?;
        let contradicts = load_identifier_list(
            connection,
            "SELECT ordinal, contradicted_memory_id FROM radishmemory_record_contradicts WHERE memory_id = ?1 ORDER BY ordinal",
            memory_id,
        )?;
        let events = load_event_chain(connection, namespace_id, memory_id)?;
        let root = events.first().ok_or_else(|| {
            SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
        })?;
        if root.params().previous_event_id.is_some() || root.to_state() != MemoryState::Confirmed {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::StoredIntegrityMismatch,
            ));
        }
        let initial = stored.into_domain(
            sources,
            supersedes,
            contradicts,
            MemoryState::Confirmed,
            root.params().event_id.clone(),
        )?;
        let proposal = load_memory_proposal(
            connection,
            namespace_id,
            &initial.params().origin_proposal_id,
        )?
        .ok_or_else(|| SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch))?;
        let decision = load_memory_decision(
            connection,
            namespace_id,
            &initial.params().accepted_by_decision_id,
        )?
        .ok_or_else(|| SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch))?;
        validate_memory_materialization(&proposal, &decision, &initial, root).map_err(
            |source| {
                SqliteError::invalid_stored_with_source(
                    SqliteStorageReason::StoredIntegrityMismatch,
                    source,
                )
            },
        )?;

        let tip = events.last().ok_or_else(|| {
            SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
        })?;
        let projected = project_record(&initial, tip.to_state(), &tip.params().event_id)?;
        let refs = events.iter().collect::<Vec<_>>();
        validate_memory_event_chain(&projected, &refs).map_err(|source| {
            SqliteError::invalid_stored_with_source(
                SqliteStorageReason::StoredIntegrityMismatch,
                source,
            )
        })?;
        for event in &events {
            validate_event_references(connection, &projected, event, true, None)?;
        }
        if tip.params().event_type == MemoryEventType::Superseded {
            validate_stored_supersession_cause(connection, &projected, tip)?;
        }
        for target_id in &projected.params().contradicts_memory_ids {
            if !memory_exists_in_namespace(connection, namespace_id, target_id)? {
                return Err(SqliteError::invalid_stored(
                    SqliteStorageReason::StoredIntegrityMismatch,
                ));
            }
        }
        if proposal.params().operation == ProposalOperation::Supersede {
            let mut target_closures = Vec::new();
            for target_id in &projected.params().supersedes_memory_ids {
                let (target, target_events) = load_memory_record_closure_inner(
                    connection,
                    namespace_id,
                    target_id,
                    visiting,
                )?
                .ok_or_else(|| {
                    SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
                })?;
                let event = target_events
                    .iter()
                    .find(|event| {
                        event.params().event_id == target.params().last_state_event_id
                            && event.params().cause_ref.evidence_type()
                                == EvidenceType::MemoryRecord
                            && event.params().cause_ref.evidence_id()
                                == &projected.params().memory_id
                    })
                    .cloned()
                    .ok_or_else(|| {
                        SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
                    })?;
                target_closures.push((target, event));
            }
            let targets = target_closures
                .iter()
                .map(|(target, event)| SupersededTarget {
                    record: target,
                    superseded_event: event,
                })
                .collect::<Vec<_>>();
            validate_memory_supersession(&proposal, &projected, &targets).map_err(|source| {
                SqliteError::invalid_stored_with_source(
                    SqliteStorageReason::StoredIntegrityMismatch,
                    source,
                )
            })?;
        }
        Ok((projected, events))
    })();
    visiting.remove(memory_id);
    result.map(Some)
}

fn validate_stored_supersession_cause(
    connection: &Connection,
    old_record: &MemoryRecord,
    event: &MemoryStateEvent,
) -> Result<(), SqliteError> {
    if event.params().cause_ref.evidence_type() != EvidenceType::MemoryRecord
        || !event
            .params()
            .related_memory_ids
            .contains(event.params().cause_ref.evidence_id())
    {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::StoredIntegrityMismatch,
        ));
    }
    let new_id = event.params().cause_ref.evidence_id();
    let metadata = connection
        .query_row(
            "SELECT namespace_id, lineage_id, version, origin_proposal_id,
                    valid_time_start_at
             FROM radishmemory_memory_records WHERE memory_id = ?1",
            params![new_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(SqliteError::storage)?
        .ok_or_else(|| SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch))?;
    if metadata.0 != old_record.params().namespace_id.as_str()
        || metadata.1 != old_record.params().lineage_id.as_str()
        || from_i64(metadata.2)? <= old_record.params().version.get()
        || metadata.4.as_deref().map(timestamp).transpose()?.as_ref()
            != event.params().effective_at.as_ref()
    {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::StoredIntegrityMismatch,
        ));
    }
    let proposal_id = identifier(metadata.3)?;
    let operation = connection
        .query_row(
            "SELECT operation FROM radishmemory_memory_proposals WHERE proposal_id = ?1",
            params![proposal_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(SqliteError::storage)?;
    if operation.as_deref() != Some("supersede") {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::StoredIntegrityMismatch,
        ));
    }
    let proposal_targets = load_identifier_list(
        connection,
        "SELECT ordinal, target_memory_id FROM radishmemory_proposal_targets WHERE proposal_id = ?1 ORDER BY ordinal",
        &proposal_id,
    )?;
    let record_targets = load_identifier_list(
        connection,
        "SELECT ordinal, superseded_memory_id FROM radishmemory_record_supersedes WHERE memory_id = ?1 ORDER BY ordinal",
        new_id,
    )?;
    if proposal_targets.iter().collect::<BTreeSet<_>>()
        != record_targets.iter().collect::<BTreeSet<_>>()
        || !record_targets.contains(&old_record.params().memory_id)
    {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::StoredIntegrityMismatch,
        ));
    }
    Ok(())
}

fn load_event_chain(
    connection: &Connection,
    namespace_id: &Identifier,
    memory_id: &Identifier,
) -> Result<Vec<MemoryStateEvent>, SqliteError> {
    let mut statement = connection
        .prepare(
            "SELECT event_id, canonical_schema_version, object_type, namespace_id, memory_id,
                    previous_event_id, event_type, from_state, cause_type, cause_id, actor_type,
                    actor_id, actor_version, reason_code, effective_at, occurred_at
             FROM radishmemory_memory_state_events WHERE memory_id = ?1",
        )
        .map_err(SqliteError::storage)?;
    let stored = statement
        .query_map(params![memory_id.as_str()], StoredEvent::from_row)
        .map_err(SqliteError::storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SqliteError::storage)?;
    let mut events = Vec::with_capacity(stored.len());
    for stored in stored {
        let event_id = identifier(stored.event_id.clone())?;
        let related = load_identifier_list(
            connection,
            "SELECT ordinal, related_memory_id FROM radishmemory_event_related_memories WHERE event_id = ?1 ORDER BY ordinal",
            &event_id,
        )?;
        let event = stored.into_domain(related)?;
        if event.params().namespace_id != *namespace_id || event.params().memory_id != *memory_id {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::StoredIntegrityMismatch,
            ));
        }
        events.push(event);
    }
    order_event_chain(events)
}

fn order_event_chain(events: Vec<MemoryStateEvent>) -> Result<Vec<MemoryStateEvent>, SqliteError> {
    if events.is_empty() {
        return Ok(events);
    }
    let mut by_previous = BTreeMap::new();
    let mut root = None;
    for event in events {
        if let Some(previous) = event.params().previous_event_id.clone() {
            if by_previous.insert(previous, event).is_some() {
                return Err(SqliteError::invalid_stored(
                    SqliteStorageReason::StoredIntegrityMismatch,
                ));
            }
        } else if root.replace(event).is_some() {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::StoredIntegrityMismatch,
            ));
        }
    }
    let mut ordered = Vec::new();
    let mut current = root
        .ok_or_else(|| SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch))?;
    loop {
        let current_id = current.params().event_id.clone();
        ordered.push(current);
        let Some(next) = by_previous.remove(&current_id) else {
            break;
        };
        current = next;
    }
    if !by_previous.is_empty() {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::StoredIntegrityMismatch,
        ));
    }
    Ok(ordered)
}

fn project_record(
    record: &MemoryRecord,
    current_state: MemoryState,
    last_event_id: &Identifier,
) -> Result<MemoryRecord, SqliteError> {
    let mut params = record.params().clone();
    params.current_state = current_state;
    params.last_state_event_id = last_event_id.clone();
    MemoryRecord::new(params).map_err(invalid_core)
}

fn memory_exists_in_namespace(
    connection: &Connection,
    namespace_id: &Identifier,
    memory_id: &Identifier,
) -> Result<bool, SqliteError> {
    connection
        .query_row(
            "SELECT 1 FROM radishmemory_memory_records
             WHERE namespace_id = ?1 AND memory_id = ?2 LIMIT 1",
            params![namespace_id.as_str(), memory_id.as_str()],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(SqliteError::storage)
}

fn decision_exists_in_namespace(
    connection: &Connection,
    namespace_id: &Identifier,
    decision_id: &Identifier,
) -> Result<bool, SqliteError> {
    connection
        .query_row(
            "SELECT 1 FROM radishmemory_memory_decisions
             WHERE namespace_id = ?1 AND decision_id = ?2 LIMIT 1",
            params![namespace_id.as_str(), decision_id.as_str()],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(SqliteError::storage)
}

fn insert_identifier_list(
    connection: &Connection,
    sql: &str,
    parent_id: &Identifier,
    values: &[Identifier],
) -> Result<(), SqliteError> {
    for (ordinal, value) in values.iter().enumerate() {
        connection
            .execute(
                sql,
                params![
                    parent_id.as_str(),
                    memory_to_i64(memory_usize_to_u64(ordinal)?)?,
                    value.as_str(),
                ],
            )
            .map_err(SqliteError::storage)?;
    }
    Ok(())
}

fn load_identifier_list(
    connection: &Connection,
    sql: &str,
    parent_id: &Identifier,
) -> Result<Vec<Identifier>, SqliteError> {
    let mut statement = connection.prepare(sql).map_err(SqliteError::storage)?;
    let rows = statement
        .query_map(params![parent_id.as_str()], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(SqliteError::storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SqliteError::storage)?;
    let mut result = Vec::with_capacity(rows.len());
    for (expected, (ordinal, value)) in rows.into_iter().enumerate() {
        if ordinal != stored_ordinal(expected)? {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::StoredIntegrityMismatch,
            ));
        }
        result.push(identifier(value)?);
    }
    Ok(result)
}

fn memory_to_i64(value: u64) -> Result<i64, SqliteError> {
    i64::try_from(value)
        .map_err(|_| SqliteError::memory_invariant(SqliteStorageReason::NumericRange))
}

fn memory_usize_to_u64(value: usize) -> Result<u64, SqliteError> {
    u64::try_from(value)
        .map_err(|_| SqliteError::memory_invariant(SqliteStorageReason::NumericRange))
}

fn stored_ordinal(value: usize) -> Result<i64, SqliteError> {
    i64::try_from(value).map_err(|_| SqliteError::invalid_stored(SqliteStorageReason::NumericRange))
}

struct StoredProposal {
    proposal_id: String,
    canonical_schema_version: String,
    object_type: String,
    namespace_id: String,
    operation: String,
    memory_type: String,
    subject_ref: String,
    content_kind: String,
    content_text: String,
    digest_algorithm: String,
    digest_profile: String,
    digest_value: String,
    observed_at: String,
    valid_time: StoredValidTime,
    confidence: f64,
    importance: f64,
    governance: StoredGovernance,
    producer: StoredProducer,
    reason_code: String,
    proposed_at: String,
}

impl StoredProposal {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            proposal_id: row.get("proposal_id")?,
            canonical_schema_version: row.get("canonical_schema_version")?,
            object_type: row.get("object_type")?,
            namespace_id: row.get("namespace_id")?,
            operation: row.get("operation")?,
            memory_type: row.get("memory_type")?,
            subject_ref: row.get("subject_ref")?,
            content_kind: row.get("content_kind")?,
            content_text: row.get("content_text")?,
            digest_algorithm: row.get("content_digest_algorithm")?,
            digest_profile: row.get("content_digest_profile")?,
            digest_value: row.get("content_digest_value")?,
            observed_at: row.get("observed_at")?,
            valid_time: StoredValidTime::from_row(row)?,
            confidence: row.get("confidence")?,
            importance: row.get("importance")?,
            governance: StoredGovernance::from_row(row)?,
            producer: StoredProducer::from_row(
                row,
                "producer_type",
                "producer_id",
                "producer_version",
            )?,
            reason_code: row.get("reason_code")?,
            proposed_at: row.get("proposed_at")?,
        })
    }

    fn into_domain(
        self,
        source_fragment_refs: Vec<Identifier>,
        target_memory_ids: Vec<Identifier>,
    ) -> Result<MemoryProposal, SqliteError> {
        require_stored_identity(
            &self.canonical_schema_version,
            &self.object_type,
            CanonicalObjectType::MemoryProposal,
        )?;
        let proposed_content = memory_value(
            &self.content_kind,
            self.content_text,
            &self.digest_algorithm,
            &self.digest_profile,
            &self.digest_value,
        )?;
        MemoryProposal::new(MemoryProposalParams {
            proposal_id: identifier(self.proposal_id)?,
            namespace_id: identifier(self.namespace_id)?,
            operation: parse_proposal_operation(&self.operation)?,
            memory_type: parse_memory_type(&self.memory_type)?,
            subject_ref: identifier(self.subject_ref)?,
            proposed_content,
            source_fragment_refs,
            target_memory_ids,
            observed_at: timestamp(&self.observed_at)?,
            valid_time: self.valid_time.into_domain()?,
            confidence: unit_interval(self.confidence)?,
            importance: unit_interval(self.importance)?,
            governance: self.governance.into_domain()?,
            producer: self.producer.into_domain()?,
            reason_code: non_empty_text(self.reason_code)?,
            proposed_at: timestamp(&self.proposed_at)?,
        })
        .map_err(invalid_core)
    }
}

struct StoredDecision {
    decision_id: String,
    canonical_schema_version: String,
    object_type: String,
    namespace_id: String,
    proposal_id: String,
    previous_decision_id: Option<String>,
    decision: String,
    actor: StoredActor,
    authorization_basis: String,
    reason_code: String,
    reason_text: Option<String>,
    result_memory_id: Option<String>,
    decided_at: String,
}

impl StoredDecision {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            decision_id: row.get("decision_id")?,
            canonical_schema_version: row.get("canonical_schema_version")?,
            object_type: row.get("object_type")?,
            namespace_id: row.get("namespace_id")?,
            proposal_id: row.get("proposal_id")?,
            previous_decision_id: row.get("previous_decision_id")?,
            decision: row.get("decision")?,
            actor: StoredActor::from_row(row)?,
            authorization_basis: row.get("authorization_basis")?,
            reason_code: row.get("reason_code")?,
            reason_text: row.get("reason_text")?,
            result_memory_id: row.get("result_memory_id")?,
            decided_at: row.get("decided_at")?,
        })
    }

    fn into_domain(self) -> Result<MemoryDecision, SqliteError> {
        require_stored_identity(
            &self.canonical_schema_version,
            &self.object_type,
            CanonicalObjectType::MemoryDecision,
        )?;
        MemoryDecision::new(MemoryDecisionParams {
            decision_id: identifier(self.decision_id)?,
            namespace_id: identifier(self.namespace_id)?,
            proposal_id: identifier(self.proposal_id)?,
            previous_decision_id: self.previous_decision_id.map(identifier).transpose()?,
            decision: parse_decision(&self.decision)?,
            decided_by: self.actor.into_domain()?,
            authorization_basis: non_empty_text(self.authorization_basis)?,
            reason_code: non_empty_text(self.reason_code)?,
            reason_text: optional_text(self.reason_text)?,
            result_memory_id: self.result_memory_id.map(identifier).transpose()?,
            decided_at: timestamp(&self.decided_at)?,
        })
        .map_err(invalid_core)
    }
}

struct StoredRecord {
    memory_id: String,
    canonical_schema_version: String,
    object_type: String,
    lineage_id: String,
    version: i64,
    namespace_id: String,
    memory_type: String,
    subject_ref: String,
    content_kind: String,
    content_text: String,
    digest_algorithm: String,
    digest_profile: String,
    digest_value: String,
    origin_proposal_id: String,
    accepted_by_decision_id: String,
    observed_at: String,
    valid_time: StoredValidTime,
    confidence: f64,
    importance: f64,
    governance: StoredGovernance,
    created_at: String,
}

impl StoredRecord {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            memory_id: row.get("memory_id")?,
            canonical_schema_version: row.get("canonical_schema_version")?,
            object_type: row.get("object_type")?,
            lineage_id: row.get("lineage_id")?,
            version: row.get("version")?,
            namespace_id: row.get("namespace_id")?,
            memory_type: row.get("memory_type")?,
            subject_ref: row.get("subject_ref")?,
            content_kind: row.get("content_kind")?,
            content_text: row.get("content_text")?,
            digest_algorithm: row.get("content_digest_algorithm")?,
            digest_profile: row.get("content_digest_profile")?,
            digest_value: row.get("content_digest_value")?,
            origin_proposal_id: row.get("origin_proposal_id")?,
            accepted_by_decision_id: row.get("accepted_by_decision_id")?,
            observed_at: row.get("observed_at")?,
            valid_time: StoredValidTime::from_row(row)?,
            confidence: row.get("confidence")?,
            importance: row.get("importance")?,
            governance: StoredGovernance::from_row(row)?,
            created_at: row.get("created_at")?,
        })
    }

    fn into_domain(
        self,
        source_fragment_refs: Vec<Identifier>,
        supersedes_memory_ids: Vec<Identifier>,
        contradicts_memory_ids: Vec<Identifier>,
        current_state: MemoryState,
        last_state_event_id: Identifier,
    ) -> Result<MemoryRecord, SqliteError> {
        require_stored_identity(
            &self.canonical_schema_version,
            &self.object_type,
            CanonicalObjectType::MemoryRecord,
        )?;
        let content = memory_value(
            &self.content_kind,
            self.content_text,
            &self.digest_algorithm,
            &self.digest_profile,
            &self.digest_value,
        )?;
        let content_digest = content.content_digest().clone();
        MemoryRecord::new(MemoryRecordParams {
            memory_id: identifier(self.memory_id)?,
            lineage_id: identifier(self.lineage_id)?,
            version: version(self.version)?,
            namespace_id: identifier(self.namespace_id)?,
            memory_type: parse_memory_type(&self.memory_type)?,
            subject_ref: identifier(self.subject_ref)?,
            content,
            source_fragment_refs,
            origin_proposal_id: identifier(self.origin_proposal_id)?,
            accepted_by_decision_id: identifier(self.accepted_by_decision_id)?,
            observed_at: timestamp(&self.observed_at)?,
            valid_time: self.valid_time.into_domain()?,
            confidence: unit_interval(self.confidence)?,
            importance: unit_interval(self.importance)?,
            governance: self.governance.into_domain()?,
            current_state,
            last_state_event_id,
            supersedes_memory_ids,
            contradicts_memory_ids,
            content_digest,
            created_at: timestamp(&self.created_at)?,
        })
        .map_err(invalid_core)
    }
}

struct StoredEvent {
    event_id: String,
    canonical_schema_version: String,
    object_type: String,
    namespace_id: String,
    memory_id: String,
    previous_event_id: Option<String>,
    event_type: String,
    from_state: Option<String>,
    cause_type: String,
    cause_id: String,
    actor: StoredActor,
    reason_code: String,
    effective_at: Option<String>,
    occurred_at: String,
}

impl StoredEvent {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            event_id: row.get("event_id")?,
            canonical_schema_version: row.get("canonical_schema_version")?,
            object_type: row.get("object_type")?,
            namespace_id: row.get("namespace_id")?,
            memory_id: row.get("memory_id")?,
            previous_event_id: row.get("previous_event_id")?,
            event_type: row.get("event_type")?,
            from_state: row.get("from_state")?,
            cause_type: row.get("cause_type")?,
            cause_id: row.get("cause_id")?,
            actor: StoredActor::from_row(row)?,
            reason_code: row.get("reason_code")?,
            effective_at: row.get("effective_at")?,
            occurred_at: row.get("occurred_at")?,
        })
    }

    fn into_domain(
        self,
        related_memory_ids: Vec<Identifier>,
    ) -> Result<MemoryStateEvent, SqliteError> {
        require_stored_identity(
            &self.canonical_schema_version,
            &self.object_type,
            CanonicalObjectType::MemoryStateEvent,
        )?;
        MemoryStateEvent::new(MemoryStateEventParams {
            event_id: identifier(self.event_id)?,
            namespace_id: identifier(self.namespace_id)?,
            memory_id: identifier(self.memory_id)?,
            previous_event_id: self.previous_event_id.map(identifier).transpose()?,
            event_type: parse_memory_event_type(&self.event_type)?,
            from_state: self
                .from_state
                .as_deref()
                .map(parse_memory_state)
                .transpose()?,
            cause_ref: EvidenceRef::new(
                parse_evidence_type(&self.cause_type)?,
                identifier(self.cause_id)?,
            ),
            related_memory_ids,
            actor: self.actor.into_domain()?,
            reason_code: non_empty_text(self.reason_code)?,
            effective_at: self.effective_at.as_deref().map(timestamp).transpose()?,
            occurred_at: timestamp(&self.occurred_at)?,
        })
        .map_err(invalid_core)
    }
}

struct StoredActor {
    actor_type: String,
    actor_id: String,
    actor_version: Option<String>,
}

impl StoredActor {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            actor_type: row.get("actor_type")?,
            actor_id: row.get("actor_id")?,
            actor_version: row.get("actor_version")?,
        })
    }

    fn into_domain(self) -> Result<ActorRef, SqliteError> {
        Ok(ActorRef::new(
            parse_actor_type(&self.actor_type)?,
            identifier(self.actor_id)?,
            optional_text(self.actor_version)?,
        ))
    }
}

struct StoredValidTime {
    mode: String,
    start_at: Option<String>,
    end_at: Option<String>,
    precision: String,
}

impl StoredValidTime {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            mode: row.get("valid_time_mode")?,
            start_at: row.get("valid_time_start_at")?,
            end_at: row.get("valid_time_end_at")?,
            precision: row.get("valid_time_precision")?,
        })
    }

    fn into_domain(self) -> Result<ValidTime, SqliteError> {
        ValidTime::new(
            parse_valid_time_mode(&self.mode)?,
            self.start_at.as_deref().map(timestamp).transpose()?,
            self.end_at.as_deref().map(timestamp).transpose()?,
            parse_time_precision(&self.precision)?,
        )
        .map_err(invalid_core)
    }
}

fn require_stored_identity(
    schema_version: &str,
    object_type: &str,
    expected_type: CanonicalObjectType,
) -> Result<(), SqliteError> {
    if schema_version != M0_SCHEMA_VERSION || object_type != expected_type.as_str() {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::StoredIntegrityMismatch,
        ));
    }
    Ok(())
}

fn memory_value(
    kind: &str,
    text: String,
    algorithm: &str,
    profile: &str,
    value: &str,
) -> Result<MemoryValue, SqliteError> {
    if parse_memory_value_kind(kind)? != MemoryValueKind::Text {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::UnknownEnum,
        ));
    }
    MemoryValue::new(non_empty_text(text)?, digest(algorithm, profile, value)?)
        .map_err(invalid_core)
}

fn unit_interval(value: f64) -> Result<UnitInterval, SqliteError> {
    UnitInterval::new(value).map_err(invalid_core)
}

fn unknown_enum<T>() -> Result<T, SqliteError> {
    Err(SqliteError::invalid_stored(
        SqliteStorageReason::UnknownEnum,
    ))
}

fn proposal_operation_str(value: ProposalOperation) -> &'static str {
    match value {
        ProposalOperation::Create => "create",
        ProposalOperation::Supersede => "supersede",
    }
}

fn parse_proposal_operation(value: &str) -> Result<ProposalOperation, SqliteError> {
    match value {
        "create" => Ok(ProposalOperation::Create),
        "supersede" => Ok(ProposalOperation::Supersede),
        _ => unknown_enum(),
    }
}

fn memory_type_str(value: MemoryType) -> &'static str {
    match value {
        MemoryType::Observation => "observation",
        MemoryType::Claim => "claim",
        MemoryType::Episode => "episode",
        MemoryType::Preference => "preference",
        MemoryType::Procedure => "procedure",
    }
}

fn parse_memory_type(value: &str) -> Result<MemoryType, SqliteError> {
    match value {
        "observation" => Ok(MemoryType::Observation),
        "claim" => Ok(MemoryType::Claim),
        "episode" => Ok(MemoryType::Episode),
        "preference" => Ok(MemoryType::Preference),
        "procedure" => Ok(MemoryType::Procedure),
        _ => unknown_enum(),
    }
}

fn memory_value_kind_str(value: MemoryValueKind) -> &'static str {
    match value {
        MemoryValueKind::Text => "text",
    }
}

fn parse_memory_value_kind(value: &str) -> Result<MemoryValueKind, SqliteError> {
    match value {
        "text" => Ok(MemoryValueKind::Text),
        _ => unknown_enum(),
    }
}

fn decision_str(value: Decision) -> &'static str {
    match value {
        Decision::Accept => "accept",
        Decision::Reject => "reject",
        Decision::Defer => "defer",
    }
}

fn parse_decision(value: &str) -> Result<Decision, SqliteError> {
    match value {
        "accept" => Ok(Decision::Accept),
        "reject" => Ok(Decision::Reject),
        "defer" => Ok(Decision::Defer),
        _ => unknown_enum(),
    }
}

fn memory_state_str(value: MemoryState) -> &'static str {
    match value {
        MemoryState::Confirmed => "confirmed",
        MemoryState::Superseded => "superseded",
        MemoryState::Contradicted => "contradicted",
        MemoryState::Retracted => "retracted",
        MemoryState::Expired => "expired",
    }
}

fn parse_memory_state(value: &str) -> Result<MemoryState, SqliteError> {
    match value {
        "confirmed" => Ok(MemoryState::Confirmed),
        "superseded" => Ok(MemoryState::Superseded),
        "contradicted" => Ok(MemoryState::Contradicted),
        "retracted" => Ok(MemoryState::Retracted),
        "expired" => Ok(MemoryState::Expired),
        _ => unknown_enum(),
    }
}

fn memory_event_type_str(value: MemoryEventType) -> &'static str {
    match value {
        MemoryEventType::Confirmed => "confirmed",
        MemoryEventType::Superseded => "superseded",
        MemoryEventType::Contradicted => "contradicted",
        MemoryEventType::Retracted => "retracted",
        MemoryEventType::Expired => "expired",
    }
}

fn parse_memory_event_type(value: &str) -> Result<MemoryEventType, SqliteError> {
    match value {
        "confirmed" => Ok(MemoryEventType::Confirmed),
        "superseded" => Ok(MemoryEventType::Superseded),
        "contradicted" => Ok(MemoryEventType::Contradicted),
        "retracted" => Ok(MemoryEventType::Retracted),
        "expired" => Ok(MemoryEventType::Expired),
        _ => unknown_enum(),
    }
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
        _ => unknown_enum(),
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
        _ => unknown_enum(),
    }
}

fn valid_time_mode_str(value: ValidTimeMode) -> &'static str {
    match value {
        ValidTimeMode::Unknown => "unknown",
        ValidTimeMode::Instant => "instant",
        ValidTimeMode::Interval => "interval",
        ValidTimeMode::OpenEnded => "open_ended",
    }
}

fn parse_valid_time_mode(value: &str) -> Result<ValidTimeMode, SqliteError> {
    match value {
        "unknown" => Ok(ValidTimeMode::Unknown),
        "instant" => Ok(ValidTimeMode::Instant),
        "interval" => Ok(ValidTimeMode::Interval),
        "open_ended" => Ok(ValidTimeMode::OpenEnded),
        _ => unknown_enum(),
    }
}

fn time_precision_str(value: TimePrecision) -> &'static str {
    match value {
        TimePrecision::Exact => "exact",
        TimePrecision::Day => "day",
        TimePrecision::Month => "month",
        TimePrecision::Year => "year",
        TimePrecision::Unknown => "unknown",
    }
}

fn parse_time_precision(value: &str) -> Result<TimePrecision, SqliteError> {
    match value {
        "exact" => Ok(TimePrecision::Exact),
        "day" => Ok(TimePrecision::Day),
        "month" => Ok(TimePrecision::Month),
        "year" => Ok(TimePrecision::Year),
        "unknown" => Ok(TimePrecision::Unknown),
        _ => unknown_enum(),
    }
}
