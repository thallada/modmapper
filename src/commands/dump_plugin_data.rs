use anyhow::Result;
use chrono::NaiveDateTime;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::fs::create_dir_all;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

use crate::models::{format_radix, plugin};

pub async fn dump_plugin_data(dir: &str, updated_after: Option<NaiveDateTime>) -> Result<()> {
    let mut pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL")?)
        .await?;
    let mut plugin_count = 0;
    let mut page: u32 = 1;
    let page_size = 20;
    let mut last_hash = None;
    loop {
        if page % 5 == 0 {
            // There's a weird issue that slows down this query after 5 iterations. Recreating the
            // connection pool seems to fix it. I don't know why.
            info!("reconnecting to database");
            pool = PgPoolOptions::new()
                .max_connections(5)
                .connect(&env::var("DATABASE_URL")?)
                .await?;
        }
        let plugins = plugin::batched_get_by_hash_with_mods(
            &pool,
            page_size,
            last_hash,
            "Skyrim.esm",
            1,
            updated_after,
        )
        .await?;
        if plugins.is_empty() {
            break;
        }
        for plugin in plugins {
            let path = Path::new(&dir);
            create_dir_all(path)?;
            let path = path.join(format!("{}.json", format_radix(plugin.hash as u64, 36)));
            debug!(
                page = page,
                hash = plugin.hash,
                "dumping plugin data to {}",
                path.display()
            );
            let mut file = File::create(path).await?;
            let json_val = serde_json::to_string(&plugin)?;
            file.write_all(json_val.as_bytes()).await?;
            last_hash = Some(plugin.hash);
            plugin_count += 1;
        }
        info!("dumped page {}", page);
        page += 1;
    }
    info!("dumped {} plugin data files", plugin_count);
    Ok(())
}
