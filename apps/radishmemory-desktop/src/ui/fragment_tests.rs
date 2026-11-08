use super::{
    test_support::{Harness, painted_text},
    *,
};
use crate::worker::tests::{Fixture, ok, ready};
use radishmemory_application::DeletionOverallStatus;

fn search(harness: &mut Harness, query: &str) -> Command {
    harness.app.search_query = query.into();
    harness.click("Search").drop_without_applying_deltas();
    harness.commands.try_recv().unwrap()
}

fn click_citation(harness: &mut Harness, snapshot: Snapshot) -> Command {
    let result = &snapshot.view.as_ref().unwrap().search_results()[0];
    let link = format!(
        "{} · v{} · bytes {}..{}",
        result.title().unwrap().as_str(),
        result.version().get(),
        result.byte_start(),
        result.byte_end()
    );
    harness.results.send(snapshot).unwrap();
    harness.click(&link).drop_without_applying_deltas();
    harness.commands.try_recv().unwrap()
}

fn visible_text(harness: &mut Harness) -> String {
    let frame = harness.frame(false, vec![]);
    let text = painted_text(&frame);
    frame.drop_without_applying_deltas();
    text
}

fn assert_search_cleared(harness: &mut Harness, previous_text: &str) {
    let text = visible_text(harness);
    assert!(!text.contains(previous_text));
    assert!(!text.contains("Source fragment"));
    let view = harness.app.controller.as_ref().unwrap();
    assert!(view.search_results().is_empty());
    assert!(view.selected_excerpt().is_none());
}

#[test]
fn search_citation_opens_a_late_fragment_and_withdraws_stale_text_in_both_body_modes() {
    for encrypted in [false, true] {
        let fixture = Fixture::new();
        let mut engine = fixture.engine();
        let opened = if encrypted {
            ready(&mut engine);
            ok(engine.execute(Command::Refresh))
        } else {
            ok(engine.execute(Command::OpenPlain))
        };
        let mut harness = Harness::new();
        harness.viewport_size = egui::vec2(760.0, 520.0);
        harness.results.send(opened).unwrap();
        let prefix = format!("\u{feff}# Synthetic\r\n{}\r\n", "前文".repeat(225));
        assert!(prefix.chars().count() > 400);
        let tail = "蓝色主题 first-tail-context\r\n";
        let body = format!("{prefix}{tail}");
        let imported = ok(engine.execute(Command::Import(
            fixture.named_input("fragment-v1.md", &body),
        )));
        let source = imported
            .view
            .as_ref()
            .unwrap()
            .selected_source_id()
            .unwrap()
            .clone();
        harness.results.send(imported).unwrap();
        let searched = ok(engine.execute(search(&mut harness, "蓝色主题")));
        let results = searched.view.as_ref().unwrap().search_results();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_id(), &source);
        assert_eq!(results[0].version().get(), 1);
        assert_eq!(
            (results[0].byte_start(), results[0].byte_end()),
            (0, body.len() as u64)
        );
        assert_eq!(results[0].content().as_str().as_bytes(), body.as_bytes());
        let lineage = results[0].lineage_id().clone();
        let fragment = results[0].fragment_id().clone();
        // Editing the draft must not move the citation from the executed query.
        harness.app.search_query = "unsubmitted draft".into();
        let selected = ok(engine.execute(click_citation(&mut harness, searched)));
        let view = selected.view.as_ref().unwrap();
        assert_eq!(view.selected_source_id(), Some(&source));
        let excerpt = view.selected_excerpt().unwrap();
        assert_eq!(excerpt.result.fragment_id(), &fragment);
        assert_eq!(excerpt.byte_start, prefix.len() as u64);
        assert_eq!(excerpt.byte_end, body.len() as u64);
        assert_eq!(excerpt.text.as_bytes(), tail.as_bytes());
        harness.results.send(selected).unwrap();
        let text = visible_text(&mut harness);
        assert!(text.contains("Source fragment"));
        assert!(text.contains("蓝色主题 first-tail-context"));
        assert!(text.contains(&format!(
            "v1 · excerpt bytes {}..{}",
            prefix.len(),
            body.len()
        )));
        harness
            .click("Previous excerpt")
            .drop_without_applying_deltas();
        harness.no_command();
        let previous_end = harness
            .app
            .controller
            .as_ref()
            .unwrap()
            .selected_excerpt()
            .unwrap()
            .byte_end;
        assert_eq!(previous_end, prefix.len() as u64);
        harness.click("Next excerpt").drop_without_applying_deltas();
        harness.no_command();
        assert!(visible_text(&mut harness).contains("蓝色主题 first-tail-context"));

        // Update with an authorized picker result and keep the old text hidden
        // during the gap between command acceptance and the refreshed snapshot.
        let revised_tail = "café second-tail-context\r\n";
        let revised = format!("{prefix}{revised_tail}");
        harness.app.send(Command::Update(
            fixture.named_input("fragment-v2.md", &revised),
        ));
        assert_search_cleared(&mut harness, "first-tail-context");
        let updated = ok(engine.execute(harness.commands.try_recv().unwrap()));
        let view = updated.view.as_ref().unwrap();
        let current = view.selected_source_id().unwrap().clone();
        assert_ne!(current, source);
        assert_eq!(view.selected_version().unwrap().version().get(), 2);
        harness.results.send(updated).unwrap();
        assert_search_cleared(&mut harness, "first-tail-context");

        let old_query = ok(engine.execute(search(&mut harness, "蓝色主题")));
        assert!(old_query.view.as_ref().unwrap().search_results().is_empty());
        harness.results.send(old_query).unwrap();
        let searched = ok(engine.execute(search(&mut harness, "CAFE")));
        let results = searched.view.as_ref().unwrap().search_results();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_id(), &current);
        assert_eq!(results[0].lineage_id(), &lineage);
        assert_ne!(results[0].fragment_id(), &fragment);
        assert_eq!(results[0].content().as_str().as_bytes(), revised.as_bytes());

        // This Unicode61 hit has no literal query occurrence. The exact
        // original bytes remain available through the fragment pager.
        let selected = ok(engine.execute(click_citation(&mut harness, searched)));
        let view = selected.view.as_ref().unwrap();
        let excerpt = view.selected_excerpt().unwrap();
        assert_eq!(excerpt.byte_start, 0);
        assert_eq!(excerpt.text.chars().count(), 400);
        assert_eq!(excerpt.result.source_id(), &current);
        let first_end = excerpt.byte_end;
        harness.results.send(selected).unwrap();
        harness.click("Next excerpt").drop_without_applying_deltas();
        harness.no_command();
        let excerpt = harness
            .app
            .controller
            .as_ref()
            .unwrap()
            .selected_excerpt()
            .unwrap();
        assert_eq!(excerpt.byte_start, first_end);
        assert!(excerpt.text.ends_with(revised_tail));
        assert!(visible_text(&mut harness).contains("café second-tail-context"));

        harness
            .click("Delete managed lineage…")
            .drop_without_applying_deltas();
        harness
            .click("Delete managed lineage")
            .drop_without_applying_deltas();
        assert_search_cleared(&mut harness, "second-tail-context");
        let command = harness.commands.try_recv().unwrap();
        assert!(matches!(command, Command::Delete));
        let deleted = ok(engine.execute(command));
        assert_eq!(
            deleted
                .last_deletion
                .as_ref()
                .unwrap()
                .params()
                .overall_status,
            DeletionOverallStatus::Completed
        );
        harness.results.send(deleted).unwrap();
        assert_search_cleared(&mut harness, "second-tail-context");
        let searched = ok(engine.execute(search(&mut harness, "CAFE")));
        assert!(searched.view.as_ref().unwrap().search_results().is_empty());
        harness.results.send(searched).unwrap();
        assert_search_cleared(&mut harness, "second-tail-context");

        let stale = engine.execute(Command::SelectResult(lineage, source, fragment));
        assert_eq!(
            stale.error.as_ref().unwrap().reason(),
            crate::DesktopErrorReason::SelectionInvalid
        );
        assert!(stale.view.is_none());
        harness.results.send(stale).unwrap();
        let text = visible_text(&mut harness);
        assert!(text.contains("Refresh the library and search again"));
        assert!(!text.contains("first-tail-context"));
        assert!(!text.contains("second-tail-context"));
        harness.click("Refresh").drop_without_applying_deltas();
        let refreshed = ok(engine.execute(harness.commands.try_recv().unwrap()));
        harness.results.send(refreshed).unwrap();
        assert_search_cleared(&mut harness, "second-tail-context");
    }
}
