use crate::{document::ParsedDocument, error::ChidoriError};
use html_escape::encode_text;
use scraper::{ElementRef, Selector};

mod schema;
mod scoring;
mod sites;
mod types;
mod util;

use schema::structured_content_candidate;
use scoring::{
    best_candidate_for_selectors, push_candidate_diagnostics, should_retry_with_body,
    ScoringSelectors,
};
use types::Candidate;
use util::{element_text, push_link, resolve_url, text_word_count};

pub use types::{
    CandidateDecision, ContentCandidateDiagnostic, ExtractedContent, ExtractionDiagnostics,
    FallbackAttemptDiagnostic,
};

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
const BROAD_RETRY_SELECTORS: &[&str] = &["main", "[role=\"main\"]", "article", "body"];
const LOW_WORD_COUNT_RETRY_THRESHOLD: usize = 50;

pub fn extract_main_html(doc: &ParsedDocument) -> Result<String, ChidoriError> {
    extract_main_content(doc).map(|content| content.html)
}

pub fn extract_main_content(doc: &ParsedDocument) -> Result<ExtractedContent, ChidoriError> {
    let mut diagnostics = ExtractionDiagnostics::default();
    extract_main_content_inner(doc, &mut diagnostics)
}

fn extract_main_content_inner(
    doc: &ParsedDocument,
    diagnostics: &mut ExtractionDiagnostics,
) -> Result<ExtractedContent, ChidoriError> {
    if let Some(extraction) = sites::extract_site_content(doc)? {
        return Ok(ExtractedContent {
            html: extraction.html,
            selector: Some(extraction.selector),
            score: None,
            fallbacks: Vec::new(),
            diagnostics: diagnostics.clone(),
        });
    }

    if let Some(html) = hacker_news_listing_candidate(doc)? {
        return Ok(ExtractedContent {
            html,
            selector: Some("hacker-news-listing".to_string()),
            score: None,
            fallbacks: Vec::new(),
            diagnostics: diagnostics.clone(),
        });
    }

    let selectors = ScoringSelectors::new()?;

    let mut fallback_steps = Vec::new();
    let mut candidate_diagnostics = Vec::new();
    let mut next_candidate_id = 0;
    let (mut best_candidate, primary_candidates) = best_candidate_for_selectors(
        doc,
        PRIMARY_ENTRY_SELECTORS,
        &selectors,
        false,
        "primary",
        &mut next_candidate_id,
    )?;
    candidate_diagnostics.push(primary_candidates);

    if best_candidate
        .as_ref()
        .is_none_or(|candidate| candidate.word_count < LOW_WORD_COUNT_RETRY_THRESHOLD)
    {
        let (hidden_candidate, hidden_candidates) = best_candidate_for_selectors(
            doc,
            PRIMARY_ENTRY_SELECTORS,
            &selectors,
            true,
            "hidden",
            &mut next_candidate_id,
        )?;
        candidate_diagnostics.push(hidden_candidates);
        if let Some(hidden_candidate) = hidden_candidate {
            let previous_word_count = best_candidate
                .as_ref()
                .map_or(0, |candidate| candidate.word_count);
            let hidden_word_count = hidden_candidate.word_count;
            let use_hidden = best_candidate
                .as_ref()
                .is_none_or(|candidate| should_retry_with_body(candidate, &hidden_candidate));
            if use_hidden {
                best_candidate = Some(hidden_candidate);
            }
            diagnostics
                .fallback_attempts
                .push(FallbackAttemptDiagnostic {
                    name: "hidden-content".to_string(),
                    accepted: use_hidden,
                    previous_word_count,
                    candidate_word_count: hidden_word_count,
                    reason: if use_hidden {
                        "hidden candidate was more complete".to_string()
                    } else {
                        "hidden candidate did not improve enough".to_string()
                    },
                });
            if use_hidden {
                fallback_steps.push("hidden-content".to_string());
            }
        }
    }

    if best_candidate.is_none() {
        let (body_candidate, body_candidates) = best_candidate_for_selectors(
            doc,
            BODY_FALLBACK_SELECTORS,
            &selectors,
            false,
            "body-fallback",
            &mut next_candidate_id,
        )?;
        candidate_diagnostics.push(body_candidates);
        best_candidate = body_candidate;
    }

    let mut used_structured_content = false;
    if let Some(candidate) = best_candidate.as_ref() {
        if let Some(html) = structured_content_candidate(doc, candidate.word_count)? {
            used_structured_content = true;
            let word_count = text_word_count(&html);
            diagnostics
                .fallback_attempts
                .push(FallbackAttemptDiagnostic {
                    name: "schema-org".to_string(),
                    accepted: true,
                    previous_word_count: candidate.word_count,
                    candidate_word_count: word_count,
                    reason: "structured content was more complete".to_string(),
                });
            let schema_candidate = Candidate {
                diagnostic_id: next_candidate_id,
                diagnostic_pass: "schema-org".to_string(),
                score: candidate.score,
                selector_index: candidate.selector_index,
                selector: "schema-org".to_string(),
                word_count,
                content_block_count: candidate.content_block_count,
                html,
            };
            next_candidate_id += 1;
            candidate_diagnostics.push(vec![schema_candidate.diagnostic_record()]);
            best_candidate = Some(schema_candidate);
        }
    }

    if !used_structured_content {
        if let Some(candidate) = best_candidate
            .as_ref()
            .filter(|candidate| candidate.word_count < LOW_WORD_COUNT_RETRY_THRESHOLD)
            .cloned()
        {
            let (broad_retry_candidate, broad_candidates) = best_candidate_for_selectors(
                doc,
                BROAD_RETRY_SELECTORS,
                &selectors,
                false,
                "broad-retry",
                &mut next_candidate_id,
            )?;
            candidate_diagnostics.push(broad_candidates);
            let broad_retry_candidate = broad_retry_candidate
                .filter(|broad_candidate| should_retry_with_body(&candidate, broad_candidate));
            let (body_retry_candidate, body_candidates) = best_candidate_for_selectors(
                doc,
                BODY_FALLBACK_SELECTORS,
                &selectors,
                false,
                "body-retry",
                &mut next_candidate_id,
            )?;
            candidate_diagnostics.push(body_candidates);
            let body_retry_candidate = body_retry_candidate
                .filter(|body_candidate| should_retry_with_body(&candidate, body_candidate));

            let retry_candidate = match (broad_retry_candidate, body_retry_candidate) {
                (Some(broad_candidate), Some(body_candidate))
                    if should_retry_with_body(&broad_candidate, &body_candidate) =>
                {
                    Some(body_candidate)
                }
                (Some(broad_candidate), _) => Some(broad_candidate),
                (None, body_candidate) => body_candidate,
            };

            diagnostics
                .fallback_attempts
                .push(FallbackAttemptDiagnostic {
                    name: "low-word-selector-retry".to_string(),
                    accepted: retry_candidate.is_some(),
                    previous_word_count: candidate.word_count,
                    candidate_word_count: retry_candidate
                        .as_ref()
                        .map_or(0, |candidate| candidate.word_count),
                    reason: if retry_candidate.is_some() {
                        "retry candidate had enough additional text".to_string()
                    } else {
                        "retry candidates did not improve enough".to_string()
                    },
                });
            if let Some(retry_candidate) = retry_candidate {
                fallback_steps.push("low-word-selector-retry".to_string());
                best_candidate = Some(retry_candidate);
            }
        }
    }

    if let Some(candidate) = best_candidate {
        push_candidate_diagnostics(
            diagnostics,
            &candidate_diagnostics,
            Some(candidate.diagnostic_id),
        );
        Ok(ExtractedContent {
            html: candidate.html,
            selector: Some(candidate.selector),
            score: Some(candidate.score),
            fallbacks: fallback_steps,
            diagnostics: diagnostics.clone(),
        })
    } else if let Some(html) = structured_content_candidate(doc, 0)? {
        let word_count = text_word_count(&html);
        diagnostics
            .fallback_attempts
            .push(FallbackAttemptDiagnostic {
                name: "schema-org".to_string(),
                accepted: true,
                previous_word_count: 0,
                candidate_word_count: word_count,
                reason: "structured content was available without a visible candidate".to_string(),
            });
        let schema_candidate = Candidate {
            diagnostic_id: next_candidate_id,
            diagnostic_pass: "schema-org".to_string(),
            score: 0,
            selector_index: 0,
            selector: "schema-org".to_string(),
            word_count,
            content_block_count: 0,
            html,
        };
        candidate_diagnostics.push(vec![schema_candidate.diagnostic_record()]);
        push_candidate_diagnostics(
            diagnostics,
            &candidate_diagnostics,
            Some(schema_candidate.diagnostic_id),
        );
        Ok(ExtractedContent {
            html: schema_candidate.html,
            selector: Some("schema-org".to_string()),
            score: None,
            fallbacks: vec!["schema-org".to_string()],
            diagnostics: diagnostics.clone(),
        })
    } else {
        push_candidate_diagnostics(diagnostics, &candidate_diagnostics, None);
        Err(ChidoriError::ExtractionFailed)
    }
}

fn hacker_news_listing_candidate(doc: &ParsedDocument) -> Result<Option<String>, ChidoriError> {
    if doc.url.host_str() != Some("news.ycombinator.com") || !is_hacker_news_listing_path(doc) {
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

fn is_hacker_news_listing_path(doc: &ParsedDocument) -> bool {
    matches!(
        doc.url.path(),
        "/" | "/news" | "/newest" | "/front" | "/ask" | "/show" | "/jobs" | "/submitted"
    )
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
