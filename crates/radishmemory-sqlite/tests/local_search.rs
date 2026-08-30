use radishmemory_core::{
    ActorRef, ActorType, Decision, DeletionState, EgressPolicy, EvidenceRef, EvidenceType,
    Governance, Identifier, LocalSearch, LocalSearchHit, LocalSearchRequest, MediaType,
    MemoryDecision, MemoryDecisionParams, MemoryEventType, MemoryProposal, MemoryProposalParams,
    MemoryRecord, MemoryRecordParams, MemoryState, MemoryStateEvent, MemoryStateEventParams,
    MemoryStore, MemoryType, MemoryValue, NonEmptyText, ProducerRef, ProducerType,
    ProposalOperation, RetentionMode, RetentionRule, Sensitivity, SourceArtifact,
    SourceArtifactParams, SourceFragment, SourceFragmentParams, SourceKind, SourceOriginKind,
    SourceVault, TimePrecision, Timestamp, UnitInterval, ValidTime, ValidTimeMode, Version,
    compute_exact_bytes_digest,
};
use radishmemory_file_entry as _;
use radishmemory_sqlite::{SqliteDatabase, SqliteErrorCode, SqliteStorageReason};
use rusqlite::{Connection, params};

mod support;

use support::SyntheticDatabase;

fn id(value: &str) -> Identifier {
    Identifier::new(value).expect("synthetic identifier must be valid")
}

fn text(value: &str) -> NonEmptyText {
    NonEmptyText::new(value).expect("synthetic text must be nonempty")
}

fn timestamp(value: &str) -> Timestamp {
    Timestamp::parse(value).expect("synthetic timestamp must be valid")
}

fn governance(sensitivity: Sensitivity) -> Governance {
    governance_with_state(sensitivity, DeletionState::Active)
}

fn governance_with_state(sensitivity: Sensitivity, deletion_state: DeletionState) -> Governance {
    Governance::new(
        sensitivity,
        EgressPolicy::LocalOnly,
        RetentionRule::new(RetentionMode::UntilDeleted, None, None)
            .expect("retention must be valid"),
        deletion_state,
        id("policy-local-only"),
    )
    .expect("M0 governance must be local")
}

fn producer() -> ProducerRef {
    ProducerRef::new(ProducerType::TestFixture, id("fixture-producer"), text("1"))
}

fn actor() -> ActorRef {
    ActorRef::new(ActorType::TestFixture, id("fixture-actor"), Some(text("1")))
}

fn valid_time(value: &str) -> ValidTime {
    ValidTime::new(
        ValidTimeMode::OpenEnded,
        Some(timestamp(value)),
        None,
        TimePrecision::Exact,
    )
    .expect("synthetic valid time must be valid")
}

struct SourceSpec<'a> {
    namespace_id: &'a str,
    source_id: &'a str,
    fragment_id: &'a str,
    content: &'a str,
    captured_at: &'a str,
    sensitivity: Sensitivity,
}

fn source_and_fragment(spec: SourceSpec<'_>) -> (SourceArtifact, SourceFragment) {
    let content = text(spec.content);
    let governance = governance(spec.sensitivity);
    let source = SourceArtifact::new(SourceArtifactParams {
        source_id: id(spec.source_id),
        lineage_id: id(&format!("{}-lineage", spec.source_id)),
        version: Version::new(1).expect("version must be positive"),
        namespace_id: id(spec.namespace_id),
        source_kind: SourceKind::Text,
        media_type: MediaType::TextPlain,
        content_length: u64::try_from(content.utf8_len()).expect("length must fit u64"),
        content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
        content: content.clone(),
        title: Some(text("Synthetic local-search source")),
        origin_kind: SourceOriginKind::SyntheticFixture,
        origin_ref: None,
        observed_at: timestamp(spec.captured_at),
        captured_at: timestamp(spec.captured_at),
        supersedes_source_ids: vec![],
        governance: governance.clone(),
        producer: producer(),
        created_at: timestamp(spec.captured_at),
    })
    .expect("synthetic source must be valid");
    let fragment = SourceFragment::new(SourceFragmentParams {
        fragment_id: id(spec.fragment_id),
        namespace_id: id(spec.namespace_id),
        source_id: id(spec.source_id),
        ordinal: 0,
        byte_start: 0,
        byte_end: u64::try_from(content.utf8_len()).expect("length must fit u64"),
        heading_path: None,
        content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
        content,
        segmenter: producer(),
        governance,
        created_at: timestamp(spec.captured_at),
    })
    .expect("synthetic fragment must be valid");
    (source, fragment)
}

fn seed_source(database: &mut SqliteDatabase, spec: SourceSpec<'_>) -> SourceFragment {
    let (source, fragment) = source_and_fragment(spec);
    database
        .store_source_artifact(&source)
        .expect("source must persist");
    database
        .store_source_fragments(std::slice::from_ref(&fragment))
        .expect("fragment must persist");
    fragment
}

struct ProposalSpec<'a> {
    proposal_id: &'a str,
    namespace_id: &'a str,
    fragment_id: &'a str,
    operation: ProposalOperation,
    target_memory_ids: Vec<Identifier>,
    content: &'a str,
    valid_from: &'a str,
}

fn proposal(spec: ProposalSpec<'_>) -> MemoryProposal {
    MemoryProposal::new(MemoryProposalParams {
        proposal_id: id(spec.proposal_id),
        namespace_id: id(spec.namespace_id),
        operation: spec.operation,
        memory_type: MemoryType::Preference,
        subject_ref: id("project:synthetic"),
        proposed_content: MemoryValue::from_text(text(spec.content)),
        source_fragment_refs: vec![id(spec.fragment_id)],
        target_memory_ids: spec.target_memory_ids,
        observed_at: timestamp(spec.valid_from),
        valid_time: valid_time(spec.valid_from),
        confidence: UnitInterval::new(0.9).expect("confidence must be valid"),
        importance: UnitInterval::new(0.7).expect("importance must be valid"),
        governance: governance(Sensitivity::Personal),
        producer: producer(),
        reason_code: text("fixture-memory-proposal"),
        proposed_at: timestamp(spec.valid_from),
    })
    .expect("synthetic proposal must be valid")
}

fn decision(
    decision_id: &str,
    namespace_id: &str,
    proposal_id: &str,
    result_memory_id: &str,
    decided_at: &str,
) -> MemoryDecision {
    MemoryDecision::new(MemoryDecisionParams {
        decision_id: id(decision_id),
        namespace_id: id(namespace_id),
        proposal_id: id(proposal_id),
        previous_decision_id: None,
        decision: Decision::Accept,
        decided_by: actor(),
        authorization_basis: text("explicit-synthetic-authorization"),
        reason_code: text("fixture-memory-decision"),
        reason_text: None,
        result_memory_id: Some(id(result_memory_id)),
        decided_at: timestamp(decided_at),
    })
    .expect("synthetic decision must be valid")
}

struct RecordSpec<'a> {
    memory_id: &'a str,
    lineage_id: &'a str,
    version: u64,
    proposal: &'a MemoryProposal,
    decision: &'a MemoryDecision,
    initial_event_id: &'a str,
    created_at: &'a str,
}

fn record(spec: RecordSpec<'_>) -> MemoryRecord {
    let proposal = spec.proposal.params();
    MemoryRecord::new(MemoryRecordParams {
        memory_id: id(spec.memory_id),
        lineage_id: id(spec.lineage_id),
        version: Version::new(spec.version).expect("version must be positive"),
        namespace_id: proposal.namespace_id.clone(),
        memory_type: proposal.memory_type,
        subject_ref: proposal.subject_ref.clone(),
        content: proposal.proposed_content.clone(),
        source_fragment_refs: proposal.source_fragment_refs.clone(),
        origin_proposal_id: proposal.proposal_id.clone(),
        accepted_by_decision_id: spec.decision.params().decision_id.clone(),
        observed_at: proposal.observed_at.clone(),
        valid_time: proposal.valid_time.clone(),
        confidence: proposal.confidence,
        importance: proposal.importance,
        governance: proposal.governance.clone(),
        current_state: MemoryState::Confirmed,
        last_state_event_id: id(spec.initial_event_id),
        supersedes_memory_ids: proposal.target_memory_ids.clone(),
        contradicts_memory_ids: vec![],
        content_digest: proposal.proposed_content.content_digest().clone(),
        created_at: timestamp(spec.created_at),
    })
    .expect("synthetic record must be valid")
}

fn initial_event(
    event_id: &str,
    namespace_id: &str,
    memory_id: &str,
    decision_id: &str,
    occurred_at: &str,
) -> MemoryStateEvent {
    MemoryStateEvent::new(MemoryStateEventParams {
        event_id: id(event_id),
        namespace_id: id(namespace_id),
        memory_id: id(memory_id),
        previous_event_id: None,
        event_type: MemoryEventType::Confirmed,
        from_state: None,
        cause_ref: EvidenceRef::new(EvidenceType::MemoryDecision, id(decision_id)),
        related_memory_ids: vec![],
        actor: actor(),
        reason_code: text("fixture-confirmed"),
        effective_at: None,
        occurred_at: timestamp(occurred_at),
    })
    .expect("synthetic initial event must be valid")
}

struct TerminalEventSpec<'a> {
    event_id: &'a str,
    namespace_id: &'a str,
    memory_id: &'a str,
    previous_event_id: &'a str,
    event_type: MemoryEventType,
    cause_ref: EvidenceRef,
    related_memory_ids: Vec<Identifier>,
    effective_at: &'a str,
}

fn terminal_event(spec: TerminalEventSpec<'_>) -> MemoryStateEvent {
    MemoryStateEvent::new(MemoryStateEventParams {
        event_id: id(spec.event_id),
        namespace_id: id(spec.namespace_id),
        memory_id: id(spec.memory_id),
        previous_event_id: Some(id(spec.previous_event_id)),
        event_type: spec.event_type,
        from_state: Some(MemoryState::Confirmed),
        cause_ref: spec.cause_ref,
        related_memory_ids: spec.related_memory_ids,
        actor: actor(),
        reason_code: text("fixture-terminal-event"),
        effective_at: Some(timestamp(spec.effective_at)),
        occurred_at: timestamp(spec.effective_at),
    })
    .expect("synthetic terminal event must be valid")
}

fn persist_confirmed(
    database: &mut SqliteDatabase,
    proposal: &MemoryProposal,
    decision: &MemoryDecision,
    record: &MemoryRecord,
    event: &MemoryStateEvent,
) {
    database
        .store_memory_proposal(proposal)
        .expect("proposal must persist");
    database
        .store_memory_decision(decision)
        .expect("decision must persist");
    database
        .materialize_accepted_memory(record, event, &[])
        .expect("accepted memory must materialize");
}

fn request(
    namespace_id: &str,
    query: &str,
    as_of: &str,
    allowed_sensitivities: Vec<Sensitivity>,
) -> LocalSearchRequest {
    LocalSearchRequest::new(
        id(namespace_id),
        text(query),
        timestamp(as_of),
        5,
        allowed_sensitivities,
    )
    .expect("synthetic search request must be valid")
}

fn hit_ids(hits: &[LocalSearchHit]) -> Vec<&str> {
    hits.iter().map(|hit| hit.object_id().as_str()).collect()
}

#[test]
fn local_search_filters_before_top_k_and_uses_stable_ids_for_ties() {
    let synthetic = SyntheticDatabase::new("search-filters");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    for (source_id, fragment_id, sensitivity) in [
        ("source-b", "fragment-b", Sensitivity::Personal),
        ("source-a", "fragment-a", Sensitivity::Personal),
        (
            "source-restricted",
            "fragment-restricted",
            Sensitivity::Restricted,
        ),
    ] {
        seed_source(
            &mut database,
            SourceSpec {
                namespace_id: "namespace-1",
                source_id,
                fragment_id,
                content: "Project Orchard launch code is amber-47.\n",
                captured_at: "2026-08-01T08:00:00Z",
                sensitivity,
            },
        );
    }

    let hits = database
        .search(&request(
            "namespace-1",
            "Orchard launch code amber-47",
            "2026-08-02T00:00:00Z",
            vec![Sensitivity::Personal],
        ))
        .expect("local search must succeed");
    assert_eq!(hit_ids(&hits), vec!["fragment-a", "fragment-b"]);

    assert!(
        database
            .search(&request(
                "namespace-other",
                "Orchard launch code amber-47",
                "2026-08-02T00:00:00Z",
                vec![Sensitivity::Personal, Sensitivity::Restricted],
            ))
            .expect("wrong namespace search must stay isolated")
            .is_empty()
    );
    assert!(
        database
            .search(&request(
                "namespace-1",
                "Orchard launch code amber-47",
                "2026-07-31T00:00:00Z",
                vec![Sensitivity::Personal, Sensitivity::Restricted],
            ))
            .expect("future captures must not enter point-in-time search")
            .is_empty()
    );
}

#[test]
fn non_active_fragments_remain_facts_but_never_enter_the_fts_derivation() {
    let synthetic = SyntheticDatabase::new("non-active-fragment");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let (source, _) = source_and_fragment(SourceSpec {
        namespace_id: "namespace-1",
        source_id: "source-1",
        fragment_id: "fragment-unused",
        content: "Pending synthetic fragment content.\n",
        captured_at: "2026-08-01T08:00:00Z",
        sensitivity: Sensitivity::Personal,
    });
    let content = text("Pending synthetic fragment content.\n");
    let fragment = SourceFragment::new(SourceFragmentParams {
        fragment_id: id("fragment-pending"),
        namespace_id: id("namespace-1"),
        source_id: id("source-1"),
        ordinal: 0,
        byte_start: 0,
        byte_end: u64::try_from(content.utf8_len()).expect("length must fit u64"),
        heading_path: None,
        content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
        content,
        segmenter: producer(),
        governance: governance_with_state(Sensitivity::Personal, DeletionState::Pending),
        created_at: timestamp("2026-08-01T08:00:00Z"),
    })
    .expect("pending fragment remains a valid governed fact");
    database
        .store_source_artifact(&source)
        .expect("source must persist");
    database
        .store_source_fragments(&[fragment])
        .expect("non-active fragment fact must persist without indexing");

    assert!(
        database
            .search(&request(
                "namespace-1",
                "pending synthetic fragment content",
                "2026-08-02T00:00:00Z",
                vec![Sensitivity::Personal],
            ))
            .expect("search must exclude non-active fragment")
            .is_empty()
    );
    database
        .verify_recall_derivations()
        .expect("non-active fact and empty derived row must be consistent");
}

#[test]
fn proposals_never_enter_recall_and_accept_materialization_is_atomic_with_indexing() {
    let synthetic = SyntheticDatabase::new("proposal-recall");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(
        &mut database,
        SourceSpec {
            namespace_id: "namespace-1",
            source_id: "source-1",
            fragment_id: "fragment-1",
            content: "Preferred explanation language is concise Chinese.\n",
            captured_at: "2026-08-01T08:00:00Z",
            sensitivity: Sensitivity::Personal,
        },
    );
    let proposal = proposal(ProposalSpec {
        proposal_id: "proposal-1",
        namespace_id: "namespace-1",
        fragment_id: "fragment-1",
        operation: ProposalOperation::Create,
        target_memory_ids: vec![],
        content: "Preferred explanation language is concise Chinese.",
        valid_from: "2026-08-01T08:00:00Z",
    });
    database
        .store_memory_proposal(&proposal)
        .expect("proposal must persist");
    let query = request(
        "namespace-1",
        "preferred explanation language",
        "2026-08-02T00:00:00Z",
        vec![Sensitivity::Personal],
    );
    let before = database.search(&query).expect("source search must succeed");
    assert_eq!(hit_ids(&before), vec!["fragment-1"]);

    let decision = decision(
        "decision-1",
        "namespace-1",
        "proposal-1",
        "memory-1",
        "2026-08-01T08:00:01Z",
    );
    database
        .store_memory_decision(&decision)
        .expect("decision must persist");
    let record = record(RecordSpec {
        memory_id: "memory-1",
        lineage_id: "memory-lineage-1",
        version: 1,
        proposal: &proposal,
        decision: &decision,
        initial_event_id: "event-1",
        created_at: "2026-08-01T08:00:02Z",
    });
    let event = initial_event(
        "event-1",
        "namespace-1",
        "memory-1",
        "decision-1",
        "2026-08-01T08:00:02Z",
    );
    database
        .materialize_accepted_memory(&record, &event, &[])
        .expect("accepted memory must materialize");

    let after = database.search(&query).expect("memory search must succeed");
    assert_eq!(hit_ids(&after), vec!["memory-1", "fragment-1"]);
    database
        .verify_recall_derivations()
        .expect("live derivations must match a full rebuild");
}

#[test]
fn supersede_and_retract_remove_memories_from_ordinary_recall() {
    let synthetic = SyntheticDatabase::new("state-recall");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(
        &mut database,
        SourceSpec {
            namespace_id: "namespace-1",
            source_id: "source-blue",
            fragment_id: "fragment-blue",
            content: "Offline project theme is blue.\n",
            captured_at: "2026-08-01T08:00:00Z",
            sensitivity: Sensitivity::Personal,
        },
    );
    let old_proposal = proposal(ProposalSpec {
        proposal_id: "proposal-blue",
        namespace_id: "namespace-1",
        fragment_id: "fragment-blue",
        operation: ProposalOperation::Create,
        target_memory_ids: vec![],
        content: "Offline project theme is blue.",
        valid_from: "2026-08-01T08:00:00Z",
    });
    let old_decision = decision(
        "decision-blue",
        "namespace-1",
        "proposal-blue",
        "memory-blue",
        "2026-08-01T08:00:01Z",
    );
    let old_record = record(RecordSpec {
        memory_id: "memory-blue",
        lineage_id: "theme-lineage",
        version: 1,
        proposal: &old_proposal,
        decision: &old_decision,
        initial_event_id: "event-blue-confirmed",
        created_at: "2026-08-01T08:00:02Z",
    });
    let old_initial = initial_event(
        "event-blue-confirmed",
        "namespace-1",
        "memory-blue",
        "decision-blue",
        "2026-08-01T08:00:02Z",
    );
    persist_confirmed(
        &mut database,
        &old_proposal,
        &old_decision,
        &old_record,
        &old_initial,
    );

    seed_source(
        &mut database,
        SourceSpec {
            namespace_id: "namespace-1",
            source_id: "source-green",
            fragment_id: "fragment-green",
            content: "Offline project theme is green.\n",
            captured_at: "2026-08-03T08:00:00Z",
            sensitivity: Sensitivity::Personal,
        },
    );
    let new_proposal = proposal(ProposalSpec {
        proposal_id: "proposal-green",
        namespace_id: "namespace-1",
        fragment_id: "fragment-green",
        operation: ProposalOperation::Supersede,
        target_memory_ids: vec![id("memory-blue")],
        content: "Offline project theme is green.",
        valid_from: "2026-08-03T08:00:00Z",
    });
    let new_decision = decision(
        "decision-green",
        "namespace-1",
        "proposal-green",
        "memory-green",
        "2026-08-03T08:00:01Z",
    );
    let new_record = record(RecordSpec {
        memory_id: "memory-green",
        lineage_id: "theme-lineage",
        version: 2,
        proposal: &new_proposal,
        decision: &new_decision,
        initial_event_id: "event-green-confirmed",
        created_at: "2026-08-03T08:00:02Z",
    });
    let new_initial = initial_event(
        "event-green-confirmed",
        "namespace-1",
        "memory-green",
        "decision-green",
        "2026-08-03T08:00:02Z",
    );
    let old_superseded = terminal_event(TerminalEventSpec {
        event_id: "event-blue-superseded",
        namespace_id: "namespace-1",
        memory_id: "memory-blue",
        previous_event_id: "event-blue-confirmed",
        event_type: MemoryEventType::Superseded,
        cause_ref: EvidenceRef::new(EvidenceType::MemoryRecord, id("memory-green")),
        related_memory_ids: vec![id("memory-green")],
        effective_at: "2026-08-03T08:00:00Z",
    });
    database
        .store_memory_proposal(&new_proposal)
        .expect("replacement proposal must persist");
    database
        .store_memory_decision(&new_decision)
        .expect("replacement decision must persist");
    database
        .materialize_accepted_memory(&new_record, &new_initial, &[old_superseded])
        .expect("supersede must update facts and recall atomically");

    let blue_hits = database
        .search(&request(
            "namespace-1",
            "offline project theme blue",
            "2026-08-04T00:00:00Z",
            vec![Sensitivity::Personal],
        ))
        .expect("old source remains locally searchable");
    assert!(!hit_ids(&blue_hits).contains(&"memory-blue"));
    let green_hits = database
        .search(&request(
            "namespace-1",
            "offline project theme green",
            "2026-08-04T00:00:00Z",
            vec![Sensitivity::Personal],
        ))
        .expect("current memory must be searchable");
    assert!(hit_ids(&green_hits).contains(&"memory-green"));

    let retracted = terminal_event(TerminalEventSpec {
        event_id: "event-green-retracted",
        namespace_id: "namespace-1",
        memory_id: "memory-green",
        previous_event_id: "event-green-confirmed",
        event_type: MemoryEventType::Retracted,
        cause_ref: EvidenceRef::new(EvidenceType::PolicyBasis, id("policy-local-only")),
        related_memory_ids: vec![],
        effective_at: "2026-08-05T08:00:00Z",
    });
    database
        .append_memory_state_event(&retracted)
        .expect("retraction must update projection and recall atomically");
    let after_retract = database
        .search(&request(
            "namespace-1",
            "offline project theme green",
            "2026-08-06T00:00:00Z",
            vec![Sensitivity::Personal],
        ))
        .expect("search after retraction must succeed");
    assert!(!hit_ids(&after_retract).contains(&"memory-green"));
    database
        .verify_recall_derivations()
        .expect("state transitions must leave no derivation drift");
}

#[test]
fn derived_write_failures_roll_back_source_and_memory_facts() {
    let source_synthetic = SyntheticDatabase::new("source-index-rollback");
    let mut source_database =
        SqliteDatabase::open(source_synthetic.path()).expect("database must initialize");
    let (source, fragment) = source_and_fragment(SourceSpec {
        namespace_id: "namespace-1",
        source_id: "source-1",
        fragment_id: "fragment-1",
        content: "Atomic source indexing.\n",
        captured_at: "2026-08-01T08:00:00Z",
        sensitivity: Sensitivity::Personal,
    });
    source_database
        .store_source_artifact(&source)
        .expect("source must persist");
    let raw = Connection::open(source_synthetic.path()).expect("raw database must open");
    raw.execute_batch("DROP TABLE radishmemory_recall_fts;")
        .expect("synthetic FTS drift must be created");
    drop(raw);
    let error = source_database
        .store_source_fragments(&[fragment])
        .expect_err("missing FTS table must roll back fragment facts");
    assert_eq!(error.code(), SqliteErrorCode::Storage);
    let raw = Connection::open(source_synthetic.path()).expect("raw database must reopen");
    let fragment_count: i64 = raw
        .query_row(
            "SELECT COUNT(*) FROM radishmemory_source_fragments",
            [],
            |row| row.get(0),
        )
        .expect("fragment facts must remain readable");
    assert_eq!(fragment_count, 0);

    let memory_synthetic = SyntheticDatabase::new("memory-projection-rollback");
    let mut memory_database =
        SqliteDatabase::open(memory_synthetic.path()).expect("database must initialize");
    seed_source(
        &mut memory_database,
        SourceSpec {
            namespace_id: "namespace-1",
            source_id: "source-1",
            fragment_id: "fragment-1",
            content: "Atomic memory indexing.\n",
            captured_at: "2026-08-01T08:00:00Z",
            sensitivity: Sensitivity::Personal,
        },
    );
    let proposal = proposal(ProposalSpec {
        proposal_id: "proposal-1",
        namespace_id: "namespace-1",
        fragment_id: "fragment-1",
        operation: ProposalOperation::Create,
        target_memory_ids: vec![],
        content: "Atomic memory indexing.",
        valid_from: "2026-08-01T08:00:00Z",
    });
    let decision = decision(
        "decision-1",
        "namespace-1",
        "proposal-1",
        "memory-1",
        "2026-08-01T08:00:01Z",
    );
    memory_database
        .store_memory_proposal(&proposal)
        .expect("proposal must persist");
    memory_database
        .store_memory_decision(&decision)
        .expect("decision must persist");
    let record = record(RecordSpec {
        memory_id: "memory-1",
        lineage_id: "memory-lineage-1",
        version: 1,
        proposal: &proposal,
        decision: &decision,
        initial_event_id: "event-1",
        created_at: "2026-08-01T08:00:02Z",
    });
    let event = initial_event(
        "event-1",
        "namespace-1",
        "memory-1",
        "decision-1",
        "2026-08-01T08:00:02Z",
    );
    let raw = Connection::open(memory_synthetic.path()).expect("raw database must open");
    raw.execute_batch("DROP TABLE radishmemory_memory_current_projection;")
        .expect("synthetic projection drift must be created");
    drop(raw);
    let error = memory_database
        .materialize_accepted_memory(&record, &event, &[])
        .expect_err("missing projection must roll back memory facts");
    assert_eq!(error.code(), SqliteErrorCode::Storage);
    assert!(
        memory_database
            .load_memory_record(&id("namespace-1"), &id("memory-1"))
            .expect("fact lookup must remain usable")
            .is_none()
    );
}

#[test]
fn drift_fails_closed_and_full_rebuild_restores_exact_derivations() {
    let synthetic = SyntheticDatabase::new("recall-rebuild");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(
        &mut database,
        SourceSpec {
            namespace_id: "namespace-1",
            source_id: "source-1",
            fragment_id: "fragment-1",
            content: "Private synthetic orchard marker.\n",
            captured_at: "2026-08-01T08:00:00Z",
            sensitivity: Sensitivity::Personal,
        },
    );
    let raw = Connection::open(synthetic.path()).expect("raw database must open");
    raw.execute(
        "UPDATE radishmemory_recall_fts SET content = ?1 WHERE object_id = ?2",
        params!["private-tampered-derived-content", "fragment-1"],
    )
    .expect("synthetic FTS row must be tampered");
    drop(raw);

    let error = database
        .verify_recall_derivations()
        .expect_err("derived drift must fail closed");
    assert_eq!(error.code(), SqliteErrorCode::InvalidStoredData);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::DerivedDataMismatch)
    );
    assert!(!format!("{error:?}").contains("private-tampered-derived-content"));
    let query = request(
        "namespace-1",
        "private orchard marker",
        "2026-08-02T00:00:00Z",
        vec![Sensitivity::Personal],
    );
    assert!(database.search(&query).is_err());

    database
        .rebuild_recall_derivations()
        .expect("full rebuild must restore derivations from facts");
    assert_eq!(
        hit_ids(
            &database
                .search(&query)
                .expect("rebuilt search must succeed")
        ),
        vec!["fragment-1"]
    );
}

#[test]
fn version_three_facts_upgrade_and_rebuild_before_the_database_opens() {
    let synthetic = SyntheticDatabase::new("v3-recall-upgrade");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(
        &mut database,
        SourceSpec {
            namespace_id: "namespace-1",
            source_id: "source-1",
            fragment_id: "fragment-1",
            content: "Existing version three source facts.\n",
            captured_at: "2026-08-01T08:00:00Z",
            sensitivity: Sensitivity::Personal,
        },
    );
    drop(database);

    let raw = Connection::open(synthetic.path()).expect("raw database must open");
    restore_version_three_schema(&raw);
    drop(raw);

    let upgraded = SqliteDatabase::open(synthetic.path())
        .expect("version three facts must migrate and rebuild atomically");
    let hits = upgraded
        .search(&request(
            "namespace-1",
            "existing version three source facts",
            "2026-08-02T00:00:00Z",
            vec![Sensitivity::Personal],
        ))
        .expect("rebuilt upgraded index must be searchable");
    assert_eq!(hit_ids(&hits), vec!["fragment-1"]);
}

#[test]
fn invalid_version_three_facts_roll_back_the_entire_recall_migration() {
    let synthetic = SyntheticDatabase::new("v3-recall-rollback");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(
        &mut database,
        SourceSpec {
            namespace_id: "namespace-1",
            source_id: "source-1",
            fragment_id: "fragment-1",
            content: "Version three integrity source.\n",
            captured_at: "2026-08-01T08:00:00Z",
            sensitivity: Sensitivity::Personal,
        },
    );
    drop(database);

    let raw = Connection::open(synthetic.path()).expect("raw database must open");
    restore_version_three_schema(&raw);
    raw.execute(
        "UPDATE radishmemory_source_bodies SET content = ?1 WHERE source_id = ?2",
        params![b"Tampered version three source.\n".as_slice(), "source-1"],
    )
    .expect("synthetic canonical fact must be tampered");
    drop(raw);

    let error = SqliteDatabase::open(synthetic.path())
        .expect_err("invalid version three facts must fail the v4 rebuild");
    assert_eq!(error.code(), SqliteErrorCode::InvalidStoredData);
    let raw = Connection::open(synthetic.path()).expect("failed migration must remain readable");
    let version: u32 = raw
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("old schema version must remain readable");
    let v4_table_count: i64 = raw
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'table' AND name IN (
                 'radishmemory_recall_fts', 'radishmemory_memory_current_projection'
             )",
            [],
            |row| row.get(0),
        )
        .expect("rolled-back schema must remain queryable");
    assert_eq!(version, 3);
    assert_eq!(v4_table_count, 0);
}

fn restore_version_three_schema(connection: &Connection) {
    connection
        .execute_batch(
            "DROP TABLE radishmemory_source_capture_audit;
             DROP TABLE radishmemory_source_origin_bindings;
             DROP TABLE radishmemory_source_lineage_tips;
             DROP TABLE radishmemory_deletion_evidence;
             DROP TABLE radishmemory_deletion_execution_results;
             DROP TABLE radishmemory_deletion_execution_attempts;
             DROP TABLE radishmemory_delete_execution_closure;
             DROP TABLE radishmemory_delete_component_targets;
             DROP TABLE radishmemory_delete_request_components;
             DROP TABLE radishmemory_delete_request_targets;
             DROP TABLE radishmemory_delete_requests;
             DROP TABLE radishmemory_recall_fts;
             DROP TABLE radishmemory_memory_current_projection;
             DELETE FROM radishmemory_schema_migrations WHERE version >= 4;
             PRAGMA user_version = 3;",
        )
        .expect("synthetic version three database must be restored");
}
