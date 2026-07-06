use color_eyre::eyre::Result;
use reqwest::header::{HeaderMap, HeaderValue, ORIGIN, REFERER, USER_AGENT};
use rig::tool::ToolDyn;
use scraper::{Html, Selector};
use serde::Serialize;
use std::collections::HashMap;
use thiserror::Error;

pub fn toolset() -> Vec<Box<dyn ToolDyn>> {
    vec![Box::new(WebSearch)]
}

#[derive(Debug, Error)]
pub enum WebSearchError {
    #[error("Failed to make web search request")]
    RequestError(#[from] reqwest::Error),

    #[error("Status code not OK")]
    HttpError(String),
}

#[derive(Serialize)]
pub struct WebSearchResult {
    title: String,
    link: String,
    snippet: String,
}

#[rig::tool_macro(
    description = "Search the web for a query",
    params(query = "The query to search for")
)]
pub async fn web_search(query: String) -> Result<Vec<WebSearchResult>, WebSearchError> {
    let url = "https://html.duckduckgo.com/html";
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
    ));
    headers.insert(ORIGIN, HeaderValue::from_static("https://duckduckgo.com"));
    headers.insert(REFERER, HeaderValue::from_static("https://duckduckgo.com/"));

    let mut params = HashMap::new();
    params.insert("q", query);

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .headers(headers)
        .query(&params)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(WebSearchError::HttpError(format!(
            "Server returned error code: {}",
            response.status()
        )));
    }

    let html_content = response.text().await?;
    let document = Html::parse_document(&html_content);

    let result_selector = Selector::parse("div.links_main").unwrap();
    let title_selector = Selector::parse("a.result__a").unwrap();
    let url_selector = Selector::parse("a.result__url").unwrap();
    let snippet_selector = Selector::parse("a.result__snippet").unwrap();

    let mut results = Vec::new();
    for result in document.select(&result_selector) {
        let Some(title) = result.select(&title_selector).next() else {
            continue;
        };

        let title_text = title.text().collect::<String>().trim().to_owned();

        let link = result
            .select(&url_selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_owned())
            .unwrap_or_default();

        let snippet = result
            .select(&snippet_selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_owned())
            .unwrap_or_default();

        results.push(WebSearchResult {
            title: title_text,
            link,
            snippet,
        });
    }

    Ok(results)
}
