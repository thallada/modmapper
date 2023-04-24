use anyhow::Result;
use chrono::NaiveDateTime;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tracing::{debug, info};

use crate::models::game;
use crate::models::game_mod;

pub async fn dump_mod_data(
    pool: &sqlx::Pool<sqlx::Postgres>,
    dir: &str,
    updated_after: Option<NaiveDateTime>,
) -> Result<()> {
    let mut mod_count = 0;
    let mut page = 1;
    let page_size = 20;
    let mut last_id = None;
    let game_id_to_name: HashMap<_, _> = game::get_all(pool)
        .await?
        .into_iter()
        .map(|game| (game.id, game.name))
        .collect();
    loop {
        let mods = game_mod::batched_get_with_cells_and_files(
            pool,
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
            let mut file = File::create(path)?;
            write!(file, "{}", serde_json::to_string(&mod_with_cells)?)?;
            last_id = Some(mod_with_cells.id);
            mod_count += 1;
        }
        page += 1;
    }
    info!("dumped {} mod data files", mod_count);
    Ok(())
}
