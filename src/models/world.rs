use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::instrument;

use super::BATCH_SIZE;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct World {
    pub id: i32,
    pub form_id: i32,
    pub master: String,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Debug)]
pub struct UnsavedWorld<'a> {
    pub form_id: i32,
    pub master: &'a str,
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

#[instrument(level = "debug", skip(pool))]
pub async fn batched_insert<'a>(
    pool: &sqlx::Pool<sqlx::Postgres>,
    worlds: &[UnsavedWorld<'a>],
) -> Result<Vec<World>> {
    let mut saved_worlds = vec![];
    for batch in worlds.chunks(BATCH_SIZE) {
        let mut form_ids: Vec<i32> = vec![];
        let mut masters: Vec<&str> = vec![];
        batch.iter().for_each(|unsaved_world| {
            form_ids.push(unsaved_world.form_id);
            masters.push(unsaved_world.master);
        });
        saved_worlds.append(
            // cannot use macro with types that have lifetimes: https://github.com/launchbadge/sqlx/issues/280
            &mut sqlx::query_as(
                r#"INSERT INTO worlds (form_id, master, created_at, updated_at)
                SELECT *, now(), now() FROM UNNEST($1::int[], $2::text[])
                ON CONFLICT (form_id, master) DO UPDATE
                SET updated_at = now()
                RETURNING *"#,
            )
            .bind(&form_ids)
            .bind(&masters)
            .fetch_all(pool)
            .await
            .context("Failed to insert worlds")?,
        );
    }
    Ok(saved_worlds)
}
