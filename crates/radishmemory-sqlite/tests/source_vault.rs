use radishmemory_core::{
    DeletionState, EgressPolicy, Governance, Identifier, MediaType, NonEmptyText, ProducerRef,
    ProducerType, RetentionMode, RetentionRule, Sensitivity, SourceArtifact, SourceArtifactParams,
    SourceFragment, SourceFragmentParams, SourceKind, SourceOriginKind, SourceVault, Timestamp,
    Version, compute_exact_bytes_digest,
};
use radishmemory_file_entry as _;
use radishmemory_sqlite::{SqliteDatabase, SqliteErrorCode, SqliteStorageReason};
use rusqlite::{Connection, params};

mod support;

use support::SyntheticDatabase;

fn id(value: &str) -> Identifier {
    Identifier::new(value).expect("synthetic identifier must be valid")
}

fn text(value: &str) -> NonEmptyText {
    NonEmptyText::new(value).expect("synthetic text must be nonempty")
}

fn timestamp(value: &str) -> Timestamp {
    Timestamp::parse(value).expect("synthetic timestamp must be valid")
}

fn governance() -> Governance {
    Governance::new(
        Sensitivity::Personal,
        EgressPolicy::LocalOnly,
        RetentionRule::new(RetentionMode::UntilDeleted, None, None)
            .expect("retention must be valid"),
        DeletionState::Active,
        id("policy-local-only"),
    )
    .expect("M0 governance must be local")
}

fn producer() -> ProducerRef {
    ProducerRef::new(ProducerType::TestFixture, id("fixture-producer"), text("1"))
}

fn source(
    source_id: &str,
    lineage_id: &str,
    version: u64,
    namespace_id: &str,
    content_value: &str,
    supersedes_source_ids: Vec<Identifier>,
) -> SourceArtifact {
    let content = text(content_value);
    SourceArtifact::new(SourceArtifactParams {
        source_id: id(source_id),
        lineage_id: id(lineage_id),
        version: Version::new(version).expect("version must be positive"),
        namespace_id: id(namespace_id),
        source_kind: SourceKind::Markdown,
        media_type: MediaType::TextMarkdown,
        content_length: u64::try_from(content.utf8_len()).expect("content length must fit u64"),
        content_digest: compute_exact_bytes_digest(content.as_str().as_bytes()),
        content,
        title: Some(text("Synthetic source")),
        origin_kind: SourceOriginKind::SyntheticFixture,
        origin_ref: Some(text("fixture://source")),
        observed_at: timestamp("2026-08-23T08:00:00+08:00"),
        captured_at: timestamp("2026-08-23T00:00:01Z"),
        supersedes_source_ids,
        governance: governance(),
        producer: producer(),
        created_at: timestamp("2026-08-23T00:00:01.000Z"),
    })
    .expect("synthetic source must be valid")
}

fn fragment(
    source: &SourceArtifact,
    fragment_id: &str,
    ordinal: u64,
    byte_start: usize,
    byte_end: usize,
) -> SourceFragment {
    let content = source
        .params()
        .content
        .as_str()
        .get(byte_start..byte_end)
        .expect("synthetic byte range must resolve");
    SourceFragment::new(SourceFragmentParams {
        fragment_id: id(fragment_id),
        namespace_id: source.params().namespace_id.clone(),
        source_id: source.params().source_id.clone(),
        ordinal,
        byte_start: u64::try_from(byte_start).expect("byte start must fit u64"),
        byte_end: u64::try_from(byte_end).expect("byte end must fit u64"),
        heading_path: Some(vec![text("Note"), text("Details")]),
        content_digest: compute_exact_bytes_digest(content.as_bytes()),
        content: text(content),
        segmenter: producer(),
        governance: source.params().governance.clone(),
        created_at: timestamp("2026-08-23T00:00:02Z"),
    })
    .expect("synthetic fragment must be valid")
}

#[test]
fn source_metadata_and_exact_body_blob_round_trip_without_namespace_leakage() {
    let synthetic = SyntheticDatabase::new("source-round-trip");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let expected = source(
        "source-1",
        "source-lineage-1",
        1,
        "namespace-1",
        "# Note\r\nCafé 🌱\n",
        vec![],
    );

    database
        .store_source_artifact(&expected)
        .expect("source must persist");

    let loaded = database
        .load_source_artifact(&id("namespace-1"), &id("source-1"))
        .expect("source lookup must succeed")
        .expect("source must exist");
    assert_eq!(loaded, expected);
    assert_eq!(
        loaded.params().content.as_str().as_bytes(),
        expected.params().content.as_str().as_bytes()
    );
    assert_eq!(
        loaded.params().observed_at.original(),
        "2026-08-23T08:00:00+08:00"
    );
    assert_eq!(
        loaded.params().created_at.original(),
        "2026-08-23T00:00:01.000Z"
    );
    assert!(
        database
            .load_source_artifact(&id("namespace-other"), &id("source-1"))
            .expect("wrong-namespace lookup must remain safe")
            .is_none()
    );

    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    let (storage_type, body): (String, Vec<u8>) = raw
        .query_row(
            "SELECT typeof(content), content FROM radishmemory_source_bodies
             WHERE source_id = ?1",
            params!["source-1"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("stored body must be queryable");
    assert_eq!(storage_type, "blob");
    assert_eq!(body, expected.params().content.as_str().as_bytes());
}

#[test]
fn fragments_rebuild_exact_content_from_source_body_in_stable_order() {
    let synthetic = SyntheticDatabase::new("fragment-round-trip");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let source = source(
        "source-1",
        "source-lineage-1",
        1,
        "namespace-1",
        "# Note\r\nCafé 🌱\nSecond line.\n",
        vec![],
    );
    database
        .store_source_artifact(&source)
        .expect("source must persist");
    let empty_error = database
        .store_source_fragments(&[])
        .expect_err("empty fragment batch must not report success");
    assert_eq!(empty_error.code(), SqliteErrorCode::SourceInvariant);
    assert_eq!(
        empty_error.storage_reason(),
        Some(SqliteStorageReason::EmptyFragmentBatch)
    );
    let body = source.params().content.as_str();
    let first_start = body.find("Café").expect("first fragment must exist");
    let first_end = first_start + "Café 🌱\n".len();
    let second_start = first_end;
    let second_end = body.len();
    let first = fragment(&source, "fragment-1", 0, first_start, first_end);
    let second = fragment(&source, "fragment-2", 1, second_start, second_end);

    database
        .store_source_fragments(&[first.clone(), second.clone()])
        .expect("fragment batch must persist");

    let loaded = database
        .load_source_fragments(&id("namespace-1"), &id("source-1"))
        .expect("fragment lookup must succeed")
        .expect("source must exist");
    assert_eq!(loaded, vec![first, second]);
    assert_eq!(loaded[0].params().content.as_str(), "Café 🌱\n");
    assert_eq!(
        loaded[0]
            .params()
            .heading_path
            .as_ref()
            .expect("heading path must exist")
            .iter()
            .map(NonEmptyText::as_str)
            .collect::<Vec<_>>(),
        vec!["Note", "Details"]
    );

    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    let fragment_columns = raw
        .prepare("SELECT name FROM pragma_table_info('radishmemory_source_fragments')")
        .expect("fragment schema must be queryable")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("fragment columns must be queryable")
        .collect::<Result<Vec<_>, _>>()
        .expect("fragment columns must decode");
    assert!(!fragment_columns.iter().any(|column| column == "content"));

    raw.execute(
        "UPDATE radishmemory_source_fragments SET namespace_id = ?1 WHERE fragment_id = ?2",
        params!["namespace-other", "fragment-1"],
    )
    .expect("synthetic fragment namespace must be tampered for the test");
    drop(raw);
    let error = database
        .load_source_fragments(&id("namespace-1"), &id("source-1"))
        .expect_err("stored fragment namespace drift must fail closed");
    assert_eq!(error.code(), SqliteErrorCode::InvalidStoredData);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::StoredIntegrityMismatch)
    );
}

#[test]
fn duplicate_source_conflicts_without_overwriting_original_body() {
    let synthetic = SyntheticDatabase::new("source-conflict");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let original = source(
        "source-1",
        "source-lineage-1",
        1,
        "namespace-1",
        "Original synthetic body.\n",
        vec![],
    );
    let replacement = source(
        "source-1",
        "source-lineage-other",
        1,
        "namespace-1",
        "Replacement synthetic body.\n",
        vec![],
    );
    database
        .store_source_artifact(&original)
        .expect("original source must persist");

    let error = database
        .store_source_artifact(&replacement)
        .expect_err("immutable source ID must reject overwrite");
    assert_eq!(error.code(), SqliteErrorCode::Conflict);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::DuplicateObject)
    );
    assert!(!format!("{error:?}").contains("Replacement synthetic body"));

    let loaded = database
        .load_source_artifact(&id("namespace-1"), &id("source-1"))
        .expect("source lookup must succeed")
        .expect("original source must remain");
    assert_eq!(loaded.params().content, original.params().content);
}

#[test]
fn fragment_conflict_rolls_back_the_entire_new_batch() {
    let synthetic = SyntheticDatabase::new("fragment-rollback");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let source = source(
        "source-1",
        "source-lineage-1",
        1,
        "namespace-1",
        "First.\nSecond.\nThird.\n",
        vec![],
    );
    database
        .store_source_artifact(&source)
        .expect("source must persist");
    let first = fragment(&source, "fragment-existing", 0, 0, "First.\n".len());
    database
        .store_source_fragments(std::slice::from_ref(&first))
        .expect("initial fragment must persist");
    let second_start = "First.\n".len();
    let second_end = second_start + "Second.\n".len();
    let new_fragment = fragment(&source, "fragment-new", 1, second_start, second_end);
    let duplicate = fragment(
        &source,
        "fragment-existing",
        2,
        second_end,
        source.params().content.utf8_len(),
    );

    let error = database
        .store_source_fragments(&[new_fragment, duplicate])
        .expect_err("duplicate persisted fragment ID must fail atomically");
    assert_eq!(error.code(), SqliteErrorCode::Conflict);

    let loaded = database
        .load_source_fragments(&id("namespace-1"), &id("source-1"))
        .expect("fragment lookup must succeed")
        .expect("source must exist");
    assert_eq!(loaded, vec![first]);
}

#[test]
fn tampered_source_body_fails_integrity_without_echoing_content() {
    let synthetic = SyntheticDatabase::new("body-integrity");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let source = source(
        "source-1",
        "source-lineage-1",
        1,
        "namespace-1",
        "Original synthetic body.\n",
        vec![],
    );
    database
        .store_source_artifact(&source)
        .expect("source must persist");
    let tampered = b"Tampered synthetic body.\n";
    assert_eq!(tampered.len(), source.params().content.utf8_len());
    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    raw.execute(
        "UPDATE radishmemory_source_bodies SET content = ?1 WHERE source_id = ?2",
        params![tampered.as_slice(), "source-1"],
    )
    .expect("synthetic body must be tampered for the test");
    drop(raw);

    let error = database
        .load_source_artifact(&id("namespace-1"), &id("source-1"))
        .expect_err("digest mismatch must fail closed");
    assert_eq!(error.code(), SqliteErrorCode::InvalidStoredData);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::StoredIntegrityMismatch)
    );
    assert!(!format!("{error:?}").contains("Tampered synthetic body"));

    let invalid_utf8 = vec![0xff_u8; source.params().content.utf8_len()];
    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    raw.execute(
        "UPDATE radishmemory_source_bodies SET content = ?1 WHERE source_id = ?2",
        params![invalid_utf8, "source-1"],
    )
    .expect("synthetic body must accept invalid UTF-8 bytes for the test");
    drop(raw);
    let error = database
        .load_source_artifact(&id("namespace-1"), &id("source-1"))
        .expect_err("invalid stored UTF-8 must fail closed");
    assert_eq!(error.code(), SqliteErrorCode::InvalidStoredData);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::InvalidUtf8)
    );
    assert!(format!("{error:?}").contains("has_source: false"));
}

#[test]
fn source_version_relations_require_existing_same_namespace_lineage() {
    let synthetic = SyntheticDatabase::new("source-version-chain");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let first = source(
        "source-1",
        "source-lineage-1",
        1,
        "namespace-1",
        "Version one.\n",
        vec![],
    );
    database
        .store_source_artifact(&first)
        .expect("first source version must persist");
    let second = source(
        "source-2",
        "source-lineage-1",
        2,
        "namespace-1",
        "Version two.\n",
        vec![id("source-1")],
    );
    database
        .store_source_artifact(&second)
        .expect("valid second source version must persist");
    let loaded = database
        .load_source_artifact(&id("namespace-1"), &id("source-2"))
        .expect("source lookup must succeed")
        .expect("second source version must exist");
    assert_eq!(loaded.params().supersedes_source_ids, vec![id("source-1")]);

    let invalid = source(
        "source-3",
        "different-lineage",
        2,
        "namespace-1",
        "Invalid version.\n",
        vec![id("source-1")],
    );
    let error = database
        .store_source_artifact(&invalid)
        .expect_err("cross-lineage supersession must fail closed");
    assert_eq!(error.code(), SqliteErrorCode::SourceInvariant);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::SourceResolution)
    );

    let unrelated = source(
        "source-unrelated",
        "source-lineage-unrelated",
        1,
        "namespace-1",
        "Unrelated source.\n",
        vec![],
    );
    database
        .store_source_artifact(&unrelated)
        .expect("unrelated source must persist");
    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    raw.execute(
        "UPDATE radishmemory_source_supersedes
         SET superseded_source_id = ?1 WHERE source_id = ?2",
        params!["source-unrelated", "source-2"],
    )
    .expect("synthetic relation must be tampered for the test");
    drop(raw);

    let error = database
        .load_source_artifact(&id("namespace-1"), &id("source-2"))
        .expect_err("stored cross-lineage relation must fail closed");
    assert_eq!(error.code(), SqliteErrorCode::InvalidStoredData);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::StoredIntegrityMismatch)
    );
}
