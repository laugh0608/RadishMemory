use radishmemory_core::{
    ActorRef, ActorType, Decision, DeletionState, EgressPolicy, EvidenceRef, EvidenceType,
    Governance, Identifier, MediaType, MemoryDecision, MemoryDecisionParams, MemoryEventType,
    MemoryProposal, MemoryProposalParams, MemoryRecord, MemoryRecordParams, MemoryState,
    MemoryStateEvent, MemoryStateEventParams, MemoryStore, MemoryType, MemoryValue, NonEmptyText,
    ProducerRef, ProducerType, ProposalOperation, RetentionMode, RetentionRule, Sensitivity,
    SourceArtifact, SourceArtifactParams, SourceFragment, SourceFragmentParams, SourceKind,
    SourceOriginKind, SourceVault, TimePrecision, Timestamp, UnitInterval, ValidTime,
    ValidTimeMode, Version, compute_exact_bytes_digest,
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

fn source_and_fragment(
    source_id: &str,
    fragment_id: &str,
    content_value: &str,
) -> (SourceArtifact, SourceFragment) {
    let content = text(content_value);
    let source = SourceArtifact::new(SourceArtifactParams {
        source_id: id(source_id),
        lineage_id: id(&format!("{source_id}-lineage")),
        version: Version::new(1).expect("version must be positive"),
        namespace_id: id("namespace-1"),
        source_kind: SourceKind::Text,
        media_type: MediaType::TextPlain,
        content_length: u64::try_from(content.utf8_len()).expect("length must fit u64"),
        content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
        content: content.clone(),
        title: Some(text("Synthetic memory source")),
        origin_kind: SourceOriginKind::SyntheticFixture,
        origin_ref: None,
        observed_at: timestamp("2026-08-23T08:00:00Z"),
        captured_at: timestamp("2026-08-23T08:00:01Z"),
        supersedes_source_ids: vec![],
        governance: governance(),
        producer: producer(),
        created_at: timestamp("2026-08-23T08:00:01Z"),
    })
    .expect("synthetic source must be valid");
    let fragment = SourceFragment::new(SourceFragmentParams {
        fragment_id: id(fragment_id),
        namespace_id: id("namespace-1"),
        source_id: id(source_id),
        ordinal: 0,
        byte_start: 0,
        byte_end: u64::try_from(content.utf8_len()).expect("length must fit u64"),
        heading_path: None,
        content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
        content,
        segmenter: producer(),
        governance: governance(),
        created_at: timestamp("2026-08-23T08:00:02Z"),
    })
    .expect("synthetic fragment must be valid");
    (source, fragment)
}

fn seed_source(database: &mut SqliteDatabase, source_id: &str, fragment_id: &str, content: &str) {
    let (source, fragment) = source_and_fragment(source_id, fragment_id, content);
    database
        .store_source_artifact(&source)
        .expect("source must persist");
    database
        .store_source_fragments(&[fragment])
        .expect("fragment must persist");
}

struct ProposalSpec<'a> {
    proposal_id: &'a str,
    operation: ProposalOperation,
    fragment_id: &'a str,
    target_memory_ids: Vec<Identifier>,
    content: &'a str,
    valid_from: &'a str,
}

fn proposal(spec: ProposalSpec<'_>) -> MemoryProposal {
    MemoryProposal::new(MemoryProposalParams {
        proposal_id: id(spec.proposal_id),
        namespace_id: id("namespace-1"),
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
        governance: governance(),
        producer: producer(),
        reason_code: text("fixture-memory-proposal"),
        proposed_at: timestamp("2026-08-23T08:00:03Z"),
    })
    .expect("synthetic proposal must be valid")
}

fn decision(
    decision_id: &str,
    proposal_id: &str,
    previous_decision_id: Option<&str>,
    value: Decision,
    result_memory_id: Option<&str>,
) -> MemoryDecision {
    MemoryDecision::new(MemoryDecisionParams {
        decision_id: id(decision_id),
        namespace_id: id("namespace-1"),
        proposal_id: id(proposal_id),
        previous_decision_id: previous_decision_id.map(id),
        decision: value,
        decided_by: actor(),
        authorization_basis: text("explicit-synthetic-authorization"),
        reason_code: text("fixture-memory-decision"),
        reason_text: Some(text("Synthetic decision reason")),
        result_memory_id: result_memory_id.map(id),
        decided_at: timestamp("2026-08-23T08:00:04Z"),
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
        created_at: timestamp("2026-08-23T08:00:05Z"),
    })
    .expect("synthetic record must be valid")
}

fn initial_event(event_id: &str, memory_id: &str, decision_id: &str) -> MemoryStateEvent {
    MemoryStateEvent::new(MemoryStateEventParams {
        event_id: id(event_id),
        namespace_id: id("namespace-1"),
        memory_id: id(memory_id),
        previous_event_id: None,
        event_type: MemoryEventType::Confirmed,
        from_state: None,
        cause_ref: EvidenceRef::new(EvidenceType::MemoryDecision, id(decision_id)),
        related_memory_ids: vec![],
        actor: actor(),
        reason_code: text("fixture-confirmed"),
        effective_at: None,
        occurred_at: timestamp("2026-08-23T08:00:05Z"),
    })
    .expect("synthetic initial event must be valid")
}

struct TerminalEventSpec<'a> {
    event_id: &'a str,
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
        namespace_id: id("namespace-1"),
        memory_id: id(spec.memory_id),
        previous_event_id: Some(id(spec.previous_event_id)),
        event_type: spec.event_type,
        from_state: Some(MemoryState::Confirmed),
        cause_ref: spec.cause_ref,
        related_memory_ids: spec.related_memory_ids,
        actor: actor(),
        reason_code: text("fixture-terminal-event"),
        effective_at: Some(timestamp(spec.effective_at)),
        occurred_at: timestamp("2026-08-23T09:00:01Z"),
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

#[test]
fn proposal_round_trip_resolves_sources_and_suppresses_semantic_duplicates() {
    let synthetic = SyntheticDatabase::new("proposal-round-trip");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(
        &mut database,
        "source-1",
        "fragment-1",
        "Synthetic private preference.\n",
    );
    let expected = proposal(ProposalSpec {
        proposal_id: "proposal-1",
        operation: ProposalOperation::Create,
        fragment_id: "fragment-1",
        target_memory_ids: vec![],
        content: "User prefers synthetic concise output.",
        valid_from: "2026-08-23T08:00:00Z",
    });
    database
        .store_memory_proposal(&expected)
        .expect("proposal must persist");

    let loaded = database
        .load_memory_proposal(&id("namespace-1"), &id("proposal-1"))
        .expect("proposal lookup must succeed")
        .expect("proposal must exist");
    assert_eq!(loaded, expected);
    assert!(
        database
            .load_memory_proposal(&id("namespace-other"), &id("proposal-1"))
            .expect("wrong namespace must remain safe")
            .is_none()
    );

    let duplicate = proposal(ProposalSpec {
        proposal_id: "proposal-duplicate",
        operation: ProposalOperation::Create,
        fragment_id: "fragment-1",
        target_memory_ids: vec![],
        content: "User prefers synthetic concise output.",
        valid_from: "2026-08-24T08:00:00Z",
    });
    let error = database
        .store_memory_proposal(&duplicate)
        .expect_err("same semantic evidence must be deduplicated");
    assert_eq!(error.code(), SqliteErrorCode::Conflict);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::DuplicateProposal)
    );
    assert!(!format!("{error:?}").contains("concise output"));
}

#[test]
fn decisions_form_an_unbranched_terminal_chain_before_materialization() {
    let synthetic = SyntheticDatabase::new("decision-chain");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(
        &mut database,
        "source-1",
        "fragment-1",
        "Decision source.\n",
    );
    let proposal = proposal(ProposalSpec {
        proposal_id: "proposal-1",
        operation: ProposalOperation::Create,
        fragment_id: "fragment-1",
        target_memory_ids: vec![],
        content: "Synthetic decision memory.",
        valid_from: "2026-08-23T08:00:00Z",
    });
    database
        .store_memory_proposal(&proposal)
        .expect("proposal must persist");
    let deferred = decision("decision-defer", "proposal-1", None, Decision::Defer, None);
    let accepted = decision(
        "decision-accept",
        "proposal-1",
        Some("decision-defer"),
        Decision::Accept,
        Some("memory-1"),
    );
    database
        .store_memory_decision(&deferred)
        .expect("defer decision must persist");
    database
        .store_memory_decision(&accepted)
        .expect("accept decision must extend defer");
    assert_eq!(
        database
            .load_memory_decision(&id("namespace-1"), &id("decision-accept"))
            .expect("decision lookup must succeed"),
        Some(accepted.clone())
    );
    assert!(
        database
            .load_memory_record(&id("namespace-1"), &id("memory-1"))
            .expect("record lookup must succeed")
            .is_none(),
        "accept decision remains a separate persisted event"
    );

    let after_terminal = decision(
        "decision-after-terminal",
        "proposal-1",
        Some("decision-accept"),
        Decision::Reject,
        None,
    );
    let error = database
        .store_memory_decision(&after_terminal)
        .expect_err("terminal decision must not accept another child");
    assert_eq!(error.code(), SqliteErrorCode::MemoryInvariant);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::TerminalDecision)
    );

    assert!(
        database
            .load_memory_decision(&id("namespace-other"), &id("decision-accept"))
            .expect("wrong namespace must remain safe")
            .is_none()
    );
    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    raw.pragma_update(None, "foreign_keys", false)
        .expect("synthetic tamper connection must disable foreign keys");
    raw.execute(
        "UPDATE radishmemory_memory_decisions
         SET previous_decision_id = ?1 WHERE decision_id = ?2",
        params!["decision-missing", "decision-accept"],
    )
    .expect("synthetic decision chain must be tampered for the test");
    drop(raw);
    let error = database
        .load_memory_decision(&id("namespace-1"), &id("decision-accept"))
        .expect_err("stored decision chain drift must fail closed");
    assert_eq!(error.code(), SqliteErrorCode::InvalidStoredData);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::StoredIntegrityMismatch)
    );
}

#[test]
fn accepted_memory_and_initial_event_are_atomic_and_projection_is_event_derived() {
    let synthetic = SyntheticDatabase::new("accepted-memory");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(
        &mut database,
        "source-1",
        "fragment-1",
        "Accepted source.\n",
    );
    let proposal = proposal(ProposalSpec {
        proposal_id: "proposal-1",
        operation: ProposalOperation::Create,
        fragment_id: "fragment-1",
        target_memory_ids: vec![],
        content: "Private accepted synthetic memory.",
        valid_from: "2026-08-23T08:00:00Z",
    });
    let decision = decision(
        "decision-1",
        "proposal-1",
        None,
        Decision::Accept,
        Some("memory-1"),
    );
    let record = record(RecordSpec {
        memory_id: "memory-1",
        lineage_id: "memory-lineage-1",
        version: 1,
        proposal: &proposal,
        decision: &decision,
        initial_event_id: "event-initial",
    });
    let initial = initial_event("event-initial", "memory-1", "decision-1");
    persist_confirmed(&mut database, &proposal, &decision, &record, &initial);

    assert_eq!(
        database
            .load_memory_record(&id("namespace-1"), &id("memory-1"))
            .expect("record lookup must succeed"),
        Some(record.clone())
    );
    assert_eq!(
        database
            .load_memory_state_events(&id("namespace-1"), &id("memory-1"))
            .expect("event lookup must succeed"),
        Some(vec![initial.clone()])
    );
    assert!(
        database
            .load_memory_record(&id("namespace-other"), &id("memory-1"))
            .expect("wrong namespace must remain safe")
            .is_none()
    );

    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    let columns = raw
        .prepare("SELECT name FROM pragma_table_info('radishmemory_memory_records')")
        .expect("record schema must be queryable")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("record columns must be queryable")
        .collect::<Result<Vec<_>, _>>()
        .expect("record columns must decode");
    assert!(!columns.iter().any(|column| column == "current_state"));
    assert!(!columns.iter().any(|column| column == "last_state_event_id"));
    drop(raw);

    let bypassed_supersede = terminal_event(TerminalEventSpec {
        event_id: "event-bypassed-supersede",
        memory_id: "memory-1",
        previous_event_id: "event-initial",
        event_type: MemoryEventType::Superseded,
        cause_ref: EvidenceRef::new(EvidenceType::MemoryRecord, id("memory-other")),
        related_memory_ids: vec![id("memory-other")],
        effective_at: "2026-08-23T08:30:00Z",
    });
    let error = database
        .append_memory_state_event(&bypassed_supersede)
        .expect_err("supersede must stay inside materialization transaction");
    assert_eq!(error.code(), SqliteErrorCode::MemoryInvariant);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::Materialization)
    );

    let retracted = terminal_event(TerminalEventSpec {
        event_id: "event-retracted",
        memory_id: "memory-1",
        previous_event_id: "event-initial",
        event_type: MemoryEventType::Retracted,
        cause_ref: EvidenceRef::new(EvidenceType::PolicyBasis, id("policy-retract")),
        related_memory_ids: vec![],
        effective_at: "2026-08-23T09:00:00Z",
    });
    database
        .append_memory_state_event(&retracted)
        .expect("terminal event must append");
    let loaded = database
        .load_memory_record(&id("namespace-1"), &id("memory-1"))
        .expect("record lookup must succeed")
        .expect("record must exist");
    assert_eq!(loaded.params().current_state, MemoryState::Retracted);
    assert_eq!(loaded.params().last_state_event_id, id("event-retracted"));
    assert_eq!(loaded.params().content, record.params().content);

    let branch = terminal_event(TerminalEventSpec {
        event_id: "event-branch",
        memory_id: "memory-1",
        previous_event_id: "event-initial",
        event_type: MemoryEventType::Expired,
        cause_ref: EvidenceRef::new(EvidenceType::PolicyBasis, id("policy-expire")),
        related_memory_ids: vec![],
        effective_at: "2026-08-23T09:30:00Z",
    });
    let error = database
        .append_memory_state_event(&branch)
        .expect_err("event branch must fail closed");
    assert_eq!(error.code(), SqliteErrorCode::MemoryInvariant);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::EventChain)
    );
}

#[test]
fn supersede_materialization_closes_old_record_without_mutating_its_fact_row() {
    let synthetic = SyntheticDatabase::new("memory-supersede");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(
        &mut database,
        "source-1",
        "fragment-1",
        "Old value source.\n",
    );
    let old_proposal = proposal(ProposalSpec {
        proposal_id: "proposal-old",
        operation: ProposalOperation::Create,
        fragment_id: "fragment-1",
        target_memory_ids: vec![],
        content: "Synthetic value is blue.",
        valid_from: "2026-02-01T08:00:00Z",
    });
    let old_decision = decision(
        "decision-old",
        "proposal-old",
        None,
        Decision::Accept,
        Some("memory-old"),
    );
    let old_record = record(RecordSpec {
        memory_id: "memory-old",
        lineage_id: "memory-lineage",
        version: 1,
        proposal: &old_proposal,
        decision: &old_decision,
        initial_event_id: "event-old-initial",
    });
    let old_initial = initial_event("event-old-initial", "memory-old", "decision-old");
    persist_confirmed(
        &mut database,
        &old_proposal,
        &old_decision,
        &old_record,
        &old_initial,
    );
    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    let old_fact_before: (String, String) = raw
        .query_row(
            "SELECT content_text, content_digest_value FROM radishmemory_memory_records WHERE memory_id = ?1",
            params!["memory-old"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("old immutable fact must be queryable");
    drop(raw);

    seed_source(
        &mut database,
        "source-2",
        "fragment-2",
        "New value source.\n",
    );
    let new_proposal = proposal(ProposalSpec {
        proposal_id: "proposal-new",
        operation: ProposalOperation::Supersede,
        fragment_id: "fragment-2",
        target_memory_ids: vec![id("memory-old")],
        content: "Synthetic value is green.",
        valid_from: "2026-03-20T10:00:00Z",
    });
    let new_decision = decision(
        "decision-new",
        "proposal-new",
        None,
        Decision::Accept,
        Some("memory-new"),
    );
    database
        .store_memory_proposal(&new_proposal)
        .expect("supersede proposal must persist");
    database
        .store_memory_decision(&new_decision)
        .expect("supersede decision must persist");
    let new_record = record(RecordSpec {
        memory_id: "memory-new",
        lineage_id: "memory-lineage",
        version: 2,
        proposal: &new_proposal,
        decision: &new_decision,
        initial_event_id: "event-new-initial",
    });
    let new_initial = initial_event("event-new-initial", "memory-new", "decision-new");
    let old_superseded = terminal_event(TerminalEventSpec {
        event_id: "event-old-superseded",
        memory_id: "memory-old",
        previous_event_id: "event-old-initial",
        event_type: MemoryEventType::Superseded,
        cause_ref: EvidenceRef::new(EvidenceType::MemoryRecord, id("memory-new")),
        related_memory_ids: vec![id("memory-new")],
        effective_at: "2026-03-20T10:00:00Z",
    });
    database
        .materialize_accepted_memory(
            &new_record,
            &new_initial,
            std::slice::from_ref(&old_superseded),
        )
        .expect("supersede closure must commit atomically");

    let loaded_old = database
        .load_memory_record(&id("namespace-1"), &id("memory-old"))
        .expect("old record lookup must succeed")
        .expect("old record must exist");
    assert_eq!(loaded_old.params().current_state, MemoryState::Superseded);
    assert_eq!(
        loaded_old.params().last_state_event_id,
        id("event-old-superseded")
    );
    assert_eq!(
        database
            .load_memory_record(&id("namespace-1"), &id("memory-new"))
            .expect("new record lookup must succeed"),
        Some(new_record)
    );
    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    let old_fact_after: (String, String) = raw
        .query_row(
            "SELECT content_text, content_digest_value FROM radishmemory_memory_records WHERE memory_id = ?1",
            params!["memory-old"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("old immutable fact must remain queryable");
    assert_eq!(old_fact_after, old_fact_before);
    raw.execute(
        "UPDATE radishmemory_memory_records SET lineage_id = ?1 WHERE memory_id = ?2",
        params!["tampered-lineage", "memory-new"],
    )
    .expect("synthetic supersession cause must be tampered for the test");
    drop(raw);
    let error = database
        .load_memory_record(&id("namespace-1"), &id("memory-old"))
        .expect_err("old record must revalidate its stored supersession cause");
    assert_eq!(error.code(), SqliteErrorCode::InvalidStoredData);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::StoredIntegrityMismatch)
    );
}

#[test]
fn invalid_supersede_closure_rolls_back_new_record_and_old_event() {
    let synthetic = SyntheticDatabase::new("memory-rollback");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(
        &mut database,
        "source-1",
        "fragment-1",
        "Old rollback source.\n",
    );
    let old_proposal = proposal(ProposalSpec {
        proposal_id: "proposal-old",
        operation: ProposalOperation::Create,
        fragment_id: "fragment-1",
        target_memory_ids: vec![],
        content: "Rollback value is old.",
        valid_from: "2026-02-01T08:00:00Z",
    });
    let old_decision = decision(
        "decision-old",
        "proposal-old",
        None,
        Decision::Accept,
        Some("memory-old"),
    );
    let old_record = record(RecordSpec {
        memory_id: "memory-old",
        lineage_id: "rollback-lineage",
        version: 1,
        proposal: &old_proposal,
        decision: &old_decision,
        initial_event_id: "event-old-initial",
    });
    let old_initial = initial_event("event-old-initial", "memory-old", "decision-old");
    persist_confirmed(
        &mut database,
        &old_proposal,
        &old_decision,
        &old_record,
        &old_initial,
    );

    seed_source(
        &mut database,
        "source-2",
        "fragment-2",
        "New rollback source.\n",
    );
    let new_proposal = proposal(ProposalSpec {
        proposal_id: "proposal-new",
        operation: ProposalOperation::Supersede,
        fragment_id: "fragment-2",
        target_memory_ids: vec![id("memory-old")],
        content: "Rollback value is new.",
        valid_from: "2026-03-20T10:00:00Z",
    });
    let new_decision = decision(
        "decision-new",
        "proposal-new",
        None,
        Decision::Accept,
        Some("memory-new"),
    );
    database
        .store_memory_proposal(&new_proposal)
        .expect("new proposal must persist");
    database
        .store_memory_decision(&new_decision)
        .expect("new decision must persist");
    let new_record = record(RecordSpec {
        memory_id: "memory-new",
        lineage_id: "rollback-lineage",
        version: 2,
        proposal: &new_proposal,
        decision: &new_decision,
        initial_event_id: "event-new-initial",
    });
    let new_initial = initial_event("event-new-initial", "memory-new", "decision-new");
    let wrong_time = terminal_event(TerminalEventSpec {
        event_id: "event-old-superseded",
        memory_id: "memory-old",
        previous_event_id: "event-old-initial",
        event_type: MemoryEventType::Superseded,
        cause_ref: EvidenceRef::new(EvidenceType::MemoryRecord, id("memory-new")),
        related_memory_ids: vec![id("memory-new")],
        effective_at: "2026-03-20T10:30:00Z",
    });
    let error = database
        .materialize_accepted_memory(&new_record, &new_initial, &[wrong_time])
        .expect_err("misaligned supersede boundary must fail");
    assert_eq!(error.code(), SqliteErrorCode::MemoryInvariant);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::Materialization)
    );
    assert!(
        database
            .load_memory_record(&id("namespace-1"), &id("memory-new"))
            .expect("new record lookup must succeed")
            .is_none()
    );
    assert_eq!(
        database
            .load_memory_record(&id("namespace-1"), &id("memory-old"))
            .expect("old record lookup must succeed")
            .expect("old record must remain")
            .params()
            .current_state,
        MemoryState::Confirmed
    );
}

#[test]
fn event_conflict_after_record_insert_rolls_back_the_materialization_transaction() {
    let synthetic = SyntheticDatabase::new("materialization-conflict");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(
        &mut database,
        "source-1",
        "fragment-1",
        "First atomic source.\n",
    );
    let first_proposal = proposal(ProposalSpec {
        proposal_id: "proposal-first",
        operation: ProposalOperation::Create,
        fragment_id: "fragment-1",
        target_memory_ids: vec![],
        content: "First atomic memory.",
        valid_from: "2026-08-23T08:00:00Z",
    });
    let first_decision = decision(
        "decision-first",
        "proposal-first",
        None,
        Decision::Accept,
        Some("memory-first"),
    );
    let first_record = record(RecordSpec {
        memory_id: "memory-first",
        lineage_id: "lineage-first",
        version: 1,
        proposal: &first_proposal,
        decision: &first_decision,
        initial_event_id: "event-shared",
    });
    let first_event = initial_event("event-shared", "memory-first", "decision-first");
    persist_confirmed(
        &mut database,
        &first_proposal,
        &first_decision,
        &first_record,
        &first_event,
    );

    seed_source(
        &mut database,
        "source-2",
        "fragment-2",
        "Second atomic source.\n",
    );
    let second_proposal = proposal(ProposalSpec {
        proposal_id: "proposal-second",
        operation: ProposalOperation::Create,
        fragment_id: "fragment-2",
        target_memory_ids: vec![],
        content: "Second atomic memory.",
        valid_from: "2026-08-24T08:00:00Z",
    });
    let second_decision = decision(
        "decision-second",
        "proposal-second",
        None,
        Decision::Accept,
        Some("memory-second"),
    );
    database
        .store_memory_proposal(&second_proposal)
        .expect("second proposal must persist");
    database
        .store_memory_decision(&second_decision)
        .expect("second decision must persist");
    let second_record = record(RecordSpec {
        memory_id: "memory-second",
        lineage_id: "lineage-second",
        version: 1,
        proposal: &second_proposal,
        decision: &second_decision,
        initial_event_id: "event-shared",
    });
    let conflicting_event = initial_event("event-shared", "memory-second", "decision-second");

    let error = database
        .materialize_accepted_memory(&second_record, &conflicting_event, &[])
        .expect_err("duplicate event ID must fail after record insertion");
    assert_eq!(error.code(), SqliteErrorCode::Conflict);
    assert!(
        database
            .load_memory_record(&id("namespace-1"), &id("memory-second"))
            .expect("rolled-back record lookup must succeed")
            .is_none()
    );
    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    let count: i64 = raw
        .query_row(
            "SELECT COUNT(*) FROM radishmemory_memory_records WHERE memory_id = ?1",
            params!["memory-second"],
            |row| row.get(0),
        )
        .expect("rolled-back record count must be queryable");
    assert_eq!(count, 0);
}

#[test]
fn tampered_memory_content_fails_integrity_without_echoing_text() {
    let synthetic = SyntheticDatabase::new("memory-integrity");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_source(
        &mut database,
        "source-1",
        "fragment-1",
        "Integrity source.\n",
    );
    let proposal = proposal(ProposalSpec {
        proposal_id: "proposal-1",
        operation: ProposalOperation::Create,
        fragment_id: "fragment-1",
        target_memory_ids: vec![],
        content: "Original private memory text.",
        valid_from: "2026-08-23T08:00:00Z",
    });
    let decision = decision(
        "decision-1",
        "proposal-1",
        None,
        Decision::Accept,
        Some("memory-1"),
    );
    let record = record(RecordSpec {
        memory_id: "memory-1",
        lineage_id: "memory-lineage-1",
        version: 1,
        proposal: &proposal,
        decision: &decision,
        initial_event_id: "event-initial",
    });
    let initial = initial_event("event-initial", "memory-1", "decision-1");
    persist_confirmed(&mut database, &proposal, &decision, &record, &initial);
    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    raw.execute(
        "UPDATE radishmemory_memory_records SET content_text = ?1 WHERE memory_id = ?2",
        params!["Tampered private memory text.", "memory-1"],
    )
    .expect("synthetic memory row must be tampered for the test");
    drop(raw);

    let error = database
        .load_memory_record(&id("namespace-1"), &id("memory-1"))
        .expect_err("content digest drift must fail closed");
    assert_eq!(error.code(), SqliteErrorCode::InvalidStoredData);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::StoredIntegrityMismatch)
    );
    assert!(!format!("{error:?}").contains("Tampered private"));
}
