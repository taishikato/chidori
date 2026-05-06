use chidori::{
    error::ChidoriError,
    fetcher::{fetch_url, FetchConfig},
};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn fetches_html_with_language_and_user_agent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/page"))
        .and(header("accept-language", "ja"))
        .and(header("user-agent", "TestAgent/1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            "<html><body>Hello</body></html>",
            "text/html; charset=utf-8",
        ))
        .mount(&server)
        .await;

    let html = fetch_url(
        &format!("{}/page", server.uri()).parse().unwrap(),
        &FetchConfig {
            timeout: Duration::from_millis(1000),
            max_bytes: 5 * 1024 * 1024,
            user_agent: "TestAgent/1.0".to_string(),
            lang: Some("ja".to_string()),
        },
    )
    .await
    .unwrap();

    assert!(html.body.contains("Hello"));
}

#[tokio::test]
async fn rejects_non_html_content_type() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string("{}"),
        )
        .mount(&server)
        .await;

    let error = fetch_url(&server.uri().parse().unwrap(), &FetchConfig::default())
        .await
        .unwrap_err();

    assert!(matches!(error, ChidoriError::UnsupportedContentType(_)));
}

#[tokio::test]
async fn rejects_content_type_with_html_only_in_parameter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw("{}", "application/json; profile=\"text/html\""),
        )
        .mount(&server)
        .await;

    let error = fetch_url(&server.uri().parse().unwrap(), &FetchConfig::default())
        .await
        .unwrap_err();

    assert!(matches!(error, ChidoriError::UnsupportedContentType(_)));
}

#[tokio::test]
async fn maps_body_read_timeout_to_timeout_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0; 1024];
        let _ = stream.read(&mut buffer);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: 32\r\n\r\n")
            .unwrap();
        stream.flush().unwrap();
        thread::sleep(Duration::from_millis(300));
        let _ = stream.write_all(b"<html><body>Hello</body></html>");
    });

    let error = fetch_url(
        &url.parse().unwrap(),
        &FetchConfig {
            timeout: Duration::from_millis(50),
            ..FetchConfig::default()
        },
    )
    .await
    .unwrap_err();

    server.join().unwrap();
    assert!(matches!(error, ChidoriError::Timeout(50)));
}
