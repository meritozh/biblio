-- Categories table
CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    icon TEXT,
    is_default BOOLEAN DEFAULT 0,
    folder_name TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Files table
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    file_status TEXT DEFAULT 'available' CHECK (file_status IN ('available', 'missing', 'moved')),
    in_storage BOOLEAN DEFAULT 0,
    original_path TEXT,
    progress TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- App settings table
CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Default storage path setting
INSERT INTO app_settings (key, value) VALUES ('storage_path', '');

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

-- Authors table
CREATE TABLE IF NOT EXISTS authors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- File-Author junction table
CREATE TABLE IF NOT EXISTS file_authors (
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    author_id INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
    PRIMARY KEY (file_id, author_id)
);

-- Covers table (stores cover images as BLOB)
CREATE TABLE IF NOT EXISTS covers (
    file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    data BLOB NOT NULL,
    mime_type TEXT DEFAULT 'image/png',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Prompts table (LLM prompt management)
CREATE TABLE IF NOT EXISTS prompts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    category TEXT,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Filename-extraction default prompt
INSERT INTO prompts (name, content, category, is_default) VALUES (
    'Filename Extraction',
    'Extract metadata from this filename only. Rules:
- display_name: the clean title (remove site prefixes like [sxsy.org], brackets, file extension)
- authors: if filename has "作者：xxx" or "xxx - title" pattern, extract the author
- progress: combine chapter range + status, e.g. "第1-45章 未完结", "完结", "连载中"
- Use null for unknown fields',
    'filename',
    1
);

-- Content-analysis default prompt
INSERT INTO prompts (name, content, category, is_default) VALUES (
    'Content Analysis',
    '- category: return ONLY the category name. If a category is shown as "name (description)", the parenthesized text is just a hint — return "name" without the parentheses or description. Example: for "h-novel (novel with sexual content)", return "h-novel".
- tags: at most 6 total; prefer existing tags; only propose new ones when you''re confident they clearly apply — be confident, not eager
- description: 1-2 sentence plot summary based on content
- Use null for unknown fields',
    'content',
    1
);

-- Performance indexes
CREATE INDEX IF NOT EXISTS idx_files_category ON files(category_id);
CREATE INDEX IF NOT EXISTS idx_files_status ON files(file_status);
CREATE INDEX IF NOT EXISTS idx_files_name ON files(display_name);
CREATE INDEX IF NOT EXISTS idx_file_tags_tag ON file_tags(tag_id);
CREATE INDEX IF NOT EXISTS idx_metadata_file ON metadata(file_id);
CREATE INDEX IF NOT EXISTS idx_metadata_key ON metadata(key);
CREATE INDEX IF NOT EXISTS idx_file_authors_author ON file_authors(author_id);

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
