use chidori::{document::ParsedDocument, metadata::extract_metadata};
use url::Url;

#[test]
fn extracts_basic_metadata() {
    let html = r#"<!doctype html>
    <html lang="en">
      <head>
        <title>Article Title</title>
        <meta name="description" content="A useful article">
        <meta property="og:site_name" content="Example Site">
        <meta name="author" content="Ada Lovelace">
        <meta property="article:published_time" content="2026-05-06">
      </head>
      <body><article><p>Hello world.</p></article></body>
    </html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let metadata = extract_metadata(&doc);

    assert_eq!(metadata.title, "Article Title");
    assert_eq!(metadata.description, "A useful article");
    assert_eq!(metadata.site, "Example Site");
    assert_eq!(metadata.author, "Ada Lovelace");
    assert_eq!(metadata.published, "2026-05-06");
    assert_eq!(metadata.language, "en");
}
