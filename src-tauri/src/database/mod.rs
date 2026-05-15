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
        Migration {
            version: 3,
            description: "group prompts by mime type + add comic prompts",
            // The `category` column previously held a single token
            // ('filename' / 'content') that conflated *which step* with
            // *which mime group*. Splitting it lets us seed comic
            // (archive) prompts alongside the existing novel (text) ones
            // without overloading the value, and gives the UI a clean
            // grouping axis.
            sql: "
                ALTER TABLE prompts ADD COLUMN mime_group TEXT NOT NULL DEFAULT 'text';
                ALTER TABLE prompts ADD COLUMN step TEXT NOT NULL DEFAULT 'filename';
                UPDATE prompts SET mime_group = 'text', step = category WHERE category IN ('filename', 'content');
                CREATE INDEX IF NOT EXISTS idx_prompts_mime_step ON prompts(mime_group, step);

                INSERT INTO prompts (name, content, category, mime_group, step, is_default) VALUES (
                    'Comic Filename Extraction',
                    'Extract metadata from this comic archive filename only. Rules:
- display_name: clean comic title (strip site prefixes like [sxsy.org], scanlator brackets like [天蝎座汉化], the file extension, and any volume/chapter markers like 第01-10话/Vol.5/Ch.42)
- authors: extract the original author/artist if present (patterns: ''作者：xxx'', ''xxx著'', or pulled from the title itself when obvious). Treat scanlator/translator credits as NOT authors
- progress: combine volume/chapter range + status, e.g. ''第1-10话 连载中'', ''第1卷-第5卷 完结'', ''第42话''
- Use null for unknown fields',
                    'comic_filename',
                    'archive',
                    'filename',
                    1
                );

                INSERT INTO prompts (name, content, category, mime_group, step, is_default) VALUES (
                    'Comic Cover Detection',
                    'You are picking the cover image of a comic archive given the list of image filenames inside it. Rules:
- Return up to 5 filenames from the input list, ordered best-first.
- The first file in the input (often named 000.jpg, 001.png, cover.jpg, cover01.jpg) is usually the cover — rank it first when plausible.
- Filenames containing ''cover'', ''front'', ''title'', or ''扉'' (Chinese for ''title page'') are strong cover signals.
- Return the filenames verbatim — do not invent entries.
- If nothing looks like a cover, return an empty list.',
                    'comic_cover_pick',
                    'archive',
                    'cover_pick',
                    1
                );
            ",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 4,
            description: "add image_folder filename prompt",
            // Folder-as-comic imports take their author from the picked
            // root folder (cleaned via the existing folder-name LLM
            // pass), so the per-folder filename extraction should NOT
            // re-extract authors. A dedicated prompt makes that
            // separation explicit and keeps the archive prompt unchanged.
            // Cover-pick reasoning is identical to archives, so we do
            // NOT seed a separate `(image_folder, cover_pick)` prompt —
            // the comic pipeline keeps using `(archive, cover_pick)`
            // for both source kinds.
            sql: "
                INSERT INTO prompts (name, content, category, mime_group, step, is_default) VALUES (
                    'Image Folder Filename Extraction',
                    'Extract metadata from this image-folder name only. The author is already extracted separately from the parent folder, so DO NOT extract authors here. Rules:
- display_name: clean comic title (strip site prefixes like [sxsy.org], scanlator brackets like [天蝎座汉化] / [镜面光折射汉化], date prefixes like [2022.12], and any volume/chapter markers like 第01-10话/Vol.5/Ch.42)
- authors: always return an empty list. Authors are NOT extracted from folder names in this prompt.
- progress: combine volume/chapter range + status when present, e.g. ''第1-10话 连载中'', ''第1卷-第5卷 完结'', ''第42话''. Use null when no progress markers exist.
- Use null for unknown fields (other than authors, which is always empty).',
                    'image_folder_filename',
                    'image_folder',
                    'filename',
                    1
                );
            ",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 5,
            description: "rebuild files_fts with trigram tokenizer",
            // The default `unicode61` tokenizer treats CJK runs as one
            // token and only supports left-anchored prefix queries, so
            // typing a middle character (e.g. `体` for `三体`) or a
            // mid-word Latin substring returned nothing. `trigram` indexes
            // every 3-character window, giving substring search natively.
            // Queries shorter than 3 chars must fall back to LIKE in the
            // command layer — trigram has no rows below the window size.
            sql: "
                DROP TABLE IF EXISTS files_fts;
                CREATE VIRTUAL TABLE files_fts USING fts5(
                    display_name,
                    path,
                    content='files',
                    content_rowid='id',
                    tokenize='trigram'
                );
                INSERT INTO files_fts(rowid, display_name, path)
                    SELECT id, display_name, path FROM files;
            ",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 6,
            description: "track local cache for downloaded remote files",
            // Populated by the Download worker when a remote row is pulled
            // back to disk. Nullable: NULL means no local cache. Lets the
            // UI show a "cached locally" badge without an extra fs check
            // and gives the Delete worker something to clean up.
            sql: "ALTER TABLE files ADD COLUMN local_cache_path TEXT;",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 7,
            description: "category-driven schema slugs",
            // Categories now own a `schema_slug` that the frontend reads to
            // pick form sections + card layout, and the backend reads to
            // resolve which prompts to run. Built-in slugs: 'novel',
            // 'comic'. Backfill: name=comic → comic, otherwise novel
            // (covers 'novel', 'h-novel', and any user-added category).
            //
            // Prompts move from `(mime_group, step)` to
            // `(schema_slug, step)`. The image_folder filename prompt
            // becomes `(comic, filename_folder)` — distinct step under the
            // comic schema, since the comic pipeline picks between archive
            // and folder sources at runtime.
            //
            // `mime_group` stays in place for one release as a backstop
            // for any reader we missed; the next migration drops it.
            sql: "
                ALTER TABLE categories ADD COLUMN schema_slug TEXT NOT NULL DEFAULT 'novel';
                UPDATE categories SET schema_slug = 'comic' WHERE LOWER(name) = 'comic';
                UPDATE categories SET schema_slug = 'novel' WHERE LOWER(name) IN ('novel', 'h-novel');

                ALTER TABLE prompts ADD COLUMN schema_slug TEXT;
                UPDATE prompts SET schema_slug = 'novel' WHERE mime_group = 'text';
                UPDATE prompts SET schema_slug = 'comic' WHERE mime_group IN ('archive', 'image_folder');
                UPDATE prompts SET step = 'filename_folder' WHERE mime_group = 'image_folder' AND step = 'filename';

                CREATE INDEX IF NOT EXISTS idx_categories_schema_slug ON categories(schema_slug);
                CREATE INDEX IF NOT EXISTS idx_prompts_schema_step ON prompts(schema_slug, step);
            ",
            kind: MigrationKind::Up,
        },
    ]
}
