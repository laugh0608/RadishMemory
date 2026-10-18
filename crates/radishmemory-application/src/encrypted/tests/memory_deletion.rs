use super::*;

fn seed_memory(fixture: &Fixture) {
    use radishmemory_core::SourceVault;
    use radishmemory_core::{
        ActorRef, ActorType, Decision, EvidenceRef, EvidenceType, MemoryDecision,
        MemoryDecisionParams, MemoryEventType, MemoryProposal, MemoryProposalParams, MemoryRecord,
        MemoryRecordParams, MemoryState, MemoryStateEvent, MemoryStateEventParams, MemoryStore,
        MemoryType, MemoryValue, ProposalOperation, TimePrecision, UnitInterval, ValidTime,
        ValidTimeMode,
    };
    let mut db =
        radishmemory_sqlite::SqliteDatabase::open(fixture.root.0.join("library.sqlite3")).unwrap();
    let source = db
        .load_source_artifact(&id(NS), fixture.first.source_id())
        .unwrap()
        .unwrap();
    let fragments = db
        .load_source_fragments(&id(NS), fixture.first.source_id())
        .unwrap()
        .unwrap();
    let p = source.params();
    let actor = ActorRef::new(ActorType::TestFixture, id("actor-synthetic"), None);
    let proposal = MemoryProposal::new(MemoryProposalParams {
        proposal_id: id("proposal-synthetic"),
        namespace_id: id(NS),
        operation: ProposalOperation::Create,
        memory_type: MemoryType::Preference,
        subject_ref: id("subject-synthetic"),
        proposed_content: MemoryValue::from_text(text("Synthetic confirmed preference")),
        source_fragment_refs: vec![fragments[0].params().fragment_id.clone()],
        target_memory_ids: vec![],
        observed_at: p.observed_at.clone(),
        valid_time: ValidTime::new(
            ValidTimeMode::OpenEnded,
            Some(p.observed_at.clone()),
            None,
            TimePrecision::Exact,
        )
        .unwrap(),
        confidence: UnitInterval::new(0.9).unwrap(),
        importance: UnitInterval::new(0.7).unwrap(),
        governance: p.governance.clone(),
        producer: p.producer.clone(),
        reason_code: text("synthetic-proposal"),
        proposed_at: p.created_at.clone(),
    })
    .unwrap();
    let decision = MemoryDecision::new(MemoryDecisionParams {
        decision_id: id("decision-synthetic"),
        namespace_id: id(NS),
        proposal_id: proposal.params().proposal_id.clone(),
        previous_decision_id: None,
        decision: Decision::Accept,
        decided_by: actor.clone(),
        authorization_basis: text("explicit-synthetic-acceptance"),
        reason_code: text("synthetic-accept"),
        reason_text: None,
        result_memory_id: Some(id("memory-synthetic")),
        decided_at: p.created_at.clone(),
    })
    .unwrap();
    let value = proposal.params();
    let memory = MemoryRecord::new(MemoryRecordParams {
        memory_id: id("memory-synthetic"),
        lineage_id: id("memory-lineage-synthetic"),
        version: Version::new(1).unwrap(),
        namespace_id: id(NS),
        memory_type: value.memory_type,
        subject_ref: value.subject_ref.clone(),
        content: value.proposed_content.clone(),
        source_fragment_refs: value.source_fragment_refs.clone(),
        origin_proposal_id: value.proposal_id.clone(),
        accepted_by_decision_id: decision.params().decision_id.clone(),
        observed_at: value.observed_at.clone(),
        valid_time: value.valid_time.clone(),
        confidence: value.confidence,
        importance: value.importance,
        governance: value.governance.clone(),
        current_state: MemoryState::Confirmed,
        last_state_event_id: id("event-synthetic"),
        supersedes_memory_ids: vec![],
        contradicts_memory_ids: vec![],
        content_digest: value.proposed_content.content_digest().clone(),
        created_at: p.created_at.clone(),
    })
    .unwrap();
    let event = MemoryStateEvent::new(MemoryStateEventParams {
        event_id: id("event-synthetic"),
        namespace_id: id(NS),
        memory_id: memory.params().memory_id.clone(),
        previous_event_id: None,
        event_type: MemoryEventType::Confirmed,
        from_state: None,
        cause_ref: EvidenceRef::new(
            EvidenceType::MemoryDecision,
            decision.params().decision_id.clone(),
        ),
        related_memory_ids: vec![],
        actor,
        reason_code: text("synthetic-confirmed"),
        effective_at: None,
        occurred_at: p.created_at.clone(),
    })
    .unwrap();
    db.store_memory_proposal(&proposal).unwrap();
    db.store_memory_decision(&decision).unwrap();
    db.materialize_accepted_memory(&memory, &event, &[])
        .unwrap();
}

#[test]
fn lineage_deletion_includes_confirmed_memory_and_all_versions_but_keeps_other_lineages() {
    let fixture = Fixture::legacy();
    seed_memory(&fixture);
    fixture.location().initialize_key().unwrap();
    fixture.location().migrate_bodies().unwrap();
    let mut library = fixture
        .location()
        .open(Runtime {
            next: fixture.runtime.next,
            fail_clock: false,
        })
        .unwrap();
    let input = fixture.root.0.join("synthetic.md");
    fs::write(&input, UPDATED).unwrap();
    let read = FileReadRequest::new(&input, vec![fixture.root.0.clone()]).unwrap();
    let update = library
        .prepare_update_source(fixture.first.lineage_id(), &read)
        .unwrap();
    library.capture_source(&update).unwrap();
    let other = library.prepare_import_new_source(&read).unwrap();
    let other = library.capture_source(&other).unwrap();
    let request = library
        .prepare_source_lineage_deletion(fixture.first.lineage_id())
        .unwrap();
    assert_eq!(request.params().target_refs.len(), 3);
    assert!(request.params().target_refs.contains(&ObjectRef::new(
        CanonicalObjectType::MemoryRecord,
        id("memory-synthetic")
    )));
    let evidence = library.execute_source_lineage_deletion(&request).unwrap();
    assert!(
        evidence
            .params()
            .component_results
            .iter()
            .all(|result| result.params().status == ComponentStatus::Succeeded)
    );
    assert_eq!(
        fixture.scalar(
            "SELECT count(*) FROM radishmemory_memory_records WHERE deletion_state='active'"
        ),
        0
    );
    assert_eq!(
        fixture.scalar("SELECT count(*) FROM radishmemory_memory_current_projection"),
        0
    );
    assert!(
        library
            .list_source_versions(fixture.first.lineage_id())
            .unwrap()
            .is_empty()
    );
    assert!(library.get_source(other.source_id()).unwrap().is_some());
    assert_eq!(
        library.rebuild_recall().unwrap().committed_objects_verified,
        1
    );
    assert_eq!(library.list_sources(0, 20).unwrap().len(), 1);
}
