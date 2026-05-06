use crate::document::ParsedDocument;
use scraper::Selector;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub url: String,
    pub final_url: String,
    pub title: String,
    pub description: String,
    pub site: String,
    pub author: String,
    pub published: String,
    pub language: String,
    pub word_count: usize,
}

pub fn extract_metadata(doc: &ParsedDocument) -> Metadata {
    Metadata {
        url: doc.url.to_string(),
        final_url: doc.url.to_string(),
        title: title(doc),
        description: meta(doc, "name", "description")
            .or_else(|| meta(doc, "property", "og:description"))
            .unwrap_or_default(),
        site: meta(doc, "property", "og:site_name").unwrap_or_default(),
        author: meta(doc, "name", "author")
            .or_else(|| meta(doc, "property", "article:author"))
            .unwrap_or_default(),
        published: meta(doc, "property", "article:published_time")
            .or_else(|| meta(doc, "name", "date"))
            .unwrap_or_default(),
        language: html_lang(doc),
        word_count: 0,
    }
}

fn title(doc: &ParsedDocument) -> String {
    let selector = Selector::parse("title").unwrap();
    doc.dom
        .select(&selector)
        .next()
        .map(|node| node.text().collect::<Vec<_>>().join("").trim().to_string())
        .unwrap_or_default()
}

fn html_lang(doc: &ParsedDocument) -> String {
    let selector = Selector::parse("html").unwrap();
    doc.dom
        .select(&selector)
        .next()
        .and_then(|node| node.value().attr("lang"))
        .unwrap_or("")
        .to_string()
}

fn meta(doc: &ParsedDocument, attr: &str, value: &str) -> Option<String> {
    let selector = Selector::parse(&format!(r#"meta[{}="{}"]"#, attr, value)).ok()?;
    doc.dom
        .select(&selector)
        .next()
        .and_then(|node| node.value().attr("content"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
