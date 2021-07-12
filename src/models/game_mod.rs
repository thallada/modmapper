use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Serialize, Deserialize)]
pub struct Mod {
    pub id: i32,
    pub name: String,
    pub nexus_mod_id: i32,
    pub author: String,
    pub category: String,
    pub description: Option<String>,
    pub game_id: i32,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[instrument(level = "debug", skip(pool))]
pub async fn get_by_nexus_mod_id(
    pool: &sqlx::Pool<sqlx::Postgres>,
    nexus_mod_id: i32,
) -> Result<Option<Mod>> {
    sqlx::query_as!(
        Mod,
        "SELECT * FROM mods WHERE nexus_mod_id = $1",
        nexus_mod_id,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to get mod")
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert(
    pool: &sqlx::Pool<sqlx::Postgres>,
    name: &str,
    nexus_mod_id: i32,
    author: &str,
    category: &str,
    description: Option<&str>,
    game_id: i32,
) -> Result<Mod> {
    sqlx::query_as!(
        Mod,
        "INSERT INTO mods
            (name, nexus_mod_id, author, category, description, game_id, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, now(), now())
            ON CONFLICT (game_id, nexus_mod_id) DO UPDATE
            SET (name, author, category, description, updated_at) =
            (EXCLUDED.name, EXCLUDED.author, EXCLUDED.category, EXCLUDED.description, now())
            RETURNING *",
        name,
        nexus_mod_id,
        author,
        category,
        description,
        game_id
    )
    .fetch_one(pool)
    .await
    .context("Failed to insert or update mod")
}
