use anyhow::Result;
use tempfile::tempdir;
use tracing::{error, info, warn};
use unrar::Archive;

use crate::models::file::{self, File};
use crate::models::game_mod::Mod;
use crate::plugin_processor::process_plugin;

pub async fn extract_with_unrar(
    file: &mut std::fs::File,
    pool: &sqlx::Pool<sqlx::Postgres>,
    db_file: &File,
    db_mod: &Mod,
    checked_metadata: bool,
) -> Result<()> {
    let temp_dir = tempdir()?;
    let temp_file_path = temp_dir.path().join("download.rar");
    let mut temp_file = std::fs::File::create(&temp_file_path)?;
    std::io::copy(file, &mut temp_file)?;

    let mut plugin_file_paths = Vec::new();
    let list = Archive::new(&temp_file_path.to_string_lossy().to_string())?.list();
    match list {
        Ok(list) => {
            for entry in list.flatten() {
                if let Some(extension) = entry.filename.extension() {
                    if entry.is_file()
                        && (extension == "esp" || extension == "esm" || extension == "esl")
                    {
                        plugin_file_paths.push(entry.filename);
                    }
                }
            }
        }
        Err(_) => {
            if !checked_metadata {
                warn!("failed to read archive and server has no metadata, skipping file");
                file::update_unable_to_extract_plugins(&pool, db_file.id, true).await?;
                return Ok(());
            } else {
                error!("failed to read archive, but server had metadata");
                panic!("failed to read archive, but server had metadata");
            }
        }
    }
    info!(
        num_plugin_files = plugin_file_paths.len(),
        "listed plugins in downloaded archive"
    );

    if !plugin_file_paths.is_empty() {
        info!("uncompressing downloaded archive");
        let extract = Archive::new(&temp_file_path.to_string_lossy().to_string())?
            .extract_to(temp_dir.path().to_string_lossy().to_string());

        let mut extract = match extract {
            Err(err) => {
                warn!(error = %err, "failed to extract with unrar");
                file::update_unable_to_extract_plugins(&pool, db_file.id, true).await?;
                return Ok(());
            }
            Ok(extract) => extract,
        };
        if let Err(err) = extract.process() {
            warn!(error = %err, "failed to extract with unrar");
            file::update_unable_to_extract_plugins(&pool, db_file.id, true).await?;
            return Ok(());
        }

        for file_path in plugin_file_paths.iter() {
            info!(
                ?file_path,
                "processing uncompressed file from downloaded archive"
            );
            let mut plugin_buf = std::fs::read(temp_dir.path().join(file_path))?;
            process_plugin(
                &mut plugin_buf,
                &pool,
                &db_file,
                &db_mod,
                &file_path.to_string_lossy(),
            )
            .await?;
        }
    }
    Ok(())
}
