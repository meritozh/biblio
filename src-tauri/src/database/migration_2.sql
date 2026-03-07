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

-- Performance index for file_authors
CREATE INDEX IF NOT EXISTS idx_file_authors_author ON file_authors(author_id);