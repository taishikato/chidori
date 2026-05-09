use assert_cmd::Command;
use chidori::fetcher::{BOT_USER_AGENT, DEFAULT_USER_AGENT};
use predicates::prelude::*;
use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn html_response(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body.to_string(), "text/html; charset=utf-8")
}

#[test]
fn help_mentions_agent_first_options() {
    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--max-chars"))
        .stdout(predicate::str::contains("--no-images"))
        .stdout(predicate::str::contains("--json"));
}

#[test]
fn version_prints_package_version() {
    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")))
        .stderr(predicate::str::is_empty());
}

#[test]
fn invalid_url_exits_with_code_2() {
    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg("not-a-url")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("invalid URL"));
}

#[tokio::test]
async fn fetches_url_and_prints_markdown_to_stdout() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/article"))
        .respond_with(html_response(
            r#"
            <html><head><title>Example</title></head>
            <body><nav>Menu</nav><article><h1>Example</h1><p>Useful body.</p></article></body></html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/article", server.uri()))
        .assert()
        .success()
        .stdout(predicate::str::contains("# Example"))
        .stdout(predicate::str::contains("Useful body."))
        .stdout(predicate::str::contains("Menu").not())
        .stderr(predicate::str::is_empty());
}

#[tokio::test]
async fn retries_short_entry_candidate_and_prints_recovered_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/retry"))
        .respond_with(html_response(
            r#"
            <html><body>
              <article><p>Stub.</p></article>
              <div class="docs-page">
                <h1>Recovered Docs</h1>
                <p>This useful documentation section is recovered by the retry path.</p>
                <p>It gives coding agents enough Markdown to work with.</p>
                <p>The first article candidate was too short to be useful.</p>
              </div>
            </body></html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/retry", server.uri()))
        .assert()
        .success()
        .stdout(predicate::str::contains("# Recovered Docs"))
        .stdout(predicate::str::contains("coding agents enough Markdown"))
        .stderr(predicate::str::is_empty());
}

#[tokio::test]
async fn json_outputs_metadata_and_markdown() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/json"))
        .respond_with(html_response(
            r#"
            <html lang="en">
              <head><title>JSON Article</title><meta name="description" content="JSON fixture"></head>
              <body><article><h1>JSON Article</h1><p>Machine readable body.</p></article></body>
            </html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    let output = cmd
        .arg(format!("{}/json", server.uri()))
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["title"], "JSON Article");
    assert_eq!(json["description"], "JSON fixture");
    assert!(json["markdown"]
        .as_str()
        .unwrap()
        .contains("# JSON Article"));
}

#[tokio::test]
async fn json_title_falls_back_to_extracted_article_heading() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/chrome-title"))
        .respond_with(html_response(
            r#"
            <html lang="en">
              <head><title>Loading</title></head>
              <body>
                <h1>Cookie settings</h1>
                <article>
                  <h1>Real Article Title</h1>
                  <p>This real article body has enough useful words to be selected by extraction.</p>
                </article>
              </body>
            </html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    let output = cmd
        .arg(format!("{}/chrome-title", server.uri()))
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["title"], "Real Article Title");
    assert!(json["markdown"]
        .as_str()
        .unwrap()
        .contains("# Real Article Title"));
}

#[tokio::test]
async fn extracts_raw_markdown_body_without_dom_whitespace_loss() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/raw-markdown"))
        .respond_with(html_response(
            r#"
            <html><head><title>Raw Markdown</title></head><body>
# Raw Markdown

This body is already **Markdown** and should stay that way.

- first item
- second item with [a link](https://example.com)

    cargo test --all
            </body></html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/raw-markdown", server.uri()))
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "This body is already **Markdown**",
        ))
        .stdout(predicate::str::contains(
            "- second item with [a link](https://example.com)",
        ))
        .stdout(predicate::str::contains("    cargo test --all"))
        .stderr(predicate::str::is_empty());
}

#[tokio::test]
async fn no_images_removes_raw_markdown_images() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/raw-markdown-images"))
        .respond_with(html_response(
            r#"
            <html><head><title>Raw Markdown Images</title></head><body>
# Raw Markdown Images

This body is already **Markdown**.

![Hero image](https://example.com/hero.png)

- item with [a link](https://example.com)
            </body></html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/raw-markdown-images", server.uri()))
        .arg("--no-images")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "This body is already **Markdown**",
        ))
        .stdout(predicate::str::contains("[a link](https://example.com)"))
        .stdout(predicate::str::contains("Hero image").not())
        .stdout(predicate::str::contains("hero.png").not())
        .stderr(predicate::str::is_empty());
}

#[tokio::test]
async fn source_url_exercises_domain_specific_extraction_for_local_fixtures() {
    let server = MockServer::start().await;
    let fixture =
        std::fs::read_to_string("tests/fixtures/reference/domain--video-watch-page.html").unwrap();
    Mock::given(method("GET"))
        .and(path("/video-fixture"))
        .respond_with(html_response(&fixture))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/video-fixture", server.uri()))
        .arg("--source-url")
        .arg("https://www.youtube.com/watch?v=abc123xyz")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Building a Parser Garden"))
        .stdout(predicate::str::contains("Example Channel"))
        .stdout(predicate::str::contains(
            "This walkthrough shows how small extraction fixtures make CLI output predictable.",
        ))
        .stdout(predicate::str::contains("Inline Recommendation That Should Not Win").not())
        .stdout(predicate::str::contains("Related Video That Should Not Win").not())
        .stdout(predicate::str::contains("123K views").not())
        .stderr(predicate::str::is_empty());
}

#[tokio::test]
async fn source_url_extracts_repository_issue_discussion() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repo-issue"))
        .respond_with(html_response(
            r#"
            <html><body>
              <h1 id="search-suggestions-dialog-header">Search code, repositories, users, issues, pull requests...</h1>
              <aside>Repository sidebar noise</aside>
              <main>
                <h1><bdi data-testid="issue-title">Improve parser diagnostics</bdi></h1>
                <div class="markdown-body"><p>Issue body with useful reproduction details.</p></div>
                <div class="timeline-comment">
                  <a class="author">alice</a>
                  <div class="markdown-body"><p>First comment should be preserved.</p></div>
                </div>
              </main>
            </body></html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/repo-issue", server.uri()))
        .arg("--source-url")
        .arg("https://github.com/acme/widgets/issues/42")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Improve parser diagnostics"))
        .stdout(predicate::str::contains("Search code, repositories").not())
        .stdout(predicate::str::contains(
            "Issue body with useful reproduction details.",
        ))
        .stdout(predicate::str::contains(
            "First comment should be preserved.",
        ))
        .stdout(predicate::str::contains("Repository sidebar noise").not());
}

#[tokio::test]
async fn source_url_prefers_known_encyclopedia_content_selector() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/wiki"))
        .respond_with(html_response(
            r#"
            <html><body>
              <main>
                <div class="sidebar">Navigation box should not win.</div>
                <h1 id="firstHeading">Parser Combinators</h1>
                <div id="mw-content-text">
                  <p>Parser combinators are a technique for building parsers from small functions.</p>
                </div>
              </main>
            </body></html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/wiki", server.uri()))
        .arg("--source-url")
        .arg("https://en.wikipedia.org/wiki/Parser_combinator")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Parser Combinators"))
        .stdout(predicate::str::contains(
            "Parser combinators are a technique",
        ))
        .stdout(predicate::str::contains("Navigation box should not win").not());
}

#[tokio::test]
async fn retries_with_bot_user_agent_when_initial_page_has_no_extractable_content() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/bot-markdown"))
        .and(header("user-agent", DEFAULT_USER_AGENT))
        .respond_with(html_response(
            r#"<html><head><title>Bot Markdown</title></head><body><div id="app"></div><script>hydrate()</script></body></html>"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/bot-markdown"))
        .and(header("user-agent", BOT_USER_AGENT))
        .respond_with(html_response(
            r#"
            <html><head><title>Bot Markdown</title></head><body>
# Bot Markdown

This bot-rendered body keeps **Markdown** syntax intact.

- recovered item
- [recovered link](https://example.com/recovered)
            </body></html>
            "#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/bot-markdown", server.uri()))
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "This bot-rendered body keeps **Markdown** syntax intact.",
        ))
        .stdout(predicate::str::contains(
            "- [recovered link](https://example.com/recovered)",
        ))
        .stderr(predicate::str::is_empty());
}

#[tokio::test]
async fn json_debug_records_bot_user_agent_fallback() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/bot-debug"))
        .and(header("user-agent", DEFAULT_USER_AGENT))
        .respond_with(html_response(
            r#"<html><head><title>Bot Debug</title></head><body><div id="app"></div><script>hydrate()</script></body></html>"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/bot-debug"))
        .and(header("user-agent", BOT_USER_AGENT))
        .respond_with(html_response(
            r#"
            <html><head><title>Bot Debug</title></head><body>
# Bot Debug

This fallback markdown body came from the bot user agent.
            </body></html>
            "#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    let output = cmd
        .arg(format!("{}/bot-debug", server.uri()))
        .arg("--json")
        .arg("--debug")
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["debug"]["extractionPath"], "html");
    assert_eq!(
        json["debug"]["fallbacks"],
        Value::Array(vec![Value::String("bot-user-agent".to_string())])
    );
}

#[tokio::test]
async fn json_debug_records_hidden_content_fallback() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/hidden-article"))
        .respond_with(html_response(
            r#"
            <html><head><title>Hidden Article</title></head><body>
              <article hidden>
                <h1>Hidden Article</h1>
                <p>This useful article is hidden in the raw HTML but can still be recovered.</p>
              </article>
            </body></html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    let output = cmd
        .arg(format!("{}/hidden-article", server.uri()))
        .arg("--json")
        .arg("--debug")
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["debug"]["contentSelector"], "article");
    assert!(json["debug"]["fallbacks"]
        .as_array()
        .unwrap()
        .contains(&Value::String("hidden-content".to_string())));
    assert!(json["markdown"]
        .as_str()
        .unwrap()
        .contains("Hidden Article"));
}

#[tokio::test]
async fn json_debug_recovers_hidden_content_when_visible_body_is_shell_text() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/hidden-shell"))
        .respond_with(html_response(
            r#"
            <html><head><title>Hidden Shell</title></head><body>
              <div id="app">Loading...</div>
              <article hidden>
                <h1>Hidden Shell Article</h1>
                <p>This hidden article has useful words that should beat the visible loading shell.</p>
              </article>
            </body></html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    let output = cmd
        .arg(format!("{}/hidden-shell", server.uri()))
        .arg("--json")
        .arg("--debug")
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["debug"]["fallbacks"]
        .as_array()
        .unwrap()
        .contains(&Value::String("hidden-content".to_string())));
    assert!(json["markdown"]
        .as_str()
        .unwrap()
        .contains("Hidden Shell Article"));
    assert!(!json["markdown"].as_str().unwrap().contains("Loading..."));
}

#[tokio::test]
async fn does_not_retry_with_bot_user_agent_when_initial_page_is_extractable() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/normal"))
        .and(header("user-agent", DEFAULT_USER_AGENT))
        .respond_with(html_response(
            r#"<html><body><article><h1>Normal Article</h1><p>Initial content is enough.</p></article></body></html>"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/normal"))
        .and(header("user-agent", BOT_USER_AGENT))
        .respond_with(html_response(
            r#"<html><body># Bot content that should not be requested</body></html>"#,
        ))
        .expect(0)
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/normal", server.uri()))
        .assert()
        .success()
        .stdout(predicate::str::contains("# Normal Article"))
        .stdout(predicate::str::contains("Bot content").not())
        .stderr(predicate::str::is_empty());
}

#[tokio::test]
async fn custom_user_agent_disables_automatic_bot_retry() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/custom-ua"))
        .and(header("user-agent", "ChidoriTest/2.0"))
        .respond_with(html_response(
            r#"<html><head><title>Custom UA</title></head><body><div id="app"></div><script>hydrate()</script></body></html>"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/custom-ua"))
        .and(header("user-agent", BOT_USER_AGENT))
        .respond_with(html_response(
            r#"<html><body># Bot content that should not be requested</body></html>"#,
        ))
        .expect(0)
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/custom-ua", server.uri()))
        .arg("--user-agent")
        .arg("ChidoriTest/2.0")
        .assert()
        .failure()
        .code(7)
        .stderr(predicate::str::contains("no content could be extracted"));
}

#[tokio::test]
async fn json_outputs_extended_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/metadata"))
        .respond_with(html_response(
            r#"
            <html lang="en">
              <head>
                <title>Fallback</title>
                <link rel="icon" href="/favicon.ico">
                <meta property="og:title" content="Structured JSON Article">
                <meta property="og:image" content="https://cdn.example.com/cover.png">
                <script type="application/ld+json">
                  {
                    "@context": "https://schema.org",
                    "@type": "Article",
                    "author": { "name": "Grace Hopper" },
                    "publisher": { "name": "Example Journal" }
                  }
                </script>
              </head>
              <body><article><h1>Structured JSON Article</h1><p>Machine readable body.</p></article></body>
            </html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    let output = cmd
        .arg(format!("{}/metadata", server.uri()))
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["title"], "Structured JSON Article");
    assert_eq!(json["author"], "Grace Hopper");
    assert_eq!(json["site"], "Example Journal");
    assert_eq!(json["domain"], "127.0.0.1");
    assert!(json["favicon"].as_str().unwrap().ends_with("/favicon.ico"));
    assert_eq!(json["image"], "https://cdn.example.com/cover.png");
    assert!(json["schemaOrgData"].is_object());
}

#[tokio::test]
async fn output_writes_markdown_to_file_without_stdout() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/file"))
        .respond_with(html_response(
            r#"<html><body><article><h1>File Article</h1><p>Saved body.</p></article></body></html>"#,
        ))
        .mount(&server)
        .await;
    let dir = tempdir().unwrap();
    let output_path = dir.path().join("article.md");

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/file", server.uri()))
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::is_empty());

    let markdown = std::fs::read_to_string(output_path).unwrap();
    assert!(markdown.contains("# File Article"));
    assert!(markdown.contains("Saved body."));
}

#[tokio::test]
async fn max_chars_truncates_markdown_output() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/long"))
        .respond_with(html_response(
            r#"<html><body><article><h1>Long Article</h1><p>abcdefghijklmnopqrstuvwxyz</p></article></body></html>"#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    let output = cmd
        .arg(format!("{}/long", server.uri()))
        .arg("--max-chars")
        .arg("12")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.chars().count(), 12);
    assert_eq!(stdout, "# Long Artic");
}

#[tokio::test]
async fn timeout_exits_with_code_4() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/slow"))
        .respond_with(
            html_response(
                r#"<html><body><article><h1>Slow Article</h1><p>Slow body.</p></article></body></html>"#,
            )
            .set_delay(Duration::from_millis(200)),
        )
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/slow", server.uri()))
        .arg("--timeout")
        .arg("20")
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains(
            "timed out fetching page after 20 ms",
        ));
}

#[tokio::test]
async fn user_agent_and_lang_are_sent_as_headers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/headers"))
        .and(header("user-agent", "ChidoriTest/1.0"))
        .and(header("accept-language", "ja"))
        .respond_with(html_response(
            r#"<html><body><article><h1>Headers Article</h1><p>Header body.</p></article></body></html>"#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/headers", server.uri()))
        .arg("--user-agent")
        .arg("ChidoriTest/1.0")
        .arg("--lang")
        .arg("ja")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Headers Article"))
        .stderr(predicate::str::is_empty());
}

#[tokio::test]
async fn short_aliases_for_output_and_lang_work() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/aliases"))
        .and(header("accept-language", "en-US"))
        .respond_with(html_response(
            r#"<html><body><article><h1>Alias Article</h1><p>Alias body.</p></article></body></html>"#,
        ))
        .mount(&server)
        .await;
    let dir = tempdir().unwrap();
    let output_path = dir.path().join("alias.md");

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/aliases", server.uri()))
        .arg("-l")
        .arg("en-US")
        .arg("-o")
        .arg(&output_path)
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::is_empty());

    let markdown = std::fs::read_to_string(output_path).unwrap();
    assert!(markdown.contains("# Alias Article"));
}

#[tokio::test]
async fn no_images_removes_image_markdown() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/images"))
        .respond_with(html_response(
            r#"
            <html><body><article>
              <h1>Images Article</h1>
              <p>Text survives.</p>
              <img src="/hero.png" alt="Hero image">
            </article></body></html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/images", server.uri()))
        .arg("--no-images")
        .assert()
        .success()
        .stdout(predicate::str::contains("Text survives."))
        .stdout(predicate::str::contains("Hero image").not())
        .stdout(predicate::str::contains("hero.png").not())
        .stderr(predicate::str::is_empty());
}

#[tokio::test]
async fn debug_emits_diagnostics_to_stderr_without_polluting_stdout() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/debug"))
        .respond_with(html_response(
            r#"<html><body><article><h1>Debug Article</h1><p>Debug body.</p></article></body></html>"#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/debug", server.uri()))
        .arg("--debug")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Debug Article"))
        .stdout(predicate::str::contains("debug:").not())
        .stderr(predicate::str::contains("debug: fetched"))
        .stderr(predicate::str::contains("debug: extracted"));
}

#[tokio::test]
async fn debug_classifies_spa_shell_extraction_failures() {
    let server = MockServer::start().await;
    let shell = r#"
        <html><head><title>Client App</title></head>
        <body><div id="root"></div><script src="/assets/app.js"></script></body></html>
    "#;
    Mock::given(method("GET"))
        .and(path("/app"))
        .and(header("user-agent", DEFAULT_USER_AGENT))
        .respond_with(html_response(shell))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/app"))
        .and(header("user-agent", BOT_USER_AGENT))
        .respond_with(html_response(shell))
        .expect(1)
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/app", server.uri()))
        .arg("--debug")
        .assert()
        .failure()
        .code(7)
        .stderr(predicate::str::contains(
            "debug: extraction failed: spa-shell",
        ));
}

#[tokio::test]
async fn render_auto_uses_external_renderer_after_static_extraction_fails() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/spa"))
        .and(header("user-agent", DEFAULT_USER_AGENT))
        .respond_with(html_response(
            r#"<html><body><div id="root"></div><script>hydrate()</script></body></html>"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    let dir = tempdir().unwrap();
    let renderer = dir.path().join("renderer.sh");
    std::fs::write(
        &renderer,
        r#"#!/bin/sh
cat <<'HTML'
<html><body><article><h1>Rendered Article</h1><p>Hydrated content from renderer.</p></article></body></html>
HTML
"#,
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&renderer).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&renderer, permissions).unwrap();

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/spa", server.uri()))
        .arg("--render=auto")
        .env("CHIDORI_RENDER_COMMAND", &renderer)
        .assert()
        .success()
        .stdout(predicate::str::contains("# Rendered Article"))
        .stdout(predicate::str::contains("Hydrated content from renderer."));
}

#[tokio::test]
async fn render_auto_allows_renderer_command_arguments() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/spa-with-render-args"))
        .and(header("user-agent", DEFAULT_USER_AGENT))
        .respond_with(html_response(
            r#"<html><body><div id="root"></div><script>hydrate()</script></body></html>"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    let dir = tempdir().unwrap();
    let renderer = dir.path().join("renderer.sh");
    std::fs::write(
        &renderer,
        r#"#!/bin/sh
if [ "$1" != "--fixture" ] || [ "$2" != "rendered" ]; then
  exit 2
fi
cat <<'HTML'
<html><body><article><h1>Rendered With Args</h1><p>Renderer arguments were preserved.</p></article></body></html>
HTML
"#,
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&renderer).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&renderer, permissions).unwrap();

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/spa-with-render-args", server.uri()))
        .arg("--render=auto")
        .env(
            "CHIDORI_RENDER_COMMAND",
            format!("{} --fixture rendered", renderer.display()),
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("# Rendered With Args"))
        .stdout(predicate::str::contains(
            "Renderer arguments were preserved.",
        ));
}

#[tokio::test]
async fn render_auto_preserves_literal_renderer_paths_with_spaces() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/spa-with-space-path-renderer"))
        .and(header("user-agent", DEFAULT_USER_AGENT))
        .respond_with(html_response(
            r#"<html><body><div id="root"></div><script>hydrate()</script></body></html>"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    let dir = tempdir().unwrap();
    let renderer_dir = dir.path().join("renderer dir");
    std::fs::create_dir(&renderer_dir).unwrap();
    let renderer = renderer_dir.join("renderer.sh");
    std::fs::write(
        &renderer,
        r#"#!/bin/sh
cat <<'HTML'
<html><body><article><h1>Rendered Space Path</h1><p>Literal renderer path was preserved.</p></article></body></html>
HTML
"#,
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&renderer).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&renderer, permissions).unwrap();

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/spa-with-space-path-renderer", server.uri()))
        .arg("--render=auto")
        .env("CHIDORI_RENDER_COMMAND", &renderer)
        .assert()
        .success()
        .stdout(predicate::str::contains("# Rendered Space Path"))
        .stdout(predicate::str::contains(
            "Literal renderer path was preserved.",
        ));
}

#[tokio::test]
async fn render_auto_times_out_external_renderer() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/slow-renderer"))
        .and(header("user-agent", "ChidoriTest/2.0"))
        .respond_with(html_response(
            r#"<html><body><div id="root"></div><script>hydrate()</script></body></html>"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    let dir = tempdir().unwrap();
    let renderer = dir.path().join("slow-renderer.sh");
    std::fs::write(
        &renderer,
        r#"#!/bin/sh
sleep 2
cat <<'HTML'
<html><body><article><h1>Late Rendered Article</h1><p>This should arrive too late.</p></article></body></html>
HTML
"#,
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&renderer).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&renderer, permissions).unwrap();

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    let started = Instant::now();
    cmd.arg(format!("{}/slow-renderer", server.uri()))
        .arg("--render=auto")
        .arg("--timeout")
        .arg("50")
        .arg("--user-agent")
        .arg("ChidoriTest/2.0")
        .env("CHIDORI_RENDER_COMMAND", &renderer)
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains(
            "timed out fetching page after 50 ms",
        ));
    assert!(started.elapsed() < Duration::from_millis(1500));
}

#[tokio::test]
async fn render_auto_rejects_renderer_output_over_fetch_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/huge-renderer-output"))
        .and(header("user-agent", "ChidoriTest/2.0"))
        .respond_with(html_response(
            r#"<html><body><div id="root"></div><script>hydrate()</script></body></html>"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    let dir = tempdir().unwrap();
    let renderer = dir.path().join("huge-renderer.sh");
    std::fs::write(
        &renderer,
        r#"#!/bin/sh
printf '<html><body><article><h1>Rendered Too Large</h1><p>'
yes word | head -c 5500000
printf '</p></article></body></html>'
"#,
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&renderer).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&renderer, permissions).unwrap();

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/huge-renderer-output", server.uri()))
        .arg("--render=auto")
        .arg("--max-chars")
        .arg("20")
        .arg("--user-agent")
        .arg("ChidoriTest/2.0")
        .env("CHIDORI_RENDER_COMMAND", &renderer)
        .assert()
        .failure()
        .code(5)
        .stderr(predicate::str::contains("page too large"));
}

#[tokio::test]
async fn render_auto_times_out_waiting_for_renderer_descendant_stdout() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/forking-renderer"))
        .and(header("user-agent", "ChidoriTest/2.0"))
        .respond_with(html_response(
            r#"<html><body><div id="root"></div><script>hydrate()</script></body></html>"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    let dir = tempdir().unwrap();
    let renderer = dir.path().join("forking-renderer.py");
    std::fs::write(
        &renderer,
        r#"#!/usr/bin/env python3
import os
import sys
import time

pid = os.fork()
if pid:
    os._exit(0)
time.sleep(1)
"#,
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&renderer).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&renderer, permissions).unwrap();

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/forking-renderer", server.uri()))
        .arg("--render=auto")
        .arg("--timeout")
        .arg("500")
        .arg("--user-agent")
        .arg("ChidoriTest/2.0")
        .env("CHIDORI_RENDER_COMMAND", &renderer)
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains(
            "timed out fetching page after 500 ms",
        ));
}

#[tokio::test]
async fn render_auto_falls_back_to_bot_user_agent_when_renderer_is_unavailable() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/bot-render-auto"))
        .and(header("user-agent", DEFAULT_USER_AGENT))
        .respond_with(html_response(
            r#"<html><body><div id="root"></div><script>hydrate()</script></body></html>"#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/bot-render-auto"))
        .and(header("user-agent", BOT_USER_AGENT))
        .respond_with(html_response(
            r#"
            <html><body>
              <article>
                <h1>Bot Render Auto</h1>
                <p>The bot user-agent fallback still runs when rendering is unavailable.</p>
              </article>
            </body></html>
            "#,
        ))
        .expect(1)
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/bot-render-auto", server.uri()))
        .arg("--render=auto")
        .env_remove("CHIDORI_RENDER_COMMAND")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Bot Render Auto"))
        .stdout(predicate::str::contains(
            "The bot user-agent fallback still runs",
        ));
}

#[tokio::test]
async fn debug_classifies_unsupported_content_type_fetch_failures() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string("{}"),
        )
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/json", server.uri()))
        .arg("--debug")
        .assert()
        .failure()
        .code(6)
        .stderr(predicate::str::contains(
            "debug: fetch failed: unsupported-content-type",
        ));
}

#[tokio::test]
async fn debug_classifies_blocked_or_login_fetch_failures() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/login"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/login", server.uri()))
        .arg("--debug")
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains(
            "debug: fetch failed: blocked-or-login",
        ));
}

#[tokio::test]
async fn debug_classifies_link_dense_extraction_failures() {
    let server = MockServer::start().await;
    let links = (0..80)
        .map(|index| format!(r#"<a href="/{index}">Link {index}</a>"#))
        .collect::<Vec<_>>()
        .join(" ");
    let html = format!("<html><body><main>{links}</main></body></html>");
    Mock::given(method("GET"))
        .and(path("/links"))
        .and(header("user-agent", DEFAULT_USER_AGENT))
        .respond_with(html_response(&html))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/links"))
        .and(header("user-agent", BOT_USER_AGENT))
        .respond_with(html_response(&html))
        .expect(1)
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    cmd.arg(format!("{}/links", server.uri()))
        .arg("--debug")
        .assert()
        .failure()
        .code(7)
        .stderr(predicate::str::contains(
            "debug: extraction failed: too-link-dense",
        ));
}

#[tokio::test]
async fn json_debug_includes_structured_extraction_diagnostics() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/debug-json"))
        .respond_with(html_response(
            r#"
            <html><head><title>Debug JSON Article</title></head><body>
              <nav>Menu</nav>
              <article><h1>Debug JSON Article</h1><p>Debug JSON body.</p></article>
            </body></html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    let output = cmd
        .arg(format!("{}/debug-json", server.uri()))
        .arg("--json")
        .arg("--debug")
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["title"], "Debug JSON Article");
    assert_eq!(json["debug"]["extractionPath"], "html");
    assert_eq!(json["debug"]["fallbacks"], Value::Array(vec![]));
    assert!(json["debug"]["wordCount"].as_u64().unwrap() > 0);
    assert!(json["debug"]["timings"]["totalMs"].as_u64().is_some());
}

#[tokio::test]
async fn json_debug_includes_selected_content_candidate_details() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/candidate-debug"))
        .respond_with(html_response(
            r#"
            <html><head><title>Candidate Debug</title></head><body>
              <main><p>Short shell.</p></main>
              <article>
                <h1>Candidate Debug</h1>
                <p>This article body has enough useful words to win the extraction candidate.</p>
              </article>
            </body></html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    let output = cmd
        .arg(format!("{}/candidate-debug", server.uri()))
        .arg("--json")
        .arg("--debug")
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["debug"]["contentSelector"], "article");
    assert!(json["debug"]["contentScore"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn json_debug_includes_cleanup_removal_reasons() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/removal-debug"))
        .respond_with(html_response(
            r#"
            <html><head><title>Removal Debug</title></head><body>
              <article>
                <h1>Removal Debug</h1>
                <nav>Article-local menu</nav>
                <p>Useful body survives cleanup.</p>
              </article>
            </body></html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    let output = cmd
        .arg(format!("{}/removal-debug", server.uri()))
        .arg("--json")
        .arg("--debug")
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let removals = json["debug"]["removals"].as_array().unwrap();
    assert!(removals.iter().any(|removal| {
        removal["step"] == "clean-html"
            && removal["reason"] == "noise-tag"
            && removal["selector"] == "nav"
    }));
    assert!(!json["markdown"]
        .as_str()
        .unwrap()
        .contains("Article-local menu"));
}

#[tokio::test]
async fn json_debug_reports_body_selector_after_low_word_retry() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/body-retry-debug"))
        .respond_with(html_response(
            r#"
            <html><head><title>Body Retry</title></head><body>
              <article><p>Stub.</p></article>
              <div class="docs-page">
                <h1>Recovered Docs</h1>
                <p>This useful documentation section is recovered by the body retry path.</p>
                <p>It contains enough words to beat the placeholder article candidate.</p>
              </div>
            </body></html>
            "#,
        ))
        .mount(&server)
        .await;

    let mut cmd = Command::cargo_bin("chidori").unwrap();
    let output = cmd
        .arg(format!("{}/body-retry-debug", server.uri()))
        .arg("--json")
        .arg("--debug")
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["debug"]["contentSelector"], "body");
    assert!(json["markdown"]
        .as_str()
        .unwrap()
        .contains("Recovered Docs"));
}
