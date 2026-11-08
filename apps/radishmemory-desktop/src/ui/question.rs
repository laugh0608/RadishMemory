use super::{Command, LibraryView, QuestionInput, UiAction, egui, source_label};
use crate::controller::{QUESTION_INPUT_LIMIT, QuestionState};

pub(super) fn panel(
    ui: &mut egui::Ui,
    view: &LibraryView,
    input: &mut QuestionInput,
    busy: bool,
    holds_original: bool,
    action: &mut Option<UiAction>,
) {
    ui.heading("Local Q&A prototype");
    ui.small("Mock answers quote source text. Preview uses the current search filters.");
    let changed_question = ui
        .add(
            egui::TextEdit::singleline(&mut input.question)
                .hint_text("Ask a question")
                .char_limit(QUESTION_INPUT_LIMIT)
                .desired_width(f32::INFINITY),
        )
        .changed();
    let changed_keywords = ui
        .add(
            egui::TextEdit::singleline(&mut input.keywords)
                .hint_text("Optional evidence keywords")
                .char_limit(QUESTION_INPUT_LIMIT)
                .desired_width(f32::INFINITY),
        )
        .changed();
    if (changed_question || changed_keywords) && view.question().is_some() {
        *action = Some(UiAction::Command(Command::DismissQuestion));
    }
    if ui
        .add_enabled(
            !holds_original && input.is_valid(),
            egui::Button::new("Preview evidence"),
        )
        .clicked()
    {
        *action = Some(UiAction::PreviewQuestion);
    }
    if busy || holds_original {
        ui.label(if busy {
            "Checking source evidence…"
        } else {
            "Resolve the pending library operation to ask a question."
        });
        return;
    }
    let Some(question) = view.question().filter(|question| &question.input == input) else {
        ui.label("Preview evidence to check the current sources.");
        return;
    };
    egui::ScrollArea::vertical()
        .id_salt("local-question")
        .show(ui, |ui| {
            match &question.state {
                QuestionState::Insufficient => {
                    ui.label("Insufficient source evidence.");
                }
                QuestionState::Changed => {
                    ui.label("Source evidence changed. Preview again.");
                }
                QuestionState::Preview => {
                    ui.strong("Evidence preview");
                    if ui.button("Run local mock").clicked() {
                        *action = Some(UiAction::Command(Command::RunLocalAnswer));
                    }
                }
                QuestionState::MockAnswer(_) => {
                    ui.strong("Mock answer · source excerpts");
                }
            }
            for (index, evidence) in question.evidence.iter().enumerate() {
                ui.group(|ui| {
                    ui.strong(format!(
                        "[{}] {}",
                        index + 1,
                        source_label(evidence.title.as_ref())
                    ));
                    ui.small(format!(
                        "v{} · bytes {}..{}",
                        evidence.version, evidence.byte_start, evidence.byte_end
                    ));
                    let text = match &question.state {
                        QuestionState::MockAnswer(answer) => &answer[index],
                        _ => &evidence.text,
                    };
                    ui.label(text);
                    if ui
                        .button(format!("Open evidence [{}]", index + 1))
                        .clicked()
                    {
                        *action = Some(UiAction::Command(Command::OpenQuestionSource(index)));
                    }
                });
            }
        });
}
