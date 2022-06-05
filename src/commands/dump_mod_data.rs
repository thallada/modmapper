use anyhow::Result;
use chrono::NaiveDateTime;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tracing::info;

use crate::models::game_mod;

pub async fn dump_mod_data(pool: &sqlx::Pool<sqlx::Postgres>, dir: &str, updated_after: Option<NaiveDateTime>) -> Result<()> {
    let mut page = 1;
    let page_size = 20;
    let mut last_id = None;
    loop {
        let mods =
            game_mod::batched_get_with_cells_and_files(&pool, page_size, last_id, "Skyrim.esm", 1, updated_after).await?;
        if mods.is_empty() {
            break;
        }
        for mod_with_cells in mods {
            let path = Path::new(&dir);
            std::fs::create_dir_all(&path)?;
            let path = path.join(format!("{}.json", mod_with_cells.nexus_mod_id));
            info!(page = page, nexus_mod_id = mod_with_cells.nexus_mod_id, "dumping mod data to {}", path.display());
            let mut file = File::create(path)?;
            write!(file, "{}", serde_json::to_string(&mod_with_cells)?)?;
            last_id = Some(mod_with_cells.id);
        }
        page += 1;
    }
    return Ok(());
}
