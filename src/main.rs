use anyhow::Result;
use argh::FromArgs;
use chrono::{NaiveDate, NaiveDateTime, Utc};
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
    backfills::backfill_is_base_game, backfills::backfill_is_translation,
    backfills::deduplicate_interior_cells, download_tiles, dump_cell_data, dump_cell_edit_counts,
    dump_cell_edit_counts_over_time, dump_file_data, dump_games, dump_mod_cell_counts,
    dump_mod_data, dump_mod_search_index, dump_plugin_data, update, TimeStep,
};

#[derive(FromArgs)]
/// Downloads every mod off nexus mods, parses CELL and WRLD data from plugins in each, and saves the da&ta to the database.
struct Args {
    #[argh(option, short = 'p', default = "1")]
    /// the page number to start scraping for mods on nexus mods
    page: usize,

    #[argh(
        option,
        short = 'g',
        default = "String::from(\"skyrimspecialedition\")"
    )]
    /// name of nexus game to scrape (e.g. "skyrim" or "skyrimspecialedition")
    game: String,

    #[argh(switch, short = 'f')]
    /// enable full scrape of all pages, rather than stopping after 50 pages of no updates
    full: bool,

    /// file to output the cell mod edit counts as json
    #[argh(option, short = 'e')]
    dump_edits: Option<String>,

    /// file to output the cell mod edit counts over time as json (time_step option required with 
    /// this option)
    #[argh(option, short = 'E')]
    dump_edits_over_time: Option<String>,

    /// the span of time to group cell edit counts into (day, week, or month) when dumping cell 
    /// edits (only relevant for use with dump_edits_over_time option)
    #[argh(option, short = 'T')]
    time_step: Option<TimeStep>,

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

    /// file to output all the game data as json
    #[argh(option, short = 'G')]
    game_data: Option<String>,

    /// folder to output all map tile images downloaded from the UESP wiki
    #[argh(option, short = 't')]
    download_tiles: Option<String>,

    /// backfill the is_translation column in the mods table
    #[argh(switch)]
    backfill_is_translation: bool,

    /// backfill the is_base_game column in the cells table (for Skyrim.esm)
    #[argh(switch)]
    backfill_is_base_game: bool,

    /// deduplicate the interior cells with same form_id and master
    #[argh(switch)]
    deduplicate_interior_cells: bool,

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
    if let Some(path) = args.dump_edits_over_time {
        if let Some(time_step) = args.time_step {
            return dump_cell_edit_counts_over_time(
                NaiveDate::from_ymd_opt(2011, 11, 11)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
                Utc::now().naive_utc(),
                time_step,
                &path,
            )
            .await;
        } else {
            panic!("time_step option required with dump_edits_over_time option");
        }
    }
    if let Some(dir) = args.cell_data {
        return dump_cell_data(&dir).await;
    }
    if let Some(dir) = args.mod_data {
        return dump_mod_data(&dir, args.updated_after).await;
    }
    if let Some(path) = args.mod_search_index {
        return dump_mod_search_index(&args.game, &path).await;
    }
    if let Some(path) = args.mod_cell_counts {
        return dump_mod_cell_counts(&path).await;
    }
    if let Some(path) = args.plugin_data {
        return dump_plugin_data(&path, args.updated_after).await;
    }
    if let Some(path) = args.file_data {
        return dump_file_data(&path, args.updated_after).await;
    }
    if let Some(path) = args.game_data {
        return dump_games(&pool, &path).await;
    }
    if let Some(dir) = args.download_tiles {
        return download_tiles(&dir).await;
    }
    if args.backfill_is_translation {
        return backfill_is_translation(&pool).await;
    }
    if args.backfill_is_base_game {
        return backfill_is_base_game(&pool).await;
    }
    if args.deduplicate_interior_cells {
        return deduplicate_interior_cells(&pool).await;
    }

    update(&pool, args.page, &args.game, args.full).await
}
