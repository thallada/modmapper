use anyhow::Result;
use argh::FromArgs;
use chrono::NaiveDateTime;
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;

mod commands;
mod extractors;
mod models;
mod nexus_api;
mod nexus_scraper;
mod plugin_processor;

use commands::{
    backfills::backfill_is_translation, download_tiles, dump_cell_data, dump_cell_edit_counts,
    dump_mod_cell_counts, dump_mod_data, dump_mod_search_index, dump_plugin_data, dump_file_data, update,
};

#[derive(FromArgs)]
/// Downloads every mod off nexus mods, parses CELL and WRLD data from plugins in each, and saves the data to the database.
struct Args {
    #[argh(option, short = 'p', default = "1")]
    /// the page number to start scraping for mods on nexus mods
    page: usize,

    #[argh(switch, short = 'f')]
    /// enable full scrape of all pages, rather than stopping after 50 pages of no updates
    full: bool,

    /// file to output the cell mod edit counts as json
    #[argh(option, short = 'e')]
    dump_edits: Option<String>,

    /// folder to output all cell data as json files
    #[argh(option, short = 'c')]
    cell_data: Option<String>,

    /// folder to output all mod data as json files
    #[argh(option, short = 'm')]
    mod_data: Option<String>,

    /// file to output all mod titles and ids as a json search index
    #[argh(option, short = 's')]
    mod_search_index: Option<String>,

    /// file to output all mod cell edit counts and ids as a json index
    #[argh(option, short = 'M')]
    mod_cell_counts: Option<String>,

    /// folder to output all plugin data as json files
    #[argh(option, short = 'P')]
    plugin_data: Option<String>,

    /// folder to output all files data as json files
    #[argh(option, short = 'F')]
    file_data: Option<String>,

    /// folder to output all map tile images downloaded from the UESP wiki
    #[argh(option, short = 't')]
    download_tiles: Option<String>,

    /// backfill the is_translation column in the mods table
    #[argh(switch)]
    backfill_is_translation: bool,

    /// when dumping data, only dump data for mods or files that have been updated since this date
    #[argh(option, short = 'u')]
    updated_after: Option<NaiveDateTime>,
}

#[tokio::main]
pub async fn main() -> Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt::init();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&env::var("DATABASE_URL")?)
        .await?;

    let args: Args = argh::from_env();

    if let Some(path) = args.dump_edits {
        return dump_cell_edit_counts(&pool, &path).await;
    }
    if let Some(dir) = args.cell_data {
        return dump_cell_data(&pool, &dir).await;
    }
    if let Some(dir) = args.mod_data {
        return dump_mod_data(&pool, &dir, args.updated_after).await;
    }
    if let Some(path) = args.mod_search_index {
        return dump_mod_search_index(&pool, &path).await;
    }
    if let Some(path) = args.mod_cell_counts {
        return dump_mod_cell_counts(&pool, &path).await;
    }
    if let Some(path) = args.plugin_data {
        return dump_plugin_data(&pool, &path, args.updated_after).await;
    }
    if let Some(path) = args.file_data {
        return dump_file_data(&pool, &path, args.updated_after).await;
    }
    if let Some(dir) = args.download_tiles {
        return download_tiles(&dir).await;
    }
    if args.backfill_is_translation {
        return backfill_is_translation(&pool).await;
    }

    return update(&pool, args.page, args.full).await;
}
