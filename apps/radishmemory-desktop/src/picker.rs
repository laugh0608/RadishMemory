use radishmemory_application::{FileExportRequest, FileReadRequest};
use std::path::PathBuf;

use crate::{DesktopError, DesktopErrorCode, DesktopErrorReason};

pub enum PickerOutcome<T> {
    Cancelled,
    Selected(T),
}

pub struct NativeFilePicker;

impl NativeFilePicker {
    pub fn pick_import() -> Result<PickerOutcome<FileReadRequest>, DesktopError> {
        let path = rfd::FileDialog::new()
            .add_filter("Text and Markdown", &["txt", "md"])
            .pick_file();
        import_request_from_path(path)
    }

    pub fn pick_export(
        suggested_name: Option<&str>,
    ) -> Result<PickerOutcome<FileExportRequest>, DesktopError> {
        let mut dialog = rfd::FileDialog::new().add_filter("Text and Markdown", &["txt", "md"]);
        if let Some(name) = suggested_name.filter(|name| !name.is_empty()) {
            dialog = dialog.set_file_name(name);
        }
        export_request_from_path(dialog.save_file())
    }
}

fn import_request_from_path(
    path: Option<PathBuf>,
) -> Result<PickerOutcome<FileReadRequest>, DesktopError> {
    let Some(path) = path else {
        return Ok(PickerOutcome::Cancelled);
    };
    let parent = path
        .parent()
        .filter(|parent| parent.is_absolute())
        .ok_or_else(|| {
            DesktopError::without_source(
                DesktopErrorCode::Picker,
                DesktopErrorReason::SelectionInvalid,
                false,
            )
        })?;
    let request = FileReadRequest::new(&path, vec![parent.to_path_buf()]).map_err(|_| {
        DesktopError::without_source(
            DesktopErrorCode::Picker,
            DesktopErrorReason::SelectionInvalid,
            false,
        )
    })?;
    Ok(PickerOutcome::Selected(request))
}

fn export_request_from_path(
    path: Option<PathBuf>,
) -> Result<PickerOutcome<FileExportRequest>, DesktopError> {
    let Some(path) = path else {
        return Ok(PickerOutcome::Cancelled);
    };
    let parent = path
        .parent()
        .filter(|parent| parent.is_absolute())
        .ok_or_else(|| {
            DesktopError::without_source(
                DesktopErrorCode::Picker,
                DesktopErrorReason::SelectionInvalid,
                false,
            )
        })?;
    let request = FileExportRequest::new(&path, vec![parent.to_path_buf()]).map_err(|_| {
        DesktopError::without_source(
            DesktopErrorCode::Picker,
            DesktopErrorReason::SelectionInvalid,
            false,
        )
    })?;
    Ok(PickerOutcome::Selected(request))
}

impl std::fmt::Debug for NativeFilePicker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeFilePicker")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_picker_produces_no_file_request() {
        assert!(matches!(
            import_request_from_path(None).unwrap(),
            PickerOutcome::Cancelled
        ));
        assert!(matches!(
            export_request_from_path(None).unwrap(),
            PickerOutcome::Cancelled
        ));
    }

    #[test]
    fn selected_paths_are_reduced_to_the_exact_parent_capability() {
        let root = std::env::temp_dir().join("radishmemory-picker-synthetic-root");
        let input = root.join("input").join("note.md");
        let output = root.join("output").join("note.md");

        let PickerOutcome::Selected(import) =
            import_request_from_path(Some(input.clone())).unwrap()
        else {
            panic!("synthetic import path should be selected");
        };
        assert_eq!(
            format!("{import:?}"),
            "FileReadRequest { allowed_root_count: 1, .. }"
        );
        assert!(!format!("{import:?}").contains(input.to_str().unwrap()));

        let PickerOutcome::Selected(export) =
            export_request_from_path(Some(output.clone())).unwrap()
        else {
            panic!("synthetic export path should be selected");
        };
        assert_eq!(
            format!("{export:?}"),
            "FileExportRequest { allowed_root_count: 1, .. }"
        );
        assert!(!format!("{export:?}").contains(output.to_str().unwrap()));
    }

    #[test]
    fn relative_picker_result_is_rejected_without_echoing_the_path() {
        let marker = "synthetic-private-relative-path.md";
        let Err(error) = import_request_from_path(Some(PathBuf::from(marker))) else {
            panic!("relative synthetic picker result must be rejected");
        };
        assert_eq!(error.code(), DesktopErrorCode::Picker);
        assert_eq!(error.reason(), DesktopErrorReason::SelectionInvalid);
        assert!(!format!("{error:?} {error}").contains(marker));
    }
}
