//! Tool implementations for the AI assistant example.
//!
//! This module provides all the tool implementations used by the assistant,
//! including web search via Brave API, web page content fetching, and datetime retrieval.

use std::fmt::{self, Write};

use chrono::{DateTime, Utc};
use reqwest::{
    Method,
    blocking::{Client, Request},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Brave Search API endpoint
const BRAVE_SEARCH_ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";

/// Length to truncate fetched webpages to, in characters.
const SENSIBLE_TEXT_LENGTH: usize = 50_000;

/// Input to the web search tool.
#[derive(Debug, JsonSchema, Serialize, Deserialize)]
pub struct WebSearchInput {
    /// The query to search for.
    pub query: String,
}

/// Input to the datetime tool (empty).
#[derive(Debug, JsonSchema, Serialize, Deserialize)]
pub struct DateTimeInput {}

/// Input to the fetch page tool.
#[derive(Debug, JsonSchema, Serialize, Deserialize)]
pub struct FetchPageInput {
    /// The URL of the page to fetch.
    pub url: String,
}

/// A search result from the web search API.
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    /// The title of the web page
    pub title: String,
    /// A brief description or snippet from the page
    pub description: String,
    /// The full URL of the page
    pub url: String,
}

impl fmt::Display for SearchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}

/// Tool that returns the current date and time in ISO 8601 format.
pub fn tool_get_datetime() -> String {
    let now: DateTime<Utc> = Utc::now();
    now.to_rfc3339()
}

/// Performs a web search using the Brave Search API.
pub fn tool_web_search(
    client: &Client,
    api_key: Option<&str>,
    term: &str,
) -> Result<Vec<SearchResult>, String> {
    #[derive(Debug, Deserialize)]
    struct BraveWebSearchApiResponse {
        web: Option<BraveSearch>,
    }

    #[derive(Debug, Deserialize, Default)]
    struct BraveSearch {
        results: Vec<BraveResult>,
    }

    #[derive(Debug, Deserialize)]
    struct BraveResult {
        title: String,
        description: Option<String>,
        url: String,
    }

    let api_key = api_key.ok_or("API key is required for web search")?;

    let request = client
        .get(BRAVE_SEARCH_ENDPOINT)
        .query(&[("q", term)])
        .header("Accept", "application/json")
        .header("X-Subscription-Token", api_key)
        .build()
        .expect("Failed to build request");

    let response = super::send_request(client, request)?;
    let search_response: BraveWebSearchApiResponse =
        serde_json::from_str(&response).map_err(|e| format!("Failed to parse response: {}", e))?;

    let results = search_response
        .web
        .unwrap_or_default()
        .results
        .into_iter()
        .map(|result| SearchResult {
            title: result.title,
            description: result.description.unwrap_or_default(),
            url: result.url,
        })
        .collect();

    Ok(results)
}

/// Fetches the content of a web page and converts it to clean text.
///
/// Truncates if the fetched page exceeds [`SENSIBLE_TEXT_LENGTH`] *in bytes*.
pub fn tool_fetch_page(client: &Client, url: &str) -> Result<String, String> {
    let request = Request::new(Method::GET, url.parse().expect("Failed to parse URL"));
    let html = super::send_request(client, request)?;

    // Convert HTML to clean text with reasonable width for readability
    let text = html2text::from_read(html.as_bytes(), 80)
        .map_err(|e| format!("Failed to convert HTML to text: {}", e))?;

    let mut truncated = text.chars().take(SENSIBLE_TEXT_LENGTH).collect::<String>();
    let new_len = truncated.len();

    if new_len != text.len() {
        write!(
            &mut truncated,
            "\nTHIS PAGE WAS {} BYTES ORIGINALLY, TRUNCATED TO {}\n",
            text.len(),
            new_len
        )
        .expect("write to string should not fail");
    }

    Ok(truncated)
}
