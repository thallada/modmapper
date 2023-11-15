use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::env;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

use crate::models::game_mod;

pub async fn dump_mod_cell_counts(path: &str) -> Result<()> {
    let mut pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL")?)
        .await?;
    let mut page = 1;
    let page_size = 100;
    let mut last_id = None;
    let mut counts = HashMap::new();
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
        let mod_cell_counts =
            game_mod::batched_get_cell_counts(&pool, page_size, last_id, "Skyrim.esm", 1).await?;
        if mod_cell_counts.is_empty() {
            break;
        }
        for mod_cell_count in mod_cell_counts {
            debug!(
                page = page,
                nexus_mod_id = mod_cell_count.nexus_mod_id,
                count = mod_cell_count.cells.unwrap_or(0),
                "read mod cell count"
            );
            counts.insert(mod_cell_count.nexus_mod_id, mod_cell_count.cells);
            last_id = Some(mod_cell_count.nexus_mod_id);
        }
        info!("dumped page {}", page);
        page += 1;
    }
    info!("writing {} mod cell counts to {}", counts.len(), path);
    let mut file = File::create(path).await?;
    file.write_all(serde_json::to_string(&counts)?.as_bytes())
        .await?;
    Ok(())
}
