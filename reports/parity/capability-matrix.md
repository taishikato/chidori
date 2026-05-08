# Extraction Capability Matrix

This matrix maps relevant behavior from the local reference project to current
`chidori` behavior. The curated corpus in `tools/parity-corpus.json` provides
the executable evidence for implemented rows.

| Area | Reference behavior reviewed | Chidori status | Evidence or rationale |
| --- | --- | --- | --- |
| Main article entry-point selection | Prioritizes article-like selectors, markdown bodies, GitHub discussion containers, and body fallback when needed. | Implemented | `src/extractor.rs`; `daringfireball-iphone-16e`, `obsidian-sync-encryption`, `github-pull-request`. |
| Low-word-count fallback | Retries broader body extraction when a selected entry point is too small and the body clearly has more content. | Implemented | `LOW_WORD_COUNT_RETRY_THRESHOLD`; pipeline tests for short article retry behavior. |
| Schema-backed article body fallback | Uses structured article body text when visible extraction is missing or materially shorter. | Implemented | `schema-backed-span-blocks`; `structured_content_candidate`. |
| Navigation, footer, aside, form, button cleanup | Removes common non-content containers before scoring and Markdown conversion. | Implemented | `src/cleaner.rs`; `daringfireball-iphone-16e`, `obsidian-sync-encryption`. |
| Hidden content cleanup | Removes hidden nodes and visibility-hidden fallback content. | Implemented | `hidden-visibility-cleanup`; `removes_hidden_and_embedded_noise_elements`. |
| Embedded fallback cleanup | Removes iframe/object/embed fallback text that pollutes Markdown. | Implemented | `hidden-visibility-cleanup`. |
| JavaScript pseudo-link cleanup | Unwraps `javascript:` anchors while preserving inner formatting and real links. | Implemented | `javascript-link-unwrapping`; `unwraps_javascript_links_without_losing_inner_content`. |
| Table-of-contents cleanup | Removes fragment-only link lists that duplicate headings. | Implemented | `table-of-contents-cleanup`; `removes_fragment_only_table_of_contents_lists`. |
| Breadcrumb cleanup | Removes non-semantic breadcrumb blocks injected into content areas. | Implemented | `leading-breadcrumb-cleanup`; `removes_breadcrumb_blocks_without_semantic_nav_tags`. |
| Related-link section cleanup | Removes short, link-dense related-post sections without dropping prose sections. | Implemented | `trailing-related-links-cleanup`; `removes_link_dense_related_sections`. |
| Code block language preservation | Preserves language labels from common code class and data attributes. | Implemented | `rehype-pretty-copy-code`; code-fence language pipeline tests. |
| Copy-button/script noise in code blocks | Removes copy controls and scripts without dropping fenced code. | Implemented | `rehype-pretty-copy-code`. |
| Core metadata extraction | Extracts title, description, author, published date, site, language, favicon, image, and JSON-LD. | Implemented | `tests/pipeline.rs` metadata tests; CLI JSON tests. |
| Parameterized JSON-LD script types | Accepts JSON-LD content types with charset parameters. | Implemented | `extracts_schema_org_data_from_parameterized_json_ld_type`. |
| Markdown output normalization | Normalizes setext headings and trims unstable whitespace while preserving code blocks. | Implemented | Markdown pipeline tests. |
| URL fetching: redirects, charset, compression, timeout, user agent, language | Uses reqwest with charset, gzip, brotli, deflate, rustls TLS, timeout, UA, and Accept-Language support. | Implemented | `src/fetcher.rs`; `tests/fetcher.rs`; CLI tests. |
| Raw Markdown extraction from bot-only pages | Reference can retry some URLs with a bot UA and extract embedded raw Markdown. | Deferred | Live network behavior is not part of deterministic curated corpus yet. Closest current behavior is configurable `--user-agent` and `--lang`. Add a fixture when a saved raw-Markdown page is admitted. |
| Domain-specific social/comment extractors | Reference includes specialized extractors for Reddit, Hacker News, X/Twitter, Mastodon, YouTube, and others. | Deferred | Current milestone prioritizes generic web article extraction and GitHub discussion preservation. Add individual deterministic fixtures before implementing each specialized extractor. |
| Math, callout, and footnote specialization | Reference has richer element-specific transforms. | Deferred | Not required by the current curated corpus. Track as Markdown fidelity expansion after main content and cleanup remain stable. |
| Browser/runtime-specific DOM compatibility | Reference supports browser-oriented DOM implementations. | Not applicable | `chidori` is a Rust CLI with `scraper`; parity target is output behavior, not DOM runtime APIs. |
| NPM library API compatibility | Reference exposes a JS package API in addition to CLI behavior. | Not applicable | `chidori` preserves its Rust CLI architecture with a thin npm binary wrapper. |
