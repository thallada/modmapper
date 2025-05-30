use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::instrument;

pub const PAGE_SIZE: usize = 20;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphQLRequest {
    query: String,
    variables: Value,
    #[serde(rename = "operationName")]
    operation_name: String,
}

#[derive(Debug, Deserialize)]
pub struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
pub struct GraphQLError {
    #[allow(dead_code)]
    message: String,
}

#[derive(Debug, Deserialize)]
pub struct ModsResponse {
    pub mods: ModsData,
}

#[derive(Debug, Deserialize)]
pub struct ModsData {
    #[serde(rename = "facetsData")]
    #[allow(dead_code)]
    pub facets_data: Option<Value>,
    pub nodes: Vec<Mod>,
    #[allow(dead_code)]
    #[serde(rename = "totalCount")]
    pub total_count: i32,
}

#[derive(Debug, Deserialize)]
pub struct Mod {
    #[serde(rename = "modId")]
    pub mod_id: i32,
    pub name: String,
    pub summary: Option<String>,
    #[allow(dead_code)]
    pub downloads: i32,
    #[allow(dead_code)]
    pub endorsements: i32,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "modCategory")]
    pub mod_category: Option<ModCategory>,
    pub uploader: Uploader,
    #[serde(rename = "thumbnailUrl")]
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModCategory {
    #[serde(rename = "categoryId")]
    pub category_id: i32,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Uploader {
    #[serde(rename = "memberId")]
    pub member_id: i32,
    pub name: String,
}

pub struct NexusScraper {
    client: Client,
    base_url: String,
}

impl<'a> ScrapedMod<'a> {
    pub fn from_api_mod(api_mod: &'a Mod) -> Result<Self> {
        // Parse dates from ISO 8601 format like "2025-05-30T15:29:50Z"
        let parse_date = |date_str: &str| -> Result<NaiveDate, chrono::ParseError> {
            chrono::DateTime::parse_from_rfc3339(date_str).map(|dt| dt.naive_utc().date())
        };

        let last_update_at = parse_date(&api_mod.updated_at)?;
        let first_upload_at = parse_date(&api_mod.created_at)?;

        Ok(ScrapedMod {
            nexus_mod_id: api_mod.mod_id,
            name: &api_mod.name,
            category_name: api_mod.mod_category.as_ref().map(|cat| cat.name.as_str()),
            category_id: api_mod.mod_category.as_ref().map(|cat| cat.category_id),
            author_name: &api_mod.uploader.name,
            author_id: api_mod.uploader.member_id,
            desc: api_mod.summary.as_deref(),
            thumbnail_link: api_mod.thumbnail_url.as_deref(),
            last_update_at,
            first_upload_at,
        })
    }
}

pub fn convert_mods_to_scraped<'a>(api_mods: &'a [Mod]) -> Result<Vec<ScrapedMod<'a>>> {
    api_mods.iter().map(ScrapedMod::from_api_mod).collect()
}

impl NexusScraper {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            base_url: "https://api-router.nexusmods.com/graphql".to_string(),
        }
    }

    #[instrument(skip(self))]
    pub async fn get_mods(
        &self,
        game_domain: &str,
        offset: usize,
        include_translations: bool,
    ) -> Result<ModsResponse> {
        let mut filter = json!({ "tag": [{ "op": "NOT_EQUALS", "value": "Translation" }] });
        if include_translations {
            filter = json!({ "tag": [{ "op": "EQUALS", "value": "Translation" }] });
        }
        let query = r#"
    query ModsListing($count: Int = 0, $facets: ModsFacet, $filter: ModsFilter, $offset: Int, $postFilter: ModsFilter, $sort: [ModsSort!]) {
  mods(
    count: $count
    facets: $facets
    filter: $filter
    offset: $offset
    postFilter: $postFilter
    sort: $sort
    viewUserBlockedContent: false
  ) {
    facetsData
    nodes {
      ...ModFragment
    }
    totalCount
  }
}
    fragment ModFragment on Mod {
  adultContent
  createdAt
  downloads
  endorsements
  fileSize
  game {
    domainName
    id
    name
  }
  modCategory {
    categoryId
    name
  }
  modId
  name
  status
  summary
  thumbnailUrl
  thumbnailBlurredUrl
  uid
  updatedAt
  uploader {
    avatar
    memberId
    name
  }
  viewerDownloaded
  viewerEndorsed
  viewerTracked
  viewerUpdateAvailable
}"#;

        let variables = json!({
            "count": 20,
            "facets": {
                "categoryName": [],
                "languageName": [],
                "tag": []
            },
            "filter": {
                "filter": [],
                "gameDomainName": [{"op": "EQUALS", "value": game_domain}],
                "name": []
            },
            "offset": offset,
            "postFilter": filter,
            "sort": {
                "updatedAt": {"direction": "DESC"}
            }
        });

        let request_body = GraphQLRequest {
            query: query.to_string(),
            variables,
            operation_name: "ModsListing".to_string(),
        };

        let response = self
            .client
            .post(&self.base_url)
            .header("Referer", "https://www.nexusmods.com/")
            .header("content-type", "application/json")
            .header("x-graphql-operationname", "GameModsListing")
            .header("Origin", "https://www.nexusmods.com")
            .header("Sec-Fetch-Dest", "empty")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Site", "same-site")
            .json(&request_body)
            .send()
            .await?;

        let graphql_response: GraphQLResponse<ModsResponse> = response.json().await?;

        if let Some(errors) = graphql_response.errors {
            return Err(anyhow!("GraphQL errors: {:?}", errors));
        }

        graphql_response
            .data
            .ok_or_else(|| anyhow!("No data returned from GraphQL"))
    }
}
