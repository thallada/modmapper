use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::Json;
use tracing::instrument;

use super::hash_to_string;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Plugin {
    pub id: i32,
    pub name: String,
    #[serde(serialize_with = "hash_to_string")]
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

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct PluginsByHashWithMods {
    #[serde(serialize_with = "hash_to_string")]
    pub hash: i64,
    pub plugins: Option<Json<Vec<Plugin>>>,
    pub files: Option<serde_json::Value>,
    pub mods: Option<serde_json::Value>,
    pub cells: Option<serde_json::Value>,
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

#[instrument(level = "debug", skip(pool))]
pub async fn batched_get_by_hash_with_mods(
    pool: &sqlx::Pool<sqlx::Postgres>,
    page_size: i64,
    last_hash: Option<i64>,
    master: &str,
    world_id: i32,
    updated_after: Option<NaiveDateTime>,
) -> Result<Vec<PluginsByHashWithMods>> {
    let last_hash = last_hash.unwrap_or(-9223372036854775808); // psql bigint min
    if let Some(updated_after) = updated_after {
        let hashes = sqlx::query!(
            r#"SELECT
                plugins.hash
            FROM plugins
            WHERE plugins.hash > $2 AND plugins.updated_at > $3
            GROUP BY plugins.hash
            ORDER BY plugins.hash ASC
            LIMIT $1"#,
            page_size,
            last_hash,
            updated_after
        )
        .fetch_all(pool)
        .await
        .context("Failed to batch get plugin hashes")?;
        sqlx::query_as!(
            PluginsByHashWithMods,
            r#"SELECT
                plugins.hash,
                json_agg(DISTINCT plugins.*) as "plugins: Json<Vec<Plugin>>",
                json_agg(DISTINCT files.*) as files,
                json_agg(DISTINCT mods.*) as mods,
                COALESCE(json_agg(DISTINCT jsonb_build_object('x', cells.x, 'y', cells.y)) FILTER (WHERE cells.x IS NOT NULL AND cells.y IS NOT NULL AND cells.master = $2 AND cells.world_id = $3), '[]') AS cells
            FROM plugins
            LEFT OUTER JOIN files ON files.id = plugins.file_id
            LEFT OUTER JOIN mods ON mods.id = files.mod_id
            LEFT OUTER JOIN plugin_cells ON plugin_cells.plugin_id = plugins.id
            LEFT OUTER JOIN cells ON cells.id = plugin_cells.cell_id
            WHERE plugins.hash = ANY($1::bigint[])
            GROUP BY plugins.hash"#,
            &hashes.into_iter().map(|h| h.hash).collect::<Vec<i64>>(),
            master,
            world_id
        )
        .fetch_all(pool)
        .await
        .context("Failed to batch get by hash with mods")
    } else {
        sqlx::query_as!(
            PluginsByHashWithMods,
            r#"SELECT
                plugins.hash,
                json_agg(DISTINCT plugins.*) as "plugins: Json<Vec<Plugin>>",
                json_agg(DISTINCT files.*) as files,
                json_agg(DISTINCT mods.*) as mods,
                COALESCE(json_agg(DISTINCT jsonb_build_object('x', cells.x, 'y', cells.y)) FILTER (WHERE cells.x IS NOT NULL AND cells.y IS NOT NULL AND cells.master = $3 AND cells.world_id = $4), '[]') AS cells
            FROM plugins
            LEFT OUTER JOIN files ON files.id = plugins.file_id
            LEFT OUTER JOIN mods ON mods.id = files.mod_id
            LEFT OUTER JOIN plugin_cells ON plugin_cells.plugin_id = plugins.id
            LEFT OUTER JOIN cells ON cells.id = plugin_cells.cell_id
            WHERE plugins.hash > $2
            GROUP BY plugins.hash
            ORDER BY plugins.hash ASC
            LIMIT $1"#,
            page_size,
            last_hash,
            master,
            world_id
        )
        .fetch_all(pool)
        .await
        .context("Failed to batch get by hash with mods")
    }
}
