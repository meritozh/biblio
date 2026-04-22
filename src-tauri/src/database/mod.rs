pub mod seed;
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
            description: "add remote storage columns",
            // Every column is nullable (or defaults) so existing rows keep
            // storage_kind='local' and NULL remote_* fields without a
            // backfill step. `path` stays non-null — for remote rows it
            // holds the provider path (e.g. /apps/biblio/<base64>.cbz),
            // disambiguated from local filesystem paths by storage_kind.
            sql: "
                ALTER TABLE files ADD COLUMN storage_kind TEXT NOT NULL DEFAULT 'local';
                ALTER TABLE files ADD COLUMN remote_provider TEXT;
                ALTER TABLE files ADD COLUMN remote_fs_id TEXT;
                ALTER TABLE files ADD COLUMN remote_size INTEGER;
                ALTER TABLE files ADD COLUMN remote_md5 TEXT;
                CREATE INDEX IF NOT EXISTS idx_files_storage_kind ON files(storage_kind);
            ",
            kind: MigrationKind::Up,
        },
    ]
}
