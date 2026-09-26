//! Exclusive maintenance session; checkpoints are durable across transactions.
use std::{fmt, path::Path};

use radishmemory_core::compute_exact_bytes_digest;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::{SqliteError, SqliteStorageReason, migration, source_store};

type Result<T> = std::result::Result<T, SqliteError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyMigrationState {
    Planned,
    Prepared,
    Referenced,
    Retired,
}

/// Adapter-private facts, never receipt or diagnostic fields.
pub struct BodyMigrationItem {
    pub source_id: String,
    pub namespace_id: String,
    pub digest_value: String,
    pub content_length: u64,
    pub media_type: String,
    pub state: BodyMigrationState,
    pub locator: Option<String>,
    pub attempt_id: Option<String>,
}

pub struct BodyMigrationDatabase {
    connection: Connection,
    namespace: String,
    version: u32,
}

impl BodyMigrationDatabase {
    /// Requires an existing key checkpoint. Holds an EXCLUSIVE SQLite lock even
    /// between commits until dropped. Hosts must still close old v6 handles before
    /// migration; this is not a substitute for product lifecycle coordination.
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
            migration::OBJECT_MIGRATION_SCHEMA_VERSION,
            &[7, 8],
        )?;
        if version == 7 {
            crate::source_vault_key::verify_plaintext_facts(&connection, namespace)?;
        }
        let database = Self {
            connection,
            namespace: namespace.into(),
            version,
        };
        database.verify_structure()?;
        Ok(database)
    }

    pub fn has_inventory(&self) -> bool {
        self.version == 8
    }

    /// Called only after the coordinator loads the existing key and verifies an
    /// empty object directory. No ordinary operation is enabled by this upgrade.
    pub fn initialize_inventory(&mut self) -> Result<()> {
        if self.version == 8 {
            return self.verify_structure();
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        crate::source_vault_key::verify_plaintext_facts(&tx, &self.namespace)?;
        migration::apply_pending(&tx, 7, migration::OBJECT_MIGRATION_SCHEMA_VERSION)?;
        tx.execute(
            "INSERT INTO radishmemory_source_vault_migration VALUES (1, 'migrating')",
            [],
        )
        .map_err(SqliteError::storage)?;
        tx.execute(
            "INSERT INTO radishmemory_source_vault_attempts
             (source_id, namespace_id, digest_value, content_length, media_type, state)
             SELECT a.source_id, a.namespace_id, a.content_digest_value, a.content_length, a.media_type, 'planned'
             FROM radishmemory_source_artifacts a JOIN radishmemory_source_bodies b ON a.source_id = b.source_id", [],
        ).map_err(SqliteError::storage)?;
        tx.commit().map_err(SqliteError::storage)?;
        self.version = 8;
        self.verify_structure()
    }

    pub fn items(&self) -> Result<Vec<BodyMigrationItem>> {
        let mut stmt = self.connection.prepare(
            "SELECT source_id, namespace_id, digest_value, content_length, media_type, state, locator, attempt_id
             FROM radishmemory_source_vault_attempts ORDER BY source_id"
        ).map_err(SqliteError::storage)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            })
            .map_err(SqliteError::storage)?;
        rows.map(|r| {
            let (
                source_id,
                namespace_id,
                digest_value,
                length,
                media_type,
                state,
                locator,
                attempt_id,
            ) = r.map_err(SqliteError::storage)?;
            let state = match state.as_str() {
                "planned" => BodyMigrationState::Planned,
                "prepared" => BodyMigrationState::Prepared,
                "referenced" => BodyMigrationState::Referenced,
                "retired" => BodyMigrationState::Retired,
                _ => return Err(invalid()),
            };
            Ok(BodyMigrationItem {
                source_id,
                namespace_id,
                digest_value,
                content_length: u64::try_from(length).map_err(|_| invalid())?,
                media_type,
                state,
                locator,
                attempt_id,
            })
        })
        .collect()
    }

    pub fn inline_body(&self, item: &BodyMigrationItem) -> Result<Option<Vec<u8>>> {
        let body: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT content FROM radishmemory_source_bodies WHERE source_id = ?1",
                [&item.source_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(SqliteError::storage)?;
        if let Some(body) = &body {
            self.verify_body(item, body)?;
        }
        Ok(body)
    }

    /// Reuses the canonical source/fragment decoders with authenticated content.
    /// Deleted residuals are checked for exact bytes but never exposed as active.
    pub fn verify_body(&self, item: &BodyMigrationItem, body: &[u8]) -> Result<()> {
        let digest = source_store::digest("sha256", "exact-bytes-v1", &item.digest_value)?;
        if body.len() as u64 != item.content_length || compute_exact_bytes_digest(body) != digest {
            return Err(invalid());
        }
        let ns = source_store::identifier(item.namespace_id.clone())?;
        let id = source_store::identifier(item.source_id.clone())?;
        if let Some(source) =
            source_store::load_source_with_body(&self.connection, &ns, &id, Some(body.to_vec()))?
        {
            source_store::load_fragments_for_source(&self.connection, &source)?;
            if source.params().origin_kind == radishmemory_core::SourceOriginKind::ExplicitUserInput
            {
                let binding = source.params().origin_ref.as_ref().ok_or_else(invalid)?;
                crate::source_capture::validate_existing_capture(
                    &self.connection,
                    &source_store::identifier(binding.as_str().into())?,
                    &source,
                )?;
            }
        }
        Ok(())
    }

    /// May replace a prepared token only after the coordinator proves both exact
    /// paths absent under the exclusive session. No canonical provenance changes.
    pub fn prepare(
        &mut self,
        item: &BodyMigrationItem,
        locator: &str,
        attempt: &str,
    ) -> Result<()> {
        validate_token(locator)?;
        validate_token(attempt)?;
        let changed = self.connection.execute(
            "UPDATE radishmemory_source_vault_attempts SET state = 'prepared', locator = ?2, attempt_id = ?3
             WHERE source_id = ?1 AND state IN ('planned', 'prepared')",
            params![item.source_id, locator, attempt],
        ).map_err(SqliteError::storage)?;
        if changed != 1 {
            return Err(invalid());
        }
        Ok(())
    }

    pub fn commit_reference(&mut self, item: &BodyMigrationItem) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        let changed = tx.execute(
            "INSERT INTO radishmemory_source_vault_references (source_id, locator, attempt_id)
             SELECT source_id, locator, attempt_id FROM radishmemory_source_vault_attempts WHERE source_id = ?1 AND state = 'prepared'",
            [&item.source_id],
        ).map_err(SqliteError::storage)?;
        if changed != 1 {
            return Err(invalid());
        }
        tx.execute("UPDATE radishmemory_source_vault_attempts SET state = 'referenced' WHERE source_id = ?1", [&item.source_id]).map_err(SqliteError::storage)?;
        tx.commit().map_err(SqliteError::storage)
    }

    /// Read the committed reference, rather than reusing an in-memory publish receipt.
    pub fn reference(&self, item: &BodyMigrationItem) -> Result<(String, String)> {
        self.connection.query_row("SELECT locator, attempt_id FROM radishmemory_source_vault_references WHERE source_id = ?1",
            [&item.source_id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(SqliteError::storage)
    }

    /// The caller must authenticate the committed reference immediately before
    /// invoking this transition; checkpoint and inline removal commit atomically.
    pub fn retire_inline(&mut self, item: &BodyMigrationItem) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        let changed = tx.execute("UPDATE radishmemory_source_vault_attempts SET state = 'retired' WHERE source_id = ?1 AND state = 'referenced'", [&item.source_id]).map_err(SqliteError::storage)?;
        if changed != 1 {
            return Err(invalid());
        }
        let removed = tx
            .execute(
                "DELETE FROM radishmemory_source_bodies WHERE source_id = ?1",
                [&item.source_id],
            )
            .map_err(SqliteError::storage)?;
        if removed != 1 {
            return Err(invalid());
        }
        tx.commit().map_err(SqliteError::storage)
    }

    pub fn finish(&mut self) -> Result<()> {
        self.verify_structure()?;
        if self
            .items()?
            .iter()
            .any(|i| i.state != BodyMigrationState::Retired)
        {
            return Err(invalid());
        }
        self.connection.execute("UPDATE radishmemory_source_vault_migration SET state = 'objects_ready' WHERE singleton = 1", []).map_err(SqliteError::storage)?;
        Ok(())
    }

    fn verify_structure(&self) -> Result<()> {
        let c = &self.connection;
        crate::source_vault_key::verify_database_facts(c, &self.namespace)?;
        crate::source_capture::verify_origin_binding_rows(c)?;
        if self.version == 7 {
            return Ok(());
        }
        let state: String = c
            .query_row(
                "SELECT state FROM radishmemory_source_vault_migration WHERE singleton = 1",
                [],
                |r| r.get(0),
            )
            .map_err(SqliteError::storage)?;
        if !matches!(state.as_str(), "migrating" | "objects_ready") {
            return Err(invalid());
        }
        let mismatch: bool = c.query_row(
            "SELECT EXISTS (
               SELECT 1 FROM radishmemory_source_artifacts a
               LEFT JOIN radishmemory_source_bodies b ON b.source_id = a.source_id
               LEFT JOIN radishmemory_source_vault_attempts m ON m.source_id = a.source_id
               LEFT JOIN radishmemory_source_vault_references r ON r.source_id = a.source_id
               WHERE a.namespace_id != ?1
                  OR (m.source_id IS NULL AND (b.source_id IS NOT NULL OR a.deletion_state = 'active'))
                  OR (m.source_id IS NOT NULL AND (
                       m.namespace_id != a.namespace_id OR m.digest_value != a.content_digest_value
                       OR a.content_digest_algorithm != 'sha256' OR a.content_digest_profile != 'exact-bytes-v1'
                       OR m.content_length != a.content_length OR m.media_type != a.media_type
                       OR (m.state = 'retired') != (b.source_id IS NULL)
                       OR (m.state IN ('referenced', 'retired')) != (r.source_id IS NOT NULL)
                       OR (r.source_id IS NOT NULL AND (r.locator != m.locator OR r.attempt_id != m.attempt_id))
                       OR (?2 = 'objects_ready' AND m.state != 'retired')))
             )", params![self.namespace, state], |r| r.get(0),
        ).map_err(SqliteError::storage)?;
        if mismatch {
            return Err(invalid());
        }
        Ok(())
    }
}

fn validate_token(token: &str) -> Result<()> {
    if token.len() != 64
        || !token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid());
    }
    Ok(())
}
fn invalid() -> SqliteError {
    SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
}
impl fmt::Debug for BodyMigrationDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BodyMigrationDatabase([REDACTED])")
    }
}
impl fmt::Debug for BodyMigrationItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BodyMigrationItem([REDACTED])")
    }
}
