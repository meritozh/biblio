pub mod recovery;

use tauri_plugin_sql::{Migration, MigrationKind};

pub fn get_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "complete schema",
            sql: include_str!("schema.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "remote container marker",
            // NULL = legacy raw upload; 'bbx1' = encrypted container object.
            // The download/re-encrypt paths key off this column, so it is the
            // authority for whether a remote object must be unwrapped.
            sql: "ALTER TABLE files ADD COLUMN remote_container TEXT;",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 3,
            description: "seed galgame filename prompt",
            // The galgame pipeline's filename LLM node resolves the active
            // (galgame, filename) prompt; `prompt_get_active` errors when none
            // exists, which would mark every galgame import as failed. Seed a
            // default so a fresh galgame category works out of the box.
            // INSERT-only: idempotent across existing DBs (no prior galgame
            // rows) and never touches user-edited prompts.
            sql: "INSERT INTO prompts (name, content, category, is_default, mime_group, step, schema_slug) VALUES \
                  ('Galgame Filename Extraction', \
                   'Extract the clean visual-novel / galgame title from this archive or folder name. Rules:' || char(10) || \
                   '- display_name: the game title only. Strip brand/circle tags like [Brand] or （ブランド）, release dates like (2020) or [2020.12], version markers, scanlator/cracker tags, and any [DL版] / [体験版] / region markers. Keep the original language; DO NOT translate.' || char(10) || \
                   '- authors: always return an empty list. The developer is filled from VNDB after the user confirms a match, not from the filename.' || char(10) || \
                   '- progress: null.' || char(10) || \
                   '' || char(10) || \
                   'Example:' || char(10) || \
                   '[Brand] 素晴らしき日々 (2010) [DL版] => display_name: 素晴らしき日々' || char(10) || \
                   '[まどそふと] ハミダシクリエイティブ凸 [DL版].zip => display_name: ハミダシクリエイティブ凸', \
                   'galgame_filename', 1, 'game', 'filename', 'galgame');",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 4,
            description: "remote multi-part objects",
            // A file larger than the remote per-object cap (Baidu SVIP: 20 GB)
            // is uploaded as N encrypted parts instead of one object. Each part
            // is an independent `.bbx` with its own remote object; this table
            // maps one logical file -> its ordered parts. The file row carries
            // `remote_container = 'bbx1-split'`; single-object rows ('bbx1') and
            // legacy raw rows (NULL) have no parts and are untouched.
            sql: "CREATE TABLE IF NOT EXISTS remote_parts (\
                      file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE, \
                      part_index INTEGER NOT NULL, \
                      object_name TEXT NOT NULL, \
                      fs_id TEXT, \
                      md5 TEXT, \
                      ciphertext_size INTEGER, \
                      plaintext_size INTEGER, \
                      PRIMARY KEY (file_id, part_index)\
                  );",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 5,
            description: "favorite file marker",
            sql: "ALTER TABLE files ADD COLUMN is_favorite BOOLEAN NOT NULL DEFAULT 0;\
                  CREATE INDEX IF NOT EXISTS idx_files_favorite ON files(is_favorite);",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 6,
            description: "runtime-defined schemas",
            // Schemas (resource types) become first-class rows instead of a
            // hard-coded enum + TS registry. `pipeline_template` picks the
            // built-in node composition (novel/comic/galgame) so behavior is
            // unchanged for the three seeded schemas; custom schemas added
            // later reuse a template. `semantic` on schema_fields marks the
            // fields code behavior attaches to (authors/cover/progress/...);
            // custom fields carry NULL and are pure data (metadata EAV).
            // Seeds mirror exactly the former frontend REGISTRY and
            // PROMPT_STEPS_BY_SCHEMA in src/lib/categorySchema.ts.
            sql: "CREATE TABLE IF NOT EXISTS schemas (\
                      id TEXT PRIMARY KEY,\
                      name TEXT NOT NULL,\
                      icon TEXT,\
                      description TEXT,\
                      accepted_extensions TEXT NOT NULL DEFAULT '[]',\
                      pipeline_template TEXT NOT NULL DEFAULT 'novel',\
                      is_builtin BOOLEAN NOT NULL DEFAULT 0,\
                      sort_order INTEGER NOT NULL DEFAULT 0,\
                      created_at TEXT NOT NULL DEFAULT (datetime('now')),\
                      updated_at TEXT NOT NULL DEFAULT (datetime('now'))\
                  );\
                  CREATE TABLE IF NOT EXISTS schema_fields (\
                      id INTEGER PRIMARY KEY AUTOINCREMENT,\
                      schema_id TEXT NOT NULL REFERENCES schemas(id) ON DELETE CASCADE,\
                      field_key TEXT NOT NULL,\
                      semantic TEXT,\
                      field_type TEXT NOT NULL DEFAULT 'builtin',\
                      label TEXT NOT NULL,\
                      options TEXT,\
                      form_visible BOOLEAN NOT NULL DEFAULT 1,\
                      card_visible BOOLEAN NOT NULL DEFAULT 0,\
                      sortable BOOLEAN NOT NULL DEFAULT 0,\
                      filterable BOOLEAN NOT NULL DEFAULT 0,\
                      required BOOLEAN NOT NULL DEFAULT 0,\
                      order_index INTEGER NOT NULL DEFAULT 0,\
                      UNIQUE(schema_id, field_key)\
                  );\
                  CREATE TABLE IF NOT EXISTS schema_pipeline_steps (\
                      id INTEGER PRIMARY KEY AUTOINCREMENT,\
                      schema_id TEXT NOT NULL REFERENCES schemas(id) ON DELETE CASCADE,\
                      step_key TEXT NOT NULL,\
                      label TEXT NOT NULL,\
                      enabled BOOLEAN NOT NULL DEFAULT 1,\
                      order_index INTEGER NOT NULL DEFAULT 0,\
                      UNIQUE(schema_id, step_key)\
                  );\
                  INSERT INTO schemas (id, name, accepted_extensions, pipeline_template, is_builtin, sort_order) VALUES \
                      ('novel', 'Novel', '[\"txt\"]', 'novel', 1, 0),\
                      ('comic', 'Comic', '[\"cbz\",\"zip\",\"cbr\",\"rar\"]', 'comic', 1, 1),\
                      ('galgame', 'Galgame', '[\"zip\",\"7z\",\"rar\"]', 'galgame', 1, 2);\
                  INSERT INTO schema_fields (schema_id, field_key, semantic, field_type, label, form_visible, card_visible, order_index) VALUES \
                      ('novel', 'display_name', 'display_name', 'builtin', 'Display Name', 1, 0, 0),\
                      ('novel', 'category', 'category', 'builtin', 'Category', 1, 0, 1),\
                      ('novel', 'authors', 'authors', 'builtin', 'Authors', 1, 1, 2),\
                      ('novel', 'tags', 'tags', 'builtin', 'Tags', 1, 0, 3),\
                      ('novel', 'progress', 'progress', 'builtin', 'Progress', 1, 0, 4),\
                      ('comic', 'display_name', 'display_name', 'builtin', 'Display Name', 1, 0, 0),\
                      ('comic', 'category', 'category', 'builtin', 'Category', 1, 0, 1),\
                      ('comic', 'authors', 'authors', 'builtin', 'Authors', 1, 1, 2),\
                      ('comic', 'cover', 'cover', 'builtin', 'Cover', 1, 0, 3),\
                      ('galgame', 'display_name', 'display_name', 'builtin', 'Display Name', 1, 0, 0),\
                      ('galgame', 'category', 'category', 'builtin', 'Category', 1, 0, 1),\
                      ('galgame', 'authors', 'authors', 'builtin', 'Authors', 1, 1, 2),\
                      ('galgame', 'cover', 'cover', 'builtin', 'Cover', 1, 0, 3);\
                  INSERT INTO schema_pipeline_steps (schema_id, step_key, label, enabled, order_index) VALUES \
                      ('novel', 'filename', 'Filename extraction', 1, 0),\
                      ('novel', 'content', 'Content analysis', 1, 1),\
                      ('comic', 'filename', 'Filename extraction', 1, 0),\
                      ('comic', 'cover_pick', 'Cover detection', 1, 1),\
                      ('comic', 'filename_folder', 'Folder filename extraction', 1, 2),\
                      ('galgame', 'filename', 'Filename extraction', 1, 0);",
            kind: MigrationKind::Up,
        },
    ]
}
