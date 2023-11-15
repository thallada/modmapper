use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::fs::create_dir_all;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

use crate::models::cell;

pub async fn dump_cell_data(dir: &str) -> Result<()> {
    let mut pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL")?)
        .await?;
    let mut cell_count = 0;
    for x in -77..75 {
        for y in -50..44 {
            if cell_count % 5 == 0 {
                // There's a weird issue that slows down this query after 5 iterations. Recreating the
                // connection pool seems to fix it. I don't know why.
                info!("reconnecting to database");
                pool = PgPoolOptions::new()
                    .max_connections(5)
                    .connect(&env::var("DATABASE_URL")?)
                    .await?;
            }
            if let Ok(data) = cell::get_cell_data(&pool, "Skyrim.esm", 1, x, y, true).await {
                let path = format!("{}/{}", &dir, x);
                let path = Path::new(&path);
                create_dir_all(path)?;
                let path = path.join(format!("{}.json", y));
                debug!(
                    x = x,
                    y = y,
                    form_id = data.form_id,
                    "dumping cell data to {}",
                    path.display()
                );
                let mut file = File::create(path).await?;
                file.write_all(serde_json::to_string(&data)?.as_bytes())
                    .await?;
                cell_count += 1;
            }
        }
        info!("dumped all rows in x: {}", x);
    }
    info!("dumped {} cell data files", cell_count);
    Ok(())
}
