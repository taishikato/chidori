use crate::error::ChidoriError;
use kuchiki::traits::TendrilSink;
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone)]
pub struct StandardizeOptions {
    pub base_url: Option<Url>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StandardizeRecord {
    pub reason: String,
    pub selector: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct StandardizeResult {
    pub html: String,
    pub records: Vec<StandardizeRecord>,
}

pub fn standardize_html(
    html: &str,
    options: &StandardizeOptions,
) -> Result<StandardizeResult, ChidoriError> {
    let document = parse_fragment_document(html);
    let mut records = Vec::new();

    push_record(
        &mut records,
        "noscript-image-fallback",
        "noscript img",
        recover_noscript_images(&document),
    );
    push_record(
        &mut records,
        "lazy-image-source",
        "img[data-src]",
        promote_lazy_image_sources(&document),
    );
    push_record(
        &mut records,
        "picture-source",
        "picture source[srcset]",
        promote_picture_sources(&document),
    );
    push_record(
        &mut records,
        "lazy-image-srcset",
        "source[data-srcset]",
        promote_lazy_image_srcsets(&document),
    );
    push_record(
        &mut records,
        "placeholder-image",
        "img",
        remove_placeholder_images(&document),
    );
    if let Some(base_url) = &options.base_url {
        push_record(
            &mut records,
            "relative-url",
            "[src], [poster], [srcset]",
            resolve_relative_urls(&document, base_url),
        );
    }
    push_record(
        &mut records,
        "preformatted-code",
        "code",
        wrap_preformatted_code(&document),
    );

    Ok(StandardizeResult {
        html: serialize_body_inner(&document),
        records,
    })
}

fn parse_fragment_document(html: &str) -> kuchiki::NodeRef {
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

fn push_record(records: &mut Vec<StandardizeRecord>, reason: &str, selector: &str, count: usize) {
    if count == 0 {
        return;
    }

    records.push(StandardizeRecord {
        reason: reason.to_string(),
        selector: selector.to_string(),
        count,
    });
}

fn recover_noscript_images(document: &kuchiki::NodeRef) -> usize {
    let Ok(noscripts) = document.select("noscript") else {
        return 0;
    };
    let mut count = 0;

    for noscript in noscripts.collect::<Vec<_>>() {
        for image in noscript_fallback_images(noscript.as_node()) {
            image.detach();
            noscript.as_node().insert_before(image);
            count += 1;
        }
    }

    count
}

fn noscript_fallback_images(noscript: &kuchiki::NodeRef) -> Vec<kuchiki::NodeRef> {
    let serialized = noscript
        .children()
        .map(|child| child.to_string())
        .collect::<String>();
    let text = noscript.text_contents();
    let mut images = images_from_fragment(&serialized);
    if images.is_empty() && text != serialized {
        images = images_from_fragment(&text);
    }
    if images.is_empty() {
        let decoded = html_escape::decode_html_entities(&text);
        images = images_from_fragment(decoded.as_ref());
    }
    images
}

fn images_from_fragment(html: &str) -> Vec<kuchiki::NodeRef> {
    let fragment = parse_fragment_document(html);
    let Ok(images) = fragment.select("img") else {
        return Vec::new();
    };
    images
        .map(|image| image.as_node().clone())
        .collect::<Vec<_>>()
}

fn promote_lazy_image_sources(document: &kuchiki::NodeRef) -> usize {
    let Ok(images) = document.select("img") else {
        return 0;
    };
    let mut count = 0;

    for image in images {
        let mut attributes = image.attributes.borrow_mut();
        let Some(lazy_src) = first_attribute(
            &attributes,
            &[
                "data-src",
                "data-original",
                "data-lazy-src",
                "data-actualsrc",
                "data-url",
            ],
        ) else {
            continue;
        };
        if !should_promote_image_src(attributes.get("src")) {
            continue;
        }

        attributes.insert("src", lazy_src);
        count += 1;
    }

    count
}

fn promote_lazy_image_srcsets(document: &kuchiki::NodeRef) -> usize {
    let Ok(nodes) = document.select("img, source") else {
        return 0;
    };
    let mut count = 0;

    for node in nodes {
        let mut attributes = node.attributes.borrow_mut();
        if attributes
            .get("srcset")
            .is_some_and(|value| !value.trim().is_empty())
        {
            continue;
        }
        let Some(lazy_srcset) = first_attribute(&attributes, &["data-srcset", "data-lazy-srcset"])
        else {
            continue;
        };

        attributes.insert("srcset", lazy_srcset);
        count += 1;
    }

    count
}

fn promote_picture_sources(document: &kuchiki::NodeRef) -> usize {
    let Ok(pictures) = document.select("picture") else {
        return 0;
    };
    let mut count = 0;

    for picture in pictures {
        let Some(src) = preferred_picture_source(picture.as_node()) else {
            continue;
        };
        let Ok(image) = picture.as_node().select_first("img") else {
            continue;
        };
        let mut attributes = image.attributes.borrow_mut();
        if !should_promote_image_src(attributes.get("src")) {
            continue;
        }

        attributes.insert("src", src);
        count += 1;
    }

    count
}

fn preferred_picture_source(picture: &kuchiki::NodeRef) -> Option<String> {
    let sources = picture.select("source").ok()?;
    let mut fallback = None;

    for source in sources {
        let attributes = source.attributes.borrow();
        let srcset = attributes
            .get("srcset")
            .or_else(|| attributes.get("data-srcset"))?;
        if contains_unsafe_srcset_candidate(srcset) {
            continue;
        }
        let candidate = largest_srcset_candidate(srcset).unwrap_or_else(|| srcset.trim());
        if candidate.is_empty() {
            continue;
        }
        let candidate = candidate.to_string();
        if attributes
            .get("type")
            .is_some_and(|value| value.eq_ignore_ascii_case("image/webp"))
        {
            return Some(candidate);
        }
        fallback.get_or_insert(candidate);
    }

    fallback
}

fn first_attribute(attributes: &kuchiki::Attributes, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        attributes
            .get(*name)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

fn should_promote_image_src(current_src: Option<&str>) -> bool {
    let Some(current_src) = current_src.map(str::trim).filter(|src| !src.is_empty()) else {
        return true;
    };
    is_placeholder_src(current_src)
}

fn is_placeholder_src(src: &str) -> bool {
    let lower = src.trim().to_ascii_lowercase();
    lower.starts_with("data:image")
        || lower.contains("/max/60/")
        || lower.contains("?q=20")
        || lower.contains("&q=20")
        || lower.contains("placeholder")
        || lower.contains("spacer")
        || lower.contains("transparent")
        || lower.ends_with("/blank.gif")
        || lower.ends_with("/blank.png")
}

fn remove_placeholder_images(document: &kuchiki::NodeRef) -> usize {
    let Ok(images) = document.select("img") else {
        return 0;
    };
    let mut count = 0;

    for image in images.collect::<Vec<_>>() {
        let attributes = image.attributes.borrow();
        let placeholder = attributes
            .get("src")
            .map(is_placeholder_src)
            .unwrap_or(true)
            && attributes
                .get("srcset")
                .is_none_or(|value| value.trim().is_empty());
        drop(attributes);

        if !placeholder || !nearby_useful_image_or_source(image.as_node()) {
            continue;
        }

        image.as_node().detach();
        count += 1;
    }

    count
}

fn nearby_useful_image_or_source(image: &kuchiki::NodeRef) -> bool {
    let Some(container) = nearest_ancestor(image, &["figure", "picture"]) else {
        return false;
    };

    if let Ok(images) = container.select("img") {
        for candidate in images {
            if candidate.as_node() == image {
                continue;
            }
            let attributes = candidate.attributes.borrow();
            if attributes
                .get("src")
                .is_some_and(|src| !is_placeholder_src(src))
                || attributes
                    .get("srcset")
                    .is_some_and(|srcset| !srcset.trim().is_empty())
            {
                return true;
            }
        }
    }

    if let Ok(sources) = container.select("source") {
        for source in sources {
            let attributes = source.attributes.borrow();
            if attributes
                .get("srcset")
                .is_some_and(|srcset| !srcset.trim().is_empty())
            {
                return true;
            }
        }
    }

    false
}

fn nearest_ancestor(node: &kuchiki::NodeRef, tag_names: &[&str]) -> Option<kuchiki::NodeRef> {
    node.ancestors().skip(1).find(|ancestor| {
        ancestor.as_element().is_some_and(|element| {
            tag_names
                .iter()
                .any(|tag_name| element.name.local.as_ref() == *tag_name)
        })
    })
}

fn resolve_relative_urls(document: &kuchiki::NodeRef, base_url: &Url) -> usize {
    let mut count = 0;
    count += resolve_attribute_urls(
        document,
        "img, source, video, audio, track, iframe",
        "src",
        base_url,
    );
    count += resolve_attribute_urls(document, "video", "poster", base_url);
    count += resolve_srcset_urls(document, base_url);
    count
}

fn resolve_attribute_urls(
    document: &kuchiki::NodeRef,
    selector: &str,
    attribute_name: &str,
    base_url: &Url,
) -> usize {
    let Ok(nodes) = document.select(selector) else {
        return 0;
    };
    let mut count = 0;

    for node in nodes {
        let mut attributes = node.attributes.borrow_mut();
        let Some(value) = attributes.get(attribute_name).map(str::trim) else {
            continue;
        };
        if !is_resolvable_url(value) {
            continue;
        }
        let Ok(resolved) = base_url.join(value) else {
            continue;
        };
        let resolved = resolved.to_string();
        if resolved == value {
            continue;
        }

        attributes.insert(attribute_name, resolved);
        count += 1;
    }

    count
}

fn resolve_srcset_urls(document: &kuchiki::NodeRef, base_url: &Url) -> usize {
    let Ok(nodes) = document.select("img, source") else {
        return 0;
    };
    let mut count = 0;

    for node in nodes {
        let mut attributes = node.attributes.borrow_mut();
        let Some(srcset) = attributes.get("srcset") else {
            continue;
        };
        let resolved = resolve_srcset(srcset, base_url);
        if resolved == srcset {
            continue;
        }

        attributes.insert("srcset", resolved);
        count += 1;
    }

    count
}

fn resolve_srcset(srcset: &str, base_url: &Url) -> String {
    if contains_unsafe_srcset_candidate(srcset) {
        return srcset.to_string();
    }

    srcset
        .split(',')
        .map(|candidate| resolve_srcset_candidate(candidate, base_url))
        .collect::<Vec<_>>()
        .join(", ")
}

fn resolve_srcset_candidate(candidate: &str, base_url: &Url) -> String {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let url = parts.next().unwrap_or_default();
    if !is_resolvable_url(url) {
        return trimmed.to_string();
    }
    let Ok(resolved) = base_url.join(url) else {
        return trimmed.to_string();
    };
    if let Some(descriptor) = parts.next().map(str::trim).filter(|part| !part.is_empty()) {
        format!("{resolved} {descriptor}")
    } else {
        resolved.to_string()
    }
}

fn largest_srcset_candidate(srcset: &str) -> Option<&str> {
    srcset
        .split(',')
        .filter_map(|candidate| {
            let trimmed = candidate.trim();
            if trimmed.is_empty() {
                return None;
            }
            let mut parts = trimmed.split_whitespace();
            let url = parts.next()?;
            let score = parts
                .next()
                .and_then(srcset_descriptor_score)
                .unwrap_or(1.0);
            Some((url, score))
        })
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(url, _)| url)
}

fn srcset_descriptor_score(descriptor: &str) -> Option<f64> {
    descriptor
        .strip_suffix('w')
        .or_else(|| descriptor.strip_suffix('x'))
        .and_then(|value| value.parse().ok())
}

fn is_resolvable_url(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.starts_with('#') {
        return false;
    }

    match url_scheme(value) {
        Some(scheme) => matches!(scheme.as_str(), "http" | "https"),
        None => true,
    }
}

fn contains_unsafe_srcset_candidate(srcset: &str) -> bool {
    let trimmed = srcset.trim();
    starts_with_unsafe_scheme(trimmed)
        || trimmed
            .split(',')
            .map(str::trim)
            .any(starts_with_unsafe_scheme)
}

fn starts_with_unsafe_scheme(value: &str) -> bool {
    matches!(
        url_scheme(value).as_deref(),
        Some("mailto" | "tel" | "javascript" | "data" | "blob" | "cid" | "file")
    )
}

fn url_scheme(value: &str) -> Option<String> {
    let value = value.trim_start();
    let colon = value.find(':')?;
    let first_separator = value.find(['/', '?', '#']).unwrap_or(value.len());
    if colon > first_separator {
        return None;
    }

    let scheme = &value[..colon];
    if scheme.is_empty()
        || !scheme
            .chars()
            .enumerate()
            .all(|(index, ch)| is_url_scheme_char(ch, index == 0))
    {
        return None;
    }

    Some(scheme.to_ascii_lowercase())
}

fn is_url_scheme_char(ch: char, first: bool) -> bool {
    if first {
        return ch.is_ascii_alphabetic();
    }

    ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.')
}

fn wrap_preformatted_code(document: &kuchiki::NodeRef) -> usize {
    let Ok(codes) = document.select("code") else {
        return 0;
    };
    let mut count = 0;

    for code in codes.collect::<Vec<_>>() {
        if has_ancestor(code.as_node(), "pre") || !looks_preformatted(code.as_node()) {
            continue;
        }

        let wrapper = parse_fragment_document("<pre></pre>");
        let Ok(pre) = wrapper.select_first("pre") else {
            continue;
        };
        let pre = pre.as_node().clone();
        pre.detach();
        code.as_node().insert_before(pre.clone());
        code.as_node().detach();
        pre.append(code.as_node().clone());
        count += 1;
    }

    count
}

fn has_ancestor(node: &kuchiki::NodeRef, tag_name: &str) -> bool {
    node.ancestors().skip(1).any(|ancestor| {
        ancestor
            .as_element()
            .is_some_and(|element| element.name.local.as_ref() == tag_name)
    })
}

fn looks_preformatted(node: &kuchiki::NodeRef) -> bool {
    let text = node.text_contents();
    text.contains('\n') || text.contains('\t') || text.starts_with("    ")
}
