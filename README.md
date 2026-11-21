# Biblio

Biblio is a Rust desktop app for organizing novels, comics and visual novels. The native interface uses [NSH](https://nsh.dev/). The library, search, import pipelines and transfer workers run in the same process and store their records in SQLite.

## Run and build

Install stable Rust and the native build tools for your platform. On macOS, use macOS 14 or later with Xcode Command Line Tools.

```sh
cargo run --locked
```

Choose a storage folder in **Settings**, create a category, then choose or drop files or folders in **Import**. Review extracted metadata before importing. The storage setting controls the destination, and the import mode determines whether source files are moved or copied.

Build a locally signed macOS app with the project icon and bundle identifier:

```sh
./scripts/bundle-macos.sh release
open target/release/Biblio.app
```

Use `debug` for a development bundle. Cargo can use a custom target directory. The bundling script prints the resulting path.

## Library

- Search titles and paths, select tag and author suggestions, combine filter conditions, and save presets. Browse by category, author or title group.
- Edit titles, covers, reading progress, favorites, tags, authors and schema-defined metadata. Use bulk selection for transfers and file management.
- The built-in novel, comic and visual novel schemas control accepted files and import steps. Manage custom schemas and prompts inside the app. Comics can be imported from image folders or archives.
- Optional OpenAI-compatible endpoints provide metadata extraction. Visual novel imports can use VNDB. Optional Baidu Netdisk storage uses the existing encrypted container format, including multipart files and legacy downloads.

## Existing installations

The native app opens the existing Biblio profile. On macOS the catalog is at `~/Library/Application Support/io.augite.biblio/biblio.db`. The storage folder, associations, cover bytes, remote settings and migration history live in this database. Configure `BIBLIO_CONFIG_DIR` to use a separate native profile for development or testing. Close the running app before opening the same profile in another build.

The earlier React/Tauri interface is available in Git at `9171902`. The native desktop reads appearance settings and named filters from SQLite. Preferences saved only in the former WebView can be set again in **Settings** and **Filters**.

## Source and verification

| Path | Purpose |
| --- | --- |
| `src/main.rs` | Chooses the profile directory and starts the native platform runner |
| `crates/biblio-ui/src/` | NSH window, navigation, screens and shared controls |
| `crates/biblio-core/src/` | Catalog services, SQLite migrations, import pipelines and remote storage |
| `crates/biblio-ui/tests/` | Headless interaction tests using the same window as the desktop app |
| `tests/` | Workspace source checks |
| `assets/` and `packaging/` | Source artwork and platform packaging resources |

```sh
cargo test --locked --workspace
cargo check --locked --workspace --all-targets
```

Run the UI tests independently with `cargo test --locked -p biblio-ui`.

Add numbered migrations in `crates/biblio-core/src/database/mod.rs`. Existing installations verify the checksum of each applied migration, including the baseline in `schema.sql`.
