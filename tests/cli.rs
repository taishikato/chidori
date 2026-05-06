use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(
                    r#"
            <html><head><title>Example</title></head>
            <body><nav>Menu</nav><article><h1>Example</h1><p>Useful body.</p></article></body></html>
            "#,
                    "text/html; charset=utf-8",
                ),
        )
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
