//! Fixed catalog/recovery read bundle. No caller callback can publish partial results.
use super::*;
use crate::CaptureAbandonmentTarget;
use radishmemory_core::DeleteRequest;

/// Owned read results, released only after the final key and object checks.
/// Contains no key/session and does not authorize abandonment or deletion.
pub struct LibraryReadSnapshot {
    pub sources: Vec<SourceLineageSummary>,
    pub selected_lineage_id: Option<Identifier>,
    pub selected_source_id: Option<Identifier>,
    pub versions: Vec<SourceVersionSummary>,
    pub abandonment_target: Option<CaptureAbandonmentTarget>,
    pub deletion_requests: Vec<DeleteRequest>,
}

#[cfg(test)]
mod tests;

impl fmt::Debug for LibraryReadSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LibraryReadSnapshot([REDACTED])")
    }
}

pub(crate) fn snapshot(
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    request: &SourceCatalogRequest,
    preferred_lineage: Option<&Identifier>,
    mut load_existing: impl FnMut() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
) -> Result<LibraryReadSnapshot> {
    if request.namespace_id().as_str() != namespace {
        return Err(SourceVaultError::new(
            SourceVaultErrorCode::MetadataMismatch,
            "snapshot namespace mismatch",
        )
        .into());
    }
    // The loader connects lazily, only after schema/profile/identity/lock checks.
    // Both invocations must use the same store identity observation.
    let reader = open(directory, namespace, device, &mut load_existing)?;
    let sources = reader.list_source_lineages(request)?;
    let selected = preferred_lineage
        .and_then(|id| sources.iter().find(|source| source.lineage_id() == id))
        .or_else(|| sources.first());
    let selected_lineage_id = selected.map(|source| source.lineage_id().clone());
    let selected_source_id = selected.map(|source| source.current_source_id().clone());
    let versions = match &selected_lineage_id {
        Some(id) => reader.list_source_versions(request.namespace_id(), id)?,
        None => Vec::new(),
    };
    // Use the existing exact target query under the same authenticated reader.
    // Inspecting a target neither persists a decision nor authorizes cleanup.
    let abandonment_target = reader.read(request.namespace_id(), |_| {
        reader.database.pending_abandonment_target()
    })?;
    let deletion_requests = reader.unfinished_delete_requests(request.namespace_id())?;
    let rechecked_key = load_existing()?;
    if !reader.key.same_key(&rechecked_key) {
        return Err(SourceVaultError::new(
            SourceVaultErrorCode::KeyReadbackMismatch,
            "snapshot key changed",
        )
        .into());
    }
    drop(rechecked_key);
    // Recheck after any potentially long system prompt, before result release.
    reader.authenticated_sources()?;
    Ok(LibraryReadSnapshot {
        sources,
        selected_lineage_id,
        selected_source_id,
        versions,
        abandonment_target,
        deletion_requests,
    })
    // Reader, key and exclusive session drop before the caller receives results,
    // including early errors and unwind. Drop never performs OS key-store access.
}
