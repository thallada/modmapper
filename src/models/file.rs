use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Serialize, Deserialize)]
pub struct File {
    pub id: i32,
    pub name: String,
    pub file_name: String,
    pub nexus_file_id: i32,
    pub mod_id: i32,
    pub category: Option<String>,
    pub version: Option<String>,
    pub mod_version: Option<String>,
    pub size: i64,
    pub uploaded_at: NaiveDateTime,
    pub has_download_link: bool,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub downloaded_at: Option<NaiveDateTime>,
}

#[instrument(level = "debug", skip(pool))]
pub async fn get_by_nexus_file_id(
    pool: &sqlx::Pool<sqlx::Postgres>,
    nexus_file_id: i32,
) -> Result<Option<File>> {
    sqlx::query_as!(
        File,
        "SELECT * FROM files WHERE nexus_file_id = $1",
        nexus_file_id,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to get file")
}

#[instrument(level = "debug", skip(pool))]
pub async fn get_nexus_file_ids_by_mod_id(
    pool: &sqlx::Pool<sqlx::Postgres>,
    mod_id: i32,
) -> Result<Vec<i32>> {
    sqlx::query!("SELECT nexus_file_id FROM files WHERE mod_id = $1", mod_id)
        .map(|row| row.nexus_file_id)
        .fetch_all(pool)
        .await
        .context("Failed to get files")
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert(
    pool: &sqlx::Pool<sqlx::Postgres>,
    name: &str,
    file_name: &str,
    nexus_file_id: i32,
    mod_id: i32,
    category: Option<&str>,
    version: Option<&str>,
    mod_version: Option<&str>,
    size: i64,
    uploaded_at: NaiveDateTime,
) -> Result<File> {
    sqlx::query_as!(
        File,
        "INSERT INTO files
            (name, file_name, nexus_file_id, mod_id, category, version, mod_version, size, uploaded_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now(), now())
            ON CONFLICT (mod_id, nexus_file_id) DO UPDATE
            SET (name, file_name, category, version, mod_version, uploaded_at, updated_at) =
            (EXCLUDED.name, EXCLUDED.file_name, EXCLUDED.category, EXCLUDED.version, EXCLUDED.mod_version, EXCLUDED.uploaded_at, now())
            RETURNING *",
        name,
        file_name,
        nexus_file_id,
        mod_id,
        category,
        version,
        mod_version,
        size,
        uploaded_at
    )
    .fetch_one(pool)
    .await
    .context("Failed to insert file")
}

#[instrument(level = "debug", skip(pool))]
pub async fn update_has_download_link(
    pool: &sqlx::Pool<sqlx::Postgres>,
    id: i32,
    has_download_link: bool,
) -> Result<File> {
    sqlx::query_as!(
        File,
        "UPDATE files
            SET has_download_link = $2
            WHERE id = $1
            RETURNING *",
        id,
        has_download_link,
    )
    .fetch_one(pool)
    .await
    .context("Failed to update file")
}

#[instrument(level = "debug", skip(pool))]
pub async fn update_downloaded_at(pool: &sqlx::Pool<sqlx::Postgres>, id: i32) -> Result<File> {
    sqlx::query_as!(
        File,
        "UPDATE files
            SET downloaded_at = now()
            WHERE id = $1
            RETURNING *",
        id,
    )
    .fetch_one(pool)
    .await
    .context("Failed to update file")
}
