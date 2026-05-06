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
    let mut best_candidate: Option<(isize, String)> = None;

    for raw_selector in ENTRY_SELECTORS {
        let selector = Selector::parse(raw_selector)
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
        for element in doc.dom.select(&selector) {
            let text = element.text().collect::<Vec<_>>().join(" ");
            let link_count = element.select(&Selector::parse("a").unwrap()).count();
            let score = text.split_whitespace().count() as isize - (link_count * 3) as isize;
            let html = element.html();
            if html.trim().is_empty() {
                continue;
            }
            if best_candidate
                .as_ref()
                .is_none_or(|(best_score, _)| score > *best_score)
            {
                best_candidate = Some((score, html));
            }
        }
    }

    best_candidate
        .map(|(_, html)| html)
        .ok_or(ChidoriError::ExtractionFailed)
}
