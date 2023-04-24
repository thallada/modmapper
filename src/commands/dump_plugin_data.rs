use anyhow::Result;
use chrono::NaiveDateTime;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::Path;
use tracing::{debug, info};

use crate::models::{format_radix, plugin};

pub async fn dump_plugin_data(
    pool: &sqlx::Pool<sqlx::Postgres>,
    dir: &str,
    updated_after: Option<NaiveDateTime>,
) -> Result<()> {
    let mut plugin_count = 0;
    let mut page: u32 = 1;
    let page_size = 20;
    let mut last_hash = None;
    loop {
        let plugins = plugin::batched_get_by_hash_with_mods(
            pool,
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
            let mut file = File::create(path)?;
            let json_val = serde_json::to_string(&plugin)?;
            write!(file, "{}", json_val)?;
            last_hash = Some(plugin.hash);
            plugin_count += 1;
        }
        page += 1;
    }
    info!("dumped {} plugin data files", plugin_count);
    Ok(())
}
