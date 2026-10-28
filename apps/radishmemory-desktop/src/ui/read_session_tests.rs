//! Real encrypted engine snapshots delivered to production UI; key failures are synthetic.
use super::test_support::Harness;
use super::*;
use crate::worker::tests::{Fixture, ok, ready};
use radishmemory_source_vault::SourceVaultErrorCode;

fn painted_text(output: &egui::FullOutput) -> String {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) => Some(text.galley.text()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn cancelled_authenticated_read_withdraws_populated_ui_until_explicit_recovery() {
    for reopen in [false, true] {
        for failed_load in [1, 2] {
            let fixture = Fixture::new();
            let mut engine = fixture.engine();
            ready(&mut engine);
            ok(engine.execute(Command::Import(
                fixture.input("# Synthetic display boundary\nprivateviewtoken 中文资料\n"),
            )));
            let searched = ok(engine.execute(Command::Search("privateviewtoken".into())));
            let view = searched.view.as_ref().unwrap();
            assert_eq!(view.search_results().len(), 1);
            let source = view.selected_source_id().unwrap().clone();
            let title = source_label(view.selected_lineage().unwrap());
            let mut h = Harness::new();
            h.results.send(searched).unwrap();
            let output = h.frame(false, vec![]);
            let text = painted_text(&output);
            assert!(text.contains("privateviewtoken"));
            assert!(text.contains(source.as_str()));
            assert!(text.contains(&title));
            output.drop_without_applying_deltas();

            let loads = fixture.provider.key_loads();
            fixture.provider.fail_on_key_load_with(
                loads + failed_load,
                SourceVaultErrorCode::KeyStoreCancelled,
            );
            h.app.send(if reopen {
                Command::OpenEncrypted
            } else {
                Command::Refresh
            });
            let result = engine.execute(h.commands.try_recv().unwrap());
            assert_eq!(
                result
                    .error
                    .as_ref()
                    .unwrap()
                    .application_failure()
                    .unwrap()
                    .vault_code(),
                Some(SourceVaultErrorCode::KeyStoreCancelled)
            );
            h.results.send(result).unwrap();
            for _ in 0..2 {
                let output = h.frame(false, vec![]);
                let text = painted_text(&output);
                assert!(text.contains("Local library unavailable"));
                assert!(text.contains("KeyStoreCancelled"));
                for stale in ["privateviewtoken", "中文资料", source.as_str(), &title] {
                    assert!(!text.contains(stale), "stale data still painted: {stale}");
                }
                output.drop_without_applying_deltas();
                assert!(h.app.controller.is_none());
                assert!(!h.app.state.recovery_known);
                assert!(!h.app.can_mutate());
                h.no_command();
                assert_eq!(fixture.provider.key_loads(), loads + failed_load);
            }

            // Recovery must come from a new authenticated command, not the cached view.
            h.click(if reopen {
                "Retry encrypted open"
            } else {
                "Refresh"
            })
            .drop_without_applying_deltas();
            let command = h.commands.try_recv().unwrap();
            assert!(matches!(
                (&command, reopen),
                (Command::OpenEncrypted, true) | (Command::Refresh, false)
            ));
            let recovered = ok(engine.execute(command));
            let view = recovered.view.as_ref().unwrap();
            assert_eq!(view.selected_source_id(), Some(&source));
            assert!(view.search_results().is_empty());
            h.results.send(recovered).unwrap();
            let output = h.frame(false, vec![]);
            let text = painted_text(&output);
            assert!(text.contains(&title));
            assert!(!text.contains("privateviewtoken"));
            output.drop_without_applying_deltas();
            assert!(h.app.state.recovery_known);
            assert!(h.app.can_mutate());
            assert_eq!(fixture.provider.key_loads(), loads + failed_load + 2);
            h.no_command();

            h.app.send(Command::Search("privateviewtoken".into()));
            h.results
                .send(ok(engine.execute(h.commands.try_recv().unwrap())))
                .unwrap();
            let output = h.frame(false, vec![]);
            assert!(painted_text(&output).contains("privateviewtoken"));
            output.drop_without_applying_deltas();
            assert_eq!(fixture.provider.key_creations(), 1);
        }
    }
}

#[test]
fn damaged_object_refresh_withdraws_painted_view_until_repair_and_explicit_refresh() {
    let fixture = Fixture::new();
    let mut engine = fixture.engine();
    ready(&mut engine);
    ok(engine.execute(Command::Import(
        fixture.input("# Synthetic integrity boundary\nobjectviewtoken 中文资料\n"),
    )));
    let searched = ok(engine.execute(Command::Search("objectviewtoken".into())));
    let source = searched
        .view
        .as_ref()
        .unwrap()
        .selected_source_id()
        .unwrap()
        .clone();
    let mut h = Harness::new();
    h.results.send(searched).unwrap();
    let output = h.frame(false, vec![]);
    assert!(painted_text(&output).contains("objectviewtoken"));
    output.drop_without_applying_deltas();

    let objects = std::fs::read_dir(fixture.root.join("data/source-objects-v1"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(objects.len(), 1);
    let original = std::fs::read(&objects[0]).unwrap();
    let mut corrupted = original.clone();
    *corrupted.last_mut().unwrap() ^= 1;
    std::fs::write(&objects[0], &corrupted).unwrap();
    h.click("Refresh").drop_without_applying_deltas();
    let command = h.commands.try_recv().unwrap();
    assert!(matches!(command, Command::Refresh));
    let failed = engine.execute(command);
    assert_eq!(
        failed
            .error
            .as_ref()
            .unwrap()
            .application_failure()
            .unwrap()
            .vault_code(),
        Some(SourceVaultErrorCode::AuthenticationFailed)
    );
    h.results.send(failed).unwrap();
    let output = h.frame(false, vec![]);
    let text = painted_text(&output);
    assert!(text.contains("AuthenticationFailed"));
    assert!(text.contains("Local library unavailable"));
    assert!(!text.contains("objectviewtoken"));
    assert!(!text.contains(source.as_str()));
    assert!(h.app.controller.is_none());
    assert!(!h.app.can_mutate());
    output.drop_without_applying_deltas();
    assert_eq!(std::fs::read(&objects[0]).unwrap(), corrupted);
    h.no_command();

    std::fs::write(&objects[0], &original).unwrap();
    // External repair alone must not cause the old snapshot to reappear.
    h.frame(false, vec![]).drop_without_applying_deltas();
    assert!(h.app.controller.is_none());
    h.no_command();
    h.click("Refresh").drop_without_applying_deltas();
    let recovered = ok(engine.execute(h.commands.try_recv().unwrap()));
    let view = recovered.view.as_ref().unwrap();
    assert_eq!(view.selected_source_id(), Some(&source));
    assert!(view.search_results().is_empty());
    h.results.send(recovered).unwrap();
    h.frame(false, vec![]).drop_without_applying_deltas();
    assert!(h.app.can_mutate());
    h.app.send(Command::Search("objectviewtoken".into()));
    h.results
        .send(ok(engine.execute(h.commands.try_recv().unwrap())))
        .unwrap();
    let output = h.frame(false, vec![]);
    assert!(painted_text(&output).contains("objectviewtoken"));
    output.drop_without_applying_deltas();
    assert_eq!(std::fs::read(&objects[0]).unwrap(), original);
    assert_eq!(fixture.provider.key_creations(), 1);
}
