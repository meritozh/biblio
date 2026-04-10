-- Categories table
CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
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

-- Generic default prompt (no category)
INSERT INTO prompts (name, content, category, is_default) VALUES (
    'Default Metadata Extraction',
    'You are a file metadata extraction assistant. Given a file name, existing metadata, and optionally some file content, extract structured metadata. Return a JSON object with these fields:
- display_name: a clean, human-readable title for the file
- category: the most appropriate category (e.g. Novels, Comics, Documents, Academic, Music, Video, Other)
- authors: a list of author names found
- tags: a list of relevant tags/keywords
- description: a brief description of the file content
Only fill in fields you can determine from the provided information. Use null for fields you cannot determine.',
    NULL,
    1
);

-- Novel-specific prompt
INSERT INTO prompts (name, content, category, is_default) VALUES (
    'Novel Metadata Extraction',
    'You are a novel metadata extraction assistant. Given a file name, existing metadata, and optionally some file content (first pages of a book), extract structured metadata about this novel. Return a JSON object with these fields:
- display_name: the clean, full title of the novel
- authors: list of author names
- tags: relevant genre/theme tags (e.g. Fantasy, Romance, Sci-Fi)
- description: a brief plot summary or description of the content
- isbn: ISBN number if found in the text
- publisher: publisher name if found
- year: year of publication
- language: the language the novel is written in (e.g. English, Chinese, Japanese)
- series: series name if part of a series
Only fill in fields you can determine from the provided information. Use null for fields you cannot determine.',
    'Novels',
    0
);

-- Comic-specific prompt
INSERT INTO prompts (name, content, category, is_default) VALUES (
    'Comic Metadata Extraction',
    'You are a comic/manga metadata extraction assistant. Given a file name and existing metadata, extract structured metadata about this comic. Return a JSON object with these fields:
- display_name: the clean, full title of the comic
- authors: list of author/artist/mangaka names
- tags: relevant genre/theme tags (e.g. Action, Shounen, Slice of Life)
- description: a brief description of the comic
- volume: volume number if applicable
- series: series/franchise name
- issue_number: issue or chapter number
Only fill in fields you can determine from the provided information. Use null for fields you cannot determine.',
    'Comics',
    0
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
