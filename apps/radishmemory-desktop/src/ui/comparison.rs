use super::{Command, LibraryView, UiAction, egui, source_label};
use radishmemory_application::{SOURCE_COMPARISON_LIMIT, SourceSearchResult};

pub(super) fn navigation(
    ui: &mut egui::Ui,
    view: &LibraryView,
    show_comparison: bool,
    action: &mut Option<UiAction>,
) {
    ui.horizontal_wrapped(|ui| {
        if ui.selectable_label(!show_comparison, "Sources").clicked() && show_comparison {
            *action = Some(UiAction::ShowComparison(false));
        }
        if ui
            .selectable_label(
                show_comparison,
                format!("Compare ({})", view.comparison().len()),
            )
            .clicked()
            && !show_comparison
        {
            *action = Some(UiAction::ShowComparison(true));
        }
    });
}

pub(super) fn panel(
    ui: &mut egui::Ui,
    view: &LibraryView,
    busy: bool,
    holds_original: bool,
    action: &mut Option<UiAction>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.heading("Source comparison");
        if ui.button("Refresh comparison").clicked() {
            *action = Some(UiAction::Command(Command::RefreshComparison));
        }
    });
    if busy || holds_original {
        ui.label(if busy {
            "Checking source availability…"
        } else {
            "Resolve the pending library operation to view comparisons."
        });
        return;
    }
    if view.comparison().is_empty() {
        ui.label("Add up to five search results to compare their source text.");
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("source-comparison")
        .show(ui, |ui| {
            for (index, card) in view.comparison().iter().enumerate() {
                ui.push_id(card.fragment_id.as_str(), |ui| {
                    ui.group(|ui| {
                        if let Some(excerpt) = &card.excerpt {
                            let status = if excerpt.historical {
                                "Historical"
                            } else {
                                "Current"
                            };
                            ui.strong(source_label(excerpt.title.as_ref()));
                            ui.small(format!(
                                "v{} · {status} · bytes {}..{}",
                                excerpt.version, excerpt.byte_start, excerpt.byte_end
                            ));
                            ui.label(&excerpt.text);
                        } else {
                            ui.label("Source unavailable");
                        }
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_enabled(
                                    card.excerpt.is_some(),
                                    egui::Button::new(format!("Open citation {}", index + 1)),
                                )
                                .clicked()
                            {
                                *action = Some(UiAction::Command(Command::OpenComparisonSource(
                                    card.fragment_id.clone(),
                                )));
                            }
                            if ui.button(format!("Remove {}", index + 1)).clicked() {
                                *action = Some(UiAction::Command(Command::RemoveComparison(
                                    card.fragment_id.clone(),
                                )));
                            }
                        });
                    });
                });
                ui.add_space(4.0);
            }
        });
}

pub(super) fn add_button(
    ui: &mut egui::Ui,
    view: &LibraryView,
    result: &SourceSearchResult,
    holds_original: bool,
    action: &mut Option<UiAction>,
) {
    let pinned = view
        .comparison()
        .iter()
        .any(|card| &card.fragment_id == result.fragment_id());
    let full = view.comparison().len() >= SOURCE_COMPARISON_LIMIT;
    let label = if pinned {
        "Added to comparison"
    } else if full {
        "Comparison full"
    } else {
        "Add to comparison"
    };
    if ui
        .add_enabled(
            !pinned && !full && !holds_original,
            egui::Button::new(label),
        )
        .clicked()
    {
        *action = Some(UiAction::AddComparison(result.fragment_id().clone()));
    }
}
