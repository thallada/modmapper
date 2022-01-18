use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::instrument;

use super::BATCH_SIZE;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Cell {
    pub id: i32,
    pub form_id: i32,
    pub master: String,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub world_id: Option<i32>,
    pub is_persistent: bool,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Debug)]
pub struct UnsavedCell<'a> {
    pub form_id: i32,
    pub master: &'a str,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub world_id: Option<i32>,
    pub is_persistent: bool,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct CellData {
    pub form_id: i32,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub is_persistent: bool,
    pub plugins_count: Option<i64>,
    pub files_count: Option<i64>,
    pub mods_count: Option<i64>,
    pub mods: Option<serde_json::Value>,
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert(
    pool: &sqlx::Pool<sqlx::Postgres>,
    form_id: i32,
    master: &str,
    x: Option<i32>,
    y: Option<i32>,
    world_id: Option<i32>,
    is_persistent: bool,
) -> Result<Cell> {
    sqlx::query_as!(
        Cell,
        "INSERT INTO cells
            (form_id, master, x, y, world_id, is_persistent, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, now(), now())
            ON CONFLICT (form_id, master, world_id) DO UPDATE
            SET (x, y, is_persistent, updated_at) =
            (EXCLUDED.x, EXCLUDED.y, EXCLUDED.is_persistent, now())
            RETURNING *",
        form_id,
        master,
        x,
        y,
        world_id,
        is_persistent
    )
    .fetch_one(pool)
    .await
    .context("Failed to insert cell")
}

#[instrument(level = "debug", skip(pool))]
pub async fn batched_insert<'a>(
    pool: &sqlx::Pool<sqlx::Postgres>,
    cells: &[UnsavedCell<'a>],
) -> Result<Vec<Cell>> {
    let mut saved_cells = vec![];
    for batch in cells.chunks(BATCH_SIZE) {
        let mut form_ids: Vec<i32> = vec![];
        let mut masters: Vec<&str> = vec![];
        let mut xs: Vec<Option<i32>> = vec![];
        let mut ys: Vec<Option<i32>> = vec![];
        let mut world_ids: Vec<Option<i32>> = vec![];
        let mut is_persistents: Vec<bool> = vec![];
        batch.iter().for_each(|unsaved_cell| {
            form_ids.push(unsaved_cell.form_id);
            masters.push(unsaved_cell.master);
            xs.push(unsaved_cell.x);
            ys.push(unsaved_cell.y);
            world_ids.push(unsaved_cell.world_id);
            is_persistents.push(unsaved_cell.is_persistent);
        });
        saved_cells.append(
            // sqlx doesn't understand arrays of Options with the query_as! macro
            &mut sqlx::query_as(
                r#"INSERT INTO cells (form_id, master, x, y, world_id, is_persistent, created_at, updated_at)
                SELECT *, now(), now() FROM UNNEST($1::int[], $2::text[], $3::int[], $4::int[], $5::int[], $6::bool[])
                ON CONFLICT (form_id, master, world_id) DO UPDATE
                SET (x, y, is_persistent, updated_at) =
                (EXCLUDED.x, EXCLUDED.y, EXCLUDED.is_persistent, now())
                RETURNING *"#,
            )
            .bind(&form_ids)
            .bind(&masters)
            .bind(&xs)
            .bind(&ys)
            .bind(&world_ids)
            .bind(&is_persistents)
            .fetch_all(pool)
            .await
            .context("Failed to insert cells")?,
        );
    }
    Ok(saved_cells)
}

#[instrument(level = "debug", skip(pool))]
pub async fn count_mod_edits(
    pool: &sqlx::Pool<sqlx::Postgres>,
    master: &str,
    world_id: i32,
    x: i32,
    y: i32,
) -> Result<Option<i64>> {
    sqlx::query_scalar!(
        "SELECT COUNT(DISTINCT mods.id)
            FROM cells
            JOIN plugin_cells on cells.id = cell_id
            JOIN plugins ON plugins.id = plugin_id
            JOIN files ON files.id = file_id
            JOIN mods ON mods.id = mod_id
            WHERE master = $1 AND world_id = $2 AND x = $3 and y = $4",
        master,
        world_id,
        x,
        y,
    )
    .fetch_one(pool)
    .await
    .context("Failed to count mod edits on cell")
}

/// Returns cell properties plus a list of mods that edit the cell
#[instrument(level = "debug", skip(pool))]
pub async fn get_cell_data(
    pool: &sqlx::Pool<sqlx::Postgres>,
    master: &str,
    world_id: i32,
    x: i32,
    y: i32,
) -> Result<CellData> {
    sqlx::query_as!(
        CellData,
        r#"SELECT
                cells.x,
                cells.y,
                cells.is_persistent,
                cells.form_id,
                COUNT(DISTINCT plugins.id) as plugins_count,
                COUNT(DISTINCT files.id) as files_count,
                COUNT(DISTINCT mods.id) as mods_count,
                json_agg(mods.*) as mods
            FROM cells
            JOIN plugin_cells on cells.id = cell_id
            JOIN plugins ON plugins.id = plugin_id
            JOIN files ON files.id = file_id
            JOIN mods ON mods.id = mod_id
            WHERE master = $1 AND world_id = $2 AND x = $3 and y = $4
            GROUP BY cells.x, cells.y, cells.is_persistent, cells.form_id"#,
        master,
        world_id,
        x,
        y
    )
    .fetch_one(pool)
    .await
    .context("Failed get cell data")
}
