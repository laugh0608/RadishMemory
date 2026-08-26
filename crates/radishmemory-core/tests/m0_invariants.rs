use serde_json as _;
use sha2 as _;
use time as _;
use unicode_normalization as _;

use radishmemory_core::*;

fn id(value: &str) -> Identifier {
    Identifier::new(value).expect("synthetic identifier must be valid")
}

fn text(value: &str) -> NonEmptyText {
    NonEmptyText::new(value).expect("synthetic text must be nonempty")
}

fn timestamp(value: &str) -> Timestamp {
    Timestamp::parse(value).expect("synthetic timestamp must be valid")
}

fn producer() -> ProducerRef {
    ProducerRef::new(ProducerType::TestFixture, id("producer-fixture"), text("1"))
}

fn actor() -> ActorRef {
    ActorRef::new(ActorType::TestFixture, id("actor-fixture"), Some(text("1")))
}

fn governance(state: DeletionState) -> Governance {
    Governance::new(
        Sensitivity::Personal,
        EgressPolicy::LocalOnly,
        RetentionRule::new(RetentionMode::UntilDeleted, None, None)
            .expect("retention must be valid"),
        state,
        id("policy-local-only"),
    )
    .expect("governance must be local")
}

fn source_with(namespace: &str, content_value: &str, state: DeletionState) -> SourceArtifact {
    let content = text(content_value);
    SourceArtifact::new(SourceArtifactParams {
        source_id: id("source-1"),
        lineage_id: id("source-lineage-1"),
        version: Version::new(1).expect("version must be positive"),
        namespace_id: id(namespace),
        source_kind: SourceKind::Text,
        media_type: MediaType::TextPlain,
        content_length: u64::try_from(content.utf8_len()).expect("length must fit u64"),
        content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
        content,
        title: None,
        origin_kind: SourceOriginKind::SyntheticFixture,
        origin_ref: None,
        observed_at: timestamp("2026-08-23T08:00:00Z"),
        captured_at: timestamp("2026-08-23T08:00:01Z"),
        supersedes_source_ids: vec![],
        governance: governance(state),
        producer: producer(),
        created_at: timestamp("2026-08-23T08:00:01Z"),
    })
    .expect("source must be valid")
}

fn source() -> SourceArtifact {
    source_with("namespace-1", "abc", DeletionState::Active)
}

fn fragment_with(namespace: &str, content_value: &str, state: DeletionState) -> SourceFragment {
    let content = text(content_value);
    SourceFragment::new(SourceFragmentParams {
        fragment_id: id("fragment-1"),
        namespace_id: id(namespace),
        source_id: id("source-1"),
        ordinal: 0,
        byte_start: 0,
        byte_end: u64::try_from(content.utf8_len()).expect("length must fit u64"),
        heading_path: None,
        content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
        content,
        segmenter: producer(),
        governance: governance(state),
        created_at: timestamp("2026-08-23T08:00:02Z"),
    })
    .expect("fragment must be valid")
}

fn fragment() -> SourceFragment {
    fragment_with("namespace-1", "abc", DeletionState::Active)
}

fn valid_time_at(value: &str) -> ValidTime {
    ValidTime::new(
        ValidTimeMode::Instant,
        Some(timestamp(value)),
        None,
        TimePrecision::Exact,
    )
    .expect("instant valid time must be valid")
}

fn proposal_with(
    operation: ProposalOperation,
    target_memory_ids: Vec<Identifier>,
    valid_time: ValidTime,
) -> MemoryProposal {
    MemoryProposal::new(MemoryProposalParams {
        proposal_id: id("proposal-1"),
        namespace_id: id("namespace-1"),
        operation,
        memory_type: MemoryType::Observation,
        subject_ref: id("subject-1"),
        proposed_content: MemoryValue::from_text(text("Synthetic memory")),
        source_fragment_refs: vec![id("fragment-1")],
        target_memory_ids,
        observed_at: timestamp("2026-08-23T08:00:00Z"),
        valid_time,
        confidence: UnitInterval::new(0.8).expect("confidence must be valid"),
        importance: UnitInterval::new(0.6).expect("importance must be valid"),
        governance: governance(DeletionState::Active),
        producer: producer(),
        reason_code: text("fixture-observation"),
        proposed_at: timestamp("2026-08-23T08:00:03Z"),
    })
    .expect("proposal must be valid")
}

fn proposal() -> MemoryProposal {
    proposal_with(
        ProposalOperation::Create,
        vec![],
        valid_time_at("2026-08-23T08:00:00Z"),
    )
}

fn decision_with(decision: Decision, result_memory_id: Option<Identifier>) -> MemoryDecision {
    MemoryDecision::new(MemoryDecisionParams {
        decision_id: id("decision-1"),
        namespace_id: id("namespace-1"),
        proposal_id: id("proposal-1"),
        previous_decision_id: None,
        decision,
        decided_by: actor(),
        authorization_basis: text("fixture-authorization"),
        reason_code: text("fixture-decision"),
        reason_text: None,
        result_memory_id,
        decided_at: timestamp("2026-08-23T08:00:04Z"),
    })
    .expect("decision must be valid")
}

fn accepted_decision() -> MemoryDecision {
    decision_with(Decision::Accept, Some(id("memory-1")))
}

struct RecordSpec<'a> {
    memory_id: &'a str,
    lineage_id: &'a str,
    version: u64,
    current_state: MemoryState,
    last_event_id: &'a str,
    supersedes: Vec<Identifier>,
    valid_time: ValidTime,
}

fn record_with(spec: RecordSpec<'_>) -> MemoryRecord {
    let content = MemoryValue::from_text(text("Synthetic memory"));
    MemoryRecord::new(MemoryRecordParams {
        memory_id: id(spec.memory_id),
        lineage_id: id(spec.lineage_id),
        version: Version::new(spec.version).expect("version must be positive"),
        namespace_id: id("namespace-1"),
        memory_type: MemoryType::Observation,
        subject_ref: id("subject-1"),
        content_digest: content.content_digest().clone(),
        content,
        source_fragment_refs: vec![id("fragment-1")],
        origin_proposal_id: id("proposal-1"),
        accepted_by_decision_id: id("decision-1"),
        observed_at: timestamp("2026-08-23T08:00:00Z"),
        valid_time: spec.valid_time,
        confidence: UnitInterval::new(0.8).expect("confidence must be valid"),
        importance: UnitInterval::new(0.6).expect("importance must be valid"),
        governance: governance(DeletionState::Active),
        current_state: spec.current_state,
        last_state_event_id: id(spec.last_event_id),
        supersedes_memory_ids: spec.supersedes,
        contradicts_memory_ids: vec![],
        created_at: timestamp("2026-08-23T08:00:05Z"),
    })
    .expect("record must be valid")
}

fn confirmed_record() -> MemoryRecord {
    record_with(RecordSpec {
        memory_id: "memory-1",
        lineage_id: "memory-lineage-1",
        version: 1,
        current_state: MemoryState::Confirmed,
        last_event_id: "event-initial",
        supersedes: vec![],
        valid_time: valid_time_at("2026-08-23T08:00:00Z"),
    })
}

fn initial_event_for(memory_id: &str, event_id: &str) -> MemoryStateEvent {
    MemoryStateEvent::new(MemoryStateEventParams {
        event_id: id(event_id),
        namespace_id: id("namespace-1"),
        memory_id: id(memory_id),
        previous_event_id: None,
        event_type: MemoryEventType::Confirmed,
        from_state: None,
        cause_ref: EvidenceRef::new(EvidenceType::MemoryDecision, id("decision-1")),
        related_memory_ids: vec![],
        actor: actor(),
        reason_code: text("fixture-confirmed"),
        effective_at: None,
        occurred_at: timestamp("2026-08-23T08:00:05Z"),
    })
    .expect("initial event must be valid")
}

struct TerminalEventSpec<'a> {
    event_id: &'a str,
    memory_id: &'a str,
    previous_event_id: &'a str,
    event_type: MemoryEventType,
    related_memory_ids: Vec<Identifier>,
    cause_ref: EvidenceRef,
    effective_at: &'a str,
}

fn terminal_event(spec: TerminalEventSpec<'_>) -> MemoryStateEvent {
    MemoryStateEvent::new(MemoryStateEventParams {
        event_id: id(spec.event_id),
        namespace_id: id("namespace-1"),
        memory_id: id(spec.memory_id),
        previous_event_id: Some(id(spec.previous_event_id)),
        event_type: spec.event_type,
        from_state: Some(MemoryState::Confirmed),
        cause_ref: spec.cause_ref,
        related_memory_ids: spec.related_memory_ids,
        actor: actor(),
        reason_code: text("fixture-terminal"),
        effective_at: Some(timestamp(spec.effective_at)),
        occurred_at: timestamp("2026-08-23T09:00:01Z"),
    })
    .expect("terminal event must be valid")
}

fn context_item_for(object_type: CanonicalObjectType, object_id: &str) -> ContextItem {
    let rendered = text("abc");
    ContextItem::new(ContextItemParams {
        item_id: id("item-1"),
        ordinal: 0,
        item_type: match object_type {
            CanonicalObjectType::SourceFragment => ContextItemType::SourceFragment,
            CanonicalObjectType::MemoryRecord => ContextItemType::MemoryRecord,
            _ => panic!("unsupported synthetic context item type"),
        },
        object_refs: vec![ObjectRef::new(object_type, id(object_id))],
        content_digest: compute_nfc_text_digest(rendered.as_str()),
        rendered_content: rendered,
        evidence_refs: vec![EvidenceRef::new(
            EvidenceType::SourceFragment,
            id("fragment-1"),
        )],
        citation_ids: vec![id("citation-1")],
        selection_reason_codes: vec![text("fixture-selected")],
        temporal_role: TemporalRole::Current,
        truncation: TruncationFacts::new(false, 3, 3, None)
            .expect("truncation facts must be valid"),
    })
    .expect("context item must be valid")
}

fn context_pack_with(item: ContextItem, citation_end: u64) -> ContextPack {
    let task = text("Find synthetic source");
    ContextPack::new(ContextPackParams {
        context_pack_id: id("context-pack-1"),
        namespace_id: id("namespace-1"),
        request_id: id("request-1"),
        task_digest: compute_nfc_text_digest(task.as_str()),
        task,
        as_of: timestamp("2026-08-23T09:00:00Z"),
        compiled_at: timestamp("2026-08-23T09:00:00Z"),
        governance: governance(DeletionState::Active),
        budget: Budget::new(100, 3).expect("budget must be valid"),
        items: vec![item],
        citation_map: vec![
            Citation::new(
                id("citation-1"),
                id("source-1"),
                id("fragment-1"),
                0,
                citation_end,
                compute_exact_bytes_digest(b"abc"),
            )
            .expect("citation must be valid"),
        ],
        filter_summary: vec![
            FilterCount::new(text("fixture-selected"), 1, 0, 0)
                .expect("filter count must be valid"),
        ],
        content_digest: compute_digest("context-pack-v1", "{}")
            .expect("context digest must compute"),
    })
    .expect("context pack must be valid")
}

fn deletion_target() -> DeletionTarget {
    DeletionTarget::new(
        id("source-body"),
        DeletionComponentType::SourceBody,
        DeletionTargetRef::Object(ObjectRef::new(
            CanonicalObjectType::SourceArtifact,
            id("source-1"),
        )),
        1,
        RequiredAction::Delete,
    )
    .expect("deletion target must be valid")
}

fn delete_request() -> DeleteRequest {
    DeleteRequest::new(DeleteRequestParams {
        delete_request_id: id("delete-request-1"),
        namespace_id: id("namespace-1"),
        requested_by: actor(),
        authorization_basis: text("fixture-delete-authorization"),
        requested_guarantee: RequestedGuarantee::LocalPurge,
        device_id: id("device-1"),
        target_refs: vec![ObjectRef::new(
            CanonicalObjectType::SourceArtifact,
            id("source-1"),
        )],
        planned_components: vec![deletion_target()],
        reason_code: text("fixture-delete"),
        requested_at: timestamp("2026-08-23T10:00:00Z"),
    })
    .expect("delete request must be valid")
}

fn component_result(component_type: DeletionComponentType) -> ComponentResult {
    ComponentResult::new(ComponentResultParams {
        component_key: id("source-body"),
        component_type,
        target_ref: DeletionTargetRef::Object(ObjectRef::new(
            CanonicalObjectType::SourceArtifact,
            id("source-1"),
        )),
        required_action: RequiredAction::Delete,
        target_count: 1,
        processed_count: 1,
        status: ComponentStatus::Succeeded,
        outcome: ComponentOutcome::Deleted,
        verification_method: text("fixture-read-after-delete"),
        checked_at: timestamp("2026-08-23T10:00:02Z"),
        error_code: None,
        retryable: None,
        retention_basis: None,
    })
    .expect("component result must be valid")
}

fn deletion_evidence(component_type: DeletionComponentType) -> DeletionEvidence {
    DeletionEvidence::new(DeletionEvidenceParams {
        deletion_evidence_id: id("deletion-evidence-1"),
        delete_request_id: id("delete-request-1"),
        previous_evidence_id: None,
        namespace_id: id("namespace-1"),
        device_id: id("device-1"),
        overall_status: DeletionOverallStatus::Completed,
        component_results: vec![component_result(component_type)],
        started_at: timestamp("2026-08-23T10:00:01Z"),
        finished_at: Some(timestamp("2026-08-23T10:00:02Z")),
        verified_by: producer(),
        evidence_digest: compute_digest("deletion-evidence-v1", "{}")
            .expect("evidence digest must compute"),
    })
    .expect("deletion evidence must be valid")
}

#[test]
fn source_and_proposal_closure_resolve_exact_active_bytes() {
    let source = source();
    let fragment = fragment();
    let proposal = proposal();
    let resolved = [ResolvedSource {
        fragment: &fragment,
        source: &source,
    }];

    validate_source_fragment_resolution(&fragment, &source)
        .expect("fragment must resolve to exact source bytes");
    validate_memory_proposal_sources(&proposal, &resolved)
        .expect("proposal must resolve exact source closure");

    let missing_error = validate_memory_proposal_sources(&proposal, &[])
        .expect_err("proposal source closure must be exact and nonempty");
    assert_eq!(
        missing_error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::MissingReference)
    );

    let mismatched_fragment = fragment_with("namespace-1", "xbc", DeletionState::Active);
    let slice_error = validate_source_fragment_resolution(&mismatched_fragment, &source)
        .expect_err("same-length content drift must fail");
    assert_eq!(
        slice_error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::SourceSliceMismatch)
    );

    let other_namespace = source_with("namespace-2", "abc", DeletionState::Active);
    let namespace_error = validate_source_fragment_resolution(&fragment, &other_namespace)
        .expect_err("cross-namespace reference must fail");
    assert_eq!(
        namespace_error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::NamespaceMismatch)
    );

    let blocked_source = source_with("namespace-1", "abc", DeletionState::Pending);
    let blocked_error = validate_source_fragment_resolution(&fragment, &blocked_source)
        .expect_err("pending source cannot resolve for recall");
    assert_eq!(
        blocked_error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::RecallBlocked)
    );

    let mut restricted_params = source.params().clone();
    restricted_params.governance = Governance::new(
        Sensitivity::Restricted,
        EgressPolicy::LocalOnly,
        RetentionRule::new(RetentionMode::UntilDeleted, None, None)
            .expect("retention must be valid"),
        DeletionState::Active,
        id("policy-restricted"),
    )
    .expect("restricted governance must be valid");
    let restricted_source =
        SourceArtifact::new(restricted_params).expect("restricted source must be valid");
    let governance_error = validate_source_fragment_resolution(&fragment, &restricted_source)
        .expect_err("derived fragment cannot weaken source sensitivity");
    assert_eq!(
        governance_error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::GovernanceMismatch)
    );
}

#[test]
fn accepted_materialization_forms_one_traceable_closure() {
    let proposal = proposal();
    let decision = accepted_decision();
    let record = confirmed_record();
    let event = initial_event_for("memory-1", "event-initial");

    validate_memory_materialization(&proposal, &decision, &record, &event)
        .expect("accept closure must be valid");

    let rejected = decision_with(Decision::Reject, None);
    let error = validate_memory_materialization(&proposal, &rejected, &record, &event)
        .expect_err("rejected proposal cannot materialize");
    assert_eq!(
        error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::MaterializationMismatch)
    );

    let wrong_event = initial_event_for("memory-other", "event-initial");
    let error = validate_memory_materialization(&proposal, &decision, &record, &wrong_event)
        .expect_err("initial event must point to materialized memory");
    assert_eq!(
        error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::MaterializationMismatch)
    );
}

#[test]
fn event_chain_is_order_independent_but_rejects_branches_and_stale_projection() {
    let record = record_with(RecordSpec {
        memory_id: "memory-1",
        lineage_id: "memory-lineage-1",
        version: 1,
        current_state: MemoryState::Retracted,
        last_event_id: "event-terminal",
        supersedes: vec![],
        valid_time: valid_time_at("2026-08-23T08:00:00Z"),
    });
    let initial = initial_event_for("memory-1", "event-initial");
    let terminal = terminal_event(TerminalEventSpec {
        event_id: "event-terminal",
        memory_id: "memory-1",
        previous_event_id: "event-initial",
        event_type: MemoryEventType::Retracted,
        related_memory_ids: vec![],
        cause_ref: EvidenceRef::new(EvidenceType::PolicyBasis, id("policy-retract")),
        effective_at: "2026-08-23T09:00:00Z",
    });

    validate_memory_event_chain(&record, &[&terminal, &initial])
        .expect("unordered event set must resolve by previous IDs");

    let branch = terminal_event(TerminalEventSpec {
        event_id: "event-branch",
        memory_id: "memory-1",
        previous_event_id: "event-initial",
        event_type: MemoryEventType::Expired,
        related_memory_ids: vec![],
        cause_ref: EvidenceRef::new(EvidenceType::PolicyBasis, id("policy-expiry")),
        effective_at: "2026-08-23T09:00:00Z",
    });
    let branch_error = validate_memory_event_chain(&record, &[&initial, &terminal, &branch])
        .expect_err("event branch must fail");
    assert_eq!(
        branch_error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::EventChainConflict)
    );

    let stale = confirmed_record();
    let stale_error = validate_memory_event_chain(&stale, &[&initial, &terminal])
        .expect_err("record projection must equal final event");
    assert_eq!(
        stale_error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::StateProjectionMismatch)
    );
}

#[test]
fn supersession_closes_old_version_at_new_valid_start() {
    let valid_start = "2026-08-23T09:00:00Z";
    let proposal = proposal_with(
        ProposalOperation::Supersede,
        vec![id("memory-old")],
        valid_time_at(valid_start),
    );
    let new_record = record_with(RecordSpec {
        memory_id: "memory-1",
        lineage_id: "memory-lineage-1",
        version: 2,
        current_state: MemoryState::Confirmed,
        last_event_id: "event-new-initial",
        supersedes: vec![id("memory-old")],
        valid_time: valid_time_at(valid_start),
    });
    let old_record = record_with(RecordSpec {
        memory_id: "memory-old",
        lineage_id: "memory-lineage-1",
        version: 1,
        current_state: MemoryState::Superseded,
        last_event_id: "event-old-superseded",
        supersedes: vec![],
        valid_time: valid_time_at("2026-08-23T08:00:00Z"),
    });
    let event = terminal_event(TerminalEventSpec {
        event_id: "event-old-superseded",
        memory_id: "memory-old",
        previous_event_id: "event-old-initial",
        event_type: MemoryEventType::Superseded,
        related_memory_ids: vec![id("memory-1")],
        cause_ref: EvidenceRef::new(EvidenceType::MemoryRecord, id("memory-1")),
        effective_at: valid_start,
    });
    validate_memory_supersession(
        &proposal,
        &new_record,
        &[SupersededTarget {
            record: &old_record,
            superseded_event: &event,
        }],
    )
    .expect("supersession closure must be valid");

    let wrong_time = terminal_event(TerminalEventSpec {
        event_id: "event-old-superseded",
        memory_id: "memory-old",
        previous_event_id: "event-old-initial",
        event_type: MemoryEventType::Superseded,
        related_memory_ids: vec![id("memory-1")],
        cause_ref: EvidenceRef::new(EvidenceType::MemoryRecord, id("memory-1")),
        effective_at: "2026-08-23T09:30:00Z",
    });
    let error = validate_memory_supersession(
        &proposal,
        &new_record,
        &[SupersededTarget {
            record: &old_record,
            superseded_event: &wrong_time,
        }],
    )
    .expect_err("supersede event must align with new valid start");
    assert_eq!(
        error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::TimeAlignmentMismatch)
    );
}

#[test]
fn context_pack_resolves_only_active_confirmed_content_and_exact_citations() {
    let source = source();
    let fragment = fragment();
    let record = confirmed_record();
    let pack = context_pack_with(
        context_item_for(CanonicalObjectType::SourceFragment, "fragment-1"),
        3,
    );
    validate_context_pack_resolution(&pack, &[&source], &[&fragment], &[&record])
        .expect("context references and citation must resolve");

    let stale_record = record_with(RecordSpec {
        memory_id: "memory-1",
        lineage_id: "memory-lineage-1",
        version: 1,
        current_state: MemoryState::Retracted,
        last_event_id: "event-terminal",
        supersedes: vec![],
        valid_time: valid_time_at("2026-08-23T08:00:00Z"),
    });
    let record_pack = context_pack_with(
        context_item_for(CanonicalObjectType::MemoryRecord, "memory-1"),
        3,
    );
    let stale_error =
        validate_context_pack_resolution(&record_pack, &[&source], &[&fragment], &[&stale_record])
            .expect_err("ordinary record item must be confirmed");
    assert_eq!(
        stale_error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::RecallBlocked)
    );

    let bad_citation = context_pack_with(
        context_item_for(CanonicalObjectType::SourceFragment, "fragment-1"),
        2,
    );
    let citation_error =
        validate_context_pack_resolution(&bad_citation, &[&source], &[&fragment], &[&record])
            .expect_err("citation byte range must match fragment");
    assert_eq!(
        citation_error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::CitationMismatch)
    );
}

#[test]
fn deletion_requires_recall_block_and_exact_component_evidence() {
    let request = delete_request();
    let pending_source = source_with("namespace-1", "abc", DeletionState::Pending);
    validate_delete_recall_block(&request, &[&pending_source])
        .expect("pending target must be excluded from recall");
    validate_deletion_evidence(
        &request,
        &deletion_evidence(DeletionComponentType::SourceBody),
    )
    .expect("component result must match frozen plan");

    let active_source = source();
    let state_error = validate_delete_recall_block(&request, &[&active_source])
        .expect_err("active target cannot satisfy delete recall block");
    assert_eq!(
        state_error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::DeletionStateMismatch)
    );

    let mismatched = deletion_evidence(DeletionComponentType::SourceMetadata);
    let plan_error = validate_deletion_evidence(&request, &mismatched)
        .expect_err("evidence component type must match plan");
    assert_eq!(
        plan_error.cross_object_invariant_reason(),
        Some(CrossObjectInvariantReason::DeletionPlanMismatch)
    );
}

#[test]
fn invariant_errors_do_not_copy_source_or_context_content() {
    let private_source = source_with(
        "namespace-1",
        "private fixture source body",
        DeletionState::Active,
    );
    let private_fragment = fragment_with(
        "namespace-1",
        "different private fixture",
        DeletionState::Active,
    );
    let error = validate_source_fragment_resolution(&private_fragment, &private_source)
        .expect_err("mismatched private source must fail");

    assert_eq!(error.code(), CoreErrorCode::CrossObjectInvariant);
    assert!(!error.to_string().contains("private fixture"));
    assert!(!format!("{error:?}").contains("private fixture"));
}
