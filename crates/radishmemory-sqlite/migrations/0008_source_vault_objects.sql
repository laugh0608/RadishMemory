-- Maintenance-only format. No canonical source, governance or deletion facts change.
CREATE TABLE radishmemory_source_vault_migration (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
    state TEXT NOT NULL CHECK (state IN ('migrating', 'objects_ready'))
) STRICT;

-- A migration attempt owns one existing source version. Tokens are allocated
-- before filesystem writes; a planned row is also the durable migration inventory.
CREATE TABLE radishmemory_source_vault_attempts (
    source_id TEXT PRIMARY KEY NOT NULL,
    namespace_id TEXT NOT NULL,
    digest_value TEXT NOT NULL CHECK (length(digest_value) = 64),
    content_length INTEGER NOT NULL CHECK (content_length > 0),
    media_type TEXT NOT NULL CHECK (media_type IN ('text/plain', 'text/markdown')),
    state TEXT NOT NULL CHECK (state IN ('planned', 'prepared', 'referenced', 'retired')),
    locator TEXT UNIQUE CHECK (length(locator) = 64 AND locator NOT GLOB '*[^0-9a-f]*'),
    attempt_id TEXT UNIQUE CHECK (length(attempt_id) = 64 AND attempt_id NOT GLOB '*[^0-9a-f]*'),
    CHECK ((state = 'planned' AND locator IS NULL AND attempt_id IS NULL) OR
           (state != 'planned' AND locator IS NOT NULL AND attempt_id IS NOT NULL)),
    UNIQUE (source_id, locator, attempt_id),
    FOREIGN KEY (source_id) REFERENCES radishmemory_source_artifacts(source_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_source_vault_references (
    source_id TEXT PRIMARY KEY NOT NULL,
    locator TEXT UNIQUE NOT NULL,
    attempt_id TEXT UNIQUE NOT NULL,
    FOREIGN KEY (source_id, locator, attempt_id)
        REFERENCES radishmemory_source_vault_attempts(source_id, locator, attempt_id) ON DELETE RESTRICT
) STRICT;
