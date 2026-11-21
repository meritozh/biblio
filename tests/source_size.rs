use std::{fs, io, path::Path};

fn check_sources(dir: &Path, oversized: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if !matches!(
                entry.file_name().to_str(),
                Some("generated" | "schema" | "templates" | "template" | "fixtures" | "fixture")
            ) {
                check_sources(&path, oversized)?;
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let lines = fs::read_to_string(&path)?.lines().count();
            if lines > 800 {
                oversized.push(format!("{} ({lines} lines)", path.display()));
            }
        }
    }
    Ok(())
}

#[test]
fn rust_source_files_stay_within_800_lines() -> io::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut oversized = Vec::new();
    for source in ["src", "crates/biblio-core/src", "tests"] {
        check_sources(&root.join(source), &mut oversized)?;
    }
    oversized.sort();
    assert!(oversized.is_empty(), "{}", oversized.join("\n"));
    Ok(())
}
