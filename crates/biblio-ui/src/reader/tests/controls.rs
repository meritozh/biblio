use super::*;
use crate::reader::{contents::Outliner, fonts::FontFamilies};
use biblio_core::text_reader::{
    book::Encoding,
    rules::ChapterRule,
    store::{BookRule, DefaultRule, ReaderState, Typography},
};

fn state(books: &Books) -> ReaderState {
    with_db(&books.profile, async |db| {
        ReaderStore::new(db)
            .load(Some(books.paths[0].0))
            .await
            .unwrap()
    })
}

fn settings(w: &mut Headless) -> Result<(), String> {
    w.wait_for("label:Settings")?;
    w.press("label:Settings")?;
    w.wait_for("label:Reader")?;
    w.press("label:Reader")?;
    w.wait_for("label:Reset typography")
}

fn book_rules(w: &mut Headless) -> Result<(), String> {
    w.press("label:Reading settings")?;
    w.press("label:Chapter rules")?;
    w.wait_for("label:New rule")
}

fn click(w: &mut Headless, label: &str) -> Result<(), String> {
    let selector = format!("label:{label}");
    w.wait_for(&selector)?;
    w.press(&selector)
}

#[test]
fn settings_and_reader_share_type_and_reset_only_typography() -> Result<(), String> {
    let text = format!(
        "Start\n\n  \n\t\n  Line with indent\n{}",
        (0..320)
            .map(|i| format!("Marker {i:03}\n"))
            .collect::<String>()
    );
    let books = Books::new(&[("Book", &text)]);
    let original = std::fs::read(&books.paths[0].1).unwrap();
    let (mut w, writes) =
        books.window(|| provide_context(FontFamilies(vec!["Test Reader Serif".into()].into())));
    settings(&mut w)?;
    click(&mut w, "External application")?;
    click(&mut w, "Save external application")?;
    w.wait_for("Choose an external application")?;
    w.type_text("label:Application name or path", "Example Reader")?;
    click(&mut w, "Save external application")?;
    click(&mut w, "General")?;
    click(&mut w, "Reader")?;
    w.wait_for("label:Application name or path")?;
    assert_eq!(
        w.screen().input_text("label:Application name or path")?,
        Some("Example Reader")
    );
    click(&mut w, "Text Reader")?;
    click(&mut w, "Font family")?;
    w.press("text:Test Reader Serif")?;
    click(&mut w, "Increase font size")?;
    click(&mut w, "Increase line height")?;
    click(&mut w, "Increase letter spacing")?;
    click(&mut w, "Collapse blank lines")?;
    click(&mut w, "All files")?;
    open(&mut w, "Book")?;
    w.wait_for("  Line with indent")?;
    let style = w.screen().span_style("text:Start")?;
    assert_eq!(style.font_size, px(19.));
    assert_eq!(style.line_height, px(19. * 1.7));
    assert_eq!(style.letter_spacing, px(0.25));
    assert_eq!(
        style.font_family.unwrap().name().as_ref(),
        "Test Reader Serif"
    );
    assert!(
        w.screen().texts().contains(&"\t"),
        "uncollapsed whitespace is visible"
    );
    w.press_key("mod+f")?;
    w.type_text("label:Find text", "Marker 200")?;
    w.wait_for("Marker 200")?;
    click(&mut w, "Close find")?;
    click(&mut w, "Reading settings")?;
    click(&mut w, "Increase font size")?;
    w.wait_until("same passage enlarges", WAIT, |s| {
        s.span_style("text:Marker 200")
            .is_ok_and(|style| style.font_size == px(20.))
    })?;
    click(&mut w, "Reset typography")?;
    w.wait_until("same passage uses default metrics", WAIT, |s| {
        s.span_style("text:Marker 200").is_ok_and(|s| {
            s.font_size == px(18.)
                && s.line_height == px(18. * 1.6)
                && s.letter_spacing == px(0.)
                && s.font_family.is_none()
        })
    })?;
    assert_eq!(
        w.screen()
            .accessibility_state("label:Collapse blank lines")?
            .checked,
        Some(nsh_core::Checked::False)
    );
    click(&mut w, "Collapse blank lines")?;
    w.wait_for("Marker 200")?;
    drop(w);
    writes.drain()?;
    let saved = state(&books);
    assert_eq!(saved.typography, Typography::default());
    assert!(saved.collapse_blanks);
    let anchor = saved.book.unwrap().checkpoint.unwrap().at;
    assert_eq!(
        text[anchor.paragraph as usize..].lines().next(),
        Some("Marker 200")
    );
    assert_eq!(std::fs::read(&books.paths[0].1).unwrap(), original);
    Ok(())
}

#[test]
fn preview_cancel_apply_and_set_default_have_distinct_effects() -> Result<(), String> {
    let books = Books::new(&[("Book", "Intro\nChapter 1\nDogs\n1\nCat\n2\nB")]);
    let (mut w, writes) = books.window(|| {});
    open(&mut w, "Book")?;
    w.wait_for("1 chapter")?;
    book_rules(&mut w)?;
    w.wait_for("Detected · 英文章节")?;
    w.wait_for("第一回 风起")?;
    click(&mut w, "纯数字")?;
    w.wait_for("Preview · 2 chapters")?;
    w.wait_for("1 · 3")?;
    w.wait_for("2 · 1")?;
    click(&mut w, "Cancel selection")?;
    click(&mut w, "Close")?;
    click(&mut w, "Contents")?;
    w.wait_for("label:Chapter 1 · 10")?;
    click(&mut w, "Contents")?;
    click(&mut w, "Chapter rules")?;
    click(&mut w, "纯数字")?;
    click(&mut w, "Apply to this book")?;
    w.wait_for("Default · 自动检测   This book · 纯数字")?;
    click(&mut w, "特殊符号")?;
    click(&mut w, "Set as default")?;
    w.wait_for("Default · 特殊符号   This book · 纯数字")?;
    click(&mut w, "Close")?;
    click(&mut w, "Contents")?;
    w.wait_for("label:2 · 1")?;
    drop(w);
    writes.drain()?;
    let saved = state(&books);
    assert_eq!(saved.default_rule, DefaultRule::Fixed("symbols".into()));
    assert_eq!(saved.book.unwrap().rule, BookRule::Fixed("digits".into()));
    Ok(())
}

#[test]
fn settings_edit_and_test_rules_then_delete_an_applied_default() -> Result<(), String> {
    let books = Books::new(&[("Book", "Volume 1\nAct 1\nA\nAct 2\nB\nChapter 1\nC")]);
    let (mut w, writes) = books.window(|| {});
    settings(&mut w)?;
    click(&mut w, "Chapter rules")?;
    click(&mut w, "英文章节")?;
    assert!(!w.shows("Apply to this book") && !w.shows("Edit rule") && !w.shows("Delete rule"));
    click(&mut w, "Duplicate rule")?;
    assert!(
        w.screen()
            .input_text("label:Chapter expression")?
            .unwrap()
            .contains("chapter")
    );
    click(&mut w, "Cancel edit")?;
    click(&mut w, "New rule")?;
    w.type_text("label:Rule name", "Acts")?;
    w.type_text("label:Chapter expression", "Act ")?;
    click(&mut w, "{序号}")?;
    w.type_text("label:Volume expression", "Volume ")?;
    click(&mut w, "{序号}")?;
    for (sample, result) in [
        ("Act 1", "Chapter"),
        ("Volume 9", "Volume"),
        ("Words", "No match"),
    ] {
        w.clear("label:Example heading")?;
        w.type_text("label:Example heading", sample)?;
        click(&mut w, "Test example")?;
        w.wait_for(&format!("Example · {result}"))?;
    }
    click(&mut w, "Save and set default")?;
    w.wait_for("Default · Acts")?;
    click(&mut w, "Edit rule")?;
    w.clear("label:Rule name")?;
    w.type_text("label:Rule name", "Scene")?;
    click(&mut w, "Regular expression")?;
    w.clear("label:Volume expression")?;
    w.clear("label:Chapter expression")?;
    w.type_text("label:Chapter expression", "(")?;
    click(&mut w, "Save rule")?;
    w.wait_for("text:/Chapter expression:/")?;
    click(&mut w, "Cancel edit")?;
    w.wait_for("Default · Acts")?;
    click(&mut w, "Edit rule")?;
    assert_eq!(w.screen().input_text("label:Rule name")?, Some("Acts"));
    w.clear("label:Rule name")?;
    w.type_text("label:Rule name", "Scene")?;
    click(&mut w, "Regular expression")?;
    w.clear("label:Volume expression")?;
    w.clear("label:Chapter expression")?;
    w.type_text("label:Chapter expression", "^Act [0-9]+$")?;
    w.type_text("label:Volume expression", "^Volume [0-9]+$")?;
    w.type_text("label:Example heading", "Volume 1")?;
    click(&mut w, "Test example")?;
    w.wait_for("Example · Volume")?;
    click(&mut w, "Save rule")?;
    w.wait_for("Default · Scene")?;
    click(&mut w, "Close")?;
    click(&mut w, "All files")?;
    open(&mut w, "Book")?;
    w.wait_for("2 chapters")?;
    book_rules(&mut w)?;
    click(&mut w, "Scene")?;
    w.wait_for("Volume 1 · 0")?;
    click(&mut w, "Apply to this book")?;
    w.wait_for_gone("label:Apply to this book", WAIT)?;
    drop(w);
    writes.drain()?;
    let before = state(&books);
    assert!(matches!(
        before.book.as_ref().unwrap().rule,
        BookRule::Fixed(_)
    ));
    let (mut w, _) = books.window(|| {});
    open(&mut w, "Book")?;
    w.wait_for("2 chapters")?;
    book_rules(&mut w)?;
    click(&mut w, "Scene")?;
    click(&mut w, "Delete rule")?;
    w.wait_for("Default · 自动检测   This book · 自动检测")?;
    click(&mut w, "Close")?;
    w.wait_for("1 chapter")?;
    click(&mut w, "Contents")?;
    w.wait_for("label:Chapter 1 · 1")?;
    drop(w);
    let after = state(&books);
    assert_eq!(after.default_rule, DefaultRule::Auto);
    assert_eq!(after.book.as_ref().unwrap().rule, BookRule::Inherit);
    assert_eq!(
        after.book.unwrap().checkpoint,
        before.book.unwrap().checkpoint
    );
    Ok(())
}

#[test]
fn encoding_preview_cancel_and_commit_preserve_the_source() -> Result<(), String> {
    let books = Books::new(&[("Book", "")]);
    let source: Vec<u8> = "Chapter 1\n新正文"
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect();
    std::fs::write(&books.paths[0].1, &source).unwrap();
    with_db(&books.profile, async |db| {
        ReaderStore::new(db)
            .edit_book(books.paths[0].0, BookEdit::Encoding(Some(Encoding::Gbk)))
            .await
            .unwrap()
    });
    let (mut w, started, release, ended) = gated(&books);
    open(&mut w, "Book")?;
    w.wait_for("id:reader-viewport")?;
    started.recv_timeout(WAIT).unwrap();
    click(&mut w, "Book encoding")?;
    click(&mut w, "UTF-16 BE")?;
    w.wait_for("text:/新正文/")?;
    click(&mut w, "Cancel encoding")?;

    assert!(!w.shows("新正文"));
    click(&mut w, "Book encoding")?;
    w.wait_for("Current encoding · GBK")?;
    click(&mut w, "UTF-16 BE")?;
    w.wait_for("text:/新正文/")?;
    click(&mut w, "Confirm encoding")?;
    w.wait_for("id:reader-viewport >> text:新正文")?;
    w.wait_for("1 chapter")?;
    release.send(()).unwrap();
    assert!(ended.recv_timeout(WAIT).unwrap());
    click(&mut w, "Contents")?;
    w.wait_for("label:Chapter 1 · 3")?;
    assert!(!w.shows("abandoned directory"));
    drop(w);
    assert_eq!(
        state(&books).book.unwrap().encoding,
        Some(Encoding::Utf16Be)
    );
    assert_eq!(std::fs::read(&books.paths[0].1).unwrap(), source);
    Ok(())
}

#[test]
fn later_preview_can_finish_before_a_cancelled_outline() -> Result<(), String> {
    let books = Books::new(&[("Book", "Chapter 1\nA\n1\na\n2\nb\n【Title】\nlast")]);
    let (started, rx) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let (done, ended) = mpsc::channel();
    let gate = Arc::new(Mutex::new(gate));
    let (mut w, _) = books.window(move || {
        let (started, gate, done) = (started.clone(), gate.clone(), done.clone());
        provide_context(Outliner(Arc::new(move |book, draft, stop| {
            if draft.name == "纯数字" {
                started.send(()).unwrap();
                let _ = gate.lock().unwrap().recv_timeout(WAIT * 2);
                done.send(stop.is_cancelled()).ok();
                Err("stale preview error".into())
            } else {
                let rule = ChapterRule::compile(draft).map_err(|e| e.to_string())?;
                rules::outline(book, &rule, stop).map_err(|e| e.to_string())
            }
        })));
    });
    open(&mut w, "Book")?;
    w.wait_for("1 chapter")?;
    book_rules(&mut w)?;
    click(&mut w, "纯数字")?;
    rx.recv_timeout(WAIT).unwrap();
    click(&mut w, "特殊符号")?;
    w.wait_for("【Title】 · 4")?;
    release.send(()).unwrap();
    assert!(ended.recv_timeout(WAIT).unwrap());
    click(&mut w, "Apply to this book")?;
    w.wait_for("Default · 自动检测   This book · 特殊符号")?;
    click(&mut w, "Close")?;
    click(&mut w, "Contents")?;
    w.wait_for("label:【Title】 · 4")?;
    assert!(!w.shows("stale preview error"));
    Ok(())
}

#[test]
fn a_missing_family_falls_back_and_blank_changes_reproject_the_visible_text() -> Result<(), String>
{
    let books = Books::new(&[("Book", "Start\n\n \n\t\n  Indented")]);
    with_db(&books.profile, async |db| {
        ReaderStore::new(db)
            .preference(Preference::Type(Typography {
                family: Some("Removed font".into()),
                ..Default::default()
            }))
            .await
            .unwrap();
    });
    let (mut w, _) =
        books.window(|| provide_context(FontFamilies(vec!["Available".into()].into())));
    open(&mut w, "Book")?;
    w.wait_for("  Indented")?;
    assert!(w.screen().span_style("text:Start")?.font_family.is_none());
    assert!(!w.screen().texts().contains(&"\t"));
    click(&mut w, "Reading settings")?;
    w.wait_for("System font")?;
    click(&mut w, "Collapse blank lines")?;
    w.wait_until("whitespace-only lines restored", WAIT, |s| {
        s.texts().contains(&"\t")
    })?;
    click(&mut w, "Collapse blank lines")?;
    w.wait_until("extra blank lines collapsed", WAIT, |s| {
        !s.texts().contains(&"\t") && s.texts().contains(&"  Indented")
    })?;
    click(&mut w, "Font family")?;
    w.press("text:Available")?;
    w.wait_until("replacement font applied", WAIT, |s| {
        s.span_style("text:Start").is_ok_and(|s| {
            s.font_family
                .is_some_and(|f| f.name().as_ref() == "Available")
        })
    })?;
    Ok(())
}

#[test]
fn opening_from_details_observes_an_accepted_settings_edit() -> Result<(), String> {
    let books = Books::new(&[("Book", "Built-in text")]);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let launched = calls.clone();
    let (mut w, writes) = books.window(move || {
        provide_context(OpenServices {
            launcher: Rc::new(Launcher(launched.clone())),
            ..Default::default()
        })
    });
    settings(&mut w)?;
    click(&mut w, "External application")?;
    w.type_text("label:Application name or path", "Example Reader")?;
    let (release, gate) = tokio::sync::oneshot::channel::<()>();
    drop(writes.ordered(async move {
        let _ = gate.await;
    }));
    click(&mut w, "Save external application")?;
    click(&mut w, "All files")?;
    open(&mut w, "Book")?;
    assert!(calls.lock().unwrap().is_empty());
    release.send(()).unwrap();
    w.wait_until("the saved program opens this book", WAIT, |s| {
        s.matches("label:Open file").unwrap_or(false) && calls.lock().unwrap().len() == 1
    })?;
    assert_eq!(
        *calls.lock().unwrap(),
        vec![(books.paths[0].1.clone(), "Example Reader".into())]
    );
    Ok(())
}
