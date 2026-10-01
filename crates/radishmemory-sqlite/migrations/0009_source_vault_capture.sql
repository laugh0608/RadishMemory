-- Evolve the same attempt/reference boundary: capture preparation precedes the
-- canonical source, whereas migration inventory always refers to an old source.
CREATE TABLE radishmemory_source_vault_attempts_v9 (
    source_id TEXT PRIMARY KEY NOT NULL,
    namespace_id TEXT NOT NULL,
    digest_value TEXT NOT NULL CHECK (length(digest_value) = 64),
    content_length INTEGER NOT NULL CHECK (content_length > 0),
    media_type TEXT NOT NULL CHECK (media_type IN ('text/plain', 'text/markdown')),
    state TEXT NOT NULL CHECK (state IN ('retired', 'prepared', 'committed')),
    locator TEXT UNIQUE NOT NULL CHECK (length(locator) = 64 AND locator NOT GLOB '*[^0-9a-f]*'),
    attempt_id TEXT UNIQUE NOT NULL CHECK (length(attempt_id) = 64 AND attempt_id NOT GLOB '*[^0-9a-f]*'),
    kind TEXT NOT NULL CHECK (kind IN ('migration', 'capture')),
    request_digest TEXT CHECK (length(request_digest) = 64 AND request_digest NOT GLOB '*[^0-9a-f]*'),
    origin_binding_id TEXT,
    CHECK ((kind = 'migration' AND state = 'retired' AND request_digest IS NULL AND origin_binding_id IS NULL) OR
           (kind = 'capture' AND state IN ('prepared', 'committed') AND request_digest IS NOT NULL AND origin_binding_id IS NOT NULL)),
    UNIQUE (source_id, locator, attempt_id)
) STRICT;

CREATE TABLE radishmemory_source_vault_references_v9 (
    source_id TEXT PRIMARY KEY NOT NULL,
    locator TEXT UNIQUE NOT NULL,
    attempt_id TEXT UNIQUE NOT NULL,
    FOREIGN KEY (source_id) REFERENCES radishmemory_source_artifacts(source_id) ON DELETE RESTRICT,
    FOREIGN KEY (source_id, locator, attempt_id)
        REFERENCES radishmemory_source_vault_attempts_v9(source_id, locator, attempt_id) ON DELETE RESTRICT
) STRICT;

INSERT INTO radishmemory_source_vault_attempts_v9
    SELECT source_id, namespace_id, digest_value, content_length, media_type, state,
           locator, attempt_id, 'migration', NULL, NULL
    FROM radishmemory_source_vault_attempts;
INSERT INTO radishmemory_source_vault_references_v9
    SELECT source_id, locator, attempt_id FROM radishmemory_source_vault_references;
DROP TABLE radishmemory_source_vault_references;
DROP TABLE radishmemory_source_vault_attempts;
ALTER TABLE radishmemory_source_vault_attempts_v9 RENAME TO radishmemory_source_vault_attempts;
ALTER TABLE radishmemory_source_vault_references_v9 RENAME TO radishmemory_source_vault_references;

-- One unresolved capture per library. Recovery must finish that precise request
-- before unrelated writes; no background cancellation or broad orphan cleanup.
CREATE UNIQUE INDEX radishmemory_source_vault_one_pending_capture
    ON radishmemory_source_vault_attempts ((1)) WHERE state = 'prepared';
