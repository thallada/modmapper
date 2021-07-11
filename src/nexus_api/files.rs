use anyhow::{anyhow, Result};
use chrono::NaiveDateTime;
use reqwest::Client;
use serde_json::Value;
use std::{env, time::Duration};
use tracing::{info, instrument};

use super::{rate_limit_wait_duration, GAME_NAME, USER_AGENT};

pub struct FilesResponse {
    pub wait: Option<Duration>,
    json: Value,
}

pub struct ApiFile<'a> {
    pub file_id: i64,
    pub name: &'a str,
    pub file_name: &'a str,
    pub category: Option<&'a str>,
    pub version: Option<&'a str>,
    pub mod_version: Option<&'a str>,
    pub uploaded_at: NaiveDateTime,
}

#[instrument(skip(client))]
pub async fn get(client: &Client, nexus_mod_id: i32) -> Result<FilesResponse> {
    let res = client
        .get(format!(
            "https://api.nexusmods.com/v1/games/{}/mods/{}/files.json",
            GAME_NAME, nexus_mod_id
        ))
        .header("accept", "application/json")
        .header("apikey", env::var("NEXUS_API_KEY")?)
        .header("user-agent", USER_AGENT)
        .send()
        .await?
        .error_for_status()?;

    info!(status = %res.status(), "fetched files for mod from API");
    let wait = rate_limit_wait_duration(&res)?;
    let json = res.json::<Value>().await?;

    Ok(FilesResponse { wait, json })
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
            .into_iter()
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
                    uploaded_at,
                })
            })
            .collect::<Result<Vec<ApiFile>>>()?;
        info!(num_files = files.len(), "parsed files out of API response");
        Ok(files)
    }
}
