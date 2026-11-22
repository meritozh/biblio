use super::*;
use crate::{
    commands::file::create_replace::file_get,
    text_reader::{
        book::{ReadFrom, TextBook},
        rules::{Heading, PatternMode},
    },
};
use std::{fs::File, future::Future, path::Path};
use tokio_util::sync::CancellationToken;

async fn open(profile: &Path) -> Library {
    Library::open(profile.to_owned(), |_, _| {}).await.unwrap()
}

// A fresh runtime on each launch also drops the old profile's background workers.
fn launch<T>(task: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(task)
}

async fn add_file(db: &Library, path: &Path) -> i64 {
    sqlx::query(
        "INSERT INTO files(path, display_name, progress, is_favorite)
                 VALUES (?, 'Catalog title', 'finished', 1)",
    )
    .bind(path.to_str().unwrap())
    .execute(&db.pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

fn text_book(path: &Path) -> TextBook {
    TextBook::open(File::open(path).unwrap(), None, &CancellationToken::new()).unwrap()
}

fn typography() -> Typography {
    Typography {
        family: Some("Georgia".into()),
        size: 23.5,
        leading: 2.0,
        spacing: 0.75,
    }
}

fn template() -> RuleDraft {
    RuleDraft {
        name: "Parts".into(),
        mode: PatternMode::Template,
        chapter: "Part {序号} {标题}".into(),
        volume: None,
    }
}

fn save(id: Option<&str>, draft: RuleDraft, make_default: bool) -> RuleEdit {
    RuleEdit::Save {
        id: id.map(str::to_owned),
        draft,
        make_default,
    }
}

#[test]
fn reopen_restores_book_offsets_and_global_preferences() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("novel.txt");
    std::fs::write(&path, "Title\nOne passage\nLast passage\n").unwrap();
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH - std::time::Duration::from_millis(1250))
        .unwrap();
    let book = text_book(&path);
    let last = book.checkpoint(ReadingPosition {
        paragraph: 18,
        restart: 23,
    });
    let (file, default_id) = launch(async {
        let db = open(dir.path()).await;
        let file = add_file(&db, &path).await;
        let store = ReaderStore::new(db.clone());
        let initial = store.load(Some(file)).await.unwrap();
        assert_eq!(initial.book, Some(BookState::default()));
        assert_eq!(initial.opener, TxtOpener::BuiltIn);
        assert_eq!(initial.typography, Typography::default());
        assert!(initial.collapse_blanks);
        assert_eq!(initial.default_rule, DefaultRule::Auto);
        assert_eq!(initial.rules, builtin_rules());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reader_books")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(count, 0);

        store
            .edit_book(
                file,
                BookEdit::Position(book.checkpoint(ReadingPosition::default())),
            )
            .await
            .unwrap();
        let id = store
            .edit_rule(save(None, template(), true))
            .await
            .unwrap()
            .unwrap();
        tokio::try_join!(
            store.preference(Preference::Type(typography())),
            store.preference(Preference::Collapse(false)),
            store.preference(Preference::Opener(TxtOpener::External("Text Edit".into()))),
            store.edit_book(file, BookEdit::Rule(BookRule::Auto)),
            store.edit_book(file, BookEdit::Encoding(Some(Encoding::Big5))),
        )
        .unwrap();
        // A late position from the UTF-8 view only owns the checkpoint fields.
        store
            .edit_book(file, BookEdit::Position(last.clone()))
            .await
            .unwrap();
        (file, id)
    });

    launch(async {
        let db = open(dir.path()).await;
        let store = ReaderStore::new(db.clone());
        let global = store.load(None).await.unwrap();
        assert_eq!(global.book, None);
        assert_eq!(global.opener, TxtOpener::External("Text Edit".into()));
        assert_eq!(global.typography, typography());
        assert!(!global.collapse_blanks);
        assert_eq!(global.default_rule, DefaultRule::Fixed(default_id.clone()));
        assert!(global.rules.contains(&SavedRule {
            id: default_id,
            draft: template()
        }));
        let state = store.load(Some(file)).await.unwrap();
        assert_eq!(state.effective_rule(), DefaultRule::Auto);
        let saved = state.book.unwrap();
        assert_eq!(saved.rule, BookRule::Auto);
        assert_eq!(saved.encoding, Some(Encoding::Big5));
        assert_eq!(saved.checkpoint.as_ref(), Some(&last));
        let stop = CancellationToken::new();
        let reopened = TextBook::open(File::open(&path).unwrap(), saved.encoding, &stop).unwrap();
        let window = reopened
            .window(ReadFrom::Resume(saved.checkpoint), false, &stop)
            .unwrap();
        assert_eq!(window.rows[0].start.paragraph, last.at.paragraph);
        assert_eq!(window.rows[0].text, "Last passage");
        store
            .edit_book(file, BookEdit::Encoding(None))
            .await
            .unwrap();
        store
            .edit_book(file, BookEdit::Rule(BookRule::Inherit))
            .await
            .unwrap();
        let state = store.load(Some(file)).await.unwrap();
        assert_eq!(state.effective_rule(), global.default_rule);
        assert_eq!(
            state.book.unwrap(),
            BookState {
                checkpoint: Some(last),
                ..Default::default()
            }
        );
        let catalog = file_get(db, file).await.unwrap();
        assert_eq!(catalog.display_name, "Catalog title");
        assert_eq!(catalog.progress.as_deref(), Some("finished"));
        assert!(catalog.is_favorite);
    });
}

#[tokio::test]
async fn type_reset_changes_only_the_four_typography_fields() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(dir.path()).await;
    let file = add_file(&db, &dir.path().join("example.txt")).await;
    let store = ReaderStore::new(db);
    tokio::try_join!(
        store.preference(Preference::Opener(TxtOpener::External("My Reader".into()))),
        store.preference(Preference::Collapse(false)),
        store.preference(Preference::Type(typography())),
        store.preference(Preference::Default(DefaultRule::Fixed("english".into()))),
        store.edit_book(file, BookEdit::Rule(BookRule::Fixed("digits".into()))),
    )
    .unwrap();
    let mut expected = store.load(Some(file)).await.unwrap();
    assert_eq!(expected.typography, typography());
    for bad in [
        Typography {
            size: 72.1,
            ..typography()
        },
        Typography {
            leading: 0.9,
            ..typography()
        },
        Typography {
            spacing: f32::NAN,
            ..typography()
        },
    ] {
        assert!(store.preference(Preference::Type(bad)).await.is_err());
    }
    assert!(
        store
            .preference(Preference::Opener(TxtOpener::External(" \t ".into())))
            .await
            .is_err()
    );
    assert_eq!(store.load(Some(file)).await.unwrap(), expected);
    store.preference(Preference::ResetType).await.unwrap();
    expected.typography = Typography {
        family: None,
        size: 18.0,
        leading: 1.6,
        spacing: 0.0,
    };
    assert_eq!(store.load(Some(file)).await.unwrap(), expected);
    store
        .preference(Preference::Opener(TxtOpener::BuiltIn))
        .await
        .unwrap();
    expected.opener = TxtOpener::BuiltIn;
    assert_eq!(store.load(Some(file)).await.unwrap(), expected);
    store
        .preference(Preference::Default(DefaultRule::Auto))
        .await
        .unwrap();
    let state = store.load(Some(file)).await.unwrap();
    assert_eq!(state.default_rule, DefaultRule::Auto);
    assert_eq!(state.effective_rule(), DefaultRule::Fixed("digits".into()));
}

#[tokio::test]
async fn invalid_edit_keeps_previous_rule_and_default() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(dir.path()).await;
    let store = ReaderStore::new(db.clone());
    let id = store
        .edit_rule(save(None, template(), false))
        .await
        .unwrap()
        .unwrap();
    store
        .preference(Preference::Default(DefaultRule::Fixed("english".into())))
        .await
        .unwrap();
    let before = store.load(None).await.unwrap();

    let invalid = RuleDraft {
        mode: PatternMode::Regex,
        chapter: "^Part [0-9]+".into(),
        volume: Some("(".into()),
        ..template()
    };
    for target in [Some(id.as_str()), None] {
        assert!(matches!(
            store.edit_rule(save(target, invalid.clone(), true)).await,
            Err(StoreError::Rule(RuleError::Expression {
                field: Heading::Volume,
                ..
            }))
        ));
    }
    assert_eq!(store.load(None).await.unwrap(), before);

    // Fail the second statement of Save-and-set-default to exercise SQLite rollback.
    sqlx::query(
        "CREATE TRIGGER fail_default BEFORE UPDATE OF default_rule ON reader_preferences
                 BEGIN SELECT RAISE(ABORT, 'default write failed'); END",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let changed = RuleDraft {
        name: "Renamed".into(),
        ..template()
    };
    let edit = save(Some(&id), changed.clone(), true);
    assert!(store.edit_rule(edit.clone()).await.is_err());
    assert_eq!(store.load(None).await.unwrap(), before);
    sqlx::query("DROP TRIGGER fail_default")
        .execute(&db.pool)
        .await
        .unwrap();
    let saved = store.edit_rule(edit).await.unwrap();
    assert_eq!(saved.as_ref(), Some(&id));
    let after = store.load(None).await.unwrap();
    assert_eq!(after.default_rule, DefaultRule::Fixed(id.clone()));
    assert_eq!(
        after.rules.iter().find(|r| r.id == id).unwrap().draft,
        changed
    );
}

#[test]
fn rule_edits_are_atomic_and_deletion_falls_back_on_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("first.txt");
    std::fs::write(&path, "Title\nChapter 1\nBody\n").unwrap();
    let position = text_book(&path).checkpoint(ReadingPosition {
        paragraph: 6,
        restart: 6,
    });
    let files = launch(async {
        let db = open(dir.path()).await;
        let first = add_file(&db, &path).await;
        let second = add_file(&db, &dir.path().join("second.txt")).await;
        let third = add_file(&db, &dir.path().join("third.txt")).await;
        let store = ReaderStore::new(db);
        let preset = store
            .load(None)
            .await
            .unwrap()
            .rules
            .into_iter()
            .find(|r| r.id == "english")
            .unwrap();
        for id in [AUTO_ID, "english"] {
            assert!(matches!(
                store.edit_rule(RuleEdit::Remove(id.into())).await,
                Err(StoreError::NoEditableRule(_))
            ));
            assert!(matches!(
                store.edit_rule(save(Some(id), template(), true)).await,
                Err(StoreError::NoEditableRule(_))
            ));
        }
        assert!(
            store
                .preference(Preference::Default(DefaultRule::Fixed(AUTO_ID.into())))
                .await
                .is_err()
        );
        assert!(
            store
                .edit_book(first, BookEdit::Rule(BookRule::Fixed(AUTO_ID.into())))
                .await
                .is_err()
        );
        let copy = RuleDraft {
            name: "My copy".into(),
            ..preset.draft.clone()
        };
        let id = store
            .edit_rule(save(None, copy.clone(), true))
            .await
            .unwrap()
            .unwrap();
        let alternative = store
            .edit_rule(save(None, template(), false))
            .await
            .unwrap()
            .unwrap();
        let state = store.load(None).await.unwrap();
        assert_eq!(state.rules.len(), 8);
        assert!(state.rules.contains(&preset));
        assert!(state.rules.contains(&SavedRule {
            id: id.clone(),
            draft: copy
        }));
        for file in [first, second] {
            store
                .edit_book(file, BookEdit::Rule(BookRule::Fixed(id.clone())))
                .await
                .unwrap();
            store
                .edit_book(file, BookEdit::Position(position.clone()))
                .await
                .unwrap();
        }
        store
            .edit_book(third, BookEdit::Rule(BookRule::Fixed(alternative.clone())))
            .await
            .unwrap();
        let third_state = store.load(Some(third)).await.unwrap();
        assert_eq!(
            third_state.effective_rule(),
            DefaultRule::Fixed(alternative.clone())
        );
        assert_eq!(third_state.default_rule, DefaultRule::Fixed(id.clone()));
        assert_eq!(
            store
                .edit_rule(RuleEdit::Remove(alternative.clone()))
                .await
                .unwrap(),
            None
        );
        let fallback = store.load(Some(third)).await.unwrap();
        assert_eq!(fallback.book.as_ref().unwrap().rule, BookRule::Inherit);
        assert_eq!(fallback.effective_rule(), DefaultRule::Fixed(id.clone()));
        store
            .edit_book(third, BookEdit::Rule(BookRule::Auto))
            .await
            .unwrap();
        store.edit_rule(RuleEdit::Remove(id.clone())).await.unwrap();
        assert!(matches!(
            store.edit_rule(RuleEdit::Remove(id.clone())).await,
            Err(StoreError::NoEditableRule(_))
        ));
        assert!(matches!(
            store.edit_rule(save(Some(&id), template(), true)).await,
            Err(StoreError::NoEditableRule(_))
        ));
        assert!(matches!(
            store
                .preference(Preference::Default(DefaultRule::Fixed(id.clone())))
                .await,
            Err(StoreError::MissingSelection)
        ));
        assert!(matches!(
            store
                .edit_book(first, BookEdit::Rule(BookRule::Fixed(id)))
                .await,
            Err(StoreError::MissingSelection)
        ));
        [first, second, third]
    });
    launch(async {
        let store = ReaderStore::new(open(dir.path()).await);
        for file in &files[..2] {
            let state = store.load(Some(*file)).await.unwrap();
            assert_eq!(state.rules, builtin_rules());
            assert_eq!(state.default_rule, DefaultRule::Auto);
            assert_eq!(state.effective_rule(), DefaultRule::Auto);
            let book = state.book.unwrap();
            assert_eq!(book.rule, BookRule::Inherit);
            assert_eq!(book.checkpoint.as_ref(), Some(&position));
        }
        assert_eq!(
            store.load(Some(files[2])).await.unwrap().book.unwrap().rule,
            BookRule::Auto
        );
    });
}
