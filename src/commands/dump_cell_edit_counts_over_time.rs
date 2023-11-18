use crate::models::cell::{self, CellFileEditCount};
use anyhow::Result;
use chrono::{Duration, NaiveDateTime, Months};
use sqlx::postgres::PgPoolOptions;
use std::{collections::HashMap, env, str::FromStr};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

#[derive(Debug)]
pub enum TimeStep {
    Day,
    Week,
    Month,
}

impl FromStr for TimeStep {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "day" => Ok(TimeStep::Day),
            "week" => Ok(TimeStep::Week),
            "month" => Ok(TimeStep::Month),
            _ => Err(format!("invalid time step: {}", s)),
        }
    }
}

pub async fn dump_cell_edit_counts_over_time(
    start_date: NaiveDateTime,
    end_date: NaiveDateTime,
    time_step: TimeStep,
    path: &str,
) -> Result<()> {
    let mut pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL")?)
        .await?;
    let mut i = 0;
    let mut current_date = start_date;
    while current_date <= end_date {
        if i % 5 == 0 {
            // There's a weird issue that slows down this query after 5 iterations. Recreating the
            // connection pool seems to fix it. I don't know why.
            info!("reconnecting to database");
            pool = PgPoolOptions::new()
                .max_connections(5)
                .connect(&env::var("DATABASE_URL")?)
                .await?;
        }
        let next_date = match &time_step {
            TimeStep::Day => current_date + Duration::days(1),
            TimeStep::Week => current_date + Duration::weeks(1),
            TimeStep::Month => current_date.checked_add_months(Months::new(1)).unwrap(),
        };
        let mut cell_file_edit_counts = HashMap::new();
        let counts =
            cell::count_file_edits_in_time_range(&pool, "Skyrim.esm", 1, current_date, next_date)
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
        i += 1;
    }
    Ok(())
}
