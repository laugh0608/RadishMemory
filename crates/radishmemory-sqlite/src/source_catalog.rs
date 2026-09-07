use std::cmp::Ordering;

use radishmemory_core::{
    Identifier, SourceCatalog, SourceCatalogRequest, SourceLineageState, SourceLineageSummary,
    SourceVersionSummary,
};
use rusqlite::{OptionalExtension, params};

use crate::source_store::{from_i64, identifier, load_source_artifact, version};
use crate::{SqliteDatabase, SqliteError, SqliteStorageReason};

impl SourceCatalog for SqliteDatabase {
    type Error = SqliteError;

    fn resolve_source_lineage(
        &self,
        namespace_id: &Identifier,
        lineage_id: &Identifier,
    ) -> Result<Option<SourceLineageState>, Self::Error> {
        crate::source_capture::verify_origin_bindings(&self.connection)?;
        let stored = self
            .connection
            .query_row(
                "SELECT binding.origin_binding_id, tip.source_id, tip.version
                 FROM radishmemory_source_origin_bindings AS binding
                 JOIN radishmemory_source_lineage_tips AS tip
                   ON tip.namespace_id = binding.namespace_id
                  AND tip.lineage_id = binding.lineage_id
                 JOIN radishmemory_source_artifacts AS source
                   ON source.source_id = tip.source_id
                 WHERE binding.namespace_id = ?1 AND binding.lineage_id = ?2
                   AND source.deletion_state = 'active'",
                params![namespace_id.as_str(), lineage_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(SqliteError::storage)?;
        let Some((origin_binding_id, source_id, stored_version)) = stored else {
            return Ok(None);
        };
        let source_id = identifier(source_id)?;
        let current = load_source_artifact(&self.connection, namespace_id, &source_id)?
            .ok_or_else(lineage_tip_mismatch)?;
        let params = current.params();
        if &params.lineage_id != lineage_id || params.version.get() != from_i64(stored_version)? {
            return Err(lineage_tip_mismatch());
        }
        Ok(Some(SourceLineageState::new(
            namespace_id.clone(),
            identifier(origin_binding_id)?,
            lineage_id.clone(),
            source_id,
            version(stored_version)?,
        )))
    }

    fn list_source_lineages(
        &self,
        request: &SourceCatalogRequest,
    ) -> Result<Vec<SourceLineageSummary>, Self::Error> {
        crate::source_capture::verify_origin_bindings(&self.connection)?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT source_id
                 FROM radishmemory_source_lineage_tips
                 WHERE namespace_id = ?1",
            )
            .map_err(SqliteError::storage)?;
        let source_ids = statement
            .query_map(params![request.namespace_id().as_str()], |row| {
                row.get::<_, String>(0)
            })
            .map_err(SqliteError::storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SqliteError::storage)?;
        drop(statement);

        let mut summaries = Vec::with_capacity(source_ids.len());
        for source_id in source_ids {
            let source_id = identifier(source_id)?;
            let source =
                load_source_artifact(&self.connection, request.namespace_id(), &source_id)?
                    .ok_or_else(lineage_tip_mismatch)?;
            let count = self
                .connection
                .query_row(
                    "SELECT COUNT(*)
                     FROM radishmemory_source_artifacts
                     WHERE namespace_id = ?1 AND lineage_id = ?2
                       AND deletion_state = 'active'",
                    params![
                        request.namespace_id().as_str(),
                        source.params().lineage_id.as_str(),
                    ],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(SqliteError::storage)?;
            summaries.push(
                SourceLineageSummary::from_current_source(&source, from_i64(count)?)
                    .map_err(invalid_catalog)?,
            );
        }
        summaries.sort_by(|left, right| {
            right
                .captured_at()
                .cmp(left.captured_at())
                .then_with(|| left.lineage_id().as_str().cmp(right.lineage_id().as_str()))
        });
        let start = usize::try_from(request.offset())
            .unwrap_or(usize::MAX)
            .min(summaries.len());
        let end = start.saturating_add(request.limit()).min(summaries.len());
        Ok(summaries[start..end].to_vec())
    }

    fn list_source_versions(
        &self,
        namespace_id: &Identifier,
        lineage_id: &Identifier,
    ) -> Result<Vec<SourceVersionSummary>, Self::Error> {
        crate::source_capture::verify_origin_bindings(&self.connection)?;
        let current_source_id = self
            .connection
            .query_row(
                "SELECT source_id
                 FROM radishmemory_source_lineage_tips
                 WHERE namespace_id = ?1 AND lineage_id = ?2",
                params![namespace_id.as_str(), lineage_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(SqliteError::storage)?;
        let Some(current_source_id) = current_source_id else {
            return Ok(Vec::new());
        };

        let mut statement = self
            .connection
            .prepare(
                "SELECT source_id, version
                 FROM radishmemory_source_artifacts
                 WHERE namespace_id = ?1 AND lineage_id = ?2
                   AND deletion_state = 'active'
                 ORDER BY version, source_id",
            )
            .map_err(SqliteError::storage)?;
        let rows = statement
            .query_map(params![namespace_id.as_str(), lineage_id.as_str()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(SqliteError::storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SqliteError::storage)?;
        drop(statement);

        let mut versions = Vec::with_capacity(rows.len());
        let mut current_count = 0_u64;
        for (index, (source_id, stored_version)) in rows.into_iter().enumerate() {
            let expected_version = u64::try_from(index)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or_else(lineage_tip_mismatch)?;
            if from_i64(stored_version)? != expected_version {
                return Err(lineage_tip_mismatch());
            }
            let is_current = source_id == current_source_id;
            current_count += u64::from(is_current);
            let source_id = identifier(source_id)?;
            let source = load_source_artifact(&self.connection, namespace_id, &source_id)?
                .ok_or_else(lineage_tip_mismatch)?;
            if &source.params().lineage_id != lineage_id
                || source.params().version.get() != expected_version
            {
                return Err(lineage_tip_mismatch());
            }
            versions.push(
                SourceVersionSummary::from_source(&source, is_current).map_err(invalid_catalog)?,
            );
        }
        if versions.is_empty() || current_count.cmp(&1) != Ordering::Equal {
            return Err(lineage_tip_mismatch());
        }
        Ok(versions)
    }
}

fn lineage_tip_mismatch() -> SqliteError {
    SqliteError::invalid_stored(SqliteStorageReason::LineageTipMismatch)
}

fn invalid_catalog(source: radishmemory_core::CoreError) -> SqliteError {
    SqliteError::invalid_stored_with_source(SqliteStorageReason::StoredIntegrityMismatch, source)
}
