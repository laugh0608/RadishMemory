-- Retain the real pre-migration source-body result; absence alone is not evidence.
CREATE TABLE radishmemory_legacy_body_retirements (
    source_id TEXT PRIMARY KEY NOT NULL,
    delete_request_id TEXT NOT NULL,
    attempt_ordinal INTEGER NOT NULL CHECK (attempt_ordinal > 0),
    FOREIGN KEY (source_id) REFERENCES radishmemory_source_artifacts(source_id) ON DELETE RESTRICT,
    FOREIGN KEY (delete_request_id) REFERENCES radishmemory_source_vault_delete_plans(delete_request_id) ON DELETE RESTRICT,
    FOREIGN KEY (delete_request_id, attempt_ordinal)
        REFERENCES radishmemory_deletion_execution_attempts(delete_request_id, attempt_ordinal) ON DELETE RESTRICT
) STRICT;
