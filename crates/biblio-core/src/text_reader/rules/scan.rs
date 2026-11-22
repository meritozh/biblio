use super::presets::PRESETS;
use super::{AUTO_ID, ChapterRule, Heading, RuleError, RuleId, SavedRule, builtin_rules};
use crate::text_reader::book::{LineRange, ReadError, ReadingPosition, TextBook, check_stop};
use std::borrow::Cow;
use tokio_util::sync::CancellationToken;

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error(transparent)]
    Rule(#[from] RuleError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Title {
    Text(String),
    Source(LineRange),
}

impl Title {
    /// Resolve a title for display using the same book and decoder that produced it.
    pub fn resolve(
        &self,
        book: &TextBook,
        stop: &CancellationToken,
    ) -> Result<Cow<'_, str>, ReadError> {
        check_stop(stop)?;
        match self {
            Self::Text(text) => Ok(Cow::Borrowed(text)),
            Self::Source(range) => Ok(Cow::Owned(range.text(book, stop)?)),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuleStats {
    pub chapters: u64,
    pub example: Option<Title>,
}

#[derive(Clone, Debug)]
pub struct RuleCard {
    pub id: RuleId,
    pub name: String,
    pub stats: Result<RuleStats, RuleError>,
}

#[derive(Clone, Debug)]
pub struct RuleSurvey {
    /// The automatic card comes first and shares the detected preset's statistics.
    pub cards: Vec<RuleCard>,
    /// Identifies the fixed card containing the selected preset's display name.
    pub detected: Option<RuleId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentKind {
    FrontMatter,
    Volume,
    Chapter,
}

#[derive(Clone, Debug)]
pub struct ContentEntry {
    pub kind: ContentKind,
    pub title: Title,
    pub anchor: ReadingPosition,
    /// Non-whitespace Unicode scalars following this entry, up to the next heading.
    pub words: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Contents {
    pub entries: Vec<ContentEntry>,
    pub chapters: u64,
}

struct Line<'a> {
    start: ReadingPosition,
    end: u64,
    text: &'a str,
}

impl Line<'_> {
    fn title(&self, book: &TextBook) -> Title {
        if self.text.len() > 512 {
            Title::Source(LineRange::new(book, self.start, self.end))
        } else {
            Title::Text(self.text.into())
        }
    }
}

// Most paragraphs borrow the decoded chunk directly. Only a continued paragraph
// needs a second buffer, so regex anchors always see the entire logical line.
fn scan_lines(
    book: &TextBook,
    stop: &CancellationToken,
    mut visit: impl FnMut(Line<'_>) -> Result<(), ScanError>,
) -> Result<(), ScanError> {
    let mut joined = String::new();
    for chunk in book.chunks(stop) {
        let chunk = chunk?;
        let text = if chunk.continues_to_next || chunk.continues_from_previous() {
            joined.push_str(&chunk.text);
            if chunk.continues_to_next {
                continue;
            }
            &joined
        } else {
            &chunk.text
        };
        let at = chunk.start.paragraph;
        visit(Line {
            start: ReadingPosition {
                paragraph: at,
                restart: at,
            },
            end: chunk.end.restart,
            text,
        })?;
        joined.clear();
    }
    Ok(())
}

/// Survey every built-in preset and supplied custom rule in one pass over the book.
/// A compilation or matching failure belongs only to the affected card.
pub fn survey(
    book: &TextBook,
    rules: &[SavedRule],
    stop: &CancellationToken,
) -> Result<RuleSurvey, ScanError> {
    let builtins = builtin_rules();
    let custom = rules
        .iter()
        .filter(|r| r.id != AUTO_ID && !PRESETS.iter().any(|p| p.id == r.id));
    let mut cards = Vec::new();
    let mut matchers = Vec::new();
    for rule in builtins.iter().chain(custom) {
        check_stop(stop)?;
        let compiled = ChapterRule::compile(&rule.draft);
        let (matcher, stats) = match compiled {
            Ok(matcher) => (Some(matcher), Ok(RuleStats::default())),
            Err(error) => (None, Err(error)),
        };
        matchers.push(matcher);
        cards.push(RuleCard {
            id: rule.id.clone(),
            name: rule.draft.name.clone(),
            stats,
        });
    }

    scan_lines(book, stop, |line| {
        for (matcher, card) in matchers.iter().zip(&mut cards) {
            check_stop(stop)?;
            if let (Some(matcher), Ok(stats)) = (matcher, &mut card.stats) {
                match matcher.match_line(line.text) {
                    Ok(Some(Heading::Chapter)) => {
                        stats.chapters += 1;
                        stats.example.get_or_insert_with(|| line.title(book));
                    }
                    Ok(_) => {}
                    Err(error) => card.stats = Err(error),
                }
            }
        }
        Ok(())
    })?;

    let mut best = None;
    for (index, (preset, card)) in PRESETS.iter().zip(&mut cards).enumerate() {
        if let Ok(stats) = &mut card.stats {
            if stats.chapters == 0 {
                stats.example = Some(Title::Text(preset.example.into()));
            } else {
                let score = (preset.explicit, stats.chapters);
                if best.is_none_or(|(previous, _)| score > previous) {
                    best = Some((score, index));
                }
            }
        }
    }
    let winner = best.map(|(_, index)| &cards[index]);
    let detected = winner.map(|card| card.id.clone());
    let auto = RuleCard {
        id: AUTO_ID.into(),
        name: "自动检测".into(),
        stats: winner.map_or_else(|| Ok(RuleStats::default()), |card| card.stats.clone()),
    };
    cards.insert(0, auto);
    check_stop(stop)?;
    Ok(RuleSurvey { cards, detected })
}

pub fn outline(
    book: &TextBook,
    rule: &ChapterRule,
    stop: &CancellationToken,
) -> Result<Contents, ScanError> {
    let mut contents = Contents::default();
    let mut front_start = None;
    let mut front_words = 0;
    scan_lines(book, stop, |line| {
        match rule.match_line(line.text)? {
            Some(heading) => {
                if let Some(anchor) = front_start.take()
                    && front_words > 0
                {
                    contents.entries.push(ContentEntry {
                        kind: ContentKind::FrontMatter,
                        title: Title::Text("扉页".into()),
                        anchor,
                        words: front_words,
                    });
                }
                let kind = match heading {
                    Heading::Volume => ContentKind::Volume,
                    Heading::Chapter => {
                        contents.chapters += 1;
                        ContentKind::Chapter
                    }
                };
                contents.entries.push(ContentEntry {
                    kind,
                    title: line.title(book),
                    anchor: line.start,
                    words: 0,
                });
            }
            None => {
                let words = count_words(line.text, stop)?;
                if let Some(entry) = contents.entries.last_mut() {
                    entry.words += words;
                } else {
                    front_start.get_or_insert(line.start);
                    front_words += words;
                }
            }
        }
        Ok(())
    })?;
    Ok(contents)
}

fn count_words(mut text: &str, stop: &CancellationToken) -> Result<u64, ReadError> {
    let mut words = 0;
    while !text.is_empty() {
        check_stop(stop)?;
        let end = text.floor_char_boundary(64 * 1024);
        let (batch, rest) = text.split_at(end);
        words += batch.chars().filter(|c| !c.is_whitespace()).count() as u64;
        text = rest;
    }
    Ok(words)
}
