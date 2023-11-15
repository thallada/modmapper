use crate::models::cell::{self, CellFileEditCount};
use anyhow::Result;
use chrono::{Duration, NaiveDateTime};
use std::collections::HashMap;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

pub async fn dump_cell_edit_counts_over_time(
    pool: &sqlx::Pool<sqlx::Postgres>,
    start_date: NaiveDateTime,
    end_date: NaiveDateTime,
    path: &str,
) -> Result<()> {
    let mut current_date = start_date;
    while current_date <= end_date {
        let next_date = current_date + Duration::weeks(1);
        let mut cell_file_edit_counts = HashMap::new();
        let counts =
            cell::count_file_edits_in_time_range(pool, "Skyrim.esm", 1, current_date, next_date)
                .await?;
        for x in -77..75 {
            for y in -50..44 {
                let count: Option<&CellFileEditCount> = counts.iter().find(|c| c.x.unwrap() == x && c.y.unwrap() == y);
                let count = count.map(|c| c.count).unwrap_or(Some(0)).unwrap();
                debug!(x = x, y = y, count = count, "read cell edit count");
                cell_file_edit_counts.insert(format!("{},{}", x, y), count);
            }
        }

        let file_name = format!(
            "{}/cell_edits_{}.json",
            path,
            current_date.format("%Y-%m-%d")
        );
        info!(
            "writing {} cell edit counts to {}",
            cell_file_edit_counts.values().sum::<i64>(),
            file_name
        );
        let mut file = File::create(&file_name).await?;
        file.write_all(serde_json::to_string(&cell_file_edit_counts)?.as_bytes()).await?;

        current_date = next_date;
    }
    Ok(())
}
