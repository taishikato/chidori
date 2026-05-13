use crate::{document::ParsedDocument, error::ChidoriError};
use html_escape::encode_text;
use scraper::Selector;

use super::super::{
    types::SiteExtraction,
    util::{element_text, host_matches},
};

pub(super) fn extract(doc: &ParsedDocument) -> Result<Option<SiteExtraction>, ChidoriError> {
    Ok(repository_discussion_candidate(doc)?
        .map(|html| SiteExtraction::new("repository-discussion", html)))
}

fn repository_discussion_candidate(doc: &ParsedDocument) -> Result<Option<String>, ChidoriError> {
    if !is_repository_discussion_path(doc) {
        return Ok(None);
    }

    let title_selector = Selector::parse(
        r#"[data-testid="issue-title"], .js-issue-title, bdi.markdown-title, h1 bdi"#,
    )
    .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let body_selector = Selector::parse(".markdown-body")
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let comment_selector = Selector::parse(".timeline-comment, [data-testid=\"issue-comment\"]")
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;

    let Some(title) = doc
        .dom
        .select(&title_selector)
        .map(element_text)
        .find(|title| !title.is_empty())
    else {
        return Ok(None);
    };
    let bodies = doc.dom.select(&body_selector).collect::<Vec<_>>();
    if bodies.is_empty() {
        return Ok(None);
    }

    let mut output = String::from("<article class=\"chidori-repository-discussion\">");
    output.push_str("<h1>");
    output.push_str(&encode_text(&title));
    output.push_str("</h1>");

    let primary_body = bodies.first().copied();
    if let Some(body) = primary_body {
        output.push_str(&body.inner_html());
    }

    for comment in doc.dom.select(&comment_selector) {
        if let Some(body) = comment.select(&body_selector).next() {
            if Some(body) == primary_body {
                continue;
            }
            output.push_str("<blockquote>");
            output.push_str(&body.inner_html());
            output.push_str("</blockquote>");
        }
    }

    output.push_str("</article>");
    Ok(Some(output))
}

fn is_repository_discussion_path(doc: &ParsedDocument) -> bool {
    let Some(host) = doc.url.host_str() else {
        return false;
    };
    if !host_matches(host, "github.com") {
        return false;
    }
    let segments = doc
        .url
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())
        .unwrap_or_default();

    segments.len() >= 4 && matches!(segments[2], "issues" | "pull")
}
