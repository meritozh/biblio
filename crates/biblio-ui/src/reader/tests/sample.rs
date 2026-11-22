//! Reproducible 50 MiB source for the blocked-scan and packaged-reader checks.
use std::{
    fs::File,
    io::{self, BufWriter, Seek, SeekFrom, Write},
    path::Path,
};

pub const SIZE: u64 = 50 * 1024 * 1024;
pub const MIDDLE: u64 = 32 * 1024 * 1024;

pub fn write_sample(path: &Path) -> io::Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    for i in 0..SIZE / 128 {
        let line = format!(
            "Passage {i:06} | The ferry crosses the river. On the eastern bank, the keeper counts the lights."
        );
        writeln!(file, "{line:<127}")?;
    }
    for (at, text) in [
        (0, "First book begins\nChapter 1\n"),
        (MIDDLE, "Lower half begins\nChapter 2\n"),
        (MIDDLE + 8192, "1\n"),
        (MIDDLE + 16384, "2\n"),
    ] {
        file.seek(SeekFrom::Start(at))?;
        file.write_all(text.as_bytes())?;
    }
    // One paragraph crosses several source windows and hundreds of display lines.
    file.seek(SeekFrom::Start(1024 * 1024))?;
    file.write_all(&b"lantern ".repeat(16384))?;
    file.write_all(b"COPPER COMET LANTERN ")?;
    file.write_all(&b"harbor ".repeat(8192))?;
    file.write_all(b"\n")?;
    file.flush()
}
