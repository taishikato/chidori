use chidori::{document::ParsedDocument, extractor::extract_main_html, metadata::extract_metadata};
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

    assert_eq!(metadata.url, "https://example.com/post");
    assert_eq!(metadata.final_url, "https://example.com/post");
    assert_eq!(metadata.title, "Article Title");
    assert_eq!(metadata.description, "A useful article");
    assert_eq!(metadata.site, "Example Site");
    assert_eq!(metadata.author, "Ada Lovelace");
    assert_eq!(metadata.published, "2026-05-06");
    assert_eq!(metadata.language, "en");
    assert_eq!(metadata.word_count, 0);
}

#[test]
fn extracts_article_over_navigation() {
    let html = r#"
    <html><body>
      <nav><a href="/a">Home</a><a href="/b">Docs</a></nav>
      <article><h1>Real Title</h1><p>This is the useful article body with enough words to win scoring.</p></article>
      <footer>Copyright</footer>
    </body></html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("Real Title"));
    assert!(main.contains("useful article body"));
    assert!(!main.contains("Copyright"));
}

#[test]
fn extracts_zero_score_candidate() {
    let html = r#"
    <html><body>
      <article><a href="/x">OnlyLink</a></article>
    </body></html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("OnlyLink"));
}

#[test]
fn prefers_less_negative_score_candidate() {
    let html = r#"
    <html><body>
      <article><a href="/x">OnlyLink</a></article>
      <main><a href="/y">BetterLink</a> extra</main>
    </body></html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("BetterLink"));
    assert!(!main.contains("OnlyLink"));
}

#[test]
fn body_does_not_beat_article() {
    let html = r#"
    <html><body>
      <article><p>Focused article content.</p></article>
      <aside>
        Sidebar filler text with many many many many many many many many many many extra words.
      </aside>
      <footer>Unrelated footer text that should not be included in extracted output.</footer>
    </body></html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("Focused article content"));
    assert!(!main.contains("Sidebar filler text"));
    assert!(!main.contains("Unrelated footer text"));
}

#[test]
fn skips_empty_candidates() {
    let html = r#"
    <html><body>
      <article><span></span></article>
      <main><a href="/fallback">FallbackLink</a></main>
    </body></html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("FallbackLink"));
    assert!(!main.contains("<article"));
}
