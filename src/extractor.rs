use crate::{document::ParsedDocument, error::ChidoriError};
use scraper::{ElementRef, Selector};

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

struct ScoringSelectors {
    links: Selector,
    paragraphs: Selector,
    images: Selector,
}

fn selector_priority(selector_count: usize, selector_index: usize) -> isize {
    ((selector_count - selector_index) * 40) as isize
}

pub fn extract_main_html(doc: &ParsedDocument) -> Result<String, ChidoriError> {
    let selectors = ScoringSelectors {
        links: Selector::parse("a").map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        paragraphs: Selector::parse("p")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        images: Selector::parse("img").map_err(|error| ChidoriError::Unknown(error.to_string()))?,
    };

    let candidate = match best_candidate_for_selectors(doc, PRIMARY_ENTRY_SELECTORS, &selectors)? {
        Some(candidate) => Some(candidate),
        None => best_candidate_for_selectors(doc, BODY_FALLBACK_SELECTORS, &selectors)?,
    };

    candidate
        .map(|candidate| candidate.html)
        .ok_or(ChidoriError::ExtractionFailed)
}

fn score_element(element: ElementRef<'_>, selectors: &ScoringSelectors) -> (isize, usize) {
    let text = element.text().collect::<Vec<_>>().join(" ");
    let word_count = text.split_whitespace().count();
    if word_count == 0 {
        return (0, 0);
    }

    let paragraph_count = element.select(&selectors.paragraphs).count();
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

    (score, word_count)
}

fn best_candidate_for_selectors(
    doc: &ParsedDocument,
    raw_selectors: &[&str],
    selectors: &ScoringSelectors,
) -> Result<Option<Candidate>, ChidoriError> {
    let mut best_candidate: Option<Candidate> = None;

    for (selector_index, raw_selector) in raw_selectors.iter().enumerate() {
        let selector = Selector::parse(raw_selector)
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
        for element in doc.dom.select(&selector) {
            let (content_score, word_count) = score_element(element, selectors);
            if word_count == 0 {
                continue;
            }
            let score = selector_priority(raw_selectors.len(), selector_index) + content_score;
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

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    fn scoring_selectors() -> ScoringSelectors {
        ScoringSelectors {
            links: Selector::parse("a").unwrap(),
            paragraphs: Selector::parse("p").unwrap(),
            images: Selector::parse("img").unwrap(),
        }
    }

    fn score_first(html: &str, selector: &str) -> isize {
        let dom = Html::parse_document(html);
        let selector = Selector::parse(selector).unwrap();
        let element = dom.select(&selector).next().unwrap();
        score_element(element, &scoring_selectors()).0
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
}
