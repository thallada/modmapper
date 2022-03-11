use anyhow::Result;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::Path;

use crate::models::plugin;

// From: https://stackoverflow.com/a/50278316/6620612
fn format_radix(mut x: u64, radix: u32) -> String {
    let mut result = vec![];
    loop {
        let m = x % radix as u64;
        x = x / radix as u64;

        // will panic if you use a bad radix (< 2 or > 36).
        result.push(std::char::from_digit(m as u32, radix).unwrap());
        if x == 0 {
            break;
        }
    }
    result.into_iter().rev().collect()
}

pub async fn dump_plugin_data(pool: &sqlx::Pool<sqlx::Postgres>, dir: &str) -> Result<()> {
    let page_size = 20;
    let mut last_id = None;
    loop {
        let plugins =
            plugin::batched_get_with_data(&pool, page_size, last_id, "Skyrim.esm", 1).await?;
        if plugins.is_empty() {
            break;
        }
        for plugin in plugins {
            let path = Path::new(&dir);
            create_dir_all(&path)?;
            let path = path.join(format!("{}.json", format_radix(plugin.hash as u64, 36)));
            let mut file = File::create(path)?;
            write!(file, "{}", serde_json::to_string(&plugin)?)?;
            last_id = Some(plugin.id);
        }
    }
    return Ok(());
}