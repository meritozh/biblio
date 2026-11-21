use biblio_ui::root;
use nsh_core::prelude::{px, size};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

const WAIT: Duration = Duration::from_secs(5);

/// Writes to the profile at `config` straight through the catalogue, before
/// a window opens it.
fn seed<F>(config: &Path, write: impl FnOnce(biblio_core::Library) -> F)
where
    F: Future<Output = Result<(), String>>,
{
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime
        .block_on(async {
            let db = biblio_core::Library::open(config.to_path_buf(), |_, _| {}).await?;
            write(db).await
        })
        .unwrap();
    runtime.shutdown_timeout(Duration::from_secs(1));
}

/// A profile under `dir` with a storage folder and the category "Shelf"
/// holding one text file per name.
fn shelf(dir: &Path, names: Vec<String>) -> PathBuf {
    use biblio_core::commands::{category, file::create_replace::file_create, settings};
    let config = dir.join("config");
    let storage = dir.join("books");
    let sources = dir.join("sources");
    std::fs::create_dir(&storage).unwrap();
    std::fs::create_dir(&sources).unwrap();
    seed(&config, |db| async move {
        let path = storage.display().to_string();
        settings::settings_set(db.clone(), "storage_path".into(), path).await?;
        let shelf =
            category::category_create(db.clone(), "Shelf".into(), None, "novel".into(), None)
                .await?
                .id;
        for name in names {
            let source = sources.join(format!("{name}.txt"));
            std::fs::write(&source, &name).unwrap();
            let path = source.display().to_string();
            file_create(
                db.clone(),
                path,
                name,
                Some(shelf),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
        }
        Ok(())
    });
    config
}

fn window(config: &Path) -> Headless {
    let root = root(config).unwrap();
    Headless::start(EngineConfig {
        environment: nsh_core::platform::Environment {
            os: nsh_core::platform::Os::MacOs,
            ..Default::default()
        },
        viewport: size(px(1200.), px(800.)),
        ..EngineConfig::new(root)
    })
    .unwrap()
}
#[test]
fn import_search_favorite_edit_and_reopen() -> Result<(), String> {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let storage = dir.path().join("books");
    std::fs::create_dir(&storage).unwrap();
    let source = dir.path().join("Autumn catalogue.txt");
    std::fs::write(&source, "Test import").unwrap();
    let mut app = window(&config);
    app.wait_for("Your library is empty")?;
    app.press("label:Settings")?;
    app.wait_for("label:Storage folder")?;
    app.type_text("label:Storage folder", storage.to_str().unwrap())?;
    app.press("label:Save storage folder")?;
    app.wait_for("Saved")?;
    app.wait_for("label:Copy")?;
    app.press("label:Copy")?;
    app.wait_for(
        "Copies imported files into the storage folder and leaves the originals in place.",
    )?;
    app.press("label:Categories")?;
    app.wait_for("label:Category name")?;
    app.type_text("label:Category name", "Books")?;
    app.press("label:Create category")?;
    app.wait_for("id:navigation >> label:Books")?;
    app.press("id:navigation >> label:Books")?;
    app.wait_for("This category is empty")?;
    app.press("label:Import files")?;
    app.wait_for("label:Analyze selected files")?;
    app.drop_files(
        "label:Analyze selected files",
        vec![url::Url::from_file_path(&source).unwrap().to_string()],
    )?;
    app.wait_for("1 selected · 0 analyzing · 0 ready for review")?;
    app.press("label:Analyze selected files")?;
    app.wait_for("label:Import this file")?;
    app.clear("label:Title")?;
    app.type_text("label:Title", "Autumn catalogue")?;
    app.press("label:Import this file")?;
    app.wait_for("Imported")?;
    assert!(source.is_file(), "copy mode retains the source");
    app.press("label:All files")?;
    app.wait_for("label:Autumn catalogue")?;
    app.press("label:Favorite")?;
    app.wait_for("label:Unfavorite")?;
    app.type_text("label:Search library", "Autumn")?;
    app.submit("label:Search library")?;
    app.wait_for("label:Autumn catalogue")?;
    app.press("label:Autumn catalogue")?;
    app.wait_for("label:Save file")?;
    app.clear("label:Title")?;
    app.type_text("label:Title", "Autumn shelves")?;
    app.press("label:Save file")?;
    app.wait_for("Autumn shelves")?;
    drop(app);
    let mut reopened = window(&config);
    reopened.wait_for("label:Autumn shelves")?;
    reopened.wait_for("label:Unfavorite")?;
    for (page, expected) in [
        ("Tags", "label:New name"),
        ("Authors", "label:New name"),
        ("Prompts", "label:Create prompt"),
        ("Schemas", "label:New schema"),
        ("Cleanup", "label:Find possible duplicates"),
    ] {
        reopened.press(&format!("label:{page}"))?;
        reopened.wait_for(expected)?;
    }
    Ok(())
}

#[test]
fn keyboard_filter_survives_navigation_and_preset_reopens() -> Result<(), String> {
    let profile = tempfile::tempdir().unwrap();
    let mut app = window(profile.path());
    app.wait_for("Your library is empty")?;
    app.type_text("label:Search library", "star")?;
    app.wait_for("label:Favorite…")?;
    app.press_key("down")?;
    app.press_key("enter")?;
    app.wait_for("label:Favorited")?;
    app.press_key("down")?;
    app.wait_until("the first suggestion highlighted", WAIT, |screen| {
        screen
            .accessibility_state("label:Favorited")
            .is_ok_and(|s| s.selected)
    })?;
    app.press_key("enter")?;
    app.wait_for("label:Favorited ×")
        .map_err(|e| format!("After selecting favorite: {e}"))?;
    assert_eq!(
        app.screen()
            .input_text("label:Search library")?
            .unwrap_or_default(),
        "",
        "the text that found the suggestion is not left behind as a search"
    );
    app.press_key("tab")?;
    app.wait_for_gone("label:Favorite…", std::time::Duration::from_secs(1))?;
    app.press("label:Filters")?;
    app.wait_for("label:Preset name")?;
    app.type_text("label:Preset name", "Reading list")?;
    app.press("label:Save current filter")?;
    app.wait_for("label:Reading list")?;
    app.press("label:Settings")?;
    app.press("label:All files")?;
    app.wait_for("label:Favorited ×")?;
    drop(app);
    let mut reopened = window(profile.path());
    reopened.wait_for("label:Filters")?;
    reopened.press("label:Filters")?;
    reopened.wait_for("label:Reading list")?;
    reopened.press("label:Reading list")?;
    reopened.wait_for("label:favorite is true ×")?;
    Ok(())
}

#[test]
fn compact_navigation_preserves_search_and_reaches_settings() -> Result<(), String> {
    let profile = tempfile::tempdir().unwrap();
    let mut app = window(profile.path());
    app.wait_for("Your library is empty")?;
    app.type_text("label:Search library", "absent title")?;
    app.submit("label:Search library")?;
    app.wait_for("No matching files")?;
    app.set_viewport(size(px(375.), px(680.)))?;
    app.press("label:Navigation")?;
    app.wait_for("label:Settings")?;
    app.press("label:Navigation")?;
    app.wait_for("No matching files")?;
    app.press("label:Clear")?;
    app.wait_for("Your library is empty")?;
    app.press("label:Navigation")?;
    app.press("label:Settings")?;
    app.wait_for("label:Storage folder")?;
    app.set_viewport(size(px(1200.), px(800.)))?;
    app.press("label:All files")?;
    app.wait_for("Your library is empty")
}

#[test]
fn file_menu_asks_before_deleting() -> Result<(), String> {
    use nsh_testing::DialogAnswer;
    let dir = tempfile::tempdir().unwrap();
    let storage = dir.path().join("books");
    std::fs::create_dir(&storage).unwrap();
    let source = dir.path().join("Winter ledger.txt");
    std::fs::write(&source, "Test import").unwrap();
    let mut app = window(&dir.path().join("config"));
    app.wait_for("Your library is empty")?;
    app.press("label:Settings")?;
    app.wait_for("label:Storage folder")?;
    app.type_text("label:Storage folder", storage.to_str().unwrap())?;
    app.press("label:Save storage folder")?;
    app.wait_for("Saved")?;
    app.press("label:Categories")?;
    app.wait_for("label:Category name")?;
    app.type_text("label:Category name", "Ledgers")?;
    app.press("label:Create category")?;
    app.press("id:navigation >> label:Ledgers")?;
    app.wait_for("This category is empty")?;
    app.press("label:Import files")?;
    app.drop_files(
        "label:Analyze selected files",
        vec![url::Url::from_file_path(&source).unwrap().to_string()],
    )?;
    app.press("label:Analyze selected files")?;
    app.wait_for("label:Import this file")?;
    app.press("label:Import this file")?;
    app.wait_for("Imported")?;
    app.press("id:navigation >> label:Ledgers")?;
    app.press("label:Winter ledger.txt")?;
    app.wait_for("label:Save file")?;
    // Cancel keeps the file.
    app.on_dialog(|_| DialogAnswer::Press("Cancel".into()));
    app.press("label:More actions")?;
    app.press("label:Delete file and library record")?;
    app.wait_for("label:Save file")?;
    // Confirming deletes it and returns to the (now empty) category.
    app.on_dialog(|d| {
        assert!(
            d.title
                .as_deref()
                .unwrap_or_default()
                .contains("Winter ledger"),
            "the alert names the file"
        );
        DialogAnswer::Press("Delete".into())
    });
    app.press("label:More actions")?;
    app.press("label:Delete file and library record")?;
    app.wait_for("This category is empty")
}

#[test]
fn command_k_focuses_search_from_another_page() -> Result<(), String> {
    use nsh_core::Command;
    let profile = tempfile::tempdir().unwrap();
    let mut app = window(profile.path());
    app.wait_for("Your library is empty")?;
    app.press("label:Settings")?;
    app.wait_for("label:Storage folder")?;
    let focused = |s: &nsh_testing::Screen| {
        s.commands()
            .iter()
            .filter(|(_, c)| *c == Command::Focus)
            .count()
    };
    let before = focused(app.screen());
    app.press_key("mod+k")?;
    app.wait_for("Your library is empty")?;
    app.wait_until("the search field focused", WAIT, |s| focused(s) > before)?;
    for key in ["t", "e", "a"] {
        app.press_key(key)?;
    }
    app.wait_until("typing goes to the search field", WAIT, |s| {
        s.input_text("label:Search library") == Ok(Some("tea"))
    })
}

#[test]
fn deleting_the_last_file_on_a_page_shows_the_last_page_left() -> Result<(), String> {
    use nsh_testing::DialogAnswer;
    let dir = tempfile::tempdir().unwrap();
    let names: Vec<String> = (1..=31).map(|n| format!("File {n:02}")).collect();
    let config = shelf(dir.path(), names);
    let mut app = window(&config);
    app.wait_for("1–30 of 31")?;
    app.press("label:Next")?;
    app.wait_for("label:File 31")?;
    app.press("label:File 31")?;
    app.wait_for("label:Save file")?;
    app.on_dialog(|_| DialogAnswer::Press("Delete".into()));
    app.press("label:More actions")?;
    app.press("label:Delete file and library record")?;
    // The second page is gone; the first, now the only one, takes its place.
    app.wait_for("30 files")?;
    app.wait_for("label:File 01")
}

#[test]
fn deleting_the_last_tag_on_a_page_shows_the_last_page_left() -> Result<(), String> {
    use biblio_core::commands::tag;
    use nsh_testing::DialogAnswer;
    let profile = tempfile::tempdir().unwrap();
    seed(profile.path(), |db| async move {
        for n in 1..=31 {
            tag::tag_create(db.clone(), format!("Tag {n:02}"), None).await?;
        }
        Ok(())
    });
    let mut app = window(profile.path());
    app.wait_for("Your library is empty")?;
    app.press("label:Tags")?;
    app.wait_for("31 tags")?;
    app.press("label:Next")?;
    app.wait_for("label:Delete Tag 31")?;
    app.on_dialog(|_| DialogAnswer::Press("Delete".into()));
    app.press("label:Delete Tag 31")?;
    app.wait_for("30 tags")?;
    app.wait_for("label:Delete Tag 01")?;
    app.type_text("label:New name", "Tag 32")?;
    app.press("label:Create")?;
    app.wait_for("31 tags")?;
    app.wait_for("label:Delete Tag 01")?;
    app.press("label:Next")?;
    app.wait_for("label:Delete Tag 32")
}

#[test]
fn select_this_page_takes_the_files_in_open_groups() -> Result<(), String> {
    let dir = tempfile::tempdir().unwrap();
    let mut names: Vec<_> = (1..=31).map(|n| format!("Saga {n:02}")).collect();
    names.extend(["Pair 1".into(), "Pair 2".into(), "Solo".into()]);
    let config = shelf(dir.path(), names);
    seed(&config, |db| async move {
        biblio_core::commands::category::category_create(
            db,
            "Empty".into(),
            None,
            "novel".into(),
            None,
        )
        .await?;
        Ok(())
    });
    let mut app = window(&config);
    app.wait_for("1–30 of 34")?;
    app.press("id:navigation >> label:Shelf")?;
    app.wait_for("label:View options")?;
    app.press("label:View options")?;
    app.press("label:Group by title")?;
    app.wait_for("label:Saga")?;
    app.press("label:Select files")?;
    // With every group closed no file is on screen.
    app.press("label:Select this page")?;
    app.wait_for("0 selected")?;
    app.press("label:Saga")?;
    app.wait_for("label:Saga 01")?;
    app.press("label:Select this page")?;
    app.wait_for("30 selected")?;
    app.press("label:Clear selection")?;
    app.press("label:Next in group")?;
    app.wait_for("label:Saga 31")?;
    app.press("label:Pair")?;
    app.wait_for("label:Pair 2")?;
    app.press("label:Select this page")?;
    app.wait_for("3 selected")?;
    app.press("label:Clear selection")?;
    app.press("label:Saga")?;
    app.press("label:Select this page")?;
    app.wait_for("2 selected")?;
    app.press("id:navigation >> label:Empty")?;
    app.wait_for("This category is empty")?;
    app.press("label:Select this page")?;
    app.wait_for("0 selected")
}
