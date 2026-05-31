ALTER TABLE documents ADD COLUMN hlc_wall_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE documents ADD COLUMN hlc_counter INTEGER NOT NULL DEFAULT 0;
ALTER TABLE documents ADD COLUMN origin_node TEXT NOT NULL DEFAULT '';
ALTER TABLE documents ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_documents_hlc ON documents(hlc_wall_ms, hlc_counter);
CREATE INDEX IF NOT EXISTS idx_documents_deleted ON documents(deleted);
