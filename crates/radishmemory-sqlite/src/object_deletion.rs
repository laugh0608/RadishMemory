//! Object retirement checkpoints use the existing ten-component deletion plan.
use super::*;
use crate::deletion_store;
use radishmemory_core::{
    CanonicalObjectType, ComponentResult, DeleteRequest, DeletionEvidence, LocalDeletionExecution,
};

impl EncryptedCaptureDatabase {
    /// Persist the request, frozen closure, recall exclusion and closed references
    /// atomically. The caller must authenticate all objects before entering here.
    pub fn begin_object_deletion(
        &mut self,
        request: &DeleteRequest,
        sources: &[SourceArtifact],
    ) -> Result<()> {
        self.begin_with_check(request, sources, |_| Ok(()))
    }

    fn begin_with_check(
        &mut self,
        request: &DeleteRequest,
        sources: &[SourceArtifact],
        before_commit: impl FnOnce(&Connection) -> Result<()>,
    ) -> Result<()> {
        self.verify_facts(sources)?;
        self.validate_delete_identity(request)?;
        if let Some(existing) = deletion_store::load_request(
            &self.connection,
            &request.params().namespace_id,
            &request.params().delete_request_id,
        )? {
            if existing != *request || self.version < 11 {
                return Err(invalid());
            }
            self.verify_delete_plan(request)?;
            return Ok(());
        }
        if self.objects()?.iter().any(|i| {
            matches!(
                i.state,
                CaptureObjectState::Prepared | CaptureObjectState::Abandoning
            )
        }) || self.deletion_in_progress()?
        {
            return Err(invalid());
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        deletion_store::validate_semantic_targets(&tx, request)?;
        let closure = deletion_store::build_execution_closure(&tx, request)?;
        migration::apply_pending(
            &tx,
            i64::from(self.version),
            migration::OBJECT_DELETION_SCHEMA_VERSION,
        )?;
        deletion_store::insert_request(&tx, request, &closure)?;
        deletion_store::close_targets_to_recall(&tx, request, &closure)?;
        let digest = plan_digest(&tx, request.params().delete_request_id.as_str())?;
        tx.execute(
            "INSERT INTO radishmemory_source_vault_delete_plans VALUES(?1,?2)",
            params![request.params().delete_request_id.as_str(), digest],
        )
        .map_err(SqliteError::storage)?;
        for target in request
            .params()
            .target_refs
            .iter()
            .filter(|t| t.object_type() == CanonicalObjectType::SourceArtifact)
        {
            let removed = tx
                .execute(
                    "DELETE FROM radishmemory_source_vault_references WHERE source_id=?1",
                    [target.object_id().as_str()],
                )
                .map_err(SqliteError::storage)?;
            if removed != 1 {
                return Err(invalid());
            }
            tx.execute(
                "INSERT INTO radishmemory_source_vault_deletions VALUES(?1,?2,'deleting')",
                params![
                    target.object_id().as_str(),
                    request.params().delete_request_id.as_str()
                ],
            )
            .map_err(SqliteError::storage)?;
        }
        let remaining: Vec<_> = sources
            .iter()
            .filter(|s| {
                !request.params().target_refs.iter().any(|t| {
                    t.object_type() == CanonicalObjectType::SourceArtifact
                        && t.object_id() == &s.params().source_id
                })
            })
            .cloned()
            .collect();
        verify_facts(&tx, &remaining)?;
        before_commit(&tx)?;
        tx.commit().map_err(SqliteError::storage)?;
        self.version = 11;
        self.verify_structure()
    }

    pub fn deletion_objects(&self, request: &DeleteRequest) -> Result<Vec<CaptureObject>> {
        self.validate_delete_identity(request)?;
        self.verify_delete_plan(request)?;
        let mut result = Vec::new();
        for item in self.objects()? {
            let belongs: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM radishmemory_source_vault_deletions WHERE source_id=?1 AND delete_request_id=?2)", params![item.source_id,request.params().delete_request_id.as_str()], |r| r.get(0)).map_err(SqliteError::storage)?;
            if belongs {
                result.push(item);
            }
        }
        Ok(result)
    }

    /// Called only after exact object/staging removal, directory sync and absence check.
    pub fn finish_object_retirement(
        &mut self,
        request: &DeleteRequest,
        source_id: &str,
    ) -> Result<()> {
        self.finish_with_check(request, source_id, |_| Ok(()))
    }

    fn finish_with_check(
        &mut self,
        request: &DeleteRequest,
        source_id: &str,
        before_commit: impl FnOnce(&Connection) -> Result<()>,
    ) -> Result<()> {
        if !self
            .deletion_objects(request)?
            .iter()
            .any(|i| i.source_id == source_id)
        {
            return Err(invalid());
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        if tx.execute("UPDATE radishmemory_source_vault_deletions SET state='deleted' WHERE source_id=?1 AND delete_request_id=?2",params![source_id,request.params().delete_request_id.as_str()]).map_err(SqliteError::storage)? != 1 { return Err(invalid()); }
        before_commit(&tx)?;
        tx.commit().map_err(SqliteError::storage)
    }

    pub fn execute_object_deletion(
        &mut self,
        request: &DeleteRequest,
        execution: &LocalDeletionExecution,
    ) -> Result<Vec<ComponentResult>> {
        if self
            .deletion_objects(request)?
            .iter()
            .any(|i| i.state != CaptureObjectState::Deleted)
        {
            return Err(invalid());
        }
        deletion_store::execute_request(&mut self.connection, request, execution)
    }

    pub fn store_object_deletion_evidence(&mut self, evidence: &DeletionEvidence) -> Result<()> {
        let request = deletion_store::load_request(
            &self.connection,
            &evidence.params().namespace_id,
            &evidence.params().delete_request_id,
        )?
        .ok_or_else(invalid)?;
        self.validate_delete_identity(&request)?;
        self.verify_delete_plan(&request)?;
        deletion_store::store_evidence(&mut self.connection, evidence)
    }

    pub fn load_object_deletion_evidence(
        &self,
        id: &Identifier,
    ) -> Result<Option<DeletionEvidence>> {
        deletion_store::load_evidence(
            &self.connection,
            &source_store::identifier(self.namespace.clone())?,
            id,
        )
    }

    fn validate_delete_identity(&self, request: &DeleteRequest) -> Result<()> {
        deletion_store::validate_request_profile(request)?;
        let device: String = self
            .connection
            .query_row(
                "SELECT device_id FROM radishmemory_source_vault_key_profile WHERE singleton=1",
                [],
                |r| r.get(0),
            )
            .map_err(SqliteError::storage)?;
        if request.params().namespace_id.as_str() != self.namespace
            || request.params().device_id.as_str() != device
        {
            return Err(invalid());
        }
        Ok(())
    }

    fn verify_delete_plan(&self, request: &DeleteRequest) -> Result<()> {
        let stored = deletion_store::load_request(
            &self.connection,
            &request.params().namespace_id,
            &request.params().delete_request_id,
        )?
        .ok_or_else(invalid)?;
        if stored != *request {
            return Err(invalid());
        }
        let expected: String = self.connection.query_row("SELECT plan_digest FROM radishmemory_source_vault_delete_plans WHERE delete_request_id=?1",[request.params().delete_request_id.as_str()],|r|r.get(0)).map_err(SqliteError::storage)?;
        if plan_digest(
            &self.connection,
            request.params().delete_request_id.as_str(),
        )? != expected
        {
            return Err(invalid());
        }
        Ok(())
    }

    pub(super) fn deletion_in_progress(&self) -> Result<bool> {
        self.connection.query_row("SELECT EXISTS(SELECT 1 FROM radishmemory_source_artifacts WHERE deletion_state IN ('pending','failed')) OR EXISTS(SELECT 1 FROM radishmemory_memory_records WHERE deletion_state IN ('pending','failed'))",[],|r|r.get(0)).map_err(SqliteError::storage)
    }

    pub(super) fn verify_deletion_inventory(&self) -> Result<()> {
        if self.version < 11 {
            return Ok(());
        }
        let mut stmt = self.connection.prepare("SELECT delete_request_id FROM radishmemory_source_vault_delete_plans ORDER BY delete_request_id").map_err(SqliteError::storage)?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(SqliteError::storage)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(SqliteError::storage)?;
        for id in ids {
            let request = deletion_store::load_request(
                &self.connection,
                &source_store::identifier(self.namespace.clone())?,
                &source_store::identifier(id)?,
            )?
            .ok_or_else(invalid)?;
            self.validate_delete_identity(&request)?;
            self.verify_delete_plan(&request)?;
        }
        let mismatch: bool = self.connection.query_row("SELECT EXISTS(
            SELECT 1 FROM radishmemory_source_vault_deletions d
            JOIN radishmemory_source_vault_attempts m ON m.source_id=d.source_id
            LEFT JOIN radishmemory_source_artifacts a ON a.source_id=d.source_id
            LEFT JOIN radishmemory_source_vault_references r ON r.source_id=d.source_id
            WHERE m.state NOT IN ('retired','committed') OR a.source_id IS NULL OR a.deletion_state='active' OR r.source_id IS NOT NULL
               OR NOT EXISTS(SELECT 1 FROM radishmemory_delete_execution_closure c WHERE c.delete_request_id=d.delete_request_id AND c.component_type='source_body' AND c.object_type='SourceArtifact' AND c.object_id=d.source_id)
        ) OR EXISTS(
            SELECT 1 FROM radishmemory_delete_execution_closure c JOIN radishmemory_source_vault_delete_plans p ON p.delete_request_id=c.delete_request_id
            LEFT JOIN radishmemory_source_vault_deletions d ON d.source_id=c.object_id AND d.delete_request_id=c.delete_request_id
            WHERE c.component_type='source_body' AND (c.object_type!='SourceArtifact' OR d.source_id IS NULL)
        )",[],|r|r.get(0)).map_err(SqliteError::storage)?;
        if mismatch {
            return Err(invalid());
        }
        Ok(())
    }
}

fn plan_digest(connection: &Connection, request: &str) -> Result<String> {
    let mut bytes = b"radishmemory.object-deletion-plan/1".to_vec();
    // Fingerprint both authority and physical closure. Fixed schema and sorted,
    // length-framed values avoid delimiter ambiguity or row-order dependence.
    for table in [
        "radishmemory_delete_requests",
        "radishmemory_delete_request_targets",
        "radishmemory_delete_request_components",
        "radishmemory_delete_component_targets",
        "radishmemory_delete_execution_closure",
    ] {
        bytes.extend_from_slice(table.as_bytes());
        let mut stmt = connection
            .prepare(&format!("SELECT * FROM {table} WHERE delete_request_id=?1"))
            .map_err(SqliteError::storage)?;
        let columns = stmt.column_count();
        let mut rows = stmt
            .query_map([request], |row| {
                let mut encoded = Vec::new();
                for index in 0..columns {
                    let value = row.get_ref(index)?;
                    let (tag, value) = match value {
                        rusqlite::types::ValueRef::Null => (0, Vec::new()),
                        rusqlite::types::ValueRef::Integer(v) => (1, v.to_be_bytes().to_vec()),
                        rusqlite::types::ValueRef::Real(v) => {
                            (2, v.to_bits().to_be_bytes().to_vec())
                        }
                        rusqlite::types::ValueRef::Text(v) => (3, v.to_vec()),
                        rusqlite::types::ValueRef::Blob(v) => (4, v.to_vec()),
                    };
                    encoded.push(tag);
                    encoded.extend_from_slice(&(value.len() as u64).to_be_bytes());
                    encoded.extend_from_slice(&value);
                }
                Ok(encoded)
            })
            .map_err(SqliteError::storage)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(SqliteError::storage)?;
        rows.sort();
        bytes.extend_from_slice(&(rows.len() as u64).to_be_bytes());
        for row in rows {
            bytes.extend_from_slice(&(row.len() as u64).to_be_bytes());
            bytes.extend_from_slice(&row);
        }
    }
    Ok(compute_exact_bytes_digest(&bytes).value().to_owned())
}

#[cfg(test)]
#[path = "object_deletion/tests.rs"]
mod tests;
