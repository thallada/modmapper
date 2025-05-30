use anyhow::{Context, Result};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, info_span};

use crate::nexus_api::SSE_GAME_NAME;
use crate::nexus_scraper;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(7200); // 2 hours
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

struct UpdatedMods {
    id: i32,
}

pub async fn backfill_is_translation(pool: &sqlx::Pool<sqlx::Postgres>) -> Result<()> {
    let mut page = 0;
    let mut has_next_page = true;

    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()?;
    let scraper_client = nexus_scraper::NexusScraper::new(client.clone());

    while has_next_page {
        let page_span = info_span!("page", page);
        let _page_span = page_span.enter();
        let mods_response = scraper_client
            .get_mods(&SSE_GAME_NAME, page * nexus_scraper::PAGE_SIZE, true)
            .await?;
        let scraped_mods = nexus_scraper::convert_mods_to_scraped(&mods_response.mods.nodes)?;
        let scraped_ids: Vec<i32> = scraped_mods.iter().map(|m| m.nexus_mod_id).collect();

        has_next_page = scraped_mods.len() == 20;

        let updated_ids: Vec<i32> = sqlx::query_as!(
            UpdatedMods,
            "UPDATE mods
                SET is_translation = true
                WHERE nexus_mod_id = ANY($1::int[])
                RETURNING id",
            &scraped_ids,
        )
        .fetch_all(pool)
        .await
        .context("Failed to update mod is_translation values")?
        .iter()
        .map(|u| u.id)
        .collect();
        info!(?updated_ids, "updated mods is_translation values");

        page += 1;
        debug!(?page, ?has_next_page, "sleeping 1 second");
        sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}
