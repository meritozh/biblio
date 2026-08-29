use super::*;
#[cfg(test)]
mod filename_tests {
    use super::*;

    #[test]
    fn test_build_novel_filename_full() {
        let result = build_novel_filename("三体", Some("完结"), &["刘慈欣".to_string()], ".txt");
        assert_eq!(result, "三体 完结 刘慈欣.txt");
    }

    #[test]
    fn test_build_novel_filename_no_progress() {
        let result = build_novel_filename("三体", None, &["刘慈欣".to_string()], ".txt");
        assert_eq!(result, "三体 刘慈欣.txt");
    }

    #[test]
    fn test_build_novel_filename_no_authors() {
        let result = build_novel_filename("三体", Some("完结"), &[], ".txt");
        assert_eq!(result, "三体 完结.txt");
    }

    #[test]
    fn test_build_novel_filename_multiple_authors() {
        let result =
            build_novel_filename("三体", None, &["A".to_string(), "B".to_string()], ".txt");
        assert_eq!(result, "三体 A, B.txt");
    }

    #[test]
    fn test_build_novel_filename_empty_progress() {
        let result = build_novel_filename("三体", Some(""), &["刘慈欣".to_string()], ".txt");
        assert_eq!(result, "三体 刘慈欣.txt");
    }

    #[test]
    fn test_sanitize_filename_invalid_chars() {
        assert_eq!(
            sanitize_filename("a/b\\c:d*e?f\"g<h>i|j.txt"),
            "abcdefghij.txt"
        );
    }

    #[test]
    fn test_sanitize_filename_preserves_valid() {
        assert_eq!(
            sanitize_filename("三体 完结 刘慈欣.txt"),
            "三体 完结 刘慈欣.txt"
        );
    }

    fn fts_expr(raw: &str) -> Option<String> {
        match prepare_search_filter(raw)? {
            SearchFilter::Fts(s) => Some(s),
            SearchFilter::Like(_) => None,
        }
    }

    fn like_pattern(raw: &str) -> Option<String> {
        match prepare_search_filter(raw)? {
            SearchFilter::Like(s) => Some(s),
            SearchFilter::Fts(_) => None,
        }
    }

    #[test]
    fn search_filter_single_token_quoted_for_fts() {
        // Trigram tokenizer matches substrings inside indexed text, so the
        // quoted whole-token form is enough — no `*` prefix marker needed.
        assert_eq!(fts_expr("三体老师").as_deref(), Some("\"三体老师\""));
    }

    #[test]
    fn search_filter_multiple_tokens_anded_with_quotes() {
        assert_eq!(
            fts_expr("三体老师 刘慈欣").as_deref(),
            Some("\"三体老师\" \"刘慈欣\"")
        );
    }

    #[test]
    fn search_filter_strips_fts5_operators() {
        assert_eq!(
            fts_expr("hello(world):today").as_deref(),
            Some("\"hello\" \"world\" \"today\"")
        );
    }

    #[test]
    fn search_filter_short_query_falls_back_to_like() {
        // Below the trigram window — must use LIKE with the raw pattern.
        assert_eq!(like_pattern("体").as_deref(), Some("%体%"));
        assert_eq!(like_pattern("三体").as_deref(), Some("%三体%"));
    }

    #[test]
    fn search_filter_short_query_escapes_like_wildcards() {
        // SQL wildcards in user input must be escaped so a literal `%` or
        // `_` can't expand the match. We escape with `\` and bind `ESCAPE '\\'`.
        assert_eq!(like_pattern("a%").as_deref(), Some("%a\\%%"));
        assert_eq!(like_pattern("a_").as_deref(), Some("%a\\_%"));
        assert_eq!(like_pattern("\\a").as_deref(), Some("%\\\\a%"));
    }

    #[test]
    fn search_filter_empty_or_punctuation_returns_none() {
        assert!(prepare_search_filter("").is_none());
        assert!(prepare_search_filter("   ").is_none());
        assert!(prepare_search_filter("\"\"()").is_none());
    }
}

#[cfg(test)]
mod reverse_index_tests {
    use crate::commands::test_helpers::setup_db;

    // Note: tests for `list_files_by_tag_impl` and `list_files_by_author_impl`
    // were removed when those helpers were deleted during the tag/author
    // route revamp (the routes now query through the general `file_list`
    // path with seeded conditions). The smoke test stays so the module
    // doesn't become empty.

    #[tokio::test]
    async fn setup_db_smoke_test() {
        let pool = setup_db().await;
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}

#[cfg(test)]
mod filter_sql_tests {
    use super::*;

    #[test]
    fn favorite_filter_true_uses_boolean_column() {
        let conditions = vec![FilterCondition {
            field: "favorite".to_string(),
            op: "is".to_string(),
            value: Some(Value::Bool(true)),
            ..Default::default()
        }];

        let (sql, binds) = build_filter_sql(&conditions, "f");

        assert_eq!(sql, " AND f.is_favorite = 1");
        assert!(binds.is_empty());
    }

    #[test]
    fn favorite_filter_false_uses_boolean_column() {
        let conditions = vec![FilterCondition {
            field: "favorite".to_string(),
            op: "is".to_string(),
            value: Some(Value::Bool(false)),
            ..Default::default()
        }];

        let (sql, binds) = build_filter_sql(&conditions, "");

        assert_eq!(sql, " AND is_favorite = 0");
        assert!(binds.is_empty());
    }

    #[test]
    fn custom_filter_binds_key_and_value() {
        let conditions = vec![FilterCondition {
            field: "custom".to_string(),
            op: "is".to_string(),
            key: Some("publisher".to_string()),
            text: Some("ACME".to_string()),
            ..Default::default()
        }];

        let (sql, binds) = build_filter_sql(&conditions, "");

        assert_eq!(
            sql,
            " AND EXISTS (SELECT 1 FROM metadata m WHERE m.file_id = id AND m.key = ? AND m.value = ?)"
        );
        assert_eq!(binds, vec!["publisher".to_string(), "ACME".to_string()]);
    }

    #[test]
    fn custom_filter_contains_escapes_like_meta() {
        let conditions = vec![FilterCondition {
            field: "custom".to_string(),
            op: "contains".to_string(),
            key: Some("publisher".to_string()),
            text: Some("50%".to_string()),
            ..Default::default()
        }];

        let (sql, binds) = build_filter_sql(&conditions, "f");

        assert_eq!(
            sql,
            " AND EXISTS (SELECT 1 FROM metadata m WHERE m.file_id = f.id AND m.key = ? AND LOWER(m.value) LIKE ? ESCAPE '\\')"
        );
        assert_eq!(binds, vec!["publisher".to_string(), "%50\\%%".to_string()]);
    }

    #[test]
    fn custom_filter_without_key_is_noop() {
        let conditions = vec![FilterCondition {
            field: "custom".to_string(),
            op: "is".to_string(),
            text: Some("x".to_string()),
            ..Default::default()
        }];

        let (sql, binds) = build_filter_sql(&conditions, "");

        assert_eq!(sql, "");
        assert!(binds.is_empty());
    }
}

#[cfg(test)]
mod custom_sort_tests {
    use super::super::query_filter::custom_sort_clause;

    async fn pool_with_sortable_field() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE schema_fields (\
                id INTEGER PRIMARY KEY,\
                schema_id TEXT NOT NULL,\
                field_key TEXT NOT NULL,\
                field_type TEXT NOT NULL,\
                sortable BOOLEAN NOT NULL DEFAULT 0\
            );\
            INSERT INTO schema_fields (schema_id, field_key, field_type, sortable) VALUES \
                ('novel', 'rating', 'rating', 1),\
                ('novel', 'publisher', 'text', 1),\
                ('novel', 'internal_note', 'text', 0);",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn numeric_field_sorts_as_real() {
        let pool = pool_with_sortable_field().await;
        let (join, order) = custom_sort_clause(&pool, "field:rating", true, "")
            .await
            .unwrap();
        assert_eq!(
            join,
            " LEFT JOIN metadata ms ON ms.file_id = id AND ms.key = 'rating'"
        );
        assert_eq!(order, "ORDER BY ms.value IS NULL, CAST(ms.value AS REAL) DESC");
    }

    #[tokio::test]
    async fn text_field_sorts_case_insensitive_asc() {
        let pool = pool_with_sortable_field().await;
        let (_, order) = custom_sort_clause(&pool, "field:publisher", false, "f")
            .await
            .unwrap();
        assert_eq!(order, "ORDER BY ms.value IS NULL, LOWER(ms.value) ASC");
    }

    #[tokio::test]
    async fn non_sortable_or_malformed_keys_fall_back() {
        let pool = pool_with_sortable_field().await;
        // Not marked sortable → no custom clause.
        assert!(custom_sort_clause(&pool, "field:internal_note", true, "").await.is_none());
        // Identifier-shape violation (would be an injection sink) → none.
        assert!(custom_sort_clause(&pool, "field:x'; DROP TABLE files;--", true, "").await.is_none());
        // Unknown key → none.
        assert!(custom_sort_clause(&pool, "field:missing", true, "").await.is_none());
        // Not a field sort at all → none.
        assert!(custom_sort_clause(&pool, "name", true, "").await.is_none());
    }
}
