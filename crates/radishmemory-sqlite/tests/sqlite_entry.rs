use radishmemory_core as _;
use radishmemory_sqlite::{
    REVIEWED_BUNDLED_SQLITE_VERSION, SQLITE_SCHEMA_VERSION, SqliteDatabase, SqliteErrorCode,
};
use rusqlite::Connection;

mod support;

use support::SyntheticDatabase;

#[test]
fn new_database_initializes_capabilities_and_reopens_current_schema() {
    let synthetic = SyntheticDatabase::new("initialize");
    let database = SqliteDatabase::open(synthetic.path()).expect("new database must initialize");

    assert_eq!(database.schema_version(), SQLITE_SCHEMA_VERSION);
    assert_eq!(
        database.capabilities().sqlite_version(),
        REVIEWED_BUNDLED_SQLITE_VERSION
    );
    assert!(database.capabilities().fts5());
    drop(database);

    let reopened = SqliteDatabase::open(synthetic.path()).expect("current schema must reopen");
    assert_eq!(reopened.schema_version(), SQLITE_SCHEMA_VERSION);
}

#[test]
fn newer_schema_version_fails_closed_without_migration() {
    let synthetic = SyntheticDatabase::new("future-schema");
    let connection = Connection::open(synthetic.path()).expect("synthetic database must open");
    let future_version = SQLITE_SCHEMA_VERSION + 1;
    let journal_mode: String = connection
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .expect("future database journal mode must be writable");
    assert_eq!(journal_mode, "wal");
    connection
        .pragma_update(None, "user_version", future_version)
        .expect("future schema marker must be writable");
    drop(connection);

    let error = SqliteDatabase::open(synthetic.path()).expect_err("future schema must fail closed");
    assert_eq!(error.code(), SqliteErrorCode::UnsupportedSchemaVersion);
    assert_eq!(
        error.found_schema_version(),
        Some(i64::from(future_version))
    );
    assert_eq!(
        error.supported_schema_version(),
        Some(SQLITE_SCHEMA_VERSION)
    );

    let connection = Connection::open(synthetic.path()).expect("database must remain readable");
    let actual: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("schema version must remain queryable");
    let journal_mode: String = connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .expect("journal mode must remain queryable");
    let migration_table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'table' AND name = 'radishmemory_schema_migrations'",
            [],
            |row| row.get(0),
        )
        .expect("schema must remain queryable");
    assert_eq!(actual, future_version);
    assert_eq!(journal_mode, "wal");
    assert_eq!(migration_table_count, 0);
}

#[test]
fn unversioned_foreign_schema_is_not_claimed_or_modified() {
    let synthetic = SyntheticDatabase::new("foreign-schema");
    let connection = Connection::open(synthetic.path()).expect("synthetic database must open");
    connection
        .execute_batch("CREATE TABLE synthetic_foreign_data (value TEXT NOT NULL) STRICT;")
        .expect("foreign schema must be created");
    drop(connection);

    let error =
        SqliteDatabase::open(synthetic.path()).expect_err("foreign schema must fail closed");
    assert_eq!(error.code(), SqliteErrorCode::SchemaDrift);

    let connection = Connection::open(synthetic.path()).expect("database must remain readable");
    let foreign_table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'table' AND name = 'synthetic_foreign_data'",
            [],
            |row| row.get(0),
        )
        .expect("schema must remain queryable");
    let migration_table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'table' AND name = 'radishmemory_schema_migrations'",
            [],
            |row| row.get(0),
        )
        .expect("schema must remain queryable");
    assert_eq!(foreign_table_count, 1);
    assert_eq!(migration_table_count, 0);
}

#[test]
fn current_version_without_history_is_reported_as_schema_drift() {
    let synthetic = SyntheticDatabase::new("missing-history");
    let connection = Connection::open(synthetic.path()).expect("synthetic database must open");
    connection
        .pragma_update(None, "user_version", SQLITE_SCHEMA_VERSION)
        .expect("schema marker must be writable");
    drop(connection);

    let error = SqliteDatabase::open(synthetic.path())
        .expect_err("missing migration history must fail closed");
    assert_eq!(error.code(), SqliteErrorCode::SchemaDrift);
}

#[test]
fn altered_migration_history_is_reported_as_schema_drift() {
    let synthetic = SyntheticDatabase::new("altered-history");
    let database = SqliteDatabase::open(synthetic.path()).expect("new database must initialize");
    drop(database);

    let connection = Connection::open(synthetic.path()).expect("synthetic database must open");
    let changed = connection
        .execute(
            "UPDATE radishmemory_schema_migrations
             SET canonical_schema_version = 'synthetic-future-schema'",
            [],
        )
        .expect("synthetic migration history must be writable");
    assert_eq!(
        changed,
        usize::try_from(SQLITE_SCHEMA_VERSION).expect("schema version must fit usize")
    );
    drop(connection);

    let error = SqliteDatabase::open(synthetic.path())
        .expect_err("altered migration history must fail closed");
    assert_eq!(error.code(), SqliteErrorCode::SchemaDrift);
}

#[test]
fn public_error_rendering_does_not_echo_database_path() {
    let private_marker = "synthetic-private-database-path";
    let missing_parent = std::env::temp_dir()
        .join(private_marker)
        .join("nested")
        .join("database.sqlite3");

    let error = SqliteDatabase::open(&missing_parent)
        .expect_err("missing parent must reject database open");
    assert_eq!(error.code(), SqliteErrorCode::Open);
    assert!(!error.to_string().contains(private_marker));
    assert!(!format!("{error:?}").contains(private_marker));
}
