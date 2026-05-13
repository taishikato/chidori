use crate::{document::ParsedDocument, error::ChidoriError};
use html_escape::encode_text;
use scraper::{ElementRef, Selector};

use super::super::{
    types::SiteExtraction,
    util::{
        element_text, host_matches, href_path_matches_target, normalize_href_path, normalize_text,
    },
};
use super::common::push_meta_paragraph;

#[derive(Clone, Copy)]
enum SocialThreadSite {
    Bluesky,
    Threads,
}

pub(super) fn extract(doc: &ParsedDocument) -> Result<Option<SiteExtraction>, ChidoriError> {
    Ok(known_site_content_candidate(doc)?
        .map(|(selector, html)| SiteExtraction::new(selector, html)))
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
    } else if host_matches(host, "lwn.net") {
        ".ArticleText"
    } else if host_matches(host, "bsky.app")
        || host_matches(host, "threads.net")
        || host_matches(host, "threads.com")
    {
        r#"main"#
    } else if host_matches(host, "linkedin.com") {
        r#"main article, article"#
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
    if let Some(site) = social_thread_site(doc, host) {
        if let Some(html) = social_thread_candidate(content, site, doc.url.path())? {
            return Ok(Some((content_selector.to_string(), html)));
        }
    }

    let title_selector = Selector::parse("h1, #firstHeading, .PageHeadline")
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

fn social_thread_site(doc: &ParsedDocument, host: &str) -> Option<SocialThreadSite> {
    let segments: Vec<_> = doc.url.path_segments()?.collect();

    if host_matches(host, "bsky.app") {
        (segments.len() >= 4
            && segments[0] == "profile"
            && !segments[1].is_empty()
            && segments[2] == "post"
            && !segments[3].is_empty())
        .then_some(SocialThreadSite::Bluesky)
    } else if host_matches(host, "threads.net") || host_matches(host, "threads.com") {
        (segments.len() >= 3
            && segments[0].starts_with('@')
            && segments[0].len() > 1
            && segments[1] == "post"
            && !segments[2].is_empty())
        .then_some(SocialThreadSite::Threads)
    } else {
        None
    }
}

fn social_thread_candidate(
    root: ElementRef<'_>,
    site: SocialThreadSite,
    target_path: &str,
) -> Result<Option<String>, ChidoriError> {
    let article_selector =
        Selector::parse("article").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let author_selector = Selector::parse(r#"a[href^="/profile/"], a[href^="/@"]"#)
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let handle_selector =
        Selector::parse("span").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let time_selector =
        Selector::parse("time").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let bluesky_body_selector = Selector::parse(r#"[data-testid="postText"]"#)
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let threads_body_selector =
        Selector::parse("div").map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let threads_chrome_selector = Selector::parse(r#"time, button, header, footer, nav"#)
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let threads_profile_link_selector = Selector::parse(r#"a[href^="/@"]"#)
        .map_err(|error| ChidoriError::Unknown(error.to_string()))?;
    let permalink_selector =
        Selector::parse(r#"a[href]"#).map_err(|error| ChidoriError::Unknown(error.to_string()))?;

    let mut output = String::from("<article class=\"chidori-social-thread\">");
    let mut post_count = 0;
    let articles = root
        .select(&article_selector)
        .filter(|article| nearest_social_article_parent(*article, &article_selector).is_none())
        .collect::<Vec<_>>();
    let start_index = articles.iter().position(|article| {
        article_matches_target_permalink(*article, &permalink_selector, target_path)
    });

    for article in articles
        .into_iter()
        .enumerate()
        .filter_map(|(index, article)| {
            if let Some(start_index) = start_index {
                (index >= start_index
                    && !article_has_other_post_permalink(
                        article,
                        site,
                        &permalink_selector,
                        target_path,
                    ))
                .then_some(article)
            } else {
                Some(article)
            }
        })
    {
        let Some(body) = social_thread_body(
            article,
            site,
            &article_selector,
            &bluesky_body_selector,
            &threads_body_selector,
            &threads_chrome_selector,
            &threads_profile_link_selector,
        ) else {
            continue;
        };

        if post_count > 0 {
            output.push_str("<blockquote>");
        } else {
            output.push_str("<section class=\"chidori-social-post\">");
        }

        if let Some(author) = article
            .descendent_elements()
            .find(|element| {
                author_selector.matches(element)
                    && nearest_social_article(*element, &article_selector) == Some(article)
            })
            .map(element_text)
            .filter(|author| !author.is_empty())
        {
            output.push_str("<p>");
            output.push_str(&encode_text(&author));
            output.push_str("</p>");
        }

        let mut meta = Vec::new();
        if let Some(handle) = article
            .descendent_elements()
            .find(|element| {
                handle_selector.matches(element)
                    && nearest_social_article(*element, &article_selector) == Some(article)
            })
            .map(element_text)
            .filter(|handle| handle.starts_with('@'))
        {
            meta.push(handle);
        }
        if let Some(date) = article
            .descendent_elements()
            .find(|element| {
                time_selector.matches(element)
                    && nearest_social_article(*element, &article_selector) == Some(article)
            })
            .map(element_text)
            .filter(|date| !date.is_empty())
        {
            meta.push(date);
        }
        push_meta_paragraph(&mut output, &meta);
        output.push_str(&body.inner_html());

        if post_count > 0 {
            output.push_str("</blockquote>");
        } else {
            output.push_str("</section>");
        }
        post_count += 1;
    }
    output.push_str("</article>");

    if post_count == 0 {
        Ok(None)
    } else {
        Ok(Some(output))
    }
}

fn article_matches_target_permalink(
    article: ElementRef<'_>,
    permalink_selector: &Selector,
    target_path: &str,
) -> bool {
    article.select(permalink_selector).any(|link| {
        link.value()
            .attr("href")
            .is_some_and(|href| href_path_matches_target(href, target_path))
    })
}

fn article_has_other_post_permalink(
    article: ElementRef<'_>,
    site: SocialThreadSite,
    permalink_selector: &Selector,
    target_path: &str,
) -> bool {
    article.select(permalink_selector).any(|link| {
        link.value().attr("href").is_some_and(|href| {
            href_looks_like_social_post(href, site) && !href_path_matches_target(href, target_path)
        })
    })
}

fn href_looks_like_social_post(href: &str, site: SocialThreadSite) -> bool {
    let path = normalize_href_path(href);
    match site {
        SocialThreadSite::Bluesky => path.contains("/profile/") && path.contains("/post/"),
        SocialThreadSite::Threads => path.starts_with("/@") && path.contains("/post/"),
    }
}

fn social_thread_body<'a>(
    article: ElementRef<'a>,
    site: SocialThreadSite,
    article_selector: &Selector,
    bluesky_body_selector: &Selector,
    threads_body_selector: &Selector,
    threads_chrome_selector: &Selector,
    threads_profile_link_selector: &Selector,
) -> Option<ElementRef<'a>> {
    match site {
        SocialThreadSite::Bluesky => article.descendent_elements().find(|element| {
            bluesky_body_selector.matches(element)
                && nearest_social_article(*element, article_selector) == Some(article)
                && !element_text(*element).is_empty()
        }),
        SocialThreadSite::Threads => article.descendent_elements().find(|element| {
            threads_body_selector.matches(element)
                && nearest_social_article(*element, article_selector) == Some(article)
                && !element_text(*element).is_empty()
                && element.select(threads_chrome_selector).next().is_none()
                && !is_threads_profile_only_block(*element, threads_profile_link_selector)
                && !has_threads_profile_only_child(
                    *element,
                    threads_body_selector,
                    threads_profile_link_selector,
                )
        }),
    }
}

fn has_threads_profile_only_child(
    element: ElementRef<'_>,
    body_selector: &Selector,
    profile_link_selector: &Selector,
) -> bool {
    element
        .select(body_selector)
        .any(|child| is_threads_profile_only_block(child, profile_link_selector))
}

fn is_threads_profile_only_block(
    element: ElementRef<'_>,
    profile_link_selector: &Selector,
) -> bool {
    let text = element_text(element);
    let profile_text = normalize_text(
        &element
            .select(profile_link_selector)
            .map(element_text)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
    );

    !profile_text.is_empty() && text == profile_text
}

fn nearest_social_article<'a>(
    element: ElementRef<'a>,
    article_selector: &Selector,
) -> Option<ElementRef<'a>> {
    element.ancestors().find_map(|ancestor| {
        ElementRef::wrap(ancestor).filter(|ancestor| article_selector.matches(ancestor))
    })
}

fn nearest_social_article_parent<'a>(
    article: ElementRef<'a>,
    article_selector: &Selector,
) -> Option<ElementRef<'a>> {
    article.ancestors().skip(1).find_map(|ancestor| {
        ElementRef::wrap(ancestor).filter(|ancestor| article_selector.matches(ancestor))
    })
}
