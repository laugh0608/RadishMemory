//! Desktop-local evidence and extractive mock answers. Only current, revalidated
//! source citations may contribute quotes.
use super::{LibraryController, citation_is_current, selection_invalid, text_excerpt};
use crate::DesktopError;
use radishmemory_application::{
    ApplicationRuntime, Citation, NonEmptyText, SOURCE_COMPARISON_LIMIT, Sensitivity,
    SourceArtifact, SourceCitationEvidence, SourceSearchFilters,
};
use radishmemory_source_vault::LibraryProvider;

pub(crate) const QUESTION_INPUT_LIMIT: usize = 512;

#[derive(Clone, Default, Eq, PartialEq)]
pub(crate) struct QuestionInput {
    pub question: String,
    pub keywords: String,
}

impl QuestionInput {
    pub fn is_valid(&self) -> bool {
        !self.question.trim().is_empty()
            && self.question.chars().count() <= QUESTION_INPUT_LIMIT
            && self.keywords.chars().count() <= QUESTION_INPUT_LIMIT
    }

    fn query(&self) -> &str {
        if self.keywords.trim().is_empty() {
            self.question.trim()
        } else {
            self.keywords.trim()
        }
    }
}

#[derive(Clone)]
pub(crate) struct QuestionView {
    pub input: QuestionInput,
    pub evidence: Vec<QuestionEvidence>,
    pub state: QuestionState,
}

impl QuestionView {
    pub(super) fn invalidate(&mut self) {
        self.evidence.clear();
        self.state = QuestionState::Changed;
    }
}

#[derive(Clone)]
pub(crate) enum QuestionState {
    Insufficient,
    Changed,
    Preview,
    MockAnswer(Vec<String>),
}

#[derive(Clone)]
pub(crate) struct QuestionEvidence {
    citation: Citation,
    start: usize,
    pub title: Option<NonEmptyText>,
    pub version: u64,
    pub byte_start: u64,
    pub byte_end: u64,
    pub text: String,
}

impl<R: ApplicationRuntime, P: LibraryProvider> LibraryController<R, P> {
    pub(crate) fn clear_question(&mut self) {
        self.view.clear_question();
    }

    pub(crate) fn preview_question(
        &mut self,
        input: QuestionInput,
        filters: SourceSearchFilters,
    ) -> Result<(), DesktopError> {
        self.require_no_original()?;
        if !input.is_valid() {
            return Err(selection_invalid());
        }
        let query = input.query();
        let request = NonEmptyText::new(query).map_err(|_| selection_invalid())?;
        let hits = self
            .library
            .search_sources(
                request,
                SOURCE_COMPARISON_LIMIT,
                [Sensitivity::Personal],
                filters,
            )
            .map_err(crate::backend::app_error)?;
        let candidates = hits
            .iter()
            .filter_map(|hit| {
                let content = hit.content().as_str();
                let start = content.find(query).or_else(|| {
                    query
                        .split(|ch: char| !ch.is_alphanumeric())
                        .find(|word| !word.is_empty())
                        .and_then(|word| content.find(word))
                })?;
                Some((hit.citation().clone(), start))
            })
            .collect::<Vec<_>>();
        let mut evidence = Vec::new();
        if !candidates.is_empty() {
            let citations = candidates
                .iter()
                .map(|(citation, _)| citation.clone())
                .collect::<Vec<_>>();
            let resolved = self
                .library
                .resolve_source_citations(&citations)
                .map_err(crate::backend::app_error)?;
            for ((citation, start), entry) in candidates.into_iter().zip(resolved) {
                let Some(entry) = entry.filter(citation_is_current) else {
                    continue;
                };
                let result = entry.result;
                let (text, _, _) = text_excerpt(result.content().as_str(), start);
                let byte_start = result.byte_start() + start as u64;
                evidence.push(QuestionEvidence {
                    citation,
                    start,
                    title: result.title().cloned(),
                    version: result.version().get(),
                    byte_start,
                    byte_end: byte_start + text.len() as u64,
                    text: text.to_owned(),
                });
            }
        }
        let state = if evidence.is_empty() {
            QuestionState::Insufficient
        } else {
            QuestionState::Preview
        };
        self.view.question = Some(QuestionView {
            input,
            evidence,
            state,
        });
        Ok(())
    }

    pub(crate) fn run_local_answer(&mut self) -> Result<(), DesktopError> {
        let mut view = self.view.question.take().ok_or_else(selection_invalid)?;
        if !matches!(view.state, QuestionState::Preview) {
            return Err(selection_invalid());
        }
        if self.revalidate_question(&mut view)?.is_some() {
            let answer = view
                .evidence
                .iter()
                .enumerate()
                .map(|(index, evidence)| format!("[{}] {}", index + 1, evidence.text))
                .collect();
            view.state = QuestionState::MockAnswer(answer);
        }
        self.view.question = Some(view);
        Ok(())
    }

    pub(crate) fn open_question_source(
        &mut self,
        index: usize,
    ) -> Result<Option<(SourceArtifact, usize)>, DesktopError> {
        let mut view = self.view.question.take().ok_or_else(selection_invalid)?;
        let start = view
            .evidence
            .get(index)
            .ok_or_else(selection_invalid)?
            .start;
        let resolved = self.revalidate_question(&mut view)?;
        self.view.question = Some(view);
        let Some(entry) = resolved.and_then(|entries| entries.into_iter().nth(index)) else {
            return Ok(None);
        };
        self.select_citation_evidence(entry, start).map(Some)
    }

    fn revalidate_question(
        &mut self,
        view: &mut QuestionView,
    ) -> Result<Option<Vec<SourceCitationEvidence>>, DesktopError> {
        self.require_no_original()?;
        let citations = view
            .evidence
            .iter()
            .map(|evidence| evidence.citation.clone())
            .collect::<Vec<_>>();
        let resolved = self
            .library
            .resolve_source_citations(&citations)
            .map_err(crate::backend::app_error)?
            .into_iter()
            .map(|entry| entry.filter(citation_is_current))
            .collect::<Option<Vec<_>>>();
        if resolved.is_none() {
            view.invalidate();
            self.refresh_changed_sources(None)?;
        }
        Ok(resolved)
    }
}
