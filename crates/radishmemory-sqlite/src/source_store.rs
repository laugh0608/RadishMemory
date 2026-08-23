use std::collections::BTreeSet;

use radishmemory_core::{
    CanonicalObjectType, CoreError, CoreErrorCode, DeletionState, Digest, EgressPolicy, Governance,
    Identifier, M0_SCHEMA_VERSION, MediaType, NonEmptyText, ProducerRef, ProducerType,
    RetentionMode, RetentionRule, Sensitivity, SourceArtifact, SourceArtifactParams,
    SourceFragment, SourceFragmentParams, SourceKind, SourceOriginKind, SourceVault, Timestamp,
    Version, validate_source_fragment_resolution,
};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};

use crate::{SqliteDatabase, SqliteError, SqliteStorageReason};

impl SourceVault for SqliteDatabase {
    type Error = SqliteError;

    fn store_source_artifact(&mut self, source: &SourceArtifact) -> Result<(), Self::Error> {
        let params = source.params();
        let version = to_i64(params.version.get())?;
        let content_length = to_i64(params.content_length)?;
        let retention = params.governance.retention();

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        validate_superseded_sources(&transaction, source)?;
        transaction
            .execute(
                "INSERT INTO radishmemory_source_artifacts (
                     source_id, canonical_schema_version, object_type, lineage_id, version,
                     namespace_id, source_kind, media_type, content_length,
                     content_digest_algorithm, content_digest_profile, content_digest_value,
                     title, origin_kind, origin_ref, observed_at, captured_at, sensitivity,
                     egress_policy, retention_mode, retention_expires_at, retention_policy_id,
                     deletion_state, policy_basis, producer_type, producer_id, producer_version,
                     created_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                     ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28
                 )",
                params![
                    params.source_id.as_str(),
                    M0_SCHEMA_VERSION,
                    CanonicalObjectType::SourceArtifact.as_str(),
                    params.lineage_id.as_str(),
                    version,
                    params.namespace_id.as_str(),
                    source_kind_str(params.source_kind),
                    params.media_type.as_str(),
                    content_length,
                    params.content_digest.algorithm(),
                    params.content_digest.profile().as_str(),
                    params.content_digest.value(),
                    params.title.as_ref().map(NonEmptyText::as_str),
                    source_origin_kind_str(params.origin_kind),
                    params.origin_ref.as_ref().map(NonEmptyText::as_str),
                    params.observed_at.original(),
                    params.captured_at.original(),
                    sensitivity_str(params.governance.sensitivity()),
                    egress_policy_str(params.governance.egress_policy()),
                    retention_mode_str(retention.mode()),
                    retention.expires_at().map(Timestamp::original),
                    retention.policy_id().map(Identifier::as_str),
                    deletion_state_str(params.governance.deletion_state()),
                    params.governance.policy_basis().as_str(),
                    producer_type_str(params.producer.producer_type()),
                    params.producer.producer_id().as_str(),
                    params.producer.producer_version().as_str(),
                    params.created_at.original(),
                ],
            )
            .map_err(SqliteError::storage)?;
        transaction
            .execute(
                "INSERT INTO radishmemory_source_bodies (source_id, content) VALUES (?1, ?2)",
                params![
                    params.source_id.as_str(),
                    params.content.as_str().as_bytes()
                ],
            )
            .map_err(SqliteError::storage)?;
        for (ordinal, superseded_source_id) in params.supersedes_source_ids.iter().enumerate() {
            transaction
                .execute(
                    "INSERT INTO radishmemory_source_supersedes (
                         source_id, ordinal, superseded_source_id
                     ) VALUES (?1, ?2, ?3)",
                    params![
                        params.source_id.as_str(),
                        to_i64(usize_to_u64(ordinal)?)?,
                        superseded_source_id.as_str(),
                    ],
                )
                .map_err(SqliteError::storage)?;
        }
        transaction.commit().map_err(SqliteError::storage)
    }

    fn store_source_fragments(&mut self, fragments: &[SourceFragment]) -> Result<(), Self::Error> {
        let (namespace_id, source_id) = validate_fragment_batch(fragments)?;
        for fragment in fragments {
            validate_fragment_numbers(fragment)?;
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        let source = load_source_artifact(&transaction, namespace_id, source_id)?
            .ok_or_else(|| SqliteError::source_invariant(SqliteStorageReason::MissingSource))?;
        for fragment in fragments {
            validate_source_fragment_resolution(fragment, &source).map_err(|source| {
                SqliteError::source_invariant_with_core(
                    SqliteStorageReason::SourceResolution,
                    source,
                )
            })?;
        }

        for fragment in fragments {
            let fragment_params = fragment.params();
            let retention = fragment_params.governance.retention();
            transaction
                .execute(
                    "INSERT INTO radishmemory_source_fragments (
                         fragment_id, canonical_schema_version, object_type, namespace_id,
                         source_id, ordinal, byte_start, byte_end, content_digest_algorithm,
                         content_digest_profile, content_digest_value, segmenter_type,
                         segmenter_id, segmenter_version, sensitivity, egress_policy,
                         retention_mode, retention_expires_at, retention_policy_id,
                         deletion_state, policy_basis, created_at
                     ) VALUES (
                         ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                         ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
                     )",
                    params![
                        fragment_params.fragment_id.as_str(),
                        M0_SCHEMA_VERSION,
                        CanonicalObjectType::SourceFragment.as_str(),
                        fragment_params.namespace_id.as_str(),
                        fragment_params.source_id.as_str(),
                        to_i64(fragment_params.ordinal)?,
                        to_i64(fragment_params.byte_start)?,
                        to_i64(fragment_params.byte_end)?,
                        fragment_params.content_digest.algorithm(),
                        fragment_params.content_digest.profile().as_str(),
                        fragment_params.content_digest.value(),
                        producer_type_str(fragment_params.segmenter.producer_type()),
                        fragment_params.segmenter.producer_id().as_str(),
                        fragment_params.segmenter.producer_version().as_str(),
                        sensitivity_str(fragment_params.governance.sensitivity()),
                        egress_policy_str(fragment_params.governance.egress_policy()),
                        retention_mode_str(retention.mode()),
                        retention.expires_at().map(Timestamp::original),
                        retention.policy_id().map(Identifier::as_str),
                        deletion_state_str(fragment_params.governance.deletion_state()),
                        fragment_params.governance.policy_basis().as_str(),
                        fragment_params.created_at.original(),
                    ],
                )
                .map_err(SqliteError::storage)?;
            if let Some(headings) = &fragment_params.heading_path {
                for (ordinal, heading) in headings.iter().enumerate() {
                    transaction
                        .execute(
                            "INSERT INTO radishmemory_fragment_heading_path (
                                 fragment_id, ordinal, heading
                             ) VALUES (?1, ?2, ?3)",
                            params![
                                fragment_params.fragment_id.as_str(),
                                to_i64(usize_to_u64(ordinal)?)?,
                                heading.as_str(),
                            ],
                        )
                        .map_err(SqliteError::storage)?;
                }
            }
        }
        transaction.commit().map_err(SqliteError::storage)
    }

    fn load_source_artifact(
        &self,
        namespace_id: &Identifier,
        source_id: &Identifier,
    ) -> Result<Option<SourceArtifact>, Self::Error> {
        load_source_artifact(&self.connection, namespace_id, source_id)
    }

    fn load_source_fragments(
        &self,
        namespace_id: &Identifier,
        source_id: &Identifier,
    ) -> Result<Option<Vec<SourceFragment>>, Self::Error> {
        let Some(source) = load_source_artifact(&self.connection, namespace_id, source_id)? else {
            return Ok(None);
        };
        let mut statement = self
            .connection
            .prepare(
                "SELECT fragment_id, canonical_schema_version, object_type, namespace_id,
                        source_id, ordinal, byte_start, byte_end, content_digest_algorithm,
                        content_digest_profile, content_digest_value, segmenter_type,
                        segmenter_id, segmenter_version, sensitivity, egress_policy,
                        retention_mode, retention_expires_at, retention_policy_id,
                        deletion_state, policy_basis, created_at
                 FROM radishmemory_source_fragments
                 WHERE source_id = ?1
                 ORDER BY ordinal, fragment_id",
            )
            .map_err(SqliteError::storage)?;
        let rows = statement
            .query_map(params![source_id.as_str()], |row| {
                StoredFragment::from_row(row)
            })
            .map_err(SqliteError::storage)?;
        let stored = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(SqliteError::storage)?;
        let mut fragments = Vec::with_capacity(stored.len());
        for stored_fragment in stored {
            let headings = load_heading_path(&self.connection, &stored_fragment.fragment_id)?;
            let fragment = stored_fragment.into_domain(&source, headings)?;
            validate_source_fragment_resolution(&fragment, &source).map_err(|source| {
                SqliteError::invalid_stored_with_source(
                    SqliteStorageReason::StoredIntegrityMismatch,
                    source,
                )
            })?;
            fragments.push(fragment);
        }
        Ok(Some(fragments))
    }
}

fn validate_superseded_sources(
    connection: &Connection,
    source: &SourceArtifact,
) -> Result<(), SqliteError> {
    let params = source.params();
    for target_id in &params.supersedes_source_ids {
        let target = load_source_relation_target(connection, target_id)?
            .ok_or_else(|| SqliteError::source_invariant(SqliteStorageReason::MissingSource))?;
        if target.0 != params.namespace_id.as_str() {
            return Err(SqliteError::source_invariant(
                SqliteStorageReason::NamespaceMismatch,
            ));
        }
        if target.1 != params.lineage_id.as_str()
            || u64::try_from(target.2)
                .ok()
                .is_none_or(|version| version >= params.version.get())
        {
            return Err(SqliteError::source_invariant(
                SqliteStorageReason::SourceResolution,
            ));
        }
    }
    Ok(())
}

fn validate_stored_superseded_sources(
    connection: &Connection,
    source: &SourceArtifact,
) -> Result<(), SqliteError> {
    let params = source.params();
    for target_id in &params.supersedes_source_ids {
        let target = load_source_relation_target(connection, target_id)?.ok_or_else(|| {
            SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
        })?;
        let valid_target_version = u64::try_from(target.2)
            .ok()
            .is_some_and(|version| version < params.version.get());
        if target.0 != params.namespace_id.as_str()
            || target.1 != params.lineage_id.as_str()
            || !valid_target_version
        {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::StoredIntegrityMismatch,
            ));
        }
    }
    Ok(())
}

fn load_source_relation_target(
    connection: &Connection,
    target_id: &Identifier,
) -> Result<Option<(String, String, i64)>, SqliteError> {
    connection
        .query_row(
            "SELECT namespace_id, lineage_id, version
             FROM radishmemory_source_artifacts WHERE source_id = ?1",
            params![target_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(SqliteError::storage)
}

fn validate_fragment_batch(
    fragments: &[SourceFragment],
) -> Result<(&Identifier, &Identifier), SqliteError> {
    let first = fragments
        .first()
        .ok_or_else(|| SqliteError::source_invariant(SqliteStorageReason::EmptyFragmentBatch))?;
    let namespace_id = &first.params().namespace_id;
    let source_id = &first.params().source_id;
    let mut fragment_ids = BTreeSet::new();
    let mut ordinals = BTreeSet::new();
    for fragment in fragments {
        let params = fragment.params();
        if &params.namespace_id != namespace_id || &params.source_id != source_id {
            return Err(SqliteError::source_invariant(
                SqliteStorageReason::MixedFragmentBatch,
            ));
        }
        if !fragment_ids.insert(&params.fragment_id) || !ordinals.insert(params.ordinal) {
            return Err(SqliteError::source_invariant(
                SqliteStorageReason::DuplicateFragment,
            ));
        }
    }
    Ok((namespace_id, source_id))
}

fn validate_fragment_numbers(fragment: &SourceFragment) -> Result<(), SqliteError> {
    let params = fragment.params();
    to_i64(params.ordinal)?;
    to_i64(params.byte_start)?;
    to_i64(params.byte_end)?;
    if let Some(headings) = &params.heading_path {
        to_i64(usize_to_u64(headings.len())?)?;
    }
    Ok(())
}

pub(crate) fn load_source_artifact(
    connection: &Connection,
    namespace_id: &Identifier,
    source_id: &Identifier,
) -> Result<Option<SourceArtifact>, SqliteError> {
    let stored = connection
        .query_row(
            "SELECT a.source_id, a.canonical_schema_version, a.object_type, a.lineage_id,
                    a.version, a.namespace_id, a.source_kind, a.media_type, a.content_length,
                    a.content_digest_algorithm, a.content_digest_profile,
                    a.content_digest_value, a.title, a.origin_kind, a.origin_ref,
                    a.observed_at, a.captured_at, a.sensitivity, a.egress_policy,
                    a.retention_mode, a.retention_expires_at, a.retention_policy_id,
                    a.deletion_state, a.policy_basis, a.producer_type, a.producer_id,
                    a.producer_version, a.created_at, b.content
             FROM radishmemory_source_artifacts AS a
             LEFT JOIN radishmemory_source_bodies AS b ON b.source_id = a.source_id
             WHERE a.namespace_id = ?1 AND a.source_id = ?2",
            params![namespace_id.as_str(), source_id.as_str()],
            StoredSource::from_row,
        )
        .optional()
        .map_err(SqliteError::storage)?;
    let Some(stored) = stored else {
        return Ok(None);
    };
    let supersedes = load_superseded_ids(connection, source_id)?;
    let source = stored.into_domain(supersedes)?;
    validate_stored_superseded_sources(connection, &source)?;
    Ok(Some(source))
}

pub(crate) fn load_resolved_source_fragment(
    connection: &Connection,
    namespace_id: &Identifier,
    fragment_id: &Identifier,
) -> Result<Option<(SourceFragment, SourceArtifact)>, SqliteError> {
    let stored = connection
        .query_row(
            "SELECT fragment_id, canonical_schema_version, object_type, namespace_id,
                    source_id, ordinal, byte_start, byte_end, content_digest_algorithm,
                    content_digest_profile, content_digest_value, segmenter_type,
                    segmenter_id, segmenter_version, sensitivity, egress_policy,
                    retention_mode, retention_expires_at, retention_policy_id,
                    deletion_state, policy_basis, created_at
             FROM radishmemory_source_fragments
             WHERE fragment_id = ?1 AND namespace_id = ?2",
            params![fragment_id.as_str(), namespace_id.as_str()],
            StoredFragment::from_row,
        )
        .optional()
        .map_err(SqliteError::storage)?;
    let Some(stored) = stored else {
        return Ok(None);
    };
    let source_id = identifier(stored.source_id.clone())?;
    let source = load_source_artifact(connection, namespace_id, &source_id)?
        .ok_or_else(|| SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch))?;
    let headings = load_heading_path(connection, &stored.fragment_id)?;
    let fragment = stored.into_domain(&source, headings)?;
    validate_source_fragment_resolution(&fragment, &source).map_err(|source| {
        SqliteError::invalid_stored_with_source(
            SqliteStorageReason::StoredIntegrityMismatch,
            source,
        )
    })?;
    Ok(Some((fragment, source)))
}

fn load_superseded_ids(
    connection: &Connection,
    source_id: &Identifier,
) -> Result<Vec<Identifier>, SqliteError> {
    let mut statement = connection
        .prepare(
            "SELECT ordinal, superseded_source_id
             FROM radishmemory_source_supersedes
             WHERE source_id = ?1 ORDER BY ordinal",
        )
        .map_err(SqliteError::storage)?;
    let rows = statement
        .query_map(params![source_id.as_str()], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(SqliteError::storage)?;
    let rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(SqliteError::storage)?;
    let mut result = Vec::with_capacity(rows.len());
    for (expected, (ordinal, value)) in rows.into_iter().enumerate() {
        if ordinal != to_i64(usize_to_u64(expected)?)? {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::StoredIntegrityMismatch,
            ));
        }
        result.push(identifier(value)?);
    }
    Ok(result)
}

fn load_heading_path(
    connection: &Connection,
    fragment_id: &str,
) -> Result<Option<Vec<NonEmptyText>>, SqliteError> {
    let mut statement = connection
        .prepare(
            "SELECT ordinal, heading FROM radishmemory_fragment_heading_path
             WHERE fragment_id = ?1 ORDER BY ordinal",
        )
        .map_err(SqliteError::storage)?;
    let rows = statement
        .query_map(params![fragment_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(SqliteError::storage)?;
    let rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(SqliteError::storage)?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut headings = Vec::with_capacity(rows.len());
    for (expected, (ordinal, value)) in rows.into_iter().enumerate() {
        if ordinal != to_i64(usize_to_u64(expected)?)? {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::StoredIntegrityMismatch,
            ));
        }
        headings.push(non_empty_text(value)?);
    }
    Ok(Some(headings))
}

struct StoredSource {
    source_id: String,
    canonical_schema_version: String,
    object_type: String,
    lineage_id: String,
    version: i64,
    namespace_id: String,
    source_kind: String,
    media_type: String,
    content_length: i64,
    digest_algorithm: String,
    digest_profile: String,
    digest_value: String,
    title: Option<String>,
    origin_kind: String,
    origin_ref: Option<String>,
    observed_at: String,
    captured_at: String,
    governance: StoredGovernance,
    producer: StoredProducer,
    created_at: String,
    content: Option<Vec<u8>>,
}

impl StoredSource {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            source_id: row.get("source_id")?,
            canonical_schema_version: row.get("canonical_schema_version")?,
            object_type: row.get("object_type")?,
            lineage_id: row.get("lineage_id")?,
            version: row.get("version")?,
            namespace_id: row.get("namespace_id")?,
            source_kind: row.get("source_kind")?,
            media_type: row.get("media_type")?,
            content_length: row.get("content_length")?,
            digest_algorithm: row.get("content_digest_algorithm")?,
            digest_profile: row.get("content_digest_profile")?,
            digest_value: row.get("content_digest_value")?,
            title: row.get("title")?,
            origin_kind: row.get("origin_kind")?,
            origin_ref: row.get("origin_ref")?,
            observed_at: row.get("observed_at")?,
            captured_at: row.get("captured_at")?,
            governance: StoredGovernance::from_row(row)?,
            producer: StoredProducer::from_row(
                row,
                "producer_type",
                "producer_id",
                "producer_version",
            )?,
            created_at: row.get("created_at")?,
            content: row.get("content")?,
        })
    }

    fn into_domain(
        self,
        supersedes_source_ids: Vec<Identifier>,
    ) -> Result<SourceArtifact, SqliteError> {
        if self.canonical_schema_version != M0_SCHEMA_VERSION
            || self.object_type != CanonicalObjectType::SourceArtifact.as_str()
        {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::StoredIntegrityMismatch,
            ));
        }
        let content = self.content.ok_or_else(|| {
            SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
        })?;
        let content = String::from_utf8(content)
            .map_err(|_| SqliteError::invalid_stored(SqliteStorageReason::InvalidUtf8))?;
        let params = SourceArtifactParams {
            source_id: identifier(self.source_id)?,
            lineage_id: identifier(self.lineage_id)?,
            version: version(self.version)?,
            namespace_id: identifier(self.namespace_id)?,
            source_kind: parse_source_kind(&self.source_kind)?,
            media_type: parse_media_type(&self.media_type)?,
            content: non_empty_text(content)?,
            content_length: from_i64(self.content_length)?,
            content_digest: digest(
                &self.digest_algorithm,
                &self.digest_profile,
                &self.digest_value,
            )?,
            title: optional_text(self.title)?,
            origin_kind: parse_source_origin_kind(&self.origin_kind)?,
            origin_ref: optional_text(self.origin_ref)?,
            observed_at: timestamp(&self.observed_at)?,
            captured_at: timestamp(&self.captured_at)?,
            supersedes_source_ids,
            governance: self.governance.into_domain()?,
            producer: self.producer.into_domain()?,
            created_at: timestamp(&self.created_at)?,
        };
        SourceArtifact::new(params).map_err(invalid_core)
    }
}

struct StoredFragment {
    fragment_id: String,
    canonical_schema_version: String,
    object_type: String,
    namespace_id: String,
    source_id: String,
    ordinal: i64,
    byte_start: i64,
    byte_end: i64,
    digest_algorithm: String,
    digest_profile: String,
    digest_value: String,
    segmenter: StoredProducer,
    governance: StoredGovernance,
    created_at: String,
}

impl StoredFragment {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            fragment_id: row.get("fragment_id")?,
            canonical_schema_version: row.get("canonical_schema_version")?,
            object_type: row.get("object_type")?,
            namespace_id: row.get("namespace_id")?,
            source_id: row.get("source_id")?,
            ordinal: row.get("ordinal")?,
            byte_start: row.get("byte_start")?,
            byte_end: row.get("byte_end")?,
            digest_algorithm: row.get("content_digest_algorithm")?,
            digest_profile: row.get("content_digest_profile")?,
            digest_value: row.get("content_digest_value")?,
            segmenter: StoredProducer::from_row(
                row,
                "segmenter_type",
                "segmenter_id",
                "segmenter_version",
            )?,
            governance: StoredGovernance::from_row(row)?,
            created_at: row.get("created_at")?,
        })
    }

    fn into_domain(
        self,
        source: &SourceArtifact,
        heading_path: Option<Vec<NonEmptyText>>,
    ) -> Result<SourceFragment, SqliteError> {
        if self.canonical_schema_version != M0_SCHEMA_VERSION
            || self.object_type != CanonicalObjectType::SourceFragment.as_str()
        {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::StoredIntegrityMismatch,
            ));
        }
        let byte_start = from_i64(self.byte_start)?;
        let byte_end = from_i64(self.byte_end)?;
        let start = usize::try_from(byte_start)
            .map_err(|_| SqliteError::invalid_stored(SqliteStorageReason::NumericRange))?;
        let end = usize::try_from(byte_end)
            .map_err(|_| SqliteError::invalid_stored(SqliteStorageReason::NumericRange))?;
        let content = source
            .params()
            .content
            .as_str()
            .get(start..end)
            .ok_or_else(|| {
                SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
            })?
            .to_owned();
        let params = SourceFragmentParams {
            fragment_id: identifier(self.fragment_id)?,
            namespace_id: identifier(self.namespace_id)?,
            source_id: identifier(self.source_id)?,
            ordinal: from_i64(self.ordinal)?,
            byte_start,
            byte_end,
            heading_path,
            content: non_empty_text(content)?,
            content_digest: digest(
                &self.digest_algorithm,
                &self.digest_profile,
                &self.digest_value,
            )?,
            segmenter: self.segmenter.into_domain()?,
            governance: self.governance.into_domain()?,
            created_at: timestamp(&self.created_at)?,
        };
        SourceFragment::new(params).map_err(invalid_core)
    }
}

pub(crate) struct StoredGovernance {
    sensitivity: String,
    egress_policy: String,
    retention_mode: String,
    retention_expires_at: Option<String>,
    retention_policy_id: Option<String>,
    deletion_state: String,
    policy_basis: String,
}

impl StoredGovernance {
    pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            sensitivity: row.get("sensitivity")?,
            egress_policy: row.get("egress_policy")?,
            retention_mode: row.get("retention_mode")?,
            retention_expires_at: row.get("retention_expires_at")?,
            retention_policy_id: row.get("retention_policy_id")?,
            deletion_state: row.get("deletion_state")?,
            policy_basis: row.get("policy_basis")?,
        })
    }

    pub(crate) fn into_domain(self) -> Result<Governance, SqliteError> {
        let retention = RetentionRule::new(
            parse_retention_mode(&self.retention_mode)?,
            self.retention_expires_at
                .as_deref()
                .map(timestamp)
                .transpose()?,
            self.retention_policy_id.map(identifier).transpose()?,
        )
        .map_err(invalid_core)?;
        Governance::new(
            parse_sensitivity(&self.sensitivity)?,
            parse_egress_policy(&self.egress_policy)?,
            retention,
            parse_deletion_state(&self.deletion_state)?,
            identifier(self.policy_basis)?,
        )
        .map_err(invalid_core)
    }
}

pub(crate) struct StoredProducer {
    producer_type: String,
    producer_id: String,
    producer_version: String,
}

impl StoredProducer {
    pub(crate) fn from_row(
        row: &Row<'_>,
        type_column: &str,
        id_column: &str,
        version_column: &str,
    ) -> rusqlite::Result<Self> {
        Ok(Self {
            producer_type: row.get(type_column)?,
            producer_id: row.get(id_column)?,
            producer_version: row.get(version_column)?,
        })
    }

    pub(crate) fn into_domain(self) -> Result<ProducerRef, SqliteError> {
        Ok(ProducerRef::new(
            parse_producer_type(&self.producer_type)?,
            identifier(self.producer_id)?,
            non_empty_text(self.producer_version)?,
        ))
    }
}

pub(crate) fn to_i64(value: u64) -> Result<i64, SqliteError> {
    i64::try_from(value)
        .map_err(|_| SqliteError::source_invariant(SqliteStorageReason::NumericRange))
}

pub(crate) fn from_i64(value: i64) -> Result<u64, SqliteError> {
    u64::try_from(value).map_err(|_| SqliteError::invalid_stored(SqliteStorageReason::NumericRange))
}

pub(crate) fn usize_to_u64(value: usize) -> Result<u64, SqliteError> {
    u64::try_from(value)
        .map_err(|_| SqliteError::source_invariant(SqliteStorageReason::NumericRange))
}

pub(crate) fn identifier(value: String) -> Result<Identifier, SqliteError> {
    Identifier::new(value).map_err(invalid_core)
}

pub(crate) fn non_empty_text(value: String) -> Result<NonEmptyText, SqliteError> {
    NonEmptyText::new(value).map_err(invalid_core)
}

pub(crate) fn optional_text(value: Option<String>) -> Result<Option<NonEmptyText>, SqliteError> {
    value.map(non_empty_text).transpose()
}

pub(crate) fn version(value: i64) -> Result<Version, SqliteError> {
    Version::new(from_i64(value)?).map_err(invalid_core)
}

pub(crate) fn timestamp(value: &str) -> Result<Timestamp, SqliteError> {
    Timestamp::parse(value).map_err(invalid_core)
}

pub(crate) fn digest(algorithm: &str, profile: &str, value: &str) -> Result<Digest, SqliteError> {
    Digest::parse(algorithm, profile, value).map_err(invalid_core)
}

pub(crate) fn invalid_core(source: CoreError) -> SqliteError {
    let reason = if source.code() == CoreErrorCode::DigestMismatch {
        SqliteStorageReason::StoredIntegrityMismatch
    } else {
        SqliteStorageReason::InvalidCanonicalObject
    };
    SqliteError::invalid_stored_with_source(reason, source)
}

fn unknown_enum<T>() -> Result<T, SqliteError> {
    Err(SqliteError::invalid_stored(
        SqliteStorageReason::UnknownEnum,
    ))
}

fn source_kind_str(value: SourceKind) -> &'static str {
    match value {
        SourceKind::Text => "text",
        SourceKind::Markdown => "markdown",
    }
}

fn parse_source_kind(value: &str) -> Result<SourceKind, SqliteError> {
    match value {
        "text" => Ok(SourceKind::Text),
        "markdown" => Ok(SourceKind::Markdown),
        _ => unknown_enum(),
    }
}

fn parse_media_type(value: &str) -> Result<MediaType, SqliteError> {
    match value {
        "text/plain" => Ok(MediaType::TextPlain),
        "text/markdown" => Ok(MediaType::TextMarkdown),
        _ => unknown_enum(),
    }
}

fn source_origin_kind_str(value: SourceOriginKind) -> &'static str {
    match value {
        SourceOriginKind::SyntheticFixture => "synthetic_fixture",
        SourceOriginKind::ExplicitUserInput => "explicit_user_input",
    }
}

fn parse_source_origin_kind(value: &str) -> Result<SourceOriginKind, SqliteError> {
    match value {
        "synthetic_fixture" => Ok(SourceOriginKind::SyntheticFixture),
        "explicit_user_input" => Ok(SourceOriginKind::ExplicitUserInput),
        _ => unknown_enum(),
    }
}

pub(crate) fn sensitivity_str(value: Sensitivity) -> &'static str {
    match value {
        Sensitivity::Personal => "personal",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Restricted => "restricted",
    }
}

fn parse_sensitivity(value: &str) -> Result<Sensitivity, SqliteError> {
    match value {
        "personal" => Ok(Sensitivity::Personal),
        "sensitive" => Ok(Sensitivity::Sensitive),
        "restricted" => Ok(Sensitivity::Restricted),
        _ => unknown_enum(),
    }
}

pub(crate) fn egress_policy_str(value: EgressPolicy) -> &'static str {
    match value {
        EgressPolicy::LocalOnly => "local_only",
        EgressPolicy::TrustedDeviceOnly => "trusted_device_only",
        EgressPolicy::TrustedServerOnly => "trusted_server_only",
        EgressPolicy::CloudAllowed => "cloud_allowed",
    }
}

fn parse_egress_policy(value: &str) -> Result<EgressPolicy, SqliteError> {
    match value {
        "local_only" => Ok(EgressPolicy::LocalOnly),
        "trusted_device_only" => Ok(EgressPolicy::TrustedDeviceOnly),
        "trusted_server_only" => Ok(EgressPolicy::TrustedServerOnly),
        "cloud_allowed" => Ok(EgressPolicy::CloudAllowed),
        _ => unknown_enum(),
    }
}

pub(crate) fn retention_mode_str(value: RetentionMode) -> &'static str {
    match value {
        RetentionMode::UntilDeleted => "until_deleted",
        RetentionMode::UntilTime => "until_time",
        RetentionMode::Policy => "policy",
    }
}

fn parse_retention_mode(value: &str) -> Result<RetentionMode, SqliteError> {
    match value {
        "until_deleted" => Ok(RetentionMode::UntilDeleted),
        "until_time" => Ok(RetentionMode::UntilTime),
        "policy" => Ok(RetentionMode::Policy),
        _ => unknown_enum(),
    }
}

pub(crate) fn deletion_state_str(value: DeletionState) -> &'static str {
    match value {
        DeletionState::Active => "active",
        DeletionState::Pending => "pending",
        DeletionState::Failed => "failed",
        DeletionState::Deleted => "deleted",
    }
}

fn parse_deletion_state(value: &str) -> Result<DeletionState, SqliteError> {
    match value {
        "active" => Ok(DeletionState::Active),
        "pending" => Ok(DeletionState::Pending),
        "failed" => Ok(DeletionState::Failed),
        "deleted" => Ok(DeletionState::Deleted),
        _ => unknown_enum(),
    }
}

pub(crate) fn producer_type_str(value: ProducerType) -> &'static str {
    match value {
        ProducerType::Rule => "rule",
        ProducerType::Parser => "parser",
        ProducerType::TestFixture => "test_fixture",
        ProducerType::System => "system",
    }
}

fn parse_producer_type(value: &str) -> Result<ProducerType, SqliteError> {
    match value {
        "rule" => Ok(ProducerType::Rule),
        "parser" => Ok(ProducerType::Parser),
        "test_fixture" => Ok(ProducerType::TestFixture),
        "system" => Ok(ProducerType::System),
        _ => unknown_enum(),
    }
}
