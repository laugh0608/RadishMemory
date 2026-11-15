use super::*;
use std::cell::Cell;

const BODY: &str = "合成启动引导 synthetic readback";

#[test]
fn startup_inspects_plaintext_pending_and_completed_migration_without_provider_access() {
    let fixture = Fixture::new();
    let mut setup = fixture.engine();
    let first = ok(setup.execute(Command::StartLibrary));
    assert!(first.view.unwrap().sources().is_empty());
    ok(setup.execute(Command::Import(fixture.input(BODY))));
    drop(setup);

    let factory_calls = Cell::new(0);
    let path = fixture.root.join("data/library.sqlite3");
    for (format, sources) in [
        (LibraryFormat::Plaintext, Some(1)),
        (LibraryFormat::MigrationPending, None), // v7 checkpoint
        (LibraryFormat::MigrationPending, None), // interrupted v8 migration
        (LibraryFormat::EncryptedObjects, None), // completed v8 migration
    ] {
        let before = fs::read(&path).unwrap();
        let loads = fixture.provider.key_loads();
        let creations = fixture.provider.key_creations();
        let mut engine = Engine::new(
            ApplicationPaths::from_data_directory(fixture.root.join("data")).unwrap(),
            || {
                factory_calls.set(factory_calls.get() + 1);
                fixture.provider.clone()
            },
        );
        for _ in 0..2 {
            let started = ok(engine.execute(Command::StartLibrary));
            assert_eq!(started.startup, StartupState::Existing(format));
            assert_eq!(started.view.map(|view| view.sources().len()), sources);
            assert_eq!(factory_calls.get(), 0);
            assert_eq!(fixture.provider.key_loads(), loads);
            assert_eq!(fixture.provider.key_creations(), creations);
            assert_eq!(fs::read(&path).unwrap(), before);
        }
        drop(engine);

        let mut setup = fixture.engine();
        match (
            format,
            before[60..64].try_into().map(u32::from_be_bytes).unwrap(),
        ) {
            (LibraryFormat::Plaintext, 6) => {
                ok(setup.execute(Command::InitializeKey));
            }
            (LibraryFormat::MigrationPending, 7) => {
                fixture.provider.interrupt_next_migration_after_publish();
                assert!(setup.execute(Command::Migrate).error.is_some());
            }
            (LibraryFormat::MigrationPending, 8) => {
                ok(setup.execute(Command::Migrate));
            }
            (LibraryFormat::EncryptedObjects, 8) => {}
            route => panic!("unexpected database format and version: {route:?}"),
        }
    }

    let before = fs::read(&path).unwrap();
    let mut reopened = fixture.engine();
    ok(reopened.execute(Command::StartLibrary));
    fixture
        .provider
        .fail_key_access(Some(SourceVaultErrorCode::KeyStoreCancelled));
    let cancelled = reopened.execute(Command::OpenEncrypted);
    assert_eq!(
        cancelled
            .error
            .unwrap()
            .application_failure()
            .unwrap()
            .vault_code(),
        Some(SourceVaultErrorCode::KeyStoreCancelled),
    );
    assert!(cancelled.view.is_none());
    assert!(reopened.execute(Command::OpenPlain).view.is_none());
    assert_eq!(fs::read(&path).unwrap(), before);

    fixture.provider.fail_key_access(None);
    let opened = ok(reopened.execute(Command::OpenEncrypted));
    assert_eq!(opened.view.unwrap().sources().len(), 1);
    ok(reopened.execute(Command::Export(fixture.export())));
    assert_eq!(
        fs::read(fixture.root.join("export/result.md")).unwrap(),
        BODY.as_bytes()
    );
}

#[test]
fn unknown_damaged_and_missing_databases_stay_closed_without_replacement() {
    let fixture = Fixture::new();
    let mut setup = fixture.engine();
    ok(setup.execute(Command::StartLibrary));
    let path = fixture.root.join("data/library.sqlite3");
    drop(setup);

    let mut future = fs::read(&path).unwrap();
    // SQLite's file header stores user_version as a big-endian u32 at offset 60.
    future[60..64].copy_from_slice(&99_u32.to_be_bytes());
    for (bytes, reason) in [
        (future.as_slice(), "UnsupportedSchemaVersion"),
        (
            b"synthetic invalid database".as_slice(),
            "ConnectionConfiguration",
        ),
    ] {
        fs::write(&path, bytes).unwrap();
        let mut engine = fixture.engine();
        for command in [Command::StartLibrary, Command::OpenPlain] {
            let failed = engine.execute(command);
            assert_eq!(failed.startup, StartupState::Unavailable);
            assert!(failed.view.is_none());
            let error = failed.error.unwrap();
            assert!(
                error.plaintext_database().unwrap().contains(reason),
                "{error:?}"
            );
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }
    fs::remove_file(&path).unwrap();
    let mut engine = fixture.engine();
    for command in [
        Command::StartLibrary,
        Command::OpenPlain,
        Command::OpenEncrypted,
    ] {
        let failed = engine.execute(command);
        assert_eq!(failed.startup, StartupState::Unavailable);
        assert!(failed.view.is_none());
        assert_eq!(
            failed.error.unwrap().reason(),
            DesktopErrorReason::DatabaseMissing
        );
        assert!(!path.exists());
    }
    assert_eq!(fixture.provider.key_loads(), 0);
    assert_eq!(fixture.provider.key_creations(), 0);
}
