use anyhow::{anyhow, Result};
use chrono::NaiveDateTime;
use reqwest::Client;
use serde_json::Value;
use std::{env, time::Duration};
use tracing::{info, instrument};

use super::{rate_limit_wait_duration, warn_and_sleep, USER_AGENT};

pub struct ModResponse {
    pub wait: Duration,
    json: Value,
}

#[instrument(skip(client))]
pub async fn get(client: &Client, game_name: &str, mod_id: i32) -> Result<ModResponse> {
    for attempt in 1..=3 {
        let res = match client
            .get(format!(
                "https://api.nexusmods.com/v1/games/{}/mods/{}.json",
                game_name, mod_id
            ))
            .header("accept", "application/json")
            .header("apikey", env::var("NEXUS_API_KEY")?)
            .header("user-agent", USER_AGENT)
            .send()
            .await
        {
            Ok(res) => match res.error_for_status() {
                Ok(res) => res,
                Err(err) => {
                    warn_and_sleep("game_mod::get", anyhow!(err), attempt).await;
                    continue;
                }
            },
            Err(err) => {
                warn_and_sleep("game_mod::get", anyhow!(err), attempt).await;
                continue;
            }
        };

        info!(status = %res.status(), "fetched mod data from API");
        let wait = rate_limit_wait_duration(&res)?;
        let json = res.json::<Value>().await?;

        return Ok(ModResponse { wait, json });
    }
    Err(anyhow!("Failed to get mod data in three attempts"))
}

pub struct ExtractedModData<'a> {
    pub nexus_mod_id: i32,
    pub name: Option<&'a str>,
    pub category_id: Option<i32>,
    pub author_name: &'a str,
    pub author_id: i32,
    pub description: Option<&'a str>,
    pub thumbnail_link: Option<&'a str>,
    pub last_update_at: NaiveDateTime,
    pub first_upload_at: NaiveDateTime,
}

impl ModResponse {
    #[instrument(skip(self))]
    pub fn extract_data<'a>(&'a self) -> Result<ExtractedModData<'a>> {
        let nexus_mod_id = self
            .json
            .get("mod_id")
            .expect("Missing mod_id in mod response")
            .as_i64()
            .expect("Failed to parse mod_id in mod response") as i32;
        let category_id = self.json.get("category_id").map(|id| {
            id.as_i64()
                .expect("Failed to parse category_id in mod response") as i32
        });
        let name = self
            .json
            .get("name")
            .map(|name| name.as_str().expect("Failed to parse name in mod response"));
        let description = self.json.get("description").map(|description| {
            description
                .as_str()
                .expect("Failed to parse description in mod response")
        });
        let thumbnail_link = self
            .json
            .get("picture_url")
            .and_then(|thumbnail_link| thumbnail_link.as_str());
        let user = self.json.get("user").expect("Missing user in mod response");
        let author_name = user
            .get("name")
            .expect("Missing user name in mod response")
            .as_str()
            .expect("Failed to parse user name in mod response");
        let author_id = user
            .get("member_id")
            .expect("Missing member_id in mod response")
            .as_i64()
            .expect("Failed to parse member_id in mod response") as i32;
        let updated_timestamp = self
            .json
            .get("updated_timestamp")
            .expect("Missing updated_timestamp in mod response")
            .as_i64()
            .expect("Failed to parse updated_timestamp in mod response");
        let last_update_at = NaiveDateTime::from_timestamp(updated_timestamp, 0);
        let created_timestamp = self
            .json
            .get("created_timestamp")
            .expect("Missing created_timestamp in mod response")
            .as_i64()
            .expect("Failed to parse created_timestamp in mod response");
        let first_upload_at = NaiveDateTime::from_timestamp(created_timestamp, 0);
        info!("parsed mod data from API response");
        Ok(ExtractedModData {
            nexus_mod_id,
            name,
            category_id,
            author_name,
            author_id,
            description,
            thumbnail_link,
            last_update_at,
            first_upload_at,
        })
    }
}
