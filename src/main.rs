use anyhow::Result;
use argh::FromArgs;
use compress_tools::{list_archive_files, uncompress_archive_file};
use dotenv::dotenv;
use reqwest::StatusCode;
use skyrim_cell_dump::parse_plugin;
use sqlx::postgres::PgPoolOptions;
use std::borrow::Borrow;
use std::convert::TryInto;
use std::env;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tempfile::tempdir;
use tokio::fs::create_dir_all;
use tokio::io::AsyncWriteExt;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::time::sleep;
use tracing::{debug, error, info, info_span, warn};
use unrar::Archive;

mod models;
mod nexus_api;
mod nexus_scraper;

use models::game;
use models::plugin;
use models::{cell, cell::UnsavedCell};
use models::{file, file::File};
use models::{
    game_mod,
    game_mod::{Mod, UnsavedMod},
};
use models::{plugin_cell, plugin_cell::UnsavedPluginCell};
use models::{plugin_world, plugin_world::UnsavedPluginWorld};
use models::{world, world::UnsavedWorld};
use nexus_api::{GAME_ID, GAME_NAME};

#[derive(FromArgs)]
/// Downloads every mod off nexus mods, parses CELL and WRLD data from plugins in each, and saves the data to the database.
struct Args {
    #[argh(option, short = 'p', default = "1")]
    /// the page number to start scraping for mods on nexus mods.
    page: usize,
}

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

async fn process_plugin(
    plugin_buf: &mut [u8],
    pool: &sqlx::Pool<sqlx::Postgres>,
    db_file: &File,
    db_mod: &Mod,
    file_path: &str,
) -> Result<()> {
    if plugin_buf.len() == 0 {
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
                &db_file.name,
                hash as i64,
                db_file.id,
                plugin.header.version as f64,
                plugin_buf.len() as i64,
                author,
                description,
                &masters,
                &file_name,
                file_path,
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
                    editor_id: plugin_cell.editor_id.as_ref().map(|id| id.as_ref()),
                })
                .collect();
            plugin_cell::batched_insert(&pool, &plugin_cells).await?;
        }
        Err(err) => {
            warn!(error = %err, "Failed to parse plugin, skipping plugin");
        }
    }

    let plugin_path = format!(
        "plugins/{}/{}/{}/{}",
        GAME_NAME, db_mod.nexus_mod_id, db_file.nexus_file_id, file_path
    );
    let plugin_path = Path::new(&plugin_path);
    if let Some(dir) = plugin_path.parent() {
        create_dir_all(dir).await?;
    }
    let mut file = tokio::fs::File::create(plugin_path).await?;

    info!(path = %plugin_path.display(), "saving plugin to disk");
    file.write_all(&plugin_buf).await?;
    Ok(())
}

#[tokio::main]
pub async fn main() -> Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt::init();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL")?)
        .await?;
    let game = game::insert(&pool, GAME_NAME, GAME_ID as i32).await?;
    let client = reqwest::Client::new();

    let args: Args = argh::from_env();
    let mut page = args.page;
    let mut has_next_page = true;

    while has_next_page {
        let page_span = info_span!("page", page);
        let _page_span = page_span.enter();
        let mod_list_resp = nexus_scraper::get_mod_list_page(&client, page).await?;
        let scraped = mod_list_resp.scrape_mods()?;

        has_next_page = scraped.has_next_page;
        let present_mods = game_mod::bulk_get_present_nexus_mod_ids(
            &pool,
            &scraped
                .mods
                .iter()
                .map(|scraped_mod| scraped_mod.nexus_mod_id)
                .collect::<Vec<i32>>(),
        )
        .await?;
        let mods_to_create: Vec<UnsavedMod> = scraped
            .mods
            .iter()
            .filter(|scraped_mod| !present_mods.contains(&scraped_mod.nexus_mod_id))
            .map(|scraped_mod| UnsavedMod {
                name: scraped_mod.name,
                nexus_mod_id: scraped_mod.nexus_mod_id,
                author: scraped_mod.author,
                category: scraped_mod.category,
                description: scraped_mod.desc,
                game_id: game.id,
            })
            .collect();

        let mods = game_mod::batched_insert(&pool, &mods_to_create).await?;

        for db_mod in mods {
            let mod_span = info_span!("mod", name = ?&db_mod.name, id = &db_mod.nexus_mod_id);
            let _mod_span = mod_span.enter();
            let files_resp = nexus_api::files::get(&client, db_mod.nexus_mod_id).await?;

            debug!(duration = ?files_resp.wait, "sleeping");
            sleep(files_resp.wait).await;

            // Filter out replaced/deleted files (indicated by null category) and archived files
            let files = files_resp
                .files()?
                .into_iter()
                .filter(|file| match file.category {
                    None => {
                        info!(
                            name = file.file_name,
                            id = file.file_id,
                            "skipping file with no category"
                        );
                        false
                    }
                    Some(category) if category == "ARCHIVED" => false,
                    Some(_) => true,
                });

            for api_file in files {
                let file_span =
                    info_span!("file", name = &api_file.file_name, id = &api_file.file_id);
                let _file_span = file_span.enter();
                let db_file = file::insert(
                    &pool,
                    api_file.name,
                    api_file.file_name,
                    api_file.file_id as i32,
                    db_mod.id,
                    api_file.category,
                    api_file.version,
                    api_file.mod_version,
                    api_file.size,
                    api_file.uploaded_at,
                )
                .await?;

                let mut checked_metadata = false;
                match nexus_api::metadata::contains_plugin(&client, &api_file).await {
                    Ok(contains_plugin) => {
                        if let Some(contains_plugin) = contains_plugin {
                            checked_metadata = true;
                            if !contains_plugin {
                                info!("file metadata does not contain a plugin, skip downloading");
                                continue;
                            }
                        } else {
                            warn!("file has no metadata link, continuing with download");
                        }
                    }
                    Err(err) => {
                        warn!(error = %err, "error retreiving metadata for file, continuing with download");
                    }
                };

                let download_link_resp =
                    nexus_api::download_link::get(&client, db_mod.nexus_mod_id, api_file.file_id)
                        .await;
                if let Err(err) = &download_link_resp {
                    if let Some(reqwest_err) = err.downcast_ref::<reqwest::Error>() {
                        if reqwest_err.status() == Some(StatusCode::NOT_FOUND) {
                            warn!(
                                status = ?reqwest_err.status(),
                                "failed to get download link for file, skipping file"
                            );
                            file::update_has_download_link(&pool, db_file.id, false).await?;
                            continue;
                        }
                    }
                }
                let download_link_resp = download_link_resp?;

                let mut tokio_file = match download_link_resp.download_file(&client).await {
                    Ok(file) => {
                        info!(bytes = api_file.size, "download finished");
                        file::update_downloaded_at(&pool, db_file.id).await?;
                        file
                    }
                    Err(err) => {
                        warn!(error = %err, "failed all attempts at downloading file, skipping file");
                        continue;
                    }
                };

                let mut initial_bytes = [0; 8];
                tokio_file.seek(SeekFrom::Start(0)).await?;
                match tokio_file.read_exact(&mut initial_bytes).await {
                    Err(err) => {
                        warn!(error = %err, "failed to read initial bytes, skipping file");
                        continue;
                    }
                    _ => {}
                }
                let kind = infer::get(&initial_bytes).expect("unknown file type of file download");
                info!(
                    mime_type = kind.mime_type(),
                    "inferred mime_type of downloaded archive"
                );

                match kind.mime_type() {
                    "application/vnd.rar" => {
                        info!("downloaded archive is RAR archive, attempt to uncompress entire archive");
                        // Use unrar to uncompress the entire .rar file to avoid bugs with compress_tools uncompressing certain .rar files:
                        // https://github.com/libarchive/libarchive/issues/373, https://github.com/libarchive/libarchive/issues/1426
                        tokio_file.seek(SeekFrom::Start(0)).await?;
                        let mut file = tokio_file.try_clone().await?.into_std().await;
                        let temp_dir = tempdir()?;
                        let temp_file_path = temp_dir.path().join("download.rar");
                        let mut temp_file = std::fs::File::create(&temp_file_path)?;
                        std::io::copy(&mut file, &mut temp_file)?;

                        let mut plugin_file_paths = Vec::new();
                        let list =
                            Archive::new(&temp_file_path.to_string_lossy().to_string())?.list();
                        if let Ok(list) = list {
                            for entry in list {
                                if let Ok(entry) = entry {
                                    if let Some(extension) = entry.filename.extension() {
                                        if entry.is_file()
                                            && (extension == "esp"
                                                || extension == "esm"
                                                || extension == "esl")
                                        {
                                            plugin_file_paths.push(entry.filename);
                                        }
                                    }
                                }
                            }
                        } else {
                            if !checked_metadata {
                                warn!("failed to read archive and server has no metadata, skipping file");
                                continue;
                            } else {
                                error!("failed to read archive, but server had metadata");
                                panic!("failed to read archive, but server had metadata");
                            }
                        }
                        info!(
                            num_plugin_files = plugin_file_paths.len(),
                            "listed plugins in downloaded archive"
                        );

                        if plugin_file_paths.len() > 0 {
                            info!("uncompressing downloaded archive");
                            let extract =
                                Archive::new(&temp_file_path.to_string_lossy().to_string())?
                                    .extract_to(temp_dir.path().to_string_lossy().to_string());
                            extract
                                .expect("failed to extract")
                                .process()
                                .expect("failed to extract");

                            for file_path in plugin_file_paths.iter() {
                                info!(
                                    ?file_path,
                                    "processing uncompressed file from downloaded archive"
                                );
                                let mut plugin_buf =
                                    std::fs::read(temp_dir.path().join(file_path))?;
                                process_plugin(
                                    &mut plugin_buf,
                                    &pool,
                                    &db_file,
                                    &db_mod,
                                    &file_path.to_string_lossy(),
                                )
                                .await?;
                            }
                        }
                        temp_dir.close()?;
                    }
                    _ => {
                        tokio_file.seek(SeekFrom::Start(0)).await?;
                        let mut file = tokio_file.try_clone().await?.into_std().await;
                        let mut plugin_file_paths = Vec::new();

                        let archive_files = match list_archive_files(&file) {
                            Ok(files) => Ok(files),
                            Err(err) => {
                                if !checked_metadata {
                                    warn!(error = %err, "failed to read archive and server has no metadata, skipping file");
                                    continue;
                                } else {
                                    error!(error = %err, "failed to read archive, but server had metadata");
                                    Err(err)
                                }
                            }
                        }?;
                        for file_path in archive_files {
                            if file_path.ends_with(".esp")
                                || file_path.ends_with(".esm")
                                || file_path.ends_with(".esl")
                            {
                                plugin_file_paths.push(file_path);
                            }
                        }
                        info!(
                            num_plugin_files = plugin_file_paths.len(),
                            "listed plugins in downloaded archive"
                        );

                        for file_path in plugin_file_paths.iter() {
                            let plugin_span = info_span!("plugin", name = ?file_path);
                            let plugin_span = plugin_span.enter();
                            file.seek(SeekFrom::Start(0))?;
                            let mut buf = Vec::default();
                            info!("uncompressing plugin file from downloaded archive");
                            match uncompress_archive_file(&mut file, &mut buf, file_path) {
                                Ok(_) => Ok(()),
                                Err(err) => {
                                    if kind.mime_type() == "application/zip" {
                                        // compress_tools or libarchive failed to extract zip file (e.g. archive is deflate64 compressed)
                                        // Attempt to uncompress the archive using `unzip` unix command instead
                                        warn!(error = %err, "failed to extract file with compress_tools, extracting whole archive with unzip instead");
                                        drop(plugin_span);
                                        file.seek(SeekFrom::Start(0))?;
                                        let temp_dir = tempdir()?;
                                        let temp_file_path = temp_dir
                                            .path()
                                            .join(format!("download.{}", kind.extension()));
                                        let mut temp_file = std::fs::File::create(&temp_file_path)?;
                                        std::io::copy(&mut file, &mut temp_file)?;
                                        let extracted_path = temp_dir.path().join("extracted");

                                        Command::new("unzip")
                                            .args(&[
                                                &temp_file_path.to_string_lossy(),
                                                "-d",
                                                &extracted_path.to_string_lossy(),
                                            ])
                                            .status()?;

                                        for file_path in plugin_file_paths.iter() {
                                            let plugin_span =
                                                info_span!("plugin", name = ?file_path);
                                            let _plugin_span = plugin_span.enter();
                                            info!("processing uncompressed file from downloaded archive");
                                            let mut plugin_buf =
                                                std::fs::read(extracted_path.join(file_path))?;
                                            process_plugin(
                                                &mut plugin_buf,
                                                &pool,
                                                &db_file,
                                                &db_mod,
                                                file_path,
                                            )
                                            .await?;
                                        }

                                        break;
                                    }
                                    Err(err)
                                }
                            }?;
                            process_plugin(&mut buf, &pool, &db_file, &db_mod, file_path).await?;
                        }
                    }
                }

                debug!(duration = ?download_link_resp.wait, "sleeping");
                sleep(download_link_resp.wait).await;
            }
        }

        page += 1;
        debug!(?page, ?has_next_page, "sleeping 1 second");
        sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}
