use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use tracing::info;

use crate::models::game;
use crate::models::game_mod;

#[derive(Serialize)]
struct ModForSearchIdTranslated {
    name: String,
    game: String,
    id: i32,
}

pub async fn dump_mod_search_index(pool: &sqlx::Pool<sqlx::Postgres>, path: &str) -> Result<()> {
    let mut page = 1;
    let mut search_index = vec![];
    let page_size = 20;
    let mut last_id = None;
    let game_id_to_names: HashMap<_, _> = game::get_all(&pool).await?.into_iter().map(|game| (game.id, game.name)).collect();
    loop {
        let mods = game_mod::batched_get_for_search(&pool, page_size, last_id).await?;
        if mods.is_empty() {
            break;
        }
        for mod_for_search in mods {
            info!(page = page, nexus_mod_id = mod_for_search.nexus_mod_id, "read mod name for search index");
            search_index.push(ModForSearchIdTranslated {
                name: mod_for_search.name,
                game: game_id_to_names.get(&mod_for_search.game_id).expect("known game id").to_string(),
                id: mod_for_search.nexus_mod_id,
            });
            last_id = Some(mod_for_search.id);
        }
        page += 1;
    }
    info!("writing {} mod names for search index to {}", search_index.len(), path);
    let mut file = File::create(path)?;
    write!(file, "{}", serde_json::to_string(&search_index)?)?;
    return Ok(());
}
