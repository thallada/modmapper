use anyhow::Result;
use chrono::NaiveDateTime;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

use crate::models::game;
use crate::models::game_mod;

pub async fn dump_mod_data(dir: &str, updated_after: Option<NaiveDateTime>) -> Result<()> {
    let mut pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL")?)
        .await?;
    let mut mod_count = 0;
    let mut page = 1;
    let page_size = 20;
    let mut last_id = None;
    let game_id_to_name: HashMap<_, _> = game::get_all(&pool)
        .await?
        .into_iter()
        .map(|game| (game.id, game.name))
        .collect();
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
        let mods = game_mod::batched_get_with_cells_and_files(
            &pool,
            page_size,
            last_id,
            "Skyrim.esm",
            1,
            updated_after,
        )
        .await?;
        if mods.is_empty() {
            break;
        }
        for mod_with_cells in mods {
            let path = Path::new(&dir).join(
                game_id_to_name
                    .get(&mod_with_cells.game_id)
                    .expect("valid mod.game_id"),
            );
            std::fs::create_dir_all(&path)?;
            let path = path.join(format!("{}.json", mod_with_cells.nexus_mod_id));
            debug!(
                page = page,
                nexus_mod_id = mod_with_cells.nexus_mod_id,
                "dumping mod data to {}",
                path.display()
            );
            let mut file = File::create(path).await?;
            file.write_all(serde_json::to_string(&mod_with_cells)?.as_bytes())
                .await?;
            last_id = Some(mod_with_cells.id);
            mod_count += 1;
        }
        info!("dumped page {}", page);
        page += 1;
    }
    info!("dumped {} mod data files", mod_count);
    Ok(())
}
