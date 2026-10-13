use super::*;

fn assert_rejected(snapshot: Snapshot, code: SourceVaultErrorCode) {
    assert_eq!(
        snapshot
            .error
            .unwrap()
            .application_failure()
            .unwrap()
            .vault_code(),
        Some(code)
    );
    assert!(snapshot.view.is_none());
    assert!(snapshot.message.is_none());
    assert!(!snapshot.recovery_known);
    assert!(!snapshot.abandonment_available);
    assert!(snapshot.deletion_requests.is_empty());
}

#[test]
fn open_and_explicit_refresh_use_two_loads_and_preserve_selection() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    for populated in [false, true] {
        if populated {
            ok(engine.execute(Command::Import(fixture.input("First synthetic body"))));
            ok(engine.execute(Command::Import(fixture.input("Second synthetic body"))));
        }
        for command in [Command::OpenEncrypted, Command::Refresh] {
            let loads = fixture.provider.key_loads();
            let result = ok(engine.execute(command));
            assert_eq!(fixture.provider.key_loads() - loads, 2);
            assert!(result.recovery_known);
            assert_eq!(
                result.view.unwrap().sources().len(),
                if populated { 2 } else { 0 }
            );
        }
    }
    let view = ok(engine.execute(Command::Refresh)).view.unwrap();
    let lineage = view.sources().last().unwrap().lineage_id().clone();
    let loads = fixture.provider.key_loads();
    ok(engine.execute(Command::SelectLineage(lineage.clone())));
    assert_eq!(fixture.provider.key_loads() - loads, 1);
    let refreshed = ok(engine.execute(Command::Refresh)).view.unwrap();
    assert_eq!(refreshed.selected_lineage_id(), Some(&lineage));
    assert_eq!(fixture.provider.key_loads() - loads, 3);
    // Ordinary reads/maintenance stay independent; no snapshot key is retained.
    fixture
        .provider
        .fail_key_access(Some(SourceVaultErrorCode::KeyStoreDenied));
    assert!(
        engine
            .execute(Command::Search("synthetic".into()))
            .error
            .is_some()
    );
    assert!(
        engine
            .execute(Command::Export(fixture.export()))
            .error
            .is_some()
    );
    assert!(engine.execute(Command::Verify).error.is_some());
    assert!(!fixture.root.join("export/result.md").exists());
}

#[test]
fn open_and_refresh_fail_closed_at_entry_and_release_without_automatic_retry() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    ok(engine.execute(Command::Import(fixture.input("Synthetic private view"))));
    for code in [
        SourceVaultErrorCode::KeyStoreCancelled,
        SourceVaultErrorCode::KeyStoreDenied,
        SourceVaultErrorCode::KeyStoreLocked,
        SourceVaultErrorCode::KeyMissing,
        SourceVaultErrorCode::KeyAmbiguous,
        SourceVaultErrorCode::KeyMetadataMismatch,
        SourceVaultErrorCode::KeyStoreFailure,
        SourceVaultErrorCode::KeyCorrupt,
        SourceVaultErrorCode::KeyStoreUnavailable,
    ] {
        for at in [1, 2] {
            for command in [Command::OpenEncrypted, Command::Refresh] {
                ok(engine.execute(Command::OpenEncrypted));
                let loads = fixture.provider.key_loads();
                fixture.provider.fail_on_key_load_with(loads + at, code);
                assert_rejected(engine.execute(command), code);
                assert_eq!(fixture.provider.key_loads(), loads + at);
                assert_eq!(fixture.provider.key_creations(), 1);
            }
        }
    }
    let opened = ok(engine.execute(Command::OpenEncrypted));
    assert_eq!(opened.view.unwrap().sources().len(), 1);
}

#[test]
fn key_replacement_at_release_rejects_even_empty_open_and_refresh() {
    for populated in [false, true] {
        let fixture = Fixture::new();
        let mut engine = fixture.engine();
        ready(&mut engine);
        if populated {
            ok(engine.execute(Command::Import(fixture.input("Synthetic stored source"))));
        }
        for command in [Command::OpenEncrypted, Command::Refresh] {
            fixture.provider.use_wrong_key(false);
            ok(engine.execute(Command::OpenEncrypted));
            fixture
                .provider
                .replace_on_key_load(fixture.provider.key_loads() + 2);
            assert_rejected(
                engine.execute(command),
                SourceVaultErrorCode::KeyReadbackMismatch,
            );
            assert_eq!(fixture.provider.key_creations(), 1);
        }
    }
}

#[test]
fn failed_explicit_refresh_preserves_original_capture_but_invalidates_recovery() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    fixture.provider.interrupt_next_capture_after_publish();
    let pending = engine.execute(Command::Import(fixture.input("Original pending snapshot")));
    assert!(pending.holds_capture);
    let inspected = ok(engine.execute(Command::Refresh));
    assert!(inspected.abandonment_available);
    fixture.provider.fail_on_key_load_with(
        fixture.provider.key_loads() + 2,
        SourceVaultErrorCode::KeyStoreCancelled,
    );
    let failed = engine.execute(Command::Refresh);
    assert!(failed.holds_capture);
    assert_rejected(failed, SourceVaultErrorCode::KeyStoreCancelled);
    fs::remove_file(fixture.root.join("input/note.md")).unwrap();
    let retried = ok(engine.execute(Command::RetryOriginal));
    assert!(!retried.holds_original);
    assert_eq!(retried.view.unwrap().sources().len(), 1);
}

#[test]
fn failed_refresh_withdraws_discovered_deletion_without_losing_original_authority() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    ok(engine.execute(Command::Import(fixture.input("Synthetic deletion target"))));
    fixture.provider.interrupt_next_deletion_after_intent();
    assert!(engine.execute(Command::Delete).holds_original);
    let found = ok(engine.execute(Command::Refresh));
    assert_eq!(found.deletion_requests.len(), 1);
    let original_id = found.deletion_requests[0].clone();
    fixture.provider.fail_on_key_load_with(
        fixture.provider.key_loads() + 2,
        SourceVaultErrorCode::KeyStoreDenied,
    );
    let failed = engine.execute(Command::Refresh);
    assert!(failed.holds_original);
    assert_rejected(failed, SourceVaultErrorCode::KeyStoreDenied);
    assert!(
        engine
            .execute(Command::ResumeDelete(original_id.clone()))
            .error
            .is_some()
    );
    let result = ok(engine.execute(Command::RetryOriginal));
    let evidence = result.last_deletion.unwrap();
    assert_eq!(evidence.params().delete_request_id, original_id);
    assert_eq!(
        evidence.params().overall_status,
        DeletionOverallStatus::Completed
    );
}

#[test]
fn pinned_profile_failure_invalidates_recovery_before_any_key_access() {
    let fixture = crate::startup::tests::Fixture::new();
    let input = Fixture::new();
    let provider = SyntheticLibraryProvider::default();
    let mut engine = Worker::engine(
        crate::startup::Startup::parse(fixture.arguments()).unwrap(),
        || provider.clone(),
    )
    .unwrap();
    ready(&mut engine);
    provider.interrupt_next_capture_after_publish();
    assert!(
        engine
            .execute(Command::Import(
                input.input("Synthetic pending profile check")
            ))
            .holds_capture
    );
    assert!(ok(engine.execute(Command::Refresh)).abandonment_available);
    let path = fixture.root.join("host-profile-v1.txt");
    let bytes = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    let loads = provider.key_loads();
    let failed = engine.execute(Command::Refresh);
    assert_eq!(
        failed.error.unwrap().reason(),
        DesktopErrorReason::ProfileReadFailed
    );
    assert!(failed.view.is_none());
    assert!(!failed.recovery_known);
    assert!(!failed.abandonment_available);
    assert!(failed.deletion_requests.is_empty());
    assert!(failed.holds_capture);
    assert_eq!(provider.key_loads(), loads);
    fs::write(&path, bytes).unwrap();
    assert!(ok(engine.execute(Command::Refresh)).abandonment_available);
}
