use biblio_ui::root;
use nsh_core::prelude::{px, size};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;
use std::{path::Path, time::Duration};

const WAIT: Duration = Duration::from_secs(5);

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
