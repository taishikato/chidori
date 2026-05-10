<img src="https://github.com/user-attachments/assets/72b469ac-cb4d-413f-a372-2b3d98370c91" />

# chidori ⚡️

A fast Rust-built web-to-Markdown fetcher for AI agents.

```bash
npx chidori-fetch https://example.com
```

`chidori` fetches a URL, extracts the main readable content, removes page noise, and prints Markdown to stdout by default. Logs and errors go to stderr so AI agents and shell scripts can safely pipe stdout into prompts, files, or other tools.

## Installation

```bash
npm install -g chidori-fetch
```

## Usage

```bash
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
`CHIDORI_RENDER_COMMAND <url>` and expects rendered HTML on stdout. Renderer
output uses the same timeout and 5 MiB fetch limit.

## Why

AI agents need web pages as clean, pipeable Markdown. `chidori` is built in Rust for fast CLI startup and predictable shell behavior in automated workflows.
