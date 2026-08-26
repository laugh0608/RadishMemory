CREATE TABLE radishmemory_delete_requests (
    delete_request_id TEXT PRIMARY KEY NOT NULL CHECK (length(delete_request_id) > 0),
    canonical_schema_version TEXT NOT NULL CHECK (canonical_schema_version = 'radishmemory.m0/1'),
    object_type TEXT NOT NULL CHECK (object_type = 'DeleteRequest'),
    namespace_id TEXT NOT NULL CHECK (length(namespace_id) > 0),
    requested_by_type TEXT NOT NULL CHECK (
        requested_by_type IN ('user', 'device', 'rule', 'parser', 'test_fixture', 'system')
    ),
    requested_by_id TEXT NOT NULL CHECK (length(requested_by_id) > 0),
    requested_by_version TEXT CHECK (requested_by_version IS NULL OR length(requested_by_version) > 0),
    authorization_basis TEXT NOT NULL CHECK (length(authorization_basis) > 0),
    requested_guarantee TEXT NOT NULL CHECK (requested_guarantee IN ('stop_recall', 'local_purge')),
    scope TEXT NOT NULL CHECK (scope = 'local_device'),
    device_id TEXT NOT NULL CHECK (length(device_id) > 0),
    reason_code TEXT NOT NULL CHECK (length(reason_code) > 0),
    requested_at TEXT NOT NULL CHECK (length(requested_at) > 0)
) STRICT;

CREATE TABLE radishmemory_delete_request_targets (
    delete_request_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    object_type TEXT NOT NULL CHECK (object_type IN ('SourceArtifact', 'MemoryRecord')),
    object_id TEXT NOT NULL CHECK (length(object_id) > 0),
    PRIMARY KEY (delete_request_id, ordinal),
    UNIQUE (delete_request_id, object_type, object_id),
    FOREIGN KEY (delete_request_id)
        REFERENCES radishmemory_delete_requests(delete_request_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_delete_request_components (
    delete_request_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    component_key TEXT NOT NULL CHECK (length(component_key) > 0),
    component_type TEXT NOT NULL CHECK (component_type IN (
        'source_body', 'source_metadata', 'source_fragment', 'memory_proposal',
        'memory_decision', 'memory_record', 'memory_state_event', 'full_text_index',
        'context_cache', 'minimal_audit'
    )),
    target_ref_kind TEXT NOT NULL CHECK (target_ref_kind IN ('object', 'frozen_closure')),
    target_count INTEGER NOT NULL CHECK (target_count > 0),
    required_action TEXT NOT NULL CHECK (required_action IN ('delete', 'redact', 'retain_minimal')),
    target_digest_algorithm TEXT,
    target_digest_profile TEXT,
    target_digest_value TEXT,
    PRIMARY KEY (delete_request_id, component_key),
    UNIQUE (delete_request_id, ordinal),
    CHECK (
        (target_ref_kind = 'object' AND target_count = 1
            AND target_digest_algorithm IS NULL
            AND target_digest_profile IS NULL
            AND target_digest_value IS NULL) OR
        (target_ref_kind = 'frozen_closure'
            AND target_digest_algorithm = 'sha256'
            AND target_digest_profile = 'canonical-json-v1'
            AND length(target_digest_value) = 64)
    ),
    FOREIGN KEY (delete_request_id)
        REFERENCES radishmemory_delete_requests(delete_request_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_delete_component_targets (
    delete_request_id TEXT NOT NULL,
    component_key TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    object_type TEXT NOT NULL CHECK (object_type IN ('SourceArtifact', 'MemoryRecord')),
    object_id TEXT NOT NULL CHECK (length(object_id) > 0),
    PRIMARY KEY (delete_request_id, component_key, ordinal),
    UNIQUE (delete_request_id, component_key, object_type, object_id),
    FOREIGN KEY (delete_request_id, component_key)
        REFERENCES radishmemory_delete_request_components(delete_request_id, component_key)
        ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_delete_execution_closure (
    delete_request_id TEXT NOT NULL,
    component_type TEXT NOT NULL CHECK (component_type IN (
        'source_body', 'source_metadata', 'source_fragment', 'memory_proposal',
        'memory_decision', 'memory_record', 'memory_state_event', 'full_text_index',
        'context_cache', 'minimal_audit'
    )),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    object_type TEXT NOT NULL CHECK (object_type IN (
        'SourceArtifact', 'SourceFragment', 'MemoryProposal', 'MemoryDecision',
        'MemoryRecord', 'MemoryStateEvent', 'DeleteRequest'
    )),
    object_id TEXT NOT NULL CHECK (length(object_id) > 0),
    PRIMARY KEY (delete_request_id, component_type, ordinal),
    UNIQUE (delete_request_id, component_type, object_type, object_id),
    FOREIGN KEY (delete_request_id)
        REFERENCES radishmemory_delete_requests(delete_request_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_deletion_execution_attempts (
    delete_request_id TEXT NOT NULL,
    attempt_ordinal INTEGER NOT NULL CHECK (attempt_ordinal > 0),
    checked_at TEXT NOT NULL CHECK (length(checked_at) > 0),
    PRIMARY KEY (delete_request_id, attempt_ordinal),
    FOREIGN KEY (delete_request_id)
        REFERENCES radishmemory_delete_requests(delete_request_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_deletion_execution_results (
    delete_request_id TEXT NOT NULL,
    attempt_ordinal INTEGER NOT NULL CHECK (attempt_ordinal > 0),
    component_key TEXT NOT NULL CHECK (length(component_key) > 0),
    processed_count INTEGER NOT NULL CHECK (processed_count >= 0),
    status TEXT NOT NULL CHECK (status IN ('pending', 'succeeded', 'failed')),
    outcome TEXT NOT NULL CHECK (outcome IN (
        'deleted', 'redacted', 'retained_minimal', 'not_found', 'not_applicable'
    )),
    verification_method TEXT NOT NULL CHECK (length(verification_method) > 0),
    checked_at TEXT NOT NULL CHECK (length(checked_at) > 0),
    error_code TEXT,
    retryable INTEGER CHECK (retryable IN (0, 1)),
    retention_basis_type TEXT CHECK (retention_basis_type IS NULL OR retention_basis_type = 'policy_basis'),
    retention_basis_id TEXT CHECK (retention_basis_id IS NULL OR length(retention_basis_id) > 0),
    PRIMARY KEY (delete_request_id, attempt_ordinal, component_key),
    CHECK (
        (status = 'failed' AND error_code IS NOT NULL AND length(error_code) > 0 AND retryable IS NOT NULL) OR
        (status <> 'failed' AND error_code IS NULL AND retryable IS NULL)
    ),
    CHECK (
        (status = 'succeeded' AND outcome = 'retained_minimal'
            AND retention_basis_type = 'policy_basis' AND retention_basis_id IS NOT NULL) OR
        (NOT (status = 'succeeded' AND outcome = 'retained_minimal')
            AND retention_basis_type IS NULL AND retention_basis_id IS NULL)
    ),
    FOREIGN KEY (delete_request_id, attempt_ordinal)
        REFERENCES radishmemory_deletion_execution_attempts(delete_request_id, attempt_ordinal)
        ON DELETE RESTRICT,
    FOREIGN KEY (delete_request_id, component_key)
        REFERENCES radishmemory_delete_request_components(delete_request_id, component_key)
        ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_deletion_evidence (
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
    evidence_digest_profile TEXT NOT NULL CHECK (evidence_digest_profile = 'deletion-evidence-v1'),
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
        REFERENCES radishmemory_deletion_evidence(delete_request_id, deletion_evidence_id)
        ON DELETE RESTRICT
) STRICT;
