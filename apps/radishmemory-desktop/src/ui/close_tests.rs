//! Headless production UI frames with controlled worker-result delivery.
use super::*;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};

struct Harness {
    app: RadishMemoryApp,
    context: egui::Context,
    results: Sender<Snapshot>,
    commands: Receiver<Command>,
}
impl Harness {
    fn new() -> Self {
        let (worker, results, commands) = crate::worker::tests::channel_worker();
        Self {
            app: RadishMemoryApp {
                worker: Some(worker),
                controller: None,
                state: Snapshot::default(),
                search_query: String::new(),
                notice: None,
                refresh_notice: None,
                last_deletion: None,
                confirm_delete: false,
                confirmation: None,
                allow_close: false,
                deferred_close_frame: None,
                acceptance: true,
            },
            context: egui::Context::default(),
            results,
            commands,
        }
    }
    fn start(&mut self) {
        self.app.send(Command::OpenEncrypted);
        assert!(matches!(
            self.commands.try_recv(),
            Ok(Command::OpenEncrypted)
        ));
        assert!(self.app.busy());
    }
    fn frame(&mut self, close: bool, events: Vec<egui::Event>) -> egui::FullOutput {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1080.0, 720.0),
            )),
            events,
            ..Default::default()
        };
        if close {
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .events
                .push(egui::ViewportEvent::Close);
        }
        let mut output = self.context.run_ui(input, |ui| self.app.render(ui));
        // No renderer consumes textures here; clear before assertions can unwind.
        output.textures_delta.clear();
        output
    }
    fn no_command(&self) {
        assert!(matches!(self.commands.try_recv(), Err(TryRecvError::Empty)));
    }
    fn click(&mut self, label: &str) -> egui::FullOutput {
        // Let the confirmation window settle before using its painted label position.
        self.frame(false, vec![]).drop_without_applying_deltas();
        let output = self.frame(false, vec![]);
        let position = label_position(&output, label);
        output.drop_without_applying_deltas();
        self.frame(false, pointer(position, true))
            .drop_without_applying_deltas();
        self.frame(false, pointer(position, false))
    }
}
fn pointer(position: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(position),
        egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}
fn label_position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() / 2.0)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing painted label: {label}"))
}
fn cancelled(output: &egui::FullOutput) -> bool {
    output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .contains(&egui::ViewportCommand::CancelClose)
}
fn closed(output: &egui::FullOutput) -> bool {
    output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .contains(&egui::ViewportCommand::Close)
}
fn failure() -> DesktopError {
    DesktopError::without_source(
        crate::DesktopErrorCode::Runtime,
        crate::DesktopErrorReason::WorkerStopped,
        false,
    )
}

#[test]
fn busy_close_is_cancelled_without_shutdown_or_second_command() {
    let mut h = Harness::new();
    h.start();
    for _ in 0..2 {
        let output = h.frame(true, vec![]);
        assert!(cancelled(&output));
        assert!(!closed(&output));
        output.drop_without_applying_deltas();
        assert!(h.app.busy());
        assert!(!h.app.allow_close);
        assert!(h.app.confirmation.is_none());
        h.no_command();
    }
}

#[test]
fn close_with_committed_result_preserves_success_and_refresh_warning() {
    let mut h = Harness::new();
    h.start();
    h.results
        .send(Snapshot {
            message: Some("Capture committed; original request released.".into()),
            refresh_error: Some(failure()),
            ..Snapshot::default()
        })
        .unwrap();
    let output = h.frame(true, vec![]);
    assert!(cancelled(&output));
    output.drop_without_applying_deltas();
    assert!(!h.app.busy());
    assert!(!h.app.allow_close);
    assert!(!h.app.state.holds_original);
    assert!(matches!(
        h.app.notice.as_ref().unwrap().kind,
        NoticeKind::Success
    ));
    assert!(h.app.notice.as_ref().unwrap().message.contains("committed"));
    assert!(
        h.app
            .refresh_notice
            .as_ref()
            .unwrap()
            .contains("do not repeat the write")
    );
    // Completion must not turn a cancelled request into an automatic later close.
    let output = h.frame(false, vec![]);
    assert!(!closed(&output));
    output.drop_without_applying_deltas();
    assert!(!h.app.allow_close);
    let output = h.frame(true, vec![]);
    assert!(!cancelled(&output));
    output.drop_without_applying_deltas();
    assert!(h.app.allow_close);
    h.no_command();
}

#[test]
fn close_with_failed_original_requires_fresh_close_and_explicit_confirmation() {
    let mut h = Harness::new();
    h.start();
    h.results
        .send(Snapshot {
            error: Some(failure()),
            holds_original: true,
            holds_capture: true,
            ..Snapshot::default()
        })
        .unwrap();
    let output = h.frame(true, vec![]);
    assert!(cancelled(&output));
    output.drop_without_applying_deltas();
    assert!(h.app.confirmation.is_none());
    assert!(matches!(
        h.app.notice.as_ref().unwrap().kind,
        NoticeKind::Error
    ));
    let output = h.frame(true, vec![]);
    assert!(cancelled(&output));
    output.drop_without_applying_deltas();
    assert!(matches!(h.app.confirmation, Some(Confirmation::Close)));
    let output = h.click("Cancel");
    assert!(!closed(&output));
    output.drop_without_applying_deltas();
    assert!(h.app.confirmation.is_none());
    assert!(h.app.state.holds_capture);
    assert!(!h.app.allow_close);
    h.frame(true, vec![]).drop_without_applying_deltas();
    let output = h.click("Confirm");
    assert!(closed(&output));
    output.drop_without_applying_deltas();
    assert!(h.app.allow_close);
    h.no_command();
}

#[test]
fn close_with_worker_disconnect_shows_failure_before_accepting_another_close() {
    let mut h = Harness::new();
    h.start();
    // Replace and drop the only sender feeding this worker.
    let (unused, _) = std::sync::mpsc::channel();
    drop(std::mem::replace(&mut h.results, unused));
    let output = h.frame(true, vec![]);
    assert!(cancelled(&output));
    output.drop_without_applying_deltas();
    assert!(h.app.worker.as_ref().unwrap().stopped);
    assert!(
        h.app
            .notice
            .as_ref()
            .unwrap()
            .message
            .contains("WorkerStopped")
    );
    assert!(!h.app.allow_close);
    h.no_command();
}

#[test]
fn accepted_idle_close_does_not_dispatch_a_simultaneous_button_release() {
    let mut h = Harness::new();
    let output = h.frame(false, vec![]);
    let position = label_position(&output, "Refresh");
    output.drop_without_applying_deltas();
    h.frame(false, pointer(position, true))
        .drop_without_applying_deltas();
    let output = h.frame(true, pointer(position, false));
    assert!(!cancelled(&output));
    output.drop_without_applying_deltas();
    assert!(h.app.allow_close);
    assert!(!h.app.busy());
    h.no_command();
}

#[test]
fn close_cancellation_survives_multiple_layout_passes() {
    let mut h = Harness::new();
    h.start();
    h.results
        .send(Snapshot {
            message: Some("Finished".into()),
            ..Snapshot::default()
        })
        .unwrap();
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let mut passes = 0;
    let mut output = h.context.run_ui(input, |ui| {
        h.app.render(ui);
        passes += 1;
        if passes == 1 {
            ui.ctx().request_discard("exercise another layout pass");
        }
    });
    output.textures_delta.clear();
    assert!(passes >= 2);
    assert!(cancelled(&output));
    output.drop_without_applying_deltas();
    assert!(!h.app.allow_close);
    assert_eq!(h.app.notice.as_ref().unwrap().message, "Finished");
    h.no_command();
}
