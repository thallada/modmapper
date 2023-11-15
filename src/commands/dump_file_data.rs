use anyhow::Result;
use chrono::NaiveDateTime;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

use crate::models::file;

pub async fn dump_file_data(dir: &str, updated_after: Option<NaiveDateTime>) -> Result<()> {
    let mut pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL")?)
        .await?;
    let mut file_count = 0;
    let mut page = 1;
    let page_size = 20;
    let mut last_id = None;
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
        let files =
            file::batched_get_with_cells(&pool, page_size, last_id, "Skyrim.esm", 1, updated_after)
                .await?;
        if files.is_empty() {
            break;
        }
        for file_with_cells in files {
            let path = Path::new(&dir);
            std::fs::create_dir_all(path)?;
            let path = path.join(format!("{}.json", file_with_cells.nexus_file_id));
            debug!(
                page = page,
                nexus_file_id = file_with_cells.nexus_file_id,
                "dumping file data to {}",
                path.display()
            );
            let mut file = File::create(path).await?;
            file.write_all(serde_json::to_string(&file_with_cells)?.as_bytes())
                .await?;
            last_id = Some(file_with_cells.id);
            file_count += 1;
        }
        info!("dumped page {}", page);
        page += 1;
    }
    info!("dumped {} file data files", file_count);
    Ok(())
}
