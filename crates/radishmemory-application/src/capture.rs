//! File snapshot preparation shared by inline and encrypted application writes.
use crate::{
    ApplicationError, ApplicationIdentifierKind, ApplicationOperation, ApplicationRuntime,
    FileReadRequest, Identifier, LocalLibraryConfig, SourceArtifact, Timestamp,
};
use radishmemory_core::{
    SourceCapture, SourceLineageState, Version, source_origin_binding_id_is_valid,
};
use radishmemory_file_entry::{FileCapturePlan, build_source_capture, read_file_snapshot};

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
    let observed_at = now(runtime, operation)?;
    let snapshot = read_file_snapshot(request)
        .map_err(|error| ApplicationError::file_entry(operation, error))?;
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
    let plan = FileCapturePlan {
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
    };
    build_source_capture(snapshot, plan)
        .map_err(|error| ApplicationError::canonical(operation, error))
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
