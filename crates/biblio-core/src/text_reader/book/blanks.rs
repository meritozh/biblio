//! A run of leading spaces can span several read windows. Resolve the first
//! body character once per paragraph and carry unfinished work in the cursor.
use super::{ReadError, ReadingPosition, TextBook, TextRow, stream::Chunks};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug)]
pub(super) enum Prefix {
    Blank,
    Body(u64),
}

fn prefix(row: &TextRow) -> Option<Prefix> {
    if let Some(at) = row.text.find(|c: char| !c.is_whitespace()) {
        return Some(Prefix::Body(row.point_at(at)?.restart.restart));
    }
    (!row.continues_to_next).then_some(Prefix::Blank)
}

#[derive(Clone, Copy, Debug)]
struct Probe {
    paragraph: u64,
    next: u64,
    answer: Option<Prefix>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct BlankContext(Option<Probe>);

impl BlankContext {
    pub(super) fn prefix(
        &mut self,
        book: &TextBook,
        row: &TextRow,
        budget: &mut usize,
        stop: &CancellationToken,
    ) -> Result<Option<Prefix>, ReadError> {
        if !row.continues_from_previous() && !row.continues_to_next {
            return Ok(prefix(row));
        }
        let paragraph = row.start.paragraph;
        if self.0.is_none_or(|p| p.paragraph != paragraph) {
            self.0 = Some(Probe {
                paragraph,
                next: paragraph,
                answer: None,
            });
        }
        let probe = self.0.as_mut().unwrap();
        if probe.answer.is_none() && probe.next == row.start.restart {
            probe.answer = prefix(row);
            probe.next = row.end.restart;
        }
        if probe.answer.is_none() {
            let at = ReadingPosition {
                paragraph,
                restart: probe.next,
            };
            let mut chunks = Chunks::new(book, at, book.source.len, stop);
            while *budget > 0 {
                let Some(chunk) = chunks.next().transpose()? else {
                    probe.answer = Some(Prefix::Blank);
                    break;
                };
                *budget = budget.saturating_sub((chunk.end.restart - probe.next) as usize);
                probe.next = chunk.end.restart;
                probe.answer = prefix(&chunk);
                if probe.answer.is_some() {
                    break;
                }
            }
        }
        Ok(probe.answer)
    }
}
