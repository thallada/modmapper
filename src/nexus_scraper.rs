use anyhow::Result;
use reqwest::Client;
use scraper::{Html, Selector};
use tracing::{info, instrument};

use crate::nexus_api::GAME_ID;

pub struct ModListResponse {
    html: Html,
}
pub struct ScrapedMod<'a> {
    pub nexus_mod_id: i32,
    pub name: &'a str,
    pub category: &'a str,
    pub author: &'a str,
    pub desc: Option<&'a str>,
}

pub struct ModListScrape<'a> {
    pub mods: Vec<ScrapedMod<'a>>,
    pub has_next_page: bool,
}

#[instrument(skip(client))]
pub async fn get_mod_list_page(client: &Client, page: i32) -> Result<ModListResponse> {
    let res = client
        .get(format!(
            "https://www.nexusmods.com/Core/Libs/Common/Widgets/ModList?RH_ModList=nav:true,home:false,type:0,user_id:0,game_id:{},advfilt:true,include_adult:true,page_size:80,show_game_filter:false,open:false,page:{},sort_by:OLD_u_downloads",
            GAME_ID,
            page
        ))
        .send()
        .await?
        .error_for_status()?;
    info!(status = %res.status(), "fetched mod list page");
    let text = res.text().await?;
    let html = Html::parse_document(&text);

    Ok(ModListResponse { html })
}

impl ModListResponse {
    #[instrument(skip(self))]
    pub fn scrape_mods<'a>(&'a self) -> Result<ModListScrape> {
        let mod_select = Selector::parse("li.mod-tile").expect("failed to parse CSS selector");
        let left_select =
            Selector::parse("div.mod-tile-left").expect("failed to parse CSS selector");
        let right_select =
            Selector::parse("div.mod-tile-right").expect("failed to parse CSS selector");
        let name_select = Selector::parse("p.tile-name a").expect("failed to parse CSS selector");
        let category_select =
            Selector::parse("div.category a").expect("failed to parse CSS selector");
        let author_select = Selector::parse("div.author a").expect("failed to parse CSS selector");
        let desc_select = Selector::parse("p.desc").expect("failed to parse CSS selector");
        let next_page_select =
            Selector::parse("div.pagination li.next").expect("failed to parse CSS selector");

        let next_page_elem = self.html.select(&next_page_select).next();

        let has_next_page = next_page_elem.is_some();

        let mods: Vec<ScrapedMod> = self
            .html
            .select(&mod_select)
            .map(|element| {
                let left = element
                    .select(&left_select)
                    .next()
                    .expect("Missing left div for mod");
                let right = element
                    .select(&right_select)
                    .next()
                    .expect("Missing right div for mod");
                let nexus_mod_id = left
                    .value()
                    .attr("data-mod-id")
                    .expect("Missing mod id attribute")
                    .parse::<i32>()
                    .ok()
                    .expect("Failed to parse mod id");
                let name_elem = right
                    .select(&name_select)
                    .next()
                    .expect("Missing name link for mod");
                let name = name_elem.text().next().expect("Missing name text for mod");
                let category_elem = right
                    .select(&category_select)
                    .next()
                    .expect("Missing category link for mod");
                let category = category_elem
                    .text()
                    .next()
                    .expect("Missing category text for mod");
                let author_elem = right
                    .select(&author_select)
                    .next()
                    .expect("Missing author link for mod");
                let author = author_elem
                    .text()
                    .next()
                    .expect("Missing author text for mod");
                let desc_elem = right
                    .select(&desc_select)
                    .next()
                    .expect("Missing desc elem for mod");
                let desc = desc_elem.text().next();

                ScrapedMod {
                    nexus_mod_id,
                    name,
                    category,
                    author,
                    desc,
                }
            })
            .collect();
        info!(
            len = mods.len(),
            has_next_page, "scraped mods from mod list page"
        );
        Ok(ModListScrape {
            mods,
            has_next_page,
        })
    }
}
