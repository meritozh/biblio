use super::*;

/// In-memory DB with the v6 tables (plus the categories/files/
/// metadata slices the data-count and delete paths touch) and the
/// built-in seeds.
async fn seeded_pool() -> sqlx::SqlitePool {
    let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE schemas (\
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
        CREATE TABLE schema_fields (\
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
        CREATE TABLE schema_pipeline_steps (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            schema_id TEXT NOT NULL REFERENCES schemas(id) ON DELETE CASCADE,\
            step_key TEXT NOT NULL,\
            label TEXT NOT NULL,\
            enabled BOOLEAN NOT NULL DEFAULT 1,\
            order_index INTEGER NOT NULL DEFAULT 0,\
            UNIQUE(schema_id, step_key)\
        );\
        CREATE TABLE categories (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            name TEXT NOT NULL UNIQUE,\
            schema_slug TEXT NOT NULL DEFAULT 'novel'\
        );\
        CREATE TABLE files (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            category_id INTEGER\
        );\
        CREATE TABLE metadata (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            file_id INTEGER,\
            key TEXT,\
            value TEXT\
        );\
        CREATE TABLE prompts (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            schema_slug TEXT\
        );\
        INSERT INTO schemas (id, name, accepted_extensions, pipeline_template, is_builtin, sort_order) VALUES \
            ('novel', 'Novel', '[\"txt\"]', 'novel', 1, 0),\
            ('comic', 'Comic', '[\"cbz\",\"zip\",\"cbr\",\"rar\"]', 'comic', 1, 1);\
        INSERT INTO schema_fields (schema_id, field_key, semantic, field_type, label, form_visible, card_visible, order_index) VALUES \
            ('novel', 'display_name', 'display_name', 'builtin', 'Display Name', 1, 0, 0),\
            ('novel', 'authors', 'authors', 'builtin', 'Authors', 1, 1, 1);\
        INSERT INTO schema_pipeline_steps (schema_id, step_key, label, enabled, order_index) VALUES \
            ('novel', 'filename', 'Filename extraction', 1, 0),\
            ('novel', 'content', 'Content analysis', 1, 1);",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

fn builtin_field(key: &str, order: i64) -> SchemaFieldInput {
    SchemaFieldInput {
        field_key: key.to_string(),
        semantic: Some(key.to_string()),
        field_type: "builtin".to_string(),
        label: key.to_string(),
        options: None,
        form_visible: true,
        card_visible: false,
        sortable: false,
        filterable: false,
        required: false,
        order_index: order,
    }
}

fn custom_field(key: &str, field_type: &str, order: i64) -> SchemaFieldInput {
    SchemaFieldInput {
        field_key: key.to_string(),
        semantic: None,
        field_type: field_type.to_string(),
        label: key.to_string(),
        options: if field_type == "enum" {
            Some("[\"a\",\"b\"]".to_string())
        } else {
            None
        },
        form_visible: true,
        card_visible: false,
        sortable: false,
        filterable: false,
        required: false,
        order_index: order,
    }
}

fn payload(fields: Vec<SchemaFieldInput>) -> SchemaUpsertPayload {
    SchemaUpsertPayload {
        name: "Podcast".to_string(),
        icon: None,
        description: None,
        accepted_extensions: vec!["mp3".to_string()],
        pipeline_template: "novel".to_string(),
        fields,
    }
}

#[test]
fn schema_id_rules() {
    assert!(validate_schema_id("podcast").is_ok());
    assert!(validate_schema_id("audio_book2").is_ok());
    assert!(validate_schema_id("Podcast").is_err());
    assert!(validate_schema_id("2books").is_err());
    assert!(validate_schema_id("has-dash").is_err());
    assert!(validate_schema_id("").is_err());
}

#[test]
fn extension_normalization() {
    let exts = normalize_extensions(&[" MP3 ".to_string(), ".mp3".to_string(), "Flac".to_string()])
        .unwrap();
    assert_eq!(exts, vec!["mp3", "flac"]);
    assert!(normalize_extensions(&["a b".to_string()]).is_err());
}

#[test]
fn payload_validation_rejects_bad_fields() {
    assert_eq!(
        validate_payload(&payload(vec![custom_field("Bad", "text", 0)])).unwrap_err(),
        "INVALID_FIELD_KEY"
    );
    assert_eq!(
        validate_payload(&payload(vec![custom_field("x", "widget", 0)])).unwrap_err(),
        "INVALID_FIELD_TYPE"
    );
    assert_eq!(
        validate_payload(&payload(vec![SchemaFieldInput {
            options: None,
            ..custom_field("status", "enum", 0)
        }]))
        .unwrap_err(),
        "INVALID_FIELD_OPTIONS"
    );
    let mut dup = payload(vec![
        custom_field("a", "text", 0),
        custom_field("a", "text", 1),
    ]);
    assert_eq!(validate_payload(&dup).unwrap_err(), "DUPLICATE_FIELD_KEY");
    dup.pipeline_template = "weird".to_string();
    assert_eq!(
        validate_payload(&dup).unwrap_err(),
        "INVALID_PIPELINE_TEMPLATE"
    );
}

#[tokio::test]
async fn count_field_data_scopes_to_schema_files() {
    let pool = seeded_pool().await;
    // Two categories on different schemas, one file each, both with
    // a 'publisher' metadata row — only the novel one counts.
    sqlx::query(
        "INSERT INTO categories (id, name, schema_slug) VALUES (1, 'Books', 'novel'), (2, 'Manga', 'comic');\
         INSERT INTO files (id, category_id) VALUES (10, 1), (20, 2);\
         INSERT INTO metadata (file_id, key, value) VALUES (10, 'publisher', 'A'), (20, 'publisher', 'B');",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        count_field_data(&pool, "novel", "publisher").await.unwrap(),
        1
    );
    assert_eq!(
        count_field_data(&pool, "comic", "publisher").await.unwrap(),
        1
    );
    assert_eq!(
        count_field_data(&pool, "novel", "missing").await.unwrap(),
        0
    );
}
#[tokio::test]
async fn invalid_step_rolls_back_the_whole_native_definition() {
    let dir = tempfile::tempdir().unwrap();
    let db = crate::Library::open(dir.path().to_owned(), |_, _| {})
        .await
        .unwrap();
    let result = save_definition(
        db.clone(),
        "audio_book".into(),
        payload(vec![builtin_field("display_name", 0)]),
        false,
        Some(vec![SchemaStepInput {
            step_key: "unknown_step".into(),
            label: "Unknown".into(),
            enabled: true,
            order_index: 0,
        }]),
    )
    .await;
    assert_eq!(result.err().as_deref(), Some("INVALID_STEP_KEY"));
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM schemas WHERE id='audio_book')")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(!exists);
}
