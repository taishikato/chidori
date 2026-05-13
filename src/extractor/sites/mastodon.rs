use crate::{document::ParsedDocument, error::ChidoriError};
use html_escape::encode_text;
use scraper::{ElementRef, Selector};

use super::super::{types::SiteExtraction, util::element_text};

struct MastodonSelectors {
    statuses: Selector,
    threads: Selector,
    display_names: Selector,
    handles: Selector,
    times: Selector,
    bodies: Selector,
}

pub(super) fn extract(doc: &ParsedDocument) -> Result<Option<SiteExtraction>, ChidoriError> {
    Ok(mastodon_status_thread_candidate(doc)?
        .map(|html| SiteExtraction::new("mastodon-status-thread", html)))
}

fn mastodon_status_thread_candidate(doc: &ParsedDocument) -> Result<Option<String>, ChidoriError> {
    if !is_mastodon_status_path(doc) {
        return Ok(None);
    }

    let selectors = MastodonSelectors {
        statuses: Selector::parse("article.status, article.status-public, article[data-id]")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        threads: Selector::parse(".status-thread")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        display_names: Selector::parse(
            ".display-name__html, .status__display-name strong, .p-name",
        )
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        handles: Selector::parse(".display-name__account, .status__display-name span")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        times: Selector::parse("time").map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        bodies: Selector::parse(".status__content, .e-content")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
    };

    let Some(status_id) = mastodon_status_id(doc) else {
        return Ok(None);
    };
    let Some(thread) = doc.dom.select(&selectors.threads).find(|candidate| {
        candidate
            .select(&selectors.statuses)
            .any(|status| status.value().attr("data-id") == Some(status_id.as_str()))
    }) else {
        return Ok(None);
    };
    let statuses: Vec<_> = thread.select(&selectors.statuses).collect();
    if statuses.is_empty() {
        return Ok(None);
    }

    let Some(start_index) = statuses
        .iter()
        .position(|status| status.value().attr("data-id") == Some(status_id.as_str()))
    else {
        return Ok(None);
    };

    let mut output = String::from("<article class=\"chidori-mastodon-thread\">");
    let mut status_count = 0;
    for status in statuses.into_iter().skip(start_index) {
        if push_mastodon_status(&mut output, status, &selectors, status_count > 0) {
            status_count += 1;
        }
    }
    output.push_str("</article>");

    if status_count == 0 {
        Ok(None)
    } else {
        Ok(Some(output))
    }
}

fn is_mastodon_status_path(doc: &ParsedDocument) -> bool {
    let Some(host) = doc.url.host_str() else {
        return false;
    };

    host.contains('.')
        && doc.url.path_segments().is_some_and(|segments| {
            let segments: Vec<_> = segments.collect();
            segments.len() >= 2
                && segments[0].starts_with('@')
                && segments[0].len() > 1
                && segments[1]
                    .chars()
                    .all(|character| character.is_ascii_digit())
        })
}

fn mastodon_status_id(doc: &ParsedDocument) -> Option<String> {
    doc.url.path_segments().and_then(|mut segments| {
        let _account = segments.next()?;
        segments.next().map(ToString::to_string)
    })
}

fn push_mastodon_status(
    output: &mut String,
    status: ElementRef<'_>,
    selectors: &MastodonSelectors,
    nested: bool,
) -> bool {
    let Some(body) = status
        .select(&selectors.bodies)
        .next()
        .filter(|body| !element_text(*body).is_empty())
    else {
        return false;
    };

    if nested {
        output.push_str("<blockquote>");
    } else {
        output.push_str("<section class=\"chidori-mastodon-status\">");
    }

    if let Some(display_name) = status
        .select(&selectors.display_names)
        .next()
        .map(element_text)
        .filter(|display_name| !display_name.is_empty())
    {
        output.push_str("<p>");
        output.push_str(&encode_text(&display_name));
        output.push_str("</p>");
    }

    if let Some(handle) = status
        .select(&selectors.handles)
        .next()
        .map(element_text)
        .filter(|handle| !handle.is_empty())
    {
        push_small_paragraph(output, &handle);
    }
    if let Some(date) = status
        .select(&selectors.times)
        .next()
        .map(element_text)
        .filter(|date| !date.is_empty())
    {
        push_small_paragraph(output, &date);
    }

    output.push_str(&body.inner_html());

    if nested {
        output.push_str("</blockquote>");
    } else {
        output.push_str("</section>");
    }

    true
}

fn push_small_paragraph(output: &mut String, text: &str) {
    output.push_str("<p><small>");
    output.push_str(&encode_text(text));
    output.push_str("</small></p>");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ParsedDocument;
    use url::Url;

    fn parse_doc_with_url(url: &str) -> ParsedDocument {
        ParsedDocument::parse(
            "<html><body></body></html>".to_string(),
            Url::parse(url).unwrap(),
        )
    }

    fn parse_doc(html: &str, url: &str) -> ParsedDocument {
        ParsedDocument::parse(html.to_string(), Url::parse(url).unwrap())
    }

    #[test]
    fn mastodon_status_path_accepts_federated_instances() {
        assert!(is_mastodon_status_path(&parse_doc_with_url(
            "https://example.social/@alice/112233445566778899"
        )));
        assert!(!is_mastodon_status_path(&parse_doc_with_url(
            "https://example.social/alice/112233445566778899"
        )));
        assert!(!is_mastodon_status_path(&parse_doc_with_url(
            "https://example.social/@alice/not-a-number"
        )));
    }

    #[test]
    fn mastodon_thread_matching_accepts_single_quoted_status_id() {
        let doc = parse_doc(
            r#"
            <section class="status-thread">
              <article class="status" data-id='112233445566778899'>
                <a class="status__display-name">
                  <strong>Alice Example</strong>
                  <span>@alice@example.social</span>
                </a>
                <time>May 8, 2026</time>
                <div class="status__content">
                  <p>Single quoted status id should match.</p>
                </div>
              </article>
            </section>
            "#,
            "https://example.social/@alice/112233445566778899",
        );

        let html = mastodon_status_thread_candidate(&doc).unwrap().unwrap();

        assert!(html.contains("Alice Example"));
        assert!(html.contains("@alice@example.social"));
        assert!(html.contains("Single quoted status id should match."));
    }
}
