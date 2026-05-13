use crate::{document::ParsedDocument, error::ChidoriError};
use html_escape::encode_text;
use scraper::Selector;

use super::super::{
    types::SiteExtraction,
    util::{element_text, host_matches},
};

pub(super) fn extract(doc: &ParsedDocument) -> Result<Option<SiteExtraction>, ChidoriError> {
    if let Some(html) = github_release_candidate(doc)? {
        return Ok(Some(SiteExtraction::new("github-release", html)));
    }

    if let Some(html) = github_wiki_candidate(doc)? {
        return Ok(Some(SiteExtraction::new("github-wiki", html)));
    }

    Ok(repository_discussion_candidate(doc)?
        .map(|html| SiteExtraction::new("repository-discussion", html)))
}

fn github_release_candidate(doc: &ParsedDocument) -> Result<Option<String>, ChidoriError> {
    if !is_github_release_path(doc) {
        return Ok(None);
    }

    let title_selector = Selector::parse(
        r#".release-entry h1 a, h1 a[href*="/releases/tag/"], [data-testid="release-header"] h1, h1"#,
    )
    .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let Some(body_html) = first_html_for_selectors(
        doc,
        &[
            ".release-entry .markdown-body",
            "[data-testid=\"release-body\"] .markdown-body",
            "[data-testid=\"release-body\"]",
        ],
    )?
    else {
        return Ok(None);
    };
    let title = doc
        .dom
        .select(&title_selector)
        .map(element_text)
        .find(|title| !title.is_empty())
        .or_else(|| {
            github_path_segments(doc)
                .get(4)
                .map(|tag| (*tag).to_string())
        });
    let Some(title) = title else {
        return Ok(None);
    };

    let mut output = String::from("<article class=\"chidori-github-release\">");
    output.push_str("<h1>");
    output.push_str(&encode_text(&title));
    output.push_str("</h1>");
    output.push_str(&body_html);
    output.push_str("</article>");

    Ok(Some(output))
}

fn github_wiki_candidate(doc: &ParsedDocument) -> Result<Option<String>, ChidoriError> {
    if !is_github_wiki_path(doc) {
        return Ok(None);
    }

    let body_html =
        if let Some(html) = first_outer_html_for_selectors(doc, &["#wiki-body .markdown-body"])? {
            html
        } else if let Some(html) = first_inner_html_for_selectors(doc, &["#wiki-body"])? {
            html
        } else {
            return Ok(None);
        };

    Ok(Some(body_html))
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
    if !is_github_host(doc) {
        return false;
    }
    let segments = github_path_segments(doc);

    segments.len() >= 4 && matches!(segments[2], "issues" | "pull")
}

fn is_github_release_path(doc: &ParsedDocument) -> bool {
    if !is_github_host(doc) {
        return false;
    }
    let segments = github_path_segments(doc);

    segments.len() >= 5 && segments[2] == "releases" && segments[3] == "tag"
}

fn is_github_wiki_path(doc: &ParsedDocument) -> bool {
    if !is_github_host(doc) {
        return false;
    }
    let segments = github_path_segments(doc);

    matches!(segments.get(2), Some(&"wiki"))
}

fn is_github_host(doc: &ParsedDocument) -> bool {
    doc.url
        .host_str()
        .is_some_and(|host| host_matches(host, "github.com"))
}

fn github_path_segments(doc: &ParsedDocument) -> Vec<&str> {
    doc.url
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())
        .unwrap_or_default()
}

fn first_html_for_selectors(
    doc: &ParsedDocument,
    selectors: &[&str],
) -> Result<Option<String>, ChidoriError> {
    for selector in selectors {
        let selector =
            Selector::parse(selector).map_err(|error| ChidoriError::Unknown(error.to_string()))?;
        if let Some(element) = doc.dom.select(&selector).next() {
            return Ok(Some(element.inner_html()));
        }
    }

    Ok(None)
}

fn first_outer_html_for_selectors(
    doc: &ParsedDocument,
    selectors: &[&str],
) -> Result<Option<String>, ChidoriError> {
    for selector in selectors {
        let selector =
            Selector::parse(selector).map_err(|error| ChidoriError::Unknown(error.to_string()))?;
        if let Some(element) = doc.dom.select(&selector).next() {
            return Ok(Some(element.html()));
        }
    }

    Ok(None)
}

fn first_inner_html_for_selectors(
    doc: &ParsedDocument,
    selectors: &[&str],
) -> Result<Option<String>, ChidoriError> {
    first_html_for_selectors(doc, selectors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn github_wiki_path_accepts_root_wiki_url() {
        let doc = ParsedDocument::parse(
            "<html><body></body></html>",
            Url::parse("https://github.com/owner/repo/wiki").unwrap(),
        );

        assert!(is_github_wiki_path(&doc));
    }
}
