//! SQLite half of the encrypted capture coordinator. No filesystem or key access.
use crate::{
    SqliteError, SqliteStorageReason, capture_fingerprint::fingerprint, migration, source_store,
};
use radishmemory_core::{
    DeletionState, Identifier, SourceArtifact, SourceCapture, SourceCaptureOutcome,
    compute_exact_bytes_digest,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{collections::BTreeMap, fmt, path::Path};

#[path = "capture_abandonment.rs"]
mod abandonment;
#[path = "object_deletion.rs"]
mod deletion;
pub use abandonment::CaptureAbandonmentTarget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureObjectState {
    Committed,
    Prepared,
    Abandoning,
    Abandoned,
    Deleting,
    Deleted,
}

type Result<T> = std::result::Result<T, SqliteError>;

/// A private object fact, not an authorization or diagnostic payload.
pub struct CaptureObject {
    pub source_id: String,
    pub namespace_id: String,
    pub digest_value: String,
    pub content_length: u64,
    pub media_type: String,
    pub locator: String,
    pub attempt_id: String,
    pub state: CaptureObjectState,
}

pub struct EncryptedCaptureDatabase {
    connection: Connection,
    namespace: String,
    version: u32,
}

impl EncryptedCaptureDatabase {
    pub fn open(
        path: impl AsRef<Path>,
        namespace: &str,
        device: &str,
        provider: &str,
    ) -> Result<Self> {
        let (connection, version) = crate::vault_session::open(
            path,
            namespace,
            device,
            provider,
            migration::LEGACY_DELETION_SCHEMA_VERSION,
            &[8, 9, 10, 11, 12],
        )?;
        let db = Self {
            connection,
            namespace: namespace.into(),
            version,
        };
        db.verify_structure()?;
        Ok(db)
    }

    /// Coordinator calls only after authentication/validation of all v8 objects.
    pub fn initialize_capture_schema(&mut self) -> Result<()> {
        if self.version == 8 {
            let tx = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(SqliteError::storage)?;
            migration::apply_pending(&tx, 8, migration::CAPTURE_SCHEMA_VERSION)?;
            tx.commit().map_err(SqliteError::storage)?;
            self.version = 9;
        }
        self.verify_structure()
    }

    pub fn objects(&self) -> Result<Vec<CaptureObject>> {
        let sql = if self.version >= 11 {
            "SELECT m.source_id,m.namespace_id,m.digest_value,m.content_length,m.media_type,m.locator,m.attempt_id,COALESCE(d.state,m.state) FROM radishmemory_source_vault_attempts m LEFT JOIN radishmemory_source_vault_deletions d ON d.source_id=m.source_id ORDER BY m.source_id"
        } else {
            "SELECT source_id,namespace_id,digest_value,content_length,media_type,locator,attempt_id,state FROM radishmemory_source_vault_attempts ORDER BY source_id"
        };
        let mut stmt = self.connection.prepare(sql).map_err(SqliteError::storage)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                ))
            })
            .map_err(SqliteError::storage)?;
        rows.map(|r| {
            let (
                source_id,
                namespace_id,
                digest_value,
                len,
                media_type,
                locator,
                attempt_id,
                state,
            ) = r.map_err(SqliteError::storage)?;
            Ok(CaptureObject {
                source_id,
                namespace_id,
                digest_value,
                content_length: u64::try_from(len).map_err(|_| invalid())?,
                media_type,
                locator,
                attempt_id,
                state: match state.as_str() {
                    "retired" | "committed" => CaptureObjectState::Committed,
                    "prepared" => CaptureObjectState::Prepared,
                    "abandoning" => CaptureObjectState::Abandoning,
                    "abandoned" => CaptureObjectState::Abandoned,
                    "deleting" => CaptureObjectState::Deleting,
                    "deleted" => CaptureObjectState::Deleted,
                    _ => return Err(invalid()),
                },
            })
        })
        .collect()
    }

    /// Canonical metadata is reloaded and checked against authenticated bytes.
    pub fn resolve_object(
        &self,
        item: &CaptureObject,
        body: &[u8],
    ) -> Result<Option<SourceArtifact>> {
        if item.state != CaptureObjectState::Committed
            || body.len() as u64 != item.content_length
            || compute_exact_bytes_digest(body).value() != item.digest_value
        {
            return Err(invalid());
        }
        let source = source_store::load_source_with_body(
            &self.connection,
            &source_store::identifier(item.namespace_id.clone())?,
            &source_store::identifier(item.source_id.clone())?,
            Some(body.to_vec()),
        )?;
        if let Some(source) = &source {
            source_store::load_fragments_for_source(&self.connection, source)?;
        }
        Ok(source)
    }

    /// No inline-body fallback: every active source must be provided by the
    /// authenticated object reader and agree with the stored canonical fields.
    pub fn verify_facts(&self, sources: &[SourceArtifact]) -> Result<()> {
        self.verify_structure()?;
        verify_facts(&self.connection, sources)
    }

    /// Check FTS5's internal index as well as its separately stored content rows.
    /// This FTS5 command performs verification; it does not rebuild the index.
    pub fn verify_recall_index_structure(&self) -> Result<()> {
        self.connection.execute(
            "INSERT INTO radishmemory_recall_fts(radishmemory_recall_fts) VALUES('integrity-check')",
            [],
        ).map_err(SqliteError::storage)?;
        Ok(())
    }

    /// Rebuild only derived rows under the existing exclusive maintenance session.
    /// The caller supplies authenticated bodies and must reauthenticate objects in
    /// `before_commit`. Callback failure rolls back all derived writes. No source,
    /// attempt, reference, schema or key is created or modified here.
    pub fn rebuild_recall_derivations<E: From<SqliteError>>(
        &self,
        sources: &[SourceArtifact],
        before_commit: impl FnOnce() -> std::result::Result<(), E>,
    ) -> std::result::Result<(), E> {
        self.verify_structure()?;
        verify_canonical_facts(&self.connection, sources)?;
        let load = source_loader(sources)?;
        let tx =
            rusqlite::Transaction::new_unchecked(&self.connection, TransactionBehavior::Exclusive)
                .map_err(SqliteError::storage)?;
        crate::derived_index::rebuild_with_sources(&tx, &|_, ns, id| load(ns, id))?;
        verify_facts(&tx, sources)?;
        before_commit()?;
        tx.commit().map_err(SqliteError::storage)?;
        Ok(())
    }

    pub fn reference(&self, source_id: &str) -> Result<(String, String)> {
        self.connection.query_row("SELECT locator,attempt_id FROM radishmemory_source_vault_references WHERE source_id=?1",[source_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(SqliteError::storage)
    }

    pub fn decide(
        &self,
        capture: &SourceCapture,
        sources: &[SourceArtifact],
    ) -> Result<(SourceArtifact, SourceCaptureOutcome)> {
        if capture.source().params().namespace_id.as_str() != self.namespace
            || capture.source().params().governance.deletion_state() != DeletionState::Active
        {
            return Err(invalid());
        }
        if self.deletion_in_progress()?
            || self
                .objects()?
                .iter()
                .any(|item| item.state == CaptureObjectState::Abandoning)
        {
            return Err(invalid());
        }
        if self.version >= 9 {
            let pending:Option<String>=self.connection.query_row("SELECT request_digest FROM radishmemory_source_vault_attempts WHERE state='prepared'",[],|r|r.get(0)).optional().map_err(SqliteError::storage)?;
            if pending.is_some_and(|d| d != fingerprint(capture)) {
                return Err(invalid());
            }
            // Repeating the precise committed operation remains idempotent even
            // if a later capture has advanced this binding's tip.
            let prior:Option<(String,String,String)>=self.connection.query_row("SELECT request_digest,state,origin_binding_id FROM radishmemory_source_vault_attempts WHERE kind='capture' AND source_id=?1",[capture.source().params().source_id.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(SqliteError::storage)?;
            if let Some((digest, state, binding)) = prior {
                if matches!(state.as_str(), "abandoning" | "abandoned") {
                    return Err(invalid());
                }
                if binding != capture.origin_binding_id().as_str() {
                    return Err(invalid());
                }
                if digest != fingerprint(capture) {
                    return Err(invalid());
                }
                let item = self
                    .objects()?
                    .into_iter()
                    .find(|i| i.source_id == capture.source().params().source_id.as_str())
                    .ok_or_else(invalid)?;
                let p = capture.source().params();
                if item.namespace_id != p.namespace_id.as_str()
                    || item.digest_value != p.content_digest.value()
                    || item.content_length != p.content_length
                    || item.media_type != p.media_type.as_str()
                {
                    return Err(invalid());
                }
                if state == "committed" {
                    let source = sources
                        .iter()
                        .find(|s| s.params().source_id == capture.source().params().source_id)
                        .ok_or_else(invalid)?;
                    if source != capture.source() {
                        return Err(invalid());
                    }
                    return Ok((source.clone(), SourceCaptureOutcome::Idempotent));
                }
            }
        }
        let load = source_loader(sources)?;
        let decision =
            crate::source_capture::decide_capture(&self.connection, capture, &|_, ns, id| {
                load(ns, id)
            })?;
        if decision.1 != SourceCaptureOutcome::Idempotent {
            self.validate_candidate(capture)?;
        }
        Ok(decision)
    }

    fn validate_candidate(&self, capture: &SourceCapture) -> Result<()> {
        let p = capture.source().params();
        source_store::to_i64(p.version.get())?;
        source_store::to_i64(p.content_length)?;
        let tip:Option<(String,i64)>=self.connection.query_row("SELECT source_id,version FROM radishmemory_source_lineage_tips WHERE namespace_id=?1 AND lineage_id=?2",params![p.namespace_id.as_str(),p.lineage_id.as_str()],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(SqliteError::storage)?;
        match tip {
            None if p.version.get() == 1 && p.supersedes_source_ids.is_empty() => (),
            Some((id, version))
                if version.checked_add(1) == Some(source_store::to_i64(p.version.get())?)
                    && p.supersedes_source_ids.len() == 1
                    && p.supersedes_source_ids[0].as_str() == id => {}
            _ => return Err(invalid()),
        }
        if self
            .connection
            .prepare("SELECT 1 FROM radishmemory_source_artifacts WHERE source_id=?1")
            .map_err(SqliteError::storage)?
            .exists([p.source_id.as_str()])
            .map_err(SqliteError::storage)?
        {
            return Err(invalid());
        }
        for fragment in capture.fragments() {
            let f = fragment.params();
            if f.governance.deletion_state() != DeletionState::Active {
                return Err(invalid());
            }
            for number in [f.ordinal, f.byte_start, f.byte_end] {
                source_store::to_i64(number)?;
            }
            if self
                .connection
                .prepare("SELECT 1 FROM radishmemory_source_fragments WHERE fragment_id=?1")
                .map_err(SqliteError::storage)?
                .exists([f.fragment_id.as_str()])
                .map_err(SqliteError::storage)?
            {
                return Err(invalid());
            }
        }
        Ok(())
    }

    /// Replaces tokens only when the coordinator has proved the exact old paths
    /// absent. Immutable request fingerprint and reservation are preserved.
    pub fn prepare_capture(
        &mut self,
        capture: &SourceCapture,
        locator: &str,
        attempt: &str,
    ) -> Result<()> {
        if !matches!(self.version, 9..=12) {
            return Err(invalid());
        }
        let p = capture.source().params();
        let digest = fingerprint(capture);
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        let existing:Option<String>=tx.query_row("SELECT request_digest FROM radishmemory_source_vault_attempts WHERE state='prepared'",[],|r|r.get(0)).optional().map_err(SqliteError::storage)?;
        if let Some(existing) = existing {
            if existing != digest {
                return Err(invalid());
            }
            let changed=tx.execute("UPDATE radishmemory_source_vault_attempts SET locator=?2,attempt_id=?3 WHERE source_id=?1 AND state='prepared'",params![p.source_id.as_str(),locator,attempt]).map_err(SqliteError::storage)?;
            if changed != 1 {
                return Err(invalid());
            }
        } else {
            tx.execute("INSERT INTO radishmemory_source_vault_attempts (source_id,namespace_id,digest_value,content_length,media_type,state,locator,attempt_id,kind,request_digest,origin_binding_id) VALUES (?1,?2,?3,?4,?5,'prepared',?6,?7,'capture',?8,?9)",params![p.source_id.as_str(),p.namespace_id.as_str(),p.content_digest.value(),source_store::to_i64(p.content_length)?,p.media_type.as_str(),locator,attempt,digest,capture.origin_binding_id().as_str()]).map_err(SqliteError::storage)?;
        }
        tx.commit().map_err(SqliteError::storage)
    }

    pub fn commit_capture(
        &mut self,
        capture: &SourceCapture,
        sources: &[SourceArtifact],
    ) -> Result<SourceCaptureOutcome> {
        self.commit_with_check(capture, sources, |_| Ok(()))
    }
    fn commit_with_check(
        &mut self,
        capture: &SourceCapture,
        sources: &[SourceArtifact],
        before_commit: impl FnOnce(&Connection) -> Result<()>,
    ) -> Result<SourceCaptureOutcome> {
        let (_, outcome) = self.decide(capture, sources)?;
        if outcome == SourceCaptureOutcome::Idempotent {
            return Err(invalid());
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        let digest:String=tx.query_row("SELECT request_digest FROM radishmemory_source_vault_attempts WHERE source_id=?1 AND state='prepared'",[capture.source().params().source_id.as_str()],|r|r.get(0)).map_err(SqliteError::storage)?;
        if digest != fingerprint(capture) {
            return Err(invalid());
        }
        crate::source_capture::insert_capture_facts(&tx, capture, outcome, false)?;
        tx.execute("INSERT INTO radishmemory_source_vault_references SELECT source_id,locator,attempt_id FROM radishmemory_source_vault_attempts WHERE source_id=?1 AND state='prepared'",[capture.source().params().source_id.as_str()]).map_err(SqliteError::storage)?;
        tx.execute(
            "UPDATE radishmemory_source_vault_attempts SET state='committed' WHERE source_id=?1",
            [capture.source().params().source_id.as_str()],
        )
        .map_err(SqliteError::storage)?;
        let mut updated = sources.to_vec();
        updated.push(capture.source().clone());
        verify_facts(&tx, &updated)?;
        before_commit(&tx)?;
        tx.commit().map_err(SqliteError::storage)?;
        Ok(outcome)
    }

    fn verify_structure(&self) -> Result<()> {
        crate::source_vault_key::verify_database_facts(&self.connection, &self.namespace)?;
        let state: String = self
            .connection
            .query_row(
                "SELECT state FROM radishmemory_source_vault_migration WHERE singleton=1",
                [],
                |r| r.get(0),
            )
            .map_err(SqliteError::storage)?;
        if state != "objects_ready" {
            return Err(invalid());
        }
        if self
            .connection
            .prepare("SELECT 1 FROM radishmemory_source_bodies LIMIT 1")
            .map_err(SqliteError::storage)?
            .exists([])
            .map_err(SqliteError::storage)?
        {
            return Err(invalid());
        }
        let deletion_rows = if self.version >= 11 {
            "SELECT source_id FROM radishmemory_source_vault_deletions"
        } else {
            "SELECT NULL AS source_id WHERE 0"
        };
        let mismatch:bool=self.connection.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM radishmemory_source_vault_attempts m
             LEFT JOIN radishmemory_source_artifacts a ON a.source_id=m.source_id
             LEFT JOIN radishmemory_source_vault_references r ON r.source_id=m.source_id
             LEFT JOIN ({deletion_rows}) d ON d.source_id=m.source_id
             WHERE m.namespace_id!=?1 OR
                (m.state IN ('prepared','abandoning','abandoned') AND (a.source_id IS NOT NULL OR r.source_id IS NOT NULL)) OR
                (m.state IN ('retired','committed') AND (a.source_id IS NULL OR
                   (d.source_id IS NULL AND r.source_id IS NULL) OR
                   (d.source_id IS NOT NULL AND (r.source_id IS NOT NULL OR a.deletion_state='active')) OR
                   m.namespace_id!=a.namespace_id OR m.digest_value!=a.content_digest_value OR
                   a.content_digest_algorithm!='sha256' OR a.content_digest_profile!='exact-bytes-v1' OR
                   m.content_length!=a.content_length OR m.media_type!=a.media_type OR
                   m.locator!=r.locator OR m.attempt_id!=r.attempt_id))) OR
             EXISTS(SELECT 1 FROM radishmemory_source_artifacts a LEFT JOIN radishmemory_source_vault_references r ON a.source_id=r.source_id WHERE a.deletion_state='active' AND r.source_id IS NULL)"),
            [&self.namespace],|r|r.get(0)).map_err(SqliteError::storage)?;
        if mismatch {
            return Err(invalid());
        }
        self.verify_deletion_inventory()?;
        if self.version == 8
            && self
                .objects()?
                .iter()
                .any(|o| o.state != CaptureObjectState::Committed)
        {
            return Err(invalid());
        }
        Ok(())
    }
}

fn verify_facts(connection: &Connection, sources: &[SourceArtifact]) -> Result<()> {
    verify_canonical_facts(connection, sources)?;
    let load = source_loader(sources)?;
    crate::derived_index::verify_with_sources(connection, &|_, ns, id| load(ns, id))
}

fn verify_canonical_facts(connection: &Connection, sources: &[SourceArtifact]) -> Result<()> {
    let _ = source_loader(sources)?;
    let count: i64 = connection
        .query_row(
            "SELECT count(*) FROM radishmemory_source_artifacts WHERE deletion_state='active'",
            [],
            |r| r.get(0),
        )
        .map_err(SqliteError::storage)?;
    if usize::try_from(count).ok() != Some(sources.len()) {
        return Err(invalid());
    }
    let schema: i64 = connection
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(SqliteError::storage)?;
    for source in sources {
        let p = source.params();
        let stored = source_store::load_source_with_body(
            connection,
            &p.namespace_id,
            &p.source_id,
            Some(p.content.as_str().as_bytes().to_vec()),
        )?
        .ok_or_else(invalid)?;
        if stored != *source {
            return Err(invalid());
        }
        let fragments = source_store::load_fragments_for_source(connection, source)?;
        if schema >= 9 {
            let persisted:Option<(String,String)>=connection.query_row("SELECT request_digest,origin_binding_id FROM radishmemory_source_vault_attempts WHERE source_id=?1 AND kind='capture' AND state='committed'",[p.source_id.as_str()],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(SqliteError::storage)?;
            if let Some((digest, binding)) = persisted {
                let capture = SourceCapture::new(
                    source_store::identifier(binding)?,
                    source.clone(),
                    fragments,
                )
                .map_err(source_store::invalid_core)?;
                if fingerprint(&capture) != digest {
                    return Err(invalid());
                }
            }
        }
        if p.origin_kind == radishmemory_core::SourceOriginKind::ExplicitUserInput {
            crate::source_capture::validate_existing_capture(
                connection,
                &source_store::identifier(
                    p.origin_ref.as_ref().ok_or_else(invalid)?.as_str().into(),
                )?,
                source,
            )?;
        }
    }
    crate::source_capture::verify_origin_binding_rows(connection)
}
fn source_loader(
    sources: &[SourceArtifact],
) -> Result<impl Fn(&Identifier, &Identifier) -> Result<Option<SourceArtifact>> + '_> {
    let mut map = BTreeMap::new();
    for source in sources {
        if map
            .insert(
                (
                    source.params().namespace_id.clone(),
                    source.params().source_id.clone(),
                ),
                source,
            )
            .is_some()
        {
            return Err(invalid());
        }
    }
    Ok(move |ns: &Identifier, id: &Identifier| {
        Ok(map.get(&(ns.clone(), id.clone())).map(|s| (*s).clone()))
    })
}
fn invalid() -> SqliteError {
    SqliteError::invalid_stored(SqliteStorageReason::CaptureStateMismatch)
}
impl fmt::Debug for CaptureObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CaptureObject([REDACTED])")
    }
}
impl fmt::Debug for EncryptedCaptureDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EncryptedCaptureDatabase([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rebuild_commit_failure_restores_damaged_derivations() {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::configure_connection(&connection).unwrap();
        let tx = connection.transaction().unwrap();
        migration::apply_pending(&tx, 0, 8).unwrap();
        tx.execute("INSERT INTO radishmemory_source_vault_key_profile VALUES(1,'namespace-commit-1','device-synthetic','provider-synthetic','key_ready')", []).unwrap();
        tx.execute(
            "INSERT INTO radishmemory_source_vault_migration VALUES(1,'objects_ready')",
            [],
        )
        .unwrap();
        tx.execute("INSERT INTO radishmemory_recall_fts VALUES('source_fragment','missing-fragment','namespace-commit-1','personal','Synthetic stale row')", []).unwrap();
        tx.commit().unwrap();
        let db = EncryptedCaptureDatabase {
            connection,
            namespace: "namespace-commit-1".into(),
            version: 8,
        };
        let error: SqliteError = db.rebuild_recall_derivations(&[], || {
            db.connection.execute_batch("PRAGMA defer_foreign_keys=ON; INSERT INTO radishmemory_fragment_heading_path VALUES('missing-synthetic-fragment',0,'Synthetic heading')").map_err(SqliteError::storage)
        }).unwrap_err();
        assert_eq!(
            error.sqlite_extended_code(),
            Some(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY)
        );
        let rows: i64 = db
            .connection
            .query_row("SELECT count(*) FROM radishmemory_recall_fts", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(rows, 1);
        assert!(db.verify_facts(&[]).is_err());
        db.rebuild_recall_derivations(&[], || Ok::<(), SqliteError>(()))
            .unwrap();
        db.verify_facts(&[]).unwrap();
    }

    #[test]
    fn real_deferred_commit_failure_rolls_back_every_fact_but_keeps_prepared_attempt() {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::configure_connection(&connection).unwrap();
        let tx = connection.transaction().unwrap();
        migration::apply_pending(&tx, 0, 8).unwrap();
        tx.execute("INSERT INTO radishmemory_source_vault_key_profile VALUES(1,'namespace-commit-1','device-synthetic','provider-synthetic','key_ready')",[]).unwrap();
        tx.execute(
            "INSERT INTO radishmemory_source_vault_migration VALUES(1,'objects_ready')",
            [],
        )
        .unwrap();
        tx.commit().unwrap();
        let mut db = EncryptedCaptureDatabase {
            connection,
            namespace: "namespace-commit-1".into(),
            version: 8,
        };
        db.initialize_capture_schema().unwrap();
        let req = crate::source_capture::tests::capture(
            "source-commit-1",
            "fragment-commit-1",
            1,
            "Synthetic transactional body",
            "2026-09-26T00:00:00Z",
        );
        db.prepare_capture(&req, &"a".repeat(64), &"b".repeat(64))
            .unwrap();
        let error=db.commit_with_check(&req,&[],|c| {
            c.execute_batch("PRAGMA defer_foreign_keys=ON; INSERT INTO radishmemory_fragment_heading_path VALUES('missing-synthetic-fragment',0,'Synthetic heading')").map_err(SqliteError::storage)
        }).unwrap_err();
        assert_eq!(
            error.sqlite_extended_code(),
            Some(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY)
        );
        for table in [
            "radishmemory_source_artifacts",
            "radishmemory_source_fragments",
            "radishmemory_source_bodies",
            "radishmemory_source_capture_audit",
            "radishmemory_source_origin_bindings",
            "radishmemory_source_lineage_tips",
            "radishmemory_recall_fts",
            "radishmemory_source_vault_references",
        ] {
            let count: i64 = db
                .connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 0);
        }
        assert_eq!(db.objects().unwrap()[0].state, CaptureObjectState::Prepared);
        db.verify_facts(&[]).unwrap();
        db.commit_capture(&req, &[]).unwrap();
        assert_eq!(
            db.objects().unwrap()[0].state,
            CaptureObjectState::Committed
        );
        db.verify_facts(&[req.source().clone()]).unwrap();
    }
}
