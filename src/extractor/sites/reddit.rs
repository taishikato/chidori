use crate::{document::ParsedDocument, error::ChidoriError};
use html_escape::encode_text;
use scraper::{ElementRef, Selector};

use super::super::{
    types::SiteExtraction,
    util::{element_text, host_matches},
};
use super::common::push_meta_paragraph;

struct RedditSelectors {
    comments: Selector,
    wrappers: Selector,
    body: Selector,
    users: Selector,
    times: Selector,
}

pub(super) fn extract(doc: &ParsedDocument) -> Result<Option<SiteExtraction>, ChidoriError> {
    Ok(
        reddit_discussion_candidate(doc)?
            .map(|html| SiteExtraction::new("reddit-discussion", html)),
    )
}

fn reddit_discussion_candidate(doc: &ParsedDocument) -> Result<Option<String>, ChidoriError> {
    if !is_reddit_discussion_path(doc) {
        return Ok(None);
    }

    let post_selector = Selector::parse(
        "shreddit-post, article[data-testid=\"post-container\"], article#post, [data-testid=\"post-container\"]",
    )
    .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let title_selector = Selector::parse("h1, [slot=\"title\"]")
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let body_selector =
        Selector::parse("[slot=\"text-body\"], .md, [data-testid=\"post-content\"]")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let user_selector = Selector::parse("a[href*=\"/user/\"], a[href*=\"/u/\"]")
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let score_selector =
        Selector::parse("[score], [id*=\"score\"], faceplate-number, [slot=\"credit-bar\"] span")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let time_selector =
        Selector::parse("time").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let reddit_selectors = RedditSelectors {
        comments: Selector::parse("shreddit-comment, [data-testid=\"comment\"]")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        wrappers: Selector::parse("shreddit-comment, [data-testid=\"comment\"]")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        body: Selector::parse("[slot=\"comment\"], [data-testid=\"comment\"] .md")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        users: user_selector,
        times: time_selector,
    };

    let Some(post) = doc.dom.select(&post_selector).next() else {
        return Ok(None);
    };

    let title = post
        .select(&title_selector)
        .next()
        .map(element_text)
        .filter(|title| !title.is_empty());
    let Some(title) = title else {
        return Ok(None);
    };

    let mut output = String::from("<article class=\"chidori-reddit-discussion\">");
    output.push_str("<h1>");
    output.push_str(&encode_text(&title));
    output.push_str("</h1>");

    let mut post_meta = Vec::new();
    if let Some(author) = post_author(post, &reddit_selectors.users) {
        post_meta.push(author);
    }
    if let Some(score) = post_score(post, &score_selector) {
        post_meta.push(score);
    }
    if let Some(date) = post
        .select(&reddit_selectors.times)
        .next()
        .map(element_text)
        .filter(|date| !date.is_empty())
    {
        post_meta.push(date);
    }
    push_meta_paragraph(&mut output, &post_meta);

    if let Some(body) = post.select(&body_selector).next() {
        output.push_str(&body.inner_html());
    }

    let comments: Vec<_> = doc.dom.select(&reddit_selectors.comments).collect();
    if !comments.is_empty() {
        output.push_str("<h2>Comments</h2>");
        for comment in comments
            .iter()
            .copied()
            .filter(|comment| comment_depth(*comment) == 0)
        {
            push_reddit_comment(&mut output, comment, &reddit_selectors, 0);
        }
    }

    output.push_str("</article>");
    Ok(Some(output))
}

fn is_reddit_discussion_path(doc: &ParsedDocument) -> bool {
    let Some(host) = doc.url.host_str() else {
        return false;
    };

    host_matches(host, "reddit.com") && doc.url.path().contains("/comments/")
}

fn post_author(post: ElementRef<'_>, user_selector: &Selector) -> Option<String> {
    post.value()
        .attr("author")
        .map(|author| format!("u/{author}"))
        .or_else(|| post.select(user_selector).next().map(element_text))
        .filter(|author| !author.is_empty())
}

fn post_score(post: ElementRef<'_>, score_selector: &Selector) -> Option<String> {
    post.value()
        .attr("score")
        .map(ToString::to_string)
        .or_else(|| {
            post.select(score_selector).find_map(|element| {
                element
                    .value()
                    .attr("score")
                    .map(ToString::to_string)
                    .or_else(|| {
                        let text = element_text(element);
                        (!text.is_empty()).then_some(text)
                    })
            })
        })
}

fn push_reddit_comment(
    output: &mut String,
    comment: ElementRef<'_>,
    selectors: &RedditSelectors,
    depth: usize,
) {
    if depth > 0 {
        output.push_str("<blockquote>");
    } else {
        output.push_str("<section class=\"chidori-reddit-comment\">");
    }

    let mut meta = Vec::new();
    if let Some(author) = comment_author(comment, selectors) {
        output.push_str("<p>");
        output.push_str(&encode_text(&author));
        output.push_str("</p>");
    }
    if let Some(score) = comment_score(comment, selectors) {
        meta.push(score);
    }
    if let Some(date) = comment
        .descendent_elements()
        .find(|element| {
            selectors.times.matches(element)
                && nearest_reddit_comment(*element, &selectors.wrappers) == Some(comment)
        })
        .map(element_text)
        .filter(|date| !date.is_empty())
    {
        meta.push(date);
    }
    push_meta_paragraph(output, &meta);

    if let Some(body) = comment.descendent_elements().find(|element| {
        selectors.body.matches(element)
            && nearest_reddit_comment(*element, &selectors.wrappers) == Some(comment)
    }) {
        output.push_str(&body.inner_html());
    }

    let child_depth = depth + 1;
    for reply in comment
        .select(&selectors.comments)
        .filter(|reply| comment_depth(*reply) == child_depth)
    {
        push_reddit_comment(output, reply, selectors, child_depth);
    }

    if depth > 0 {
        output.push_str("</blockquote>");
    } else {
        output.push_str("</section>");
    }
}

fn comment_author(comment: ElementRef<'_>, selectors: &RedditSelectors) -> Option<String> {
    comment
        .value()
        .attr("author")
        .map(|author| format!("u/{author}"))
        .or_else(|| {
            comment
                .descendent_elements()
                .find(|element| {
                    selectors.users.matches(element)
                        && nearest_reddit_comment(*element, &selectors.wrappers) == Some(comment)
                })
                .map(element_text)
        })
        .filter(|author| !author.is_empty())
}

fn comment_score(comment: ElementRef<'_>, selectors: &RedditSelectors) -> Option<String> {
    comment
        .value()
        .attr("score")
        .map(ToString::to_string)
        .or_else(|| {
            comment.descendent_elements().find_map(|element| {
                if nearest_reddit_comment(element, &selectors.wrappers) != Some(comment) {
                    return None;
                }
                let score = element.value().attr("score")?;
                if score.is_empty() {
                    let text = element_text(element);
                    (!text.is_empty()).then_some(text)
                } else {
                    Some(score.to_string())
                }
            })
        })
        .filter(|score| !score.is_empty())
}

fn comment_depth(comment: ElementRef<'_>) -> usize {
    comment
        .value()
        .attr("depth")
        .and_then(|depth| depth.parse().ok())
        .unwrap_or_else(|| {
            comment
                .ancestors()
                .filter_map(ElementRef::wrap)
                .filter(|ancestor| {
                    matches!(
                        ancestor.value().name(),
                        "shreddit-comment" | "div" | "article" | "section"
                    ) && (ancestor.value().name() == "shreddit-comment"
                        || ancestor.value().attr("data-testid") == Some("comment"))
                })
                .count()
        })
}

fn nearest_reddit_comment<'a>(
    element: ElementRef<'a>,
    comment_wrapper_selector: &Selector,
) -> Option<ElementRef<'a>> {
    element.ancestors().find_map(|ancestor| {
        ElementRef::wrap(ancestor).filter(|ancestor| comment_wrapper_selector.matches(ancestor))
    })
}
