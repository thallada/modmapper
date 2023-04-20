use anyhow::Result;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::Path;
use tracing::info;

use crate::models::cell;

pub async fn dump_cell_data(pool: &sqlx::Pool<sqlx::Postgres>, dir: &str) -> Result<()> {
    for x in -77..75 {
        for y in -50..44 {
            if let Ok(data) = cell::get_cell_data(pool, "Skyrim.esm", 1, x, y).await {
                let path = format!("{}/{}", &dir, x);
                let path = Path::new(&path);
                create_dir_all(path)?;
                let path = path.join(format!("{}.json", y));
                info!(
                    x = x,
                    y = y,
                    form_id = data.form_id,
                    "dumping cell data to {}",
                    path.display()
                );
                let mut file = File::create(path)?;
                write!(file, "{}", serde_json::to_string(&data)?)?;
            }
        }
    }
    Ok(())
}
