pub mod recovery;

struct Migration {
    version: i64,
    description: &'static str,
    sql: &'static str,
}

fn get_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "complete schema",
            sql: include_str!("schema.sql"),
        },
        Migration {
            version: 2,
            description: "remote container marker",
            // NULL = legacy raw upload; 'bbx1' = encrypted container object.
            // The download/re-encrypt paths key off this column, so it is the
            // authority for whether a remote object must be unwrapped.
            sql: "ALTER TABLE files ADD COLUMN remote_container TEXT;",
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
        },
        Migration {
            version: 5,
            description: "favorite file marker",
            sql: "ALTER TABLE files ADD COLUMN is_favorite BOOLEAN NOT NULL DEFAULT 0;\
                  CREATE INDEX IF NOT EXISTS idx_files_favorite ON files(is_favorite);",
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
        },
        Migration {
            version: 7,
            description: "vndb pipeline step for galgame",
            // The VNDB metadata lookup (review-dialog enrichment for
            // galgame imports) becomes a toggleable pipeline step so the
            // schema editor can switch it off. Frontend gates its VNDB
            // panel on this step being enabled.
            sql: "INSERT OR IGNORE INTO schema_pipeline_steps (schema_id, step_key, label, enabled, order_index) VALUES \
                      ('galgame', 'vndb', 'VNDB metadata lookup', 1, 1);",
        },
    ]
}

pub(crate) async fn open(directory: &std::path::Path) -> Result<sqlx::SqlitePool, String> {
    use sqlx::migrate::{Migration as SqlMigration, MigrationType, Migrator};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let options = SqliteConnectOptions::new()
        .filename(directory.join("biblio.db"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .map_err(|e| e.to_string())?;
    let migrations = get_migrations()
        .into_iter()
        .map(|m| {
            SqlMigration::new(
                m.version,
                m.description.into(),
                MigrationType::ReversibleUp,
                m.sql.into(),
                false,
            )
        })
        .collect::<Vec<_>>();
    Migrator {
        migrations: migrations.into(),
        ..Migrator::DEFAULT
    }
    .run(&pool)
    .await
    .map_err(|e| format!("Database migration failed: {e}"))?;
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::open;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    #[tokio::test]
    async fn opens_populated_tauri_database_and_preserves_migration_history() {
        let dir = tempfile::tempdir().unwrap();
        let options = SqliteConnectOptions::new()
            .filename(dir.path().join("biblio.db"))
            .create_if_missing(true);
        let old = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&old)
            .await
            .unwrap();
        // Ledger format and SHA-384 of v1 shipped by the Tauri SQL plugin.
        sqlx::raw_sql("CREATE TABLE _sqlx_migrations(version BIGINT PRIMARY KEY, description TEXT NOT NULL, installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, success BOOLEAN NOT NULL, checksum BLOB NOT NULL, execution_time BIGINT NOT NULL);
            INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES(1,'complete schema',1,X'E6DA0E47BB56EA628D419AB453B823D44220E6E7DE067DF1ECCF21A4588001F73A1EC334C26201372A3855D317BA6367',0);
            INSERT INTO categories(id,name,schema_slug) VALUES(71,'Old library','novel');
            INSERT INTO files(id,path,display_name,category_id) VALUES(91,'Old library/title.txt','水浒传',71);
            INSERT INTO tags(id,name) VALUES(81,'经典');
            INSERT INTO file_tags(file_id,tag_id) VALUES(91,81);").execute(&old).await.unwrap();
        old.close().await;
        for _ in 0..2 {
            let pool = open(dir.path()).await.unwrap();
            let ledger: Vec<(i64, bool)> =
                sqlx::query_as("SELECT version,success FROM _sqlx_migrations ORDER BY version")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
            assert_eq!(ledger, (1..=7).map(|v| (v, true)).collect::<Vec<_>>());
            let row: (String,String,bool)=sqlx::query_as("SELECT f.path,t.name,f.is_favorite FROM files f JOIN file_tags ft ON f.id=ft.file_id JOIN tags t ON t.id=ft.tag_id WHERE f.id=91").fetch_one(&pool).await.unwrap();
            assert_eq!(row, ("Old library/title.txt".into(), "经典".into(), false));
            pool.close().await;
        }
    }

    #[tokio::test]
    async fn failed_replacement_preserves_existing_file_and_database_backup() {
        use crate::{
            Library, commands::file::search_lucky_status::file_replace,
            database::recovery::DatabaseRecovery,
        };
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(dir.path().to_owned(), |_, _| {})
            .await
            .unwrap();
        assert!(
            Library::open(dir.path().to_owned(), |_, _| {})
                .await
                .is_err()
        );
        let storage = dir.path().join("storage");
        std::fs::create_dir(&storage).unwrap();
        std::fs::write(storage.join("old.txt"), b"original").unwrap();
        let candidate = dir.path().join("incoming.txt");
        std::fs::write(&candidate, b"replacement").unwrap();
        sqlx::query("UPDATE app_settings SET value=? WHERE key='storage_path'")
            .bind(storage.to_str().unwrap())
            .execute(&lib.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO files(id,path,display_name,in_storage,is_favorite) VALUES(77,'old.txt','Old',1,1)").execute(&lib.pool).await.unwrap();
        let result = file_replace(
            lib.clone(),
            77,
            candidate.to_string_lossy().into_owned(),
            "invalid".into(),
            Some(-1),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await;
        assert_eq!(result.err().as_deref(), Some("CATEGORY_NOT_FOUND"));
        assert_eq!(std::fs::read(storage.join("old.txt")).unwrap(), b"original");
        assert!(candidate.is_file());
        let backup = DatabaseRecovery::create_backup(&lib).await.unwrap();
        let snapshot = SqlitePoolOptions::new()
            .connect_with(SqliteConnectOptions::new().filename(backup).read_only(true))
            .await
            .unwrap();
        let favorite: bool = sqlx::query_scalar("SELECT is_favorite FROM files WHERE id=77")
            .fetch_one(&snapshot)
            .await
            .unwrap();
        assert!(favorite);
    }
}
