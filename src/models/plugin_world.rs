use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginWorld {
    pub id: i32,
    pub plugin_id: i32,
    pub world_id: i32,
    pub editor_id: String,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert(
    pool: &sqlx::Pool<sqlx::Postgres>,
    plugin_id: i32,
    world_id: i32,
    editor_id: &str,
) -> Result<PluginWorld> {
    sqlx::query_as!(
        PluginWorld,
        "INSERT INTO plugin_worlds
            (plugin_id, world_id, editor_id, created_at, updated_at)
            VALUES ($1, $2, $3, now(), now())
            ON CONFLICT (plugin_id, world_id) DO UPDATE
            SET (editor_id, updated_at) = (EXCLUDED.editor_id, now())
            RETURNING *",
        plugin_id,
        world_id,
        editor_id,
    )
    .fetch_one(pool)
    .await
    .context("Failed to insert plugin_world")
}
