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
fn extracts_extended_metadata_from_social_and_structured_sources() {
    let html = r#"<!doctype html>
    <html lang="en">
      <head>
        <title>Fallback Title</title>
        <link rel="icon" href="/favicon.ico">
        <meta property="og:title" content="Social Title">
        <meta name="twitter:description" content="Social description">
        <meta property="og:image" content="https://cdn.example.com/cover.png">
        <script type="application/ld+json">
          {
            "@context": "https://schema.org",
            "@type": "Article",
            "author": { "name": "Grace Hopper" },
            "datePublished": "2026-05-07T12:00:00Z",
            "publisher": { "name": "Structured Site" }
          }
        </script>
      </head>
      <body><article><p>Hello world.</p></article></body>
    </html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let metadata = extract_metadata(&doc);

    assert_eq!(metadata.title, "Social Title");
    assert_eq!(metadata.description, "Social description");
    assert_eq!(metadata.author, "Grace Hopper");
    assert_eq!(metadata.published, "2026-05-07T12:00:00Z");
    assert_eq!(metadata.site, "Structured Site");
    assert_eq!(metadata.domain, "example.com");
    assert_eq!(metadata.favicon, "https://example.com/favicon.ico");
    assert_eq!(metadata.image, "https://cdn.example.com/cover.png");
    assert!(metadata.schema_org_data.is_some());
}

#[test]
fn extracts_schema_org_data_from_type_with_parameters() {
    let html = r#"<!doctype html>
    <html lang="en">
      <head>
        <title>Fallback Title</title>
        <script type="application/ld+json; charset=utf-8">
          {
            "@context": "https://schema.org",
            "@type": "Article",
            "headline": "Parameterized JSON-LD Title"
          }
        </script>
      </head>
      <body><article><p>Hello world.</p></article></body>
    </html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let metadata = extract_metadata(&doc);

    assert_eq!(metadata.title, "Parameterized JSON-LD Title");
    assert!(metadata.schema_org_data.is_some());
}

#[test]
fn extracts_author_from_scalar_structured_source() {
    let html = r#"<!doctype html>
    <html lang="en">
      <head>
        <title>Scalar Author</title>
        <script type="application/ld+json">
          {
            "@context": "https://schema.org",
            "@type": "Article",
            "author": "Grace Hopper"
          }
        </script>
      </head>
      <body><article><p>Hello world.</p></article></body>
    </html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let metadata = extract_metadata(&doc);

    assert_eq!(metadata.author, "Grace Hopper");
}

#[test]
fn extracts_site_name_from_website_schema_node() {
    let html = r#"<!doctype html>
    <html lang="en">
      <head>
        <title>Article Title</title>
        <script type="application/ld+json">
          {
            "@context": "https://schema.org",
            "@graph": [
              { "@type": "WebSite", "name": "Example Journal" },
              { "@type": "Article", "headline": "Article Title" }
            ]
          }
        </script>
      </head>
      <body><article><p>Hello world.</p></article></body>
    </html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let metadata = extract_metadata(&doc);

    assert_eq!(metadata.site, "Example Journal");
}

#[test]
fn does_not_use_website_schema_name_as_article_title() {
    let html = r#"<!doctype html>
    <html lang="en">
      <head>
        <title>HTML Article Title</title>
        <script type="application/ld+json">
          {
            "@context": "https://schema.org",
            "@graph": [
              { "@type": "WebSite", "name": "Example Journal" },
              { "@type": "Organization", "name": "Example Org" },
              { "@type": "Article", "name": "Schema Article Title" }
            ]
          }
        </script>
      </head>
      <body><article><p>Hello world.</p></article></body>
    </html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let metadata = extract_metadata(&doc);

    assert_eq!(metadata.title, "Schema Article Title");
}

#[test]
fn falls_back_to_html_title_when_schema_has_only_site_names() {
    let html = r#"<!doctype html>
    <html lang="en">
      <head>
        <title>HTML Article Title</title>
        <script type="application/ld+json">
          {
            "@context": "https://schema.org",
            "@graph": [
              { "@type": "WebSite", "name": "Example Journal" },
              { "@type": "Organization", "name": "Example Org" }
            ]
          }
        </script>
      </head>
      <body><article><p>Hello world.</p></article></body>
    </html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let metadata = extract_metadata(&doc);

    assert_eq!(metadata.title, "HTML Article Title");
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
fn reference_priority_selectors_can_beat_larger_generic_main() {
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
fn generic_content_paragraph_does_not_beat_larger_main_container() {
    let first_section = "Opening paragraph with useful context. ".repeat(18);
    let second_section = "Second section with details that must stay with the article. ".repeat(18);
    let html = format!(
        r#"
    <html><body>
      <main>
        <h1>Complete Article</h1>
        <div class="content-paragraph"><p>{first_section}</p></div>
        <p>{second_section}</p>
      </main>
    </body></html>"#
    );
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());

    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("Complete Article"));
    assert!(main.contains("Opening paragraph with useful context"));
    assert!(main.contains("Second section with details"));
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
fn short_article_candidate_is_not_replaced_by_paragraph_wrapped_noise() {
    let noise = "sidebar noise ".repeat(200);
    let html = format!(
        r#"
    <html><body>
      <article><p>Focused article content stays selected even when surrounding layout contains noisy sidebar text for readers.</p></article>
      <aside><p>{noise}</p></aside>
      <footer><p>Footer text that should not be included.</p></footer>
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
fn uses_structured_body_when_it_is_more_complete_than_visible_shell() {
    let structured_text = "This is the full article body with enough words to beat the short visible shell. It includes the important details that agents need, and it should become the extracted content when the page markup only exposes a tiny placeholder.";
    let html = format!(
        r#"
    <html>
      <head>
        <script type="application/ld+json">
          {{
            "@context": "https://schema.org",
            "@type": "Article",
            "articleBody": "{structured_text}"
          }}
        </script>
      </head>
      <body>
        <article><p>Short shell.</p></article>
        <section id="full-story">
          <p>This is the <strong>full article body</strong> with enough words to beat the short visible shell. It includes the important details that agents need, and it should become the extracted content when the page markup only exposes a tiny placeholder.</p>
        </section>
      </body>
    </html>"#
    );
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());

    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("full article body"));
    assert!(main.contains("<strong>full article body</strong>"));
    assert!(!main.contains("Short shell"));
}

#[test]
fn structured_body_is_not_replaced_by_noisy_body_retry() {
    let structured_text = "Structured article text has enough complete details for agents and should remain selected.";
    let noise = "navigation sidebar footer noise ".repeat(120);
    let html = format!(
        r#"
    <html>
      <head>
        <script type="application/ld+json">
          {{
            "@context": "https://schema.org",
            "@type": "Article",
            "articleBody": "{structured_text}"
          }}
        </script>
      </head>
      <body>
        <article><p>Short shell.</p></article>
        <section><p>{structured_text}</p></section>
        <aside>{noise}</aside>
      </body>
    </html>"#
    );
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());

    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("Structured article text"));
    assert!(!main.contains("navigation sidebar footer noise"));
    assert!(!main.contains("Short shell"));
}

#[test]
fn uses_structured_body_when_visible_body_has_no_words() {
    let html = r#"
    <html>
      <head>
        <script type="application/ld+json">
          {
            "@context": "https://schema.org",
            "@type": "Article",
            "articleBody": "Only structured article text is available here, but it is still useful content for agents."
          }
        </script>
      </head>
      <body></body>
    </html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());

    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("Only structured article text is available here"));
}

#[test]
fn falls_back_to_plain_structured_body_when_body_only_contains_schema_script() {
    let html = r#"
    <html>
      <head></head>
      <body>
        <script type="application/ld+json">
          {
            "@context": "https://schema.org",
            "@type": "Article",
            "articleBody": "Only structured article text is available here, but the schema script should not be selected as visible content."
          }
        </script>
      </body>
    </html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());

    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("Only structured article text is available here"));
    assert!(!main.contains("<script"));
}

#[test]
fn ignores_non_article_schema_text_for_structured_body_fallback() {
    let html = r#"
    <html>
      <head>
        <script type="application/ld+json">
          {
            "@context": "https://schema.org",
            "@type": "FAQPage",
            "mainEntity": [
              {
                "@type": "Question",
                "name": "What is included?",
                "acceptedAnswer": {
                  "@type": "Answer",
                  "text": "This FAQ answer is long enough to beat the short visible article shell, but it is not the article body and should not replace the visible content."
                }
              }
            ]
          }
        </script>
      </head>
      <body><article><p>Short visible article shell.</p></article></body>
    </html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());

    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("Short visible article shell"));
    assert!(!main.contains("This FAQ answer is long enough"));
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
fn unwraps_javascript_links_without_losing_inner_content() {
    let html = r#"
    <article>
      <p>This has a <a href="javascript:void(0)">simple js link</a> in a sentence.</p>
      <p>A <a href="javascript:void(0)"><strong>bold js link</strong></a> should keep formatting.</p>
      <p>Normal <a href="https://example.com">links</a> should stay linked.</p>
    </article>"#;

    let cleaned = clean_html(html, &CleanOptions { no_images: false });
    let markdown = html_to_markdown(&cleaned, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("This has a simple js link in a sentence."));
    assert!(markdown.contains("A **bold js link** should keep formatting."));
    assert!(markdown.contains("[links](https://example.com)"));
    assert!(!markdown.contains("javascript:"));
}

#[test]
fn removes_hidden_and_embedded_noise_elements() {
    let html = r#"
    <article>
      <h1>Visible article</h1>
      <div style="visibility: hidden;"><p>Hidden teaser should disappear.</p></div>
      <div hidden><p>Hidden attribute should disappear.</p></div>
      <p>Keep this visible paragraph.</p>
      <iframe src="about:blank">Iframe fallback test</iframe>
      <object data="foo.swf">Object fallback test</object>
      <embed src="foo.swf">
    </article>"#;

    let cleaned = clean_html(html, &CleanOptions { no_images: false });

    assert!(cleaned.contains("Keep this visible paragraph."));
    assert!(!cleaned.contains("Hidden teaser"));
    assert!(!cleaned.contains("Hidden attribute"));
    assert!(!cleaned.contains("Iframe fallback"));
    assert!(!cleaned.contains("Object fallback"));
    assert!(!cleaned.contains("<embed"));
}

#[test]
fn hidden_cleanup_does_not_match_attribute_text() {
    let html = r#"
    <article>
      <section aria-label="The hidden costs of parser shortcuts">
        <p>Visible section with wording that should not trigger hidden cleanup.</p>
      </section>
      <p data-note="a hidden gem">Visible paragraph with attribute prose.</p>
    </article>"#;

    let cleaned = clean_html(html, &CleanOptions { no_images: false });

    assert!(cleaned.contains("Visible section with wording"));
    assert!(cleaned.contains("Visible paragraph with attribute prose."));
}

#[test]
fn removes_breadcrumb_blocks_without_semantic_nav_tags() {
    let html = r#"
    <main>
      <div data-block="nav">
        <ul><li><a href="/">Home</a></li><li><a href="/archive">Posts</a></li></ul>
      </div>
      <p>Not a shadowing day or research interview — a real job.</p>
    </main>"#;

    let cleaned = clean_html(html, &CleanOptions { no_images: false });

    assert!(cleaned.contains("Not a shadowing day"));
    assert!(!cleaned.contains("Home"));
    assert!(!cleaned.contains("Posts"));
}

#[test]
fn removes_fragment_only_table_of_contents_lists() {
    let html = r##"
    <article>
      <h1>Installation Guide</h1>
      <ul>
        <li><a href="#start">Start Here</a></li>
        <li><a href="#configure">Configure</a></li>
        <li><a href="#finish">Finish Up</a></li>
      </ul>
      <h2 id="start">Start Here</h2>
      <p>The system is installed as the sole operating system.</p>
    </article>"##;

    let cleaned = clean_html(html, &CleanOptions { no_images: false });

    assert!(cleaned.contains("The system is installed"));
    assert!(cleaned.contains("Start Here</h2>"));
    assert!(!cleaned.contains("href=\"#start\""));
    assert!(!cleaned.contains("Configure</a>"));
}

#[test]
fn keeps_short_fragment_link_lists_that_are_not_table_of_contents() {
    let html = r##"
    <article>
      <h1>Release notes</h1>
      <p>The two internal references below are part of the sentence flow.</p>
      <ul>
        <li><a href="#api">API compatibility note</a></li>
        <li><a href="#migration">Migration footnote</a></li>
      </ul>
      <h2 id="api">API</h2>
      <p>Keep the API section.</p>
    </article>"##;

    let cleaned = clean_html(html, &CleanOptions { no_images: false });

    assert!(cleaned.contains("API compatibility note"));
    assert!(cleaned.contains("Migration footnote"));
    assert!(cleaned.contains("Keep the API section."));
}

#[test]
fn keeps_descriptive_fragment_link_lists_that_are_not_table_of_contents() {
    let html = r##"
    <article>
      <h1>Parser Concepts</h1>
      <ul>
        <li><a href="#tokens">Tokens</a>: the smallest pieces emitted by the tokenizer.</li>
        <li><a href="#tree">Tree construction</a>: how nested HTML nodes are assembled.</li>
        <li><a href="#cleanup">Cleanup</a>: post-processing that removes boilerplate.</li>
      </ul>
      <h2 id="tokens">Tokens</h2>
      <p>The tokenizer detail remains part of the article.</p>
    </article>"##;

    let cleaned = clean_html(html, &CleanOptions { no_images: false });

    assert!(cleaned.contains("the smallest pieces emitted"));
    assert!(cleaned.contains("how nested HTML nodes are assembled"));
    assert!(cleaned.contains("post-processing that removes boilerplate"));
}

#[test]
fn removes_link_dense_related_sections() {
    let html = r##"
    <article>
      <h1>How Coffee Cools</h1>
      <p>Coffee cools following Newton's law of cooling.</p>
      <section>
        <p><a href="/pattern/">Maybe there's a pattern here?</a> · <a href="/#science">science</a> <a href="/#ai">AI</a></p>
        <p><a href="/data-wall/">The real data wall is billions of years of evolution</a> · <a href="/#ai">AI</a></p>
      </section>
    </article>"##;

    let cleaned = clean_html(html, &CleanOptions { no_images: false });

    assert!(cleaned.contains("Coffee cools following"));
    assert!(!cleaned.contains("Maybe there's a pattern"));
    assert!(!cleaned.contains("The real data wall"));
}

#[test]
fn keeps_mid_article_link_dense_resource_sections() {
    let html = r##"
    <article>
      <h1>Protocol Notes</h1>
      <p>Read these references before changing the parser.</p>
      <section>
        <p><a href="/spec/a">Specification A</a> · <a href="/spec/b">Specification B</a></p>
        <p><a href="/guide">Implementation guide</a> · <a href="/examples">Examples</a></p>
      </section>
      <p>The next paragraph explains why those links matter to the implementation.</p>
    </article>"##;

    let cleaned = clean_html(html, &CleanOptions { no_images: false });

    assert!(cleaned.contains("Specification A"));
    assert!(cleaned.contains("Implementation guide"));
    assert!(cleaned.contains("The next paragraph explains"));
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
fn keeps_code_fence_languages_aligned_with_their_code_blocks() {
    let html = r#"
    <article>
      <pre><code>plain text</code></pre>
      <pre><code class="language-rust">fn main() {}</code></pre>
    </article>"#;

    let markdown = html_to_markdown(html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("```\nplain text\n```"));
    assert!(markdown.contains("```rust\nfn main() {}\n```"));
    assert!(!markdown.contains("```rust\nplain text"));
}

#[test]
fn preserves_code_fence_language_when_attribute_has_spaces() {
    let html = r#"
    <article>
      <pre><code class = "language-rust">fn main() {}</code></pre>
    </article>"#;

    let markdown = html_to_markdown(html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("```rust\nfn main() {}\n```"));
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
