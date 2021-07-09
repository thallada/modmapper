use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct File {
    pub id: i32,
    pub name: String,
    pub file_name: String,
    pub nexus_file_id: i32,
    pub mod_id: i32,
    pub category: Option<String>,
    pub version: Option<String>,
    pub mod_version: Option<String>,
    pub uploaded_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

pub async fn insert_file(
    pool: &sqlx::Pool<sqlx::Postgres>,
    name: &str,
    file_name: &str,
    nexus_file_id: i32,
    mod_id: i32,
    category: Option<&str>,
    version: Option<&str>,
    mod_version: Option<&str>,
    uploaded_at: NaiveDateTime,
) -> Result<File> {
    sqlx::query_as!(
        File,
        "INSERT INTO files
            (name, file_name, nexus_file_id, mod_id, category, version, mod_version, uploaded_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now(), now())
            ON CONFLICT (mod_id, nexus_file_id) DO UPDATE
            SET (name, file_name, category, version, mod_version, uploaded_at, updated_at) =
            (EXCLUDED.name, EXCLUDED.file_name, EXCLUDED.category, EXCLUDED.version, EXCLUDED.mod_version, EXCLUDED.uploaded_at, now())
            RETURNING *",
        name,
        file_name,
        nexus_file_id,
        mod_id,
        category,
        version,
        mod_version,
        uploaded_at
    )
    .fetch_one(pool)
    .await
    .context("Failed to insert file")
}
