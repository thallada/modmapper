use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Serialize, Deserialize)]
pub struct Cell {
    pub id: i32,
    pub form_id: i32,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub is_persistent: bool,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert_cell(
    pool: &sqlx::Pool<sqlx::Postgres>,
    form_id: i32,
    x: Option<i32>,
    y: Option<i32>,
    is_persistent: bool,
) -> Result<Cell> {
    sqlx::query_as!(
        Cell,
        "INSERT INTO cells
            (form_id, x, y, is_persistent, created_at, updated_at)
            VALUES ($1, $2, $3, $4, now(), now())
            ON CONFLICT (form_id) DO UPDATE
            SET (x, y, is_persistent, updated_at) =
            (EXCLUDED.x, EXCLUDED.y, EXCLUDED.is_persistent, now())
            RETURNING *",
        form_id,
        x,
        y,
        is_persistent
    )
    .fetch_one(pool)
    .await
    .context("Failed to insert cell")
}
