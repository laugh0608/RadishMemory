use crate::controller::LibraryView;
use crate::worker::{Command, Snapshot, Worker};
use crate::{DesktopError, NativeFilePicker, PickerOutcome};
use eframe::egui;
use radishmemory_application::{Identifier, SourceLineageSummary};

pub struct RadishMemoryApp {
    worker: Option<Worker>,
    controller: Option<LibraryView>,
    state: Snapshot,
    search_query: String,
    notice: Option<Notice>,
    refresh_notice: Option<String>,
    last_deletion: Option<radishmemory_application::DeletionEvidence>,
    confirm_delete: bool,
    confirmation: Option<Confirmation>,
    allow_close: bool,
}
#[derive(Clone, Copy)]
enum Confirmation {
    Initialize,
    Migrate,
    Abandon,
    Close,
}

impl RadishMemoryApp {
    #[must_use]
    pub fn bootstrap() -> Self {
        let mut app = Self {
            worker: None,
            controller: None,
            state: Snapshot::default(),
            search_query: String::new(),
            notice: None,
            refresh_notice: None,
            last_deletion: None,
            confirm_delete: false,
            confirmation: None,
            allow_close: false,
        };
        match Worker::start() {
            Ok(worker) => {
                app.worker = Some(worker);
                app.send(Command::OpenPlain);
            }
            Err(error) => app.notice = Some(Notice::error(&error)),
        }
        app
    }
    fn can_mutate(&self) -> bool {
        self.state.recovery_known
            && !self.state.holds_original
            && !self.state.abandonment_available
            && self.state.deletion_requests.is_empty()
    }
    fn busy(&self) -> bool {
        self.worker.as_ref().is_some_and(|w| w.busy)
    }
    fn send(&mut self, command: Command) {
        if let Some(worker) = &mut self.worker {
            match worker.send(command) {
                Ok(()) => {
                    self.notice = Some(Notice::neutral("Working…"));
                    self.refresh_notice = None;
                }
                Err(error) => self.notice = Some(Notice::error(&error)),
            }
        }
    }
    fn poll(&mut self) {
        let Some(mut state) = self.worker.as_mut().and_then(Worker::poll) else {
            return;
        };
        self.controller = state.view.take();
        self.notice = if let Some(error) = state.error.take() {
            Some(Notice::error(&error))
        } else {
            state.message.take().map(|message| Notice {
                kind: NoticeKind::Success,
                message,
            })
        };
        self.refresh_notice = state.refresh_error.take().map(|error| format!("The operation completed, but the view could not refresh: {}. Use Refresh; do not repeat the write.", redacted_error(&error)));
        if let Some(evidence) = state.last_deletion.take() {
            self.last_deletion = Some(evidence);
        }
        self.state = state;
    }
    fn execute(&mut self, action: UiAction) {
        match action {
            UiAction::RetryStartup => self.send(Command::OpenPlain),
            UiAction::Import | UiAction::Update => match NativeFilePicker::pick_import() {
                Ok(PickerOutcome::Selected(request)) => {
                    self.send(if matches!(action, UiAction::Import) {
                        Command::Import(request)
                    } else {
                        Command::Update(request)
                    })
                }
                Ok(PickerOutcome::Cancelled) => {
                    self.notice = Some(Notice::neutral("Selection cancelled. No library changes."))
                }
                Err(error) => self.notice = Some(Notice::error(&error)),
            },
            UiAction::Export => {
                let name = self
                    .controller
                    .as_ref()
                    .and_then(LibraryView::selected_version)
                    .and_then(|v| v.title())
                    .map(|t| t.as_str().to_owned());
                match NativeFilePicker::pick_export(name.as_deref()) {
                    Ok(PickerOutcome::Selected(request)) => self.send(Command::Export(request)),
                    Ok(PickerOutcome::Cancelled) => {
                        self.notice = Some(Notice::neutral("Export cancelled."))
                    }
                    Err(error) => self.notice = Some(Notice::error(&error)),
                }
            }
            UiAction::Verify => self.send(Command::Verify),
            UiAction::Rebuild => self.send(Command::Rebuild),
            UiAction::Search => self.send(Command::Search(self.search_query.clone())),
            UiAction::SelectLineage(id) => self.send(Command::SelectLineage(id)),
            UiAction::SelectVersion(id) => self.send(Command::SelectVersion(id)),
            UiAction::SelectSearchResult {
                lineage_id,
                source_id,
            } => self.send(Command::SelectResult(lineage_id, source_id)),
            UiAction::RequestDelete => self.confirm_delete = true,
            UiAction::CancelDelete => self.confirm_delete = false,
            UiAction::ConfirmDelete => {
                self.confirm_delete = false;
                self.send(Command::Delete);
            }
            UiAction::Command(command) => self.send(command),
            UiAction::Confirm(confirmation) => self.confirmation = Some(confirmation),
        }
    }
}

impl eframe::App for RadishMemoryApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        let context = ui.ctx().clone();
        if self.busy() {
            context.request_repaint_after(std::time::Duration::from_millis(80));
        }
        if context.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.busy() {
                context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.notice = Some(Notice::neutral(
                    "An operation is still running. Wait for its real result before closing.",
                ));
            } else if self.state.holds_original {
                context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirmation = Some(Confirmation::Close);
            }
        }
        let can_mutate = self.can_mutate();
        let mut action = None;
        egui::CentralPanel::default().show(ui, |ui| {
            if self.confirmation.is_some() || self.confirm_delete { ui.disable(); }
            if self.busy() { ui.spinner(); ui.label("Library worker is busy; repeated actions are disabled."); ui.disable(); }
            if self.worker.as_ref().is_none_or(|w| w.stopped) { ui.label("Worker unavailable. Restart the app; no success is assumed."); ui.disable(); }
            ui.horizontal_wrapped(|ui| {
                ui.heading("RadishMemory");
                ui.separator();
                ui.label("Local-only text library");
                if self.controller.is_some() && ui.add_enabled(can_mutate, egui::Button::new("Import file…")).clicked() { action = Some(UiAction::Import); }
                if ui.button("Refresh").clicked() { action = Some(UiAction::Command(Command::Refresh)); }
                if ui.button("Verify").clicked() { action = Some(UiAction::Verify); }
                if ui.button("Rebuild recall").clicked() { action = Some(UiAction::Rebuild); }
            });
            ui.horizontal_wrapped(|ui| {
                let can_switch = !self.state.holds_original;
                if ui.add_enabled(can_switch, egui::Button::new("Open legacy plaintext")).clicked() { action = Some(UiAction::RetryStartup); }
                if ui.add_enabled(can_switch, egui::Button::new("Open encrypted objects")).clicked() { action = Some(UiAction::Command(Command::OpenEncrypted)); }
                if ui.add_enabled(can_switch, egui::Button::new("Prepare key…")).clicked() { action = Some(UiAction::Confirm(Confirmation::Initialize)); }
                if ui.add_enabled(can_switch, egui::Button::new("Migrate / resume bodies…")).clicked() { action = Some(UiAction::Confirm(Confirmation::Migrate)); }
            });
            ui.small(if self.state.encrypted { "Encrypted object mode: FTS retains full readable text. This is not whole-library encryption." } else { "Default legacy mode: SQLite stores plaintext source bodies." });
            if self.state.encrypted {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Inspect recovery").clicked() { action = Some(UiAction::Command(Command::InspectRecovery)); }
                    if self.state.holds_original && ui.button("Retry original request").clicked() { action = Some(UiAction::Command(Command::RetryOriginal)); }
                    if self.state.abandonment_available && ui.button("Abandon inspected capture…").clicked() { action = Some(UiAction::Confirm(Confirmation::Abandon)); }
                });
                if self.state.holds_capture { ui.label("Original plaintext snapshot is held in worker memory. Retry uses the same bytes and provenance."); }
                if !self.state.recovery_known { ui.label("Recovery state is not verified. New mutations are disabled; inspect recovery or reopen after repair."); }
                if self.state.abandonment_available && !self.state.holds_capture { ui.label("An unfinished capture exists, but its original snapshot is unavailable. It cannot be reconstructed from the selected file. Inspect and explicitly abandon to unblock it."); }
                for id in &self.state.deletion_requests {
                    if ui.add_enabled(!self.state.holds_original, egui::Button::new(format!("Continue persisted deletion {}", id.as_str()))).clicked() { action = Some(UiAction::Command(Command::ResumeDelete(id.clone()))); }
                }
            }
            if let Some(notice) = &self.notice {
                ui.colored_label(notice.color(), &notice.message);
            }
            if let Some(message) = &self.refresh_notice { ui.colored_label(egui::Color32::YELLOW, message); }
            ui.separator();

            if let Some(evidence) = &self.last_deletion {
                ui.label(format!("Latest local deletion: {:?}; {} real component results. Originals, exports and backups are outside this receipt.", evidence.params().overall_status, evidence.params().component_results.len()));
            }
            let Some(controller) = self.controller.as_ref() else {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.heading("Local library unavailable");
                    ui.label("Library data is unavailable. Use recovery, Refresh, or an explicit open action after preparation or repair.");
                    ui.add_space(12.0);
                    if ui.add_enabled(!self.state.holds_original, egui::Button::new(if self.state.encrypted { "Retry encrypted open" } else { "Retry legacy open" })).clicked() {
                        action = Some(UiAction::Command(if self.state.encrypted { Command::OpenEncrypted } else { Command::OpenPlain }));
                    }
                });
                return;
            };

            ui.columns(2, |columns| {
                let (source_columns, content_columns) = columns.split_at_mut(1);
                let source_ui = &mut source_columns[0];
                source_ui.heading("Sources");
                source_ui.label(format!("{} active lineage(s)", controller.sources().len()));
                source_ui.separator();
                egui::ScrollArea::vertical().show(source_ui, |ui| {
                    for source in controller.sources() {
                        let selected =
                            controller.selected_lineage_id() == Some(source.lineage_id());
                        if ui.selectable_label(selected, source_label(source)).clicked() {
                            action = Some(UiAction::SelectLineage(source.lineage_id().clone()));
                        }
                        ui.small(format!(
                            "v{} · {} · {}",
                            source.current_version().get(),
                            format_bytes(source.content_length()),
                            source.captured_at().original()
                        ));
                        ui.add_space(6.0);
                    }
                });

                let content_ui = &mut content_columns[0];
                content_ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Search managed text locally")
                            .desired_width(f32::INFINITY),
                    );
                    if ui.button("Search").clicked()
                        || (response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                    {
                        action = Some(UiAction::Search);
                    }
                });
                content_ui.separator();

                if let Some(source) = controller.selected_lineage() {
                    content_ui.heading(source_label(source));
                    content_ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "Current version: {}",
                            source.current_version().get()
                        ));
                        ui.label(format!(
                            "Managed bytes: {}",
                            format_bytes(source.content_length())
                        ));
                        ui.label(format!("Versions: {}", source.version_count()));
                    });
                    content_ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(can_mutate, egui::Button::new("Update from file…")).clicked() {
                            action = Some(UiAction::Update);
                        }
                        if ui.button("Export selected version…").clicked() {
                            action = Some(UiAction::Export);
                        }
                        if ui.add_enabled(can_mutate, egui::Button::new("Delete managed lineage…")).clicked() {
                            action = Some(UiAction::RequestDelete);
                        }
                    });
                    content_ui.add_space(10.0);
                    content_ui.strong("Version history");
                    content_ui.horizontal_wrapped(|ui| {
                        for version in controller.versions() {
                            let selected =
                                controller.selected_source_id() == Some(version.source_id());
                            let suffix = if version.current() { " (current)" } else { "" };
                            if ui
                                .selectable_label(
                                    selected,
                                    format!("v{}{}", version.version().get(), suffix),
                                )
                                .clicked()
                            {
                                action = Some(UiAction::SelectVersion(version.source_id().clone()));
                            }
                        }
                    });
                    if let Some(version) = controller.selected_version() {
                        content_ui.small(format!(
                            "Captured {} · {} · {:?}",
                            version.captured_at().original(),
                            format_bytes(version.content_length()),
                            version.media_type()
                        ));
                    }
                } else {
                    content_ui.heading("No managed sources");
                    content_ui
                        .label("Choose “Import file…” to add one UTF-8 .txt or .md file.");
                }

                content_ui.add_space(16.0);
                content_ui.separator();
                content_ui.heading("Search results");
                if controller.search_results().is_empty() {
                    content_ui.label("No local search results to show.");
                } else {
                    egui::ScrollArea::vertical().show(content_ui, |ui| {
                        for result in controller.search_results() {
                            ui.group(|ui| {
                                let title = result
                                    .title()
                                    .map_or("Untitled source", |title| title.as_str());
                                if ui
                                    .link(format!(
                                        "{title} · v{} · bytes {}..{}",
                                        result.version().get(),
                                        result.byte_start(),
                                        result.byte_end()
                                    ))
                                    .clicked()
                                {
                                    action = Some(UiAction::SelectSearchResult {
                                        lineage_id: result.lineage_id().clone(),
                                        source_id: result.source_id().clone(),
                                    });
                                }
                                ui.label(content_preview(result.content().as_str()));
                                ui.small(format!(
                                    "source {} · fragment {}",
                                    result.source_id().as_str(),
                                    result.fragment_id().as_str()
                                ));
                            });
                            ui.add_space(6.0);
                        }
                    });
                }

                if let Some(evidence) = &self.last_deletion {
                    content_ui.add_space(16.0);
                    content_ui.separator();
                    content_ui.heading("Latest deletion evidence");
                    content_ui.label(format!(
                        "Local device scope · {:?} · {} component result(s)",
                        evidence.params().overall_status,
                        evidence.params().component_results.len()
                    ));
                    content_ui.small(format!(
                        "Evidence {} · request {}",
                        evidence.params().deletion_evidence_id.as_str(),
                        evidence.params().delete_request_id.as_str()
                    ));
                    for result in &evidence.params().component_results {
                        let result = result.params();
                        content_ui.monospace(format!(
                            "{} · {:?} · {:?} · {}/{}",
                            result.component_key.as_str(),
                            result.status,
                            result.outcome,
                            result.processed_count,
                            result.target_count
                        ));
                        if let Some(error_code) = &result.error_code {
                            content_ui.small(format!(
                                "stable error {} · retryable={}",
                                error_code.as_str(),
                                result.retryable.unwrap_or(false)
                            ));
                        }
                    }
                    content_ui.small(
                        "This evidence covers only RadishMemory-managed local facts and derived data; it does not delete the original file, exports, backups, or other devices.",
                    );
                }
            });

            if self.confirm_delete {
                let context = ui.ctx().clone();
                egui::Window::new("Delete managed lineage")
                    .collapsible(false)
                    .resizable(false)
                    .show(&context, |ui| {
                        ui.label("All managed versions and active memory dependencies will be closed and purged locally.");
                        ui.label("The original selected file and prior exports are not deleted.");
                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                action = Some(UiAction::CancelDelete);
                            }
                            if ui.button("Delete managed lineage").clicked() {
                                action = Some(UiAction::ConfirmDelete);
                            }
                        });
                    });
            }
        });

        if let Some(confirmation) = self.confirmation {
            egui::Window::new("Confirm explicit action").collapsible(false).resizable(false).show(&context, |ui| {
                ui.label(match confirmation {
                    Confirmation::Initialize => "Prepare a device-local key checkpoint for this library. This may write the system credential store. An existing missing key is never replaced. Continue with explicit body migration afterward.",
                    Confirmation::Migrate => "Close the current library and migrate or resume its source bodies using the existing key. FTS remains plaintext. Do not delete the key afterward; it is required to reopen encrypted objects.",
                    Confirmation::Abandon => "Permanently abandon exactly the inspected uncommitted capture. Its original request cannot be replayed afterward. Existing committed sources and unknown files are not targets.",
                    Confirmation::Close => "Closing loses the original request held in memory. An unfinished capture then requires explicit abandonment or the exact original request from elsewhere. Persisted deletion authority can be discovered after restart.",
                });
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() { self.confirmation = None; }
                    if ui.button("Confirm").clicked() {
                        self.confirmation = None;
                        match confirmation {
                            Confirmation::Initialize => action = Some(UiAction::Command(Command::InitializeKey)),
                            Confirmation::Migrate => action = Some(UiAction::Command(Command::Migrate)),
                            Confirmation::Abandon => action = Some(UiAction::Command(Command::Abandon)),
                            Confirmation::Close => { self.allow_close = true; context.send_viewport_cmd(egui::ViewportCommand::Close); }
                        }
                    }
                });
            });
        }
        if let Some(action) = action {
            self.execute(action);
        }
    }
}

fn source_label(source: &SourceLineageSummary) -> String {
    source.title().map_or_else(
        || "Untitled source".to_owned(),
        |title| title.as_str().to_owned(),
    )
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    }
}

fn content_preview(content: &str) -> String {
    const MAX_CHARACTERS: usize = 400;
    let mut preview = content.chars().take(MAX_CHARACTERS).collect::<String>();
    if content.chars().count() > MAX_CHARACTERS {
        preview.push('…');
    }
    preview
}

fn redacted_error(error: &DesktopError) -> String {
    let mut summary = format!("{:?} / {:?}", error.code(), error.reason());
    if let Some(application) = error.application_failure() {
        summary.push_str(&format!(
            " · {:?} / {:?} / {:?}",
            application.operation(),
            application.code(),
            application.reason()
        ));
    }
    if let Some(code) = error
        .application_failure()
        .and_then(|failure| failure.vault_code())
    {
        summary.push_str(&format!(" · vault={code:?}"));
    }
    if let Some(database) = error.vault_database() {
        summary.push_str(&format!(" · database={database}"));
    }
    if let Some(os_error_code) = error.os_error_code() {
        summary.push_str(&format!(" · os_error={os_error_code}"));
    }
    summary
}

enum UiAction {
    Command(Command),
    Confirm(Confirmation),
    RetryStartup,
    Import,
    Update,
    Export,
    Verify,
    Rebuild,
    Search,
    SelectLineage(Identifier),
    SelectVersion(Identifier),
    SelectSearchResult {
        lineage_id: Identifier,
        source_id: Identifier,
    },
    RequestDelete,
    CancelDelete,
    ConfirmDelete,
}

enum NoticeKind {
    Success,
    Neutral,
    Error,
}

struct Notice {
    kind: NoticeKind,
    message: String,
}

impl Notice {
    fn neutral(message: &'static str) -> Self {
        Self {
            kind: NoticeKind::Neutral,
            message: message.to_owned(),
        }
    }

    fn error(error: &DesktopError) -> Self {
        Self {
            kind: NoticeKind::Error,
            message: redacted_error(error),
        }
    }

    fn color(&self) -> egui::Color32 {
        match self.kind {
            NoticeKind::Success => egui::Color32::from_rgb(52, 140, 90),
            NoticeKind::Neutral => egui::Color32::from_rgb(190, 140, 45),
            NoticeKind::Error => egui::Color32::from_rgb(190, 65, 65),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_preview_respects_unicode_boundaries() {
        let content = "萝".repeat(401);
        let preview = content_preview(&content);
        assert_eq!(preview.chars().count(), 401);
        assert!(preview.ends_with('…'));
    }

    #[test]
    fn redacted_error_contains_only_stable_failure_facts() {
        let error = DesktopError::without_source(
            crate::DesktopErrorCode::HostProfile,
            crate::DesktopErrorReason::ProfileInvalid,
            false,
        );
        assert_eq!(redacted_error(&error), "HostProfile / ProfileInvalid");
    }
}
