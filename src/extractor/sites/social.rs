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
        let body = turn_body_html(turn, role_element);
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
    element.ancestors().find_map(|ancestor| {
        ElementRef::wrap(ancestor).filter(|ancestor| selector.matches(ancestor))
    })
}

fn turn_body_html(turn: ElementRef<'_>, role_element: Option<ElementRef<'_>>) -> String {
    let turn_html = turn.inner_html();
    let Some(role_element) = role_element else {
        return turn_html;
    };
    if role_element == turn {
        return turn_html;
    }

    let role_html = role_element.html();
    turn_html
        .find(&role_html)
        .map(|start| turn_html[start..].to_string())
        .unwrap_or(turn_html)
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn conversation_turn_body_uses_full_turn_when_role_is_nested() {
        let html = r#"
        <html><body>
          <div data-testid="conversation-turn">
            <div data-message-author-role="user"><span>User label</span></div>
            <p>The actual user message should stay attached to this turn.</p>
          </div>
          <div data-testid="conversation-turn">
            <div data-message-author-role="assistant"><span>Assistant label</span></div>
            <p>The assistant answer should also stay attached to this turn.</p>
          </div>
        </body></html>"#;
        let doc = ParsedDocument::parse(html, Url::parse("https://chatgpt.com/c/123").unwrap());

        let output = ai_conversation_candidate(&doc).unwrap().unwrap();

        assert!(output.contains("The actual user message should stay attached"));
        assert!(output.contains("The assistant answer should also stay attached"));
    }
}
