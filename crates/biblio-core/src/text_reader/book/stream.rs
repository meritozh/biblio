use super::{BLOCK, Encoding, ReadError, ReadingPosition, TextBook, TextPoint, check_stop};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineEnding {
    None,
    Lf,
    Cr,
    CrLf,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Lf => "\n",
            Self::Cr => "\r",
            Self::CrLf => "\r\n",
        }
    }
}

// Consecutive units of the same source and UTF-8 width share one mapping.
#[derive(Clone, Debug)]
struct Run {
    source: u32,
    utf8: u32,
    source_width: u8,
    utf8_width: u8,
}

#[derive(Clone, Debug)]
pub struct TextRow {
    pub start: ReadingPosition,
    pub end: ReadingPosition,
    pub text: String,
    pub ending: LineEnding,
    pub continues_to_next: bool,
    collapsed: bool,
    content_end: ReadingPosition,
    stride: u8,
    runs: Vec<Run>,
}

impl TextRow {
    pub fn continues_from_previous(&self) -> bool {
        !self.collapsed && self.start.paragraph != self.start.restart
    }

    /// Offsets include the original line ending after the display text.
    pub fn point_at(&self, offset: usize) -> Option<TextPoint> {
        if self.collapsed && offset == 0 {
            return Some(self.start.into());
        }
        let n = self.text.len();
        if offset == n + self.ending.as_str().len() {
            return Some(self.end.into());
        }
        if offset >= n {
            if offset >= n + self.ending.as_str().len() {
                return None;
            }
            return Some(
                ReadingPosition {
                    paragraph: self.content_end.paragraph,
                    restart: self.content_end.restart
                        + ((offset - n) * self.stride as usize) as u64,
                }
                .into(),
            );
        }
        if !self.text.is_char_boundary(offset) {
            return None;
        }
        let i = self.runs.partition_point(|r| r.utf8 <= offset as u32);
        let run = &self.runs[i - 1];
        let delta = offset as u32 - run.utf8;
        let unit = delta / run.utf8_width as u32;
        Some(self.point(
            self.start.restart + (run.source + unit * run.source_width as u32) as u64,
            delta % run.utf8_width as u32,
        ))
    }

    /// Inverse of point_at, for positioning or highlighting in this fragment.
    pub fn offset_of(&self, point: TextPoint) -> Option<usize> {
        let at = point.restart.restart;
        if at < self.start.restart || at > self.end.restart {
            return None;
        }
        if self.collapsed && at <= self.content_end.restart {
            return Some(0);
        }
        let offset = if at >= self.content_end.restart {
            let delta = at - self.content_end.restart;
            self.text.len() + usize::try_from(delta / self.stride as u64).ok()?
        } else {
            let delta = (at - self.start.restart) as u32;
            let i = self.runs.partition_point(|r| r.source <= delta);
            let run = self.runs.get(i.checked_sub(1)?)?;
            (run.utf8 + (delta - run.source) / run.source_width as u32 * run.utf8_width as u32)
                as usize
                + point.utf8_skip as usize
        };
        (self.point_at(offset)? == point).then_some(offset)
    }

    fn point(&self, restart: u64, utf8_skip: u32) -> TextPoint {
        TextPoint {
            restart: ReadingPosition {
                paragraph: self.start.paragraph,
                restart,
            },
            utf8_skip,
        }
    }

    pub(super) fn collapse(&mut self) {
        self.collapsed = true;
        self.continues_to_next = false;
        self.text.clear();
        self.runs.clear();
    }

    pub(super) fn include_blank(&mut self, adjacent: &Self, prepend: bool) {
        debug_assert!(self.collapsed);
        if prepend {
            self.start = adjacent.start;
        } else {
            self.content_end = adjacent.content_end;
            self.end = adjacent.end;
            self.ending = adjacent.ending;
        }
    }

    fn map(&mut self, source: u64, source_width: usize, utf8_width: usize) {
        if self.runs.last().is_some_and(|r| {
            r.source_width as usize == source_width && r.utf8_width as usize == utf8_width
        }) {
            return;
        }
        self.runs.push(Run {
            source: (source - self.start.restart) as u32,
            utf8: self.text.len() as u32,
            source_width: source_width as u8,
            utf8_width: utf8_width as u8,
        });
    }
}

pub(super) struct Chunks<'a> {
    book: &'a TextBook,
    stop: &'a CancellationToken,
    at: ReadingPosition,
    until: u64,
    block_start: u64,
    block_end: u64,
    bytes: Vec<u8>,
    finished: bool,
}

impl<'a> Chunks<'a> {
    pub(super) fn new(
        book: &'a TextBook,
        at: ReadingPosition,
        until: u64,
        stop: &'a CancellationToken,
    ) -> Self {
        Self {
            book,
            at,
            until,
            stop,
            block_start: 0,
            block_end: 0,
            bytes: Vec::new(),
            finished: false,
        }
    }

    fn read_chunk(&mut self) -> Result<Option<TextRow>, ReadError> {
        check_stop(self.stop)?;
        if self.at.restart >= self.until {
            self.book.verify(self.stop)?;
            return Ok(None);
        }
        let enc = self.book.encoding();
        let mut row = TextRow {
            start: self.at,
            end: self.at,
            text: String::new(),
            ending: LineEnding::None,
            continues_to_next: false,
            collapsed: false,
            content_end: self.at,
            stride: enc.stride() as u8,
            runs: Vec::new(),
        };
        let mut next = self.at.restart;
        loop {
            if next >= self.block_end {
                self.book.remember(ReadingPosition {
                    paragraph: self.at.paragraph,
                    restart: next,
                });
                self.block_start = next;
                let n = (self.until - next).min((BLOCK + 8) as u64) as usize;
                self.book.read(next, n, &mut self.bytes, self.stop)?;
                self.block_end = next + n.min(BLOCK) as u64;
            }
            let mut i = (next - self.block_start) as usize;
            let limit =
                (self.block_end.min(row.start.restart + BLOCK as u64) - self.block_start) as usize;
            while i < limit {
                let src = &self.bytes[i..];
                if matches!(enc.ascii(src), Some(b'\n' | b'\r')) {
                    break;
                }
                if enc.stride() == 1 && src[0].is_ascii() {
                    let run = self.bytes[i..limit]
                        .iter()
                        .take_while(|&&b| b.is_ascii() && b != b'\r' && b != b'\n')
                        .count();
                    row.map(self.block_start + i as u64, 1, 1);
                    row.text.push_str(str::from_utf8(&src[..run]).unwrap());
                    i += run;
                } else {
                    let mut output = [0; 8];
                    let (n, text) = enc.unit(src, &mut output);
                    row.map(self.block_start + i as u64, n, text.len());
                    row.text.push_str(text);
                    i += n;
                }
            }
            row.content_end.restart = self.block_start + i as u64;
            row.ending = line_ending(enc, &self.bytes[i..]);
            i += row.ending.as_str().len() * enc.stride();
            next = self.block_start + i as u64;
            if row.ending != LineEnding::None
                || next == self.until
                || next - row.start.restart >= BLOCK as u64
            {
                break;
            }
        }

        row.end = if row.ending != LineEnding::None {
            ReadingPosition::paragraph(next)
        } else {
            ReadingPosition {
                paragraph: self.at.paragraph,
                restart: next,
            }
        };
        row.continues_to_next = row.ending == LineEnding::None && next < self.book.source.len;
        self.at = row.end;
        Ok(Some(row))
    }
}

fn line_ending(enc: Encoding, bytes: &[u8]) -> LineEnding {
    match enc.ascii(bytes) {
        Some(b'\r') if enc.ascii(&bytes[enc.stride()..]) == Some(b'\n') => LineEnding::CrLf,
        Some(b'\r') => LineEnding::Cr,
        Some(b'\n') => LineEnding::Lf,
        _ => LineEnding::None,
    }
}

impl Iterator for Chunks<'_> {
    type Item = Result<TextRow, ReadError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let row = self.read_chunk().transpose();
        self.finished = !matches!(row, Some(Ok(_)));
        row
    }
}
