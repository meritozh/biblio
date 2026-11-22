use super::{BLOCK, Chunks, Direction, Hit, ReadError, TextBook, TextPoint, TextRow, check_stop};
use std::collections::VecDeque;
use tokio_util::sync::CancellationToken;

fn key(point: TextPoint) -> (u64, u32) {
    (point.restart.restart, point.utf8_skip)
}

#[derive(Clone, Copy)]
enum Pass {
    Next,
    Previous,
    Wrapped,
}

fn point_at(rows: &VecDeque<(u64, TextRow)>, byte: u64) -> TextPoint {
    let i = rows.partition_point(|(start, _)| *start <= byte) - 1;
    let (start, row) = &rows[i];
    row.point_at((byte - start) as usize).unwrap()
}

// KMP keeps a partial literal across fragments, including the bytes of the
// original line endings. Retain just enough source maps to locate that literal.
struct Literal<'a> {
    bytes: &'a [u8],
    prefix: Vec<usize>,
}

impl<'a> Literal<'a> {
    fn new(bytes: &'a [u8], stop: &CancellationToken) -> Result<Self, ReadError> {
        let mut prefix = vec![0; bytes.len()];
        let mut matched = 0;
        for i in 1..bytes.len() {
            if i % BLOCK == 0 {
                check_stop(stop)?;
            }
            while matched > 0 && bytes[i] != bytes[matched] {
                matched = prefix[matched - 1];
            }
            if bytes[i] == bytes[matched] {
                matched += 1;
            }
            prefix[i] = matched;
        }
        Ok(Self { bytes, prefix })
    }

    fn scan(
        &self,
        chunks: impl Iterator<Item = Result<TextRow, ReadError>>,
        from: TextPoint,
        pass: Pass,
    ) -> Result<Option<Hit>, ReadError> {
        let mut recent: VecDeque<(u64, TextRow)> = VecDeque::new();
        let mut consumed = 0u64;
        let mut matched = 0;
        let mut previous = None;
        let mut last = None;
        for row in chunks {
            let row = row?;
            let end = row.end;
            recent.push_back((consumed, row));
            let (_, row) = recent.back().unwrap();
            for (i, b) in row
                .text
                .bytes()
                .chain(row.ending.as_str().bytes())
                .enumerate()
            {
                while matched > 0 && self.bytes[matched] != b {
                    matched = self.prefix[matched - 1];
                }
                if self.bytes[matched] == b {
                    matched += 1;
                }
                consumed += 1;
                if matched == self.bytes.len() {
                    let offset = consumed - matched as u64;
                    let hit = Hit {
                        start: point_at(&recent, offset),
                        end: row.point_at(i + 1).unwrap(),
                    };
                    match pass {
                        Pass::Next if key(hit.start) >= key(from) => return Ok(Some(hit)),
                        Pass::Wrapped => return Ok((key(hit.start) < key(from)).then_some(hit)),
                        Pass::Previous => {
                            last = Some(hit);
                            if key(hit.end) <= key(from) {
                                previous = last;
                            }
                        }
                        _ => {}
                    }
                    matched = self.prefix[matched - 1];
                }
            }
            if matches!(pass, Pass::Previous)
                && previous.is_some()
                && end.restart > from.restart.restart
            {
                return Ok(previous);
            }
            let retain_from = consumed - matched as u64;
            if matches!(pass, Pass::Wrapped)
                && key(end.into()) >= key(from)
                && (matched == 0 || key(point_at(&recent, retain_from)) >= key(from))
            {
                return Ok(None);
            }
            while recent.get(1).is_some_and(|(base, _)| *base <= retain_from) {
                recent.pop_front();
            }
        }
        Ok(previous.or(last))
    }
}

impl TextBook {
    pub fn find(
        &self,
        needle: &str,
        from: &TextPoint,
        direction: Direction,
        stop: &CancellationToken,
    ) -> Result<Option<Hit>, ReadError> {
        self.verify(stop)?;
        if needle.is_empty() {
            return Ok(None);
        }
        let literal = Literal::new(needle.as_bytes(), stop)?;
        let hit = match direction {
            Direction::Previous => literal.scan(self.chunks(stop), *from, Pass::Previous)?,
            Direction::Next => {
                let first = if from.restart.restart < self.source.len {
                    let at = self.visible_start(from.restart, true, stop)?;
                    literal.scan(
                        Chunks::new(self, at, self.source.len, stop),
                        *from,
                        Pass::Next,
                    )?
                } else {
                    None
                };
                if first.is_none() && key(*from) > (self.body, 0) {
                    literal.scan(self.chunks(stop), *from, Pass::Wrapped)?
                } else {
                    first
                }
            }
        };
        self.verify(stop)?;
        Ok(hit)
    }
}
