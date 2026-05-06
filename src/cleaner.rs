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
    output
}
