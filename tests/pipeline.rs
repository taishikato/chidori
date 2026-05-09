use chidori::{
    cleaner::{clean_html, CleanOptions},
    document::ParsedDocument,
    extractor::extract_main_html,
    markdown::{extract_raw_markdown, html_to_markdown, remove_markdown_images, MarkdownOptions},
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
fn cleans_site_suffix_from_html_title_when_site_name_is_known() {
    let html = r#"<!doctype html>
    <html lang="en">
      <head>
        <title>Readable Article | Example Site</title>
        <meta property="og:site_name" content="Example Site">
      </head>
      <body><article><p>Hello world.</p></article></body>
    </html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let metadata = extract_metadata(&doc);

    assert_eq!(metadata.title, "Readable Article");
    assert_eq!(metadata.site, "Example Site");
}

#[test]
fn extracts_canonical_meta_tags_and_richer_article_metadata() {
    let html = r#"<!doctype html>
    <html lang="en">
      <head>
        <title>Untitled</title>
        <link rel="canonical" href="/canonical-post">
        <meta property="og:site_name" content="Example Site">
        <meta name="citation_author" content="Katherine Johnson">
        <meta name="datePublished" content="2026-05-08">
        <meta name="description" content="A richer article">
      </head>
      <body>
        <article>
          <h1>Real Article Title</h1>
          <p>Hello world.</p>
        </article>
      </body>
    </html>"#;
    let doc = ParsedDocument::parse(html, Url::parse("https://example.com/post").unwrap());
    let metadata = extract_metadata(&doc);

    assert_eq!(metadata.title, "Real Article Title");
    assert_eq!(metadata.canonical_url, "https://example.com/canonical-post");
    assert_eq!(metadata.author, "Katherine Johnson");
    assert_eq!(metadata.published, "2026-05-08");
    assert!(metadata.meta_tags.iter().any(|tag| {
        tag.name.as_deref() == Some("description")
            && tag.content.as_deref() == Some("A richer article")
    }));
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
fn extracts_hacker_news_listing_items_as_readable_content() {
    let html = std::fs::read_to_string("tests/fixtures/reference/domain--hacker-news-listing.html")
        .unwrap();
    let doc = ParsedDocument::parse(
        html,
        Url::parse("https://news.ycombinator.com/news").unwrap(),
    );
    let main = extract_main_html(&doc).unwrap();
    let cleaned = clean_html(&main, &CleanOptions { no_images: false });
    let markdown = html_to_markdown(&cleaned, &MarkdownOptions { max_chars: None });

    assert!(
        markdown.contains("1. [Launch notes for a useful parser](https://example.com/post-one)")
    );
    assert!(markdown.contains("example.com"));
    assert!(markdown.contains("123 points"));
    assert!(markdown.contains("ada"));
    assert!(markdown.contains("17 comments"));
    assert!(markdown.contains(
        "2. [Ask HN: Keeping extracted content stable?](https://news.ycombinator.com/item?id=402)"
    ));
    assert!(markdown.contains("45 points"));
    assert!(markdown.contains("discuss"));
    assert!(!markdown.contains("past"));
    assert!(!markdown.contains("More"));
}

#[test]
fn hacker_news_item_pages_use_generic_extraction_for_discussions() {
    let html = r#"
    <html>
      <body>
        <table>
          <tr class="athing" id="401">
            <td class="title">
              <span class="titleline"><a href="https://example.com/post">Launch notes</a></span>
            </td>
          </tr>
          <tr><td class="subtext"><span class="score">123 points</span> | <a href="item?id=401">17 comments</a></td></tr>
        </table>
        <table class="comment-tree">
          <tr class="athing comtr"><td class="comment">This discussion comment should be extracted.</td></tr>
        </table>
      </body>
    </html>"#;
    let doc = ParsedDocument::parse(
        html,
        Url::parse("https://news.ycombinator.com/item?id=401").unwrap(),
    );

    let main = extract_main_html(&doc).unwrap();

    assert!(main.contains("This discussion comment should be extracted."));
}

#[test]
fn reddit_fallback_comments_keep_nested_replies_subordinate_and_unique() {
    let html = r#"
    <html><body>
      <main>
        <article data-testid="post-container">
          <h1>Fallback Reddit Thread</h1>
          <div data-testid="post-content"><p>Post body remains readable.</p></div>
        </article>
        <section aria-label="Comments">
          <div data-testid="comment">
            <header><a href="/user/parentuser/">u/parentuser</a></header>
            <span score="13 points"></span>
            <time>3 hours ago</time>
            <div class="md"><p>Parent comment should not absorb the reply.</p></div>
            <div class="replies">
              <div data-testid="comment">
                <header><a href="/user/replyuser/">u/replyuser</a></header>
                <span score="5 points"></span>
                <time>2 hours ago</time>
                <div class="md"><p>Nested fallback reply appears once.</p></div>
              </div>
            </div>
          </div>
        </section>
      </main>
    </body></html>"#;
    let doc = ParsedDocument::parse(
        html,
        Url::parse("https://www.reddit.com/r/rust/comments/abc123/example_post/").unwrap(),
    );
    let main = extract_main_html(&doc).unwrap();
    let cleaned = clean_html(&main, &CleanOptions { no_images: false });
    let markdown = html_to_markdown(&cleaned, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("u/parentuser"));
    assert!(markdown.contains("13 points · 3 hours ago"));
    assert!(markdown.contains("Parent comment should not absorb the reply."));
    assert!(markdown.contains("> u/replyuser"));
    assert!(markdown.contains("> 5 points · 2 hours ago"));
    assert!(markdown.contains("> Nested fallback reply appears once."));
    assert_eq!(markdown.matches("u/replyuser").count(), 1);
    assert_eq!(
        markdown
            .matches("Nested fallback reply appears once.")
            .count(),
        1
    );
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
      <p>A spaced <a href = ' javascript:void(0) '>js link</a> should also unwrap.</p>
      <p>A <a href="javascript:void(0)"><strong>bold js link</strong></a> should keep formatting.</p>
      <p>Normal <a href="https://example.com">links</a> should stay linked.</p>
    </article>"#;

    let cleaned = clean_html(html, &CleanOptions { no_images: false });
    let markdown = html_to_markdown(&cleaned, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("This has a simple js link in a sentence."));
    assert!(markdown.contains("A spaced js link should also unwrap."));
    assert!(markdown.contains("A **bold js link** should keep formatting."));
    assert!(markdown.contains("[links](https://example.com)"));
    assert!(!markdown.contains("javascript:"));
}

#[test]
fn converts_math_elements_to_markdown_delimiters() {
    let html = r#"
    <article>
      <p>Inline energy uses <math data-latex="E=mc^2"></math> in prose.</p>
      <math display="block" data-latex="\int_0^1 x\,dx = \frac{1}{2}"></math>
    </article>"#;

    let markdown = html_to_markdown(html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("Inline energy uses $E=mc^2$ in prose."));
    assert!(markdown.contains("$$\n\\int_0^1 x\\,dx = \\frac{1}{2}\n$$"));
    assert!(!markdown.contains("<math"));
}

#[test]
fn raw_markdown_detection_skips_normal_html_articles() {
    let html = r#"
    <html>
      <body>
        <article>
          <h1>Markdown Examples in HTML</h1>
          <p>This article talks about **bold syntax** and [links](https://example.com).</p>
          <p>The HTML structure should still go through normal extraction.</p>
        </article>
      </body>
    </html>"#;

    assert!(extract_raw_markdown(html).is_none());
}

#[test]
fn raw_markdown_detection_preserves_literal_less_than_text() {
    let html = r#"
    <html>
      <body>
# Markdown Notes

- Compare with a < b before choosing.
- Keep **bold** text after the comparison.
      </body>
    </html>"#;

    let markdown = extract_raw_markdown(html).unwrap();

    assert!(markdown.contains("Compare with a < b before choosing."));
    assert!(markdown.contains("Keep **bold** text after the comparison."));
}

#[test]
fn raw_markdown_detection_preserves_unclosed_angle_text() {
    let html = r#"
    <html>
      <body>
# Markdown Notes

- Run `chidori <url` from a shell.
- Keep **bold** text after the snippet.
      </body>
    </html>"#;

    let markdown = extract_raw_markdown(html).unwrap();

    assert!(markdown.contains("Run `chidori <url` from a shell."));
    assert!(markdown.contains("Keep **bold** text after the snippet."));
}

#[test]
fn remove_markdown_images_handles_parenthesized_urls() {
    let markdown = "Before ![plot](https://example.com/foo_(1).png) after.";

    let without_images = remove_markdown_images(markdown);

    assert_eq!(without_images, "Before  after.");
}

#[test]
fn converts_special_elements_with_whitespace_around_attribute_equals() {
    let html = r##"
    <article>
      <p>Inline acceleration uses <math data-latex = "a=b^2"></math> in prose.</p>
      <math display = "block" data-latex = "x=y^2"></math>
      <div class = "callout" data-callout = "note">
        <div class="callout-title-inner">Heads up</div>
        <div class="callout-content"><p>Whitespace in attributes should be fine.</p></div>
      </div>
      <p>The parser keeps cited claims<sup id = "fnref-1"><a href="#fn-1">1</a></sup> readable.</p>
      <section id = "footnotes"><ol><li id = "fn-1">Footnote text survives.</li></ol></section>
    </article>"##;

    let markdown = html_to_markdown(html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("Inline acceleration uses $a=b^2$ in prose."));
    assert!(markdown.contains("$$\nx=y^2\n$$"));
    assert!(markdown.contains("> [!note] Heads up"));
    assert!(markdown.contains("> Whitespace in attributes should be fine."));
    assert!(markdown.contains("cited claims[^1] readable."));
    assert!(markdown.contains("[^1]: Footnote text survives."));
}

#[test]
fn converts_single_quoted_math_attributes() {
    let html = r#"
    <article>
      <p>Inline acceleration uses <math data-latex='a=b^2'></math> in prose.</p>
    </article>"#;

    let markdown = html_to_markdown(html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("Inline acceleration uses $a=b^2$ in prose."));
}

#[test]
fn converts_callouts_to_obsidian_style_blockquotes() {
    let html = r#"
    <article>
      <div class="callout" data-callout="warning">
        <div class="callout-title"><div class="callout-title-inner">Careful</div></div>
        <div class="callout-content"><p>Do not delete the quoted payload.</p></div>
      </div>
    </article>"#;

    let markdown = html_to_markdown(html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("> [!warning] Careful"));
    assert!(markdown.contains("> Do not delete the quoted payload."));
    assert!(!markdown.contains("callout-title"));
}

#[test]
fn converts_callouts_without_flattening_nested_markdown() {
    let html = r#"
    <article>
      <div class="callout" data-callout="note">
        <div class="callout-title-inner">Keep shape</div>
        <div class="callout-content">
          <p>First paragraph.</p>
          <p>Second paragraph.</p>
          <pre><code>cargo test
cargo clippy</code></pre>
        </div>
      </div>
    </article>"#;

    let markdown = html_to_markdown(html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("> [!note] Keep shape"));
    assert!(markdown.contains("> First paragraph."));
    assert!(markdown.contains(">\n> Second paragraph."));
    assert!(markdown.contains("> ```"));
    assert!(markdown.contains("> cargo test"));
    assert!(markdown.contains("> cargo clippy"));
}

#[test]
fn converts_footnotes_to_markdown_references() {
    let html = r##"
    <article>
      <p>The parser keeps cited claims<sup id="fnref-1"><a href="#fn-1">1</a></sup> readable.</p>
      <section id="footnotes">
        <ol>
          <li id="fn-1"><p>Footnote text survives. <a class="footnote-backref" href="#fnref-1">↩</a></p></li>
        </ol>
      </section>
    </article>"##;

    let markdown = html_to_markdown(html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("cited claims[^1] readable."));
    assert!(markdown.contains("[^1]: Footnote text survives."));
    assert!(!markdown.contains("↩"));
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
fn removes_navigation_blocks_with_spaced_data_block_attribute() {
    let html = r#"
    <main>
      <div data-block = "nav">
        <ul><li><a href="/">Home</a></li><li><a href="/archive">Posts</a></li></ul>
      </div>
      <p>Visible paragraph survives.</p>
    </main>"#;

    let cleaned = clean_html(html, &CleanOptions { no_images: false });

    assert!(cleaned.contains("Visible paragraph survives."));
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
fn unwraps_soft_wrapped_paragraphs_without_flattening_blocks() {
    let long_text = "This paragraph is long enough for html2md to wrap it across multiple output lines, but the markdown renderer should keep it as one paragraph line for cleaner downstream parsing and comparison.";
    let html = format!(
        r#"
    <article>
      <p>{long_text}</p>
      <ol>
        <li>First item that should remain a list item.</li>
        <li>Second item that should remain a list item.</li>
      </ol>
      <pre><code>alpha
beta</code></pre>
    </article>"#
    );

    let markdown = html_to_markdown(&html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains(long_text));
    assert!(markdown.contains("1. First item that should remain a list item."));
    assert!(markdown.contains("2. Second item that should remain a list item."));
    assert!(markdown.contains("```\nalpha\nbeta\n```"));
}

#[test]
fn unwraps_soft_wrapped_paragraphs_without_breaking_setext_headings() {
    let html = r#"
    <article>
      <h1>Setext Title</h1>
      <p>Short body.</p>
    </article>"#;

    let markdown = html_to_markdown(html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("# Setext Title"));
    assert!(!markdown.contains("Setext Title =========="));
}

#[test]
fn unwraps_soft_wrapped_paragraphs_without_flattening_br_breaks() {
    let html = r#"
    <article>
      <p>First line<br>Second line</p>
    </article>"#;

    let markdown = html_to_markdown(html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains("First line  \nSecond line"));
    assert!(!markdown.contains("First line Second line"));
}

#[test]
fn unwraps_soft_wrapped_list_items() {
    let long_item = "A list item can be long enough for html2md to wrap the continuation onto another line, but it should stay on the same markdown list line.";
    let html = format!(
        r#"
    <article>
      <ul>
        <li>{long_item}</li>
      </ul>
    </article>"#
    );

    let markdown = html_to_markdown(&html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains(&format!("* {long_item}")));
}

#[test]
fn unwraps_soft_wrapped_list_items_without_flattening_nested_lists() {
    let child_item = "Nested list items should keep their indentation even when the parent list item is normalized.";
    let html = format!(
        r#"
    <article>
      <ul>
        <li>Parent item
          <ul>
            <li>{child_item}</li>
          </ul>
        </li>
      </ul>
    </article>"#
    );

    let markdown = html_to_markdown(&html, &MarkdownOptions { max_chars: None });

    assert!(markdown.contains(&format!("  * {child_item}")));
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
