CREATE TABLE radishmemory_source_lineage_tips (
    namespace_id TEXT NOT NULL CHECK (length(namespace_id) > 0),
    lineage_id TEXT NOT NULL CHECK (length(lineage_id) > 0),
    source_id TEXT NOT NULL UNIQUE CHECK (length(source_id) > 0),
    version INTEGER NOT NULL CHECK (version > 0),
    PRIMARY KEY (namespace_id, lineage_id),
    FOREIGN KEY (source_id) REFERENCES radishmemory_source_artifacts(source_id) ON DELETE CASCADE
) STRICT;

INSERT INTO radishmemory_source_lineage_tips (
    namespace_id, lineage_id, source_id, version
)
SELECT source.namespace_id, source.lineage_id, source.source_id, source.version
FROM radishmemory_source_artifacts AS source
WHERE source.deletion_state = 'active'
  AND NOT EXISTS (
      SELECT 1
      FROM radishmemory_source_artifacts AS newer
      WHERE newer.namespace_id = source.namespace_id
        AND newer.lineage_id = source.lineage_id
        AND newer.version > source.version
  );

CREATE TABLE radishmemory_source_origin_bindings (
    namespace_id TEXT NOT NULL CHECK (length(namespace_id) > 0),
    origin_binding_id TEXT NOT NULL CHECK (
        length(origin_binding_id) > 15
        AND length(origin_binding_id) <= 128
        AND origin_binding_id GLOB 'origin-binding-*'
        AND origin_binding_id NOT GLOB '*[^A-Za-z0-9._-]*'
    ),
    lineage_id TEXT NOT NULL CHECK (length(lineage_id) > 0),
    PRIMARY KEY (namespace_id, origin_binding_id),
    UNIQUE (namespace_id, lineage_id)
) STRICT;

INSERT INTO radishmemory_source_origin_bindings (
    namespace_id, origin_binding_id, lineage_id
)
SELECT tip.namespace_id, source.origin_ref, tip.lineage_id
FROM radishmemory_source_lineage_tips AS tip
JOIN radishmemory_source_artifacts AS source ON source.source_id = tip.source_id
WHERE source.origin_kind = 'explicit_user_input'
  AND source.origin_ref IS NOT NULL;

CREATE TABLE radishmemory_source_capture_audit (
    source_id TEXT PRIMARY KEY NOT NULL CHECK (length(source_id) > 0),
    namespace_id TEXT NOT NULL CHECK (length(namespace_id) > 0),
    origin_binding_id TEXT NOT NULL CHECK (
        length(origin_binding_id) > 15
        AND length(origin_binding_id) <= 128
        AND origin_binding_id GLOB 'origin-binding-*'
        AND origin_binding_id NOT GLOB '*[^A-Za-z0-9._-]*'
    ),
    outcome TEXT NOT NULL CHECK (outcome IN ('created', 'versioned')),
    recorded_at TEXT NOT NULL CHECK (length(recorded_at) > 0),
    FOREIGN KEY (source_id) REFERENCES radishmemory_source_artifacts(source_id) ON DELETE CASCADE,
    FOREIGN KEY (namespace_id, origin_binding_id)
        REFERENCES radishmemory_source_origin_bindings(namespace_id, origin_binding_id)
        ON DELETE CASCADE
) STRICT;
