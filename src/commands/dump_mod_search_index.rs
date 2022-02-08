use anyhow::Result;
use serde::Serialize;
use std::fs::File;
use std::io::Write;

use crate::models::game_mod;

#[derive(Serialize)]
struct ModForSearchIdTranslated {
    name: String,
    id: i32,
}

pub async fn dump_mod_search_index(pool: &sqlx::Pool<sqlx::Postgres>, path: &str) -> Result<()> {
    let mut search_index = vec![];
    let page_size = 20;
    let mut last_id = None;
    loop {
        let mods = game_mod::batched_get_for_search(&pool, page_size, last_id).await?;
        if mods.is_empty() {
            break;
        }
        for mod_for_search in mods {
            search_index.push(ModForSearchIdTranslated {
                name: mod_for_search.name,
                id: mod_for_search.nexus_mod_id,
            });
            last_id = Some(mod_for_search.id);
        }
    }
    let mut file = File::create(path)?;
    write!(file, "{}", serde_json::to_string(&search_index)?)?;
    return Ok(());
}
