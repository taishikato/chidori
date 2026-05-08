use crate::document::ParsedDocument;
use scraper::Selector;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub url: String,
    pub final_url: String,
    pub domain: String,
    pub title: String,
    pub description: String,
    pub favicon: String,
    pub image: String,
    pub site: String,
    pub author: String,
    pub published: String,
    pub language: String,
    pub schema_org_data: Option<Value>,
    pub word_count: usize,
}

pub fn extract_metadata(doc: &ParsedDocument) -> Metadata {
    let schema_org_data = extract_schema_org_data(doc);
    Metadata {
        url: doc.url.to_string(),
        final_url: doc.url.to_string(),
        domain: doc.url.host_str().unwrap_or_default().to_string(),
        title: meta(doc, "property", "og:title")
            .or_else(|| meta(doc, "name", "twitter:title"))
            .or_else(|| schema_string(&schema_org_data, &["headline"]))
            .or_else(|| schema_article_string(&schema_org_data, "name"))
            .or_else(|| title(doc))
            .unwrap_or_default(),
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
        site: meta(doc, "property", "og:site_name")
            .or_else(|| schema_string(&schema_org_data, &["publisher.name"]))
            .or_else(|| schema_type_string(&schema_org_data, "WebSite", "name"))
            .unwrap_or_default(),
        author: meta(doc, "name", "author")
            .or_else(|| meta(doc, "property", "article:author"))
            .or_else(|| schema_string(&schema_org_data, &["author.name", "creator.name"]))
            .unwrap_or_default(),
        published: meta(doc, "property", "article:published_time")
            .or_else(|| meta(doc, "name", "date"))
            .or_else(|| schema_string(&schema_org_data, &["datePublished", "dateCreated"]))
            .unwrap_or_default(),
        language: schema_string(&schema_org_data, &["inLanguage"])
            .unwrap_or_else(|| html_lang(doc)),
        schema_org_data,
        word_count: 0,
    }
}

pub fn structured_content_text(doc: &ParsedDocument) -> Option<String> {
    let schema = extract_schema_org_data(doc);
    schema_string(&schema, &["articleBody", "text"]).filter(|text| !text.trim().is_empty())
}

pub fn extract_schema_org_data(doc: &ParsedDocument) -> Option<Value> {
    let selector = Selector::parse(r#"script[type="application/ld+json"]"#).ok()?;
    let values = doc
        .dom
        .select(&selector)
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

fn title(doc: &ParsedDocument) -> Option<String> {
    let selector = Selector::parse("title").unwrap();
    doc.dom
        .select(&selector)
        .next()
        .map(|node| node.text().collect::<Vec<_>>().join("").trim().to_string())
        .filter(|value| !value.is_empty())
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
