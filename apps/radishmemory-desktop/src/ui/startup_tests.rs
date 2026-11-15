use super::test_support::{Harness, painted_text};
use super::*;

#[test]
fn only_completed_migration_offers_explicit_encrypted_open() {
    let mut app = Harness::new();
    app.viewport_size = egui::vec2(760.0, 520.0);
    for (startup, expected) in [
        (StartupState::Unavailable, "Recheck library"),
        (
            StartupState::Existing(LibraryFormat::MigrationPending),
            "Migrate / resume bodies…",
        ),
        (
            StartupState::Existing(LibraryFormat::EncryptedObjects),
            "Open encrypted objects",
        ),
    ] {
        app.results
            .send(Snapshot {
                startup,
                ..Snapshot::default()
            })
            .unwrap();
        let output = app.frame(false, vec![]);
        let text = painted_text(&output);
        output.drop_without_applying_deltas();
        assert!(text.contains(expected), "{text}");
        assert_eq!(
            text.contains("Open encrypted objects"),
            startup == StartupState::Existing(LibraryFormat::EncryptedObjects),
        );
        assert!(!text.contains("Retry legacy open"));
        app.no_command();
    }
    app.click("Open encrypted objects")
        .drop_without_applying_deltas();
    assert!(matches!(
        app.commands.try_recv(),
        Ok(Command::OpenEncrypted)
    ));
    app.no_command();
}
