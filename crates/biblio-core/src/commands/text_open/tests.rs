use super::*;
use crate::commands::cache::{cache_open, cache_open_with};
use crate::text_reader::store::Preference;
use std::{cell::RefCell, fs, path::PathBuf};

async fn library(profile: &Path) -> Library {
    let db = Library::open(profile.to_owned(), |_, _| {}).await.unwrap();
    sqlx::query("UPDATE app_settings SET value = ? WHERE key = 'storage_path'")
        .bind(profile.to_str().unwrap())
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::raw_sql(
        r#"INSERT INTO schemas (id, name) VALUES ('field-notes', 'Field notes');
           INSERT INTO categories (id, name, schema_slug, view_config)
           VALUES (100, 'Expeditions', 'field-notes', '{"open_app":"Field Viewer"}')"#,
    )
    .execute(&db.pool)
    .await
    .unwrap();
    db
}

#[tokio::test]
async fn recognizes_catalog_txt_across_storage_kinds() {
    let profile = tempfile::tempdir().unwrap();
    let db = library(profile.path()).await;
    sqlx::raw_sql(
        "INSERT INTO files (id, category_id, display_name, path, storage_kind,
                            remote_container, original_path, local_cache_path) VALUES
         (1, 100, 'Note', 'category/note.TxT', 'local', NULL, 'old.pdf', NULL),
         (2, 100, 'Note', 'legacy/note.TXT', 'remote', NULL, '', NULL),
         (3, 100, 'Note', 'opaque1', 'remote', 'bbx1', 'before/upload.txt', NULL),
         (4, 100, 'Note', 'opaque2', 'remote', 'bbx1-split', 'old/novel.Txt', NULL),
         (5, 100, 'Note', 'opaque3', 'remote', 'bbx1', NULL, '.cache/5_note.TXT'),
         (6, 100, 'Title.txt', 'notes.txt/report.pdf', 'local', NULL, 'old.txt', 'note.txt'),
         (7, 100, 'Title.txt', 'legacy/old.txt', 'remote', NULL, 'actual.pdf', 'note.txt'),
         (8, 100, 'Title.txt', 'opaque4', 'remote', 'bbx1-split', NULL, NULL),
         (9, 100, 'Title.txt', 'object.txt', 'remote', 'bbx1', NULL, NULL),
         (10, 100, 'Title.txt', 'legacy/report.pdf', 'remote', NULL, NULL, 'note.txt'),
         (11, 100, 'Title.txt', 'opaque5', 'remote', 'bbx1', 'original/no-ext', 'note.txt')",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    for entry in [Entry::Shelf, Entry::DetailOpen] {
        for id in 1..=5 {
            assert_eq!(
                target(&db, id, entry).await.unwrap(),
                OpenTarget::Reader(id)
            );
        }
        for id in 6..=11 {
            let expected = match entry {
                Entry::Shelf => OpenTarget::Details(id),
                Entry::DetailOpen => OpenTarget::CategoryFile(id),
            };
            assert_eq!(target(&db, id, entry).await.unwrap(), expected);
        }
        assert_eq!(target(&db, 99, entry).await.unwrap_err(), "FILE_NOT_FOUND");
    }
}

#[derive(Default)]
struct RecordingOpener {
    calls: RefCell<Vec<(PathBuf, Option<String>)>>,
    error: Option<&'static str>,
}

impl ExternalOpener for RecordingOpener {
    fn launch(&self, path: &Path, program: Option<&str>) -> Result<(), String> {
        self.calls
            .borrow_mut()
            .push((path.to_owned(), program.map(str::to_owned)));
        match self.error {
            Some(error) => Err(error.to_owned()),
            None => Ok(()),
        }
    }
}

#[tokio::test]
async fn txt_uses_global_opener_and_other_formats_keep_category_opener() {
    let profile = tempfile::tempdir().unwrap();
    let db = library(profile.path()).await;
    sqlx::raw_sql(
        "INSERT INTO files (id, category_id, display_name, path, storage_kind,
                            original_path, local_cache_path) VALUES
         (1, 100, 'Note', 'note.txt', 'local', NULL, NULL),
         (2, 100, 'Map', 'map.pdf', 'local', NULL, NULL),
         (3, NULL, 'Map', 'old/map.pdf', 'remote', NULL, '.cache/3_map.pdf'),
         (4, 100, 'Map', 'old/uncached.pdf', 'remote', NULL, NULL),
         (5, NULL, 'Note', 'uncached', 'remote', 'field/note.txt', NULL)",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let store = ReaderStore::new(db.clone());
    store
        .preference(Preference::Opener(TxtOpener::External(
            "Text Editor".into(),
        )))
        .await
        .unwrap();
    for id in [1, 5] {
        for entry in [Entry::Shelf, Entry::DetailOpen] {
            assert_eq!(
                target(&db, id, entry).await.unwrap(),
                OpenTarget::ExternalTxt {
                    id,
                    program: "Text Editor".into()
                }
            );
        }
    }

    fs::create_dir(profile.path().join(".cache")).unwrap();
    for name in ["map.pdf", ".cache/3_map.pdf"] {
        fs::write(profile.path().join(name), b"map").unwrap();
    }
    let opener = RecordingOpener::default();
    for id in [2, 3] {
        assert_eq!(
            target(&db, id, Entry::Shelf).await.unwrap(),
            OpenTarget::Details(id)
        );
        let OpenTarget::CategoryFile(file) = target(&db, id, Entry::DetailOpen).await.unwrap()
        else {
            panic!("expected category file {id}")
        };
        assert!(cache_open_with(&db, file, &opener).await.unwrap().success);
    }
    assert_eq!(
        *opener.calls.borrow(),
        [
            (profile.path().join("map.pdf"), Some("Field Viewer".into())),
            (profile.path().join(".cache/3_map.pdf"), None),
        ]
    );
    assert!(matches!(cache_open(db.clone(), 4).await, Err(e) if e == "CACHE_NOT_FOUND"));
    assert!(matches!(cache_open(db.clone(), 99).await, Err(e) if e == "FILE_NOT_FOUND"));

    store
        .preference(Preference::Opener(TxtOpener::BuiltIn))
        .await
        .unwrap();
    assert_eq!(
        target(&db, 5, Entry::Shelf).await.unwrap(),
        OpenTarget::Reader(5)
    );
}

#[tokio::test]
async fn category_launch_reports_the_opener_error_once() {
    let profile = tempfile::tempdir().unwrap();
    let db = library(profile.path()).await;
    sqlx::query(
        "INSERT INTO files (id, category_id, display_name, path) VALUES (1, 100, 'Map', 'map.pdf')",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    fs::write(profile.path().join("map.pdf"), b"map").unwrap();
    let opener = RecordingOpener {
        error: Some("application unavailable"),
        ..Default::default()
    };
    assert!(
        matches!(cache_open_with(&db, 1, &opener).await, Err(e) if e == "application unavailable")
    );
    assert_eq!(
        *opener.calls.borrow(),
        [(profile.path().join("map.pdf"), Some("Field Viewer".into()))]
    );
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn external_txt_reports_system_launch_failure_for_a_missing_program() {
    let profile = tempfile::tempdir().unwrap();
    let db = library(profile.path()).await;
    sqlx::query("INSERT INTO files (id, path, display_name) VALUES (1, 'note.txt', 'Note')")
        .execute(&db.pool)
        .await
        .unwrap();
    let source = profile.path().join("note.txt");
    fs::write(&source, b"note").unwrap();
    let missing = profile.path().join("missing.app");
    ReaderStore::new(db.clone())
        .preference(Preference::Opener(TxtOpener::External(
            missing.to_str().unwrap().to_owned(),
        )))
        .await
        .unwrap();
    let OpenTarget::ExternalTxt { id, program } = target(&db, 1, Entry::Shelf).await.unwrap()
    else {
        panic!("expected an external TXT target")
    };
    assert_eq!(Path::new(&program), missing);
    let copy = readable_copy(&db, id).await.unwrap();
    let CopyState::Ready(path) = copy.borrow().clone() else {
        panic!("expected a ready path")
    };
    assert_eq!(path, source);
    let error = SystemOpener.launch(&path, Some(&program)).unwrap_err();
    assert!(error.starts_with("Failed to open:"), "{error}");
}
