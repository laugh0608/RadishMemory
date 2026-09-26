-- Physical execution checkpoints subordinate to the existing canonical DeleteRequest.
CREATE TABLE radishmemory_source_vault_delete_plans (
    delete_request_id TEXT PRIMARY KEY NOT NULL,
    plan_digest TEXT NOT NULL CHECK (length(plan_digest) = 64),
    FOREIGN KEY (delete_request_id) REFERENCES radishmemory_delete_requests(delete_request_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_source_vault_deletions (
    source_id TEXT PRIMARY KEY NOT NULL,
    delete_request_id TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('deleting', 'deleted')),
    FOREIGN KEY (source_id) REFERENCES radishmemory_source_vault_attempts(source_id) ON DELETE RESTRICT,
    FOREIGN KEY (delete_request_id) REFERENCES radishmemory_source_vault_delete_plans(delete_request_id) ON DELETE RESTRICT
) STRICT;
