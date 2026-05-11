use crate::document::ParsedDocument;
use scraper::{ElementRef, Selector};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub url: String,
    pub final_url: String,
    pub canonical_url: String,
    pub domain: String,
    pub title: String,
    pub description: String,
    pub favicon: String,
    pub image: String,
    pub site: String,
    pub author: String,
    pub published: String,
    pub language: String,
    pub meta_tags: Vec<MetaTag>,
    pub schema_org_data: Option<Value>,
    pub word_count: usize,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MetaTag {
    pub name: Option<String>,
    pub property: Option<String>,
    pub content: Option<String>,
}

pub fn extract_metadata(doc: &ParsedDocument) -> Metadata {
    extract_metadata_with_content_title(doc, None)
}

pub fn extract_metadata_with_content_title(
    doc: &ParsedDocument,
    content_title: Option<&str>,
) -> Metadata {
    let schema_org_data = extract_schema_org_data(doc);
    let meta_tags = collect_meta_tags(doc);
    let site = meta(doc, "property", "og:site_name")
        .or_else(|| schema_string(&schema_org_data, &["publisher.name"]))
        .or_else(|| schema_type_string(&schema_org_data, "WebSite", "name"))
        .unwrap_or_default();
    let content_title = content_title
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty() && !is_placeholder_title(title));
    let semantic_title = valid_title_candidate(semantic_h1_title(doc));
    let html_title = valid_title_candidate(title(doc).map(|title| clean_title(&title, &site)));
    let html_title_is_site_only = html_title
        .as_deref()
        .is_some_and(|title| is_site_only_title(title, &site, doc.url.host_str()));
    let title = valid_title_candidate(
        meta(doc, "property", "og:title")
            .or_else(|| meta(doc, "name", "twitter:title"))
            .or_else(|| schema_string(&schema_org_data, &["headline"]))
            .or_else(|| schema_article_string(&schema_org_data, "name")),
    )
    .or_else(|| {
        preferred_extracted_title(&html_title, content_title.clone(), html_title_is_site_only)
    })
    .or_else(|| {
        preferred_extracted_title(&html_title, semantic_title.clone(), html_title_is_site_only)
    })
    .or(html_title)
    .or(content_title)
    .or(semantic_title)
    .or_else(|| h1_title(doc))
    .unwrap_or_default();

    Metadata {
        url: doc.url.to_string(),
        final_url: doc.url.to_string(),
        canonical_url: canonical_url(doc),
        domain: doc.url.host_str().unwrap_or_default().to_string(),
        title,
        description: meta(doc, "name", "description")
            .or_else(|| meta(doc, "property", "og:description"))
            .or_else(|| meta(doc, "name", "twitter:description"))
            .or_else(|| schema_string(&schema_org_data, &["description"]))
            .unwrap_or_default(),
        favicon: favicon(doc),
        image: meta(doc, "property", "og:image")
            .or_else(|| meta(doc, "name", "twitter:image"))
            .or_else(|| schema_string(&schema_org_data, &["image.url", "image"]))
            .unwrap_or_default(),
        site,
        author: meta(doc, "name", "author")
            .or_else(|| meta(doc, "name", "citation_author"))
            .or_else(|| meta(doc, "property", "article:author"))
            .or_else(|| schema_string(&schema_org_data, &["author.name", "creator.name"]))
            .or_else(|| schema_article_string(&schema_org_data, "author"))
            .or_else(|| schema_article_string(&schema_org_data, "creator"))
            .or_else(|| scoped_author(doc))
            .or_else(|| global_author(doc))
            .unwrap_or_default(),
        published: meta(doc, "property", "article:published_time")
            .or_else(|| meta(doc, "name", "date"))
            .or_else(|| meta(doc, "name", "datePublished"))
            .or_else(|| meta(doc, "name", "citation_publication_date"))
            .or_else(|| published_near_h1(doc))
            .or_else(|| time_datetime(doc))
            .or_else(|| schema_string(&schema_org_data, &["datePublished", "dateCreated"]))
            .unwrap_or_default(),
        language: schema_string(&schema_org_data, &["inLanguage"])
            .unwrap_or_else(|| html_lang(doc)),
        meta_tags,
        schema_org_data,
        word_count: 0,
    }
}

pub fn title_from_html_fragment(html: &str) -> Option<String> {
    let fragment = scraper::Html::parse_fragment(html);
    let selector = Selector::parse("h1").ok()?;
    fragment
        .select(&selector)
        .next()
        .map(|node| node.text().collect::<Vec<_>>().join("").trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn structured_content_text(doc: &ParsedDocument) -> Option<String> {
    let schema = extract_schema_org_data(doc);
    schema_string(&schema, &["articleBody"])
        .or_else(|| schema_article_string(&schema, "text"))
        .filter(|text| !text.trim().is_empty())
}

pub fn extract_schema_org_data(doc: &ParsedDocument) -> Option<Value> {
    let selector = Selector::parse("script").ok()?;
    let values = doc
        .dom
        .select(&selector)
        .filter(|node| {
            node.value()
                .attr("type")
                .is_some_and(is_json_ld_script_type)
        })
        .filter_map(|node| {
            let text = node.text().collect::<Vec<_>>().join("");
            let text = text.trim();
            if text.is_empty() {
                return None;
            }
            serde_json::from_str::<Value>(text).ok()
        })
        .collect::<Vec<_>>();

    match values.len() {
        0 => None,
        1 => values.into_iter().next(),
        _ => Some(Value::Array(values)),
    }
}

fn is_json_ld_script_type(value: &str) -> bool {
    value
        .split(';')
        .next()
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/ld+json"))
}

fn title(doc: &ParsedDocument) -> Option<String> {
    let selector = Selector::parse("title").unwrap();
    doc.dom
        .select(&selector)
        .next()
        .map(|node| node.text().collect::<Vec<_>>().join("").trim().to_string())
        .filter(|value| !value.is_empty())
}

fn h1_title(doc: &ParsedDocument) -> Option<String> {
    h1_title_for_selector(doc, "h1")
}

fn semantic_h1_title(doc: &ParsedDocument) -> Option<String> {
    h1_title_for_selector(
        doc,
        r#"article h1, [role="article"] h1, main h1, [role="main"] h1"#,
    )
}

fn h1_title_for_selector(doc: &ParsedDocument, raw_selector: &str) -> Option<String> {
    let selector = Selector::parse(raw_selector).unwrap();
    doc.dom
        .select(&selector)
        .next()
        .map(|node| node.text().collect::<Vec<_>>().join("").trim().to_string())
        .filter(|value| !value.is_empty())
}

fn is_placeholder_title(title: &str) -> bool {
    let normalized = title
        .trim()
        .trim_matches(|ch| matches!(ch, '.' | '!' | '…'))
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "" | "untitled"
            | "home"
            | "index"
            | "loading"
            | "just a moment"
            | "access denied"
            | "forbidden"
            | "please wait"
    )
}

fn valid_title_candidate(title: Option<String>) -> Option<String> {
    title.filter(|title| !is_placeholder_title(title))
}

fn preferred_extracted_title(
    html_title: &Option<String>,
    extracted_title: Option<String>,
    html_title_is_site_only: bool,
) -> Option<String> {
    let extracted_title = extracted_title?;
    match html_title {
        Some(_) if html_title_is_site_only && is_substantive_title(&extracted_title) => {
            Some(extracted_title)
        }
        Some(html_title) if !title_overlaps(html_title, &extracted_title) => None,
        _ => Some(extracted_title),
    }
}

fn is_substantive_title(title: &str) -> bool {
    title_word_count(title) >= 2 || normalized_title_for_overlap(title).len() >= 12
}

fn is_site_only_title(title: &str, site: &str, host: Option<&str>) -> bool {
    let title_normalized = normalized_title_for_overlap(title);
    if title_normalized.is_empty() {
        return false;
    }
    if !site.is_empty() && title_normalized == normalized_title_for_overlap(site) {
        return true;
    }

    let Some(domain_stem) = host.and_then(domain_stem) else {
        return false;
    };
    let domain_stem = domain_stem.to_ascii_lowercase();
    let words = normalized_title_words(title);
    !words.is_empty()
        && words
            .iter()
            .all(|word| word == &domain_stem || matches!(word.as_str(), "site" | "home"))
        && words.iter().any(|word| word == &domain_stem)
}

fn domain_stem(host: &str) -> Option<&str> {
    host.trim_start_matches("www.").split('.').next()
}

fn title_overlaps(left: &str, right: &str) -> bool {
    let left_normalized = normalized_title_for_overlap(left);
    let right_normalized = normalized_title_for_overlap(right);
    if left_normalized.is_empty() || right_normalized.is_empty() {
        return false;
    }
    if left_normalized == right_normalized {
        return true;
    }

    let shorter_len = left_normalized.len().min(right_normalized.len());
    let longer_len = left_normalized.len().max(right_normalized.len());
    let shorter_word_count = if left_normalized.len() <= right_normalized.len() {
        title_word_count(left)
    } else {
        title_word_count(right)
    };

    (left_normalized.contains(&right_normalized) || right_normalized.contains(&left_normalized))
        && shorter_len * 20 >= longer_len * 11
        && (shorter_word_count >= 2 || shorter_len >= 12)
}

fn normalized_title_for_overlap(title: &str) -> String {
    let mut normalized = String::new();
    for ch in title.chars().filter(|ch| ch.is_alphanumeric()) {
        normalized.extend(ch.to_lowercase());
    }
    normalized
}

fn title_word_count(title: &str) -> usize {
    title
        .split_whitespace()
        .filter(|word| !word.is_empty())
        .count()
}

fn normalized_title_words(title: &str) -> Vec<String> {
    title
        .split_whitespace()
        .filter_map(|word| {
            let normalized = normalized_title_for_overlap(word);
            (!normalized.is_empty()).then_some(normalized)
        })
        .collect()
}

fn clean_title(title: &str, site: &str) -> String {
    let title = title.trim();
    let site = site.trim();
    if site.is_empty() {
        return title.to_string();
    }

    [" | ", " - ", " – ", " — ", " :: "]
        .iter()
        .find_map(|separator| {
            title
                .strip_suffix(&format!("{separator}{site}"))
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| title.to_string())
}

fn canonical_url(doc: &ParsedDocument) -> String {
    let selector = Selector::parse(r#"link[rel~="canonical"]"#).unwrap();
    doc.dom
        .select(&selector)
        .next()
        .and_then(|node| node.value().attr("href"))
        .and_then(|href| doc.url.join(href).ok())
        .map(|url| url.to_string())
        .unwrap_or_default()
}

fn time_datetime(doc: &ParsedDocument) -> Option<String> {
    let selector = Selector::parse("time[datetime]").unwrap();
    doc.dom
        .select(&selector)
        .next()
        .and_then(|node| node.value().attr("datetime"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn published_near_h1(doc: &ParsedDocument) -> Option<String> {
    published_datetime_for_selector(
        doc,
        r#"article .date time[datetime], article .dateline time[datetime], article .published time[datetime], article [class*="date"] time[datetime]"#,
    )
    .or_else(|| {
        published_datetime_for_selector(
            doc,
            r#"main .date time[datetime], main .dateline time[datetime], main .published time[datetime], main [class*="date"] time[datetime]"#,
        )
    })
    .or_else(|| {
        published_datetime_for_selector(
            doc,
            r#".date time[datetime], .dateline time[datetime], .published time[datetime], [class*="date"] time[datetime]"#,
        )
    })
}

fn published_datetime_for_selector(doc: &ParsedDocument, raw_selector: &str) -> Option<String> {
    let selector = Selector::parse(raw_selector).unwrap();
    doc.dom
        .select(&selector)
        .next()
        .and_then(|node| node.value().attr("datetime"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn scoped_author(doc: &ParsedDocument) -> Option<String> {
    scoped_article_author(doc).or_else(|| scoped_main_author(doc))
}

fn scoped_article_author(doc: &ParsedDocument) -> Option<String> {
    rel_author_for_selector(
        doc,
        r#"article a[rel~="author"], article address[rel~="author"]"#,
    )
    .or_else(|| {
        byline_author_for_selector(
            doc,
            r#"article .byline, article [class*="byline"], article [itemprop="author"]"#,
        )
    })
}

fn scoped_main_author(doc: &ParsedDocument) -> Option<String> {
    rel_author_for_selector(doc, r#"main a[rel~="author"], main address[rel~="author"]"#).or_else(
        || {
            byline_author_for_selector(
                doc,
                r#"main .byline, main [class*="byline"], main [itemprop="author"]"#,
            )
        },
    )
}

fn global_author(doc: &ParsedDocument) -> Option<String> {
    rel_author_for_selector(doc, r#"a[rel~="author"], address[rel~="author"]"#).or_else(|| {
        byline_author_for_selector(doc, r#".byline, [class*="byline"], [itemprop="author"]"#)
    })
}

fn rel_author_for_selector(doc: &ParsedDocument, raw_selector: &str) -> Option<String> {
    let selector = Selector::parse(raw_selector).unwrap();
    unique_short_values(
        doc.dom
            .select(&selector)
            .filter_map(|node| {
                let text = node.text().collect::<Vec<_>>().join(" ");
                clean_author_candidate(&text)
            })
            .collect::<Vec<_>>(),
    )
    .first()
    .cloned()
}

fn byline_author_for_selector(doc: &ParsedDocument, raw_selector: &str) -> Option<String> {
    let selector = Selector::parse(raw_selector).unwrap();
    doc.dom.select(&selector).find_map(byline_author_from_node)
}

fn byline_author_from_node(node: ElementRef<'_>) -> Option<String> {
    let author_selector = Selector::parse(r#"a[rel~="author"], [itemprop="name"]"#).unwrap();
    node.select(&author_selector)
        .find_map(|author| {
            let text = author.text().collect::<Vec<_>>().join(" ");
            clean_author_candidate(&text)
        })
        .or_else(|| {
            let text = node.text().collect::<Vec<_>>().join(" ");
            let text = strip_byline_prefix(&text);
            clean_author_candidate(trim_trailing_byline_noise(text))
        })
}

fn strip_byline_prefix(value: &str) -> &str {
    let value = value.trim();
    value
        .strip_prefix("By ")
        .or_else(|| value.strip_prefix("by "))
        .or_else(|| value.strip_prefix("BY "))
        .or_else(|| value.strip_prefix("By:"))
        .map(str::trim)
        .unwrap_or(value)
}

fn trim_trailing_byline_noise(value: &str) -> &str {
    let lower = value.to_ascii_lowercase();
    let mut end = value.len();
    for marker in [" published ", " updated ", " posted "] {
        if let Some(index) = lower.find(marker) {
            end = end.min(index);
        }
    }
    for marker in [" follow", " subscribe"] {
        let mut offset = 0;
        while let Some(index) = lower[offset..].find(marker) {
            let start = offset + index;
            let after = start + marker.len();
            if lower[after..]
                .chars()
                .next()
                .is_none_or(|ch| !ch.is_alphabetic())
            {
                end = end.min(start);
                break;
            }
            offset = after;
        }
    }
    value[..end].trim()
}

fn clean_author_candidate(value: &str) -> Option<String> {
    let cleaned = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .trim_matches(|ch| matches!(ch, ',' | '|' | '-' | '–' | '—'))
        .to_string();
    let lower = cleaned.to_ascii_lowercase();
    if cleaned.is_empty()
        || cleaned.len() > 100
        || matches!(lower.as_str(), "author" | "authors" | "by")
        || is_placeholder_title(&cleaned)
    {
        None
    } else {
        Some(cleaned)
    }
}

fn unique_short_values(values: Vec<String>) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values {
        if !unique.iter().any(|existing| existing == &value) {
            unique.push(value);
        }
    }
    unique
}

fn collect_meta_tags(doc: &ParsedDocument) -> Vec<MetaTag> {
    let selector = Selector::parse("meta").unwrap();
    doc.dom
        .select(&selector)
        .filter_map(|node| {
            let value = node.value();
            let name = value.attr("name").map(ToString::to_string);
            let property = value.attr("property").map(ToString::to_string);
            let content = value.attr("content").map(ToString::to_string);
            (name.is_some() || property.is_some() || content.is_some()).then_some(MetaTag {
                name,
                property,
                content,
            })
        })
        .collect()
}

fn html_lang(doc: &ParsedDocument) -> String {
    let selector = Selector::parse("html").unwrap();
    doc.dom
        .select(&selector)
        .next()
        .and_then(|node| node.value().attr("lang"))
        .unwrap_or("")
        .to_string()
}

fn meta(doc: &ParsedDocument, attr: &str, value: &str) -> Option<String> {
    let selector = Selector::parse(&format!(r#"meta[{}="{}"]"#, attr, value)).ok()?;
    doc.dom
        .select(&selector)
        .next()
        .and_then(|node| node.value().attr("content"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn favicon(doc: &ParsedDocument) -> String {
    let selector = Selector::parse(r#"link[rel~="icon"], link[rel="shortcut icon"]"#).unwrap();
    doc.dom
        .select(&selector)
        .next()
        .and_then(|node| node.value().attr("href"))
        .and_then(|href| doc.url.join(href).ok())
        .map(|url| url.to_string())
        .unwrap_or_default()
}

fn schema_string(schema: &Option<Value>, paths: &[&str]) -> Option<String> {
    let schema = schema.as_ref()?;
    paths.iter().find_map(|path| {
        let parts = path.split('.').collect::<Vec<_>>();
        find_path_deep(schema, &parts)
            .into_iter()
            .filter_map(value_to_string)
            .find(|value| !value.trim().is_empty())
    })
}

fn schema_type_string(schema: &Option<Value>, schema_type: &str, property: &str) -> Option<String> {
    let schema = schema.as_ref()?;
    find_typed_nodes(schema, schema_type)
        .into_iter()
        .filter_map(|node| match node {
            Value::Object(map) => map.get(property).and_then(value_to_string),
            _ => None,
        })
        .find(|value| !value.trim().is_empty())
}

fn schema_article_string(schema: &Option<Value>, property: &str) -> Option<String> {
    ["Article", "NewsArticle", "BlogPosting"]
        .iter()
        .find_map(|schema_type| schema_type_string(schema, schema_type, property))
}

fn find_typed_nodes<'a>(value: &'a Value, schema_type: &str) -> Vec<&'a Value> {
    let mut matches = Vec::new();

    match value {
        Value::Object(map) => {
            if map
                .get("@type")
                .is_some_and(|value| value_matches_schema_type(value, schema_type))
            {
                matches.push(value);
            }

            for child in map.values() {
                matches.extend(find_typed_nodes(child, schema_type));
            }
        }
        Value::Array(items) => {
            for item in items {
                matches.extend(find_typed_nodes(item, schema_type));
            }
        }
        _ => {}
    }

    matches
}

fn value_matches_schema_type(value: &Value, schema_type: &str) -> bool {
    match value {
        Value::String(value) => value == schema_type,
        Value::Array(items) => items
            .iter()
            .any(|item| value_matches_schema_type(item, schema_type)),
        _ => false,
    }
}

fn find_path_deep<'a>(value: &'a Value, parts: &[&str]) -> Vec<&'a Value> {
    let mut matches = find_path_from(value, parts);

    match value {
        Value::Array(items) => {
            for item in items {
                matches.extend(find_path_deep(item, parts));
            }
        }
        Value::Object(map) => {
            for child in map.values() {
                matches.extend(find_path_deep(child, parts));
            }
        }
        _ => {}
    }

    matches
}

fn find_path_from<'a>(value: &'a Value, parts: &[&str]) -> Vec<&'a Value> {
    if parts.is_empty() {
        return vec![value];
    }

    match value {
        Value::Object(map) => map
            .get(parts[0])
            .map(|next| find_path_from(next, &parts[1..]))
            .unwrap_or_default(),
        Value::Array(items) => items
            .iter()
            .flat_map(|item| find_path_from(item, parts))
            .collect(),
        _ => Vec::new(),
    }
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(items) => items.iter().find_map(value_to_string),
        Value::Object(map) => map
            .get("url")
            .or_else(|| map.get("name"))
            .and_then(value_to_string),
        Value::Null => None,
    }
}
