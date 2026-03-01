-- Categories table
CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    icon TEXT,
    is_default BOOLEAN DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Files table
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    file_status TEXT DEFAULT 'available' CHECK (file_status IN ('available', 'missing', 'moved')),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Tags table
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    color TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- File-Tag junction table
CREATE TABLE IF NOT EXISTS file_tags (
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (file_id, tag_id)
);

-- Metadata table
CREATE TABLE IF NOT EXISTS metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    data_type TEXT DEFAULT 'text' CHECK (data_type IN ('text', 'number', 'date', 'boolean')),
    UNIQUE(file_id, key)
);

-- Performance indexes
CREATE INDEX IF NOT EXISTS idx_files_category ON files(category_id);
CREATE INDEX IF NOT EXISTS idx_files_status ON files(file_status);
CREATE INDEX IF NOT EXISTS idx_files_name ON files(display_name);
CREATE INDEX IF NOT EXISTS idx_file_tags_tag ON file_tags(tag_id);
CREATE INDEX IF NOT EXISTS idx_metadata_file ON metadata(file_id);
CREATE INDEX IF NOT EXISTS idx_metadata_key ON metadata(key);

-- Full-text search virtual table
CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
    display_name,
    path,
    content='files',
    content_rowid='id'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER IF NOT EXISTS files_ai AFTER INSERT ON files BEGIN
    INSERT INTO files_fts(rowid, display_name, path) VALUES (NEW.id, NEW.display_name, NEW.path);
END;

CREATE TRIGGER IF NOT EXISTS files_ad AFTER DELETE ON files BEGIN
    INSERT INTO files_fts(files_fts, rowid, display_name, path) VALUES ('delete', OLD.id, OLD.display_name, OLD.path);
END;

CREATE TRIGGER IF NOT EXISTS files_au AFTER UPDATE ON files BEGIN
    INSERT INTO files_fts(files_fts, rowid, display_name, path) VALUES ('delete', OLD.id, OLD.display_name, OLD.path);
    INSERT INTO files_fts(rowid, display_name, path) VALUES (NEW.id, NEW.display_name, NEW.path);
END;