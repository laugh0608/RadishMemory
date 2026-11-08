use super::{
    test_support::{Harness, painted_text},
    *,
};
use crate::controller::QuestionState;
use crate::worker::tests::{Fixture, ok, ready};

fn rendered(harness: &mut Harness) -> String {
    let frame = harness.frame(false, vec![]);
    let text = painted_text(&frame);
    frame.drop_without_applying_deltas();
    text
}

fn click_command(harness: &mut Harness, label: &str) -> Command {
    harness.click(label).drop_without_applying_deltas();
    harness.commands.try_recv().unwrap()
}

fn paste_field(harness: &mut Harness, label: &str, value: &str) {
    harness.click(label).drop_without_applying_deltas();
    harness
        .frame(false, vec![egui::Event::Paste(value.into())])
        .drop_without_applying_deltas();
}

#[test]
fn local_question_revalidates_exact_evidence_before_answer_and_open_in_both_body_modes() {
    for encrypted in [false, true] {
        let fixture = Fixture::new();
        let mut engine = fixture.engine();
        let open = || {
            if encrypted {
                Command::OpenEncrypted
            } else {
                Command::OpenPlain
            }
        };
        if encrypted {
            ready(&mut engine);
        } else {
            ok(engine.execute(open()));
        }
        let prefix = format!("# Synthetic\r\n{}\r\n", "前文".repeat(225));
        let v1 = format!("{prefix}orchard 青色方案");
        let alpha = ok(engine.execute(Command::SaveNote(
            NoteFormat::Markdown,
            Some(NonEmptyText::new("QA Alpha").unwrap()),
            v1.clone(),
        )));
        let alpha = alpha.view.unwrap();
        let alpha_lineage = alpha.selected_lineage_id().unwrap().clone();
        let alpha_v1 = alpha.selected_source_id().unwrap().clone();
        let beta = ok(engine.execute(Command::Import(
            fixture.named_input("QA Beta.txt", "orchard 合成待撤回依据"),
        )));
        let beta_lineage = beta
            .view
            .as_ref()
            .unwrap()
            .selected_lineage_id()
            .unwrap()
            .clone();
        let mut harness = Harness::new();
        harness.viewport_size = egui::vec2(760.0, 520.0);
        harness.results.send(beta).unwrap();
        harness.click("Ask locally").drop_without_applying_deltas();
        harness.no_command();
        paste_field(&mut harness, "Ask a question", "这个项目采用哪套方案？");
        paste_field(&mut harness, "Optional evidence keywords", "orchard");
        harness.app.search_filters.media_type = Some(MediaType::TextMarkdown);

        let preview = click_command(&mut harness, "Preview evidence");
        assert!(matches!(&preview, Command::PreviewQuestion(input, filters)
            if input.question == "这个项目采用哪套方案？" && input.keywords == "orchard"
                && filters.media_type == Some(MediaType::TextMarkdown)));
        let preview = ok(engine.execute(preview));
        let question = preview.view.as_ref().unwrap().question().unwrap();
        assert_eq!(question.evidence.len(), 1);
        assert_eq!(question.evidence[0].text, "orchard 青色方案");
        assert_eq!(question.evidence[0].byte_start, prefix.len() as u64);
        harness.results.send(preview).unwrap();
        assert!(rendered(&mut harness).contains("Evidence preview"));

        let answer = click_command(&mut harness, "Run local mock");
        assert!(!rendered(&mut harness).contains("青色方案"));
        let answer = ok(engine.execute(answer));
        assert!(
            matches!(&answer.view.as_ref().unwrap().question().unwrap().state,
            QuestionState::MockAnswer(quotes) if quotes == &["[1] orchard 青色方案"])
        );
        harness.results.send(answer).unwrap();
        assert!(rendered(&mut harness).contains("Mock answer · source excerpts"));
        let original = ok(engine.execute(click_command(&mut harness, "Open evidence [1]")));
        assert_eq!(original.original_offset, prefix.len());
        assert_eq!(
            original
                .original
                .as_ref()
                .unwrap()
                .params()
                .content
                .as_str(),
            v1
        );
        assert_eq!(
            original.view.as_ref().unwrap().selected_source_id(),
            Some(&alpha_v1)
        );
        harness.results.send(original).unwrap();
        assert!(rendered(&mut harness).contains("Original text"));

        harness.app.send(Command::CorrectNote(
            alpha_lineage.clone(),
            alpha_v1.clone(),
            "orchard 紫色方案".into(),
        ));
        assert!(!rendered(&mut harness).contains("青色方案"));
        let corrected = ok(engine.execute(harness.commands.try_recv().unwrap()));
        assert!(corrected.view.as_ref().unwrap().question().is_none());
        let alpha_v2 = corrected
            .view
            .as_ref()
            .unwrap()
            .selected_source_id()
            .unwrap()
            .clone();
        assert_ne!(alpha_v2, alpha_v1);
        harness.results.send(corrected).unwrap();

        harness.app.search_filters.media_type = None;
        let preview = ok(engine.execute(click_command(&mut harness, "Preview evidence")));
        let evidence = &preview.view.as_ref().unwrap().question().unwrap().evidence;
        assert_eq!(evidence.len(), 2);
        assert!(
            evidence
                .iter()
                .any(|entry| entry.version == 2 && entry.text == "orchard 紫色方案")
        );
        assert!(
            evidence
                .iter()
                .all(|entry| !entry.text.contains("青色方案"))
        );
        harness.results.send(preview).unwrap();
        let mut other = fixture.engine();
        ok(other.execute(open()));
        ok(other.execute(Command::SelectLineage(beta_lineage)));
        ok(other.execute(Command::Delete));
        let changed = ok(engine.execute(click_command(&mut harness, "Run local mock")));
        let question = changed.view.as_ref().unwrap().question().unwrap();
        assert!(matches!(question.state, QuestionState::Changed));
        assert!(question.evidence.is_empty());
        harness.results.send(changed).unwrap();
        let text = rendered(&mut harness);
        assert!(text.contains("Source evidence changed. Preview again."));
        assert!(!text.contains("合成待撤回依据"));

        let preview = ok(engine.execute(click_command(&mut harness, "Preview evidence")));
        assert_eq!(
            preview
                .view
                .as_ref()
                .unwrap()
                .question()
                .unwrap()
                .evidence
                .len(),
            1
        );
        harness.results.send(preview).unwrap();
        let answer = ok(engine.execute(click_command(&mut harness, "Run local mock")));
        assert!(
            matches!(&answer.view.as_ref().unwrap().question().unwrap().state,
            QuestionState::MockAnswer(quotes) if quotes == &["[1] orchard 紫色方案"])
        );
        harness.results.send(answer).unwrap();
        ok(other.execute(Command::SelectLineage(alpha_lineage)));
        ok(other.execute(Command::Delete));
        drop(other);
        let invalidated = ok(engine.execute(click_command(&mut harness, "Open evidence [1]")));
        assert!(invalidated.original.is_none());
        assert!(matches!(
            invalidated.view.as_ref().unwrap().question().unwrap().state,
            QuestionState::Changed
        ));
        harness.results.send(invalidated).unwrap();
        assert!(!rendered(&mut harness).contains("紫色方案"));

        let empty = ok(engine.execute(click_command(&mut harness, "Preview evidence")));
        assert!(matches!(
            empty.view.as_ref().unwrap().question().unwrap().state,
            QuestionState::Insufficient
        ));
        harness.results.send(empty).unwrap();
        assert!(rendered(&mut harness).contains("Insufficient source evidence."));
    }
}
