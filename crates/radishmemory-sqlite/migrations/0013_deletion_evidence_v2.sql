-- The migration coordinator verifies the copy before replacing the old table.
CREATE TABLE radishmemory_deletion_evidence_v2 (
    deletion_evidence_id TEXT PRIMARY KEY NOT NULL CHECK (length(deletion_evidence_id) > 0),
    canonical_schema_version TEXT NOT NULL CHECK (canonical_schema_version = 'radishmemory.m0/1'),
    object_type TEXT NOT NULL CHECK (object_type = 'DeletionEvidence'),
    delete_request_id TEXT NOT NULL,
    execution_ordinal INTEGER NOT NULL CHECK (execution_ordinal > 0),
    previous_evidence_id TEXT,
    namespace_id TEXT NOT NULL CHECK (length(namespace_id) > 0),
    scope TEXT NOT NULL CHECK (scope = 'local_device'),
    device_id TEXT NOT NULL CHECK (length(device_id) > 0),
    overall_status TEXT NOT NULL CHECK (overall_status IN ('pending', 'partial', 'failed', 'completed')),
    started_at TEXT NOT NULL CHECK (length(started_at) > 0),
    finished_at TEXT,
    verified_by_type TEXT NOT NULL CHECK (verified_by_type IN ('rule', 'parser', 'test_fixture', 'system')),
    verified_by_id TEXT NOT NULL CHECK (length(verified_by_id) > 0),
    verified_by_version TEXT NOT NULL CHECK (length(verified_by_version) > 0),
    evidence_digest_algorithm TEXT NOT NULL CHECK (evidence_digest_algorithm = 'sha256'),
    evidence_digest_profile TEXT NOT NULL CHECK (evidence_digest_profile IN ('deletion-evidence-v1', 'deletion-evidence-v2')),
    evidence_digest_value TEXT NOT NULL CHECK (length(evidence_digest_value) = 64),
    UNIQUE (delete_request_id, deletion_evidence_id),
    UNIQUE (delete_request_id, execution_ordinal),
    UNIQUE (delete_request_id, previous_evidence_id),
    CHECK ((overall_status = 'pending') = (finished_at IS NULL)),
    FOREIGN KEY (delete_request_id)
        REFERENCES radishmemory_delete_requests(delete_request_id) ON DELETE RESTRICT,
    FOREIGN KEY (delete_request_id, execution_ordinal)
        REFERENCES radishmemory_deletion_execution_attempts(delete_request_id, attempt_ordinal)
        ON DELETE RESTRICT,
    FOREIGN KEY (delete_request_id, previous_evidence_id)
        REFERENCES radishmemory_deletion_evidence_v2(delete_request_id, deletion_evidence_id)
        ON DELETE RESTRICT
) STRICT;
