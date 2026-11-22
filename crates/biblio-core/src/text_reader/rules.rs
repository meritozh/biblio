//! Chapter recognition shared by the rule editor, rule cards, and table of contents.
mod presets;
mod scan;

use fancy_regex::{Regex, RegexBuilder, RuntimeError};
use thiserror::Error;

pub use presets::{AUTO_ID, builtin_rules, preset_example};
pub use scan::{
    ContentEntry, ContentKind, Contents, RuleCard, RuleStats, RuleSurvey, ScanError, Title,
    outline, survey,
};

pub type RuleId = String;

#[derive(Clone, Copy, Debug, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum PatternMode {
    Template,
    Regex,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleDraft {
    pub name: String,
    pub mode: PatternMode,
    pub chapter: String,
    pub volume: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedRule {
    pub id: RuleId,
    pub draft: RuleDraft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heading {
    Volume,
    Chapter,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RuleError {
    #[error("{field:?} expression: {message}")]
    Expression { field: Heading, message: String },
    #[error("{field:?} expression exceeded the matching budget")]
    MatchBudget { field: Heading },
    #[error("Enter one line of sample text")]
    MultilineSample,
}

const COMPILE_BYTES: usize = 2 * 1024 * 1024;
const BACKTRACK_LIMIT: usize = 100_000;
const NUMBER: &str = "(?:[0-9０-９]+|[零〇○一二两兩三四五六七八九十百千万萬亿億兆壹贰貳貮叁參肆伍陆陸柒捌玖拾佰仟廿卅]+)";
const TITLE: &str = r".*\S.*";

#[derive(Clone, Debug)]
pub struct ChapterRule {
    chapter: Regex,
    volume: Option<Regex>,
}

impl ChapterRule {
    pub fn compile(draft: &RuleDraft) -> Result<Self, RuleError> {
        let chapter = compile_expression(draft.mode, Heading::Chapter, &draft.chapter)?;
        let volume = draft
            .volume
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| compile_expression(draft.mode, Heading::Volume, s))
            .transpose()?;
        Ok(Self { chapter, volume })
    }

    pub fn probe(&self, line: &str) -> Result<Option<Heading>, RuleError> {
        let line = line
            .strip_suffix("\r\n")
            .or_else(|| line.strip_suffix(['\r', '\n']))
            .unwrap_or(line);
        if line.contains(['\r', '\n']) {
            return Err(RuleError::MultilineSample);
        }
        self.match_line(line)
    }

    fn match_line(&self, line: &str) -> Result<Option<Heading>, RuleError> {
        if let Some(volume) = &self.volume
            && matches_expression(volume, Heading::Volume, line)?
        {
            return Ok(Some(Heading::Volume));
        }
        Ok(matches_expression(&self.chapter, Heading::Chapter, line)?.then_some(Heading::Chapter))
    }
}

fn matches_expression(regex: &Regex, field: Heading, line: &str) -> Result<bool, RuleError> {
    regex.is_match(line).map_err(|error| match error {
        fancy_regex::Error::RuntimeError(
            RuntimeError::BacktrackLimitExceeded | RuntimeError::StackOverflow,
        ) => RuleError::MatchBudget { field },
        other => RuleError::Expression {
            field,
            message: other.to_string(),
        },
    })
}

fn compile_expression(mode: PatternMode, field: Heading, source: &str) -> Result<Regex, RuleError> {
    let invalid = |message: &str| RuleError::Expression {
        field,
        message: message.into(),
    };
    if source.len() > COMPILE_BYTES {
        return Err(invalid("Expression exceeds the 2 MiB compilation limit"));
    }
    if source.contains(['\r', '\n']) {
        return Err(invalid("Enter a single-line expression"));
    }
    if source.trim().is_empty() {
        return Err(invalid("Enter a heading expression"));
    }
    let pattern = match mode {
        PatternMode::Template => template_pattern(source).map_err(invalid)?,
        PatternMode::Regex => source.into(),
    };
    RegexBuilder::new(&pattern)
        .delegate_size_limit(COMPILE_BYTES)
        .max_prog_size(32_768) // VM instructions, independently of the delegated regex size in bytes
        .backtrack_limit(BACKTRACK_LIMIT)
        .build()
        .map_err(|error| invalid(&error.to_string()))
}

fn template_pattern(mut rest: &str) -> Result<String, &'static str> {
    let mut pattern = String::from(r"\A");
    while !rest.is_empty() {
        if let Some(tail) = rest.strip_prefix("{序号}") {
            pattern.push_str(NUMBER);
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("{标题}") {
            pattern.push_str(TITLE);
            rest = tail;
        } else if rest.starts_with(['{', '}']) {
            return Err("Unknown or incomplete placeholder");
        } else if rest.starts_with(char::is_whitespace) {
            pattern.push_str(r"[ \t]+");
            rest = rest.trim_start_matches(char::is_whitespace);
        } else {
            let n = rest
                .find(|c: char| matches!(c, '{' | '}') || c.is_whitespace())
                .unwrap_or(rest.len());
            pattern.push_str(&regex::escape(&rest[..n]));
            rest = &rest[n..];
        }
        if pattern.len() + 2 > COMPILE_BYTES {
            return Err("Template exceeds the 2 MiB compilation limit");
        }
    }
    pattern.push_str(r"\z");
    Ok(pattern)
}

#[cfg(test)]
mod tests;
