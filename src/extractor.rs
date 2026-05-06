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
];

pub fn extract_main_html(doc: &ParsedDocument) -> Result<String, ChidoriError> {
    let link_selector =
        Selector::parse("a").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let mut best_candidate = best_candidate_for_selectors(doc, ENTRY_SELECTORS, &link_selector)?;

    if best_candidate.is_none() {
        best_candidate = best_candidate_for_selectors(doc, &["body"], &link_selector)?;
    }

    best_candidate
        .map(|(_, html)| html)
        .ok_or(ChidoriError::ExtractionFailed)
}

fn best_candidate_for_selectors(
    doc: &ParsedDocument,
    raw_selectors: &[&str],
    link_selector: &Selector,
) -> Result<Option<(isize, String)>, ChidoriError> {
    let mut best_candidate: Option<(isize, String)> = None;

    for raw_selector in raw_selectors {
        let selector = Selector::parse(raw_selector)
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
        for element in doc.dom.select(&selector) {
            let text = element.text().collect::<Vec<_>>().join(" ");
            let word_count = text.split_whitespace().count();
            if word_count == 0 {
                continue;
            }
            let link_count = element.select(link_selector).count();
            let score = word_count as isize - (link_count * 3) as isize;
            let html = element.html();
            if best_candidate
                .as_ref()
                .is_none_or(|(best_score, _)| score > *best_score)
            {
                best_candidate = Some((score, html));
            }
        }
    }

    Ok(best_candidate)
}
