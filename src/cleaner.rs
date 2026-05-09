use scraper::{Html, Selector};

#[derive(Debug, Clone, Copy)]
pub struct CleanOptions {
    pub no_images: bool,
}

pub fn clean_html(html: &str, options: &CleanOptions) -> String {
    let mut cleaned = html.to_string();
    for tag in [
        "script", "style", "noscript", "nav", "footer", "aside", "button", "form", "iframe",
        "object", "embed",
    ] {
        cleaned = remove_tag(&cleaned, tag);
    }
    cleaned = remove_hidden_elements(&cleaned);
    cleaned = remove_navigation_like_blocks(&cleaned);
    cleaned = remove_fragment_only_link_lists(&cleaned);
    cleaned = remove_link_dense_related_sections(&cleaned);
    cleaned = unwrap_javascript_links(&cleaned);
    if options.no_images {
        cleaned = remove_tag(&cleaned, "img");
        cleaned = remove_tag(&cleaned, "picture");
    }
    cleaned
}

fn remove_hidden_elements(html: &str) -> String {
    let mut cleaned = html.to_string();
    for tag in [
        "div", "section", "article", "header", "span", "p", "ul", "ol", "li",
    ] {
        cleaned = remove_matching_tags_where(&cleaned, tag, is_hidden_opening_tag);
    }
    cleaned
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

fn is_hidden_opening_tag(opening_tag: &str) -> bool {
    has_attribute(opening_tag, "hidden")
        || attribute_value_eq(opening_tag, "aria-hidden", "true")
        || attribute_value_contains_normalized(opening_tag, "style", "display:none")
        || attribute_value_contains_normalized(opening_tag, "style", "visibility:hidden")
        || has_class_token(opening_tag, "hidden")
        || has_class_token(opening_tag, "sr-only")
}

fn unwrap_javascript_links(html: &str) -> String {
    unwrap_matching_tags_where(html, "a", |opening_tag| {
        attribute_values(opening_tag, "href").any(|href| {
            href.trim_start()
                .to_ascii_lowercase()
                .starts_with("javascript:")
        })
    })
}

fn has_class_token(opening_tag: &str, expected: &str) -> bool {
    attribute_values(opening_tag, "class").any(|value| {
        value
            .split_ascii_whitespace()
            .any(|token| token.eq_ignore_ascii_case(expected))
    })
}

fn has_attribute(opening_tag: &str, expected: &str) -> bool {
    opening_attributes(opening_tag).any(|(name, _)| name.eq_ignore_ascii_case(expected))
}

fn attribute_value_eq(opening_tag: &str, expected: &str, expected_value: &str) -> bool {
    attribute_values(opening_tag, expected)
        .any(|value| value.trim().eq_ignore_ascii_case(expected_value))
}

fn attribute_value_contains_normalized(
    opening_tag: &str,
    expected: &str,
    expected_value: &str,
) -> bool {
    attribute_values(opening_tag, expected).any(|value| {
        value
            .chars()
            .filter(|ch| !ch.is_ascii_whitespace())
            .collect::<String>()
            .to_ascii_lowercase()
            .contains(expected_value)
    })
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
            .find(|ch: char| ch.is_ascii_whitespace() || ch == '/')
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

fn unwrap_matching_tags_where(
    html: &str,
    tag: &str,
    should_unwrap: impl Fn(&str) -> bool,
) -> String {
    let mut output = String::with_capacity(html.len());
    let mut rest = html;
    let mut suppressed_closing_tags = 0usize;

    while let Some(index) = rest.find('<') {
        output.push_str(&rest[..index]);
        let candidate = &rest[index..];
        let after_name = &candidate[1..];

        if closing_tag_name_matches(after_name, tag) {
            if let Some(end) = candidate.find('>') {
                if suppressed_closing_tags > 0 {
                    suppressed_closing_tags -= 1;
                    rest = &candidate[end + 1..];
                    continue;
                }
            }
        } else if tag_name_matches(after_name, tag) {
            if let Some(end) = candidate.find('>') {
                let opening_tag = &candidate[..=end];
                if should_unwrap(opening_tag) {
                    if !opening_tag.ends_with("/>") {
                        suppressed_closing_tags += 1;
                    }
                    rest = &candidate[end + 1..];
                    continue;
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
