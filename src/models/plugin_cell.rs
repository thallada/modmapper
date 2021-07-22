use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::instrument;

use super::BATCH_SIZE;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct PluginCell {
    pub id: i32,
    pub plugin_id: i32,
    pub cell_id: i32,
    pub editor_id: Option<String>,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Debug)]
pub struct UnsavedPluginCell<'a> {
    pub plugin_id: i32,
    pub cell_id: i32,
    pub editor_id: Option<&'a str>,
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert(
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
    .context("Failed to insert plugin_cell")
}

#[instrument(level = "debug", skip(pool))]
pub async fn batched_insert<'a>(
    pool: &sqlx::Pool<sqlx::Postgres>,
    plugin_cells: &[UnsavedPluginCell<'a>],
) -> Result<Vec<PluginCell>> {
    let mut saved_plugin_cells = vec![];
    for batch in plugin_cells.chunks(BATCH_SIZE) {
        let mut plugin_ids: Vec<i32> = vec![];
        let mut cell_ids: Vec<i32> = vec![];
        let mut editor_ids: Vec<Option<&str>> = vec![];
        batch.into_iter().for_each(|unsaved_plugin_cell| {
            plugin_ids.push(unsaved_plugin_cell.plugin_id);
            cell_ids.push(unsaved_plugin_cell.cell_id);
            editor_ids.push(unsaved_plugin_cell.editor_id);
        });
        saved_plugin_cells.append(
            // sqlx doesn't understand arrays of Options with the query_as! macro
            &mut sqlx::query_as(
                r#"INSERT INTO plugin_cells (plugin_id, cell_id, editor_id, created_at, updated_at)
                SELECT *, now(), now() FROM UNNEST($1::int[], $2::int[], $3::text[])
                ON CONFLICT (plugin_id, cell_id) DO UPDATE
                SET (editor_id, updated_at) = (EXCLUDED.editor_id, now())
                RETURNING *"#,
            )
            .bind(&plugin_ids)
            .bind(&cell_ids)
            .bind(&editor_ids)
            .fetch_all(pool)
            .await
            .context("Failed to insert plugin_cells")?,
        );
    }
    Ok(saved_plugin_cells)
}
