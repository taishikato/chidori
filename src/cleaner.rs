use scraper::{Html, Selector};

#[derive(Debug, Clone, Copy)]
pub struct CleanOptions {
    pub no_images: bool,
}

pub fn clean_html(html: &str, options: &CleanOptions) -> String {
    let mut cleaned = html.to_string();
    for tag in [
        "script", "style", "noscript", "nav", "footer", "aside", "button", "form",
    ] {
        cleaned = remove_tag(&cleaned, tag);
    }
    if options.no_images {
        cleaned = remove_tag(&cleaned, "img");
        cleaned = remove_tag(&cleaned, "picture");
    }
    cleaned
}

fn remove_tag(html: &str, tag: &str) -> String {
    let fragment = Html::parse_fragment(html);
    let selector = Selector::parse(tag).unwrap();
    let mut output = html.to_string();
    for node in fragment.select(&selector) {
        let node_html = node.html();
        output = output.replace(&node_html, "");
    }
    remove_matching_tags(&output, tag)
}

fn remove_matching_tags(html: &str, tag: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut rest = html;
    let closing_tag = format!("</{tag}>");

    while let Some(index) = rest.find('<') {
        output.push_str(&rest[..index]);
        let candidate = &rest[index..];
        let after_name = &candidate[1..];
        if tag_name_matches(after_name, tag) {
            if let Some(end) = candidate.find('>') {
                if !candidate[..=end].ends_with("/>") {
                    if let Some(close_index) =
                        candidate[end + 1..].to_ascii_lowercase().find(&closing_tag)
                    {
                        let closing_end = end + 1 + close_index + closing_tag.len();
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

fn tag_name_matches(input: &str, tag: &str) -> bool {
    input.len() >= tag.len()
        && input[..tag.len()].eq_ignore_ascii_case(tag)
        && input[tag.len()..]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_whitespace() || ch == '>' || ch == '/')
}
