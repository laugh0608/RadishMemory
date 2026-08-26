CREATE TABLE radishmemory_memory_current_projection (
    memory_id TEXT PRIMARY KEY NOT NULL CHECK (length(memory_id) > 0),
    namespace_id TEXT NOT NULL CHECK (length(namespace_id) > 0),
    current_state TEXT NOT NULL CHECK (
        current_state IN ('confirmed', 'superseded', 'contradicted', 'retracted', 'expired')
    ),
    last_state_event_id TEXT NOT NULL UNIQUE CHECK (length(last_state_event_id) > 0),
    FOREIGN KEY (memory_id) REFERENCES radishmemory_memory_records(memory_id) ON DELETE CASCADE,
    FOREIGN KEY (memory_id, last_state_event_id)
        REFERENCES radishmemory_memory_state_events(memory_id, event_id) ON DELETE CASCADE
) STRICT;

CREATE VIRTUAL TABLE radishmemory_recall_fts USING fts5(
    object_kind UNINDEXED,
    object_id UNINDEXED,
    namespace_id UNINDEXED,
    sensitivity UNINDEXED,
    content,
    tokenize = 'unicode61'
);
