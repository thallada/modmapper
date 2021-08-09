use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::instrument;

use super::BATCH_SIZE;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Mod {
    pub id: i32,
    pub name: String,
    pub nexus_mod_id: i32,
    pub author: String,
    pub category: Option<String>,
    pub description: Option<String>,
    pub game_id: i32,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub last_updated_files_at: Option<NaiveDateTime>,
}

#[derive(Debug)]
pub struct UnsavedMod<'a> {
    pub name: &'a str,
    pub nexus_mod_id: i32,
    pub author: &'a str,
    pub category: Option<&'a str>,
    pub description: Option<&'a str>,
    pub game_id: i32,
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
pub async fn bulk_get_fully_processed_nexus_mod_ids(
    pool: &sqlx::Pool<sqlx::Postgres>,
    nexus_mod_ids: &[i32],
) -> Result<Vec<i32>> {
    sqlx::query!(
        "SELECT nexus_mod_id FROM mods
            WHERE nexus_mod_id = ANY($1::int[])
            AND last_updated_files_at IS NOT NULL",
        nexus_mod_ids,
    )
    .map(|row| row.nexus_mod_id)
    .fetch_all(pool)
    .await
    .context("Failed to get fully processed , last_updated_files_at: () mods")
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert(
    pool: &sqlx::Pool<sqlx::Postgres>,
    name: &str,
    nexus_mod_id: i32,
    author: &str,
    category: Option<&str>,
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

#[instrument(level = "debug", skip(pool))]
pub async fn batched_insert<'a>(
    pool: &sqlx::Pool<sqlx::Postgres>,
    mods: &[UnsavedMod<'a>],
) -> Result<Vec<Mod>> {
    let mut saved_mods = vec![];
    for batch in mods.chunks(BATCH_SIZE) {
        let mut names: Vec<&str> = vec![];
        let mut nexus_mod_ids: Vec<i32> = vec![];
        let mut authors: Vec<&str> = vec![];
        let mut categories: Vec<Option<&str>> = vec![];
        let mut descriptions: Vec<Option<&str>> = vec![];
        let mut game_ids: Vec<i32> = vec![];
        batch.into_iter().for_each(|unsaved_mod| {
            names.push(unsaved_mod.name);
            nexus_mod_ids.push(unsaved_mod.nexus_mod_id);
            authors.push(unsaved_mod.author);
            categories.push(unsaved_mod.category);
            descriptions.push(unsaved_mod.description);
            game_ids.push(unsaved_mod.game_id);
        });
        saved_mods.append(
            // sqlx doesn't understand arrays of Options with the query_as! macro
            &mut sqlx::query_as(
                r#"INSERT INTO mods
                (name, nexus_mod_id, author, category, description, game_id, created_at, updated_at)
                SELECT *, now(), now()
                FROM UNNEST($1::text[], $2::int[], $3::text[], $4::text[], $5::text[], $6::int[])
                ON CONFLICT (game_id, nexus_mod_id) DO UPDATE
                SET (name, author, category, description, updated_at) =
                (EXCLUDED.name, EXCLUDED.author, EXCLUDED.category, EXCLUDED.description, now())
                RETURNING *"#,
            )
            .bind(&names)
            .bind(&nexus_mod_ids)
            .bind(&authors)
            .bind(&categories)
            .bind(&descriptions)
            .bind(&game_ids)
            .fetch_all(pool)
            .await
            .context("Failed to insert mods")?,
        );
    }
    Ok(saved_mods)
}

#[instrument(level = "debug", skip(pool))]
pub async fn get(pool: &sqlx::Pool<sqlx::Postgres>, id: i32) -> Result<Option<Mod>> {
    sqlx::query_as!(Mod, "SELECT * FROM mods WHERE id = $1", id)
        .fetch_optional(pool)
        .await
        .context("Failed to get mod")
}

#[instrument(level = "debug", skip(pool))]
pub async fn update_last_updated_files_at(
    pool: &sqlx::Pool<sqlx::Postgres>,
    id: i32,
) -> Result<Mod> {
    sqlx::query_as!(
        Mod,
        "UPDATE mods
            SET last_updated_files_at = now()
            WHERE id = $1
            RETURNING *",
        id,
    )
    .fetch_one(pool)
    .await
    .context("Failed to update mod")
}
