use super::*;
use radishmemory_source_vault::{SourceVaultErrorCode, acceptance::SyntheticLibraryProvider};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Fixture {
    root: std::path::PathBuf,
    provider: SyntheticLibraryProvider,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "radishmemory-host-worker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("input")).unwrap();
        fs::create_dir(root.join("export")).unwrap();
        Self {
            root,
            provider: SyntheticLibraryProvider::default(),
        }
    }
    fn engine(
        &self,
    ) -> Engine<SyntheticLibraryProvider, impl Fn() -> SyntheticLibraryProvider + use<>> {
        let provider = self.provider.clone();
        Engine::new(
            ApplicationPaths::from_data_directory(self.root.join("data")).unwrap(),
            move || provider.clone(),
        )
    }
    fn input(&self, text: &str) -> FileReadRequest {
        let path = self.root.join("input/note.md");
        fs::write(&path, text).unwrap();
        FileReadRequest::new(path, vec![self.root.join("input")]).unwrap()
    }
    fn export(&self) -> FileExportRequest {
        FileExportRequest::new(
            self.root.join("export/result.md"),
            vec![self.root.join("export")],
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn ok(snapshot: Snapshot) -> Snapshot {
    assert!(snapshot.error.is_none(), "{:?}", snapshot.error);
    snapshot
}
fn ready<P: LibraryProvider, F: Fn() -> P>(engine: &mut Engine<P, F>) {
    ok(engine.execute(Command::OpenPlain));
    ok(engine.execute(Command::InitializeKey));
    ok(engine.execute(Command::Migrate));
    ok(engine.execute(Command::OpenEncrypted));
}

#[test]
fn explicit_preparation_closes_legacy_and_missing_key_never_bootstraps() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    assert!(engine.execute(Command::OpenEncrypted).error.is_some());
    assert_eq!(fixture.provider.key_creations(), 0);
    ready(&mut engine);
    assert_eq!(fixture.provider.key_creations(), 1);
    ok(engine.execute(Command::Import(fixture.input("synthetic source"))));
    fixture.provider.forget_key();
    let failed = engine.execute(Command::OpenEncrypted);
    assert!(failed.view.is_none());
    assert_eq!(
        failed
            .error
            .unwrap()
            .application_failure()
            .unwrap()
            .vault_code(),
        Some(SourceVaultErrorCode::KeyMissing)
    );
    assert!(engine.execute(Command::InitializeKey).error.is_some());
    assert_eq!(fixture.provider.key_creations(), 1);
}

#[test]
fn capture_retry_keeps_original_bytes_and_provenance_and_blocks_replacement() {
    for after_commit in [false, true] {
        let fixture = Fixture::new();
        let mut engine = fixture.engine();
        ready(&mut engine);
        if after_commit {
            fixture.provider.interrupt_next_capture_after_commit();
        } else {
            fixture.provider.interrupt_next_capture_after_publish();
        }
        let failed = engine.execute(Command::Import(fixture.input("synthetic original capture")));
        assert!(failed.error.is_some());
        assert!(failed.holds_capture);
        assert!(failed.view.is_none());
        let Library::Encrypted {
            capture: Some(original),
            ..
        } = engine.controller().unwrap().backend()
        else {
            panic!("missing original");
        };
        let source_id = original.source().params().source_id.clone();
        assert!(
            engine
                .execute(Command::Import(fixture.input("must not replace original")))
                .error
                .is_some()
        );
        assert!(engine.execute(Command::OpenPlain).error.is_some());
        fs::remove_file(fixture.root.join("input/note.md")).unwrap();
        let retried = ok(engine.execute(Command::RetryOriginal));
        assert!(!retried.holds_original);
        assert_eq!(
            retried.view.as_ref().unwrap().sources()[0].current_source_id(),
            &source_id
        );
        ok(engine.execute(Command::Export(fixture.export())));
        assert_eq!(
            fs::read_to_string(fixture.root.join("export/result.md")).unwrap(),
            "synthetic original capture"
        );
    }
}

#[test]
fn committed_capture_refresh_failure_reports_success_and_does_not_retain_retry() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    fixture
        .provider
        .fail_on_key_load(fixture.provider.key_loads() + 3);
    let snapshot = ok(engine.execute(Command::Import(
        fixture.input("committed before refresh failure"),
    )));
    assert!(snapshot.message.unwrap().contains("committed"));
    assert!(snapshot.refresh_error.is_some());
    assert!(snapshot.view.is_none());
    assert!(!snapshot.holds_original);
    assert!(engine.execute(Command::RetryOriginal).error.is_some());
    let refreshed = ok(engine.execute(Command::Refresh));
    assert_eq!(refreshed.view.unwrap().sources().len(), 1);
}

#[test]
fn restart_loses_capture_snapshot_and_requires_inspected_explicit_abandonment() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    ok(engine.execute(Command::Import(fixture.input("previous committed bytes"))));
    fixture.provider.interrupt_next_capture_after_publish();
    assert!(
        engine
            .execute(Command::Update(fixture.input("uncommitted replacement")))
            .holds_capture
    );
    drop(engine);
    let mut reopened = fixture.engine();
    let snapshot = ok(reopened.execute(Command::OpenEncrypted));
    assert!(!snapshot.holds_capture);
    assert!(snapshot.abandonment_available);
    assert!(
        reopened
            .execute(Command::Import(fixture.input("new request blocked")))
            .error
            .is_some()
    );
    assert!(reopened.execute(Command::RetryOriginal).error.is_some());
    // Merely inspecting has not removed the pending operation.
    assert!(ok(reopened.execute(Command::InspectRecovery)).abandonment_available);
    let abandoned = ok(reopened.execute(Command::Abandon));
    assert!(!abandoned.abandonment_available);
    ok(reopened.execute(Command::Refresh));
    ok(reopened.execute(Command::Export(fixture.export())));
    assert_eq!(
        fs::read_to_string(fixture.root.join("export/result.md")).unwrap(),
        "previous committed bytes"
    );
    ok(reopened.execute(Command::Update(fixture.input("explicit new replacement"))));
}

#[test]
fn restart_discovers_original_deletion_and_resumes_all_interruption_windows() {
    for phase in 0..4 {
        let fixture = Fixture::new();
        let mut engine = fixture.engine();
        ready(&mut engine);
        ok(engine.execute(Command::Import(fixture.input("synthetic deleted source"))));
        match phase {
            0 => fixture.provider.interrupt_next_deletion_after_intent(),
            1 => fixture.provider.interrupt_next_deletion_after_execution(),
            2 => fixture.provider.interrupt_next_deletion_evidence(false),
            _ => fixture.provider.interrupt_next_deletion_evidence(true),
        }
        let failed = engine.execute(Command::Delete);
        assert!(failed.error.is_some());
        assert!(failed.holds_original);
        let Library::Encrypted {
            deletion: Some(original),
            ..
        } = engine.controller().unwrap().backend()
        else {
            panic!("missing deletion authority");
        };
        let id = original.params().delete_request_id.clone();
        drop(engine);
        let mut reopened = fixture.engine();
        let snapshot = ok(reopened.execute(Command::OpenEncrypted));
        if phase == 3 {
            assert!(snapshot.deletion_requests.is_empty());
        } else {
            assert_eq!(snapshot.deletion_requests, vec![id.clone()]);
            assert!(
                reopened
                    .execute(Command::Import(fixture.input("must stay blocked")))
                    .error
                    .is_some()
            );
            let resumed = ok(reopened.execute(Command::ResumeDelete(id.clone())));
            let evidence = resumed.last_deletion.unwrap();
            assert_eq!(evidence.params().delete_request_id, id);
            assert_eq!(
                evidence.params().overall_status,
                DeletionOverallStatus::Completed
            );
            assert!(resumed.deletion_requests.is_empty());
        }
        assert!(
            ok(reopened.execute(Command::Refresh))
                .view
                .unwrap()
                .sources()
                .is_empty()
        );
        assert!(fixture.root.join("input/note.md").is_file());
    }
}

#[test]
fn provider_failure_is_bounded_and_stale_view_is_hidden_until_refresh() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    ok(engine.execute(Command::Import(fixture.input("private synthetic marker"))));
    for code in [
        SourceVaultErrorCode::KeyStoreLocked,
        SourceVaultErrorCode::KeyStoreDenied,
        SourceVaultErrorCode::KeyStoreCancelled,
        SourceVaultErrorCode::AuthenticationFailed,
    ] {
        fixture.provider.fail_key_access(Some(code));
        let snapshot = engine.execute(Command::Search("marker".into()));
        assert!(snapshot.view.is_none());
        let error = snapshot.error.unwrap();
        assert_eq!(
            error.application_failure().unwrap().vault_code(),
            Some(code)
        );
        let diagnostic = format!("{error:?}");
        assert!(!diagnostic.contains("marker"));
        assert!(!diagnostic.contains(fixture.root.to_str().unwrap()));
        assert!(!diagnostic.contains("synthetic key access"));
    }
    fixture.provider.fail_key_access(None);
    assert!(ok(engine.execute(Command::InspectRecovery)).view.is_none());
    assert!(ok(engine.execute(Command::Refresh)).view.is_some());
}

#[test]
fn real_worker_serializes_commands_and_exits_without_background_work() {
    let fixture = Fixture::new();
    let data = fixture.root.join("thread-data");
    let (entered, entry) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let caller = thread::current().id();
    let mut worker = Worker::spawn(move || {
        entered.send(thread::current().id()).unwrap();
        gate.recv().unwrap();
        let provider = SyntheticLibraryProvider::default();
        Ok(Engine::new(
            ApplicationPaths::from_data_directory(data).unwrap(),
            move || provider.clone(),
        ))
    })
    .unwrap();
    assert_ne!(
        entry
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap(),
        caller
    );
    worker.send(Command::OpenPlain).unwrap();
    assert!(worker.busy);
    assert!(worker.send(Command::InitializeKey).is_err());
    release.send(()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(snapshot) = worker.poll() {
            ok(snapshot);
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(!worker.busy);
    worker.send(Command::Shutdown).unwrap();
    worker.thread.take().unwrap().join().unwrap();
    assert!(worker.poll().unwrap().error.is_some());
    assert!(worker.stopped);
    assert!(worker.send(Command::Verify).is_err());
}

#[test]
fn failed_open_keeps_explicit_maintenance_location_and_never_repairs_implicitly() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    ok(engine.execute(Command::Import(fixture.input("maintenance source"))));
    fixture
        .provider
        .fail_key_access(Some(SourceVaultErrorCode::KeyStoreLocked));
    assert!(engine.execute(Command::OpenEncrypted).view.is_none());
    assert!(engine.execute(Command::Verify).error.is_some());
    fixture.provider.fail_key_access(None);
    let verified = ok(engine.execute(Command::Verify));
    assert!(verified.view.is_none());
    assert!(
        verified
            .message
            .unwrap()
            .contains("derivations rebuilt: false")
    );
    let rebuilt = ok(engine.execute(Command::Rebuild));
    assert!(rebuilt.view.is_none());
    assert!(
        rebuilt
            .message
            .unwrap()
            .contains("derivations rebuilt: true")
    );
    assert_eq!(
        ok(engine.execute(Command::OpenEncrypted))
            .view
            .unwrap()
            .sources()
            .len(),
        1
    );
    assert_eq!(fixture.provider.key_creations(), 1);
}

#[test]
fn encrypted_worker_updates_searches_and_exports_selected_history() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    let first = ok(engine.execute(Command::Import(fixture.input("historical synthetic text"))));
    let old_id = first.view.unwrap().selected_source_id().unwrap().clone();
    let current = ok(engine.execute(Command::Update(fixture.input("current synthetic text"))));
    assert_eq!(current.view.unwrap().versions().len(), 2);
    let searched = ok(engine.execute(Command::Search("current".into())));
    assert_eq!(searched.view.unwrap().search_results().len(), 1);
    assert!(
        ok(engine.execute(Command::Search("historical".into())))
            .view
            .unwrap()
            .search_results()
            .is_empty()
    );
    ok(engine.execute(Command::SelectVersion(old_id)));
    ok(engine.execute(Command::Export(fixture.export())));
    assert_eq!(
        fs::read_to_string(fixture.root.join("export/result.md")).unwrap(),
        "historical synthetic text"
    );
}

#[test]
fn retry_commit_survives_recovery_inspection_failure_without_becoming_a_write_error() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    fixture.provider.interrupt_next_capture_after_publish();
    assert!(
        engine
            .execute(Command::Import(fixture.input("retry inspection boundary")))
            .holds_capture
    );
    fixture
        .provider
        .fail_on_key_load(fixture.provider.key_loads() + 4);
    let snapshot = ok(engine.execute(Command::RetryOriginal));
    assert!(!snapshot.holds_original);
    assert!(snapshot.refresh_error.is_some());
    assert!(snapshot.message.unwrap().contains("executed"));
    assert!(!snapshot.recovery_known);
    assert!(snapshot.view.is_none());
    let refreshed = ok(engine.execute(Command::Refresh));
    assert_eq!(refreshed.view.unwrap().sources().len(), 1);
}
