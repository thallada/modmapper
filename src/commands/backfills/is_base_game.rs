use std::borrow::Borrow;
use std::fs::File;
use std::io::BufReader;

use anyhow::{Context, Result};
use skyrim_cell_dump::Plugin;
use tracing::info;

use crate::models::cell::{self, UnsavedCell};
use crate::models::world::{self, UnsavedWorld};
use crate::plugin_processor::get_local_form_id_and_master;

pub async fn backfill_is_base_game(pool: &sqlx::Pool<sqlx::Postgres>) -> Result<()> {
    let file = File::open("./data/skyrim.json")?;
    let reader = BufReader::new(file);
    let plugin: Plugin =
        serde_json::from_reader(reader).context("failed to deserialize data/skyrim.json")?;
    let file_name = "Skyrim.esm";
    let masters: Vec<&str> = plugin.header.masters.iter().map(|s| s.borrow()).collect();
    let base_worlds: Vec<UnsavedWorld> = plugin
        .worlds
        .iter()
        .map(|world| {
            let (form_id, master) =
                get_local_form_id_and_master(world.form_id, &masters, file_name)
                    .expect("form_id to be a valid i32");
            UnsavedWorld { form_id, master }
        })
        .collect();
    let db_worlds = world::batched_insert(pool, &base_worlds).await?;
    info!("Upserted {} Skyrim.esm base worlds", db_worlds.len());
    let base_cells: Vec<UnsavedCell> = plugin
        .cells
        .iter()
        .map(|cell| {
            let world_id = if let Some(world_form_id) = cell.world_form_id {
                let (form_id, master) =
                    get_local_form_id_and_master(world_form_id, &masters, file_name)
                        .expect("form_id to be valid i32");
                Some(
                    db_worlds
                        .iter()
                        .find(|&world| world.form_id == form_id && world.master == master)
                        .expect("cell references world in the plugin worlds")
                        .id,
                )
            } else {
                None
            };
            let (form_id, master) = get_local_form_id_and_master(cell.form_id, &masters, file_name)
                .expect("form_id is a valid i32");
            UnsavedCell {
                form_id,
                master,
                x: cell.x,
                y: cell.y,
                world_id,
                is_persistent: cell.is_persistent,
                is_base_game: true, // the whole point of this function
            }
        })
        .collect();
    let db_cells = cell::batched_insert(pool, &base_cells).await?;
    info!("Upserted {} Skyrim.esm base cells", db_cells.len());
    // This works for exterior cells, but there's a bug with the unique index on cells that
    // creates duplicate interior cells. To fix that, I need to upgrade postgres to
    // 15 or later, migate the data to the new db cluster, consolidate all of the duplicate cells
    // into one cell in a separate backfill command, then fix the unique index.
    Ok(())
}
