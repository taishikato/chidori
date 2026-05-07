use crate::{document::ParsedDocument, error::ChidoriError};
use scraper::Selector;

const PRIMARY_ENTRY_SELECTORS: &[&str] = &[
    "#post",
    ".post-content",
    ".post-body",
    ".article-content",
    "#article-content",
    ".js-article-content",
    ".article_post",
    ".article-wrapper",
    ".entry-content",
    ".content-article",
    ".instapaper_body",
    ".post",
    ".markdown-body",
    "article",
    "[role=\"article\"]",
    "main",
    "[role=\"main\"]",
    ".article-body",
    "#content",
];

const BODY_FALLBACK_SELECTORS: &[&str] = &["body"];

#[derive(Debug, Clone)]
struct Candidate {
    score: isize,
    selector_index: usize,
    word_count: usize,
    html: String,
}

fn selector_priority(selector_count: usize, selector_index: usize) -> isize {
    ((selector_count - selector_index) * 40) as isize
}

pub fn extract_main_html(doc: &ParsedDocument) -> Result<String, ChidoriError> {
    let link_selector =
        Selector::parse("a").map_err(|error| ChidoriError::Unknown(error.to_string()))?;

    let candidate =
        match best_candidate_for_selectors(doc, PRIMARY_ENTRY_SELECTORS, &link_selector)? {
            Some(candidate) => Some(candidate),
            None => best_candidate_for_selectors(doc, BODY_FALLBACK_SELECTORS, &link_selector)?,
        };

    candidate
        .map(|candidate| candidate.html)
        .ok_or(ChidoriError::ExtractionFailed)
}

fn best_candidate_for_selectors(
    doc: &ParsedDocument,
    raw_selectors: &[&str],
    link_selector: &Selector,
) -> Result<Option<Candidate>, ChidoriError> {
    let mut best_candidate: Option<Candidate> = None;

    for (selector_index, raw_selector) in raw_selectors.iter().enumerate() {
        let selector = Selector::parse(raw_selector)
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
        for element in doc.dom.select(&selector) {
            let text = element.text().collect::<Vec<_>>().join(" ");
            let word_count = text.split_whitespace().count();
            if word_count == 0 {
                continue;
            }
            let link_count = element.select(link_selector).count();
            let score = selector_priority(raw_selectors.len(), selector_index)
                + word_count as isize
                - (link_count * 3) as isize;
            let candidate = Candidate {
                score,
                selector_index,
                word_count,
                html: element.html(),
            };
            if best_candidate.as_ref().is_none_or(|best_candidate| {
                candidate.score > best_candidate.score
                    || (candidate.score == best_candidate.score
                        && candidate.selector_index < best_candidate.selector_index)
                    || (candidate.score == best_candidate.score
                        && candidate.selector_index == best_candidate.selector_index
                        && candidate.word_count > best_candidate.word_count)
            }) {
                best_candidate = Some(candidate);
            }
        }
    }

    Ok(best_candidate)
}
