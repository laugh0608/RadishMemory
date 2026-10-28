//! Shared headless production UI frames with controlled worker-result delivery.
use super::*;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};

pub(super) struct Harness {
    pub(super) app: RadishMemoryApp,
    pub(super) context: egui::Context,
    pub(super) results: Sender<Snapshot>,
    pub(super) commands: Receiver<Command>,
}
impl Harness {
    pub(super) fn new() -> Self {
        let (worker, results, commands) = crate::worker::tests::channel_worker();
        let context = egui::Context::default();
        crate::fonts::install(&context);
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
            context,
            results,
            commands,
        }
    }
    pub(super) fn start(&mut self) {
        self.app.send(Command::OpenEncrypted);
        assert!(matches!(
            self.commands.try_recv(),
            Ok(Command::OpenEncrypted)
        ));
        assert!(self.app.busy());
    }
    pub(super) fn frame(&mut self, close: bool, events: Vec<egui::Event>) -> egui::FullOutput {
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
    pub(super) fn no_command(&self) {
        assert!(matches!(self.commands.try_recv(), Err(TryRecvError::Empty)));
    }
    pub(super) fn click(&mut self, label: &str) -> egui::FullOutput {
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
pub(super) fn pointer(position: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
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
pub(super) fn label_position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
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
pub(super) fn cancelled(output: &egui::FullOutput) -> bool {
    output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .contains(&egui::ViewportCommand::CancelClose)
}
pub(super) fn closed(output: &egui::FullOutput) -> bool {
    output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .contains(&egui::ViewportCommand::Close)
}
pub(super) fn failure() -> DesktopError {
    DesktopError::without_source(
        crate::DesktopErrorCode::Runtime,
        crate::DesktopErrorReason::WorkerStopped,
        false,
    )
}
