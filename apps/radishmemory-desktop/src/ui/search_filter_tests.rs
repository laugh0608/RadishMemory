use super::{test_support::Harness, *};
use crate::worker::tests::{Fixture, ok, ready};

#[test]
fn search_filters_combine_and_clear_stale_results_in_both_body_modes() {
    for encrypted in [false, true] {
        let fixture = Fixture::new();
        let mut engine = fixture.engine();
        if encrypted {
            ready(&mut engine);
        } else {
            ok(engine.execute(Command::OpenPlain));
        }
        let body = "synthetic orchard";
        let imported =
            ok(engine.execute(Command::Import(fixture.named_input("orchard.txt", body))));
        let imported_id = imported.view.unwrap().selected_source_id().unwrap().clone();
        ok(engine.execute(Command::SaveNote(
            NoteFormat::Text,
            Some(NonEmptyText::new("Plain note").unwrap()),
            body.into(),
        )));
        let markdown = ok(engine.execute(Command::SaveNote(
            NoteFormat::Markdown,
            Some(NonEmptyText::new("Markdown note").unwrap()),
            body.into(),
        )));
        let markdown_id = markdown.view.unwrap().selected_source_id().unwrap().clone();

        let mut harness = Harness::new();
        harness.viewport_size = egui::vec2(760.0, 520.0);
        let mut state = ok(engine.execute(Command::Search("orchard".into())));
        assert_eq!(state.view.as_ref().unwrap().search_results().len(), 3);
        harness.app.search_query = "orchard".into();

        for (menu, selection, entry, media_type, saved_within, expected) in [
            (
                "All sources",
                "Imported files",
                Some(SourceEntry::FileImport),
                None,
                None,
                Some(&imported_id),
            ),
            (
                "All formats",
                "Markdown",
                Some(SourceEntry::FileImport),
                Some(MediaType::TextMarkdown),
                None,
                None,
            ),
            (
                "Imported files",
                "In-app notes",
                Some(SourceEntry::Note),
                Some(MediaType::TextMarkdown),
                None,
                Some(&markdown_id),
            ),
            (
                "Any save time",
                "Last 24 hours",
                Some(SourceEntry::Note),
                Some(MediaType::TextMarkdown),
                Some(SourceSavedWithin::Day),
                Some(&markdown_id),
            ),
        ] {
            harness.results.send(state).unwrap();
            harness.click(menu).drop_without_applying_deltas();
            harness.no_command();
            harness.click(selection).drop_without_applying_deltas();
            let view = harness.app.controller.as_ref().unwrap();
            assert!(view.search_results().is_empty());
            assert!(view.selected_excerpt().is_none());
            let command = harness.commands.try_recv().unwrap();
            let Command::Search(query) = &command else {
                panic!("expected filtered search")
            };
            assert_eq!(query.text, "orchard");
            assert_eq!(
                query.filters,
                SourceSearchFilters {
                    entry,
                    media_type,
                    saved_within
                }
            );
            state = ok(engine.execute(command));
            let hits = state.view.as_ref().unwrap().search_results();
            assert_eq!(
                hits.iter().map(|hit| hit.source_id()).collect::<Vec<_>>(),
                expected.into_iter().collect::<Vec<_>>()
            );
        }
    }
}
