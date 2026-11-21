use async_trait::async_trait;
use unicode_normalization::UnicodeNormalization;

use crate::commands::FileEntry;
use crate::pipeline::{
    DuplicateAction, DuplicateInfo, FileContext, NodeError, Phase2Node, PipelineEnv,
};

/// Look up the same canonical title among already-imported files and attach a
/// `DuplicateInfo` with a suggested `Replace`/`Delete` action.
///
/// Serialization state belongs in `progress`, so title suffixes are not fuzzy
/// duplicate evidence: `错位` and `错位愈合` are different works even though
/// one is a full prefix of the other.
pub struct DbDuplicateDetectNode;

#[async_trait]
impl Phase2Node for DbDuplicateDetectNode {
    fn name(&self) -> &'static str {
        "DbDuplicateDetect"
    }

    async fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError> {
        let display = incoming_title(ctx);
        let title_key = canonical_title(display);
        if title_key.is_empty() {
            return Ok(());
        }

        let existing_match = find_best_duplicate(
            &title_key,
            env.requested_category_id,
            ctx.category_id,
            &env.existing_files,
        );

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
        let existing_author_names: Vec<String> = author_rows.into_iter().map(|(n,)| n).collect();

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

/// Use the same fallback the prepared import persists when filename analysis
/// is disabled or fails. Keeping the extension here matters: such rows store
/// the full `file_name` as their display name, not the path's file stem.
fn incoming_title(ctx: &FileContext) -> &str {
    ctx.display_name.as_deref().unwrap_or(&ctx.file_name)
}

/// Canonical title identity shared by incoming and stored display names.
/// NFC mirrors the rest of the catalog-name paths, so APFS-derived combining
/// sequences compare equal to the precomposed form normally returned by the
/// LLM. Deliberately do not strip punctuation, subtitles, sequel markers, or
/// arbitrary suffixes: those can distinguish different works.
fn canonical_title(name: &str) -> String {
    name.nfc().collect::<String>().trim().to_lowercase()
}

/// Return the most recently updated exact-title row. The import review/API
/// exposes one candidate, so exact duplicate rows need a deterministic tie
/// rule; replacement itself creates a fresh row, making the newest row the
/// best representation of the current serialized copy.
fn find_best_duplicate<'a>(
    title_key: &str,
    requested_category_id: Option<i64>,
    extracted_category_id: Option<i64>,
    existing_files: &'a [FileEntry],
) -> Option<&'a FileEntry> {
    // The category the user selected to start the import is authoritative.
    // Content analysis supplies a fallback only for legacy/no-target callers.
    let category_id = requested_category_id.or(extracted_category_id);
    existing_files
        .iter()
        .filter(|file| {
            // A same-named file in a different category is a different work
            // (for example, a novel and a comic sharing a title). Keep the
            // existing fallback when either category is unknown.
            !matches!(
                (category_id, file.category_id),
                (Some(incoming), Some(existing)) if incoming != existing
            )
        })
        .filter(|file| canonical_title(&file.display_name) == title_key)
        .max_by(|left, right| {
            category_precision(category_id, left.category_id)
                .cmp(&category_precision(category_id, right.category_id))
                .then_with(|| left.updated_at.cmp(&right.updated_at))
                .then_with(|| left.id.cmp(&right.id))
        })
}

/// Prefer a candidate explicitly in the selected/extracted category over a
/// legacy row whose category is unknown. Unknown rows remain eligible only as
/// a fallback when no exact-category title exists.
fn category_precision(category_id: Option<i64>, candidate_category_id: Option<i64>) -> u8 {
    u8::from(category_id.is_some() && category_id == candidate_category_id)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: i64, display_name: &str, category_id: Option<i64>, updated_at: &str) -> FileEntry {
        FileEntry {
            id,
            path: format!("{id}.txt"),
            display_name: display_name.to_string(),
            category_id,
            file_status: "available".to_string(),
            in_storage: true,
            original_path: None,
            progress: None,
            storage_kind: Some("local".to_string()),
            remote_provider: None,
            local_cache_path: None,
            is_favorite: false,
            created_at: updated_at.to_string(),
            updated_at: updated_at.to_string(),
        }
    }

    #[test]
    fn canonical_title_matches_case_whitespace_and_unicode_composition() {
        assert_eq!(canonical_title("  CAFÉ  "), canonical_title("cafe\u{301}"));
    }

    #[test]
    fn filename_fallback_matches_the_persisted_display_name_shape() {
        let ctx = FileContext::new(std::path::PathBuf::from("/tmp/三体.txt"), 0, 1, Vec::new());

        assert_eq!(incoming_title(&ctx), "三体.txt");
    }

    #[test]
    fn exact_title_beats_an_earlier_prefix_candidate_regardless_of_order() {
        let prefix = file(1, "错位", Some(1), "2026-01-01 00:00:00");
        let exact = file(2, "错位愈合", Some(1), "2026-01-02 00:00:00");
        let key = canonical_title("错位愈合");
        let forward_candidates = [prefix, exact];

        let forward = find_best_duplicate(&key, Some(1), None, &forward_candidates)
            .expect("exact title should match");
        assert_eq!(forward.id, 2);

        let reverse_candidates = [
            file(2, "错位愈合", Some(1), "2026-01-02 00:00:00"),
            file(1, "错位", Some(1), "2026-01-01 00:00:00"),
        ];
        let reverse = find_best_duplicate(&key, Some(1), None, &reverse_candidates)
            .expect("candidate order must not change the match");
        assert_eq!(reverse.id, 2);
    }

    #[test]
    fn distinct_full_prefix_titles_are_not_duplicates() {
        let files = [file(1, "三体 完结", Some(1), "2026-01-01 00:00:00")];

        assert!(find_best_duplicate(&canonical_title("三体"), Some(1), None, &files).is_none());
    }

    #[test]
    fn exact_title_in_another_known_category_is_not_a_duplicate() {
        let files = [file(1, "三体", Some(2), "2026-01-01 00:00:00")];

        assert!(find_best_duplicate(&canonical_title("三体"), Some(1), None, &files).is_none());
    }

    #[test]
    fn requested_category_overrides_an_extracted_category() {
        let files = [
            file(1, "三体", Some(1), "2026-01-01 00:00:00"),
            file(2, "三体", Some(2), "2026-02-01 00:00:00"),
        ];

        let matched = find_best_duplicate(&canonical_title("三体"), Some(1), Some(2), &files)
            .expect("the user-selected category should remain authoritative");
        assert_eq!(matched.id, 1);
    }

    #[test]
    fn exact_category_beats_a_newer_uncategorized_legacy_row() {
        let files = [
            file(1, "三体", Some(1), "2026-01-01 00:00:00"),
            file(2, "三体", None, "2026-02-01 00:00:00"),
        ];

        let matched = find_best_duplicate(&canonical_title("三体"), Some(1), None, &files)
            .expect("an exact-category title should match before a legacy fallback");
        assert_eq!(matched.id, 1);
    }

    #[test]
    fn serialized_update_matches_by_title_not_progress() {
        let mut existing = file(1, "三体", Some(1), "2026-01-01 00:00:00");
        existing.progress = Some("1-80章".to_string());
        let files = [existing];

        let matched = find_best_duplicate(&canonical_title("三体"), Some(1), None, &files)
            .expect("a newer serialization with the same title should match");
        assert_eq!(matched.progress.as_deref(), Some("1-80章"));
    }

    #[test]
    fn newest_exact_title_wins_deterministically() {
        let files = [
            file(7, "三体", Some(1), "2026-01-01 00:00:00"),
            file(8, "三体", Some(1), "2026-02-01 00:00:00"),
            file(9, "三体", Some(1), "2026-02-01 00:00:00"),
        ];

        let matched = find_best_duplicate(&canonical_title("三体"), Some(1), None, &files)
            .expect("an exact title should match");
        assert_eq!(matched.id, 9);
    }
}
