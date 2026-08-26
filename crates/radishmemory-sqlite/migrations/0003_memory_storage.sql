CREATE TABLE radishmemory_memory_proposals (
    proposal_id TEXT PRIMARY KEY NOT NULL CHECK (length(proposal_id) > 0),
    canonical_schema_version TEXT NOT NULL CHECK (canonical_schema_version = 'radishmemory.m0/1'),
    object_type TEXT NOT NULL CHECK (object_type = 'MemoryProposal'),
    namespace_id TEXT NOT NULL CHECK (length(namespace_id) > 0),
    operation TEXT NOT NULL CHECK (operation IN ('create', 'supersede')),
    memory_type TEXT NOT NULL CHECK (memory_type IN ('observation', 'claim', 'episode', 'preference', 'procedure')),
    subject_ref TEXT NOT NULL CHECK (length(subject_ref) > 0),
    content_kind TEXT NOT NULL CHECK (content_kind = 'text'),
    content_text TEXT NOT NULL CHECK (length(CAST(content_text AS BLOB)) > 0),
    content_digest_algorithm TEXT NOT NULL CHECK (content_digest_algorithm = 'sha256'),
    content_digest_profile TEXT NOT NULL CHECK (content_digest_profile = 'utf8-nfc-text-v1'),
    content_digest_value TEXT NOT NULL CHECK (length(content_digest_value) = 64),
    observed_at TEXT NOT NULL CHECK (length(observed_at) > 0),
    valid_time_mode TEXT NOT NULL CHECK (valid_time_mode IN ('unknown', 'instant', 'interval', 'open_ended')),
    valid_time_start_at TEXT,
    valid_time_end_at TEXT,
    valid_time_precision TEXT NOT NULL CHECK (valid_time_precision IN ('exact', 'day', 'month', 'year', 'unknown')),
    confidence REAL NOT NULL CHECK (typeof(confidence) = 'real' AND confidence >= 0.0 AND confidence <= 1.0),
    importance REAL NOT NULL CHECK (typeof(importance) = 'real' AND importance >= 0.0 AND importance <= 1.0),
    sensitivity TEXT NOT NULL CHECK (sensitivity IN ('personal', 'sensitive', 'restricted')),
    egress_policy TEXT NOT NULL CHECK (egress_policy = 'local_only'),
    retention_mode TEXT NOT NULL CHECK (retention_mode IN ('until_deleted', 'until_time', 'policy')),
    retention_expires_at TEXT,
    retention_policy_id TEXT,
    deletion_state TEXT NOT NULL CHECK (deletion_state IN ('active', 'pending', 'failed', 'deleted')),
    policy_basis TEXT NOT NULL CHECK (length(policy_basis) > 0),
    producer_type TEXT NOT NULL CHECK (producer_type IN ('rule', 'parser', 'test_fixture', 'system')),
    producer_id TEXT NOT NULL CHECK (length(producer_id) > 0),
    producer_version TEXT NOT NULL CHECK (length(producer_version) > 0),
    reason_code TEXT NOT NULL CHECK (length(reason_code) > 0),
    proposed_at TEXT NOT NULL CHECK (length(proposed_at) > 0),
    CHECK (
        (valid_time_mode = 'unknown' AND valid_time_start_at IS NULL AND valid_time_end_at IS NULL) OR
        (valid_time_mode IN ('instant', 'open_ended') AND valid_time_start_at IS NOT NULL AND valid_time_end_at IS NULL) OR
        (valid_time_mode = 'interval' AND valid_time_start_at IS NOT NULL AND valid_time_end_at IS NOT NULL)
    ),
    CHECK (
        (retention_mode = 'until_deleted' AND retention_expires_at IS NULL AND retention_policy_id IS NULL) OR
        (retention_mode = 'until_time' AND retention_expires_at IS NOT NULL AND retention_policy_id IS NULL) OR
        (retention_mode = 'policy' AND retention_expires_at IS NULL AND retention_policy_id IS NOT NULL)
    )
) STRICT;

CREATE TABLE radishmemory_proposal_source_fragments (
    proposal_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    fragment_id TEXT NOT NULL CHECK (length(fragment_id) > 0),
    PRIMARY KEY (proposal_id, ordinal),
    UNIQUE (proposal_id, fragment_id),
    FOREIGN KEY (proposal_id) REFERENCES radishmemory_memory_proposals(proposal_id) ON DELETE RESTRICT,
    FOREIGN KEY (fragment_id) REFERENCES radishmemory_source_fragments(fragment_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_memory_decisions (
    decision_id TEXT PRIMARY KEY NOT NULL CHECK (length(decision_id) > 0),
    canonical_schema_version TEXT NOT NULL CHECK (canonical_schema_version = 'radishmemory.m0/1'),
    object_type TEXT NOT NULL CHECK (object_type = 'MemoryDecision'),
    namespace_id TEXT NOT NULL CHECK (length(namespace_id) > 0),
    proposal_id TEXT NOT NULL CHECK (length(proposal_id) > 0),
    previous_decision_id TEXT UNIQUE,
    decision TEXT NOT NULL CHECK (decision IN ('accept', 'reject', 'defer')),
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'device', 'rule', 'parser', 'test_fixture', 'system')),
    actor_id TEXT NOT NULL CHECK (length(actor_id) > 0),
    actor_version TEXT CHECK (actor_version IS NULL OR length(actor_version) > 0),
    authorization_basis TEXT NOT NULL CHECK (length(authorization_basis) > 0),
    reason_code TEXT NOT NULL CHECK (length(reason_code) > 0),
    reason_text TEXT CHECK (reason_text IS NULL OR length(reason_text) > 0),
    result_memory_id TEXT UNIQUE,
    decided_at TEXT NOT NULL CHECK (length(decided_at) > 0),
    UNIQUE (proposal_id, decision_id),
    CHECK ((decision = 'accept') = (result_memory_id IS NOT NULL)),
    FOREIGN KEY (proposal_id) REFERENCES radishmemory_memory_proposals(proposal_id) ON DELETE RESTRICT,
    FOREIGN KEY (proposal_id, previous_decision_id)
        REFERENCES radishmemory_memory_decisions(proposal_id, decision_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_memory_records (
    memory_id TEXT PRIMARY KEY NOT NULL CHECK (length(memory_id) > 0),
    canonical_schema_version TEXT NOT NULL CHECK (canonical_schema_version = 'radishmemory.m0/1'),
    object_type TEXT NOT NULL CHECK (object_type = 'MemoryRecord'),
    lineage_id TEXT NOT NULL CHECK (length(lineage_id) > 0),
    version INTEGER NOT NULL CHECK (version > 0),
    namespace_id TEXT NOT NULL CHECK (length(namespace_id) > 0),
    memory_type TEXT NOT NULL CHECK (memory_type IN ('observation', 'claim', 'episode', 'preference', 'procedure')),
    subject_ref TEXT NOT NULL CHECK (length(subject_ref) > 0),
    content_kind TEXT NOT NULL CHECK (content_kind = 'text'),
    content_text TEXT NOT NULL CHECK (length(CAST(content_text AS BLOB)) > 0),
    content_digest_algorithm TEXT NOT NULL CHECK (content_digest_algorithm = 'sha256'),
    content_digest_profile TEXT NOT NULL CHECK (content_digest_profile = 'utf8-nfc-text-v1'),
    content_digest_value TEXT NOT NULL CHECK (length(content_digest_value) = 64),
    origin_proposal_id TEXT NOT NULL CHECK (length(origin_proposal_id) > 0),
    accepted_by_decision_id TEXT NOT NULL UNIQUE CHECK (length(accepted_by_decision_id) > 0),
    observed_at TEXT NOT NULL CHECK (length(observed_at) > 0),
    valid_time_mode TEXT NOT NULL CHECK (valid_time_mode IN ('unknown', 'instant', 'interval', 'open_ended')),
    valid_time_start_at TEXT,
    valid_time_end_at TEXT,
    valid_time_precision TEXT NOT NULL CHECK (valid_time_precision IN ('exact', 'day', 'month', 'year', 'unknown')),
    confidence REAL NOT NULL CHECK (typeof(confidence) = 'real' AND confidence >= 0.0 AND confidence <= 1.0),
    importance REAL NOT NULL CHECK (typeof(importance) = 'real' AND importance >= 0.0 AND importance <= 1.0),
    sensitivity TEXT NOT NULL CHECK (sensitivity IN ('personal', 'sensitive', 'restricted')),
    egress_policy TEXT NOT NULL CHECK (egress_policy = 'local_only'),
    retention_mode TEXT NOT NULL CHECK (retention_mode IN ('until_deleted', 'until_time', 'policy')),
    retention_expires_at TEXT,
    retention_policy_id TEXT,
    deletion_state TEXT NOT NULL CHECK (deletion_state IN ('active', 'pending', 'failed', 'deleted')),
    policy_basis TEXT NOT NULL CHECK (length(policy_basis) > 0),
    created_at TEXT NOT NULL CHECK (length(created_at) > 0),
    UNIQUE (namespace_id, lineage_id, version),
    CHECK (
        (valid_time_mode = 'unknown' AND valid_time_start_at IS NULL AND valid_time_end_at IS NULL) OR
        (valid_time_mode IN ('instant', 'open_ended') AND valid_time_start_at IS NOT NULL AND valid_time_end_at IS NULL) OR
        (valid_time_mode = 'interval' AND valid_time_start_at IS NOT NULL AND valid_time_end_at IS NOT NULL)
    ),
    CHECK (
        (retention_mode = 'until_deleted' AND retention_expires_at IS NULL AND retention_policy_id IS NULL) OR
        (retention_mode = 'until_time' AND retention_expires_at IS NOT NULL AND retention_policy_id IS NULL) OR
        (retention_mode = 'policy' AND retention_expires_at IS NULL AND retention_policy_id IS NOT NULL)
    ),
    FOREIGN KEY (origin_proposal_id) REFERENCES radishmemory_memory_proposals(proposal_id) ON DELETE RESTRICT,
    FOREIGN KEY (accepted_by_decision_id) REFERENCES radishmemory_memory_decisions(decision_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_proposal_targets (
    proposal_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    target_memory_id TEXT NOT NULL CHECK (length(target_memory_id) > 0),
    PRIMARY KEY (proposal_id, ordinal),
    UNIQUE (proposal_id, target_memory_id),
    FOREIGN KEY (proposal_id) REFERENCES radishmemory_memory_proposals(proposal_id) ON DELETE RESTRICT,
    FOREIGN KEY (target_memory_id) REFERENCES radishmemory_memory_records(memory_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_record_source_fragments (
    memory_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    fragment_id TEXT NOT NULL CHECK (length(fragment_id) > 0),
    PRIMARY KEY (memory_id, ordinal),
    UNIQUE (memory_id, fragment_id),
    FOREIGN KEY (memory_id) REFERENCES radishmemory_memory_records(memory_id) ON DELETE RESTRICT,
    FOREIGN KEY (fragment_id) REFERENCES radishmemory_source_fragments(fragment_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_record_supersedes (
    memory_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    superseded_memory_id TEXT NOT NULL CHECK (length(superseded_memory_id) > 0),
    PRIMARY KEY (memory_id, ordinal),
    UNIQUE (memory_id, superseded_memory_id),
    CHECK (memory_id <> superseded_memory_id),
    FOREIGN KEY (memory_id) REFERENCES radishmemory_memory_records(memory_id) ON DELETE RESTRICT,
    FOREIGN KEY (superseded_memory_id) REFERENCES radishmemory_memory_records(memory_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_record_contradicts (
    memory_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    contradicted_memory_id TEXT NOT NULL CHECK (length(contradicted_memory_id) > 0),
    PRIMARY KEY (memory_id, ordinal),
    UNIQUE (memory_id, contradicted_memory_id),
    CHECK (memory_id <> contradicted_memory_id),
    FOREIGN KEY (memory_id) REFERENCES radishmemory_memory_records(memory_id) ON DELETE RESTRICT,
    FOREIGN KEY (contradicted_memory_id) REFERENCES radishmemory_memory_records(memory_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_memory_state_events (
    event_id TEXT PRIMARY KEY NOT NULL CHECK (length(event_id) > 0),
    canonical_schema_version TEXT NOT NULL CHECK (canonical_schema_version = 'radishmemory.m0/1'),
    object_type TEXT NOT NULL CHECK (object_type = 'MemoryStateEvent'),
    namespace_id TEXT NOT NULL CHECK (length(namespace_id) > 0),
    memory_id TEXT NOT NULL CHECK (length(memory_id) > 0),
    previous_event_id TEXT UNIQUE,
    event_type TEXT NOT NULL CHECK (event_type IN ('confirmed', 'superseded', 'contradicted', 'retracted', 'expired')),
    from_state TEXT CHECK (from_state IS NULL OR from_state = 'confirmed'),
    cause_type TEXT NOT NULL CHECK (cause_type IN ('memory_decision', 'memory_record', 'delete_request', 'policy_basis')),
    cause_id TEXT NOT NULL CHECK (length(cause_id) > 0),
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'device', 'rule', 'parser', 'test_fixture', 'system')),
    actor_id TEXT NOT NULL CHECK (length(actor_id) > 0),
    actor_version TEXT CHECK (actor_version IS NULL OR length(actor_version) > 0),
    reason_code TEXT NOT NULL CHECK (length(reason_code) > 0),
    effective_at TEXT,
    occurred_at TEXT NOT NULL CHECK (length(occurred_at) > 0),
    UNIQUE (memory_id, event_id),
    CHECK (
        (event_type = 'confirmed' AND previous_event_id IS NULL AND from_state IS NULL AND effective_at IS NULL AND cause_type = 'memory_decision') OR
        (event_type <> 'confirmed' AND previous_event_id IS NOT NULL AND from_state = 'confirmed' AND effective_at IS NOT NULL)
    ),
    FOREIGN KEY (memory_id) REFERENCES radishmemory_memory_records(memory_id) ON DELETE RESTRICT,
    FOREIGN KEY (memory_id, previous_event_id)
        REFERENCES radishmemory_memory_state_events(memory_id, event_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_event_related_memories (
    event_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    related_memory_id TEXT NOT NULL CHECK (length(related_memory_id) > 0),
    PRIMARY KEY (event_id, ordinal),
    UNIQUE (event_id, related_memory_id),
    FOREIGN KEY (event_id) REFERENCES radishmemory_memory_state_events(event_id) ON DELETE RESTRICT,
    FOREIGN KEY (related_memory_id) REFERENCES radishmemory_memory_records(memory_id) ON DELETE RESTRICT
) STRICT;
