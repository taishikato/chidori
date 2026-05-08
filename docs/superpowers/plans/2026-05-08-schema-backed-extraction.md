# Schema-Backed Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve chidori extraction when visible HTML is incomplete by using structured article data as a fallback, while enriching JSON metadata for agent workflows.

**Architecture:** Keep `extract_main_html(&ParsedDocument)` as the public extraction entry point. Add structured-data parsing to `src/metadata.rs`, then let `src/extractor.rs` use the parsed article text only when it is clearly more complete than the visible candidate.

**Tech Stack:** Rust 2021, `scraper` for DOM selection, `serde_json` for structured data, existing `cargo test` integration tests.

---

## File Structure

- Modify `src/metadata.rs`: parse JSON-LD article data, expose structured content text, and enrich serialized metadata.
- Modify `src/extractor.rs`: compare the selected visible candidate against structured article text and prefer the smallest matching DOM element when appropriate.
- Modify `tests/pipeline.rs`: add focused tests for enriched metadata and structured-content fallback.
- Modify `tests/cli.rs`: add one JSON-mode E2E test proving the new metadata fields are emitted through the CLI.
- No changes to fetching, Markdown rendering, package metadata, or npm wrapper behavior.

---

## Task 1: Extended Metadata From Structured And Social Sources

**Files:**
- Modify: `src/metadata.rs`
- Test: `tests/pipeline.rs`

- [ ] **Step 1: Write the failing metadata test**

Add this test after `extracts_basic_metadata` in `tests/pipeline.rs`:

```rust
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
```

- [ ] **Step 2: Verify the test fails**

Run:

```bash
cargo test extracts_extended_metadata_from_social_and_structured_sources
```

Expected: compile failure because `Metadata` does not yet expose `domain`, `favicon`, `image`, or `schema_org_data`.

- [ ] **Step 3: Add metadata fields and structured-data parsing**

Update `src/metadata.rs` so `Metadata` includes:

```rust
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub url: String,
    pub final_url: String,
    pub domain: String,
    pub title: String,
    pub description: String,
    pub favicon: String,
    pub image: String,
    pub site: String,
    pub author: String,
    pub published: String,
    pub language: String,
    pub schema_org_data: Option<serde_json::Value>,
    pub word_count: usize,
}
```

Add helpers that:

```rust
pub fn extract_schema_org_data(doc: &ParsedDocument) -> Option<serde_json::Value>;
pub fn structured_content_text(doc: &ParsedDocument) -> Option<String>;
```

Implementation requirements:
- Parse every `script[type="application/ld+json"]`.
- Return `None` when no script parses successfully.
- Return the single parsed object when one script parses.
- Return `Value::Array` when multiple scripts parse.
- Resolve `link[rel~="icon"]` against `doc.url`.
- Prefer `og:title` / `twitter:title` before `<title>`.
- Prefer `twitter:description` after regular and Open Graph description.
- Pull author, site, published date, image, and language from structured data when normal meta tags are missing.

- [ ] **Step 4: Verify metadata test passes**

Run:

```bash
cargo test extracts_extended_metadata_from_social_and_structured_sources
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/metadata.rs tests/pipeline.rs
git commit -m "feat: enrich extraction metadata"
```

---

## Task 2: Structured Content Fallback

**Files:**
- Modify: `src/extractor.rs`
- Test: `tests/pipeline.rs`

- [ ] **Step 1: Write the failing extraction test**

Add this test near the existing retry tests in `tests/pipeline.rs`:

```rust
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
```

- [ ] **Step 2: Verify the test fails for the right reason**

Run:

```bash
cargo test uses_structured_body_when_it_is_more_complete_than_visible_shell
```

Expected: FAIL because extraction selects the short visible article or an overly broad body retry.

- [ ] **Step 3: Add structured fallback before broad body retry**

In `src/extractor.rs`, add a helper with this behavior:

```rust
fn structured_content_candidate(
    doc: &ParsedDocument,
    current_word_count: usize,
) -> Result<Option<String>, ChidoriError>
```

Implementation requirements:
- Call `crate::metadata::structured_content_text(doc)`.
- Ignore empty structured text.
- Use the structured candidate only when its word count is more than 1.5x the current visible candidate.
- Search `body *` for the smallest element whose normalized text contains the structured text.
- Return that element's HTML when found, preserving inline formatting.
- Return escaped plain text when no matching DOM element exists.
- Run this fallback before broad body retry so a focused structured match is not displaced by layout text.

- [ ] **Step 4: Verify the extraction test passes**

Run:

```bash
cargo test uses_structured_body_when_it_is_more_complete_than_visible_shell
```

Expected: PASS.

- [ ] **Step 5: Run existing pipeline and full suite checks**

Run:

```bash
cargo test --test pipeline
cargo test
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/extractor.rs tests/pipeline.rs
git commit -m "feat: use structured article fallback"
```

---

## Task 3: CLI JSON Coverage

**Files:**
- Modify: `tests/cli.rs`
- Verify: `src/output.rs`

- [ ] **Step 1: Write the failing CLI test**

Add this async test near `json_outputs_metadata_and_markdown` in `tests/cli.rs`:

```rust
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
```

- [ ] **Step 2: Verify the test fails if Task 1 is not complete**

Run:

```bash
cargo test json_outputs_extended_metadata
```

Expected before Task 1: FAIL or compile failure due to missing metadata fields. Expected after Task 1: PASS without changing `src/output.rs`, because JSON output flattens `Metadata`.

- [ ] **Step 3: Run the focused CLI test**

Run:

```bash
cargo test json_outputs_extended_metadata
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/cli.rs
git commit -m "test: cover extended metadata output"
```

---

## Task 4: Full Verification

**Files:**
- Verify all modified files.

- [ ] **Step 1: Format code**

Run:

```bash
cargo fmt
```

Expected: no output unless files are reformatted.

- [ ] **Step 2: Run full test suite**

Run:

```bash
cargo test
```

Expected: all unit, integration, and doc tests pass.

- [ ] **Step 3: Inspect public text for restricted wording**

Run:

```bash
rg -n "restricted-reference-token" docs/superpowers/plans/2026-05-08-schema-backed-extraction.md
```

Expected: no matches. The literal command token is intentionally not the restricted project name.

- [ ] **Step 4: Commit final verification cleanup if needed**

If formatting or cleanup changed files:

```bash
git add src tests
git commit -m "chore: finalize schema-backed extraction"
```

If no files changed, do not create an empty commit.

---

## Self-Review

- Spec coverage: This plan covers structured metadata extraction, structured body fallback, CLI JSON coverage, and full verification.
- Placeholder scan: No task contains open-ended implementation placeholders.
- Type consistency: `Metadata.schema_org_data` serializes as `schemaOrgData`; helper names are consistent across tasks.
