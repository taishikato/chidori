use crate::{document::ParsedDocument, error::ChidoriError};
use scraper::Selector;

const ENTRY_SELECTORS: &[&str] = &[
    "article",
    "[role=\"article\"]",
    "main",
    "[role=\"main\"]",
    ".markdown-body",
    ".post-content",
    ".entry-content",
    ".article-content",
    "#content",
    "body",
];

pub fn extract_main_html(doc: &ParsedDocument) -> Result<String, ChidoriError> {
    let mut best_html = String::new();
    let mut best_score = 0usize;

    for raw_selector in ENTRY_SELECTORS {
        let selector = Selector::parse(raw_selector)
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
        for element in doc.dom.select(&selector) {
            let text = element.text().collect::<Vec<_>>().join(" ");
            let link_count = element.select(&Selector::parse("a").unwrap()).count();
            let score = text.split_whitespace().count().saturating_sub(link_count * 3);
            if score > best_score {
                best_score = score;
                best_html = element.html();
            }
        }
    }

    if best_html.trim().is_empty() {
        Err(ChidoriError::ExtractionFailed)
    } else {
        Ok(best_html)
    }
}
