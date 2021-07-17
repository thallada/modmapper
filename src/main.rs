use anyhow::Result;
use compress_tools::{list_archive_files, uncompress_archive_file};
use dotenv::dotenv;
use reqwest::StatusCode;
use skyrim_cell_dump::parse_plugin;
use sqlx::postgres::PgPoolOptions;
use std::convert::TryInto;
use std::env;
use std::fs::OpenOptions;
use std::io::Seek;
use std::io::SeekFrom;
use std::process::Command;
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::time::sleep;
use tracing::{debug, info, info_span, warn};
use unrar::Archive;
use zip::write::{FileOptions, ZipWriter};

mod models;
mod nexus_api;
mod nexus_scraper;

use models::cell;
use models::game;
use models::plugin;
use models::plugin_cell;
use models::{file, file::File};
use models::{game_mod, game_mod::Mod};
use nexus_api::{GAME_ID, GAME_NAME};

async fn process_plugin<W>(
    plugin_buf: &mut [u8],
    pool: &sqlx::Pool<sqlx::Postgres>,
    plugin_archive: &mut ZipWriter<W>,
    db_file: &File,
    mod_obj: &Mod,
    file_name: &str,
) -> Result<()>
where
    W: std::io::Write + std::io::Seek,
{
    if plugin_buf.len() == 0 {
        warn!("skipping processing of invalid empty plugin");
        return Ok(());
    }
    info!(bytes = plugin_buf.len(), "parsing plugin");
    let plugin = parse_plugin(&plugin_buf)?;
    info!(num_cells = plugin.cells.len(), "parse finished");
    let hash = seahash::hash(&plugin_buf);
    let plugin_row = plugin::insert(
        &pool,
        &db_file.name,
        hash as i64,
        db_file.id,
        Some(plugin.header.version as f64),
        plugin_buf.len() as i64,
        plugin.header.author,
        plugin.header.description,
        Some(
            &plugin
                .header
                .masters
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>(),
        ),
    )
    .await?;
    for cell in plugin.cells {
        let cell_row = cell::insert(
            &pool,
            cell.form_id.try_into().unwrap(),
            cell.x,
            cell.y,
            cell.is_persistent,
        )
        .await?;
        plugin_cell::insert(&pool, plugin_row.id, cell_row.id, cell.editor_id).await?;
    }
    plugin_archive.start_file(
        format!(
            "{}/{}/{}/{}",
            GAME_NAME, mod_obj.nexus_mod_id, db_file.nexus_file_id, file_name
        ),
        FileOptions::default(),
    )?;

    let mut reader = std::io::Cursor::new(&plugin_buf);
    std::io::copy(&mut reader, plugin_archive)?;
    Ok(())
}

fn initialize_plugins_archive(mod_id: i32, file_id: i32) -> Result<()> {
    let mut plugins_archive = ZipWriter::new(
        OpenOptions::new()
            .write(true)
            .create(true)
            .open("plugins.zip")?,
    );
    plugins_archive.add_directory(
        format!("{}/{}/{}", GAME_NAME, mod_id, file_id),
        FileOptions::default(),
    )?;
    plugins_archive.finish()?;
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

    let mut page: i32 = 1;
    let mut has_next_page = true;

    while has_next_page {
        let page_span = info_span!("page", page);
        let _page_span = page_span.enter();
        let mod_list_resp = nexus_scraper::get_mod_list_page(&client, page).await?;
        let scraped = mod_list_resp.scrape_mods()?;

        has_next_page = scraped.has_next_page;
        let mut mods = Vec::new();
        for scraped_mod in scraped.mods {
            if let None = game_mod::get_by_nexus_mod_id(&pool, scraped_mod.nexus_mod_id).await? {
                mods.push(
                    game_mod::insert(
                        &pool,
                        scraped_mod.name,
                        scraped_mod.nexus_mod_id,
                        scraped_mod.author,
                        scraped_mod.category,
                        scraped_mod.desc,
                        game.id,
                    )
                    .await?,
                );
            }
        }

        for db_mod in mods {
            let mod_span = info_span!("mod", name = ?&db_mod.name, id = &db_mod.nexus_mod_id);
            let _mod_span = mod_span.enter();
            let files_resp = nexus_api::files::get(&client, db_mod.nexus_mod_id).await?;

            debug!(duration = ?files_resp.wait, "sleeping");
            sleep(files_resp.wait).await;

            // Filter out replaced/deleted files (indicated by null category)
            let files = files_resp
                .files()?
                .into_iter()
                .filter(|file| match file.category {
                    Some(_) => true,
                    None => {
                        info!(
                            name = file.file_name,
                            id = file.file_id,
                            "skipping file with no category"
                        );
                        false
                    }
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

                match nexus_api::metadata::contains_plugin(&client, &api_file).await {
                    Ok(contains_plugin) => {
                        if let Some(contains_plugin) = contains_plugin {
                            if !contains_plugin {
                                info!("file metadata does not contain a plugin, skip downloading");
                                continue;
                            }
                        } else {
                            warn!("file has no metadata link");
                        }
                        Ok(())
                    }
                    Err(err) => {
                        if let Some(reqwest_err) = err.downcast_ref::<reqwest::Error>() {
                            if reqwest_err.status() == Some(StatusCode::NOT_FOUND) {
                                warn!(
                                    status = ?reqwest_err.status(),
                                    "metadata for file not found on server"
                                );
                                Ok(())
                            } else {
                                Err(err)
                            }
                        } else {
                            Err(err)
                        }
                    }
                }?;

                let download_link_resp =
                    nexus_api::download_link::get(&client, db_mod.nexus_mod_id, api_file.file_id)
                        .await;
                if let Err(err) = &download_link_resp {
                    if let Some(reqwest_err) = err.downcast_ref::<reqwest::Error>() {
                        if reqwest_err.status() == Some(StatusCode::NOT_FOUND) {
                            warn!(
                                status = ?reqwest_err.status(),
                                "failed to get download link for file"
                            );
                            file::update_has_download_link(&pool, db_file.id, false).await?;
                            continue;
                        }
                    }
                }
                let download_link_resp = download_link_resp?;
                let mut tokio_file = download_link_resp.download_file(&client).await?;
                info!(bytes = api_file.size, "download finished");

                initialize_plugins_archive(db_mod.nexus_mod_id, db_file.nexus_file_id)?;
                let mut plugins_archive = ZipWriter::new_append(
                    OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open("plugins.zip")?,
                )?;

                let mut initial_bytes = [0; 8];
                tokio_file.seek(SeekFrom::Start(0)).await?;
                tokio_file.read_exact(&mut initial_bytes).await?;
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
                            Archive::new(temp_file_path.to_string_lossy().to_string()).list();
                        if let Ok(list) = list {
                            for entry in list {
                                if let Ok(entry) = entry {
                                    if entry.is_file()
                                        && (entry.filename.ends_with(".esp")
                                            || entry.filename.ends_with(".esm")
                                            || entry.filename.ends_with(".esl"))
                                    {
                                        plugin_file_paths.push(entry.filename);
                                    }
                                }
                            }
                        }
                        info!(
                            num_plugin_files = plugin_file_paths.len(),
                            "listed plugins in downloaded archive"
                        );

                        if plugin_file_paths.len() > 0 {
                            info!("uncompressing downloaded archive");
                            let extract =
                                Archive::new(temp_file_path.to_string_lossy().to_string())
                                    .extract_to(temp_dir.path().to_string_lossy().to_string());
                            extract
                                .expect("failed to extract")
                                .process()
                                .expect("failed to extract");

                            for file_name in plugin_file_paths.iter() {
                                info!(
                                    ?file_name,
                                    "processing uncompressed file from downloaded archive"
                                );
                                let mut plugin_buf =
                                    std::fs::read(temp_dir.path().join(file_name))?;
                                process_plugin(
                                    &mut plugin_buf,
                                    &pool,
                                    &mut plugins_archive,
                                    &db_file,
                                    &db_mod,
                                    file_name,
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

                        for file_name in list_archive_files(&file)? {
                            if file_name.ends_with(".esp")
                                || file_name.ends_with(".esm")
                                || file_name.ends_with(".esl")
                            {
                                plugin_file_paths.push(file_name);
                            }
                        }
                        info!(
                            num_plugin_files = plugin_file_paths.len(),
                            "listed plugins in downloaded archive"
                        );

                        for file_name in plugin_file_paths.iter() {
                            let plugin_span = info_span!("plugin", name = ?file_name);
                            let _plugin_span = plugin_span.enter();
                            file.seek(SeekFrom::Start(0))?;
                            let mut buf = Vec::default();
                            info!("uncompressing plugin file from downloaded archive");
                            match uncompress_archive_file(&mut file, &mut buf, file_name) {
                                Ok(_) => Ok(()),
                                Err(err) => {
                                    if kind.mime_type() == "application/zip" {
                                        // compress_tools or libarchive failed to extract zip file (e.g. archive is deflate64 compressed)
                                        // Attempt to uncompress the archive using `unzip` unix command instead
                                        warn!(error = %err, "failed to extract file with compress_tools, extracting whole archive with unzip instead");
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

                                        for file_name in plugin_file_paths.iter() {
                                            info!(
                                                ?file_name,
                                                "processing uncompressed file from downloaded archive"
                                            );
                                            let mut plugin_buf =
                                                std::fs::read(extracted_path.join(file_name))?;
                                            process_plugin(
                                                &mut plugin_buf,
                                                &pool,
                                                &mut plugins_archive,
                                                &db_file,
                                                &db_mod,
                                                file_name,
                                            )
                                            .await?;
                                        }

                                        break;
                                    }
                                    Err(err)
                                }
                            }?;
                            process_plugin(
                                &mut buf,
                                &pool,
                                &mut plugins_archive,
                                &db_file,
                                &db_mod,
                                file_name,
                            )
                            .await?;
                        }
                    }
                }

                plugins_archive.finish()?;
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
