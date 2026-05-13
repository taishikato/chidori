use crate::document::ParsedDocument;
use html_escape::{encode_double_quoted_attribute, encode_text};
use scraper::ElementRef;

pub(crate) fn href_path_matches_target(href: &str, target_path: &str) -> bool {
    normalize_href_path(href) == target_path.trim_end_matches('/')
}

pub(crate) fn normalize_href_path(href: &str) -> String {
    if let Ok(url) = url::Url::parse(href) {
        return url.path().trim_end_matches('/').to_string();
    }
    href.split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_string()
}

pub(crate) fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

pub(crate) fn push_link(output: &mut String, url: &str, text: &str) {
    output.push_str("<a href=\"");
    output.push_str(&encode_double_quoted_attribute(url));
    output.push_str("\">");
    output.push_str(&encode_text(text));
    output.push_str("</a>");
}

pub(crate) fn resolve_url(doc: &ParsedDocument, href: &str) -> String {
    doc.url
        .join(href)
        .map(|url| url.to_string())
        .unwrap_or_else(|_| href.to_string())
}

pub(crate) fn element_text(element: ElementRef<'_>) -> String {
    normalize_text(&element.text().collect::<Vec<_>>().join(" "))
}

pub(crate) fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn text_word_count(html: &str) -> usize {
    scraper::Html::parse_fragment(html)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .count()
}
