use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::time::Duration;
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
