use anyhow::Result;
use chrono::NaiveDateTime;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::Path;
use tracing::info;

use crate::models::plugin;

// From: https://stackoverflow.com/a/50278316/6620612
fn format_radix(mut x: u64, radix: u32) -> String {
    let mut result = vec![];
    loop {
        let m = x % radix as u64;
        x /= radix as u64;

        // will panic if you use a bad radix (< 2 or > 36).
        result.push(std::char::from_digit(m as u32, radix).unwrap());
        if x == 0 {
            break;
        }
    }
    result.into_iter().rev().collect()
}

pub async fn dump_plugin_data(pool: &sqlx::Pool<sqlx::Postgres>, dir: &str, updated_after: Option<NaiveDateTime>) -> Result<()> {
    let mut page: u32 = 1;
    let page_size = 20;
    let mut last_hash = None;
    loop {
        let plugins =
            plugin::batched_get_by_hash_with_mods(pool, page_size, last_hash, "Skyrim.esm", 1, updated_after).await?;
        if plugins.is_empty() {
            break;
        }
        for plugin in plugins {
            let path = Path::new(&dir);
            create_dir_all(&path)?;
            let path = path.join(format!("{}.json", format_radix(plugin.hash as u64, 36)));
            info!(page = page, hash = plugin.hash, "dumping plugin data to {}", path.display());
            let mut file = File::create(path)?;
            write!(file, "{}", serde_json::to_string(&plugin)?)?;
            last_hash = Some(plugin.hash);
        }
        page += 1;
    }
    Ok(())
}
