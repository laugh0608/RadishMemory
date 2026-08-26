use radishmemory_core::{
    Decision, EvidenceRef, EvidenceType, MediaType, MemoryDecision, MemoryDecisionParams,
    MemoryEventType, MemoryProposal, MemoryProposalParams, MemoryRecord, MemoryRecordParams,
    MemoryState, MemoryStateEvent, MemoryStateEventParams, MemoryStore, MemoryType, MemoryValue,
    ProposalOperation, SourceArtifact, SourceArtifactParams, SourceFragment, SourceFragmentParams,
    SourceKind, SourceOriginKind, SourceVault, TimePrecision, UnitInterval, ValidTime,
    ValidTimeMode, Version, compute_exact_bytes_digest,
};
use radishmemory_sqlite::{SqliteErrorCode, SqliteStorageReason};
use serde_json::Value;

use super::OperationOutcome;
use crate::error::{RunnerError, RunnerErrorCode, RunnerResult};
use crate::fixture::{
    object, optional_array, optional_string, optional_strings, string, u64_value,
};
use crate::state::{
    ScenarioState, actor, fixture_producer, governance, id, producer, text, timestamp,
};

pub fn capture(state: &mut ScenarioState, input: &Value) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    if string(input, "governance_profile")? != "m0-local-personal" {
        return Err(invalid_operation("governance-profile-unsupported"));
    }
    let logical_key = string(input, "logical_key")?;
    let content = text(string(input, "content")?)?;
    let source_kind = match string(input, "source_kind")? {
        "text" => SourceKind::Text,
        "markdown" => SourceKind::Markdown,
        _ => return Err(invalid_operation("source-kind-unsupported")),
    };
    let media_type = match string(input, "media_type")? {
        "text/plain" => MediaType::TextPlain,
        "text/markdown" => MediaType::TextMarkdown,
        _ => return Err(invalid_operation("media-type-unsupported")),
    };
    let source_id = state.stable_id("SourceArtifact", logical_key)?;
    let captured_at = timestamp(string(input, "captured_at")?)?;
    let digest = compute_exact_bytes_digest(content.as_str().as_bytes());
    let source = SourceArtifact::new(SourceArtifactParams {
        source_id: source_id.clone(),
        lineage_id: state.helper_id("source-lineage", logical_key)?,
        version: Version::new(1).map_err(core("source-version-invalid"))?,
        namespace_id: state.namespace_id.clone(),
        source_kind,
        media_type,
        content_length: u64::try_from(content.utf8_len())
            .map_err(|_| invalid_operation("content-length-overflow"))?,
        content_digest: digest.clone(),
        content,
        title: None,
        origin_kind: SourceOriginKind::SyntheticFixture,
        origin_ref: None,
        observed_at: timestamp(string(input, "observed_at")?)?,
        captured_at: captured_at.clone(),
        supersedes_source_ids: vec![],
        governance: governance()?,
        producer: fixture_producer()?,
        created_at: captured_at,
    })
    .map_err(core("source-canonical-invalid"))?;
    state
        .storage
        .database
        .store_source_artifact(&source)
        .map_err(storage("source-store-failed"))?;
    let loaded = state
        .storage
        .database
        .load_source_artifact(&state.namespace_id, &source_id)
        .map_err(storage("source-load-failed"))?;
    let preserved = loaded.as_ref() == Some(&source);
    state
        .source_keys_by_id
        .insert(source_id.clone(), logical_key.to_owned());
    state.sources.insert(logical_key.to_owned(), source);
    state.emit(
        logical_key,
        &source_id,
        Some((digest.profile().as_str(), digest.value())),
    );
    Ok(OperationOutcome::succeeded([
        ("source-content-preserved", preserved),
        ("source-exact-digest-matches", preserved),
    ]))
}

pub fn segment(state: &mut ScenarioState, input: &Value) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    if string(input, "segmenter_profile")? != "m0-lines-v1" {
        return Err(invalid_operation("segmenter-profile-unsupported"));
    }
    let source_key = string(input, "source_key")?;
    let source = state
        .sources
        .get(source_key)
        .cloned()
        .ok_or_else(|| invalid_operation("source-key-unresolved"))?;
    let specs = optional_array(input, "expected_fragments");
    let specs = if specs.is_empty() {
        vec![FragmentSpec {
            logical_key: format!("{source_key}-fragment"),
            byte_start: 0,
            byte_end: u64::try_from(source.params().content.utf8_len())
                .map_err(|_| invalid_operation("fragment-range-overflow"))?,
            content: source.params().content.as_str().to_owned(),
        }]
    } else {
        specs
            .iter()
            .map(|value| {
                let value = object(value)?;
                Ok(FragmentSpec {
                    logical_key: string(value, "logical_key")?.to_owned(),
                    byte_start: u64_value(value, "byte_start")?,
                    byte_end: u64_value(value, "byte_end")?,
                    content: string(value, "content")?.to_owned(),
                })
            })
            .collect::<RunnerResult<Vec<_>>>()?
    };
    let mut fragments = Vec::with_capacity(specs.len());
    for (ordinal, spec) in specs.iter().enumerate() {
        let fragment_id = state.stable_id("SourceFragment", &spec.logical_key)?;
        let content = text(&spec.content)?;
        let fragment = SourceFragment::new(SourceFragmentParams {
            fragment_id: fragment_id.clone(),
            namespace_id: state.namespace_id.clone(),
            source_id: source.params().source_id.clone(),
            ordinal: u64::try_from(ordinal)
                .map_err(|_| invalid_operation("fragment-ordinal-overflow"))?,
            byte_start: spec.byte_start,
            byte_end: spec.byte_end,
            heading_path: None,
            content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
            content,
            segmenter: fixture_producer()?,
            governance: governance()?,
            created_at: source.params().captured_at.clone(),
        })
        .map_err(core("fragment-canonical-invalid"))?;
        state
            .fragment_keys_by_id
            .insert(fragment_id.clone(), spec.logical_key.clone());
        state.emit(
            &spec.logical_key,
            &fragment_id,
            Some((
                fragment.params().content_digest.profile().as_str(),
                fragment.params().content_digest.value(),
            )),
        );
        fragments.push(fragment);
    }
    state
        .storage
        .database
        .store_source_fragments(&fragments)
        .map_err(storage("fragment-store-failed"))?;
    let loaded = state
        .storage
        .database
        .load_source_fragments(&state.namespace_id, &source.params().source_id)
        .map_err(storage("fragment-load-failed"))?;
    let resolved = loaded.as_ref() == Some(&fragments);
    let stable = specs.iter().all(|spec| {
        state
            .stable_id("SourceFragment", &spec.logical_key)
            .is_ok_and(|expected| state.fragment_keys_by_id.contains_key(&expected))
    });
    state.fragments.insert(source_key.to_owned(), fragments);
    Ok(OperationOutcome::succeeded([
        ("fragment-id-stable", stable),
        ("fragment-range-resolves", resolved),
    ]))
}

struct FragmentSpec {
    logical_key: String,
    byte_start: u64,
    byte_end: u64,
    content: String,
}

pub fn propose(state: &mut ScenarioState, input: &Value) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    let logical_key = string(input, "logical_key")?;
    let source_keys = crate::fixture::strings(input, "source_keys")?;
    let source_fragment_refs = source_keys
        .iter()
        .flat_map(|key| state.fragments.get(key).into_iter().flatten())
        .map(|fragment| fragment.params().fragment_id.clone())
        .collect::<Vec<_>>();
    if source_fragment_refs.is_empty() {
        return Err(invalid_operation("proposal-source-unresolved"));
    }
    let target_memory_ids = optional_strings(input, "target_memory_keys")?
        .iter()
        .map(|key| {
            state
                .records
                .get(key)
                .map(|record| record.params().memory_id.clone())
                .ok_or_else(|| invalid_operation("proposal-target-unresolved"))
        })
        .collect::<RunnerResult<Vec<_>>>()?;
    let valid_time = parse_valid_time(
        input
            .get("valid_time")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_operation("valid-time-missing"))?,
    )?;
    let observed_at = valid_time
        .start_at()
        .cloned()
        .or_else(|| {
            source_keys
                .first()
                .and_then(|key| state.sources.get(key))
                .map(|source| source.params().observed_at.clone())
        })
        .ok_or_else(|| invalid_operation("proposal-observed-at-missing"))?;
    let proposal_id = state.stable_id("MemoryProposal", logical_key)?;
    let proposal = MemoryProposal::new(MemoryProposalParams {
        proposal_id: proposal_id.clone(),
        namespace_id: state.namespace_id.clone(),
        operation: match string(input, "operation")? {
            "create" => ProposalOperation::Create,
            "supersede" => ProposalOperation::Supersede,
            _ => return Err(invalid_operation("proposal-operation-unsupported")),
        },
        memory_type: parse_memory_type(string(input, "memory_type")?)?,
        subject_ref: id(string(input, "subject_ref")?)?,
        proposed_content: MemoryValue::from_text(text(string(input, "content")?)?),
        source_fragment_refs,
        target_memory_ids,
        observed_at: observed_at.clone(),
        valid_time,
        confidence: UnitInterval::new(number(input, "confidence")?)
            .map_err(core("proposal-confidence-invalid"))?,
        importance: UnitInterval::new(number(input, "importance")?)
            .map_err(core("proposal-importance-invalid"))?,
        governance: governance()?,
        producer: producer(string(input, "producer")?)?,
        reason_code: text(string(input, "reason_code")?)?,
        proposed_at: observed_at,
    })
    .map_err(core("proposal-canonical-invalid"))?;
    state
        .storage
        .database
        .store_memory_proposal(&proposal)
        .map_err(storage("proposal-store-failed"))?;
    let loaded = state
        .storage
        .database
        .load_memory_proposal(&state.namespace_id, &proposal_id)
        .map_err(storage("proposal-load-failed"))?;
    let remains_unconfirmed =
        loaded.as_ref() == Some(&proposal) && !state.records.contains_key(logical_key);
    let supersede_target_explicit = proposal.params().operation != ProposalOperation::Supersede
        || !proposal.params().target_memory_ids.is_empty();
    state.proposals.insert(logical_key.to_owned(), proposal);
    state.emit(logical_key, &proposal_id, None);
    Ok(OperationOutcome::succeeded([
        ("proposal-remains-unconfirmed", remains_unconfirmed),
        ("supersede-target-explicit", supersede_target_explicit),
    ]))
}

pub fn decide(state: &mut ScenarioState, input: &Value) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    let logical_key = string(input, "logical_key")?;
    let proposal_key = string(input, "proposal_key")?;
    let proposal = state
        .proposals
        .get(proposal_key)
        .ok_or_else(|| invalid_operation("decision-proposal-unresolved"))?;
    let decision_value = match string(input, "decision")? {
        "accept" => Decision::Accept,
        "reject" => Decision::Reject,
        "defer" => Decision::Defer,
        _ => return Err(invalid_operation("decision-value-unsupported")),
    };
    let result_memory_id = optional_string(input, "result_memory_key")
        .map(|key| state.stable_id("MemoryRecord", key))
        .transpose()?;
    let decision_id = state.stable_id("MemoryDecision", logical_key)?;
    let decision = MemoryDecision::new(MemoryDecisionParams {
        decision_id: decision_id.clone(),
        namespace_id: state.namespace_id.clone(),
        proposal_id: proposal.params().proposal_id.clone(),
        previous_decision_id: None,
        decision: decision_value,
        decided_by: actor(string(input, "decided_by")?)?,
        authorization_basis: text(string(input, "authorization_basis")?)?,
        reason_code: text(match decision_value {
            Decision::Accept => "fixture-accept",
            Decision::Reject => "fixture-reject",
            Decision::Defer => "fixture-defer",
        })?,
        reason_text: None,
        result_memory_id,
        decided_at: timestamp(string(input, "decided_at")?)?,
    })
    .map_err(core("decision-canonical-invalid"))?;
    state
        .storage
        .database
        .store_memory_decision(&decision)
        .map_err(storage("decision-store-failed"))?;
    let loaded = state
        .storage
        .database
        .load_memory_decision(&state.namespace_id, &decision_id)
        .map_err(storage("decision-load-failed"))?;
    let separate = loaded.as_ref() == Some(&decision)
        && decision.params().decision_id != proposal.params().proposal_id;
    let terminal = decision_value != Decision::Reject || loaded.is_some();
    state.decisions.insert(logical_key.to_owned(), decision);
    state.emit(logical_key, &decision_id, None);
    Ok(OperationOutcome::succeeded([
        ("decision-is-separate-event", separate),
        ("reject-decision-is-terminal", terminal),
    ]))
}

pub fn materialize_memory(
    state: &mut ScenarioState,
    input: &Value,
) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    let proposal_key = string(input, "proposal_key")?;
    let decision_key = string(input, "decision_key")?;
    let memory_key = string(input, "memory_key")?;
    let proposal = state
        .proposals
        .get(proposal_key)
        .cloned()
        .ok_or_else(|| invalid_operation("materialize-proposal-unresolved"))?;
    let decision = state
        .decisions
        .get(decision_key)
        .cloned()
        .ok_or_else(|| invalid_operation("materialize-decision-unresolved"))?;
    let version = u64_value(input, "version")?;
    let event_key = memory_key.replace("-memory-", "-confirmed-");
    let event_id = state.stable_id("MemoryStateEvent", &event_key)?;
    let memory_id = state.stable_id("MemoryRecord", memory_key)?;
    let initial_event = MemoryStateEvent::new(MemoryStateEventParams {
        event_id: event_id.clone(),
        namespace_id: state.namespace_id.clone(),
        memory_id: memory_id.clone(),
        previous_event_id: None,
        event_type: MemoryEventType::Confirmed,
        from_state: None,
        cause_ref: EvidenceRef::new(
            EvidenceType::MemoryDecision,
            decision.params().decision_id.clone(),
        ),
        related_memory_ids: vec![],
        actor: decision.params().decided_by.clone(),
        reason_code: text("fixture-confirmed")?,
        effective_at: None,
        occurred_at: decision.params().decided_at.clone(),
    })
    .map_err(core("initial-event-invalid"))?;
    let record = MemoryRecord::new(MemoryRecordParams {
        memory_id: memory_id.clone(),
        lineage_id: state.helper_id("memory-lineage", string(input, "lineage_key")?)?,
        version: Version::new(version).map_err(core("memory-version-invalid"))?,
        namespace_id: state.namespace_id.clone(),
        memory_type: proposal.params().memory_type,
        subject_ref: proposal.params().subject_ref.clone(),
        content: proposal.params().proposed_content.clone(),
        source_fragment_refs: proposal.params().source_fragment_refs.clone(),
        origin_proposal_id: proposal.params().proposal_id.clone(),
        accepted_by_decision_id: decision.params().decision_id.clone(),
        observed_at: proposal.params().observed_at.clone(),
        valid_time: proposal.params().valid_time.clone(),
        confidence: proposal.params().confidence,
        importance: proposal.params().importance,
        governance: proposal.params().governance.clone(),
        current_state: MemoryState::Confirmed,
        last_state_event_id: event_id.clone(),
        supersedes_memory_ids: proposal.params().target_memory_ids.clone(),
        contradicts_memory_ids: vec![],
        content_digest: proposal.params().proposed_content.content_digest().clone(),
        created_at: decision.params().decided_at.clone(),
    })
    .map_err(core("memory-record-invalid"))?;

    let mut superseded_events = Vec::new();
    let old_key = optional_string(input, "supersedes_memory_key");
    let old_before = old_key.and_then(|key| state.records.get(key).cloned());
    if let (Some(old_key), Some(old_record)) = (old_key, old_before.as_ref()) {
        let old_events = state
            .events
            .get(old_key)
            .ok_or_else(|| invalid_operation("superseded-event-chain-missing"))?;
        let previous = old_events
            .last()
            .ok_or_else(|| invalid_operation("superseded-event-tip-missing"))?;
        let key = format!("{old_key}-superseded-by-v{version}");
        superseded_events.push(
            MemoryStateEvent::new(MemoryStateEventParams {
                event_id: state.stable_id("MemoryStateEvent", &key)?,
                namespace_id: state.namespace_id.clone(),
                memory_id: old_record.params().memory_id.clone(),
                previous_event_id: Some(previous.params().event_id.clone()),
                event_type: MemoryEventType::Superseded,
                from_state: Some(MemoryState::Confirmed),
                cause_ref: EvidenceRef::new(EvidenceType::MemoryRecord, memory_id.clone()),
                related_memory_ids: vec![memory_id.clone()],
                actor: decision.params().decided_by.clone(),
                reason_code: text("fixture-superseded")?,
                effective_at: Some(timestamp(string(input, "effective_at")?)?),
                occurred_at: decision.params().decided_at.clone(),
            })
            .map_err(core("superseded-event-invalid"))?,
        );
    }
    state
        .storage
        .database
        .materialize_accepted_memory(&record, &initial_event, &superseded_events)
        .map_err(storage("memory-materialization-failed"))?;
    state.records.insert(memory_key.to_owned(), record.clone());
    state
        .record_keys_by_id
        .insert(memory_id.clone(), memory_key.to_owned());
    state
        .events
        .insert(memory_key.to_owned(), vec![initial_event.clone()]);
    if let Some(old_key) = old_key {
        let loaded_old = state
            .storage
            .database
            .load_memory_record(
                &state.namespace_id,
                &state
                    .records
                    .get(old_key)
                    .ok_or_else(|| invalid_operation("superseded-record-missing"))?
                    .params()
                    .memory_id,
            )
            .map_err(storage("superseded-record-load-failed"))?
            .ok_or_else(|| invalid_operation("superseded-record-load-missing"))?;
        state.records.insert(old_key.to_owned(), loaded_old);
        state
            .events
            .get_mut(old_key)
            .ok_or_else(|| invalid_operation("superseded-events-missing"))?
            .extend(superseded_events.clone());
    }
    let loaded = state
        .storage
        .database
        .load_memory_record(&state.namespace_id, &memory_id)
        .map_err(storage("memory-record-load-failed"))?;
    let record_resolves = loaded.as_ref() == Some(&record)
        && record.params().origin_proposal_id == proposal.params().proposal_id
        && record.params().accepted_by_decision_id == decision.params().decision_id;
    let old_not_mutated = match (old_before.as_ref(), old_key) {
        (None, _) => true,
        (Some(old), Some(key)) => state.records.get(key).is_some_and(|current| {
            current.params().content == old.params().content
                && current.params().content_digest == old.params().content_digest
                && current.params().current_state == MemoryState::Superseded
        }),
        (Some(_), None) => false,
    };
    state.metrics.silent_overwrite_count += u64::from(!old_not_mutated);
    let boundary_matches = superseded_events.first().is_none_or(|event| {
        event.params().effective_at.as_ref() == record.params().valid_time.start_at()
    });
    state.emit(
        memory_key,
        &memory_id,
        Some((
            record.params().content_digest.profile().as_str(),
            record.params().content_digest.value(),
        )),
    );
    state.emit(&event_key, &event_id, None);
    Ok(OperationOutcome::succeeded([
        ("confirmed-record-has-source-and-decision", record_resolves),
        ("initial-state-event-created", loaded.is_some()),
        ("old-record-not-mutated", old_not_mutated),
        ("superseded-event-boundary-matches", boundary_matches),
    ]))
}

pub fn attempt_duplicate_proposal(
    state: &mut ScenarioState,
    input: &Value,
) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    let proposal_key = string(input, "original_proposal_key")?;
    if !crate::fixture::bool_value(input, "same_source")?
        || !crate::fixture::bool_value(input, "same_content")?
        || !crate::fixture::bool_value(input, "same_targets")?
    {
        return Err(invalid_operation("duplicate-proposal-shape-unsupported"));
    }
    let original = state
        .proposals
        .get(proposal_key)
        .ok_or_else(|| invalid_operation("duplicate-proposal-unresolved"))?;
    let mut params = original.params().clone();
    params.proposal_id = state.helper_id("duplicate-proposal-attempt", proposal_key)?;
    let duplicate = MemoryProposal::new(params).map_err(core("duplicate-proposal-invalid"))?;
    let rejected = match state.storage.database.store_memory_proposal(&duplicate) {
        Ok(()) => false,
        Err(error) => {
            error.code() == SqliteErrorCode::Conflict
                && error.storage_reason() == Some(SqliteStorageReason::DuplicateProposal)
        }
    };
    state.metrics.duplicate_reproposal_count += u64::from(!rejected);
    Ok(
        OperationOutcome::succeeded([("duplicate-proposal-suppressed", rejected)])
            .with_status(if rejected { "rejected" } else { "succeeded" }),
    )
}

fn parse_valid_time(value: &serde_json::Map<String, Value>) -> RunnerResult<ValidTime> {
    let mode = match string(value, "mode")? {
        "unknown" => ValidTimeMode::Unknown,
        "instant" => ValidTimeMode::Instant,
        "interval" => ValidTimeMode::Interval,
        "open_ended" => ValidTimeMode::OpenEnded,
        _ => return Err(invalid_operation("valid-time-mode-unsupported")),
    };
    let precision = match string(value, "precision")? {
        "exact" => TimePrecision::Exact,
        "day" => TimePrecision::Day,
        "month" => TimePrecision::Month,
        "year" => TimePrecision::Year,
        "unknown" => TimePrecision::Unknown,
        _ => return Err(invalid_operation("time-precision-unsupported")),
    };
    ValidTime::new(
        mode,
        optional_string(value, "start_at")
            .map(timestamp)
            .transpose()?,
        optional_string(value, "end_at")
            .map(timestamp)
            .transpose()?,
        precision,
    )
    .map_err(core("valid-time-invalid"))
}

fn parse_memory_type(value: &str) -> RunnerResult<MemoryType> {
    match value {
        "observation" => Ok(MemoryType::Observation),
        "claim" => Ok(MemoryType::Claim),
        "episode" => Ok(MemoryType::Episode),
        "preference" => Ok(MemoryType::Preference),
        "procedure" => Ok(MemoryType::Procedure),
        _ => Err(invalid_operation("memory-type-unsupported")),
    }
}

fn number(object: &serde_json::Map<String, Value>, key: &str) -> RunnerResult<f64> {
    object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid_operation("numeric-input-invalid"))
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
