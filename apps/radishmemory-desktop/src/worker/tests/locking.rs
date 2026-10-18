//! Desktop behavior under a real competing SQLite session; keys remain synthetic.
use super::*;
use radishmemory_source_vault::ObjectDirectory;

#[test]
fn competing_reader_blocks_export_and_hides_stale_view_until_explicit_refresh() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    let original = "\u{feff}Synthetic lock boundary\r\n锁竞争 nebula\r\n";
    let imported = ok(engine.execute(Command::Import(fixture.input(original))));
    let source = imported.view.unwrap().selected_source_id().unwrap().clone();
    let paths = ApplicationPaths::from_data_directory(fixture.root.join("data")).unwrap();
    let profile = crate::profile::load_profile(&paths).unwrap();
    let directory = ObjectDirectory::open_application_directory(paths.data_directory()).unwrap();
    let reader = fixture
        .provider
        .open_library_reader(
            &directory,
            profile.namespace_id().as_str(),
            profile.device_id().as_str(),
        )
        .unwrap();
    let loads = fixture.provider.key_loads();
    for command in [Command::Export(fixture.export()), Command::Refresh] {
        let failed = engine.execute(command);
        let error = failed.error.as_ref().unwrap();
        assert!(error.vault_database().is_some());
        assert!(failed.view.is_none());
        assert!(failed.message.is_none());
        assert!(!failed.holds_original);
        assert!(!fixture.root.join("export/result.md").exists());
        assert_eq!(fixture.provider.key_loads(), loads);
    }
    // Failed competing connections must not release the first session's exclusion.
    assert!(
        fixture
            .provider
            .open_library_reader(
                &directory,
                profile.namespace_id().as_str(),
                profile.device_id().as_str(),
            )
            .is_err()
    );
    assert_eq!(fixture.provider.key_loads(), loads);
    drop(reader);
    // A successful maintenance inspection does not re-expose a stale UI snapshot.
    assert!(ok(engine.execute(Command::InspectRecovery)).view.is_none());
    let refreshed = ok(engine.execute(Command::Refresh));
    let view = refreshed.view.unwrap();
    assert_eq!(view.sources().len(), 1);
    assert_eq!(view.selected_source_id(), Some(&source));
    ok(engine.execute(Command::Export(fixture.export())));
    assert_eq!(
        fs::read(fixture.root.join("export/result.md")).unwrap(),
        original.as_bytes()
    );
    assert_eq!(fixture.provider.key_creations(), 1);
}
