use anyhow::Result;
use chrono::NaiveDate;
use reqwest::Client;
use scraper::{Html, Selector};
use tracing::{info, instrument};

pub struct ModListResponse {
    html: Html,
}

#[derive(Debug)]
pub struct ScrapedMod<'a> {
    pub nexus_mod_id: i32,
    pub name: &'a str,
    pub category_name: Option<&'a str>,
    pub category_id: Option<i32>,
    pub author_name: &'a str,
    pub author_id: i32,
    pub desc: Option<&'a str>,
    pub thumbnail_link: Option<&'a str>,
    pub last_update_at: NaiveDate,
    pub first_upload_at: NaiveDate,
}

pub struct ModListScrape<'a> {
    pub mods: Vec<ScrapedMod<'a>>,
    pub has_next_page: bool,
}

#[instrument(skip(client))]
pub async fn get_mod_list_page(
    client: &Client,
    page: usize,
    game_name: &str,
    game_id: i32,
    include_translations: bool,
) -> Result<ModListResponse> {
    let res = client
        .get(format!(
            "https://www.nexusmods.com/Core/Libs/Common/Widgets/ModList?RH_ModList=nav:true,home:false,type:0,user_id:0,game_id:{},advfilt:true,tags_{}%5B%5D:1428,include_adult:true,page_size:20,show_game_filter:false,open:false,page:{},sort_by:lastupdate",
            game_id,
            match include_translations { true => "yes", false => "no" },
            page
        ))
        .header("host", "www.nexusmods.com")
        .header("referrer", format!("https://www.nexusmods.com/{}/mods/", game_name))
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "same-origin")
        .header("x-requested-with", "XMLHttpRequest")
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
        let thumbnail_select =
            Selector::parse("a.mod-image img.fore").expect("failed to parse CSS selector");
        let first_upload_date_select =
            Selector::parse("time.date").expect("failed to parse CSS selector");
        let last_update_date_select =
            Selector::parse("div.date").expect("failed to parse CSS selector");
        let next_page_select =
            Selector::parse("div.pagination li:last-child a.page-selected").expect("failed to parse CSS selector");

        let next_page_elem = self.html.select(&next_page_select).next();

        let has_next_page = next_page_elem.is_none();

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
                let category_id = match category_elem.value().attr("href") {
                    Some(href) => href
                        .split("/")
                        .nth(6)
                        .expect("Missing category id for mod")
                        .parse::<i32>()
                        .ok(),
                    None => None,
                };
                let category_name = category_elem.text().next();
                let author_elem = right
                    .select(&author_select)
                    .next()
                    .expect("Missing author link for mod");
                let author_id = author_elem
                    .value()
                    .attr("href")
                    .expect("Missing author link href for mod")
                    .split("/")
                    .last()
                    .expect("Missing author id for mod")
                    .parse::<i32>()
                    .expect("Failed to parse author id");
                let author_name = author_elem
                    .text()
                    .next()
                    .unwrap_or("Unknown");
                let desc_elem = right
                    .select(&desc_select)
                    .next()
                    .expect("Missing desc elem for mod");
                let desc = desc_elem.text().next();
                let thumbnail_elem = left
                    .select(&thumbnail_select)
                    .next()
                    .expect("Missing thumbnail elem for mod");
                let thumbnail_link = thumbnail_elem.value().attr("src");
                let first_upload_date_text = right
                    .select(&first_upload_date_select)
                    .next()
                    .expect("Missing dates elem for mod")
                    .text();
                let first_upload_at = first_upload_date_text
                    .skip(2)
                    .next()
                    .expect("Missing last update text for mod")
                    .trim();
                let first_upload_at = NaiveDate::parse_from_str(first_upload_at, "%d %b %Y")
                    .expect("Cannot parse first upload date");
                let last_update_date_text = right
                    .select(&last_update_date_select)
                    .next()
                    .expect("Missing dates elem for mod")
                    .text();
                let last_update_at = last_update_date_text
                    .skip(1)
                    .next()
                    .expect("Missing last update text for mod")
                    .trim();
                let last_update_at = NaiveDate::parse_from_str(last_update_at, "%d %b %Y")
                    .expect("Cannot parse last update date");

                ScrapedMod {
                    nexus_mod_id,
                    name,
                    category_name,
                    category_id,
                    author_name,
                    author_id,
                    desc,
                    thumbnail_link,
                    last_update_at,
                    first_upload_at,
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
