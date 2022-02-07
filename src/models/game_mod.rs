use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::instrument;

use crate::nexus_api::game_mod::ExtractedModData;

use super::BATCH_SIZE;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Mod {
    pub id: i32,
    pub name: String,
    pub nexus_mod_id: i32,
    pub author_name: String,
    pub author_id: i32,
    pub category_name: Option<String>,
    pub category_id: Option<i32>,
    pub description: Option<String>,
    pub thumbnail_link: Option<String>,
    pub game_id: i32,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub last_update_at: NaiveDateTime,
    pub first_upload_at: NaiveDateTime,
    pub last_updated_files_at: Option<NaiveDateTime>,
}

#[derive(Debug)]
pub struct UnsavedMod<'a> {
    pub name: &'a str,
    pub nexus_mod_id: i32,
    pub author_name: &'a str,
    pub author_id: i32,
    pub category_name: Option<&'a str>,
    pub category_id: Option<i32>,
    pub description: Option<&'a str>,
    pub thumbnail_link: Option<&'a str>,
    pub game_id: i32,
    pub last_update_at: NaiveDateTime,
    pub first_upload_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ModForSearch {
    pub id: i32,
    pub name: String,
    pub nexus_mod_id: i32,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ModWithCells {
    pub id: i32,
    pub name: String,
    pub nexus_mod_id: i32,
    pub author_name: String,
    pub author_id: i32,
    pub category_name: Option<String>,
    pub category_id: Option<i32>,
    pub description: Option<String>,
    pub thumbnail_link: Option<String>,
    pub game_id: i32,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub last_update_at: NaiveDateTime,
    pub first_upload_at: NaiveDateTime,
    pub last_updated_files_at: Option<NaiveDateTime>,
    pub cells: Option<serde_json::Value>,
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

pub struct ModLastUpdatedFilesAt {
    pub nexus_mod_id: i32,
    pub last_updated_files_at: NaiveDateTime,
}

#[instrument(level = "debug", skip(pool))]
pub async fn bulk_get_last_updated_by_nexus_mod_ids(
    pool: &sqlx::Pool<sqlx::Postgres>,
    nexus_mod_ids: &[i32],
) -> Result<Vec<ModLastUpdatedFilesAt>> {
    sqlx::query!(
        "SELECT nexus_mod_id, last_updated_files_at FROM mods
            WHERE nexus_mod_id = ANY($1::int[])
            AND last_updated_files_at IS NOT NULL",
        nexus_mod_ids,
    )
    .map(|row| ModLastUpdatedFilesAt {
        nexus_mod_id: row.nexus_mod_id,
        last_updated_files_at: row
            .last_updated_files_at
            .expect("last_updated_files_at is null"),
    })
    .fetch_all(pool)
    .await
    .context("Failed to bulk get last_updated_files_at by nexus_mod_ids")
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert(
    pool: &sqlx::Pool<sqlx::Postgres>,
    name: &str,
    nexus_mod_id: i32,
    author_name: &str,
    author_id: i32,
    category_name: Option<&str>,
    category_id: Option<i32>,
    description: Option<&str>,
    thumbnail_link: Option<&str>,
    game_id: i32,
    last_update_at: NaiveDateTime,
    first_upload_at: NaiveDateTime,
) -> Result<Mod> {
    sqlx::query_as!(
        Mod,
        "INSERT INTO mods
            (name, nexus_mod_id, author_name, author_id, category_name, category_id, description, thumbnail_link, game_id, last_update_at, first_upload_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now(), now())
            ON CONFLICT (game_id, nexus_mod_id) DO UPDATE
            SET (name, author_name, author_id, category_name, category_id, description, thumbnail_link, last_update_at, first_upload_at, updated_at) =
            (EXCLUDED.name, EXCLUDED.author_name, EXCLUDED.author_id, EXCLUDED.category_name, EXCLUDED.category_id, EXCLUDED.description, EXCLUDED.thumbnail_link, EXCLUDED.last_update_at, EXCLUDED.first_upload_at, now())
            RETURNING *",
        name,
        nexus_mod_id,
        author_name,
        author_id,
        category_name,
        category_id,
        description,
        thumbnail_link,
        game_id,
        last_update_at,
        first_upload_at
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
        let mut author_names: Vec<&str> = vec![];
        let mut author_ids: Vec<i32> = vec![];
        let mut category_names: Vec<Option<&str>> = vec![];
        let mut category_ids: Vec<Option<i32>> = vec![];
        let mut descriptions: Vec<Option<&str>> = vec![];
        let mut thumbnail_links: Vec<Option<&str>> = vec![];
        let mut game_ids: Vec<i32> = vec![];
        let mut last_update_ats: Vec<NaiveDateTime> = vec![];
        let mut first_upload_ats: Vec<NaiveDateTime> = vec![];
        batch.iter().for_each(|unsaved_mod| {
            names.push(unsaved_mod.name);
            nexus_mod_ids.push(unsaved_mod.nexus_mod_id);
            author_names.push(unsaved_mod.author_name);
            author_ids.push(unsaved_mod.author_id);
            category_names.push(unsaved_mod.category_name);
            category_ids.push(unsaved_mod.category_id);
            descriptions.push(unsaved_mod.description);
            thumbnail_links.push(unsaved_mod.thumbnail_link);
            game_ids.push(unsaved_mod.game_id);
            last_update_ats.push(unsaved_mod.last_update_at);
            first_upload_ats.push(unsaved_mod.first_upload_at);
        });
        saved_mods.append(
            // sqlx doesn't understand arrays of Options with the query_as! macro
            &mut sqlx::query_as(
                r#"INSERT INTO mods
                (name, nexus_mod_id, author_name, author_id, category_name, category_id, description, thumbnail_link, game_id, last_update_at, first_upload_at, created_at, updated_at)
                SELECT *, now(), now()
                FROM UNNEST($1::text[], $2::int[], $3::text[], $4::int[], $5::text[], $6::int[], $7::text[], $8::text[], $9::int[], $10::timestamp(3)[], $11::timestamp(3)[])
                ON CONFLICT (game_id, nexus_mod_id) DO UPDATE
                SET (name, author_name, author_id, category_name, category_id, description, thumbnail_link, last_update_at, first_upload_at, updated_at) =
                (EXCLUDED.name, EXCLUDED.author_name, EXCLUDED.author_id, EXCLUDED.category_name, EXCLUDED.category_id, EXCLUDED.description, EXCLUDED.thumbnail_link, EXCLUDED.last_update_at, EXCLUDED.first_upload_at, now())
                RETURNING *"#,
            )
            .bind(&names)
            .bind(&nexus_mod_ids)
            .bind(&author_names)
            .bind(&author_ids)
            .bind(&category_names)
            .bind(&category_ids)
            .bind(&descriptions)
            .bind(&thumbnail_links)
            .bind(&game_ids)
            .bind(&last_update_ats)
            .bind(&first_upload_ats)
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

#[instrument(level = "debug", skip(pool))]
pub async fn bulk_get_need_backfill(pool: &sqlx::Pool<sqlx::Postgres>) -> Result<Vec<Mod>> {
    sqlx::query_as!(
        Mod,
        "SELECT * FROM mods
            WHERE author_id IS NULL"
    )
    .fetch_all(pool)
    .await
    .context("Failed to bulk get need backfill")
}

#[instrument(level = "debug", skip(pool, game_mod, mod_data))]
pub async fn update_from_api_response<'a>(
    pool: &sqlx::Pool<sqlx::Postgres>,
    game_mod: &Mod,
    mod_data: &ExtractedModData<'a>,
) -> Result<Mod> {
    let name = mod_data.name.unwrap_or(&game_mod.name);
    let category_id = match mod_data.category_id {
        Some(category_id) => Some(category_id),
        None => game_mod.category_id,
    };

    let mut ret = sqlx::query_as!(
        Mod,
        "UPDATE mods
            SET
                nexus_mod_id = $2,
                name = $3,
                category_id = $4,
                author_name = $5,
                author_id = $6,
                last_update_at = $7,
                first_upload_at = $8
            WHERE id = $1
            RETURNING *",
        game_mod.id,
        mod_data.nexus_mod_id,
        name,
        category_id,
        mod_data.author_name,
        mod_data.author_id,
        mod_data.last_update_at,
        mod_data.first_upload_at,
    )
    .fetch_one(pool)
    .await
    .context("Failed to update mod from api response")?;

    if let Some(description) = mod_data.description {
        ret = sqlx::query_as!(
            Mod,
            "UPDATE mods
                SET
                    description = $2
                WHERE id = $1
                RETURNING *",
            game_mod.id,
            description,
        )
        .fetch_one(pool)
        .await
        .context("Failed to update mod from api response")?;
    }

    if let Some(thumbnail_link) = mod_data.thumbnail_link {
        ret = sqlx::query_as!(
            Mod,
            "UPDATE mods
                SET
                    thumbnail_link = $2
                WHERE id = $1
                RETURNING *",
            game_mod.id,
            thumbnail_link,
        )
        .fetch_one(pool)
        .await
        .context("Failed to update mod from api response")?;
    }

    Ok(ret)
}

#[instrument(level = "debug", skip(pool))]
pub async fn batched_get_for_search(
    pool: &sqlx::Pool<sqlx::Postgres>,
    page_size: i64,
    last_id: Option<i32>,
) -> Result<Vec<ModForSearch>> {
    let last_id = last_id.unwrap_or(0);
    sqlx::query_as!(
        ModForSearch,
        "SELECT
            id,
            name,
            nexus_mod_id
        FROM mods
        WHERE id > $2
        ORDER BY mods.id ASC
        LIMIT $1",
        page_size,
        last_id,
    )
    .fetch_all(pool)
    .await
    .context("Failed to batch get for search")
}

#[instrument(level = "debug", skip(pool))]
pub async fn batched_get_with_cells(
    pool: &sqlx::Pool<sqlx::Postgres>,
    page_size: i64,
    last_id: Option<i32>,
    master: &str,
    world_id: i32,
) -> Result<Vec<ModWithCells>> {
    let last_id = last_id.unwrap_or(0);
    sqlx::query_as!(
        ModWithCells,
        "SELECT
            mods.*,
            COALESCE(json_agg(DISTINCT jsonb_build_object('x', cells.x, 'y', cells.y)) FILTER (WHERE cells.x IS NOT NULL AND cells.y IS NOT NULL AND cells.master = $3 AND cells.world_id = $4), '[]') AS cells
        FROM mods
        LEFT OUTER JOIN plugin_cells ON plugin_cells.mod_id = mods.id
        LEFT OUTER JOIN cells ON cells.id = plugin_cells.cell_id
        WHERE mods.id > $2
        GROUP BY mods.id
        ORDER BY mods.id ASC
        LIMIT $1",
        page_size,
        last_id,
        master,
        world_id
    )
    .fetch_all(pool)
    .await
    .context("Failed to batch get with cells")
}
