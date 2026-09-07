use eframe::egui;
use radishmemory_application::{
    DeletionOverallStatus, FileCaptureOutcome, Identifier, SourceLineageSummary,
    SourceVersionSummary,
};

use crate::{
    ApplicationPaths, DesktopError, LibraryController, NativeFilePicker, PickerOutcome,
    ProductionRuntime,
};

pub struct RadishMemoryApp {
    controller: Option<LibraryController<ProductionRuntime>>,
    startup_error: Option<DesktopError>,
    search_query: String,
    notice: Option<Notice>,
    last_deletion: Option<radishmemory_application::DeletionEvidence>,
    confirm_delete: bool,
}

impl RadishMemoryApp {
    #[must_use]
    pub fn bootstrap() -> Self {
        match open_production_controller() {
            Ok(controller) => Self {
                controller: Some(controller),
                startup_error: None,
                search_query: String::new(),
                notice: None,
                last_deletion: None,
                confirm_delete: false,
            },
            Err(error) => Self {
                controller: None,
                startup_error: Some(error),
                search_query: String::new(),
                notice: None,
                last_deletion: None,
                confirm_delete: false,
            },
        }
    }

    fn retry_startup(&mut self) {
        match open_production_controller() {
            Ok(controller) => {
                self.controller = Some(controller);
                self.startup_error = None;
                self.notice = Some(Notice::success("Local library opened."));
            }
            Err(error) => {
                self.startup_error = Some(error);
                self.notice = None;
            }
        }
    }

    fn execute(&mut self, action: UiAction) {
        match action {
            UiAction::RetryStartup => self.retry_startup(),
            UiAction::Import => self.import_source(),
            UiAction::Update => self.update_source(),
            UiAction::Export => self.export_source(),
            UiAction::Verify => self.verify_library(),
            UiAction::Rebuild => self.rebuild_library(),
            UiAction::Search => self.search(),
            UiAction::SelectLineage(lineage_id) => {
                if let Some(controller) = self.controller.as_mut()
                    && let Err(error) = controller.select_lineage(&lineage_id)
                {
                    self.notice = Some(Notice::error(&error));
                }
            }
            UiAction::SelectVersion(source_id) => {
                if let Some(controller) = self.controller.as_mut()
                    && let Err(error) = controller.select_source_version(&source_id)
                {
                    self.notice = Some(Notice::error(&error));
                }
            }
            UiAction::SelectSearchResult {
                lineage_id,
                source_id,
            } => {
                if let Some(controller) = self.controller.as_mut() {
                    let result = controller
                        .select_lineage(&lineage_id)
                        .and_then(|()| controller.select_source_version(&source_id));
                    if let Err(error) = result {
                        self.notice = Some(Notice::error(&error));
                    }
                }
            }
            UiAction::RequestDelete => self.confirm_delete = true,
            UiAction::CancelDelete => self.confirm_delete = false,
            UiAction::ConfirmDelete => self.delete_source(),
        }
    }

    fn import_source(&mut self) {
        let outcome = NativeFilePicker::pick_import();
        match outcome {
            Ok(PickerOutcome::Cancelled) => {
                self.notice = Some(Notice::neutral("Import cancelled. No library changes."));
            }
            Ok(PickerOutcome::Selected(request)) => {
                let Some(controller) = self.controller.as_mut() else {
                    self.notice = Some(Notice::neutral("Local library is unavailable."));
                    return;
                };
                let result = controller.import_source(&request);
                self.notice = Some(match result {
                    Ok(receipt) => match receipt.outcome() {
                        FileCaptureOutcome::Created => Notice::success("Source imported."),
                        FileCaptureOutcome::Idempotent => {
                            Notice::neutral("The selected bytes are already current.")
                        }
                        FileCaptureOutcome::Versioned => {
                            Notice::success("A new source version was recorded.")
                        }
                    },
                    Err(error) => Notice::error(&error),
                });
            }
            Err(error) => self.notice = Some(Notice::error(&error)),
        }
    }

    fn update_source(&mut self) {
        let outcome = NativeFilePicker::pick_import();
        match outcome {
            Ok(PickerOutcome::Cancelled) => {
                self.notice = Some(Notice::neutral("Update cancelled. No library changes."));
            }
            Ok(PickerOutcome::Selected(request)) => {
                let Some(controller) = self.controller.as_mut() else {
                    self.notice = Some(Notice::neutral("Local library is unavailable."));
                    return;
                };
                let result = controller.update_selected(&request);
                self.notice = Some(match result {
                    Ok(receipt) if receipt.outcome() == FileCaptureOutcome::Idempotent => {
                        Notice::neutral("The selected bytes are already current.")
                    }
                    Ok(_) => Notice::success("Source version updated."),
                    Err(error) => Notice::error(&error),
                });
            }
            Err(error) => self.notice = Some(Notice::error(&error)),
        }
    }

    fn export_source(&mut self) {
        let suggested_name = self
            .controller
            .as_ref()
            .and_then(LibraryController::selected_version)
            .and_then(SourceVersionSummary::title)
            .map(|title| title.as_str().to_owned());
        match NativeFilePicker::pick_export(suggested_name.as_deref()) {
            Ok(PickerOutcome::Cancelled) => {
                self.notice = Some(Notice::neutral("Export cancelled. No file was written."));
            }
            Ok(PickerOutcome::Selected(request)) => {
                let Some(controller) = self.controller.as_ref() else {
                    self.notice = Some(Notice::neutral("Local library is unavailable."));
                    return;
                };
                let result = controller.export_selected(&request);
                self.notice = Some(match result {
                    Ok(_) => Notice::success("Managed bytes exported without overwrite."),
                    Err(error) => Notice::error(&error),
                });
            }
            Err(error) => self.notice = Some(Notice::error(&error)),
        }
    }

    fn verify_library(&mut self) {
        let Some(controller) = self.controller.as_ref() else {
            self.notice = Some(Notice::neutral("Local library is unavailable."));
            return;
        };
        let result = controller.verify();
        self.notice = Some(match result {
            Ok(()) => Notice::success("Canonical facts and derived recall are consistent."),
            Err(error) => Notice::error(&error),
        });
    }

    fn rebuild_library(&mut self) {
        let Some(controller) = self.controller.as_mut() else {
            self.notice = Some(Notice::neutral("Local library is unavailable."));
            return;
        };
        let result = controller.rebuild();
        self.notice = Some(match result {
            Ok(()) => Notice::success("Derived recall was rebuilt from verified facts."),
            Err(error) => Notice::error(&error),
        });
    }

    fn search(&mut self) {
        let Some(controller) = self.controller.as_mut() else {
            self.notice = Some(Notice::neutral("Local library is unavailable."));
            return;
        };
        let result = controller.search(&self.search_query);
        self.notice = Some(match result {
            Ok(()) if self.search_query.trim().is_empty() => Notice::neutral("Search cleared."),
            Ok(()) => Notice::success("Local-only search completed."),
            Err(error) => Notice::error(&error),
        });
    }

    fn delete_source(&mut self) {
        self.confirm_delete = false;
        let Some(controller) = self.controller.as_mut() else {
            self.notice = Some(Notice::neutral("Local library is unavailable."));
            return;
        };
        let result = controller.delete_selected_lineage();
        self.notice = Some(match result {
            Ok(evidence) => {
                let notice = match evidence.params().overall_status {
                    DeletionOverallStatus::Completed => Notice::success(
                        "Local managed lineage deletion completed with persisted evidence.",
                    ),
                    DeletionOverallStatus::Pending => {
                        Notice::neutral("Deletion remains pending and recall stays closed.")
                    }
                    DeletionOverallStatus::Partial | DeletionOverallStatus::Failed => {
                        Notice::neutral(
                            "Deletion did not fully complete; recall stays closed and evidence was retained.",
                        )
                    }
                };
                self.last_deletion = Some(evidence);
                notice
            }
            Err(error) => Notice::error(&error),
        });
    }
}

impl eframe::App for RadishMemoryApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut action = None;
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("RadishMemory");
                ui.separator();
                ui.label("Local-only text library");
                if self.controller.is_some() {
                    if ui.button("Import file…").clicked() {
                        action = Some(UiAction::Import);
                    }
                    if ui.button("Verify").clicked() {
                        action = Some(UiAction::Verify);
                    }
                    if ui.button("Rebuild recall").clicked() {
                        action = Some(UiAction::Rebuild);
                    }
                }
            });
            if let Some(notice) = &self.notice {
                ui.colored_label(notice.color(), &notice.message);
            }
            ui.separator();

            let Some(controller) = self.controller.as_ref() else {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.heading("Local library unavailable");
                    ui.label("RadishMemory failed closed before exposing library operations.");
                    if let Some(error) = &self.startup_error {
                        ui.monospace(redacted_error(error));
                    }
                    ui.add_space(12.0);
                    if ui.button("Retry opening library").clicked() {
                        action = Some(UiAction::RetryStartup);
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
                        if ui.button("Update from file…").clicked() {
                            action = Some(UiAction::Update);
                        }
                        if ui.button("Export selected version…").clicked() {
                            action = Some(UiAction::Export);
                        }
                        if ui.button("Delete managed lineage…").clicked() {
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

        if let Some(action) = action {
            self.execute(action);
        }
    }
}

fn open_production_controller() -> Result<LibraryController<ProductionRuntime>, DesktopError> {
    let paths = ApplicationPaths::resolve()?;
    LibraryController::bootstrap(&paths, ProductionRuntime)
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
    if let Some(os_error_code) = error.os_error_code() {
        summary.push_str(&format!(" · os_error={os_error_code}"));
    }
    summary
}

enum UiAction {
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
    fn success(message: &'static str) -> Self {
        Self {
            kind: NoticeKind::Success,
            message: message.to_owned(),
        }
    }

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
