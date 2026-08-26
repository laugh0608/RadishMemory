CREATE TABLE radishmemory_schema_migrations (
    version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0),
    migration_name TEXT NOT NULL UNIQUE CHECK (length(migration_name) > 0),
    canonical_schema_version TEXT NOT NULL CHECK (length(canonical_schema_version) > 0)
) STRICT;
