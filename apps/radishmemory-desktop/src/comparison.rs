use crate::controller::text_excerpt;
use radishmemory_application::{Citation, Identifier, NonEmptyText, SourceCitationEvidence};

pub(crate) struct ComparisonSelection {
    pub citation: Citation,
    pub start: usize,
}

#[derive(Clone)]
pub(crate) struct ComparisonCard {
    pub fragment_id: Identifier,
    pub excerpt: Option<ComparisonExcerpt>,
}

#[derive(Clone)]
pub(crate) struct ComparisonExcerpt {
    pub title: Option<NonEmptyText>,
    pub version: u64,
    pub historical: bool,
    pub byte_start: u64,
    pub byte_end: u64,
    pub text: String,
}

impl ComparisonSelection {
    pub fn card(&self, evidence: Option<&SourceCitationEvidence>) -> ComparisonCard {
        let excerpt = evidence.map(|evidence| {
            let result = &evidence.result;
            let (text, _, _) = text_excerpt(result.content().as_str(), self.start);
            let byte_start = result.byte_start() + self.start as u64;
            ComparisonExcerpt {
                title: result.title().cloned(),
                version: result.version().get(),
                historical: evidence
                    .versions
                    .iter()
                    .any(|version| version.current() && version.source_id() != result.source_id()),
                byte_start,
                byte_end: byte_start + text.len() as u64,
                text: text.to_owned(),
            }
        });
        ComparisonCard {
            fragment_id: self.citation.fragment_id().clone(),
            excerpt,
        }
    }
}
