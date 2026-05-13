use crate::error::ChidoriError;
use reqwest::{
    header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_LENGTH, CONTENT_TYPE, USER_AGENT},
    Response,
};
use std::time::Duration;
use url::Url;

pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (compatible; Chidori/0.1; +https://github.com/taishi/chidori)";
pub const BOT_USER_AGENT: &str =
    "Mozilla/5.0 (compatible; Chidori/0.1; +https://github.com/taishi/chidori) bot";

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
    if !is_supported_html_content_type(&content_type) {
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

    let bytes = read_limited_body(response, config).await?;
    let body = decode_body(&bytes, &content_type);
    Ok(FetchedPage { final_url, body })
}

async fn read_limited_body(
    mut response: Response,
    config: &FetchConfig,
) -> Result<Vec<u8>, ChidoriError> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        if error.is_timeout() {
            ChidoriError::Timeout(config.timeout.as_millis() as u64)
        } else {
            ChidoriError::FetchFailed(error.to_string())
        }
    })? {
        let actual = body.len() as u64 + chunk.len() as u64;
        if actual > config.max_bytes {
            return Err(ChidoriError::TooLarge(actual, config.max_bytes));
        }
        body.extend_from_slice(&chunk);
    }

    Ok(body)
}

fn is_supported_html_content_type(content_type: &str) -> bool {
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();

    matches!(media_type.as_str(), "text/html" | "application/xhtml+xml")
}

pub(crate) fn decode_body(bytes: &[u8], content_type: &str) -> String {
    match charset_from_content_type(content_type)
        .or_else(|| charset_from_meta_tag(bytes))
        .as_deref()
    {
        Some("windows-1252") => decode_windows_1252(bytes),
        Some("iso-8859-1") => bytes.iter().map(|&byte| char::from(byte)).collect(),
        _ => String::from_utf8_lossy(bytes).to_string(),
    }
}

fn charset_from_content_type(content_type: &str) -> Option<String> {
    content_type.split(';').skip(1).find_map(|parameter| {
        let (name, value) = parameter.trim().split_once('=')?;
        if name.trim().eq_ignore_ascii_case("charset") {
            Some(
                value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_ascii_lowercase(),
            )
        } else {
            None
        }
    })
}

fn charset_from_meta_tag(bytes: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
    let charset_index = head.find("charset")?;
    let after_charset = &head[charset_index + "charset".len()..];
    let value_start = after_charset.find('=')? + 1;
    let value = after_charset[value_start..].trim_start();
    let value = value.trim_start_matches(['"', '\'']);
    let charset = value
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .collect::<String>();

    if charset.is_empty() {
        None
    } else {
        Some(charset)
    }
}

fn decode_windows_1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| match byte {
            0x80 => '\u{20AC}',
            0x82 => '\u{201A}',
            0x83 => '\u{0192}',
            0x84 => '\u{201E}',
            0x85 => '\u{2026}',
            0x86 => '\u{2020}',
            0x87 => '\u{2021}',
            0x88 => '\u{02C6}',
            0x89 => '\u{2030}',
            0x8A => '\u{0160}',
            0x8B => '\u{2039}',
            0x8C => '\u{0152}',
            0x8E => '\u{017D}',
            0x91 => '\u{2018}',
            0x92 => '\u{2019}',
            0x93 => '\u{201C}',
            0x94 => '\u{201D}',
            0x95 => '\u{2022}',
            0x96 => '\u{2013}',
            0x97 => '\u{2014}',
            0x98 => '\u{02DC}',
            0x99 => '\u{2122}',
            0x9A => '\u{0161}',
            0x9B => '\u{203A}',
            0x9C => '\u{0153}',
            0x9E => '\u{017E}',
            0x9F => '\u{0178}',
            _ => char::from(byte),
        })
        .collect()
}
