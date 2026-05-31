CREATE TABLE IF NOT EXISTS relationships (
    id                 TEXT PRIMARY KEY,
    source_entity_id   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    target_entity_id   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relationship_type  TEXT NOT NULL,
    weight             REAL NOT NULL DEFAULT 1.0,
    document_id        TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    created_at         INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_rel_source ON relationships(source_entity_id);
CREATE INDEX IF NOT EXISTS idx_rel_target ON relationships(target_entity_id);
CREATE INDEX IF NOT EXISTS idx_rel_type ON relationships(relationship_type);
CREATE INDEX IF NOT EXISTS idx_rel_document ON relationships(document_id);
