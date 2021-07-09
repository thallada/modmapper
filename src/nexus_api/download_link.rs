use anyhow::{anyhow, Result};
use futures::TryStreamExt;
use reqwest::Client;
use serde_json::Value;
use std::{env, time::Duration};
use tempfile::tempfile;
use tokio::fs::File;
use tokio_util::compat::FuturesAsyncReadCompatExt;

use super::{rate_limit_wait_duration, GAME_NAME, USER_AGENT};

pub struct DownloadLinkResponse {
    pub wait: Option<Duration>,
    json: Value,
}

pub async fn get(client: &Client, mod_id: i32, file_id: i64) -> Result<DownloadLinkResponse> {
    let res = client
        .get(format!(
            "https://api.nexusmods.com/v1/games/{}/mods/{}/files/{}/download_link.json",
            GAME_NAME, mod_id, file_id
        ))
        .header("accept", "application/json")
        .header("apikey", env::var("NEXUS_API_KEY")?)
        .header("user-agent", USER_AGENT)
        .send()
        .await?
        .error_for_status()?;

    let wait = rate_limit_wait_duration(&res)?;
    let json = res.json::<Value>().await?;

    Ok(DownloadLinkResponse { wait, json })
}

impl DownloadLinkResponse {
    pub fn link<'a>(&'a self) -> Result<&'a str> {
        let link = self
            .json
            .get(0)
            .ok_or_else(|| anyhow!("Links array in API response is missing first element"))?
            .get("URI")
            .ok_or_else(|| anyhow!("Missing URI key in link in API response"))?
            .as_str()
            .ok_or_else(|| anyhow!("URI value in API response link is not a string"))?;
        Ok(link)
    }

    pub async fn download_file(&self, client: &Client) -> Result<File> {
        let mut tokio_file = File::from_std(tempfile()?);
        let res = client
            .get(self.link()?)
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

        return Ok(tokio_file);
    }
}
