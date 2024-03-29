use anyhow::{anyhow, Result};
use chrono::NaiveDateTime;
use reqwest::Client;
use serde_json::Value;
use std::{env, time::Duration};
use tracing::{info, instrument};

use super::{rate_limit_wait_duration, warn_and_sleep};

pub struct FilesResponse {
    pub wait: Duration,
    json: Value,
}

pub struct ApiFile<'a> {
    pub file_id: i64,
    pub name: &'a str,
    pub file_name: &'a str,
    pub category: Option<&'a str>,
    pub version: Option<&'a str>,
    pub mod_version: Option<&'a str>,
    pub size: i64,
    pub content_preview_link: Option<&'a str>,
    pub uploaded_at: NaiveDateTime,
}

#[instrument(skip(client))]
pub async fn get(client: &Client, game_name: &str, nexus_mod_id: i32) -> Result<FilesResponse> {
    for attempt in 1..=3 {
        let res = match client
            .get(format!(
                "https://api.nexusmods.com/v1/games/{}/mods/{}/files.json",
                game_name, nexus_mod_id
            ))
            .header("accept", "application/json")
            .header("apikey", env::var("NEXUS_API_KEY")?)
            .send()
            .await
        {
            Ok(res) => match res.error_for_status() {
                Ok(res) => res,
                Err(err) => {
                    warn_and_sleep("files::get", anyhow!(err), attempt).await;
                    continue;
                }
            },
            Err(err) => {
                warn_and_sleep("files::get", anyhow!(err), attempt).await;
                continue;
            }
        };

        info!(status = %res.status(), "fetched files for mod from API");
        let wait = rate_limit_wait_duration(&res)?;
        let json = res.json::<Value>().await?;

        return Ok(FilesResponse { wait, json });
    }
    Err(anyhow!("Failed to get files for mod in three attempts"))
}

impl FilesResponse {
    #[instrument(skip(self))]
    pub fn files<'a>(&'a self) -> Result<Vec<ApiFile<'a>>> {
        let files = self
            .json
            .get("files")
            .ok_or_else(|| anyhow!("Missing files key in API response"))?
            .as_array()
            .ok_or_else(|| anyhow!("files value in API response is not an array"))?;
        let files: Vec<ApiFile> = files
            .iter()
            .map(|file| {
                let file_id = file
                    .get("file_id")
                    .ok_or_else(|| anyhow!("Missing file_id key in file in API response"))?
                    .as_i64()
                    .ok_or_else(|| anyhow!("file_id value in API response file is not a number"))?;
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
                let size = file
                    .get("size_in_bytes")
                    .ok_or_else(|| anyhow!("Missing size_in_bytes key in file in API response"))?
                    .as_i64();
                let content_preview_link = file
                    .get("content_preview_link")
                    .ok_or_else(|| anyhow!("Missing content_preview_link key in file in API response"))?
                    .as_str();
                let size = if let Some(size) = size {
                    size
                } else {
                    file
                        .get("size_kb")
                        .ok_or_else(|| anyhow!("Missing size_kb key in file in API response"))?
                        .as_i64()
                        .ok_or_else(|| {
                            anyhow!("size_in_bytes and size_kb values in API response file are not numbers")
                        })? * 1000
                };

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

                Ok(ApiFile {
                    file_id,
                    name,
                    file_name,
                    category,
                    version,
                    mod_version,
                    size,
                    content_preview_link,
                    uploaded_at,
                })
            })
            .collect::<Result<Vec<ApiFile>>>()?;
        info!(num_files = files.len(), "parsed files out of API response");
        Ok(files)
    }
}
