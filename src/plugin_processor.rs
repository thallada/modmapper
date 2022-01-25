use anyhow::Result;
use skyrim_cell_dump::parse_plugin;
use std::borrow::Borrow;
use std::convert::TryInto;
use std::path::{Path, PathBuf};
use tokio::fs::create_dir_all;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::models::file::File;
use crate::models::game_mod::Mod;
use crate::models::{cell, cell::UnsavedCell};
use crate::models::{plugin, plugin::UnsavedPlugin};
use crate::models::{plugin_cell, plugin_cell::UnsavedPluginCell};
use crate::models::{plugin_world, plugin_world::UnsavedPluginWorld};
use crate::models::{world, world::UnsavedWorld};
use crate::nexus_api::GAME_NAME;

fn get_local_form_id_and_master<'a>(
    form_id: u32,
    masters: &'a [&str],
    file_name: &'a str,
) -> Result<(i32, &'a str)> {
    let master_index = (form_id >> 24) as usize;
    let local_form_id = (form_id & 0xFFFFFF).try_into()?;
    if master_index >= masters.len() {
        return Ok((local_form_id, file_name));
    }
    Ok((local_form_id, masters[master_index]))
}

pub async fn process_plugin(
    plugin_buf: &mut [u8],
    pool: &sqlx::Pool<sqlx::Postgres>,
    db_file: &File,
    db_mod: &Mod,
    file_path: &str,
) -> Result<()> {
    if plugin_buf.is_empty() {
        warn!("skipping processing of invalid empty plugin");
        return Ok(());
    }
    info!(bytes = plugin_buf.len(), "parsing plugin");
    match parse_plugin(&plugin_buf) {
        Ok(plugin) => {
            info!(
                num_worlds = plugin.worlds.len(),
                num_cells = plugin.cells.len(),
                "parse finished"
            );
            let hash = seahash::hash(&plugin_buf);
            let file_name = Path::new(file_path)
                .file_name()
                .expect("plugin path ends in a valid file_name")
                .to_string_lossy();
            let author = plugin.header.author.as_deref();
            let description = plugin.header.description.as_deref();
            let masters: Vec<&str> = plugin.header.masters.iter().map(|s| s.borrow()).collect();
            let plugin_row = plugin::insert(
                &pool,
                &UnsavedPlugin {
                    name: &db_file.name,
                    hash: hash as i64,
                    file_id: db_file.id,
                    mod_id: db_mod.id,
                    version: plugin.header.version as f64,
                    size: plugin_buf.len() as i64,
                    author,
                    description,
                    masters: &masters,
                    file_name: &file_name,
                    file_path,
                },
            )
            .await?;

            let worlds: Vec<UnsavedWorld> = plugin
                .worlds
                .iter()
                .map(|world| {
                    let (form_id, master) =
                        get_local_form_id_and_master(world.form_id, &masters, &file_name)
                            .expect("form_id to be a valid i32");
                    UnsavedWorld { form_id, master }
                })
                .collect();
            let db_worlds = world::batched_insert(&pool, &worlds).await?;
            let plugin_worlds: Vec<UnsavedPluginWorld> = db_worlds
                .iter()
                .zip(&plugin.worlds)
                .map(|(db_world, plugin_world)| UnsavedPluginWorld {
                    plugin_id: plugin_row.id,
                    world_id: db_world.id,
                    editor_id: &plugin_world.editor_id,
                })
                .collect();
            plugin_world::batched_insert(&pool, &plugin_worlds).await?;

            let cells: Vec<UnsavedCell> = plugin
                .cells
                .iter()
                .map(|cell| {
                    let world_id = if let Some(world_form_id) = cell.world_form_id {
                        let (form_id, master) =
                            get_local_form_id_and_master(world_form_id, &masters, &file_name)
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
                    let (form_id, master) =
                        get_local_form_id_and_master(cell.form_id, &masters, &file_name)
                            .expect("form_id is a valid i32");
                    UnsavedCell {
                        form_id,
                        master,
                        x: cell.x,
                        y: cell.y,
                        world_id,
                        is_persistent: cell.is_persistent,
                    }
                })
                .collect();
            let db_cells = cell::batched_insert(&pool, &cells).await?;
            let plugin_cells: Vec<UnsavedPluginCell> = db_cells
                .iter()
                .zip(&plugin.cells)
                .map(|(db_cell, plugin_cell)| UnsavedPluginCell {
                    plugin_id: plugin_row.id,
                    cell_id: db_cell.id,
                    file_id: db_file.id,
                    mod_id: db_mod.id,
                    editor_id: plugin_cell.editor_id.as_ref().map(|id| id.as_ref()),
                })
                .collect();
            plugin_cell::batched_insert(&pool, &plugin_cells).await?;
        }
        Err(err) => {
            warn!(error = %err, "Failed to parse plugin, skipping plugin");
        }
    }

    let plugin_path = [
        "plugins",
        GAME_NAME,
        &format!("{}", db_mod.nexus_mod_id),
        &format!("{}", db_file.nexus_file_id),
        file_path,
    ]
    .iter()
    .collect::<PathBuf>();
    let plugin_path = plugin_path.as_path();
    if let Some(dir) = plugin_path.parent() {
        create_dir_all(dir).await?;
    }
    let mut file = tokio::fs::File::create(plugin_path).await?;

    info!(path = %plugin_path.display(), "saving plugin to disk");
    file.write_all(&plugin_buf).await?;
    Ok(())
}
