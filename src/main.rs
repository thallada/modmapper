use anyhow::Result;
use argh::FromArgs;
use dotenv::dotenv;
use humansize::{FileSize, file_size_opts};
use models::file::File;
use models::game_mod::Mod;
use reqwest::StatusCode;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashSet;
use std::env;
use std::io::Seek;
use std::io::SeekFrom;
use std::process::Command;
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::time::sleep;
use tracing::{debug, error, info, info_span, warn};
use unrar::Archive;
use walkdir::WalkDir;

mod extractors;
mod models;
mod nexus_api;
mod nexus_scraper;
mod plugin_processor;

use models::file;
use models::game;
use models::{game_mod, game_mod::UnsavedMod};
use nexus_api::{GAME_ID, GAME_NAME};
use plugin_processor::process_plugin;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(7200); // 2 hours
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(FromArgs)]
/// Downloads every mod off nexus mods, parses CELL and WRLD data from plugins in each, and saves the data to the database.
struct Args {
    #[argh(option, short = 'p', default = "1")]
    /// the page number to start scraping for mods on nexus mods.
    page: usize,
}

async fn extract_with_compress_tools(
    file: &mut std::fs::File,
    pool: &sqlx::Pool<sqlx::Postgres>,
    db_file: &File,
    db_mod: &Mod,
) -> Result<()> {
    let extractor = extractors::compress_tools::Extractor::new(file);
    for plugin in extractor.into_iter() {
        let (file_path, mut plugin_buf) = plugin?;
        let plugin_span = info_span!("plugin", name = ?file_path);
        let _plugin_span = plugin_span.enter();
        process_plugin(&mut plugin_buf, &pool, &db_file, &db_mod, &file_path).await?;
    }
    Ok(())
}

async fn extract_with_7zip(
    file: &mut std::fs::File,
    pool: &sqlx::Pool<sqlx::Postgres>,
    db_file: &File,
    db_mod: &Mod,
) -> Result<()> {
    file.seek(SeekFrom::Start(0))?;
    let temp_dir = tempdir()?;
    let temp_file_path = temp_dir.path().join("download.zip");
    let mut temp_file = std::fs::File::create(&temp_file_path)?;
    std::io::copy(file, &mut temp_file)?;
    drop(temp_file); // close handle to temp file so 7zip process can open it
    let extracted_path = temp_dir.path().join("extracted");

    Command::new("7z")
        .args(&[
            "x",
            &format!("-o{}", &extracted_path.to_string_lossy()),
            &temp_file_path.to_string_lossy().to_string(),
        ])
        .status()?;

    for entry in WalkDir::new(&extracted_path)
        .contents_first(true)
        .into_iter()
        .filter_entry(|e| {
            if let Some(extension) = e.path().extension() {
                extension == "esp" || extension == "esm" || extension == "esl"
            } else {
                false
            }
        })
    {
        let entry = entry?;
        let file_path = entry.path();
        let plugin_span = info_span!("plugin", name = ?file_path);
        let _plugin_span = plugin_span.enter();
        info!("processing uncompressed file from downloaded archive");
        let mut plugin_buf = std::fs::read(extracted_path.join(file_path))?;
        process_plugin(
            &mut plugin_buf,
            &pool,
            &db_file,
            &db_mod,
            &file_path.to_string_lossy(),
        )
        .await?;
    }
    Ok(())
}

async fn extract_with_unrar(
    file: &mut std::fs::File,
    pool: &sqlx::Pool<sqlx::Postgres>,
    db_file: &File,
    db_mod: &Mod,
    checked_metadata: bool,
) -> Result<()> {
    let temp_dir = tempdir()?;
    let temp_file_path = temp_dir.path().join("download.rar");
    let mut temp_file = std::fs::File::create(&temp_file_path)?;
    std::io::copy(file, &mut temp_file)?;

    let mut plugin_file_paths = Vec::new();
    let list = Archive::new(&temp_file_path.to_string_lossy().to_string())?.list();
    if let Ok(list) = list {
        for entry in list {
            if let Ok(entry) = entry {
                if let Some(extension) = entry.filename.extension() {
                    if entry.is_file()
                        && (extension == "esp" || extension == "esm" || extension == "esl")
                    {
                        plugin_file_paths.push(entry.filename);
                    }
                }
            }
        }
    } else {
        if !checked_metadata {
            warn!("failed to read archive and server has no metadata, skipping file");
            file::update_unable_to_extract_plugins(&pool, db_file.id, true).await?;
            return Ok(());
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
        let extract = Archive::new(&temp_file_path.to_string_lossy().to_string())?
            .extract_to(temp_dir.path().to_string_lossy().to_string());

        let mut extract = match extract {
            Err(err) => {
                warn!(error = %err, "failed to extract with unrar");
                file::update_unable_to_extract_plugins(&pool, db_file.id, true).await?;
                return Ok(())
            }
            Ok(extract) => extract
        };
        match extract.process() {
            Err(err) => {
                warn!(error = %err, "failed to extract with unrar");
                file::update_unable_to_extract_plugins(&pool, db_file.id, true).await?;
                return Ok(())
            }
            _ => {}
        }

        for file_path in plugin_file_paths.iter() {
            info!(
                ?file_path,
                "processing uncompressed file from downloaded archive"
            );
            let mut plugin_buf = std::fs::read(temp_dir.path().join(file_path))?;
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
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()?;

    let args: Args = argh::from_env();
    let mut page = args.page;
    let mut has_next_page = true;

    while has_next_page {
        let page_span = info_span!("page", page);
        let _page_span = page_span.enter();
        let mod_list_resp = nexus_scraper::get_mod_list_page(&client, page).await?;
        let scraped = mod_list_resp.scrape_mods()?;

        has_next_page = scraped.has_next_page;
        let processed_mods = game_mod::bulk_get_fully_processed_nexus_mod_ids(
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
            .filter(|scraped_mod| !processed_mods.contains(&scraped_mod.nexus_mod_id))
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

            let processed_file_ids: HashSet<i32> =
                file::get_processed_nexus_file_ids_by_mod_id(&pool, db_mod.id)
                    .await?
                    .into_iter()
                    .collect();

            for api_file in files {
                let file_span =
                    info_span!("file", name = &api_file.file_name, id = &api_file.file_id,);
                let _file_span = file_span.enter();

                if processed_file_ids.contains(&(api_file.file_id as i32)) {
                    info!("skipping file already present and processed in database");
                    continue;
                }
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
                                file::update_has_plugin(&pool, db_file.id, false).await?;
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

                let humanized_size = api_file.size.file_size(file_size_opts::CONVENTIONAL)
                    .expect("unable to create human-readable file size");
                info!(size = %humanized_size, "decided to download file");
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
                        file::update_unable_to_extract_plugins(&pool, db_file.id, true).await?;
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
                        extract_with_unrar(&mut file, &pool, &db_file, &db_mod, checked_metadata)
                            .await?;
                    }
                    _ => {
                        tokio_file.seek(SeekFrom::Start(0)).await?;
                        let mut file = tokio_file.try_clone().await?.into_std().await;

                        match extract_with_compress_tools(&mut file, &pool, &db_file, &db_mod).await
                        {
                            Ok(_) => Ok(()),
                            Err(err) => {
                                if err
                                    .downcast_ref::<extractors::compress_tools::ExtractorError>()
                                    .is_some()
                                    && kind.mime_type() == "application/zip"
                                {
                                    // compress_tools or libarchive failed to extract zip file (e.g. archive is deflate64 compressed)
                                    // Attempt to uncompress the archive using `7z` unix command instead
                                    warn!(error = %err, "failed to extract file with compress_tools, extracting whole archive with 7z instead");
                                    extract_with_7zip(&mut file, &pool, &db_file, &db_mod).await
                                } else {
                                    Err(err)
                                }
                            }
                        }?;
                    }
                }

                debug!(duration = ?download_link_resp.wait, "sleeping");
                sleep(download_link_resp.wait).await;
            }

            game_mod::update_last_updated_files_at(&pool, db_mod.id).await?;
        }

        page += 1;
        debug!(?page, ?has_next_page, "sleeping 1 second");
        sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}
