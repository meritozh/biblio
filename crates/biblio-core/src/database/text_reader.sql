CREATE TABLE reader_rules (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('builtin', 'custom')),
    name TEXT NOT NULL,
    mode TEXT CHECK (mode IN ('template', 'regex')),
    chapter TEXT,
    volume TEXT,
    CHECK ((kind = 'builtin' AND mode IS NULL AND chapter IS NULL AND volume IS NULL)
        OR (kind = 'custom' AND mode IS NOT NULL AND chapter IS NOT NULL AND length(chapter) > 0))
);

INSERT INTO reader_rules (id, kind, name) VALUES
    ('auto', 'builtin', '自动检测'),
    ('common', 'builtin', '常规中文'),
    ('classic', 'builtin', '回／话'),
    ('english', 'builtin', '英文章节'),
    ('digits', 'builtin', '纯数字'),
    ('number-title', 'builtin', '数字加标题'),
    ('symbols', 'builtin', '特殊符号');

CREATE TABLE reader_preferences (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    external_app TEXT CHECK (external_app IS NULL OR length(trim(external_app)) > 0),
    font_family TEXT,
    size_pt REAL NOT NULL DEFAULT 18 CHECK (size_pt BETWEEN 8 AND 72),
    line_ratio REAL NOT NULL DEFAULT 1.6 CHECK (line_ratio BETWEEN 1 AND 3),
    spacing_pt REAL NOT NULL DEFAULT 0 CHECK (spacing_pt BETWEEN -1 AND 8),
    collapse_blanks INTEGER NOT NULL DEFAULT 1 CHECK (collapse_blanks IN (0, 1)),
    default_rule TEXT REFERENCES reader_rules(id) ON DELETE SET NULL
);

INSERT INTO reader_preferences (id) VALUES (1);

CREATE TABLE reader_books (
    file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    paragraph_byte INTEGER NOT NULL DEFAULT 0 CHECK (paragraph_byte >= 0),
    restart_byte INTEGER NOT NULL DEFAULT 0 CHECK (restart_byte >= paragraph_byte),
    source_stamp TEXT,
    position_codec TEXT,
    encoding TEXT CHECK (encoding IN ('utf8', 'utf16le', 'utf16be', 'gbk', 'gb18030', 'big5')),
    rule_id TEXT REFERENCES reader_rules(id) ON DELETE SET NULL
);

CREATE INDEX reader_books_by_rule ON reader_books(rule_id);
