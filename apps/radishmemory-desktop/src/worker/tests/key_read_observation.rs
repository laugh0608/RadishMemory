use super::*;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;

fn observed(
    fixture: &crate::startup::tests::Fixture,
    provider: &SyntheticLibraryProvider,
    path: &std::path::Path,
) -> Engine<SyntheticLibraryProvider, impl Fn() -> SyntheticLibraryProvider + use<>> {
    let mut args = fixture.arguments();
    args.extend([
        "--acceptance-key-read-trace".into(),
        path.as_os_str().to_owned(),
    ]);
    let provider = provider.clone();
    Worker::engine(crate::startup::Startup::parse(args).unwrap(), move || {
        provider.clone()
    })
    .unwrap()
}
fn prepare(fixture: &crate::startup::tests::Fixture, provider: &SyntheticLibraryProvider) {
    let mut engine = Worker::engine(
        crate::startup::Startup::parse(fixture.arguments()).unwrap(),
        || provider.clone(),
    )
    .unwrap();
    ready(&mut engine);
}
fn assert_hidden(snapshot: Snapshot) {
    assert!(snapshot.error.is_some());
    assert!(snapshot.view.is_none());
    assert!(!snapshot.recovery_known);
    assert!(!snapshot.abandonment_available);
    assert!(snapshot.deletion_requests.is_empty());
}
#[test]
fn single_diagnostic_action_success_or_business_failure_never_retries_or_writes() {
    for failure in [false, true] {
        let fixture = crate::startup::tests::Fixture::new();
        let output = Fixture::new();
        fs::set_permissions(&output.root, fs::Permissions::from_mode(0o700)).unwrap();
        let provider = SyntheticLibraryProvider::default();
        prepare(&fixture, &provider);
        let trace = fs::canonicalize(&output.root).unwrap().join("trace");
        let mut engine = observed(&fixture, &provider, &trace);
        assert!(!trace.exists());
        let database_before = fs::read(fixture.root.join("library.sqlite3")).unwrap();
        let loads_before_initial = provider.key_loads();
        let initial = ok(engine.execute(Command::OpenPlain));
        assert!(initial.view.is_none());
        assert!(!initial.recovery_known);
        assert_eq!(
            initial.message.as_deref(),
            Some("Key-read observation is ready; open encrypted objects once.")
        );
        assert!(!trace.exists());
        assert_eq!(provider.key_loads(), loads_before_initial);
        assert_eq!(
            fs::read(fixture.root.join("library.sqlite3")).unwrap(),
            database_before
        );
        if failure {
            provider.fail_key_access(Some(SourceVaultErrorCode::KeyStoreDenied));
        }
        let before = provider.key_loads();
        let snapshot = engine.execute(Command::OpenEncrypted);
        if failure {
            assert_hidden(snapshot);
        } else {
            assert!(ok(snapshot).view.is_some());
        }
        assert_eq!(provider.key_loads() - before, if failure { 1 } else { 2 });
        let bytes = fs::read(&trace).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("synthetic"));
        let after = provider.key_loads();
        for command in [
            Command::OpenPlain,
            Command::OpenEncrypted,
            Command::Refresh,
            Command::InitializeKey,
            Command::Migrate,
            Command::Verify,
            Command::Rebuild,
            Command::Delete,
            Command::InspectRecovery,
            Command::Abandon,
            Command::Search("synthetic-private-marker".into()),
            Command::Import(output.input("synthetic-private-marker")),
        ] {
            let rejected = engine.execute(command);
            assert_eq!(
                rejected.error.as_ref().unwrap().reason(),
                DesktopErrorReason::KeyReadObservationConsumed
            );
            assert_hidden(rejected);
        }
        assert_eq!(provider.key_loads(), after);
        assert_eq!(provider.key_creations(), 1);
        assert_eq!(fs::read(&trace).unwrap(), bytes);
    }
}
#[test]
fn trace_create_profile_and_real_end_integrity_failures_withdraw_all_results() {
    for failure in [0, 1, 2, 3, 4] {
        let fixture = crate::startup::tests::Fixture::new();
        let output = Fixture::new();
        fs::set_permissions(&output.root, fs::Permissions::from_mode(0o700)).unwrap();
        let provider = SyntheticLibraryProvider::default();
        prepare(&fixture, &provider);
        let trace = fs::canonicalize(&output.root).unwrap().join("trace");
        let mut engine = observed(&fixture, &provider, &trace);
        let before = provider.key_loads();
        match failure {
            0 => fs::write(&trace, b"synthetic-existing").unwrap(),
            1 => fs::remove_file(fixture.root.join("host-profile-v1.txt")).unwrap(),
            4 => fs::remove_file(fixture.root.join("library.sqlite3")).unwrap(),
            _ => {
                let path = trace.clone();
                engine.after_observed_action = Some(Box::new(move || {
                    fs::OpenOptions::new()
                        .append(true)
                        .open(path)
                        .unwrap()
                        .write_all(b"synthetic-tamper")
                        .unwrap();
                }));
                if failure == 3 {
                    provider.fail_key_access(Some(SourceVaultErrorCode::KeyStoreDenied));
                }
            }
        }
        let result = engine.execute(Command::OpenEncrypted);
        if failure == 3 {
            assert_eq!(
                result
                    .error
                    .as_ref()
                    .unwrap()
                    .application_failure()
                    .unwrap()
                    .vault_code(),
                Some(SourceVaultErrorCode::KeyStoreDenied)
            );
        } else if !matches!(failure, 1 | 4) {
            assert_eq!(
                result.error.as_ref().unwrap().reason(),
                DesktopErrorReason::KeyReadObservationFailed
            );
        }
        assert_hidden(result);
        assert_eq!(
            provider.key_loads() - before,
            match failure {
                0 | 1 | 4 => 0,
                2 => 2,
                _ => 1,
            }
        );
        let after = provider.key_loads();
        assert_hidden(engine.execute(Command::OpenEncrypted));
        assert_eq!(provider.key_loads(), after);
    }
}
