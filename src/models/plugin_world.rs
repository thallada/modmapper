use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::BATCH_SIZE;

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginWorld {
    pub id: i32,
    pub plugin_id: i32,
    pub world_id: i32,
    pub editor_id: String,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnsavedPluginWorld {
    pub plugin_id: i32,
    pub world_id: i32,
    pub editor_id: String,
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

#[instrument(level = "debug", skip(pool))]
pub async fn batched_insert(
    pool: &sqlx::Pool<sqlx::Postgres>,
    plugin_worlds: &[UnsavedPluginWorld],
) -> Result<Vec<PluginWorld>> {
    let mut saved_plugin_worlds = vec![];
    for batch in plugin_worlds.chunks(BATCH_SIZE) {
        let mut plugin_ids: Vec<i32> = vec![];
        let mut world_ids: Vec<i32> = vec![];
        let mut editor_ids: Vec<String> = vec![];
        batch.into_iter().for_each(|unsaved_plugin_world| {
            plugin_ids.push(unsaved_plugin_world.plugin_id);
            world_ids.push(unsaved_plugin_world.world_id);
            editor_ids.push(unsaved_plugin_world.editor_id.clone());
        });
        saved_plugin_worlds.append(
            &mut sqlx::query_as!(
                PluginWorld,
                r#"INSERT INTO plugin_worlds (plugin_id, world_id, editor_id, created_at, updated_at)
                SELECT *, now(), now() FROM UNNEST($1::int[], $2::int[], $3::text[])
                ON CONFLICT (plugin_id, world_id) DO UPDATE
                SET (editor_id, updated_at) = (EXCLUDED.editor_id, now())
                RETURNING *"#,
                &plugin_ids,
                &world_ids,
                &editor_ids,
            )
            .fetch_all(pool)
            .await
            .context("Failed to insert plugin_worlds")?,
        );
    }
    Ok(saved_plugin_worlds)
}
