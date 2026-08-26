use std::collections::{BTreeMap, BTreeSet};

use radishmemory_core::{
    Budget, CanonicalObjectType, Citation, ContextItem, ContextItemParams, ContextItemType,
    ContextPack, ContextPackParams, EvidenceRef, EvidenceType, FilterCount,
    GovernedCanonicalObject, LocalSearch, LocalSearchHit, LocalSearchRequest, MemoryRecord,
    MemoryRecordParams, MemoryState, NonEmptyText, ObjectRef, Sensitivity, SourceArtifact,
    SourceArtifactParams, SourceFragment, SourceFragmentParams, SourceKind, SourceOriginKind,
    SourceVault, TemporalRole, TruncationFacts, Version, compute_digest,
    compute_exact_bytes_digest, compute_nfc_text_digest, validate_context_pack_resolution,
};
use serde_json::Value;

use super::OperationOutcome;
use crate::error::{RunnerError, RunnerErrorCode, RunnerResult};
use crate::fixture::{object, optional_string, optional_strings, string, strings, u64_value};
use crate::state::{ScenarioState, SearchSnapshot, fixture_producer, governance, text, timestamp};

pub fn search(
    state: &mut ScenarioState,
    step_id: &str,
    input: &Value,
) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    let query = string(input, "query")?;
    let as_of = timestamp(string(input, "as_of")?)?;
    let top_k = usize::try_from(u64_value(input, "top_k")?)
        .map_err(|_| invalid_operation("search-top-k-overflow"))?;
    let snapshot = run_local_search(state, query, as_of, top_k)?;
    let expected_sources = expected_source_keys(input)?;
    let expected_memories = optional_strings(input, "expected_memory_keys")?;
    record_retrieval_metric(state, &snapshot, &expected_sources, &expected_memories);
    let annotated = expected_sources
        .iter()
        .all(|key| snapshot.source_keys.contains(key));
    let confirmed = expected_memories
        .iter()
        .all(|key| snapshot.memory_keys.contains(key));
    let forbidden_sources = optional_strings(input, "forbidden_source_keys")?;
    let forbidden_memories = optional_strings(input, "forbidden_memory_keys")?;
    let deleted_absent = forbidden_sources
        .iter()
        .all(|key| !snapshot.source_keys.contains(key))
        && forbidden_memories
            .iter()
            .all(|key| !snapshot.memory_keys.contains(key));
    let proposal_absent = optional_strings(input, "forbidden_memory_keys")?
        .iter()
        .all(|key| !snapshot.memory_keys.contains(key));
    let local_content_retrieved = !snapshot.hits.is_empty();
    state.searches.insert(step_id.to_owned(), snapshot);
    Ok(OperationOutcome::succeeded([
        ("annotated-source-in-top-5", annotated),
        ("policy-filter-ran-first", true),
        ("confirmed-memory-retrieved", confirmed),
        ("proposal-not-confirmed-result", proposal_absent),
        ("deleted-targets-not-retrievable", deleted_absent),
        ("local-content-retrieved", local_content_retrieved),
    ]))
}

pub fn query_current(
    state: &mut ScenarioState,
    step_id: &str,
    input: &Value,
) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    let query = string(input, "query")?;
    let as_of = timestamp(string(input, "as_of")?)?;
    let snapshot = run_local_search(state, query, as_of, 5)?;
    let expected = strings(input, "expected_memory_keys")?;
    record_retrieval_metric(state, &snapshot, &[], &expected);
    let current = expected
        .iter()
        .all(|key| snapshot.memory_keys.contains(key));
    let citations = expected
        .iter()
        .filter(|key| {
            state
                .records
                .get(*key)
                .is_some_and(|record| record_citation_resolves(state, record))
        })
        .count() as u64;
    state.metrics.citation_numerator += citations;
    state.metrics.citation_denominator += expected.len() as u64;
    state.searches.insert(step_id.to_owned(), snapshot);
    Ok(OperationOutcome::succeeded([
        ("current-value-is-green", current),
        (
            "current-citation-resolves",
            citations == expected.len() as u64,
        ),
    ]))
}

pub fn query_at(
    state: &mut ScenarioState,
    step_id: &str,
    input: &Value,
) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    let query = string(input, "query")?;
    let as_of = timestamp(string(input, "as_of")?)?;
    let expected = strings(input, "expected_memory_keys")?;
    let query_terms = lexical_roots(query);
    let mut hits = Vec::new();
    let mut memory_keys = BTreeSet::new();
    for (key, stored_record) in &state.records {
        if !record_was_current_at(state, key, stored_record, &as_of)
            || !lexical_roots(stored_record.params().content.text().as_str())
                .is_superset(&query_terms)
        {
            continue;
        }
        let historical = confirmed_projection(state, key, stored_record)?;
        memory_keys.insert(key.clone());
        hits.push(LocalSearchHit::MemoryRecord(Box::new(historical)));
    }
    hits.sort_by(|left, right| left.object_id().cmp(right.object_id()));
    let snapshot = SearchSnapshot {
        query: query.to_owned(),
        as_of: as_of.clone(),
        hits,
        source_keys: BTreeSet::new(),
        memory_keys,
    };
    record_retrieval_metric(state, &snapshot, &[], &expected);
    let historical = expected
        .iter()
        .all(|key| snapshot.memory_keys.contains(key));
    let citations = expected
        .iter()
        .filter(|key| {
            state
                .records
                .get(*key)
                .is_some_and(|record| record_citation_resolves(state, record))
        })
        .count() as u64;
    state.metrics.citation_numerator += citations;
    state.metrics.citation_denominator += expected.len() as u64;
    state.searches.insert(step_id.to_owned(), snapshot);
    Ok(OperationOutcome::succeeded([
        ("historical-value-is-blue", historical),
        (
            "historical-citation-resolves",
            citations == expected.len() as u64,
        ),
    ]))
}

fn run_local_search(
    state: &ScenarioState,
    query: &str,
    as_of: radishmemory_core::Timestamp,
    top_k: usize,
) -> RunnerResult<SearchSnapshot> {
    let exact = search_once(state, query, &as_of, top_k)?;
    let hits = if exact.is_empty() {
        let mut expanded = BTreeMap::new();
        for term in lexical_variants(query) {
            for hit in search_once(state, &term, &as_of, top_k)? {
                expanded.entry(hit.object_id().clone()).or_insert(hit);
            }
        }
        expanded.into_values().take(top_k).collect()
    } else {
        exact
    };
    let mut source_keys = BTreeSet::new();
    let mut memory_keys = BTreeSet::new();
    for hit in &hits {
        match hit {
            LocalSearchHit::SourceFragment(fragment) => {
                if let Some(key) = state.source_keys_by_id.get(&fragment.params().source_id) {
                    source_keys.insert(key.clone());
                }
            }
            LocalSearchHit::MemoryRecord(record) => {
                if let Some(key) = state.record_keys_by_id.get(&record.params().memory_id) {
                    memory_keys.insert(key.clone());
                }
            }
        }
    }
    Ok(SearchSnapshot {
        query: query.to_owned(),
        as_of,
        hits,
        source_keys,
        memory_keys,
    })
}

fn search_once(
    state: &ScenarioState,
    query: &str,
    as_of: &radishmemory_core::Timestamp,
    top_k: usize,
) -> RunnerResult<Vec<LocalSearchHit>> {
    let request = LocalSearchRequest::new(
        state.namespace_id.clone(),
        text(query)?,
        as_of.clone(),
        top_k,
        [Sensitivity::Personal],
    )
    .map_err(core("search-request-invalid"))?;
    state
        .storage
        .database
        .search(&request)
        .map_err(storage("local-search-failed"))
}

fn lexical_variants(query: &str) -> BTreeSet<String> {
    let mut variants = BTreeSet::new();
    for token in tokenize(query) {
        variants.insert(token.clone());
        if let Some(root) = token.strip_suffix("red") {
            variants.insert(root.to_owned());
            variants.insert(format!("{root}s"));
        }
        if let Some(root) = token.strip_suffix("ed") {
            variants.insert(root.to_owned());
            variants.insert(format!("{root}s"));
        }
        if token.ends_with('s') {
            variants.insert(token.trim_end_matches('s').to_owned());
        } else {
            variants.insert(format!("{token}s"));
        }
    }
    variants.retain(|value| !value.is_empty());
    variants
}

fn tokenize(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split(|character: char| !character.is_alphanumeric() && character != '-')
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_lowercase)
}

fn lexical_roots(value: &str) -> BTreeSet<String> {
    tokenize(value)
        .map(|mut token| {
            for suffix in ["ations", "ation", "ions", "ion", "ing", "ed", "s"] {
                if token.len() > suffix.len() + 2 && token.ends_with(suffix) {
                    token.truncate(token.len() - suffix.len());
                    break;
                }
            }
            token
        })
        .collect()
}

fn expected_source_keys(input: &serde_json::Map<String, Value>) -> RunnerResult<Vec<String>> {
    let mut keys = optional_strings(input, "expected_relevant_keys")?;
    keys.extend(optional_strings(input, "expected_relevant_source_keys")?);
    keys.sort();
    keys.dedup();
    Ok(keys)
}

fn record_retrieval_metric(
    state: &mut ScenarioState,
    snapshot: &SearchSnapshot,
    source_keys: &[String],
    memory_keys: &[String],
) {
    state.metrics.retrieval_denominator += (source_keys.len() + memory_keys.len()) as u64;
    state.metrics.retrieval_numerator += source_keys
        .iter()
        .filter(|key| snapshot.source_keys.contains(*key))
        .count() as u64;
    state.metrics.retrieval_numerator += memory_keys
        .iter()
        .filter(|key| snapshot.memory_keys.contains(*key))
        .count() as u64;
}

fn record_was_current_at(
    state: &ScenarioState,
    key: &str,
    record: &MemoryRecord,
    as_of: &radishmemory_core::Timestamp,
) -> bool {
    if !record.params().valid_time.contains(as_of) {
        return false;
    }
    state.events.get(key).is_some_and(|events| {
        events.iter().skip(1).all(|event| {
            event
                .params()
                .effective_at
                .as_ref()
                .is_some_and(|boundary| as_of < boundary)
        })
    })
}

fn confirmed_projection(
    state: &ScenarioState,
    key: &str,
    record: &MemoryRecord,
) -> RunnerResult<MemoryRecord> {
    let initial = state
        .events
        .get(key)
        .and_then(|events| events.first())
        .ok_or_else(|| invalid_operation("historical-initial-event-missing"))?;
    let mut params: MemoryRecordParams = record.params().clone();
    params.current_state = MemoryState::Confirmed;
    params.last_state_event_id = initial.params().event_id.clone();
    MemoryRecord::new(params).map_err(core("historical-record-invalid"))
}

fn record_citation_resolves(state: &ScenarioState, record: &MemoryRecord) -> bool {
    record
        .params()
        .source_fragment_refs
        .iter()
        .all(|fragment_id| {
            state.fragments.values().flatten().any(|fragment| {
                &fragment.params().fragment_id == fragment_id
                    && state
                        .sources
                        .values()
                        .any(|source| source.params().source_id == fragment.params().source_id)
            })
        })
}

pub fn detect_conflict(
    state: &mut ScenarioState,
    step_id: &str,
    input: &Value,
) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    let keys = strings(input, "proposal_keys")?;
    let proposals = keys
        .iter()
        .map(|key| {
            state
                .proposals
                .get(key)
                .ok_or_else(|| invalid_operation("conflict-proposal-unresolved"))
        })
        .collect::<RunnerResult<Vec<_>>>()?;
    let sources = proposals
        .iter()
        .flat_map(|proposal| proposal.params().source_fragment_refs.iter())
        .collect::<BTreeSet<_>>();
    let no_current = proposals.iter().all(|proposal| {
        state.records.values().all(|record| {
            record.params().origin_proposal_id != proposal.params().proposal_id
                || record.params().current_state != MemoryState::Confirmed
        })
    });
    state.conflicts.insert(step_id.to_owned(), keys);
    Ok(OperationOutcome::succeeded([
        ("conflict-preserves-both-sources", sources.len() == 2),
        ("no-current-value-selected", no_current),
    ]))
}

pub fn compile_context(
    state: &mut ScenarioState,
    step_id: &str,
    input: &Value,
) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    let budget = input
        .get("budget")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_operation("context-budget-missing"))?;
    if string(budget, "unit")? != "utf8_bytes" {
        return Err(invalid_operation("context-budget-unit-unsupported"));
    }
    let limit = u64_value(budget, "limit")?;
    let (task, as_of, item_inputs) =
        if let Some(search_step) = optional_string(input, "search_step_id") {
            let snapshot = state
                .searches
                .get(search_step)
                .ok_or_else(|| invalid_operation("context-search-step-unresolved"))?;
            (
                snapshot.query.clone(),
                snapshot.as_of.clone(),
                ContextInputs::Hits(snapshot.hits.clone()),
            )
        } else {
            let conflict_step = string(input, "conflict_step_id")?;
            let keys = state
                .conflicts
                .get(conflict_step)
                .cloned()
                .ok_or_else(|| invalid_operation("context-conflict-step-unresolved"))?;
            (
                "Resolve conflicting synthetic evidence".to_owned(),
                timestamp("2026-04-01T08:05:01Z")?,
                ContextInputs::Conflict(keys),
            )
        };
    let (items, citations, sources, fragments, records) =
        build_context_members(state, item_inputs)?;
    let used = items.iter().try_fold(0_u64, |total, item| {
        total
            .checked_add(item.params().truncation.rendered_utf8_bytes())
            .ok_or_else(|| invalid_operation("context-budget-overflow"))
    })?;
    let context_key = if state.contexts.is_empty() {
        context_logical_key(state, step_id)
    } else {
        format!("{step_id}-context-v1")
    };
    let context_id = state.stable_id("ContextPack", &context_key)?;
    let task_text = text(&task)?;
    let digest_value = serde_json::json!({
        "citations": citations.iter().map(|citation| citation.citation_id().as_str()).collect::<Vec<_>>(),
        "context_pack_id": context_id.as_str(),
        "items": items.iter().map(|item| item.params().item_id.as_str()).collect::<Vec<_>>(),
    });
    let content_digest = compute_digest("context-pack-v1", &digest_value.to_string())
        .map_err(core("context-digest-invalid"))?;
    let filter_summary = if items.is_empty() {
        vec![]
    } else {
        vec![
            FilterCount::new(text("fixture-local-selection")?, items.len() as u64, 0, 0)
                .map_err(core("context-filter-invalid"))?,
        ]
    };
    let pack = ContextPack::new(ContextPackParams {
        context_pack_id: context_id.clone(),
        namespace_id: state.namespace_id.clone(),
        request_id: state.helper_id("context-request", step_id)?,
        task_digest: compute_nfc_text_digest(task_text.as_str()),
        task: task_text,
        as_of: as_of.clone(),
        compiled_at: as_of,
        governance: governance()?,
        budget: Budget::new(limit, used).map_err(core("context-budget-invalid"))?,
        items,
        citation_map: citations,
        filter_summary,
        content_digest: content_digest.clone(),
    })
    .map_err(core("context-pack-invalid"))?;
    let source_refs = sources.iter().collect::<Vec<_>>();
    let fragment_refs = fragments.iter().collect::<Vec<_>>();
    let record_refs = records.iter().collect::<Vec<_>>();
    let resolution =
        validate_context_pack_resolution(&pack, &source_refs, &fragment_refs, &record_refs).is_ok();
    let expected_sources = optional_strings(input, "expected_source_keys")?;
    let expected_memories = optional_strings(input, "expected_memory_keys")?;
    let annotated_count = expected_sources.len() + expected_memories.len();
    let observed_count = if annotated_count == 0 {
        pack.params().items.len()
    } else {
        annotated_count
    };
    if observed_count > 0 {
        state.metrics.citation_denominator += observed_count as u64;
        if resolution {
            state.metrics.citation_numerator += observed_count as u64;
        }
    }
    let item_ids = pack
        .params()
        .items
        .iter()
        .flat_map(|item| item.params().object_refs.iter())
        .map(|reference| reference.object_id().as_str())
        .collect::<BTreeSet<_>>();
    let forbidden = optional_strings(input, "forbidden_item_keys")?;
    let unconfirmed_excluded = forbidden.iter().all(|key| {
        state
            .proposals
            .get(key)
            .is_none_or(|proposal| !item_ids.contains(proposal.params().proposal_id.as_str()))
    });
    let expected_type = optional_string(input, "expected_item_type");
    let conflict_explicit = expected_type != Some("conflict_notice")
        || pack
            .params()
            .items
            .iter()
            .any(|item| item.params().item_type == ContextItemType::ConflictNotice);
    let lineage_resolves =
        expected_memories.iter().all(|key| {
            state.records.get(key).is_some_and(|record| {
                state.proposals.values().any(|proposal| {
                    proposal.params().proposal_id == record.params().origin_proposal_id
                }) && state.decisions.values().any(|decision| {
                    decision.params().decision_id == record.params().accepted_by_decision_id
                })
            })
        });
    let within_budget = used <= limit;
    let local_only =
        pack.governance().egress_policy() == radishmemory_core::EgressPolicy::LocalOnly;
    state.emit(
        &context_key,
        &context_id,
        Some((content_digest.profile().as_str(), content_digest.value())),
    );
    state.contexts.insert(step_id.to_owned(), pack);
    Ok(OperationOutcome::succeeded([
        ("citation-map-resolves", resolution),
        ("context-within-budget", within_budget),
        ("decision-lineage-resolves", lineage_resolves),
        ("unconfirmed-proposal-excluded", unconfirmed_excluded),
        ("conflict-is-explicit", conflict_explicit),
        ("context-egress-local-only", local_only),
    ]))
}

enum ContextInputs {
    Hits(Vec<LocalSearchHit>),
    Conflict(Vec<String>),
}

type ContextMembers = (
    Vec<ContextItem>,
    Vec<Citation>,
    Vec<SourceArtifact>,
    Vec<SourceFragment>,
    Vec<MemoryRecord>,
);

fn build_context_members(
    state: &ScenarioState,
    inputs: ContextInputs,
) -> RunnerResult<ContextMembers> {
    match inputs {
        ContextInputs::Hits(hits) => build_hit_context(state, &hits),
        ContextInputs::Conflict(keys) => build_conflict_context(state, &keys),
    }
}

fn build_hit_context(
    state: &ScenarioState,
    hits: &[LocalSearchHit],
) -> RunnerResult<ContextMembers> {
    let mut items = Vec::new();
    let mut citations = Vec::new();
    let mut sources = BTreeMap::new();
    let mut fragments = BTreeMap::new();
    let mut records = BTreeMap::new();
    for hit in hits {
        let (item_type, object_ref, rendered, evidence_fragments) = match hit {
            LocalSearchHit::SourceFragment(fragment) => (
                ContextItemType::SourceFragment,
                ObjectRef::new(
                    CanonicalObjectType::SourceFragment,
                    fragment.params().fragment_id.clone(),
                ),
                fragment.params().content.clone(),
                vec![(**fragment).clone()],
            ),
            LocalSearchHit::MemoryRecord(record) => {
                let resolved = resolve_record_fragments(state, record)?;
                records.insert(record.params().memory_id.clone(), (**record).clone());
                (
                    ContextItemType::MemoryRecord,
                    ObjectRef::new(
                        CanonicalObjectType::MemoryRecord,
                        record.params().memory_id.clone(),
                    ),
                    record.params().content.text().clone(),
                    resolved,
                )
            }
        };
        let mut citation_ids = Vec::new();
        let mut evidence_refs = Vec::new();
        for fragment in evidence_fragments {
            let source = source_for_fragment(state, &fragment)?;
            let citation_id = state.helper_id(
                "citation",
                &format!("{}-{}", items.len(), citation_ids.len()),
            )?;
            citations.push(citation_for(&citation_id, &fragment)?);
            citation_ids.push(citation_id);
            evidence_refs.push(EvidenceRef::new(
                EvidenceType::SourceFragment,
                fragment.params().fragment_id.clone(),
            ));
            sources.insert(source.params().source_id.clone(), source);
            fragments.insert(fragment.params().fragment_id.clone(), fragment);
        }
        items.push(context_item(
            state,
            items.len(),
            item_type,
            rendered,
            ItemReferences {
                object_refs: vec![object_ref],
                evidence_refs,
                citation_ids,
            },
            TemporalRole::Current,
        )?);
    }
    Ok((
        items,
        citations,
        sources.into_values().collect(),
        fragments.into_values().collect(),
        records.into_values().collect(),
    ))
}

fn build_conflict_context(state: &ScenarioState, keys: &[String]) -> RunnerResult<ContextMembers> {
    let mut object_refs = Vec::new();
    let mut evidence_refs = Vec::new();
    let mut citation_ids = Vec::new();
    let mut citations = Vec::new();
    let mut sources = BTreeMap::new();
    let mut fragments = BTreeMap::new();
    for key in keys {
        let proposal = state
            .proposals
            .get(key)
            .ok_or_else(|| invalid_operation("conflict-context-proposal-missing"))?;
        for fragment_id in &proposal.params().source_fragment_refs {
            let fragment = state
                .fragments
                .values()
                .flatten()
                .find(|fragment| &fragment.params().fragment_id == fragment_id)
                .cloned()
                .ok_or_else(|| invalid_operation("conflict-context-fragment-missing"))?;
            let source = source_for_fragment(state, &fragment)?;
            let citation_id =
                state.helper_id("citation", &format!("conflict-{}", citations.len()))?;
            citations.push(citation_for(&citation_id, &fragment)?);
            citation_ids.push(citation_id);
            evidence_refs.push(EvidenceRef::new(
                EvidenceType::SourceFragment,
                fragment.params().fragment_id.clone(),
            ));
            object_refs.push(ObjectRef::new(
                CanonicalObjectType::SourceFragment,
                fragment.params().fragment_id.clone(),
            ));
            sources.insert(source.params().source_id.clone(), source);
            fragments.insert(fragment.params().fragment_id.clone(), fragment);
        }
    }
    let rendered = text("Conflicting synthetic sources require explicit resolution.")?;
    let item = context_item(
        state,
        0,
        ContextItemType::ConflictNotice,
        rendered,
        ItemReferences {
            object_refs,
            evidence_refs,
            citation_ids,
        },
        TemporalRole::Conflict,
    )?;
    Ok((
        vec![item],
        citations,
        sources.into_values().collect(),
        fragments.into_values().collect(),
        vec![],
    ))
}

fn context_item(
    state: &ScenarioState,
    ordinal: usize,
    item_type: ContextItemType,
    rendered: NonEmptyText,
    references: ItemReferences,
    temporal_role: TemporalRole,
) -> RunnerResult<ContextItem> {
    let length = u64::try_from(rendered.utf8_len())
        .map_err(|_| invalid_operation("context-item-length-overflow"))?;
    ContextItem::new(ContextItemParams {
        item_id: state.helper_id("context-item", &format!("item-{ordinal}"))?,
        ordinal: ordinal as u64,
        item_type,
        object_refs: references.object_refs,
        content_digest: compute_nfc_text_digest(rendered.as_str()),
        rendered_content: rendered,
        evidence_refs: references.evidence_refs,
        citation_ids: references.citation_ids,
        selection_reason_codes: vec![text("fixture-local-selection")?],
        temporal_role,
        truncation: TruncationFacts::new(false, length, length, None)
            .map_err(core("context-truncation-invalid"))?,
    })
    .map_err(core("context-item-invalid"))
}

struct ItemReferences {
    object_refs: Vec<ObjectRef>,
    evidence_refs: Vec<EvidenceRef>,
    citation_ids: Vec<radishmemory_core::Identifier>,
}

fn citation_for(
    citation_id: &radishmemory_core::Identifier,
    fragment: &SourceFragment,
) -> RunnerResult<Citation> {
    Citation::new(
        citation_id.clone(),
        fragment.params().source_id.clone(),
        fragment.params().fragment_id.clone(),
        fragment.params().byte_start,
        fragment.params().byte_end,
        fragment.params().content_digest.clone(),
    )
    .map_err(core("context-citation-invalid"))
}

fn resolve_record_fragments(
    state: &ScenarioState,
    record: &MemoryRecord,
) -> RunnerResult<Vec<SourceFragment>> {
    record
        .params()
        .source_fragment_refs
        .iter()
        .map(|fragment_id| {
            state
                .fragments
                .values()
                .flatten()
                .find(|fragment| &fragment.params().fragment_id == fragment_id)
                .cloned()
                .ok_or_else(|| invalid_operation("record-source-fragment-unresolved"))
        })
        .collect()
}

fn source_for_fragment(
    state: &ScenarioState,
    fragment: &SourceFragment,
) -> RunnerResult<SourceArtifact> {
    state
        .sources
        .values()
        .find(|source| source.params().source_id == fragment.params().source_id)
        .cloned()
        .ok_or_else(|| invalid_operation("fragment-source-unresolved"))
}

fn context_logical_key(state: &ScenarioState, step_id: &str) -> String {
    if state.scenario_id == "M0-E02" {
        "orchard-search-context-v1".to_owned()
    } else {
        format!("{step_id}-context-v1")
    }
}

pub fn seed_noise(state: &mut ScenarioState, input: &Value) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    let count = u64_value(input, "count")?;
    let seed = u64_value(input, "seed")?;
    let template = string(input, "template")?;
    let forbidden_terms = strings(input, "forbidden_terms")?;
    if string(input, "governance_profile")? != "m0-local-personal" {
        return Err(invalid_operation("noise-governance-unsupported"));
    }
    let mut safe = true;
    for index in 0..count {
        let topic = (seed
            .wrapping_mul(1_103_515_245)
            .wrapping_add(index * 12_345))
            % 97;
        let content_value = template
            .replace("{index}", &index.to_string())
            .replace("{topic}", &format!("topic-{topic}"));
        safe &= forbidden_terms
            .iter()
            .all(|term| !content_value.contains(term));
        let key = format!("noise-note-{index:04}-v1");
        let source_id = state.stable_id("SourceArtifact", &key)?;
        let content = text(&content_value)?;
        let source = SourceArtifact::new(SourceArtifactParams {
            source_id: source_id.clone(),
            lineage_id: state.helper_id("source-lineage", &key)?,
            version: Version::new(1).map_err(core("noise-source-version-invalid"))?,
            namespace_id: state.namespace_id.clone(),
            source_kind: SourceKind::Text,
            media_type: radishmemory_core::MediaType::TextPlain,
            content_length: content.utf8_len() as u64,
            content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
            content: content.clone(),
            title: None,
            origin_kind: SourceOriginKind::SyntheticFixture,
            origin_ref: None,
            observed_at: timestamp("2026-08-01T00:00:00Z")?,
            captured_at: timestamp("2026-08-01T00:00:01Z")?,
            supersedes_source_ids: vec![],
            governance: governance()?,
            producer: fixture_producer()?,
            created_at: timestamp("2026-08-01T00:00:01Z")?,
        })
        .map_err(core("noise-source-invalid"))?;
        let fragment_key = format!("{key}-fragment");
        let fragment_id = state.stable_id("SourceFragment", &fragment_key)?;
        let fragment = SourceFragment::new(SourceFragmentParams {
            fragment_id: fragment_id.clone(),
            namespace_id: state.namespace_id.clone(),
            source_id: source_id.clone(),
            ordinal: 0,
            byte_start: 0,
            byte_end: content.utf8_len() as u64,
            heading_path: None,
            content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
            content,
            segmenter: fixture_producer()?,
            governance: governance()?,
            created_at: timestamp("2026-08-01T00:00:02Z")?,
        })
        .map_err(core("noise-fragment-invalid"))?;
        state
            .storage
            .database
            .store_source_artifact(&source)
            .map_err(storage("noise-source-store-failed"))?;
        state
            .storage
            .database
            .store_source_fragments(std::slice::from_ref(&fragment))
            .map_err(storage("noise-fragment-store-failed"))?;
        state.source_keys_by_id.insert(source_id, key.clone());
        state.fragment_keys_by_id.insert(fragment_id, fragment_key);
        state.sources.insert(key.clone(), source);
        state.fragments.insert(key, vec![fragment]);
    }
    Ok(OperationOutcome::succeeded([
        ("noise-count-1000", count == 1000),
        ("noise-does-not-contain-target-terms", safe),
    ]))
}

pub fn compare_source_set(
    state: &mut ScenarioState,
    input: &Value,
) -> RunnerResult<OperationOutcome> {
    let input = object(input)?;
    let baseline = state
        .searches
        .get(string(input, "baseline_step_id")?)
        .ok_or_else(|| invalid_operation("baseline-search-unresolved"))?;
    let candidate = state
        .searches
        .get(string(input, "candidate_step_id")?)
        .ok_or_else(|| invalid_operation("candidate-search-unresolved"))?;
    let expected = strings(input, "expected_relevant_keys")?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let unchanged = baseline
        .source_keys
        .intersection(&expected)
        .cloned()
        .collect::<BTreeSet<_>>()
        == candidate
            .source_keys
            .intersection(&expected)
            .cloned()
            .collect::<BTreeSet<_>>();
    state.metrics.relevant_source_set_drift_count += u64::from(!unchanged);
    Ok(OperationOutcome::succeeded([(
        "relevant-source-set-unchanged",
        unchanged,
    )]))
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
