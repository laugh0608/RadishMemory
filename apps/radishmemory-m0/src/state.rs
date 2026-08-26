use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use radishmemory_core::{
    ActorRef, ActorType, ComponentResult, ContextPack, DeleteRequest, DeletionEvidence,
    DeletionState, EgressPolicy, Governance, Identifier, LocalSearchHit, MemoryDecision,
    MemoryProposal, MemoryRecord, MemoryStateEvent, NonEmptyText, ProducerRef, ProducerType,
    RetentionMode, RetentionRule, Sensitivity, SourceArtifact, SourceFragment, Timestamp,
};
use radishmemory_sqlite::SqliteDatabase;

use crate::error::{RunnerError, RunnerErrorCode, RunnerResult};
use crate::fixture::stable_fixture_id;

static DATABASE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TemporaryDatabase {
    path: PathBuf,
    pub database: SqliteDatabase,
}

impl TemporaryDatabase {
    fn new(isolation_key: &str) -> RunnerResult<Self> {
        let counter = DATABASE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "radishmemory-m0-{}-{}-{counter}.sqlite3",
            std::process::id(),
            isolation_key
        ));
        let database = SqliteDatabase::open(&path).map_err(|source| {
            RunnerError::with_source(
                RunnerErrorCode::Storage,
                "scenario-database-open-failed",
                source,
            )
        })?;
        Ok(Self { path, database })
    }
}

impl Drop for TemporaryDatabase {
    fn drop(&mut self) {
        remove_database_file(&self.path);
        for suffix in ["-journal", "-wal", "-shm"] {
            let mut adjacent = self.path.as_os_str().to_os_string();
            adjacent.push(suffix);
            remove_database_file(Path::new(&adjacent));
        }
    }
}

fn remove_database_file(path: &Path) {
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        // Cleanup failure cannot weaken a completed assertion. The path is never logged.
    }
}

#[derive(Clone)]
pub struct SearchSnapshot {
    pub query: String,
    pub as_of: Timestamp,
    pub hits: Vec<LocalSearchHit>,
    pub source_keys: BTreeSet<String>,
    pub memory_keys: BTreeSet<String>,
}

#[derive(Default)]
pub struct MetricFacts {
    pub citation_numerator: u64,
    pub citation_denominator: u64,
    pub retrieval_numerator: u64,
    pub retrieval_denominator: u64,
    pub deletion_numerator: u64,
    pub deletion_denominator: u64,
    pub relevant_source_set_drift_count: u64,
    pub duplicate_reproposal_count: u64,
    pub silent_overwrite_count: u64,
}

pub struct ExecutionSnapshot {
    pub results: Vec<ComponentResult>,
    pub expected_error_code: Option<String>,
}

pub struct ScenarioState {
    pub scenario_id: String,
    pub namespace_id: Identifier,
    pub device_id: Identifier,
    pub storage: TemporaryDatabase,
    pub sources: BTreeMap<String, SourceArtifact>,
    pub source_keys_by_id: BTreeMap<Identifier, String>,
    pub fragments: BTreeMap<String, Vec<SourceFragment>>,
    pub fragment_keys_by_id: BTreeMap<Identifier, String>,
    pub proposals: BTreeMap<String, MemoryProposal>,
    pub decisions: BTreeMap<String, MemoryDecision>,
    pub records: BTreeMap<String, MemoryRecord>,
    pub record_keys_by_id: BTreeMap<Identifier, String>,
    pub events: BTreeMap<String, Vec<MemoryStateEvent>>,
    pub searches: BTreeMap<String, SearchSnapshot>,
    pub contexts: BTreeMap<String, ContextPack>,
    pub conflicts: BTreeMap<String, Vec<String>>,
    pub delete_requests: BTreeMap<String, DeleteRequest>,
    pub executions: BTreeMap<String, ExecutionSnapshot>,
    pub evidences: BTreeMap<String, DeletionEvidence>,
    pub emitted: BTreeMap<String, (String, Option<(String, String)>)>,
    pub metrics: MetricFacts,
    pub network_request_count: u64,
    pub provider_artifact_count: u64,
}

impl ScenarioState {
    pub fn new(
        scenario_id: &str,
        isolation_key: &str,
        namespace_id: &str,
        device_id: &str,
    ) -> RunnerResult<Self> {
        Ok(Self {
            scenario_id: scenario_id.to_owned(),
            namespace_id: id(namespace_id)?,
            device_id: id(device_id)?,
            storage: TemporaryDatabase::new(isolation_key)?,
            sources: BTreeMap::new(),
            source_keys_by_id: BTreeMap::new(),
            fragments: BTreeMap::new(),
            fragment_keys_by_id: BTreeMap::new(),
            proposals: BTreeMap::new(),
            decisions: BTreeMap::new(),
            records: BTreeMap::new(),
            record_keys_by_id: BTreeMap::new(),
            events: BTreeMap::new(),
            searches: BTreeMap::new(),
            contexts: BTreeMap::new(),
            conflicts: BTreeMap::new(),
            delete_requests: BTreeMap::new(),
            executions: BTreeMap::new(),
            evidences: BTreeMap::new(),
            emitted: BTreeMap::new(),
            metrics: MetricFacts::default(),
            network_request_count: 0,
            provider_artifact_count: 0,
        })
    }

    pub fn stable_id(&self, object_type: &str, logical_key: &str) -> RunnerResult<Identifier> {
        id(&stable_fixture_id(
            &self.scenario_id,
            object_type,
            logical_key,
        )?)
    }

    pub fn helper_id(&self, kind: &str, logical_key: &str) -> RunnerResult<Identifier> {
        id(&format!(
            "urn:radishmemory:fixture:{}:{kind}:{logical_key}",
            self.scenario_id.to_ascii_lowercase()
        ))
    }

    pub fn emit(
        &mut self,
        logical_key: &str,
        object_id: &Identifier,
        digest: Option<(&str, &str)>,
    ) {
        self.emitted.insert(
            logical_key.to_owned(),
            (
                object_id.as_str().to_owned(),
                digest.map(|(profile, value)| (profile.to_owned(), value.to_owned())),
            ),
        );
    }
}

pub fn id(value: &str) -> RunnerResult<Identifier> {
    Identifier::new(value).map_err(|source| {
        RunnerError::with_source(
            RunnerErrorCode::OperationFailed,
            "canonical-identifier-invalid",
            source,
        )
    })
}

pub fn text(value: &str) -> RunnerResult<NonEmptyText> {
    NonEmptyText::new(value).map_err(|source| {
        RunnerError::with_source(
            RunnerErrorCode::OperationFailed,
            "canonical-text-invalid",
            source,
        )
    })
}

pub fn timestamp(value: &str) -> RunnerResult<Timestamp> {
    Timestamp::parse(value).map_err(|source| {
        RunnerError::with_source(
            RunnerErrorCode::OperationFailed,
            "canonical-time-invalid",
            source,
        )
    })
}

pub fn governance() -> RunnerResult<Governance> {
    Governance::new(
        Sensitivity::Personal,
        EgressPolicy::LocalOnly,
        RetentionRule::new(RetentionMode::UntilDeleted, None, None).map_err(|source| {
            RunnerError::with_source(
                RunnerErrorCode::OperationFailed,
                "retention-invalid",
                source,
            )
        })?,
        DeletionState::Active,
        id("policy:m0:local-only")?,
    )
    .map_err(|source| {
        RunnerError::with_source(
            RunnerErrorCode::OperationFailed,
            "governance-invalid",
            source,
        )
    })
}

pub fn fixture_producer() -> RunnerResult<ProducerRef> {
    Ok(ProducerRef::new(
        ProducerType::TestFixture,
        id("fixture:m0-runner")?,
        text("1")?,
    ))
}

pub fn producer(value: &str) -> RunnerResult<ProducerRef> {
    let (kind, identifier) = value.split_once(':').ok_or_else(|| {
        RunnerError::new(
            RunnerErrorCode::InvalidFixture,
            "producer-reference-invalid",
        )
    })?;
    let producer_type = match kind {
        "rule" => ProducerType::Rule,
        "parser" => ProducerType::Parser,
        _ => {
            return Err(RunnerError::new(
                RunnerErrorCode::InvalidFixture,
                "producer-reference-invalid",
            ));
        }
    };
    Ok(ProducerRef::new(
        producer_type,
        id(value)?,
        text(identifier.rsplit('-').next().unwrap_or("1"))?,
    ))
}

pub fn actor(value: &str) -> RunnerResult<ActorRef> {
    let actor_type = if value.starts_with("user:") {
        ActorType::User
    } else if value.starts_with("device:") {
        ActorType::Device
    } else {
        ActorType::TestFixture
    };
    Ok(ActorRef::new(actor_type, id(value)?, None))
}
