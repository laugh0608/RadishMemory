use radishmemory_core::{
    ActorRef, ActorType, CanonicalObjectType, ComponentStatus, Decision, DeleteRequest,
    DeleteRequestParams, DeletionComponentType, DeletionEvidence, DeletionEvidenceParams,
    DeletionOverallStatus, DeletionState, DeletionStore, DeletionTarget, DeletionTargetRef,
    EgressPolicy, EvidenceRef, EvidenceType, FrozenTargetClosure, Governance, Identifier,
    LocalDeletionExecution, LocalSearch, LocalSearchRequest, MediaType, MemoryDecision,
    MemoryDecisionParams, MemoryEventType, MemoryProposal, MemoryProposalParams, MemoryRecord,
    MemoryRecordParams, MemoryState, MemoryStateEvent, MemoryStateEventParams, MemoryStore,
    MemoryType, MemoryValue, NonEmptyText, ObjectRef, ProducerRef, ProducerType, ProposalOperation,
    RequestedGuarantee, RequiredAction, RetentionMode, RetentionRule, Sensitivity, SourceArtifact,
    SourceArtifactParams, SourceFragment, SourceFragmentParams, SourceKind, SourceOriginKind,
    SourceVault, TimePrecision, Timestamp, UnitInterval, ValidTime, ValidTimeMode, Version,
    compute_digest, compute_exact_bytes_digest,
};
use radishmemory_file_entry as _;
use radishmemory_sqlite::SqliteDatabase;
use rusqlite::Connection;

mod support;

use support::SyntheticDatabase;

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

fn id(value: &str) -> Identifier {
    Identifier::new(value).expect("synthetic identifier must be valid")
}

fn text(value: &str) -> NonEmptyText {
    NonEmptyText::new(value).expect("synthetic text must be nonempty")
}

fn timestamp(value: &str) -> Timestamp {
    Timestamp::parse(value).expect("synthetic timestamp must be valid")
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
    .expect("governance must be local")
}

fn producer() -> ProducerRef {
    ProducerRef::new(ProducerType::TestFixture, id("fixture-producer"), text("1"))
}

fn actor() -> ActorRef {
    ActorRef::new(ActorType::TestFixture, id("fixture-actor"), Some(text("1")))
}

fn seed_source(database: &mut SqliteDatabase, source_id: &str, fragment_id: &str) {
    let content = text("offline memory deletion fixture");
    let source = SourceArtifact::new(SourceArtifactParams {
        source_id: id(source_id),
        lineage_id: id(&format!("{source_id}-lineage")),
        version: Version::new(1).expect("version must be valid"),
        namespace_id: id("namespace-1"),
        source_kind: SourceKind::Text,
        media_type: MediaType::TextPlain,
        content_length: u64::try_from(content.utf8_len()).expect("length must fit"),
        content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
        content: content.clone(),
        title: Some(text("Synthetic deletion source")),
        origin_kind: SourceOriginKind::SyntheticFixture,
        origin_ref: Some(text("fixture://deletion-source")),
        observed_at: timestamp("2026-08-26T08:00:00Z"),
        captured_at: timestamp("2026-08-26T08:00:01Z"),
        supersedes_source_ids: vec![],
        governance: governance(),
        producer: producer(),
        created_at: timestamp("2026-08-26T08:00:01Z"),
    })
    .expect("source must be valid");
    let fragment = SourceFragment::new(SourceFragmentParams {
        fragment_id: id(fragment_id),
        namespace_id: id("namespace-1"),
        source_id: id(source_id),
        ordinal: 0,
        byte_start: 0,
        byte_end: u64::try_from(content.utf8_len()).expect("length must fit"),
        heading_path: Some(vec![text("Deletion fixture")]),
        content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
        content,
        segmenter: producer(),
        governance: governance(),
        created_at: timestamp("2026-08-26T08:00:02Z"),
    })
    .expect("fragment must be valid");
    database
        .store_source_artifact(&source)
        .expect("source must persist");
    database
        .store_source_fragments(&[fragment])
        .expect("fragment must persist");
}

fn seed_memory(database: &mut SqliteDatabase, fragment_id: &str, memory_id: &str) {
    let proposal = MemoryProposal::new(MemoryProposalParams {
        proposal_id: id("proposal-1"),
        namespace_id: id("namespace-1"),
        operation: ProposalOperation::Create,
        memory_type: MemoryType::Preference,
        subject_ref: id("project:synthetic"),
        proposed_content: MemoryValue::from_text(text("offline memory deletion preference")),
        source_fragment_refs: vec![id(fragment_id)],
        target_memory_ids: vec![],
        observed_at: timestamp("2026-08-26T08:00:03Z"),
        valid_time: ValidTime::new(
            ValidTimeMode::OpenEnded,
            Some(timestamp("2026-08-26T08:00:03Z")),
            None,
            TimePrecision::Exact,
        )
        .expect("valid time must be valid"),
        confidence: UnitInterval::new(0.9).expect("confidence must be valid"),
        importance: UnitInterval::new(0.8).expect("importance must be valid"),
        governance: governance(),
        producer: producer(),
        reason_code: text("fixture-proposal"),
        proposed_at: timestamp("2026-08-26T08:00:03Z"),
    })
    .expect("proposal must be valid");
    let decision = MemoryDecision::new(MemoryDecisionParams {
        decision_id: id("decision-1"),
        namespace_id: id("namespace-1"),
        proposal_id: proposal.params().proposal_id.clone(),
        previous_decision_id: None,
        decision: Decision::Accept,
        decided_by: actor(),
        authorization_basis: text("explicit-fixture-authorization"),
        reason_code: text("fixture-accept"),
        reason_text: Some(text("Synthetic reason that must be redacted")),
        result_memory_id: Some(id(memory_id)),
        decided_at: timestamp("2026-08-26T08:00:04Z"),
    })
    .expect("decision must be valid");
    let event = MemoryStateEvent::new(MemoryStateEventParams {
        event_id: id("event-1"),
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
        occurred_at: timestamp("2026-08-26T08:00:05Z"),
    })
    .expect("event must be valid");
    let record = MemoryRecord::new(MemoryRecordParams {
        memory_id: id(memory_id),
        lineage_id: id("memory-lineage-1"),
        version: Version::new(1).expect("version must be valid"),
        namespace_id: id("namespace-1"),
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
        governance: governance(),
        current_state: MemoryState::Confirmed,
        last_state_event_id: event.params().event_id.clone(),
        supersedes_memory_ids: vec![],
        contradicts_memory_ids: vec![],
        content_digest: proposal.params().proposed_content.content_digest().clone(),
        created_at: timestamp("2026-08-26T08:00:05Z"),
    })
    .expect("record must be valid");
    database
        .store_memory_proposal(&proposal)
        .expect("proposal must persist");
    database
        .store_memory_decision(&decision)
        .expect("decision must persist");
    database
        .materialize_accepted_memory(&record, &event, &[])
        .expect("record must materialize");
}

fn delete_request(request_id: &str, target_refs: Vec<ObjectRef>) -> DeleteRequest {
    let target_ref = if target_refs.len() == 1 {
        DeletionTargetRef::Object(target_refs[0].clone())
    } else {
        DeletionTargetRef::FrozenClosure(
            FrozenTargetClosure::freeze(target_refs.clone()).expect("closure must freeze"),
        )
    };
    let target_count = u64::try_from(target_refs.len()).expect("target count must fit");
    let components = PROFILE
        .iter()
        .map(|(key, component_type, action)| {
            DeletionTarget::new(
                id(key),
                *component_type,
                target_ref.clone(),
                target_count,
                *action,
            )
            .expect("component must be valid")
        })
        .collect();
    DeleteRequest::new(DeleteRequestParams {
        delete_request_id: id(request_id),
        namespace_id: id("namespace-1"),
        requested_by: actor(),
        authorization_basis: text("explicit-fixture-deletion-authorization"),
        requested_guarantee: RequestedGuarantee::LocalPurge,
        device_id: id("device-local-1"),
        target_refs,
        planned_components: components,
        reason_code: text("fixture-local-purge"),
        requested_at: timestamp("2026-08-26T08:01:00Z"),
    })
    .expect("request must be valid")
}

fn execution(at: &str) -> LocalDeletionExecution {
    LocalDeletionExecution::new(
        timestamp(at),
        EvidenceRef::new(EvidenceType::PolicyBasis, id("policy-local-deletion-v1")),
    )
    .expect("execution inputs must be valid")
}

fn evidence(
    evidence_id: &str,
    request: &DeleteRequest,
    previous: Option<&str>,
    results: Vec<radishmemory_core::ComponentResult>,
    started_at: &str,
) -> DeletionEvidence {
    let overall_status = if results
        .iter()
        .all(|result| result.params().status == ComponentStatus::Succeeded)
    {
        DeletionOverallStatus::Completed
    } else {
        DeletionOverallStatus::Failed
    };
    DeletionEvidence::new(DeletionEvidenceParams {
        deletion_evidence_id: id(evidence_id),
        delete_request_id: request.params().delete_request_id.clone(),
        previous_evidence_id: previous.map(id),
        namespace_id: request.params().namespace_id.clone(),
        device_id: request.params().device_id.clone(),
        overall_status,
        component_results: results,
        started_at: timestamp(started_at),
        finished_at: Some(timestamp("2026-08-26T08:02:59Z")),
        verified_by: producer(),
        evidence_digest: compute_digest(
            "deletion-evidence-v1",
            &format!(r#"{{"evidence_id":"{evidence_id}"}}"#),
        )
        .expect("evidence digest must compute"),
    })
    .expect("evidence must be valid")
}

fn search_request() -> LocalSearchRequest {
    LocalSearchRequest::new(
        id("namespace-1"),
        text("offline"),
        timestamp("2026-08-26T09:00:00Z"),
        10,
        vec![Sensitivity::Personal],
    )
    .expect("search request must be valid")
}

#[test]
fn local_purge_closes_recall_executes_all_components_and_chains_idempotent_evidence() {
    let synthetic = SyntheticDatabase::new("deletion-complete");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(&mut database, "source-1", "fragment-1");
    seed_memory(&mut database, "fragment-1", "memory-1");
    assert_eq!(
        database
            .search(&search_request())
            .expect("pre-delete search must work")
            .len(),
        2
    );

    let request = delete_request(
        "delete-request-1",
        vec![
            ObjectRef::new(CanonicalObjectType::SourceArtifact, id("source-1")),
            ObjectRef::new(CanonicalObjectType::MemoryRecord, id("memory-1")),
        ],
    );
    database
        .store_delete_request(&request)
        .expect("deletion plan must persist");
    assert_eq!(
        database
            .load_delete_request(&id("namespace-1"), &id("delete-request-1"))
            .expect("request lookup must work"),
        Some(request.clone())
    );
    assert!(
        database
            .load_delete_request(&id("namespace-other"), &id("delete-request-1"))
            .expect("wrong namespace request lookup must remain safe")
            .is_none()
    );

    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-1"))
            .expect("source lookup must work")
            .is_none()
    );
    assert!(
        database
            .load_memory_record(&id("namespace-1"), &id("memory-1"))
            .expect("memory lookup must work")
            .is_none()
    );
    assert!(
        database
            .search(&search_request())
            .expect("planned deletion must already stop recall")
            .is_empty()
    );
    database
        .verify_recall_derivations()
        .expect("closed recall derivations must remain exact");

    let first_results = database
        .execute_deletion(
            &id("namespace-1"),
            &id("delete-request-1"),
            &execution("2026-08-26T08:02:00Z"),
        )
        .expect("deletion must execute");
    assert_eq!(first_results.len(), 10);
    assert!(
        first_results
            .iter()
            .all(|result| result.params().status == ComponentStatus::Succeeded)
    );
    let first_evidence = evidence(
        "deletion-evidence-1",
        &request,
        None,
        first_results,
        "2026-08-26T08:02:00Z",
    );
    database
        .store_deletion_evidence(&first_evidence)
        .expect("first evidence must persist");
    assert_eq!(
        database
            .load_deletion_evidence(&id("namespace-1"), &id("deletion-evidence-1"))
            .expect("evidence lookup must work"),
        Some(first_evidence)
    );
    assert!(
        database
            .load_deletion_evidence(&id("namespace-other"), &id("deletion-evidence-1"))
            .expect("wrong namespace lookup must remain safe")
            .is_none()
    );

    let second_results = database
        .execute_deletion(
            &id("namespace-1"),
            &id("delete-request-1"),
            &execution("2026-08-26T08:02:30Z"),
        )
        .expect("idempotent retry must execute");
    assert!(
        second_results
            .iter()
            .all(|result| result.params().status == ComponentStatus::Succeeded)
    );
    let second_evidence = evidence(
        "deletion-evidence-2",
        &request,
        Some("deletion-evidence-1"),
        second_results,
        "2026-08-26T08:02:30Z",
    );
    database
        .store_deletion_evidence(&second_evidence)
        .expect("second evidence must extend the chain");

    database
        .rebuild_recall_derivations()
        .expect("rebuild must not restore deleted targets");
    assert!(
        database
            .search(&search_request())
            .expect("search after rebuild must work")
            .is_empty()
    );
    drop(database);
    let database = SqliteDatabase::open(synthetic.path()).expect("deleted database must reopen");
    assert!(
        database
            .search(&search_request())
            .expect("search after reopen must work")
            .is_empty()
    );
    assert_eq!(
        database
            .load_deletion_evidence(&id("namespace-1"), &id("deletion-evidence-2"))
            .expect("latest evidence must survive reopen"),
        Some(second_evidence)
    );
    drop(database);

    let raw = Connection::open(synthetic.path()).expect("database must reopen for inspection");
    let source: (String, Option<String>, Option<String>) = raw
        .query_row(
            "SELECT deletion_state, title, origin_ref
             FROM radishmemory_source_artifacts WHERE source_id = 'source-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("source state must exist");
    let memory: (String, String) = raw
        .query_row(
            "SELECT deletion_state, content_text FROM radishmemory_memory_records
             WHERE memory_id = 'memory-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("memory audit row must exist");
    let source_body_count: i64 = raw
        .query_row(
            "SELECT COUNT(*) FROM radishmemory_source_bodies WHERE source_id = 'source-1'",
            [],
            |row| row.get(0),
        )
        .expect("source body count must be queryable");
    let proposal: (String, String) = raw
        .query_row(
            "SELECT deletion_state, content_text FROM radishmemory_memory_proposals
             WHERE proposal_id = 'proposal-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("proposal audit row must exist");
    let decision_reason: Option<String> = raw
        .query_row(
            "SELECT reason_text FROM radishmemory_memory_decisions
             WHERE decision_id = 'decision-1'",
            [],
            |row| row.get(0),
        )
        .expect("decision audit row must exist");
    let component_counts: (i64, i64, i64, i64) = raw
        .query_row(
            "SELECT
                 (SELECT COUNT(*) FROM radishmemory_source_fragments WHERE source_id = 'source-1'),
                 (SELECT COUNT(*) FROM radishmemory_memory_state_events WHERE event_id = 'event-1'),
                 (SELECT COUNT(*) FROM radishmemory_recall_fts),
                 (SELECT COUNT(*) FROM radishmemory_memory_current_projection
                  WHERE memory_id = 'memory-1')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("component verification counts must be queryable");
    assert_eq!(source, ("deleted".to_owned(), None, None));
    assert_eq!(
        memory,
        ("deleted".to_owned(), "[redacted:local-deletion]".to_owned())
    );
    assert_eq!(proposal, memory);
    assert_eq!(decision_reason, None);
    assert_eq!(source_body_count, 0);
    assert_eq!(component_counts, (0, 1, 0, 0));
}

#[test]
fn failed_full_text_component_keeps_target_closed_and_persists_failed_evidence() {
    let synthetic = SyntheticDatabase::new("deletion-failed-component");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(&mut database, "source-failed", "fragment-failed");
    let request = delete_request(
        "delete-request-failed",
        vec![ObjectRef::new(
            CanonicalObjectType::SourceArtifact,
            id("source-failed"),
        )],
    );
    database
        .store_delete_request(&request)
        .expect("deletion plan must persist");

    let raw = Connection::open(synthetic.path()).expect("database must reopen for fault injection");
    raw.execute_batch("DROP TABLE radishmemory_recall_fts;")
        .expect("synthetic FTS failure must be installed");
    drop(raw);

    let results = database
        .execute_deletion(
            &id("namespace-1"),
            &id("delete-request-failed"),
            &execution("2026-08-26T08:02:00Z"),
        )
        .expect("component failure must be represented as results");
    let failures = results
        .iter()
        .filter(|result| result.params().status == ComponentStatus::Failed)
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0].params().component_type,
        DeletionComponentType::FullTextIndex
    );
    assert_eq!(
        failures[0]
            .params()
            .error_code
            .as_ref()
            .map(NonEmptyText::as_str),
        Some("sqlite-deletion-component-failed")
    );
    assert_eq!(failures[0].params().retryable, Some(true));

    let failed_evidence = evidence(
        "deletion-evidence-failed",
        &request,
        None,
        results,
        "2026-08-26T08:02:00Z",
    );
    database
        .store_deletion_evidence(&failed_evidence)
        .expect("failed evidence must persist truthfully");
    assert_eq!(
        database
            .load_deletion_evidence(&id("namespace-1"), &id("deletion-evidence-failed"))
            .expect("failed evidence must load"),
        Some(failed_evidence)
    );
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-failed"))
            .expect("closed target lookup must work")
            .is_none()
    );

    let raw = Connection::open(synthetic.path()).expect("database must reopen for inspection");
    let deletion_state: String = raw
        .query_row(
            "SELECT deletion_state FROM radishmemory_source_artifacts
             WHERE source_id = 'source-failed'",
            [],
            |row| row.get(0),
        )
        .expect("source state must remain auditable");
    let fragment_count: i64 = raw
        .query_row(
            "SELECT COUNT(*) FROM radishmemory_source_fragments
             WHERE source_id = 'source-failed'",
            [],
            |row| row.get(0),
        )
        .expect("fragment count must be queryable");
    assert_eq!(deletion_state, "failed");
    assert_eq!(fragment_count, 0);
}

#[test]
fn source_plan_rejects_an_unexpanded_active_memory_dependency() {
    let synthetic = SyntheticDatabase::new("deletion-unexpanded-memory");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(&mut database, "source-linked", "fragment-linked");
    seed_memory(&mut database, "fragment-linked", "memory-linked");
    let incomplete = delete_request(
        "delete-request-incomplete",
        vec![ObjectRef::new(
            CanonicalObjectType::SourceArtifact,
            id("source-linked"),
        )],
    );

    database
        .store_delete_request(&incomplete)
        .expect_err("active linked memory must be included explicitly");
    assert_eq!(
        database
            .search(&search_request())
            .expect("rejected plan must not mutate recall")
            .len(),
        2
    );
}
