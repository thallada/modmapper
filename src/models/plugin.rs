use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::instrument;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Plugin {
    pub id: i32,
    pub name: String,
    pub hash: i64,
    pub file_id: i32,
    pub version: f64,
    pub size: i64,
    pub author: Option<String>,
    pub description: Option<String>,
    pub masters: Vec<String>,
    pub file_name: String,
    pub file_path: String,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert(
    pool: &sqlx::Pool<sqlx::Postgres>,
    name: &str,
    hash: i64,
    file_id: i32,
    version: f64,
    size: i64,
    author: Option<&str>,
    description: Option<&str>,
    masters: &[&str],
    file_name: &str,
    file_path: &str,
) -> Result<Plugin> {
    // sqlx doesn't understand slices of &str with the query_as! macro: https://github.com/launchbadge/sqlx/issues/280
    sqlx::query_as(
        r#"INSERT INTO plugins
            (name, hash, file_id, version, size, author, description, masters, file_name, file_path, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now(), now())
            ON CONFLICT (file_id, file_path) DO UPDATE
            SET (name, hash, version, author, description, masters, file_name, updated_at) =
            (EXCLUDED.name, EXCLUDED.hash, EXCLUDED.version, EXCLUDED.author, EXCLUDED.description, EXCLUDED.masters, EXCLUDED.file_name, now())
            RETURNING *"#,
    )
    .bind(name)
    .bind(hash)
    .bind(file_id)
    .bind(version)
    .bind(size)
    .bind(author)
    .bind(description)
    .bind(masters)
    .bind(file_name)
    .fetch_one(pool)
    .await
    .context("Failed to insert plugin")
}
