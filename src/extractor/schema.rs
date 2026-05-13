use crate::{document::ParsedDocument, error::ChidoriError};
use html_escape::encode_text;
use scraper::Selector;

use super::util::normalize_text;

pub(crate) fn structured_content_candidate(
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
