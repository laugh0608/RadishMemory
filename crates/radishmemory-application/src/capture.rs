//! Canonical capture preparation shared by plaintext and encrypted library writes.
use crate::{
    ApplicationError, ApplicationErrorReason, ApplicationIdentifierKind, ApplicationOperation,
    ApplicationRuntime, FileReadRequest, Identifier, LocalLibraryConfig, NonEmptyText,
    SourceArtifact, Timestamp,
};
use radishmemory_core::{
    MediaType, ProducerRef, ProducerType, SourceCapture, SourceKind, SourceLineageState,
    TextSourceCapturePlan, Version, source_origin_binding_id_is_valid,
};
use radishmemory_file_entry::{MAX_FILE_BYTES, build_source_capture, read_file_snapshot};

pub(crate) const NOTE_PRODUCER_ID: &str = "radishmemory-desktop-text-entry";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoteFormat {
    Text,
    Markdown,
}

/// The stored producer selects the editor; reads and writes still require library authority.
#[must_use]
pub fn is_note_producer(producer: &ProducerRef) -> bool {
    producer.producer_type() == ProducerType::System
        && producer.producer_id().as_str() == NOTE_PRODUCER_ID
}

pub(crate) fn prepare<R: ApplicationRuntime>(
    runtime: &mut R,
    config: &LocalLibraryConfig,
    request: &FileReadRequest,
    current: Option<(SourceLineageState, SourceArtifact)>,
) -> Result<SourceCapture, ApplicationError> {
    let operation = if current.is_some() {
        ApplicationOperation::UpdateSource
    } else {
        ApplicationOperation::ImportNewSource
    };
    if current
        .as_ref()
        .is_some_and(|(_, source)| is_note_producer(&source.params().producer))
    {
        return Err(ApplicationError::invalid_note_request(
            operation,
            ApplicationErrorReason::NoteEntryMismatch,
        ));
    }
    let observed_at = now(runtime, operation)?;
    let snapshot = read_file_snapshot(request)
        .map_err(|error| ApplicationError::file_entry(operation, error))?;
    let plan = capture_plan(runtime, config, operation, observed_at, current.as_ref())?;
    build_source_capture(snapshot, plan)
        .map_err(|error| ApplicationError::canonical(operation, error))
}

pub(crate) fn prepare_note<R: ApplicationRuntime>(
    runtime: &mut R,
    config: &LocalLibraryConfig,
    format: NoteFormat,
    title: Option<NonEmptyText>,
    body: String,
) -> Result<SourceCapture, ApplicationError> {
    let (kind, media) = match format {
        NoteFormat::Text => (SourceKind::Text, MediaType::TextPlain),
        NoteFormat::Markdown => (SourceKind::Markdown, MediaType::TextMarkdown),
    };
    let operation = ApplicationOperation::ImportNewSource;
    let body = note_body(operation, body)?;
    let observed_at = now(runtime, operation)?;
    let mut plan = capture_plan(runtime, config, operation, observed_at, None)?;
    plan.source_producer = ProducerRef::new(
        ProducerType::System,
        Identifier::new(NOTE_PRODUCER_ID)
            .map_err(|error| ApplicationError::canonical(operation, error))?,
        NonEmptyText::new("1").map_err(|error| ApplicationError::canonical(operation, error))?,
    );
    plan.build(kind, media, body, title)
        .map_err(|error| ApplicationError::canonical(operation, error))
}

pub(crate) fn prepare_note_correction<R: ApplicationRuntime>(
    runtime: &mut R,
    config: &LocalLibraryConfig,
    base_source_id: &Identifier,
    body: String,
    current: (SourceLineageState, SourceArtifact),
) -> Result<SourceCapture, ApplicationError> {
    let operation = ApplicationOperation::UpdateSource;
    if current.0.current_source_id() != base_source_id {
        return Err(ApplicationError::invalid_note_request(
            operation,
            ApplicationErrorReason::NoteVersionChanged,
        ));
    }
    let source = current.1.params();
    if !is_note_producer(&source.producer) {
        return Err(ApplicationError::invalid_note_request(
            operation,
            ApplicationErrorReason::NoteEntryMismatch,
        ));
    }
    let body = note_body(operation, body)?;
    let observed_at = now(runtime, operation)?;
    let mut plan = capture_plan(runtime, config, operation, observed_at, Some(&current))?;
    plan.source_producer = source.producer.clone();
    plan.build(
        source.source_kind,
        source.media_type,
        body,
        source.title.clone(),
    )
    .map_err(|error| ApplicationError::canonical(operation, error))
}

fn note_body(
    operation: ApplicationOperation,
    body: String,
) -> Result<NonEmptyText, ApplicationError> {
    if !note_body_is_valid(&body) {
        return Err(ApplicationError::invalid_note_request(
            operation,
            ApplicationErrorReason::NoteBodyInvalid,
        ));
    }
    NonEmptyText::new(body).map_err(|error| ApplicationError::canonical(operation, error))
}

#[must_use]
pub fn note_body_is_valid(body: &str) -> bool {
    body.len() as u64 <= MAX_FILE_BYTES && !body.contains('\0') && !body.trim().is_empty()
}

fn capture_plan<R: ApplicationRuntime>(
    runtime: &mut R,
    config: &LocalLibraryConfig,
    operation: ApplicationOperation,
    observed_at: Timestamp,
    current: Option<&(SourceLineageState, SourceArtifact)>,
) -> Result<TextSourceCapturePlan, ApplicationError> {
    let (origin_binding_id, source_id, lineage_id, version, supersedes_source_ids, governance) =
        if let Some((state, source)) = current {
            let version = state
                .current_version()
                .get()
                .checked_add(1)
                .ok_or_else(|| ApplicationError::invalid_runtime_identifier(operation))?;
            (
                state.origin_binding_id().clone(),
                next_identifier(runtime, operation, ApplicationIdentifierKind::Source)?,
                state.lineage_id().clone(),
                version,
                vec![state.current_source_id().clone()],
                source.params().governance.clone(),
            )
        } else {
            let origin =
                next_identifier(runtime, operation, ApplicationIdentifierKind::OriginBinding)?;
            if !source_origin_binding_id_is_valid(origin.as_str()) {
                return Err(ApplicationError::invalid_runtime_identifier(operation));
            }
            (
                origin,
                next_identifier(runtime, operation, ApplicationIdentifierKind::Source)?,
                next_identifier(runtime, operation, ApplicationIdentifierKind::Lineage)?,
                1,
                Vec::new(),
                config.governance.clone(),
            )
        };
    Ok(TextSourceCapturePlan {
        namespace_id: config.namespace_id.clone(),
        origin_binding_id,
        source_id,
        lineage_id,
        version: Version::new(version)
            .map_err(|error| ApplicationError::canonical(operation, error))?,
        supersedes_source_ids,
        fragment_id: next_identifier(runtime, operation, ApplicationIdentifierKind::Fragment)?,
        observed_at,
        captured_at: now(runtime, operation)?,
        governance,
        source_producer: config.source_producer.clone(),
        segmenter: config.segmenter.clone(),
    })
}

pub(crate) fn next_identifier<R: ApplicationRuntime>(
    runtime: &mut R,
    operation: ApplicationOperation,
    kind: ApplicationIdentifierKind,
) -> Result<Identifier, ApplicationError> {
    runtime
        .next_identifier(kind)
        .map_err(|error| ApplicationError::identifier_generation(operation, error))
}
pub(crate) fn now<R: ApplicationRuntime>(
    runtime: &mut R,
    operation: ApplicationOperation,
) -> Result<Timestamp, ApplicationError> {
    runtime
        .now()
        .map_err(|error| ApplicationError::clock(operation, error))
}
