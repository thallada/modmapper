use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Serialize, Deserialize)]
pub struct World {
    pub id: i32,
    pub form_id: i32,
    pub master: String,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert(
    pool: &sqlx::Pool<sqlx::Postgres>,
    form_id: i32,
    master: &str,
) -> Result<World> {
    sqlx::query_as!(
        World,
        "INSERT INTO worlds
            (form_id, master, created_at, updated_at)
            VALUES ($1, $2, now(), now())
            ON CONFLICT (form_id, master) DO UPDATE
            SET updated_at = now()
            RETURNING *",
        form_id,
        master
    )
    .fetch_one(pool)
    .await
    .context("Failed to insert world")
}
