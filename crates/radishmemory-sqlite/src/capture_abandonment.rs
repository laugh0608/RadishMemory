//! Exact selection and irreversible intent; physical removal belongs to Source Vault.
use super::*;

/// Opaque internal selection, not an authorization or a public receipt. Retain it
/// for retries; all fields are compared again under the exclusive session.
#[derive(Clone, Eq, PartialEq)]
pub struct CaptureAbandonmentTarget {
    namespace: String,
    device: String,
    provider: String,
    source: String,
    request_digest: String,
    binding: String,
    locator: String,
    attempt: String,
    digest: String,
    length: i64,
    media: String,
}

impl fmt::Debug for CaptureAbandonmentTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CaptureAbandonmentTarget([REDACTED])")
    }
}

impl EncryptedCaptureDatabase {
    /// Selection does not upgrade the schema or make an abandonment decision.
    pub fn pending_abandonment_target(&self) -> Result<Option<CaptureAbandonmentTarget>> {
        if self.version == 8 {
            return Ok(None);
        }
        let source: Option<String> = self
            .connection
            .query_row(
                "SELECT source_id FROM radishmemory_source_vault_attempts
             WHERE kind='capture' AND state IN ('prepared','abandoning')",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(SqliteError::storage)?;
        source
            .map(|source| self.load_abandonment_target(&source).map(|v| v.0))
            .transpose()
    }

    fn load_abandonment_target(&self, source: &str) -> Result<(CaptureAbandonmentTarget, String)> {
        self.connection
            .query_row(
                "SELECT m.namespace_id,k.device_id,k.provider_profile,m.source_id,
                    m.request_digest,m.origin_binding_id,m.locator,m.attempt_id,
                    m.digest_value,m.content_length,m.media_type,m.state
             FROM radishmemory_source_vault_attempts m
             JOIN radishmemory_source_vault_key_profile k ON k.namespace_id=m.namespace_id
             WHERE m.kind='capture' AND m.source_id=?1",
                [source],
                |r| {
                    Ok((
                        CaptureAbandonmentTarget {
                            namespace: r.get(0)?,
                            device: r.get(1)?,
                            provider: r.get(2)?,
                            source: r.get(3)?,
                            request_digest: r.get(4)?,
                            binding: r.get(5)?,
                            locator: r.get(6)?,
                            attempt: r.get(7)?,
                            digest: r.get(8)?,
                            length: r.get(9)?,
                            media: r.get(10)?,
                        },
                        r.get(11)?,
                    ))
                },
            )
            .map_err(SqliteError::storage)
    }

    pub fn abandonment_item(&self, target: &CaptureAbandonmentTarget) -> Result<CaptureObject> {
        self.verify_structure()?;
        if self.version < 9 || target.namespace != self.namespace {
            return Err(invalid());
        }
        let (current, state) = self.load_abandonment_target(&target.source)?;
        if current != *target || !matches!(state.as_str(), "prepared" | "abandoning" | "abandoned")
        {
            return Err(invalid());
        }
        self.objects()?
            .into_iter()
            .find(|i| i.source_id == target.source)
            .ok_or_else(invalid)
    }

    /// Must follow complete object authentication. A successful commit is the
    /// irreversible decision; file removal is forbidden before this returns.
    pub fn begin_abandonment(&mut self, target: &CaptureAbandonmentTarget) -> Result<()> {
        self.transition_abandonment(target, false, |_| Ok(()))
    }

    /// Coordinator has removed both exact paths, synced directories and rechecked
    /// absence. Keep the tombstone so old capture requests cannot come back.
    pub fn finish_abandonment(&mut self, target: &CaptureAbandonmentTarget) -> Result<()> {
        self.transition_abandonment(target, true, |_| Ok(()))
    }

    fn transition_abandonment(
        &mut self,
        target: &CaptureAbandonmentTarget,
        finish: bool,
        before_commit: impl FnOnce(&Connection) -> Result<()>,
    ) -> Result<()> {
        let item = self.abandonment_item(target)?;
        match item.state {
            CaptureObjectState::Abandoned => return Ok(()),
            CaptureObjectState::Abandoning if !finish => return Ok(()),
            CaptureObjectState::Prepared if finish => return Err(invalid()),
            CaptureObjectState::Committed => return Err(invalid()),
            _ => (),
        }
        let (from, to) = if finish {
            ("abandoning", "abandoned")
        } else {
            ("prepared", "abandoning")
        };
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        if self.version == 9 {
            migration::apply_pending(&tx, 9, migration::ABANDONMENT_SCHEMA_VERSION)?;
        }
        let changed = tx
            .execute(
                "UPDATE radishmemory_source_vault_attempts SET state=?2
             WHERE source_id=?1 AND state=?3 AND request_digest=?4 AND attempt_id=?5
               AND NOT EXISTS(SELECT 1 FROM radishmemory_source_vault_references WHERE source_id=?1)
               AND NOT EXISTS(SELECT 1 FROM radishmemory_source_artifacts WHERE source_id=?1)",
                params![
                    target.source,
                    to,
                    from,
                    target.request_digest,
                    target.attempt
                ],
            )
            .map_err(SqliteError::storage)?;
        if changed != 1 {
            return Err(invalid());
        }
        before_commit(&tx)?;
        tx.commit().map_err(SqliteError::storage)?;
        self.version = self.version.max(migration::ABANDONMENT_SCHEMA_VERSION);
        self.verify_structure()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending() -> (
        EncryptedCaptureDatabase,
        CaptureAbandonmentTarget,
        SourceCapture,
    ) {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::configure_connection(&connection).unwrap();
        let tx = connection.transaction().unwrap();
        migration::apply_pending(&tx, 0, 9).unwrap();
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
            version: 9,
        };
        let req = crate::source_capture::tests::capture(
            "source-commit-1",
            "fragment-commit-1",
            1,
            "Synthetic abandonment bytes",
            "2026-09-26T00:00:00Z",
        );
        db.prepare_capture(&req, &"a".repeat(64), &"b".repeat(64))
            .unwrap();
        let target = db.pending_abandonment_target().unwrap().unwrap();
        (db, target, req)
    }

    #[test]
    fn real_commit_failure_rolls_back_intent_and_schema_upgrade() {
        let (mut db, target, req) = pending();
        let error=db.transition_abandonment(&target,false,|c| {
            c.execute_batch("PRAGMA defer_foreign_keys=ON; INSERT INTO radishmemory_fragment_heading_path VALUES('missing-synthetic-fragment',0,'Synthetic heading')").map_err(SqliteError::storage)
        }).unwrap_err();
        assert_eq!(
            error.sqlite_extended_code(),
            Some(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY)
        );
        assert_eq!(db.version, 9);
        assert_eq!(
            db.connection
                .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            9
        );
        migration::verify_schema_definition(&db.connection, 9).unwrap();
        assert_eq!(
            db.abandonment_item(&target).unwrap().state,
            CaptureObjectState::Prepared
        );
        assert!(db.decide(&req, &[]).is_ok());
        db.begin_abandonment(&target).unwrap();
        assert_eq!(db.version, 10);
        assert!(db.decide(&req, &[]).is_err());
    }

    #[test]
    fn real_terminal_commit_failure_preserves_write_gate_and_terminal_is_idempotent() {
        let (mut db, target, req) = pending();
        db.begin_abandonment(&target).unwrap();
        let error=db.transition_abandonment(&target,true,|c| {
            c.execute_batch("PRAGMA defer_foreign_keys=ON; INSERT INTO radishmemory_fragment_heading_path VALUES('missing-synthetic-fragment',0,'Synthetic heading')").map_err(SqliteError::storage)
        }).unwrap_err();
        assert_eq!(
            error.sqlite_extended_code(),
            Some(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY)
        );
        assert_eq!(
            db.abandonment_item(&target).unwrap().state,
            CaptureObjectState::Abandoning
        );
        assert!(db.decide(&req, &[]).is_err());
        let other = crate::source_capture::tests::capture(
            "source-other",
            "fragment-other",
            1,
            "Synthetic other bytes",
            "2026-09-26T00:00:00Z",
        );
        assert!(db.decide(&other, &[]).is_err());
        assert!(
            db.prepare_capture(&other, &"c".repeat(64), &"d".repeat(64))
                .is_err()
        );
        db.finish_abandonment(&target).unwrap();
        db.finish_abandonment(&target).unwrap();
        assert_eq!(
            db.abandonment_item(&target).unwrap().state,
            CaptureObjectState::Abandoned
        );
        assert!(db.decide(&req, &[]).is_err());
        assert!(db.decide(&other, &[]).is_ok());
        migration::verify_schema_definition(&db.connection, 10).unwrap();
    }
}
