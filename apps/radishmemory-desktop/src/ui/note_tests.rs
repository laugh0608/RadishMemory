use super::{
    test_support::{Harness, painted_text},
    *,
};
use crate::worker::tests::{Fixture, ok, ready};
use radishmemory_application::{DeletionOverallStatus, FileExportRequest};
use std::fs;

fn painted(harness: &mut Harness) -> String {
    let frame = harness.frame(false, vec![]);
    let text = painted_text(&frame);
    frame.drop_without_applying_deltas();
    text
}

fn paste(harness: &mut Harness, text: &str) {
    harness
        .frame(false, vec![egui::Event::Paste(text.into())])
        .drop_without_applying_deltas();
}

#[test]
fn notes_can_be_written_found_corrected_and_read_from_history_in_both_body_modes() {
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

        let previous = "# Synthetic project\r\n\r\n蓝色主题 cobalt";
        let corrected = "红色主题 crimson";
        harness.click("New note…").drop_without_applying_deltas();
        harness.click("Save note").drop_without_applying_deltas();
        harness.no_command();
        harness.click("Markdown").drop_without_applying_deltas();
        harness
            .click("Optional title")
            .drop_without_applying_deltas();
        paste(&mut harness, "Synthetic project");
        harness
            .click("Enter or paste the source text")
            .drop_without_applying_deltas();
        paste(&mut harness, previous);
        harness.click("Save note").drop_without_applying_deltas();
        let command = harness.commands.try_recv().unwrap();
        assert!(
            matches!(&command, Command::SaveNote(NoteFormat::Markdown, Some(title), body)
            if title.as_str() == "Synthetic project" && body == previous)
        );
        let saved = ok(engine.execute(command));
        let version = saved.view.as_ref().unwrap().selected_version().unwrap();
        let lineage = version.lineage_id().clone();
        let base = version.source_id().clone();
        assert_eq!(version.version().get(), 1);
        assert_eq!(version.media_type().as_str(), "text/markdown");
        assert!(is_note_producer(version.producer()));
        harness.results.send(saved).unwrap();

        harness.app.search_query = "蓝色主题".into();
        harness.click("Search").drop_without_applying_deltas();
        let found = ok(engine.execute(harness.commands.try_recv().unwrap()));
        let result = &found.view.as_ref().unwrap().search_results()[0];
        assert_eq!(result.source_id(), &base);
        assert_eq!(result.content().as_str(), previous);
        let link = format!(
            "Synthetic project · v1 · bytes {}..{}",
            result.byte_start(),
            result.byte_end(),
        );
        harness.results.send(found).unwrap();
        harness.click(&link).drop_without_applying_deltas();
        let selected = ok(engine.execute(harness.commands.try_recv().unwrap()));
        assert_eq!(
            selected.view.as_ref().unwrap().selected_source_id(),
            Some(&base)
        );
        harness.results.send(selected).unwrap();

        harness
            .click("View original")
            .drop_without_applying_deltas();
        let read = ok(engine.execute(harness.commands.try_recv().unwrap()));
        let original = read.original.as_ref().unwrap().params();
        let binding = original.origin_ref.clone();
        assert_eq!(original.content.as_str(), previous);
        assert!(
            binding
                .as_ref()
                .unwrap()
                .as_str()
                .starts_with("origin-binding-")
        );
        harness.results.send(read).unwrap();
        assert!(painted(&mut harness).contains("# Synthetic project"));
        harness.click("Edit note…").drop_without_applying_deltas();
        harness
            .results
            .send(ok(engine.execute(harness.commands.try_recv().unwrap())))
            .unwrap();
        assert!(painted(&mut harness).contains("The previous text stays available"));
        match harness.app.note_draft.as_mut().unwrap() {
            NoteDraft::Correction { body, .. } => {
                assert_eq!(body, previous);
                *body = corrected.into();
            }
            NoteDraft::New { .. } => panic!("expected a correction draft"),
        }
        harness
            .click("Save correction")
            .drop_without_applying_deltas();
        assert!(!painted(&mut harness).contains("cobalt"));
        let command = harness.commands.try_recv().unwrap();
        assert!(matches!(&command, Command::CorrectNote(id, version, body)
            if id == &lineage && version == &base && body == corrected));
        let saved = ok(engine.execute(command));
        let current = saved.view.as_ref().unwrap().selected_version().unwrap();
        assert_eq!(current.lineage_id(), &lineage);
        assert_eq!(current.title().unwrap().as_str(), "Synthetic project");
        assert_eq!(current.version().get(), 2);
        assert_eq!(current.media_type().as_str(), "text/markdown");
        let new_id = current.source_id().clone();
        assert_eq!(saved.view.as_ref().unwrap().versions().len(), 2);
        harness.results.send(saved).unwrap();

        let old_query = ok(engine.execute(Command::Search("蓝色主题".into())));
        assert!(old_query.view.as_ref().unwrap().search_results().is_empty());
        let new_query = ok(engine.execute(Command::Search("红色主题".into())));
        let result = &new_query.view.as_ref().unwrap().search_results()[0];
        assert_eq!(result.source_id(), &new_id);
        assert_eq!(result.content().as_str(), corrected);
        let new_original = ok(engine.execute(Command::ReadOriginal { edit: false }));
        let source = new_original.original.as_ref().unwrap().params();
        assert_eq!(source.origin_ref, binding);
        assert_eq!(source.supersedes_source_ids, vec![base.clone()]);
        harness.results.send(new_original).unwrap();
        harness.click("v1").drop_without_applying_deltas();
        let historical = ok(engine.execute(harness.commands.try_recv().unwrap()));
        assert_eq!(
            historical.view.as_ref().unwrap().selected_source_id(),
            Some(&base)
        );
        harness.results.send(historical).unwrap();
        assert!(!painted(&mut harness).contains("Original text"));
        harness.click("Edit note…").drop_without_applying_deltas();
        harness.no_command();
        harness
            .click("View original")
            .drop_without_applying_deltas();
        let original = ok(engine.execute(harness.commands.try_recv().unwrap()));
        assert_eq!(
            original
                .original
                .as_ref()
                .unwrap()
                .params()
                .content
                .as_str(),
            previous
        );
        harness.results.send(original).unwrap();
        assert!(painted(&mut harness).contains("蓝色主题 cobalt"));

        let exports = fixture.root.join("export");
        let export = exports.join("synthetic-v1.md");
        ok(engine.execute(Command::Export(
            FileExportRequest::new(export.clone(), vec![exports]).unwrap(),
        )));
        assert_eq!(fs::read(export).unwrap(), previous.as_bytes());
        assert_eq!(fs::read_dir(fixture.root.join("input")).unwrap().count(), 0);

        drop(engine);
        let mut engine = fixture.engine();
        let reopened = ok(engine.execute(if encrypted {
            Command::OpenEncrypted
        } else {
            Command::OpenPlain
        }));
        assert_eq!(reopened.view.unwrap().selected_source_id(), Some(&new_id));
        ok(engine.execute(Command::SelectVersion(base)));
        let historical = ok(engine.execute(Command::ReadOriginal { edit: false }));
        assert_eq!(
            historical.original.unwrap().params().content.as_str(),
            previous
        );

        let deleted = ok(engine.execute(Command::Delete));
        assert_eq!(
            deleted.last_deletion.unwrap().params().overall_status,
            DeletionOverallStatus::Completed
        );
        assert!(deleted.view.unwrap().sources().is_empty());
        assert!(
            ok(engine.execute(Command::Search("红色主题".into())))
                .view
                .unwrap()
                .search_results()
                .is_empty()
        );
    }
}
