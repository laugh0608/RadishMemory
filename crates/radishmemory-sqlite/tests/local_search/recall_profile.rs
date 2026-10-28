use super::*;

#[test]
fn chinese_memory_recall_keeps_governance_and_survives_format_round_trips() {
    use radishmemory_sqlite::RecallProfile;
    let synthetic = SyntheticDatabase::new("chinese-memory-governance");
    let mut database = SqliteDatabase::open(synthetic.path()).unwrap();
    seed_source(
        &mut database,
        SourceSpec {
            namespace_id: "namespace-1",
            source_id: "source-blue",
            fragment_id: "fragment-blue",
            content: "项目默认使用蓝色主题。",
            captured_at: "2026-08-01T08:00:00Z",
            sensitivity: Sensitivity::Personal,
        },
    );
    let proposal = proposal(ProposalSpec {
        proposal_id: "proposal-blue",
        namespace_id: "namespace-1",
        fragment_id: "fragment-blue",
        operation: ProposalOperation::Create,
        target_memory_ids: vec![],
        content: "项目默认使用蓝色主题。",
        valid_from: "2026-08-01T08:00:00Z",
    });
    database.store_memory_proposal(&proposal).unwrap();
    let query = request(
        "namespace-1",
        "蓝色",
        "2026-08-02T00:00:00Z",
        vec![Sensitivity::Personal],
    );
    assert_eq!(
        hit_ids(&database.search(&query).unwrap()),
        ["fragment-blue"]
    );
    let decision = decision(
        "decision-blue",
        "namespace-1",
        "proposal-blue",
        "memory-blue",
        "2026-08-01T08:00:01Z",
    );
    let record = record(RecordSpec {
        memory_id: "memory-blue",
        lineage_id: "lineage-blue",
        version: 1,
        proposal: &proposal,
        decision: &decision,
        initial_event_id: "event-blue",
        created_at: "2026-08-01T08:00:02Z",
    });
    database.store_memory_decision(&decision).unwrap();
    database
        .materialize_accepted_memory(
            &record,
            &initial_event(
                "event-blue",
                "namespace-1",
                "memory-blue",
                "decision-blue",
                "2026-08-01T08:00:02Z",
            ),
            &[],
        )
        .unwrap();
    for _ in 0..2 {
        let hits = database.search(&query).unwrap();
        let ids = hit_ids(&hits);
        assert!(ids.contains(&"memory-blue") && ids.contains(&"fragment-blue"));
        assert_eq!(ids.len(), 2);
        for (namespace, time, sensitivity) in [
            ("namespace-2", "2026-08-02T00:00:00Z", Sensitivity::Personal),
            ("namespace-1", "2026-08-01T07:00:00Z", Sensitivity::Personal),
            (
                "namespace-1",
                "2026-08-02T00:00:00Z",
                Sensitivity::Restricted,
            ),
        ] {
            assert!(
                database
                    .search(&request(namespace, "蓝色", time, vec![sensitivity]))
                    .unwrap()
                    .is_empty()
            );
        }
        database
            .change_recall_profile(RecallProfile::LegacyUnicode61)
            .unwrap();
        assert!(database.search(&query).unwrap().is_empty());
        database
            .change_recall_profile(RecallProfile::HanPhraseV1)
            .unwrap();
    }
    database
        .append_memory_state_event(&terminal_event(TerminalEventSpec {
            event_id: "event-retract",
            namespace_id: "namespace-1",
            memory_id: "memory-blue",
            previous_event_id: "event-blue",
            event_type: MemoryEventType::Retracted,
            cause_ref: EvidenceRef::new(EvidenceType::PolicyBasis, id("policy-local-only")),
            related_memory_ids: vec![],
            effective_at: "2026-08-03T00:00:00Z",
        }))
        .unwrap();
    database
        .change_recall_profile(RecallProfile::LegacyUnicode61)
        .unwrap();
    database
        .change_recall_profile(RecallProfile::HanPhraseV1)
        .unwrap();
    assert_eq!(
        hit_ids(
            &database
                .search(&request(
                    "namespace-1",
                    "蓝色",
                    "2026-08-04T00:00:00Z",
                    vec![Sensitivity::Personal]
                ))
                .unwrap()
        ),
        ["fragment-blue"]
    );
}
