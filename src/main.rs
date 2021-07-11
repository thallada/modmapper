use anyhow::Result;
use compress_tools::{list_archive_files, uncompress_archive_file};
use dotenv::dotenv;
use skyrim_cell_dump::parse_plugin;
use sqlx::postgres::PgPoolOptions;
use std::convert::TryInto;
use std::env;
use std::fs::OpenOptions;
use std::io::Seek;
use std::io::SeekFrom;
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::time::sleep;
use tracing::{debug, error, info, instrument, warn};
use unrar::Archive;
use zip::write::{FileOptions, ZipWriter};

mod models;
mod nexus_api;
mod nexus_scraper;

use models::cell::insert_cell;
use models::file::{insert_file, File};
use models::game::insert_game;
use models::game_mod::{get_mod_by_nexus_mod_id, insert_mod, Mod};
use models::plugin::insert_plugin;
use models::plugin_cell::insert_plugin_cell;
use nexus_api::{GAME_ID, GAME_NAME};

#[instrument(level = "debug", skip(plugin_buf, pool, plugin_archive, db_file, mod_obj), fields(name = ?mod_obj.name, id = mod_obj.nexus_mod_id))]
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
    let plugin = parse_plugin(&plugin_buf)?;
    info!(file_name, num_cells = plugin.cells.len(), "parsed plugin");
    let hash = seahash::hash(&plugin_buf);
    let plugin_row = insert_plugin(
        &pool,
        &db_file.name,
        hash as i64,
        db_file.id,
        Some(plugin.header.version as f64),
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
        let cell_row = insert_cell(
            &pool,
            cell.form_id.try_into().unwrap(),
            cell.x,
            cell.y,
            cell.is_persistent,
        )
        .await?;
        insert_plugin_cell(&pool, plugin_row.id, cell_row.id, cell.editor_id).await?;
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
    let game = insert_game(&pool, GAME_NAME, GAME_ID as i32).await?;
    let client = reqwest::Client::new();

    let mut page: i32 = 1;
    let mut has_next_page = true;

    while has_next_page {
        let mod_list_resp = nexus_scraper::get_mod_list_page(&client, page).await?;
        let scraped = mod_list_resp.scrape_mods()?;

        has_next_page = scraped.has_next_page;
        let mut mods = Vec::new();
        for scraped_mod in scraped.mods {
            if let None = get_mod_by_nexus_mod_id(&pool, scraped_mod.nexus_mod_id).await? {
                mods.push(
                    insert_mod(
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
            info!(
                mod_name = ?&db_mod.name,
                mod_id = ?&db_mod.nexus_mod_id,
                "fetching files for mod"
            );
            let files_resp = nexus_api::files::get(&client, db_mod.nexus_mod_id).await?;
            // TODO: download other files than just MAIN files
            // let files = files.into_iter().filter(|file| {
            //     if let Some(category_name) = file.get("category_name") {
            //         category_name.as_str() == Some("MAIN")
            //     } else {
            //         false
            //     }
            // });
            if let Some(duration) = files_resp.wait {
                debug!(?duration, "sleeping");
                sleep(duration).await;
            }

            for api_file in files_resp.files()? {
                let db_file = insert_file(
                    &pool,
                    api_file.name,
                    api_file.file_name,
                    api_file.file_id as i32,
                    db_mod.id,
                    api_file.category,
                    api_file.version,
                    api_file.mod_version,
                    api_file.uploaded_at,
                )
                .await?;

                // TODO: check the file metadata to see if there are any plugin files in the archive before bothering to download the file (checking metadata does not count against rate-limit)

                let download_link_resp =
                    nexus_api::download_link::get(&client, db_mod.nexus_mod_id, api_file.file_id)
                        .await?;
                let mut tokio_file = download_link_resp.download_file(&client).await?;

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
                    file.seek(SeekFrom::Start(0))?;
                    info!(
                        ?file_name,
                        "attempting to uncompress file from downloaded archive"
                    );
                    let mut buf = Vec::default();
                    match uncompress_archive_file(&mut file, &mut buf, file_name) {
                        Ok(_) => {
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
                        Err(error) => {
                            warn!(
                                ?error,
                                "error occurred while attempting to uncompress archive file"
                            );
                            if kind.mime_type() == "application/x-rar-compressed"
                                || kind.mime_type() == "application/vnd.rar"
                            {
                                info!("downloaded archive is RAR archive, attempt to uncompress entire archive instead");
                                // Use unrar to uncompress the entire .rar file to avoid a bug with compress_tools panicking when uncompressing
                                // certain .rar files: https://github.com/libarchive/libarchive/issues/373
                                tokio_file.seek(SeekFrom::Start(0)).await?;
                                let mut file = tokio_file.try_clone().await?.into_std().await;
                                let temp_dir = tempdir()?;
                                let temp_file_path = temp_dir.path().join("download.rar");
                                let mut temp_file = std::fs::File::create(&temp_file_path)?;
                                std::io::copy(&mut file, &mut temp_file)?;

                                let mut plugin_file_paths = Vec::new();
                                let list =
                                    Archive::new(temp_file_path.to_string_lossy().to_string())
                                        .list();
                                if let Ok(list) = list {
                                    for entry in list {
                                        if let Ok(entry) = entry {
                                            if entry.filename.ends_with(".esp")
                                                || entry.filename.ends_with(".esm")
                                                || entry.filename.ends_with(".esl")
                                            {
                                                plugin_file_paths.push(entry.filename);
                                            }
                                        }
                                    }
                                }

                                if plugin_file_paths.len() > 0 {
                                    let extract =
                                        Archive::new(temp_file_path.to_string_lossy().to_string())
                                            .extract_to(
                                                temp_dir.path().to_string_lossy().to_string(),
                                            );
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
                            error!(mime_type = ?kind.mime_type(), "downloaded archive is not RAR archive, skipping processing of this file");
                        }
                    }
                }

                plugins_archive.finish()?;
                if let Some(duration) = download_link_resp.wait {
                    debug!(?duration, "sleeping");
                    sleep(duration).await;
                }
            }
        }

        page += 1;
        debug!(?page, ?has_next_page, "sleeping 1 second");
        sleep(Duration::new(1, 0)).await;
    }

    Ok(())
}
