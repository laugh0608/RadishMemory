use super::{
    test_support::{Harness, painted_text},
    *,
};
use crate::worker::tests::{Fixture, ok, ready};

fn rendered(harness: &mut Harness) -> String {
    let frame = harness.frame(false, vec![]);
    let text = painted_text(&frame);
    frame.drop_without_applying_deltas();
    text
}

#[test]
fn comparison_reopens_exact_versions_and_withdraws_deleted_sources_in_both_body_modes() {
    for encrypted in [false, true] {
        let fixture = Fixture::new();
        let mut engine = fixture.engine();
        if encrypted {
            ready(&mut engine);
        } else {
            ok(engine.execute(Command::OpenPlain));
        }
        let prefix = format!("# Synthetic\r\n{}\r\n", "前文".repeat(225));
        let v1 = format!("{prefix}comparison alpha 青色方案");
        let note = ok(engine.execute(Command::SaveNote(
            NoteFormat::Markdown,
            Some(NonEmptyText::new("Comparison Alpha").unwrap()),
            v1.clone(),
        )));
        let note = note.view.unwrap();
        let version = note.selected_version().unwrap();
        let alpha_lineage = version.lineage_id().clone();
        let alpha_v1 = version.source_id().clone();
        let beta = ok(engine.execute(Command::Import(
            fixture.named_input("Comparison Beta.txt", "comparison beta 合成待删除正文"),
        )));
        let beta_lineage = beta.view.unwrap().selected_lineage_id().unwrap().clone();
        let gamma = ok(engine.execute(Command::SaveNote(
            NoteFormat::Text,
            Some(NonEmptyText::new("Comparison Gamma").unwrap()),
            "comparison gamma 合成外部删除正文".into(),
        )));
        let gamma_lineage = gamma.view.unwrap().selected_lineage_id().unwrap().clone();

        let mut harness = Harness::new();
        harness.viewport_size = egui::vec2(760.0, 520.0);
        for query in ["alpha", "beta", "gamma"] {
            let found = ok(engine.execute(Command::Search(query.into())));
            assert_eq!(found.view.as_ref().unwrap().search_results().len(), 1);
            harness.results.send(found).unwrap();
            harness
                .click("Add to comparison")
                .drop_without_applying_deltas();
            let command = harness.commands.try_recv().unwrap();
            assert!(matches!(command, Command::AddComparison(_)));
            harness.results.send(ok(engine.execute(command))).unwrap();
            rendered(&mut harness);
        }
        let text = rendered(&mut harness);
        assert!(text.contains("Compare (3)"));
        assert!(text.contains("青色方案"));
        assert!(text.contains("合成待删除正文"));
        assert!(text.contains("合成外部删除正文"));

        harness
            .click("Open citation 1")
            .drop_without_applying_deltas();
        let opened = ok(engine.execute(harness.commands.try_recv().unwrap()));
        assert_eq!(
            opened.view.as_ref().unwrap().selected_source_id(),
            Some(&alpha_v1)
        );
        assert_eq!(opened.original_offset, prefix.len() + "comparison ".len());
        assert_eq!(
            opened.original.as_ref().unwrap().params().content.as_str(),
            v1
        );
        harness.results.send(opened).unwrap();
        assert!(rendered(&mut harness).contains("Original text"));

        harness.app.send(Command::CorrectNote(
            alpha_lineage,
            alpha_v1.clone(),
            "comparison alpha 紫色方案".into(),
        ));
        assert!(!rendered(&mut harness).contains("青色方案"));
        let corrected = ok(engine.execute(harness.commands.try_recv().unwrap()));
        assert!(
            corrected.view.as_ref().unwrap().comparison()[0]
                .excerpt
                .as_ref()
                .unwrap()
                .historical
        );
        let new_id = corrected
            .view
            .as_ref()
            .unwrap()
            .selected_source_id()
            .unwrap()
            .clone();
        assert_ne!(new_id, alpha_v1);
        harness.results.send(corrected).unwrap();
        let search = ok(engine.execute(Command::Search("alpha".into())));
        assert_eq!(
            search.view.as_ref().unwrap().search_results()[0].source_id(),
            &new_id
        );
        harness.results.send(search).unwrap();
        let text = rendered(&mut harness);
        assert!(text.contains("Historical"));
        assert!(text.contains("青色方案"));

        harness
            .click("Open citation 1")
            .drop_without_applying_deltas();
        let reopened = ok(engine.execute(harness.commands.try_recv().unwrap()));
        assert_eq!(
            reopened.view.as_ref().unwrap().selected_source_id(),
            Some(&alpha_v1)
        );
        assert_eq!(
            reopened
                .original
                .as_ref()
                .unwrap()
                .params()
                .content
                .as_str(),
            v1
        );
        harness.results.send(reopened).unwrap();
        rendered(&mut harness);

        let selected = ok(engine.execute(Command::SelectLineage(beta_lineage)));
        harness.results.send(selected).unwrap();
        harness
            .click("Delete managed lineage…")
            .drop_without_applying_deltas();
        harness
            .click("Delete managed lineage")
            .drop_without_applying_deltas();
        assert!(!rendered(&mut harness).contains("合成待删除正文"));
        let deleted = ok(engine.execute(harness.commands.try_recv().unwrap()));
        assert!(
            deleted.view.as_ref().unwrap().comparison()[1]
                .excerpt
                .is_none()
        );
        harness.results.send(deleted).unwrap();
        assert!(!rendered(&mut harness).contains("Comparison Beta.txt"));

        let mut other = fixture.engine();
        ok(other.execute(if encrypted {
            Command::OpenEncrypted
        } else {
            Command::OpenPlain
        }));
        ok(other.execute(Command::SelectLineage(gamma_lineage)));
        ok(other.execute(Command::Delete));
        drop(other);
        harness
            .click("Refresh comparison")
            .drop_without_applying_deltas();
        assert!(!rendered(&mut harness).contains("合成外部删除正文"));
        let refreshed = ok(engine.execute(harness.commands.try_recv().unwrap()));
        assert!(
            refreshed.view.as_ref().unwrap().comparison()[2]
                .excerpt
                .is_none()
        );
        harness.results.send(refreshed).unwrap();
        let text = rendered(&mut harness);
        assert_eq!(text.matches("Source unavailable").count(), 2);
        assert!(text.contains("Comparison Alpha"));
        assert!(!text.contains("Comparison Gamma"));
        harness
            .click("Open citation 3")
            .drop_without_applying_deltas();
        harness.no_command();
        harness.click("Remove 3").drop_without_applying_deltas();
        harness
            .results
            .send(ok(engine.execute(harness.commands.try_recv().unwrap())))
            .unwrap();
        assert!(rendered(&mut harness).contains("Compare (2)"));
    }
}
