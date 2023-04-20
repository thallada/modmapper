use anyhow::Result;
use chrono::{NaiveDateTime, NaiveTime};
use humansize::{format_size_i, DECIMAL};
use reqwest::StatusCode;
use std::collections::HashSet;
use std::io::SeekFrom;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::time::sleep;
use tracing::{debug, info, info_span, warn};

use crate::extractors::{self, extract_with_7zip, extract_with_compress_tools, extract_with_unrar};
use crate::models::file;
use crate::models::game;
use crate::models::{game_mod, game_mod::UnsavedMod};
use crate::nexus_api::{self, get_game_id};
use crate::nexus_scraper;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(7200); // 2 hours
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn update(
    pool: &sqlx::Pool<sqlx::Postgres>,
    start_page: usize,
    game_name: &str,
    full: bool,
) -> Result<()> {
    for include_translations in [false, true] {
        let mut page = start_page;
        let mut has_next_page = true;
        let mut pages_with_no_updates = 0;

        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()?;

        let game_id = get_game_id(game_name).expect("valid game name");
        let game = game::insert(pool, game_name, game_id).await?;

        while has_next_page {
            if !full && pages_with_no_updates >= 50 {
                warn!("No updates found for 50 pages in a row, aborting");
                break;
            }

            let page_span = info_span!("page", page, game_name, include_translations);
            let _page_span = page_span.enter();
            let mod_list_resp = nexus_scraper::get_mod_list_page(
                &client,
                page,
                game.nexus_game_id,
                include_translations,
            )
            .await?;
            let scraped = mod_list_resp.scrape_mods()?;

            has_next_page = scraped.has_next_page;
            let processed_mods = game_mod::bulk_get_last_updated_by_nexus_mod_ids(
                pool,
                game.id,
                &scraped
                    .mods
                    .iter()
                    .map(|scraped_mod| scraped_mod.nexus_mod_id)
                    .collect::<Vec<i32>>(),
            )
            .await?;
            let mods_to_create_or_update: Vec<UnsavedMod> = scraped
                .mods
                .iter()
                .filter(|scraped_mod| {
                    if let Some(processed_mod) = processed_mods.iter().find(|processed_mod| {
                        processed_mod.nexus_mod_id == scraped_mod.nexus_mod_id
                    }) {
                        if processed_mod.last_updated_files_at
                            > NaiveDateTime::new(
                                scraped_mod.last_update_at,
                                NaiveTime::from_hms(0, 0, 0),
                            )
                        {
                            return false;
                        }
                    }
                    true
                })
                .map(|scraped_mod| UnsavedMod {
                    name: scraped_mod.name,
                    nexus_mod_id: scraped_mod.nexus_mod_id,
                    author_name: scraped_mod.author_name,
                    author_id: scraped_mod.author_id,
                    category_name: scraped_mod.category_name,
                    category_id: scraped_mod.category_id,
                    description: scraped_mod.desc,
                    thumbnail_link: scraped_mod.thumbnail_link,
                    game_id: game.id,
                    is_translation: include_translations,
                    last_update_at: NaiveDateTime::new(
                        scraped_mod.last_update_at,
                        NaiveTime::from_hms(0, 0, 0),
                    ),
                    first_upload_at: NaiveDateTime::new(
                        scraped_mod.first_upload_at,
                        NaiveTime::from_hms(0, 0, 0),
                    ),
                })
                .collect();

            let mods = game_mod::batched_insert(pool, &mods_to_create_or_update).await?;

            if mods.is_empty() {
                pages_with_no_updates += 1;
            } else {
                pages_with_no_updates = 0;
            }

            for db_mod in mods {
                let mod_span = info_span!("mod", name = ?&db_mod.name, id = &db_mod.nexus_mod_id);
                let _mod_span = mod_span.enter();
                let files_resp =
                    nexus_api::files::get(&client, game_name, db_mod.nexus_mod_id).await?;

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
                    file::get_processed_nexus_file_ids_by_mod_id(pool, db_mod.id)
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
                        pool,
                        &file::UnsavedFile {
                            name: api_file.name,
                            file_name: api_file.file_name,
                            nexus_file_id: api_file.file_id as i32,
                            mod_id: db_mod.id,
                            category: api_file.category,
                            version: api_file.version,
                            mod_version: api_file.mod_version,
                            size: api_file.size,
                            uploaded_at: api_file.uploaded_at,
                        },
                    )
                    .await?;

                    let mut checked_metadata = false;
                    match nexus_api::metadata::contains_plugin(&client, &api_file).await {
                        Ok(contains_plugin) => {
                            if let Some(contains_plugin) = contains_plugin {
                                checked_metadata = true;
                                if !contains_plugin {
                                    info!(
                                        "file metadata does not contain a plugin, skip downloading"
                                    );
                                    file::update_has_plugin(pool, db_file.id, false).await?;
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

                    let humanized_size = format_size_i(api_file.size, DECIMAL);
                    info!(size = %humanized_size, "decided to download file");
                    let download_link_resp = nexus_api::download_link::get(
                        &client,
                        game_name,
                        db_mod.nexus_mod_id,
                        api_file.file_id,
                    )
                    .await;
                    if let Err(err) = &download_link_resp {
                        if let Some(reqwest_err) = err.downcast_ref::<reqwest::Error>() {
                            if reqwest_err.status() == Some(StatusCode::NOT_FOUND) {
                                warn!(
                                    status = ?reqwest_err.status(),
                                    "failed to get download link for file, skipping file"
                                );
                                file::update_has_download_link(pool, db_file.id, false).await?;
                                continue;
                            }
                        }
                    }
                    let download_link_resp = download_link_resp?;

                    let mut tokio_file = match download_link_resp.download_file(&client).await {
                        Ok(file) => {
                            info!(bytes = api_file.size, "download finished");
                            file::update_downloaded_at(pool, db_file.id).await?;
                            file
                        }
                        Err(err) => {
                            warn!(error = %err, "failed all attempts at downloading file, skipping file");
                            continue;
                        }
                    };

                    let mut initial_bytes = [0; 8];
                    tokio_file.seek(SeekFrom::Start(0)).await?;
                    if let Err(err) = tokio_file.read_exact(&mut initial_bytes).await {
                        warn!(error = %err, "failed to read initial bytes, skipping file");
                        file::update_unable_to_extract_plugins(pool, db_file.id, true).await?;
                        continue;
                    }
                    let kind = match infer::get(&initial_bytes) {
                        Some(kind) => kind,
                        None => {
                            warn!(initial_bytes = ?initial_bytes, "unable to determine file type of archive, skipping file");
                            file::update_unable_to_extract_plugins(pool, db_file.id, true).await?;
                            continue;
                        }
                    };
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
                            match extract_with_unrar(
                                &mut file,
                                pool,
                                &db_file,
                                &db_mod,
                                game_name,
                                checked_metadata,
                            )
                            .await
                            {
                                Ok(_) => Ok(()),
                                Err(err) => {
                                    // unrar failed to extract rar file (e.g. archive has unicode filenames)
                                    // Attempt to uncompress the archive using `7z` unix command instead
                                    warn!(error = %err, "failed to extract file with unrar, extracting whole archive with 7z instead");
                                    extract_with_7zip(
                                        &mut file,
                                        pool,
                                        &db_file,
                                        &db_mod,
                                        game_name,
                                        checked_metadata,
                                    )
                                    .await
                                }
                            }?;
                        }
                        _ => {
                            tokio_file.seek(SeekFrom::Start(0)).await?;
                            let mut file = tokio_file.try_clone().await?.into_std().await;

                            match extract_with_compress_tools(
                                &mut file, pool, &db_file, &db_mod, game_name,
                            )
                            .await
                            {
                                Ok(_) => Ok(()),
                                Err(err) => {
                                    if err
                                        .downcast_ref::<extractors::compress_tools::ExtractorError>(
                                        )
                                        .is_some()
                                        && (kind.mime_type() == "application/zip"
                                            || kind.mime_type() == "application/x-7z-compressed")
                                    {
                                        // compress_tools or libarchive failed to extract zip/7z file (e.g. archive is deflate64 compressed)
                                        // Attempt to uncompress the archive using `7z` unix command instead
                                        warn!(error = %err, "failed to extract file with compress_tools, extracting whole archive with 7z instead");
                                        extract_with_7zip(
                                            &mut file,
                                            pool,
                                            &db_file,
                                            &db_mod,
                                            game_name,
                                            checked_metadata,
                                        )
                                        .await
                                    } else if kind.mime_type()
                                        == "application/vnd.microsoft.portable-executable"
                                    {
                                        // we tried to extract this .exe file, but it's not an archive so there's nothing we can do
                                        warn!("archive is an .exe file that cannot be extracted, skipping file");
                                        file::update_unable_to_extract_plugins(
                                            pool, db_file.id, true,
                                        )
                                        .await?;
                                        continue;
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

                game_mod::update_last_updated_files_at(pool, db_mod.id).await?;
            }

            page += 1;
            debug!(?page, ?has_next_page, "sleeping 1 second");
            sleep(Duration::from_secs(1)).await;
        }
    }

    Ok(())
}
