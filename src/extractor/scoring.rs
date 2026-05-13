use crate::{document::ParsedDocument, error::ChidoriError};
use scraper::{ElementRef, Selector};

use super::types::{
    Candidate, CandidateDecision, ContentCandidateDiagnostic, ExtractionDiagnostics,
};

const RETRY_MIN_GAIN_MULTIPLIER: usize = 2;
const ARTICLE_RETRY_PROTECTION_MIN_WORDS: usize = 10;

pub(crate) struct ScoringSelectors {
    links: Selector,
    paragraphs: Selector,
    images: Selector,
    body_content_blocks: Selector,
}
impl ScoringSelectors {
    pub(crate) fn new() -> Result<Self, ChidoriError> {
        Ok(Self {
            links: Selector::parse("a")
                .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
            paragraphs: Selector::parse("p")
                .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
            images: Selector::parse("img")
                .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
            body_content_blocks: Selector::parse(
                "body > article, body > main, body > section, body > div",
            )
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        })
    }
}

fn selector_priority(selector_count: usize, selector_index: usize) -> isize {
    ((selector_count - selector_index) * 40) as isize
}
fn is_protected_article_candidate(candidate: &Candidate) -> bool {
    if candidate.word_count < ARTICLE_RETRY_PROTECTION_MIN_WORDS {
        return false;
    }

    let html = candidate.html.to_ascii_lowercase();
    html.contains("<article") || html.contains("role=\"article\"")
}

pub(crate) fn should_retry_with_body(candidate: &Candidate, body_candidate: &Candidate) -> bool {
    if body_candidate.word_count <= candidate.word_count * RETRY_MIN_GAIN_MULTIPLIER {
        return false;
    }

    if !is_protected_article_candidate(candidate) {
        return true;
    }

    body_candidate.content_block_count > 1
}

#[cfg(test)]
fn score_element(
    element: ElementRef<'_>,
    selectors: &ScoringSelectors,
) -> (isize, usize, usize, usize) {
    score_element_with_visibility(element, selectors, false)
}

fn score_element_with_visibility(
    element: ElementRef<'_>,
    selectors: &ScoringSelectors,
    include_hidden: bool,
) -> (isize, usize, usize, usize) {
    let text = if include_hidden {
        element.text().collect::<Vec<_>>().join(" ")
    } else {
        text_without_invisible_nodes(&element.html())
    };
    let word_count = text.split_whitespace().count();
    let paragraph_count = element.select(&selectors.paragraphs).count();
    let content_block_count = element.select(&selectors.body_content_blocks).count();
    if word_count == 0 {
        return (0, 0, paragraph_count, content_block_count);
    }

    let comma_count = text.matches(',').count();
    let image_count = element.select(&selectors.images).count();
    let mut score = word_count as isize;
    score += (paragraph_count * 10) as isize;
    score += comma_count as isize;

    let class_attr = element
        .value()
        .attr("class")
        .unwrap_or("")
        .to_ascii_lowercase();
    let id_attr = element
        .value()
        .attr("id")
        .unwrap_or("")
        .to_ascii_lowercase();
    if class_attr.contains("content")
        || class_attr.contains("article")
        || class_attr.contains("post")
        || id_attr.contains("content")
        || id_attr.contains("article")
        || id_attr.contains("post")
    {
        score += 15;
    }

    if text.contains(" by ") || text.contains("By ") {
        score += 10;
    }
    if text.contains("202")
        || text.contains("Jan ")
        || text.contains("Feb ")
        || text.contains("Mar ")
    {
        score += 10;
    }

    score -= (image_count * 3) as isize;

    let link_text_len: usize = element
        .select(&selectors.links)
        .map(|link| link.text().collect::<Vec<_>>().join(" ").len())
        .sum();
    let text_len = text.len().max(1);
    let link_density = (link_text_len as f64 / text_len as f64).min(0.5);
    score = ((score as f64) * (1.0 - link_density)).round() as isize;

    (score, word_count, paragraph_count, content_block_count)
}

fn text_without_invisible_nodes(html: &str) -> String {
    let cleaned = crate::cleaner::clean_html(html, &crate::cleaner::CleanOptions::new(false));

    scraper::Html::parse_fragment(&cleaned)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn best_candidate_for_selectors(
    doc: &ParsedDocument,
    raw_selectors: &[&str],
    selectors: &ScoringSelectors,
    include_hidden: bool,
    diagnostic_pass: &str,
    next_candidate_id: &mut usize,
) -> Result<(Option<Candidate>, Vec<Candidate>), ChidoriError> {
    let mut best_candidate: Option<Candidate> = None;
    let mut candidates = Vec::new();

    for (selector_index, raw_selector) in raw_selectors.iter().enumerate() {
        let selector = Selector::parse(raw_selector)
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
        for element in doc.dom.select(&selector) {
            let (content_score, word_count, _paragraph_count, content_block_count) =
                score_element_with_visibility(element, selectors, include_hidden);
            if word_count == 0 {
                continue;
            }
            let score = selector_priority(raw_selectors.len(), selector_index) + content_score;
            let candidate = Candidate {
                diagnostic_id: *next_candidate_id,
                diagnostic_pass: diagnostic_pass.to_string(),
                score,
                selector_index,
                selector: (*raw_selector).to_string(),
                word_count,
                content_block_count,
                html: String::new(),
            };
            *next_candidate_id += 1;
            if best_candidate.as_ref().is_none_or(|best_candidate| {
                candidate.score > best_candidate.score
                    || (candidate.score == best_candidate.score
                        && candidate.selector_index < best_candidate.selector_index)
                    || (candidate.score == best_candidate.score
                        && candidate.selector_index == best_candidate.selector_index
                        && candidate.word_count > best_candidate.word_count)
            }) {
                best_candidate = Some(Candidate {
                    html: element.html(),
                    ..candidate.clone()
                });
            }
            candidates.push(candidate);
        }
    }

    Ok((best_candidate, candidates))
}

pub(crate) fn push_candidate_diagnostics(
    diagnostics: &mut ExtractionDiagnostics,
    candidate_sets: &[Vec<Candidate>],
    selected_id: Option<usize>,
) {
    for candidate in candidate_sets.iter().flatten() {
        diagnostics.candidates.push(ContentCandidateDiagnostic {
            id: candidate.diagnostic_id,
            pass: candidate.diagnostic_pass.clone(),
            selector: candidate.selector.clone(),
            score: candidate.score,
            word_count: candidate.word_count,
            content_block_count: candidate.content_block_count,
            decision: if Some(candidate.diagnostic_id) == selected_id {
                CandidateDecision::Selected
            } else {
                CandidateDecision::Rejected
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ParsedDocument;
    use scraper::Html;
    use url::Url;

    fn scoring_selectors() -> ScoringSelectors {
        ScoringSelectors::new().unwrap()
    }

    fn score_first(html: &str, selector: &str) -> isize {
        let dom = Html::parse_document(html);
        let selector = Selector::parse(selector).unwrap();
        let element = dom.select(&selector).next().unwrap();
        score_element(element, &scoring_selectors()).0
    }

    fn parse_doc(html: &str, url: &str) -> ParsedDocument {
        ParsedDocument::parse(html.to_string(), Url::parse(url).unwrap())
    }

    #[test]
    fn score_element_rewards_paragraph_density() {
        let paragraph_score = score_first(
            r#"<section><p>One useful paragraph with clear prose.</p><p>Another useful paragraph with detail.</p></section>"#,
            "section",
        );
        let loose_score = score_first(
            r#"<section>Loose words words words words words words words words words words words.</section>"#,
            "section",
        );

        assert!(paragraph_score > loose_score);
    }

    #[test]
    fn score_element_rewards_content_class_names() {
        let content_score = score_first(
            r#"<section class="article-content">Short focused article text.</section>"#,
            "section",
        );
        let plain_score = score_first(
            r#"<section>Short focused article text.</section>"#,
            "section",
        );

        assert!(content_score > plain_score);
    }

    #[test]
    fn score_element_penalizes_link_text_density() {
        let link_score = score_first(
            r#"<section><a href="/one">Alpha beta gamma, delta epsilon zeta, eta theta.</a></section>"#,
            "section",
        );
        let prose_score = score_first(
            r#"<section>Alpha beta gamma, delta epsilon zeta, eta theta.</section>"#,
            "section",
        );

        assert!(prose_score > link_score);
    }

    #[test]
    fn candidate_diagnostics_do_not_retain_html_fragments() {
        let doc = parse_doc(
            r#"<html><body>
              <article><h1>First</h1><p>Short article text.</p></article>
              <article><h1>Second</h1><p>This article has enough extra prose to be selected over the first candidate.</p></article>
            </body></html>"#,
            "https://example.com/post",
        );
        let mut next_candidate_id = 0;

        let (best_candidate, diagnostics) = best_candidate_for_selectors(
            &doc,
            &["article"],
            &scoring_selectors(),
            false,
            "primary",
            &mut next_candidate_id,
        )
        .unwrap();

        assert!(
            best_candidate.unwrap().html.contains("Second"),
            "selected candidate still needs its HTML"
        );
        assert_eq!(
            diagnostics
                .iter()
                .map(|candidate| candidate.html.len())
                .sum::<usize>(),
            0,
            "diagnostic candidates should only retain scalar fields"
        );
    }
}
