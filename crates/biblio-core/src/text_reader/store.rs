//! Persistent reader choices and source-byte checkpoints in the library database.
use super::{
    book::{Checkpoint, Encoding, ReadingPosition, SourceStamp},
    rules::{AUTO_ID, ChapterRule, RuleDraft, RuleError, RuleId, SavedRule, builtin_rules},
};
use crate::Library;
use sqlx::{Row, SqliteConnection, types::Json};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxtOpener {
    BuiltIn,
    External(String),
}

impl TxtOpener {
    fn from_program(program: Option<String>) -> Self {
        program.map_or(Self::BuiltIn, Self::External)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    pub family: Option<String>,
    pub size: f32,
    pub leading: f32,
    pub spacing: f32,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            family: None,
            size: 18.0,
            leading: 1.6,
            spacing: 0.0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BookRule {
    #[default]
    Inherit,
    Auto,
    Fixed(RuleId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefaultRule {
    Auto,
    Fixed(RuleId),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BookState {
    pub checkpoint: Option<Checkpoint>,
    pub encoding: Option<Encoding>,
    pub rule: BookRule,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReaderState {
    pub opener: TxtOpener,
    pub typography: Typography,
    pub collapse_blanks: bool,
    pub default_rule: DefaultRule,
    /// The six built-in drafts available to copy, followed by the custom rules.
    pub rules: Vec<SavedRule>,
    /// Present for a requested file, including a book opened for the first time.
    pub book: Option<BookState>,
}

impl ReaderState {
    pub fn effective_rule(&self) -> DefaultRule {
        match self.book.as_ref().map(|book| &book.rule) {
            None | Some(BookRule::Inherit) => self.default_rule.clone(),
            Some(BookRule::Auto) => DefaultRule::Auto,
            Some(BookRule::Fixed(id)) => DefaultRule::Fixed(id.clone()),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Preference {
    Opener(TxtOpener),
    Type(Typography),
    ResetType,
    Collapse(bool),
    Default(DefaultRule),
}

#[derive(Clone, Debug)]
pub enum RuleEdit {
    Save {
        id: Option<RuleId>,
        draft: RuleDraft,
        make_default: bool,
    },
    Remove(RuleId),
}

#[derive(Clone, Debug)]
pub enum BookEdit {
    Position(Checkpoint),
    Encoding(Option<Encoding>),
    Rule(BookRule),
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Rule(#[from] RuleError),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("No editable rule with ID {0}. Reload the rules and try again.")]
    NoEditableRule(RuleId),
    #[error("The selected file or rule no longer exists. Reload and try again.")]
    MissingSelection,
    #[error(transparent)]
    Database(sqlx::Error),
}

impl From<sqlx::Error> for StoreError {
    fn from(error: sqlx::Error) -> Self {
        if error
            .as_database_error()
            .is_some_and(|e| e.is_foreign_key_violation())
        {
            Self::MissingSelection
        } else {
            Self::Database(error)
        }
    }
}

#[derive(Clone)]
pub struct ReaderStore {
    db: Library,
}

impl ReaderStore {
    pub fn new(db: Library) -> Self {
        Self { db }
    }

    /// Resolve an open request without loading the rule library or book state.
    pub async fn opener(&self) -> Result<TxtOpener, StoreError> {
        let program =
            sqlx::query_scalar("SELECT external_app FROM reader_preferences WHERE id = 1")
                .fetch_one(&self.db.pool)
                .await?;
        Ok(TxtOpener::from_program(program))
    }

    pub async fn load(&self, file: Option<i64>) -> Result<ReaderState, StoreError> {
        // Rule deletion can change all three tables. Read them from one snapshot.
        let mut tx = self.db.pool.begin().await?;
        let prefs = sqlx::query(
            "SELECT external_app, font_family, size_pt, line_ratio, spacing_pt, collapse_blanks,
                    default_rule FROM reader_preferences WHERE id = 1",
        )
        .fetch_one(&mut *tx)
        .await?;
        let mut rules = builtin_rules();
        let custom = sqlx::query(
            "SELECT id, name, mode, chapter, volume FROM reader_rules WHERE kind = 'custom'
             ORDER BY name COLLATE NOCASE, id",
        )
        .fetch_all(&mut *tx)
        .await?;
        for row in custom {
            rules.push(SavedRule {
                id: row.try_get("id")?,
                draft: RuleDraft {
                    name: row.try_get("name")?,
                    mode: row.try_get("mode")?,
                    chapter: row.try_get("chapter")?,
                    volume: row.try_get("volume")?,
                },
            });
        }
        let book = match file {
            Some(file) => Some(load_book(&mut tx, file).await?),
            None => None,
        };
        tx.commit().await?;
        let default: Option<String> = prefs.try_get("default_rule")?;
        Ok(ReaderState {
            opener: TxtOpener::from_program(prefs.try_get("external_app")?),
            typography: Typography {
                family: prefs.try_get("font_family")?,
                size: prefs.try_get("size_pt")?,
                leading: prefs.try_get("line_ratio")?,
                spacing: prefs.try_get("spacing_pt")?,
            },
            collapse_blanks: prefs.try_get("collapse_blanks")?,
            default_rule: default
                .filter(|id| id != AUTO_ID)
                .map_or(DefaultRule::Auto, DefaultRule::Fixed),
            rules,
            book,
        })
    }

    pub async fn preference(&self, change: Preference) -> Result<(), StoreError> {
        let update = match change {
            Preference::Opener(opener) => {
                let program = match opener {
                    TxtOpener::BuiltIn => None,
                    TxtOpener::External(program) if program.trim().is_empty() => {
                        return Err(StoreError::Invalid("Choose an external application"));
                    }
                    TxtOpener::External(program) => Some(program),
                };
                sqlx::query("UPDATE reader_preferences SET external_app = ? WHERE id = 1")
                    .bind(program)
            }
            Preference::Type(value) => return self.set_typography(value).await,
            Preference::ResetType => return self.set_typography(Typography::default()).await,
            Preference::Collapse(value) => {
                sqlx::query("UPDATE reader_preferences SET collapse_blanks = ? WHERE id = 1")
                    .bind(value)
            }
            Preference::Default(rule) => {
                let id = match rule {
                    DefaultRule::Auto => None,
                    DefaultRule::Fixed(id) => Some(fixed_id(id)?),
                };
                sqlx::query("UPDATE reader_preferences SET default_rule = ? WHERE id = 1").bind(id)
            }
        };
        update.execute(&self.db.pool).await?;
        Ok(())
    }

    pub async fn edit_rule(&self, edit: RuleEdit) -> Result<Option<RuleId>, StoreError> {
        let (id, draft, make_default) = match edit {
            RuleEdit::Save {
                id,
                draft,
                make_default,
            } => (id, draft, make_default),
            RuleEdit::Remove(id) => {
                let removed =
                    sqlx::query("DELETE FROM reader_rules WHERE id = ? AND kind = 'custom'")
                        .bind(&id)
                        .execute(&self.db.pool)
                        .await?;
                if removed.rows_affected() == 0 {
                    return Err(StoreError::NoEditableRule(id));
                }
                return Ok(None);
            }
        };
        ChapterRule::compile(&draft)?;
        let mut tx = self.db.pool.begin().await?;
        let (id, sql) = match id {
            Some(id) => (
                id,
                "UPDATE reader_rules SET name = ?, mode = ?, chapter = ?, volume = ?
                 WHERE id = ? AND kind = 'custom'",
            ),
            None => (
                uuid::Uuid::new_v4().to_string(),
                "INSERT INTO reader_rules(name, mode, chapter, volume, id, kind)
                 VALUES (?, ?, ?, ?, ?, 'custom')",
            ),
        };
        let saved = sqlx::query(sql)
            .bind(draft.name)
            .bind(draft.mode)
            .bind(draft.chapter)
            .bind(draft.volume)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
        if saved.rows_affected() == 0 {
            return Err(StoreError::NoEditableRule(id));
        }
        if make_default {
            sqlx::query("UPDATE reader_preferences SET default_rule = ? WHERE id = 1")
                .bind(&id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(Some(id))
    }

    pub async fn edit_book(&self, file: i64, change: BookEdit) -> Result<(), StoreError> {
        let update = match change {
            BookEdit::Position(checkpoint) => {
                let offset = |n| {
                    i64::try_from(n).map_err(|_| {
                        StoreError::Invalid("Reading position exceeds SQLite's offset range")
                    })
                };
                sqlx::query(
                    "INSERT INTO reader_books(file_id, paragraph_byte, restart_byte, source_stamp, position_codec)
                     VALUES (?, ?, ?, ?, ?) ON CONFLICT(file_id) DO UPDATE SET
                     paragraph_byte = excluded.paragraph_byte, restart_byte = excluded.restart_byte,
                     source_stamp = excluded.source_stamp, position_codec = excluded.position_codec",
                )
                .bind(file)
                .bind(offset(checkpoint.at.paragraph)?)
                .bind(offset(checkpoint.at.restart)?)
                .bind(Json(checkpoint.source))
                .bind(checkpoint.codec)
            }
            BookEdit::Encoding(encoding) => sqlx::query(
                "INSERT INTO reader_books(file_id, encoding) VALUES (?, ?)
                 ON CONFLICT(file_id) DO UPDATE SET encoding = excluded.encoding",
            )
            .bind(file)
            .bind(encoding),
            BookEdit::Rule(rule) => {
                let id = match rule {
                    BookRule::Inherit => None,
                    BookRule::Auto => Some(AUTO_ID.to_owned()),
                    BookRule::Fixed(id) => Some(fixed_id(id)?),
                };
                sqlx::query(
                    "INSERT INTO reader_books(file_id, rule_id) VALUES (?, ?)
                     ON CONFLICT(file_id) DO UPDATE SET rule_id = excluded.rule_id",
                )
                .bind(file)
                .bind(id)
            }
        };
        update.execute(&self.db.pool).await?;
        Ok(())
    }

    async fn set_typography(&self, typography: Typography) -> Result<(), StoreError> {
        if !(8.0..=72.0).contains(&typography.size) {
            return Err(StoreError::Invalid("Font size must be between 8 and 72 pt"));
        }
        if !(1.0..=3.0).contains(&typography.leading) {
            return Err(StoreError::Invalid("Line height must be between 1 and 3"));
        }
        if !(-1.0..=8.0).contains(&typography.spacing) {
            return Err(StoreError::Invalid(
                "Letter spacing must be between -1 and 8 pt",
            ));
        }
        sqlx::query(
            "UPDATE reader_preferences SET font_family = ?, size_pt = ?, line_ratio = ?, spacing_pt = ? WHERE id = 1",
        )
        .bind(typography.family)
        .bind(typography.size)
        .bind(typography.leading)
        .bind(typography.spacing)
        .execute(&self.db.pool)
        .await?;
        Ok(())
    }
}

fn fixed_id(id: RuleId) -> Result<RuleId, StoreError> {
    if id == AUTO_ID {
        return Err(StoreError::Invalid(
            "Automatic detection is not a fixed chapter rule",
        ));
    }
    Ok(id)
}

async fn load_book(db: &mut SqliteConnection, file: i64) -> Result<BookState, StoreError> {
    let row = sqlx::query(
        "SELECT b.paragraph_byte, b.restart_byte, b.source_stamp, b.position_codec, b.encoding, b.rule_id
         FROM files f LEFT JOIN reader_books b ON b.file_id = f.id WHERE f.id = ?",
    )
    .bind(file)
    .fetch_optional(db)
    .await?
    .ok_or(StoreError::MissingSelection)?;
    let stamp: Option<Json<SourceStamp>> = row.try_get("source_stamp")?;
    let checkpoint = match (stamp, row.try_get("position_codec")?) {
        (None, None) => None,
        (Some(Json(source)), Some(codec)) => Some(Checkpoint {
            at: ReadingPosition {
                paragraph: row.try_get("paragraph_byte")?,
                restart: row.try_get("restart_byte")?,
            },
            source,
            codec,
        }),
        _ => return Err(StoreError::Invalid("Incomplete saved reading position")),
    };
    let rule = match row.try_get::<Option<String>, _>("rule_id")? {
        Some(id) if id == AUTO_ID => BookRule::Auto,
        Some(id) => BookRule::Fixed(id),
        None => BookRule::Inherit,
    };
    Ok(BookState {
        checkpoint,
        encoding: row.try_get("encoding")?,
        rule,
    })
}

#[cfg(test)]
mod tests;
