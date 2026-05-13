use crate::{document::ParsedDocument, error::ChidoriError};
use html_escape::encode_text;
use scraper::{ElementRef, Selector};

use super::super::{
    types::SiteExtraction,
    util::{host_matches, text_word_count},
};

pub(super) fn extract(doc: &ParsedDocument) -> Result<Option<SiteExtraction>, ChidoriError> {
    Ok(ai_conversation_candidate(doc)?.map(|html| SiteExtraction::new("ai-conversation", html)))
}

fn ai_conversation_candidate(doc: &ParsedDocument) -> Result<Option<String>, ChidoriError> {
    let Some(host) = doc.url.host_str() else {
        return Ok(None);
    };
    let is_supported = host_matches(host, "chatgpt.com")
        || host_matches(host, "claude.ai")
        || host_matches(host, "grok.com")
        || host_matches(host, "gemini.google.com");
    if !is_supported {
        return Ok(None);
    }

    let turn_selector =
        Selector::parse(r#"[data-testid="conversation-turn"], [data-message-author-role]"#)
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let role_selector = Selector::parse(r#"[data-message-author-role]"#)
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;

    let mut output = String::from("<article class=\"chidori-ai-conversation\">");
    let mut count = 0;
    for turn in doc.dom.select(&turn_selector) {
        if nearest_conversation_parent(turn, &turn_selector).is_some() {
            continue;
        }

        let role_element = if turn.value().attr("data-message-author-role").is_some() {
            Some(turn)
        } else {
            turn.select(&role_selector).next()
        };
        let role = role_element
            .and_then(|element| element.value().attr("data-message-author-role"))
            .unwrap_or("message");
        let body = role_element
            .map(|element| element.inner_html())
            .unwrap_or_else(|| turn.inner_html());
        if text_word_count(&body) == 0 {
            continue;
        }

        output.push_str("<section>");
        output.push_str("<p><strong>");
        output.push_str(&encode_text(role));
        output.push_str("</strong></p>");
        output.push_str(&body);
        output.push_str("</section>");
        count += 1;
    }
    output.push_str("</article>");

    if count >= 2 {
        Ok(Some(output))
    } else {
        Ok(None)
    }
}

fn nearest_conversation_parent<'a>(
    element: ElementRef<'a>,
    selector: &Selector,
) -> Option<ElementRef<'a>> {
    element.ancestors().skip(1).find_map(|ancestor| {
        ElementRef::wrap(ancestor).filter(|ancestor| selector.matches(ancestor))
    })
}
