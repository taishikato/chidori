use kuchiki::traits::TendrilSink;
use scraper::{Html, Selector};
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct CleanOptions {
    pub no_images: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovalRecord {
    pub step: String,
    pub reason: String,
    pub selector: String,
    pub count: usize,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub text_preview: String,
}

#[derive(Debug, Clone)]
pub struct CleanResult {
    pub html: String,
    pub removals: Vec<RemovalRecord>,
}

pub fn clean_html(html: &str, options: &CleanOptions) -> String {
    clean_html_with_report(html, options).html
}

pub fn clean_html_with_report(html: &str, options: &CleanOptions) -> CleanResult {
    clean_html_inner(html, options, true)
}

pub fn clean_html_preserving_hidden_with_report(html: &str, options: &CleanOptions) -> CleanResult {
    clean_html_inner(html, options, false)
}

fn parse_fragment_document(html: &str) -> kuchiki::NodeRef {
    let html = normalize_unquoted_self_closing_slashes(html);
    kuchiki::parse_html().one(format!("<html><body>{html}</body></html>"))
}

fn serialize_body_inner(document: &kuchiki::NodeRef) -> String {
    let Ok(body) = document.select_first("body") else {
        return String::new();
    };
    body.as_node()
        .children()
        .map(|child| child.to_string())
        .collect::<String>()
}

fn normalize_unquoted_self_closing_slashes(html: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(index) = rest.find('<') {
        output.push_str(&rest[..index]);
        let candidate = &rest[index..];
        let Some(end) = opening_tag_end(candidate) else {
            output.push_str(candidate);
            return output;
        };
        let opening_tag = &candidate[..=end];
        output.push_str(&normalize_self_closing_opening_tag(opening_tag));
        rest = &candidate[end + 1..];
    }

    output.push_str(rest);
    output
}

fn normalize_self_closing_opening_tag(opening_tag: &str) -> String {
    let Some(input) = opening_tag
        .strip_prefix('<')
        .and_then(|value| value.strip_suffix('>'))
    else {
        return opening_tag.to_string();
    };
    let input = input.trim_start();
    if input.starts_with('/') || input.starts_with('!') || input.starts_with('?') {
        return opening_tag.to_string();
    }

    let name_end = input
        .find(|ch: char| ch.is_ascii_whitespace() || ch == '/')
        .unwrap_or(input.len());
    let tag_name = &input[..name_end];
    if !is_void_element(tag_name) || !opening_tag.ends_with("/>") {
        return opening_tag.to_string();
    }

    let Some(before_slash) = opening_tag.strip_suffix("/>") else {
        return opening_tag.to_string();
    };
    if trailing_unquoted_attribute_name(before_slash).is_none_or(is_url_attribute) {
        return opening_tag.to_string();
    }

    format!("{before_slash} />")
}

fn is_void_element(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn clean_html_inner(html: &str, options: &CleanOptions, remove_hidden: bool) -> CleanResult {
    let document = parse_fragment_document(html);
    let mut removals = Vec::new();

    for tag in [
        "script", "style", "noscript", "nav", "footer", "aside", "button", "form", "iframe",
        "object", "embed",
    ] {
        remove_dom_selector(&document, tag, "noise-tag", tag, &mut removals);
    }
    if remove_hidden {
        remove_dom_selector(
            &document,
            "[hidden], [aria-hidden=\"true\"], .hidden, .sr-only",
            "hidden-element",
            "[hidden], [aria-hidden=\"true\"], .hidden, .sr-only",
            &mut removals,
        );
        remove_dom_hidden_style_elements(&document, &mut removals);
    }
    unwrap_dom_javascript_links(&document);
    strip_dom_dangerous_attributes(&document);
    remove_dom_related_card_sections(&document, &mut removals);

    let mut cleaned = serialize_body_inner(&document);

    let next = remove_navigation_like_blocks(&cleaned);
    push_removal_if_changed(
        &mut removals,
        &cleaned,
        &next,
        "navigation-like-block",
        "[data-block=\"nav\"], .breadcrumb, .toc",
    );
    cleaned = next;
    let next = remove_footer_like_blocks(&cleaned);
    push_removal_if_changed(
        &mut removals,
        &cleaned,
        &next,
        "footer-like-block",
        ".footer, [role=\"contentinfo\"]",
    );
    cleaned = next;
    let next = remove_fragment_only_link_lists(&cleaned);
    push_removal_if_changed(&mut removals, &cleaned, &next, "fragment-link-list", "ul");
    cleaned = next;
    let next = remove_related_card_sections(&cleaned);
    push_removal_if_changed(
        &mut removals,
        &cleaned,
        &next,
        "related-card-section",
        "section, div",
    );
    cleaned = next;
    let next = remove_link_dense_related_sections(&cleaned);
    push_removal_if_changed(
        &mut removals,
        &cleaned,
        &next,
        "link-dense-related-section",
        "section",
    );
    cleaned = next;

    if options.no_images {
        let next = remove_tag(&cleaned, "img");
        push_removal_if_changed(&mut removals, &cleaned, &next, "image-disabled", "img");
        cleaned = next;
        let next = remove_tag(&cleaned, "picture");
        push_removal_if_changed(&mut removals, &cleaned, &next, "image-disabled", "picture");
        cleaned = next;
    }
    CleanResult {
        html: cleaned,
        removals,
    }
}

fn push_removal_if_changed(
    removals: &mut Vec<RemovalRecord>,
    before: &str,
    after: &str,
    reason: &str,
    selector: &str,
) {
    if before == after {
        return;
    }

    removals.push(RemovalRecord {
        step: "clean-html".to_string(),
        reason: reason.to_string(),
        selector: selector.to_string(),
        count: removed_element_count(before, after, selector).max(1),
        text_preview: removed_text_preview(before, after, selector),
    });
}

fn removed_text_preview(before: &str, after: &str, selector: &str) -> String {
    let Ok(selector) = Selector::parse(selector) else {
        return String::new();
    };
    let before_dom = Html::parse_fragment(before);
    let after_dom = Html::parse_fragment(after);
    let after_texts = after_dom
        .select(&selector)
        .map(|node| normalize_preview_text(&node.text().collect::<Vec<_>>().join(" ")))
        .collect::<Vec<_>>();

    before_dom
        .select(&selector)
        .map(|node| text_preview_from_html(&node.html()))
        .find(|preview| {
            !preview.is_empty()
                && !after_texts
                    .iter()
                    .any(|text| text.contains(preview.as_str()))
        })
        .unwrap_or_default()
}

fn remove_dom_selector(
    document: &kuchiki::NodeRef,
    selector: &str,
    reason: &str,
    report_selector: &str,
    removals: &mut Vec<RemovalRecord>,
) {
    let Ok(matches) = document.select(selector) else {
        return;
    };
    let nodes = matches
        .map(|matched| matched.as_node().clone())
        .collect::<Vec<_>>();
    let count = nodes.len();
    if count == 0 {
        return;
    }
    let text_preview = nodes
        .iter()
        .map(text_preview_from_node)
        .find(|preview| !preview.is_empty())
        .unwrap_or_default();
    for node in nodes {
        node.detach();
    }
    removals.push(RemovalRecord {
        step: "clean-html".to_string(),
        reason: reason.to_string(),
        selector: report_selector.to_string(),
        count,
        text_preview,
    });
}

fn unwrap_dom_javascript_links(document: &kuchiki::NodeRef) {
    let Ok(matches) = document.select("a[href]") else {
        return;
    };
    let nodes = matches
        .filter_map(|matched| {
            let attrs = matched.attributes.borrow();
            let href = attrs.get("href")?;
            href.trim_start()
                .to_ascii_lowercase()
                .starts_with("javascript:")
                .then(|| matched.as_node().clone())
        })
        .collect::<Vec<_>>();

    for node in nodes {
        let children = node.children().collect::<Vec<_>>();
        for child in children {
            node.insert_before(child);
        }
        node.detach();
    }
}

fn remove_dom_hidden_style_elements(
    document: &kuchiki::NodeRef,
    removals: &mut Vec<RemovalRecord>,
) {
    let Ok(matches) = document.select("[style]") else {
        return;
    };
    let nodes = matches
        .filter_map(|matched| {
            let attrs = matched.attributes.borrow();
            let style = attrs.get("style")?;
            is_hidden_style_value(style).then(|| matched.as_node().clone())
        })
        .collect::<Vec<_>>();
    let count = nodes.len();
    if count == 0 {
        return;
    }
    let text_preview = nodes
        .iter()
        .map(text_preview_from_node)
        .find(|preview| !preview.is_empty())
        .unwrap_or_default();
    for node in nodes {
        node.detach();
    }
    removals.push(RemovalRecord {
        step: "clean-html".to_string(),
        reason: "hidden-element".to_string(),
        selector: "[style]".to_string(),
        count,
        text_preview,
    });
}

fn remove_dom_related_card_sections(
    document: &kuchiki::NodeRef,
    removals: &mut Vec<RemovalRecord>,
) {
    let Ok(matches) = document.select("section, div") else {
        return;
    };
    let nodes = matches
        .filter_map(|matched| {
            is_related_card_section(&matched.as_node().to_string())
                .then(|| matched.as_node().clone())
        })
        .collect::<Vec<_>>();
    let nodes = deepest_nodes(nodes);
    let count = nodes.len();
    if count == 0 {
        return;
    }
    let text_preview = nodes
        .iter()
        .map(text_preview_from_node)
        .find(|preview| !preview.is_empty())
        .unwrap_or_default();
    for node in nodes {
        node.detach();
    }
    removals.push(RemovalRecord {
        step: "clean-html".to_string(),
        reason: "related-card-section".to_string(),
        selector: "section, div".to_string(),
        count,
        text_preview,
    });
}

fn deepest_nodes(nodes: Vec<kuchiki::NodeRef>) -> Vec<kuchiki::NodeRef> {
    nodes
        .iter()
        .filter(|node| !nodes.iter().any(|other| node_contains(node, other)))
        .cloned()
        .collect()
}

fn node_contains(parent: &kuchiki::NodeRef, child: &kuchiki::NodeRef) -> bool {
    parent != child && child.ancestors().any(|ancestor| ancestor == *parent)
}

fn text_preview_from_html(html: &str) -> String {
    let dom = Html::parse_fragment(html);
    normalize_preview_text(&dom.root_element().text().collect::<Vec<_>>().join(" "))
}

fn text_preview_from_node(node: &kuchiki::NodeRef) -> String {
    normalize_preview_text(&node.text_contents())
}

fn normalize_preview_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(200)
        .collect()
}

fn is_hidden_style_value(value: &str) -> bool {
    let normalized = value
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    normalized.contains("display:none") || normalized.contains("visibility:hidden")
}

fn strip_dom_dangerous_attributes(document: &kuchiki::NodeRef) {
    let Ok(matches) = document.select("*") else {
        return;
    };
    for matched in matches {
        let mut attrs = matched.attributes.borrow_mut();
        let names = attrs
            .map
            .keys()
            .map(|name| name.local.to_string())
            .collect::<Vec<_>>();
        for name in names {
            let value = attrs.get(name.as_str()).map(ToString::to_string);
            if is_dangerous_attribute(&name, value.as_deref()) {
                attrs.remove(name.as_str());
            }
        }
    }
}

fn removed_element_count(before: &str, after: &str, selector: &str) -> usize {
    let Ok(selector) = Selector::parse(selector) else {
        return 0;
    };
    let before = Html::parse_fragment(before);
    let after = Html::parse_fragment(after);

    before
        .select(&selector)
        .count()
        .saturating_sub(after.select(&selector).count())
}

fn remove_navigation_like_blocks(html: &str) -> String {
    let mut cleaned = html.to_string();
    for tag in ["div", "section"] {
        cleaned = remove_matching_tags_where(&cleaned, tag, is_navigation_like_opening_tag);
    }
    cleaned
}

fn is_navigation_like_opening_tag(opening_tag: &str) -> bool {
    attribute_value_eq(opening_tag, "data-block", "nav")
        || has_class_token(opening_tag, "breadcrumb")
        || has_class_token(opening_tag, "breadcrumbs")
        || has_class_token(opening_tag, "toc")
        || has_class_token(opening_tag, "toc-panel")
}

fn remove_footer_like_blocks(html: &str) -> String {
    let mut cleaned = html.to_string();
    for tag in ["div", "section", "header"] {
        cleaned = remove_matching_tags_where(&cleaned, tag, is_footer_like_opening_tag);
    }
    cleaned
}

fn is_footer_like_opening_tag(opening_tag: &str) -> bool {
    attribute_value_eq(opening_tag, "role", "contentinfo")
        || has_class_token(opening_tag, "footer")
        || has_class_token(opening_tag, "site-footer")
        || has_class_token(opening_tag, "page-footer")
        || has_class_token(opening_tag, "global-footer")
        || class_tokens(opening_tag).any(is_footer_link_class)
}

fn is_footer_link_class(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "footer_links_wrap"
            | "footer-links-wrap"
            | "footer_links_layout"
            | "footer-links-layout"
            | "footer_links_col"
            | "footer-links-col"
            | "footer_links_list_wrap"
            | "footer-links-list-wrap"
            | "footer_links_list"
            | "footer-links-list"
            | "footer_footer"
            | "footer-footer"
            | "footer_social_icon_wrap"
            | "footer-social-icon-wrap"
            | "footer_anthropic_link"
            | "footer-anthropic-link"
            | "footer_copyright"
            | "footer-copyright"
    )
}

fn remove_fragment_only_link_lists(html: &str) -> String {
    remove_matching_tags_by_content(html, "ul", |fragment| {
        let dom = Html::parse_fragment(fragment);
        let link_selector = Selector::parse("a").unwrap();
        let links = dom.select(&link_selector).collect::<Vec<_>>();
        if links.len() < 3
            || !links.iter().all(|link| {
                link.value()
                    .attr("href")
                    .is_some_and(|href| href.starts_with('#'))
            })
        {
            return false;
        }

        let text = dom.root_element().text().collect::<Vec<_>>().join(" ");
        let text_len = text.split_whitespace().collect::<String>().len();
        let link_text_len: usize = links
            .iter()
            .map(|link| {
                link.text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .split_whitespace()
                    .collect::<String>()
                    .len()
            })
            .sum();

        text_len > 0 && (link_text_len as f64 / text_len as f64) > 0.85
    })
}

fn remove_related_card_sections(html: &str) -> String {
    let mut cleaned = html.to_string();
    for tag in ["section", "div"] {
        cleaned = remove_matching_tags_by_content(&cleaned, tag, is_related_card_section);
    }
    cleaned
}

fn is_related_card_section(fragment: &str) -> bool {
    let dom = Html::parse_fragment(fragment);
    let root = dom.root_element();
    let text = root.text().collect::<Vec<_>>().join(" ");
    let word_count = text.split_whitespace().count();
    if word_count == 0 || word_count > 220 {
        return false;
    }

    let heading_selector = Selector::parse("h1, h2, h3, h4, h5, h6").unwrap();
    let has_related_heading = root.select(&heading_selector).any(|heading| {
        let heading_text = heading.text().collect::<Vec<_>>().join(" ");
        is_related_heading(&heading_text)
    });
    if !has_related_heading {
        return false;
    }

    let link_selector = Selector::parse("a").unwrap();
    let links = root.select(&link_selector).collect::<Vec<_>>();
    if links.len() < 2 {
        return false;
    }

    let link_text_len: usize = links
        .iter()
        .map(|link| link.text().collect::<Vec<_>>().join(" ").trim().len())
        .sum();
    let text_len = text.trim().len().max(1);
    let heading_count = root.select(&heading_selector).count();
    let link_text_ratio = link_text_len as f64 / text_len as f64;

    link_text_ratio > 0.35 || heading_count >= 3
}

fn is_related_heading(text: &str) -> bool {
    let normalized = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();

    matches!(
        normalized.as_str(),
        "related plugins"
            | "related posts"
            | "related articles"
            | "related content"
            | "related stories"
            | "related reads"
            | "read next"
            | "more articles"
            | "more posts"
            | "further reading"
            | "see also"
    )
}

fn remove_link_dense_related_sections(html: &str) -> String {
    remove_matching_tags_by_content_and_tail(html, "section", |fragment, tail| {
        if tail_has_meaningful_text(tail) {
            return false;
        }

        let dom = Html::parse_fragment(fragment);
        let link_selector = Selector::parse("a").unwrap();
        let text = dom.root_element().text().collect::<Vec<_>>().join(" ");
        let text_len = text.trim().len();
        let word_count = text.split_whitespace().count();
        if text_len == 0 || word_count > 80 {
            return false;
        }

        let links = dom.select(&link_selector).collect::<Vec<_>>();
        if links.len() < 2 {
            return false;
        }

        let link_text_len: usize = links
            .iter()
            .map(|link| link.text().collect::<Vec<_>>().join(" ").len())
            .sum();
        (link_text_len as f64 / text_len as f64) > 0.45
    })
}

fn is_url_attribute(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "href" | "src" | "action" | "formaction" | "xlink:href"
    )
}

fn opening_tag_end(input: &str) -> Option<usize> {
    let mut quote: Option<char> = None;

    for (index, character) in input.char_indices() {
        match quote {
            Some(current) if character == current => quote = None,
            Some(_) => {}
            None if character == '"' || character == '\'' => quote = Some(character),
            None if character == '>' => return Some(index),
            None => {}
        }
    }

    None
}

fn trailing_unquoted_attribute_name(input: &str) -> Option<&str> {
    let value_start = input.rfind('=')? + 1;
    let value = input[value_start..].trim_start();
    if value.is_empty() || value.starts_with('"') || value.starts_with('\'') {
        return None;
    }

    let name_end = input[..value_start - 1].trim_end().len();
    let name_start = input[..name_end]
        .rfind(|character: char| character.is_ascii_whitespace() || character == '/')
        .map_or(0, |index| index + 1);
    (name_start < name_end).then_some(&input[name_start..name_end])
}

fn is_dangerous_attribute(name: &str, value: Option<&str>) -> bool {
    let name = name.to_ascii_lowercase();
    if name.starts_with("on") || name == "srcdoc" {
        return true;
    }

    is_url_attribute(&name) && value.is_some_and(is_dangerous_url)
}

fn is_dangerous_url(value: &str) -> bool {
    let normalized = value
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace() && !ch.is_control())
        .collect::<String>()
        .to_ascii_lowercase();
    normalized.starts_with("javascript:") || normalized.starts_with("data:text/html")
}

fn has_class_token(opening_tag: &str, expected: &str) -> bool {
    class_tokens(opening_tag).any(|token| token.eq_ignore_ascii_case(expected))
}

fn class_tokens(opening_tag: &str) -> impl Iterator<Item = &str> {
    attribute_values(opening_tag, "class").flat_map(|value| value.split_ascii_whitespace())
}

fn attribute_value_eq(opening_tag: &str, expected: &str, expected_value: &str) -> bool {
    attribute_values(opening_tag, expected)
        .any(|value| value.trim().eq_ignore_ascii_case(expected_value))
}

fn attribute_values<'a>(
    opening_tag: &'a str,
    expected: &'a str,
) -> impl Iterator<Item = &'a str> + 'a {
    opening_attributes(opening_tag).filter_map(move |(name, value)| {
        name.eq_ignore_ascii_case(expected)
            .then_some(value)
            .flatten()
    })
}

fn opening_attributes(opening_tag: &str) -> OpeningAttributes<'_> {
    let input = opening_tag
        .strip_prefix('<')
        .and_then(|value| value.strip_suffix('>'))
        .unwrap_or(opening_tag);
    let input = input.trim_start().trim_end_matches('/').trim_end();
    let name_end = input
        .find(|ch: char| ch.is_ascii_whitespace() || ch == '/')
        .unwrap_or(input.len());

    OpeningAttributes {
        input: &input[name_end..],
    }
}

struct OpeningAttributes<'a> {
    input: &'a str,
}

impl<'a> Iterator for OpeningAttributes<'a> {
    type Item = (&'a str, Option<&'a str>);

    fn next(&mut self) -> Option<Self::Item> {
        self.input = self.input.trim_start();
        if self.input.is_empty() || self.input.starts_with('/') {
            return None;
        }

        let name_end = self
            .input
            .find(|ch: char| ch.is_ascii_whitespace() || ch == '=' || ch == '/')
            .unwrap_or(self.input.len());
        if name_end == 0 {
            self.input = &self.input[1..];
            return self.next();
        }

        let name = &self.input[..name_end];
        let mut rest = self.input[name_end..].trim_start();
        if !rest.starts_with('=') {
            self.input = rest;
            return Some((name, None));
        }

        rest = rest[1..].trim_start();
        if rest.is_empty() {
            self.input = rest;
            return Some((name, Some("")));
        }

        if let Some(quote) = rest
            .chars()
            .next()
            .filter(|quote| matches!(quote, '"' | '\''))
        {
            let value = &rest[quote.len_utf8()..];
            if let Some(end) = value.find(quote) {
                self.input = &value[end + quote.len_utf8()..];
                return Some((name, Some(&value[..end])));
            }
            self.input = "";
            return Some((name, Some(value)));
        }

        let value_end = rest
            .find(|ch: char| ch.is_ascii_whitespace() || ch == '>')
            .unwrap_or(rest.len());
        self.input = &rest[value_end..];
        Some((name, Some(&rest[..value_end])))
    }
}

fn tail_has_meaningful_text(html: &str) -> bool {
    let dom = Html::parse_fragment(html);
    dom.root_element()
        .text()
        .any(|text| !text.trim().is_empty())
}

fn remove_tag(html: &str, tag: &str) -> String {
    remove_matching_tags(html, tag)
}

fn remove_matching_tags(html: &str, tag: &str) -> String {
    remove_matching_tags_where(html, tag, |_| true)
}

fn remove_matching_tags_where(
    html: &str,
    tag: &str,
    should_remove: impl Fn(&str) -> bool,
) -> String {
    let mut output = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(index) = rest.find('<') {
        output.push_str(&rest[..index]);
        let candidate = &rest[index..];
        let after_name = &candidate[1..];
        if tag_name_matches(after_name, tag) {
            if let Some(end) = candidate.find('>') {
                let opening_tag = &candidate[..=end];
                if !should_remove(opening_tag) {
                    output.push_str(opening_tag);
                    rest = &candidate[end + 1..];
                    continue;
                }
                if !candidate[..=end].ends_with("/>") {
                    if let Some(closing_end) = find_matching_close(candidate, tag, end + 1) {
                        rest = &candidate[closing_end..];
                        continue;
                    }
                }
                rest = &candidate[end + 1..];
                continue;
            }
        }

        output.push('<');
        rest = &candidate[1..];
    }

    output.push_str(rest);
    output
}

fn remove_matching_tags_by_content(
    html: &str,
    tag: &str,
    should_remove: impl Fn(&str) -> bool,
) -> String {
    remove_matching_tags_by_content_and_tail(html, tag, |fragment, _| should_remove(fragment))
}

fn remove_matching_tags_by_content_and_tail(
    html: &str,
    tag: &str,
    should_remove: impl Fn(&str, &str) -> bool,
) -> String {
    let mut output = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(index) = rest.find('<') {
        output.push_str(&rest[..index]);
        let candidate = &rest[index..];
        let after_name = &candidate[1..];
        if tag_name_matches(after_name, tag) {
            if let Some(end) = candidate.find('>') {
                if !candidate[..=end].ends_with("/>") {
                    if let Some(closing_end) = find_matching_close(candidate, tag, end + 1) {
                        let fragment = &candidate[..closing_end];
                        let tail = &candidate[closing_end..];
                        if should_remove(fragment, tail) {
                            rest = &candidate[closing_end..];
                            continue;
                        }
                    }
                }
            }
        }

        output.push('<');
        rest = &candidate[1..];
    }

    output.push_str(rest);
    output
}

fn find_matching_close(html: &str, tag: &str, search_start: usize) -> Option<usize> {
    let mut depth = 1;
    let mut offset = search_start;

    while let Some(index) = html[offset..].find('<') {
        let start = offset + index;
        let candidate = &html[start..];

        if closing_tag_name_matches(&candidate[1..], tag) {
            if let Some(end) = candidate.find('>') {
                depth -= 1;
                if depth == 0 {
                    return Some(start + end + 1);
                }
                offset = start + end + 1;
                continue;
            }
        } else if tag_name_matches(&candidate[1..], tag) {
            if let Some(end) = candidate.find('>') {
                if !candidate[..=end].ends_with("/>") {
                    depth += 1;
                }
                offset = start + end + 1;
                continue;
            }
        }

        offset = start + 1;
    }

    None
}

fn tag_name_matches(input: &str, tag: &str) -> bool {
    let mut chars = input.chars();
    for expected in tag.chars() {
        match chars.next() {
            Some(actual) if actual.eq_ignore_ascii_case(&expected) => {}
            _ => return false,
        }
    }

    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_whitespace() || ch == '>' || ch == '/')
}

fn closing_tag_name_matches(input: &str, tag: &str) -> bool {
    input
        .strip_prefix('/')
        .is_some_and(|rest| tag_name_matches(rest, tag))
}
