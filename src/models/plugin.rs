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
    pub mod_id: i32,
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

#[derive(Debug)]
pub struct UnsavedPlugin<'a> {
    pub name: &'a str,
    pub hash: i64,
    pub file_id: i32,
    pub mod_id: i32,
    pub version: f64,
    pub size: i64,
    pub author: Option<&'a str>,
    pub description: Option<&'a str>,
    pub masters: &'a [&'a str],
    pub file_name: &'a str,
    pub file_path: &'a str,
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert<'a>(
    pool: &sqlx::Pool<sqlx::Postgres>,
    unsaved_plugin: &UnsavedPlugin<'a>,
) -> Result<Plugin> {
    // sqlx doesn't understand slices of &str with the query_as! macro: https://github.com/launchbadge/sqlx/issues/280
    sqlx::query_as(
        r#"INSERT INTO plugins
            (name, hash, file_id, mod_id, version, size, author, description, masters, file_name, file_path, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now(), now())
            ON CONFLICT (file_id, file_path) DO UPDATE
            SET (name, hash, mod_id, version, author, description, masters, file_name, updated_at) =
            (EXCLUDED.name, EXCLUDED.hash, EXCLUDED.mod_id, EXCLUDED.version, EXCLUDED.author, EXCLUDED.description, EXCLUDED.masters, EXCLUDED.file_name, now())
            RETURNING *"#,
    )
    .bind(unsaved_plugin.name)
    .bind(unsaved_plugin.hash)
    .bind(unsaved_plugin.file_id)
    .bind(unsaved_plugin.mod_id)
    .bind(unsaved_plugin.version)
    .bind(unsaved_plugin.size)
    .bind(unsaved_plugin.author)
    .bind(unsaved_plugin.description)
    .bind(unsaved_plugin.masters)
    .bind(unsaved_plugin.file_name)
    .bind(unsaved_plugin.file_path)
    .fetch_one(pool)
    .await
    .context("Failed to insert plugin")
}
