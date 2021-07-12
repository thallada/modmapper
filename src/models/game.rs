use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Serialize, Deserialize)]
pub struct Game {
    pub id: i32,
    pub name: String,
    pub nexus_game_id: i32,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert(
    pool: &sqlx::Pool<sqlx::Postgres>,
    name: &str,
    nexus_game_id: i32,
) -> Result<Game> {
    sqlx::query_as!(
        Game,
        "INSERT INTO games
            (name, nexus_game_id, created_at, updated_at)
            VALUES ($1, $2, now(), now())
            ON CONFLICT (nexus_game_id, name) DO UPDATE SET (name, updated_at) = (EXCLUDED.name, now())
            RETURNING *",
        name,
        nexus_game_id
    )
    .fetch_one(pool)
    .await
    .context("Failed to insert game")
}
