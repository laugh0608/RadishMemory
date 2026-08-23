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

fn governance() -> Governance {
    Governance::new(
        Sensitivity::Personal,
        EgressPolicy::LocalOnly,
        RetentionRule::new(RetentionMode::UntilDeleted, None, None)
            .expect("retention must be valid"),
        DeletionState::Active,
        id("policy-local-only"),
    )
    .expect("M0 governance must be local")
}

fn unknown_valid_time() -> ValidTime {
    ValidTime::new(ValidTimeMode::Unknown, None, None, TimePrecision::Unknown)
        .expect("unknown time must be valid")
}

fn source_artifact_params() -> SourceArtifactParams {
    let content = text("abc");
    SourceArtifactParams {
        source_id: id("source-1"),
        lineage_id: id("source-lineage-1"),
        version: Version::new(1).expect("version must be positive"),
        namespace_id: id("namespace-1"),
        source_kind: SourceKind::Text,
        media_type: MediaType::TextPlain,
        content_length: 3,
        content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
        content,
        title: Some(text("Synthetic source")),
        origin_kind: SourceOriginKind::SyntheticFixture,
        origin_ref: None,
        observed_at: timestamp("2026-08-23T08:00:00Z"),
        captured_at: timestamp("2026-08-23T08:00:01Z"),
        supersedes_source_ids: vec![],
        governance: governance(),
        producer: producer(),
        created_at: timestamp("2026-08-23T08:00:01Z"),
    }
}

fn source_fragment_params() -> SourceFragmentParams {
    let content = text("abc");
    SourceFragmentParams {
        fragment_id: id("fragment-1"),
        namespace_id: id("namespace-1"),
        source_id: id("source-1"),
        ordinal: 0,
        byte_start: 0,
        byte_end: 3,
        heading_path: None,
        content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
        content,
        segmenter: producer(),
        governance: governance(),
        created_at: timestamp("2026-08-23T08:00:02Z"),
    }
}

fn proposal_params() -> MemoryProposalParams {
    MemoryProposalParams {
        proposal_id: id("proposal-1"),
        namespace_id: id("namespace-1"),
        operation: ProposalOperation::Create,
        memory_type: MemoryType::Observation,
        subject_ref: id("subject-1"),
        proposed_content: MemoryValue::from_text(text("Synthetic memory")),
        source_fragment_refs: vec![id("fragment-1")],
        target_memory_ids: vec![],
        observed_at: timestamp("2026-08-23T08:00:00Z"),
        valid_time: unknown_valid_time(),
        confidence: UnitInterval::new(0.8).expect("confidence must be valid"),
        importance: UnitInterval::new(0.6).expect("importance must be valid"),
        governance: governance(),
        producer: producer(),
        reason_code: text("fixture-observation"),
        proposed_at: timestamp("2026-08-23T08:00:03Z"),
    }
}

fn decision_params() -> MemoryDecisionParams {
    MemoryDecisionParams {
        decision_id: id("decision-1"),
        namespace_id: id("namespace-1"),
        proposal_id: id("proposal-1"),
        previous_decision_id: None,
        decision: Decision::Accept,
        decided_by: actor(),
        authorization_basis: text("fixture-authorization"),
        reason_code: text("fixture-accept"),
        reason_text: None,
        result_memory_id: Some(id("memory-1")),
        decided_at: timestamp("2026-08-23T08:00:04Z"),
    }
}

fn memory_record_params() -> MemoryRecordParams {
    let content = MemoryValue::from_text(text("Synthetic memory"));
    MemoryRecordParams {
        memory_id: id("memory-1"),
        lineage_id: id("memory-lineage-1"),
        version: Version::new(1).expect("version must be positive"),
        namespace_id: id("namespace-1"),
        memory_type: MemoryType::Observation,
        subject_ref: id("subject-1"),
        content_digest: content.content_digest().clone(),
        content,
        source_fragment_refs: vec![id("fragment-1")],
        origin_proposal_id: id("proposal-1"),
        accepted_by_decision_id: id("decision-1"),
        observed_at: timestamp("2026-08-23T08:00:00Z"),
        valid_time: unknown_valid_time(),
        confidence: UnitInterval::new(0.8).expect("confidence must be valid"),
        importance: UnitInterval::new(0.6).expect("importance must be valid"),
        governance: governance(),
        current_state: MemoryState::Confirmed,
        last_state_event_id: id("event-1"),
        supersedes_memory_ids: vec![],
        contradicts_memory_ids: vec![],
        created_at: timestamp("2026-08-23T08:00:05Z"),
    }
}

fn state_event_params() -> MemoryStateEventParams {
    MemoryStateEventParams {
        event_id: id("event-1"),
        namespace_id: id("namespace-1"),
        memory_id: id("memory-1"),
        previous_event_id: None,
        event_type: MemoryEventType::Confirmed,
        from_state: None,
        cause_ref: EvidenceRef::new(EvidenceType::MemoryDecision, id("decision-1")),
        related_memory_ids: vec![],
        actor: actor(),
        reason_code: text("fixture-confirmed"),
        effective_at: None,
        occurred_at: timestamp("2026-08-23T08:00:05Z"),
    }
}

fn context_item() -> ContextItem {
    let rendered_content = text("abc");
    ContextItem::new(ContextItemParams {
        item_id: id("item-1"),
        ordinal: 0,
        item_type: ContextItemType::SourceFragment,
        object_refs: vec![ObjectRef::new(
            CanonicalObjectType::SourceFragment,
            id("fragment-1"),
        )],
        content_digest: compute_nfc_text_digest(rendered_content.as_str()),
        rendered_content,
        evidence_refs: vec![EvidenceRef::new(
            EvidenceType::SourceFragment,
            id("fragment-1"),
        )],
        citation_ids: vec![id("citation-1")],
        selection_reason_codes: vec![text("fixture-selected")],
        temporal_role: TemporalRole::Current,
        truncation: TruncationFacts::new(false, 3, 3, None)
            .expect("untruncated facts must be valid"),
    })
    .expect("context item must be valid")
}

fn context_pack_params() -> ContextPackParams {
    let task = text("Find synthetic source");
    ContextPackParams {
        context_pack_id: id("context-pack-1"),
        namespace_id: id("namespace-1"),
        request_id: id("request-1"),
        task_digest: compute_nfc_text_digest(task.as_str()),
        task,
        as_of: timestamp("2026-08-23T08:00:06Z"),
        compiled_at: timestamp("2026-08-23T08:00:06Z"),
        governance: governance(),
        budget: Budget::new(100, 3).expect("budget must be valid"),
        items: vec![context_item()],
        citation_map: vec![
            Citation::new(
                id("citation-1"),
                id("source-1"),
                id("fragment-1"),
                0,
                3,
                compute_exact_bytes_digest(b"abc"),
            )
            .expect("citation must be valid"),
        ],
        filter_summary: vec![
            FilterCount::new(text("fixture-selected"), 1, 0, 0)
                .expect("filter count must be nonzero"),
        ],
        content_digest: compute_digest("context-pack-v1", "{}")
            .expect("context pack digest must compute"),
    }
}

fn deletion_target_ref() -> DeletionTargetRef {
    DeletionTargetRef::Object(ObjectRef::new(
        CanonicalObjectType::SourceArtifact,
        id("source-1"),
    ))
}

fn deletion_target() -> DeletionTarget {
    DeletionTarget::new(
        id("source-body"),
        DeletionComponentType::SourceBody,
        deletion_target_ref(),
        1,
        RequiredAction::Delete,
    )
    .expect("deletion target must be valid")
}

fn delete_request_params() -> DeleteRequestParams {
    DeleteRequestParams {
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
        requested_at: timestamp("2026-08-23T08:00:07Z"),
    }
}

fn component_result() -> ComponentResult {
    ComponentResult::new(ComponentResultParams {
        component_key: id("source-body"),
        component_type: DeletionComponentType::SourceBody,
        target_ref: deletion_target_ref(),
        required_action: RequiredAction::Delete,
        target_count: 1,
        processed_count: 1,
        status: ComponentStatus::Succeeded,
        outcome: ComponentOutcome::Deleted,
        verification_method: text("fixture-read-after-delete"),
        checked_at: timestamp("2026-08-23T08:00:08Z"),
        error_code: None,
        retryable: None,
        retention_basis: None,
    })
    .expect("component result must be valid")
}

fn deletion_evidence_params() -> DeletionEvidenceParams {
    DeletionEvidenceParams {
        deletion_evidence_id: id("deletion-evidence-1"),
        delete_request_id: id("delete-request-1"),
        previous_evidence_id: None,
        namespace_id: id("namespace-1"),
        device_id: id("device-1"),
        overall_status: DeletionOverallStatus::Completed,
        component_results: vec![component_result()],
        started_at: timestamp("2026-08-23T08:00:07Z"),
        finished_at: Some(timestamp("2026-08-23T08:00:08Z")),
        verified_by: producer(),
        evidence_digest: compute_digest("deletion-evidence-v1", "{}")
            .expect("deletion evidence digest must compute"),
    }
}

#[test]
fn all_nine_canonical_objects_have_frozen_identity_fields() {
    let source = SourceArtifact::new(source_artifact_params()).expect("source must be valid");
    let fragment = SourceFragment::new(source_fragment_params()).expect("fragment must be valid");
    let proposal = MemoryProposal::new(proposal_params()).expect("proposal must be valid");
    let decision = MemoryDecision::new(decision_params()).expect("decision must be valid");
    let record = MemoryRecord::new(memory_record_params()).expect("record must be valid");
    let event = MemoryStateEvent::new(state_event_params()).expect("event must be valid");
    let context = ContextPack::new(context_pack_params()).expect("context must be valid");
    let request = DeleteRequest::new(delete_request_params()).expect("request must be valid");
    let evidence = DeletionEvidence::new(deletion_evidence_params()).expect("evidence valid");

    let objects: [&dyn CanonicalObject; 9] = [
        &source, &fragment, &proposal, &decision, &record, &event, &context, &request, &evidence,
    ];
    let expected = [
        CanonicalObjectType::SourceArtifact,
        CanonicalObjectType::SourceFragment,
        CanonicalObjectType::MemoryProposal,
        CanonicalObjectType::MemoryDecision,
        CanonicalObjectType::MemoryRecord,
        CanonicalObjectType::MemoryStateEvent,
        CanonicalObjectType::ContextPack,
        CanonicalObjectType::DeleteRequest,
        CanonicalObjectType::DeletionEvidence,
    ];

    for (object, object_type) in objects.into_iter().zip(expected) {
        assert_eq!(object.schema_version(), M0_SCHEMA_VERSION);
        assert_eq!(object.object_type(), object_type);
        assert!(!object.object_id().as_str().is_empty());
        assert_eq!(object.namespace_id().as_str(), "namespace-1");
    }
    assert_eq!(source.params().media_type.as_str(), "text/plain");
    assert_eq!(record.initial_state(), MemoryState::Confirmed);
    assert_eq!(event.to_state(), MemoryState::Confirmed);
    assert_eq!(context.delivery_scope(), DeliveryScope::Local);
    assert_eq!(request.scope(), DeletionScope::LocalDevice);
    assert_eq!(evidence.scope(), DeletionScope::LocalDevice);
}

#[test]
fn shared_values_and_governance_fail_closed() {
    let empty_id = Identifier::new("").expect_err("empty ID must fail");
    assert_eq!(
        empty_id.invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::EmptyIdentifier)
    );
    assert_eq!(
        NonEmptyText::new("")
            .expect_err("empty text must fail")
            .invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::EmptyText)
    );
    for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert_eq!(
            UnitInterval::new(value)
                .expect_err("invalid unit interval must fail")
                .invalid_canonical_object_reason(),
            Some(InvalidCanonicalObjectReason::InvalidUnitInterval)
        );
    }

    let invalid_retention = RetentionRule::new(
        RetentionMode::UntilDeleted,
        Some(timestamp("2026-08-24T00:00:00Z")),
        None,
    )
    .expect_err("irrelevant retention fields must fail");
    assert_eq!(
        invalid_retention.invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::InvalidFieldCombination)
    );

    let non_local = Governance::new(
        Sensitivity::Personal,
        EgressPolicy::CloudAllowed,
        RetentionRule::new(RetentionMode::UntilDeleted, None, None)
            .expect("retention must be valid"),
        DeletionState::Active,
        id("policy-too-wide"),
    )
    .expect_err("M0 governance must be local only");
    assert_eq!(
        non_local.invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::NonLocalGovernance)
    );

    assert_eq!(
        Digest::parse("sha256", "exact-bytes-v1", &"A".repeat(64))
            .expect_err("uppercase digest must fail")
            .invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::InvalidDigestValue)
    );
}

#[test]
fn source_memory_and_state_conditions_are_enforced() {
    let mut source = source_artifact_params();
    source.media_type = MediaType::TextMarkdown;
    assert_eq!(
        SourceArtifact::new(source)
            .expect_err("source kind and media type must match")
            .invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::InvalidFieldCombination)
    );

    let mut fragment = source_fragment_params();
    fragment.byte_end = 2;
    assert_eq!(
        SourceFragment::new(fragment)
            .expect_err("fragment length must match byte range")
            .invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::ContentLengthMismatch)
    );

    let mut proposal = proposal_params();
    proposal.target_memory_ids.push(id("unexpected-target"));
    assert_eq!(
        MemoryProposal::new(proposal)
            .expect_err("create proposal cannot have target memory")
            .invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::InvalidFieldCombination)
    );

    let mut decision = decision_params();
    decision.result_memory_id = None;
    assert_eq!(
        MemoryDecision::new(decision)
            .expect_err("accept decision requires result memory")
            .invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::InvalidFieldCombination)
    );

    let mut record = memory_record_params();
    record.content_digest = compute_nfc_text_digest("different memory");
    assert_eq!(
        MemoryRecord::new(record)
            .expect_err("record digest must match value digest")
            .code(),
        CoreErrorCode::DigestMismatch
    );

    let mut event = state_event_params();
    event.from_state = Some(MemoryState::Confirmed);
    assert_eq!(
        MemoryStateEvent::new(event)
            .expect_err("initial confirmed event cannot have from state")
            .invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::InvalidStateTransition)
    );
}

#[test]
fn context_conditions_reject_ambiguous_or_unverifiable_fields() {
    assert_eq!(
        Budget::new(2, 3)
            .expect_err("used budget cannot exceed limit")
            .invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::BudgetExceeded)
    );
    assert_eq!(
        TruncationFacts::new(true, 3, 3, Some(text("budget")))
            .expect_err("truncated output must be shorter")
            .invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::InvalidFieldCombination)
    );

    let rendered_content = text("policy constraint");
    let invalid_constraint = ContextItem::new(ContextItemParams {
        item_id: id("constraint-1"),
        ordinal: 0,
        item_type: ContextItemType::Constraint,
        object_refs: vec![],
        content_digest: compute_nfc_text_digest(rendered_content.as_str()),
        rendered_content,
        evidence_refs: vec![EvidenceRef::new(EvidenceType::MemoryRecord, id("memory-1"))],
        citation_ids: vec![],
        selection_reason_codes: vec![text("policy")],
        temporal_role: TemporalRole::NotApplicable,
        truncation: TruncationFacts::new(false, 17, 17, None).expect("length facts must be valid"),
    })
    .expect_err("constraint without object refs needs policy evidence");
    assert_eq!(
        invalid_constraint.invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::InvalidFieldCombination)
    );

    let mut context = context_pack_params();
    context.citation_map.clear();
    assert_eq!(
        ContextPack::new(context)
            .expect_err("citation IDs must resolve inside the pack")
            .invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::InvalidFieldCombination)
    );
}

#[test]
fn deletion_conditions_prevent_false_completion() {
    let left = ObjectRef::new(CanonicalObjectType::MemoryRecord, id("memory-z"));
    let right = ObjectRef::new(CanonicalObjectType::MemoryRecord, id("memory-a"));
    let unsorted = FrozenTargetClosure::new(
        vec![left, right],
        compute_digest("canonical-json-v1", "[]").expect("digest must compute"),
    )
    .expect_err("frozen target closure must already be sorted");
    assert_eq!(
        unsorted.invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::UnsortedTargetClosure)
    );

    let mut failed = component_result().params().clone();
    failed.status = ComponentStatus::Failed;
    failed.processed_count = 0;
    failed.outcome = ComponentOutcome::NotApplicable;
    assert_eq!(
        ComponentResult::new(failed)
            .expect_err("failed result needs error fields")
            .invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::InvalidFieldCombination)
    );

    let retained = ComponentResult::new(ComponentResultParams {
        component_key: id("minimal-audit"),
        component_type: DeletionComponentType::MinimalAudit,
        target_ref: deletion_target_ref(),
        required_action: RequiredAction::RetainMinimal,
        target_count: 1,
        processed_count: 1,
        status: ComponentStatus::Succeeded,
        outcome: ComponentOutcome::RetainedMinimal,
        verification_method: text("fixture-minimal-audit-check"),
        checked_at: timestamp("2026-08-23T08:00:08Z"),
        error_code: None,
        retryable: None,
        retention_basis: None,
    })
    .expect_err("retained minimal result needs policy basis");
    assert_eq!(
        retained.invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::InvalidFieldCombination)
    );

    let mut evidence = deletion_evidence_params();
    let mut failed = component_result().params().clone();
    failed.status = ComponentStatus::Failed;
    failed.processed_count = 0;
    failed.outcome = ComponentOutcome::NotApplicable;
    failed.error_code = Some(text("fixture-delete-failed"));
    failed.retryable = Some(true);
    evidence.component_results = vec![ComponentResult::new(failed).expect("failure must be valid")];
    assert_eq!(
        DeletionEvidence::new(evidence)
            .expect_err("completed evidence cannot contain failed component")
            .invalid_canonical_object_reason(),
        Some(InvalidCanonicalObjectReason::InvalidFieldCombination)
    );
}

#[test]
fn rejected_sensitive_content_is_not_copied_into_errors_or_debug_output() {
    let private_content = text("private fixture body that must not leak");
    let error = MemoryValue::new(
        private_content,
        compute_nfc_text_digest("different synthetic body"),
    )
    .expect_err("mismatched digest must fail");

    assert_eq!(error.code(), CoreErrorCode::DigestMismatch);
    assert!(!error.to_string().contains("private fixture body"));
    assert!(!format!("{error:?}").contains("private fixture body"));

    let object_debug = format!(
        "{:?}",
        SourceArtifactParams {
            content: text("another private fixture body"),
            ..source_artifact_params()
        }
    );
    assert!(!object_debug.contains("another private fixture body"));
    assert!(object_debug.contains("utf8_bytes"));
}
