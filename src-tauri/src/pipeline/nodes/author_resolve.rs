use std::collections::HashSet;

use async_trait::async_trait;
use unicode_normalization::UnicodeNormalization;

use crate::pipeline::{FileContext, NodeError, Phase2Node, PipelineEnv};

/// Resolve every name collected in `ctx.suggested_author_names` — whether
/// surfaced by ParentDirAuthorHint, the filename LLM, or any other Phase-1
/// processor — against `env.author_map`. Hits become `author_ids`, misses
/// become `unresolved_authors`. Runs once, near the end of Phase 2.
///
/// `suggested_author_names` is deduped under an NFC + lowercase key first,
/// since folder and filename sources can independently surface the same
/// author in different cases (`SAVAN` vs `Savan`) or different Unicode
/// forms (NFD `フ`+`゙` from APFS-derived paths vs NFC `ブ` from the LLM).
/// The first occurrence's casing is kept; we also store its NFC form so
/// a downstream `author_create` lands the same byte sequence the
/// `env.author_map` was built from.
pub struct AuthorResolveNode;

#[async_trait]
impl Phase2Node for AuthorResolveNode {
    fn name(&self) -> &'static str {
        "AuthorResolve"
    }

    fn applies(&self, ctx: &FileContext, _env: &PipelineEnv) -> bool {
        !ctx.suggested_author_names.is_empty()
    }

    async fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut deduped: Vec<String> = Vec::new();
        for name in std::mem::take(&mut ctx.suggested_author_names) {
            let nfc: String = name.nfc().collect::<String>().trim().to_string();
            if nfc.is_empty() {
                continue;
            }
            let key = nfc.to_lowercase();
            if seen.insert(key) {
                deduped.push(nfc);
            }
        }
        ctx.suggested_author_names = deduped;

        for name in &ctx.suggested_author_names {
            // `name` is already NFC-trimmed; lowercase only to match the
            // env.author_map keys built in `file_prepare_import`.
            if let Some(&id) = env.author_map.get(&name.to_lowercase()) {
                if !ctx.author_ids.contains(&id) {
                    ctx.author_ids.push(id);
                }
            } else if !ctx.unresolved_authors.contains(name) {
                ctx.unresolved_authors.push(name.clone());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use unicode_normalization::UnicodeNormalization;

    /// Faithful copy of the dedup pass at the top of `AuthorResolveNode::run`.
    /// Exercised here without a `PipelineEnv` (which needs a real sqlite
    /// pool) since the dedup itself is pure.
    fn dedup(input: Vec<&str>) -> Vec<String> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut out: Vec<String> = Vec::new();
        for name in input {
            let nfc: String = name.nfc().collect::<String>().trim().to_string();
            if nfc.is_empty() {
                continue;
            }
            if seen.insert(nfc.to_lowercase()) {
                out.push(nfc);
            }
        }
        out
    }

    #[test]
    fn dedup_collapses_case_variants() {
        assert_eq!(dedup(vec!["SAVAN", "Savan", "savan"]), vec!["SAVAN"]);
    }

    #[test]
    fn dedup_collapses_whitespace_variants() {
        assert_eq!(dedup(vec!["  作者  ", "作者", "作者 "]), vec!["作者"]);
    }

    #[test]
    fn dedup_drops_empty_and_whitespace_only() {
        assert_eq!(dedup(vec!["", "   ", "作者"]), vec!["作者"]);
    }

    #[test]
    fn dedup_preserves_first_occurrence_casing() {
        assert_eq!(dedup(vec!["Author", "AUTHOR", "author"]), vec!["Author"]);
    }

    #[test]
    fn dedup_keeps_distinct_names() {
        assert_eq!(dedup(vec!["A", "B", "a"]), vec!["A", "B"]);
    }

    /// Regression for the visible bug: same Japanese name surfaced in NFD
    /// (decomposed katakana voicing mark, as APFS stores it) from a folder
    /// path AND in NFC (precomposed) from the LLM extraction collapse to
    /// one chip after dedup. Without NFC folding the dialog showed two
    /// identical-looking author chips that resolved to two distinct DB rows.
    #[test]
    fn dedup_collapses_nfc_and_nfd_variants() {
        // `ブ` precomposed (NFC) vs `フ` + combining voicing mark (NFD).
        let nfc = "ブッパスタジオ";
        let nfd: String = nfc.nfd().collect();
        assert_ne!(nfc, nfd.as_str(), "test inputs must differ byte-wise");
        let result = dedup(vec![nfc, nfd.as_str()]);
        assert_eq!(result, vec![nfc.to_string()]);
    }
}
