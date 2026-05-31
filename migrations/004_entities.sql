CREATE TABLE IF NOT EXISTS entities (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    entity_type   TEXT NOT NULL,
    document_id   TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    created_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name);
CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_document ON entities(document_id);
