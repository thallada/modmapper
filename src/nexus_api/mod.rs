use anyhow::Result;
use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use reqwest::Response;
use tracing::info;

pub mod download_link;
pub mod files;

pub static GAME_NAME: &str = "skyrimspecialedition";
pub const GAME_ID: u32 = 1704;
pub static USER_AGENT: &str = "mod-mapper/0.1";

pub fn rate_limit_wait_duration(res: &Response) -> Result<Option<std::time::Duration>> {
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
    info!(daily_remaining = ?daily_remaining, hourly_remaining = ?hourly_remaining, "rate limit check");

    if hourly_remaining == "0" {
        let hourly_reset = hourly_reset.to_str()?.trim();
        let hourly_reset: DateTime<Utc> =
            (DateTime::parse_from_str(hourly_reset, "%Y-%m-%d %H:%M:%S %z")?
                + Duration::seconds(5))
            .into();
        let duration = (hourly_reset - Utc::now()).to_std()?;
        info!(
            hourly_reset = ?hourly_reset,
            duration = ?duration, "need to wait until rate-limit hourly reset"
        );

        return Ok(Some(duration));
    }

    Ok(None)
}
