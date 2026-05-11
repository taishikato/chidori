use crate::{document::ParsedDocument, error::ChidoriError};
use html_escape::{encode_double_quoted_attribute, encode_text};
use scraper::{ElementRef, Selector};

const PRIMARY_ENTRY_SELECTORS: &[&str] = &[
    "#post",
    ".post-content",
    ".post-body",
    ".article-content",
    "#article-content",
    ".js-article-content",
    ".article_post",
    ".article-wrapper",
    ".entry-content",
    ".content-article",
    ".instapaper_body",
    ".post",
    ".js-discussion",
    ".pull-discussion-timeline",
    "#article-block",
    "#section-content",
    ".markdown-body",
    "article",
    "[role=\"article\"]",
    "main",
    "[role=\"main\"]",
    ".article-body",
    "#content",
    ".article",
    ".content-paragraph",
];

const BODY_FALLBACK_SELECTORS: &[&str] = &["body"];
const BROAD_RETRY_SELECTORS: &[&str] = &["main", "[role=\"main\"]", "article", "body"];
const LOW_WORD_COUNT_RETRY_THRESHOLD: usize = 50;
const RETRY_MIN_GAIN_MULTIPLIER: usize = 2;
const ARTICLE_RETRY_PROTECTION_MIN_WORDS: usize = 10;

#[derive(Debug, Clone)]
struct Candidate {
    score: isize,
    selector_index: usize,
    selector: String,
    word_count: usize,
    content_block_count: usize,
    html: String,
}

#[derive(Debug, Clone)]
pub struct ExtractedContent {
    pub html: String,
    pub selector: Option<String>,
    pub score: Option<isize>,
    pub fallbacks: Vec<String>,
}

struct ScoringSelectors {
    links: Selector,
    paragraphs: Selector,
    images: Selector,
    body_content_blocks: Selector,
}

struct RedditSelectors {
    comments: Selector,
    wrappers: Selector,
    body: Selector,
    users: Selector,
    times: Selector,
}

struct MastodonSelectors {
    statuses: Selector,
    threads: Selector,
    display_names: Selector,
    handles: Selector,
    times: Selector,
    bodies: Selector,
}

struct MicroblogSelectors {
    statuses: Selector,
    users: Selector,
    spans: Selector,
    links: Selector,
    times: Selector,
    bodies: Selector,
}

fn selector_priority(selector_count: usize, selector_index: usize) -> isize {
    ((selector_count - selector_index) * 40) as isize
}

pub fn extract_main_html(doc: &ParsedDocument) -> Result<String, ChidoriError> {
    extract_main_content(doc).map(|content| content.html)
}

pub fn extract_main_content(doc: &ParsedDocument) -> Result<ExtractedContent, ChidoriError> {
    if let Some(html) = youtube_watch_candidate(doc)? {
        return Ok(ExtractedContent {
            html,
            selector: Some("youtube-watch".to_string()),
            score: None,
            fallbacks: Vec::new(),
        });
    }

    if let Some(html) = microblog_status_thread_candidate(doc)? {
        return Ok(ExtractedContent {
            html,
            selector: Some("microblog-status-thread".to_string()),
            score: None,
            fallbacks: Vec::new(),
        });
    }

    if let Some(html) = mastodon_status_thread_candidate(doc)? {
        return Ok(ExtractedContent {
            html,
            selector: Some("mastodon-status-thread".to_string()),
            score: None,
            fallbacks: Vec::new(),
        });
    }

    if let Some(html) = repository_discussion_candidate(doc)? {
        return Ok(ExtractedContent {
            html,
            selector: Some("repository-discussion".to_string()),
            score: None,
            fallbacks: Vec::new(),
        });
    }

    if let Some((selector, html)) = known_site_content_candidate(doc)? {
        return Ok(ExtractedContent {
            html,
            selector: Some(selector),
            score: None,
            fallbacks: Vec::new(),
        });
    }

    if let Some(html) = hacker_news_listing_candidate(doc)? {
        return Ok(ExtractedContent {
            html,
            selector: Some("hacker-news-listing".to_string()),
            score: None,
            fallbacks: Vec::new(),
        });
    }

    if let Some(html) = reddit_discussion_candidate(doc)? {
        return Ok(ExtractedContent {
            html,
            selector: Some("reddit-discussion".to_string()),
            score: None,
            fallbacks: Vec::new(),
        });
    }

    let selectors = ScoringSelectors {
        links: Selector::parse("a").map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        paragraphs: Selector::parse("p")
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        images: Selector::parse("img").map_err(|error| ChidoriError::Unknown(error.to_string()))?,
        body_content_blocks: Selector::parse(
            "body > article, body > main, body > section, body > div",
        )
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?,
    };

    let mut fallback_steps = Vec::new();
    let mut best_candidate =
        best_candidate_for_selectors(doc, PRIMARY_ENTRY_SELECTORS, &selectors, false)?;

    if best_candidate
        .as_ref()
        .is_none_or(|candidate| candidate.word_count < LOW_WORD_COUNT_RETRY_THRESHOLD)
    {
        if let Some(hidden_candidate) =
            best_candidate_for_selectors(doc, PRIMARY_ENTRY_SELECTORS, &selectors, true)?
        {
            let use_hidden = best_candidate
                .as_ref()
                .is_none_or(|candidate| should_retry_with_body(candidate, &hidden_candidate));
            if use_hidden {
                best_candidate = Some(hidden_candidate);
            }
            if use_hidden {
                fallback_steps.push("hidden-content".to_string());
            }
        }
    }

    if best_candidate.is_none() {
        best_candidate =
            best_candidate_for_selectors(doc, BODY_FALLBACK_SELECTORS, &selectors, false)?;
    }

    let mut used_structured_content = false;
    if let Some(candidate) = best_candidate.as_ref() {
        if let Some(html) = structured_content_candidate(doc, candidate.word_count)? {
            used_structured_content = true;
            best_candidate = Some(Candidate {
                score: candidate.score,
                selector_index: candidate.selector_index,
                selector: "schema-org".to_string(),
                word_count: text_word_count(&html),
                content_block_count: candidate.content_block_count,
                html,
            });
        }
    }

    if !used_structured_content {
        if let Some(candidate) = best_candidate
            .as_ref()
            .filter(|candidate| candidate.word_count < LOW_WORD_COUNT_RETRY_THRESHOLD)
            .cloned()
        {
            let broad_retry_candidate =
                best_candidate_for_selectors(doc, BROAD_RETRY_SELECTORS, &selectors, false)?
                    .filter(|broad_candidate| should_retry_with_body(&candidate, broad_candidate));
            let body_retry_candidate =
                best_candidate_for_selectors(doc, BODY_FALLBACK_SELECTORS, &selectors, false)?
                    .filter(|body_candidate| should_retry_with_body(&candidate, body_candidate));

            let retry_candidate = match (broad_retry_candidate, body_retry_candidate) {
                (Some(broad_candidate), Some(body_candidate))
                    if should_retry_with_body(&broad_candidate, &body_candidate) =>
                {
                    Some(body_candidate)
                }
                (Some(broad_candidate), _) => Some(broad_candidate),
                (None, body_candidate) => body_candidate,
            };

            if let Some(retry_candidate) = retry_candidate {
                fallback_steps.push("low-word-selector-retry".to_string());
                best_candidate = Some(retry_candidate);
            }
        }
    }

    if let Some(candidate) = best_candidate {
        Ok(ExtractedContent {
            html: candidate.html,
            selector: Some(candidate.selector),
            score: Some(candidate.score),
            fallbacks: fallback_steps,
        })
    } else if let Some(html) = structured_content_candidate(doc, 0)? {
        Ok(ExtractedContent {
            html,
            selector: Some("schema-org".to_string()),
            score: None,
            fallbacks: vec!["schema-org".to_string()],
        })
    } else {
        Err(ChidoriError::ExtractionFailed)
    }
}

fn known_site_content_candidate(
    doc: &ParsedDocument,
) -> Result<Option<(String, String)>, ChidoriError> {
    let Some(host) = doc.url.host_str() else {
        return Ok(None);
    };

    let content_selector = if host_matches(host, "wikipedia.org") {
        "#mw-content-text"
    } else if host_matches(host, "medium.com") {
        "article"
    } else if host_matches(host, "substack.com") {
        "article, .body.markup, .available-content"
    } else if host_matches(host, "discourse.org") || host_matches(host, "discourse.group") {
        ".topic-post .cooked, #post_1 .cooked, article .cooked"
    } else if host_matches(host, "leetcode.com") {
        r#"[data-track-load="description_content"]"#
    } else {
        return Ok(None);
    };

    let selector = Selector::parse(content_selector)
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let Some(content) = doc
        .dom
        .select(&selector)
        .find(|content| !element_text(*content).is_empty())
    else {
        return Ok(None);
    };

    let title_selector = Selector::parse("h1, #firstHeading")
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let title = doc
        .dom
        .select(&title_selector)
        .map(element_text)
        .find(|title| !title.is_empty());

    let mut output = String::from("<article class=\"chidori-known-site-content\">");
    if let Some(title) = title.filter(|_| content.select(&title_selector).next().is_none()) {
        output.push_str("<h1>");
        output.push_str(&encode_text(&title));
        output.push_str("</h1>");
    }
    output.push_str(&content.inner_html());
    output.push_str("</article>");

    Ok(Some((content_selector.to_string(), output)))
}

fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
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
    let Some(host) = doc.url.host_str() else {
        return false;
    };
    if host != "github.com" && !host.ends_with(".github.com") {
        return false;
    }
    let segments = doc
        .url
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())
        .unwrap_or_default();

    segments.len() >= 4 && matches!(segments[2], "issues" | "pull")
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

    (host == "youtube.com" || host.ends_with(".youtube.com"))
        && doc.url.path() == "/watch"
        && doc
            .url
            .query_pairs()
            .any(|(key, value)| key == "v" && !value.is_empty())
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
    let is_microblog_host = matches!(host, "x.com" | "twitter.com")
        || host.ends_with(".x.com")
        || host.ends_with(".twitter.com");

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

    (host == "reddit.com" || host.ends_with(".reddit.com")) && doc.url.path().contains("/comments/")
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

fn push_meta_paragraph(output: &mut String, parts: &[String]) {
    if parts.is_empty() {
        return;
    }

    output.push_str("<p><small>");
    output.push_str(&encode_text(&parts.join(" · ")));
    output.push_str("</small></p>");
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

fn hacker_news_listing_candidate(doc: &ParsedDocument) -> Result<Option<String>, ChidoriError> {
    if doc.url.host_str() != Some("news.ycombinator.com") || !is_hacker_news_listing_path(doc) {
        return Ok(None);
    }

    let row_selector =
        Selector::parse("tr.athing").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let title_selector = Selector::parse(".titleline a")
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let site_selector =
        Selector::parse(".sitestr").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let subtext_selector =
        Selector::parse("td.subtext").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let score_selector =
        Selector::parse(".score").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let user_selector =
        Selector::parse(".hnuser").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let age_selector =
        Selector::parse(".age a").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let link_selector =
        Selector::parse("a").map_err(|error| ChidoriError::Unknown(error.to_string()))?;

    let story_rows: Vec<_> = doc.dom.select(&row_selector).collect();
    if story_rows.is_empty() {
        return Ok(None);
    }

    let subtext_cells: Vec<_> = doc.dom.select(&subtext_selector).collect();
    let mut output = String::from("<ol class=\"chidori-hn-listing\">");
    let mut story_count = 0;

    for (index, row) in story_rows.into_iter().enumerate() {
        let Some(title_link) = row.select(&title_selector).next() else {
            continue;
        };
        let title = element_text(title_link);
        if title.is_empty() {
            continue;
        }

        let href = title_link.value().attr("href").unwrap_or("");
        let story_url = resolve_url(doc, href);
        let site = row.select(&site_selector).next().map(element_text);
        let subtext = subtext_cells.get(index).copied();

        output.push_str("<li>");
        push_link(&mut output, &story_url, &title);
        if let Some(site) = site.filter(|site| !site.is_empty()) {
            output.push_str(" <span class=\"site\">(");
            output.push_str(&encode_text(&site));
            output.push_str(")</span>");
        }

        let mut meta_parts = Vec::new();
        if let Some(subtext) = subtext {
            if let Some(score) = subtext
                .select(&score_selector)
                .next()
                .map(element_text)
                .filter(|score| !score.is_empty())
            {
                meta_parts.push(encode_text(&score).to_string());
            }
            if let Some(user) = subtext
                .select(&user_selector)
                .next()
                .map(element_text)
                .filter(|user| !user.is_empty())
            {
                meta_parts.push(format!("by {}", encode_text(&user)));
            }
            if let Some(age) = subtext
                .select(&age_selector)
                .next()
                .map(element_text)
                .filter(|age| !age.is_empty())
            {
                meta_parts.push(encode_text(&age).to_string());
            }
            if let Some((comments_url, comments_text)) = comments_link(doc, subtext, &link_selector)
            {
                let mut comments = String::new();
                push_link(&mut comments, &comments_url, &comments_text);
                meta_parts.push(comments);
            }
        }

        if !meta_parts.is_empty() {
            output.push_str("<br><small>");
            output.push_str(&meta_parts.join(" · "));
            output.push_str("</small>");
        }
        output.push_str("</li>");
        story_count += 1;
    }

    output.push_str("</ol>");

    if story_count == 0 {
        Ok(None)
    } else {
        Ok(Some(output))
    }
}

fn is_hacker_news_listing_path(doc: &ParsedDocument) -> bool {
    matches!(
        doc.url.path(),
        "/" | "/news" | "/newest" | "/front" | "/ask" | "/show" | "/jobs" | "/submitted"
    )
}

fn comments_link(
    doc: &ParsedDocument,
    subtext: ElementRef<'_>,
    link_selector: &Selector,
) -> Option<(String, String)> {
    subtext
        .select(link_selector)
        .filter_map(|link| {
            let text = element_text(link);
            let href = link.value().attr("href")?;
            let is_comment_link = text == "discuss" || text.contains("comment");
            is_comment_link.then(|| (resolve_url(doc, href), text))
        })
        .last()
}

fn push_link(output: &mut String, url: &str, text: &str) {
    output.push_str("<a href=\"");
    output.push_str(&encode_double_quoted_attribute(url));
    output.push_str("\">");
    output.push_str(&encode_text(text));
    output.push_str("</a>");
}

fn resolve_url(doc: &ParsedDocument, href: &str) -> String {
    doc.url
        .join(href)
        .map(|url| url.to_string())
        .unwrap_or_else(|_| href.to_string())
}

fn element_text(element: ElementRef<'_>) -> String {
    normalize_text(&element.text().collect::<Vec<_>>().join(" "))
}

fn structured_content_candidate(
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

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text_word_count(html: &str) -> usize {
    scraper::Html::parse_fragment(html)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .count()
}

fn is_protected_article_candidate(candidate: &Candidate) -> bool {
    if candidate.word_count < ARTICLE_RETRY_PROTECTION_MIN_WORDS {
        return false;
    }

    let html = candidate.html.to_ascii_lowercase();
    html.contains("<article") || html.contains("role=\"article\"")
}

fn should_retry_with_body(candidate: &Candidate, body_candidate: &Candidate) -> bool {
    if body_candidate.word_count <= candidate.word_count * RETRY_MIN_GAIN_MULTIPLIER {
        return false;
    }

    if !is_protected_article_candidate(candidate) {
        return true;
    }

    body_candidate.content_block_count > 1
}

#[cfg(test)]
fn score_element(
    element: ElementRef<'_>,
    selectors: &ScoringSelectors,
) -> (isize, usize, usize, usize) {
    score_element_with_visibility(element, selectors, false)
}

fn score_element_with_visibility(
    element: ElementRef<'_>,
    selectors: &ScoringSelectors,
    include_hidden: bool,
) -> (isize, usize, usize, usize) {
    let text = if include_hidden {
        element.text().collect::<Vec<_>>().join(" ")
    } else {
        text_without_invisible_nodes(&element.html())
    };
    let word_count = text.split_whitespace().count();
    let paragraph_count = element.select(&selectors.paragraphs).count();
    let content_block_count = element.select(&selectors.body_content_blocks).count();
    if word_count == 0 {
        return (0, 0, paragraph_count, content_block_count);
    }

    let comma_count = text.matches(',').count();
    let image_count = element.select(&selectors.images).count();
    let mut score = word_count as isize;
    score += (paragraph_count * 10) as isize;
    score += comma_count as isize;

    let class_attr = element
        .value()
        .attr("class")
        .unwrap_or("")
        .to_ascii_lowercase();
    let id_attr = element
        .value()
        .attr("id")
        .unwrap_or("")
        .to_ascii_lowercase();
    if class_attr.contains("content")
        || class_attr.contains("article")
        || class_attr.contains("post")
        || id_attr.contains("content")
        || id_attr.contains("article")
        || id_attr.contains("post")
    {
        score += 15;
    }

    if text.contains(" by ") || text.contains("By ") {
        score += 10;
    }
    if text.contains("202")
        || text.contains("Jan ")
        || text.contains("Feb ")
        || text.contains("Mar ")
    {
        score += 10;
    }

    score -= (image_count * 3) as isize;

    let link_text_len: usize = element
        .select(&selectors.links)
        .map(|link| link.text().collect::<Vec<_>>().join(" ").len())
        .sum();
    let text_len = text.len().max(1);
    let link_density = (link_text_len as f64 / text_len as f64).min(0.5);
    score = ((score as f64) * (1.0 - link_density)).round() as isize;

    (score, word_count, paragraph_count, content_block_count)
}

fn text_without_invisible_nodes(html: &str) -> String {
    let cleaned =
        crate::cleaner::clean_html(html, &crate::cleaner::CleanOptions { no_images: false });

    scraper::Html::parse_fragment(&cleaned)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
}

fn best_candidate_for_selectors(
    doc: &ParsedDocument,
    raw_selectors: &[&str],
    selectors: &ScoringSelectors,
    include_hidden: bool,
) -> Result<Option<Candidate>, ChidoriError> {
    let mut best_candidate: Option<Candidate> = None;

    for (selector_index, raw_selector) in raw_selectors.iter().enumerate() {
        let selector = Selector::parse(raw_selector)
            .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
        for element in doc.dom.select(&selector) {
            let (content_score, word_count, _paragraph_count, content_block_count) =
                score_element_with_visibility(element, selectors, include_hidden);
            if word_count == 0 {
                continue;
            }
            let score = selector_priority(raw_selectors.len(), selector_index) + content_score;
            let candidate = Candidate {
                score,
                selector_index,
                selector: (*raw_selector).to_string(),
                word_count,
                content_block_count,
                html: element.html(),
            };
            if best_candidate.as_ref().is_none_or(|best_candidate| {
                candidate.score > best_candidate.score
                    || (candidate.score == best_candidate.score
                        && candidate.selector_index < best_candidate.selector_index)
                    || (candidate.score == best_candidate.score
                        && candidate.selector_index == best_candidate.selector_index
                        && candidate.word_count > best_candidate.word_count)
            }) {
                best_candidate = Some(candidate);
            }
        }
    }

    Ok(best_candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ParsedDocument;
    use scraper::Html;
    use url::Url;

    fn scoring_selectors() -> ScoringSelectors {
        ScoringSelectors {
            links: Selector::parse("a").unwrap(),
            paragraphs: Selector::parse("p").unwrap(),
            images: Selector::parse("img").unwrap(),
            body_content_blocks: Selector::parse(
                "body > article, body > main, body > section, body > div",
            )
            .unwrap(),
        }
    }

    fn score_first(html: &str, selector: &str) -> isize {
        let dom = Html::parse_document(html);
        let selector = Selector::parse(selector).unwrap();
        let element = dom.select(&selector).next().unwrap();
        score_element(element, &scoring_selectors()).0
    }

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
    fn score_element_rewards_paragraph_density() {
        let paragraph_score = score_first(
            r#"<section><p>One useful paragraph with clear prose.</p><p>Another useful paragraph with detail.</p></section>"#,
            "section",
        );
        let loose_score = score_first(
            r#"<section>Loose words words words words words words words words words words words.</section>"#,
            "section",
        );

        assert!(paragraph_score > loose_score);
    }

    #[test]
    fn score_element_rewards_content_class_names() {
        let content_score = score_first(
            r#"<section class="article-content">Short focused article text.</section>"#,
            "section",
        );
        let plain_score = score_first(
            r#"<section>Short focused article text.</section>"#,
            "section",
        );

        assert!(content_score > plain_score);
    }

    #[test]
    fn score_element_penalizes_link_text_density() {
        let link_score = score_first(
            r#"<section><a href="/one">Alpha beta gamma, delta epsilon zeta, eta theta.</a></section>"#,
            "section",
        );
        let prose_score = score_first(
            r#"<section>Alpha beta gamma, delta epsilon zeta, eta theta.</section>"#,
            "section",
        );

        assert!(prose_score > link_score);
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
