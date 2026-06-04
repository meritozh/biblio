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
    ]
}
