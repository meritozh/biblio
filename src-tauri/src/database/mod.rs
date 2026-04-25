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
    ]
}
