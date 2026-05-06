# Rust Web-to-Markdown CLI Design

## Summary

Build an npm-distributed CLI that fetches a URL and prints clean Markdown for coding agents. The tool is built in Rust and optimized for the common agent workflow: give it a web page URL, receive readable Markdown on stdout, and keep logs, warnings, and errors off stdout.

This is not a Defuddle-compatible clone. It is an agent-first web-to-Markdown fetcher that borrows Defuddle's strongest ideas: robust fetching, metadata extraction, main-content detection, cleanup heuristics, schema.org fallback, and fixture-driven quality testing.

## Goals

- Provide a fast Rust-built CLI installable and runnable through npm.
- Make Markdown the default output for immediate use in coding-agent context.
- Keep stdout machine-friendly by emitting only the requested content.
- Support structured metadata output through JSON.
- Build a small, testable extraction pipeline that can grow toward local files and site-specific extractors later.
- Use Defuddle as a reference for practical extraction behavior without copying its CLI surface or making HTML the default.

## Non-Goals

- Local HTML file input in the MVP.
- Full Defuddle CLI compatibility.
- HTML as the default output format.
- A large set of site-specific extractors in the MVP.
- A full JavaScript API in the first release.
- A `--property` option in the MVP.

## Primary Use Case

A developer or coding agent runs:

```bash
npx <name> https://example.com/page
```

The command fetches the URL, extracts the main content, cleans noise, converts the result to Markdown, and prints that Markdown to stdout. Errors and diagnostics go to stderr so agent tooling can safely pipe stdout into a prompt, file, or downstream command.

## CLI

The MVP uses a direct URL argument and no subcommands:

```bash
npx <name> https://example.com/article
```

Initial options:

```text
--json                 Output metadata and Markdown as JSON
-o, --output <file>    Write output to a file
--max-chars <n>        Truncate Markdown to a maximum character count
--timeout <ms>         Set fetch timeout
--user-agent <ua>      Override the User-Agent header
-l, --lang <code>      Set Accept-Language
--no-images            Remove images from Markdown output
--debug                Emit extraction diagnostics and timing information
-V, --version          Print version
-h, --help             Print help
```

Markdown is the default, so there is no `--markdown` flag. HTML output can be added later as `--html` if demand appears.

## JSON Output

With `--json`, stdout contains a stable JSON object:

```json
{
  "url": "https://example.com/page",
  "finalUrl": "https://example.com/page",
  "title": "Example Page",
  "description": "",
  "site": "Example",
  "author": "",
  "published": "",
  "language": "en",
  "wordCount": 1234,
  "markdown": "# Example Page\n\n..."
}
```

Debug fields may be included when `--json --debug` is used. In plain Markdown mode, debug information should go to stderr.

## Architecture

The Rust implementation should be split into focused modules:

```text
cli -> fetcher -> html_parser -> extractor -> cleaner -> markdown -> output
```

`cli` parses arguments, configures the run, and maps errors to exit codes.

`fetcher` retrieves the URL. It should support timeout, maximum response size, redirects, content-type checks, charset detection, proxy environment variables, Accept-Language, User-Agent override, and a bot User-Agent fallback strategy inspired by Defuddle.

`html_parser` converts fetched HTML into a traversable tree. The implementation should choose Rust crates that make selector matching and DOM-like cleanup practical.

`extractor` selects the main content. It should start with generic heuristics: known article/content entry points, text density, link density, tag density, and schema.org `articleBody` or `text` fallback.

`cleaner` removes noise such as scripts, styles, navigation, footers, sidebars, ads, comments, share widgets, buttons, hidden elements, and small images. It should borrow the shape of Defuddle's selector removal, scoring, and content-pattern cleanup.

`markdown` converts the cleaned content into Markdown. MVP coverage should include headings, paragraphs, links, images, code blocks with language preservation, blockquotes, lists, and simple tables.

`output` handles stdout, file writing, JSON serialization, stderr diagnostics, and max-character truncation.

## Error Handling

The CLI should be easy for agents and shell scripts to reason about:

- Success prints Markdown or JSON to stdout and exits with code `0`.
- Failure prints a concise error to stderr and exits non-zero.
- Warnings and diagnostics never mix into stdout.

Initial exit codes:

```text
1 unknown
2 invalid_url
3 fetch_failed
4 timeout
5 too_large
6 unsupported_content_type
7 extraction_failed
8 output_failed
```

In the MVP, failed `--json` runs may still report errors through stderr and non-zero exit codes. Structured JSON errors can be considered later if downstream usage requires them.

## Defuddle Reference Points

Defuddle was fetched into `opensrc/defuddle` for local reference and is intentionally excluded from Git. The Rust tool should study and adapt these design ideas:

- Robust URL fetching with timeout, size limit, redirect handling, content-type validation, proxy support, charset detection, Accept-Language, and User-Agent handling.
- Main content entry-point selectors and fallback retries.
- Selector-based clutter removal.
- Scoring-based removal for low-value blocks.
- Content-pattern cleanup for boilerplate.
- Metadata extraction from meta tags and schema.org.
- Schema.org text fallback when DOM selection misses the article body.
- Fixture-based regression testing.

The Rust tool should not copy Defuddle's default HTML output, broad browser/library compatibility, or large initial site-specific extractor set.

## Testing

Testing should combine unit tests, integration tests, fixture tests, and CLI behavior tests.

Fixture coverage should start with developer- and documentation-heavy pages:

- MDN documentation.
- GitHub issue and pull request pages.
- Obsidian blog pages.
- Daring Fireball-style articles.
- Code block fixtures.
- Table fixtures.
- Hidden/noise fixtures.
- Schema fallback fixtures.

Early assertions should focus on behavior rather than byte-for-byte parity:

- Important content remains.
- Navigation and boilerplate are removed.
- Markdown is valid enough for agent context.
- Code block language hints are preserved.
- Links resolve correctly.
- stdout, stderr, and exit codes are predictable.

## Packaging

The npm package should prioritize CLI binary distribution. The package may use a small Node wrapper to invoke the correct platform-specific Rust binary.

Primary usage:

```bash
npx <name> https://example.com/page
npm i -D <name>
```

A JavaScript API such as `fetchMarkdown(url)` can be added later, but the MVP should optimize for the CLI experience first.

## Positioning

Working copy:

> A fast Rust-built web-to-Markdown fetcher for coding agents.

The marketing should lead with Rust and speed because modern developer tooling audiences respond well to fast native CLIs. The durable value, however, is not just speed: it is clean Markdown, robust fetch behavior, predictable shell semantics, and output designed for coding agents.

## Future Work

- Local HTML file input.
- HTML output through `--html`.
- A JavaScript API.
- Batch URL input.
- Caching.
- Token-aware truncation.
- Site-specific extractors for high-value developer sources.
- Structured JSON errors.
