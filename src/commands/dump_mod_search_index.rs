use anyhow::Result;
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use std::env;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

use crate::models::game;
use crate::models::game_mod;

#[derive(Serialize)]
struct ModForSearchIdTranslated {
    name: String,
    id: i32,
}

pub async fn dump_mod_search_index(game: &str, path: &str) -> Result<()> {
    let mut pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL")?)
        .await?;
    let mut page = 1;
    let mut search_index = vec![];
    let page_size = 20;
    let mut last_id = None;
    let game_id = game::get_id_by_name(&pool, game).await?;
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
        let mods = game_mod::batched_get_for_search(&pool, game_id, page_size, last_id).await?;
        if mods.is_empty() {
            break;
        }
        for mod_for_search in mods {
            debug!(
                page = page,
                nexus_mod_id = mod_for_search.nexus_mod_id,
                "read mod name for search index"
            );
            search_index.push(ModForSearchIdTranslated {
                name: mod_for_search.name,
                id: mod_for_search.nexus_mod_id,
            });
            last_id = Some(mod_for_search.id);
        }
        page += 1;
    }
    info!(
        "writing {} mod names for search index to {}",
        search_index.len(),
        path
    );
    let mut file = File::create(path).await?;
    file.write_all(serde_json::to_string(&search_index)?.as_bytes())
        .await?;
    Ok(())
}
