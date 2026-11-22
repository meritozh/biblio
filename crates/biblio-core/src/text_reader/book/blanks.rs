//! A long prefix of spaces may be indentation or an entirely blank paragraph.
//! Carry unfinished checks in the paging cursor so either case yields after
//! bounded I/O. A page can have a continuation cursor and no visible rows.
use super::{BLOCK, ReadError, TextBook, TextRow};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug)]
struct Probe {
    paragraph: u64,
    next: u64,
    answer: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct BlankContext(Option<Probe>);

impl BlankContext {
    pub(super) fn blank(
        &mut self,
        book: &TextBook,
        row: &TextRow,
        budget: &mut usize,
        stop: &CancellationToken,
    ) -> Result<Option<bool>, ReadError> {
        let spaces = row.text.bytes().all(|b| matches!(b, b' ' | b'\t'));
        if !row.continues_from_previous() && !row.continues_to_next {
            return Ok(Some(spaces));
        }
        let para = row.start.paragraph;
        if self.0.is_none_or(|p| p.paragraph != para) {
            self.0 = Some(Probe {
                paragraph: para,
                next: para,
                answer: None,
            });
        }
        let probe = self.0.as_mut().unwrap();
        if !spaces {
            probe.answer = Some(false);
        }
        let mut buf = Vec::new();
        let stride = book.codec.stride();
        while probe.answer.is_none() {
            let remaining = book.source.len - probe.next;
            if remaining == 0 {
                probe.answer = Some(true);
                break;
            }
            if *budget < stride {
                break;
            }
            let n = remaining.min(BLOCK.min(*budget - *budget % stride) as u64) as usize;
            book.read(probe.next, n, &mut buf, stop)?;
            *budget -= n;
            probe.next += n as u64;
            probe.answer = buf.chunks(stride).find_map(|s| match book.codec.ascii(s) {
                Some(b' ' | b'\t') => None,
                Some(b'\r' | b'\n') => Some(true),
                _ => Some(false),
            });
        }
        Ok(probe.answer)
    }
}
