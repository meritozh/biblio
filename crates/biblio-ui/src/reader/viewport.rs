//! Source-keyed native paragraphs over a bounded set of adjacent book windows.
mod text;

use super::session::ReaderSession;
use crate::ui::*;
use biblio_core::text_reader::{
    book::{Cursor as BookCursor, Hit, ReadingPosition, TextPoint, TextRow, Window},
    rules::{ContentKind, Contents},
};
use nsh_core::prelude::*;
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    ops::Range,
    rc::Rc,
};

struct Block {
    rows: Vec<Rc<TextRow>>,
    before: Option<BookCursor>,
    after: Option<BookCursor>,
}

impl From<Window> for Block {
    fn from(w: Window) -> Self {
        Self {
            rows: w.rows.into_iter().map(Rc::new).collect(),
            before: w.before,
            after: w.after,
        }
    }
}

#[derive(Clone)]
pub(super) struct Pages(VecDeque<Rc<Block>>);

impl Pages {
    pub fn new(window: Window) -> Self {
        Self(VecDeque::from([Rc::new(window.into())]))
    }

    pub fn extend(&mut self, window: Window, before: bool) -> Option<u64> {
        let continuation = |block: &Block| {
            block
                .rows
                .first()
                .filter(|r| r.continues_from_previous())
                .map(|r| r.start.paragraph)
        };
        let mut changed = if before {
            self.0.front().and_then(|b| continuation(b))
        } else {
            None
        };
        let block = Rc::new(Block::from(window));
        if before {
            self.0.push_front(block);
        } else {
            self.0.push_back(block);
        }
        // Sparse projection can leave only one visible line per source
        // window. Keep enough of those light windows to fill a screen.
        while self.0.len() > 3 {
            let retained = if before {
                0..self.0.len() - 1
            } else {
                1..self.0.len()
            };
            let mut lines = 0;
            let mut bytes = 0;
            for block in self.0.range(retained) {
                lines += block.rows.len();
                bytes += block.rows.iter().map(|r| r.text.len()).sum::<usize>();
                if lines >= 128 || bytes >= 256 * 1024 {
                    break;
                }
            }
            if lines < 128 && bytes < 256 * 1024 {
                break;
            }
            if before {
                self.0.pop_back();
            } else {
                self.0.pop_front();
                changed = self.0.front().and_then(|b| continuation(b));
            }
        }
        // NSH preserves the first visible keyed row across list splices.
        // A continued paragraph can change its display-line boundaries.
        changed
    }

    fn paragraphs(&self) -> Vec<Rc<Paragraph>> {
        let mut out = Vec::new();
        let mut current: Option<Paragraph> = None;
        for row in self.0.iter().flat_map(|b| &b.rows) {
            if (!row.continues_from_previous() || row.body_start.is_some())
                && let Some(previous) = current.take()
            {
                out.push(Rc::new(previous));
            }
            let p = current.get_or_insert_with(Paragraph::default);
            p.parts.push((p.len, row.clone()));
            p.len += row.text.len();
        }
        if let Some(p) = current {
            out.push(Rc::new(p));
        }
        out
    }
}

#[derive(Default)]
struct Paragraph {
    parts: Vec<(usize, Rc<TextRow>)>,
    len: usize,
}

impl Paragraph {
    fn start(&self) -> ReadingPosition {
        let row = &self.parts[0].1;
        let mut at = row.start;
        if row.body_start.is_some() {
            at.restart = at.paragraph;
        }
        at
    }

    fn point(&self, offset: usize) -> TextPoint {
        let i = self
            .parts
            .partition_point(|(start, _)| *start <= offset)
            .saturating_sub(1);
        let (start, row) = &self.parts[i];
        row.point_at(offset - start).unwrap_or(row.start.into())
    }

    fn offset(&self, p: TextPoint) -> usize {
        let i = self
            .parts
            .partition_point(|(_, r)| r.start.restart <= p.restart.restart);
        if i == 0 {
            return 0;
        }
        let (base, row) = &self.parts[i - 1];
        base + row
            .offset_of(p)
            .unwrap_or(row.text.len())
            .min(row.text.len())
    }

    fn slice(&self, range: Range<usize>) -> String {
        let mut s = String::with_capacity(range.len());
        for (base, row) in &self.parts {
            let a = range.start.saturating_sub(*base);
            let b = range.end.saturating_sub(*base).min(row.text.len());
            if a < b {
                s.push_str(&row.text[a..b]);
            }
        }
        s
    }

    fn boundary(&self, limit: usize) -> usize {
        if limit >= self.len {
            return self.len;
        }
        let (base, row) = &self.parts[self.parts.partition_point(|(base, _)| *base <= limit) - 1];
        base + row.text.floor_char_boundary(limit - base)
    }
}

#[derive(Clone)]
struct Passage {
    p: Rc<Paragraph>,
    range: Range<usize>,
}
// The book is immutable for the lifetime of this list. A neighboring window
// or a new measured break may move an unchanged passage to a different index.
impl PartialEq for Passage {
    fn eq(&self, other: &Self) -> bool {
        self.break_key() == other.break_key() && self.range.len() == other.range.len()
    }
}

impl Passage {
    fn is_refined_start(&self) -> bool {
        self.p.parts[0].1.body_start == Some(self.range.start)
    }

    fn break_key(&self) -> (u64, Option<u64>) {
        (
            self.start().restart,
            self.is_refined_start()
                .then(|| self.p.point(self.range.start).restart.restart),
        )
    }

    fn indent_hit(&self, hit: Hit) -> bool {
        self.is_refined_start()
            && hit.start.restart.restart < self.p.point(self.range.start).restart.restart
            && hit.end.restart.restart > self.start().restart
    }

    fn start(&self) -> ReadingPosition {
        if self.range.start == self.p.parts[0].1.body_start.unwrap_or(0) {
            self.p.start()
        } else {
            self.p.point(self.range.start).restart
        }
    }
    fn offset(&self, point: TextPoint) -> usize {
        self.p.offset(point).clamp(self.range.start, self.range.end) - self.range.start
    }
}

const COLUMN_WIDTH: f32 = 720.;
const PAGE_MARGIN: f32 = 20.;
const SEGMENT: usize = 32 * 1024;
type Breaks = HashMap<(u64, Option<u64>), usize>;
struct Layout {
    rows: Vec<Passage>,
    probe: Option<Passage>,
}
impl Layout {
    /// Shape at most one more bounded segment per native layout pass. Keep
    /// its final, possibly incomplete display line for the next segment.
    fn new(paragraphs: &[Rc<Paragraph>], breaks: &mut Breaks) -> Self {
        let mut rows = Vec::new();
        let mut probe = None;
        let first = paragraphs.first().map_or(0, |p| p.start().restart);
        let end = paragraphs
            .last()
            .map_or(0, |p| p.parts.last().unwrap().1.end.restart);
        breaks.retain(|(at, _), _| (first..end).contains(at));
        for p in paragraphs {
            let mut start = p.parts[0].1.body_start.unwrap_or(0);
            loop {
                let attachment_bytes = usize::from(p.parts[0].1.body_start == Some(start))
                    * nsh_core::ATTACHMENT_CHAR.len_utf8();
                let limit = p.boundary(start + SEGMENT - attachment_bytes);
                let mut row = Passage {
                    p: p.clone(),
                    range: start..limit,
                };
                if limit < p.len {
                    if let Some(n) = breaks.get(&row.break_key()) {
                        row.range.end = start + *n;
                    } else {
                        probe.get_or_insert_with(|| row.clone());
                        rows.push(row);
                        break;
                    }
                }
                start = row.range.end;
                rows.push(row);
                if start == p.len {
                    break;
                }
            }
        }
        Self { rows, probe }
    }

    fn index(&self, point: TextPoint) -> Option<usize> {
        if self.rows.is_empty() {
            return None;
        }
        Some(
            self.rows
                .partition_point(|r| r.start().restart <= point.restart.restart)
                .saturating_sub(1),
        )
    }
}

fn heading(row: &Passage, contents: Option<&Contents>) -> bool {
    let Some(contents) = contents else {
        return false;
    };
    let paragraph = row.start().paragraph;
    contents
        .entries
        .binary_search_by_key(&paragraph, |entry| entry.anchor.paragraph)
        .is_ok_and(|i| contents.entries[i].kind != ContentKind::FrontMatter)
}

fn byte_for_utf16(s: &str, units: u32) -> usize {
    let mut n = 0;
    for (i, c) in s.char_indices() {
        if n >= units {
            return i;
        }
        n += c.len_utf16() as u32;
    }
    s.len()
}

fn target_line_y(byte: usize, lines: &[(usize, f32)]) -> Option<f32> {
    // The start of a passage is also the top of its list item. A reused
    // native text layout need not send its line metrics again.
    if byte == 0 {
        return Some(0.);
    }
    let i = lines.partition_point(|(start, _)| *start <= byte);
    lines.get(i.saturating_sub(1)).map(|(_, y)| *y)
}

struct Measurement {
    element: ElementRef,
    // UTF-8 start and native y for each display line.
    lines: Vec<(usize, f32)>,
    top: Option<f32>,
}

#[derive(Clone, Copy)]
struct Viewport {
    session: ReaderSession,
    list: ElementRef,
    layout: Signal<Rc<Layout>>,
    measured: Stored<Rc<RefCell<HashMap<u64, Measurement>>>>,
    first: Signal<Option<usize>>,
    offset: Stored<f32>,
    height: Stored<f32>,
    content_height: Stored<f32>,
    positioning: Stored<Option<TextPoint>>,
}

impl Viewport {
    fn visible(self) {
        if self.session.destination.get_untracked().is_some() {
            return;
        }
        let Some(i) = self.first.get_untracked() else {
            return;
        };
        let layout = self.layout.get_untracked();
        let Some(row) = layout.rows.get(i) else {
            return;
        };
        let start = row.start();
        let dy = self.offset.with(|y| *y);
        let byte = self
            .measured
            .with(|m| {
                let map = m.borrow();
                let m = map.get(&start.restart)?;
                let within = dy - m.top?;
                let line = m.lines.partition_point(|(_, y)| *y <= within + 0.5);
                Some(m.lines.get(line.saturating_sub(1))?.0)
            })
            .unwrap_or(0);
        self.session.visible(if byte == 0 {
            start
        } else {
            row.p.point(row.range.start + byte).restart
        });
    }

    fn measure(self, at: u64) {
        let Some(el) = self
            .measured
            .with(|m| m.borrow().get(&at).map(|m| m.element))
        else {
            return;
        };
        spawn(async move {
            let a = el.measure();
            let b = self.list.measure();
            if let (Some(row), Some(list)) = (a.await, b.await) {
                let y = self.offset.with(|y| *y) + row.page.y.0 - list.page.y.0;
                self.measured.with(|m| {
                    if let Some(m) = m.borrow_mut().get_mut(&at)
                        && m.element.node() == el.node()
                    {
                        m.top = Some(y);
                    }
                });
                self.visible();
            }
        });
    }

    fn target(self) {
        let Some(point) = self.session.destination.get_untracked() else {
            return;
        };
        if self.positioning.get() == Some(point) {
            return;
        }
        let data = self.layout.get_untracked();
        let Some(i) = data.index(point) else {
            self.session.destination.set(None);
            return;
        };
        let row = &data.rows[i];
        if data.probe.is_some() && row.range.end < row.p.offset(point) {
            return;
        }
        let off = row.offset(point);
        let line_y = self.measured.with(|m| {
            let map = m.borrow();
            let lines = map.get(&row.start().restart).map(|m| m.lines.as_slice());
            target_line_y(off, lines.unwrap_or_default())
        });
        self.list
            .scroll_to_index(i as u32, Percentage(0.), px(-line_y.unwrap_or(0.)), false);
        if line_y.is_some() {
            self.positioning.replace(Some(point));
            spawn(async move {
                next_frame().await;
                if self.positioning.get() == Some(point) {
                    self.positioning.replace(None);
                    if self.session.destination.get_untracked() == Some(point) {
                        self.session.destination.set(None);
                        self.visible();
                        self.edges();
                    }
                }
            });
        }
    }

    fn cell(self, row: Passage, heading: bool) -> AnyElement {
        let start = row.start().restart;
        let ty = self.session.state.get_untracked().typography.clone();
        let el = element_ref();
        self.measured.with(|m| {
            m.borrow_mut().insert(
                start,
                Measurement {
                    element: el,
                    lines: Vec::new(),
                    top: None,
                },
            )
        });
        let measured = self.measured.get();
        cleanup(move || {
            let mut m = measured.borrow_mut();
            if m.get(&start).is_some_and(|m| m.element == el) {
                m.remove(&start);
            }
        });
        let content = row.p.slice(row.range.clone());
        let content_is_empty = content.is_empty();
        let indent = row.is_refined_start() && !heading;
        let body = row.text(&content, &ty, heading, self.session.hit.get_untracked());
        let first = row.is_refined_start()
            || (row.range.start == 0 && !row.p.parts[0].1.continues_from_previous());
        let last = row.range.end == row.p.len && !row.p.parts.last().unwrap().1.continues_to_next;
        let text = body.element_ref(&el).min_h(px(ty.size * ty.leading)).on(
            EventKind::TextLayout,
            move |payload: &EventPayload| {
                if let EventPayload::TextLayout(lines) = payload {
                    let mut metrics = Vec::with_capacity(lines.len());
                    let mut chars = content.char_indices().peekable();
                    let mut units = u32::from(indent);
                    for line in lines {
                        while units < line.start
                            && let Some((_, c)) = chars.next()
                        {
                            units += c.len_utf16() as u32;
                        }
                        metrics.push((chars.peek().map_or(content.len(), |(i, _)| *i), line.y.0));
                    }
                    self.measured.with(|m| {
                        if let Some(m) = m.borrow_mut().get_mut(&start) {
                            m.lines = metrics;
                        }
                    });
                    self.target();
                    self.measure(start);
                }
            },
        );
        view()
            .items_center()
            .px(px(PAGE_MARGIN))
            .pt(px(if first && heading && start > 0 {
                ty.size * 0.85
            } else {
                0.
            }))
            .pb(px(if last && !content_is_empty {
                ty.size * if heading { 0.55 } else { 0.25 }
            } else {
                0.
            }))
            .child(text.w_full().max_w(px(COLUMN_WIDTH)))
            .into_element()
    }

    fn edges(self) {
        let y = self.offset.get();
        let height = self.height.get();
        let end = self.content_height.get();
        if height <= 0. {
            return;
        }
        if y + height >= end - height * 0.25 {
            self.more(false);
        }
        if y < height * 0.25 {
            self.more(true);
        }
    }

    fn more(self, before: bool) {
        if self.session.destination.get_untracked().is_some()
            || self.layout.with_untracked(|l| l.probe.is_some())
        {
            return;
        }
        let cursor = self.session.pages.with_untracked(|p| {
            if before {
                p.0.front()?.before
            } else {
                p.0.back()?.after
            }
        });
        if let Some(cursor) = cursor {
            self.session.extend(cursor, before);
        }
    }
}

pub fn viewport(session: ReaderSession, list: ElementRef) -> AnyElement {
    // The scrolling surface spans the page. Both visible lines and hidden
    // line-break probes must use the same inset text width.
    let available = signal(0.0_f32);
    let width = computed(move || (available.get() - 2. * PAGE_MARGIN).clamp(0., COLUMN_WIDTH));
    let breaks = stored(Breaks::new());
    let revision = signal(0_u64);
    let paragraphs = signal_with(Rc::new(Vec::new()), SignalEquals::Never);
    effect(move || paragraphs.set(Rc::new(session.pages.with(Pages::paragraphs))));
    let layout = signal_with(
        Rc::new(Layout {
            rows: Vec::new(),
            probe: None,
        }),
        SignalEquals::Never,
    );
    let typography = computed(move || session.state.get().typography.clone());
    let refine = computed(move || session.state.get().collapse_blanks);
    effect(move || {
        width.get();
        typography.get();
        refine.get();
        session.contents.get();
        breaks.with(|b| b.clear());
        if session.destination.get_untracked().is_none()
            && let Some(at) = session.anchor.get_untracked()
        {
            session.destination.set(Some(at.into()));
        }
        revision.update(|n| *n += 1);
    });
    effect(move || {
        revision.get();
        layout.set(Rc::new(breaks.with(|b| Layout::new(&paragraphs.get(), b))));
    });
    let vp = Viewport {
        session,
        list,
        layout,
        measured: stored(Rc::new(RefCell::new(HashMap::new()))),
        first: signal(None),
        offset: stored(0.),
        height: stored(0.),
        content_height: stored(0.),
        positioning: stored(None),
    };
    effect(move || {
        layout.get();
        if session.destination.get().is_some() {
            spawn(async move {
                next_frame().await;
                vp.target();
            });
        }
    });
    // Each new first row releases the previous asynchronous measurement.
    effect(move || {
        if let Some(i) = vp.first.get()
            && let Some(at) = layout.with(|l| l.rows.get(i).map(|r| r.start().restart))
        {
            vp.measure(at);
        }
    });
    watch_scroll(
        move || vp.list.node(),
        move |e| {
            vp.offset.with(|v| *v = e.offset.y.0);
            vp.height.with(|v| *v = e.size.height.0);
            vp.content_height.replace(e.content_size.height.0);
            vp.visible();
            vp.edges();
        },
    );
    let focused = signal(false);
    effect(move || {
        if focused.get() {
            for (key, dir, screen) in [
                ("down", 1., false),
                ("up", -1., false),
                ("right", 1., false),
                ("left", -1., false),
                ("space", 1., true),
                ("pagedown", 1., true),
                ("pageup", -1., true),
            ] {
                on_key_with(
                    key,
                    KeyOptions {
                        when: nsh_core::keymap::When::NotEditing,
                        repeat: true,
                    },
                    move |_| {
                        let step = if screen {
                            vp.height.with(|h| *h)
                        } else {
                            session.state.get_untracked().typography.size
                                * session.state.get_untracked().typography.leading
                        };
                        // A key press takes over from a resume or reflow still
                        // waiting for the target paragraph's native line metrics.
                        vp.positioning.replace(None);
                        session.destination.set(None);
                        vp.list.scroll_to_offset(
                            px((vp.offset.with(|v| *v) + dir * step).max(0.)),
                            false,
                        );
                    },
                );
            }
        }
    });
    let list = virtual_list(
        move || layout.with(|l| l.rows.len()),
        move |at| {
            let item = computed(move || {
                let row = layout.with(|l| l.rows.get(at.index()).cloned())?;
                let hit = session
                    .hit
                    .get()
                    .filter(|h| row.offset(h.start) < row.offset(h.end) || row.indent_hit(*h));
                let heading = session.contents.with(|c| heading(&row, c.as_deref()));
                Some((row, hit, typography.get(), heading))
            });
            dynamic(move || {
                item.get().map_or_else(
                    || view().into_element(),
                    |(row, _, _, heading)| untracked(|| vp.cell(row, heading)),
                )
            })
        },
    )
    .keys(move || layout.with(|l| l.rows.iter().map(|r| r.start().restart).collect::<Vec<_>>()))
    .content_padding_bottom(px(48.))
    .element_ref(&vp.list)
    // AppKit reserves Space for activating a focused pressable control.
    // Put keyboard focus on the reading list when the body is clicked.
    .focusable(true)
    .on_focus(move || focused.set(true))
    .on_blur(move || focused.set(false))
    .test_id("reader-viewport")
    .flex_1()
    .min_h_0()
    .empty(note("This text file is empty"))
    .on_viewable_items_change(move |v: &ViewableItems| {
        if let Some((i, _)) = v.viewable {
            vp.first.set(Some(i));
            vp.visible();
        }
    })
    .on_load_more_at_start(move || vp.more(true))
    .on_load_more(move || vp.more(false));

    let next_probe = computed(move || {
        let row = layout.with(|l| l.probe.clone())?;
        let heading = session.contents.with(|c| heading(&row, c.as_deref()));
        Some((row, width.get(), typography.get(), heading))
    });
    let probe = dynamic(move || {
        let Some((row, w, ty, heading)) = next_probe.get() else {
            return view().into_element();
        };
        let key = row.break_key();
        let start = key.0;
        let content = row.p.slice(row.range.clone());
        if w <= 0. {
            return view().into_element();
        }
        let indent = row.is_refined_start() && !heading;
        row.text(&content, &ty, heading, None)
            .key(format!("measure-{start}"))
            .absolute()
            .top(px(0.))
            .w(px(w))
            .opacity(0.)
            .pointer_events(PointerEvents::None)
            .accessible(false)
            .on(EventKind::TextLayout, move |payload: &EventPayload| {
                if let EventPayload::TextLayout(lines) = payload {
                    let cut = lines
                        .last()
                        .filter(|l| l.start > 0)
                        .map_or(content.len(), |l| {
                            byte_for_utf16(&content, l.start.saturating_sub(u32::from(indent)))
                        });
                    if cut > 0 && breaks.with(|b| b.insert(key, cut) != Some(cut)) {
                        revision.update(|n| *n += 1);
                    }
                }
            })
            .into_element()
    });
    view()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .overflow_hidden()
        .pt(move || px(if compact() { 18. } else { 28. }))
        .label("Book text")
        .focusable(false)
        .on_press(move || vp.list.focus())
        .on_layout(move |b: &Bounds<Points>| available.set(b.size.width.0))
        .child(list)
        .child(probe)
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::target_line_y;

    #[test]
    fn chapter_jump_settles_without_a_new_text_layout_event() {
        assert_eq!(target_line_y(0, &[]), Some(0.));
        assert_eq!(target_line_y(21, &[]), None);
        assert_eq!(target_line_y(21, &[(0, 0.), (18, 29.)]), Some(29.));
    }
}
