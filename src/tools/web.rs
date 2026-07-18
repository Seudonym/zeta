use color_eyre::eyre::Result;
use rig::tool::ToolDyn;
use serde::{Deserialize, Serialize};
use std::env::VarError;
use thiserror::Error;

pub fn toolset() -> Vec<Box<dyn ToolDyn>> {
    vec![Box::new(WebSearch)]
}

#[derive(Debug, Error)]
pub enum WebSearchError {
    #[error("Tavily API key is invalid")]
    ApiKeyError(#[from] VarError),

    #[error("Failed to send request")]
    RequestError(#[from] reqwest::Error),

    #[error("Response was not a 200 OK")]
    HttpError(String),
}

#[derive(Serialize)]
pub struct TavilyRequest {
    query: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TavilySearchResult {
    title: String,
    url: String,
    content: String,
    score: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TavilyResponse {
    query: String,
    answer: Option<String>,
    results: Vec<TavilySearchResult>,
}

#[rig::tool_macro(
    description = "Search the web for a query",
    params(query = "The query to search for")
)]
pub async fn web_search(query: String) -> Result<TavilyResponse, WebSearchError> {
    let url = "https://api.tavily.com/search";
    let api_key = std::env::var("TAVILY_API_KEY")?;

    let payload = TavilyRequest { query };

    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(WebSearchError::HttpError(format!(
            "Server returned error code: {}",
            response.status()
        )));
    }

    let parsed_response: TavilyResponse = response.json().await?;

    Ok(parsed_response)
}
