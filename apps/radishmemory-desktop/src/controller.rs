use radishmemory_application::{
    ApplicationRuntime, DeletionEvidence, FileCaptureReceipt, FileExportReceipt, FileExportRequest,
    FileReadRequest, Identifier, LocalLibrary, LocalLibraryConfig, NonEmptyText, Sensitivity,
    SourceLineageSummary, SourceSearchResult, SourceVersionSummary,
};

use crate::{
    ApplicationPaths, DesktopError, DesktopErrorCode, DesktopErrorReason,
    load_or_create_host_profile,
};

const CATALOG_PAGE_SIZE: usize = 200;
const SEARCH_RESULT_LIMIT: usize = 50;

pub struct LibraryController<R>
where
    R: ApplicationRuntime,
{
    library: LocalLibrary<R>,
    sources: Vec<SourceLineageSummary>,
    selected_lineage_id: Option<Identifier>,
    versions: Vec<SourceVersionSummary>,
    selected_source_id: Option<Identifier>,
    search_results: Vec<SourceSearchResult>,
}

impl<R> LibraryController<R>
where
    R: ApplicationRuntime,
{
    pub fn bootstrap(paths: &ApplicationPaths, mut runtime: R) -> Result<Self, DesktopError> {
        let profile = load_or_create_host_profile(paths, &mut runtime)?;
        let config = LocalLibraryConfig::phase1_local(
            profile.namespace_id().clone(),
            profile.device_id().clone(),
        )
        .map_err(|source| DesktopError::application(&source))?;
        let library = LocalLibrary::open(paths.database_path(), runtime, config)
            .map_err(|source| DesktopError::application(&source))?;
        let mut controller = Self {
            library,
            sources: Vec::new(),
            selected_lineage_id: None,
            versions: Vec::new(),
            selected_source_id: None,
            search_results: Vec::new(),
        };
        controller.refresh_sources()?;
        Ok(controller)
    }

    pub fn refresh_sources(&mut self) -> Result<(), DesktopError> {
        let previous = self.selected_lineage_id.clone();
        self.sources = self
            .library
            .list_sources(0, CATALOG_PAGE_SIZE)
            .map_err(|source| DesktopError::application(&source))?;
        let selected = previous
            .filter(|lineage_id| {
                self.sources
                    .iter()
                    .any(|source| source.lineage_id() == lineage_id)
            })
            .or_else(|| {
                self.sources
                    .first()
                    .map(|source| source.lineage_id().clone())
            });
        if let Some(lineage_id) = selected {
            self.select_lineage(&lineage_id)
        } else {
            self.selected_lineage_id = None;
            self.selected_source_id = None;
            self.versions.clear();
            Ok(())
        }
    }

    pub fn select_lineage(&mut self, lineage_id: &Identifier) -> Result<(), DesktopError> {
        let source = self
            .sources
            .iter()
            .find(|source| source.lineage_id() == lineage_id)
            .ok_or_else(selection_invalid)?;
        let current_source_id = source.current_source_id().clone();
        let versions = self
            .library
            .list_source_versions(lineage_id)
            .map_err(|source| DesktopError::application(&source))?;
        self.selected_lineage_id = Some(lineage_id.clone());
        self.selected_source_id = Some(current_source_id);
        self.versions = versions;
        Ok(())
    }

    pub fn select_source_version(&mut self, source_id: &Identifier) -> Result<(), DesktopError> {
        if !self
            .versions
            .iter()
            .any(|version| version.source_id() == source_id)
        {
            return Err(selection_invalid());
        }
        self.selected_source_id = Some(source_id.clone());
        Ok(())
    }

    pub fn import_source(
        &mut self,
        request: &FileReadRequest,
    ) -> Result<FileCaptureReceipt, DesktopError> {
        let receipt = self
            .library
            .import_new_source(request)
            .map_err(|source| DesktopError::application(&source))?;
        let lineage_id = receipt.lineage_id().clone();
        self.refresh_sources()?;
        self.select_lineage(&lineage_id)?;
        Ok(receipt)
    }

    pub fn update_selected(
        &mut self,
        request: &FileReadRequest,
    ) -> Result<FileCaptureReceipt, DesktopError> {
        let lineage_id = self
            .selected_lineage_id
            .clone()
            .ok_or_else(selection_invalid)?;
        let receipt = self
            .library
            .update_source(&lineage_id, request)
            .map_err(|source| DesktopError::application(&source))?;
        self.refresh_sources()?;
        self.select_lineage(&lineage_id)?;
        Ok(receipt)
    }

    pub fn export_selected(
        &self,
        request: &FileExportRequest,
    ) -> Result<FileExportReceipt, DesktopError> {
        let source_id = self
            .selected_source_id
            .as_ref()
            .ok_or_else(selection_invalid)?;
        self.library
            .export_source(source_id, request)
            .map_err(|source| DesktopError::application(&source))
    }

    pub fn search(&mut self, query: &str) -> Result<(), DesktopError> {
        if query.trim().is_empty() {
            self.search_results.clear();
            return Ok(());
        }
        let query = NonEmptyText::new(query.to_owned()).map_err(|_| selection_invalid())?;
        self.search_results = self
            .library
            .search_sources(query, SEARCH_RESULT_LIMIT, [Sensitivity::Personal])
            .map_err(|source| DesktopError::application(&source))?;
        Ok(())
    }

    pub fn delete_selected_lineage(&mut self) -> Result<DeletionEvidence, DesktopError> {
        let lineage_id = self
            .selected_lineage_id
            .clone()
            .ok_or_else(selection_invalid)?;
        let evidence = self
            .library
            .delete_source_lineage(&lineage_id)
            .map_err(|source| DesktopError::application(&source))?;
        self.search_results.clear();
        self.selected_lineage_id = None;
        self.selected_source_id = None;
        self.versions.clear();
        self.refresh_sources()?;
        Ok(evidence)
    }

    pub fn verify(&self) -> Result<(), DesktopError> {
        self.library
            .verify_library()
            .map_err(|source| DesktopError::application(&source))
    }

    pub fn rebuild(&mut self) -> Result<(), DesktopError> {
        self.library
            .rebuild_recall()
            .map_err(|source| DesktopError::application(&source))?;
        self.refresh_sources()
    }

    #[must_use]
    pub fn sources(&self) -> &[SourceLineageSummary] {
        &self.sources
    }

    #[must_use]
    pub fn versions(&self) -> &[SourceVersionSummary] {
        &self.versions
    }

    #[must_use]
    pub fn search_results(&self) -> &[SourceSearchResult] {
        &self.search_results
    }

    #[must_use]
    pub fn selected_lineage_id(&self) -> Option<&Identifier> {
        self.selected_lineage_id.as_ref()
    }

    #[must_use]
    pub fn selected_source_id(&self) -> Option<&Identifier> {
        self.selected_source_id.as_ref()
    }

    #[must_use]
    pub fn selected_lineage(&self) -> Option<&SourceLineageSummary> {
        let selected = self.selected_lineage_id.as_ref()?;
        self.sources
            .iter()
            .find(|source| source.lineage_id() == selected)
    }

    #[must_use]
    pub fn selected_version(&self) -> Option<&SourceVersionSummary> {
        let selected = self.selected_source_id.as_ref()?;
        self.versions
            .iter()
            .find(|version| version.source_id() == selected)
    }
}

impl<R> std::fmt::Debug for LibraryController<R>
where
    R: ApplicationRuntime,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LibraryController")
            .field("source_count", &self.sources.len())
            .field("version_count", &self.versions.len())
            .field("search_result_count", &self.search_results.len())
            .finish_non_exhaustive()
    }
}

fn selection_invalid() -> DesktopError {
    DesktopError::without_source(
        DesktopErrorCode::LocalLibrary,
        DesktopErrorReason::SelectionInvalid,
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    use radishmemory_application::{
        ApplicationIdentifierKind, DeletionOverallStatus, FileExportRequest, FileReadRequest,
        Identifier, Timestamp,
    };

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(std::path::PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "radishmemory-desktop-controller-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Default)]
    struct TestRuntime {
        next_id: u64,
        next_time: u8,
    }

    impl ApplicationRuntime for TestRuntime {
        type Error = io::Error;

        fn next_identifier(
            &mut self,
            kind: ApplicationIdentifierKind,
        ) -> Result<Identifier, Self::Error> {
            self.next_id += 1;
            let prefix = match kind {
                ApplicationIdentifierKind::Namespace => "namespace-",
                ApplicationIdentifierKind::Device => "device-",
                ApplicationIdentifierKind::OriginBinding => "origin-binding-",
                ApplicationIdentifierKind::Source => "source-",
                ApplicationIdentifierKind::Lineage => "lineage-",
                ApplicationIdentifierKind::Fragment => "fragment-",
                ApplicationIdentifierKind::DeleteRequest => "delete-request-",
                ApplicationIdentifierKind::DeletionEvidence => "deletion-evidence-",
            };
            Identifier::new(format!("{prefix}{:032x}", self.next_id))
                .map_err(|_| io::Error::other("invalid synthetic identifier"))
        }

        fn now(&mut self) -> Result<Timestamp, Self::Error> {
            self.next_time += 1;
            Timestamp::parse(&format!("2026-08-30T12:00:{:02}Z", self.next_time))
                .map_err(|_| io::Error::other("invalid synthetic timestamp"))
        }
    }

    #[test]
    fn host_profile_library_and_managed_bytes_survive_reopen_without_origin() {
        let directory = TestDirectory::new();
        let data_directory = directory.path().join("data");
        let input_directory = directory.path().join("input");
        fs::create_dir(&input_directory).unwrap();
        let input = input_directory.join("synthetic-note.md");
        fs::write(&input, b"# Synthetic\nlocal citation survives restart\n").unwrap();
        let paths = ApplicationPaths::from_data_directory(&data_directory).unwrap();

        let mut controller = LibraryController::bootstrap(&paths, TestRuntime::default()).unwrap();
        let request = FileReadRequest::new(&input, vec![input_directory.clone()]).unwrap();
        controller.import_source(&request).unwrap();
        controller.search("citation").unwrap();
        assert_eq!(controller.sources().len(), 1);
        assert_eq!(controller.search_results().len(), 1);
        drop(controller);
        fs::remove_file(&input).unwrap();

        let mut reopened = LibraryController::bootstrap(
            &paths,
            TestRuntime {
                next_time: 20,
                ..TestRuntime::default()
            },
        )
        .unwrap();
        reopened.search("restart").unwrap();
        assert_eq!(reopened.sources().len(), 1);
        assert_eq!(reopened.search_results().len(), 1);
        assert!(paths.profile_path().is_file());
        assert!(paths.database_path().is_file());
    }

    #[test]
    fn controller_updates_exports_history_and_deletes_only_managed_lineage() {
        let directory = TestDirectory::new();
        let data_directory = directory.path().join("data");
        let input_directory = directory.path().join("input");
        let export_directory = directory.path().join("export");
        fs::create_dir(&input_directory).unwrap();
        fs::create_dir(&export_directory).unwrap();
        let input = input_directory.join("synthetic-note.txt");
        let export = export_directory.join("historical-note.txt");
        let first_bytes = b"synthetic first desktop version\n";
        let second_bytes = b"synthetic second desktop version\n";
        fs::write(&input, first_bytes).unwrap();
        let paths = ApplicationPaths::from_data_directory(&data_directory).unwrap();

        let mut controller = LibraryController::bootstrap(&paths, TestRuntime::default()).unwrap();
        let request = FileReadRequest::new(&input, vec![input_directory.clone()]).unwrap();
        controller.import_source(&request).unwrap();
        fs::write(&input, second_bytes).unwrap();
        let request = FileReadRequest::new(&input, vec![input_directory.clone()]).unwrap();
        controller.update_selected(&request).unwrap();
        assert_eq!(controller.sources().len(), 1);
        assert_eq!(controller.versions().len(), 2);

        controller.search("second").unwrap();
        assert_eq!(controller.search_results().len(), 1);
        controller.search("first").unwrap();
        assert!(controller.search_results().is_empty());

        let historical_source_id = controller
            .versions()
            .iter()
            .find(|version| !version.current())
            .unwrap()
            .source_id()
            .clone();
        controller
            .select_source_version(&historical_source_id)
            .unwrap();
        let export_request =
            FileExportRequest::new(&export, vec![export_directory.clone()]).unwrap();
        controller.export_selected(&export_request).unwrap();
        assert_eq!(fs::read(&export).unwrap(), first_bytes);

        let evidence = controller.delete_selected_lineage().unwrap();
        assert_eq!(
            evidence.params().overall_status,
            DeletionOverallStatus::Completed
        );
        assert_eq!(evidence.params().component_results.len(), 10);
        assert!(controller.sources().is_empty());
        assert_eq!(fs::read(&input).unwrap(), second_bytes);
        assert_eq!(fs::read(&export).unwrap(), first_bytes);
        controller.verify().unwrap();
        drop(controller);

        let reopened = LibraryController::bootstrap(
            &paths,
            TestRuntime {
                next_time: 20,
                ..TestRuntime::default()
            },
        )
        .unwrap();
        assert!(reopened.sources().is_empty());
        reopened.verify().unwrap();
    }
}
