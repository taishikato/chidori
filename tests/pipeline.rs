use chidori::{
    cleaner::{clean_html, CleanOptions},
    document::ParsedDocument,
    extractor::extract_main_html,
    markdown::{html_to_markdown, MarkdownOptions},
    metadata::extract_metadata,
};
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

#[test]
fn removes_noise_and_optionally_images() {
    let html = r#"
    <article>
      <script>alert(1)</script>
      <style>.x{}</style>
      <p>Keep this paragraph.</p>
      <aside>Related links</aside>
      <img src="/hero.png" alt="Hero">
      <button>Share</button>
    </article>"#;

    let cleaned = clean_html(html, &CleanOptions { no_images: true });
    assert!(cleaned.contains("Keep this paragraph."));
    assert!(!cleaned.contains("script"));
    assert!(!cleaned.contains("Related links"));
    assert!(!cleaned.contains("<img"));
    assert!(!cleaned.contains("Share"));
}

#[test]
fn removes_nested_noise_tags() {
    let html = r#"
    <article>
      <p>Keep this paragraph.</p>
      <ASIDE><ASIDE>inner</ASIDE>outer</ASIDE>
    </article>"#;

    let cleaned = clean_html(html, &CleanOptions { no_images: false });
    assert!(cleaned.contains("Keep this paragraph."));
    assert!(!cleaned.contains("inner"));
    assert!(!cleaned.contains("outer"));
    assert!(!cleaned.contains("</aside>"));
    assert!(!cleaned.contains("</ASIDE>"));
}

#[test]
fn keeps_images_when_allowed() {
    let html = r#"
    <article>
      <p>Keep this paragraph.</p>
      <img src="/hero.png" alt="Hero">
    </article>"#;

    let cleaned = clean_html(html, &CleanOptions { no_images: false });
    assert!(cleaned.contains("Keep this paragraph."));
    assert!(cleaned.contains("<img"));
}

#[test]
fn removes_picture_when_images_disabled() {
    let html = r#"
    <article>
      <p>Keep this paragraph.</p>
      <picture>
        <source srcset="/hero.webp" type="image/webp">
        <img src="/hero.png" alt="Hero">
      </picture>
    </article>"#;

    let cleaned = clean_html(html, &CleanOptions { no_images: true });
    assert!(cleaned.contains("Keep this paragraph."));
    assert!(!cleaned.contains("<picture"));
    assert!(!cleaned.contains("<source"));
    assert!(!cleaned.contains("<img"));
    assert!(!cleaned.contains("</picture>"));
}

#[test]
fn converts_html_to_agent_friendly_markdown() {
    let html = r#"
    <article>
      <h1>Title</h1>
      <p>Hello <strong>world</strong>.</p>
      <pre><code class="language-rust">fn main() {}</code></pre>
    </article>"#;

    let markdown = html_to_markdown(html, &MarkdownOptions { max_chars: None });
    assert!(markdown.contains("# Title"));
    assert!(markdown.contains("**world**"));
    assert!(markdown.contains("```"));
    assert!(markdown.contains("fn main() {}"));
}

#[test]
fn truncates_markdown_when_max_chars_is_set() {
    let html = "<article><p>abcdefghijklmnopqrstuvwxyz</p></article>";
    let markdown = html_to_markdown(
        html,
        &MarkdownOptions {
            max_chars: Some(10),
        },
    );
    assert_eq!(markdown.chars().count(), 10);
}
