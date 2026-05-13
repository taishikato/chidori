use crate::{document::ParsedDocument, error::ChidoriError};
use html_escape::encode_text;
use scraper::{ElementRef, Selector};

use super::super::{
    types::SiteExtraction,
    util::{element_text, host_matches},
};
use super::common::push_meta_paragraph;

struct MicroblogSelectors {
    statuses: Selector,
    users: Selector,
    spans: Selector,
    links: Selector,
    times: Selector,
    bodies: Selector,
}

pub(super) fn extract(doc: &ParsedDocument) -> Result<Option<SiteExtraction>, ChidoriError> {
    Ok(microblog_status_thread_candidate(doc)?
        .map(|html| SiteExtraction::new("microblog-status-thread", html)))
}

fn microblog_status_thread_candidate(doc: &ParsedDocument) -> Result<Option<String>, ChidoriError> {
    if !is_microblog_status_path(doc) {
        return Ok(None);
    }

    let selectors = MicroblogSelectors {
        statuses: Selector::parse("article[data-testid=\"tweet\"], article[role=\"article\"]")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        users: Selector::parse("[data-testid=\"User-Name\"]")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        spans: Selector::parse("span").map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        links: Selector::parse("a[href]")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        times: Selector::parse("time").map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        bodies: Selector::parse("[data-testid=\"tweetText\"]")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
    };

    let Some(status_id) = microblog_status_id(doc) else {
        return Ok(None);
    };
    let Some(target_status) = doc
        .dom
        .select(&selectors.statuses)
        .find(|status| microblog_status_links_to(*status, &selectors, &status_id))
    else {
        return Ok(None);
    };
    let mut output = String::from("<article class=\"chidori-microblog-thread\">");
    let mut status_count = 0;
    if let Some(thread_root) = microblog_thread_root(target_status) {
        for status in thread_root
            .select(&selectors.statuses)
            .filter(|status| {
                nearest_microblog_status_parent(*status, &selectors.statuses).is_none()
            })
            .skip_while(|status| *status != target_status)
        {
            if push_microblog_status(&mut output, status, &selectors, status_count > 0) {
                status_count += 1;
            }
        }
    } else if push_microblog_status(&mut output, target_status, &selectors, false) {
        status_count += 1;
    }

    output.push_str("</article>");

    if status_count == 0 {
        Ok(None)
    } else {
        Ok(Some(output))
    }
}

fn is_microblog_status_path(doc: &ParsedDocument) -> bool {
    let Some(host) = doc.url.host_str() else {
        return false;
    };
    let is_microblog_host = host_matches(host, "x.com") || host_matches(host, "twitter.com");

    is_microblog_host && microblog_status_id(doc).is_some()
}

fn microblog_status_id(doc: &ParsedDocument) -> Option<String> {
    doc.url.path_segments().and_then(|segments| {
        let segments: Vec<_> = segments.collect();
        let status_id = *segments.get(2)?;
        (segments.len() >= 3
            && segments.get(1) == Some(&"status")
            && !status_id.is_empty()
            && status_id
                .chars()
                .all(|character| character.is_ascii_digit()))
        .then(|| status_id.to_string())
    })
}

fn microblog_status_links_to(
    status: ElementRef<'_>,
    selectors: &MicroblogSelectors,
    status_id: &str,
) -> bool {
    status.select(&selectors.links).any(|link| {
        nearest_microblog_status(link, &selectors.statuses) == Some(status)
            && link
                .value()
                .attr("href")
                .and_then(microblog_status_id_from_href)
                .is_some_and(|link_status_id| link_status_id == status_id)
    })
}

fn microblog_status_id_from_href(href: &str) -> Option<String> {
    let path = &href[..href.find(&['?', '#'][..]).unwrap_or(href.len())];
    let segments: Vec<_> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    segments.windows(2).find_map(|window| {
        (window[0] == "status"
            && !window[1].is_empty()
            && window[1]
                .chars()
                .all(|character| character.is_ascii_digit()))
        .then(|| window[1].to_string())
    })
}

fn microblog_thread_root(status: ElementRef<'_>) -> Option<ElementRef<'_>> {
    status.ancestors().find_map(|ancestor| {
        let element = ElementRef::wrap(ancestor)?;
        (element.value().name() == "section"
            && element
                .value()
                .attr("aria-label")
                .is_some_and(|label| label.contains("Conversation")))
        .then_some(element)
    })
}

fn push_microblog_status(
    output: &mut String,
    status: ElementRef<'_>,
    selectors: &MicroblogSelectors,
    nested: bool,
) -> bool {
    let Some(body) = status.descendent_elements().find(|element| {
        selectors.bodies.matches(element)
            && nearest_microblog_status(*element, &selectors.statuses) == Some(status)
            && !element_text(*element).is_empty()
    }) else {
        return false;
    };

    if nested {
        output.push_str("<blockquote>");
    } else {
        output.push_str("<section class=\"chidori-microblog-status\">");
    }

    let (display_name, handle) = microblog_author(status, selectors);
    if let Some(display_name) = display_name {
        output.push_str("<p>");
        output.push_str(&encode_text(&display_name));
        output.push_str("</p>");
    }

    let mut meta = Vec::new();
    if let Some(handle) = handle {
        meta.push(handle);
    }
    if let Some(date) = status
        .descendent_elements()
        .find(|element| {
            selectors.times.matches(element)
                && nearest_microblog_status(*element, &selectors.statuses) == Some(status)
        })
        .map(element_text)
        .filter(|date| !date.is_empty())
    {
        meta.push(date);
    }
    push_meta_paragraph(output, &meta);

    output.push_str(&body.inner_html());
    for quoted_status in status.select(&selectors.statuses).filter(|quoted_status| {
        nearest_microblog_status_parent(*quoted_status, &selectors.statuses) == Some(status)
    }) {
        push_microblog_status(output, quoted_status, selectors, true);
    }

    if nested {
        output.push_str("</blockquote>");
    } else {
        output.push_str("</section>");
    }

    true
}

fn microblog_author(
    status: ElementRef<'_>,
    selectors: &MicroblogSelectors,
) -> (Option<String>, Option<String>) {
    let Some(user_block) = status.descendent_elements().find(|element| {
        selectors.users.matches(element)
            && nearest_microblog_status(*element, &selectors.statuses) == Some(status)
    }) else {
        return (None, None);
    };

    let names: Vec<_> = user_block
        .select(&selectors.spans)
        .map(element_text)
        .filter(|text| !text.is_empty())
        .collect();
    let display_name = names.iter().find(|text| !text.starts_with('@')).cloned();
    let handle = names.iter().find(|text| text.starts_with('@')).cloned();

    (display_name, handle)
}

fn nearest_microblog_status<'a>(
    element: ElementRef<'a>,
    status_selector: &Selector,
) -> Option<ElementRef<'a>> {
    element.ancestors().find_map(|ancestor| {
        ElementRef::wrap(ancestor).filter(|ancestor| status_selector.matches(ancestor))
    })
}

fn nearest_microblog_status_parent<'a>(
    status: ElementRef<'a>,
    status_selector: &Selector,
) -> Option<ElementRef<'a>> {
    status.ancestors().skip(1).find_map(|ancestor| {
        ElementRef::wrap(ancestor).filter(|ancestor| status_selector.matches(ancestor))
    })
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
    fn microblog_status_path_requires_numeric_status_id() {
        assert!(is_microblog_status_path(&parse_doc_with_url(
            "https://x.com/parserbird/status/1788600000000000000"
        )));
        assert!(!is_microblog_status_path(&parse_doc_with_url(
            "https://x.com/parserbird/status/"
        )));
        assert!(!is_microblog_status_path(&parse_doc_with_url(
            "https://twitter.com/parserbird/status/not-a-number"
        )));
        assert!(!is_microblog_status_path(&parse_doc_with_url(
            "https://x.com/parserbird"
        )));
    }

    #[test]
    fn microblog_extraction_handles_minimal_status_without_conversation_root() {
        let doc = parse_doc(
            r#"
            <main>
              <article data-testid="tweet" role="article">
                <div data-testid="User-Name">
                  <span>Parser Bird</span>
                  <span>@parserbird</span>
                  <a href="/parserbird/status/123"><time>May 8, 2026</time></a>
                </div>
                <div data-testid="tweetText">Tiny status body.</div>
              </article>
            </main>
            "#,
            "https://x.com/parserbird/status/123",
        );

        let html = microblog_status_thread_candidate(&doc).unwrap().unwrap();

        assert!(html.contains("Parser Bird"));
        assert!(html.contains("Tiny status body."));
    }

    #[test]
    fn microblog_status_link_matching_requires_exact_status_id() {
        let doc = parse_doc(
            r#"
            <section aria-label="Timeline: Conversation">
              <article data-testid="tweet" role="article">
                <div data-testid="User-Name">
                  <span>Wrong Bird</span>
                  <span>@wrongbird</span>
                  <a href="/wrongbird/status/1234"><time>May 8, 2026</time></a>
                </div>
                <div data-testid="tweetText">Wrong status body.</div>
              </article>
              <article data-testid="tweet" role="article">
                <div data-testid="User-Name">
                  <span>Parser Bird</span>
                  <span>@parserbird</span>
                  <a href="/parserbird/status/123"><time>May 8, 2026</time></a>
                </div>
                <div data-testid="tweetText">Right status body.</div>
              </article>
            </section>
            "#,
            "https://x.com/parserbird/status/123",
        );

        let html = microblog_status_thread_candidate(&doc).unwrap().unwrap();

        assert!(html.contains("Parser Bird"));
        assert!(html.contains("Right status body."));
        assert!(!html.contains("Wrong Bird"));
        assert!(!html.contains("Wrong status body."));
    }
}
