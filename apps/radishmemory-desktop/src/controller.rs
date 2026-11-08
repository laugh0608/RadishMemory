use radishmemory_application::{
    ApplicationRuntime, DeletionEvidence, FileCaptureReceipt, FileExportReceipt, FileExportRequest,
    FileReadRequest, Identifier, LocalLibrary, LocalLibraryConfig, NonEmptyText, NoteFormat,
    SOURCE_COMPARISON_LIMIT, Sensitivity, SourceArtifact, SourceCitationEvidence,
    SourceLineageSummary, SourceSearchFilters, SourceSearchResult, SourceVersionSummary,
};

use crate::comparison::{ComparisonCard, ComparisonSelection};
use crate::{
    ApplicationPaths, DesktopError, DesktopErrorCode, DesktopErrorReason,
    load_or_create_host_profile,
};

pub(crate) const CATALOG_PAGE_SIZE: usize = 200;
pub(crate) const CATALOG_READ_LIMIT: usize = CATALOG_PAGE_SIZE + 1;
const SEARCH_RESULT_LIMIT: usize = 50;
const EXCERPT_CHARACTERS: usize = 400;

pub struct LibraryController<R, P = radishmemory_source_vault::PlatformKeyProvider>
where
    R: ApplicationRuntime,
    P: radishmemory_source_vault::LibraryProvider,
{
    library: crate::backend::Library<R, P>,
    pub(crate) refresh_error: Option<DesktopError>,
    view: LibraryView,
    comparison: Vec<ComparisonSelection>,
}

#[derive(Clone, Default)]
pub struct LibraryView {
    sources: Vec<SourceLineageSummary>,
    catalog_offset: u64,
    has_next_catalog_page: bool,
    selected_lineage_id: Option<Identifier>,
    versions: Vec<SourceVersionSummary>,
    selected_source_id: Option<Identifier>,
    search_results: Vec<SourceSearchResult>,
    submitted_query: String,
    selected_fragment: Option<SelectedFragment>,
    comparison: Vec<ComparisonCard>,
}

#[derive(Clone, Copy)]
struct SelectedFragment {
    result_index: usize,
    start: usize,
}

pub(crate) struct FragmentExcerpt<'a> {
    pub(crate) result: &'a SourceSearchResult,
    pub(crate) text: &'a str,
    pub(crate) byte_start: u64,
    pub(crate) byte_end: u64,
    pub(crate) previous: Option<usize>,
    pub(crate) next: Option<usize>,
}

impl<R, P: radishmemory_source_vault::LibraryProvider> LibraryController<R, P>
where
    R: ApplicationRuntime,
    P: radishmemory_source_vault::LibraryProvider,
{
    pub(crate) fn from_encrypted_snapshot(
        library: radishmemory_application::EncryptedLibrary<R, P>,
        snapshot: &radishmemory_source_vault::LibraryReadSnapshot,
    ) -> Self {
        Self {
            library: crate::backend::Library::Encrypted {
                library,
                capture: None,
                deletion: None,
            },
            refresh_error: None,
            view: LibraryView::from_read_snapshot(snapshot),
            comparison: Vec::new(),
        }
    }

    /// Only the explicit encrypted Refresh command uses this combined action.
    /// Post-write refresh keeps its existing independent session semantics.
    pub(crate) fn refresh_encrypted_snapshot(
        &mut self,
    ) -> Result<radishmemory_source_vault::LibraryReadSnapshot, DesktopError> {
        let previous = self.view.selected_lineage_id.clone();
        self.view = LibraryView::default();
        self.refresh_error = None;
        let crate::backend::Library::Encrypted { library, .. } = &self.library else {
            return Err(crate::backend::recovery_required());
        };
        let snapshot = library
            .read_snapshot(previous.as_ref(), CATALOG_READ_LIMIT)
            .map_err(crate::backend::app_error)?;
        self.view = LibraryView::from_read_snapshot(&snapshot);
        self.refresh_comparison()?;
        Ok(snapshot)
    }

    pub(crate) fn from_library(
        library: crate::backend::Library<R, P>,
    ) -> Result<Self, DesktopError> {
        let mut controller = Self {
            library,
            refresh_error: None,
            view: LibraryView::default(),
            comparison: Vec::new(),
        };
        controller.refresh_sources()?;
        Ok(controller)
    }

    pub(crate) fn snapshot(&self) -> LibraryView {
        self.view.clone()
    }
    pub(crate) fn backend(&mut self) -> &mut crate::backend::Library<R, P> {
        &mut self.library
    }
    pub(crate) fn refresh_after_commit(&mut self, selected: Option<&Identifier>) {
        self.refresh_error = self
            .refresh_sources()
            .and_then(|()| {
                if let Some(lineage) = selected {
                    self.load_selection(lineage, None)
                } else {
                    Ok(())
                }
            })
            .err();
    }
    fn require_no_original(&self) -> Result<(), DesktopError> {
        if self.library.holds_original() {
            Err(crate::backend::recovery_required())
        } else {
            Ok(())
        }
    }

    pub fn refresh_sources(&mut self) -> Result<(), DesktopError> {
        let previous = self.view.selected_lineage_id.clone();
        self.view = LibraryView::default();
        self.load_catalog_page(0, previous.as_ref())?;
        self.refresh_comparison()
    }

    pub(crate) fn show_catalog_page(&mut self, offset: u64) -> Result<(), DesktopError> {
        self.load_catalog_page(offset, None)
    }

    fn load_catalog_page(
        &mut self,
        mut offset: u64,
        preferred: Option<&Identifier>,
    ) -> Result<(), DesktopError> {
        let mut sources = self
            .library
            .list_sources(offset, CATALOG_READ_LIMIT)
            .map_err(crate::backend::app_error)?;
        // Another window can remove the last page between clicks.
        if offset != 0 && sources.is_empty() {
            offset = 0;
            sources = self
                .library
                .list_sources(offset, CATALOG_READ_LIMIT)
                .map_err(crate::backend::app_error)?;
        }
        let visible = &sources[..sources.len().min(CATALOG_PAGE_SIZE)];
        let selected = preferred
            .and_then(|id| visible.iter().find(|source| source.lineage_id() == id))
            .or_else(|| visible.first());
        if let Some(source) = selected {
            self.load_selection(source.lineage_id(), None)?;
        } else {
            self.view.selected_lineage_id = None;
            self.view.selected_source_id = None;
            self.view.selected_fragment = None;
            self.view.versions.clear();
        }
        self.view.set_catalog_page(offset, sources);
        Ok(())
    }

    pub fn select_lineage(&mut self, lineage_id: &Identifier) -> Result<(), DesktopError> {
        if !self
            .sources
            .iter()
            .any(|source| source.lineage_id() == lineage_id)
        {
            return Err(selection_invalid());
        }
        self.load_selection(lineage_id, None)
    }

    pub(crate) fn select_search_result(
        &mut self,
        lineage_id: &Identifier,
        source_id: &Identifier,
        fragment_id: &Identifier,
    ) -> Result<(), DesktopError> {
        let result_index = self
            .search_results
            .iter()
            .position(|result| {
                result.lineage_id() == lineage_id
                    && result.source_id() == source_id
                    && result.fragment_id() == fragment_id
            })
            .ok_or_else(selection_invalid)?;
        self.load_selection(lineage_id, Some(source_id))?;
        // Literal positioning is a navigation aid. MATCH may find text by
        // normalization or separate query terms, so the whole fragment stays browsable.
        let start = self.view.search_results[result_index]
            .content()
            .as_str()
            .find(self.view.submitted_query.trim())
            .unwrap_or(0);
        self.view.selected_fragment = Some(SelectedFragment {
            result_index,
            start,
        });
        Ok(())
    }

    pub(crate) fn add_to_comparison(
        &mut self,
        fragment_id: &Identifier,
    ) -> Result<(), DesktopError> {
        self.require_no_original()?;
        if self.comparison.len() == SOURCE_COMPARISON_LIMIT
            || self
                .comparison
                .iter()
                .any(|item| item.citation.fragment_id() == fragment_id)
        {
            return Err(selection_invalid());
        }
        let result = self
            .view
            .search_results
            .iter()
            .find(|result| result.fragment_id() == fragment_id)
            .ok_or_else(selection_invalid)?;
        self.comparison.push(ComparisonSelection {
            citation: result.citation().clone(),
            start: result
                .content()
                .as_str()
                .find(self.view.submitted_query.trim())
                .unwrap_or(0),
        });
        if let Err(error) = self.refresh_comparison() {
            self.comparison.pop();
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn remove_from_comparison(&mut self, fragment_id: &Identifier) {
        self.comparison
            .retain(|item| item.citation.fragment_id() != fragment_id);
        self.view
            .comparison
            .retain(|card| &card.fragment_id != fragment_id);
    }

    pub(crate) fn refresh_comparison(&mut self) -> Result<(), DesktopError> {
        self.resolve_comparison().map(|_| ())
    }

    fn resolve_comparison(&mut self) -> Result<Vec<Option<SourceCitationEvidence>>, DesktopError> {
        self.view.clear_comparison_content();
        if self.comparison.is_empty() || self.library.holds_original() {
            self.view.comparison = self.comparison.iter().map(|item| item.card(None)).collect();
            return Ok(Vec::new());
        }
        let citations = self
            .comparison
            .iter()
            .map(|item| item.citation.clone())
            .collect::<Vec<_>>();
        let resolved = self
            .library
            .resolve_source_citations(&citations)
            .map_err(crate::backend::app_error)?;
        self.view.comparison = self
            .comparison
            .iter()
            .zip(&resolved)
            .map(|(item, evidence)| item.card(evidence.as_ref()))
            .collect();
        // A refreshed citation can invalidate an older search snapshot from another window.
        if self.view.search_results.iter().any(|result| {
            self.view.comparison.iter().any(|card| {
                &card.fragment_id == result.fragment_id()
                    && card
                        .excerpt
                        .as_ref()
                        .is_none_or(|excerpt| excerpt.historical)
            })
        }) {
            self.view.clear_search_results();
        }
        Ok(resolved)
    }

    pub(crate) fn open_comparison_source(
        &mut self,
        fragment_id: &Identifier,
    ) -> Result<Option<(SourceArtifact, usize)>, DesktopError> {
        self.require_no_original()?;
        let index = self
            .comparison
            .iter()
            .position(|item| item.citation.fragment_id() == fragment_id)
            .ok_or_else(selection_invalid)?;
        let Some(evidence) = self.resolve_comparison()?.into_iter().nth(index).flatten() else {
            return Ok(None);
        };
        let offset = usize::try_from(evidence.result.byte_start())
            .map_err(|_| selection_invalid())?
            + self.comparison[index].start;
        self.view.selected_lineage_id = Some(evidence.result.lineage_id().clone());
        self.view.selected_source_id = Some(evidence.result.source_id().clone());
        self.view.versions = evidence.versions;
        self.view.selected_fragment = None;
        Ok(Some((evidence.original, offset)))
    }

    fn load_selection(
        &mut self,
        lineage_id: &Identifier,
        source_id: Option<&Identifier>,
    ) -> Result<(), DesktopError> {
        let versions = self
            .library
            .list_source_versions(lineage_id)
            .map_err(crate::backend::app_error)?;
        let selected = versions
            .iter()
            .find(|version| match source_id {
                Some(id) => version.source_id() == id,
                None => version.current(),
            })
            .ok_or_else(selection_invalid)?;
        self.view.selected_lineage_id = Some(lineage_id.clone());
        self.view.selected_source_id = Some(selected.source_id().clone());
        self.view.selected_fragment = None;
        self.view.versions = versions;
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
        self.view.selected_source_id = Some(source_id.clone());
        self.view.selected_fragment = None;
        Ok(())
    }

    pub fn import_source(
        &mut self,
        request: &FileReadRequest,
    ) -> Result<FileCaptureReceipt, DesktopError> {
        self.require_no_original()?;
        let receipt = self
            .library
            .import_new_source(request)
            .map_err(|source| DesktopError::application(&source))?;
        self.refresh_after_commit(Some(receipt.lineage_id()));
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
        self.require_no_original()?;
        let receipt = self
            .library
            .update_source(&lineage_id, request)
            .map_err(|source| DesktopError::application(&source))?;
        self.refresh_after_commit(Some(receipt.lineage_id()));
        Ok(receipt)
    }

    pub(crate) fn save_note(
        &mut self,
        format: NoteFormat,
        title: Option<NonEmptyText>,
        body: String,
    ) -> Result<FileCaptureReceipt, DesktopError> {
        self.require_no_original()?;
        let receipt = self
            .library
            .save_note(format, title, body)
            .map_err(crate::backend::app_error)?;
        self.refresh_after_commit(Some(receipt.lineage_id()));
        Ok(receipt)
    }

    pub(crate) fn correct_note(
        &mut self,
        lineage: &Identifier,
        base: &Identifier,
        body: String,
    ) -> Result<FileCaptureReceipt, DesktopError> {
        self.require_no_original()?;
        let receipt = self
            .library
            .correct_note(lineage, base, body)
            .map_err(crate::backend::app_error)?;
        self.refresh_after_commit(Some(receipt.lineage_id()));
        Ok(receipt)
    }

    pub(crate) fn selected_original(&self) -> Result<SourceArtifact, DesktopError> {
        self.require_no_original()?;
        let id = self.selected_source_id().ok_or_else(selection_invalid)?;
        self.library
            .get_source(id)
            .map_err(crate::backend::app_error)?
            .ok_or_else(selection_invalid)
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
        self.search_filtered(query, SourceSearchFilters::default())
    }

    pub(crate) fn search_filtered(
        &mut self,
        query: &str,
        filters: SourceSearchFilters,
    ) -> Result<(), DesktopError> {
        self.view.clear_search_results();
        if query.trim().is_empty() {
            return self.refresh_comparison();
        }
        let request = NonEmptyText::new(query.to_owned()).map_err(|_| selection_invalid())?;
        self.view.search_results = self
            .library
            .search_sources(
                request,
                SEARCH_RESULT_LIMIT,
                [Sensitivity::Personal],
                filters,
            )
            .map_err(|source| DesktopError::application(&source))?;
        self.view.submitted_query.push_str(query);
        self.refresh_comparison()
    }

    pub fn delete_selected_lineage(&mut self) -> Result<DeletionEvidence, DesktopError> {
        let lineage_id = self
            .selected_lineage_id
            .clone()
            .ok_or_else(selection_invalid)?;
        self.require_no_original()?;
        let evidence = self
            .library
            .delete_source_lineage(&lineage_id)
            .map_err(|source| DesktopError::application(&source))?;
        self.view.clear_search_results();
        self.view.selected_lineage_id = None;
        self.view.selected_source_id = None;
        self.view.versions.clear();
        self.refresh_after_commit(None);
        Ok(evidence)
    }

    pub fn verify(&self) -> Result<(), DesktopError> {
        self.library
            .verify_library()
            .map_err(|source| DesktopError::application(&source))
    }

    pub(crate) fn change_recall_profile(
        &mut self,
        target: radishmemory_application::RecallProfile,
    ) -> Result<radishmemory_application::RecallProfile, DesktopError> {
        self.require_no_original()?;
        let profile = self
            .library
            .change_recall_profile(target)
            .map_err(crate::backend::app_error)?;
        self.refresh_after_commit(None);
        Ok(profile)
    }

    pub fn rebuild(&mut self) -> Result<(), DesktopError> {
        self.library
            .rebuild_recall()
            .map_err(|source| DesktopError::application(&source))?;
        self.refresh_after_commit(None);
        Ok(())
    }
}

impl LibraryView {
    fn from_read_snapshot(snapshot: &radishmemory_source_vault::LibraryReadSnapshot) -> Self {
        let mut view = Self {
            selected_lineage_id: snapshot.selected_lineage_id.clone(),
            selected_source_id: snapshot.selected_source_id.clone(),
            versions: snapshot.versions.clone(),
            ..Self::default()
        };
        view.set_catalog_page(0, snapshot.sources.clone());
        view
    }

    fn set_catalog_page(&mut self, offset: u64, mut sources: Vec<SourceLineageSummary>) {
        self.catalog_offset = offset;
        self.has_next_catalog_page = sources.len() > CATALOG_PAGE_SIZE;
        sources.truncate(CATALOG_PAGE_SIZE);
        self.sources = sources;
    }

    #[must_use]
    pub fn sources(&self) -> &[SourceLineageSummary] {
        &self.sources
    }

    pub(crate) fn catalog_offset(&self) -> u64 {
        self.catalog_offset
    }

    pub(crate) fn has_next_catalog_page(&self) -> bool {
        self.has_next_catalog_page
    }

    #[must_use]
    pub fn versions(&self) -> &[SourceVersionSummary] {
        &self.versions
    }

    #[must_use]
    pub fn search_results(&self) -> &[SourceSearchResult] {
        &self.search_results
    }

    pub(crate) fn clear_search_results(&mut self) {
        self.search_results.clear();
        self.submitted_query.clear();
        self.selected_fragment = None;
    }

    pub(crate) fn comparison(&self) -> &[ComparisonCard] {
        &self.comparison
    }

    pub(crate) fn clear_comparison_content(&mut self) {
        for card in &mut self.comparison {
            card.excerpt = None;
        }
    }

    pub(crate) fn show_fragment_excerpt(&mut self, start: usize) {
        if let Some(selection) = &mut self.selected_fragment {
            selection.start = start;
        }
    }

    pub(crate) fn selected_excerpt(&self) -> Option<FragmentExcerpt<'_>> {
        let SelectedFragment {
            result_index,
            start,
        } = self.selected_fragment?;
        let result = &self.search_results[result_index];
        let content = result.content().as_str();
        let (text, previous, next) = text_excerpt(content, start);
        Some(FragmentExcerpt {
            result,
            text,
            byte_start: result.byte_start() + start as u64,
            byte_end: result.byte_start() + (start + text.len()) as u64,
            previous,
            next,
        })
    }

    #[must_use]
    pub fn selected_lineage_id(&self) -> Option<&Identifier> {
        self.selected_lineage_id.as_ref()
    }

    #[must_use]
    pub fn selected_source_id(&self) -> Option<&Identifier> {
        self.selected_source_id.as_ref()
    }

    /// The selected lineage's catalog row, if visible on the current page.
    #[must_use]
    pub fn selected_lineage(&self) -> Option<&SourceLineageSummary> {
        let selected = self.selected_lineage_id.as_ref()?;
        self.sources
            .iter()
            .find(|source| source.lineage_id() == selected)
    }

    pub(crate) fn current_version(&self) -> Option<&SourceVersionSummary> {
        self.versions.iter().find(|version| version.current())
    }

    #[must_use]
    pub fn selected_version(&self) -> Option<&SourceVersionSummary> {
        let selected = self.selected_source_id.as_ref()?;
        self.versions
            .iter()
            .find(|version| version.source_id() == selected)
    }
}

pub(crate) fn text_excerpt(content: &str, start: usize) -> (&str, Option<usize>, Option<usize>) {
    let remaining = &content[start..];
    let len = remaining
        .char_indices()
        .nth(EXCERPT_CHARACTERS)
        .map_or(remaining.len(), |(offset, _)| offset);
    let end = start + len;
    let previous = (start > 0).then(|| {
        content[..start]
            .char_indices()
            .rev()
            .nth(EXCERPT_CHARACTERS - 1)
            .map_or(0, |(offset, _)| offset)
    });
    (
        &content[start..end],
        previous,
        (end < content.len()).then_some(end),
    )
}

impl<R, P> std::fmt::Debug for LibraryController<R, P>
where
    R: ApplicationRuntime,
    P: radishmemory_source_vault::LibraryProvider,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LibraryController")
            .field("source_count", &self.view.sources.len())
            .field("version_count", &self.view.versions.len())
            .field("search_result_count", &self.view.search_results.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn selection_invalid() -> DesktopError {
    DesktopError::without_source(
        DesktopErrorCode::LocalLibrary,
        DesktopErrorReason::SelectionInvalid,
        false,
    )
}

impl<R: ApplicationRuntime> LibraryController<R> {
    pub fn bootstrap(paths: &ApplicationPaths, mut runtime: R) -> Result<Self, DesktopError> {
        let profile = load_or_create_host_profile(paths, &mut runtime)?;
        let config = LocalLibraryConfig::phase1_local(
            profile.namespace_id().clone(),
            profile.device_id().clone(),
        )
        .map_err(|source| DesktopError::application(&source))?;
        let library = LocalLibrary::open(paths.database_path(), runtime, config)
            .map_err(|source| DesktopError::application(&source))?;
        Self::from_library(crate::backend::Library::Plain(library))
    }
}
impl<R: ApplicationRuntime, P: radishmemory_source_vault::LibraryProvider> std::ops::Deref
    for LibraryController<R, P>
{
    type Target = LibraryView;
    fn deref(&self) -> &Self::Target {
        &self.view
    }
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
