use crate::{document::ParsedDocument, error::ChidoriError};
use html_escape::{encode_double_quoted_attribute, encode_text};
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
    ".js-discussion",
    ".pull-discussion-timeline",
    "#article-block",
    "#section-content",
    ".markdown-body",
    "article",
    "[role=\"article\"]",
    "main",
    "[role=\"main\"]",
    ".article-body",
    "#content",
    ".article",
    ".content-paragraph",
];

const BODY_FALLBACK_SELECTORS: &[&str] = &["body"];
const LOW_WORD_COUNT_RETRY_THRESHOLD: usize = 50;
const RETRY_MIN_GAIN_MULTIPLIER: usize = 2;
const ARTICLE_RETRY_PROTECTION_MIN_WORDS: usize = 10;

#[derive(Debug, Clone)]
struct Candidate {
    score: isize,
    selector_index: usize,
    word_count: usize,
    content_block_count: usize,
    html: String,
}

struct ScoringSelectors {
    links: Selector,
    paragraphs: Selector,
    images: Selector,
    body_content_blocks: Selector,
}

fn selector_priority(selector_count: usize, selector_index: usize) -> isize {
    ((selector_count - selector_index) * 40) as isize
}

pub fn extract_main_html(doc: &ParsedDocument) -> Result<String, ChidoriError> {
    if let Some(html) = hacker_news_listing_candidate(doc)? {
        return Ok(html);
    }

    let selectors = ScoringSelectors {
        links: Selector::parse("a").map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        paragraphs: Selector::parse("p")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        images: Selector::parse("img").map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        body_content_blocks: Selector::parse(
            "body > article, body > main, body > section, body > div",
        )
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
    };

    let mut best_candidate =
        best_candidate_for_selectors(doc, PRIMARY_ENTRY_SELECTORS, &selectors)?;

    if best_candidate.is_none() {
        best_candidate = best_candidate_for_selectors(doc, BODY_FALLBACK_SELECTORS, &selectors)?;
    }

    let mut used_structured_content = false;
    if let Some(candidate) = best_candidate.as_ref() {
        if let Some(html) = structured_content_candidate(doc, candidate.word_count)? {
            used_structured_content = true;
            best_candidate = Some(Candidate {
                score: candidate.score,
                selector_index: candidate.selector_index,
                word_count: text_word_count(&html),
                content_block_count: candidate.content_block_count,
                html,
            });
        }
    }

    if !used_structured_content {
        if let Some(candidate) = best_candidate
            .as_ref()
            .filter(|candidate| candidate.word_count < LOW_WORD_COUNT_RETRY_THRESHOLD)
        {
            if let Some(body_candidate) =
                best_candidate_for_selectors(doc, BODY_FALLBACK_SELECTORS, &selectors)?
            {
                if should_retry_with_body(candidate, &body_candidate) {
                    best_candidate = Some(body_candidate);
                }
            }
        }
    }

    if let Some(candidate) = best_candidate {
        Ok(candidate.html)
    } else if let Some(html) = structured_content_candidate(doc, 0)? {
        Ok(html)
    } else {
        Err(ChidoriError::ExtractionFailed)
    }
}

fn hacker_news_listing_candidate(doc: &ParsedDocument) -> Result<Option<String>, ChidoriError> {
    if doc.url.host_str() != Some("news.ycombinator.com") {
        return Ok(None);
    }

    let row_selector =
        Selector::parse("tr.athing").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let title_selector = Selector::parse(".titleline a")
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let site_selector =
        Selector::parse(".sitestr").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let subtext_selector =
        Selector::parse("td.subtext").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let score_selector =
        Selector::parse(".score").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let user_selector =
        Selector::parse(".hnuser").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let age_selector =
        Selector::parse(".age a").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let link_selector =
        Selector::parse("a").map_err(|error| ChidoriError::Unknown(error.to_string()))?;

    let story_rows: Vec<_> = doc.dom.select(&row_selector).collect();
    if story_rows.is_empty() {
        return Ok(None);
    }

    let subtext_cells: Vec<_> = doc.dom.select(&subtext_selector).collect();
    let mut output = String::from("<ol class=\"chidori-hn-listing\">");
    let mut story_count = 0;

    for (index, row) in story_rows.into_iter().enumerate() {
        let Some(title_link) = row.select(&title_selector).next() else {
            continue;
        };
        let title = element_text(title_link);
        if title.is_empty() {
            continue;
        }

        let href = title_link.value().attr("href").unwrap_or("");
        let story_url = resolve_url(doc, href);
        let site = row.select(&site_selector).next().map(element_text);
        let subtext = subtext_cells.get(index).copied();

        output.push_str("<li>");
        push_link(&mut output, &story_url, &title);
        if let Some(site) = site.filter(|site| !site.is_empty()) {
            output.push_str(" <span class=\"site\">(");
            output.push_str(&encode_text(&site));
            output.push_str(")</span>");
        }

        let mut meta_parts = Vec::new();
        if let Some(subtext) = subtext {
            if let Some(score) = subtext
                .select(&score_selector)
                .next()
                .map(element_text)
                .filter(|score| !score.is_empty())
            {
                meta_parts.push(encode_text(&score).to_string());
            }
            if let Some(user) = subtext
                .select(&user_selector)
                .next()
                .map(element_text)
                .filter(|user| !user.is_empty())
            {
                meta_parts.push(format!("by {}", encode_text(&user)));
            }
            if let Some(age) = subtext
                .select(&age_selector)
                .next()
                .map(element_text)
                .filter(|age| !age.is_empty())
            {
                meta_parts.push(encode_text(&age).to_string());
            }
            if let Some((comments_url, comments_text)) = comments_link(doc, subtext, &link_selector)
            {
                let mut comments = String::new();
                push_link(&mut comments, &comments_url, &comments_text);
                meta_parts.push(comments);
            }
        }

        if !meta_parts.is_empty() {
            output.push_str("<br><small>");
            output.push_str(&meta_parts.join(" · "));
            output.push_str("</small>");
        }
        output.push_str("</li>");
        story_count += 1;
    }

    output.push_str("</ol>");

    if story_count == 0 {
        Ok(None)
    } else {
        Ok(Some(output))
    }
}

fn comments_link(
    doc: &ParsedDocument,
    subtext: ElementRef<'_>,
    link_selector: &Selector,
) -> Option<(String, String)> {
    subtext
        .select(link_selector)
        .filter_map(|link| {
            let text = element_text(link);
            let href = link.value().attr("href")?;
            let is_comment_link = text == "discuss" || text.contains("comment");
            is_comment_link.then(|| (resolve_url(doc, href), text))
        })
        .last()
}

fn push_link(output: &mut String, url: &str, text: &str) {
    output.push_str("<a href=\"");
    output.push_str(&encode_double_quoted_attribute(url));
    output.push_str("\">");
    output.push_str(&encode_text(text));
    output.push_str("</a>");
}

fn resolve_url(doc: &ParsedDocument, href: &str) -> String {
    doc.url
        .join(href)
        .map(|url| url.to_string())
        .unwrap_or_else(|_| href.to_string())
}

fn element_text(element: ElementRef<'_>) -> String {
    normalize_text(&element.text().collect::<Vec<_>>().join(" "))
}

fn structured_content_candidate(
    doc: &ParsedDocument,
    current_word_count: usize,
) -> Result<Option<String>, ChidoriError> {
    let Some(text) = crate::metadata::structured_content_text(doc) else {
        return Ok(None);
    };
    let structured_word_count = text.split_whitespace().count();
    if structured_word_count == 0 || structured_word_count * 2 <= current_word_count * 3 {
        return Ok(None);
    }

    if let Some(html) = smallest_element_containing_text(doc, &text)? {
        return Ok(Some(html));
    }

    Ok(Some(encode_text(&text).to_string()))
}

fn smallest_element_containing_text(
    doc: &ParsedDocument,
    target_text: &str,
) -> Result<Option<String>, ChidoriError> {
    let selector =
        Selector::parse("body *").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let target = normalize_text(target_text);

    Ok(doc
        .dom
        .select(&selector)
        .filter(|element| !matches!(element.value().name(), "script" | "style" | "noscript"))
        .filter_map(|element| {
            let text = element.text().collect::<Vec<_>>().join(" ");
            let normalized = normalize_text(&text);
            normalized
                .contains(&target)
                .then(|| (normalized.len(), element.html()))
        })
        .min_by_key(|(len, _html)| *len)
        .map(|(_len, html)| html))
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text_word_count(html: &str) -> usize {
    scraper::Html::parse_fragment(html)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .count()
}

fn is_protected_article_candidate(candidate: &Candidate) -> bool {
    if candidate.word_count < ARTICLE_RETRY_PROTECTION_MIN_WORDS {
        return false;
    }

    let html = candidate.html.to_ascii_lowercase();
    html.contains("<article") || html.contains("role=\"article\"")
}

fn should_retry_with_body(candidate: &Candidate, body_candidate: &Candidate) -> bool {
    if body_candidate.word_count <= candidate.word_count * RETRY_MIN_GAIN_MULTIPLIER {
        return false;
    }

    if !is_protected_article_candidate(candidate) {
        return true;
    }

    body_candidate.content_block_count > 1
}

fn score_element(
    element: ElementRef<'_>,
    selectors: &ScoringSelectors,
) -> (isize, usize, usize, usize) {
    let text = text_without_invisible_nodes(&element.html());
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
    let cleaned =
        crate::cleaner::clean_html(html, &crate::cleaner::CleanOptions { no_images: false });

    scraper::Html::parse_fragment(&cleaned)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
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
            let (content_score, word_count, _paragraph_count, content_block_count) =
                score_element(element, selectors);
            if word_count == 0 {
                continue;
            }
            let score = selector_priority(raw_selectors.len(), selector_index) + content_score;
            let candidate = Candidate {
                score,
                selector_index,
                word_count,
                content_block_count,
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
            body_content_blocks: Selector::parse(
                "body > article, body > main, body > section, body > div",
            )
            .unwrap(),
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
