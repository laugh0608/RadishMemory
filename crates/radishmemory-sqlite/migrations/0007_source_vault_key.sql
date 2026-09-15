-- Maintenance-only checkpoint. Inline bodies remain until the later object migration.
CREATE TABLE radishmemory_source_vault_key_profile (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    namespace_id TEXT NOT NULL CHECK (length(namespace_id) > 0),
    device_id TEXT NOT NULL CHECK (length(device_id) > 0),
    provider_profile TEXT NOT NULL CHECK (length(provider_profile) > 0),
    state TEXT NOT NULL CHECK (state = 'key_ready')
) STRICT;
