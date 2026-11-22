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
/// holding one file per name (TXT by default).
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
            let mut source = sources.join(&name);
            if source.extension().is_none() {
                source.set_extension("txt");
            }
            std::fs::write(&source, format!("Text of {name}")).unwrap();
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

fn grid_details(app: &mut Headless, card: &str) -> Result<(), String> {
    app.wait_for(card)?;
    app.right_click(card)?;
    app.wait_for("label:View details")?;
    app.press("label:View details")
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
    app.wait_for("No categories yet")?;
    app.press("label:New category")?;
    app.wait_for("label:Category name")?;
    app.type_text("label:Category name", "Books")?;
    app.press("label:Create category")?;
    app.wait_for("id:navigation >> label:Books")?;
    app.wait_for_gone("label:Category name", WAIT)?;
    app.wait_for("label:Edit Books")?;
    app.press("id:navigation >> label:Books")?;
    app.wait_for("This category is empty")?;
    app.press("label:Add files")?;
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
    grid_details(&mut app, "label:Autumn catalogue")?;
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
        ("Tags", "label:New tag"),
        ("Authors", "label:New author"),
        ("Prompts", "label:New prompt"),
        ("Schemas", "label:Edit Novel"),
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
    app.press("label:New category")?;
    app.wait_for("label:Category name")?;
    app.type_text("label:Category name", "Ledgers")?;
    app.press("label:Create category")?;
    app.wait_for("id:navigation >> label:Ledgers")?;
    app.press("id:navigation >> label:Ledgers")?;
    app.wait_for("This category is empty")?;
    app.press("label:Add files")?;
    app.drop_files(
        "label:Analyze selected files",
        vec![url::Url::from_file_path(&source).unwrap().to_string()],
    )?;
    app.press("label:Analyze selected files")?;
    app.wait_for("label:Import this file")?;
    app.press("label:Import this file")?;
    app.wait_for("Imported")?;
    app.press("id:navigation >> label:Ledgers")?;
    grid_details(&mut app, "label:Winter ledger.txt")?;
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

/// Scrolls `scroller` down until `target` shows (rows load as a list
/// nears its end) or it can scroll no further.
fn scroll_until(app: &mut Headless, scroller: &str, target: &str) -> Result<(), String> {
    for _ in 0..60 {
        if app.screen().count(target)? > 0 {
            return Ok(());
        }
        if app.scroll(scroller, px(400.)).is_err() {
            break;
        }
        app.wait(Duration::from_millis(50))?;
    }
    app.wait_for(target)
}

/// True if the scroller moved. Reaching an edge is the expected alternative.
fn scroll_forward(app: &mut Headless, scroller: &str, points: f32) -> Result<bool, String> {
    match app.scroll(scroller, px(points)) {
        Ok(()) => Ok(true),
        Err(e) if e.contains("cannot scroll by") => Ok(false),
        Err(e) => Err(e),
    }
}

#[test]
fn the_shelf_loads_more_files_as_it_scrolls() -> Result<(), String> {
    let dir = tempfile::tempdir().unwrap();
    let names: Vec<String> = (1..=75).map(|n| format!("File {n:02}")).collect();
    let config = shelf(dir.path(), names);
    let mut app = window(&config);
    app.wait_for("75 files")?;
    app.wait_for("label:File 01")?;
    // The first chunk holds 60 files; the rest come as the grid nears its
    // end.
    scroll_until(&mut app, "id:shelf", "label:File 75")?;
    // Headless matches pre-rendered cells as well. Move to the end so the
    // last cover is in the actual viewport before opening it.
    scroll_forward(&mut app, "id:shelf", 1200.)?;
    app.press("label:File 75")?;
    app.wait_for("Text of File 75")?;
    app.press("label:Back to library")?;
    app.wait_for("id:shelf >> label:File 75")?;
    // The list is a table that loads the same way.
    app.press("label:List")?;
    app.wait_for("id:table:Files")?;
    app.wait_for("local · available")?;
    scroll_until(&mut app, "id:table:Files", "File 75")?;
    scroll_forward(&mut app, "id:table:Files", 1200.)?;
    // Return to the same loaded table row without scrolling again.
    app.press("File 75")?;
    app.wait_for("Text of File 75")?;
    app.press_key("mod+[")?;
    app.wait_for("id:table:Files >> File 75")?;
    // The saved first row may have been partly clipped. The end must still
    // be within one 44-point row of the restored offset.
    scroll_forward(&mut app, "id:table:Files", 44.)?;
    assert!(!scroll_forward(&mut app, "id:table:Files", 1.)?);
    Ok(())
}

#[test]
fn opens_from_grid_table_group_and_detail_and_restores_shelf() -> Result<(), String> {
    let dir = tempfile::tempdir().unwrap();
    let mut names: Vec<_> = (1..=75).map(|n| format!("Saga {n:02}")).collect();
    names[2] = "Saga 03 Lantern".into();
    names.reverse(); // Database IDs run in the opposite order to this group.
    names.push("Map.pdf".into());
    let config = shelf(dir.path(), names);
    let mut app = window(&config);
    app.wait_for("label:Map.pdf")?;
    app.press("label:Map.pdf")?;
    app.wait_for("label:Save file")?;
    assert_eq!(app.screen().input_text("label:Title")?, Some("Map.pdf"));
    app.press("label:Back to library")?;

    app.press("id:navigation >> label:Shelf")?;
    app.wait_for("label:View")?;
    app.press("label:View")?;
    app.press("label:By title")?;
    app.wait_for("label:Saga")?;
    app.press("label:List")?;
    app.wait_for("id:table:Groups >> Saga")?;
    app.press("id:table:Groups >> Saga")?;
    let last = "id:table:Files >> Saga 75";
    app.wait_for("id:table:Files >> Saga 01")?;
    scroll_until(&mut app, "id:table:Files", last)?;
    scroll_forward(&mut app, "id:table:Files", 1200.)?;
    app.press(last)?;
    app.wait_for("Text of Saga 75")?;
    app.press("label:Back to library")?;
    app.wait_for("label:All groups")?;
    app.wait_for(last)?;

    // Right-click a different row after a primary press opened the book.
    let previous = "id:table:Files >> Saga 74";
    app.right_click(previous)?;
    app.choose_menu_item("View details")?;
    app.wait_for("label:Open file")?;
    assert_eq!(app.screen().input_text("label:Title")?, Some("Saga 74"));
    app.press("label:Open file")?;
    app.wait_for("Text of Saga 74")?;
    app.press_key("mod+[")?;
    app.wait_for(previous)?;

    app.type_text("label:Search library", "Lantern")?;
    app.submit("label:Search library")?;
    app.wait_for("id:table:Files >> Saga 03 Lantern")?;
    app.wait_for_gone("label:All groups", WAIT)?;
    app.press("label:Lucky")?;
    let pick = "label:Lucky picks >> label:Saga 03 Lantern";
    app.wait_for(pick)?;
    app.press(pick)?;
    app.wait_for("Text of Saga 03 Lantern")?;
    app.press_key("mod+[")?;
    app.wait_for("id:table:Files >> Saga 03 Lantern")?;
    assert_eq!(
        app.screen().input_text("label:Search library")?,
        Some("Lantern")
    );
    app.press("label:Clear")?;
    app.wait_for("id:table:Files >> Saga 01")?;
    app.wait_for("label:All groups")?;
    app.press("id:navigation >> label:All files")?;
    app.wait_for_gone("label:All groups", WAIT)?;
    app.wait_for("id:table:Files >> Map.pdf")?;
    app.press("id:table:Files >> Map.pdf")?;
    app.wait_for("label:Save file")
}

#[test]
fn selecting_and_favoriting_never_open_a_book() -> Result<(), String> {
    let dir = tempfile::tempdir().unwrap();
    let config = shelf(dir.path(), vec!["Alpha".into(), "Beta".into()]);
    let mut app = window(&config);
    app.wait_for("label:Alpha")?;
    app.press("label:Select files")?;
    app.press("label:Alpha")?;
    app.wait_for("1 selected")?;
    app.press("label:Select Alpha")?;
    app.wait_for("0 selected")?;
    app.press("label:Favorite >> nth=0")?;
    app.wait_for("label:Unfavorite")?;
    app.wait_for("0 selected")?;
    app.press("label:List")?;
    app.wait_for("id:table:Files >> Beta")?;
    app.press("id:table:Files >> Alpha")?;
    app.wait_for("1 selected")?;
    app.press("id:table:Files >> label:Unfavorite")?;
    app.wait_for_gone("label:Unfavorite", WAIT)?;
    app.wait_for("1 selected")?;
    app.press("label:Lucky")?;
    app.wait_for("label:Lucky picks >> label:Beta")?;
    app.press("label:Lucky picks >> label:Beta")?;
    app.wait_for("2 selected")?;

    // The row named by the hit owns its menu even with other rows selected.
    app.right_click("id:table:Files >> Title")?;
    assert!(app.menu().is_none());
    app.right_click("id:table:Files")?;
    assert!(app.menu().is_none());
    app.right_click("id:table:Files >> Beta")?;
    app.choose_menu_item("View details")?;
    app.wait_for("label:Save file")?;
    assert_eq!(app.screen().input_text("label:Title")?, Some("Beta"));
    Ok(())
}

#[test]
fn tags_load_more_and_edit_in_a_dialog() -> Result<(), String> {
    use biblio_core::commands::tag;
    use nsh_testing::DialogAnswer;
    let profile = tempfile::tempdir().unwrap();
    seed(profile.path(), |db| async move {
        for n in 1..=130 {
            tag::tag_create(db.clone(), format!("Tag {n:03}"), None).await?;
        }
        Ok(())
    });
    let mut app = window(profile.path());
    app.wait_for("Your library is empty")?;
    app.press("label:Tags")?;
    app.wait_for("100 of 130 tags")?;
    scroll_until(&mut app, "id:table:Tags", "Tag 130")?;
    app.wait_for("130 tags")?;
    // The pencil opens the record in a dialog.
    app.press("label:Edit Tag 130")?;
    app.wait_for("label:Name")?;
    app.clear("label:Name")?;
    app.type_text("label:Name", "Tag 999")?;
    app.press("label:Save")?;
    app.wait_for("Tag 999")?;
    app.wait_for_gone("label:Name", WAIT)?;
    // The bin asks first.
    app.on_dialog(|d| {
        assert!(d.title.as_deref().unwrap_or_default().contains("Tag 999"));
        DialogAnswer::Press("Delete".into())
    });
    app.press("label:Delete Tag 999")?;
    app.wait_for("129 tags")?;
    // A new one comes from the header.
    app.press("label:New tag")?;
    app.wait_for("label:Name")?;
    app.type_text("label:Name", "Tag 000")?;
    app.press("label:Create")?;
    app.wait_for("130 tags")?;
    app.wait_for("Tag 000")
}

#[test]
fn grouped_view_opens_a_group_and_selects_what_is_shown() -> Result<(), String> {
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
    app.wait_for("34 files")?;
    app.press("id:navigation >> label:Shelf")?;
    app.wait_for("label:View")?;
    app.press("label:View")?;
    app.press("label:By title")?;
    app.wait_for("label:Saga")?;
    app.press("label:Select files")?;
    // The groups themselves are not files.
    app.press("label:Select all shown")?;
    app.wait_for("0 selected")?;
    app.press("label:Saga")?;
    app.wait_for("label:Saga 01")?;
    app.wait_for("label:All groups")?;
    app.press("label:Select all shown")?;
    app.wait_for("31 selected")?;
    app.press("label:All groups")?;
    app.wait_for("label:Pair")?;
    app.press("label:Pair")?;
    app.wait_for("label:Pair 2")?;
    app.press("label:Select all shown")?;
    app.wait_for("2 selected")?;
    app.press("id:navigation >> label:Empty")?;
    app.wait_for("This category is empty")?;
    app.press("label:Select all shown")?;
    app.wait_for("0 selected")
}
