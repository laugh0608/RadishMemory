use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CanonicalObject, CanonicalObjectType, Citation, ComponentStatus, ContextItemType, ContextPack,
    CoreError, CrossObjectInvariantReason, DeleteRequest, DeletionEvidence, DeletionOverallStatus,
    DeletionState, EvidenceType, Governance, GovernedCanonicalObject, Identifier, MemoryDecision,
    MemoryEventType, MemoryProposal, MemoryRecord, MemoryState, MemoryStateEvent, ObjectRef,
    ProposalOperation, RetentionMode, Sensitivity, SourceArtifact, SourceFragment,
};

/// A fragment together with the source object that resolves its `source_id`.
#[derive(Clone, Copy)]
pub struct ResolvedSource<'a> {
    pub fragment: &'a SourceFragment,
    pub source: &'a SourceArtifact,
}

/// An old memory version together with the event that closes its applicability.
#[derive(Clone, Copy)]
pub struct SupersededTarget<'a> {
    pub record: &'a MemoryRecord,
    pub superseded_event: &'a MemoryStateEvent,
}

/// Validates that derived governance is never weaker than any supplied source.
pub fn validate_governance_derivation(
    derived: &Governance,
    sources: &[&Governance],
) -> Result<(), CoreError> {
    if sources.is_empty() {
        return invariant(CrossObjectInvariantReason::MissingReference);
    }
    for source in sources {
        if sensitivity_rank(derived.sensitivity()) < sensitivity_rank(source.sensitivity())
            || !retention_is_no_later(derived, source)
            || (source.deletion_state() != DeletionState::Active
                && derived.deletion_state() == DeletionState::Active)
        {
            return invariant(CrossObjectInvariantReason::GovernanceMismatch);
        }
    }
    Ok(())
}

/// Resolves a fragment against its active source and verifies its exact byte slice.
pub fn validate_source_fragment_resolution(
    fragment: &SourceFragment,
    source: &SourceArtifact,
) -> Result<(), CoreError> {
    if fragment.params().source_id != source.params().source_id {
        return invariant(CrossObjectInvariantReason::IdentityMismatch);
    }
    require_namespace(fragment.namespace_id(), source.namespace_id())?;
    require_recallable(source.governance())?;
    validate_governance_derivation(fragment.governance(), &[source.governance()])?;

    let start = usize::try_from(fragment.params().byte_start)
        .map_err(|_| cross_error(CrossObjectInvariantReason::SourceSliceMismatch))?;
    let end = usize::try_from(fragment.params().byte_end)
        .map_err(|_| cross_error(CrossObjectInvariantReason::SourceSliceMismatch))?;
    let resolved = source.params().content.as_str().get(start..end);
    if resolved != Some(fragment.params().content.as_str()) {
        return invariant(CrossObjectInvariantReason::SourceSliceMismatch);
    }
    Ok(())
}

/// Validates that one ordered fragment set covers the complete source body exactly once.
pub fn validate_complete_source_fragment_set(
    source: &SourceArtifact,
    fragments: &[SourceFragment],
) -> Result<(), CoreError> {
    if fragments.is_empty() {
        return invariant(CrossObjectInvariantReason::FragmentSetMismatch);
    }
    let mut fragment_ids = BTreeSet::new();
    let mut next_byte = 0_u64;
    for (expected_ordinal, fragment) in fragments.iter().enumerate() {
        validate_source_fragment_resolution(fragment, source)?;
        if fragment.governance() != source.governance()
            || !fragment_ids.insert(&fragment.params().fragment_id)
            || fragment.params().ordinal != expected_ordinal as u64
            || fragment.params().byte_start != next_byte
        {
            return invariant(CrossObjectInvariantReason::FragmentSetMismatch);
        }
        next_byte = fragment.params().byte_end;
    }
    if next_byte != source.params().content_length {
        return invariant(CrossObjectInvariantReason::FragmentSetMismatch);
    }
    Ok(())
}

/// Validates a proposal against the exact fragment/source closure used to create it.
pub fn validate_memory_proposal_sources(
    proposal: &MemoryProposal,
    sources: &[ResolvedSource<'_>],
) -> Result<(), CoreError> {
    require_exact_ids(
        &proposal.params().source_fragment_refs,
        sources
            .iter()
            .map(|resolved| &resolved.fragment.params().fragment_id),
    )?;
    require_recallable(proposal.governance())?;

    let mut source_governance = Vec::with_capacity(sources.len());
    for resolved in sources {
        require_namespace(proposal.namespace_id(), resolved.fragment.namespace_id())?;
        require_namespace(proposal.namespace_id(), resolved.source.namespace_id())?;
        validate_source_fragment_resolution(resolved.fragment, resolved.source)?;
        require_recallable(resolved.fragment.governance())?;
        source_governance.push(resolved.fragment.governance());
    }
    validate_governance_derivation(proposal.governance(), &source_governance)
}

/// Validates the one-to-one accept → record → initial confirmed event closure.
pub fn validate_memory_materialization(
    proposal: &MemoryProposal,
    decision: &MemoryDecision,
    record: &MemoryRecord,
    initial_event: &MemoryStateEvent,
) -> Result<(), CoreError> {
    require_namespace(proposal.namespace_id(), decision.namespace_id())?;
    require_namespace(proposal.namespace_id(), record.namespace_id())?;
    require_namespace(proposal.namespace_id(), initial_event.namespace_id())?;
    require_recallable(proposal.governance())?;
    require_recallable(record.governance())?;

    let decision_params = decision.params();
    let record_params = record.params();
    let event_params = initial_event.params();
    if decision_params.proposal_id != proposal.params().proposal_id
        || decision_params.decision != crate::Decision::Accept
        || decision_params.result_memory_id.as_ref() != Some(&record_params.memory_id)
        || record_params.origin_proposal_id != proposal.params().proposal_id
        || record_params.accepted_by_decision_id != decision_params.decision_id
        || event_params.memory_id != record_params.memory_id
        || event_params.event_type != MemoryEventType::Confirmed
        || event_params.cause_ref.evidence_type() != EvidenceType::MemoryDecision
        || event_params.cause_ref.evidence_id() != &decision_params.decision_id
    {
        return invariant(CrossObjectInvariantReason::MaterializationMismatch);
    }

    let proposal_params = proposal.params();
    if record_params.memory_type != proposal_params.memory_type
        || record_params.subject_ref != proposal_params.subject_ref
        || record_params.content != proposal_params.proposed_content
        || !same_identifier_set(
            &record_params.source_fragment_refs,
            &proposal_params.source_fragment_refs,
        )
        || record_params.observed_at != proposal_params.observed_at
        || record_params.valid_time != proposal_params.valid_time
        || record_params.confidence != proposal_params.confidence
        || record_params.importance != proposal_params.importance
        || !same_identifier_set(
            &record_params.supersedes_memory_ids,
            &proposal_params.target_memory_ids,
        )
    {
        return invariant(CrossObjectInvariantReason::MaterializationMismatch);
    }
    validate_governance_derivation(record.governance(), &[proposal.governance()])?;

    if record_params.current_state != MemoryState::Confirmed
        || record_params.last_state_event_id != event_params.event_id
        || event_params.previous_event_id.is_some()
        || event_params.from_state.is_some()
        || event_params.effective_at.is_some()
        || initial_event.to_state() != MemoryState::Confirmed
    {
        return invariant(CrossObjectInvariantReason::StateProjectionMismatch);
    }
    Ok(())
}

/// Validates an unordered event set as one unbranched chain and checks its record projection.
pub fn validate_memory_event_chain(
    record: &MemoryRecord,
    events: &[&MemoryStateEvent],
) -> Result<(), CoreError> {
    if events.is_empty() {
        return invariant(CrossObjectInvariantReason::MissingReference);
    }

    let mut by_id = BTreeMap::new();
    for event in events {
        require_namespace(record.namespace_id(), event.namespace_id())?;
        if event.params().memory_id != record.params().memory_id {
            return invariant(CrossObjectInvariantReason::IdentityMismatch);
        }
        if by_id
            .insert(event.params().event_id.clone(), *event)
            .is_some()
        {
            return invariant(CrossObjectInvariantReason::DuplicateObject);
        }
    }

    let initial_events = events
        .iter()
        .copied()
        .filter(|event| event.params().previous_event_id.is_none())
        .collect::<Vec<_>>();
    if initial_events.len() != 1
        || initial_events[0].params().event_type != MemoryEventType::Confirmed
    {
        return invariant(CrossObjectInvariantReason::EventChainConflict);
    }

    let mut current_event = initial_events[0];
    let mut current_state = current_event.to_state();
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(current_event.params().event_id.clone()) {
            return invariant(CrossObjectInvariantReason::EventChainConflict);
        }
        let next_events = events
            .iter()
            .copied()
            .filter(|event| {
                event.params().previous_event_id.as_ref() == Some(&current_event.params().event_id)
            })
            .collect::<Vec<_>>();
        if next_events.len() > 1 {
            return invariant(CrossObjectInvariantReason::EventChainConflict);
        }
        let Some(next_event) = next_events.first().copied() else {
            break;
        };
        if next_event.params().from_state != Some(current_state) {
            return invariant(CrossObjectInvariantReason::EventChainConflict);
        }
        current_event = next_event;
        current_state = current_event.to_state();
    }

    if visited.len() != by_id.len() {
        return invariant(CrossObjectInvariantReason::EventChainConflict);
    }
    if record.params().current_state != current_state
        || record.params().last_state_event_id != current_event.params().event_id
    {
        return invariant(CrossObjectInvariantReason::StateProjectionMismatch);
    }
    Ok(())
}

/// Validates a supersede proposal, its new record, and every old record closure event.
pub fn validate_memory_supersession(
    proposal: &MemoryProposal,
    new_record: &MemoryRecord,
    targets: &[SupersededTarget<'_>],
) -> Result<(), CoreError> {
    if proposal.params().operation != ProposalOperation::Supersede {
        return invariant(CrossObjectInvariantReason::SupersessionMismatch);
    }
    require_namespace(proposal.namespace_id(), new_record.namespace_id())?;
    require_exact_ids(
        &proposal.params().target_memory_ids,
        targets
            .iter()
            .map(|target| &target.record.params().memory_id),
    )?;
    if !same_identifier_set(
        &new_record.params().supersedes_memory_ids,
        &proposal.params().target_memory_ids,
    ) {
        return invariant(CrossObjectInvariantReason::SupersessionMismatch);
    }

    let mut maximum_version = 0_u64;
    for target in targets {
        require_namespace(proposal.namespace_id(), target.record.namespace_id())?;
        require_namespace(
            proposal.namespace_id(),
            target.superseded_event.namespace_id(),
        )?;
        if target.record.params().lineage_id != new_record.params().lineage_id
            || target.record.params().current_state != MemoryState::Superseded
            || target.record.params().last_state_event_id
                != target.superseded_event.params().event_id
            || target.superseded_event.params().memory_id != target.record.params().memory_id
            || target.superseded_event.params().event_type != MemoryEventType::Superseded
            || target.superseded_event.params().cause_ref.evidence_type()
                != EvidenceType::MemoryRecord
            || target.superseded_event.params().cause_ref.evidence_id()
                != &new_record.params().memory_id
            || !target
                .superseded_event
                .params()
                .related_memory_ids
                .contains(&new_record.params().memory_id)
        {
            return invariant(CrossObjectInvariantReason::SupersessionMismatch);
        }
        if let Some(start_at) = new_record.params().valid_time.start_at()
            && target.superseded_event.params().effective_at.as_ref() != Some(start_at)
        {
            return invariant(CrossObjectInvariantReason::TimeAlignmentMismatch);
        }
        maximum_version = maximum_version.max(target.record.params().version.get());
    }
    if maximum_version.checked_add(1) != Some(new_record.params().version.get()) {
        return invariant(CrossObjectInvariantReason::SupersessionMismatch);
    }
    Ok(())
}

/// Validates ContextPack namespace, governance, recall state, references, and citations.
pub fn validate_context_pack_resolution(
    pack: &ContextPack,
    sources: &[&SourceArtifact],
    fragments: &[&SourceFragment],
    records: &[&MemoryRecord],
) -> Result<(), CoreError> {
    require_recallable(pack.governance())?;
    let source_index = index_sources(sources)?;
    let fragment_index = index_fragments(fragments)?;
    let record_index = index_records(records)?;
    let citation_index = pack
        .params()
        .citation_map
        .iter()
        .map(|citation| (citation.citation_id().clone(), citation))
        .collect::<BTreeMap<_, _>>();

    let mut used_citations = BTreeSet::new();
    for item in &pack.params().items {
        for citation_id in &item.params().citation_ids {
            used_citations.insert(citation_id.clone());
            let citation = citation_index
                .get(citation_id)
                .copied()
                .ok_or_else(|| cross_error(CrossObjectInvariantReason::MissingReference))?;
            if !context_item_accepts_citation(
                item.params().item_type,
                &item.params().object_refs,
                citation,
                &record_index,
            ) {
                return invariant(CrossObjectInvariantReason::CitationMismatch);
            }
        }
        for object_ref in &item.params().object_refs {
            validate_context_object_ref(
                pack,
                item.params().item_type,
                object_ref,
                &source_index,
                &fragment_index,
                &record_index,
            )?;
        }
    }

    let mapped_citations = citation_index.keys().cloned().collect::<BTreeSet<_>>();
    if used_citations != mapped_citations {
        return invariant(CrossObjectInvariantReason::CitationMismatch);
    }
    for citation in &pack.params().citation_map {
        let fragment = fragment_index
            .get(citation.fragment_id())
            .copied()
            .ok_or_else(|| cross_error(CrossObjectInvariantReason::MissingReference))?;
        let source = source_index
            .get(citation.source_id())
            .copied()
            .ok_or_else(|| cross_error(CrossObjectInvariantReason::MissingReference))?;
        require_namespace(pack.namespace_id(), source.namespace_id())?;
        require_namespace(pack.namespace_id(), fragment.namespace_id())?;
        validate_source_fragment_resolution(fragment, source)?;
        require_recallable(fragment.governance())?;
        validate_governance_derivation(
            pack.governance(),
            &[source.governance(), fragment.governance()],
        )?;
        if citation.source_id() != &fragment.params().source_id
            || citation.byte_start() != fragment.params().byte_start
            || citation.byte_end() != fragment.params().byte_end
            || citation.fragment_digest() != &fragment.params().content_digest
        {
            return invariant(CrossObjectInvariantReason::CitationMismatch);
        }
    }
    Ok(())
}

fn context_item_accepts_citation(
    item_type: ContextItemType,
    object_refs: &[ObjectRef],
    citation: &Citation,
    records: &BTreeMap<Identifier, &MemoryRecord>,
) -> bool {
    match item_type {
        ContextItemType::SourceFragment => object_refs.iter().any(|object_ref| {
            object_ref.object_type() == CanonicalObjectType::SourceFragment
                && object_ref.object_id() == citation.fragment_id()
        }),
        ContextItemType::MemoryRecord => object_refs.iter().any(|object_ref| {
            object_ref.object_type() == CanonicalObjectType::MemoryRecord
                && records.get(object_ref.object_id()).is_some_and(|record| {
                    record
                        .params()
                        .source_fragment_refs
                        .contains(citation.fragment_id())
                })
        }),
        ContextItemType::ConflictNotice | ContextItemType::Constraint => {
            object_refs.iter().any(|object_ref| {
                (object_ref.object_type() == CanonicalObjectType::SourceFragment
                    && object_ref.object_id() == citation.fragment_id())
                    || (object_ref.object_type() == CanonicalObjectType::MemoryRecord
                        && records.get(object_ref.object_id()).is_some_and(|record| {
                            record
                                .params()
                                .source_fragment_refs
                                .contains(citation.fragment_id())
                        }))
            })
        }
    }
}

/// Validates that every semantic delete target is already excluded from recall.
pub fn validate_delete_recall_block(
    request: &DeleteRequest,
    targets: &[&dyn GovernedCanonicalObject],
) -> Result<(), CoreError> {
    let mut actual_refs = BTreeSet::new();
    for target in targets {
        require_namespace(request.namespace_id(), target.namespace_id())?;
        let object_ref = ObjectRef::new(target.object_type(), target.object_id().clone());
        if !actual_refs.insert(object_ref) {
            return invariant(CrossObjectInvariantReason::DuplicateObject);
        }
        if target.governance().deletion_state() == DeletionState::Active {
            return invariant(CrossObjectInvariantReason::DeletionStateMismatch);
        }
    }
    let planned_refs = request
        .params()
        .target_refs
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if actual_refs != planned_refs {
        return invariant(CrossObjectInvariantReason::MissingReference);
    }
    Ok(())
}

/// Validates the exact one-to-one relationship between a request plan and evidence results.
pub fn validate_deletion_evidence(
    request: &DeleteRequest,
    evidence: &DeletionEvidence,
) -> Result<(), CoreError> {
    require_namespace(request.namespace_id(), evidence.namespace_id())?;
    if evidence.params().delete_request_id != request.params().delete_request_id
        || evidence.params().device_id != request.params().device_id
    {
        return invariant(CrossObjectInvariantReason::IdentityMismatch);
    }
    if evidence.params().started_at < request.params().requested_at {
        return invariant(CrossObjectInvariantReason::TimeAlignmentMismatch);
    }

    let mut results = BTreeMap::new();
    for result in &evidence.params().component_results {
        if results
            .insert(result.params().component_key.clone(), result)
            .is_some()
        {
            return invariant(CrossObjectInvariantReason::DuplicateObject);
        }
    }
    if results.len() != request.params().planned_components.len() {
        return invariant(CrossObjectInvariantReason::DeletionPlanMismatch);
    }
    for target in &request.params().planned_components {
        let result = results
            .get(target.component_key())
            .copied()
            .ok_or_else(|| cross_error(CrossObjectInvariantReason::DeletionPlanMismatch))?;
        if result.params().component_type != target.component_type()
            || &result.params().target_ref != target.target_ref()
            || result.params().required_action != target.required_action()
            || result.params().target_count != target.target_count()
        {
            return invariant(CrossObjectInvariantReason::DeletionPlanMismatch);
        }
    }
    if evidence.params().overall_status == DeletionOverallStatus::Completed
        && evidence
            .params()
            .component_results
            .iter()
            .any(|result| result.params().status != ComponentStatus::Succeeded)
    {
        return invariant(CrossObjectInvariantReason::DeletionPlanMismatch);
    }
    Ok(())
}

fn validate_context_object_ref(
    pack: &ContextPack,
    item_type: ContextItemType,
    object_ref: &ObjectRef,
    sources: &BTreeMap<Identifier, &SourceArtifact>,
    fragments: &BTreeMap<Identifier, &SourceFragment>,
    records: &BTreeMap<Identifier, &MemoryRecord>,
) -> Result<(), CoreError> {
    match object_ref.object_type() {
        CanonicalObjectType::SourceFragment
            if matches!(
                item_type,
                ContextItemType::SourceFragment
                    | ContextItemType::ConflictNotice
                    | ContextItemType::Constraint
            ) =>
        {
            let fragment = fragments
                .get(object_ref.object_id())
                .copied()
                .ok_or_else(|| cross_error(CrossObjectInvariantReason::MissingReference))?;
            let source = sources
                .get(&fragment.params().source_id)
                .copied()
                .ok_or_else(|| cross_error(CrossObjectInvariantReason::MissingReference))?;
            require_namespace(pack.namespace_id(), fragment.namespace_id())?;
            validate_source_fragment_resolution(fragment, source)?;
            require_recallable(fragment.governance())?;
            validate_governance_derivation(pack.governance(), &[fragment.governance()])
        }
        CanonicalObjectType::MemoryRecord
            if matches!(
                item_type,
                ContextItemType::MemoryRecord
                    | ContextItemType::ConflictNotice
                    | ContextItemType::Constraint
            ) =>
        {
            let record = records
                .get(object_ref.object_id())
                .copied()
                .ok_or_else(|| cross_error(CrossObjectInvariantReason::MissingReference))?;
            require_namespace(pack.namespace_id(), record.namespace_id())?;
            require_recallable(record.governance())?;
            if item_type == ContextItemType::MemoryRecord
                && record.params().current_state != MemoryState::Confirmed
            {
                return invariant(CrossObjectInvariantReason::RecallBlocked);
            }
            validate_governance_derivation(pack.governance(), &[record.governance()])
        }
        _ => invariant(CrossObjectInvariantReason::IdentityMismatch),
    }
}

fn index_sources<'a>(
    values: &[&'a SourceArtifact],
) -> Result<BTreeMap<Identifier, &'a SourceArtifact>, CoreError> {
    let mut index = BTreeMap::new();
    for value in values {
        if index
            .insert(value.params().source_id.clone(), *value)
            .is_some()
        {
            return Err(cross_error(CrossObjectInvariantReason::DuplicateObject));
        }
    }
    Ok(index)
}

fn index_fragments<'a>(
    values: &[&'a SourceFragment],
) -> Result<BTreeMap<Identifier, &'a SourceFragment>, CoreError> {
    let mut index = BTreeMap::new();
    for value in values {
        if index
            .insert(value.params().fragment_id.clone(), *value)
            .is_some()
        {
            return Err(cross_error(CrossObjectInvariantReason::DuplicateObject));
        }
    }
    Ok(index)
}

fn index_records<'a>(
    values: &[&'a MemoryRecord],
) -> Result<BTreeMap<Identifier, &'a MemoryRecord>, CoreError> {
    let mut index = BTreeMap::new();
    for value in values {
        if index
            .insert(value.params().memory_id.clone(), *value)
            .is_some()
        {
            return Err(cross_error(CrossObjectInvariantReason::DuplicateObject));
        }
    }
    Ok(index)
}

fn require_exact_ids<'a>(
    expected: &[Identifier],
    actual: impl IntoIterator<Item = &'a Identifier>,
) -> Result<(), CoreError> {
    let expected = expected.iter().cloned().collect::<BTreeSet<_>>();
    let mut actual_set = BTreeSet::new();
    for identifier in actual {
        if !actual_set.insert(identifier.clone()) {
            return invariant(CrossObjectInvariantReason::DuplicateObject);
        }
    }
    if expected != actual_set {
        return invariant(CrossObjectInvariantReason::MissingReference);
    }
    Ok(())
}

fn same_identifier_set(left: &[Identifier], right: &[Identifier]) -> bool {
    left.iter().collect::<BTreeSet<_>>() == right.iter().collect::<BTreeSet<_>>()
}

fn require_namespace(expected: &Identifier, actual: &Identifier) -> Result<(), CoreError> {
    if expected != actual {
        return invariant(CrossObjectInvariantReason::NamespaceMismatch);
    }
    Ok(())
}

fn require_recallable(governance: &Governance) -> Result<(), CoreError> {
    if governance.deletion_state() != DeletionState::Active {
        return invariant(CrossObjectInvariantReason::RecallBlocked);
    }
    Ok(())
}

fn sensitivity_rank(sensitivity: Sensitivity) -> u8 {
    match sensitivity {
        Sensitivity::Personal => 0,
        Sensitivity::Sensitive => 1,
        Sensitivity::Restricted => 2,
    }
}

fn retention_is_no_later(derived: &Governance, source: &Governance) -> bool {
    let derived = derived.retention();
    let source = source.retention();
    match (derived.mode(), source.mode()) {
        (RetentionMode::UntilDeleted, RetentionMode::UntilDeleted) => true,
        (RetentionMode::UntilTime, RetentionMode::UntilDeleted) => true,
        (RetentionMode::UntilTime, RetentionMode::UntilTime) => {
            derived.expires_at() <= source.expires_at()
        }
        (RetentionMode::Policy, RetentionMode::Policy) => derived.policy_id() == source.policy_id(),
        _ => false,
    }
}

fn invariant(reason: CrossObjectInvariantReason) -> Result<(), CoreError> {
    Err(cross_error(reason))
}

fn cross_error(reason: CrossObjectInvariantReason) -> CoreError {
    CoreError::cross_object_invariant(reason)
}
