use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::BATCH_SIZE;

#[derive(Debug, Serialize, Deserialize)]
pub struct World {
    pub id: i32,
    pub form_id: i32,
    pub master: String,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnsavedWorld {
    pub form_id: i32,
    pub master: String,
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
pub async fn batched_insert(
    pool: &sqlx::Pool<sqlx::Postgres>,
    worlds: &[UnsavedWorld],
) -> Result<Vec<World>> {
    let mut saved_worlds = vec![];
    for batch in worlds.chunks(BATCH_SIZE) {
        let mut form_ids: Vec<i32> = vec![];
        let mut masters: Vec<String> = vec![];
        batch.into_iter().for_each(|unsaved_world| {
            form_ids.push(unsaved_world.form_id);
            masters.push(unsaved_world.master.clone());
        });
        saved_worlds.append(
            &mut sqlx::query_as!(
                World,
                r#"INSERT INTO worlds (form_id, master, created_at, updated_at)
                SELECT *, now(), now() FROM UNNEST($1::int[], $2::text[])
                ON CONFLICT (form_id, master) DO UPDATE
                SET updated_at = now()
                RETURNING *"#,
                &form_ids,
                &masters
            )
            .fetch_all(pool)
            .await
            .context("Failed to insert worlds")?,
        );
    }
    Ok(saved_worlds)
}
