//! Shared helpers for `#[cfg(test)]` code across the `commands` module tree.
//!
//! Gated with `#[cfg(test)]` at the module declaration in `mod.rs` so these
//! never compile into release builds.

use sqlx::SqlitePool;

/// Spin up an in-memory SQLite database with the production schema applied.
/// Reusable across per-module test suites that need realistic tables.
pub async fn setup_db() -> SqlitePool {
    let pool = SqlitePool::connect(":memory:")
        .await
        .expect("connect in-memory sqlite");
    sqlx::raw_sql(include_str!("../database/schema.sql"))
        .execute(&pool)
        .await
        .expect("apply schema");
    pool
}
