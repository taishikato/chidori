use crate::error::ChidoriError;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_LENGTH, CONTENT_TYPE, USER_AGENT};
use std::time::Duration;
use url::Url;

pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (compatible; Chidori/0.1; +https://github.com/taishi/chidori)";

#[derive(Debug, Clone)]
pub struct FetchConfig {
    pub timeout: Duration,
    pub max_bytes: u64,
    pub user_agent: String,
    pub lang: Option<String>,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(10_000),
            max_bytes: 5 * 1024 * 1024,
            user_agent: DEFAULT_USER_AGENT.to_string(),
            lang: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FetchedPage {
    pub final_url: Url,
    pub body: String,
}

pub async fn fetch_url(url: &Url, config: &FetchConfig) -> Result<FetchedPage, ChidoriError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(config.timeout)
        .build()
        .map_err(|error| ChidoriError::FetchFailed(error.to_string()))?;

    let mut request = client
        .get(url.clone())
        .header(USER_AGENT, config.user_agent.clone())
        .header(ACCEPT, "text/html,application/xhtml+xml");

    if let Some(lang) = &config.lang {
        request = request.header(ACCEPT_LANGUAGE, lang);
    }

    let response = request.send().await.map_err(|error| {
        if error.is_timeout() {
            ChidoriError::Timeout(config.timeout.as_millis() as u64)
        } else {
            ChidoriError::FetchFailed(error.to_string())
        }
    })?;

    let final_url = response.url().clone();
    if !response.status().is_success() {
        return Err(ChidoriError::FetchFailed(format!(
            "{} {}",
            response.status().as_u16(),
            response.status().canonical_reason().unwrap_or("")
        )));
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !content_type.contains("text/html") && !content_type.contains("application/xhtml+xml") {
        return Err(ChidoriError::UnsupportedContentType(content_type));
    }

    if let Some(length) = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
    {
        if let Ok(bytes) = length.parse::<u64>() {
            if bytes > config.max_bytes {
                return Err(ChidoriError::TooLarge(bytes, config.max_bytes));
            }
        }
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| ChidoriError::FetchFailed(error.to_string()))?;
    if bytes.len() as u64 > config.max_bytes {
        return Err(ChidoriError::TooLarge(bytes.len() as u64, config.max_bytes));
    }

    let body = String::from_utf8_lossy(&bytes).to_string();
    Ok(FetchedPage { final_url, body })
}
