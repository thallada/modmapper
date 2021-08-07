/// Extracts zip files most of the time with some exceptions. If this files we'll fall back to other methods.
use anyhow::{Context, Result};
use compress_tools::{list_archive_files, uncompress_archive_file};
use std::collections::VecDeque;
use std::fmt::Display;
use std::io::Seek;
use std::io::SeekFrom;
use tracing::{info, info_span};

#[derive(Debug)]
pub struct ExtractorError;

impl Display for ExtractorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "extraction error")
    }
}
pub struct Extractor<'a> {
    file: &'a mut std::fs::File,
    plugin_file_paths: Option<VecDeque<String>>,
}

impl<'a> Extractor<'a> {
    pub fn new(file: &mut std::fs::File) -> Extractor {
        Extractor {
            file,
            plugin_file_paths: None,
        }
    }

    fn list_plugins(&mut self) -> Result<()> {
        let mut plugin_file_paths = VecDeque::new();
        let archive_files = list_archive_files(&mut self.file)?;
        for file_path in archive_files {
            if file_path.ends_with(".esp")
                || file_path.ends_with(".esm")
                || file_path.ends_with(".esl")
            {
                plugin_file_paths.push_back(file_path);
            }
        }
        info!(
            num_plugin_files = plugin_file_paths.len(),
            "listed plugins in downloaded archive"
        );
        self.plugin_file_paths = Some(plugin_file_paths);
        Ok(())
    }

    fn get_plugin(&mut self, file_path: &str) -> Result<Vec<u8>> {
        let plugin_span = info_span!("plugin", name = ?file_path);
        let _plugin_span = plugin_span.enter();
        self.file.seek(SeekFrom::Start(0))?;
        let mut buf = Vec::default();
        info!("uncompressing plugin file from downloaded archive");
        uncompress_archive_file(&mut self.file, &mut buf, &file_path)?;
        Ok(buf)
    }
}

impl<'a> Iterator for Extractor<'a> {
    type Item = Result<(String, Vec<u8>)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.plugin_file_paths.is_none() {
            match self.list_plugins() {
                Err(err) => return Some(Err(err).context(ExtractorError)),
                _ => {}
            }
        }
        if let Some(plugin_file_paths) = &mut self.plugin_file_paths {
            if let Some(file_path) = plugin_file_paths.pop_front() {
                return match self.get_plugin(&file_path) {
                    Err(err) => Some(Err(err).context(ExtractorError)),
                    Ok(plugin_buf) => Some(Ok((file_path, plugin_buf))),
                };
            }
        }
        None
    }
}
