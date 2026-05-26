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
    ]
}
