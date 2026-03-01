pub mod seed;
pub mod recovery;

use tauri_plugin_sql::{Migration, MigrationKind};

pub fn get_migrations() -> Vec<Migration> {
    vec![Migration {
        version: 1,
        description: "Initial schema with categories, files, tags, metadata, and FTS",
        sql: include_str!("schema.sql"),
        kind: MigrationKind::Up,
    }]
}