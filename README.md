<img src="https://github.com/user-attachments/assets/72b469ac-cb4d-413f-a372-2b3d98370c91" />

# chidori ⚡️

A fast Rust-built web-to-Markdown fetcher for AI agents.

```bash
npx chidori-fetch https://example.com
```

`chidori` fetches a URL, extracts the main readable content, removes page noise, and prints Markdown to stdout by default. Logs and errors go to stderr so AI agents and shell scripts can safely pipe stdout into prompts, files, or other tools.

## Quick demo

Web pages are built for browsers. Agents usually need the article, not the page
chrome around it.

```html
<body>
  <nav>Docs Pricing Log in</nav>
  <article>
    <h1>How Coffee Cools</h1>
    <p>Coffee cools following Newton's law of cooling.</p>
    <pre><code>cargo test</code></pre>
  </article>
  <aside>Related posts, ads, newsletter forms...</aside>
</body>
```

```bash
npx chidori-fetch https://example.com/coffee-cooling
```

````md
# How Coffee Cools

Coffee cools following Newton's law of cooling.

```
cargo test
```
````

That output can go straight into an agent prompt, a RAG ingestion job, a note, or
another shell command:

```bash
chidori https://example.com/article | llm "summarize this for a code review"
chidori https://example.com/article --json | jq -r '.title, .markdown'
```

## Why agents use it

- **Pipeable by default:** Markdown goes to stdout. Logs and errors go to
  stderr, so scripts can trust the output stream.
- **Readable content first:** `chidori` strips navigation, footers, forms,
  hidden content, related links, script noise, and common social/page chrome.
- **Agent-friendly Markdown:** code blocks, links, images, simple tables,
  footnotes, math, and callouts are preserved in text form where possible.
- **Useful metadata:** `--json` returns the final URL, canonical URL, title,
  description, domain, language, meta tags, schema.org data, word count, and the
  extracted Markdown.
- **Fast local CLI:** it is a Rust binary with no hosted service in the loop.
  JavaScript-heavy pages can opt into your own renderer with `--render=auto`.

## How it is different

| Tool | What you usually get |
| --- | --- |
| `curl` | Raw HTML, scripts, navigation, and layout markup. |
| Browser automation | Accurate rendering, but heavier and harder to pipe through shell workflows. |
| Readability libraries | Good extraction primitives, usually embedded inside another app or runtime. |
| `chidori` | One command that fetches, extracts, cleans, converts to Markdown, and keeps stdout safe for agents. |

## Installation

```bash
npm install -g chidori-fetch
```

## Usage

```bash
chidori --help
chidori https://example.com
chidori https://example.com --json
chidori https://example.com --output article.md
chidori https://example.com --max-chars 20000
chidori https://example.com --lang ja
chidori https://example.com --no-images
chidori https://example.com --debug
CHIDORI_RENDER_COMMAND=/path/to/render-page chidori https://example.com --render=auto
```

`--json` prints metadata plus `markdown` as JSON. Metadata includes `url`,
`finalUrl`, `canonicalUrl`, `domain`, title/description fields, `metaTags`,
`schemaOrgData`, and `wordCount`. With `--json --debug`, stdout also includes a
`debug` object; human-readable debug lines still go to stderr.

`--render=auto` is an optional fallback for JavaScript-heavy pages. When static
extraction fails or only finds a short app shell, `chidori` runs
`CHIDORI_RENDER_COMMAND <url>` if configured and expects rendered HTML on stdout.
If the renderer is unavailable and no custom user agent was supplied, `chidori`
still tries its bot user-agent fallback. Renderer output uses the same timeout
and 5 MiB fetch limit.

### External renderer example

One simple renderer setup is a small Playwright script:

```bash
npm install -D playwright
npx playwright install chromium
```

Create `render-page.mjs`:

```js
#!/usr/bin/env node
import { chromium } from "playwright";

const url = process.argv[2];

if (!url) {
  console.error("Usage: render-page.mjs <url>");
  process.exit(2);
}

const browser = await chromium.launch({ headless: true });

try {
  const page = await browser.newPage();
  await page.goto(url, {
    waitUntil: "networkidle",
    timeout: 10_000,
  });

  process.stdout.write(await page.content());
} finally {
  await browser.close();
}
```

Then make it executable and pass it to `chidori`:

```bash
chmod +x render-page.mjs
CHIDORI_RENDER_COMMAND=/absolute/path/to/render-page.mjs chidori https://example.com --render=auto
```

The renderer should write only HTML to stdout. Write logs and errors to stderr so
`chidori` can safely read stdout as the rendered document.

## Design goals

AI agents need web pages as clean, pipeable Markdown. `chidori` is built for fast
CLI startup, deterministic shell behavior, and extraction that is easy to debug
when a page does something strange.
