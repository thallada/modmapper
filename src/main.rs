use anyhow::{anyhow, Context, Result};
use chrono::DateTime;
use chrono::Duration;
use chrono::NaiveDateTime;
use chrono::Utc;
use compress_tools::{list_archive_files, uncompress_archive_file};
use dotenv::dotenv;
use futures::stream::TryStreamExt;
use reqwest::Response;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use skyrim_cell_dump::parse_plugin;
use sqlx::postgres::PgPoolOptions;
use std::convert::TryInto;
use std::env;
use std::fs::OpenOptions;
use std::io::Seek;
use std::io::SeekFrom;
use tempfile::{tempdir, tempfile};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::time::sleep;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use unrar::Archive;
use zip::write::{FileOptions, ZipWriter};

mod models;

use models::cell::insert_cell;
use models::file::{insert_file, File};
use models::game::insert_game;
use models::game_mod::{get_mod_by_nexus_mod_id, insert_mod, Mod};
use models::plugin::insert_plugin;
use models::plugin_cell::insert_plugin_cell;

static USER_AGENT: &str = "mod-mapper/0.1";
static GAME_NAME: &str = "skyrimspecialedition";
const GAME_ID: u32 = 1704;

fn rate_limit_wait_duration(res: &Response) -> Result<Option<std::time::Duration>> {
    let daily_remaining = res
        .headers()
        .get("x-rl-daily-remaining")
        .expect("No daily remaining in response headers");
    let hourly_remaining = res
        .headers()
        .get("x-rl-hourly-remaining")
        .expect("No hourly limit in response headers");
    let hourly_reset = res
        .headers()
        .get("x-rl-hourly-reset")
        .expect("No hourly reset in response headers");
    dbg!(daily_remaining);
    dbg!(hourly_remaining);

    if hourly_remaining == "0" {
        let hourly_reset = hourly_reset.to_str()?.trim();
        let hourly_reset: DateTime<Utc> =
            (DateTime::parse_from_str(hourly_reset, "%Y-%m-%d %H:%M:%S %z")?
                + Duration::seconds(5))
            .into();
        dbg!(hourly_reset);
        let duration = (hourly_reset - Utc::now()).to_std()?;
        dbg!(duration);

        return Ok(Some(duration));
    }

    Ok(None)
}

async fn process_plugin<W>(
    plugin_buf: &mut [u8],
    pool: &sqlx::Pool<sqlx::Postgres>,
    plugin_archive: &mut ZipWriter<W>,
    name: &str,
    db_file: &File,
    mod_obj: &Mod,
    file_id: i64,
    file_name: &str,
) -> Result<()>
where
    W: std::io::Write + std::io::Seek,
{
    let plugin = parse_plugin(&plugin_buf)?;
    let hash = seahash::hash(&plugin_buf);
    let plugin_row = insert_plugin(
        &pool,
        name,
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
            GAME_NAME, mod_obj.nexus_mod_id, file_id, file_name
        ),
        FileOptions::default(),
    )?;

    let mut reader = std::io::Cursor::new(&plugin_buf);
    std::io::copy(&mut reader, plugin_archive)?;
    Ok(())
}

#[tokio::main]
pub async fn main() -> Result<()> {
    dotenv().ok();
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL")?)
        .await?;
    let game = insert_game(&pool, GAME_NAME, GAME_ID as i32).await?;
    let client = reqwest::Client::new();

    let mut page: i32 = 1;
    let mut has_next_page = true;

    while has_next_page {
        let res = client
            .get(format!(
                "https://www.nexusmods.com/Core/Libs/Common/Widgets/ModList?RH_ModList=nav:true,home:false,type:0,user_id:0,game_id:{},advfilt:true,include_adult:true,page_size:80,show_game_filter:false,open:false,page:{},sort_by:OLD_u_downloads",
                GAME_ID,
                page
            ))
            .send()
            .await?
            .error_for_status()?;
        let html = res.text().await?;
        let document = Html::parse_document(&html);
        let mod_select = Selector::parse("li.mod-tile").expect("failed to parse CSS selector");
        let left_select =
            Selector::parse("div.mod-tile-left").expect("failed to parse CSS selector");
        let right_select =
            Selector::parse("div.mod-tile-right").expect("failed to parse CSS selector");
        let name_select = Selector::parse("p.tile-name a").expect("failed to parse CSS selector");
        let category_select =
            Selector::parse("div.category a").expect("failed to parse CSS selector");
        let author_select = Selector::parse("div.author a").expect("failed to parse CSS selector");
        let desc_select = Selector::parse("p.desc").expect("failed to parse CSS selector");
        let next_page_select =
            Selector::parse("div.pagination li.next").expect("failed to parse CSS selector");

        let next_page_elem = document.select(&next_page_select).next();

        has_next_page = next_page_elem.is_some();

        let mut mods = vec![];
        for element in document.select(&mod_select) {
            let left = element
                .select(&left_select)
                .next()
                .expect("Missing left div for mod");
            let right = element
                .select(&right_select)
                .next()
                .expect("Missing right div for mod");
            let nexus_mod_id = left
                .value()
                .attr("data-mod-id")
                .expect("Missing mod id attribute")
                .parse::<i32>()
                .ok()
                .expect("Failed to parse mod id");
            let name_elem = right
                .select(&name_select)
                .next()
                .expect("Missing name link for mod");
            let name = name_elem.text().next().expect("Missing name text for mod");
            let category_elem = right
                .select(&category_select)
                .next()
                .expect("Missing category link for mod");
            let category = category_elem
                .text()
                .next()
                .expect("Missing category text for mod");
            let author_elem = right
                .select(&author_select)
                .next()
                .expect("Missing author link for mod");
            let author = author_elem
                .text()
                .next()
                .expect("Missing author text for mod");
            let desc_elem = right
                .select(&desc_select)
                .next()
                .expect("Missing desc elem for mod");
            let desc = desc_elem.text().next();

            if let None = get_mod_by_nexus_mod_id(&pool, nexus_mod_id).await? {
                mods.push(
                    insert_mod(&pool, name, nexus_mod_id, author, category, desc, game.id).await?,
                );
            }
        }
        dbg!(mods.len());

        for mod_obj in mods {
            dbg!(&mod_obj.name);
            let res = client
                .get(format!(
                    "https://api.nexusmods.com/v1/games/{}/mods/{}/files.json",
                    GAME_NAME, mod_obj.nexus_mod_id
                ))
                .header("accept", "application/json")
                .header("apikey", env::var("NEXUS_API_KEY")?)
                .header("user-agent", USER_AGENT)
                .send()
                .await?
                .error_for_status()?;

            if let Some(duration) = rate_limit_wait_duration(&res)? {
                sleep(duration).await;
            }

            let files = res.json::<Value>().await?;
            let files = files
                .get("files")
                .ok_or_else(|| anyhow!("Missing files key in API response"))?
                .as_array()
                .ok_or_else(|| anyhow!("files value in API response is not an array"))?;
            // TODO: download other files than just MAIN files
            let files = files.into_iter().filter(|file| {
                if let Some(category_name) = file.get("category_name") {
                    category_name.as_str() == Some("MAIN")
                } else {
                    false
                }
            });

            for file in files {
                let file_id = file
                    .get("file_id")
                    .ok_or_else(|| anyhow!("Missing file_id key in file in API response"))?
                    .as_i64()
                    .ok_or_else(|| anyhow!("file_id value in API response file is not a number"))?;
                dbg!(file_id);
                let name = file
                    .get("name")
                    .ok_or_else(|| anyhow!("Missing name key in file in API response"))?
                    .as_str()
                    .ok_or_else(|| anyhow!("name value in API response file is not a string"))?;
                let file_name = file
                    .get("file_name")
                    .ok_or_else(|| anyhow!("Missing file_name key in file in API response"))?
                    .as_str()
                    .ok_or_else(|| {
                        anyhow!("file_name value in API response file is not a string")
                    })?;
                let category = file
                    .get("category_name")
                    .ok_or_else(|| anyhow!("Missing category key in file in API response"))?
                    .as_str();
                let version = file
                    .get("version")
                    .ok_or_else(|| anyhow!("Missing version key in file in API response"))?
                    .as_str();
                let mod_version = file
                    .get("mod_version")
                    .ok_or_else(|| anyhow!("Missing mod_version key in file in API response"))?
                    .as_str();
                let uploaded_timestamp = file
                    .get("uploaded_timestamp")
                    .ok_or_else(|| {
                        anyhow!("Missing uploaded_timestamp key in file in API response")
                    })?
                    .as_i64()
                    .ok_or_else(|| {
                        anyhow!("uploaded_timestamp value in API response file is not a number")
                    })?;
                let uploaded_at = NaiveDateTime::from_timestamp(uploaded_timestamp, 0);
                let db_file = insert_file(
                    &pool,
                    name,
                    file_name,
                    file_id as i32,
                    mod_obj.id,
                    category,
                    version,
                    mod_version,
                    uploaded_at,
                )
                .await?;
                let res = client
                    .get(format!(
                        "https://api.nexusmods.com/v1/games/{}/mods/{}/files/{}/download_link.json",
                        GAME_NAME, mod_obj.nexus_mod_id, file_id
                    ))
                    .header("accept", "application/json")
                    .header("apikey", env::var("NEXUS_API_KEY")?)
                    .header("user-agent", USER_AGENT)
                    .send()
                    .await?
                    .error_for_status()?;

                let duration = rate_limit_wait_duration(&res)?;

                let links = res.json::<Value>().await?;
                let link = links
                    .get(0)
                    .ok_or_else(|| anyhow!("Links array in API response is missing first element"))?
                    .get("URI")
                    .ok_or_else(|| anyhow!("Missing URI key in link in API response"))?
                    .as_str()
                    .ok_or_else(|| anyhow!("URI value in API response link is not a string"))?;

                let mut tokio_file = tokio::fs::File::from_std(tempfile()?);
                let res = client
                    .get(link)
                    .header("apikey", env::var("NEXUS_API_KEY")?)
                    .header("user-agent", USER_AGENT)
                    .send()
                    .await?
                    .error_for_status()?;

                // See: https://github.com/benkay86/async-applied/blob/master/reqwest-tokio-compat/src/main.rs
                let mut byte_stream = res
                    .bytes_stream()
                    .map_err(|e| futures::io::Error::new(futures::io::ErrorKind::Other, e))
                    .into_async_read()
                    .compat();

                tokio::io::copy(&mut byte_stream, &mut tokio_file).await?;

                let mut plugin_archive = ZipWriter::new(
                    OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open("plugins.zip")?,
                );
                plugin_archive.add_directory(
                    format!("{}/{}/{}", GAME_NAME, mod_obj.nexus_mod_id, file_id),
                    FileOptions::default(),
                )?;
                plugin_archive.finish()?;

                let mut plugin_archive = ZipWriter::new_append(
                    OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open("plugins.zip")?,
                )?;
                let mut initial_bytes = [0; 8];
                tokio_file.seek(SeekFrom::Start(0)).await?;
                tokio_file.read_exact(&mut initial_bytes).await?;
                let kind = infer::get(&initial_bytes).expect("unknown file type of file download");
                dbg!(kind.mime_type());
                // "application/zip" => {
                //     let mut archive = ZipArchive::new(reader)?;
                //     let mut plugin_file_paths = Vec::new();
                //     for file_name in archive.file_names() {
                //         dbg!(file_name);
                //         if file_name.ends_with(".esp")
                //             || file_name.ends_with(".esm")
                //             || file_name.ends_with(".esl")
                //         {
                //             plugin_file_paths.push(file_name.to_string());
                //         }
                //     }
                //     dbg!(&plugin_file_paths);
                //     for file_name in plugin_file_paths.iter() {
                //         let mut file = archive.by_name(file_name)?;
                //         let plugin = parse_plugin(file)?;
                //         dbg!(plugin);
                //         plugin_archive.start_file(
                //             format!("{}/{}/{}/{}", GAME_NAME, mod_id, file_id, file_name),
                //             FileOptions::default(),
                //         )?;
                //         std::io::copy(&mut file, &mut plugin_archive)?;
                //     }
                // }

                // Use unrar to uncompress the entire .rar file to avoid a bug with compress_tools panicking when uncompressing
                // certain .rar files: https://github.com/libarchive/libarchive/issues/373
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

                for file_name in plugin_file_paths.iter() {
                    file.seek(SeekFrom::Start(0))?;
                    dbg!(file_name);
                    let mut buf = Vec::default();
                    match uncompress_archive_file(&mut file, &mut buf, file_name) {
                        Ok(_) => {
                            process_plugin(
                                &mut buf,
                                &pool,
                                &mut plugin_archive,
                                name,
                                &db_file,
                                &mod_obj,
                                file_id,
                                file_name,
                            )
                            .await?;
                        }
                        Err(error) => {
                            dbg!(error);
                            if kind.mime_type() == "application/x-rar-compressed"
                                || kind.mime_type() == "application/vnd.rar"
                            {
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
                                        dbg!(file_name);
                                        let mut plugin_buf =
                                            std::fs::read(temp_dir.path().join(file_name))?;
                                        process_plugin(
                                            &mut plugin_buf,
                                            &pool,
                                            &mut plugin_archive,
                                            name,
                                            &db_file,
                                            &mod_obj,
                                            file_id,
                                            file_name,
                                        )
                                        .await?;
                                    }
                                }
                                temp_dir.close()?;
                            }
                        }
                    }
                }

                plugin_archive.finish()?;
                if let Some(duration) = duration {
                    sleep(duration).await;
                }
            }
        }

        page += 1;
        dbg!(page);
        dbg!(has_next_page);
    }

    Ok(())
}
