use anyhow::Result;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use tracing::info;

use crate::models::game_mod;

pub async fn dump_mod_cell_counts(pool: &sqlx::Pool<sqlx::Postgres>, path: &str) -> Result<()> {
    let mut page = 1;
    let page_size = 100;
    let mut last_id = None;
    let mut counts = HashMap::new();
    loop {
        let mod_cell_counts =
            game_mod::batched_get_cell_counts(&pool, page_size, last_id, "Skyrim.esm", 1).await?;
        if mod_cell_counts.is_empty() {
            break;
        }
        for mod_cell_count in mod_cell_counts {
            info!(page = page, nexus_mod_id = mod_cell_count.nexus_mod_id, count = mod_cell_count.cells.unwrap_or(0), "read mod cell count");
            counts.insert(mod_cell_count.nexus_mod_id, mod_cell_count.cells);
            last_id = Some(mod_cell_count.nexus_mod_id);
        }
        page += 1;
    }
    info!("writing {} mod cell counts to {}", counts.len(), path);
    let mut file = File::create(path)?;
    write!(file, "{}", serde_json::to_string(&counts)?)?;
    return Ok(());
}
