//! Schema slug constants.
//!
//! Schemas (resource types) are data — rows in the `schemas` table
//! seeded by migration v6 — not a code enum. This module keeps only
//! the built-in identifiers that seed data and pipeline-template
//! routing refer to. Code behavior keys off field *semantics*
//! (`schema_fields.semantic`) and the schema's `pipeline_template`,
//! never off the slug itself.
//!
//! The frontend mirror lives in `src/lib/categorySchema.ts`, which
//! resolves schema definitions from the DB via the `schema_list`
//! command (with a static fallback copy for pre-load renders).

/// Built-in schema slugs seeded by migration v6.
pub const NOVEL: &str = "novel";
pub const COMIC: &str = "comic";
pub const GALGAME: &str = "galgame";

/// Slug applied when a stored value doesn't match any row in `schemas`
/// (e.g. a category written by a newer binary that introduced a slug
/// this build doesn't know). Mirrors the historical `SchemaSlug::
/// from_str` fallback so stale rows keep novel defaults instead of
/// crashing the pipeline.
pub const FALLBACK: &str = NOVEL;
