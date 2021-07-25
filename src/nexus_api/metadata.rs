use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;
use std::env;
use tokio::time::sleep;
use tracing::{info, instrument, warn};

use super::files::ApiFile;
use super::USER_AGENT;

fn has_plugin(json: &Value) -> Result<bool> {
    let node_type = json
        .get("type")
        .ok_or_else(|| anyhow!("Missing type key in metadata API response"))?
        .as_str()
        .ok_or_else(|| anyhow!("type value in metadata is not a string"))?;

    if node_type == "file" {
        let name = json
            .get("name")
            .ok_or_else(|| anyhow!("Missing name key in metadata API response"))?
            .as_str()
            .ok_or_else(|| anyhow!("name value in metadata is not a string"))?;

        if name.ends_with(".esp") || name.ends_with(".esm") || name.ends_with(".esl") {
            return Ok(true);
        }
    }

    match json.get("children") {
        None => Ok(false),
        Some(children) => {
            let children = children
                .as_array()
                .ok_or_else(|| anyhow!("children value in metadata is not an array"))?;
            for child in children {
                if has_plugin(child)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

#[instrument(skip(client, api_file), fields(metadata_link = api_file.content_preview_link.unwrap_or("null")))]
pub async fn contains_plugin(client: &Client, api_file: &ApiFile<'_>) -> Result<Option<bool>> {
    for attempt in 1..=3 {
        if let Some(metadata_link) = api_file.content_preview_link {
            let res = match client
                .get(metadata_link)
                .header("accept", "application/json")
                .header("apikey", env::var("NEXUS_API_KEY")?)
                .header("user-agent", USER_AGENT)
                .send()
                .await?
                .error_for_status()
            {
                Ok(res) => res,
                Err(err) => {
                    warn!(error = %err, attempt, "Failed to get metadata for file, trying again after 1 second");
                    sleep(std::time::Duration::from_secs(1)).await;
                    continue;
                }
            };

            info!(status = %res.status(), "fetched file metadata from API");
            let json = res.json::<Value>().await?;

            match json.get("children") {
                None => return Ok(Some(false)),
                Some(children) => {
                    let children = children
                        .as_array()
                        .ok_or_else(|| anyhow!("children value in metadata is not an array"))?;
                    for child in children {
                        if has_plugin(child)? {
                            return Ok(Some(true));
                        }
                    }
                    return Ok(Some(false));
                }
            }
        } else {
            return Ok(None);
        }
    }
    Err(anyhow!("Failed to get metadata for file in three attempts"))
}
