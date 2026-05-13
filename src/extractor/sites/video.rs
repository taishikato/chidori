use crate::{document::ParsedDocument, error::ChidoriError};
use html_escape::encode_text;
use scraper::Selector;

use super::super::{
    types::SiteExtraction,
    util::{element_text, host_matches},
};
use super::common::push_meta_paragraph;

pub(super) fn extract(doc: &ParsedDocument) -> Result<Option<SiteExtraction>, ChidoriError> {
    Ok(youtube_watch_candidate(doc)?.map(|html| SiteExtraction::new("youtube-watch", html)))
}

fn youtube_watch_candidate(doc: &ParsedDocument) -> Result<Option<String>, ChidoriError> {
    if !is_youtube_watch_path(doc) {
        return Ok(None);
    }

    let watch_selectors = [
        Selector::parse("#watch-content")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        Selector::parse("#primary-inner")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        Selector::parse("ytd-watch-flexy #primary")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        Selector::parse("main #primary")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
    ];
    let title_selector =
        Selector::parse("h1").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let channel_selector =
        Selector::parse(".channel-name, ytd-channel-name a, #channel-name a, a[href^=\"/@\"]")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let date_selector = Selector::parse(
        "[itemprop=\"datePublished\"], time, #info-strings yt-formatted-string, #date yt-formatted-string, #date-text, #info .date, #info .date-text",
    )
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let description_selector = Selector::parse(
        "#description, #description-inline-expander, ytd-text-inline-expander, [itemprop=\"description\"]",
    )
    .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let transcript_selector = Selector::parse("#transcript, ytd-transcript-renderer, .transcript")
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;

    let Some(watch) = watch_selectors
        .iter()
        .find_map(|selector| doc.dom.select(selector).next())
    else {
        return Ok(None);
    };

    let Some(title) = watch
        .select(&title_selector)
        .map(element_text)
        .find(|title| !title.is_empty())
    else {
        return Ok(None);
    };

    let mut output = String::from("<article class=\"chidori-youtube-watch\">");
    output.push_str("<h1>");
    output.push_str(&encode_text(&title));
    output.push_str("</h1>");

    let mut meta = Vec::new();
    if let Some(channel) = watch
        .select(&channel_selector)
        .map(element_text)
        .find(|channel| !channel.is_empty())
    {
        meta.push(channel);
    }
    if let Some(date) = watch
        .select(&date_selector)
        .map(element_text)
        .find(|date| !date.is_empty())
    {
        meta.push(date);
    }
    push_meta_paragraph(&mut output, &meta);

    if let Some(description) = watch
        .select(&description_selector)
        .find(|description| !element_text(*description).is_empty())
    {
        output.push_str(&description.inner_html());
    }

    if let Some(transcript) = watch
        .select(&transcript_selector)
        .find(|transcript| !element_text(*transcript).is_empty())
    {
        output.push_str(&transcript.inner_html());
    }

    output.push_str("</article>");
    Ok(Some(output))
}

fn is_youtube_watch_path(doc: &ParsedDocument) -> bool {
    let Some(host) = doc.url.host_str() else {
        return false;
    };

    host_matches(host, "youtube.com")
        && doc.url.path() == "/watch"
        && doc
            .url
            .query_pairs()
            .any(|(key, value)| key == "v" && !value.is_empty())
}
