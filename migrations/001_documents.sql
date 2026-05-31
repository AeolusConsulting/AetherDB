CREATE TABLE IF NOT EXISTS documents (
    id           TEXT PRIMARY KEY,
    content      TEXT NOT NULL,
    metadata     TEXT NOT NULL DEFAULT '{}',
    content_hash BLOB NOT NULL,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_documents_created_at ON documents(created_at);
CREATE INDEX IF NOT EXISTS idx_documents_content_hash ON documents(content_hash);
