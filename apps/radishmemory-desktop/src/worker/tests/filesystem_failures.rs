//! Real filesystem failures with a synthetic key, without coordinator interruption hooks.
use super::*;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

struct ReadOnlyDirectory {
    path: PathBuf,
    original: fs::Permissions,
}
impl ReadOnlyDirectory {
    fn new(path: PathBuf) -> Self {
        let original = fs::metadata(&path).unwrap().permissions();
        assert_eq!(original.mode() & 0o777, 0o700);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o500)).unwrap();
        Self { path, original }
    }
}
impl Drop for ReadOnlyDirectory {
    fn drop(&mut self) {
        // Restore before Fixture cleanup, including assertion failures.
        fs::set_permissions(&self.path, self.original.clone()).unwrap();
    }
}

fn files(path: &Path) -> Vec<(std::ffi::OsString, Vec<u8>)> {
    let mut files = fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn assert_io_failure(snapshot: &Snapshot) {
    assert_eq!(
        snapshot
            .error
            .as_ref()
            .unwrap()
            .application_failure()
            .unwrap()
            .vault_code(),
        Some(SourceVaultErrorCode::Io)
    );
    assert!(snapshot.view.is_none());
    assert!(snapshot.message.is_none());
    assert!(snapshot.holds_original);
}

#[test]
fn publication_permission_failure_retries_the_retained_staging_bytes() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    let objects = fixture.root.join("data/source-objects-v1");
    let staging = fixture.root.join("data/source-staging-v1");
    let denied = ReadOnlyDirectory::new(objects.clone());
    let original = "\u{feff}# Synthetic original\r\n恢复快照 nebula\r\n";
    let failed = engine.execute(Command::Import(fixture.input(original)));
    assert_io_failure(&failed);
    assert!(failed.holds_capture);
    let staged = files(&staging);
    assert_eq!(staged.len(), 1);
    assert!(files(&objects).is_empty());
    let inspected = ok(engine.execute(Command::InspectRecovery));
    assert!(inspected.abandonment_available);
    assert!(inspected.holds_capture);
    assert_eq!(files(&staging), staged);
    assert_io_failure(&engine.execute(Command::RetryOriginal));
    assert_eq!(files(&staging), staged);
    assert!(
        engine
            .execute(Command::Import(fixture.input("external replacement")))
            .error
            .is_some()
    );
    drop(denied);
    fs::remove_file(fixture.root.join("input/note.md")).unwrap();
    let retried = ok(engine.execute(Command::RetryOriginal));
    assert!(retried.refresh_error.is_none());
    assert!(!retried.holds_original);
    assert!(!retried.abandonment_available);
    assert_eq!(retried.view.unwrap().sources().len(), 1);
    assert!(files(&staging).is_empty());
    let published = files(&objects);
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].1, staged[0].1);
    ok(engine.execute(Command::Export(fixture.export())));
    assert_eq!(
        fs::read(fixture.root.join("export/result.md")).unwrap(),
        original.as_bytes()
    );
    assert_eq!(fixture.provider.key_creations(), 1);
}

#[test]
fn publication_permission_failure_restart_abandons_only_the_pending_update() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    let committed = ok(engine.execute(Command::Import(fixture.input("committed nebula"))));
    let source = committed
        .view
        .unwrap()
        .selected_source_id()
        .unwrap()
        .clone();
    let objects = fixture.root.join("data/source-objects-v1");
    let staging = fixture.root.join("data/source-staging-v1");
    let baseline = files(&objects);
    let denied = ReadOnlyDirectory::new(objects.clone());
    assert_io_failure(&engine.execute(Command::Update(fixture.input("pending replacement"))));
    assert_eq!(files(&objects), baseline);
    let staged = files(&staging);
    assert_eq!(staged.len(), 1);
    drop(engine);
    drop(denied);
    fs::remove_file(fixture.root.join("input/note.md")).unwrap();
    let mut reopened = fixture.engine();
    let opened = ok(reopened.execute(Command::OpenEncrypted));
    assert!(!opened.holds_original);
    assert!(opened.abandonment_available);
    assert_eq!(opened.view.unwrap().selected_source_id(), Some(&source));
    assert!(reopened.execute(Command::RetryOriginal).error.is_some());
    ok(reopened.execute(Command::Verify));
    ok(reopened.execute(Command::Rebuild));
    assert_eq!(files(&staging), staged);
    assert_eq!(files(&objects), baseline);
    let abandoned = ok(reopened.execute(Command::Abandon));
    assert!(abandoned.refresh_error.is_none());
    assert!(!abandoned.abandonment_available);
    assert!(!abandoned.holds_original);
    assert!(files(&staging).is_empty());
    assert_eq!(files(&objects), baseline);
    assert!(reopened.execute(Command::Abandon).error.is_some());
    ok(reopened.execute(Command::Refresh));
    ok(reopened.execute(Command::Export(fixture.export())));
    assert_eq!(
        fs::read_to_string(fixture.root.join("export/result.md")).unwrap(),
        "committed nebula"
    );
    assert_eq!(fixture.provider.key_creations(), 1);
}

#[test]
fn deletion_permission_failure_restart_resumes_original_authority_only() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    let retained = ok(engine.execute(Command::Import(fixture.input("unrelated comet"))));
    let retained = retained
        .view
        .unwrap()
        .selected_lineage_id()
        .unwrap()
        .clone();
    let objects = fixture.root.join("data/source-objects-v1");
    let survivor = files(&objects);
    ok(engine.execute(Command::Import(fixture.input("deleted nebula v1"))));
    ok(engine.execute(Command::Update(fixture.input("deleted nebula v2"))));
    let baseline = files(&objects);
    assert_eq!(baseline.len(), 3);
    let denied = ReadOnlyDirectory::new(objects.clone());
    let failed = engine.execute(Command::Delete);
    assert_io_failure(&failed);
    assert!(failed.last_deletion.is_none());
    let Library::Encrypted {
        deletion: Some(request),
        ..
    } = engine.controller().unwrap().backend()
    else {
        panic!("missing original deletion");
    };
    let request_id = request.params().delete_request_id.clone();
    assert_eq!(files(&objects), baseline);
    drop(engine);
    let mut reopened = fixture.engine();
    let opened = ok(reopened.execute(Command::OpenEncrypted));
    assert_eq!(opened.deletion_requests, vec![request_id.clone()]);
    assert!(!opened.holds_original);
    // The visible selection is now a different lineage; resuming must not re-plan it.
    assert_eq!(opened.view.unwrap().selected_lineage_id(), Some(&retained));
    assert_io_failure(&reopened.execute(Command::ResumeDelete(request_id.clone())));
    assert_eq!(files(&objects), baseline);
    drop(denied);
    let completed = ok(reopened.execute(Command::RetryOriginal));
    assert!(completed.refresh_error.is_none());
    let evidence = completed.last_deletion.unwrap();
    assert_eq!(evidence.params().delete_request_id, request_id);
    assert_eq!(
        evidence.params().overall_status,
        DeletionOverallStatus::Completed
    );
    assert_eq!(evidence.params().component_results.len(), 10);
    assert!(completed.deletion_requests.is_empty());
    assert!(!completed.holds_original);
    assert_eq!(files(&objects), survivor);
    assert!(
        reopened
            .execute(Command::ResumeDelete(request_id))
            .error
            .is_some()
    );
    ok(reopened.execute(Command::Refresh));
    ok(reopened.execute(Command::Export(fixture.export())));
    assert_eq!(
        fs::read_to_string(fixture.root.join("export/result.md")).unwrap(),
        "unrelated comet"
    );
    assert_eq!(
        fs::read_to_string(fixture.root.join("input/note.md")).unwrap(),
        "deleted nebula v2"
    );
    assert_eq!(fixture.provider.key_creations(), 1);
}
