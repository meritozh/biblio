//! Independent, position-based reads of an open TXT file. All offsets in saved
//! positions and cursors are source bytes, including any initial BOM.
mod blanks;
mod codec;
mod search;
mod stream;

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::File,
    io,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::SystemTime,
};
use tokio_util::sync::CancellationToken;

use blanks::BlankContext;
pub use codec::Encoding;
use stream::Chunks;
pub use stream::{LineEnding, TextRow};

const BLOCK: usize = 64 * 1024;
const WINDOW: u64 = 4 * BLOCK as u64;
const MAX_ROWS: usize = 2048;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReadingPosition {
    pub paragraph: u64,
    pub restart: u64,
}

impl ReadingPosition {
    fn paragraph(at: u64) -> Self {
        Self {
            paragraph: at,
            restart: at,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextPoint {
    pub restart: ReadingPosition,
    /// UTF-8 bytes into a source unit that emits multiple Unicode scalars.
    pub utf8_skip: u32,
}

impl From<ReadingPosition> for TextPoint {
    fn from(restart: ReadingPosition) -> Self {
        Self {
            restart,
            utf8_skip: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Hit {
    pub start: TextPoint,
    pub end: TextPoint,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceStamp {
    pub version: u8,
    pub len: u64,
    #[serde(with = "file_time")]
    pub modified: SystemTime,
    pub head_digest: [u8; 16],
}

// Serde's SystemTime representation only supports timestamps from 1970 onward.
// Preserve that representation and store older file times as a distance before 1970.
mod file_time {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[derive(Serialize, Deserialize)]
    #[serde(untagged)]
    enum FileTime {
        SinceEpoch(SystemTime),
        BeforeEpoch { before_epoch: Duration },
    }

    pub(super) fn serialize<S: Serializer>(
        time: &SystemTime,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match time.duration_since(UNIX_EPOCH) {
            Ok(_) => FileTime::SinceEpoch(*time),
            Err(error) => FileTime::BeforeEpoch {
                before_epoch: error.duration(),
            },
        }
        .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<SystemTime, D::Error> {
        match FileTime::deserialize(deserializer)? {
            FileTime::SinceEpoch(time) => Ok(time),
            FileTime::BeforeEpoch { before_epoch } => UNIX_EPOCH
                .checked_sub(before_epoch)
                .ok_or_else(|| serde::de::Error::custom("file time is out of range")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub at: ReadingPosition,
    pub source: SourceStamp,
    pub codec: Encoding,
}

#[derive(Clone, Copy, Debug)]
pub struct Cursor {
    book: u64,
    at: ReadingPosition,
    left_blank: bool,
    right_blank: bool,
    blanks: BlankContext,
}

#[derive(Clone, Debug)]
pub enum ReadFrom {
    Resume(Option<Checkpoint>),
    Around(TextPoint),
    Before(Cursor),
    After(Cursor),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Previous,
    Next,
}

/// A cursor can advance a collapsed run even when this window contains no visible rows.
#[derive(Debug)]
pub struct Window {
    pub rows: Vec<TextRow>,
    pub before: Option<Cursor>,
    pub after: Option<Cursor>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("Could not read the text file: {0}")]
    Io(#[from] io::Error),
    #[error("The text file changed while it was open")]
    SourceChanged,
    #[error("This cursor belongs to a different text reader")]
    InvalidCursor,
    #[error("Reading was cancelled")]
    Cancelled,
}

pub struct TextBook {
    file: File,
    source: SourceStamp,
    codec: Encoding,
    uncertain: bool,
    body: u64,
    id: u64,
    // Only useful for backward seeks into long, ambiguous multibyte lines.
    // Never hold this lock across file I/O or decoding.
    restarts: Mutex<VecDeque<ReadingPosition>>,
}

pub(super) type TextChunk = TextRow;

/// A complete logical line in the open book, including any original terminator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineRange {
    book: u64,
    start: ReadingPosition,
    end: u64,
}

impl LineRange {
    pub(super) fn new(book: &TextBook, start: ReadingPosition, end: u64) -> Self {
        Self {
            book: book.id,
            start,
            end,
        }
    }

    pub(super) fn text(
        &self,
        book: &TextBook,
        stop: &CancellationToken,
    ) -> Result<String, ReadError> {
        if self.book != book.id {
            return Err(ReadError::InvalidCursor);
        }
        let mut text = String::new();
        for chunk in Chunks::new(book, self.start, self.end, stop) {
            text.push_str(&chunk?.text);
        }
        Ok(text)
    }
}

pub(super) fn check_stop(stop: &CancellationToken) -> Result<(), ReadError> {
    if stop.is_cancelled() {
        Err(ReadError::Cancelled)
    } else {
        Ok(())
    }
}

fn read_at(file: &File, mut at: u64, mut dst: &mut [u8]) -> Result<(), ReadError> {
    while !dst.is_empty() {
        #[cfg(unix)]
        let result = std::os::unix::fs::FileExt::read_at(file, dst, at);
        #[cfg(windows)]
        let result = std::os::windows::fs::FileExt::seek_read(file, dst, at);
        match result {
            Ok(0) => return Err(ReadError::SourceChanged),
            Ok(n) => {
                at += n as u64;
                dst = &mut dst[n..];
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

impl TextBook {
    pub fn open(
        file: File,
        encoding: Option<Encoding>,
        stop: &CancellationToken,
    ) -> Result<Self, ReadError> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        check_stop(stop)?;
        let metadata = file.metadata()?;
        let mut head = vec![0; metadata.len().min(BLOCK as u64) as usize];
        read_at(&file, 0, &mut head)?;
        let (codec, uncertain) = encoding
            .map(|enc| (enc, false))
            .unwrap_or_else(|| Encoding::detect(&head, metadata.len() > head.len() as u64));
        let book = Self {
            file,
            codec,
            uncertain,
            source: SourceStamp {
                version: 1,
                len: metadata.len(),
                modified: metadata.modified()?,
                head_digest: Md5::digest(&head).into(),
            },
            body: Encoding::bom(&head).map_or(0, |(_, n)| n as u64),
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            restarts: Mutex::default(),
        };
        book.verify(stop)?;
        Ok(book)
    }

    /// Re-detect or override the codec on the same read-only source. Reads
    /// are positional, so the duplicate handle cannot disturb another lane.
    pub fn with_encoding(
        &self,
        encoding: Option<Encoding>,
        stop: &CancellationToken,
    ) -> Result<Self, ReadError> {
        self.verify(stop)?;
        let book = Self::open(self.file.try_clone()?, encoding, stop)?;
        if book.source != self.source {
            return Err(ReadError::SourceChanged);
        }
        Ok(book)
    }

    pub fn encoding(&self) -> Encoding {
        self.codec
    }
    pub fn encoding_uncertain(&self) -> bool {
        self.uncertain
    }
    pub fn source(&self) -> &SourceStamp {
        &self.source
    }

    pub fn checkpoint(&self, at: ReadingPosition) -> Checkpoint {
        Checkpoint {
            at,
            source: self.source.clone(),
            codec: self.codec,
        }
    }

    pub fn sample(
        &self,
        candidate: Encoding,
        stop: &CancellationToken,
    ) -> Result<String, ReadError> {
        let bytes = (self.source.len - self.body).min(BLOCK as u64) as usize;
        let mut head = Vec::new();
        self.read(self.body, bytes, &mut head, stop)?;
        let sample = candidate.preview(&head, self.body + bytes as u64 == self.source.len);
        self.verify(stop)?;
        Ok(sample)
    }

    pub fn verify(&self, stop: &CancellationToken) -> Result<(), ReadError> {
        check_stop(stop)?;
        let meta = self.file.metadata()?;
        if meta.len() != self.source.len || meta.modified()? != self.source.modified {
            return Err(ReadError::SourceChanged);
        }
        Ok(())
    }

    fn read(
        &self,
        at: u64,
        n: usize,
        dst: &mut Vec<u8>,
        stop: &CancellationToken,
    ) -> Result<(), ReadError> {
        self.verify(stop)?;
        dst.resize(n, 0);
        read_at(&self.file, at, dst)
    }

    /// Unprojected paragraph fragments. The original terminator is in `ending`.
    pub(super) fn chunks<'a>(
        &'a self,
        stop: &'a CancellationToken,
    ) -> impl Iterator<Item = Result<TextChunk, ReadError>> + 'a {
        Chunks::new(
            self,
            ReadingPosition::paragraph(self.body),
            self.source.len,
            stop,
        )
    }

    fn cursor(
        &self,
        at: ReadingPosition,
        left_blank: bool,
        right_blank: bool,
        blanks: BlankContext,
    ) -> Cursor {
        Cursor {
            book: self.id,
            at,
            left_blank,
            right_blank,
            blanks,
        }
    }

    // CR and LF cannot occur within a multibyte unit of the supported legacy
    // encodings. For UTF-16, only inspect aligned code units.
    fn paragraph_at(&self, target: u64, stop: &CancellationToken) -> Result<u64, ReadError> {
        let stride = self.codec.stride() as u64;
        let mut end = target - (target - self.body) % stride;
        let mut buf = Vec::new();
        while end > self.body {
            let start = end.saturating_sub(BLOCK as u64).max(self.body);
            let n = (end + stride).min(self.source.len) - start;
            self.read(start, n as usize, &mut buf, stop)?;
            for i in (0..(end - start) as usize).step_by(stride as usize).rev() {
                match self.codec.ascii(&buf[i..]) {
                    Some(b'\r') if self.codec.ascii(&buf[i + stride as usize..]) == Some(b'\n') => {
                        continue;
                    }
                    Some(b'\r' | b'\n') => return Ok(start + i as u64 + stride),
                    _ => {}
                }
            }
            end = start;
        }
        Ok(self.body)
    }

    fn position_near(
        &self,
        target: u64,
        known: Option<u64>,
        stop: &CancellationToken,
    ) -> Result<ReadingPosition, ReadError> {
        let target = target.clamp(self.body, self.source.len);
        let para = match known.filter(|&p| p <= target) {
            Some(p) => p,
            None => self.paragraph_at(target, stop)?,
        };
        let start = ReadingPosition::paragraph(para);
        if target - para < BLOCK as u64 {
            return Ok(start);
        }

        // UTF-8 and UTF-16 have locally recognizable boundaries. The legacy
        // codecs can have long runs of ambiguous lead/trail bytes.
        let mut buf = Vec::new();
        let at = match self.codec {
            Encoding::Utf8 => {
                let left = target - 3;
                self.read(
                    left,
                    (self.source.len - left).min(7) as usize,
                    &mut buf,
                    stop,
                )?;
                // Only back up if a valid or malformed source unit spans target.
                let mut i = 0;
                while i < 3 {
                    let (n, _) = self.codec.unit(&buf[i..], &mut [0; 8]);
                    if i + n > 3 {
                        break;
                    }
                    i += n;
                }
                left + i as u64
            }
            Encoding::Utf16Le | Encoding::Utf16Be => {
                let aligned = target - (target - para) % 2;
                let left = aligned - 2;
                self.read(
                    left,
                    (self.source.len - left).min(4) as usize,
                    &mut buf,
                    stop,
                )?;
                let (n, _) = self.codec.unit(&buf, &mut [0; 8]);
                if n > 2 { left } else { aligned }
            }
            _ => {
                let anchor = self
                    .restarts
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|p| p.paragraph == para && p.restart <= target)
                    .max_by_key(|p| p.restart)
                    .copied()
                    .unwrap_or(start);
                let mut last = anchor;
                for chunk in Chunks::new(self, anchor, self.source.len, stop) {
                    let chunk = chunk?;
                    if chunk.end.restart > target {
                        return Ok(chunk.start);
                    }
                    last = chunk.end;
                    if last.restart == target {
                        break;
                    }
                }
                return Ok(last);
            }
        };
        self.logical_boundary(
            ReadingPosition {
                paragraph: para,
                restart: at,
            },
            stop,
        )
    }

    fn logical_boundary(
        &self,
        pos: ReadingPosition,
        stop: &CancellationToken,
    ) -> Result<ReadingPosition, ReadError> {
        let at = pos.restart;
        let mut buf = Vec::new();
        // A cursor must never split the two units of a logical CRLF.
        let stride = self.codec.stride() as u64;
        self.read(
            at - stride,
            (self.source.len - (at - stride)).min(2 * stride) as usize,
            &mut buf,
            stop,
        )?;
        let between_crlf = self.codec.ascii(&buf) == Some(b'\r')
            && self.codec.ascii(&buf[stride as usize..]) == Some(b'\n');
        Ok(ReadingPosition {
            paragraph: pos.paragraph,
            restart: if between_crlf { at - stride } else { at },
        })
    }

    fn remember(&self, at: ReadingPosition) {
        if at.restart - at.paragraph < BLOCK as u64
            || matches!(
                self.codec,
                Encoding::Utf8 | Encoding::Utf16Le | Encoding::Utf16Be
            )
        {
            return;
        }
        let mut cache = self.restarts.lock().unwrap();
        if cache.len() == 256 {
            cache.pop_front();
        }
        cache.push_back(at);
    }

    fn visible_start(
        &self,
        pos: ReadingPosition,
        trusted: bool,
        stop: &CancellationToken,
    ) -> Result<ReadingPosition, ReadError> {
        if self.body == self.source.len {
            return Ok(ReadingPosition::paragraph(self.body));
        }
        if trusted
            && pos.paragraph >= self.body
            && pos.paragraph <= pos.restart
            && pos.restart < self.source.len
        {
            if pos.restart - pos.paragraph < BLOCK as u64 {
                return Ok(ReadingPosition::paragraph(pos.paragraph));
            }
            return self.logical_boundary(pos, stop);
        }
        self.position_near(pos.restart.min(self.source.len - 1), None, stop)
    }

    pub fn window(
        &self,
        from: ReadFrom,
        collapse_blanks: bool,
        stop: &CancellationToken,
    ) -> Result<Window, ReadError> {
        self.verify(stop)?;
        let mut until = self.source.len;
        let mut neighbor = false;
        let mut blanks = BlankContext::default();
        let backward = matches!(from, ReadFrom::Before(_));
        let start = match from {
            ReadFrom::After(c) | ReadFrom::Before(c) if c.book != self.id => {
                return Err(ReadError::InvalidCursor);
            }
            ReadFrom::After(c) => {
                blanks = c.blanks;
                neighbor = c.left_blank;
                c.at
            }
            ReadFrom::Before(c) => {
                blanks = c.blanks;
                neighbor = c.right_blank;
                until = c.at.restart;
                self.position_near(until.saturating_sub(WINDOW), Some(c.at.paragraph), stop)?
            }
            ReadFrom::Resume(Some(saved)) => self.visible_start(
                saved.at,
                saved.source == self.source && saved.codec == self.codec,
                stop,
            )?,
            ReadFrom::Around(point) => self.visible_start(point.restart, true, stop)?,
            ReadFrom::Resume(None) => ReadingPosition::paragraph(self.body),
        };

        let mut raw = VecDeque::new();
        let mut end = start;
        for chunk in Chunks::new(self, start, until, stop) {
            let chunk = chunk?;
            end = chunk.end;
            if raw.len() == MAX_ROWS {
                raw.pop_front();
            }
            raw.push_back(chunk);
            if !backward && (end.restart - start.restart >= WINDOW || raw.len() == MAX_ROWS) {
                break;
            }
        }
        let mut first = raw.front().map_or(start, |r| r.start);
        let mut last = end;
        let mut rows: Vec<TextRow> = Vec::with_capacity(raw.len());
        let mut budget = WINDOW as usize;
        let mut previous_blank = collapse_blanks && neighbor;
        let mut near_edge = None;
        while let Some(mut row) = if backward {
            raw.pop_back()
        } else {
            raw.pop_front()
        } {
            if collapse_blanks {
                let Some(prefix) = blanks.prefix(self, &row, &mut budget, stop)? else {
                    if backward {
                        first = row.end;
                    } else {
                        last = row.start;
                    }
                    break;
                };
                // At the near edge the adjacent window may already display
                // this blank run. Only one of the two windows claims it.
                let blank = matches!(prefix, blanks::Prefix::Blank);
                near_edge.get_or_insert(blank && !previous_blank);
                if blank {
                    row.collapse();
                    if previous_blank {
                        if let Some(representative) = rows.last_mut() {
                            representative.include_blank(&row, backward);
                        }
                        continue;
                    }
                }
                previous_blank = blank;
                if let blanks::Prefix::Body(at) = prefix {
                    if row.end.restart <= at {
                        continue;
                    }
                    if row.start.restart <= at {
                        let point = ReadingPosition {
                            paragraph: row.start.paragraph,
                            restart: at,
                        };
                        row.body_start = row.offset_of(point.into());
                    }
                }
            }
            rows.push(row);
        }
        if backward {
            rows.reverse();
        }
        self.verify(stop)?;
        let near_edge = near_edge.unwrap_or(false);
        let (before, after) = if backward {
            (
                self.cursor(first, false, previous_blank, blanks),
                self.cursor(last, near_edge, neighbor, blanks),
            )
        } else {
            (
                self.cursor(first, neighbor, near_edge, blanks),
                self.cursor(last, previous_blank, false, blanks),
            )
        };
        let before = (first.restart > self.body).then_some(before);
        let after = (last.restart < self.source.len).then_some(after);
        Ok(Window {
            rows,
            before,
            after,
        })
    }
}

#[cfg(test)]
mod tests;
