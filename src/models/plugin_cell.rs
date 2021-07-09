use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginCell {
    pub id: i32,
    pub plugin_id: i32,
    pub cell_id: i32,
    pub editor_id: Option<String>,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

pub async fn insert_plugin_cell(
    pool: &sqlx::Pool<sqlx::Postgres>,
    plugin_id: i32,
    cell_id: i32,
    editor_id: Option<String>,
) -> Result<PluginCell> {
    sqlx::query_as!(
        PluginCell,
        "INSERT INTO plugin_cells
            (plugin_id, cell_id, editor_id, created_at, updated_at)
            VALUES ($1, $2, $3, now(), now())
            ON CONFLICT (plugin_id, cell_id) DO UPDATE
            SET (editor_id, updated_at) = (EXCLUDED.editor_id, now())
            RETURNING *",
        plugin_id,
        cell_id,
        editor_id,
    )
    .fetch_one(pool)
    .await
    .context("Failed to insert cell")
}