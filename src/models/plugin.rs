use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Serialize, Deserialize)]
pub struct Plugin {
    pub id: i32,
    pub name: String,
    pub hash: i64,
    pub file_id: i32,
    pub version: Option<f64>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub masters: Option<Vec<String>>,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert_plugin(
    pool: &sqlx::Pool<sqlx::Postgres>,
    name: &str,
    hash: i64,
    file_id: i32,
    version: Option<f64>,
    author: Option<&str>,
    description: Option<&str>,
    masters: Option<&[String]>,
) -> Result<Plugin> {
    sqlx::query_as!(
        Plugin,
        "INSERT INTO plugins
            (name, hash, file_id, version, author, description, masters, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, now(), now())
            ON CONFLICT (file_id, name) DO UPDATE
            SET (hash, version, author, description, masters, updated_at) =
            (EXCLUDED.hash, EXCLUDED.version, EXCLUDED.author, EXCLUDED.description, EXCLUDED.masters, now())
            RETURNING *",
        name,
        hash,
        file_id,
        version,
        author,
        description,
        masters
    )
    .fetch_one(pool)
    .await
    .context("Failed to insert plugin")
}
