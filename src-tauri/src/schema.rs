//! Category-driven schema slug.
//!
//! Mirrors `src/lib/categorySchema.ts` on the frontend. Each category
//! row carries one of these slugs in its `schema_slug` column; prompts
//! pair `(schema_slug, step)` to pick which LLM rules to run for a
//! given pipeline step.
//!
//! New schemas are added in two places (here + the TS registry). Adding
//! one without the other will compile but fall back to `Novel` at the
//! Rust→DB boundary, which is the safest default for unknown user data.

use serde::{Deserialize, Serialize};

/// Built-in schema identifier. Stored as a TEXT column in SQLite via
/// `as_str` and read back via `from_str`. Case-insensitive; unknown
/// values fall back to `Novel` so a stale `schema_slug` from a row
/// written by an older binary doesn't crash the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaSlug {
    Novel,
    Comic,
}

impl SchemaSlug {
    pub fn as_str(self) -> &'static str {
        match self {
            SchemaSlug::Novel => "novel",
            SchemaSlug::Comic => "comic",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "comic" => SchemaSlug::Comic,
            // Default for any unknown value so a category row written by
            // a future binary that introduced a new slug doesn't error
            // here — it just renders with novel defaults until the
            // registry catches up.
            _ => SchemaSlug::Novel,
        }
    }

    /// True if `s` parses to a known slug exactly. Used by command-layer
    /// validation when accepting user input from a Tauri command, which
    /// should reject typos rather than silently falling back.
    pub fn is_known(s: &str) -> bool {
        matches!(s.to_ascii_lowercase().as_str(), "novel" | "comic")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_known() {
        for slug in [SchemaSlug::Novel, SchemaSlug::Comic] {
            assert_eq!(SchemaSlug::from_str(slug.as_str()), slug);
        }
    }

    #[test]
    fn from_str_falls_back_to_novel_for_unknown() {
        assert_eq!(SchemaSlug::from_str(""), SchemaSlug::Novel);
        assert_eq!(SchemaSlug::from_str("manga"), SchemaSlug::Novel);
    }

    #[test]
    fn is_known_excludes_fallback_targets() {
        assert!(SchemaSlug::is_known("novel"));
        assert!(SchemaSlug::is_known("Comic"));
        assert!(!SchemaSlug::is_known("manga"));
        assert!(!SchemaSlug::is_known(""));
    }
}
