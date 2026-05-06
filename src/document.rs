use scraper::Html;
use url::Url;

#[derive(Debug)]
pub struct ParsedDocument {
    pub html: String,
    pub url: Url,
    pub dom: Html,
}

impl ParsedDocument {
    pub fn parse(html: impl Into<String>, url: Url) -> Self {
        let html = html.into();
        let dom = Html::parse_document(&html);
        Self { html, url, dom }
    }
}
