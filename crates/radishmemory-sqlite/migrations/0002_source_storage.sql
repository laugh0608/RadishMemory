CREATE TABLE radishmemory_source_artifacts (
    source_id TEXT PRIMARY KEY NOT NULL CHECK (length(source_id) > 0),
    canonical_schema_version TEXT NOT NULL CHECK (canonical_schema_version = 'radishmemory.m0/1'),
    object_type TEXT NOT NULL CHECK (object_type = 'SourceArtifact'),
    lineage_id TEXT NOT NULL CHECK (length(lineage_id) > 0),
    version INTEGER NOT NULL CHECK (version > 0),
    namespace_id TEXT NOT NULL CHECK (length(namespace_id) > 0),
    source_kind TEXT NOT NULL CHECK (source_kind IN ('text', 'markdown')),
    media_type TEXT NOT NULL CHECK (media_type IN ('text/plain', 'text/markdown')),
    content_length INTEGER NOT NULL CHECK (content_length > 0),
    content_digest_algorithm TEXT NOT NULL CHECK (content_digest_algorithm = 'sha256'),
    content_digest_profile TEXT NOT NULL CHECK (content_digest_profile = 'exact-bytes-v1'),
    content_digest_value TEXT NOT NULL CHECK (length(content_digest_value) = 64),
    title TEXT CHECK (title IS NULL OR length(title) > 0),
    origin_kind TEXT NOT NULL CHECK (origin_kind IN ('synthetic_fixture', 'explicit_user_input')),
    origin_ref TEXT CHECK (origin_ref IS NULL OR length(origin_ref) > 0),
    observed_at TEXT NOT NULL CHECK (length(observed_at) > 0),
    captured_at TEXT NOT NULL CHECK (length(captured_at) > 0),
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
    created_at TEXT NOT NULL CHECK (length(created_at) > 0),
    UNIQUE (namespace_id, lineage_id, version),
    CHECK (
        (source_kind = 'text' AND media_type = 'text/plain') OR
        (source_kind = 'markdown' AND media_type = 'text/markdown')
    ),
    CHECK (
        (retention_mode = 'until_deleted' AND retention_expires_at IS NULL AND retention_policy_id IS NULL) OR
        (retention_mode = 'until_time' AND retention_expires_at IS NOT NULL AND retention_policy_id IS NULL) OR
        (retention_mode = 'policy' AND retention_expires_at IS NULL AND retention_policy_id IS NOT NULL)
    )
) STRICT;

CREATE TABLE radishmemory_source_bodies (
    source_id TEXT PRIMARY KEY NOT NULL,
    content BLOB NOT NULL CHECK (typeof(content) = 'blob' AND length(content) > 0),
    FOREIGN KEY (source_id) REFERENCES radishmemory_source_artifacts(source_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_source_supersedes (
    source_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    superseded_source_id TEXT NOT NULL CHECK (length(superseded_source_id) > 0),
    PRIMARY KEY (source_id, ordinal),
    UNIQUE (source_id, superseded_source_id),
    CHECK (source_id <> superseded_source_id),
    FOREIGN KEY (source_id) REFERENCES radishmemory_source_artifacts(source_id) ON DELETE RESTRICT,
    FOREIGN KEY (superseded_source_id) REFERENCES radishmemory_source_artifacts(source_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_source_fragments (
    fragment_id TEXT PRIMARY KEY NOT NULL CHECK (length(fragment_id) > 0),
    canonical_schema_version TEXT NOT NULL CHECK (canonical_schema_version = 'radishmemory.m0/1'),
    object_type TEXT NOT NULL CHECK (object_type = 'SourceFragment'),
    namespace_id TEXT NOT NULL CHECK (length(namespace_id) > 0),
    source_id TEXT NOT NULL CHECK (length(source_id) > 0),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    byte_start INTEGER NOT NULL CHECK (byte_start >= 0),
    byte_end INTEGER NOT NULL CHECK (byte_end > byte_start),
    content_digest_algorithm TEXT NOT NULL CHECK (content_digest_algorithm = 'sha256'),
    content_digest_profile TEXT NOT NULL CHECK (content_digest_profile = 'exact-bytes-v1'),
    content_digest_value TEXT NOT NULL CHECK (length(content_digest_value) = 64),
    segmenter_type TEXT NOT NULL CHECK (segmenter_type IN ('rule', 'parser', 'test_fixture', 'system')),
    segmenter_id TEXT NOT NULL CHECK (length(segmenter_id) > 0),
    segmenter_version TEXT NOT NULL CHECK (length(segmenter_version) > 0),
    sensitivity TEXT NOT NULL CHECK (sensitivity IN ('personal', 'sensitive', 'restricted')),
    egress_policy TEXT NOT NULL CHECK (egress_policy = 'local_only'),
    retention_mode TEXT NOT NULL CHECK (retention_mode IN ('until_deleted', 'until_time', 'policy')),
    retention_expires_at TEXT,
    retention_policy_id TEXT,
    deletion_state TEXT NOT NULL CHECK (deletion_state IN ('active', 'pending', 'failed', 'deleted')),
    policy_basis TEXT NOT NULL CHECK (length(policy_basis) > 0),
    created_at TEXT NOT NULL CHECK (length(created_at) > 0),
    UNIQUE (source_id, ordinal),
    CHECK (
        (retention_mode = 'until_deleted' AND retention_expires_at IS NULL AND retention_policy_id IS NULL) OR
        (retention_mode = 'until_time' AND retention_expires_at IS NOT NULL AND retention_policy_id IS NULL) OR
        (retention_mode = 'policy' AND retention_expires_at IS NULL AND retention_policy_id IS NOT NULL)
    ),
    FOREIGN KEY (source_id) REFERENCES radishmemory_source_artifacts(source_id) ON DELETE RESTRICT
) STRICT;

CREATE TABLE radishmemory_fragment_heading_path (
    fragment_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    heading TEXT NOT NULL CHECK (length(heading) > 0),
    PRIMARY KEY (fragment_id, ordinal),
    FOREIGN KEY (fragment_id) REFERENCES radishmemory_source_fragments(fragment_id) ON DELETE RESTRICT
) STRICT;
