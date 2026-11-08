use super::{
    test_support::{Harness, painted_text},
    *,
};
use crate::worker::tests::{Fixture, ok};
use radishmemory_application::{FileExportRequest, RecallProfile};
use std::fs;

const BODY: &str = "蓝色主题 q03fixture\n";

fn drift_fts(fixture: &Fixture) -> Vec<u8> {
    let path = fixture.root.join("data/library.sqlite3");
    let mut bytes = fs::read(&path).unwrap();
    // The Han FTS shadow row stores the unindexed text immediately before
    // the derived Han tokens. Match the entire pair to leave canonical copies alone.
    let pattern = format!("{BODY}h84dd h8272 h4e3b h9898");
    let offsets: Vec<_> = bytes
        .windows(pattern.len())
        .enumerate()
        .filter_map(|(offset, data)| (data == pattern.as_bytes()).then_some(offset))
        .collect();
    assert_eq!(offsets.len(), 1);
    bytes[offsets[0] + BODY.find("q03fixture").unwrap()] = b'x';
    fs::write(path, &bytes).unwrap();
    bytes
}

#[test]
fn failed_plain_open_reaches_explicit_recall_repair() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ok(engine.execute(Command::OpenPlain));
    let imported = ok(engine.execute(Command::Import(fixture.input(BODY))));
    let source = imported
        .view
        .as_ref()
        .unwrap()
        .selected_source_id()
        .unwrap()
        .clone();
    drop(engine);
    let damaged = drift_fts(&fixture);
    let path = fixture.root.join("data/library.sqlite3");

    let mut engine = fixture.engine();
    let failed = engine.execute(Command::OpenPlain);
    assert!(failed.error.is_some());
    assert!(failed.view.is_none());
    assert!(failed.plain_repair_available);
    assert!(!failed.plain_repair_checked);
    assert_eq!(fs::read(&path).unwrap(), damaged);
    let unconfirmed = engine.execute(Command::RebuildPlainRecall);
    assert!(unconfirmed.error.is_some());
    assert!(unconfirmed.view.is_none());
    assert_eq!(fs::read(&path).unwrap(), damaged);

    let mut harness = Harness::new();
    harness.viewport_size = egui::vec2(760.0, 520.0);
    harness.results.send(failed).unwrap();
    harness
        .click("Check plaintext recall repair")
        .drop_without_applying_deltas();
    let command = harness.commands.try_recv().unwrap();
    assert!(matches!(command, Command::CheckPlainRecallRepair));
    let checked = ok(engine.execute(command));
    assert!(checked.view.is_none());
    assert!(checked.plain_repair_checked);
    assert_eq!(fs::read(&path).unwrap(), damaged);
    harness.results.send(checked).unwrap();
    harness
        .click("Rebuild verified plaintext recall…")
        .drop_without_applying_deltas();
    harness.click("Cancel").drop_without_applying_deltas();
    harness.no_command();
    assert!(harness.app.controller.is_none());
    assert_eq!(fs::read(&path).unwrap(), damaged);

    harness
        .click("Rebuild verified plaintext recall…")
        .drop_without_applying_deltas();
    harness.click("Confirm").drop_without_applying_deltas();
    let command = harness.commands.try_recv().unwrap();
    assert!(matches!(command, Command::RebuildPlainRecall));
    let repaired = ok(engine.execute(command));
    assert_eq!(repaired.recall_profile, Some(RecallProfile::HanPhraseV1));
    assert!(!repaired.plain_repair_available);
    assert!(!repaired.plain_repair_checked);
    assert_eq!(
        repaired.view.as_ref().unwrap().selected_source_id(),
        Some(&source)
    );
    harness.results.send(repaired).unwrap();
    let frame = harness.frame(false, vec![]);
    assert!(painted_text(&frame).contains("Plaintext recall rebuilt and verified"));
    frame.drop_without_applying_deltas();
    assert!(harness.app.can_mutate());

    drop(engine);
    let mut reopened = fixture.engine();
    ok(reopened.execute(Command::OpenPlain));
    ok(reopened.execute(Command::Verify));
    let searched = ok(reopened.execute(Command::Search("蓝色".into())));
    let view = searched.view.unwrap();
    assert_eq!(view.search_results().len(), 1);
    assert_eq!(view.search_results()[0].source_id(), &source);
    let export = fixture.root.join("export/repaired.md");
    let request = FileExportRequest::new(&export, vec![fixture.root.join("export")]).unwrap();
    ok(reopened.execute(Command::Export(request)));
    assert_eq!(fs::read(export).unwrap(), BODY.as_bytes());
    assert_eq!(fixture.provider.key_loads(), 0);
    assert_eq!(fixture.provider.key_creations(), 0);
}

#[test]
fn failed_recall_repair_and_retry_do_not_recreate_a_missing_database() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ok(engine.execute(Command::OpenPlain));
    ok(engine.execute(Command::Import(fixture.input(BODY))));
    drop(engine);
    let damaged = drift_fts(&fixture);
    let path = fixture.root.join("data/library.sqlite3");
    let held = fixture.root.join("held.sqlite3");

    let mut engine = fixture.engine();
    assert!(engine.execute(Command::OpenPlain).plain_repair_available);
    assert!(ok(engine.execute(Command::CheckPlainRecallRepair)).plain_repair_checked);
    fs::rename(&path, &held).unwrap();
    for command in [
        Command::RebuildPlainRecall,
        Command::OpenPlain,
        Command::OpenPlain,
    ] {
        let failure = engine.execute(command);
        assert!(failure.error.is_some());
        assert!(failure.view.is_none());
        assert!(!failure.plain_repair_checked);
        assert!(!path.exists());
    }
    assert_eq!(fs::read(&held).unwrap(), damaged);
    fs::rename(held, &path).unwrap();
    assert!(ok(engine.execute(Command::CheckPlainRecallRepair)).plain_repair_checked);
    assert!(
        ok(engine.execute(Command::RebuildPlainRecall))
            .view
            .is_some()
    );
}
