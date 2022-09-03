use anyhow::Result;
use std::fs::File;
use std::io::Write;
use tracing::info;

use crate::models::game;

pub async fn dump_games(pool: &sqlx::Pool<sqlx::Postgres>, path: &str) -> Result<()> {
    let games = game::get_all(&pool).await?;
    info!("writing {} games to {}", games.len(), path);
    let mut file = File::create(path)?;
    write!(file, "{}", serde_json::to_string(&games)?)?;
    return Ok(());
}
