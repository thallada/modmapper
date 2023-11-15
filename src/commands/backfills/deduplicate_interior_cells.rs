/// There was a bug with the unique index on the `cells` table that was causing the same form_id
/// and master to be inserted multiple times. This function deduplicates those, choosing the cell
/// with `is_base_game = true` if there is one, otherwise randomly chooses one.
/// rows referencing the duplicate cells are updated to reference the chosen cell.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::FromRow;
use tracing::info;

const PAGE_SIZE: i64 = 100;

#[derive(Debug, Clone, Deserialize, Serialize, FromRow)]
pub struct CellId {
    pub id: i32,
    pub is_base_game: bool,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CellDuplicates {
    pub ids: Option<Json<Vec<CellId>>>,
    pub form_id: i32,
    pub master: String,
}

pub async fn deduplicate_interior_cells(pool: &sqlx::Pool<sqlx::Postgres>) -> Result<()> {
    let mut page = 0;
    loop {
        info!("deduplicating interior cells page {}", page);
        let duplicates = sqlx::query_as!(
            CellDuplicates,
            r#"SELECT
                json_agg(
                    json_build_object(
                        'id', id,
                        'is_base_game', is_base_game
                    )) as "ids: Json<Vec<CellId>>",
                form_id,
                master
            FROM cells
            WHERE world_id IS NULL
            GROUP BY (form_id, master)
            HAVING COUNT(*) > 1
            LIMIT $1
            "#,
            PAGE_SIZE,
        )
        .fetch_all(pool)
        .await?;

        if duplicates.is_empty() {
            break;
        }

        for duplicate_cell in duplicates.into_iter() {
            info!(
                "deduplicating cells form_id={} master={}",
                duplicate_cell.form_id, duplicate_cell.master
            );
            let duplicate_ids = duplicate_cell.ids.clone().unwrap();
            let chosen_cell = duplicate_ids
                .iter()
                .find(|cell| cell.is_base_game)
                .unwrap_or_else(|| {
                    duplicate_ids
                        .iter()
                        .next()
                        .expect("duplicate cell has no ids")
                });
            info!(
                "choosing cell_id={} is_base_game={}",
                chosen_cell.id, chosen_cell.is_base_game
            );
            // Update all plugin_cells cell_id references to point to the chosen cell
            let duplicate_ids = duplicate_cell
                .ids
                .unwrap()
                .iter()
                .map(|cell| cell.id)
                .collect::<Vec<_>>();

            // First, I need to fix-up any duplicated plugin_cells rows caused by broken
            // plugins that have multiple cells with the same form_id. For these duplicate
            // plugin_cells with the same plugin_id, I just arbitrarily choose one and delete
            // the others (since it's undefined behavior of which duplicate record should "win"
            // out in this case anyways). In the case of exterior cells, where the duplicate
            // interior cell bug is not a problem, the last processed cell record in the plugin
            // wins since `process_plugin` uses an upsert method which updates existing
            // `plugin_cells` if it tries to insert a new one that conflicts with an existing one.
            // So I am effectively retroactively doing the same here for interior cells.
            let plugin_cells_delete = sqlx::query!(
                r#"DELETE FROM plugin_cells
                WHERE id NOT IN (
                    SELECT MIN(id)
                    FROM plugin_cells
                    WHERE cell_id = ANY($1)
                    GROUP BY plugin_id
                )
                AND cell_id = ANY($1)
                "#,
                &duplicate_ids
            )
            .execute(pool)
            .await?;
            info!(
                "deleted {} duplicate plugin_cells from broken plugins",
                plugin_cells_delete.rows_affected()
            );

            let update = sqlx::query!(
                r#"UPDATE plugin_cells
                SET
                    cell_id = $1,
                    updated_at = now()
                WHERE cell_id = ANY($2)"#,
                chosen_cell.id,
                &duplicate_ids
            )
            .execute(pool)
            .await?;
            info!("updated {} plugin_cells", update.rows_affected());

            // Delete all cells that are not the chosen cell
            let delete = sqlx::query!(
                r#"DELETE FROM cells
                WHERE id != $1 AND id = ANY($2)"#,
                chosen_cell.id,
                &duplicate_ids
            )
            .execute(pool)
            .await?;
            info!("deleted {} cells", delete.rows_affected());
        }
        page += 1;
    }
    Ok(())
}
