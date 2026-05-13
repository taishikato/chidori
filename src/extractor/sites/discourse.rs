use crate::{document::ParsedDocument, error::ChidoriError};
use html_escape::encode_text;
use scraper::Selector;

use super::super::{
    types::SiteExtraction,
    util::{element_text, text_word_count},
};

pub(super) fn extract(doc: &ParsedDocument) -> Result<Option<SiteExtraction>, ChidoriError> {
    Ok(discourse_topic_candidate(doc)?.map(|html| SiteExtraction::new("discourse-topic", html)))
}

fn discourse_topic_candidate(doc: &ParsedDocument) -> Result<Option<String>, ChidoriError> {
    if !is_discourse_topic_path(doc) || !has_discourse_marker(doc)? {
        return Ok(None);
    }

    let title_selector = Selector::parse("main h1, #topic-title h1, h1")
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let post_selector = Selector::parse(".topic-post .topic-body .cooked, .topic-post .cooked")
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;

    let posts = doc
        .dom
        .select(&post_selector)
        .filter(|post| text_word_count(&post.inner_html()) > 0)
        .collect::<Vec<_>>();
    if posts.is_empty() {
        return Ok(None);
    }

    let mut output = String::from("<article class=\"chidori-discourse-topic\">");
    if let Some(title) = doc
        .dom
        .select(&title_selector)
        .map(element_text)
        .find(|title| !title.is_empty())
    {
        output.push_str("<h1>");
        output.push_str(&encode_text(&title));
        output.push_str("</h1>");
    }

    for (index, post) in posts.into_iter().enumerate() {
        if index == 0 {
            output.push_str(&post.inner_html());
        } else {
            output.push_str("<blockquote>");
            output.push_str(&post.inner_html());
            output.push_str("</blockquote>");
        }
    }

    output.push_str("</article>");
    Ok(Some(output))
}

fn is_discourse_topic_path(doc: &ParsedDocument) -> bool {
    let segments = doc
        .url
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())
        .unwrap_or_default();

    segments.len() >= 3 && segments[0] == "t"
}

fn has_discourse_marker(doc: &ParsedDocument) -> Result<bool, ChidoriError> {
    let topic_title_selector = Selector::parse("#topic-title")
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    if doc.dom.select(&topic_title_selector).next().is_some() {
        return Ok(true);
    }

    let topic_post_selector =
        Selector::parse(".topic-post .topic-body .cooked, .topic-post .cooked")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    if doc.dom.select(&topic_post_selector).next().is_some() {
        return Ok(true);
    }

    let generator_selector = Selector::parse(r#"meta[name="generator"]"#)
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let has_generator = doc.dom.select(&generator_selector).any(|meta| {
        meta.value()
            .attr("content")
            .is_some_and(|content| content.contains("Discourse"))
    });
    let topic_post_wrapper_selector =
        Selector::parse(".topic-post").map_err(|error| ChidoriError::Unknown(error.to_string()))?;

    Ok(has_generator
        && doc
            .dom
            .select(&topic_post_wrapper_selector)
            .next()
            .is_some())
}
