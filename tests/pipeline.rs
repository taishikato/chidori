use chidori::{
    cleaner::{clean_html, CleanOptions},
    document::ParsedDocument,
    extractor::extract_main_html,
    markdown::{html_to_markdown, MarkdownOptions},
    metadata::{extract_metadata, Metadata},
    output::{render_output, RenderMode},
};
use url::Url;

fn fixture_to_markdown(name: &str) -> String {
    let html = std::fs::read_to_string(format!("tests/fixtures/{name}")).unwrap();
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com").unwrap());
    let main = extract_main_html(&doc).unwrap();
    let cleaned = clean_html(&main, &CleanOptions { no_images: false });
    html_to_markdown(&cleaned, &MarkdownOptions { max_chars: None })
}

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
fn defuddle_priority_selectors_can_beat_larger_generic_main() {
    let focused_body = "Focused content wins through selector priority. ".repeat(10);
    let html = format!(
        r#"
    <html><body>
      <main>
        <p>This generic main wrapper has extra words that should not win just because it is larger.</p>
        <p>It also includes unrelated wrapper text around the actual post area.</p>
      </main>
      <div class="post-content">
        <h1>Focused Post</h1>
        <p>{focused_body}</p>
      </div>
    </body></html>"#
    );
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());

    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("Focused Post"));
    assert!(main.contains("Focused content wins"));
    assert!(!main.contains("generic main wrapper"));
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
      <main><a href="/y">BetterLink</a> extra</main>
      <main><a href="/x">OnlyLink</a></main>
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
      <article><p>Focused article content stays selected even when surrounding layout contains noisy sidebar text for readers.</p></article>
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
fn body_fallback_does_not_compete_with_primary_candidates() {
    let focused_body = "Focused article content. ".repeat(18);
    let filler = "sidebar noise ".repeat(260);
    let html = format!(
        r#"
    <html><body>
      <article><p>{focused_body}</p></article>
      <aside>{filler}</aside>
      <footer>Footer text that should not be included.</footer>
    </body></html>"#
    );
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("Focused article content"));
    assert!(!main.contains("sidebar noise"));
    assert!(!main.contains("Footer text"));
}

#[test]
fn short_article_candidate_is_not_replaced_by_noisy_body_retry() {
    let noise = "sidebar noise ".repeat(200);
    let html = format!(
        r#"
    <html><body>
      <article><p>Focused article content stays selected even when surrounding layout contains noisy sidebar text for readers.</p></article>
      <aside>{noise}</aside>
      <footer>Footer text that should not be included.</footer>
    </body></html>"#
    );
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("Focused article content stays selected"));
    assert!(!main.contains("sidebar noise"));
    assert!(!main.contains("Footer text"));
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
fn retries_with_body_when_entry_candidate_is_too_short() {
    let html = r#"
    <html><body>
      <article><p>Stub.</p></article>
      <div class="docs-page">
        <h1>Runtime Rendered Documentation</h1>
        <p>This useful documentation section lives outside common article selectors.</p>
        <p>The retry path should recover it when the first entry candidate is tiny.</p>
        <p>Agents need this text because otherwise the fetched page is almost empty.</p>
      </div>
    </body></html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/docs").unwrap());

    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("Runtime Rendered Documentation"));
    assert!(main.contains("useful documentation section"));
    assert!(main.contains("Agents need this text"));
}

#[test]
fn retries_short_article_placeholder_when_body_has_structured_content() {
    let html = r#"
    <html><body>
      <article><p>Article shell has teaser words but not the loaded documentation content.</p></article>
      <div class="docs-page">
        <h1>Runtime Rendered Documentation</h1>
        <p>This useful documentation section lives outside common article selectors.</p>
        <p>The retry path should recover it when the first entry candidate is tiny.</p>
        <p>Agents need this text because otherwise the fetched page is almost empty.</p>
      </div>
    </body></html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/docs").unwrap());

    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("Runtime Rendered Documentation"));
    assert!(main.contains("Article shell has teaser words"));
    assert!(main.contains("Agents need this text"));
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
fn cleaner_treats_non_ascii_less_than_text_as_text() {
    let html =
        "<article><p>Keep this.</p><p>It<’s text, not a tag.</p><p>Use <— as text.</p></article>";

    let cleaned = clean_html(html, &CleanOptions { no_images: false });

    assert!(cleaned.contains("It<’s text"));
    assert!(cleaned.contains("Use <— as text"));
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

#[test]
fn does_not_normalize_setext_inside_code_block() {
    let html = "<article><pre><code>title\n---</code></pre></article>";

    let markdown = html_to_markdown(html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("```\ntitle\n---\n```"));
    assert!(!markdown.contains("## title"));
    assert!(!markdown.contains("# title"));
}

#[test]
fn fixture_basic_article_preserves_content() {
    let markdown = fixture_to_markdown("basic_article.html");

    assert!(markdown.contains("# Basic Article"));
    assert!(markdown.contains("useful content for a coding agent"));
}

#[test]
fn fixture_noisy_article_removes_boilerplate() {
    let markdown = fixture_to_markdown("noisy_article.html");

    assert!(markdown.contains("# Noisy Article"));
    assert!(markdown.contains("survive cleanup"));
    assert!(!markdown.contains("Navigation"));
    assert!(!markdown.contains("Related posts"));
    assert!(!markdown.contains("Share"));
}

#[test]
fn fixture_code_article_preserves_code() {
    let markdown = fixture_to_markdown("code_article.html");

    assert!(markdown.contains("# Code Article"));
    assert!(markdown.contains("const value"));
}

#[test]
fn renders_plain_markdown_by_default() {
    let metadata = Metadata {
        title: "Title".to_string(),
        word_count: 2,
        ..Metadata::default()
    };
    let output = render_output(&metadata, "Hello world", RenderMode::Markdown).unwrap();
    assert_eq!(output, "Hello world");
}

#[test]
fn renders_json_with_markdown() {
    let metadata = Metadata {
        url: "https://example.com".to_string(),
        final_url: "https://example.com".to_string(),
        title: "Title".to_string(),
        word_count: 2,
        ..Metadata::default()
    };
    let output = render_output(&metadata, "Hello world", RenderMode::Json).unwrap();
    let json = serde_json::from_str::<serde_json::Value>(&output).unwrap();

    assert_eq!(json["title"], "Title");
    assert_eq!(json["url"], "https://example.com");
    assert_eq!(json["finalUrl"], "https://example.com");
    assert_eq!(json["wordCount"], 2);
    assert_eq!(json["markdown"], "Hello world");
}
