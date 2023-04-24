use anyhow::Result;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use tracing::{debug, info};

use crate::models::cell;

pub async fn dump_cell_edit_counts(pool: &sqlx::Pool<sqlx::Postgres>, path: &str) -> Result<()> {
    let mut cell_mod_edit_counts = HashMap::new();
    for x in -77..75 {
        for y in -50..44 {
            if let Some(count) = cell::count_mod_edits(pool, "Skyrim.esm", 1, x, y).await? {
                debug!(x = x, y = y, count = count, "read cell edit count");
                cell_mod_edit_counts.insert(format!("{},{}", x, y), count);
            }
        }
    }
    info!(
        "writing {} cell edit counts to {}",
        cell_mod_edit_counts.len(),
        path
    );
    let mut file = File::create(path)?;
    write!(file, "{}", serde_json::to_string(&cell_mod_edit_counts)?)?;
    Ok(())
}
