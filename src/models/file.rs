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
    pub has_plugin: bool,
    pub unable_to_extract_plugins: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileWithCells {
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
    pub has_plugin: bool,
    pub unable_to_extract_plugins: bool,
    pub cells: Option<serde_json::Value>,
    pub plugins: Option<serde_json::Value>,
    pub plugin_count: Option<i64>,
}

#[derive(Debug)]
pub struct UnsavedFile<'a> {
    pub name: &'a str,
    pub file_name: &'a str,
    pub nexus_file_id: i32,
    pub mod_id: i32,
    pub category: Option<&'a str>,
    pub version: Option<&'a str>,
    pub mod_version: Option<&'a str>,
    pub size: i64,
    pub uploaded_at: NaiveDateTime,
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
pub async fn get_processed_nexus_file_ids_by_mod_id(
    pool: &sqlx::Pool<sqlx::Postgres>,
    mod_id: i32,
) -> Result<Vec<i32>> {
    sqlx::query!(
        "SELECT nexus_file_id FROM files
            WHERE mod_id = $1 AND (
                downloaded_at IS NOT NULL OR
                has_plugin = false OR
                has_download_link = false
            )",
        mod_id
    )
    .map(|row| row.nexus_file_id)
    .fetch_all(pool)
    .await
    .context("Failed to get files")
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert<'a>(
    pool: &sqlx::Pool<sqlx::Postgres>,
    unsaved_file: &UnsavedFile<'a>,
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
        unsaved_file.name,
        unsaved_file.file_name,
        unsaved_file.nexus_file_id,
        unsaved_file.mod_id,
        unsaved_file.category,
        unsaved_file.version,
        unsaved_file.mod_version,
        unsaved_file.size,
        unsaved_file.uploaded_at
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

#[instrument(level = "debug", skip(pool))]
pub async fn update_has_plugin(
    pool: &sqlx::Pool<sqlx::Postgres>,
    id: i32,
    has_plugin: bool,
) -> Result<File> {
    sqlx::query_as!(
        File,
        "UPDATE files
            SET has_plugin = $2
            WHERE id = $1
            RETURNING *",
        id,
        has_plugin,
    )
    .fetch_one(pool)
    .await
    .context("Failed to update file")
}

#[instrument(level = "debug", skip(pool))]
pub async fn update_unable_to_extract_plugins(
    pool: &sqlx::Pool<sqlx::Postgres>,
    id: i32,
    unable_to_extract_plugins: bool,
) -> Result<File> {
    sqlx::query_as!(
        File,
        "UPDATE files
            SET unable_to_extract_plugins = $2
            WHERE id = $1
            RETURNING *",
        id,
        unable_to_extract_plugins,
    )
    .fetch_one(pool)
    .await
    .context("Failed to update file")
}

#[instrument(level = "debug", skip(pool))]
pub async fn batched_get_with_cells(
    pool: &sqlx::Pool<sqlx::Postgres>,
    page_size: i64,
    last_id: Option<i32>,
    master: &str,
    world_id: i32,
    updated_after: Option<NaiveDateTime>,
) -> Result<Vec<FileWithCells>> {
    let last_id = last_id.unwrap_or(0);
    if let Some(updated_after) = updated_after {
        sqlx::query_as!(
            FileWithCells,
            "SELECT
                files.*,
                COALESCE(json_agg(DISTINCT jsonb_build_object('x', cells.x, 'y', cells.y)) FILTER (WHERE cells.x IS NOT NULL AND cells.y IS NOT NULL AND cells.master = $3 AND cells.world_id = $4), '[]') AS cells,
                COALESCE(json_agg(DISTINCT jsonb_build_object('hash', plugins.hash, 'file_path', plugins.file_path)) FILTER (WHERE plugins.hash IS NOT NULL), '[]') AS plugins,
                COUNT(plugins.*) AS plugin_count
            FROM files
            LEFT OUTER JOIN plugin_cells ON plugin_cells.file_id = files.id
            LEFT OUTER JOIN cells ON cells.id = plugin_cells.cell_id
            LEFT OUTER JOIN plugins ON plugins.file_id = files.id
            WHERE files.id > $2 AND files.updated_at > $5
            GROUP BY files.id
            ORDER BY files.id ASC
            LIMIT $1",
            page_size,
            last_id,
            master,
            world_id,
            updated_after
        )
        .fetch_all(pool)
        .await
        .context("Failed to batch get with cells")
    } else {
        sqlx::query_as!(
            FileWithCells,
            "SELECT
                files.*,
                COALESCE(json_agg(DISTINCT jsonb_build_object('x', cells.x, 'y', cells.y)) FILTER (WHERE cells.x IS NOT NULL AND cells.y IS NOT NULL AND cells.master = $3 AND cells.world_id = $4), '[]') AS cells,
                COALESCE(json_agg(DISTINCT jsonb_build_object('hash', plugins.hash, 'file_path', plugins.file_path)) FILTER (WHERE plugins.hash IS NOT NULL), '[]') AS plugins,
                COUNT(plugins.*) AS plugin_count
            FROM files
            LEFT OUTER JOIN plugin_cells ON plugin_cells.file_id = files.id
            LEFT OUTER JOIN cells ON cells.id = plugin_cells.cell_id
            LEFT OUTER JOIN plugins ON plugins.file_id = files.id
            WHERE files.id > $2
            GROUP BY files.id
            ORDER BY files.id ASC
            LIMIT $1",
            page_size,
            last_id,
            master,
            world_id
        )
        .fetch_all(pool)
        .await
        .context("Failed to batch get with cells")
    }
}