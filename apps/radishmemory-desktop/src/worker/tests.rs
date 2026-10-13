use super::*;
use radishmemory_source_vault::{SourceVaultErrorCode, acceptance::SyntheticLibraryProvider};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
mod filesystem_failures;
mod locking;

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
fn native_startup_routes_migration_and_reopen_to_the_pinned_library() {
    let fixture = crate::startup::tests::Fixture::new();
    let input = Fixture::new();
    let provider = SyntheticLibraryProvider::default();
    let mut engine = Worker::engine(
        crate::startup::Startup::parse(fixture.arguments()).unwrap(),
        || provider.clone(),
    )
    .unwrap();
    ok(engine.execute(Command::OpenPlain));
    ok(engine.execute(Command::Import(
        input.input("synthetic acceptance migration"),
    )));
    let profile = fs::read(fixture.root.join("host-profile-v1.txt")).unwrap();
    ok(engine.execute(Command::InitializeKey));
    ok(engine.execute(Command::Migrate));
    ok(engine.execute(Command::OpenEncrypted));
    ok(engine.execute(Command::Export(input.export())));
    assert_eq!(
        fs::read_to_string(input.root.join("export/result.md")).unwrap(),
        "synthetic acceptance migration"
    );
    assert!(fixture.root.join("library.sqlite3").is_file());
    assert_eq!(provider.key_creations(), 1);
    drop(engine);
    let mut reopened = Worker::engine(
        crate::startup::Startup::parse(fixture.arguments()).unwrap(),
        || provider.clone(),
    )
    .unwrap();
    assert!(reopened.execute(Command::OpenPlain).error.is_some());
    assert!(ok(reopened.execute(Command::OpenEncrypted)).view.is_some());
    assert_eq!(provider.key_creations(), 1);
    assert_eq!(
        fs::read(fixture.root.join("host-profile-v1.txt")).unwrap(),
        profile
    );
}

#[test]
fn native_startup_rechecks_profile_before_bootstrap_and_each_command() {
    let fixture = crate::startup::tests::Fixture::new();
    let provider = SyntheticLibraryProvider::default();
    let startup = crate::startup::Startup::parse(fixture.arguments()).unwrap();
    let profile_path = fixture.root.join("host-profile-v1.txt");
    let profile = fs::read(&profile_path).unwrap();
    fs::remove_file(&profile_path).unwrap();
    assert!(Worker::engine(startup, || provider.clone()).is_err());
    assert!(!fixture.root.join("library.sqlite3").exists());
    fs::write(&profile_path, &profile).unwrap();
    let mut engine = Worker::engine(
        crate::startup::Startup::parse(fixture.arguments()).unwrap(),
        || provider.clone(),
    )
    .unwrap();
    ok(engine.execute(Command::OpenPlain));
    fs::remove_file(&profile_path).unwrap();
    for command in [
        Command::InitializeKey,
        Command::OpenPlain,
        Command::OpenEncrypted,
        Command::Rebuild,
        Command::RetryOriginal,
    ] {
        let snapshot = engine.execute(command);
        assert_eq!(
            snapshot.error.unwrap().reason(),
            DesktopErrorReason::ProfileReadFailed
        );
        assert!(snapshot.view.is_none());
        assert!(!profile_path.exists());
        assert_eq!(provider.key_creations(), 0);
    }
    fs::write(&profile_path, &profile).unwrap();
    ok(engine.execute(Command::InitializeKey));
    assert_eq!(provider.key_creations(), 1);
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
fn native_restart_rejects_object_damage_without_repair_or_origin_fallback() {
    for missing in [false, true] {
        let fixture = crate::startup::tests::Fixture::new();
        let input = Fixture::new();
        let provider = SyntheticLibraryProvider::default();
        let mut engine = Worker::engine(
            crate::startup::Startup::parse(fixture.arguments()).unwrap(),
            || provider.clone(),
        )
        .unwrap();
        ready(&mut engine);
        let imported = ok(engine.execute(Command::Import(
            input.input("synthetic authenticated recovery needle"),
        )));
        let source_id = imported.view.unwrap().selected_source_id().unwrap().clone();
        drop(engine);

        let profile = fs::read(fixture.root.join("host-profile-v1.txt")).unwrap();
        let mut objects = fs::read_dir(fixture.root.join("source-objects-v1")).unwrap();
        let object = objects.next().unwrap().unwrap().path();
        assert!(objects.next().is_none());
        let original = fs::read(&object).unwrap();
        let mut damaged = original.clone();
        *damaged.last_mut().unwrap() ^= 1;
        let held = fixture.root.join("held-synthetic-object");
        if missing {
            fs::rename(&object, &held).unwrap();
        } else {
            fs::write(&object, &damaged).unwrap();
        }
        fs::write(input.root.join("input/note.md"), "external replacement").unwrap();

        let mut reopened = Worker::engine(
            crate::startup::Startup::parse(fixture.arguments()).unwrap(),
            || provider.clone(),
        )
        .unwrap();
        assert!(reopened.execute(Command::OpenPlain).error.is_some());
        for command in [
            Command::OpenEncrypted,
            Command::Verify,
            Command::Rebuild,
            Command::Search("needle".into()),
            Command::Export(input.export()),
        ] {
            let rejected = reopened.execute(command);
            assert!(rejected.error.is_some());
            assert!(rejected.view.is_none());
            assert!(rejected.message.is_none());
            assert!(!input.root.join("export/result.md").exists());
            if missing {
                assert!(!object.exists());
                assert_eq!(fs::read(&held).unwrap(), original);
            } else {
                assert_eq!(fs::read(&object).unwrap(), damaged);
            }
            assert_eq!(provider.key_creations(), 1);
        }
        assert_eq!(
            fs::read(fixture.root.join("host-profile-v1.txt")).unwrap(),
            profile
        );
        drop(reopened);

        // Restore only this test's original ciphertext; the product must not repair it.
        if missing {
            fs::rename(&held, &object).unwrap();
        } else {
            fs::write(&object, &original).unwrap();
        }
        let mut restored = Worker::engine(
            crate::startup::Startup::parse(fixture.arguments()).unwrap(),
            || provider.clone(),
        )
        .unwrap();
        let opened = ok(restored.execute(Command::OpenEncrypted));
        assert_eq!(opened.view.unwrap().selected_source_id(), Some(&source_id));
        let searched = ok(restored.execute(Command::Search("needle".into())));
        assert_eq!(searched.view.unwrap().search_results().len(), 1);
        ok(restored.execute(Command::Export(input.export())));
        assert_eq!(
            fs::read_to_string(input.root.join("export/result.md")).unwrap(),
            "synthetic authenticated recovery needle"
        );
        assert_eq!(provider.key_creations(), 1);
    }
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

// Control result delivery at the UI boundary without a native window or key store.
pub(crate) fn channel_worker() -> (Worker, Sender<Snapshot>, Receiver<Command>) {
    let (sender, commands) = mpsc::channel();
    let (results, receiver) = mpsc::channel();
    (
        Worker {
            sender,
            receiver,
            thread: None,
            busy: false,
            stopped: false,
        },
        results,
        commands,
    )
}

#[test]
fn dropping_busy_worker_finishes_accepted_command_and_joins_engine() {
    use std::sync::{Arc, atomic::AtomicBool};
    use std::time::Duration;

    struct EngineLifetime(Arc<AtomicBool>);
    impl Drop for EngineLifetime {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let fixture = Fixture::new();
    let data = fixture.root.join("drop-thread-data");
    let worker_data = data.clone();
    let ended = Arc::new(AtomicBool::new(false));
    let lifetime = EngineLifetime(ended.clone());
    let (entered, entry) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let mut worker = Worker::spawn(move || {
        entered.send(()).unwrap();
        // Bounded test gate, not a production interruption or system key operation.
        gate.recv_timeout(Duration::from_secs(10)).unwrap();
        let provider = SyntheticLibraryProvider::default();
        Ok(Engine::new(
            ApplicationPaths::from_data_directory(worker_data).unwrap(),
            move || {
                let _ = &lifetime;
                provider.clone()
            },
        ))
    })
    .unwrap();
    entry.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.send(Command::OpenPlain).unwrap();
    assert!(worker.busy);
    let (dropping, drop_started) = mpsc::channel();
    let waiter = thread::spawn(move || {
        dropping.send(()).unwrap();
        drop(worker);
        // Drop must join even though the UI has not polled the accepted command.
        assert!(ended.load(Ordering::SeqCst));
    });
    drop_started.recv_timeout(Duration::from_secs(5)).unwrap();
    release.send(()).unwrap();
    waiter.join().unwrap();
    assert!(data.join("library.sqlite3").is_file());
    let provider = SyntheticLibraryProvider::default();
    let mut reopened = Engine::new(
        ApplicationPaths::from_data_directory(data).unwrap(),
        move || provider.clone(),
    );
    assert!(
        ok(reopened.execute(Command::OpenPlain))
            .view
            .unwrap()
            .sources()
            .is_empty()
    );
}
