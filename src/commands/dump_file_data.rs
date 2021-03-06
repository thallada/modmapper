use anyhow::Result;
use chrono::NaiveDateTime;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tracing::info;

use crate::models::file;

pub async fn dump_file_data(pool: &sqlx::Pool<sqlx::Postgres>, dir: &str, updated_after: Option<NaiveDateTime>) -> Result<()> {
    let mut page = 1;
    let page_size = 20;
    let mut last_id = None;
    loop {
        let files =
            file::batched_get_with_cells(&pool, page_size, last_id, "Skyrim.esm", 1, updated_after).await?;
        if files.is_empty() {
            break;
        }
        for file_with_cells in files {
            let path = Path::new(&dir);
            std::fs::create_dir_all(&path)?;
            let path = path.join(format!("{}.json", file_with_cells.nexus_file_id));
            info!(page = page, nexus_file_id = file_with_cells.nexus_file_id, "dumping file data to {}", path.display());
            let mut file = File::create(path)?;
            write!(file, "{}", serde_json::to_string(&file_with_cells)?)?;
            last_id = Some(file_with_cells.id);
        }
        page += 1;
    }
    return Ok(());
}
