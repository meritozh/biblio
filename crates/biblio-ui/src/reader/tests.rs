use super::{navigation::OpenServices, session::Scanner};
use crate::{ReaderWrites, app, desktop_root};
use biblio_core::{
    Library,
    commands::{
        cache::{CopyState, ExternalOpener},
        file::create_replace::{file_create, file_get},
        settings::settings_set,
    },
    text_reader::{
        book::{ReadingPosition, TextBook},
        rules,
        store::{BookEdit, Preference, ReaderStore, TxtOpener},
    },
};
use nsh_core::{
    Command, EventKind, EventPayload, PayloadValue, ROOT_ID,
    desktop::{DesktopCommand, MenuItem},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;
use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

const WAIT: Duration = Duration::from_secs(8);
struct Books {
    _dir: tempfile::TempDir,
    profile: PathBuf,
    paths: Vec<(i64, PathBuf)>,
}
impl Books {
    fn new(books: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("config");
        let storage = dir.path().join("storage");
        std::fs::create_dir(&storage).unwrap();
        let paths = with_db(&profile, async |db| {
            settings_set(
                db.clone(),
                "storage_path".into(),
                storage.display().to_string(),
            )
            .await
            .unwrap();
            let category = biblio_core::commands::category::category_create(
                db.clone(),
                "Shelf".into(),
                None,
                "novel".into(),
                None,
            )
            .await
            .unwrap()
            .id;
            let mut out = Vec::new();
            for (name, content) in books {
                let p = dir.path().join(format!("{name}.txt"));
                std::fs::write(&p, content).unwrap();
                let id = file_create(
                    db.clone(),
                    p.display().to_string(),
                    name.to_string(),
                    Some(category),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .await
                .unwrap()
                .id;
                let p = file_get(db.clone(), id).await.unwrap().path;
                out.push((id, p.into()));
            }
            out
        });
        Self {
            _dir: dir,
            profile,
            paths,
        }
    }
    fn window(&self, install: impl Fn() + Send + Sync + 'static) -> (Headless, ReaderWrites) {
        let (root, writer) = desktop_root(&self.profile, move |path, writes| {
            install();
            app::start(path, writes)
        })
        .unwrap();
        let config = EngineConfig {
            viewport: size(px(1100.), px(760.)),
            environment: nsh_core::platform::Environment {
                os: nsh_core::platform::Os::MacOs,
                ..Default::default()
            },
            ..EngineConfig::new(root)
        };
        (Headless::start(config).unwrap(), writer)
    }
    fn saved(&self) -> ReadingPosition {
        with_db(&self.profile, async |db| {
            ReaderStore::new(db)
                .load(Some(self.paths[0].0))
                .await
                .unwrap()
                .book
                .unwrap()
                .checkpoint
                .unwrap()
                .at
        })
    }
}
fn with_db<T>(path: &Path, action: impl AsyncFnOnce(Library) -> T) -> T {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let out = runtime.block_on(async {
        let db = Library::open(path.to_owned(), |_, _| {}).await.unwrap();
        action(db).await
    });
    runtime.shutdown_timeout(Duration::from_secs(1));
    out
}

fn view_details(window: &mut Headless, name: &str) -> Result<(), String> {
    let card = format!("label:{name}");
    window.wait_for(&card)?;
    window.right_click(&card)?;
    window.wait_for("label:View details")?;
    window.press("label:View details")?;
    window.wait_for("label:Open file")
}

fn open(window: &mut Headless, name: &str) -> Result<(), String> {
    view_details(window, name)?;
    window.press("label:Open file")
}

fn menu_action(window: &mut Headless, name: &str) -> Result<(), String> {
    let items = window
        .screen()
        .commands()
        .iter()
        .rev()
        .find_map(|(_, c)| match c {
            Command::Desktop(c) => match c.as_ref() {
                DesktopCommand::SetMenu(Some(v)) => Some(v),
                _ => None,
            },
            _ => None,
        })
        .ok_or("No application menu")?;
    let id = items
        .iter()
        .find_map(|i| match i {
            MenuItem::Submenu { items, .. } => items.iter().find_map(|i| match i {
                MenuItem::Action(a) if a.label == name => Some(a.id),
                _ => None,
            }),
            _ => None,
        })
        .ok_or("No matching command")?;
    window.engine().control().dispatch_event(
        ROOT_ID,
        EventKind::Menu,
        EventPayload::Map(vec![("id", PayloadValue::Number(id.into()))]),
    );
    Ok(())
}

#[test]
fn find_targets_the_active_reader() -> Result<(), String> {
    let books = Books::new(&[("Book", "sentence for search")]);
    let (mut w, _) = books.window(|| {});
    open(&mut w, "Book")?;
    w.wait_for("sentence for search")?;
    menu_action(&mut w, "Find")?;
    w.wait_for("label:Find text")?;
    w.press_key("x")?;
    assert_eq!(w.screen().input_text("label:Find text")?, Some("x"));
    w.press_key("space")?;
    w.press_key("left")?;
    assert_eq!(w.screen().input_text("label:Find text")?, Some("x "));
    w.press("label:Close find")?;
    w.press_key("mod+f")?;
    w.wait_for("label:Find text")?;
    w.press_key("mod+k")?;
    w.wait_for("label:Search library")
}

#[test]
fn finds_literal_across_windows_and_reveals_each_hit() -> Result<(), String> {
    let mut contents = "ordinary line\n".repeat(2047);
    contents.push_str("left needle\nbridge first\n");
    contents.push_str(&"ordinary line\n".repeat(3000));
    contents.push_str("right needle\nbridge second\n");
    contents.push_str(&"last line\n".repeat(50));
    let books = Books::new(&[("Book", &contents)]);
    let (mut w, _) = books.window(|| {});
    open(&mut w, "Book")?;
    w.wait_for("id:reader-viewport")?;
    w.press_key("mod+f")?;
    w.wait_for("label:Find text")?;
    w.type_text("label:Find text", "needle\nbridge")?;
    for (action, prefix, suffix) in [
        (None, "left needle", "bridge first"),
        (Some("Next match"), "right needle", "bridge second"),
        (Some("Previous match"), "left needle", "bridge first"),
    ] {
        if let Some(name) = action {
            w.press(&format!("label:{name}"))?;
        }
        w.wait_for(prefix)?;
        w.wait_for(suffix)?;
        w.wait_until("both source ranges highlighted", WAIT, |screen| {
            [prefix, suffix]
                .iter()
                .zip(["needle", "bridge"])
                .all(|(line, part)| {
                    screen
                        .span_style(&format!("text:{line} >> text:/{part}/"))
                        .is_ok_and(|s| s.background.is_some())
                })
        })?;
    }
    w.clear("label:Find text")?;
    w.wait_for("bridge first")?;
    Ok(())
}

#[test]
fn keeps_visible_passage_across_reflow_and_screen_moves() -> Result<(), String> {
    let content = (0..600)
        .map(|n| format!("Passage {n:04} with a sentence that can wrap across the window.\n"))
        .collect::<String>();
    let books = Books::new(&[("Book", &content)]);
    let (mut w, writes) = books.window(|| {});
    open(&mut w, "Book")?;
    w.wait_for("id:reader-viewport")?;
    w.press("label:Book text")?;
    for _ in 0..5 {
        w.press_key("pagedown")?;
    }
    // Drop immediately. The saved position must refer to a passage reached
    // by the screen commands, not the first paragraph.
    drop(w);
    writes.drain()?;
    let at = books.saved();
    assert!(at.paragraph > 1000, "saved {at:?}");
    let line = content[at.paragraph as usize..].lines().next().unwrap();
    let (mut w, _) = books.window(|| {});
    open(&mut w, "Book")?;
    w.wait_for(line)?;
    w.set_viewport(size(px(680.), px(720.)))?;
    w.wait_for(line)?;
    drop(w);
    assert_eq!(books.saved().paragraph, at.paragraph);
    Ok(())
}

#[test]
fn reader_actions_and_theme_stay_within_scope() -> Result<(), String> {
    let books = Books::new(&[(
        "Book",
        "Chapter 1\n  original words\r\n\n\t\nChapter 2\nEnding",
    )]);
    let original = std::fs::read(&books.paths[0].1).unwrap();
    let (mut w, _) = books.window(|| {});
    open(&mut w, "Book")?;
    w.wait_for("2 chapters")?;
    w.press("label:Contents")?;
    w.wait_for("label:Chapter 2 · 6")?;
    w.press("label:Chapter 2 · 6")?;
    w.wait_for("Ending")?;
    assert!(!w.shows("Bookmark") && !w.shows("Reader theme"));
    let style = w.screen().span_style("text:Ending")?;
    assert_eq!(style.font_size, px(18.));
    assert_eq!(style.line_height, px(18. * 1.6));
    w.set_environment("switch to dark appearance", |e| {
        e.window = size(px(1100.), px(760.));
        e.color_scheme = nsh_core::platform::ColorScheme::Dark
    })?;
    w.wait_until("book text follows the dark appearance", WAIT, |s| {
        s.span_style("text:Ending")
            .is_ok_and(|dark| dark.color != style.color)
    })?;
    drop(w);
    assert_eq!(std::fs::read(&books.paths[0].1).unwrap(), original);
    Ok(())
}

struct Launcher(Arc<Mutex<Vec<(PathBuf, String)>>>);
impl ExternalOpener for Launcher {
    fn launch(&self, path: &Path, program: Option<&str>) -> Result<(), String> {
        self.0
            .lock()
            .unwrap()
            .push((path.to_owned(), program.unwrap().into()));
        Ok(())
    }
}

#[test]
fn retry_after_failure_never_launches_an_abandoned_open() -> Result<(), String> {
    let books = Books::new(&[("Book", "A local copy")]);
    let path = books.paths[0].1.clone();
    with_db(&books.profile, async |db| {
        ReaderStore::new(db)
            .preference(Preference::Opener(TxtOpener::External(
                "Example Reader".into(),
            )))
            .await
            .unwrap()
    });
    let calls = Arc::new(Mutex::new(Vec::new()));
    let launched = calls.clone();
    let (tx, _) = watch::channel(CopyState::Queued);
    let source = tx.clone();
    let attempts = Arc::new(AtomicUsize::new(0));
    let requests = attempts.clone();
    let (mut w, _) = books.window(move || {
        let tx = source.clone();
        let requests = requests.clone();
        provide_context(OpenServices {
            readable: Rc::new(move |_, _| {
                requests.fetch_add(1, Ordering::SeqCst);
                let rx = tx.subscribe();
                Box::pin(async move { Ok(rx) })
            }),
            launcher: Rc::new(Launcher(launched.clone())),
        });
    });
    open(&mut w, "Book")?;
    w.wait_for("Download queued…")?;
    tx.send_replace(CopyState::Failed("Provider unavailable".into()));
    w.wait_for("Provider unavailable")?;
    tx.send_replace(CopyState::Downloading);
    w.press("label:Retry")?;
    w.wait_for("Downloading file…")?;
    w.press("label:Back to library")?;
    w.wait_for("label:Search library")?;
    tx.send_replace(CopyState::Ready(path.clone()));
    view_details(&mut w, "Book")?;
    assert!(calls.lock().unwrap().is_empty());
    w.press("label:Back to library")?;
    w.wait_for("label:Book")?;
    w.press("label:Book")?;
    w.wait_until("the active open launches exactly once", WAIT, |s| {
        calls.lock().unwrap().len() == 1 && s.matches("id:shelf >> label:Book").unwrap_or(false)
    })?;
    assert_eq!(
        *calls.lock().unwrap(),
        vec![(path, "Example Reader".into())]
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    Ok(())
}

// A gate at the actual survey service boundary. The worker deliberately
// returns a late error after cancellation to exercise owner/result rejection.
fn gated(
    books: &Books,
) -> (
    Headless,
    mpsc::Receiver<()>,
    mpsc::Sender<()>,
    mpsc::Receiver<bool>,
) {
    let (started, rx) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let (finished, end) = mpsc::channel();
    let gate = Arc::new(Mutex::new(gate));
    let first = Arc::new(AtomicUsize::new(0));
    let (w, _) = books.window(move || {
        let (first, started, gate, finished) = (
            first.clone(),
            started.clone(),
            gate.clone(),
            finished.clone(),
        );
        provide_context(Scanner(Arc::new(move |book, rules, stop| {
            if first.fetch_add(1, Ordering::SeqCst) == 0 {
                started.send(()).unwrap();
                let _ = gate.lock().unwrap().recv_timeout(Duration::from_secs(25));
                finished.send(stop.is_cancelled()).ok();
                Err("abandoned directory".into())
            } else {
                rules::survey(book, rules, stop).map_err(|e| e.to_string())
            }
        })));
    });
    (w, rx, release, end)
}

#[test]
fn reads_and_leaves_while_indexer_is_blocked_at_50_mib() -> Result<(), String> {
    const SIZE: u64 = 50 * 1024 * 1024;
    const MIDDLE: u64 = 32 * 1024 * 1024;
    let books = Books::new(&[("Long", ""), ("Short", "Chapter 1\nSecond book")]);
    let path = &books.paths[0].1;
    let mut file = File::create(path).unwrap();
    // Regular 128-byte paragraphs make both anchors exact source boundaries.
    let mut block = vec![b'x'; 64 * 1024];
    for i in (127..block.len()).step_by(128) {
        block[i] = b'\n';
    }
    for _ in 0..SIZE / block.len() as u64 {
        file.write_all(&block).unwrap();
    }
    for (offset, mark) in [(0, "First book begins"), (MIDDLE, "Lower half begins")] {
        file.seek(SeekFrom::Start(offset)).unwrap();
        file.write_all(format!("{mark}\n").as_bytes()).unwrap();
    }
    drop(file);
    for (at, mark) in [(0, "First book begins"), (MIDDLE, "Lower half begins")] {
        if at > 0 {
            let book =
                TextBook::open(File::open(path).unwrap(), None, &CancellationToken::new()).unwrap();
            let cp = book.checkpoint(ReadingPosition {
                paragraph: at,
                restart: at,
            });
            with_db(&books.profile, async |db| {
                ReaderStore::new(db)
                    .edit_book(books.paths[0].0, BookEdit::Position(cp.clone()))
                    .await
                    .unwrap()
            });
        }
        let (mut w, started, release, ended) = gated(&books);
        open(&mut w, "Long")?;
        w.wait_for(mark)?;
        started.recv_timeout(WAIT).unwrap();
        w.press("label:Book text")?;
        w.press_key("space")?;
        w.press_key("mod+[")?;
        open(&mut w, "Short")?;
        w.wait_for("Second book")?;
        release.send(()).unwrap();
        assert!(ended.recv_timeout(WAIT).unwrap(), "old scanner cancelled");
        w.wait_for("1 chapter")?;
        assert!(!w.shows("abandoned directory"));
    }
    Ok(())
}

#[test]
fn superseded_results_never_replace_current_contents() -> Result<(), String> {
    let books = Books::new(&[
        ("First", "Chapter 1\nfirst body"),
        ("Second", "Chapter 1\nA\nChapter 2\nB"),
    ]);
    let (mut w, started, release, ended) = gated(&books);
    open(&mut w, "First")?;
    w.wait_for("first body")?;
    started.recv_timeout(WAIT).unwrap();
    menu_action(&mut w, "Back to Library")?;
    open(&mut w, "Second")?;
    w.wait_for("2 chapters")?;
    release.send(()).unwrap();
    assert!(ended.recv_timeout(WAIT).unwrap());
    w.press("label:Contents")?;
    w.wait_for("label:Chapter 2 · 1")?;
    assert!(!w.shows("abandoned directory"));
    Ok(())
}

#[test]
fn reads_through_empty_projected_windows_and_finds_inside_a_long_paragraph() -> Result<(), String> {
    let long = format!("{}🌿NEEDLE{}", "庭".repeat(110_000), "中".repeat(25_000));
    let source = format!(
        "First paragraph\n{}After blank run\n{long}\nEnd",
        "\n".repeat(8000)
    );
    let books = Books::new(&[("Fragments", &source)]);
    let (mut w, _) = books.window(|| {});
    open(&mut w, "Fragments")?;
    w.wait_for("First paragraph")?;
    w.wait_for("After blank run")?;
    w.press_key("mod+f")?;
    w.wait_for("label:Find text")?;
    w.type_text("label:Find text", "🌿NEEDLE")?;
    w.wait_until(
        "the long paragraph's match is styled in a bounded native text element",
        WAIT,
        |s| {
            s.span_style("id:reader-viewport >> text:/🌿NEEDLE/")
                .is_ok_and(|style| style.background.is_some())
        },
    )?;
    assert!(w.screen().texts().iter().all(|t| t.len() <= 32 * 1024));
    Ok(())
}

#[test]
fn reopening_in_the_same_window_waits_for_the_final_position() -> Result<(), String> {
    let source = (0..500)
        .map(|i| format!("Ordered passage {i:03}\n"))
        .collect::<String>();
    let books = Books::new(&[("Ordered", &source)]);
    let (mut w, writes) = books.window(|| {});
    open(&mut w, "Ordered")?;
    w.wait_for("Ordered passage 000")?;
    let (release, gate) = tokio::sync::oneshot::channel::<()>();
    drop(writes.ordered(async move {
        let _ = gate.await;
    }));
    w.press_key("mod+f")?;
    w.wait_for("label:Find text")?;
    w.type_text("label:Find text", "Ordered passage 350")?;
    w.wait_until("jump to passage 350", WAIT, |s| {
        s.span_style("id:reader-viewport >> text:Ordered passage 350")
            .is_ok_and(|style| style.background.is_some())
    })?;
    w.press("label:Book text")?;
    w.scroll("id:reader-viewport", px(180.))?;
    w.press_key("mod+[")?;
    open(&mut w, "Ordered")?;
    w.wait_for("Loading text…")?;
    release.send(()).unwrap();
    w.wait_for("id:reader-viewport")?;
    w.wait_until("the last position, past the opening page", WAIT, |s| {
        s.texts().iter().any(|t| {
            t.strip_prefix("Ordered passage ")
                .and_then(|n| n.parse::<usize>().ok())
                .is_some_and(|n| n >= 350)
        })
    })?;
    drop(w);
    assert!(books.saved().restart >= "Ordered passage 000\n".len() as u64 * 350);
    Ok(())
}
mod controls;
