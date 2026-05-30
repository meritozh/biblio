use async_trait::async_trait;

use crate::pipeline::{
    DuplicateAction, DuplicateInfo, FileContext, NodeError, Phase2Node, PipelineEnv,
};

/// Minimum prefix-similarity ratio for two display names to be considered
/// the same work. Pairs whose common prefix (in Unicode characters) exceeds
/// this fraction of the shorter name's length trigger a duplicate warning.
/// Lets "三体" match "三体 完结" while keeping genuinely different titles
/// like "三体 第一部" / "三体 第二部" (ratio 0.6) apart.
pub(crate) const DUPLICATE_PREFIX_THRESHOLD: f64 = 0.8;

/// Look up a prefix-similar display_name among already-imported files and
/// attach a `DuplicateInfo` with a suggested `Replace`/`Delete` action.
pub struct DbDuplicateDetectNode;

#[async_trait]
impl Phase2Node for DbDuplicateDetectNode {
    fn name(&self) -> &'static str {
        "DbDuplicateDetect"
    }

    async fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError> {
        let display = ctx
            .display_name
            .as_deref()
            .unwrap_or(&ctx.file_name)
            .trim()
            .to_lowercase();
        if display.is_empty() {
            return Ok(());
        }

        let existing_match = env.existing_files.iter().find(|f| {
            // A same-named file in a different category is a different
            // work (e.g. a novel and a comic that share a title). Only
            // filter when both sides have a known category — comics
            // currently leave `category_id` empty in the pipeline, so a
            // None on either side falls through to the name check.
            if let (Some(new_cat), Some(existing_cat)) =
                (ctx.category_id, f.category_id)
            {
                if new_cat != existing_cat {
                    return false;
                }
            }
            let existing = f.display_name.trim().to_lowercase();
            prefix_similarity(&display, &existing) > DUPLICATE_PREFIX_THRESHOLD
        });

        let Some(existing) = existing_match else {
            return Ok(());
        };

        let recommendation = match (&ctx.progress, &existing.progress) {
            (Some(new_p), Some(old_p)) if progress_at_least(new_p, old_p) => {
                DuplicateAction::Replace
            }
            (Some(_), None) => DuplicateAction::Replace,
            (None, Some(_)) => DuplicateAction::Delete,
            _ => DuplicateAction::Replace,
        };
        // On-disk size lookup. Best-effort: a missing file or an
        // unreadable path renders as None in the UI ("—") so the
        // user knows we couldn't resolve a size — distinct from a
        // genuine zero-byte file.
        //
        // Existing rows store paths RELATIVE to either
        // `storage_path` (local) or `app_root` (remote, though
        // remote rows aren't on local disk so stat() will fail
        // and return None — correct, we render "—").
        let existing_kind = existing.storage_kind.as_deref().unwrap_or("local");
        let existing_abs = crate::path_resolve::to_absolute(
            existing_kind,
            &existing.path,
            &env.storage_path,
            &env.app_root,
        );
        let existing_size = std::fs::metadata(&existing_abs)
            .ok()
            .map(|m| m.len() as i64);
        let new_size = if ctx.file_path.is_dir() {
            // Folder-to-zip imports have no meaningful "file size"
            // until the archive is produced post-commit. Skip.
            None
        } else {
            std::fs::metadata(&ctx.file_path)
                .ok()
                .map(|m| m.len() as i64)
        };

        // Author lookup happens only on a dupe hit — one query per match,
        // not per row in `existing_files`. Dupes are rare per batch so
        // the per-hit cost is negligible compared to pre-loading authors
        // for every existing row at env-build time.
        let author_rows: Vec<(String,)> = sqlx::query_as(
            "SELECT a.name FROM authors a
             JOIN file_authors fa ON fa.author_id = a.id
             WHERE fa.file_id = ?
             ORDER BY a.name",
        )
        .bind(existing.id)
        .fetch_all(&env.pool)
        .await
        .map_err(|e| NodeError(format!("dupe author lookup: {e}")))?;
        let existing_author_names: Vec<String> =
            author_rows.into_iter().map(|(n,)| n).collect();

        ctx.duplicate_of = Some(DuplicateInfo {
            existing_file_id: existing.id,
            existing_display_name: existing.display_name.clone(),
            existing_progress: existing.progress.clone(),
            existing_size,
            new_size,
            existing_author_names,
            recommendation,
        });
        Ok(())
    }
}

/// Whether `new_p` represents progress at least as far as `old_p`,
/// used to decide whether the incoming file should `Replace` the
/// existing one. Progress is stored as a free-form string (e.g.
/// "10", "完结", "第 12 话"), but the common case is a chapter/volume
/// number — comparing those as raw strings is wrong ("9" >= "10"
/// lexically). Parse the leading integer from each side and compare
/// numerically; when either side has no leading number, fall back to
/// the previous lexical comparison.
fn progress_at_least(new_p: &str, old_p: &str) -> bool {
    match (leading_number(new_p), leading_number(old_p)) {
        (Some(new_n), Some(old_n)) => new_n >= old_n,
        _ => new_p >= old_p,
    }
}

/// Parse the leading run of ASCII digits from a progress string into a
/// number, ignoring leading whitespace. Returns `None` when no digit
/// starts the (trimmed) string, signalling the caller to fall back.
fn leading_number(s: &str) -> Option<u64> {
    let digits: String = s
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Prefix-similarity ratio between two already-normalized names:
/// `common_prefix_chars / min(len_a, len_b)`. Returns 0.0 if either is empty.
pub(crate) fn prefix_similarity(a: &str, b: &str) -> f64 {
    let shorter = a.chars().count().min(b.chars().count());
    if shorter == 0 {
        return 0.0;
    }
    let common = a
        .chars()
        .zip(b.chars())
        .take_while(|(ca, cb)| ca == cb)
        .count();
    common as f64 / shorter as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_is_full_similarity() {
        assert_eq!(prefix_similarity("三体", "三体"), 1.0);
        assert_eq!(prefix_similarity("abc", "abc"), 1.0);
    }

    #[test]
    fn shorter_fully_contained_crosses_threshold() {
        let ratio = prefix_similarity("三体", "三体 完结");
        assert!(ratio > DUPLICATE_PREFIX_THRESHOLD);
        assert!((ratio - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn early_divergence_stays_below_threshold() {
        let ratio = prefix_similarity("三体 第一部", "三体 第二部");
        assert!(ratio < DUPLICATE_PREFIX_THRESHOLD);
    }

    #[test]
    fn empty_strings_are_never_similar() {
        assert_eq!(prefix_similarity("", "三体"), 0.0);
        assert_eq!(prefix_similarity("三体", ""), 0.0);
        assert_eq!(prefix_similarity("", ""), 0.0);
    }

    #[test]
    fn no_common_prefix_is_zero() {
        assert_eq!(prefix_similarity("abc", "xyz"), 0.0);
    }
}
