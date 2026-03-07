pub mod seed;
pub mod recovery;

use tauri_plugin_sql::{Migration, MigrationKind};

pub fn get_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "Initial schema with categories, files, tags, metadata, and FTS",
            sql: include_str!("schema.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "Add authors, file_authors, and covers tables",
            sql: include_str!("migration_2.sql"),
            kind: MigrationKind::Up,
        },
    ]
}