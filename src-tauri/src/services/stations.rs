//! Radio station directory powered by radio-browser.info API.

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::error::AppError;

const API_BASE: &str = "https://de1.api.radio-browser.info/json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Station {
    pub stationuuid: String,
    pub name: String,
    #[serde(alias = "url_resolved")]
    pub url_resolved: String,
    #[serde(default)]
    pub favicon: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub bitrate: u32,
    #[serde(default, alias = "is_favorite")]
    pub is_favorite: bool,
    #[serde(default, alias = "fav_index")]
    pub fav_index: i32,
    #[serde(default, alias = "all_index")]
    pub all_index: i32,
    #[serde(default, alias = "lastcheckok")]
    pub lastcheckok: i32,
    #[serde(default, alias = "clickcount")]
    pub clickcount: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryItem {
    pub name: String,
    #[serde(default, alias = "iso_3166_1")]
    pub iso_3166_1: String,
    #[serde(default)]
    pub stationcount: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageItem {
    pub name: String,
    #[serde(default)]
    pub stationcount: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagItem {
    pub name: String,
    #[serde(default)]
    pub stationcount: u32,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("Radiko/1.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default()
}

/// Search stations with optional filters.
#[allow(clippy::too_many_arguments)]
pub async fn search(
    name: Option<String>,
    country: Option<String>,
    state: Option<String>,
    language: Option<String>,
    tag: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
    hide_broken: Option<bool>,
    only_verified: Option<bool>,
) -> Result<Vec<Station>, AppError> {
    let mut params: Vec<(&str, String)> = vec![
        ("hidebroken", hide_broken.unwrap_or(true).to_string()),
        ("limit", limit.unwrap_or(50).to_string()),
        ("offset", offset.unwrap_or(0).to_string()),
        ("order", "votes".into()),
        ("reverse", "true".into()),
    ];

    if let Some(true) = only_verified {
        params.push(("is_https", "true".into()));
    }

    if let Some(ref n) = name {
        if !n.is_empty() {
            params.push(("name", n.clone()));
        }
    }
    if let Some(ref c) = country {
        if !c.is_empty() {
            params.push(("country", c.clone()));
        }
    }
    if let Some(ref s) = state {
        if !s.is_empty() {
            params.push(("state", s.clone()));
        }
    }
    if let Some(ref l) = language {
        if !l.is_empty() {
            params.push(("language", l.clone()));
        }
    }
    if let Some(ref t) = tag {
        if !t.is_empty() {
            params.push(("tag", t.clone()));
        }
    }

    info!("Searching stations: {:?}", params);

    let resp = client()
        .get(format!("{}/stations/search", API_BASE))
        .query(&params)
        .send()
        .await
        .map_err(|e| AppError::Connection(e.to_string()))?;

    let stations: Vec<Station> = resp
        .json()
        .await
        .map_err(|e| AppError::Connection(format!("Parse error: {}", e)))?;

    info!("Found {} stations", stations.len());
    Ok(stations)
}

/// Get top-voted stations.
pub async fn top_stations(limit: Option<u32>) -> Result<Vec<Station>, AppError> {
    let limit = limit.unwrap_or(50);

    let resp = client()
        .get(format!("{}/stations/topvote/{}", API_BASE, limit))
        .query(&[("hidebroken", "true")])
        .send()
        .await
        .map_err(|e| AppError::Connection(e.to_string()))?;

    let stations: Vec<Station> = resp
        .json()
        .await
        .map_err(|e| AppError::Connection(format!("Parse error: {}", e)))?;

    Ok(stations)
}

/// Get available countries (sorted by station count).
pub async fn countries() -> Result<Vec<CountryItem>, AppError> {
    let resp = client()
        .get(format!("{}/countries", API_BASE))
        .query(&[("order", "stationcount"), ("reverse", "true"), ("hidebroken", "true")])
        .send()
        .await
        .map_err(|e| AppError::Connection(e.to_string()))?;

    let items: Vec<CountryItem> = resp
        .json()
        .await
        .map_err(|e| AppError::Connection(format!("Parse error: {}", e)))?;

    // Only return countries with at least 5 stations
    Ok(items.into_iter().filter(|c| c.stationcount >= 5).collect())
}

/// Get available languages (sorted by station count).
pub async fn languages() -> Result<Vec<LanguageItem>, AppError> {
    let resp = client()
        .get(format!("{}/languages", API_BASE))
        .query(&[("order", "stationcount"), ("reverse", "true"), ("hidebroken", "true")])
        .send()
        .await
        .map_err(|e| AppError::Connection(e.to_string()))?;

    let items: Vec<LanguageItem> = resp
        .json()
        .await
        .map_err(|e| AppError::Connection(format!("Parse error: {}", e)))?;

    Ok(items.into_iter().filter(|l| l.stationcount >= 5).collect())
}

/// Get available tags/genres (sorted by station count).
pub async fn tags(limit: Option<u32>) -> Result<Vec<TagItem>, AppError> {
    let limit = limit.unwrap_or(100);

    let resp = client()
        .get(format!("{}/tags", API_BASE))
        .query(&[
            ("order", "stationcount"),
            ("reverse", "true"),
            ("hidebroken", "true"),
            ("limit", &limit.to_string()),
        ])
        .send()
        .await
        .map_err(|e| AppError::Connection(e.to_string()))?;

    let items: Vec<TagItem> = resp
        .json()
        .await
        .map_err(|e| AppError::Connection(format!("Parse error: {}", e)))?;

    Ok(items.into_iter().filter(|t| t.stationcount >= 5).collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateItem {
    pub name: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub stationcount: u32,
}

/// Get states/cities for a country.
pub async fn states(country: String) -> Result<Vec<StateItem>, AppError> {
    let encoded = urlencoding::encode(&country);
    let resp = client()
        .get(format!("{}/states/{}/", API_BASE, encoded))
        .query(&[("order", "name")])
        .send()
        .await
        .map_err(|e| AppError::Connection(e.to_string()))?;

    let items: Vec<StateItem> = resp
        .json()
        .await
        .map_err(|e| AppError::Connection(format!("Parse error: {}", e)))?;

    Ok(items.into_iter().filter(|s| !s.name.is_empty() && s.stationcount > 0).collect())
}

/// Get ALL stations for a country by paginating through results.
pub async fn all_country_stations(country: String) -> Result<Vec<Station>, AppError> {
    let mut all = Vec::new();
    let page_size = 1000u32;
    let mut offset = 0u32;

    loop {
        info!("Fetching stations for {} offset={}", country, offset);
        let params: Vec<(&str, String)> = vec![
            ("hidebroken", "true".into()),
            ("limit", page_size.to_string()),
            ("offset", offset.to_string()),
            ("order", "votes".into()),
            ("reverse", "true".into()),
            ("country", country.clone()),
        ];

        let resp = client()
            .get(format!("{}/stations/search", API_BASE))
            .query(&params)
            .send()
            .await
            .map_err(|e| AppError::Connection(e.to_string()))?;

        let stations: Vec<Station> = resp
            .json()
            .await
            .map_err(|e| AppError::Connection(format!("Parse error: {}", e)))?;

        let count = stations.len();
        all.extend(stations);
        info!("Got {} stations (total: {})", count, all.len());

        if count < page_size as usize {
            break;
        }
        offset += page_size;
    }

    Ok(all)
}
