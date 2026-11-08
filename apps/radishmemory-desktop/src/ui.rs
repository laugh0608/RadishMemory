use crate::controller::{CATALOG_PAGE_SIZE, LibraryView, text_excerpt};
use crate::worker::{Command, SearchQuery, Snapshot, Worker};
use crate::{DesktopError, NativeFilePicker, PickerOutcome};
use eframe::egui;
use radishmemory_application::{
    ApplicationErrorReason, Identifier, MediaType, NonEmptyText, NoteFormat, SourceEntry,
    SourceSavedWithin, SourceSearchFilters, is_note_producer, note_body_is_valid,
};

pub struct RadishMemoryApp {
    worker: Option<Worker>,
    controller: Option<LibraryView>,
    state: Snapshot,
    note_draft: Option<NoteDraft>,
    original_offset: usize,
    search_query: String,
    search_filters: SourceSearchFilters,
    show_comparison: bool,
    notice: Option<Notice>,
    refresh_notice: Option<String>,
    last_deletion: Option<radishmemory_application::DeletionEvidence>,
    confirm_delete: bool,
    confirmation: Option<Confirmation>,
    allow_close: bool,
    deferred_close_frame: Option<u64>,
    acceptance: bool,
}
#[derive(Clone, Copy)]
enum Confirmation {
    Initialize,
    Migrate,
    UpgradeRecall,
    DowngradeRecall,
    RebuildPlainRecall,
    Abandon,
    Close,
    CloseWithDraft,
}

enum NoteDraft {
    New {
        format: NoteFormat,
        title: String,
        body: String,
    },
    Correction {
        lineage: Identifier,
        base: Identifier,
        body: String,
    },
}

impl RadishMemoryApp {
    #[must_use]
    pub fn bootstrap() -> Self {
        Self::bootstrap_with_startup(crate::startup::Startup::default())
    }
    pub(crate) fn bootstrap_with_startup(startup: crate::startup::Startup) -> Self {
        let mut app = Self {
            worker: None,
            controller: None,
            state: Snapshot::default(),
            note_draft: None,
            original_offset: 0,
            search_query: String::new(),
            search_filters: SourceSearchFilters::default(),
            show_comparison: false,
            notice: None,
            refresh_notice: None,
            last_deletion: None,
            confirm_delete: false,
            confirmation: None,
            allow_close: false,
            deferred_close_frame: None,
            acceptance: startup.is_acceptance(),
        };
        let worker = match startup {
            crate::startup::Startup::Default => Worker::start(),
            startup => Worker::start_with_startup(startup),
        };
        match worker {
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
        let invalidates_results = matches!(
            &command,
            Command::Import(_)
                | Command::Update(_)
                | Command::SaveNote(..)
                | Command::CorrectNote(..)
                | Command::Delete
                | Command::RetryOriginal
                | Command::ResumeDelete(_)
                | Command::Search(_)
        );
        let invalidates_comparison = invalidates_results
            || matches!(
                &command,
                Command::Refresh
                    | Command::RefreshComparison
                    | Command::AddComparison(_)
                    | Command::OpenComparisonSource(_)
                    | Command::OpenPlain
                    | Command::OpenEncrypted
                    | Command::InitializeKey
                    | Command::Migrate
                    | Command::Abandon
            );
        if let Some(worker) = &mut self.worker {
            match worker.send(command) {
                Ok(()) => {
                    self.state.original = None;
                    self.original_offset = 0;
                    // Previous results stop applying as soon as a new search or write is accepted.
                    if let Some(view) = &mut self.controller {
                        if invalidates_results {
                            view.clear_search_results();
                        }
                        if invalidates_comparison {
                            view.clear_comparison_content();
                        }
                    }
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
        self.original_offset = state.original_offset;
        if state.capture_completed {
            self.note_draft = None;
        }
        if state.edit_original
            && let Some(original) = state.original.take()
        {
            let source = original.params();
            self.note_draft = Some(NoteDraft::Correction {
                lineage: source.lineage_id.clone(),
                base: source.source_id.clone(),
                body: source.content.as_str().to_owned(),
            });
        }
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
            UiAction::NewNote => {
                self.note_draft = Some(NoteDraft::New {
                    format: NoteFormat::Text,
                    title: String::new(),
                    body: String::new(),
                });
            }
            UiAction::DiscardNote => self.note_draft = None,
            UiAction::ShowOriginalExcerpt(offset) => self.original_offset = offset,
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
                    .and_then(|version| {
                        if is_note_producer(version.producer()) {
                            return Some(if version.media_type().as_str() == "text/markdown" {
                                "note.md"
                            } else {
                                "note.txt"
                            });
                        }
                        version.title().map(NonEmptyText::as_str)
                    });
                match NativeFilePicker::pick_export(name) {
                    Ok(PickerOutcome::Selected(request)) => self.send(Command::Export(request)),
                    Ok(PickerOutcome::Cancelled) => {
                        self.notice = Some(Notice::neutral("Export cancelled."))
                    }
                    Err(error) => self.notice = Some(Notice::error(&error)),
                }
            }
            UiAction::Verify => self.send(Command::Verify),
            UiAction::Rebuild => self.send(Command::Rebuild),
            UiAction::Search => self.send(Command::Search(SearchQuery {
                text: self.search_query.clone(),
                filters: self.search_filters,
            })),
            UiAction::ShowComparison(show) => {
                self.show_comparison = show;
                if show {
                    self.send(Command::RefreshComparison);
                }
            }
            UiAction::AddComparison(id) => {
                self.show_comparison = true;
                self.send(Command::AddComparison(id));
            }
            UiAction::SelectLineage(id) => self.send(Command::SelectLineage(id)),
            UiAction::SelectVersion(id) => self.send(Command::SelectVersion(id)),
            UiAction::SelectSearchResult {
                lineage_id,
                source_id,
                fragment_id,
            } => self.send(Command::SelectResult(lineage_id, source_id, fragment_id)),
            UiAction::ShowFragmentExcerpt(offset) => {
                if let Some(view) = &mut self.controller {
                    view.show_fragment_excerpt(offset);
                }
            }
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

impl RadishMemoryApp {
    fn render(&mut self, ui: &mut egui::Ui) {
        if self.allow_close {
            return;
        }
        let was_busy = self.busy();
        self.poll();
        let context = ui.ctx().clone();
        if self.note_draft.is_none() {
            context.data_mut(|data| {
                for id in [NOTE_TITLE_ID, NOTE_BODY_ID] {
                    data.remove::<egui::text_edit::TextEditState>(egui::Id::new(id));
                }
            });
        }
        if self.busy() {
            context.request_repaint_after(std::time::Duration::from_millis(80));
        }
        if context.input(|i| i.viewport().close_requested()) {
            let frame = context.cumulative_frame_nr();
            // A result received in this frame has not yet been shown to the user.
            // Keep the cancellation across egui layout passes of the same frame.
            if was_busy || self.deferred_close_frame == Some(frame) {
                self.deferred_close_frame = Some(frame);
                context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                if self.busy() {
                    self.notice = Some(Notice::neutral(
                        "An operation is still running. Wait for its real result before closing.",
                    ));
                }
            } else if self.state.holds_original {
                context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirmation = Some(Confirmation::Close);
            } else if self.note_draft.is_some() {
                context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirmation = Some(Confirmation::CloseWithDraft);
            } else {
                // Do not dispatch a click delivered together with an accepted close.
                self.allow_close = true;
                return;
            }
        }
        let can_mutate = self.can_mutate();
        let mut action = None;
        egui::CentralPanel::default().show(ui, |ui| {
            if self.confirmation.is_some() || self.confirm_delete || (self.note_draft.is_some() && !self.state.holds_original) { ui.disable(); }
            if self.busy() { ui.spinner(); ui.label("Library worker is busy; repeated actions are disabled."); ui.disable(); }
            if self.worker.as_ref().is_none_or(|w| w.stopped) { ui.label("Worker unavailable. Restart the app; no success is assumed."); ui.disable(); }
            ui.horizontal_wrapped(|ui| {
                ui.heading("RadishMemory");
                ui.separator();
                ui.label("Local-only text library");
                if self.acceptance { ui.label("Synthetic acceptance library"); }
                if self.controller.is_some() && ui.add_enabled(can_mutate, egui::Button::new("New note…")).clicked() { action = Some(UiAction::NewNote); }
                if self.controller.is_some() && ui.add_enabled(can_mutate, egui::Button::new("Import file…")).clicked() { action = Some(UiAction::Import); }
                if ui.button("Refresh").clicked() { action = Some(UiAction::Command(Command::Refresh)); }
                let can_use_standard_maintenance = self.controller.is_some() || self.state.encrypted;
                if ui.add_enabled(can_use_standard_maintenance, egui::Button::new("Verify")).clicked() { action = Some(UiAction::Verify); }
                if ui.add_enabled(can_use_standard_maintenance, egui::Button::new("Rebuild recall")).clicked() { action = Some(UiAction::Rebuild); }
            });
            ui.horizontal_wrapped(|ui| {
                let can_switch = !self.state.holds_original;
                if ui.add_enabled(can_switch, egui::Button::new("Open legacy plaintext")).clicked() { action = Some(UiAction::RetryStartup); }
                if ui.add_enabled(can_switch, egui::Button::new("Open encrypted objects")).clicked() { action = Some(UiAction::Command(Command::OpenEncrypted)); }
                if ui.add_enabled(can_switch, egui::Button::new("Prepare key…")).clicked() { action = Some(UiAction::Confirm(Confirmation::Initialize)); }
                if ui.add_enabled(can_switch, egui::Button::new("Migrate / resume bodies…")).clicked() { action = Some(UiAction::Confirm(Confirmation::Migrate)); }
            });
            ui.small(if self.state.encrypted { "Encrypted object mode: FTS retains full readable text. This is not whole-library encryption." } else { "Default legacy mode: SQLite stores plaintext source bodies." });
            if self.controller.is_some() {
                ui.horizontal_wrapped(|ui| {
                    match self.state.recall_profile {
                        Some(radishmemory_application::RecallProfile::LegacyUnicode61) => {
                            ui.label("Search: legacy word matching (last refreshed)");
                            if ui.add_enabled(can_mutate, egui::Button::new("Enable Chinese phrase search…")).clicked() { action = Some(UiAction::Confirm(Confirmation::UpgradeRecall)); }
                        }
                        Some(radishmemory_application::RecallProfile::HanPhraseV1) => {
                            ui.label("Search: Chinese phrases (last refreshed)");
                            if ui.add_enabled(can_mutate, egui::Button::new("Use legacy search…")).clicked() { action = Some(UiAction::Confirm(Confirmation::DowngradeRecall)); }
                        }
                        None => { ui.label("Search format unavailable. Refresh before changing it."); }
                    }
                });
            }
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
                    if self.state.plain_repair_available {
                        if ui.button("Check plaintext recall repair").clicked() {
                            action = Some(UiAction::Command(Command::CheckPlainRecallRepair));
                        }
                        if self.state.plain_repair_checked && ui.button("Rebuild verified plaintext recall…").clicked() {
                            action = Some(UiAction::Confirm(Confirmation::RebuildPlainRecall));
                        }
                    }
                });
                return;
            };

            let selected_is_note = controller.current_version().is_some_and(|version| is_note_producer(version.producer()));
            let busy = self.busy();
            ui.columns(2, |columns| {
                let (source_columns, content_columns) = columns.split_at_mut(1);
                let source_ui = &mut source_columns[0];
                comparison::navigation(source_ui, controller, self.show_comparison, &mut action);
                if self.show_comparison {
                    comparison::panel(source_ui, controller, busy, self.state.holds_original, &mut action);
                } else {
                    let offset = controller.catalog_offset();
                    let page_size = CATALOG_PAGE_SIZE as u64;
                    source_ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(offset > 0, egui::Button::new("Previous page")).clicked() {
                            action = Some(UiAction::Command(Command::CatalogPage(offset - page_size)));
                        }
                        ui.label(format!("Page {}", offset / page_size + 1));
                        if ui.add_enabled(controller.has_next_catalog_page(), egui::Button::new("Next page")).clicked() {
                            action = Some(UiAction::Command(Command::CatalogPage(offset + page_size)));
                        }
                    });
                    source_ui.label(format!("{} active lineage(s) on this page", controller.sources().len()));
                    source_ui.separator();
                    egui::ScrollArea::vertical().id_salt(("source-catalog", offset)).show(source_ui, |ui| {
                        for source in controller.sources() {
                            let selected =
                                controller.selected_lineage_id() == Some(source.lineage_id());
                            if ui.selectable_label(selected, source_label(source.title())).clicked() {
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
                }

                let content_ui = &mut content_columns[0];
                content_ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Reserve the button before allowing the editor to fill the remaining row.
                        let search_clicked = ui.button("Search").clicked();
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text("Search managed text locally")
                                .desired_width(ui.available_width()),
                        );
                        if search_clicked
                            || (response.lost_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                        {
                            action = Some(UiAction::Search);
                        }
                    });
                });
                let previous_filters = self.search_filters;
                content_ui.horizontal_wrapped(|ui| {
                    search_filter(ui, "All sources", &mut self.search_filters.entry, &[
                        (SourceEntry::Note, "In-app notes"),
                        (SourceEntry::FileImport, "Imported files"),
                    ]);
                    search_filter(ui, "All formats", &mut self.search_filters.media_type, &[
                        (MediaType::TextPlain, "Plain text"),
                        (MediaType::TextMarkdown, "Markdown"),
                    ]);
                    search_filter(ui, "Any save time", &mut self.search_filters.saved_within, &[
                        (SourceSavedWithin::Day, "Last 24 hours"),
                        (SourceSavedWithin::Week, "Last 7 days"),
                        (SourceSavedWithin::ThirtyDays, "Last 30 days"),
                    ]);
                });
                if self.search_filters != previous_filters {
                    action = Some(UiAction::Search);
                }
                content_ui.separator();

                if let Some(source) = controller.current_version() {
                    content_ui.heading(source_label(source.title()));
                    if controller.selected_lineage().is_none() {
                        content_ui.small("Selected source is outside this catalog page.");
                    }
                    content_ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "Current version: {}",
                            source.version().get()
                        ));
                        ui.label(format!(
                            "Managed bytes: {}",
                            format_bytes(source.content_length())
                        ));
                        ui.label(format!("Versions: {}", controller.versions().len()));
                    });
                    content_ui.horizontal_wrapped(|ui| {
                        if selected_is_note {
                            let editing_current = controller.selected_version().is_some_and(|v| v.current());
                            if ui.add_enabled(can_mutate && editing_current, egui::Button::new("Edit note…")).clicked() {
                                action = Some(UiAction::Command(Command::ReadOriginal { edit: true }));
                            }
                        } else if ui.add_enabled(can_mutate, egui::Button::new("Update from file…")).clicked() {
                                action = Some(UiAction::Update);
                        }
                        if ui.add_enabled(!self.state.holds_original, egui::Button::new("View original")).clicked() {
                            action = Some(UiAction::Command(Command::ReadOriginal { edit: false }));
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
                    if let Some(original) = &self.state.original {
                        let original = original.params();
                        let start = self.original_offset;
                        let (text, previous, next) = text_excerpt(original.content.as_str(), start);
                        content_ui.add_space(8.0);
                        content_ui.strong("Original text");
                        content_ui.small(format!("v{} · bytes {}..{} of {}", original.version.get(),
                            start, start + text.len(), original.content_length));
                        content_ui.horizontal_wrapped(|ui| {
                            for (label, offset) in [("Previous page of text", previous), ("Next page of text", next)] {
                                if ui.add_enabled(offset.is_some(), egui::Button::new(label)).clicked()
                                    && let Some(offset) = offset
                                {
                                    action = Some(UiAction::ShowOriginalExcerpt(offset));
                                }
                            }
                        });
                        egui::ScrollArea::vertical()
                            .id_salt(("source-original", original.source_id.as_str(), start))
                            .max_height(110.0)
                            .show(content_ui, |ui| { ui.label(text); });
                    } else if let Some(excerpt) = controller.selected_excerpt() {
                        content_ui.add_space(8.0);
                        content_ui.strong("Source fragment").on_hover_text(
                            "Opens at the literal query text when present. Use the buttons to browse the original fragment.",
                        );
                        content_ui.small(format!(
                            "v{} · excerpt bytes {}..{}",
                            excerpt.result.version().get(),
                            excerpt.byte_start,
                            excerpt.byte_end
                        ));
                        content_ui.horizontal_wrapped(|ui| {
                            for (label, offset) in [
                                ("Previous excerpt", excerpt.previous),
                                ("Next excerpt", excerpt.next),
                            ] {
                                if ui.add_enabled(offset.is_some(), egui::Button::new(label)).clicked()
                                    && let Some(offset) = offset
                                {
                                    action = Some(UiAction::ShowFragmentExcerpt(offset));
                                }
                            }
                        });
                        egui::ScrollArea::vertical()
                            .id_salt(("source-fragment", excerpt.result.fragment_id().as_str(), excerpt.byte_start))
                            .max_height(110.0)
                            .show(content_ui, |ui| {
                                ui.label(excerpt.text);
                            });
                    }
                } else {
                    content_ui.heading("No managed sources");
                    content_ui
                        .label("Write a note or import a UTF-8 .txt or .md file.");
                }

                content_ui.add_space(16.0);
                content_ui.separator();
                content_ui.heading("Search results");
                if controller.search_results().is_empty() {
                    content_ui.label("No local search results to show.");
                } else {
                    egui::ScrollArea::vertical().id_salt("search-results").show(content_ui, |ui| {
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
                                        fragment_id: result.fragment_id().clone(),
                                    });
                                }
                                comparison::add_button(ui, controller, result, self.state.holds_original, &mut action);
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
                egui::Window::new("Confirm managed lineage deletion")
                    .collapsible(false)
                    .resizable(false)
                    .show(&context, |ui| {
                        ui.label("All managed versions and active memory dependencies will be closed and purged locally.");
                        ui.label(if selected_is_note {
                            "Prior exports remain outside the managed library."
                        } else {
                            "The original selected file and prior exports are not deleted."
                        });
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
                    Confirmation::UpgradeRecall => "Enable contiguous Chinese phrase matching and rebuild the search index. Earlier clients will refuse this library until you explicitly switch back to legacy search. Source bodies and their encryption mode are unchanged. Encrypted mode reads the existing system key.",
                    Confirmation::DowngradeRecall => "Rebuild legacy search for earlier clients. Chinese short words may be missed again. This does not restore an old backup or change source bodies, encryption mode, or body-version compatibility. Encrypted mode reads the existing system key.",
                    Confirmation::RebuildPlainRecall => "Recheck the existing plaintext library and rebuild its search index, source tips and memory projection. The library will reopen only after verification.",
                    Confirmation::Abandon => "Permanently abandon exactly the inspected uncommitted capture. Its original request cannot be replayed afterward. Existing committed sources and unknown files are not targets.",
                    Confirmation::Close => "Closing loses the original request held in memory. An unfinished capture then requires explicit abandonment or the exact original request from elsewhere. Persisted deletion authority can be discovered after restart.",
                    Confirmation::CloseWithDraft => "Close and discard the unsaved note draft?",
                });
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() { self.confirmation = None; }
                    if ui.button("Confirm").clicked() {
                        self.confirmation = None;
                        match confirmation {
                            Confirmation::Initialize => action = Some(UiAction::Command(Command::InitializeKey)),
                            Confirmation::Migrate => action = Some(UiAction::Command(Command::Migrate)),
                            Confirmation::UpgradeRecall => action = Some(UiAction::Command(Command::ChangeRecallProfile(radishmemory_application::RecallProfile::HanPhraseV1))),
                            Confirmation::DowngradeRecall => action = Some(UiAction::Command(Command::ChangeRecallProfile(radishmemory_application::RecallProfile::LegacyUnicode61))),
                            Confirmation::RebuildPlainRecall => action = Some(UiAction::Command(Command::RebuildPlainRecall)),
                            Confirmation::Abandon => action = Some(UiAction::Command(Command::Abandon)),
                            Confirmation::Close | Confirmation::CloseWithDraft => { self.allow_close = true; context.send_viewport_cmd(egui::ViewportCommand::Close); }
                        }
                    }
                });
            });
        }
        if self.confirmation.is_none() && !self.state.holds_original {
            let worker_available = self.worker.as_ref().is_some_and(|worker| !worker.stopped);
            let save_enabled = worker_available && can_mutate && self.controller.is_some();
            let busy = self.busy();
            if let Some(draft) = &mut self.note_draft {
                action = render_note_editor(
                    &context,
                    draft,
                    busy,
                    save_enabled,
                    worker_available,
                    self.notice.as_ref(),
                );
            }
        }
        if let Some(action) = action {
            self.execute(action);
        }
    }
}

impl eframe::App for RadishMemoryApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui);
    }
}

const NOTE_TITLE_ID: &str = "application-note-title";
const NOTE_BODY_ID: &str = "application-note-body";

fn render_note_editor(
    context: &egui::Context,
    draft: &mut NoteDraft,
    busy: bool,
    save_enabled: bool,
    worker_available: bool,
    notice: Option<&Notice>,
) -> Option<UiAction> {
    egui::Modal::new(egui::Id::new("application-note-editor"))
        .show(context, |ui| {
            ui.set_width((context.content_rect().width() - 64.0).clamp(200.0, 520.0));
            if busy {
                ui.disable();
            }
            let (body, save_label) = match draft {
                NoteDraft::New {
                    format,
                    title,
                    body,
                } => {
                    ui.heading("New note");
                    ui.horizontal(|ui| {
                        ui.selectable_value(format, NoteFormat::Text, "Text");
                        ui.selectable_value(format, NoteFormat::Markdown, "Markdown");
                    });
                    ui.add(
                        egui::TextEdit::singleline(title)
                            .id(egui::Id::new(NOTE_TITLE_ID))
                            .hint_text("Optional title")
                            .desired_width(f32::INFINITY),
                    );
                    (body, "Save note")
                }
                NoteDraft::Correction { body, .. } => {
                    ui.heading("Edit note");
                    ui.label("The previous text stays available in Version history.");
                    (body, "Save correction")
                }
            };
            egui::ScrollArea::vertical()
                .max_height((context.content_rect().height() - 240.0).clamp(100.0, 260.0))
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(body)
                            .id(egui::Id::new(NOTE_BODY_ID))
                            .hint_text("Enter or paste the source text")
                            .desired_rows(8)
                            .desired_width(f32::INFINITY),
                    );
                });
            let valid = note_body_is_valid(body);
            if !valid {
                ui.small("Enter some text, up to 8 MiB of UTF-8, without NUL characters.");
            }
            if let Some(notice) = notice
                .filter(|notice| busy || !save_enabled && matches!(notice.kind, NoticeKind::Error))
            {
                ui.colored_label(notice.color(), &notice.message);
            }
            let mut action = None;
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(save_enabled && valid, egui::Button::new(save_label))
                    .clicked()
                {
                    let command = match draft {
                        NoteDraft::New {
                            format,
                            title,
                            body,
                        } => Command::SaveNote(
                            *format,
                            NonEmptyText::new(title.as_str()).ok(),
                            body.clone(),
                        ),
                        NoteDraft::Correction {
                            lineage,
                            base,
                            body,
                        } => Command::CorrectNote(lineage.clone(), base.clone(), body.clone()),
                    };
                    action = Some(UiAction::Command(command));
                }
                if !save_enabled && worker_available && ui.button("Refresh library").clicked() {
                    action = Some(UiAction::Command(Command::Refresh));
                }
                if ui.button("Discard draft").clicked() {
                    action = Some(UiAction::DiscardNote);
                }
            });
            action
        })
        .inner
}

mod comparison;

#[cfg(test)]
mod catalog_tests;
#[cfg(test)]
mod close_tests;
#[cfg(test)]
mod comparison_tests;
#[cfg(test)]
mod font_tests;
#[cfg(test)]
mod fragment_tests;
#[cfg(test)]
mod note_tests;
#[cfg(test)]
mod read_session_tests;
#[cfg(test)]
mod recall_profile_tests;
#[cfg(test)]
mod recall_repair_tests;
#[cfg(test)]
mod search_filter_tests;
#[cfg(test)]
mod test_support;

fn search_filter<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    unrestricted: &str,
    value: &mut Option<T>,
    options: &[(T, &str)],
) {
    let label = options
        .iter()
        .find(|(option, _)| Some(*option) == *value)
        .map_or(unrestricted, |(_, label)| *label);
    egui::ComboBox::from_id_salt(unrestricted)
        .selected_text(label)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, unrestricted);
            for &(option, label) in options {
                ui.selectable_value(value, Some(option), label);
            }
        });
}

fn source_label(title: Option<&NonEmptyText>) -> &str {
    title.map_or("Untitled source", NonEmptyText::as_str)
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
    if let Some(database) = error
        .vault_database()
        .or_else(|| error.plaintext_database())
    {
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
    NewNote,
    DiscardNote,
    Import,
    Update,
    Export,
    Verify,
    Rebuild,
    Search,
    ShowComparison(bool),
    AddComparison(Identifier),
    SelectLineage(Identifier),
    SelectVersion(Identifier),
    SelectSearchResult {
        lineage_id: Identifier,
        source_id: Identifier,
        fragment_id: Identifier,
    },
    ShowFragmentExcerpt(usize),
    ShowOriginalExcerpt(usize),
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
        let mut message = redacted_error(error);
        match error.application_failure().map(|failure| failure.reason()) {
            Some(ApplicationErrorReason::NoteVersionChanged) => {
                message = "A newer version was saved. Keep a copy of your draft, then discard it, refresh, and reopen the current version.".into();
            }
            Some(ApplicationErrorReason::NoteEntryMismatch) => {
                message =
                    "Refresh the library and use the edit action shown for this source.".into();
            }
            _ => {}
        }
        if error.reason() == crate::DesktopErrorReason::SelectionInvalid {
            message.push_str(". Refresh the library and search again to select a current result.");
        }
        Self {
            kind: NoticeKind::Error,
            message,
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
